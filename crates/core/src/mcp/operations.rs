use super::{Job, Service, Workspace, tools};
use crate::{
    Collection, Document,
    collection::{portable_path, template},
    editing, engine, exchange, runner,
    settings::RecentProjects,
    websocket,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::mpsc;

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args[key].as_str().with_context(|| format!("Missing {key}"))
}
fn parent(args: &Value) -> Result<&Path> {
    let path = Path::new(args["parent"].as_str().unwrap_or(""));
    if !path.as_os_str().is_empty() {
        public_path(path)?;
    }
    Ok(path)
}
fn public_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path.components().all(|part| match part {
                Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
                _ => false,
            }),
        "Path must be inside the collection and outside hidden application state"
    );
    Ok(())
}
fn document(doc: &Document) -> Value {
    json!({"path":portable_path(doc.path()),"name":doc.name(),"kind":doc.kind(),"source":doc.source(),"value":doc.value()})
}
fn expected(doc: &Document, args: &Value) -> Result<()> {
    ensure!(
        doc.source() == text(args, "expectedSource")?,
        "Document changed since it was read. Read it again before writing."
    );
    Ok(())
}
fn environment(args: &Value, context: &Workspace) -> Result<Option<String>> {
    Ok(match args.get("environment") {
        None => context.environment.clone(),
        Some(Value::Null) => None,
        Some(value) => Some(value.as_str().context("Invalid environment")?.into()),
    })
}
impl Service {
    /// Called on the desktop thread, after capturing live draft/running state.
    /// This same operation boundary is tested headlessly using temporary projects.
    pub fn call(&self, name: &str, args: &Value, context: &mut Workspace) -> Result<Value> {
        tools::validate(name, args)?;
        if tools::mutates_workspace(name) {
            ensure!(
                !context.has_drafts,
                "Save or discard unsaved desktop drafts before modifying the workspace through MCP"
            );
            ensure!(
                !context.busy,
                "Wait for desktop requests to finish before modifying the workspace through MCP"
            );
            ensure!(
                !self.has_running_jobs(),
                "Wait for MCP jobs to finish or cancel them before modifying the workspace"
            );
        }
        match name {
            "list_projects" => {
                return Ok(serde_json::to_value(
                    RecentProjects::load_from(&self.path.with_file_name("recent.json"))?.entries(),
                )?);
            }
            "open_project" | "create_project" => {
                let root = PathBuf::from(text(args, "path")?);
                let collection = if name == "create_project" {
                    Collection::create(root, text(args, "name")?)?
                } else {
                    Collection::open(root)?
                };
                // Validate recoverable local state before switching the desktop.
                let local = crate::settings::WorkspaceState::load(collection.root())?;
                context.root = Some(collection.root().to_owned());
                context.environment = local.environment;
                return Ok(json!({"path":collection.root(),"name":collection.project().name()}));
            }
            "get_job" => {
                return self
                    .jobs
                    .lock()
                    .unwrap()
                    .get(text(args, "id")?)
                    .map(|job| job.value.clone())
                    .context("Unknown or expired job");
            }
            "cancel_job" => {
                let mut jobs = self.jobs.lock().unwrap();
                let job = jobs
                    .get_mut(text(args, "id")?)
                    .context("Unknown or expired job")?;
                job.cancel.store(true, Ordering::Relaxed);
                if let Some(socket) = &job.socket {
                    let _ = socket.send(websocket::Command::Close);
                }
                return Ok(json!({"cancellationRequested":true}));
            }
            "send_websocket_message" => {
                let jobs = self.jobs.lock().unwrap();
                let job = jobs.get(text(args, "id")?).context("Unknown job")?;
                ensure!(
                    job.value["status"] == "running" && job.value["connected"] == true,
                    "WebSocket is not connected"
                );
                job.socket
                    .as_ref()
                    .context("Job is not a WebSocket")?
                    .send(websocket::Command::Send(text(args, "message")?.into()))?;
                return Ok(json!({"sent":true}));
            }
            "convert_bru" => {
                return Ok(
                    json!({"source":crate::bru::convert(text(args,"source")?,text(args,"kind")?,text(args,"direction")?)?}),
                );
            }
            _ => {}
        }
        let collection = Collection::open(
            context
                .root
                .as_ref()
                .context("Open a project in ApiFuck first")?,
        )?;
        let result = match name {
            "get_workspace" => {
                json!({"path":collection.root(),"name":collection.project().name(),"environment":context.environment,"environments":collection.environment_names(),"documents":collection.documents().iter().map(|doc|json!({"path":portable_path(doc.path()),"name":doc.name(),"kind":doc.kind(),"method":doc.method()})).collect::<Vec<_>>()})
            }
            "read_document" => document(collection.find(text(args, "selector")?)?),
            "create_request" => document(&collection.create_request(
                parent(args)?,
                text(args, "protocol")?,
                text(args, "name")?,
            )?),
            "create_folder" => {
                document(&collection.create_folder(parent(args)?, text(args, "name")?)?)
            }
            "create_environment" => document(&collection.create_environment(text(args, "name")?)?),
            "update_document" | "save_document_source" => {
                let doc = collection.find(text(args, "selector")?)?;
                expected(doc, args)?;
                let source = if name == "update_document" {
                    editing::update(doc.source(), &args["value"])?
                } else {
                    text(args, "source")?.into()
                };
                document(&collection.save(doc, &source)?)
            }
            "duplicate_request" => {
                let doc = collection.find(text(args, "selector")?)?;
                ensure!(
                    doc.is_request(),
                    "Only requests can be duplicated with this tool"
                );
                let mut value = doc.value().clone();
                value["info"]["name"] = json!(text(args, "name")?);
                let source = editing::update(doc.source(), &value)?;
                document(&collection.add(
                    &parent(args)?.join(format!("api_{}.yml", uuid::Uuid::new_v4())),
                    &source,
                )?)
            }
            "move_document" => {
                let doc = collection.find(text(args, "selector")?)?;
                expected(doc, args)?;
                let target = Path::new(text(args, "target")?);
                public_path(target)?;
                collection.rename(doc, target)?;
                json!({"path":portable_path(target)})
            }
            "trash_document" => {
                let doc = collection.find(text(args, "selector")?)?;
                expected(doc, args)?;
                collection.trash(doc)?;
                json!({"trashed":true})
            }
            "reorder_document" => {
                collection.reorder_before(
                    collection.find(text(args, "selector")?)?,
                    collection.find(text(args, "before")?)?,
                )?;
                json!({"reordered":true})
            }
            "set_environment" => {
                let selected = environment(args, context)?;
                collection.environment(selected.as_deref())?;
                context.environment = selected;
                json!({"environment":context.environment})
            }
            "preview_variables" => engine::preview_variables(
                &collection,
                collection.find(text(args, "selector")?)?,
                environment(args, context)?.as_deref(),
            )?,
            "send_request" | "connect_websocket" | "export_request" => {
                let execution = crate::prepare(
                    &collection,
                    collection.find(text(args, "selector")?)?,
                    environment(args, context)?.as_deref(),
                    context.network.clone(),
                )?;
                if name == "export_request" {
                    json!({"code":exchange::export(&execution,text(args,"language")?)?})
                } else {
                    ensure!(
                        execution.is_websocket() == (name == "connect_websocket"),
                        "Use connect_websocket for WebSocket, send_request for HTTP/GraphQL/SSE"
                    );
                    self.start_execution(execution)?
                }
            }
            "import_curl" => {
                let path = parent(args)?.join(format!("api_{}.yml", uuid::Uuid::new_v4()));
                let source = exchange::import_curl_into(
                    &template("http", text(args, "name")?)?,
                    text(args, "command")?,
                    text(args, "name")?,
                )?;
                document(&collection.add(&path, &source)?)
            }
            "import_document" => {
                let path = Path::new(text(args, "path")?);
                public_path(path)?;
                ensure!(
                    matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("yaml" | "yml")
                    ),
                    "Expected a YAML document path"
                );
                document(&collection.add(path, text(args, "source")?)?)
            }
            "run_collection" => {
                let options = runner::RunOptions::new(
                    environment(args, context)?,
                    args["folder"].as_str().map(PathBuf::from),
                    args["iterations"].as_u64().unwrap_or(1) as u32,
                    args["delayMs"].as_u64().unwrap_or(0),
                    args["stopOnFailure"].as_bool().unwrap_or(false),
                );
                if let Some(folder) = &options.folder {
                    public_path(folder)?;
                    collection.safe_path(folder)?;
                }
                let cancel = Arc::new(AtomicBool::new(false));
                let id = self.insert_job(cancel.clone(), None)?;
                let jobs = self.jobs.clone();
                let job_id = id.clone();
                let network = context.network.clone();
                self.runtime.spawn(async move {
                    let result = runner::run(collection, options, network, cancel, None)
                        .await
                        .and_then(|report| Ok(serde_json::to_value(report)?));
                    finish(&jobs, &job_id, result);
                });
                json!({"id":id,"status":"running"})
            }
            _ => bail!("Unknown MCP tool"),
        };
        Ok(result)
    }
    fn has_running_jobs(&self) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .any(|job| job.value["status"] == "running")
    }
    fn insert_job(
        &self,
        cancel: Arc<AtomicBool>,
        socket: Option<mpsc::UnboundedSender<websocket::Command>>,
    ) -> Result<String> {
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.len() >= 32 {
            let old = jobs
                .iter()
                .find(|(_, job)| job.value["status"] != "running")
                .map(|(id, _)| id.clone())
                .context("Maximum 32 active MCP jobs")?;
            jobs.remove(&old);
        }
        let id = uuid::Uuid::new_v4().to_string();
        jobs.insert(
            id.clone(),
            Job {
                value: json!({"id":id,"status":"running","connected":false,"events":[]}),
                cancel,
                socket,
            },
        );
        Ok(id)
    }
    fn start_execution(&self, execution: crate::Execution) -> Result<Value> {
        let cancel = execution.cancellation();
        let (socket, commands) = mpsc::unbounded_channel();
        let is_websocket = execution.is_websocket();
        let id = self.insert_job(cancel, is_websocket.then_some(socket))?;
        let (events, mut incoming) = mpsc::unbounded_channel();
        let jobs = self.jobs.clone();
        let job_id = id.clone();
        self.runtime.spawn(async move {
            let worker = tokio::spawn(async move {
                if is_websocket {
                    websocket::connect(execution, commands, events)
                        .await
                        .map(|()| json!({"closed":true}))
                } else {
                    crate::execute(execution, Some(events))
                        .await
                        .and_then(|response| Ok(serde_json::to_value(response)?))
                }
            });
            while let Some(mut event) = incoming.recv().await {
                match &mut event {
                    crate::Event::Chunk { text, .. } | crate::Event::Message { text, .. }
                        if text.len() > 65536 =>
                    {
                        *text = text.chars().take(16384).collect()
                    }
                    _ => {}
                }
                let mut jobs = jobs.lock().unwrap();
                if let Some(job) = jobs.get_mut(&job_id) {
                    if matches!(event, crate::Event::Connected) {
                        job.value["connected"] = json!(true)
                    }
                    let buffer = job.value["events"].as_array_mut().unwrap();
                    buffer.push(serde_json::to_value(event).unwrap());
                    if buffer.len() > 100 {
                        buffer.remove(0);
                    }
                }
            }
            let result = worker
                .await
                .context("Request task failed")
                .and_then(|result| result);
            finish(&jobs, &job_id, result);
        });
        Ok(json!({"id":id,"status":"running"}))
    }
}
fn finish(
    jobs: &std::sync::Mutex<std::collections::BTreeMap<String, Job>>,
    id: &str,
    result: Result<Value>,
) {
    if let Some(job) = jobs.lock().unwrap().get_mut(id) {
        job.value["connected"] = json!(false);
        job.socket = None;
        job.value["status"] = json!(if job.cancel.load(Ordering::Relaxed) {
            "cancelled"
        } else if result.is_ok() {
            "completed"
        } else {
            "failed"
        });
        match result {
            Ok(value) => job.value["result"] = value,
            Err(error) => job.value["error"] = json!(format!("{error:#}")),
        }
    }
}
