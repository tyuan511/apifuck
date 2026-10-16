use apifuck_core::{
    Collection, NetworkSettings,
    runner::{self, RunOptions},
};
use axum::{Router, routing::get};
use serde_json::json;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
#[tokio::test]
async fn data_rows_reset_runtime_variables_and_preserve_chaining_within_each_row() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/", get(|| async { "secret-response-body" })),
        )
        .await
        .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "Data").unwrap();
    for (seq, name, code) in [
        (
            1,
            "First",
            "if (api.vars.get('leak') !== undefined) throw new Error('row leaked'); api.vars.set('leak', api.vars.get('id'));",
        ),
        (
            2,
            "Second",
            "test('chained variable',()=>expect(api.vars.get('leak')).to.equal(api.vars.get('id')));",
        ),
    ] {
        c.add(Path::new(&format!("{seq}.yml")),&serde_yaml::to_string(&json!({"info":{"name":name,"type":"http","seq":seq},"http":{"method":"GET","url":url},"runtime":{"scripts":[{"type":if seq==1{"before-request"}else{"tests"},"code":code}]}})).unwrap()).unwrap();
    }
    let mut options = RunOptions::new(None, None, 2, 0, true);
    options.data = vec![json!({"id":"one"}), json!({"id":"two"})];
    let mut network = NetworkSettings::default();
    network.proxy = Some(String::new());
    network.timeout_ms = 5000;
    let report = runner::run(
        Collection::open(c.root()).unwrap(),
        options,
        network,
        Arc::new(AtomicBool::new(false)),
        None,
    )
    .await
    .unwrap();
    assert!(report.passed(), "{report:?}");
    assert_eq!(report.planned, 8);
    assert_eq!(
        report
            .entries
            .iter()
            .map(|entry| entry.data_row)
            .collect::<Vec<_>>(),
        vec![
            Some(1),
            Some(1),
            Some(2),
            Some(2),
            Some(1),
            Some(1),
            Some(2),
            Some(2)
        ]
    );
    let json = runner::report::render(&report, runner::report::Format::Json).unwrap();
    assert!(!json.contains("secret-response-body"));
    let xml = runner::report::render(&report, runner::report::Format::Junit).unwrap();
    let parsed = roxmltree::Document::parse(&xml).unwrap();
    assert_eq!(
        parsed
            .descendants()
            .filter(|node| node.has_tag_name("testcase"))
            .count(),
        8
    );
    server.abort();
}
