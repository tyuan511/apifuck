use crate::{
    collection::selected,
    engine::{Event, Execution, RequestHead, emit, headers_and_url},
    settings,
};
use anyhow::{Context, Result, bail, ensure};
use futures_util::{SinkExt, StreamExt};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

#[derive(Debug)]
pub enum Command {
    Send(String),
    Close,
}
pub async fn connect(
    mut execution: Execution,
    mut commands: UnboundedReceiver<Command>,
    events: UnboundedSender<Event>,
) -> Result<()> {
    ensure!(execution.is_websocket(), "不是 WebSocket 请求");
    ensure!(
        execution
            .settings
            .proxy
            .as_ref()
            .is_none_or(|p| p.is_empty())
            && !execution.settings.allow_invalid_certificates,
        "WebSocket 代理和跳过证书验证尚未支持"
    );
    execution.before()?;
    let (url, headers) = headers_and_url(&execution.request)?;
    ensure!(
        matches!(url.scheme(), "ws" | "wss"),
        "WebSocket URL 必须使用 ws/wss"
    );
    let mut request = url.as_str().into_client_request()?;
    for (name, value) in &headers {
        request.headers_mut().append(name, value.clone());
    }
    let head = RequestHead::new(
        request.method().as_str(),
        &request.uri().to_string(),
        request.headers(),
    );
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_millis(execution.settings.connect_timeout_ms),
        tokio_tungstenite::connect_async(request),
    )
    .await
    .context("WebSocket 连接超时")??;
    let sender = Some(events);
    emit(&sender, Event::Connected);
    emit(&sender, Event::Request { request: head });
    settings::persist_variables(
        &execution.root,
        execution.environment.as_deref(),
        &execution.mutations,
    )?;
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    let mut last_ping = tokio::time::Instant::now();
    loop {
        tokio::select! {
            _=tick.tick()=>if execution.cancel.load(Ordering::Relaxed){let _=socket.close(None).await;break;}else if execution.keep_alive_ms>0 && last_ping.elapsed().as_millis()>=execution.keep_alive_ms as u128{socket.send(Message::Ping(Vec::new().into())).await?;last_ping=tokio::time::Instant::now();},
            command=commands.recv()=>match command {
                Some(Command::Send(text))=>{socket.send(Message::Text(text.clone().into())).await?;emit(&sender,Event::Message{direction:"outbound".into(),text});},
                Some(Command::Close)|None=>{socket.close(None).await?;break;}
            },
            incoming=socket.next()=>match incoming {
                Some(Ok(Message::Text(text)))=>{ensure!(text.len()<=execution.settings.max_response_bytes,"消息超过大小限制");emit(&sender,Event::Message{direction:"inbound".into(),text:text.to_string()});},
                Some(Ok(Message::Binary(bytes)))=>{use base64::Engine as _;ensure!(bytes.len()<=execution.settings.max_response_bytes,"消息超过大小限制");emit(&sender,Event::Message{direction:"inbound".into(),text:format!("[binary/base64] {}",base64::engine::general_purpose::STANDARD.encode(bytes))});},
                Some(Ok(Message::Ping(bytes)))=>socket.send(Message::Pong(bytes)).await?,
                Some(Ok(Message::Close(_)))|None=>break,
                Some(Err(e))=>bail!("WebSocket 连接错误：{e}"),
                _=>{}
            }
        }
    }
    emit(&sender, Event::Finished);
    Ok(())
}
pub fn default_message(execution: &Execution) -> Result<String> {
    let value = selected(&execution.request["message"], "message")?;
    Ok(value["data"].as_str().unwrap_or("").into())
}
