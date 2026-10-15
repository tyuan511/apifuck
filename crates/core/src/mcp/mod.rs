//! Native desktop MCP. Transport is independent of GPUI; calls are delivered to
//! the workbench so workspace/draft checks and desktop synchronization are atomic.
pub mod guides;
mod operations;
mod tools;
mod transport;

use crate::{NetworkSettings, settings};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot, watch};

pub use tools::mutates_workspace;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 47831,
            token: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}
impl Config {
    pub fn path() -> Result<PathBuf> {
        Ok(settings::app_state_path()?.with_file_name("mcp.json"))
    }
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }
    fn validate(&self) -> Result<()> {
        ensure!(self.port > 0, "MCP port must be between 1 and 65535");
        ensure!(
            self.token.len() >= 32 && self.token.bytes().all(|c| c.is_ascii_alphanumeric()),
            "Invalid MCP token: use at least 32 alphanumeric characters"
        );
        Ok(())
    }
}
#[derive(Clone)]
pub struct Status {
    pub config: Config,
    pub running: bool,
    pub error: Option<String>,
}

/// A snapshot supplied by the UI thread for each call, never by the remote client.
#[derive(Default)]
pub struct Workspace {
    pub root: Option<PathBuf>,
    pub environment: Option<String>,
    pub network: NetworkSettings,
    pub has_drafts: bool,
    pub busy: bool,
}

pub struct Call {
    pub name: String,
    pub arguments: Value,
    pub generation: u64,
    pub reply: oneshot::Sender<Result<Value, String>>,
}
struct Running {
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
struct State {
    config: Config,
    running: Option<Running>,
    error: Option<String>,
    generation: u64,
}

pub(super) struct Job {
    value: Value,
    cancel: Arc<AtomicBool>,
    socket: Option<mpsc::UnboundedSender<crate::websocket::Command>>,
}

pub struct Service {
    path: PathBuf,
    runtime: tokio::runtime::Handle,
    state: Mutex<State>,
    calls: mpsc::Sender<Call>,
    jobs: Arc<Mutex<BTreeMap<String, Job>>>,
    control: tokio::sync::Mutex<()>,
}
impl Service {
    /// Construct without starting a listener. Register the receiver before start.
    pub fn new(
        path: PathBuf,
        runtime: tokio::runtime::Handle,
    ) -> Result<(Arc<Self>, mpsc::Receiver<Call>)> {
        let config = match std::fs::read_to_string(&path) {
            Ok(source) => {
                serde_json::from_str::<Config>(&source).context("Invalid MCP configuration")?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                crate::collection::atomic_write(&path, &serde_json::to_string_pretty(&config)?)?;
                config
            }
            Err(error) => return Err(error.into()),
        };
        config.validate()?;
        let (calls, receiver) = mpsc::channel(32);
        Ok((
            Arc::new(Self {
                path,
                runtime,
                state: Mutex::new(State {
                    config,
                    running: None,
                    error: None,
                    generation: 0,
                }),
                calls,
                jobs: Default::default(),
                control: Default::default(),
            }),
            receiver,
        ))
    }
    pub fn status(&self) -> Status {
        let state = self.state.lock().unwrap();
        Status {
            config: state.config.clone(),
            running: state
                .running
                .as_ref()
                .is_some_and(|run| !run.task.is_finished()),
            error: state.error.clone(),
        }
    }
    pub fn accepts(&self, call: &Call) -> bool {
        let state = self.state.lock().unwrap();
        state.running.is_some() && state.generation == call.generation && !call.reply.is_closed()
    }
    pub fn configure(self: &Arc<Self>, enabled: bool) -> tokio::task::JoinHandle<Result<()>> {
        let service = self.clone();
        self.runtime
            .spawn(async move { service.set_enabled(enabled).await })
    }
    pub async fn set_enabled(&self, enabled: bool) -> Result<()> {
        let _control = self.control.lock().await;
        let result = self.configure_inner(enabled).await;
        self.state.lock().unwrap().error = result.as_ref().err().map(|e| format!("{e:#}"));
        result
    }
    async fn configure_inner(&self, enabled: bool) -> Result<()> {
        let status = self.status();
        if enabled && status.running {
            return Ok(());
        }
        let listener = if enabled {
            Some(
                tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, status.config.port))
                    .await
                    .context("Cannot bind MCP port")?,
            )
        } else {
            None
        };
        let config = Config {
            enabled,
            ..status.config
        };
        crate::collection::atomic_write(&self.path, &serde_json::to_string_pretty(&config)?)?;
        let previous = {
            let mut state = self.state.lock().unwrap();
            state.config = config.clone();
            state.generation = state.generation.wrapping_add(1);
            state.running.take()
        };
        if let Some(run) = previous {
            let _ = run.stop.send(true);
            run.task.abort();
            // Wait for the socket to close before allowing an immediate restart.
            let _ = run.task.await;
        }
        if let Some(listener) = listener {
            let (stop, stopped) = watch::channel(false);
            let mut state = self.state.lock().unwrap();
            let router = transport::router(config, self.calls.clone(), stopped, state.generation);
            let task = self.runtime.spawn(async move {
                if let Err(error) = axum::serve(listener, router).await {
                    eprintln!("MCP: {error:#}");
                }
            });
            state.running = Some(Running { stop, task });
        } else {
            self.cancel_all();
        }
        Ok(())
    }
    /// Stop on window/application teardown without changing the saved preference.
    pub fn shutdown(&self) {
        if let Some(run) = self.state.lock().unwrap().running.take() {
            let _ = run.stop.send(true);
            run.task.abort();
        }
        self.cancel_all();
    }
    fn cancel_all(&self) {
        for job in self.jobs.lock().unwrap().values() {
            job.cancel.store(true, Ordering::Relaxed);
            if let Some(socket) = &job.socket {
                let _ = socket.send(crate::websocket::Command::Close);
            }
        }
    }
    pub fn tool_count() -> usize {
        tools::list().len()
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.shutdown();
    }
}
