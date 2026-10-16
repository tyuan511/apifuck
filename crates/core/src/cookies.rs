//! Project/environment-scoped API cookie jars, including retained session cookies.
use crate::collection::atomic_write;
use anyhow::{Context, Result, ensure};
use cookie_store::CookieStore;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
use url::Url;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_COOKIES: usize = 3000;

fn path(root: &Path, environment: Option<&str>) -> PathBuf {
    let key = environment
        .map(|env| format!("env:{env}"))
        .unwrap_or_else(|| "global".into());
    root.join(".apifuck/cookies")
        .join(format!("{:x}.json", Sha256::digest(key.as_bytes())))
}
fn lock(path: &Path) -> Result<File> {
    fs::create_dir_all(path.parent().context("Cookie jar 路径无效")?)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    file.lock()?;
    Ok(file)
}
fn load(path: &Path) -> Result<CookieStore> {
    match fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CookieStore::default());
        }
        Err(error) => return Err(error.into()),
        Ok(metadata) => ensure!(metadata.len() <= MAX_BYTES, "Cookie jar 超过大小限制"),
    }
    cookie_store::serde::json::load(fs::read(path)?.as_slice())
        .map_err(|error| anyhow::anyhow!("无法读取 Cookie jar：{error}"))
}
fn save(path: &Path, store: &CookieStore) -> Result<()> {
    // Expired entries never accumulate. API sessions intentionally survive restarts.
    let cookies: Vec<_> = store.iter_unexpired().cloned().collect();
    ensure!(
        cookies.len() <= MAX_COOKIES,
        "Cookie jar 超过数量限制，请清理 Cookie"
    );
    let source = serde_json::to_string(&cookies)?;
    ensure!(source.len() as u64 <= MAX_BYTES, "Cookie jar 超过大小限制");
    atomic_write(path, &source)
}
pub fn header(root: &Path, environment: Option<&str>, url: &Url) -> Result<Option<String>> {
    let store = load(&path(root, environment))?;
    let mut matches = store.matches(url);
    matches.sort_by_key(|cookie| std::cmp::Reverse(cookie.path.len()));
    let value = matches
        .iter()
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
        .collect::<Vec<_>>()
        .join("; ");
    Ok((!value.is_empty()).then_some(value))
}
pub fn store(root: &Path, environment: Option<&str>, url: &Url, headers: &[String]) -> Result<()> {
    if headers.is_empty() {
        return Ok(());
    }
    let path = path(root, environment);
    let _lock = lock(&path)?;
    let mut store = load(&path)?;
    for header in headers {
        if header.len() > 4096 {
            continue;
        }
        let Ok(cookie) = cookie_store::RawCookie::parse(header.clone()) else {
            continue;
        };
        if let Some(domain) = cookie.domain() {
            let domain = domain.trim_start_matches('.').to_ascii_lowercase();
            if psl::suffix_str(&domain).is_some_and(|suffix| suffix == domain) {
                continue;
            }
        }
        if cookie.secure() == Some(true) && url.scheme() != "https" {
            continue;
        }
        if cookie.name().starts_with("__Secure-")
            && (cookie.secure() != Some(true) || url.scheme() != "https")
        {
            continue;
        }
        if cookie.name().starts_with("__Host-")
            && (cookie.secure() != Some(true)
                || url.scheme() != "https"
                || cookie.domain().is_some()
                || cookie.path() != Some("/"))
        {
            continue;
        }
        // Invalid domains and expired cookies are handled by the RFC6265 store.
        let _ = store.insert_raw(&cookie, url);
    }
    save(&path, &store)
}
#[derive(Clone)]
pub struct CookieRow {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    pub secure: bool,
}
pub fn list(root: &Path, environment: Option<&str>) -> Result<Vec<CookieRow>> {
    let store = load(&path(root, environment))?;
    let mut rows: Vec<_> = store
        .iter_unexpired()
        .map(|cookie| CookieRow {
            domain: match &cookie.domain {
                cookie_store::CookieDomain::HostOnly(domain)
                | cookie_store::CookieDomain::Suffix(domain) => domain.clone(),
                _ => String::new(),
            },
            path: cookie.path.to_string(),
            name: cookie.name().to_owned(),
            value: cookie.value().to_owned(),
            secure: cookie.secure() == Some(true),
        })
        .collect();
    rows.sort_by(|a, b| (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name)));
    Ok(rows)
}
pub fn remove(
    root: &Path,
    environment: Option<&str>,
    domain: &str,
    cookie_path: &str,
    name: &str,
) -> Result<()> {
    let path = path(root, environment);
    let _lock = lock(&path)?;
    let mut store = load(&path)?;
    store.remove(domain, cookie_path, name);
    save(&path, &store)
}
pub fn clear(root: &Path, environment: Option<&str>) -> Result<()> {
    let path = path(root, environment);
    let _lock = lock(&path)?;
    atomic_write(&path, "[]")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookies_obey_domains_paths_secure_expiry_and_environment_isolation() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let url = Url::parse("https://api.example.com/private/login").unwrap();
        store(
            root,
            Some("dev"),
            &url,
            &[
                "session=abc; Path=/private; Secure; HttpOnly".into(),
                "domain=ok; Domain=example.com; Path=/".into(),
                "bad=secret; Domain=com".into(),
                "other=secret; Domain=other.com".into(),
            ],
        )
        .unwrap();
        assert_eq!(list(root, Some("dev")).unwrap().len(), 2);
        assert!(header(root, None, &url).unwrap().is_none());
        assert!(header(root, Some("prod"), &url).unwrap().is_none());
        assert!(
            header(root, Some("dev"), &url)
                .unwrap()
                .unwrap()
                .contains("session=abc")
        );
        assert!(
            !header(
                root,
                Some("dev"),
                &Url::parse("http://api.example.com/private/x").unwrap()
            )
            .unwrap()
            .unwrap()
            .contains("session")
        );
        assert!(
            !header(
                root,
                Some("dev"),
                &Url::parse("https://api.example.com/private-other").unwrap()
            )
            .unwrap()
            .unwrap()
            .contains("session")
        );
        assert!(
            header(
                root,
                Some("dev"),
                &Url::parse("https://elsewhere.test/").unwrap()
            )
            .unwrap()
            .is_none()
        );
        store(
            root,
            Some("dev"),
            &url,
            &["session=; Path=/private; Max-Age=0; Secure".into()],
        )
        .unwrap();
        assert_eq!(list(root, Some("dev")).unwrap().len(), 1);
        remove(root, Some("dev"), "example.com", "/", "domain").unwrap();
        assert!(list(root, Some("dev")).unwrap().is_empty());
    }
    #[test]
    fn concurrent_updates_preserve_cookies_and_bad_storage_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::parse("https://example.com/").unwrap();
        std::thread::scope(|scope| {
            for i in 0..20 {
                let root = dir.path();
                let url = &url;
                scope.spawn(move || {
                    store(root, None, url, &[format!("key{i}=value; Path=/")]).unwrap()
                });
            }
        });
        assert_eq!(list(dir.path(), None).unwrap().len(), 20);
        fs::write(path(dir.path(), None), "broken").unwrap();
        assert!(store(dir.path(), None, &url, &["key=value".into()]).is_err());
        clear(dir.path(), None).unwrap();
        assert!(list(dir.path(), None).unwrap().is_empty());
    }
}
