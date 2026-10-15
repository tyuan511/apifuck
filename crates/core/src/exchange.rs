//! Import/export is data transformation only: it never executes shell commands.
use crate::{
    collection::{Document, selected},
    engine::{Execution, asset_path, headers_and_url, interpolate},
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

pub fn is_curl(source: &str) -> bool {
    source.split_whitespace().next() == Some("curl")
}

pub fn import_curl_into(source: &str, command: &str, name: &str) -> Result<String> {
    let mut current: Value = serde_yaml::from_str(source)?;
    ensure!(
        current["info"]["type"] == "http",
        "cURL 导入仅支持 HTTP 请求"
    );
    let imported: Value = serde_yaml::from_str(&import_curl(command, name)?)?;
    current["http"] = imported["http"].clone();
    current["info"]["name"] = json!(name);
    if !current["settings"].is_object() {
        current["settings"] = json!({});
    }
    current["settings"]["followRedirects"] = imported["settings"]["followRedirects"].clone();
    crate::editing::update(source, &current)
}

pub fn import_curl(source: &str, name: &str) -> Result<String> {
    let words = shell_words::split(&source.replace("\\\r\n", " ").replace("\\\n", " "))?;
    ensure!(
        words.first().is_some_and(|s| s == "curl"),
        "请输入 curl 命令"
    );
    let mut method = String::from("GET");
    let mut explicit = false;
    let mut url = None;
    let mut headers = vec![];
    let mut data = vec![];
    let mut parts = vec![];
    let mut auth = Value::Null;
    let mut query = false;
    let mut redirects = false;
    let mut json_data = false;
    let mut ix = 1;
    while ix < words.len() {
        let word = &words[ix];
        ix += 1;
        let (arg, mut inline) = if word.starts_with("--") && word.contains('=') {
            let (key, value) = word.split_once('=').unwrap();
            (key.to_owned(), Some(value.to_owned()))
        } else if word.len() > 2
            && ["-X", "-H", "-u", "-d", "-F", "-b"]
                .iter()
                .any(|prefix| word.starts_with(prefix))
        {
            (word[..2].into(), Some(word[2..].into()))
        } else {
            (word.clone(), None)
        };
        let mut next = || -> Result<String> {
            if let Some(value) = inline.take() {
                return Ok(value);
            }
            let value = words.get(ix).context("curl 参数缺少值")?.clone();
            ix += 1;
            Ok(value)
        };
        match arg.as_str() {
            "-X" | "--request" => {
                method = next()?;
                explicit = true;
            }
            "--url" => url = Some(next()?),
            "-H" | "--header" => {
                let h = next()?;
                let (name, value) = h.split_once(':').context("Header 必须使用 name: value")?;
                headers.push(json!({"name":name.trim(),"value":value.trim()}));
            }
            "-u" | "--user" => {
                let u = next()?;
                let (user, password) = u.split_once(':').unwrap_or((&u, ""));
                auth = json!({"type":"basic","username":user,"password":password});
            }
            "-b" | "--cookie" => {
                let value = next()?;
                ensure!(value.contains('='), "Cookie 文件导入尚未支持");
                headers.push(json!({"name":"Cookie","value":value}));
            }
            "--data-urlencode" => {
                let value = next()?;
                ensure!(
                    value.contains('=') || !value.contains('@'),
                    "--data-urlencode 文件引用尚未支持"
                );
                let (name, value) = value.split_once('=').unwrap_or(("", &value));
                let encoded: String =
                    url::form_urlencoded::byte_serialize(value.as_bytes()).collect();
                data.push(json!({"type":"text","data":if name.is_empty() { encoded } else { format!("{name}={encoded}") }}));
            }
            "-d" | "--data" | "--data-ascii" | "--data-raw" | "--data-binary" | "--json" => {
                json_data |= arg == "--json";
                let value = next()?;
                if value.starts_with('@') && arg != "--data-raw" {
                    data.push(json!({"type":"file","data":[{"filePath":&value[1..],"contentType":"application/octet-stream","selected":true}]}));
                } else {
                    data.push(json!({"type":if arg=="--json" {"json"}else{"text"},"data":value}));
                }
            }
            "-F" | "--form" | "--form-string" => {
                let value = next()?;
                let (name, val) = value.split_once('=').context("表单必须使用 name=value")?;
                parts.push(
                    if let Some(path) = val.strip_prefix('@').filter(|_| arg != "--form-string") {
                        json!({"name":name,"type":"file","value":path})
                    } else {
                        json!({"name":name,"type":"text","value":val})
                    },
                );
            }
            "-G" | "--get" => query = true,
            "-I" | "--head" => {
                method = "HEAD".into();
                explicit = true;
            }
            "-s" | "--silent" | "-S" | "--show-error" | "--compressed" | "-i" | "--include" => {}
            "-L" | "--location" => redirects = true,
            s if !s.starts_with('-') => {
                ensure!(url.is_none(), "暂不支持一次导入多个 URL");
                url = Some(s.to_owned());
            }
            other => bail!("暂不支持 curl 参数：{other}，未创建不完整请求"),
        }
    }
    let mut url = url.context("curl 缺少 URL")?;
    if let Some(header) = headers.iter().find(|h| {
        h["name"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("authorization"))
    }) {
        let value = header["value"].as_str().unwrap_or("");
        // An explicit Authorization header takes priority over curl -u.
        auth = Value::Null;
        if let Some((scheme, token)) = value.split_once(' ') {
            if scheme.eq_ignore_ascii_case("bearer") {
                auth = json!({"type":"bearer", "token":token.trim()});
            } else if scheme.eq_ignore_ascii_case("basic") {
                use base64::Engine as _;
                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(token.trim())
                    && let Ok(credentials) = String::from_utf8(bytes)
                    && let Some((user, password)) = credentials.split_once(':')
                {
                    auth = json!({"type":"basic", "username":user,"password":password});
                }
            }
        }
        if !auth.is_null() {
            headers.retain(|h| {
                !h["name"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case("authorization"))
            });
        }
    }
    if json_data {
        for name in ["Content-Type", "Accept"] {
            if !headers.iter().any(|h| {
                h["name"]
                    .as_str()
                    .is_some_and(|key| key.eq_ignore_ascii_case(name))
            }) {
                headers.push(json!({"name":name,"value":"application/json"}));
            }
        }
    }
    if !query
        && !data.is_empty()
        && data[0]["type"] != "json"
        && !headers.iter().any(|h| {
            h["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
        })
    {
        headers.push(json!({"name":"Content-Type","value":"application/x-www-form-urlencoded"}));
    }
    let mut http = json!({"method":method,"url":url,"headers":headers});
    if !auth.is_null() {
        http["auth"] = auth;
    }
    if query {
        let (base, fragment) = url
            .split_once('#')
            .map(|(a, b)| (a.to_owned(), format!("#{b}")))
            .unwrap_or((url.clone(), String::new()));
        url = base;
        for item in &data {
            ensure!(item["type"] != "file", "-G 不支持文件正文");
            url.push(if url.contains('?') { '&' } else { '?' });
            url.push_str(item["data"].as_str().unwrap_or(""));
        }
        url.push_str(&fragment);
        http["url"] = json!(url);
    } else if !data.is_empty() || !parts.is_empty() {
        if !explicit {
            http["method"] = json!("POST");
        }
        ensure!(
            data.is_empty() || parts.is_empty(),
            "不能混合 data 与 multipart"
        );
        if !parts.is_empty() {
            http["body"] = json!({"type":"multipart-form","data":parts});
        } else {
            if data.len() > 1 {
                ensure!(
                    data.iter().all(|d| d["type"] != "file"),
                    "多个 data 参数不能包含文件正文"
                );
                let text = data
                    .iter()
                    .filter_map(|d| d["data"].as_str())
                    .collect::<Vec<_>>()
                    .join(if json_data { "" } else { "&" });
                data = vec![json!({"type":if json_data {"json"} else {"text"}, "data":text})];
            }
            let mut body = data.remove(0);
            if headers.iter().any(|h| {
                h["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
                    && h["value"]
                        .as_str()
                        .is_some_and(|v| v.contains("application/json"))
            }) && body["type"] == "text"
            {
                body["type"] = json!("json");
            }
            if body["type"] == "file"
                && let Some(content_type) = headers.iter().find(|h| {
                    h["name"]
                        .as_str()
                        .is_some_and(|n| n.eq_ignore_ascii_case("content-type"))
                })
            {
                body["data"][0]["contentType"] = content_type["value"].clone();
            }
            http["body"] = body;
        }
    }
    let source = serde_yaml::to_string(
        &json!({"info":{"name":name,"type":"http","seq":1},"http":http,"settings":{"followRedirects":redirects}}),
    )?;
    Document::parse("import.yml".into(), source.clone())?;
    Ok(source)
}

pub fn export(execution: &Execution, language: &str) -> Result<String> {
    ensure!(!execution.is_websocket(), "代码导出仅支持 HTTP");
    let mut request = execution.request().clone();
    let mut vars = execution.env.clone();
    for (k, v) in execution.variables.as_object().into_iter().flatten() {
        vars[k] = v.clone();
    }
    fn walk(v: &mut Value, vars: &Value) -> Result<()> {
        match v {
            Value::String(s) => *s = interpolate(s, vars)?,
            Value::Array(a) => {
                for v in a {
                    walk(v, vars)?;
                }
            }
            Value::Object(o) => {
                if o.get("disabled") == Some(&Value::Bool(true)) {
                    return Ok(());
                }
                for (_, v) in o {
                    walk(v, vars)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&mut request, &vars)?;
    crate::engine::resolve_request_url(&mut request, &vars)?;
    if execution.graphql {
        crate::engine::graphql_http_request(&mut request)?;
    }
    let (url, headers) = headers_and_url(&request)?;
    let method = request["method"].as_str().context("缺少 HTTP 方法")?;
    let body = selected(&request["body"], "body")?;
    let mut header_pairs: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    let body_type = body["type"].as_str().unwrap_or("");
    if !header_pairs
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        && let Some(typ) = match body_type {
            "json" => Some("application/json"),
            "xml" => Some("application/xml"),
            "text" => Some("text/plain"),
            "sparql" => Some("application/sparql-query"),
            "file" => body["data"]
                .as_array()
                .and_then(|files| files.iter().find(|f| f["selected"] == true))
                .and_then(|f| f["contentType"].as_str()),
            "form-urlencoded" => Some("application/x-www-form-urlencoded"),
            _ => None,
        }
    {
        header_pairs.push(("Content-Type".into(), typ.into()));
    }
    let raw = if body_type == "form-urlencoded" {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for e in body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["disabled"] != true)
        {
            form.append_pair(
                e["name"].as_str().unwrap_or(""),
                e["value"].as_str().unwrap_or(""),
            );
        }
        form.finish()
    } else {
        body["data"].as_str().unwrap_or("").into()
    };
    if language == "curl" {
        let mut args = vec![
            "curl".to_owned(),
            "--request".into(),
            method.into(),
            url.to_string(),
        ];
        for (k, v) in &header_pairs {
            args.extend(["--header".into(), format!("{k}: {v}")]);
        }
        match body_type {
            "multipart-form" => {
                for e in body["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["disabled"] != true)
                {
                    let values: Vec<_> = if let Some(a) = e["value"].as_array() {
                        a.iter().filter_map(Value::as_str).collect()
                    } else {
                        vec![e["value"].as_str().unwrap_or("")]
                    };
                    for value in values {
                        args.extend([
                            if e["type"] == "file" {
                                "--form"
                            } else {
                                "--form-string"
                            }
                            .into(),
                            format!(
                                "{}={}{}",
                                e["name"].as_str().unwrap_or(""),
                                if e["type"] == "file" { "@" } else { "" },
                                if e["type"] == "file" {
                                    asset_path(execution, value).to_string_lossy().into_owned()
                                } else {
                                    value.to_owned()
                                }
                            ),
                        ]);
                    }
                }
            }
            "file" => {
                let file = body["data"]
                    .as_array()
                    .context("缺少文件")?
                    .iter()
                    .find(|f| f["selected"] == true)
                    .context("未选择文件")?;
                args.extend([
                    "--data-binary".into(),
                    format!(
                        "@{}",
                        asset_path(execution, file["filePath"].as_str().unwrap_or("")).display()
                    ),
                ]);
            }
            "" => {}
            _ => args.extend(["--data-raw".into(), raw]),
        }
        return Ok(shell_words::join(args));
    }
    if matches!(body_type, "multipart-form" | "file") {
        return crate::codegen::file_code(
            language,
            url.as_str(),
            method,
            &header_pairs,
            body,
            execution,
        );
    }
    if language == "python" {
        let mut names = std::collections::HashSet::new();
        ensure!(
            header_pairs
                .iter()
                .all(|(k, _)| names.insert(k.to_lowercase())),
            "Python requests 无法无损表达重复 Header，请导出 cURL"
        );
    }
    if language == "javascript" {
        ensure!(
            body_type.is_empty() || !matches!(method, "GET" | "HEAD"),
            "JavaScript fetch 不支持 GET/HEAD 正文"
        );
    }
    let q = |s: &str| serde_json::to_string(s).unwrap();
    Ok(match language {
        "javascript" => format!(
            "const response = await fetch({}, {{\n  method: {},\n  headers: {},\n{} }});\nconsole.log(await response.text());\n",
            q(url.as_str()),
            q(method),
            serde_json::to_string_pretty(&header_pairs)?,
            if body_type.is_empty() {
                String::new()
            } else {
                format!("  body: {},\n", q(&raw))
            }
        ),
        "python" => format!(
            "import requests\n\nresponse = requests.request({}, {}, headers={}, data={})\nprint(response.status_code)\nprint(response.text)\n",
            q(method),
            q(url.as_str()),
            serde_json::to_string(
                &header_pairs
                    .iter()
                    .cloned()
                    .collect::<std::collections::BTreeMap<_, _>>()
            )?,
            if body_type.is_empty() {
                "None".into()
            } else {
                format!("{}.encode('utf-8')", q(&raw))
            }
        ),
        "go" => format!(
            "package main\n\nimport (\"fmt\"; \"io\"; \"net/http\"; \"strings\")\n\nfunc main() {{\n req, err := http.NewRequest({}, {}, strings.NewReader({}))\n if err != nil {{ panic(err) }}\n{} resp, err := http.DefaultClient.Do(req)\n if err != nil {{ panic(err) }}\n defer resp.Body.Close()\n body, err := io.ReadAll(resp.Body)\n if err != nil {{ panic(err) }}\n fmt.Println(string(body))\n}}\n",
            q(method),
            q(url.as_str()),
            q(&raw),
            header_pairs
                .iter()
                .map(|(k, v)| format!(" req.Header.Add({}, {})\n", q(k), q(v)))
                .collect::<String>()
        ),
        _ => bail!("导出语言必须为 curl/javascript/python/go"),
    })
}
