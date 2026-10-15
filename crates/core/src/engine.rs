use crate::{
    Collection, Document, DocumentKind, NetworkSettings,
    collection::{merge_variables, selected},
    scripts, settings,
};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Event {
    Request {
        request: RequestHead,
    },
    Started {
        status: u16,
        headers: Vec<(String, String)>,
    },
    Chunk {
        text: String,
        received_bytes: usize,
    },
    Message {
        direction: String,
        text: String,
    },
    Connected,
    Finished,
    Error {
        message: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestHead {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
}
impl RequestHead {
    pub(crate) fn new(method: &str, url: &str, headers: &reqwest::header::HeaderMap) -> Self {
        Self {
            method: method.to_owned(),
            url: url.to_owned(),
            headers: headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub body_base64: Option<String>,
    pub elapsed_ms: u128,
    pub bytes: usize,
    pub logs: Vec<String>,
    pub tests: Vec<scripts::TestResult>,
}
#[derive(Clone)]
pub struct Execution {
    pub(crate) root: PathBuf,
    pub(crate) directory: PathBuf,
    pub(crate) options: Value,
    pub(crate) request: Value,
    pub(crate) environment: Option<String>,
    pub(crate) env: Value,
    pub(crate) variables: Value,
    pub(crate) hooks: Vec<Value>,
    pub(crate) settings: NetworkSettings,
    pub(crate) websocket: bool,
    pub(crate) graphql: bool,
    pub(crate) keep_alive_ms: u64,
    pub(crate) mutations: Value,
    pub(crate) report: scripts::ScriptReport,
    pub(crate) assertions: Vec<Value>,
    pub(crate) actions: Vec<Value>,
    pub(crate) cancel: Arc<AtomicBool>,
}
impl Execution {
    pub fn request(&self) -> &Value {
        &self.request
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }
    pub fn is_websocket(&self) -> bool {
        self.websocket
    }
    pub fn network(&self) -> &NetworkSettings {
        &self.settings
    }
    pub fn with_network(mut self, network: NetworkSettings) -> Self {
        self.settings = network;
        self
    }
    pub fn set_network(&mut self, network: NetworkSettings) {
        self.settings = network;
    }
    pub(crate) fn hooks(&mut self, phase: &str, mut response: Option<&mut Value>) -> Result<()> {
        let hooks = self.hooks.clone();
        for hook in &hooks {
            let kind = hook["type"].as_str().context("脚本缺少 type")?;
            ensure!(
                matches!(kind, "before-request" | "after-response" | "tests"),
                "此版本不支持脚本生命周期：{kind}"
            );
            if kind == phase {
                scripts::run(
                    hook["code"].as_str().unwrap_or(""),
                    &mut self.request,
                    response.as_deref_mut(),
                    &mut self.env,
                    &mut self.variables,
                    &mut self.mutations,
                    &mut self.report,
                )?;
            }
        }
        Ok(())
    }
    pub(crate) fn before(&mut self) -> Result<()> {
        self.hooks("before-request", None)?;
        let mut vars = self.env.clone();
        merge_object(&mut vars, &self.variables);
        interpolate_value(&mut self.request, &vars)?;
        interpolate_value(&mut self.options, &vars)?;
        resolve_request_url(&mut self.request, &vars)?;
        if self.graphql {
            graphql_http_request(&mut self.request)?;
        }
        Ok(())
    }
}
pub(crate) fn resolve_request_url(request: &mut Value, variables: &Value) -> Result<()> {
    let raw = request["url"].as_str().context("缺少 URL")?;
    let base = if raw.trim().starts_with(['/', '?', '#']) {
        variables["base_url"]
            .as_str()
            .map(|s| interpolate(s, variables))
            .transpose()?
    } else {
        None
    };
    request["url"] = json!(crate::url_editor::resolve(raw, base.as_deref())?);
    Ok(())
}
fn merge_object(target: &mut Value, source: &Value) {
    for (k, v) in source.as_object().into_iter().flatten() {
        target[k] = v.clone();
    }
}
/// Resolve the same static variable scopes used by execution, without running scripts.
pub fn preview_variables(
    collection: &Collection,
    document: &Document,
    environment: Option<&str>,
) -> Result<Value> {
    let mut values = environment_variables(collection, environment)?;
    merge_object(&mut values, &scoped_variables(collection, document)?);
    Ok(values)
}

fn environment_variables(collection: &Collection, environment: Option<&str>) -> Result<Value> {
    let mut values = collection.environment(environment)?;
    merge_object(
        &mut values,
        &settings::overlay(collection.root(), environment)?,
    );
    Ok(values)
}

fn scoped_variables(collection: &Collection, document: &Document) -> Result<Value> {
    let mut values = json!({});
    for scope in collection.scopes(document) {
        merge_variables(&mut values, &scope["request"]["variables"])?;
    }
    merge_variables(&mut values, &document.value()["runtime"]["variables"])?;
    Ok(values)
}

pub fn prepare(
    collection: &Collection,
    document: &Document,
    environment: Option<&str>,
    network: NetworkSettings,
) -> Result<Execution> {
    ensure!(
        document.is_request(),
        "该文档不是可执行的 HTTP / WebSocket 请求"
    );
    let websocket = document.kind() == DocumentKind::WebSocket;
    let key = document.protocol_key();
    let extensions = &collection.project().value()["extensions"]["apifuck"];
    ensure!(
        extensions.is_null() || extensions["version"] == 1,
        "不支持的 ApiFuck 扩展版本"
    );
    let options = extensions["requests"][crate::collection::portable_path(document.path())].clone();
    let mut request = document.value()[key].clone();
    ensure!(request.is_object(), "缺少 {key} 配置");
    let env = environment_variables(collection, environment)?;
    let vars = scoped_variables(collection, document)?;
    let mut headers: Vec<Value> = vec![];
    let mut inherited_auth = Value::Null;
    let mut hooks = vec![];
    let mut resolved_settings = json!({});
    for scope in collection.scopes(document) {
        let defaults = &scope["request"];
        merge_headers(&mut headers, &defaults["headers"]);
        if defaults.get("auth").is_some() && defaults["auth"] != "inherit" {
            inherited_auth = defaults["auth"].clone();
        }
        hooks.extend(
            defaults["scripts"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned(),
        );
        merge_settings(&mut resolved_settings, &defaults["settings"][key]);
    }
    merge_headers(&mut headers, &request["headers"]);
    request["headers"] = json!(headers);
    if request["auth"] == "inherit" {
        request["auth"] = inherited_auth;
    }
    if let Some(body) = request.get("body") {
        request["body"] = selected(body, "body")?.clone();
    }
    hooks.extend(
        document.value()["runtime"]["scripts"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned(),
    );
    for action in document.value()["runtime"]["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["disabled"] != true)
    {
        ensure!(
            action["type"] == "set-variable"
                && action["selector"]["method"] == "jsonq"
                && action["phase"].as_str().unwrap_or("after-response") == "after-response"
                && matches!(
                    action["variable"]["scope"].as_str(),
                    Some("runtime" | "environment" | "request")
                ),
            "此响应 action 尚未支持；请求未发送"
        );
    }
    for assertion in document.value()["runtime"]["assertions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["disabled"] != true)
    {
        ensure!(
            crate::testing::OPERATORS.contains(&assertion["operator"].as_str().unwrap_or("eq")),
            "不支持的断言操作符"
        );
    }
    merge_settings(&mut resolved_settings, &document.value()["settings"]);
    if let Some(omit) = resolved_settings["omitHeaders"].as_array() {
        request["headers"].as_array_mut().unwrap().retain(|h| {
            !omit.iter().any(|o| {
                o.as_str()
                    .is_some_and(|o| o.eq_ignore_ascii_case(h["name"].as_str().unwrap_or("")))
            })
        });
    }
    let mut network = network;
    let proxy = &collection.project().value()["config"]["proxy"];
    if proxy["disabled"] == true {
        network.proxy = Some(String::new());
    } else if proxy["inherit"] != true && proxy["config"].is_object() {
        let config = &proxy["config"];
        let scheme = config["protocol"].as_str().unwrap_or("http");
        let host = config["hostname"].as_str().context("代理缺少 hostname")?;
        let mut url = url::Url::parse(&format!("{scheme}://{host}"))?;
        if let Some(port) = config["port"].as_u64() {
            url.set_port(Some(u16::try_from(port)?))
                .map_err(|_| anyhow::anyhow!("代理端口无效"))?;
        }
        if config["auth"].is_object() && config["auth"]["disabled"] != true {
            url.set_username(config["auth"]["username"].as_str().unwrap_or(""))
                .map_err(|_| anyhow::anyhow!("代理用户名无效"))?;
            url.set_password(config["auth"]["password"].as_str())
                .map_err(|_| anyhow::anyhow!("代理密码无效"))?;
        }
        ensure!(
            config["bypassProxy"].as_str().is_none_or(str::is_empty),
            "代理 bypassProxy 尚未支持"
        );
        network.proxy = Some(url.to_string());
    }
    ensure!(
        collection.project().value()["config"]["clientCertificates"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "客户端证书尚未支持"
    );
    if let Some(ms) = options["connectTimeoutMs"].as_u64() {
        network.connect_timeout_ms = ms;
    }
    if let Some(invalid) = options["allowInvalidCertificates"].as_bool() {
        network.allow_invalid_certificates = invalid;
    }
    ensure!(
        network.max_response_bytes > 0 && network.connect_timeout_ms > 0,
        "连接超时和响应大小必须大于 0"
    );
    if let Some(v) = resolved_settings["timeout"].as_u64() {
        if websocket {
            network.connect_timeout_ms = v;
        } else {
            network.timeout_ms = v;
        }
    }
    if let Some(v) = resolved_settings["followRedirects"].as_bool() {
        network.follow_redirects = v;
    }
    if let Some(v) = resolved_settings["maxRedirects"].as_u64() {
        network.max_redirects = v as usize;
    }
    ensure!(
        resolved_settings["encodeUrl"] != false,
        "encodeUrl: false 尚未支持，请移除该设置"
    );
    if let Some(forward) = resolved_settings["forwardAuthorizationHeader"].as_bool() {
        network.forward_authorization_header = forward;
    }
    Ok(Execution {
        root: collection.root().to_owned(),
        directory: document.path().parent().unwrap_or(Path::new("")).to_owned(),
        request,
        options,
        environment: environment.map(str::to_owned),
        env,
        variables: vars,
        hooks,
        settings: network,
        websocket,
        graphql: document.kind() == DocumentKind::Graphql,
        keep_alive_ms: document.value()["settings"]["keepAliveInterval"]
            .as_u64()
            .unwrap_or(0),
        mutations: json!({}),
        report: Default::default(),
        assertions: document.value()["runtime"]["assertions"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        actions: document.value()["runtime"]["actions"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        cancel: Arc::new(AtomicBool::new(false)),
    })
}
fn merge_settings(target: &mut Value, source: &Value) {
    for (key, value) in source.as_object().into_iter().flatten() {
        if value != "inherit" {
            target[key] = value.clone();
        }
    }
}
fn merge_headers(target: &mut Vec<Value>, value: &Value) {
    let entries = value.as_array().cloned().unwrap_or_default();
    // Replace a parent's name as a group, while retaining duplicate local headers.
    target.retain(|old| {
        !entries.iter().any(|new| {
            new["name"]
                .as_str()
                .unwrap_or("")
                .eq_ignore_ascii_case(old["name"].as_str().unwrap_or(""))
        })
    });
    target.extend(entries.into_iter().filter(|e| e["disabled"] != true));
}
pub fn interpolate(text: &str, variables: &Value) -> Result<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap());
    fn expand(
        text: &str,
        vars: &Value,
        re: &regex::Regex,
        stack: &mut Vec<String>,
    ) -> Result<String> {
        let mut output = String::new();
        let mut previous = 0;
        for captures in re.captures_iter(text) {
            let full = captures.get(0).unwrap();
            let key = captures[1].trim();
            ensure!(
                !stack.iter().any(|k| k == key) && stack.len() < 32,
                "变量循环引用：{key}"
            );
            let value = vars
                .get(key)
                .with_context(|| format!("未定义变量：{key}"))?;
            stack.push(key.to_owned());
            let rendered = match value {
                Value::String(v) => expand(v, vars, re, stack)?,
                _ => value.to_string(),
            };
            stack.pop();
            output.push_str(&text[previous..full.start()]);
            output.push_str(&rendered);
            previous = full.end();
        }
        output.push_str(&text[previous..]);
        Ok(output)
    }
    expand(text, variables, re, &mut vec![])
}
fn interpolate_value(value: &mut Value, variables: &Value) -> Result<()> {
    match value {
        Value::String(s) => *s = interpolate(s, variables)?,
        Value::Array(items) => {
            for item in items {
                interpolate_value(item, variables)?;
            }
        }
        Value::Object(map) => {
            if map.get("disabled") == Some(&Value::Bool(true)) {
                return Ok(());
            }
            for (_, v) in map {
                interpolate_value(v, variables)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub(crate) fn headers_and_url(request: &Value) -> Result<(url::Url, reqwest::header::HeaderMap)> {
    use reqwest::header::{HeaderName, HeaderValue};
    let raw = request["url"].as_str().context("缺少 URL")?;
    let mut url = url::Url::parse(raw).context("请输入完整 URL，或使用 {{base_url}} 变量")?;
    for parameter in request["params"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["disabled"] != true && p["type"] == "path")
    {
        let key = parameter["name"].as_str().context("路径参数缺少 name")?;
        let encoded = url::form_urlencoded::byte_serialize(
            parameter["value"].as_str().unwrap_or("").as_bytes(),
        )
        .collect::<String>()
        .replace('+', "%20");
        let path = url
            .path()
            .split('/')
            .map(|segment| {
                if segment == format!(":{key}")
                    || segment == format!("{{{key}}}")
                    || segment == format!("%7B{key}%7D")
                {
                    encoded.clone()
                } else {
                    segment.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        url.set_path(&path);
    }
    let mut headers = reqwest::header::HeaderMap::new();
    for h in request["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["disabled"] != true)
    {
        headers.append(
            HeaderName::from_bytes(h["name"].as_str().context("Header 缺少 name")?.as_bytes())?,
            HeaderValue::from_str(h["value"].as_str().unwrap_or(""))?,
        );
    }
    for p in request["params"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["disabled"] != true && p["type"] == "query")
    {
        url.query_pairs_mut().append_pair(
            p["name"].as_str().context("Query 缺少 name")?,
            p["value"].as_str().unwrap_or(""),
        );
    }
    let auth = &request["auth"];
    if auth.is_object() {
        match auth["type"].as_str().context("认证缺少 type")? {
            "basic" => {
                let credentials = format!(
                    "{}:{}",
                    auth["username"].as_str().unwrap_or(""),
                    auth["password"].as_str().unwrap_or("")
                );
                headers.insert(
                    "authorization",
                    HeaderValue::from_str(&format!(
                        "Basic {}",
                        base64::engine::general_purpose::STANDARD.encode(credentials)
                    ))?,
                );
            }
            "bearer" => {
                headers.insert(
                    "authorization",
                    HeaderValue::from_str(&format!(
                        "Bearer {}",
                        auth["token"].as_str().unwrap_or("")
                    ))?,
                );
            }
            "apikey" => {
                let key = auth["key"].as_str().context("API Key 缺少 key")?;
                let value = auth["value"].as_str().unwrap_or("");
                if auth["placement"] == "query" {
                    url.query_pairs_mut().append_pair(key, value);
                } else {
                    headers.insert(
                        HeaderName::from_bytes(key.as_bytes())?,
                        HeaderValue::from_str(value)?,
                    );
                }
            }
            other => bail!("此版本尚未实现认证类型：{other}"),
        }
    }
    if headers
        .get_all(reqwest::header::ACCEPT)
        .iter()
        .any(|value| {
            value
                .to_str()
                .is_ok_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        })
    {
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            reqwest::header::HeaderValue::from_static("identity"),
        );
    }
    Ok((url, headers))
}
pub(crate) fn http_client(settings: &NetworkSettings) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(settings.connect_timeout_ms))
        .danger_accept_invalid_certs(settings.allow_invalid_certificates)
        .redirect(if settings.follow_redirects {
            reqwest::redirect::Policy::limited(settings.max_redirects)
        } else {
            reqwest::redirect::Policy::none()
        });
    if let Some(proxy) = &settings.proxy {
        if !proxy.trim().is_empty() {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        } else {
            builder = builder.no_proxy();
        }
    }
    Ok(builder.build()?)
}
pub(crate) fn asset_path(execution: &Execution, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_owned()
    } else {
        execution.root.join(&execution.directory).join(p)
    }
}
pub async fn request_builder(execution: &Execution) -> Result<reqwest::RequestBuilder> {
    let (url, headers) = headers_and_url(&execution.request)?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "HTTP 请求仅支持 http/https URL"
    );
    let mut network = execution.settings.clone();
    if network.forward_authorization_header {
        network.follow_redirects = false;
    }
    let client = http_client(&network)?;
    let mut builder = client
        .request(
            reqwest::Method::from_bytes(
                execution.request["method"]
                    .as_str()
                    .context("缺少 HTTP 方法")?
                    .as_bytes(),
            )?,
            url,
        )
        .headers(headers);
    let body = &execution.request["body"];
    if body.is_null() {
        return Ok(builder);
    }
    match body["type"].as_str().context("Body 缺少 type")? {
        "json" | "text" | "xml" | "sparql" => {
            let typ = body["type"].as_str().unwrap();
            let has_type = execution.request["headers"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|h| {
                    h["disabled"] != true
                        && h["name"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
                });
            if !has_type {
                builder = builder.header(
                    "Content-Type",
                    match typ {
                        "json" => "application/json",
                        "xml" => "application/xml",
                        "sparql" => "application/sparql-query",
                        _ => "text/plain",
                    },
                );
            }
            builder = builder.body(body["data"].as_str().unwrap_or("").to_owned());
        }
        "form-urlencoded" => {
            let pairs: Vec<_> = body["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["disabled"] != true)
                .map(|v| {
                    (
                        v["name"].as_str().unwrap_or(""),
                        v["value"].as_str().unwrap_or(""),
                    )
                })
                .collect();
            builder = builder.form(&pairs);
        }
        "multipart-form" => {
            let mut form = reqwest::multipart::Form::new();
            for (entry_ix, entry) in body["data"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(_, v)| v["disabled"] != true)
            {
                let name = entry["name"]
                    .as_str()
                    .context("表单项缺少 name")?
                    .to_owned();
                if entry["type"] == "file" {
                    let paths: Vec<_> = if let Some(a) = entry["value"].as_array() {
                        a.iter().filter_map(Value::as_str).collect()
                    } else {
                        vec![entry["value"].as_str().context("文件项缺少路径")?]
                    };
                    for path in paths {
                        let path = asset_path(execution, path);
                        let bytes = tokio::fs::read(&path)
                            .await
                            .with_context(|| format!("无法读取 {}", path.display()))?;
                        let mut part = reqwest::multipart::Part::bytes(bytes).file_name(
                            execution.options["multipartFileNames"][entry_ix.to_string()]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| {
                                    path.file_name()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .into_owned()
                                }),
                        );
                        if let Some(t) = entry["contentType"].as_str() {
                            part = part.mime_str(t)?;
                        }
                        form = form.part(name.clone(), part);
                    }
                } else {
                    let mut part = reqwest::multipart::Part::text(
                        entry["value"].as_str().unwrap_or("").to_owned(),
                    );
                    if let Some(t) = entry["contentType"].as_str() {
                        part = part.mime_str(t)?;
                    }
                    form = form.part(name, part);
                }
            }
            builder = builder.multipart(form);
        }
        "file" => {
            let files = body["data"].as_array().context("文件正文缺少 data")?;
            let selected: Vec<_> = files.iter().filter(|f| f["selected"] == true).collect();
            ensure!(selected.len() == 1, "二进制正文必须选择一个文件");
            let file = selected[0];
            let explicit_type = execution.request["headers"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|h| {
                    h["disabled"] != true
                        && h["name"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
                });
            if !explicit_type {
                builder = builder.header(
                    "Content-Type",
                    file["contentType"]
                        .as_str()
                        .unwrap_or("application/octet-stream"),
                );
            }
            builder = builder.body(
                tokio::fs::read(asset_path(
                    execution,
                    file["filePath"].as_str().context("缺少文件路径")?,
                ))
                .await?,
            );
        }
        other => bail!("不支持的正文类型：{other}"),
    }
    Ok(builder)
}
pub async fn execute(
    execution: Execution,
    events: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
) -> Result<Response> {
    execute_with_variables(execution, events)
        .await
        .map(|(response, _)| response)
}

// Materialize the client's implicit headers on the very request we send, so
// the UI does not have to reconstruct them from editable configuration.
fn finalize_request(request: &mut reqwest::Request) -> Result<()> {
    use reqwest::header::{
        ACCEPT, ACCEPT_ENCODING, CONTENT_LENGTH, HOST, HeaderValue, TRANSFER_ENCODING,
    };
    let host = request.url()[url::Position::BeforeHost..url::Position::AfterPort].to_owned();
    let length = request
        .body()
        .and_then(|body| body.as_bytes())
        .map(|body| body.len());
    let empty = request.body().is_none() || length == Some(0);
    let headers = request.headers_mut();
    headers.entry(HOST).or_insert(HeaderValue::from_str(&host)?);
    headers
        .entry(ACCEPT)
        .or_insert(HeaderValue::from_static("*/*"));
    headers
        .entry(ACCEPT_ENCODING)
        .or_insert(HeaderValue::from_static("gzip,deflate,br"));
    if empty {
        headers.remove(TRANSFER_ENCODING);
    } else if headers.contains_key(TRANSFER_ENCODING) {
        headers.remove(CONTENT_LENGTH);
        if let reqwest::header::Entry::Occupied(mut entry) = headers.entry(TRANSFER_ENCODING)
            && let Some(value) = entry.iter_mut().next_back()
            && !value
                .to_str()?
                .rsplit(',')
                .next()
                .is_some_and(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        {
            let mut bytes = value.as_bytes().to_vec();
            bytes.extend_from_slice(b", chunked");
            *value = HeaderValue::from_bytes(&bytes)?;
        }
    } else if let Some(length) = length.filter(|length| *length > 0) {
        headers
            .entry(CONTENT_LENGTH)
            .or_insert(HeaderValue::from(length));
    }
    Ok(())
}

async fn send_request(
    execution: &Execution,
    events: &Option<tokio::sync::mpsc::UnboundedSender<Event>>,
) -> Result<(reqwest::Response, bool, reqwest::header::HeaderMap)> {
    let mut single = execution.clone();
    single.settings.follow_redirects = false;
    let (client, request) = request_builder(&single).await?.build_split();
    let mut request = request?;
    if request.url().scheme() == "http"
        && !request
            .headers()
            .contains_key(reqwest::header::PROXY_AUTHORIZATION)
        && let Some(proxy) = execution
            .settings
            .proxy
            .as_deref()
            .filter(|proxy| !proxy.is_empty())
    {
        let proxy = url::Url::parse(proxy)?;
        if matches!(proxy.scheme(), "http" | "https") {
            // Use reqwest's own URL credential decoding, including percent escapes.
            let proxy_request = client.get(proxy).build()?;
            if let Some(auth) = proxy_request.headers().get(reqwest::header::AUTHORIZATION) {
                request
                    .headers_mut()
                    .insert(reqwest::header::PROXY_AUTHORIZATION, auth.clone());
            }
        }
    }
    let replayable = request.try_clone().is_some();
    finalize_request(&mut request)?;
    let head = RequestHead::new(
        request.method().as_str(),
        request.url().as_str(),
        request.headers(),
    );
    let headers = request.headers().clone();
    let response = client.execute(request).await.context("请求失败")?;
    // Receiving response headers proves this hop was sent, even if reading its
    // body later fails or is cancelled. Connection failures have no sent head.
    emit(events, Event::Request { request: head });
    Ok((response, replayable, headers))
}
pub(crate) async fn execute_with_variables(
    mut execution: Execution,
    events: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
) -> Result<(Response, Value)> {
    ensure!(!execution.websocket, "请使用 WebSocket 会话执行此请求");
    execution.before()?;
    let cancel = execution.cancel.clone();
    let expects_stream = execution.request["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|h| {
            h["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("accept"))
                && h["value"]
                    .as_str()
                    .is_some_and(|v| v.contains("text/event-stream"))
        });
    let timeout = if expects_stream {
        0
    } else {
        execution.settings.timeout_ms
    };
    let future = async {
        let start = Instant::now();
        let mut current = execution.clone();
        let (mut response, mut replayable, mut sent_headers) =
            send_request(&current, &events).await?;
        if execution.settings.follow_redirects {
            let mut redirects = 0;
            while matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
                    break;
                };
                let reset_body = (response.status().as_u16() == 303
                    && (!execution.settings.forward_authorization_header
                        || current.request["method"] != "HEAD"))
                    || (matches!(response.status().as_u16(), 301 | 302)
                        && current.request["method"] == "POST");
                if !reset_body && !replayable && !execution.settings.forward_authorization_header {
                    break;
                }
                ensure!(
                    redirects < execution.settings.max_redirects,
                    "超过最大重定向次数"
                );
                redirects += 1;
                let next = match location
                    .to_str()
                    .ok()
                    .and_then(|location| response.url().join(location).ok())
                {
                    Some(next) => next,
                    None if !execution.settings.forward_authorization_header => break,
                    None => bail!("无效的重定向地址"),
                };
                let cross_origin = if execution.settings.forward_authorization_header {
                    next.origin() != response.url().origin()
                } else {
                    next.host_str() != response.url().host_str()
                        || next.port_or_known_default() != response.url().port_or_known_default()
                };
                let (_, mut headers) = headers_and_url(&current.request)?;
                // URL userinfo is converted into Basic auth by RequestBuilder.
                // Carry the actual header forward before applying redirect policy.
                if let Some(auth) = sent_headers.get(reqwest::header::AUTHORIZATION) {
                    headers.insert(reqwest::header::AUTHORIZATION, auth.clone());
                }
                let forwarded_headers = headers.iter().filter(|(name, _)| {
                    (**name != reqwest::header::HOST || !execution.settings.forward_authorization_header) && (!cross_origin ||
                        (!matches!(name.as_str(), "cookie" | "cookie2" | "proxy-authorization" | "www-authenticate") &&
                        (**name != reqwest::header::AUTHORIZATION || execution.settings.forward_authorization_header)))
                }).map(|(name,value)|json!({"name":name.as_str(),"value":String::from_utf8_lossy(value.as_bytes())})).collect::<Vec<_>>();
                current.request["headers"] = json!(forwarded_headers);
                if !(execution.settings.forward_authorization_header
                    || response.url().scheme() == "https" && next.scheme() == "http")
                {
                    let mut referer = response.url().clone();
                    let _ = referer.set_username("");
                    let _ = referer.set_password(None);
                    referer.set_fragment(None);
                    current.request["headers"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|header| header["name"] != "referer");
                    current.request["headers"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"name":"referer", "value":referer.as_str()}));
                }
                current.request["url"] = json!(next.as_str());
                current.request["params"] = json!([]);
                current.request.as_object_mut().unwrap().remove("auth");
                if reset_body {
                    if current.request["method"] != "HEAD" {
                        current.request["method"] = json!("GET");
                    }
                    current.request.as_object_mut().unwrap().remove("body");
                    current.request["headers"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|header| {
                            !matches!(
                                header["name"].as_str(),
                                Some(
                                    "content-type"
                                        | "content-length"
                                        | "content-encoding"
                                        | "transfer-encoding"
                                )
                            )
                        });
                }
                (response, replayable, sent_headers) = send_request(&current, &events)
                    .await
                    .context("重定向请求失败")?;
            }
        }
        let status = response.status().as_u16();
        let headers: Vec<_> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_owned()))
            .collect();
        emit(
            &events,
            Event::Started {
                status,
                headers: headers.clone(),
            },
        );
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        let mut decoded = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            ensure!(
                bytes.len() + chunk.len() <= execution.settings.max_response_bytes,
                "响应超过大小限制（{} 字节）",
                execution.settings.max_response_bytes
            );
            bytes.extend_from_slice(&chunk);
            let valid = match std::str::from_utf8(&bytes[decoded..]) {
                Ok(s) => s.len(),
                Err(e) => e.valid_up_to(),
            };
            if valid > 0 {
                emit(
                    &events,
                    Event::Chunk {
                        text: String::from_utf8_lossy(&bytes[decoded..decoded + valid])
                            .into_owned(),
                        received_bytes: bytes.len(),
                    },
                );
                decoded += valid;
            }
        }
        let elapsed = start.elapsed().as_millis();
        let binary = std::str::from_utf8(&bytes).is_err();
        let mut response = json!({"status":status,"headers":headers.iter().cloned().collect::<BTreeMap<_,_>>(),"body":String::from_utf8_lossy(&bytes),"time":elapsed});
        let original_headers = response["headers"].clone();
        execution.hooks("after-response", Some(&mut response))?;
        for action in execution
            .actions
            .clone()
            .iter()
            .filter(|a| a["disabled"] != true)
        {
            ensure!(
                action["type"] == "set-variable" && action["selector"]["method"] == "jsonq",
                "不支持的响应 action"
            );
            ensure!(
                action["phase"].as_str().unwrap_or("after-response") == "after-response",
                "before-request action 尚未实现"
            );
            let expression = action["selector"]["expression"]
                .as_str()
                .context("action 缺少 expression")?;
            let name = action["variable"]["name"]
                .as_str()
                .context("action 缺少变量名")?;
            let target = action["variable"]["scope"].as_str().unwrap_or("runtime");
            ensure!(
                matches!(target, "runtime" | "environment" | "request"),
                "action 作用域尚未实现：{target}"
            );
            let code = if target == "request" {
                format!("state.vars[{}]=res.jq({});", json!(name), json!(expression))
            } else {
                format!(
                    "{}.set({},res.jq({}));",
                    if target == "environment" {
                        "api.env"
                    } else {
                        "api.vars"
                    },
                    json!(name),
                    json!(expression)
                )
            };
            scripts::run(
                &code,
                &mut execution.request,
                Some(&mut response),
                &mut execution.env,
                &mut execution.variables,
                &mut execution.mutations,
                &mut execution.report,
            )?;
        }
        execution.hooks("tests", Some(&mut response))?;
        let code = crate::testing::assertion_script(
            &execution.assertions,
            &execution.env,
            &execution.variables,
        )?;
        scripts::run(
            &code,
            &mut execution.request,
            Some(&mut response),
            &mut execution.env,
            &mut execution.variables,
            &mut execution.mutations,
            &mut execution.report,
        )?;
        settings::persist_variables(
            &execution.root,
            execution.environment.as_deref(),
            &execution.mutations,
        )?;
        emit(&events, Event::Finished);
        let status = u16::try_from(
            response["status"]
                .as_u64()
                .context("响应脚本设置了无效 status")?,
        )?;
        let headers = if response["headers"] == original_headers {
            headers
        } else {
            response["headers"]
                .as_object()
                .context("响应脚本设置了无效 headers")?
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()),
                    )
                })
                .collect()
        };
        Ok((
            Response {
                status,
                headers,
                body: response["body"].as_str().unwrap_or("").into(),
                body_base64: binary
                    .then(|| base64::engine::general_purpose::STANDARD.encode(&bytes)),
                elapsed_ms: elapsed,
                bytes: bytes.len(),
                logs: execution.report.logs,
                tests: execution.report.tests,
            },
            execution.report.variables,
        ))
    };
    tokio::select! {
        result=future=>result,
        _=async {loop{if cancel.load(Ordering::Relaxed){break;}tokio::time::sleep(Duration::from_millis(40)).await;}}=>bail!("请求已取消"),
        _=async {if timeout==0 {std::future::pending::<()>().await;}else{tokio::time::sleep(Duration::from_millis(timeout)).await;}}=>bail!("请求超过 {timeout} ms"),
    }
}
pub(crate) fn emit(sender: &Option<tokio::sync::mpsc::UnboundedSender<Event>>, event: Event) {
    if let Some(sender) = sender {
        let _ = sender.send(event);
    }
}

pub(crate) fn graphql_http_request(request: &mut Value) -> Result<()> {
    let body = &request["body"];
    let query = body["query"].as_str().context("GraphQL 缺少 query")?;
    let variables: Value = serde_json::from_str(
        body["variables"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("{}"),
    )
    .context("GraphQL variables 必须是 JSON")?;
    ensure!(variables.is_object(), "GraphQL variables 必须是对象");
    let payload = json!({"query":query,"variables":variables});
    if request["method"] == "GET" {
        if !request["params"].is_array() {
            request["params"] = json!([]);
        }
        request["params"].as_array_mut().unwrap().extend([
            json!({"name":"query","value":payload["query"],"type":"query"}),
            json!({"name":"variables","value":payload["variables"].to_string(),"type":"query"}),
        ]);
        request.as_object_mut().unwrap().remove("body");
    } else {
        request["body"] = json!({"type":"json","data":payload.to_string()});
    }
    Ok(())
}
