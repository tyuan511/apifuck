use apifuck_core::{Collection, import};
use serde_json::json;
#[test]
fn postman_preserves_tree_variables_disabled_rows_and_scripts_without_executing() {
    let source = json!({"info":{"name":"Imported","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"variable":[{"key":"base_url","value":"https://example.com"}],"auth":{"type":"bearer","bearer":[{"key":"token","value":"token"}]},"item":[{"name":"../../escape","auth":{"type":"noauth"},"item":[{"name":"Request","event":[{"listen":"prerequest","script":{"exec":["pm.sendRequest('https://evil.test');"]}}],"request":{"method":"POST","url":{"raw":"{{base_url}}/users?id=1&ignored=2","query":[{"key":"id","value":"1"},{"key":"ignored","value":"2","disabled":true}]},"body":{"mode":"raw","raw":"{\"name\":\"A\"}","options":{"raw":{"language":"json"}}}}}]}]});
    let plan = import::parse(&source.to_string()).unwrap();
    assert_eq!(plan.request_count(), 1);
    assert!(!plan.warnings.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let root = plan.write_new(dir.path()).unwrap();
    let collection = Collection::open(&root).unwrap();
    let request = collection
        .documents()
        .iter()
        .find(|d| d.is_request())
        .unwrap();
    assert_eq!(request.value()["http"]["url"], "{{base_url}}/users");
    assert_eq!(request.value()["http"]["params"][1]["disabled"], true);
    assert!(request.value()["http"].get("auth").is_none());
    assert!(
        request.value()["docs"]
            .as_str()
            .unwrap()
            .contains("pm.sendRequest")
    );
    assert!(request.value()["runtime"].get("scripts").is_none());
    assert!(root.starts_with(dir.path()));
    assert!(!dir.path().join("escape").exists());
    let second = plan.write_new(dir.path()).unwrap();
    assert_ne!(root, second);
}
#[test]
fn openapi_local_refs_parameter_overrides_examples_and_security_are_valid() {
    let source = json!({"openapi":"3.1.0","info":{"title":"Pets","version":"1"},"servers":[{"url":"https://example.com/v1"}],"components":{"parameters":{"id":{"name":"id","in":"path","required":true,"schema":{"type":"integer","example":7}}},"schemas":{"Pet":{"type":"object","properties":{"name":{"type":"string","example":"Milo"}}}},"securitySchemes":{"token":{"type":"http","scheme":"bearer"}}},"security":[{"token":[]}],"paths":{"/pets/{id}":{"parameters":[{"$ref":"#/components/parameters/id"},{"name":"q","in":"query","schema":{"type":"string","default":"old"}}],"post":{"tags":["Pets"],"parameters":[{"name":"q","in":"query","example":"new"}],"requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}}}});
    let plan = import::parse(&source.to_string()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = plan.write_new(dir.path()).unwrap();
    let c = Collection::open(root).unwrap();
    let request = c.documents().iter().find(|d| d.is_request()).unwrap();
    assert_eq!(
        request.value()["http"]["params"].as_array().unwrap().len(),
        2
    );
    assert_eq!(request.value()["http"]["params"][1]["value"], "new");
    assert!(
        request.value()["http"]["body"]["data"]
            .as_str()
            .unwrap()
            .contains("Milo")
    );
    assert_eq!(request.value()["http"]["auth"]["token"], "{{token_token}}");
}
#[test]
fn rejects_external_references_unknown_formats_and_unsupported_auth() {
    assert!(import::parse("openapi: 3.1.0\ninfo: {title: T}\npaths:\n  /test:\n    $ref: 'https://example.com/paths'\n").is_err());
    assert!(import::parse("{}").is_err());
    let source = json!({"swagger":"2.0","info":{"title":"Legacy"},"host":"example.com","paths":{"/test":{"get":{"responses":{}}}}});
    assert_eq!(
        import::parse(&source.to_string()).unwrap().request_count(),
        1
    );
}

#[test]
fn graphql_and_composed_openapi_schemas_import_without_losing_editor_modes() {
    let source = json!({"info":{"name":"GraphQL","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"Query","request":{"url":"https://example.com/graphql","method":"POST","auth":{"type":"noauth"},"body":{"mode":"graphql","graphql":{"query":"query { user { id } }","variables":"{\"limit\":{{limit}}}"}}}}]});
    let plan = import::parse(&source.to_string()).unwrap();
    let request = plan
        .documents()
        .iter()
        .find(|(path, _)| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("api_")
        })
        .unwrap();
    let value: serde_json::Value = serde_yaml::from_str(&request.1).unwrap();
    assert_eq!(value["info"]["type"], "graphql");
    assert_eq!(
        value["graphql"]["body"]["variables"],
        "{\"limit\":{{limit}}}"
    );
    let schema = json!({"allOf":[{"type":"object","properties":{"a":{"type":"integer","example":3}}},{"type":"object","properties":{"b":{"oneOf":[{"type":"string","example":"first"},{"type":"integer"}]}}}]});
    let mut source =
        json!({"openapi":"3.0.3","info":{"title":"Composed"},"paths":{"/test":{"post":{}}}});
    source["paths"]["/test"]["post"]["requestBody"] =
        json!({"content":{"application/json":{"schema":schema}}});
    assert_eq!(
        import::parse(&source.to_string()).unwrap().request_count(),
        1
    );
}

#[tokio::test]
async fn regression_openapi_embedded_path_parameters_match_exports_and_wire() {
    use apifuck_core::{NetworkSettings, exchange, execute, prepare};
    use axum::{Router, http::Uri, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(get(|uri: Uri| async move { uri.to_string() })),
        )
        .await
        .unwrap();
    });
    let source = json!({"openapi":"3.1.0","info":{"title":"Reports"},"servers":[{"url":base}],"paths":{"/reports/{id}.json":{"get":{"parameters":[{"in":"path","name":"id","required":true,"example":"report/123 #?"}]}}}});
    let plan = import::parse(&source.to_string()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = plan.write_new(dir.path()).unwrap();
    let collection = Collection::open(&root).unwrap();
    let document = collection
        .documents()
        .iter()
        .find(|doc| doc.is_request())
        .unwrap();
    let mut network = NetworkSettings::default();
    network.proxy = Some(String::new());
    network.timeout_ms = 5000;
    let expected = "/reports/report%2F123%20%23%3F.json";
    let execution = prepare(&collection, document, None, network).unwrap();
    for language in ["curl", "javascript", "python", "go"] {
        let code = exchange::export(&execution, language).unwrap();
        assert!(code.contains(expected), "{language}: {code}");
    }
    assert_eq!(execute(execution, None).await.unwrap().body, expected);
    server.abort();
}

#[tokio::test]
async fn regression_postman_multipart_preserves_content_types_on_text_and_files() {
    use apifuck_core::{NetworkSettings, execute, prepare};
    use axum::{Router, body::Bytes, routing::post};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/",
                post(|body: Bytes| async move { String::from_utf8(body.to_vec()).unwrap() }),
            ),
        )
        .await
        .unwrap();
    });
    let source = json!({"info":{"name":"Parts","schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"Upload","request":{"method":"POST","url":url,"body":{"mode":"formdata","formdata":[{"key":"metadata","type":"text","value":"{\"name\":\"test\"}","contentType":"application/json"},{"key":"documents","type":"file","src":["one.json","two.json"],"contentType":"application/vnd.test+json"},{"key":"disabled","type":"text","value":"not sent","contentType":"text/plain","disabled":true}]}}}]});
    let plan = import::parse(&source.to_string()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = plan.write_new(dir.path()).unwrap();
    let collection = Collection::open(&root).unwrap();
    let document = collection
        .documents()
        .iter()
        .find(|doc| doc.is_request())
        .unwrap();
    let parts = document.value()["http"]["body"]["data"].as_array().unwrap();
    assert_eq!(parts[0]["contentType"], "application/json");
    assert_eq!(parts[1]["contentType"], "application/vnd.test+json");
    assert_eq!(parts[2]["disabled"], true);
    for name in ["one.json", "two.json"] {
        std::fs::write(root.join(name), "{\"file\":true}").unwrap();
    }
    let mut network = NetworkSettings::default();
    network.proxy = Some(String::new());
    network.timeout_ms = 5000;
    let response = execute(prepare(&collection, document, None, network).unwrap(), None)
        .await
        .unwrap();
    let body = response.body.to_ascii_lowercase();
    assert_eq!(
        body.matches("content-type: application/json\r\n").count(),
        1
    );
    assert_eq!(
        body.matches("content-type: application/vnd.test+json\r\n")
            .count(),
        2
    );
    assert!(!body.contains("not sent"));
    server.abort();
}
