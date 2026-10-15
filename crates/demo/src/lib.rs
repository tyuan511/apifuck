//! A deterministic, loopback-only API playground. All credentials and records are fixtures.
use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Schema, SimpleObject};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, Multipart, Path, Query, RawQuery, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use base64::Engine as _;
use futures_util::stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::RwLock;

pub const TOKEN: &str = "demo-token-apifuck";
pub const API_KEY: &str = "af_demo_local_2026";

/// Keep the editable demo environment and local overrides on the same loopback server.
pub fn configure_workspace(
    root: &std::path::Path,
    base_url: &str,
    ws_url: &str,
) -> anyhow::Result<()> {
    use anyhow::{Context, ensure};
    let collection = apifuck_core::Collection::open(root)?;
    let document = collection.find("environments/local.yml")?;
    let mut value = document.value().clone();
    let variables = value["variables"]
        .as_array_mut()
        .context("Demo environment variables must be a list")?;
    for (name, endpoint) in [("base_url", base_url), ("ws_url", ws_url)] {
        if let Some(variable) = variables.iter_mut().find(|v| v["name"] == name) {
            ensure!(
                variable["secret"] != true,
                "Demo endpoint cannot be a secret"
            );
            variable["value"] = json!(endpoint);
            variable["disabled"] = json!(false);
        } else {
            variables.push(json!({"name":name,"value":endpoint}));
        }
    }
    collection.save(
        document,
        &apifuck_core::editing::update(document.source(), &value)?,
    )?;
    apifuck_core::settings::persist_variables(
        root,
        Some("local"),
        &json!({"base_url":base_url,"ws_url":ws_url}),
    )
}
#[derive(Clone, Serialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
struct Profile {
    city: String,
    languages: Vec<String>,
}
#[derive(Clone, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
#[graphql(rename_fields = "camelCase")]
struct User {
    id: i32,
    name: String,
    email: String,
    role: String,
    active: bool,
    profile: Profile,
    created_at: String,
}
struct Database {
    users: BTreeMap<i32, User>,
    next_id: i32,
}
impl Default for Database {
    fn default() -> Self {
        let users = [
            (1, "Ada Lovelace", "ada@example.test", "admin", "London"),
            (2, "Lin Chen", "lin@example.test", "developer", "Shanghai"),
            (3, "Demo User", "demo@example.test", "designer", "Shenzhen"),
        ]
        .into_iter()
        .map(|(id, name, email, role, city)| {
            (
                id,
                User {
                    id,
                    name: name.into(),
                    email: email.into(),
                    role: role.into(),
                    active: true,
                    profile: Profile {
                        city: city.into(),
                        languages: vec!["English".into(), "中文".into()],
                    },
                    created_at: "2026-10-15T09:00:00Z".into(),
                },
            )
        })
        .collect();
        Self {
            users,
            next_id: 100,
        }
    }
}
#[derive(Clone)]
struct AppState {
    db: Arc<RwLock<Database>>,
    schema: DemoSchema,
}
type DemoSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;
struct QueryRoot;
#[Object]
impl QueryRoot {
    async fn health(&self) -> &str {
        "ok"
    }
    async fn user(&self, ctx: &Context<'_>, id: i32) -> Option<User> {
        ctx.data_unchecked::<Arc<RwLock<Database>>>()
            .read()
            .await
            .users
            .get(&id)
            .cloned()
    }
    async fn users(&self, ctx: &Context<'_>) -> Vec<User> {
        ctx.data_unchecked::<Arc<RwLock<Database>>>()
            .read()
            .await
            .users
            .values()
            .cloned()
            .collect()
    }
}
fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message},"service":"apifuck-demo"})),
    )
        .into_response()
}

pub fn router() -> Router {
    let db = Arc::new(RwLock::new(Database::default()));
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(db.clone())
        .finish();
    Router::new()
        .route("/health", get(health))
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/{id}",
            get(get_user)
                .patch(update_user)
                .put(update_user)
                .delete(delete_user),
        )
        .route("/auth/login", post(login))
        .route("/auth/bearer", get(bearer))
        .route("/auth/basic", get(basic))
        .route("/auth/api-key", get(api_key))
        .route(
            "/echo",
            get(echo).post(echo).put(echo).patch(echo).delete(echo),
        )
        .route("/upload", post(upload))
        .route("/graphql", post(graphql_post).get(graphql_get))
        .route("/events", get(events))
        .route("/socket", get(socket))
        .route("/delay", get(delay))
        .route("/status/{code}", get(status))
        .route("/redirect", get(redirect))
        .route("/reset", post(reset))
        .fallback(|| async {
            failure(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "This route does not exist. Try GET /health.",
            )
        })
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .layer(middleware::from_fn(response_headers))
        .with_state(AppState { db, schema })
}
async fn response_headers(request: axum::extract::Request, next: Next) -> Response {
    let trace = request.headers().get("x-trace-id").cloned();
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-demo-server", "apifuck".parse().unwrap());
    if let Some(trace) = trace {
        response.headers_mut().insert("x-trace-id", trace);
    }
    response
}
async fn health() -> Json<Value> {
    Json(
        json!({"service":"apifuck-demo","status":"ok","version":"1.0.0","message":"Ready to send requests ⚡","capabilities":["REST","Auth","Multipart","GraphQL","SSE","WebSocket"]}),
    )
}
#[derive(Default, Deserialize)]
struct ListQuery {
    limit: Option<usize>,
    page: Option<usize>,
    role: Option<String>,
    q: Option<String>,
}
async fn list_users(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Json<Value> {
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let page = query.page.unwrap_or(1).max(1);
    let db = state.db.read().await;
    let rows: Vec<_> = db
        .users
        .values()
        .filter(|u| {
            query.role.as_ref().is_none_or(|role| &u.role == role)
                && query
                    .q
                    .as_ref()
                    .is_none_or(|q| u.name.to_lowercase().contains(&q.to_lowercase()))
        })
        .collect();
    let total = rows.len();
    let offset = page.saturating_sub(1).saturating_mul(limit);
    let users: Vec<_> = rows.into_iter().skip(offset).take(limit).cloned().collect();
    Json(json!({"users":users,"pagination":{"page":page,"limit":limit,"total":total}}))
}
async fn get_user(State(state): State<AppState>, Path(id): Path<i32>) -> Response {
    match state.db.read().await.users.get(&id).cloned() {
        Some(user) => Json(user).into_response(),
        None => failure(
            StatusCode::NOT_FOUND,
            "USER_NOT_FOUND",
            &format!("User {id} was not found"),
        ),
    }
}
#[derive(Deserialize)]
struct CreateUser {
    name: String,
    email: String,
    role: Option<String>,
    city: Option<String>,
}
async fn create_user(State(state): State<AppState>, Json(input): Json<CreateUser>) -> Response {
    if input.name.trim().is_empty() || !input.email.contains('@') {
        return failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR",
            "name and a valid email are required",
        );
    }
    let mut db = state.db.write().await;
    let id = db.next_id;
    db.next_id += 1;
    let user = User {
        id,
        name: input.name,
        email: input.email,
        role: input.role.unwrap_or("developer".into()),
        active: true,
        profile: Profile {
            city: input.city.unwrap_or("Shanghai".into()),
            languages: vec!["中文".into(), "English".into()],
        },
        created_at: "2026-10-15T10:00:00Z".into(),
    };
    db.users.insert(id, user.clone());
    (StatusCode::CREATED, Json(user)).into_response()
}
#[derive(Deserialize)]
struct UpdateUser {
    name: Option<String>,
    email: Option<String>,
    role: Option<String>,
    active: Option<bool>,
}
async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(input): Json<UpdateUser>,
) -> Response {
    let mut db = state.db.write().await;
    let Some(user) = db.users.get_mut(&id) else {
        return failure(
            StatusCode::NOT_FOUND,
            "USER_NOT_FOUND",
            "Create the demo user first",
        );
    };
    if let Some(name) = input.name {
        user.name = name;
    }
    if let Some(email) = input.email {
        user.email = email;
    }
    if let Some(role) = input.role {
        user.role = role;
    }
    if let Some(active) = input.active {
        user.active = active;
    }
    Json(user.clone()).into_response()
}
async fn delete_user(State(state): State<AppState>, Path(id): Path<i32>) -> StatusCode {
    state.db.write().await.users.remove(&id);
    StatusCode::NO_CONTENT
}
#[derive(Deserialize)]
struct Login {
    email: String,
    password: String,
}
async fn login(Json(input): Json<Login>) -> Response {
    if input.email == "demo@apifuck.dev" && input.password == "demo123" {
        Json(json!({"access_token":TOKEN,"token_type":"Bearer","expires_in":3600,"user":{"id":1,"name":"Demo Admin","email":input.email}})).into_response()
    } else {
        failure(
            StatusCode::UNAUTHORIZED,
            "INVALID_CREDENTIALS",
            "Use demo@apifuck.dev / demo123",
        )
    }
}
async fn bearer(headers: HeaderMap) -> Response {
    if headers.get("authorization").and_then(|v| v.to_str().ok())
        == Some(&format!("Bearer {TOKEN}"))
    {
        Json(json!({"authenticated":true,"method":"bearer","permissions":["users:read","users:write"]})).into_response()
    } else {
        failure(
            StatusCode::UNAUTHORIZED,
            "INVALID_TOKEN",
            "A valid demo Bearer token is required",
        )
    }
}
async fn basic(headers: HeaderMap) -> Response {
    let expected = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("demo:demo123")
    );
    if headers.get("authorization").and_then(|v| v.to_str().ok()) == Some(&expected) {
        Json(json!({"authenticated":true,"method":"basic","username":"demo"})).into_response()
    } else {
        failure(
            StatusCode::UNAUTHORIZED,
            "INVALID_BASIC_AUTH",
            "Use demo / demo123",
        )
    }
}
async fn api_key(headers: HeaderMap, Query(query): Query<BTreeMap<String, String>>) -> Response {
    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| query.get("api_key").map(String::as_str));
    if key == Some(API_KEY) {
        Json(json!({"authenticated":true,"method":"api-key","placement":if headers.contains_key("x-api-key"){"header"}else{"query"}})).into_response()
    } else {
        failure(
            StatusCode::UNAUTHORIZED,
            "INVALID_API_KEY",
            "Use the api_key variable from the local environment",
        )
    }
}
async fn echo(method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Json<Value> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let header_values: BTreeMap<_, _> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let query: Vec<_> = url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect();
    let form: Vec<_> = if content_type.contains("application/x-www-form-urlencoded") {
        url::form_urlencoded::parse(&body)
            .map(|(name, value)| json!({"name":name,"value":value}))
            .collect()
    } else {
        vec![]
    };
    let value =
        serde_json::from_slice::<Value>(&body).unwrap_or(json!(String::from_utf8_lossy(&body)));
    Json(
        json!({"method":method.as_str(),"path":uri.path(),"query":query,"headers":header_values,"contentType":content_type,"body":value,"form":form,"byteLength":body.len(),"base64":base64::engine::general_purpose::STANDARD.encode(&body)}),
    )
}
async fn upload(mut multipart: Multipart) -> Response {
    let mut fields = vec![];
    let mut files = vec![];
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_owned();
                let filename = field.file_name().map(str::to_owned);
                let mime = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                match field.bytes().await {
                    Ok(bytes) => {
                        if let Some(filename) = filename {
                            files.push(json!({"field":name,"filename":filename,"contentType":mime,"size":bytes.len()}));
                        } else {
                            fields
                                .push(json!({"name":name,"value":String::from_utf8_lossy(&bytes)}));
                        }
                    }
                    Err(error) => {
                        return failure(
                            StatusCode::BAD_REQUEST,
                            "UPLOAD_ERROR",
                            &error.to_string(),
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "MULTIPART_ERROR",
                    &error.to_string(),
                );
            }
        }
    }
    (
        StatusCode::CREATED,
        Json(json!({"uploaded":true,"files":files,"fields":fields})),
    )
        .into_response()
}
async fn graphql_post(
    State(state): State<AppState>,
    Json(request): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    Json(state.schema.execute(request).await)
}
async fn graphql_get(
    State(state): State<AppState>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Json<async_graphql::Response> {
    let mut request = async_graphql::Request::new(query.get("query").cloned().unwrap_or_default());
    if let Some(variables) = query.get("variables") {
        request = request.variables(async_graphql::Variables::from_json(
            serde_json::from_str(variables).unwrap_or(json!({})),
        ));
    }
    Json(state.schema.execute(request).await)
}
#[derive(Deserialize)]
struct StreamQuery {
    count: Option<usize>,
    interval: Option<u64>,
}
async fn events(
    Query(query): Query<StreamQuery>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let count = query.count.unwrap_or(8).clamp(1, 30);
    let interval = query.interval.unwrap_or(400).clamp(10, 2000);
    let stream = stream::unfold(0usize, move |index| async move {
        if index >= count {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(interval)).await;
        let data = json!({"index":index+1,"total":count,"message":format!("事件 {} / {}",index+1,count),"progress":((index+1)*100/count)});
        Some((
            Ok(Event::default()
                .id((index + 1).to_string())
                .event("progress")
                .data(data.to_string())),
            index + 1,
        ))
    });
    Sse::new(stream)
}
async fn socket(upgrade: WebSocketUpgrade, RawQuery(query): RawQuery) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| echo_socket(socket, query))
}
async fn echo_socket(mut socket: WebSocket, query: Option<String>) {
    let greeting = json!({"event":"connected","service":"apifuck-demo","query":query,"message":"Send text or JSON to receive an echo"});
    if socket
        .send(Message::Text(greeting.to_string().into()))
        .await
        .is_err()
    {
        return;
    }
    while let Some(Ok(message)) = socket.recv().await {
        if matches!(&message, Message::Close(_)) {
            break;
        }
        if matches!(&message, Message::Text(_) | Message::Binary(_))
            && socket.send(message).await.is_err()
        {
            break;
        }
    }
}
async fn delay(Query(query): Query<BTreeMap<String, String>>) -> Json<Value> {
    let ms = query
        .get("ms")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1500)
        .min(15000);
    tokio::time::sleep(Duration::from_millis(ms)).await;
    Json(json!({"delayedMs":ms,"message":"The delayed response has arrived"}))
}
async fn status(Path(code): Path<u16>) -> Response {
    let status = StatusCode::from_u16(code)
        .ok()
        .filter(|s| s.as_u16() >= 200)
        .unwrap_or(StatusCode::BAD_REQUEST);
    (status,Json(json!({"status":status.as_u16(),"message":status.canonical_reason().unwrap_or("Demo response"),"expected":true}))).into_response()
}
async fn redirect() -> impl IntoResponse {
    (StatusCode::FOUND, [("location", "/health")])
}
async fn reset(State(state): State<AppState>) -> Json<Value> {
    *state.db.write().await = Database::default();
    Json(json!({"reset":true,"users":3,"nextUserId":100}))
}
