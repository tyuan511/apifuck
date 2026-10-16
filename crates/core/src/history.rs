//! Bounded, project-local execution history. Never stores request headers or bodies.
use crate::{Execution, Response, collection::atomic_write};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_ENTRIES: usize = 200;
const PREVIEW_BYTES: usize = 32 * 1024;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp_ms: u64,
    pub path: PathBuf,
    pub name: String,
    pub environment: Option<String>,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub elapsed_ms: u128,
    pub bytes: usize,
    pub passed: Option<bool>,
    pub cancelled: bool,
    pub error: Option<String>,
    pub body_preview: String,
    pub truncated: bool,
}
fn clip(value: &str, max: usize) -> String {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
fn safe_url(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return String::new();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_fragment(None);
    let keys: Vec<_> = url.query_pairs().map(|(key, _)| key.into_owned()).collect();
    url.set_query(None);
    if !keys.is_empty() {
        url.query_pairs_mut()
            .extend_pairs(keys.iter().map(|key| (key.as_str(), "[redacted]")));
    }
    clip(url.as_str(), 4096)
}
impl HistoryEntry {
    pub(crate) fn execution(
        execution: &Execution,
        response: Option<&Response>,
        error: Option<&anyhow::Error>,
        elapsed_ms: u128,
    ) -> Self {
        let url = crate::engine::headers_and_url(&execution.request)
            .ok()
            .map(|(url, _)| url.to_string())
            .unwrap_or_default();
        let body = response
            .filter(|r| r.body_base64.is_none())
            .map(|r| r.body.as_str())
            .unwrap_or("");
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            path: execution.path.clone(),
            name: clip(&execution.name, 512),
            environment: execution.environment.clone(),
            method: execution.request["method"]
                .as_str()
                .unwrap_or(if execution.websocket { "WS" } else { "GET" })
                .to_owned(),
            url: safe_url(&url),
            status: response.map(|r| r.status),
            elapsed_ms,
            bytes: response.map(|r| r.bytes).unwrap_or(0),
            passed: response.map(|r| r.tests.iter().all(|t| t.passed)),
            cancelled: execution.cancel.load(std::sync::atomic::Ordering::Relaxed),
            error: error.map(|e| clip(&e.to_string(), 2048)),
            body_preview: clip(body, PREVIEW_BYTES),
            truncated: body.len() > PREVIEW_BYTES,
        }
    }
}
fn lock(root: &Path) -> Result<File> {
    let directory = root.join(".apifuck");
    fs::create_dir_all(&directory)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("history.lock"))?;
    file.lock()?;
    Ok(file)
}
pub fn load(root: &Path) -> Result<Vec<HistoryEntry>> {
    let path = root.join(".apifuck/history.json");
    match fs::metadata(&path) {
        Ok(metadata) => ensure!(metadata.len() <= MAX_FILE_BYTES, "请求历史文件过大"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    }
    let entries: Vec<HistoryEntry> =
        serde_json::from_slice(&fs::read(path)?).context("无法读取请求历史")?;
    ensure!(entries.len() <= MAX_ENTRIES, "请求历史记录过多");
    Ok(entries)
}
pub fn append(root: &Path, entry: HistoryEntry) -> Result<()> {
    let _lock = lock(root)?;
    let mut entries = load(root)?;
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);
    atomic_write(
        &root.join(".apifuck/history.json"),
        &serde_json::to_string(&entries)?,
    )
}
pub fn clear(root: &Path) -> Result<()> {
    let _lock = lock(root)?;
    atomic_write(&root.join(".apifuck/history.json"), "[]")
}
pub(crate) async fn record(
    execution: &Execution,
    response: Option<&Response>,
    error: Option<&anyhow::Error>,
    elapsed_ms: u128,
) -> Result<()> {
    let entry = HistoryEntry::execution(execution, response, error, elapsed_ms);
    let root = execution.root.clone();
    tokio::task::spawn_blocking(move || append(&root, entry)).await?
}

pub fn timestamp(milliseconds: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp((milliseconds / 1000) as i64)
        .ok()
        .and_then(|time| {
            time.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(id: usize) -> HistoryEntry {
        HistoryEntry {
            id: id.to_string(),
            timestamp_ms: 0,
            path: "test.yml".into(),
            name: "Test".into(),
            environment: None,
            method: "GET".into(),
            url: "http://localhost/".into(),
            status: Some(200),
            elapsed_ms: 1,
            bytes: 0,
            passed: Some(true),
            cancelled: false,
            error: None,
            body_preview: String::new(),
            truncated: false,
        }
    }
    #[test]
    fn history_survives_reload_is_bounded_and_can_be_cleared() {
        let directory = tempfile::tempdir().unwrap();
        for id in 0..MAX_ENTRIES + 2 {
            append(directory.path(), entry(id)).unwrap();
        }
        let entries = load(directory.path()).unwrap();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].id, "201");
        assert_eq!(entries.last().unwrap().id, "2");
        clear(directory.path()).unwrap();
        assert!(load(directory.path()).unwrap().is_empty());
    }
    #[test]
    fn concurrent_appends_preserve_records_and_corruption_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            for id in 0..20 {
                let root = directory.path();
                scope.spawn(move || append(root, entry(id)).unwrap());
            }
        });
        assert_eq!(load(directory.path()).unwrap().len(), 20);
        let path = directory.path().join(".apifuck/history.json");
        fs::write(&path, "invalid").unwrap();
        assert!(append(directory.path(), entry(21)).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "invalid");
        clear(directory.path()).unwrap();
    }
    #[test]
    fn urls_drop_credentials_fragments_and_query_values_and_previews_respect_unicode() {
        let url = safe_url("https://user:secret@example.com/path?token=secret&a=one&a=two#secret");
        assert!(!url.contains("secret"));
        assert!(!url.contains("user"));
        assert_eq!(url::Url::parse(&url).unwrap().query_pairs().count(), 3);
        assert_eq!(clip("中文", 4), "中");
        assert_eq!(safe_url("{{secret}}"), "");
    }
}
