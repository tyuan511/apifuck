//! An embedded LSP server using the standard JSON-RPC message types over an
//! in-memory transport. One worker serves all editors, without Node or sockets.
use crate::languages::{self, Language};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    PublishDiagnosticsParams, TextDocumentItem,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

struct Client {
    connection: Connection,
    routes: std::sync::Arc<Mutex<HashMap<String, mpsc::Sender<PublishDiagnosticsParams>>>>,
}
impl Client {
    fn shared() -> &'static Self {
        static INSTANCE: OnceLock<Client> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let (client, server) = Connection::memory();
            std::thread::Builder::new()
                .name("apifuck-lsp".into())
                .spawn(move || serve(server))
                .expect("language server thread");
            let routes: std::sync::Arc<
                Mutex<HashMap<String, mpsc::Sender<PublishDiagnosticsParams>>>,
            > = Default::default();
            let targets = routes.clone();
            let receiver = client.receiver.clone();
            std::thread::Builder::new()
                .name("apifuck-lsp-dispatch".into())
                .spawn(move || {
                    for message in receiver {
                        if let Message::Notification(n) = message
                            && n.method == "textDocument/publishDiagnostics"
                            && let Ok(params) =
                                serde_json::from_value::<PublishDiagnosticsParams>(n.params)
                            && let Some(target) = targets.lock().unwrap().get(params.uri.as_str())
                        {
                            let _ = target.send(params);
                        }
                    }
                })
                .expect("language diagnostics thread");
            let _ = client.sender.send(Message::Request(Request::new(
                0.into(),
                "initialize".into(),
                json!({"capabilities":{}}),
            )));
            let _ = client.sender.send(Message::Notification(Notification::new(
                "initialized".into(),
                json!({}),
            )));
            Client {
                connection: client,
                routes,
            }
        })
    }
}

pub struct Document {
    pub uri: String,
    pub diagnostics: mpsc::Receiver<PublishDiagnosticsParams>,
    language: Option<String>,
}
impl Document {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let uri = format!(
            "file:///apifuck/editor-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let (tx, diagnostics) = mpsc::channel();
        Client::shared()
            .routes
            .lock()
            .unwrap()
            .insert(uri.clone(), tx);
        Self {
            uri,
            diagnostics,
            language: None,
        }
    }
    fn notify(&self, method: &str, params: serde_json::Value) {
        let _ = Client::shared()
            .connection
            .sender
            .send(Message::Notification(Notification::new(
                method.into(),
                params,
            )));
    }
    pub fn change(&mut self, language: &str, text: &str, version: i32) {
        if self.language.as_deref() != Some(language) {
            if self.language.is_some() {
                self.notify(
                    "textDocument/didClose",
                    json!({"textDocument":{"uri":self.uri}}),
                );
            }
            self.notify("textDocument/didOpen", json!({"textDocument":{"uri":self.uri,"languageId":language,"version":version,"text":text}}));
            self.language = Some(language.into());
        } else {
            self.notify("textDocument/didChange", json!({"textDocument":{"uri":self.uri,"version":version},"contentChanges":[{"text":text}]}));
        }
    }
}
impl Drop for Document {
    fn drop(&mut self) {
        self.notify(
            "textDocument/didClose",
            json!({"textDocument":{"uri":self.uri}}),
        );
        Client::shared().routes.lock().unwrap().remove(&self.uri);
    }
}

fn serve(connection: Connection) {
    let mut documents: HashMap<String, TextDocumentItem> = HashMap::new();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                let response = if request.method == "initialize" {
                    Response::new_ok(
                        request.id,
                        json!({"capabilities":{"positionEncoding":"utf-16","textDocumentSync":{"openClose":true,"change":1}},"serverInfo":{"name":"ApiFuck syntax","version":"1"}}),
                    )
                } else if request.method == "shutdown" {
                    Response::new_ok(request.id, serde_json::Value::Null)
                } else {
                    Response::new_err(request.id, -32601, "Unsupported method".into())
                };
                let _ = connection.sender.send(Message::Response(response));
            }
            Message::Notification(n) => {
                let changed = match n.method.as_str() {
                    "textDocument/didOpen" => {
                        serde_json::from_value::<DidOpenTextDocumentParams>(n.params)
                            .ok()
                            .map(|p| {
                                let uri = p.text_document.uri.to_string();
                                documents.insert(uri.clone(), p.text_document);
                                uri
                            })
                    }
                    "textDocument/didChange" => {
                        serde_json::from_value::<DidChangeTextDocumentParams>(n.params)
                            .ok()
                            .and_then(|p| {
                                let document = documents.get_mut(p.text_document.uri.as_str())?;
                                if p.text_document.version <= document.version {
                                    return None;
                                }
                                let change = p.content_changes.last()?;
                                if change.range.is_some() {
                                    return None;
                                } // Full synchronization advertised above.
                                document.text = change.text.clone();
                                document.version = p.text_document.version;
                                Some(document.uri.to_string())
                            })
                    }
                    "textDocument/didClose" => {
                        if let Ok(p) =
                            serde_json::from_value::<DidCloseTextDocumentParams>(n.params)
                        {
                            documents.remove(p.text_document.uri.as_str());
                        }
                        None
                    }
                    "exit" => break,
                    _ => None,
                };
                if let Some(uri) = changed
                    && let Some(document) = documents.get(&uri)
                {
                    let diagnostics = languages::diagnostics(
                        Language::from_id(&document.language_id),
                        &document.text,
                    );
                    let params = PublishDiagnosticsParams::new(
                        document.uri.clone(),
                        diagnostics,
                        Some(document.version),
                    );
                    let _ = connection
                        .sender
                        .send(Message::Notification(Notification::new(
                            "textDocument/publishDiagnostics".into(),
                            params,
                        )));
                }
            }
            Message::Response(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lsp_lifecycle_versions_and_language_switch() {
        let mut document = Document::new();
        document.change("json", "{\"id\":}", 1);
        let invalid = document
            .diagnostics
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(invalid.version, Some(1));
        assert!(!invalid.diagnostics.is_empty());
        document.change("json", "{\"id\":1}", 2);
        let valid = document
            .diagnostics
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(valid.version, Some(2));
        assert!(valid.diagnostics.is_empty());
        document.change("graphql", "{ user { id } }", 3);
        let graphql = document
            .diagnostics
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert_eq!(graphql.version, Some(3));
        assert!(graphql.diagnostics.is_empty());
        document.change("graphql", "{", 2); // Reordered versions cannot replace newer content.
        assert!(
            document
                .diagnostics
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err()
        );
    }
}
