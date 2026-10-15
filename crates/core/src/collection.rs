use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

pub const SCHEMA: &str = include_str!("../../../schema/opencollection-1.0.0.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentKind {
    Collection,
    Folder,
    Environment,
    Http,
    Graphql,
    WebSocket,
    Other,
}

#[derive(Clone, Debug)]
pub struct Document {
    path: PathBuf,
    source: String,
    value: Value,
    kind: DocumentKind,
}
impl Document {
    pub fn parse(path: PathBuf, source: String) -> Result<Self> {
        let value: Value = serde_yaml::from_str(&source).context("YAML 格式错误")?;
        ensure!(value.is_object(), "YAML 根节点必须是对象");
        let kind = match path.file_name().and_then(|s| s.to_str()) {
            Some("opencollection.yml" | "opencollection.yaml")
                if path.components().count() == 1 =>
            {
                DocumentKind::Collection
            }
            Some("folder.yml" | "folder.yaml") => DocumentKind::Folder,
            _ if path.starts_with("environments") => DocumentKind::Environment,
            _ => match value.pointer("/info/type").and_then(Value::as_str) {
                Some("http") => DocumentKind::Http,
                Some("graphql") => DocumentKind::Graphql,
                Some("websocket") => DocumentKind::WebSocket,
                _ => DocumentKind::Other,
            },
        };
        validate(&value, kind)?;
        Ok(Self {
            path,
            source,
            value,
            kind,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn kind(&self) -> DocumentKind {
        self.kind
    }
    pub fn name(&self) -> &str {
        self.value
            .pointer("/info/name")
            .or_else(|| self.value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("未命名")
    }
    pub fn protocol_key(&self) -> &'static str {
        match self.kind {
            DocumentKind::WebSocket => "websocket",
            DocumentKind::Graphql => "graphql",
            _ => "http",
        }
    }
    pub fn method(&self) -> &str {
        if self.kind == DocumentKind::WebSocket {
            "WS"
        } else {
            self.value
                .get(self.protocol_key())
                .and_then(|v| v.get("method"))
                .and_then(Value::as_str)
                .unwrap_or("")
        }
    }
    pub fn is_request(&self) -> bool {
        matches!(
            self.kind,
            DocumentKind::Http | DocumentKind::Graphql | DocumentKind::WebSocket
        )
    }
}

pub fn validate(value: &Value, kind: DocumentKind) -> Result<()> {
    static VALIDATORS: OnceLock<BTreeMap<&'static str, jsonschema::Validator>> = OnceLock::new();
    let validators = VALIDATORS.get_or_init(|| {
        [
            "root",
            "Folder",
            "Environment",
            "HttpRequest",
            "GraphQLRequest",
            "WebSocketRequest",
            "Item",
        ]
        .into_iter()
        .map(|name| {
            let mut schema: Value = serde_json::from_str(SCHEMA).expect("embedded schema");
            if name != "root" {
                schema =
                    json!({"$defs": schema["$defs"].clone(), "$ref": format!("#/$defs/{name}")});
            }
            (
                name,
                jsonschema::validator_for(&schema).expect("embedded schema is valid"),
            )
        })
        .collect()
    });
    let name = match kind {
        DocumentKind::Collection => "root",
        DocumentKind::Folder => "Folder",
        DocumentKind::Environment => "Environment",
        DocumentKind::Http => "HttpRequest",
        DocumentKind::Graphql => "GraphQLRequest",
        DocumentKind::WebSocket => "WebSocketRequest",
        DocumentKind::Other => "Item",
    };
    let errors: Vec<String> = validators[name]
        .iter_errors(value)
        .take(8)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect();
    ensure!(
        errors.is_empty(),
        "OpenCollection 校验失败：\n{}",
        errors.join("\n")
    );
    if kind == DocumentKind::Collection {
        ensure!(
            value["opencollection"].as_str() == Some("1.0.0"),
            "只支持 OpenCollection 1.0.0"
        );
        ensure!(
            value["bundled"] != true,
            "请先将 bundled 集合拆分为请求文件"
        );
        ensure!(
            value
                .get("items")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty),
            "文件夹项目的请求应保存为独立 YAML 文件"
        );
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Collection {
    root: PathBuf,
    documents: Vec<Document>,
}
impl Collection {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root).context("无法打开项目目录")?;
        let mut documents = Vec::new();
        scan(&root, &root, &mut documents)?;
        ensure!(
            documents.iter().any(|d| d.kind == DocumentKind::Collection),
            "目录中缺少 opencollection.yml"
        );
        documents.sort_by(|a, b| {
            a.path
                .parent()
                .cmp(&b.path.parent())
                .then_with(|| {
                    a.value
                        .pointer("/info/seq")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.)
                        .total_cmp(
                            &b.value
                                .pointer("/info/seq")
                                .and_then(Value::as_f64)
                                .unwrap_or(0.),
                        )
                })
                .then(a.path.cmp(&b.path))
        });
        Ok(Self { root, documents })
    }
    pub fn create(root: impl AsRef<Path>, name: &str) -> Result<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root)?;
        ensure!(
            !root.join("opencollection.yml").exists() && !root.join("opencollection.yaml").exists(),
            "项目已存在"
        );
        atomic_write(
            &root.join("opencollection.yml"),
            &serde_yaml::to_string(
                &json!({"opencollection":"1.0.0", "info":{"name":name}, "bundled":false, "extensions":{"apifuck":{"version":1}}}),
            )?,
        )?;
        let ignore = root.join(".gitignore");
        let mut content = if ignore.exists() {
            fs::read_to_string(&ignore)?
        } else {
            String::new()
        };
        if !content.lines().any(|line| line == ".apifuck/") {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(".apifuck/\n");
            atomic_write(&ignore, &content)?;
        }
        Self::open(root)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn documents(&self) -> &[Document] {
        &self.documents
    }
    pub fn project(&self) -> &Document {
        self.documents
            .iter()
            .find(|d| d.kind == DocumentKind::Collection)
            .expect("validated project")
    }
    pub fn find(&self, selector: &str) -> Result<&Document> {
        if let Some(d) = self
            .documents
            .iter()
            .find(|d| d.path == Path::new(selector))
        {
            return Ok(d);
        }
        let found: Vec<_> = self
            .documents
            .iter()
            .filter(|d| d.name() == selector)
            .collect();
        ensure!(
            found.len() == 1,
            "未找到唯一的文档 {selector}，请使用相对文件路径"
        );
        Ok(found[0])
    }
    pub fn save(&self, document: &Document, source: &str) -> Result<Document> {
        let mut value: Value = serde_yaml::from_str(source)?;
        if document.is_request() {
            let key = document.protocol_key();
            if let Some(url) = value[key]["url"].as_str() {
                value[key]["url"] = json!(crate::url_editor::normalize(url));
            }
        }
        let source = crate::editing::update(source, &value)?;
        let parsed = Document::parse(document.path.clone(), source.clone())?;
        let path = self.safe_path(&document.path)?;
        ensure!(
            fs::read_to_string(&path)? == document.source,
            "文件已被其他程序修改，请重新载入后再保存：{}",
            document.path.display()
        );
        atomic_write(&path, &source)?;
        Ok(parsed)
    }
    /// Request names are display metadata; file identities are generated independently.
    pub fn create_request(&self, parent: &Path, protocol: &str, name: &str) -> Result<Document> {
        ensure!(
            matches!(protocol, "http" | "graphql" | "websocket"),
            "未知请求类型"
        );
        ensure!(!name.trim().is_empty(), "请求名称不能为空");
        let path = parent.join(format!("api_{}.yml", uuid::Uuid::new_v4()));
        let mut value: Value = serde_yaml::from_str(&template(protocol, name.trim())?)?;
        value[protocol]["url"] = json!("");
        let last = self
            .documents
            .iter()
            .filter(|doc| doc.path().parent().unwrap_or(Path::new("")) == parent)
            .filter_map(|doc| doc.value().pointer("/info/seq").and_then(Value::as_f64))
            .fold(0_f64, f64::max);
        value["info"]["seq"] = json!(last + 1.);
        self.add(&path, &serde_yaml::to_string(&value)?)
    }
    pub fn create_folder(&self, parent: &Path, name: &str) -> Result<Document> {
        let name = name.trim();
        ensure!(!name.is_empty(), "目录名称不能为空");
        ensure!(!parent.starts_with("environments"), "不能在环境中创建目录");
        let path = parent
            .join(format!("folder_{}", uuid::Uuid::new_v4()))
            .join("folder.yml");
        let mut value: Value = serde_yaml::from_str(&template("folder", name)?)?;
        let last = self
            .documents
            .iter()
            .filter(|doc| {
                let directory = doc.path().parent();
                if doc.kind() == DocumentKind::Folder {
                    directory.and_then(Path::parent) == Some(parent)
                } else {
                    doc.is_request() && directory == Some(parent)
                }
            })
            .filter_map(|doc| doc.value().pointer("/info/seq").and_then(Value::as_f64))
            .fold(0_f64, f64::max);
        value["info"]["seq"] = json!(last + 1.);
        self.add(&path, &serde_yaml::to_string(&value)?)
    }
    pub fn create_environment(&self, name: &str) -> Result<Document> {
        let name = name.trim();
        ensure!(!name.is_empty(), "环境名称不能为空");
        ensure!(
            !Self::open(&self.root)?
                .environment_names()
                .iter()
                .any(|existing| existing == name),
            "环境名称已存在"
        );
        let path = Path::new("environments").join(format!("env_{}.yml", uuid::Uuid::new_v4()));
        self.add(&path, &template("environment", name)?)
    }
    pub fn add(&self, path: &Path, source: &str) -> Result<Document> {
        let document = Document::parse(path.to_owned(), source.to_owned())?;
        let absolute = self.safe_path(path)?;
        ensure!(!absolute.exists(), "文件已存在：{}", path.display());
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        // Persist without clobbering a concurrently created document.
        let mut temp = tempfile::NamedTempFile::new_in(absolute.parent().unwrap())?;
        temp.write_all(source.as_bytes())?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&absolute).map_err(|e| e.error)?;
        Ok(document)
    }
    /// Duplicate a document or an entire folder into a new sibling identity.
    /// Folder assets and comments are preserved; local state is handled by the caller.
    pub fn duplicate(&self, document: &Document, source: &str) -> Result<Document> {
        ensure!(
            document.kind != DocumentKind::Collection,
            "不能复制项目根文件"
        );
        let folder = document.kind == DocumentKind::Folder;
        let from = if folder {
            document.path.parent().context("目录无效")?
        } else {
            document.path()
        };
        let parent = from.parent().context("目录无效")?;
        let to = parent.join(if folder {
            format!("folder_{}", uuid::Uuid::new_v4())
        } else {
            format!("copy_{}.yml", uuid::Uuid::new_v4())
        });
        let target = if folder {
            to.join(document.path.file_name().unwrap())
        } else {
            to.clone()
        };
        let parsed = Document::parse(target.clone(), source.to_owned())?;
        ensure!(parsed.kind == document.kind, "复制不能改变文档类型");
        // Check the source even when the caller supplies a local draft.
        ensure!(
            fs::read_to_string(self.safe_path(document.path())?)? == document.source,
            "文件已被外部修改"
        );
        let mut root = self.project().value.clone();
        if let Some(requests) = root
            .pointer_mut("/extensions/apifuck/requests")
            .and_then(Value::as_object_mut)
        {
            let copies: Vec<_> = requests
                .iter()
                .filter_map(|(key, value)| {
                    Path::new(key)
                        .strip_prefix(from)
                        .ok()
                        .map(|suffix| (portable_path(&to.join(suffix)), value.clone()))
                })
                .collect();
            requests.extend(copies);
        }
        let root_update = (root != self.project().value)
            .then(|| crate::editing::update(self.project().source(), &root))
            .transpose()?;
        if root_update.is_some() {
            ensure!(
                fs::read_to_string(self.root.join(self.project().path()))? == self.project().source,
                "项目配置已被外部修改"
            );
        }
        let absolute = self.safe_path(&to)?;
        if folder {
            fn copy_tree(from: &Path, to: &Path) -> Result<()> {
                for entry in fs::read_dir(from)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    if name == ".apifuck" || name == ".git" || name == "node_modules" {
                        continue;
                    }
                    let ty = entry.file_type()?;
                    ensure!(
                        !ty.is_symlink(),
                        "复制目录不支持符号链接：{}",
                        entry.path().display()
                    );
                    let target = to.join(name);
                    if ty.is_dir() {
                        fs::create_dir(&target)?;
                        copy_tree(&entry.path(), &target)?;
                    } else {
                        ensure!(ty.is_file(), "复制目录只支持普通文件");
                        fs::copy(entry.path(), target)?;
                    }
                }
                Ok(())
            }
            let staging = tempfile::tempdir_in(absolute.parent().context("目录无效")?)?;
            copy_tree(&self.safe_path(from)?, staging.path())?;
            atomic_write(
                &staging.path().join(document.path.file_name().unwrap()),
                source,
            )?;
            // Validate all copied documents before publishing the directory.
            scan(staging.path(), staging.path(), &mut Vec::new())?;
            ensure!(!absolute.exists(), "目标已存在");
            fs::rename(staging.path(), &absolute)?;
        } else {
            self.add(&target, source)?;
        }
        if let Some(text) = root_update
            && let Err(error) = self.save(self.project(), &text)
        {
            if folder {
                fs::remove_dir_all(&absolute)?;
            } else {
                fs::remove_file(&absolute)?;
            }
            return Err(error);
        }
        Ok(parsed)
    }
    pub fn rename(&self, document: &Document, target: &Path) -> Result<()> {
        ensure!(
            document.kind != DocumentKind::Collection,
            "不能移动项目根文件"
        );
        let parsed = Document::parse(target.to_owned(), document.source.clone())?;
        ensure!(parsed.kind == document.kind, "移动不能改变文档类型");
        let absolute = self.safe_path(document.path())?;
        ensure!(
            fs::read_to_string(&absolute)? == document.source,
            "文件已被外部修改"
        );
        let (from, to) = if document.kind == DocumentKind::Folder {
            let from = document.path.parent().context("目录无效")?;
            let to = target.parent().context("目标目录无效")?;
            ensure!(
                target.file_name() == document.path.file_name(),
                "目录配置文件名必须保持不变"
            );
            ensure!(!to.starts_with(from), "不能将目录移入自身");
            (from.to_owned(), to.to_owned())
        } else {
            (document.path.clone(), target.to_owned())
        };
        let source = self.safe_path(&from)?;
        let target = self.safe_path(&to)?;
        ensure!(!target.exists(), "目标已存在");
        let mut root = self.project().value.clone();
        if let Some(requests) = root
            .pointer_mut("/extensions/apifuck/requests")
            .and_then(Value::as_object_mut)
        {
            let changes: Vec<_> = requests
                .keys()
                .filter_map(|key| {
                    Path::new(key)
                        .strip_prefix(&from)
                        .ok()
                        .map(|suffix| (key.clone(), portable_path(&to.join(suffix))))
                })
                .collect();
            for (old, new) in changes {
                if let Some(value) = requests.remove(&old) {
                    requests.insert(new, value);
                }
            }
        }
        let root_update = if root != self.project().value {
            Some(crate::editing::update(self.project().source(), &root)?)
        } else {
            None
        };
        if root_update.is_some() {
            ensure!(
                fs::read_to_string(self.root.join(self.project().path()))? == self.project().source,
                "项目配置已被外部修改"
            );
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target)?;
        if let Some(text) = root_update
            && let Err(error) = atomic_write(&self.root.join(self.project().path()), &text)
        {
            fs::rename(&target, &source).context("更新扩展配置失败且无法回滚移动")?;
            return Err(error);
        }
        Ok(())
    }
    pub fn trash(&self, document: &Document) -> Result<()> {
        ensure!(
            document.kind != DocumentKind::Collection,
            "不能删除项目根文件"
        );
        let file = self.safe_path(&document.path)?;
        ensure!(
            fs::read_to_string(&file)? == document.source,
            "文件已被外部修改"
        );
        let relative = if document.kind == DocumentKind::Folder {
            document.path.parent().context("目录无效")?
        } else {
            &document.path
        };
        let path = self.safe_path(relative)?;
        let trash = self.root.join(".apifuck/trash");
        fs::create_dir_all(&trash)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        fs::rename(
            path,
            trash.join(format!(
                "{stamp}-{}",
                relative
                    .file_name()
                    .context("不能回收项目根目录")?
                    .to_string_lossy()
            )),
        )?;
        Ok(())
    }
    /// Reorder siblings without changing file identities or unrelated YAML fields.
    pub fn reorder_before(&self, source: &Document, target: &Document) -> Result<()> {
        ensure!(
            matches!(
                source.kind,
                DocumentKind::Http
                    | DocumentKind::Graphql
                    | DocumentKind::WebSocket
                    | DocumentKind::Folder
            ),
            "此文档不能排序"
        );
        fn parent(document: &Document) -> &Path {
            let parent = document.path.parent().unwrap_or(Path::new(""));
            if document.kind == DocumentKind::Folder {
                parent.parent().unwrap_or(Path::new(""))
            } else {
                parent
            }
        }
        ensure!(
            parent(source) == parent(target),
            "拖拽排序仅支持同级节点；跨目录请使用移动操作"
        );
        if source.path == target.path {
            return Ok(());
        }
        let mut siblings: Vec<_> = self
            .documents
            .iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    DocumentKind::Http
                        | DocumentKind::Graphql
                        | DocumentKind::WebSocket
                        | DocumentKind::Folder
                ) && parent(d) == parent(source)
                    && d.path != source.path
            })
            .collect();
        siblings.sort_by(|a, b| {
            a.value["info"]["seq"]
                .as_f64()
                .unwrap_or(0.)
                .total_cmp(&b.value["info"]["seq"].as_f64().unwrap_or(0.))
                .then(a.path.cmp(&b.path))
        });
        let index = siblings
            .iter()
            .position(|d| d.path == target.path)
            .context("目标节点无效")?;
        siblings.insert(index, source);
        let changes = siblings
            .iter()
            .enumerate()
            .map(|(ix, document)| {
                let mut value = document.value.clone();
                value["info"]["seq"] = json!(ix + 1);
                let source = crate::editing::update(document.source(), &value)?;
                ensure!(
                    fs::read_to_string(self.safe_path(document.path())?)? == document.source,
                    "文件已被外部修改"
                );
                Ok((*document, source))
            })
            .collect::<Result<Vec<_>>>()?;
        for (document, source) in changes {
            self.save(document, &source)?;
        }
        Ok(())
    }
    /// Move the entire project to a recoverable sibling trash directory.
    pub fn trash_project(&self) -> Result<PathBuf> {
        ensure!(
            fs::read_to_string(self.root.join(self.project().path()))? == self.project().source,
            "项目已被外部修改"
        );
        let parent = self.root.parent().context("不能删除文件系统根目录")?;
        let trash = parent.join(".apifuck-trash");
        fs::create_dir_all(&trash)?;
        let target = trash.join(format!(
            "{}-{}",
            uuid::Uuid::new_v4(),
            self.root
                .file_name()
                .context("项目目录无效")?
                .to_string_lossy()
        ));
        fs::rename(&self.root, &target)?;
        Ok(target)
    }
    pub fn safe_path(&self, relative: &Path) -> Result<PathBuf> {
        ensure!(
            !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "路径必须位于项目目录内"
        );
        let path = self.root.join(relative);
        let mut ancestor = path.as_path();
        while !ancestor.exists() {
            ancestor = ancestor.parent().context("无效路径")?;
        }
        ensure!(
            fs::canonicalize(ancestor)?.starts_with(&self.root),
            "路径不能指向项目目录外"
        );
        Ok(path)
    }
    pub fn scopes(&self, document: &Document) -> Vec<&Value> {
        let mut scopes = vec![self.project().value()];
        let parent = document.path.parent().unwrap_or(Path::new(""));
        let mut folders: Vec<_> = self
            .documents
            .iter()
            .filter(|d| {
                d.kind == DocumentKind::Folder
                    && parent.starts_with(d.path.parent().unwrap_or(Path::new("")))
            })
            .collect();
        folders.sort_by_key(|d| d.path.components().count());
        scopes.extend(folders.iter().map(|d| d.value()));
        scopes
    }
    pub fn environment(&self, name: Option<&str>) -> Result<Value> {
        fn resolve(collection: &Collection, name: &str, stack: &mut Vec<String>) -> Result<Value> {
            ensure!(!stack.iter().any(|s| s == name), "环境继承存在循环：{name}");
            stack.push(name.into());
            let env = collection
                .documents
                .iter()
                .find(|d| d.kind == DocumentKind::Environment && d.name() == name)
                .map(|d| d.value.clone())
                .or_else(|| {
                    collection
                        .project()
                        .value
                        .pointer("/config/environments")
                        .and_then(Value::as_array)
                        .and_then(|a| a.iter().find(|e| e["name"] == name))
                        .cloned()
                })
                .with_context(|| format!("环境不存在：{name}"))?;
            ensure!(
                env.get("externalSecrets").is_none() && env.get("dotEnvFilePath").is_none(),
                "此版本尚未实现外部 Secret 或 dotenv 环境"
            );
            let mut vars = if let Some(parent) = env["extends"].as_str() {
                resolve(collection, parent, stack)?
            } else {
                json!({})
            };
            merge_variables(&mut vars, &env["variables"])?;
            stack.pop();
            Ok(vars)
        }
        match name {
            Some(name) => resolve(self, name, &mut vec![]),
            None => Ok(json!({})),
        }
    }
    pub fn environment_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .documents
            .iter()
            .filter(|d| d.kind == DocumentKind::Environment)
            .map(|d| d.name().to_owned())
            .collect();
        if let Some(envs) = self
            .project()
            .value
            .pointer("/config/environments")
            .and_then(Value::as_array)
        {
            names.extend(
                envs.iter()
                    .filter_map(|e| e["name"].as_str().map(str::to_owned)),
            );
        }
        names.sort();
        names.dedup();
        names
    }
}
fn scan(root: &Path, dir: &Path, documents: &mut Vec<Document>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?;
        if entry.file_name().to_string_lossy().starts_with('.')
            || entry.file_name() == "node_modules"
        {
            continue;
        }
        if ty.is_symlink() {
            continue;
        }
        if ty.is_dir() {
            scan(root, &path, documents)?;
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("yml" | "yaml")
        ) {
            let relative = path.strip_prefix(root)?.to_owned();
            // Assets may contain YAML; only collection documents have info or live in environments.
            let source = fs::read_to_string(&path)?;
            let value: Value = serde_yaml::from_str(&source)
                .with_context(|| format!("{}: YAML 无效", path.display()))?;
            if value.get("info").is_some()
                || value.get("opencollection").is_some()
                || relative.starts_with("environments")
            {
                documents.push(
                    Document::parse(relative, source)
                        .with_context(|| path.display().to_string())?,
                );
            }
        }
    }
    Ok(())
}
pub fn merge_variables(target: &mut Value, entries: &Value) -> Result<()> {
    for entry in entries.as_array().into_iter().flatten() {
        if entry["disabled"] == true {
            continue;
        }
        let name = entry["name"].as_str().context("变量缺少 name")?;
        if entry["secret"] == true {
            continue;
        } // Local overlay supplies secrets.
        let value = selected(&entry["value"], "value")?;
        target[name] = if value.is_object() {
            match value["type"].as_str() {
                Some("string") => value["data"].clone(),
                _ => serde_json::from_str(value["data"].as_str().context("变量缺少 data")?)?,
            }
        } else {
            value.clone()
        };
    }
    Ok(())
}
pub fn selected<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    if let Some(items) = value.as_array() {
        let selected: Vec<_> = items.iter().filter(|v| v["selected"] == true).collect();
        ensure!(selected.len() == 1, "{key} 必须选择且只能选择一个变体");
        Ok(&selected[0][key])
    } else {
        Ok(value)
    }
}
pub fn atomic_write(path: &Path, source: &str) -> Result<()> {
    let parent = path.parent().context("文件路径无父目录")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(source.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn template(protocol: &str, name: &str) -> Result<String> {
    match protocol {
        "http" => Ok(serde_yaml::to_string(
            &json!({"info":{"name":name,"type":"http","seq":1},"http":{"method":"GET","url":"https://example.com","auth":"inherit"}}),
        )?),
        "graphql" => Ok(serde_yaml::to_string(
            &json!({"info":{"name":name,"type":"graphql","seq":1},"graphql":{"method":"POST","url":"https://example.com/graphql","body":{"query":"query { __typename }","variables":"{}"},"auth":"inherit"}}),
        )?),
        "websocket" => Ok(serde_yaml::to_string(
            &json!({"info":{"name":name,"type":"websocket","seq":1},"websocket":{"url":"wss://example.com/socket","message":{"type":"text","data":""},"auth":"inherit"}}),
        )?),
        "folder" => Ok(serde_yaml::to_string(
            &json!({"info":{"name":name,"type":"folder","seq":1},"request":{"headers":[],"variables":[]}}),
        )?),
        "environment" => Ok(serde_yaml::to_string(
            &json!({"name":name,"variables":[{"name":"base_url","value":"https://example.com"}]}),
        )?),
        _ => bail!("未知文档类型"),
    }
}

/// Stable slash-separated identity for keys stored in shared YAML.
pub fn portable_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod named_creation_tests {
    use super::*;

    #[test]
    fn duplicate_folder_preserves_nested_documents_assets_and_request_options() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::create(root.path(), "Test").unwrap();
        let folder = c
            .add(
                Path::new("api/folder.yaml"),
                &(template("folder", "API").unwrap() + "# folder comment\n"),
            )
            .unwrap();
        c.add(
            Path::new("api/nested/folder.yml"),
            &template("folder", "Nested").unwrap(),
        )
        .unwrap();
        let source = template("graphql", "Query").unwrap() + "# request comment\n";
        c.add(Path::new("api/nested/query.yml"), &source).unwrap();
        fs::write(root.path().join("api/payload.json"), "{\"upload\":true}").unwrap();
        fs::create_dir(root.path().join("api/.apifuck")).unwrap();
        fs::write(root.path().join("api/.apifuck/private"), "local state").unwrap();
        let mut value = c.project().value().clone();
        value["extensions"]["apifuck"]["requests"] =
            json!({"api/nested/query.yml": {"timeout": 123}});
        c.save(
            c.project(),
            &crate::editing::update(c.project().source(), &value).unwrap(),
        )
        .unwrap();
        let c = Collection::open(root.path()).unwrap();
        let copy = c.duplicate(&folder, folder.source()).unwrap();
        let parent = copy.path().parent().unwrap();
        assert_ne!(parent, Path::new("api"));
        assert_eq!(copy.kind(), DocumentKind::Folder);
        assert_eq!(copy.path().file_name().unwrap(), "folder.yaml");
        assert_eq!(
            fs::read_to_string(root.path().join(parent).join("nested/query.yml")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.path().join(parent).join("payload.json")).unwrap(),
            "{\"upload\":true}"
        );
        assert!(!root.path().join(parent).join(".apifuck").exists());
        let reopened = Collection::open(root.path()).unwrap();
        assert_eq!(
            reopened.project().value()["extensions"]["apifuck"]["requests"]
                [portable_path(&parent.join("nested/query.yml"))]["timeout"],
            123
        );
        assert_eq!(
            reopened.find("api/nested/query.yml").unwrap().source(),
            source
        );
        assert_eq!(
            reopened.find(&portable_path(copy.path())).unwrap().source(),
            folder.source()
        );
    }

    #[test]
    fn duplicate_request_uses_draft_and_rejects_invalid_or_stale_sources() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::create(root.path(), "Test").unwrap();
        for protocol in ["http", "graphql", "websocket"] {
            let doc = c
                .create_request(Path::new(""), protocol, "Original")
                .unwrap();
            let draft = doc.source().replace("Original", "Edited");
            let copy = c.duplicate(&doc, &draft).unwrap();
            assert_eq!(copy.kind(), doc.kind());
            assert_eq!(copy.name(), "Edited");
            assert_eq!(
                fs::read_to_string(root.path().join(doc.path())).unwrap(),
                doc.source()
            );
            assert!(c.duplicate(&doc, "info: [").is_err());
            fs::write(root.path().join(doc.path()), draft).unwrap();
            assert!(c.duplicate(&doc, doc.source()).is_err());
        }
        assert!(c.duplicate(c.project(), c.project().source()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn duplicate_folder_with_symlink_fails_without_publishing_partial_copy() {
        let root = tempfile::tempdir().unwrap();
        let c = Collection::create(root.path(), "Test").unwrap();
        let folder = c.create_folder(Path::new(""), "Folder").unwrap();
        std::os::unix::fs::symlink(
            root.path(),
            root.path()
                .join(folder.path().parent().unwrap())
                .join("link"),
        )
        .unwrap();
        let before = fs::read_dir(root.path()).unwrap().count();
        assert!(c.duplicate(&folder, folder.source()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), before);
    }

    #[test]
    fn folder_names_are_metadata_and_nested_folders_keep_their_scope() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        let folder = collection
            .create_folder(Path::new(""), " 用户 / 管理 ")
            .unwrap();
        assert_eq!(folder.name(), "用户 / 管理");
        assert_eq!(folder.kind(), DocumentKind::Folder);
        assert_eq!(folder.path().file_name().unwrap(), "folder.yml");
        assert_eq!(folder.path().components().count(), 2);
        let collection = Collection::open(root.path()).unwrap();
        let sibling = collection
            .create_folder(Path::new(""), "用户 / 管理")
            .unwrap();
        assert_ne!(sibling.path(), folder.path());
        assert_eq!(sibling.value()["info"]["seq"], 2.0);
        let nested = collection
            .create_folder(folder.path().parent().unwrap(), ".. / details")
            .unwrap();
        assert_eq!(nested.name(), ".. / details");
        assert_eq!(
            nested.path().parent().and_then(Path::parent),
            folder.path().parent()
        );
        let collection = Collection::open(root.path()).unwrap();
        let request = collection
            .create_request(nested.path().parent().unwrap(), "http", "Query")
            .unwrap();
        let names: Vec<_> = collection
            .scopes(&request)
            .iter()
            .map(|scope| scope["info"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Test", "用户 / 管理", ".. / details"]);
    }

    #[test]
    fn environments_get_automatic_files_and_reject_duplicate_display_names() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        let environment = collection.create_environment(" 测试 / staging ").unwrap();
        assert_eq!(environment.kind(), DocumentKind::Environment);
        assert_eq!(environment.name(), "测试 / staging");
        assert_eq!(environment.path().parent(), Some(Path::new("environments")));
        let stem = environment.path().file_stem().unwrap().to_str().unwrap();
        uuid::Uuid::parse_str(stem.strip_prefix("env_").unwrap()).unwrap();
        assert!(collection.create_environment("测试 / staging").is_err());
        let another = collection.create_environment("production").unwrap();
        assert_ne!(environment.path(), another.path());
        let collection = Collection::open(root.path()).unwrap();
        assert!(
            collection
                .environment_names()
                .contains(&"测试 / staging".to_owned())
        );
        assert_eq!(
            collection.environment(Some("测试 / staging")).unwrap()["base_url"],
            "https://example.com"
        );
    }

    #[test]
    fn invalid_names_and_folder_parents_do_not_create_documents() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        assert!(collection.create_folder(Path::new(""), " \n ").is_err());
        assert!(collection.create_environment(" \n ").is_err());
        assert!(collection.create_folder(Path::new(".."), "Folder").is_err());
        assert!(
            collection
                .create_folder(Path::new("environments"), "Folder")
                .is_err()
        );
        assert_eq!(Collection::open(root.path()).unwrap().documents().len(), 1);
    }
}

#[cfg(test)]
mod request_creation_tests {
    use super::*;

    #[test]
    fn new_request_types_have_unique_automatic_files_and_blank_urls() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        let mut paths = std::collections::BTreeSet::new();
        for (protocol, kind, method) in [
            ("http", DocumentKind::Http, "GET"),
            ("graphql", DocumentKind::Graphql, "POST"),
            ("websocket", DocumentKind::WebSocket, "WS"),
        ] {
            for _ in 0..2 {
                let document = collection
                    .create_request(Path::new(""), protocol, "用户列表 / details")
                    .unwrap();
                assert_eq!(document.kind(), kind);
                assert_eq!(document.method(), method);
                assert_eq!(document.value()[protocol]["url"], "");
                assert_eq!(document.name(), "用户列表 / details");
                assert_eq!(document.path().components().count(), 1);
                let stem = document.path().file_stem().unwrap().to_str().unwrap();
                uuid::Uuid::parse_str(stem.strip_prefix("api_").unwrap()).unwrap();
                assert!(paths.insert(document.path().to_owned()));
                assert!(root.path().join(document.path()).is_file());
            }
        }
        assert_eq!(
            Collection::open(root.path())
                .unwrap()
                .documents()
                .iter()
                .filter(|doc| doc.is_request())
                .count(),
            6
        );
    }

    #[test]
    fn display_name_changes_keep_file_identity_and_folder_scope() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        collection
            .add(
                Path::new("users/folder.yml"),
                &template("folder", "Users").unwrap(),
            )
            .unwrap();
        let collection = Collection::open(root.path()).unwrap();
        let request = collection
            .create_request(Path::new("users"), "http", "Untitled")
            .unwrap();
        assert_eq!(request.path().parent(), Some(Path::new("users")));
        assert!(
            collection
                .scopes(&request)
                .iter()
                .any(|scope| scope["info"]["name"] == "Users")
        );
        let mut value = request.value().clone();
        value["info"]["name"] = json!("查询用户");
        let saved = collection
            .save(&request, &serde_yaml::to_string(&value).unwrap())
            .unwrap();
        assert_eq!(saved.path(), request.path());
        assert_eq!(saved.name(), "查询用户");
        assert_eq!(
            Collection::open(root.path())
                .unwrap()
                .find("查询用户")
                .unwrap()
                .path(),
            request.path()
        );
    }

    #[test]
    fn request_creation_rejects_non_request_types_and_invalid_destinations() {
        let root = tempfile::tempdir().unwrap();
        let collection = Collection::create(root.path(), "Test").unwrap();
        assert!(
            collection
                .create_request(Path::new(""), "folder", "Folder")
                .is_err()
        );
        assert!(
            collection
                .create_request(Path::new(""), "http", "  ")
                .is_err()
        );
        assert!(
            collection
                .create_request(Path::new(".."), "http", "Request")
                .is_err()
        );
        assert_eq!(Collection::open(root.path()).unwrap().documents().len(), 1);
    }
}
