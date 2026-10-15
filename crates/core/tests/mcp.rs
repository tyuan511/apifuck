use apifuck_core::{
    Collection,
    mcp::{Config, Service, Workspace, guides::Agent},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

struct Harness {
    _temp: tempfile::TempDir,
    service: Arc<Service>,
    workspace: Arc<Mutex<Workspace>>,
    worker: tokio::task::JoinHandle<()>,
    client: reqwest::Client,
}
impl Harness {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        Collection::create(&root, "MCP Test").unwrap();
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port();
        drop(socket);
        let path = temp.path().join("mcp.json");
        let config = Config {
            port,
            ..Config::default()
        };
        std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
        let (service, mut receiver) =
            Service::new(path, tokio::runtime::Handle::current()).unwrap();
        let workspace = Arc::new(Mutex::new(Workspace {
            root: Some(root),
            ..Default::default()
        }));
        let service_copy = service.clone();
        let context = workspace.clone();
        let worker = tokio::spawn(async move {
            while let Some(call) = receiver.recv().await {
                let result = if service_copy.accepts(&call) {
                    service_copy
                        .call(&call.name, &call.arguments, &mut context.lock().unwrap())
                        .map_err(|e| format!("{e:#}"))
                } else {
                    Err("Stopped".into())
                };
                let _ = call.reply.send(result);
            }
        });
        service.set_enabled(true).await.unwrap();
        Self {
            _temp: temp,
            service,
            workspace,
            worker,
            client: reqwest::Client::builder()
                .no_proxy()
                .pool_max_idle_per_host(0)
                .build()
                .unwrap(),
        }
    }
    fn post(&self, message: Value) -> reqwest::RequestBuilder {
        let config = self.service.status().config;
        self.client
            .post(config.endpoint())
            .bearer_auth(config.token)
            .header("accept", "application/json, text/event-stream")
            .json(&message)
    }
    async fn rpc(&self, message: Value) -> Value {
        self.post(message)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
    async fn call(&self, name: &str, args: Value) -> Value {
        let response=self.rpc(json!({"jsonrpc":"2.0","id":"test","method":"tools/call","params":{"name":name,"arguments":args}})).await;
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], false, "{response}");
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    async fn error(&self, name: &str, args: Value) -> Value {
        let response=self.rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
        assert!(
            response.get("error").is_some() || response["result"]["isError"] == true,
            "{response}"
        );
        response
    }
    async fn finished(&self, id: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let job = self.call("get_job", json!({"id":id})).await;
                if job["status"] != "running" {
                    break job;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.service.shutdown();
        self.worker.abort();
    }
}

#[tokio::test]
async fn native_mcp_protocol_auth_and_lifecycle() {
    let h = Harness::new().await;
    let init=h.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}})).await;
    assert_eq!(init["result"]["serverInfo"]["name"], "apifuck-native");
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    let tools = h
        .rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    assert_eq!(
        tools["result"]["tools"].as_array().unwrap().len(),
        Service::tool_count()
    );
    assert_eq!(
        h.post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    let ping = json!({"jsonrpc":"2.0","id":3,"method":"ping"});
    assert_eq!(
        h.post(ping.clone())
            .header("authorization", "Bearer wrong")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        h.post(ping.clone())
            .header("origin", "https://evil.test")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        h.post(ping.clone())
            .header("host", "evil.test")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        h.post(ping.clone())
            .header("mcp-protocol-version", "wrong")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        h.rpc(json!({"jsonrpc":"2.0","id":4,"method":"missing"}))
            .await["error"]["code"],
        -32601
    );
    h.error("unknown", json!({})).await;
    h.error("create_request", json!({"name":"missing protocol"}))
        .await;
    let status = h.service.status();
    let config_path = h._temp.path().join("mcp.json");
    h.service.set_enabled(false).await.unwrap();
    assert!(!h.service.status().running);
    let config: Config = serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
    assert!(!config.enabled);
    assert_eq!(config.token, status.config.token);
    h.service.set_enabled(true).await.unwrap();
    assert!(h.service.status().running);
    assert_eq!(h.service.status().config.token, status.config.token);
    assert!(h.rpc(ping).await.get("result").is_some());
    h.service.shutdown();
    tokio::task::yield_now().await;
    let (restarted, _) = Service::new(config_path, tokio::runtime::Handle::current()).unwrap();
    assert!(restarted.status().config.enabled);
    assert!(!restarted.status().running);
    restarted.set_enabled(true).await.unwrap();
}

#[tokio::test]
async fn port_conflict_does_not_persist_enable_or_change_other_preferences() {
    let temp = tempfile::tempdir().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let config = Config {
        port: listener.local_addr().unwrap().port(),
        ..Default::default()
    };
    let path = temp.path().join("mcp.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let prefs = temp.path().join("preferences.json");
    std::fs::write(&prefs, "original preferences").unwrap();
    let (service, _) = Service::new(path.clone(), tokio::runtime::Handle::current()).unwrap();
    assert!(service.set_enabled(true).await.is_err());
    assert!(service.status().error.is_some());
    assert!(!service.status().running);
    assert!(
        !serde_json::from_slice::<Config>(&std::fs::read(path).unwrap())
            .unwrap()
            .enabled
    );
    assert_eq!(
        std::fs::read_to_string(prefs).unwrap(),
        "original preferences"
    );
}

#[tokio::test]
async fn real_yaml_crud_preserves_comments_and_rejects_stale_or_unsafe_writes() {
    let h = Harness::new().await;
    let folder = h.call("create_folder", json!({"name":"Users"})).await;
    let parent = Path::new(folder["path"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_string_lossy();
    let doc = h
        .call(
            "create_request",
            json!({"parent":parent,"name":"List users","protocol":"http"}),
        )
        .await;
    let selector = doc["path"].clone();
    let source = format!(
        "# Preserve this comment\n{}",
        doc["source"].as_str().unwrap()
    );
    let doc = h
        .call(
            "save_document_source",
            json!({"selector":selector,"expectedSource":doc["source"],"source":source}),
        )
        .await;
    let mut value = doc["value"].clone();
    value["info"]["name"] = json!("Renamed display");
    value["http"]["url"] = json!("https://example.test/users");
    let saved = h
        .call(
            "update_document",
            json!({"selector":selector,"expectedSource":source,"value":value}),
        )
        .await;
    assert_eq!(saved["path"], selector);
    assert!(
        saved["source"]
            .as_str()
            .unwrap()
            .contains("# Preserve this comment")
    );
    h.error(
        "update_document",
        json!({"selector":selector,"expectedSource":source,"value":value}),
    )
    .await;
    h.workspace.lock().unwrap().has_drafts = true;
    h.error(
        "trash_document",
        json!({"selector":selector,"expectedSource":saved["source"]}),
    )
    .await;
    assert_eq!(
        h.call("read_document", json!({"selector":selector})).await["source"],
        saved["source"]
    );
    h.workspace.lock().unwrap().has_drafts = false;
    h.error(
        "import_document",
        json!({"path":".apifuck/workspace.yml","source":"name: bad"}),
    )
    .await;
    h.error(
        "import_document",
        json!({"path":"../escape.yml","source":"name: bad"}),
    )
    .await;
    let copied = h
        .call(
            "duplicate_request",
            json!({"selector":selector,"name":"Copy"}),
        )
        .await;
    h.call(
        "move_document",
        json!({"selector":copied["path"],"expectedSource":copied["source"],"target":"moved.yml"}),
    )
    .await;
    let moved = h
        .call("read_document", json!({"selector":"moved.yml"}))
        .await;
    h.call(
        "trash_document",
        json!({"selector":"moved.yml","expectedSource":moved["source"]}),
    )
    .await;
    h.error("read_document", json!({"selector":"moved.yml"}))
        .await;
    let env = h.call("create_environment", json!({"name":"Dev"})).await;
    assert!(h.call("get_workspace", json!({})).await["environment"].is_null());
    h.call("set_environment", json!({"environment":"Dev"}))
        .await;
    let vars = h
        .call("preview_variables", json!({"selector":selector}))
        .await;
    assert!(vars.get("base_url").is_some());
    h.call(
        "trash_document",
        json!({"selector":env["path"],"expectedSource":env["source"]}),
    )
    .await;
}

#[tokio::test]
async fn mcp_http_executes_core_scripts_and_persists_local_variables() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/echo", listener.local_addr().unwrap());
    let target = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route(
                "/echo",
                axum::routing::get(|| async { axum::Json(json!({"ok":true})) }),
            ),
        )
        .await
        .unwrap();
    });
    let h = Harness::new().await;
    let imported = h
        .call(
            "import_curl",
            json!({"name":"Echo","command":format!("curl {url}")}),
        )
        .await;
    let mut value = imported["value"].clone();
    value["runtime"] =
        json!({"scripts":[{"type":"after-response","code":"api.env.set('fromMcp', 'yes');"}]});
    let saved = h
        .call(
            "update_document",
            json!({"selector":imported["path"],"expectedSource":imported["source"],"value":value}),
        )
        .await;
    let job = h
        .call("send_request", json!({"selector":saved["path"]}))
        .await;
    let done = h.finished(job["id"].as_str().unwrap()).await;
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(done["result"]["status"], 200);
    let root = h.workspace.lock().unwrap().root.clone().unwrap();
    assert_eq!(
        apifuck_core::settings::overlay(&root, None).unwrap()["fromMcp"],
        "yes"
    );
    assert!(
        !std::fs::read_to_string(root.join(saved["path"].as_str().unwrap()))
            .unwrap()
            .contains("variables.json")
    );
    let code = h
        .call(
            "export_request",
            json!({"selector":saved["path"],"language":"curl"}),
        )
        .await;
    assert!(code["code"].as_str().unwrap().contains(&url));
    let run = h.call("run_collection", json!({})).await;
    assert_eq!(
        h.finished(run["id"].as_str().unwrap()).await["status"],
        "completed"
    );
    target.abort();
}

#[tokio::test]
async fn four_concurrent_clients_can_read_results_after_the_job_cache_fills() {
    let h = Harness::new().await;
    h.workspace.lock().unwrap().network.proxy = Some(String::new());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let target = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().route("/", axum::routing::get(|| async { "ok" })),
        )
        .await
        .unwrap();
    });
    let request = h
        .call(
            "import_curl",
            json!({"name":"Concurrent", "command":format!("curl {url}")}),
        )
        .await;
    let arguments = json!({"selector":request["path"]});
    for _ in 0..32 {
        let job = h.call("send_request", arguments.clone()).await;
        assert_eq!(
            h.finished(job["id"].as_str().unwrap()).await["status"],
            "completed"
        );
    }
    futures_util::future::join_all((0..4).map(|_| async {
        for _ in 0..25 {
            let job = h.call("send_request", arguments.clone()).await;
            tokio::time::sleep(Duration::from_millis(5)).await;
            let result = h.finished(job["id"].as_str().unwrap()).await;
            assert_eq!(result["status"], "completed", "{result}");
            assert_eq!(result["result"]["status"], 200);
            assert_eq!(result["result"]["body"], "ok");
        }
    }))
    .await;
    eprintln!("4 concurrent clients: 100/100 HTTP results retrieved after 32 warmup jobs");
    target.abort();
}

#[test]
fn tutorials_are_exactly_the_requested_native_clients() {
    assert_eq!(
        Agent::ALL.map(|a| a.name()),
        ["Claude Code", "Codex", "OpenCode", "Pi"]
    );
    let url = "http://127.0.0.1:47831/mcp";
    let token = "test-token";
    assert!(
        Agent::ClaudeCode
            .config(url, token)
            .contains("--transport http")
    );
    assert!(
        Agent::Codex
            .config(url, token)
            .contains("[mcp_servers.apifuck]")
    );
    let open: Value = serde_json::from_str(&Agent::OpenCode.config(url, token)).unwrap();
    assert_eq!(open["mcp"]["apifuck"]["type"], "remote");
    let pi: Value = serde_json::from_str(&Agent::Pi.config(url, token)).unwrap();
    assert_eq!(
        pi["mcpServers"]["apifuck"]["headers"]["Authorization"],
        "Bearer test-token"
    );
}

#[tokio::test]
async fn websocket_messages_and_cancellation_use_native_engine() {
    use futures_util::{SinkExt, StreamExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/socket", listener.local_addr().unwrap());
    let echo = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        while let Some(Ok(message)) = socket.next().await {
            if message.is_close() {
                break;
            }
            if message.is_text() {
                socket.send(message).await.unwrap();
            }
        }
    });
    let h = Harness::new().await;
    let doc = h
        .call(
            "create_request",
            json!({"name":"Socket","protocol":"websocket"}),
        )
        .await;
    let mut value = doc["value"].clone();
    value["websocket"]["url"] = json!(url);
    let doc = h
        .call(
            "update_document",
            json!({"selector":doc["path"],"expectedSource":doc["source"],"value":value}),
        )
        .await;
    let job = h
        .call("connect_websocket", json!({"selector":doc["path"]}))
        .await;
    let id = job["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if h.call("get_job", json!({"id":id})).await["connected"] == true {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    h.call(
        "send_websocket_message",
        json!({"id":id,"message":"native MCP echo"}),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let job = h.call("get_job", json!({"id":id})).await;
            if job["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["direction"] == "inbound" && e["text"] == "native MCP echo")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    h.error("create_environment", json!({"name":"Must wait"}))
        .await;
    h.call("cancel_job", json!({"id":id})).await;
    assert_eq!(h.finished(id).await["status"], "cancelled");
    echo.await.unwrap();
}

#[tokio::test]
async fn disabling_mcp_cancels_live_sse_and_graphql_preserves_query() {
    use axum::{
        body::{Body, Bytes},
        http::header,
        response::IntoResponse,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new()
        .route(
            "/graphql",
            axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
                axum::Json(json!({"data":body}))
            }),
        )
        .route(
            "/sse",
            axum::routing::get(|| async {
                let stream = futures_util::stream::unfold(0, |n| async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Some((
                        Ok::<_, std::io::Error>(Bytes::from(format!("data: {n}\n\n"))),
                        n + 1,
                    ))
                });
                (
                    [(header::CONTENT_TYPE, "text/event-stream")],
                    Body::from_stream(stream),
                )
                    .into_response()
            }),
        );
    let target = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let h = Harness::new().await;
    let doc = h
        .call(
            "create_request",
            json!({"name":"Graph","protocol":"graphql"}),
        )
        .await;
    let mut value = doc["value"].clone();
    value["graphql"]["url"] = json!(format!("{base}/graphql"));
    let doc = h
        .call(
            "update_document",
            json!({"selector":doc["path"],"expectedSource":doc["source"],"value":value}),
        )
        .await;
    let job = h
        .call("send_request", json!({"selector":doc["path"]}))
        .await;
    let done = h.finished(job["id"].as_str().unwrap()).await;
    assert_eq!(done["status"], "completed", "{done}");
    let body: Value = serde_json::from_str(done["result"]["body"].as_str().unwrap()).unwrap();
    assert!(
        body["data"]["query"]
            .as_str()
            .unwrap()
            .contains("__typename")
    );
    let doc=h.call("import_curl",json!({"name":"Stream","command":format!("curl {base}/sse -H 'Accept: text/event-stream'")})).await;
    let job = h
        .call("send_request", json!({"selector":doc["path"]}))
        .await;
    let id = job["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let job = h.call("get_job", json!({"id":id})).await;
            if job["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "chunk")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    h.service.set_enabled(false).await.unwrap();
    h.service.set_enabled(true).await.unwrap();
    assert_eq!(h.finished(id).await["status"], "cancelled");
    target.abort();
}
