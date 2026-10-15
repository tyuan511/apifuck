//! Bidirectional URL/query editing shared by native controls and tests.
use serde_json::{Value, json};

/// Keep templates and relative references editable; add HTTPS to host input.
pub fn normalize(text: &str) -> String {
    let text = text.trim();
    if text.is_empty()
        || text.starts_with(['/', '?', '#'])
        || text.starts_with("{{")
        || text.contains("://")
    {
        return text.into();
    }
    if text.parse::<std::net::Ipv6Addr>().is_ok() {
        return format!("https://[{text}]");
    }
    format!("https://{text}")
}

pub fn resolve(text: &str, base: Option<&str>) -> anyhow::Result<String> {
    use anyhow::{Context, ensure};
    let text = normalize(text);
    ensure!(!text.is_empty(), "请求 URL 不能为空");
    let relative = text.starts_with(['/', '?', '#']);
    let url = if relative {
        let base = normalize(base.context("相对 URL 需要设置 Base URL")?);
        let base = url::Url::parse(&base).context("Base URL 无效")?;
        ensure!(!text.starts_with("//"), "请求路径不能覆盖 Base URL 主机");
        // A configured service prefix is retained for /path references.
        if text.starts_with('/') {
            let mut base = base;
            base.set_query(None);
            base.set_fragment(None);
            url::Url::parse(&format!("{}{text}", base.as_str().trim_end_matches('/')))?
        } else {
            base.join(&text)?
        }
    } else {
        url::Url::parse(&text).context("请求 URL 无效")?
    };
    ensure!(
        matches!(url.scheme(), "http" | "https" | "ws" | "wss") && url.host_str().is_some(),
        "请求 URL 必须使用 HTTP(S) 或 WS(S) 地址"
    );
    ensure!(
        !url.host_str().unwrap_or("").contains(['{', '}']),
        "请求 URL 包含未解析的变量"
    );
    Ok(url.into())
}

pub fn base_url(variables: &Value) -> &str {
    variables
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find(|v| v["name"] == "base_url" && v["disabled"] != true)
        .and_then(|v| v["value"].as_str())
        .unwrap_or("")
}

pub fn set_base_url(variables: &mut Value, text: &str) {
    let mut rows = variables.as_array().cloned().unwrap_or_default();
    rows.retain(|v| v["name"] != "base_url");
    if !text.trim().is_empty() {
        rows.push(json!({"name":"base_url", "value":normalize(text)}));
    }
    *variables = json!(rows);
}
pub fn split(text: &str) -> (String, Vec<Value>) {
    let (url, fragment) = text
        .split_once('#')
        .map(|(a, b)| (a, format!("#{b}")))
        .unwrap_or((text, String::new()));
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let rows = url::form_urlencoded::parse(query.as_bytes())
        .map(|(name, value)| json!({"name":name,"value":value,"type":"query"}))
        .collect();
    (format!("{base}{fragment}"), rows)
}
pub fn parameters(request: &Value) -> Value {
    let (_, mut rows) = split(request["url"].as_str().unwrap_or(""));
    rows.extend(request["params"].as_array().into_iter().flatten().cloned());
    json!(rows)
}
pub fn display(request: &Value) -> String {
    let (base, _) = split(request["url"].as_str().unwrap_or(""));
    let (base, fragment) = base
        .split_once('#')
        .map(|(a, b)| (a.to_owned(), format!("#{b}")))
        .unwrap_or((base.clone(), String::new()));
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for row in parameters(request)
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "query" && p["disabled"] != true)
    {
        query.append_pair(
            row["name"].as_str().unwrap_or(""),
            row["value"].as_str().unwrap_or(""),
        );
    }
    let query = query
        .finish()
        .replace("%7B%7B", "{{")
        .replace("%7D%7D", "}}");
    format!(
        "{base}{}{fragment}",
        if query.is_empty() {
            if request["url"]
                .as_str()
                .is_some_and(|url| url.starts_with('?'))
            {
                "?".into()
            } else {
                String::new()
            }
        } else {
            format!("?{query}")
        }
    )
}
pub fn apply_url(request: &mut Value, text: &str) {
    let text = text.trim();
    let (mut base, mut rows) = split(text);
    if text.starts_with('?') {
        base.insert(0, '?');
    }
    rows.extend(
        request["params"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["type"] == "path" || p["disabled"] == true)
            .cloned(),
    );
    request["url"] = json!(base);
    request["params"] = json!(rows);
}
pub fn apply_parameters(request: &mut Value, rows: Value) {
    let url = request["url"].as_str().unwrap_or("");
    let (mut base, _) = split(url);
    if url.starts_with('?') {
        base.insert(0, '?');
    }
    request["url"] = json!(base);
    request["params"] = rows;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_only_urls_remain_relative_through_table_edits() {
        let mut request = json!({"url":""});
        apply_url(&mut request, "?q=one#part");
        assert_eq!(request["url"], "?#part");
        assert_eq!(display(&request), "?q=one#part");
        apply_parameters(
            &mut request,
            json!([{"name":"q","value":"two","type":"query"}]),
        );
        assert_eq!(request["url"], "?#part");
        assert_eq!(display(&request), "?q=two#part");
        apply_parameters(&mut request, json!([]));
        assert_eq!(display(&request), "?#part");
    }
    #[test]
    fn normalizes_hosts_and_resolves_service_prefixes() {
        for (input, expected) in [
            (" api.example.com ", "https://api.example.com/"),
            ("localhost:3000/x", "https://localhost:3000/x"),
            ("127.0.0.1:8000", "https://127.0.0.1:8000/"),
            ("[::1]:8080", "https://[::1]:8080/"),
            ("::1", "https://[::1]/"),
            ("ws://localhost:3000/ws", "ws://localhost:3000/ws"),
        ] {
            assert_eq!(resolve(input, None).unwrap(), expected);
        }
        assert_eq!(normalize(" {{host}}/x "), "{{host}}/x");
        assert_eq!(
            resolve("/users?q=1", Some("http://localhost/api/")).unwrap(),
            "http://localhost/api/users?q=1"
        );
        assert_eq!(
            resolve("?q=1", Some("https://example.com/api")).unwrap(),
            "https://example.com/api?q=1"
        );
        for input in [
            "",
            "/users",
            "https://",
            "http://[bad",
            "file:///etc/passwd",
            "https://{{host}}",
        ] {
            assert!(resolve(input, None).is_err(), "{input}");
        }
        assert!(resolve("//other.com", Some("https://example.com")).is_err());
    }
    #[test]
    fn base_url_edits_preserve_other_variables_and_clear_to_inherit() {
        let mut vars =
            json!([{"name":"token","value":"x","disabled":true},{"name":"base_url","value":"old"}]);
        set_base_url(&mut vars, " localhost:3000/api ");
        assert_eq!(base_url(&vars), "https://localhost:3000/api");
        assert_eq!(vars[0]["disabled"], true);
        set_base_url(&mut vars, " ");
        assert_eq!(vars.as_array().unwrap().len(), 1);
    }
    #[test]
    fn combines_and_edits_without_duplicate_parameters() {
        let mut request = json!({"url":"{{host}}/users?tag=a#part","params":[{"name":"tag","value":"b","type":"query"},{"name":"skip","value":"x","type":"query","disabled":true},{"name":"id","value":"7","type":"path"}]});
        assert_eq!(display(&request), "{{host}}/users?tag=a&tag=b#part");
        apply_url(&mut request, "{{host}}/users?name={{user}}#part");
        assert_eq!(display(&request), "{{host}}/users?name={{user}}#part");
        assert_eq!(request["params"].as_array().unwrap().len(), 3);
    }
    #[test]
    fn table_edits_replace_embedded_query_and_keep_fragment() {
        let mut request = json!({"url":"https://example.com?q=old#x"});
        apply_parameters(
            &mut request,
            json!([{"name":"q","value":"hello world","type":"query"}]),
        );
        assert_eq!(display(&request), "https://example.com?q=hello+world#x");
        assert_eq!(request["url"], "https://example.com#x");
    }
}
