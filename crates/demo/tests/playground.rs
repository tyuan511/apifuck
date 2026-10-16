use apifuck_core::{
    Collection, DocumentKind, NetworkSettings,
    runner::{self, RunOptions},
    settings,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".apifuck" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
async fn start() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, apifuck_demo::router()).await.unwrap();
    });
    (format!("http://{address}"), task)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complete_prefilled_collection_passes_against_real_local_service() {
    let (base, server) = start().await;
    let temp = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/playground"),
        temp.path(),
    );
    let collection = Collection::open(temp.path()).unwrap();
    assert_eq!(
        collection
            .documents()
            .iter()
            .filter(|d| d.is_request())
            .count(),
        32
    );
    apifuck_demo::configure_workspace(
        collection.root(),
        &base,
        &(base.replace("http://", "ws://") + "/socket"),
    )
    .unwrap();
    let collection = Collection::open(temp.path()).unwrap();
    assert_eq!(
        collection.environment(Some("local")).unwrap()["base_url"],
        base
    );
    let options = RunOptions::new(Some("local".into()), None, 1, 0, false);
    let report = tokio::time::timeout(
        Duration::from_secs(90),
        runner::run(
            collection.clone(),
            options,
            {
                let mut network = NetworkSettings::default();
                network.proxy = Some(String::new());
                network
            },
            Arc::new(AtomicBool::new(false)),
            None,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let failures: Vec<_> = report.entries.iter().filter(|e| !e.passed).collect();
    assert!(
        failures.is_empty(),
        "{}",
        serde_json::to_string_pretty(&failures).unwrap()
    );
    assert_eq!(report.entries.iter().filter(|e| e.skipped).count(), 1);
    assert!(
        report
            .entries
            .iter()
            .filter_map(|e| e.response.as_ref())
            .flat_map(|r| &r.tests)
            .count()
            > 60
    );
    assert_eq!(
        settings::overlay(collection.root(), Some("local")).unwrap()["created_user_id"],
        "3"
    );
    assert!(
        collection
            .documents()
            .iter()
            .any(|d| d.kind() == DocumentKind::Graphql)
    );
    let (socket, _) =
        tokio_tungstenite::connect_async(base.replace("http://", "ws://") + "/socket?channel=test")
            .await
            .unwrap();
    let (mut sink, mut stream) = socket.split();
    let welcome = stream.next().await.unwrap().unwrap().into_text().unwrap();
    assert!(welcome.contains("connected"));
    sink.send(tokio_tungstenite::tungstenite::Message::Text(
        "你好 demo".into(),
    ))
    .await
    .unwrap();
    let echo = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap();
    assert_eq!(echo, "你好 demo");
    sink.close().await.unwrap();
    server.abort();
}

#[test]
fn demo_workspace_preserves_other_variables_and_comments() {
    let temp = tempfile::tempdir().unwrap();
    let project = Collection::create(temp.path(), "Demo").unwrap();
    project.add(Path::new("environments/local.yml"), "# Managed local endpoint\nname: local\nvariables:\n  - name: base_url\n    value: http://127.0.0.1:8787\n  - name: token\n    value: keep-me\n").unwrap();
    settings::persist_variables(
        project.root(),
        Some("local"),
        &json!({"base_url":"http://127.0.0.1:9999"}),
    )
    .unwrap();
    apifuck_demo::configure_workspace(
        temp.path(),
        "http://127.0.0.1:43210",
        "ws://127.0.0.1:43210/socket",
    )
    .unwrap();
    let project = Collection::open(temp.path()).unwrap();
    let environment = project.environment(Some("local")).unwrap();
    assert_eq!(environment["base_url"], "http://127.0.0.1:43210");
    assert_eq!(environment["ws_url"], "ws://127.0.0.1:43210/socket");
    assert_eq!(environment["token"], "keep-me");
    let overrides = settings::overlay(project.root(), Some("local")).unwrap();
    assert_eq!(overrides["base_url"], environment["base_url"]);
    assert_eq!(overrides["ws_url"], environment["ws_url"]);
    assert!(
        project
            .find("environments/local.yml")
            .unwrap()
            .source()
            .contains("# Managed local endpoint")
    );
}
#[tokio::test]
async fn graphql_reads_crud_state_and_validates_queries() {
    let (base, server) = start().await;
    let client = reqwest::Client::new();
    let user: serde_json::Value = client
        .post(format!("{base}/users"))
        .json(&json!({"name":"Integration User","email":"integration@example.test"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let response:serde_json::Value=client.post(format!("{base}/graphql")).json(&json!({"query":"query($id:Int!){user(id:$id){id name}}","variables":{"id":user["id"]}})).send().await.unwrap().json().await.unwrap();
    assert_eq!(response["data"]["user"]["name"], "Integration User");
    assert!(response["data"]["user"].get("email").is_none());
    let invalid: serde_json::Value = client
        .post(format!("{base}/graphql"))
        .json(&json!({"query":"query { missingField }"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(invalid["errors"].as_array().is_some_and(|a| !a.is_empty()));
    server.abort();
}
