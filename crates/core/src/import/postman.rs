use super::*;
use anyhow::{bail, ensure};

pub(super) fn convert(root: &Value, plan: &mut ImportPlan) -> Result<()> {
    let schema = root["info"]["schema"].as_str().unwrap_or("");
    ensure!(
        schema.contains("/v2.0.0/") || schema.contains("/v2.1.0/"),
        "仅支持 Postman Collection 2.0/2.1"
    );
    plan.name = root["info"]["name"]
        .as_str()
        .unwrap_or("Postman import")
        .into();
    plan.format = "Postman".into();
    let mut defaults = json!({"variables":variables(&root["variable"])});
    if let Some(auth) = root.get("auth") {
        defaults["auth"] = auth_value(auth, plan)?;
    }
    let docs = documentation(root, plan);
    plan.push(
        "opencollection.yml".into(),
        json!({"opencollection":"1.0.0","info":{"name":plan.name},"request":defaults,"docs":docs}),
    )?;
    items(
        root["item"].as_array().context("Postman 缺少 item 列表")?,
        Path::new(""),
        plan,
        0,
        false,
    )
}
fn documentation(item: &Value, plan: &mut ImportPlan) -> String {
    let mut docs = description(
        item.get("description")
            .unwrap_or(&item["info"]["description"]),
    );
    if item["event"]
        .as_array()
        .is_some_and(|events| !events.is_empty())
    {
        plan.warn("Postman scripts/tests 已原样保存在文档中，不会自动执行；需迁移 pm API 后启用。");
        docs.push_str("\n\nPostman scripts (not executed):\n\n");
        docs.push_str(&serde_json::to_string_pretty(&item["event"]).unwrap());
    }
    if item
        .get("response")
        .is_some_and(|responses| responses.as_array().is_some_and(|r| !r.is_empty()))
    {
        plan.warn("Postman 保存的响应示例未转换为执行历史或断言。");
    }
    docs
}
fn items(
    items: &[Value],
    parent: &Path,
    plan: &mut ImportPlan,
    depth: usize,
    no_auth: bool,
) -> Result<()> {
    ensure!(depth <= 32, "Postman 文件夹层级过深");
    for (index, item) in items.iter().enumerate() {
        let name = item["name"].as_str().unwrap_or("Imported request");
        if let Some(children) = item["item"].as_array() {
            let folder = parent.join(format!("folder_{}", uuid::Uuid::new_v4().simple()));
            let mut request = json!({"variables":variables(&item["variable"])});
            if let Some(auth) = item.get("auth") {
                request["auth"] = auth_value(auth, plan)?;
            }
            let docs = documentation(item, plan);
            plan.push(folder.join("folder.yml"), json!({"info":{"name":name,"type":"folder","seq":index+1},"request":request,"docs":docs}))?;
            let no_auth = match item["auth"]["type"].as_str() {
                Some("noauth") => true,
                Some(_) => false,
                None => no_auth,
            };
            self::items(children, &folder, plan, depth + 1, no_auth)?;
            continue;
        }
        let request = &item["request"];
        ensure!(
            request.is_object() || request.is_string(),
            "Postman 请求缺少 request：{name}"
        );
        let url_value = if request.is_string() {
            request
        } else {
            &request["url"]
        };
        let mut url = url_value
            .as_str()
            .or_else(|| url_value["raw"].as_str())
            .map(str::to_owned)
            .unwrap_or_default();
        if url.is_empty() && url_value.is_object() {
            let join = |value: &Value, separator: &str| {
                value.as_str().map(str::to_owned).unwrap_or_else(|| {
                    value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(separator)
                })
            };
            let host = join(&url_value["host"], ".");
            let path = join(&url_value["path"], "/");
            ensure!(!host.is_empty(), "Postman URL 缺少主机：{name}");
            url = format!(
                "{}://{}{}/{}",
                url_value["protocol"].as_str().unwrap_or("https"),
                host,
                url_value["port"]
                    .as_str()
                    .map(|port| format!(":{port}"))
                    .unwrap_or_default(),
                path
            );
        }
        ensure!(!url.is_empty(), "Postman 请求缺少 URL：{name}");
        let mut params = vec![];
        if let Some(query) = url_value["query"].as_array() {
            let (base, fragment) = url
                .split_once('#')
                .map(|(a, b)| (a.to_owned(), format!("#{b}")))
                .unwrap_or((url.clone(), String::new()));
            url = format!("{}{fragment}", base.split('?').next().unwrap_or(""));
            for row in query {
                params.push(json!({"name":row["key"].as_str().context("Query 缺少 key")?,"value":text(&row["value"]),"type":"query","disabled":row["disabled"]==true,"description":description(&row["description"])}));
            }
        }
        for row in url_value["variable"].as_array().into_iter().flatten() {
            params.push(json!({"name":row["key"].as_str().context("Path 缺少 key")?,"value":text(&row["value"]),"type":"path","disabled":row["disabled"]==true}));
        }
        let mut headers = vec![];
        if let Some(raw) = request["header"].as_str() {
            for line in raw.lines().filter(|line| !line.trim().is_empty()) {
                let (key, value) = line.split_once(':').context("Postman Header 格式错误")?;
                headers.push(json!({"name":key.trim(),"value":value.trim()}));
            }
        } else {
            for row in request["header"].as_array().into_iter().flatten() {
                headers.push(json!({"name":row["key"].as_str().context("Header 缺少 key")?,"value":text(&row["value"]),"disabled":row["disabled"]==true,"description":description(&row["description"])}));
            }
        }
        let mut auth = request
            .get("auth")
            .map(|auth| auth_value(auth, plan))
            .transpose()?
            .unwrap_or(if no_auth {
                Value::Null
            } else {
                json!("inherit")
            });
        if no_auth && auth == "inherit" {
            auth = Value::Null;
        }
        let mut http = json!({"method":request["method"].as_str().unwrap_or("GET"),"url":url,"headers":headers,"params":params,"auth":auth});
        let body = &request["body"];
        if body["disabled"] != true {
            match body["mode"].as_str().unwrap_or("") {
                "" => {}
                "raw" => {
                    let language = body["options"]["raw"]["language"]
                        .as_str()
                        .unwrap_or("text");
                    http["body"] = json!({"type":match language {"json"=>"json","xml"=>"xml",_=>"text"},"data":body["raw"].as_str().unwrap_or("")});
                }
                "urlencoded" | "formdata" => {
                    let multipart = body["mode"] == "formdata";
                    let mut rows = vec![];
                    for row in body[if multipart { "formdata" } else { "urlencoded" }]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        let file = row["type"] == "file";
                        if file {
                            plan.warn("文件正文/上传路径未复制，请在发送前重新选择本机文件。");
                        }
                        let value = if file {
                            row["src"].clone()
                        } else {
                            json!(text(&row["value"]))
                        };
                        ensure!(
                            !file || value.is_string() || value.is_array(),
                            "Postman 文件字段缺少 src"
                        );
                        let mut output = json!({"name":row["key"].as_str().context("表单缺少 key")?,"value":value,"disabled":row["disabled"]==true});
                        if multipart {
                            output["type"] = json!(if file { "file" } else { "text" });
                            if let Some(content_type) = row["contentType"]
                                .as_str()
                                .filter(|value| !value.is_empty())
                            {
                                output["contentType"] = json!(content_type);
                            }
                        }
                        rows.push(output);
                    }
                    http["body"] = json!({"type":if multipart {"multipart-form"}else{"form-urlencoded"},"data":rows});
                }
                "file" => {
                    plan.warn("文件正文/上传路径未复制，请在发送前重新选择本机文件。");
                    http["body"] = json!({"type":"file","data":[{"filePath":body["file"]["src"].as_str().context("文件正文缺少 src")?,"selected":true,"contentType":"application/octet-stream"}]});
                }
                "graphql" => {
                    http["body"] = json!({"query":body["graphql"]["query"].as_str().unwrap_or(""),"variables":body["graphql"]["variables"].as_str().filter(|text|!text.is_empty()).unwrap_or("{}")});
                }
                mode => bail!("不支持的 Postman 正文模式：{mode}"),
            }
        }
        let mut docs = documentation(item, plan);
        docs.push_str(&description(&request["description"]));
        if item.get("protocolProfileBehavior").is_some()
            || request.get("protocolProfileBehavior").is_some()
        {
            plan.warn("Postman protocolProfileBehavior 未应用，请检查请求网络设置。");
        }
        let graphql = body["mode"] == "graphql" && body["disabled"] != true;
        let key = if graphql { "graphql" } else { "http" };
        let mut document = json!({"info":{"name":name,"type":key,"seq":index+1},"docs":docs,"runtime":{"variables":variables(&item["variable"])}});
        document[key] = http;
        plan.push(parent.join(filename("api")), document)?;
    }
    Ok(())
}
fn auth_value(auth: &Value, plan: &mut ImportPlan) -> Result<Value> {
    if auth.is_null() {
        return Ok(json!("inherit"));
    }
    let kind = auth["type"].as_str().context("Postman auth 缺少 type")?;
    let values: Value = auth[kind]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row["key"]
                .as_str()
                .map(|key| (key.to_owned(), row["value"].clone()))
        })
        .collect::<serde_json::Map<_, _>>()
        .into();
    Ok(match kind {
        "noauth" => Value::Null,
        "basic" => {
            json!({"type":"basic","username":text(&values["username"]),"password":text(&values["password"])})
        }
        "bearer" => json!({"type":"bearer","token":text(&values["token"])}),
        "apikey" => {
            let placement = values["in"].as_str().unwrap_or("header");
            ensure!(
                matches!(placement, "header" | "query"),
                "API Key 位置不受支持"
            );
            json!({"type":"apikey","key":text(&values["key"]),"value":text(&values["value"]),"placement":placement})
        }
        "oauth2" if values["accessToken"].is_string() => {
            plan.warn("Postman OAuth accessToken 已导入为 Bearer；原 OAuth 授权配置需手动设置。");
            json!({"type":"bearer","token":values["accessToken"]})
        }
        other => {
            bail!("暂不能可靠转换 Postman 认证 {other}；请先转换为 Basic/Bearer/API Key 后导入")
        }
    })
}
