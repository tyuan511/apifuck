//! Local-only manual smoke-test server for examples/demo.
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8787").await?;
    eprintln!("Demo API: http://127.0.0.1:8787 · WebSocket: ws://127.0.0.1:8787/socket");
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = serve(socket).await {
                eprintln!("{e}");
            }
        });
    }
}
async fn serve(mut socket: TcpStream) -> Result<()> {
    let mut buffer = [0u8; 8192];
    let n = socket.peek(&mut buffer).await?;
    if String::from_utf8_lossy(&buffer[..n])
        .to_lowercase()
        .contains("upgrade: websocket")
    {
        let mut ws = tokio_tungstenite::accept_async(socket).await?;
        while let Some(message) = ws.next().await {
            let message = message?;
            if message.is_close() {
                let _ = ws.close(None).await;
                break;
            }
            if message.is_text() || message.is_binary() {
                ws.send(message).await?;
            }
        }
        return Ok(());
    }
    let mut bytes = vec![];
    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                break;
            }
        }
    }
    let request = String::from_utf8_lossy(&bytes);
    let first = request.lines().next().unwrap_or("");
    if first.contains(" /events") {
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n").await?;
        for i in 0..20 {
            socket
                .write_all(format!("id: {i}\ndata: {{\"count\":{i}}}\n\n").as_bytes())
                .await?;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    } else {
        let (status, body) = if first.starts_with("POST /users") {
            ("201 Created", r#"{"id":42,"name":"apifuck"}"#)
        } else if first.contains(" /users") {
            (
                "200 OK",
                r#"{"users":[{"id":1,"name":"Ada"},{"id":2,"name":"Lin"}]}"#,
            )
        } else {
            ("404 Not Found", r#"{"error":"not found"}"#)
        };
        socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await?;
    }
    Ok(())
}
