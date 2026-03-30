use std::time::{Instant, SystemTime, UNIX_EPOCH};

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_TYPE},
    Method, Url, Version,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;

use crate::{
    error::{AppError, AppResult},
    storage::{AuthType, BodyMode, KeyValue, RequestDefinition},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseType {
    Json,
    Text,
    EventStream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequestInput {
    #[serde(default)]
    pub request_id: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub request: RequestDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequestResponse {
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<ResponseHeader>,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub content_type: String,
    pub response_type: ResponseType,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SendRequestStreamEvent {
    Started {
        request_id: String,
        status: u16,
        #[serde(default)]
        headers: Vec<ResponseHeader>,
        content_type: String,
    },
    Chunk {
        request_id: String,
        chunk: String,
        received_bytes: u64,
    },
    Finished {
        request_id: String,
        duration_ms: u64,
        size_bytes: u64,
        response_type: ResponseType,
        body: String,
    },
}

pub async fn send_request(
    input: SendRequestInput,
    on_event: Channel<SendRequestStreamEvent>,
) -> AppResult<SendRequestResponse> {
    log_sse_debug(&input.request_id, "send_request:start");
    let method = parse_method(&input.method)?;
    let mut url = parse_url(&input.url)?;
    append_enabled_query_pairs(&mut url, &input.request.query);
    append_api_key_query(&mut url, &input.request)?;

    let expects_event_stream = input
        .request
        .headers
        .iter()
        .find(|header| {
            header.enabled
                && header.key.trim().eq_ignore_ascii_case("accept")
                && !header.value.trim().is_empty()
        })
        .map(|header| is_event_stream_content_type(&header.value))
        .unwrap_or(false);

    let (headers, has_content_type) = build_headers(&input.request.headers)?;
    let client = build_http_client(expects_event_stream)?;
    let mut request_builder = client.request(method, url).headers(headers);
    request_builder = apply_auth(request_builder, &input.request);
    request_builder = apply_body(request_builder, &input.request, has_content_type)?;
    if expects_event_stream {
        request_builder = apply_event_stream_headers(request_builder);
        request_builder = request_builder.version(Version::HTTP_11);
    }

    let started_at = Instant::now();
    let mut response = request_builder
        .send()
        .await
        .map_err(|error| AppError::Request(error.to_string()))?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| ResponseHeader {
            name: name.as_str().to_string(),
            value: value.to_str().unwrap_or_default().to_string(),
        })
        .collect::<Vec<_>>();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let is_event_stream = is_event_stream_content_type(&content_type);
    if is_event_stream {
        log_sse_debug(
            &input.request_id,
            &format!("response:headers status={status} content_type={content_type}"),
        );
    }

    emit_stream_event(
        &on_event,
        &input.request_id,
        is_event_stream,
        SendRequestStreamEvent::Started {
            request_id: input.request_id.clone(),
            status,
            headers: headers.clone(),
            content_type: content_type.clone(),
        },
    )?;

    let mut body_bytes = Vec::new();
    let mut streamed_bytes = 0_u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AppError::Request(error.to_string()))?
    {
        streamed_bytes += chunk.len() as u64;
        body_bytes.extend_from_slice(&chunk);
        if is_event_stream {
            log_sse_debug(
                &input.request_id,
                &format!("response:chunk len={} received={streamed_bytes}", chunk.len()),
            );
        }

        emit_stream_event(
            &on_event,
            &input.request_id,
            is_event_stream,
            SendRequestStreamEvent::Chunk {
                request_id: input.request_id.clone(),
                chunk: String::from_utf8_lossy(&chunk).to_string(),
                received_bytes: streamed_bytes,
            },
        )?;
    }

    let size_bytes = body_bytes.len() as u64;
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let text = String::from_utf8_lossy(&body_bytes).to_string();
    let (response_type, body) = classify_response_body(&content_type, &text);
    if is_event_stream {
        log_sse_debug(
            &input.request_id,
            &format!("response:finished size={size_bytes} duration_ms={duration_ms}"),
        );
    }

    emit_stream_event(
        &on_event,
        &input.request_id,
        is_event_stream,
        SendRequestStreamEvent::Finished {
            request_id: input.request_id.clone(),
            duration_ms,
            size_bytes,
            response_type: response_type.clone(),
            body: body.clone(),
        },
    )?;

    Ok(SendRequestResponse {
        status,
        headers,
        duration_ms,
        size_bytes,
        content_type,
        response_type,
        body,
    })
}

fn parse_method(input: &str) -> AppResult<Method> {
    let normalized = input.trim().to_ascii_uppercase();
    Method::from_bytes(normalized.as_bytes())
        .map_err(|error| AppError::InvalidInput(format!("invalid request method: {error}")))
}

fn parse_url(input: &str) -> AppResult<Url> {
    Url::parse(input.trim())
        .map_err(|error| AppError::InvalidInput(format!("invalid request url: {error}")))
}

fn build_headers(entries: &[KeyValue]) -> AppResult<(HeaderMap, bool)> {
    let mut headers = HeaderMap::new();
    let mut has_content_type = false;

    for entry in entries.iter().filter(|entry| entry.enabled && !entry.key.trim().is_empty()) {
        let name = HeaderName::from_bytes(entry.key.trim().as_bytes()).map_err(|error| {
            AppError::InvalidInput(format!("invalid header name {}: {error}", entry.key))
        })?;
        let value = HeaderValue::from_str(&entry.value).map_err(|error| {
            AppError::InvalidInput(format!("invalid header value for {}: {error}", entry.key))
        })?;
        if name == CONTENT_TYPE {
            has_content_type = true;
        }
        headers.insert(name, value);
    }

    Ok((headers, has_content_type))
}

fn append_enabled_query_pairs(url: &mut Url, entries: &[KeyValue]) {
    // Clear existing query pairs first - the query table is now the source of truth
    // since URL query params are synced bidirectionally with the table
    url.query_pairs_mut().clear();
    let mut pairs = url.query_pairs_mut();
    for entry in entries.iter().filter(|entry| entry.enabled && !entry.key.trim().is_empty()) {
        pairs.append_pair(entry.key.trim(), &entry.value);
    }
}

fn append_api_key_query(url: &mut Url, request: &RequestDefinition) -> AppResult<()> {
    if request.auth.auth_type != AuthType::ApiKey {
        return Ok(());
    }
    if request.auth.api_key.add_to.trim().eq_ignore_ascii_case("query")
        && !request.auth.api_key.key.trim().is_empty()
    {
        url.query_pairs_mut()
            .append_pair(request.auth.api_key.key.trim(), &request.auth.api_key.value);
    }
    Ok(())
}

fn apply_auth(
    request_builder: reqwest::RequestBuilder,
    request: &RequestDefinition,
) -> reqwest::RequestBuilder {
    match request.auth.auth_type {
        AuthType::Basic => request_builder.basic_auth(
            request.auth.basic.username.clone(),
            Some(request.auth.basic.password.clone()),
        ),
        AuthType::Bearer => request_builder.bearer_auth(request.auth.bearer_token.clone()),
        AuthType::ApiKey => {
            if request.auth.api_key.add_to.trim().eq_ignore_ascii_case("header")
                && !request.auth.api_key.key.trim().is_empty()
            {
                request_builder.header(
                    request.auth.api_key.key.trim(),
                    request.auth.api_key.value.clone(),
                )
            }
            else {
                request_builder
            }
        }
        AuthType::None => request_builder,
    }
}

fn apply_body(
    mut request_builder: reqwest::RequestBuilder,
    request: &RequestDefinition,
    has_content_type: bool,
) -> AppResult<reqwest::RequestBuilder> {
    match request.body.mode {
        BodyMode::None => Ok(request_builder),
        BodyMode::Raw => {
            if !request.body.raw.is_empty() {
                request_builder = request_builder.body(request.body.raw.clone());
            }
            Ok(request_builder)
        }
        BodyMode::Json => {
            if !has_content_type {
                request_builder =
                    request_builder.header(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            }
            if !request.body.json.is_empty() {
                request_builder = request_builder.body(request.body.json.clone());
            }
            Ok(request_builder)
        }
        BodyMode::FormData | BodyMode::XWwwFormUrlencoded | BodyMode::Binary => Err(
            AppError::InvalidInput(
                "this build currently supports request body modes: none, raw, json".to_string(),
            ),
        ),
    }
}

fn apply_event_stream_headers(
    request_builder: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    request_builder
        .header(ACCEPT_ENCODING, HeaderValue::from_static("identity"))
        .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .header("connection", HeaderValue::from_static("keep-alive"))
}

fn build_http_client(expects_event_stream: bool) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder().tcp_nodelay(true);

    if expects_event_stream {
        builder = builder
            .http1_only()
            .no_gzip()
            .no_brotli()
            .no_deflate();
    }

    builder
        .build()
        .map_err(|error| AppError::Request(error.to_string()))
}

fn classify_response_body(content_type: &str, text: &str) -> (ResponseType, String) {
    if is_event_stream_content_type(content_type) {
        return (ResponseType::EventStream, text.to_string());
    }

    if looks_like_json(content_type, text) {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
            if let Ok(pretty) = serde_json::to_string_pretty(&parsed) {
                return (ResponseType::Json, pretty);
            }
        }
    }

    (ResponseType::Text, text.to_string())
}

fn is_event_stream_content_type(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .map(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
        .unwrap_or(false)
}

fn emit_stream_event(
    on_event: &Channel<SendRequestStreamEvent>,
    request_id: &str,
    is_event_stream: bool,
    event: SendRequestStreamEvent,
) -> AppResult<()> {
    if !is_event_stream {
        return Ok(());
    }

    log_sse_debug(request_id, &format!("channel:send {:?}", event_kind(&event)));
    on_event
        .send(event)
        .map_err(|error| AppError::Request(error.to_string()))
}

fn event_kind(event: &SendRequestStreamEvent) -> &'static str {
    match event {
        SendRequestStreamEvent::Started { .. } => "started",
        SendRequestStreamEvent::Chunk { .. } => "chunk",
        SendRequestStreamEvent::Finished { .. } => "finished",
    }
}

fn log_sse_debug(request_id: &str, message: &str) {
    #[cfg(debug_assertions)]
    {
        if request_id.trim().is_empty() {
            return;
        }

        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        eprintln!("[sse-debug][{timestamp_ms}][{request_id}] {message}");
    }
}

fn looks_like_json(content_type: &str, text: &str) -> bool {
    if is_event_stream_content_type(content_type) {
        return false;
    }

    let normalized = content_type.to_ascii_lowercase();
    if normalized.contains("application/json") || normalized.contains("+json") {
        return true;
    }
    serde_json::from_str::<serde_json::Value>(text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{classify_response_body, ResponseType};

    #[test]
    fn classifies_json_response_body() {
        let (response_type, body) = classify_response_body("application/json", "{\"ok\":true}");

        assert_eq!(response_type, ResponseType::Json);
        assert!(body.contains("\"ok\": true"));
    }

    #[test]
    fn falls_back_to_text_response_body() {
        let (response_type, body) = classify_response_body("text/plain", "hello");

        assert_eq!(response_type, ResponseType::Text);
        assert_eq!(body, "hello");
    }

    #[test]
    fn classifies_event_stream_response_body() {
        let (response_type, body) = classify_response_body("text/event-stream", "data: hello\n\n");

        assert_eq!(response_type, ResponseType::EventStream);
        assert_eq!(body, "data: hello\n\n");
    }
}
