use super::{Call, Config, tools};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, watch};
const VERSIONS: &[&str] = &["2025-03-26", "2025-06-18", "2025-11-25"];
#[derive(Clone)]
struct Server {
    config: Config,
    calls: mpsc::Sender<Call>,
    stopped: watch::Receiver<bool>,
    generation: u64,
}
pub(super) fn router(
    config: Config,
    calls: mpsc::Sender<Call>,
    stopped: watch::Receiver<bool>,
    generation: u64,
) -> Router {
    Router::new()
        .route("/mcp", any(handle))
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .with_state(Server {
            config,
            calls,
            stopped,
            generation,
        })
}
fn error(id: Value, code: i32, message: &str) -> Response {
    Json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})).into_response()
}
async fn handle(
    State(mut state): State<Server>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let host = format!("127.0.0.1:{}", state.config.port);
    if headers.get("host").and_then(|v| v.to_str().ok()) != Some(&host) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if headers
        .get("origin")
        .is_some_and(|v| v.to_str().ok() != Some(format!("http://{host}").as_str()))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    if headers.get_all("authorization").iter().count() != 1 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if headers.get("authorization").and_then(|v| v.to_str().ok())
        != Some(format!("Bearer {}", state.config.token).as_str())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if headers
        .get("mcp-protocol-version")
        .is_some_and(|v| !v.to_str().ok().is_some_and(|v| VERSIONS.contains(&v)))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if *state.stopped.borrow() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if method != Method::POST {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| v.split(';').next() != Some("application/json"))
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let message: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(Value::Null, -32700, "Parse error"),
    };
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    if message["jsonrpc"] != "2.0"
        || !message.is_object()
        || !(id.is_null() || id.is_string() || id.is_number())
    {
        return error(Value::Null, -32600, "Invalid request");
    }
    let Some(method) = message["method"].as_str() else {
        return error(id, -32600, "Missing method");
    };
    if message.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let result = match method {
        "initialize" => {
            let Some(version) = message
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
            else {
                return error(id, -32602, "Missing protocolVersion");
            };
            json!({"protocolVersion":if VERSIONS.contains(&version){version}else{"2025-11-25"},"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"apifuck-native","version":env!("CARGO_PKG_VERSION")},"instructions":"Controls the open native ApiFuck workspace. Start with get_workspace. Read documents before updating; use expectedSource to prevent lost edits. All sends use saved YAML and core scripts. Poll get_job for long requests. Keep the desktop running."})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":tools::list()}),
        "tools/call" => {
            let Some(name) = message.pointer("/params/name").and_then(Value::as_str) else {
                return error(id, -32602, "Missing tool name");
            };
            let arguments = message
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or(json!({}));
            if let Err(e) = tools::validate(name, &arguments) {
                return error(id, -32602, &e.to_string());
            }
            let (reply, response) = oneshot::channel();
            if state
                .calls
                .try_send(Call {
                    name: name.into(),
                    arguments,
                    generation: state.generation,
                    reply,
                })
                .is_err()
            {
                return error(id, -32000, "Desktop is busy or unavailable");
            }
            let result = tokio::select! {
                reply=tokio::time::timeout(Duration::from_secs(120),response)=>match reply {
                    Ok(Ok(result))=>result,
                    Ok(Err(_))=>Err("Desktop closed the call".into()),
                    Err(_)=>Err("Desktop timed out; inspect workspace before retrying a write".into()),
                },
                _=state.stopped.changed()=>Err("MCP was stopped".into()),
            };
            match result {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":value.to_string()}],"isError":false})
                }
                Err(message) => json!({"content":[{"type":"text","text":message}],"isError":true}),
            }
        }
        _ => return error(id, -32601, "Method not found"),
    };
    Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response()
}
