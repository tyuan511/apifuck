use super::*;
use anyhow::{bail, ensure};

fn resolve(root: &Value, value: &Value) -> Result<Value> {
    let mut value = value.clone();
    let mut seen = std::collections::HashSet::new();
    while let Some(reference) = value["$ref"].as_str().map(str::to_owned) {
        ensure!(
            reference.starts_with("#/"),
            "仅支持文档内 $ref，不读取外部引用：{reference}"
        );
        ensure!(
            seen.len() < 32 && seen.insert(reference.clone()),
            "$ref 存在循环或过深：{reference}"
        );
        let target = root
            .pointer(&reference[1..])
            .with_context(|| format!("找不到引用：{reference}"))?;
        let mut next = target.clone();
        if let (Some(target), Some(siblings)) = (next.as_object_mut(), value.as_object()) {
            for (key, value) in siblings {
                if key != "$ref" {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        value = next;
    }
    Ok(value)
}
fn sample(root: &Value, schema: &Value, depth: usize) -> Result<Value> {
    ensure!(depth < 16, "Schema 示例嵌套过深或递归，请提供显式 example");
    let schema = resolve(root, schema)?;
    for key in ["example", "default", "const"] {
        if let Some(value) = schema.get(key) {
            return Ok(value.clone());
        }
    }
    for key in ["enum", "examples"] {
        if let Some(value) = schema[key].as_array().and_then(|v| v.first()) {
            return Ok(value.clone());
        }
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(variant) = schema[key].as_array().and_then(|variants| variants.first()) {
            return sample(root, variant, depth + 1);
        }
    }
    if let Some(parts) = schema["allOf"].as_array() {
        let mut result = serde_json::Map::new();
        for part in parts {
            let value = sample(root, part, depth + 1)?;
            let object = value.as_object().context("非对象 allOf 需要显式 example")?;
            result.extend(object.clone());
        }
        // A schema may declare properties alongside allOf.
        if schema.get("properties").is_some() {
            let mut own = schema.clone();
            own.as_object_mut().unwrap().remove("allOf");
            let own = sample(root, &own, depth + 1)?;
            if let Some(object) = own.as_object() {
                result.extend(object.clone());
            }
        }
        return Ok(result.into());
    }
    match schema["type"]
        .as_str()
        .unwrap_or(if schema["properties"].is_object() {
            "object"
        } else {
            "string"
        }) {
        "object" => {
            let mut result = serde_json::Map::new();
            for (key, value) in schema["properties"].as_object().into_iter().flatten() {
                let value = resolve(root, value)?;
                if value["readOnly"] != true {
                    result.insert(key.clone(), sample(root, &value, depth + 1)?);
                }
            }
            Ok(result.into())
        }
        "array" => Ok(json!([sample(root, &schema["items"], depth + 1)?])),
        "integer" | "number" => Ok(json!(0)),
        "boolean" => Ok(json!(false)),
        "null" => Ok(Value::Null),
        _ => Ok(json!("")),
    }
}
fn example(root: &Value, node: &Value) -> Result<Value> {
    if let Some(value) = node.get("example") {
        return Ok(value.clone());
    }
    if let Some(value) = node["examples"]
        .as_object()
        .and_then(|examples| examples.values().next())
    {
        let value = resolve(root, value)?;
        ensure!(value.get("externalValue").is_none(), "不读取外部 example");
        if let Some(value) = value.get("value") {
            return Ok(value.clone());
        }
    }
    sample(root, node.get("schema").unwrap_or(node), 0)
}
fn server(root: &Value, node: &Value, fallback: &str, plan: &mut ImportPlan) -> Result<String> {
    let Some(servers) = node["servers"].as_array() else {
        return Ok(fallback.into());
    };
    let Some(server) = servers.first() else {
        return Ok(fallback.into());
    };
    if servers.len() > 1 {
        plan.warn("OpenAPI 含多个 servers，已选择每个作用域的第一个 server。");
    }
    let mut url = server["url"]
        .as_str()
        .context("server 缺少 URL")?
        .to_owned();
    for (key, value) in server["variables"].as_object().into_iter().flatten() {
        let value = value["default"]
            .as_str()
            .context("server variable 缺少 default")?;
        url = url.replace(&format!("{{{key}}}"), value);
    }
    if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("{{") {
        plan.warn("相对或缺省 server 需要填写集合变量 base_url。");
        url = format!("{{{{base_url}}}}/{}", url.trim_start_matches('/'));
    }
    let _ = root;
    Ok(url.trim_end_matches('/').into())
}
pub(super) fn convert(root: &Value, plan: &mut ImportPlan) -> Result<()> {
    let version = root["openapi"]
        .as_str()
        .or_else(|| root["swagger"].as_str())
        .unwrap_or("");
    let v2 = version == "2.0";
    ensure!(
        v2 || version.starts_with("3.0.") || version.starts_with("3.1."),
        "仅支持 OpenAPI 2.0、3.0 和 3.1"
    );
    plan.format = format!("OpenAPI {version}");
    plan.name = root["info"]["title"]
        .as_str()
        .unwrap_or("OpenAPI import")
        .into();
    plan.warn("生成的参数和正文优先使用 example/default；oneOf/anyOf 选择首个变体，allOf 合并对象样例。生成值需检查，响应 Schema 未转换为断言。");
    if root.get("webhooks").is_some() {
        plan.warn("OpenAPI webhooks 未导入为主动请求。");
    }
    let base = if v2 {
        if let Some(host) = root["host"].as_str() {
            format!(
                "{}://{}{}",
                root["schemes"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("https"),
                host,
                root["basePath"].as_str().unwrap_or("")
            )
        } else {
            plan.warn("缺少 host，需要填写集合变量 base_url。");
            format!(
                "{{{{base_url}}}}{}",
                root["basePath"].as_str().unwrap_or("")
            )
        }
    } else {
        server(root, root, "{{base_url}}", plan)?
    };
    plan.push("opencollection.yml".into(),json!({"opencollection":"1.0.0","info":{"name":plan.name},"docs":description(&root["info"]["description"])}))?;
    let paths = root["paths"].as_object().context("OpenAPI 缺少 paths")?;
    let mut folders = std::collections::BTreeMap::new();
    let mut sequence = 0;
    for (path, node) in paths {
        if path.starts_with("x-") {
            continue;
        }
        ensure!(path.starts_with('/'), "OpenAPI 路径必须以 / 开始");
        let node = resolve(root, node)?;
        let path_base = server(root, &node, &base, plan)?;
        for method in [
            "get", "post", "put", "patch", "delete", "head", "options", "trace",
        ] {
            let Some(operation) = node.get(method) else {
                continue;
            };
            sequence += 1;
            let name = operation["summary"]
                .as_str()
                .or_else(|| operation["operationId"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{} {path}", method.to_uppercase()));
            let base = server(root, operation, &path_base, plan)?;
            let url = format!("{}{path}", base.trim_end_matches('/'));
            let mut parameters = vec![];
            for owner in [&node, operation] {
                for parameter in owner["parameters"].as_array().into_iter().flatten() {
                    let parameter = resolve(root, parameter)?;
                    parameters.retain(|old: &Value| {
                        !(old["name"] == parameter["name"] && old["in"] == parameter["in"])
                    });
                    parameters.push(parameter);
                }
            }
            let mut params = vec![];
            let mut headers = vec![];
            let mut body = None;
            let mut form = vec![];
            for parameter in parameters {
                let key = parameter["name"].as_str().context("参数缺少 name")?;
                let location = parameter["in"].as_str().context("参数缺少 in")?;
                if location == "body" {
                    ensure!(body.is_none(), "重复正文参数");
                    body = Some(
                        json!({"type":"json","data":serde_json::to_string_pretty(&example(root,&parameter)?)?}),
                    );
                    continue;
                }
                let value = example(root, &parameter)?;
                if location == "formData" {
                    let file = parameter["type"] == "file";
                    if file {
                        plan.warn("上传文件字段需要手动选择文件。");
                    }
                    form.push(
                        json!({"name":key,"value":text(&value),"type":if file{"file"}else{"text"}}),
                    );
                    continue;
                }
                ensure!(
                    matches!(location, "query" | "path" | "header"),
                    "不支持的参数位置：{location}"
                );
                ensure!(!value.is_object(), "对象参数需要手动指定序列化：{key}");
                ensure!(
                    parameter["allowReserved"] != true,
                    "allowReserved 参数序列化暂未支持：{key}"
                );
                let style = parameter["style"]
                    .as_str()
                    .unwrap_or(if location == "query" {
                        "form"
                    } else {
                        "simple"
                    });
                ensure!(
                    style
                        == if location == "query" {
                            "form"
                        } else {
                            "simple"
                        },
                    "暂不支持参数 style={style}：{key}"
                );
                let values = if let Some(array) = value.as_array() {
                    let multi = location == "query"
                        && if v2 {
                            parameter["collectionFormat"] == "multi"
                        } else {
                            parameter["explode"] != false
                        };
                    let format = parameter["collectionFormat"].as_str().unwrap_or("csv");
                    ensure!(
                        !v2 || matches!(format, "csv" | "multi"),
                        "不支持 collectionFormat={format}"
                    );
                    if multi {
                        array.iter().map(text).collect()
                    } else {
                        vec![array.iter().map(text).collect::<Vec<_>>().join(",")]
                    }
                } else {
                    vec![text(&value)]
                };
                for value in values {
                    let mut row = json!({"name":key,"value":value,"description":description(&parameter["description"])});
                    if location == "header" {
                        headers.push(row);
                    } else {
                        row["type"] = json!(location);
                        params.push(row);
                    }
                }
            }
            if let Some(request_body) = operation.get("requestBody") {
                let request_body = resolve(root, request_body)?;
                body = Some(content_body(
                    root,
                    &request_body["content"],
                    &mut headers,
                    plan,
                )?);
            } else if !form.is_empty() {
                let consumes = operation.get("consumes").unwrap_or(&root["consumes"]);
                let multipart = consumes
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|v| v == "multipart/form-data")
                    || form.iter().any(|v| v["type"] == "file");
                if !multipart {
                    for row in &mut form {
                        row.as_object_mut().unwrap().remove("type");
                    }
                }
                body = Some(
                    json!({"type":if multipart{"multipart-form"}else{"form-urlencoded"},"data":form}),
                );
            } else if v2 && body.is_some() {
                let media = operation
                    .get("consumes")
                    .unwrap_or(&root["consumes"])
                    .as_array()
                    .and_then(|v| v.first())
                    .and_then(Value::as_str)
                    .unwrap_or("application/json");
                ensure!(
                    media.contains("json"),
                    "Swagger body 的非 JSON consumes 需要显式转换：{media}"
                );
                headers.push(json!({"name":"Content-Type","value":media}));
            }
            let security = operation.get("security").unwrap_or(&root["security"]);
            let auth = auth(root, security, v2, plan)?;
            let mut http = json!({"method":method.to_uppercase(),"url":url,"params":params,"headers":headers,"auth":auth});
            if let Some(body) = body {
                http["body"] = body;
            }
            if operation.get("callbacks").is_some() {
                plan.warn("OpenAPI callbacks 未导入为主动请求。");
            }
            let parent = if let Some(tag) = operation["tags"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(Value::as_str)
            {
                if !folders.contains_key(tag) {
                    let folder = PathBuf::from(format!("folder_{}", uuid::Uuid::new_v4().simple()));
                    plan.push(
                        folder.join("folder.yml"),
                        json!({"info":{"name":tag,"type":"folder","seq":folders.len()+1}}),
                    )?;
                    folders.insert(tag.to_owned(), folder);
                }
                folders[tag].clone()
            } else {
                PathBuf::new()
            };
            plan.push(parent.join(filename("api")),json!({"info":{"name":name,"type":"http","seq":sequence},"http":http,"docs":description(&operation["description"])}))?;
        }
    }
    Ok(())
}
fn content_body(
    root: &Value,
    content: &Value,
    headers: &mut Vec<Value>,
    plan: &mut ImportPlan,
) -> Result<Value> {
    let content = content.as_object().context("requestBody 缺少 content")?;
    let (media, node) = content
        .iter()
        .find(|(key, _)| key.as_str() == "application/json")
        .or_else(|| content.iter().next())
        .context("requestBody content 为空")?;
    if content.len() > 1 {
        plan.warn("requestBody 含多种媒体类型，已选择 JSON 或第一个媒体类型。");
    }
    ensure!(
        node.get("encoding").is_none(),
        "requestBody encoding 自定义序列化暂未支持"
    );
    let value = example(root, node)?;
    if media == "application/x-www-form-urlencoded" || media == "multipart/form-data" {
        let multipart = media == "multipart/form-data";
        let mut rows = vec![];
        let schema = resolve(root, &node["schema"])?;
        for (key, value) in value.as_object().context("表单 Schema 必须为 object")? {
            let property = resolve(root, &schema["properties"][key])?;
            let file = property["format"] == "binary";
            ensure!(multipart || !file, "文件字段需要 multipart/form-data");
            if file {
                plan.warn("上传文件字段需要手动选择文件。");
            }
            ensure!(
                !value.is_array() && !value.is_object(),
                "复杂表单字段需要手动指定编码：{key}"
            );
            let mut row = json!({"name":key,"value":text(value)});
            if multipart {
                row["type"] = json!(if file { "file" } else { "text" });
            }
            rows.push(row);
        }
        Ok(json!({"type":if multipart{"multipart-form"}else{"form-urlencoded"},"data":rows}))
    } else {
        headers.push(json!({"name":"Content-Type","value":media}));
        if media.contains("json") {
            Ok(json!({"type":"json","data":serde_json::to_string_pretty(&value)?}))
        } else {
            ensure!(value.is_string(), "非 JSON 正文需要字符串 example：{media}");
            Ok(json!({"type":if media.contains("xml"){"xml"}else{"text"},"data":value}))
        }
    }
}
fn auth(root: &Value, security: &Value, v2: bool, plan: &mut ImportPlan) -> Result<Value> {
    let Some(requirements) = security.as_array() else {
        return Ok(Value::Null);
    };
    let Some(requirement) = requirements.first() else {
        return Ok(Value::Null);
    };
    if requirements.len() > 1 {
        plan.warn("security 含多个认证备选方案，已选择第一个方案。");
    }
    let requirement = requirement.as_object().context("security 格式错误")?;
    if requirement.is_empty() {
        return Ok(Value::Null);
    }
    ensure!(requirement.len() == 1, "多个认证方案的 AND 组合暂未支持");
    let (name, scopes) = requirement.iter().next().unwrap();
    let schemes = if v2 {
        &root["securityDefinitions"]
    } else {
        &root["components"]["securitySchemes"]
    };
    let scheme = resolve(root, &schemes[name])?;
    let missing = |field: &str| format!("{{{{{name}_{field}}}}}");
    Ok(
        match scheme["type"].as_str().context("找不到 securityScheme")? {
            "basic" => {
                json!({"type":"basic","username":missing("username"),"password":missing("password")})
            }
            "http" => match scheme["scheme"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "basic" => {
                    json!({"type":"basic","username":missing("username"),"password":missing("password")})
                }
                "bearer" => json!({"type":"bearer","token":missing("token")}),
                other => bail!("不支持的 HTTP 认证：{other}"),
            },
            "apiKey" => {
                let placement = scheme["in"].as_str().unwrap_or("");
                ensure!(
                    matches!(placement, "header" | "query"),
                    "不支持的 API Key 位置：{placement}"
                );
                json!({"type":"apikey","key":scheme["name"].as_str().context("API Key 缺少 name")?,"value":missing("key"),"placement":placement})
            }
            "oauth2" => {
                let flow = if v2 {
                    match scheme["flow"].as_str() {
                        Some("application") => "clientCredentials",
                        Some("accessCode") => "authorizationCode",
                        _ => bail!("不支持的 OAuth flow"),
                    }
                } else if scheme["flows"].get("clientCredentials").is_some() {
                    "clientCredentials"
                } else {
                    "authorizationCode"
                };
                let config = if v2 { &scheme } else { &scheme["flows"][flow] };
                let mut auth = json!({"type":"oauth2","flow":if flow=="clientCredentials"{"client_credentials"}else{"authorization_code"},"accessTokenUrl":config["tokenUrl"].as_str().context("OAuth 缺少 tokenUrl")?,"credentials":{"clientId":missing("client_id"),"clientSecret":missing("client_secret"),"placement":"body"},"scope":scopes.as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")});
                if flow == "authorizationCode" {
                    auth["authorizationUrl"] = config["authorizationUrl"].clone();
                    auth["callbackUrl"] = json!("http://127.0.0.1:0/oauth/callback");
                }
                auth
            }
            other => bail!("暂不支持导入认证：{other}"),
        },
    )
}
