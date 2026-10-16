//! Offline, validated collection imports. Input never supplies filesystem paths.
mod openapi;
mod postman;
use crate::{Collection, Document, collection::atomic_write};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone)]
pub struct ImportPlan {
    pub name: String,
    pub format: String,
    pub warnings: Vec<String>,
    documents: Vec<(PathBuf, String)>,
}
impl ImportPlan {
    pub fn request_count(&self) -> usize {
        self.documents
            .iter()
            .filter(|(path, _)| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("api_"))
            })
            .count()
    }
    pub fn documents(&self) -> &[(PathBuf, String)] {
        &self.documents
    }
    fn push(&mut self, path: PathBuf, mut value: Value) -> Result<()> {
        for key in ["http", "graphql", "request"] {
            if let Some(object) = value.get_mut(key).and_then(Value::as_object_mut)
                && object.get("auth").is_some_and(Value::is_null)
            {
                object.remove("auth");
            }
        }
        ensure!(self.documents.len() < 10000, "导入文档数量超过 10000");
        let source = serde_yaml::to_string(&value)?;
        Document::parse(path.clone(), source.clone())
            .with_context(|| format!("导入文档校验失败：{}", path.display()))?;
        self.documents.push((path, source));
        Ok(())
    }
    fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        if !self.warnings.contains(&message) {
            self.warnings.push(message);
        }
    }
    /// Create a uniquely named directory; failures clean up only our private staging directory.
    pub fn write_new(&self, parent: &Path) -> Result<PathBuf> {
        ensure!(parent.is_dir(), "请选择已存在的目标目录");
        let staging = tempfile::Builder::new()
            .prefix("collection_")
            .tempdir_in(parent)?;
        for (path, source) in &self.documents {
            atomic_write(&staging.path().join(path), source)?;
        }
        atomic_write(&staging.path().join(".gitignore"), ".apifuck/\n")?;
        if !self.warnings.is_empty() {
            atomic_write(
                &staging.path().join("import-notes.txt"),
                &self.warnings.join("\n"),
            )?;
        }
        Collection::open(staging.path())?;
        Ok(staging.keep())
    }
}
pub fn read(path: &Path) -> Result<ImportPlan> {
    ensure!(
        std::fs::metadata(path)?.len() <= MAX_SOURCE_BYTES as u64,
        "导入文件不能超过 16 MiB"
    );
    parse(&std::fs::read_to_string(path)?)
}
pub fn parse(source: &str) -> Result<ImportPlan> {
    ensure!(source.len() <= MAX_SOURCE_BYTES, "导入文件不能超过 16 MiB");
    let value: Value = serde_yaml::from_str(source).context("无法解析 JSON/YAML 导入文件")?;
    let mut plan = ImportPlan {
        name: String::new(),
        format: String::new(),
        warnings: vec![],
        documents: vec![],
    };
    if value.get("openapi").is_some() || value.get("swagger").is_some() {
        openapi::convert(&value, &mut plan)?;
    } else if value.get("item").is_some() && value["info"].is_object() {
        postman::convert(&value, &mut plan)?;
    } else {
        anyhow::bail!("请选择 OpenAPI 2.0/3.0/3.1 或 Postman Collection 2.0/2.1 文件");
    }
    ensure!(plan.request_count() > 0, "导入文件没有可用请求");
    Ok(plan)
}
fn filename(prefix: &str) -> String {
    format!("{prefix}_{}.yml", uuid::Uuid::new_v4().simple())
}
fn text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    })
}
fn description(value: &Value) -> String {
    value
        .as_str()
        .or_else(|| value["content"].as_str())
        .unwrap_or("")
        .into()
}
fn variables(value: &Value) -> Value {
    json!(value.as_array().into_iter().flatten().map(|row| json!({"name":row["key"].as_str().unwrap_or(""),"value":text(&row["value"]),"disabled":row["disabled"]==true})).collect::<Vec<_>>())
}
