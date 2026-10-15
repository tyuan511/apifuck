//! GitHub release updates for standalone executables and macOS application bundles.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod installer;
#[cfg(any(target_os = "linux", all(test, target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct Release {
    version: String,
    notes: String,
    url: String,
    sha256: String,
    size: u64,
    bundle: bool,
}
impl Release {
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn notes(&self) -> &str {
        &self.notes
    }
    pub fn select(value: &Value, current: &str, target: &str) -> Result<Option<Self>> {
        Self::select_for_install(value, current, target, false)
    }
    fn select_for_install(
        value: &Value,
        current: &str,
        target: &str,
        bundle: bool,
    ) -> Result<Option<Self>> {
        ensure!(
            !bundle || target.ends_with("apple-darwin"),
            "无效的应用包更新平台"
        );
        if value["draft"] == true || value["prerelease"] == true {
            return Ok(None);
        }
        let tag = value["tag_name"].as_str().context("发布记录缺少版本")?;
        let version = tag.strip_prefix('v').context("发布版本必须以 v 开头")?;
        let parsed_version = semver::Version::parse(version)?;
        if !parsed_version.pre.is_empty()
            || !parsed_version
                .cmp_precedence(&semver::Version::parse(current)?)
                .is_gt()
        {
            return Ok(None);
        }
        let suffix = if bundle {
            ".dmg"
        } else if target.contains("windows") {
            ".exe"
        } else if target.ends_with("unknown-linux-gnu") {
            ".AppImage"
        } else {
            ""
        };
        let legacy = format!("apifuck-desktop-{target}{suffix}");
        let names = [release_asset_name(version, target, bundle), Some(legacy)];
        let Some(asset) = names.iter().flatten().find_map(|name| {
            value["assets"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|asset| asset["name"] == *name)
        }) else {
            return Ok(None);
        };
        let name = asset["name"].as_str().context("发布附件缺少文件名")?;
        let url = asset["browser_download_url"]
            .as_str()
            .context("发布附件缺少下载地址")?;
        let parsed = url::Url::parse(url)?;
        ensure!(
            parsed.scheme() == "https"
                && parsed.host_str() == Some("github.com")
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.port().is_none()
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                // GitHub may encode the '+' in semver build metadata in download URLs.
                && parsed.path().replace("%2B", "+").replace("%2b", "+") == format!("/tyuan511/apifuck/releases/download/{tag}/{name}"),
            "更新附件来源无效"
        );
        let sha256 = asset["digest"]
            .as_str()
            .and_then(|v| v.strip_prefix("sha256:"))
            .context("更新附件缺少 SHA-256 校验值")?;
        ensure!(
            sha256.len() == 64 && sha256.bytes().all(|c| c.is_ascii_hexdigit()),
            "更新校验值格式错误"
        );
        let size = asset["size"].as_u64().context("更新附件缺少大小")?;
        ensure!(
            size > 0 && size <= 512 * 1024 * 1024,
            "更新附件大小超出限制"
        );
        Ok(Some(Self {
            version: version.into(),
            notes: value["body"].as_str().unwrap_or_default().into(),
            url: url.into(),
            sha256: sha256.to_lowercase(),
            size,
            bundle,
        }))
    }
    pub fn verify(&self, bytes: &[u8]) -> Result<()> {
        ensure!(bytes.len() as u64 == self.size, "更新附件大小不匹配");
        ensure!(
            format!("{:x}", Sha256::digest(bytes)) == self.sha256,
            "更新附件校验失败"
        );
        Ok(())
    }
}
fn release_asset_name(version: &str, target: &str, bundle: bool) -> Option<String> {
    let suffix = match (target, bundle) {
        ("aarch64-apple-darwin", true) => "-arm64.dmg",
        ("x86_64-pc-windows-msvc", false) => ".exe",
        ("x86_64-unknown-linux-gnu", false) => ".AppImage",
        _ => return None,
    };
    Some(format!("ApiFuck-{version}{suffix}"))
}

pub fn target() -> String {
    format!(
        "{}-{}",
        std::env::consts::ARCH,
        match std::env::consts::OS {
            "macos" => "apple-darwin",
            "windows" => "pc-windows-msvc",
            _ => "unknown-linux-gnu",
        }
    )
}
pub async fn check() -> Result<Option<Release>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("apifuck-native")
        .build()?;
    let response = client
        .get("https://api.github.com/repos/tyuan511/apifuck/releases/latest")
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let value: Value = response.error_for_status()?.json().await?;
    #[cfg(target_os = "macos")]
    let bundle = app_bundle(&std::env::current_exe()?).is_some();
    #[cfg(not(target_os = "macos"))]
    let bundle = false;
    Release::select_for_install(&value, env!("CARGO_PKG_VERSION"), &target(), bundle)
}

#[cfg(any(target_os = "macos", test))]
fn app_bundle(executable: &Path) -> Option<&Path> {
    let macos = executable.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app")
        .then_some(app)
}
/// Read notes for the installed version, never substitute the latest release.
pub async fn release_notes(version: &str) -> Result<Option<String>> {
    semver::Version::parse(version)?;
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("apifuck-native")
        .build()?
        .get(format!(
            "https://api.github.com/repos/tyuan511/apifuck/releases/tags/v{version}"
        ))
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let value: Value = response.error_for_status()?.json().await?;
    ensure!(
        value["tag_name"] == format!("v{version}"),
        "发布日志版本不匹配"
    );
    Ok(value["body"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned))
}
/// A verified download owns its temporary file, including on failure or cancellation.
#[derive(Debug)]
pub struct DownloadedUpdate {
    path: tempfile::TempPath,
    release: Release,
}
#[derive(Clone, Copy, Debug)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
}
pub type ProgressSender = tokio::sync::watch::Sender<Option<DownloadProgress>>;
impl DownloadedUpdate {
    fn verify(&self) -> Result<()> {
        let mut file = std::fs::File::open(&self.path)?;
        let mut digest = Sha256::new();
        let mut size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let len = file.read(&mut buffer)?;
            if len == 0 {
                break;
            }
            size += len as u64;
            ensure!(size <= self.release.size, "更新附件超过声明大小");
            digest.update(&buffer[..len]);
        }
        ensure!(size == self.release.size, "更新附件大小不匹配");
        ensure!(
            format!("{:x}", digest.finalize()) == self.release.sha256,
            "更新附件校验失败"
        );
        Ok(())
    }
}
pub async fn download(
    release: &Release,
    directory: &Path,
    progress: Option<ProgressSender>,
) -> Result<DownloadedUpdate> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .https_only(true)
        .user_agent("apifuck-native")
        .build()?;
    download_with_client(release, directory, &client, progress).await
}
async fn download_with_client(
    release: &Release,
    directory: &Path,
    client: &reqwest::Client,
    progress: Option<ProgressSender>,
) -> Result<DownloadedUpdate> {
    use futures_util::StreamExt;
    std::fs::create_dir_all(directory)?;
    let response = client.get(&release.url).send().await?.error_for_status()?;
    if let Some(size) = response.content_length() {
        ensure!(size == release.size, "更新附件大小不匹配");
    }
    let mut response = response.bytes_stream();
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    let mut digest = Sha256::new();
    let mut size = 0_u64;
    let mut last_progress = None::<std::time::Instant>;
    while let Some(chunk) = response.next().await {
        let chunk = chunk?;
        size += chunk.len() as u64;
        ensure!(size <= release.size, "更新附件超过声明大小");
        digest.update(&chunk);
        file.write_all(&chunk)?;
        if let Some(progress) = &progress
            && (last_progress.is_none_or(|at| at.elapsed() >= Duration::from_millis(100))
                || size == release.size)
        {
            progress.send_replace(Some(DownloadProgress {
                received: size,
                total: release.size,
            }));
            last_progress = Some(std::time::Instant::now());
        }
    }
    ensure!(size == release.size, "更新附件大小不匹配");
    ensure!(
        format!("{:x}", digest.finalize()) == release.sha256,
        "更新附件校验失败"
    );
    file.as_file().sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(DownloadedUpdate {
        path: file.into_temp_path(),
        release: release.clone(),
    })
}
pub fn install(download: &DownloadedUpdate) -> Result<()> {
    // Recheck immediately before replacing the executable, not only at download time.
    download.verify()?;
    #[cfg(target_os = "macos")]
    if let Some(bundle) = app_bundle(&std::env::current_exe()?) {
        ensure!(download.release.bundle, "应用包不能使用单文件更新");
        return macos::install(download, bundle);
    }
    ensure!(!download.release.bundle, "独立程序不能安装应用包更新");
    #[cfg(target_os = "linux")]
    if let Some(image) = linux::current_appimage()? {
        return installer::install(
            download,
            None,
            Some(&image),
            env!("CARGO_PKG_VERSION"),
            |file| {
                linux::prepare_image(file)
                    .map_err(|error| self_update::Error::verification_rejected(error.to_string()))
            },
        );
    }
    self_replace::self_replace(&download.path)
        .context("无法安装更新，请确认应用位于可写目录（macOS 请先移出磁盘映像）")?;
    Ok(())
}

pub fn restart_path() -> Result<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(image) = linux::current_appimage()? {
        return Ok(image);
    }
    let executable = std::env::current_exe()?;
    #[cfg(target_os = "macos")]
    if let Some(app) = app_bundle(&executable) {
        return Ok(app.to_owned());
    }
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn versioned_asset_names_match_release_tooling_and_accept_legacy_releases() {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../scripts/release/asset-name-fixtures.json"
        ))
        .unwrap();
        for fixture in fixtures.as_array().unwrap() {
            let version = fixture["version"].as_str().unwrap();
            for (platform, target, bundle) in [
                ("mac", "aarch64-apple-darwin", true),
                ("windows", "x86_64-pc-windows-msvc", false),
                ("linux", "x86_64-unknown-linux-gnu", false),
            ] {
                let name = fixture[platform].as_str().unwrap();
                assert_eq!(
                    release_asset_name(version, target, bundle).as_deref(),
                    Some(name)
                );
                if version.contains('-') {
                    continue;
                }
                let mut value = json!({"tag_name":format!("v{version}"),"assets":[{
                    "name":name,"size":6,"digest":format!("sha256:{:x}", Sha256::digest(b"binary")),
                    "browser_download_url":format!("https://github.com/tyuan511/apifuck/releases/download/v{version}/{name}").replace('+', "%2B")
                }]});
                assert!(
                    Release::select_for_install(&value, "0.0.1", target, bundle)
                        .unwrap()
                        .is_some()
                );
                value["assets"][0]["name"] = json!(name.replace(version, "9.9.9"));
                assert!(
                    Release::select_for_install(&value, "0.0.1", target, bundle)
                        .unwrap()
                        .is_none()
                );
                value["assets"][0]["name"] = json!(name);
                value["assets"][0]["browser_download_url"] = json!(format!(
                    "https://github.com/tyuan511/apifuck/releases/download/v9.9.9/{name}"
                ));
                assert!(Release::select_for_install(&value, "0.0.1", target, bundle).is_err());
            }
        }
        assert!(release_asset_name("1.2.3", "x86_64-apple-darwin", true).is_none());
        for target in ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"] {
            assert!(
                Release::select(&release_value(target), "0.1.0", target)
                    .unwrap()
                    .is_some()
            );
        }
    }
    #[test]
    fn application_bundles_select_only_whole_bundle_assets() {
        assert_eq!(
            app_bundle(Path::new(
                "/Applications/ApiFuck.app/Contents/MacOS/apifuck-desktop"
            )),
            Some(Path::new("/Applications/ApiFuck.app"))
        );
        assert!(app_bundle(Path::new("/tmp/apifuck-desktop")).is_none());
        assert!(app_bundle(Path::new("/tmp/Other.app/wrong/MacOS/apifuck-desktop")).is_none());
        let target = "aarch64-apple-darwin";
        let mut value = release_value(target);
        assert!(
            Release::select_for_install(&value, "0.1.0", target, true)
                .unwrap()
                .is_none()
        );
        let mut bundle = value["assets"][0].clone();
        bundle["name"] = json!(format!("apifuck-desktop-{target}.dmg"));
        bundle["browser_download_url"] = json!(format!(
            "https://github.com/tyuan511/apifuck/releases/download/v0.2.0/apifuck-desktop-{target}.dmg"
        ));
        value["assets"].as_array_mut().unwrap().push(bundle);
        assert!(
            Release::select_for_install(&value, "0.1.0", target, true)
                .unwrap()
                .unwrap()
                .bundle
        );
        assert!(
            !Release::select(&value, "0.1.0", target)
                .unwrap()
                .unwrap()
                .bundle
        );
        value["assets"][1]["digest"] = Value::Null;
        assert!(Release::select_for_install(&value, "0.1.0", target, true).is_err());
    }
    #[test]
    fn rejects_wrong_version_asset_and_checksum() {
        let data = b"test executable";
        let mut value = json!({"tag_name":"v0.2.0","assets":[{"name":"apifuck-desktop-aarch64-apple-darwin","browser_download_url":"https://github.com/tyuan511/apifuck/releases/download/v0.2.0/apifuck-desktop-aarch64-apple-darwin","digest":format!("sha256:{:x}",Sha256::digest(data)),"size":data.len()}]});
        let release = Release::select(&value, "0.1.0", "aarch64-apple-darwin")
            .unwrap()
            .unwrap();
        assert!(release.verify(data).is_ok());
        assert!(release.verify(b"altered").is_err());
        assert!(
            Release::select(&value, "0.2.0", "aarch64-apple-darwin")
                .unwrap()
                .is_none()
        );
        assert!(
            Release::select(&value, "0.1.0", "x86_64-pc-windows-msvc")
                .unwrap()
                .is_none()
        );
        value["assets"][0]["browser_download_url"] = json!("https://example.com/program");
        assert!(Release::select(&value, "0.1.0", "aarch64-apple-darwin").is_err());
    }

    fn release_value(target: &str) -> Value {
        let suffix = if target.contains("windows") {
            ".exe"
        } else if target.ends_with("unknown-linux-gnu") {
            ".AppImage"
        } else {
            ""
        };
        let name = format!("apifuck-desktop-{target}{suffix}");
        json!({"tag_name":"v0.2.0", "draft":false, "prerelease":false, "assets":[{
            "name":name, "browser_download_url":format!("https://github.com/tyuan511/apifuck/releases/download/v0.2.0/{name}"),
            "digest":format!("sha256:{:x}", Sha256::digest(b"binary")), "size":6
        }]})
    }

    #[test]
    fn selects_each_platform_and_rejects_untrusted_or_incomplete_assets() {
        for target in [
            "aarch64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
        ] {
            let value = release_value(target);
            assert!(Release::select(&value, "0.1.0", target).unwrap().is_some());
            for key in ["draft", "prerelease"] {
                let mut hidden = value.clone();
                hidden[key] = json!(true);
                assert!(Release::select(&hidden, "0.1.0", target).unwrap().is_none());
            }
            let mut prerelease = value.clone();
            prerelease["tag_name"] = json!("v0.3.0-beta.1");
            assert!(
                Release::select(&prerelease, "0.1.0", target)
                    .unwrap()
                    .is_none()
            );
            assert!(
                Release::select(&value, "0.2.0+local", target)
                    .unwrap()
                    .is_none()
            );
            for bad in [
                "http://github.com/tyuan511/apifuck/releases/download/v0.2.0/app",
                "https://github.com/other/apifuck/releases/download/v0.2.0/app",
                "https://github.com/tyuan511/apifuck/releases/download/v0.1.0/app",
            ] {
                let mut changed = value.clone();
                changed["assets"][0]["browser_download_url"] = json!(bad);
                assert!(Release::select(&changed, "0.1.0", target).is_err());
            }
            for bad in [Value::Null, json!("sha256:bad")] {
                let mut changed = value.clone();
                changed["assets"][0]["digest"] = bad;
                assert!(Release::select(&changed, "0.1.0", target).is_err());
            }
            for size in [0_u64, 512 * 1024 * 1024 + 1] {
                let mut changed = value.clone();
                changed["assets"][0]["size"] = json!(size);
                assert!(Release::select(&changed, "0.1.0", target).is_err());
            }
        }
    }

    #[tokio::test]
    async fn streamed_downloads_verify_and_clean_up_without_installing() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (body, declared_size, digest_body, succeeds) in [
            ("binary", 6, "binary", true),
            ("broken", 6, "binary", false),
            ("binary-too-long", 6, "binary", false),
            ("short", 6, "binary", false),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer).await.unwrap();
                // No content length: exercise stream bounds rather than only header checks.
                let response = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{body}");
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let release = Release {
                version: "0.2.0".into(),
                notes: String::new(),
                url: format!("http://{address}/binary"),
                sha256: format!("{:x}", Sha256::digest(digest_body)),
                size: declared_size,
                bundle: false,
            };
            let directory = tempfile::tempdir().unwrap();
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap();
            let (progress, receiver) = tokio::sync::watch::channel(None);
            let downloaded =
                download_with_client(&release, directory.path(), &client, Some(progress)).await;
            assert_eq!(downloaded.is_ok(), succeeds, "{downloaded:?}");
            if let Ok(downloaded) = downloaded {
                assert!(downloaded.verify().is_ok());
                let progress = receiver.borrow().unwrap();
                assert_eq!((progress.received, progress.total), (6, 6));
                std::fs::write(&downloaded.path, b"edited").unwrap();
                assert!(downloaded.verify().is_err());
                drop(downloaded);
            }
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
            server.await.unwrap();
        }
    }
}
