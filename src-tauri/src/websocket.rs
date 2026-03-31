use std::{
    collections::HashMap,
    io,
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tungstenite::{
    connect,
    client::IntoClientRequest,
    http::{header::HeaderName, HeaderValue, Request},
    stream::MaybeTlsStream,
    Error as WebSocketRuntimeError,
    Message, WebSocket,
};

use crate::{
    error::{AppError, AppResult},
    storage::{AuthConfig, AuthType, KeyValue, WebSocketMessageFormat},
};

const SOCKET_READ_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct WebSocketState {
    pub(crate) sessions: Arc<Mutex<HashMap<String, WebSocketSession>>>,
}

struct WebSocketSession {
    request_id: String,
    sender: mpsc::Sender<WebSocketCommand>,
    closed: Arc<AtomicBool>,
}

enum WebSocketCommand {
    SendText(String),
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWebSocketInput {
    pub connection_id: String,
    pub request_id: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub query: Vec<KeyValue>,
    pub auth: AuthConfig,
    #[serde(default)]
    pub default_message: String,
    pub message_format: WebSocketMessageFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendWebSocketMessageInput {
    pub connection_id: String,
    pub request_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectWebSocketInput {
    pub connection_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSocketConnectionInfo {
    pub connection_id: String,
    pub request_id: String,
    pub connected_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WebSocketEvent {
    Connected {
        connection_id: String,
        request_id: String,
        connected_at: u64,
    },
    Message {
        connection_id: String,
        request_id: String,
        direction: WebSocketMessageDirection,
        message: String,
        timestamp: u64,
    },
    Error {
        connection_id: String,
        request_id: String,
        error: String,
        timestamp: u64,
    },
    Closed {
        connection_id: String,
        request_id: String,
        code: Option<u16>,
        reason: Option<String>,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSocketMessageDirection {
    Inbound,
    Outbound,
}

pub async fn connect_websocket(
    state: tauri::State<'_, WebSocketState>,
    input: ConnectWebSocketInput,
    on_event: Channel<WebSocketEvent>,
) -> AppResult<WebSocketConnectionInfo> {
    {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| AppError::InvalidInput("websocket state is unavailable".to_string()))?;
        if sessions.contains_key(&input.connection_id) {
            return Err(AppError::Conflict(format!(
                "websocket connection {} already exists",
                input.connection_id
            )));
        }
    }

    let connected_at = now_millis();
    let request = build_websocket_request(&input)?;
    let (mut socket, _) = connect(request).map_err(|error| AppError::WebSocket(error.to_string()))?;
    configure_socket(&mut socket)?;
    let (sender, receiver) = mpsc::channel();
    let closed = Arc::new(AtomicBool::new(false));

    {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| AppError::InvalidInput("websocket state is unavailable".to_string()))?;
        sessions.insert(
            input.connection_id.clone(),
            WebSocketSession {
                request_id: input.request_id.clone(),
                sender: sender.clone(),
                closed: closed.clone(),
            },
        );
    }

    on_event
        .send(WebSocketEvent::Connected {
            connection_id: input.connection_id.clone(),
            request_id: input.request_id.clone(),
            connected_at,
        })
        .map_err(|error| AppError::WebSocket(error.to_string()))?;

    spawn_reader(
        state.inner().sessions.clone(),
        socket,
        receiver,
        closed,
        input.connection_id.clone(),
        input.request_id.clone(),
        on_event,
    );

    Ok(WebSocketConnectionInfo {
        connection_id: input.connection_id,
        request_id: input.request_id,
        connected_at,
    })
}

pub fn send_websocket_message(
    state: tauri::State<'_, WebSocketState>,
    input: SendWebSocketMessageInput,
) -> AppResult<()> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| AppError::InvalidInput("websocket state is unavailable".to_string()))?;
    let session = sessions
        .get(&input.connection_id)
        .ok_or_else(|| AppError::NotFound(format!("websocket {}", input.connection_id)))?;
    if session.request_id != input.request_id {
        return Err(AppError::InvalidInput(format!(
            "connection {} does not belong to request {}",
            input.connection_id, input.request_id
        )));
    }
    session
        .sender
        .send(WebSocketCommand::SendText(input.message))
        .map_err(|_| AppError::NotFound(format!("websocket {}", input.connection_id)))
}

pub fn disconnect_websocket(
    state: tauri::State<'_, WebSocketState>,
    input: DisconnectWebSocketInput,
) -> AppResult<()> {
    let session = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| AppError::InvalidInput("websocket state is unavailable".to_string()))?;
        sessions
            .remove(&input.connection_id)
            .ok_or_else(|| AppError::NotFound(format!("websocket {}", input.connection_id)))?
    };

    session.closed.store(true, Ordering::SeqCst);
    let _ = session.sender.send(WebSocketCommand::Close);
    Ok(())
}

fn build_websocket_request(input: &ConnectWebSocketInput) -> AppResult<Request<()>> {
    let mut url = input.url.trim().to_string();
    if !input.query.is_empty() {
        let query = input
            .query
            .iter()
            .filter(|item| item.enabled && !item.key.trim().is_empty())
            .map(|item| format!("{}={}", item.key.trim(), item.value))
            .collect::<Vec<_>>()
            .join("&");
        if !query.is_empty() {
            let separator = if url.contains('?') { '&' } else { '?' };
            url.push(separator);
            url.push_str(&query);
        }
    }

    let mut request = url
        .into_client_request()
        .map_err(|error| AppError::InvalidInput(format!("invalid websocket url: {error}")))?;

    for header in input.headers.iter().filter(|item| item.enabled && !item.key.trim().is_empty()) {
        let name = HeaderName::from_bytes(header.key.trim().as_bytes())
            .map_err(|error| AppError::InvalidInput(format!("invalid header name {}: {error}", header.key)))?;
        let value = HeaderValue::from_str(&header.value)
            .map_err(|error| AppError::InvalidInput(format!("invalid header value for {}: {error}", header.key)))?;
        request.headers_mut().insert(name, value);
    }

    if input.auth.auth_type == AuthType::Bearer && !input.auth.bearer_token.trim().is_empty() {
        let value = HeaderValue::from_str(&format!("Bearer {}", input.auth.bearer_token))
            .map_err(|error| AppError::InvalidInput(format!("invalid bearer authorization header: {error}")))?;
        request.headers_mut().insert("Authorization", value);
    }

    if input.auth.auth_type == AuthType::Basic {
        let credentials = base64_encode(&format!(
            "{}:{}",
            input.auth.basic.username, input.auth.basic.password
        ));
        let value = HeaderValue::from_str(&format!("Basic {}", credentials))
            .map_err(|error| AppError::InvalidInput(format!("invalid basic authorization header: {error}")))?;
        request.headers_mut().insert("Authorization", value);
    }

    Ok(request)
}

fn spawn_reader(
    sessions: Arc<Mutex<HashMap<String, WebSocketSession>>>,
    mut socket: WebSocket<MaybeTlsStream<TcpStream>>,
    receiver: mpsc::Receiver<WebSocketCommand>,
    closed: Arc<AtomicBool>,
    connection_id: String,
    request_id: String,
    on_event: Channel<WebSocketEvent>,
) {
    let reader_connection_id = connection_id.clone();
    thread::spawn(move || {
        loop {
            while let Ok(command) = receiver.try_recv() {
                match command {
                    WebSocketCommand::SendText(message) => {
                        let outbound_message = message.clone();
                        if let Err(error) = socket.send(Message::Text(message)) {
                            let _ = on_event.send(WebSocketEvent::Error {
                                connection_id: reader_connection_id.clone(),
                                request_id: request_id.clone(),
                                error: error.to_string(),
                                timestamp: now_millis(),
                            });
                            if is_fatal_socket_error(&error) {
                                closed.store(true, Ordering::SeqCst);
                                break;
                            }
                        }
                        else {
                            let _ = on_event.send(WebSocketEvent::Message {
                                connection_id: reader_connection_id.clone(),
                                request_id: request_id.clone(),
                                direction: WebSocketMessageDirection::Outbound,
                                message: outbound_message,
                                timestamp: now_millis(),
                            });
                        }
                    }
                    WebSocketCommand::Close => {
                        closed.store(true, Ordering::SeqCst);
                        let _ = socket.close(None);
                        let _ = on_event.send(WebSocketEvent::Closed {
                            connection_id: reader_connection_id.clone(),
                            request_id: request_id.clone(),
                            code: None,
                            reason: Some("Disconnected".to_string()),
                            timestamp: now_millis(),
                        });
                        let _ = sessions.lock().map(|mut state| state.remove(&connection_id));
                        return;
                    }
                }
            }

            let message = socket.read();

            match message {
                Ok(Message::Text(text)) => {
                    let _ = on_event.send(WebSocketEvent::Message {
                        connection_id: reader_connection_id.clone(),
                        request_id: request_id.clone(),
                        direction: WebSocketMessageDirection::Inbound,
                        message: text,
                        timestamp: now_millis(),
                    });
                }
                Ok(Message::Binary(_)) => {
                    let _ = on_event.send(WebSocketEvent::Error {
                        connection_id: reader_connection_id.clone(),
                        request_id: request_id.clone(),
                        error: "binary websocket frames are not supported yet".to_string(),
                        timestamp: now_millis(),
                    });
                }
                Ok(Message::Close(frame)) => {
                    closed.store(true, Ordering::SeqCst);
                    let _ = on_event.send(WebSocketEvent::Closed {
                        connection_id: reader_connection_id.clone(),
                        request_id: request_id.clone(),
                        code: frame.as_ref().map(|value| value.code.into()),
                        reason: frame.as_ref().map(|value| value.reason.to_string()),
                        timestamp: now_millis(),
                    });
                    break;
                }
                Ok(_) => {}
                Err(WebSocketRuntimeError::Io(error))
                    if error.kind() == io::ErrorKind::WouldBlock || error.kind() == io::ErrorKind::TimedOut => {}
                Err(WebSocketRuntimeError::ConnectionClosed | WebSocketRuntimeError::AlreadyClosed) => {
                    closed.store(true, Ordering::SeqCst);
                    let _ = on_event.send(WebSocketEvent::Closed {
                        connection_id: reader_connection_id.clone(),
                        request_id: request_id.clone(),
                        code: None,
                        reason: None,
                        timestamp: now_millis(),
                    });
                    break;
                }
                Err(error) => {
                    if !closed.load(Ordering::SeqCst) {
                        let _ = on_event.send(WebSocketEvent::Error {
                            connection_id: reader_connection_id.clone(),
                            request_id: request_id.clone(),
                            error: error.to_string(),
                            timestamp: now_millis(),
                        });
                    }
                    break;
                }
            }
        }

        let _ = sessions.lock().map(|mut state| state.remove(&connection_id));
    });
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn configure_socket(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> AppResult<()> {
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            stream.set_read_timeout(Some(SOCKET_READ_TIMEOUT))?;
        }
        MaybeTlsStream::Rustls(stream) => {
            stream.sock.set_read_timeout(Some(SOCKET_READ_TIMEOUT))?;
        }
        _ => {}
    }

    Ok(())
}

fn is_fatal_socket_error(error: &WebSocketRuntimeError) -> bool {
    !matches!(
        error,
        WebSocketRuntimeError::Io(io_error)
            if io_error.kind() == io::ErrorKind::WouldBlock || io_error.kind() == io::ErrorKind::TimedOut
    )
}

fn base64_encode(input: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = 0;

    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied().unwrap_or(0);
        let b2 = bytes.get(index + 2).copied().unwrap_or(0);

        output.push(TABLE[(b0 >> 2) as usize] as char);
        output.push(TABLE[((b0 & 0b0000_0011) << 4 | (b1 >> 4)) as usize] as char);

        if index + 1 < bytes.len() {
            output.push(TABLE[((b1 & 0b0000_1111) << 2 | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }

        if index + 2 < bytes.len() {
            output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }

        index += 3;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(url: &str) -> ConnectWebSocketInput {
        ConnectWebSocketInput {
            connection_id: "conn-1".to_string(),
            request_id: "req-1".to_string(),
            url: url.to_string(),
            headers: vec![],
            query: vec![],
            auth: AuthConfig::default(),
            default_message: String::new(),
            message_format: WebSocketMessageFormat::Text,
        }
    }

    #[test]
    fn build_websocket_request_includes_required_handshake_headers() {
        let request = build_websocket_request(&make_input("wss://example.com/socket")).unwrap();

        assert_eq!(request.method(), "GET");
        assert_eq!(request.uri().to_string(), "wss://example.com/socket");
        assert!(request.headers().contains_key("Host"));
        assert!(request.headers().contains_key("Connection"));
        assert!(request.headers().contains_key("Upgrade"));
        assert!(request.headers().contains_key("Sec-WebSocket-Version"));
        assert!(request.headers().contains_key("Sec-WebSocket-Key"));
    }

    #[test]
    fn build_websocket_request_appends_query_and_custom_headers() {
        let mut input = make_input("ws://example.com/socket");
        input.query = vec![KeyValue {
            id: "q1".to_string(),
            key: "token".to_string(),
            value: "abc".to_string(),
            enabled: true,
            description: String::new(),
        }];
        input.headers = vec![KeyValue {
            id: "h1".to_string(),
            key: "X-Test".to_string(),
            value: "ok".to_string(),
            enabled: true,
            description: String::new(),
        }];

        let request = build_websocket_request(&input).unwrap();

        assert_eq!(request.uri().to_string(), "ws://example.com/socket?token=abc");
        assert_eq!(request.headers().get("X-Test").unwrap(), "ok");
        assert!(request.headers().contains_key("Sec-WebSocket-Key"));
    }
}
