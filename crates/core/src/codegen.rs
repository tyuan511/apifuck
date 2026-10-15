use crate::engine::{Execution, asset_path};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
fn q(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
#[derive(Clone)]
struct Part {
    name: String,
    value: String,
    file: bool,
    filename: String,
    mime: String,
}
fn parts(body: &Value, execution: &Execution) -> Vec<Part> {
    let mut parts = vec![];
    for (ix, part) in body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, p)| p["disabled"] != true)
    {
        let values: Vec<_> = if let Some(a) = part["value"].as_array() {
            a.iter().filter_map(Value::as_str).collect()
        } else {
            vec![part["value"].as_str().unwrap_or("")]
        };
        for value in values {
            let file = part["type"] == "file";
            let path = asset_path(execution, value);
            parts.push(Part {
                name: part["name"].as_str().unwrap_or("").into(),
                value: if file {
                    path.to_string_lossy().into_owned()
                } else {
                    value.into()
                },
                file,
                filename: execution.options["multipartFileNames"][ix.to_string()]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    }),
                mime: part["contentType"]
                    .as_str()
                    .unwrap_or(if file {
                        "application/octet-stream"
                    } else {
                        "text/plain"
                    })
                    .into(),
            });
        }
    }
    parts
}
pub(crate) fn file_code(
    language: &str,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: &Value,
    execution: &Execution,
) -> Result<String> {
    let multipart = body["type"] == "multipart-form";
    let parts = if multipart {
        parts(body, execution)
    } else {
        vec![]
    };
    let file = if !multipart {
        let selected = body["data"]
            .as_array()
            .context("缺少文件")?
            .iter()
            .find(|p| p["selected"] == true)
            .context("未选择文件")?;
        Some((
            asset_path(
                execution,
                selected["filePath"].as_str().context("缺少文件路径")?,
            )
            .to_string_lossy()
            .into_owned(),
            selected["contentType"]
                .as_str()
                .unwrap_or("application/octet-stream")
                .to_owned(),
        ))
    } else {
        None
    };
    let mut headers: Vec<_> = headers
        .iter()
        .filter(|(k, _)| !k.eq_ignore_ascii_case("content-type"))
        .cloned()
        .collect();
    if let Some((_, mime)) = &file {
        headers.push(("Content-Type".into(), mime.clone()));
    }
    match language {
        "javascript" => {
            ensure!(
                !matches!(method, "GET" | "HEAD"),
                "JavaScript fetch 不支持 GET/HEAD 正文，请使用 cURL 或 Go"
            );
            let mut code = String::from("import { readFile } from 'node:fs/promises';\n\n");
            if multipart {
                code.push_str("const body = new FormData();\n");
                for part in &parts {
                    if part.file {
                        code.push_str(&format!("body.append({}, new Blob([await readFile({})], {{ type: {} }}), {});\n",q(&part.name),q(&part.value),q(&part.mime),q(&part.filename)));
                    } else {
                        code.push_str(&format!(
                            "body.append({}, {});\n",
                            q(&part.name),
                            q(&part.value)
                        ));
                    }
                }
            } else {
                code.push_str(&format!(
                    "const body = await readFile({});\n",
                    q(&file.as_ref().unwrap().0)
                ));
            }
            code.push_str(&format!("const response = await fetch({}, {{ method: {}, headers: {}, body }});\nconsole.log(await response.text());\n",q(url),q(method),serde_json::to_string(&headers)?));
            Ok(code)
        }
        "python" => {
            let mut names = std::collections::HashSet::new();
            ensure!(
                headers.iter().all(|(k, _)| names.insert(k.to_lowercase())),
                "Python requests 无法无损表达重复 Header，请导出 cURL"
            );
            let mut code = String::from(
                "import requests\nfrom contextlib import ExitStack\n\nwith ExitStack() as stack:\n",
            );
            if multipart {
                code.push_str("    files = [\n");
                for part in &parts {
                    code.push_str(&if part.file {
                        format!(
                            "        ({}, ({}, stack.enter_context(open({}, 'rb')), {})),\n",
                            q(&part.name),
                            q(&part.filename),
                            q(&part.value),
                            q(&part.mime)
                        )
                    } else {
                        format!(
                            "        ({}, (None, {}, {})),\n",
                            q(&part.name),
                            q(&part.value),
                            q(&part.mime)
                        )
                    });
                }
                code.push_str("    ]\n");
            } else {
                code.push_str(&format!(
                    "    data = stack.enter_context(open({}, 'rb'))\n",
                    q(&file.as_ref().unwrap().0)
                ));
            }
            code.push_str(&format!("    response = requests.request({}, {}, headers={}, {}={})\n    print(response.status_code)\n    print(response.text)\n",q(method),q(url),serde_json::to_string(&headers.into_iter().collect::<std::collections::BTreeMap<_,_>>())?,if multipart{"files"}else{"data"},if multipart{"files"}else{"data"}));
            Ok(code)
        }
        "go" => {
            let mut code =
                String::from("package main\n\nimport (\n \"fmt\"\n \"io\"\n \"net/http\"\n");
            if multipart {
                code.push_str(" \"bytes\"\n \"mime/multipart\"\n");
                if !parts.is_empty() {
                    code.push_str(" \"mime\"\n \"net/textproto\"\n");
                }
                if parts.iter().any(|p| p.file) {
                    code.push_str(" \"os\"\n");
                }
            } else {
                code.push_str(" \"os\"\n");
            }
            code.push_str(")\n\nfunc main() {\n");
            if multipart {
                code.push_str(
                    " var payload bytes.Buffer\n writer := multipart.NewWriter(&payload)\n",
                );
                for (ix, part) in parts.iter().enumerate() {
                    code.push_str(&format!(" header{ix} := make(textproto.MIMEHeader)\n header{ix}.Set(\"Content-Disposition\", mime.FormatMediaType(\"form-data\", map[string]string{{\"name\": {}{}}}))\n header{ix}.Set(\"Content-Type\", {})\n part{ix}, err := writer.CreatePart(header{ix})\n if err != nil {{ panic(err) }}\n",q(&part.name),if part.file{format!(", \"filename\": {}",q(&part.filename))}else{String::new()},q(&part.mime)));
                    if part.file {
                        code.push_str(&format!(" data{ix}, err := os.ReadFile({})\n if err != nil {{ panic(err) }}\n if _, err = part{ix}.Write(data{ix}); err != nil {{ panic(err) }}\n",q(&part.value)));
                    } else {
                        code.push_str(&format!(" if _, err = io.WriteString(part{ix}, {}); err != nil {{ panic(err) }}\n",q(&part.value)));
                    }
                }
                code.push_str(" if err := writer.Close(); err != nil { panic(err) }\n");
            } else {
                code.push_str(&format!(" payload, err := os.Open({})\n if err != nil {{ panic(err) }}\n defer payload.Close()\n",q(&file.as_ref().unwrap().0)));
            }
            code.push_str(&format!(
                " req, err := http.NewRequest({}, {}, {})\n if err != nil {{ panic(err) }}\n",
                q(method),
                q(url),
                if multipart { "&payload" } else { "payload" }
            ));
            for (k, v) in headers {
                code.push_str(&format!(" req.Header.Add({}, {})\n", q(&k), q(&v)));
            }
            if multipart {
                code.push_str(" req.Header.Set(\"Content-Type\", writer.FormDataContentType())\n");
            }
            code.push_str(" resp, err := http.DefaultClient.Do(req)\n if err != nil { panic(err) }\n defer resp.Body.Close()\n body, err := io.ReadAll(resp.Body)\n if err != nil { panic(err) }\n fmt.Println(string(body))\n}\n");
            Ok(code)
        }
        _ => bail!("未知代码语言"),
    }
}
