use apifuck_core::{Collection, Document, NetworkSettings, engine, exchange, prepare, settings};
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn previews_follow_execution_scopes_environment_switches_and_unsaved_request_values() {
    let dir = tempfile::tempdir().unwrap();
    let collection = Collection::create(dir.path(), "Preview").unwrap();
    let mut root = collection.project().value().clone();
    root["request"] = json!({"variables": [
        {"name":"scope", "value":"project"},
        {"name":"project_only", "value":"inherited"}
    ]});
    collection
        .save(collection.project(), &serde_yaml::to_string(&root).unwrap())
        .unwrap();
    for (path, value) in [
        (
            "api/folder.yml",
            json!({"info":{"name":"API", "type":"folder"}, "request":{"variables":[
                {"name":"scope", "value":"folder"},
                {"name":"folder_only", "value":"inherited"}
            ]}}),
        ),
        (
            "environments/base.yml",
            json!({"name":"base", "variables":[
                {"name":"host", "value":"https://base.example"},
                {"name":"inherited", "value":"base"},
                {"name":"disabled", "value":"hidden", "disabled":true},
                {"name":"secret", "secret":true}
            ]}),
        ),
        (
            "environments/local.yml",
            json!({"name":"local", "extends":"base", "variables":[
                {"name":"host", "value":"https://local.example"},
                {"name":"scope", "value":"environment"}
            ]}),
        ),
    ] {
        collection
            .add(Path::new(path), &serde_yaml::to_string(&value).unwrap())
            .unwrap();
    }
    let collection = Collection::open(dir.path()).unwrap();
    settings::persist_variables(
        dir.path(),
        Some("local"),
        &json!({
            "host":"https://overlay.example", "secret":"local-secret", "scope":"overlay"
        }),
    )
    .unwrap();
    let draft = json!({
        "info":{"name":"Unsaved request", "type":"http"},
        "http":{"method":"GET", "url":"{{endpoint}}"},
        "runtime":{"variables":[
            {"name":"endpoint", "value":"{{host}}/{{scope}}"},
            {"name":"scope", "value":"draft"},
            {"name":"count", "value":{"type":"number", "data":"0"}},
            {"name":"disabled", "value":"hidden", "disabled":true}
        ]}
    });
    let request = Document::parse(
        "api/request.yml".into(),
        serde_yaml::to_string(&draft).unwrap(),
    )
    .unwrap();
    for (environment, expected_url) in [
        ("local", "https://overlay.example/draft"),
        ("base", "https://base.example/draft"),
    ] {
        let values = engine::preview_variables(&collection, &request, Some(environment)).unwrap();
        assert_eq!(
            engine::interpolate("{{endpoint}}", &values).unwrap(),
            expected_url
        );
        assert_eq!(values["project_only"], "inherited");
        assert_eq!(values["folder_only"], "inherited");
        assert_eq!(values["inherited"], "base");
        assert_eq!(values["count"], 0);
        assert!(values.get("disabled").is_none());
        assert_eq!(
            values.get("secret"),
            (environment == "local").then_some(&Value::String("local-secret".into()))
        );
        let execution = prepare(
            &collection,
            &request,
            Some(environment),
            NetworkSettings::default(),
        )
        .unwrap();
        assert!(
            exchange::export(&execution, "curl")
                .unwrap()
                .contains(expected_url)
        );
    }
    let mut draft = draft;
    draft["runtime"]["variables"]
        .as_array_mut()
        .unwrap()
        .retain(|v| v["name"] != "scope");
    let request = Document::parse(
        "api/request.yml".into(),
        serde_yaml::to_string(&draft).unwrap(),
    )
    .unwrap();
    let values = engine::preview_variables(&collection, &request, None).unwrap();
    assert_eq!(values["scope"], "folder");
    assert!(values.get("host").is_none());
    assert!(engine::preview_variables(&collection, &request, Some("missing")).is_err());
}
