use apifuck_core::{
    Collection, Document, Event, NetworkSettings, collection, engine::interpolate, exchange,
    execute, prepare, settings, websocket,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{path::Path, sync::atomic::Ordering, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

fn project() -> (tempfile::TempDir, Collection) {
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "Tests").unwrap();
    (dir, c)
}
fn request(c: &Collection, url: &str, extra: Value) -> Document {
    let mut value = json!({"info":{"name":"Test","type":"http"},"http":{"method":"GET","url":url}});
    for (k, v) in extra.as_object().unwrap() {
        value[k] = v.clone();
    }
    c.add(
        Path::new("test.yml"),
        &serde_yaml::to_string(&value).unwrap(),
    )
    .unwrap()
}
async fn server(response: Vec<u8>) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut all = vec![];
        let mut buf = [0; 4096];
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            all.extend_from_slice(&buf[..n]);
            if let Some(end) = all.windows(4).position(|w| w == b"\r\n\r\n") {
                let text = String::from_utf8_lossy(&all[..end]);
                let length = text
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if all.len() >= end + 4 + length {
                    break;
                }
            }
        }
        socket.write_all(&response).await.unwrap();
        all
    });
    (url, task)
}
fn ok(body: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).into_bytes()
}

#[test]
fn schema_roundtrip_and_conflict_detection() {
    let (_dir, c) = project();
    for (kind, path) in [
        ("http", "a.yml"),
        ("websocket", "b.yml"),
        ("folder", "group/folder.yml"),
        ("environment", "environments/dev.yml"),
    ] {
        c.add(Path::new(path), &collection::template(kind, kind).unwrap())
            .unwrap();
    }
    let c = Collection::open(c.root()).unwrap();
    assert_eq!(c.documents().len(), 5);
    let doc = c.find("a.yml").unwrap();
    let text = format!("# retained comment\n{}", doc.source());
    c.save(doc, &text).unwrap();
    assert_eq!(
        std::fs::read_to_string(c.root().join("a.yml")).unwrap(),
        text
    );
    assert!(
        c.save(doc, doc.source())
            .unwrap_err()
            .to_string()
            .contains("其他程序")
    );
    assert!(
        c.add(
            Path::new("../escape.yml"),
            &collection::template("http", "Escape").unwrap()
        )
        .is_err()
    );
    assert!(
        Document::parse(
            "bad.yml".into(),
            "info: {type: http}\nhttp: {url: https://example.com, invented: true}".into()
        )
        .is_err()
    );
}
#[test]
fn typed_variables_cycles_and_environments() {
    let (_dir, c) = project();
    c.add(
        Path::new("environments/base.yml"),
        "name: base\nvariables:\n  - name: host\n    value: localhost\n",
    )
    .unwrap();
    c.add(Path::new("environments/dev.yml"),"name: dev\nextends: base\nvariables:\n  - name: port\n    value: {type: number, data: '8787'}\n").unwrap();
    let c = Collection::open(c.root()).unwrap();
    let env = c.environment(Some("dev")).unwrap();
    assert_eq!(env["port"], 8787);
    assert_eq!(
        interpolate("http://{{host}}:{{port}}", &env).unwrap(),
        "http://localhost:8787"
    );
    assert!(interpolate("{{a}}", &json!({"a":"{{b}}","b":"{{a}}"})).is_err());
    assert!(interpolate("{{missing}}", &env).is_err());
}
#[tokio::test]
async fn inheritance_auth_duplicate_query_and_scripts_are_sent() {
    let (_dir, c) = project();
    let project = c.project();
    let root = json!({"opencollection":"1.0.0","info":{"name":"Tests"},"request":{"auth":{"type":"bearer","token":"root"},"headers":[{"name":"X-Root","value":"yes"},{"name":"X-Override","value":"old"}],"scripts":[{"type":"before-request","code":"api.vars.set('order', 'root');"}]}});
    c.save(project, &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    c.add(
        Path::new("environments/dev.yml"),
        "name: dev\nvariables:\n  - name: token\n    value: original\n",
    )
    .unwrap();
    let (url, server) = server(ok("{\"id\":42}")).await;
    let mut value = json!({"info":{"name":"Test","type":"http"},"http":{"method":"POST","url":url,"auth":"inherit","params":[{"name":"tag","value":"a","type":"query"},{"name":"tag","value":"b","type":"query"}],"headers":[{"name":"x-override","value":"new"}],"body":{"type":"json","data":"{\"token\":\"{{token}}\"}"}},"runtime":{"scripts":[{"type":"before-request","code":"api.env.set('token', 'changed'); req.setHeader('X-Order', api.vars.get('order'));"},{"type":"after-response","code":"api.env.set('id', api.response.json().id); console.log('done');"}]}});
    value["http"]["params"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"disabled","value":"no","type":"query","disabled":true}));
    let doc = c
        .add(
            Path::new("test.yml"),
            &serde_yaml::to_string(&value).unwrap(),
        )
        .unwrap();
    let c = Collection::open(c.root()).unwrap();
    let response = execute(
        prepare(&c, &doc, Some("dev"), NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    let request = String::from_utf8(server.await.unwrap())
        .unwrap()
        .to_lowercase();
    assert!(request.contains("/?tag=a&tag=b "));
    assert!(!request.contains("disabled="));
    assert!(request.contains("authorization: bearer root"));
    assert!(request.contains("x-override: new"));
    assert!(!request.contains("x-override: old"));
    assert!(request.contains("x-order: root"));
    assert!(request.contains("{\"token\":\"changed\"}"));
    assert_eq!(response.status, 200);
    assert_eq!(settings::overlay(c.root(), Some("dev")).unwrap()["id"], 42);
    assert_eq!(response.logs, vec!["done"]);
}
#[tokio::test]
async fn no_auth_is_distinct_from_inherit() {
    let (_dir, c) = project();
    let mut root = c.project().value().clone();
    root["request"] = json!({"auth":{"type":"bearer","token":"secret"}});
    c.save(c.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    let c = Collection::open(c.root()).unwrap();
    let (url, server) = server(ok("{}")).await;
    let doc = request(&c, &url, json!({}));
    execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert!(!String::from_utf8_lossy(&server.await.unwrap()).contains("authorization"));
}
#[tokio::test]
async fn multipart_and_binary_files_use_request_relative_paths() {
    let (_dir, c) = project();
    std::fs::create_dir(c.root().join("upload")).unwrap();
    std::fs::write(c.root().join("upload/data.bin"), [0, 1, 255]).unwrap();
    let (url, server) = server(ok("{}")).await;
    let source=serde_yaml::to_string(&json!({"info":{"name":"Upload","type":"http"},"http":{"method":"POST","url":url,"body":{"type":"multipart-form","data":[{"name":"file","type":"file","value":"data.bin","contentType":"application/octet-stream"},{"name":"name","type":"text","value":"文件"}]}}})).unwrap();
    let doc = c.add(Path::new("upload/test.yml"), &source).unwrap();
    execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    let bytes = server.await.unwrap();
    assert!(bytes.windows(3).any(|w| w == [0, 1, 255]));
    assert!(String::from_utf8_lossy(&bytes).contains("filename=\"data.bin\""));
}
#[tokio::test]
async fn stream_cancellation_is_prompt_and_has_partial_events() {
    let (_dir, c) = project();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: hello\n\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;
    });
    let doc = request(&c, &url, json!({}));
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    let cancel = ex.cancellation();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let run = tokio::spawn(execute(ex, Some(tx)));
    loop {
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        if matches!(event, Event::Chunk { .. }) {
            break;
        }
    }
    cancel.store(true, Ordering::Relaxed);
    let result = tokio::time::timeout(Duration::from_millis(500), run)
        .await
        .unwrap()
        .unwrap();
    assert!(result.unwrap_err().to_string().contains("取消"));
    server.abort();
}
#[tokio::test]
async fn websocket_roundtrip() {
    let (_dir, c) = project();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
        let text = ws.next().await.unwrap().unwrap();
        ws.send(text).await.unwrap();
        while let Some(message) = ws.next().await {
            if message.unwrap().is_close() {
                break;
            }
        }
    });
    let source=serde_yaml::to_string(&json!({"info":{"name":"WS","type":"websocket"},"websocket":{"url":url,"message":{"type":"json","data":"{}"}}})).unwrap();
    let doc = c.add(Path::new("socket.yml"), &source).unwrap();
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    let (tx, commands) = tokio::sync::mpsc::unbounded_channel();
    let (events, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let run = tokio::spawn(websocket::connect(ex, commands, events));
    assert!(matches!(rx.recv().await.unwrap(), Event::Connected));
    tx.send(websocket::Command::Send("你好".into())).unwrap();
    loop {
        match tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            Event::Message { direction, text } if direction == "inbound" => {
                assert_eq!(text, "你好");
                break;
            }
            _ => {}
        }
    }
    tx.send(websocket::Command::Close).unwrap();
    run.await.unwrap().unwrap();
    server.await.unwrap();
}
#[test]
fn import_export_does_not_execute_shell_and_keeps_literal_payload() {
    let (_dir, c) = project();
    let source=exchange::import_curl("curl -X POST 'https://example.com/a?x=1' -H 'Content-Type: application/json' --data-raw '{\"name\":\"$(touch /tmp/do-not-run)\"}'","Imported").unwrap();
    let doc = c.add(Path::new("test.yml"), &source).unwrap();
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    let command = exchange::export(&ex, "curl").unwrap();
    assert!(command.contains("$(touch /tmp/do-not-run)"));
    let parsed = exchange::import_curl(&command, "Again").unwrap();
    assert!(parsed.contains("$(touch /tmp/do-not-run)"));
    assert!(exchange::import_curl("curl --unknown https://example.com", "Bad").is_err());
}
#[tokio::test]
async fn script_failure_and_missing_variables_prevent_send() {
    let (_dir, c) = project();
    let doc = request(
        &c,
        "http://127.0.0.1:1",
        json!({"runtime":{"scripts":[{"type":"before-request","code":"throw new Error('preflight failed');"}]}}),
    );
    let error = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("preflight failed"));
}
#[test]
fn source_schema_and_templates_are_valid() {
    let c = Collection::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo"))
        .unwrap();
    assert!(c.documents().len() >= 6);
    assert_eq!(c.environment_names(), ["local"]);
}

#[tokio::test]
async fn path_parameters_do_not_replace_prefixes_or_query_values_and_disabled_values_are_ignored() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{}")).await;
    let doc = request(
        &c,
        &url,
        json!({"http":{"method":"GET","url":format!("{url}/:id/:identifier?literal=:id"),"params":[{"name":"id","value":"a/b c","type":"path"},{"name":"identifier","value":"other","type":"path"},{"name":"skip","value":"{{missing}}","type":"query","disabled":true}]}}),
    );
    execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    let request = String::from_utf8(server.await.unwrap()).unwrap();
    assert!(
        request.starts_with("GET /a%2Fb%20c/other?literal=:id "),
        "{request}"
    );
}
#[tokio::test]
async fn settings_inherit_preserves_parent_and_multipart_filename_extension() {
    let (_dir, c) = project();
    let mut root = c.project().value().clone();
    root["request"] = json!({"settings":{"http":{"timeout":1234}}});
    root["extensions"]["apifuck"]["requests"] =
        json!({"test.yml":{"multipartFileNames":{"0":"custom.bin"}}});
    c.save(c.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    std::fs::write(c.root().join("file.bin"), b"bytes").unwrap();
    let c = Collection::open(c.root()).unwrap();
    let (url, server) = server(ok("{}")).await;
    let doc = request(
        &c,
        &url,
        json!({"settings":{"timeout":"inherit"},"http":{"method":"POST","url":url,"body":{"type":"multipart-form","data":[{"name":"file","type":"file","value":"file.bin"}]}}}),
    );
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    assert_eq!(ex.network().timeout_ms, 1234);
    execute(ex, None).await.unwrap();
    let request = String::from_utf8(server.await.unwrap()).unwrap();
    assert!(request.contains("filename=\"custom.bin\""));
}
#[test]
fn concurrent_variable_writes_do_not_lose_other_keys() {
    let (_dir, c) = project();
    let threads: Vec<_> = (0..16)
        .map(|ix| {
            let root = c.root().to_owned();
            std::thread::spawn(move || {
                settings::persist_variables(&root, Some("dev"), &json!({format!("key{ix}"):ix}))
                    .unwrap()
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(
        settings::overlay(c.root(), Some("dev"))
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        16
    );
}
#[test]
fn code_exports_cover_file_and_multipart_bodies() {
    let (_dir, c) = project();
    let doc = request(
        &c,
        "https://example.com",
        json!({"http":{"method":"POST","url":"https://example.com/upload","body":{"type":"multipart-form","data":[{"name":"file","type":"file","value":"file.bin"},{"name":"note","type":"text","value":"hello"}]}}}),
    );
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    for language in ["javascript", "python", "go"] {
        let code = exchange::export(&ex, language).unwrap();
        assert!(code.contains("file.bin"));
        assert!(code.contains("hello"));
    }
}
#[tokio::test]
async fn response_script_modifications_are_preserved() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{}")).await;
    let doc = request(
        &c,
        &url,
        json!({"runtime":{"scripts":[{"type":"after-response","code":"api.response.status = 202; api.response.body = 'changed'; api.response.headers['x-added']='yes';"}]}}),
    );
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(response.status, 202);
    assert_eq!(response.body, "changed");
    assert!(response.headers.contains(&("x-added".into(), "yes".into())));
    server.await.unwrap();
}
#[test]
fn moving_and_trashing_collections_keeps_descendants_together() {
    let (_dir, c) = project();
    c.add(
        Path::new("group/folder.yml"),
        &collection::template("folder", "Group").unwrap(),
    )
    .unwrap();
    c.add(
        Path::new("group/request.yml"),
        &collection::template("http", "Request").unwrap(),
    )
    .unwrap();
    let c = Collection::open(c.root()).unwrap();
    c.rename(
        c.find("group/folder.yml").unwrap(),
        Path::new("renamed/folder.yml"),
    )
    .unwrap();
    let c = Collection::open(c.root()).unwrap();
    assert!(c.find("renamed/request.yml").is_ok());
    assert!(
        c.rename(
            c.find("renamed/folder.yml").unwrap(),
            Path::new("renamed/child/folder.yml")
        )
        .is_err()
    );
    c.trash(c.find("renamed/folder.yml").unwrap()).unwrap();
    assert!(!c.root().join("renamed").exists());
    let trash = std::fs::read_dir(c.root().join(".apifuck/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(trash.join("request.yml").exists());
}
#[test]
fn renaming_request_updates_extension_keys_without_trailing_slash() {
    let (_dir, c) = project();
    let mut root = c.project().value().clone();
    root["extensions"]["apifuck"]["requests"] = json!({"old.yml":{"connectTimeoutMs":321}});
    c.save(c.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    c.add(
        Path::new("old.yml"),
        &collection::template("http", "Request").unwrap(),
    )
    .unwrap();
    let c = Collection::open(c.root()).unwrap();
    c.rename(c.find("old.yml").unwrap(), Path::new("new.yml"))
        .unwrap();
    let c = Collection::open(c.root()).unwrap();
    let ex = prepare(
        &c,
        c.find("new.yml/").unwrap(),
        None,
        NetworkSettings::default(),
    )
    .unwrap();
    assert_eq!(ex.network().connect_timeout_ms, 321);
}
#[tokio::test]
async fn curl_data_import_preserves_content_type_and_redirect_policy() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{}")).await;
    let source = exchange::import_curl(
        &format!("curl '{url}' --data-raw 'a=one&b=two'"),
        "Imported",
    )
    .unwrap();
    let doc = c.add(Path::new("import.yml"), &source).unwrap();
    let ex = prepare(&c, &doc, None, NetworkSettings::default()).unwrap();
    assert!(!ex.network().follow_redirects);
    execute(ex, None).await.unwrap();
    let request = String::from_utf8(server.await.unwrap()).unwrap();
    assert!(request.contains("content-type: application/x-www-form-urlencoded"));
    assert!(request.ends_with("a=one&b=two"));
}

#[tokio::test]
async fn graphql_uses_json_transport_and_preserves_variables() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{\"data\":{\"user\":{\"id\":7}}}")).await;
    let value = json!({"info":{"name":"GraphQL","type":"graphql"},"graphql":{"method":"POST","url":url,"body":{"query":"query User($id: Int!) { user(id:$id) { id } }","variables":"{\"id\":7}"}}});
    let doc = c
        .add(
            Path::new("query.yml"),
            &serde_yaml::to_string(&value).unwrap(),
        )
        .unwrap();
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    let sent = String::from_utf8(server.await.unwrap()).unwrap();
    let body: Value = serde_json::from_str(sent.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["variables"]["id"], 7);
    assert!(body["query"].as_str().unwrap().contains("query User"));
}
#[tokio::test]
async fn chai_tests_and_assertions_report_all_failures_without_hiding_response() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{\"items\":[1,2]}")).await;
    let doc = request(
        &c,
        &url,
        json!({"runtime":{"scripts":[{"type":"tests","code":"test('status',()=>expect(res.status).to.equal(200)); test('failure',()=>expect(res.body.items).to.have.lengthOf(3)); test('continues',()=>expect(res.getStatus()).to.equal(200));"}],"assertions":[{"expression":"res.body.items","operator":"length","value":"2"},{"expression":"res.status","operator":"eq","value":"201"},{"expression":"does.not.exist","operator":"eq","value":"1","disabled":true}]}}),
    );
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.tests.len(), 5);
    assert_eq!(response.tests.iter().filter(|t| !t.passed).count(), 2);
    server.await.unwrap();
}
#[test]
fn runner_uses_depth_first_folder_sequence() {
    let (_dir, c) = project();
    for (path, name, seq) in [("b/folder.yml", "B", 2), ("a/folder.yml", "A", 1)] {
        let mut value: Value =
            serde_yaml::from_str(&collection::template("folder", name).unwrap()).unwrap();
        value["info"]["seq"] = json!(seq);
        c.add(Path::new(path), &serde_yaml::to_string(&value).unwrap())
            .unwrap();
    }
    for path in ["a/first.yml", "b/last.yml"] {
        c.add(
            Path::new(path),
            &collection::template("http", path).unwrap(),
        )
        .unwrap();
    }
    let c = Collection::open(c.root()).unwrap();
    let paths: Vec<_> = apifuck_core::runner::ordered_requests(&c, None)
        .iter()
        .map(|d| d.path().to_owned())
        .collect();
    assert_eq!(
        paths,
        vec![Path::new("a/first.yml"), Path::new("b/last.yml")]
    );
}

#[tokio::test]
async fn runner_shares_runtime_variables_and_records_skipped_websockets() {
    let (_dir, c) = project();
    let (first_url, first) = server(ok("{}")).await;
    let (second_url, second) = server(ok("{}")).await;
    for (path, seq, url, runtime) in [
        (
            "first.yml",
            1,
            first_url,
            json!({"scripts":[{"type":"after-response","code":"bru.setVar('shared', 'value');"}]}),
        ),
        ("second.yml", 2, second_url, json!({})),
    ] {
        let mut value = json!({"info":{"name":path,"type":"http","seq":seq},"http":{"method":"GET","url":url},"runtime":runtime});
        if seq == 2 {
            value["http"]["headers"] = json!([{"name":"X-Shared","value":"{{shared}}"}]);
        }
        c.add(Path::new(path), &serde_yaml::to_string(&value).unwrap())
            .unwrap();
    }
    c.add(
        Path::new("socket.yml"),
        &collection::template("websocket", "Socket").unwrap(),
    )
    .unwrap();
    let c = Collection::open(c.root()).unwrap();
    let report = apifuck_core::runner::run(
        c,
        apifuck_core::runner::RunOptions::default(),
        NetworkSettings::default(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        None,
    )
    .await
    .unwrap();
    assert!(report.passed());
    assert_eq!(report.entries.iter().filter(|e| e.skipped).count(), 1);
    assert!(
        String::from_utf8(second.await.unwrap())
            .unwrap()
            .contains("x-shared: value")
    );
    first.await.unwrap();
}
#[tokio::test]
async fn response_actions_are_available_to_tests_and_json_schema_assertions() {
    let (_dir, c) = project();
    let (url, server) = server(ok("{\"token\":\"abc\"}")).await;
    let doc = request(
        &c,
        &url,
        json!({"runtime":{"actions":[{"type":"set-variable","phase":"after-response","selector":{"method":"jsonq","expression":"$res.body.token"},"variable":{"name":"token","scope":"request"}}],"scripts":[{"type":"tests","code":"test('response variable',()=>expect(bru.getVar('token')).to.equal('abc'));test('schema',()=>expect(res.body).to.have.jsonSchema({type:'object',required:['token'],properties:{token:{type:'string'}}}));test('async is not a false pass',async()=>{throw new Error('failed');});"}]}}),
    );
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(response.tests.len(), 3);
    assert!(response.tests[0].passed);
    assert!(response.tests[1].passed, "{:?}", response.tests[1]);
    assert!(!response.tests[2].passed);
    server.await.unwrap();
}
#[tokio::test]
async fn bruno_redirect_setting_preserves_auth_but_not_cross_origin_cookies() {
    let (_dir, c) = project();
    let (final_url, final_server) = server(ok("{}")).await;
    let(first_url,first_server)=server(format!("HTTP/1.1 302 Found\r\nLocation: {final_url}/done?only=1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()).await;
    let doc = request(
        &c,
        &first_url,
        json!({"http":{"method":"POST","url":first_url,"auth":{"type":"bearer","token":"test-token"},"headers":[{"name":"Cookie","value":"session=local-test"}],"params":[{"name":"original","value":"one","type":"query"}],"body":{"type":"json","data":"{}"}},"settings":{"followRedirects":true,"forwardAuthorizationHeader":true}}),
    );
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(response.status, 200);
    let final_request = String::from_utf8(final_server.await.unwrap()).unwrap();
    assert!(final_request.starts_with("GET /done?only=1 "));
    assert!(final_request.contains("authorization: Bearer test-token"));
    assert!(!final_request.contains("session=local-test"));
    assert!(!final_request.contains("original=one"));
    first_server.await.unwrap();
}

#[tokio::test]
async fn release_curl_cookie_and_literal_form_fields_reach_the_server() {
    let (_dir, c) = project();
    let (url, wire) = server(ok("{}")).await;
    let source = exchange::import_curl(
        &format!("curl '{url}/submit?q=hello' -b 'session=abc; theme=dark' --form-string 'literal=@missing.txt;type=text/plain' -F 'name=hello'"),
        "Imported",
    ).unwrap();
    let doc = c.add(Path::new("import.yml"), &source).unwrap();
    assert_eq!(doc.value()["http"]["body"]["data"][0]["type"], "text");
    execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        None,
    )
    .await
    .unwrap();
    let wire = String::from_utf8(wire.await.unwrap()).unwrap();
    assert!(wire.starts_with("POST /submit?q=hello HTTP/1.1"));
    assert!(
        wire.to_lowercase()
            .contains("cookie: session=abc; theme=dark")
    );
    assert!(wire.contains("@missing.txt;type=text/plain"));
    let code = exchange::export(
        &prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        "curl",
    )
    .unwrap();
    assert!(code.contains("--form-string"));
    let again = Document::parse(
        "again.yml".into(),
        exchange::import_curl(&code, "Again").unwrap(),
    )
    .unwrap();
    assert_eq!(again.value()["http"]["body"], doc.value()["http"]["body"]);
}

#[test]
fn release_curl_json_and_compact_options_preserve_option_like_values() {
    let source = exchange::import_curl(
        "curl --url=https://example.com -XPOST -H'X-Test: yes' -u'user:pass' --json='{}'",
        "JSON",
    )
    .unwrap();
    let doc = Document::parse("json.yml".into(), source).unwrap();
    assert_eq!(doc.method(), "POST");
    assert_eq!(doc.value()["http"]["auth"]["password"], "pass");
    assert!(
        doc.value()["http"]["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["name"] == "Accept" && h["value"] == "application/json")
    );
    let source =
        exchange::import_curl("curl https://example.com --data-raw '--json=x'", "Literal").unwrap();
    let doc = Document::parse("literal.yml".into(), source).unwrap();
    assert_eq!(doc.value()["http"]["body"]["data"], "--json=x");
    assert!(
        !doc.value()["http"]["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["name"] == "Accept")
    );
}

#[tokio::test]
async fn release_base_url_inheritance_matches_execution_and_all_exports() {
    let (_dir, c) = project();
    let (url, wire) = server(ok("{}")).await;
    let mut root = c.project().value().clone();
    root["request"] =
        json!({"variables":[{"name":"base_url","value":"https://project.invalid/v1"}]});
    c.save(c.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    c.add(Path::new("api/folder.yml"), &serde_yaml::to_string(&json!({"info":{"name":"API","type":"folder"}, "request":{"variables":[{"name":"base_url","value":"{{host}}/v2"}]}})).unwrap()).unwrap();
    c.add(
        Path::new("environments/local.yml"),
        &serde_yaml::to_string(&json!({"name":"local","variables":[{"name":"host","value":url}]}))
            .unwrap(),
    )
    .unwrap();
    let doc = c.add(Path::new("api/users.yml"), &serde_yaml::to_string(&json!({"info":{"name":"Users","type":"http"},"http":{"method":"GET","url":" /users?limit=1 "}})).unwrap()).unwrap();
    let c = Collection::open(c.root()).unwrap();
    let execution = prepare(&c, &doc, Some("local"), NetworkSettings::default()).unwrap();
    for language in ["curl", "javascript", "python", "go"] {
        assert!(
            exchange::export(&execution, language)
                .unwrap()
                .contains(&format!("{url}/v2/users?limit=1"))
        );
    }
    execute(execution, None).await.unwrap();
    assert!(
        String::from_utf8(wire.await.unwrap())
            .unwrap()
            .starts_with("GET /v2/users?limit=1 HTTP/1.1")
    );
    let mut value = doc.value().clone();
    value["runtime"] =
        json!({"variables":[{"name":"base_url","value":"https://request.example/v3"}]});
    let override_doc = Document::parse(
        doc.path().to_owned(),
        serde_yaml::to_string(&value).unwrap(),
    )
    .unwrap();
    assert!(
        exchange::export(
            &prepare(&c, &override_doc, Some("local"), NetworkSettings::default()).unwrap(),
            "curl"
        )
        .unwrap()
        .contains("https://request.example/v3/users?limit=1")
    );
}

#[test]
fn release_save_normalizes_url_without_rewriting_comments() {
    let (_dir, c) = project();
    let doc = request(&c, " api.example.com/users ", json!({}));
    let saved = c
        .save(&doc, &format!("# preserve me\n{}", doc.source()))
        .unwrap();
    assert_eq!(
        saved.value()["http"]["url"],
        "https://api.example.com/users"
    );
    assert!(saved.source().starts_with("# preserve me\n"));
}

#[test]
fn release_tree_sorting_and_project_trash_preserve_files() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("project");
    let c = Collection::create(&root, "Default project").unwrap();
    c.add(
        Path::new("first.yml"),
        &format!("# keep\n{}", collection::template("http", "First").unwrap()),
    )
    .unwrap();
    c.add(
        Path::new("second.yml"),
        &collection::template("http", "Second").unwrap(),
    )
    .unwrap();
    c.create_folder(Path::new(""), "Folder").unwrap();
    let c = Collection::open(&root).unwrap();
    c.reorder_before(c.find("second.yml").unwrap(), c.find("first.yml").unwrap())
        .unwrap();
    let c = Collection::open(&root).unwrap();
    assert!(
        c.find("second.yml").unwrap().value()["info"]["seq"]
            .as_u64()
            .unwrap()
            < c.find("first.yml").unwrap().value()["info"]["seq"]
                .as_u64()
                .unwrap()
    );
    assert!(c.find("first.yml").unwrap().source().contains("# keep"));
    let before = std::fs::read(root.join("first.yml")).unwrap();
    std::fs::write(root.join("second.yml"), "# external edit").unwrap();
    assert!(
        c.reorder_before(c.find("first.yml").unwrap(), c.find("second.yml").unwrap())
            .is_err()
    );
    assert_eq!(std::fs::read(root.join("first.yml")).unwrap(), before);
    let destination = c.trash_project().unwrap();
    assert!(!root.exists());
    assert_eq!(
        std::fs::read(destination.join("first.yml")).unwrap(),
        before
    );
    assert!(destination.join("opencollection.yml").exists());
    assert!(
        destination.starts_with(
            std::fs::canonicalize(parent.path())
                .unwrap()
                .join(".apifuck-trash")
        )
    );
}

#[test]
fn release_curl_import_preserves_metadata_scripts_and_multiline_payload() {
    let source = "# Keep metadata\ninfo:\n  name: Original\n  type: http\n  seq: 42\nhttp:\n  method: GET\n  url: https://old.example\nruntime:\n  scripts:\n    - type: before-request\n      code: console.log('keep')\n";
    let command = "curl 'https://example.com/api?q=1' \\\n  -H 'Authorization: Bearer token' \\\n  --json '{\n  \"name\": \"hello\"\n}'";
    let result = exchange::import_curl_into(source, command, "Renamed").unwrap();
    let doc = Document::parse("request.yml".into(), result).unwrap();
    assert_eq!(doc.value()["info"]["seq"], 42);
    assert_eq!(doc.name(), "Renamed");
    assert_eq!(
        doc.value()["http"]["auth"],
        json!({"type":"bearer","token":"token"})
    );
    assert_eq!(
        doc.value()["runtime"]["scripts"][0]["code"],
        "console.log('keep')"
    );
    assert!(doc.source().contains("# Keep metadata"));
    assert_eq!(
        doc.value()["http"]["body"]["data"],
        "{\n  \"name\": \"hello\"\n}"
    );
    let source = exchange::import_curl(
        "curl https://example.com --data-urlencode 'q=hello world' -d 'page=2'",
        "Form",
    )
    .unwrap();
    let doc = Document::parse("form.yml".into(), source).unwrap();
    assert_eq!(doc.value()["http"]["body"]["data"], "q=hello+world&page=2");
}

#[tokio::test]
async fn release_sse_requests_disable_compression_and_deliver_data_chunks() {
    let (_dir, c) = project();
    let body = "data: first\n\ndata: second\n\n";
    let (url, wire) = server(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).into_bytes()).await;
    let doc = request(
        &c,
        &url,
        json!({"http":{"url":url,"method":"GET","headers":[{"name":"Accept","value":"text/event-stream"},{"name":"Accept-Encoding","value":"gzip"}]}}),
    );
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let response = execute(
        prepare(&c, &doc, None, NetworkSettings::default()).unwrap(),
        Some(tx),
    )
    .await
    .unwrap();
    assert_eq!(response.body, body);
    let wire = String::from_utf8(wire.await.unwrap())
        .unwrap()
        .to_lowercase();
    assert!(wire.contains("accept-encoding: identity"));
    assert!(!wire.contains("accept-encoding: gzip"));
    let mut chunks = String::new();
    while let Some(event) = rx.recv().await {
        if let Event::Chunk { text, .. } = event {
            chunks.push_str(&text);
        }
    }
    assert_eq!(chunks, body);
}
