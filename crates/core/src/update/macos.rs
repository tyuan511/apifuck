//! Adapt our verified, user-approved download to self_update's whole-bundle installer.
use super::DownloadedUpdate;
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn command_output(command: &str, args: &[&std::ffi::OsStr]) -> Result<std::process::Output> {
    let output = Command::new(command).args(args).output()?;
    ensure!(
        output.status.success(),
        "{command}: {}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    Ok(output)
}

fn signing_requirement(bundle: &Path) -> Result<String> {
    command_output(
        "/usr/bin/codesign",
        &[
            "--verify".as_ref(),
            "--deep".as_ref(),
            "--strict".as_ref(),
            bundle.as_os_str(),
        ],
    )?;
    let output = command_output(
        "/usr/bin/codesign",
        &[
            "-d".as_ref(),
            "-r-".as_ref(),
            "--verbose=4".as_ref(),
            bundle.as_os_str(),
        ],
    )?;
    let metadata = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        metadata.lines().any(|line| line
            .strip_prefix("TeamIdentifier=")
            .is_some_and(|team| team != "not set" && !team.is_empty())),
        "The installed app has no Developer ID team"
    );
    metadata
        .lines()
        .find_map(|line| line.strip_prefix("designated => ").map(str::to_owned))
        .context("The installed app has no designated signing requirement")
}

fn verify_bundle(bundle: &Path, requirement: &str, version: &str) -> Result<()> {
    ensure!(
        bundle.is_dir() && !bundle.is_symlink(),
        "Update has no application bundle"
    );
    let executable = bundle.join("Contents/MacOS/apifuck-desktop");
    ensure!(
        executable.is_file() && !executable.is_symlink(),
        "Update has no application executable"
    );
    command_output(
        "/usr/bin/codesign",
        &[
            "--verify".as_ref(),
            "--deep".as_ref(),
            "--strict".as_ref(),
            "-R".as_ref(),
            // -R consumes the next argument; inline requirements must start with '='.
            format!("={requirement}").as_ref(),
            "--verbose=4".as_ref(),
            bundle.as_os_str(),
        ],
    )?;
    let info = bundle.join("Contents/Info.plist");
    for (key, expected) in [
        ("CFBundleIdentifier", "dev.fastvibe.apifuck"),
        ("CFBundleExecutable", "apifuck-desktop"),
        ("ApiFuckVersion", version),
    ] {
        let output = command_output(
            "/usr/bin/plutil",
            &[
                "-extract".as_ref(),
                key.as_ref(),
                "raw".as_ref(),
                "-o".as_ref(),
                "-".as_ref(),
                info.as_os_str(),
            ],
        )?;
        ensure!(
            String::from_utf8(output.stdout)?.trim() == expected,
            "Update bundle {key} does not match the approved release"
        );
    }
    Ok(())
}

pub(super) fn install(download: &DownloadedUpdate, bundle: &Path) -> Result<()> {
    let requirement = signing_requirement(bundle)?;
    let version = download.release.version.clone();
    let prepared = prepare_dmg(download, &requirement)?;
    install_with_verifier(
        &prepared,
        bundle,
        env!("CARGO_PKG_VERSION"),
        move |staged| {
            verify_bundle(staged, &requirement, &version)
                .map_err(|error| self_update::Error::verification_rejected(error.to_string()))
        },
    )
}

struct MountedDmg {
    _directory: tempfile::TempDir,
    mount: PathBuf,
    attached: bool,
}
impl MountedDmg {
    fn open(image: &Path) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let mount = directory.path().join("volume");
        fs::create_dir(&mount)?;
        let mut mounted = Self {
            _directory: directory,
            mount,
            attached: true,
        };
        let result = command_output(
            "/usr/bin/hdiutil",
            &[
                "attach".as_ref(),
                "-readonly".as_ref(),
                "-nobrowse".as_ref(),
                "-noautoopen".as_ref(),
                "-mountpoint".as_ref(),
                mounted.mount.as_os_str(),
                image.as_os_str(),
            ],
        );
        if result.is_err() {
            let _ = mounted.detach();
        }
        result?;
        Ok(mounted)
    }
    fn detach(&mut self) -> Result<()> {
        command_output(
            "/usr/bin/hdiutil",
            &["detach".as_ref(), self.mount.as_os_str()],
        )?;
        self.attached = false;
        Ok(())
    }
}
impl Drop for MountedDmg {
    fn drop(&mut self) {
        if self.attached {
            let _ = Command::new("/usr/bin/hdiutil")
                .arg("detach")
                .arg("-force")
                .arg(&self.mount)
                .output();
        }
    }
}

fn prepare_dmg(download: &DownloadedUpdate, requirement: &str) -> Result<DownloadedUpdate> {
    use sha2::{Digest, Sha256};
    download.verify()?;
    let mut mounted = MountedDmg::open(&download.path)?;
    let app = mounted.mount.join("ApiFuck.app");
    verify_bundle(&app, requirement, &download.release.version)?;
    // self_update installs bundle archives. Convert only the verified app from the
    // read-only DMG into a private local archive; no extra release asset or download.
    let archive = tempfile::NamedTempFile::new()?;
    let output = Command::new("/usr/bin/tar")
        .env("COPYFILE_DISABLE", "1")
        .args(["--no-xattrs", "-czf"])
        .arg(archive.path())
        .arg("-C")
        .arg(&mounted.mount)
        .arg("ApiFuck.app")
        .output()?;
    ensure!(
        output.status.success(),
        "Unable to prepare the verified app: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    mounted.detach()?;
    let mut release = download.release.clone();
    release.size = archive.as_file().metadata()?.len();
    let mut digest = Sha256::new();
    std::io::copy(&mut fs::File::open(archive.path())?, &mut digest)?;
    release.sha256 = format!("{:x}", digest.finalize());
    Ok(DownloadedUpdate {
        path: archive.into_temp_path(),
        release,
    })
}

fn install_with_verifier(
    download: &DownloadedUpdate,
    bundle: &Path,
    current_version: &str,
    verify: impl Fn(&Path) -> self_update::Result<()> + Send + Sync + 'static,
) -> Result<()> {
    super::installer::install(download, Some(bundle), None, current_version, verify)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn fixture(root: &Path, archive_name: &str) -> (DownloadedUpdate, std::path::PathBuf) {
        let installed = root.join("Installed.app");
        fs::create_dir_all(installed.join("Contents/MacOS")).unwrap();
        fs::write(
            installed.join("Contents/MacOS/apifuck-desktop"),
            "old executable",
        )
        .unwrap();
        fs::write(installed.join("obsolete-resource"), "old resource").unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join(archive_name).join("Contents/MacOS")).unwrap();
        fs::write(
            source
                .join(archive_name)
                .join("Contents/MacOS/apifuck-desktop"),
            "new executable",
        )
        .unwrap();
        fs::write(
            source.join(archive_name).join("new-resource"),
            "new resource",
        )
        .unwrap();
        let archive = root.join("update.tar.gz");
        assert!(
            Command::new("/usr/bin/tar")
                .args(["-czf"])
                .arg(&archive)
                .arg("-C")
                .arg(&source)
                .arg(archive_name)
                .status()
                .unwrap()
                .success()
        );
        let bytes = fs::read(archive).unwrap();
        let file = tempfile::NamedTempFile::new_in(root).unwrap();
        fs::write(file.path(), &bytes).unwrap();
        (DownloadedUpdate {
            path: file.into_temp_path(),
            release: super::super::Release {
                version: "99.0.0".into(), notes: String::new(),
                url: "https://github.com/tyuan511/apifuck/releases/download/v99.0.0/apifuck-desktop-aarch64-apple-darwin.app.tar.gz".into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)), size: bytes.len() as u64, bundle: true,
            },
        }, installed)
    }

    #[test]
    fn library_installs_complete_bundle_from_cached_download_without_network() {
        let root = tempfile::tempdir().unwrap();
        let (download, app) = fixture(root.path(), "ApiFuck.app");
        install_with_verifier(&download, &app, "0.1.0", |staged| {
            assert!(staged.join("new-resource").is_file());
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(app.join("Contents/MacOS/apifuck-desktop")).unwrap(),
            "new executable"
        );
        assert!(!app.join("obsolete-resource").exists());
        assert!(app.join("new-resource").is_file());
    }

    #[test]
    fn rejected_or_incomplete_updates_leave_original_bundle_untouched() {
        for failure in ["checksum", "signature", "missing-bundle", "same-version"] {
            let root = tempfile::tempdir().unwrap();
            let (download, app) = fixture(
                root.path(),
                if failure == "missing-bundle" {
                    "Other.app"
                } else {
                    "ApiFuck.app"
                },
            );
            if failure == "checksum" {
                fs::write(&download.path, "tampered").unwrap();
            }
            let result = install_with_verifier(
                &download,
                &app,
                if failure == "same-version" {
                    "99.0.0"
                } else {
                    "0.1.0"
                },
                move |_| {
                    if failure == "signature" {
                        Err(self_update::Error::verification_rejected("wrong signer"))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(result.is_err(), "{failure}");
            assert_eq!(
                fs::read_to_string(app.join("Contents/MacOS/apifuck-desktop")).unwrap(),
                "old executable",
                "{failure}"
            );
            assert!(app.join("obsolete-resource").exists(), "{failure}");
            assert!(!app.join("new-resource").exists(), "{failure}");
        }
    }

    #[test]
    fn bundle_verification_rejects_wrong_identity_version_and_tampered_resources() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("ApiFuck.app");
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        fs::copy("/usr/bin/true", app.join("Contents/MacOS/apifuck-desktop")).unwrap();
        fs::write(app.join("Contents/Resources/test.txt"), "signed resource").unwrap();
        fs::write(app.join("Contents/Info.plist"), r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.fastvibe.apifuck</string><key>CFBundleExecutable</key><string>apifuck-desktop</string>
<key>CFBundlePackageType</key><string>APPL</string><key>ApiFuckVersion</key><string>99.0.0</string></dict></plist>"#).unwrap();
        assert!(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app)
                .status()
                .unwrap()
                .success()
        );
        let requirement = "identifier \"dev.fastvibe.apifuck\"";
        verify_bundle(&app, requirement, "99.0.0").unwrap();
        assert!(verify_bundle(&app, "identifier \"different.app\"", "99.0.0").is_err());
        assert!(verify_bundle(&app, requirement, "98.0.0").is_err());
        assert!(signing_requirement(&app).is_err()); // Ad-hoc identity is never trusted for production.
        // Exercise the actual DMG -> verified app -> library install path, without GUI launch.
        let image_source = root.path().join("image-source");
        fs::create_dir(&image_source).unwrap();
        assert!(
            Command::new("/usr/bin/ditto")
                .arg(&app)
                .arg(image_source.join("ApiFuck.app"))
                .status()
                .unwrap()
                .success()
        );
        let archive = tempfile::Builder::new()
            .suffix(".dmg")
            .tempfile_in(root.path())
            .unwrap();
        assert!(
            Command::new("/usr/bin/hdiutil")
                .args(["create", "-ov", "-srcfolder"])
                .arg(&image_source)
                .args(["-format", "UDZO", "-fs", "HFS+"])
                .arg(archive.path())
                .output()
                .unwrap()
                .status
                .success()
        );
        let bytes = fs::read(archive.path()).unwrap();
        let download = DownloadedUpdate {
            path: archive.into_temp_path(),
            release: super::super::Release {
                version: "99.0.0".into(),
                notes: String::new(),
                url: "https://github.com/tyuan511/apifuck/releases/download/v99.0.0/app.dmg".into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                size: bytes.len() as u64,
                bundle: true,
            },
        };
        assert!(prepare_dmg(&download, "identifier \"wrong.signer\"").is_err());
        let prepared = prepare_dmg(&download, requirement).unwrap();
        let installed = root.path().join("Installed.app");
        fs::create_dir(&installed).unwrap();
        fs::write(installed.join("obsolete-resource"), "old").unwrap();
        install_with_verifier(&prepared, &installed, "0.1.0", move |staged| {
            verify_bundle(staged, requirement, "99.0.0")
                .map_err(|error| self_update::Error::verification_rejected(error.to_string()))
        })
        .unwrap();
        verify_bundle(&installed, requirement, "99.0.0").unwrap();
        assert!(!installed.join("obsolete-resource").exists());
        fs::write(app.join("Contents/Resources/test.txt"), "tampered resource").unwrap();
        assert!(verify_bundle(&app, requirement, "99.0.0").is_err());
    }
}
