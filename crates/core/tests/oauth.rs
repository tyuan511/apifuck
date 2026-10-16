use apifuck_core::{Collection, Document, NetworkSettings, execute, oauth, prepare};
use axum::{
    Form, Json, Router,
    extract::State,
    http::HeaderMap,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct ServerState {
    calls: AtomicUsize,
    forms: Mutex<Vec<HashMap<String, String>>>,
    authorization: Mutex<Vec<String>>,
    expire_first: bool,
}
async fn token(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Json<Value> {
    state.forms.lock().unwrap().push(form);
    state.authorization.lock().unwrap().push(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned(),
    );
    let call = state.calls.fetch_add(1, Ordering::SeqCst);
    Json(
        json!({"access_token":format!("access-{call}"),"token_type":"Bearer","refresh_token":format!("refresh-{call}"),"expires_in":if call == 0 && state.expire_first { 1 } else { 3600 }}),
    )
}
async fn resource(headers: HeaderMap) -> Json<Value> {
    Json(json!({"authorization":headers.get("authorization").unwrap().to_str().unwrap()}))
}
async fn server(expire_first: bool) -> (String, Arc<ServerState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(ServerState {
        expire_first,
        ..Default::default()
    });
    let router = Router::new()
        .route("/token", post(token))
        .route("/resource", get(resource))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let job = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, state, job)
}
fn network() -> NetworkSettings {
    let mut settings = NetworkSettings::default();
    settings.proxy = Some(String::new());
    settings.timeout_ms = 5_000;
    settings
}
fn document(c: &Collection, url: &str, flow: &str) -> Document {
    let mut auth = json!({"type":"oauth2","flow":flow,"accessTokenUrl":format!("{url}/token"),"credentials":{"clientId":"id:with space","clientSecret":"secret&value","placement":"basic_auth_header"},"scope":"read write"});
    if flow == "authorization_code" {
        auth["authorizationUrl"] = json!(format!("{url}/authorize"));
        auth["callbackUrl"] = json!("http://127.0.0.1:0/oauth/callback");
    }
    let value = json!({"info":{"name":"OAuth request","type":"http"},"http":{"method":"GET","url":format!("{url}/resource"),"auth":auth}});
    c.add(
        std::path::Path::new("oauth.yml"),
        &serde_yaml::to_string(&value).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn client_credentials_refresh_cache_and_concurrent_calls_share_tokens() {
    let (url, state, job) = server(true).await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "OAuth").unwrap();
    let doc = document(&c, &url, "client_credentials");
    let first = execute(prepare(&c, &doc, None, network()).unwrap(), None)
        .await
        .unwrap();
    assert!(first.body.contains("Bearer access-0"));
    let execution = prepare(&c, &doc, None, network()).unwrap();
    let mut jobs = vec![];
    for _ in 0..5 {
        jobs.push(tokio::spawn(execute(execution.clone(), None)));
    }
    for job in jobs {
        assert!(job.await.unwrap().unwrap().body.contains("Bearer access-1"));
    }
    assert_eq!(state.calls.load(Ordering::SeqCst), 2);
    let forms = state.forms.lock().unwrap();
    assert_eq!(forms[0]["grant_type"], "client_credentials");
    assert_eq!(forms[1]["grant_type"], "refresh_token");
    assert_eq!(forms[1]["refresh_token"], "refresh-0");
    use base64::Engine as _;
    assert_eq!(
        state.authorization.lock().unwrap()[0],
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("id%3Awith+space:secret%26value")
        )
    );
    assert_eq!(
        std::fs::read_to_string(c.root().join("oauth.yml")).unwrap(),
        doc.source()
    );
    assert_eq!(apifuck_core::history::load(c.root()).unwrap().len(), 6);
    job.abort();
}
#[tokio::test]
async fn native_pkce_flow_validates_callback_then_reuses_token_without_opening_browser() {
    let (url, state, job) = server(false).await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "OAuth").unwrap();
    let doc = document(&c, &url, "authorization_code");
    let error = execute(prepare(&c, &doc, None, network()).unwrap(), None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("获取 OAuth"));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let authorization = tokio::spawn(oauth::authorize(
        prepare(&c, &doc, None, network()).unwrap(),
        tx,
    ));
    let url = url::Url::parse(
        &tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let params: HashMap<_, _> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    assert_eq!(params["code_challenge_method"], "S256");
    let redirect = &params["redirect_uri"];
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    assert_eq!(
        client
            .get(redirect)
            .query(&[("state", "wrong"), ("code", "bad")])
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        client
            .get(redirect)
            .query(&[("state", params["state"].as_str()), ("code", "authorized")])
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    tokio::time::timeout(Duration::from_secs(5), authorization)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    {
        let forms = state.forms.lock().unwrap();
        assert_eq!(forms[0]["code"], "authorized");
        assert_eq!(forms[0]["redirect_uri"], *redirect);
        use base64::Engine as _;
        use sha2::Digest;
        assert_eq!(
            params["code_challenge"],
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sha2::Sha256::digest(forms[0]["code_verifier"].as_bytes()))
        );
    }
    let response = execute(prepare(&c, &doc, None, network()).unwrap(), None)
        .await
        .unwrap();
    assert!(response.body.contains("Bearer access-0"));
    assert_eq!(state.calls.load(Ordering::SeqCst), 1);
    assert!(client.get(redirect).send().await.is_err());
    job.abort();
}
#[tokio::test]
async fn cancelling_authorization_closes_the_callback_listener() {
    let (url, _, job) = server(false).await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "OAuth").unwrap();
    let doc = document(&c, &url, "authorization_code");
    let execution = prepare(&c, &doc, None, network()).unwrap();
    let cancel = execution.cancellation();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let authorization = tokio::spawn(oauth::authorize(execution, tx));
    let url = url::Url::parse(
        &tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let redirect = url
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .unwrap()
        .1
        .into_owned();
    cancel.store(true, Ordering::Relaxed);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), authorization)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(
        reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(redirect)
            .send()
            .await
            .is_err()
    );
    job.abort();
}

#[tokio::test]
async fn token_redirects_are_not_followed_and_disabled_auto_fetch_makes_no_call() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let router = Router::new()
        .route(
            "/token",
            post(|| async { axum::response::Redirect::temporary("/leak") }),
        )
        .route(
            "/leak",
            get(move || async move {
                observed.fetch_add(1, Ordering::SeqCst);
                "unexpected"
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "OAuth").unwrap();
    let doc = document(&c, &url, "client_credentials");
    let error = execute(prepare(&c, &doc, None, network()).unwrap(), None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 307"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!error.to_string().contains("secret"));
    server.abort();

    let (url, state, server) = server_fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let c = Collection::create(dir.path(), "OAuth").unwrap();
    let doc = document(&c, &url, "client_credentials");
    let mut value = doc.value().clone();
    value["http"]["auth"]["settings"] = json!({"autoFetchToken":false});
    let doc = Document::parse(
        "disabled.yml".into(),
        serde_yaml::to_string(&value).unwrap(),
    )
    .unwrap();
    assert!(
        execute(prepare(&c, &doc, None, network()).unwrap(), None)
            .await
            .unwrap_err()
            .to_string()
            .contains("获取 OAuth")
    );
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    server.abort();
}
async fn server_fixture() -> (String, Arc<ServerState>, tokio::task::JoinHandle<()>) {
    server(false).await
}

#[tokio::test]
async fn regression_oauth_custom_headers_obey_redirect_policy_across_all_hops() {
    use axum::response::Redirect;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let root_url = format!("http://{address}");
    let external_url = format!("http://localhost:{}/external", address.port());
    let return_url = format!("{root_url}/return");
    let captured = Arc::new(Mutex::new(Vec::<(String, HeaderMap)>::new()));
    let same_headers = captured.clone();
    let external_headers = captured.clone();
    let return_headers = captured.clone();
    let router = Router::new()
        .route("/token", post(token))
        .route("/start", get(|| async { Redirect::temporary("/same") }))
        .route(
            "/same",
            get(move |headers: HeaderMap| {
                same_headers.lock().unwrap().push(("same".into(), headers));
                let target = external_url.clone();
                async move { Redirect::temporary(&target) }
            }),
        )
        .route(
            "/external",
            get(move |headers: HeaderMap| {
                external_headers
                    .lock()
                    .unwrap()
                    .push(("external".into(), headers));
                let target = return_url.clone();
                async move { Redirect::temporary(&target) }
            }),
        )
        .route(
            "/return",
            get(move |headers: HeaderMap| {
                return_headers
                    .lock()
                    .unwrap()
                    .push(("return".into(), headers));
                async { Json(json!({"ok":true})) }
            }),
        )
        .with_state(Arc::new(ServerState::default()));
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    for (header, forward) in [
        ("X-OAuth-Token", false),
        ("X-OAuth-Token", true),
        ("Authorization", false),
        ("Authorization", true),
    ] {
        captured.lock().unwrap().clear();
        let dir = tempfile::tempdir().unwrap();
        let collection = Collection::create(dir.path(), "Redirect credentials").unwrap();
        let original = document(&collection, &root_url, "client_credentials");
        let mut value = original.value().clone();
        value["http"]["url"] = json!(format!("{root_url}/start"));
        value["http"]["auth"]["tokenConfig"] = json!({"placement":{"header":header}});
        let request = Document::parse(
            original.path().to_owned(),
            serde_yaml::to_string(&value).unwrap(),
        )
        .unwrap();
        let mut settings = network();
        settings.forward_authorization_header = forward;
        execute(
            prepare(&collection, &request, None, settings).unwrap(),
            None,
        )
        .await
        .unwrap();
        let hops = captured.lock().unwrap();
        assert_eq!(hops.len(), 3);
        let credential = hops[0]
            .1
            .get(header)
            .expect("same-origin redirect must preserve credentials");
        assert!(credential.to_str().unwrap().starts_with("Bearer access-"));
        for (name, headers) in &hops[1..] {
            assert_eq!(
                headers.get(header),
                forward.then_some(credential),
                "{header}, forward={forward}, hop={name}"
            );
        }
    }
    server.abort();
}
