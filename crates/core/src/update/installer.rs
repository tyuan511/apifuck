//! Reuse self_update with the already downloaded and approved bytes, without another network request.
use super::DownloadedUpdate;
use anyhow::{Context, Result, ensure};
use self_update::{
    backends::custom::Update,
    http_client::{HeaderMap, HttpClient, HttpResponse},
    update::{Release, ReleaseAsset, ReleaseSource},
};
use std::{fs, io::Read, path::Path, sync::Arc, time::Duration};

struct ApprovedRelease(Release);
impl ReleaseSource for ApprovedRelease {
    fn get_releases(&self) -> self_update::Result<Vec<Release>> {
        Ok(vec![self.0.clone()])
    }
}

// The UI already downloaded this exact release. An injected transport lets the library
// own verification, extraction, replacement and rollback without a second network fetch
// or silently switching to a different release after the user approved installation.
struct CachedDownload {
    url: String,
    file: fs::File,
}
struct CachedResponse {
    headers: HeaderMap,
    file: fs::File,
}
impl HttpClient for CachedDownload {
    fn get(
        &self,
        url: &str,
        _: &HeaderMap,
        _: Option<Duration>,
    ) -> self_update::Result<Box<dyn HttpResponse>> {
        if url != self.url {
            return Err(self_update::Error::verification_rejected(
                "Unapproved update URL",
            ));
        }
        let mut file = self.file.try_clone()?;
        std::io::Seek::rewind(&mut file)?;
        Ok(Box::new(CachedResponse {
            headers: HeaderMap::new(),
            file,
        }))
    }
}
impl HttpResponse for CachedResponse {
    fn headers(&self) -> &HeaderMap {
        &self.headers
    }
    fn body(self: Box<Self>) -> Box<dyn Read> {
        Box::new(self.file)
    }
}

pub(super) fn install(
    download: &DownloadedUpdate,
    bundle: Option<&Path>,
    executable: Option<&Path>,
    current_version: &str,
    verify: impl Fn(&Path) -> self_update::Result<()> + Send + Sync + 'static,
) -> Result<()> {
    download.verify()?;
    let target = super::target();
    let asset = ReleaseAsset::new(
        if bundle.is_some() {
            format!("apifuck-desktop-{target}.app.tar.gz")
        } else {
            format!("apifuck-desktop-{target}.AppImage")
        },
        &download.release.url,
    )
    .with_digest(format!("sha256:{}", download.release.sha256));
    let release = Release::builder()
        .version(&download.release.version)
        .asset(asset)
        .build()?;
    let mut updater = Update::configure();
    updater
        .source(ApprovedRelease(release))
        .http_client(Arc::new(CachedDownload {
            url: download.release.url.clone(),
            file: fs::File::open(&download.path)?,
        }))
        .bin_name("apifuck-desktop")
        .target(target)
        .current_version(current_version)
        .no_confirm(true) // The existing UI owns confirmation and draft saving.
        .show_output(false)
        .verify_binary(verify);
    if let Some(bundle) = bundle {
        updater
            .bundle_path_in_archive("ApiFuck.app")
            .bundle_install_path(bundle);
    } else if let Some(executable) = executable {
        updater.bin_install_path(executable);
    }
    let status = updater
        .build()?
        .update()
        .context("无法安装更新，请确认应用位于可写目录（macOS 请先移出磁盘映像）")?;
    ensure!(status.is_updated(), "The approved update was not installed");
    Ok(())
}
