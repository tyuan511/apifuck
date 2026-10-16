//! OAuth 2.0 client credentials and native authorization-code/PKCE flows.
use crate::{Execution, NetworkSettings, collection::atomic_write, engine::http_client};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
struct Token {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<i64>,
}
impl Token {
    fn valid(&self) -> bool {
        self.expires_at
            .is_none_or(|expiry| expiry > time::OffsetDateTime::now_utc().unix_timestamp() + 10)
    }
}
fn endpoint(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("OAuth 地址无效")?;
    ensure!(
        url.scheme() == "https"
            || (url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"))),
        "OAuth 地址须使用 HTTPS（回环地址除外）"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
        "OAuth 地址不能包含用户名、密码或片段"
    );
    Ok(url)
}
fn validate(auth: &Value) -> Result<()> {
    ensure!(auth["type"] == "oauth2", "当前请求未使用 OAuth 2.0");
    ensure!(
        matches!(
            auth["flow"].as_str(),
            Some("client_credentials" | "authorization_code")
        ),
        "仅支持 OAuth 客户端凭据和授权码流程"
    );
    endpoint(
        auth["accessTokenUrl"]
            .as_str()
            .context("OAuth 缺少访问令牌地址")?,
    )?;
    ensure!(
        auth["credentials"]["clientId"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "OAuth 缺少客户端 ID"
    );
    ensure!(
        auth["tokenConfig"]["source"]
            .as_str()
            .is_none_or(|source| source == "access_token"),
        "OAuth 仅支持 access_token"
    );
    if auth["flow"] == "authorization_code" {
        ensure!(
            auth["pkce"]["disabled"] != true
                && auth["pkce"]["method"]
                    .as_str()
                    .is_none_or(|method| method == "S256"),
            "原生授权码流程需要 PKCE S256"
        );
        ensure!(
            auth["state"].as_str().is_none_or(str::is_empty),
            "授权 state 由应用随机生成，请移除固定 state"
        );
    }
    Ok(())
}
fn token_path(execution: &Execution, auth: &Value) -> PathBuf {
    let key = json!({"environment":execution.environment,"auth":auth}).to_string();
    execution
        .root
        .join(".apifuck/oauth")
        .join(format!("{:x}.json", Sha256::digest(key.as_bytes())))
}
fn read(path: &Path) -> Result<Option<Token>> {
    match fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(metadata) => ensure!(metadata.len() < 1024 * 1024, "OAuth 缓存过大"),
    }
    let token: Token = serde_json::from_slice(&fs::read(path)?).context("OAuth 缓存损坏")?;
    ensure!(!token.access_token.is_empty(), "OAuth 缓存令牌为空");
    Ok(Some(token))
}
async fn lock(path: &Path) -> Result<File> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        fs::create_dir_all(path.parent().context("OAuth 缓存路径无效")?)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))?;
        file.lock()?;
        Ok(file)
    })
    .await?
}
fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
fn additional(auth: &Value, phase: &str) -> Result<Vec<(String, String, String)>> {
    let mut result = vec![];
    for parameter in auth["additionalParameters"][phase]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = parameter["name"]
            .as_str()
            .context("OAuth 附加参数缺少名称")?;
        ensure!(
            ![
                "grant_type",
                "client_id",
                "client_secret",
                "code",
                "redirect_uri",
                "code_verifier",
                "code_challenge",
                "code_challenge_method",
                "state",
                "response_type",
                "refresh_token",
                "scope"
            ]
            .contains(&name),
            "OAuth 附加参数不能覆盖协议字段：{name}"
        );
        let placement =
            parameter["placement"]
                .as_str()
                .unwrap_or(if phase == "authorizationRequest" {
                    "query"
                } else {
                    "body"
                });
        ensure!(
            matches!(placement, "header" | "query" | "body"),
            "OAuth 附加参数位置无效"
        );
        if phase == "authorizationRequest" {
            ensure!(placement == "query", "授权请求附加参数只能位于 query");
        }
        if placement == "header" {
            ensure!(
                ![
                    "authorization",
                    "host",
                    "content-type",
                    "content-length",
                    "transfer-encoding",
                    "cookie"
                ]
                .contains(&name.to_ascii_lowercase().as_str()),
                "OAuth 附加参数不能覆盖认证或传输头"
            );
        }
        result.push((
            name.into(),
            parameter["value"].as_str().unwrap_or("").into(),
            placement.into(),
        ));
    }
    Ok(result)
}
async fn fetch(
    auth: &Value,
    network: &NetworkSettings,
    mut form: Vec<(String, String)>,
    refresh: bool,
) -> Result<Token> {
    let endpoint_key = if refresh
        && auth["refreshTokenUrl"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        "refreshTokenUrl"
    } else {
        "accessTokenUrl"
    };
    let mut url = endpoint(auth[endpoint_key].as_str().context("OAuth 缺少令牌地址")?)?;
    let mut network = network.clone();
    network.follow_redirects = false;
    let client = http_client(&network, &url)?;
    let credentials = &auth["credentials"];
    let id = credentials["clientId"].as_str().unwrap_or("");
    let secret = credentials["clientSecret"].as_str().unwrap_or("");
    let placement = credentials["placement"]
        .as_str()
        .unwrap_or(if secret.is_empty() {
            "body"
        } else {
            "basic_auth_header"
        });
    ensure!(
        matches!(placement, "body" | "basic_auth_header"),
        "OAuth 客户端凭据位置无效"
    );
    if placement == "body" {
        form.push(("client_id".into(), id.into()));
        if !secret.is_empty() {
            form.push(("client_secret".into(), secret.into()));
        }
    }
    let phase = if refresh {
        "refreshTokenRequest"
    } else {
        "accessTokenRequest"
    };
    let extras = additional(auth, phase)?;
    for (name, value, placement) in &extras {
        if placement == "body" {
            form.push((name.clone(), value.clone()));
        }
        if placement == "query" {
            url.query_pairs_mut().append_pair(name, value);
        }
    }
    let mut request = client
        .post(url)
        .timeout(Duration::from_secs(30))
        .header("accept", "application/json");
    if placement == "basic_auth_header" {
        request = request.basic_auth(encode(id), Some(encode(secret)));
    }
    for (name, value, placement) in extras {
        if placement == "header" {
            request = request.header(name, value);
        }
    }
    let response = request
        .form(&form)
        .send()
        .await
        .map_err(|error| anyhow::anyhow!("OAuth 令牌请求失败：{}", error.without_url()))?;
    ensure!(
        response.status().is_success(),
        "OAuth 令牌请求失败：HTTP {}",
        response.status().as_u16()
    );
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(
            bytes.len() + chunk.len() <= 1024 * 1024,
            "OAuth 令牌响应过大"
        );
        bytes.extend(chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).context("OAuth 令牌响应不是 JSON")?;
    let token = value["access_token"]
        .as_str()
        .filter(|value| !value.is_empty())
        .context("OAuth 响应缺少 access_token")?;
    ensure!(
        value["token_type"]
            .as_str()
            .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer")),
        "OAuth 仅支持 Bearer 令牌"
    );
    let expires_at = match value.get("expires_in") {
        None => None,
        Some(expiry) => {
            let seconds = expiry.as_u64().context("OAuth expires_in 必须是非负整数")?;
            Some(
                time::OffsetDateTime::now_utc()
                    .unix_timestamp()
                    .saturating_add(i64::try_from(seconds).context("OAuth 过期时间过大")?),
            )
        }
    };
    Ok(Token {
        access_token: token.into(),
        refresh_token: value["refresh_token"].as_str().map(str::to_owned),
        expires_at,
    })
}
fn grant(auth: &Value) -> Vec<(String, String)> {
    let mut form = vec![("grant_type".into(), "client_credentials".into())];
    if let Some(scope) = auth["scope"].as_str().filter(|s| !s.is_empty()) {
        form.push(("scope".into(), scope.into()));
    }
    form
}
async fn token(execution: &Execution, auth: &Value) -> Result<Token> {
    validate(auth)?;
    let path = token_path(execution, auth);
    if let Some(token) = read(&path)?.filter(Token::valid) {
        return Ok(token);
    }
    let _lock = lock(&path).await?;
    let existing = read(&path)?;
    if let Some(token) = existing.as_ref().filter(|token| token.valid()) {
        return Ok(token.clone());
    }
    let mut token = if let Some(refresh) = existing
        .as_ref()
        .and_then(|token| token.refresh_token.as_ref())
        .filter(|_| auth["settings"]["autoRefreshToken"] != false)
    {
        let mut token = fetch(
            auth,
            &execution.settings,
            vec![
                ("grant_type".into(), "refresh_token".into()),
                ("refresh_token".into(), refresh.clone()),
            ],
            true,
        )
        .await?;
        if token.refresh_token.is_none() {
            token.refresh_token = Some(refresh.clone());
        }
        token
    } else {
        ensure!(
            auth["flow"] == "client_credentials" && auth["settings"]["autoFetchToken"] != false,
            "请先在认证面板获取 OAuth 令牌"
        );
        fetch(auth, &execution.settings, grant(auth), false).await?
    };
    // Never carry an expired refresh token into a completely new client grant.
    if token.refresh_token.as_deref() == Some("") {
        token.refresh_token = None;
    }
    atomic_write(&path, &serde_json::to_string(&token)?)?;
    Ok(token)
}
pub(crate) async fn apply(execution: &mut Execution) -> Result<()> {
    let auth = execution.request["auth"].clone();
    if auth["type"] != "oauth2" {
        return Ok(());
    }
    let cancel = execution.cancel.clone();
    let token = tokio::select! {
        token = token(execution, &auth) => token?,
        _ = async { loop { if cancel.load(Ordering::Relaxed) { break; } tokio::time::sleep(Duration::from_millis(40)).await; } } => bail!("OAuth 授权已取消"),
    };
    let placement = &auth["tokenConfig"]["placement"];
    if let Some(query) = placement["query"].as_str() {
        ensure!(!query.is_empty(), "OAuth Query 名称不能为空");
        if let Some(raw_url) = execution.request["url"].as_str() {
            let mut url = Url::parse(raw_url)?;
            let pairs: Vec<_> = url
                .query_pairs()
                .filter(|(name, _)| name != query)
                .map(|(name, value)| (name.into_owned(), value.into_owned()))
                .collect();
            url.set_query(None);
            if !pairs.is_empty() {
                url.query_pairs_mut().extend_pairs(pairs);
            }
            execution.request["url"] = json!(url.as_str());
        }
        if !execution.request["params"].is_array() {
            execution.request["params"] = json!([]);
        }
        let params = execution.request["params"].as_array_mut().unwrap();
        params.retain(|p| !(p["type"] == "query" && p["name"] == query));
        params.push(json!({"name":query,"value":token.access_token,"type":"query"}));
        execution.request["auth"] = Value::Null;
    } else if let Some(header) = placement["header"]
        .as_str()
        .filter(|header| !header.eq_ignore_ascii_case("authorization"))
    {
        ensure!(!header.is_empty(), "OAuth Header 名称不能为空");
        execution.request["auth"] = json!({"type":"apikey","placement":"header","key":header,"value":format!("Bearer {}", token.access_token)});
    } else {
        execution.request["auth"] = json!({"type":"bearer","token":token.access_token});
    }
    Ok(())
}
fn nonce() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn callback(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).context("OAuth 回调地址无效")?;
    ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "OAuth 回调地址须为 http://127.0.0.1:<port>/<path>"
    );
    Ok(url)
}
fn callback_code(target: &str, redirect: &Url, state: &str) -> Result<String> {
    ensure!(
        target.starts_with('/') && !target.starts_with("//"),
        "OAuth 回调路径无效"
    );
    let url = redirect.join(target)?;
    ensure!(
        url.origin() == redirect.origin() && url.path() == redirect.path(),
        "OAuth 回调路径不匹配"
    );
    let pairs: Vec<_> = url.query_pairs().collect();
    let states: Vec<_> = pairs.iter().filter(|(key, _)| key == "state").collect();
    ensure!(
        states.len() == 1 && states[0].1 == state,
        "OAuth state 不匹配"
    );
    ensure!(
        !pairs.iter().any(|(key, _)| key == "error"),
        "OAuth 授权被拒绝"
    );
    let codes: Vec<_> = pairs.iter().filter(|(key, _)| key == "code").collect();
    ensure!(
        codes.len() == 1 && !codes[0].1.is_empty(),
        "OAuth 回调缺少唯一授权码"
    );
    Ok(codes[0].1.to_string())
}
async fn authorization_code(
    auth: &Value,
    network: &NetworkSettings,
    urls: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<Token> {
    let mut redirect = callback(
        auth["callbackUrl"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("http://127.0.0.1:0/oauth/callback"),
    )?;
    let listener = TcpListener::bind(("127.0.0.1", redirect.port().unwrap_or(80)))
        .await
        .context("OAuth 回调端口不可用")?;
    redirect
        .set_port(Some(listener.local_addr()?.port()))
        .map_err(|_| anyhow::anyhow!("OAuth 回调端口无效"))?;
    let verifier = nonce();
    let state = nonce();
    let mut authorization = endpoint(
        auth["authorizationUrl"]
            .as_str()
            .context("OAuth 缺少授权地址")?,
    )?;
    // Protocol query keys must have one unambiguous value.
    let reserved = [
        "client_id",
        "redirect_uri",
        "response_type",
        "scope",
        "state",
        "code_challenge",
        "code_challenge_method",
    ];
    ensure!(
        !authorization
            .query_pairs()
            .any(|(key, _)| reserved.contains(&key.as_ref())),
        "授权地址不能预设 OAuth 协议参数"
    );
    authorization.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        (
            "client_id",
            auth["credentials"]["clientId"].as_str().unwrap_or(""),
        ),
        ("redirect_uri", redirect.as_str()),
        ("state", &state),
        ("code_challenge", &challenge(&verifier)),
        ("code_challenge_method", "S256"),
    ]);
    if let Some(scope) = auth["scope"].as_str().filter(|s| !s.is_empty()) {
        authorization.query_pairs_mut().append_pair("scope", scope);
    }
    for (key, value, _) in additional(auth, "authorizationRequest")? {
        authorization.query_pairs_mut().append_pair(&key, &value);
    }
    urls.send(authorization.to_string())
        .context("授权界面已关闭")?;
    let code = loop {
        let (mut socket, _) = listener.accept().await?;
        let request = tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let count = socket.read(&mut chunk).await?;
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                ensure!(bytes.len() <= 16 * 1024, "OAuth 回调请求过大");
                if bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                    break;
                }
            }
            let request = std::str::from_utf8(&bytes)?;
            let mut parts = request.lines().next().unwrap_or("").split_whitespace();
            ensure!(parts.next() == Some("GET"), "OAuth 回调必须是 GET");
            callback_code(parts.next().unwrap_or(""), &redirect, &state)
        })
        .await;
        let valid = matches!(&request, Ok(Ok(_)));
        let message = if valid {
            "Authorization received. Return to ApiFuck."
        } else {
            "Invalid OAuth callback."
        };
        let status = if valid { "200 OK" } else { "400 Bad Request" };
        let _ = socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}", message.len()).as_bytes()).await;
        match request {
            Ok(Ok(code)) => break code,
            Ok(Err(error)) if error.to_string() == "OAuth 授权被拒绝" => return Err(error),
            _ => {}
        }
    };
    fetch(
        auth,
        network,
        vec![
            ("grant_type".into(), "authorization_code".into()),
            ("code".into(), code),
            ("redirect_uri".into(), redirect.to_string()),
            ("code_verifier".into(), verifier),
        ],
        false,
    )
    .await
}
/// Explicit user action; only static variables are resolved, with no request scripts or API send.
pub async fn authorize(
    execution: Execution,
    urls: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<()> {
    let cancel = execution.cancel.clone();
    let future = async {
        let mut auth = execution.request["auth"].clone();
        let mut variables = execution.env.clone();
        for (key, value) in execution.variables.as_object().into_iter().flatten() {
            variables[key] = value.clone();
        }
        crate::engine::interpolate_value(&mut auth, &variables)?;
        validate(&auth)?;
        let path = token_path(&execution, &auth);
        let _lock = lock(&path).await?;
        let token = if auth["flow"] == "client_credentials" {
            fetch(&auth, &execution.settings, grant(&auth), false).await?
        } else {
            authorization_code(&auth, &execution.settings, urls).await?
        };
        atomic_write(&path, &serde_json::to_string(&token)?)
    };
    tokio::select! {
        result = future => result,
        _ = tokio::time::sleep(Duration::from_secs(300)) => bail!("OAuth 授权超时"),
        _ = async { loop { if cancel.load(Ordering::Relaxed) { break; } tokio::time::sleep(Duration::from_millis(40)).await; } } => bail!("OAuth 授权已取消"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636_vector_and_callbacks_reject_forgery() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let redirect = callback("http://127.0.0.1:9999/oauth/callback").unwrap();
        assert_eq!(
            callback_code(
                "/oauth/callback?state=correct&code=ok",
                &redirect,
                "correct"
            )
            .unwrap(),
            "ok"
        );
        for target in [
            "/else?state=correct&code=ok",
            "/oauth/callback?state=wrong&code=ok",
            "/oauth/callback?state=correct&code=a&code=b",
            "//evil.test/oauth/callback?state=correct&code=ok",
        ] {
            assert!(callback_code(target, &redirect, "correct").is_err());
        }
        assert!(callback("http://0.0.0.0/callback").is_err());
        assert!(endpoint("http://example.com/token").is_err());
    }
}
