pub mod data;
pub mod report;
use crate::{Collection, Document, NetworkSettings, Response, prepare};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunOptions {
    pub environment: Option<String>,
    pub folder: Option<PathBuf>,
    pub iterations: u32,
    pub delay_ms: u64,
    pub stop_on_failure: bool,
    #[serde(default)]
    pub data: Vec<serde_json::Value>,
}
impl Default for RunOptions {
    fn default() -> Self {
        Self {
            environment: None,
            folder: None,
            iterations: 1,
            delay_ms: 0,
            stop_on_failure: false,
            data: vec![],
        }
    }
}
impl RunOptions {
    pub fn new(
        environment: Option<String>,
        folder: Option<PathBuf>,
        iterations: u32,
        delay_ms: u64,
        stop_on_failure: bool,
    ) -> Self {
        Self {
            environment,
            folder,
            iterations,
            delay_ms,
            stop_on_failure,
            data: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunEntry {
    pub iteration: u32,
    #[serde(default)]
    pub data_row: Option<usize>,
    #[serde(default)]
    pub elapsed_ms: u128,
    pub path: PathBuf,
    pub name: String,
    pub passed: bool,
    pub skipped: bool,
    pub response: Option<Response>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunReport {
    pub entries: Vec<RunEntry>,
    pub elapsed_ms: u128,
    pub cancelled: bool,
    #[serde(default)]
    pub planned: usize,
}
impl RunReport {
    pub fn passed(&self) -> bool {
        !self.cancelled && self.entries.iter().all(|e| e.passed)
    }
}
pub fn ordered_requests(collection: &Collection, folder: Option<&Path>) -> Vec<Document> {
    fn visit(collection: &Collection, dir: &Path, result: &mut Vec<Document>) {
        let mut nodes: Vec<(f64, PathBuf, Option<Document>)> = vec![];
        let mut dirs = std::collections::BTreeSet::new();
        for doc in collection.documents() {
            let Some(parent) = doc.path().parent() else {
                continue;
            };
            if parent == dir && doc.is_request() {
                nodes.push((
                    doc.value()["info"]["seq"].as_f64().unwrap_or(0.),
                    doc.path().to_owned(),
                    Some(doc.clone()),
                ));
            }
            if let Ok(rest) = parent.strip_prefix(dir)
                && let Some(component) = rest.components().next()
            {
                dirs.insert(dir.join(component));
            }
        }
        for folder in dirs {
            let seq = collection
                .documents()
                .iter()
                .find(|d| {
                    d.kind() == crate::DocumentKind::Folder
                        && d.path().parent() == Some(folder.as_path())
                })
                .and_then(|d| d.value()["info"]["seq"].as_f64())
                .unwrap_or(0.);
            nodes.push((seq, folder, None));
        }
        nodes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, path, document) in nodes {
            if let Some(document) = document {
                result.push(document);
            } else {
                visit(collection, &path, result);
            }
        }
    }
    let mut result = vec![];
    visit(collection, folder.unwrap_or(Path::new("")), &mut result);
    result
}
pub async fn run(
    collection: Collection,
    options: RunOptions,
    network: NetworkSettings,
    cancel: Arc<AtomicBool>,
    events: Option<tokio::sync::mpsc::UnboundedSender<RunEntry>>,
) -> Result<RunReport> {
    ensure!(
        options.iterations > 0 && options.iterations <= 10000,
        "迭代次数必须在 1–10000 之间"
    );
    let requests = ordered_requests(&collection, options.folder.as_deref());
    ensure!(!requests.is_empty(), "没有可运行的请求");
    data::validate(&options.data)?;
    ensure!(options.delay_ms <= 3_600_000, "延迟不能超过 3600000 毫秒");
    let rows = options.data.len().max(1);
    let planned = requests
        .len()
        .checked_mul(options.iterations as usize)
        .and_then(|count| count.checked_mul(rows))
        .filter(|count| *count <= 10000)
        .ok_or_else(|| anyhow::anyhow!("总执行次数不能超过 10000"))?;
    let start = Instant::now();
    let mut entries = vec![];
    let mut runtime_variables = json!({});
    'iterations: for iteration in 1..=options.iterations {
        for row_index in 0..rows {
            let data_row = (!options.data.is_empty()).then_some(row_index + 1);
            if data_row.is_some() {
                runtime_variables = json!({});
            }
            for document in &requests {
                let request_start = Instant::now();
                if cancel.load(Ordering::Relaxed) {
                    break 'iterations;
                }
                if document.kind() == crate::DocumentKind::WebSocket {
                    let entry = RunEntry {
                        iteration,
                        data_row,
                        elapsed_ms: request_start.elapsed().as_millis(),
                        path: document.path().to_owned(),
                        name: document.name().into(),
                        passed: true,
                        skipped: true,
                        response: None,
                        error: None,
                    };
                    if let Some(events) = &events {
                        let _ = events.send(entry.clone());
                    }
                    entries.push(entry);
                    continue;
                }
                let result = match prepare(
                    &collection,
                    document,
                    options.environment.as_deref(),
                    network.clone(),
                ) {
                    Ok(mut execution) => {
                        execution.cancel = cancel.clone();
                        if let Some(row) = options
                            .data
                            .get(row_index)
                            .and_then(serde_json::Value::as_object)
                        {
                            for (key, value) in row {
                                execution.variables[key] = value.clone();
                            }
                        }
                        for (key, value) in runtime_variables.as_object().into_iter().flatten() {
                            execution.variables[key] = value.clone();
                        }
                        crate::engine::execute_with_variables(execution, None).await
                    }
                    Err(e) => Err(e),
                };
                let entry = match result {
                    Ok((response, variables)) => {
                        for (key, value) in variables.as_object().into_iter().flatten() {
                            runtime_variables[key] = value.clone();
                        }
                        RunEntry {
                            iteration,
                            data_row,
                            elapsed_ms: request_start.elapsed().as_millis(),
                            path: document.path().to_owned(),
                            name: document.name().into(),
                            passed: response.tests.iter().all(|t| t.passed),
                            skipped: false,
                            response: Some(response),
                            error: None,
                        }
                    }
                    Err(error) => RunEntry {
                        iteration,
                        data_row,
                        elapsed_ms: request_start.elapsed().as_millis(),
                        path: document.path().to_owned(),
                        name: document.name().into(),
                        passed: false,
                        skipped: false,
                        response: None,
                        error: Some(format!("{error:#}")),
                    },
                };
                let failed = !entry.passed;
                if let Some(events) = &events {
                    let _ = events.send(entry.clone());
                }
                entries.push(entry);
                if options.stop_on_failure && failed {
                    break 'iterations;
                }
                if options.delay_ms > 0 {
                    let until = Instant::now() + Duration::from_millis(options.delay_ms);
                    while Instant::now() < until && !cancel.load(Ordering::Relaxed) {
                        tokio::time::sleep(
                            Duration::from_millis(20)
                                .min(until.saturating_duration_since(Instant::now())),
                        )
                        .await;
                    }
                }
            }
        }
    }
    Ok(RunReport {
        planned,
        entries,
        elapsed_ms: start.elapsed().as_millis(),
        cancelled: cancel.load(Ordering::Relaxed),
    })
}
