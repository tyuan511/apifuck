use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsStr,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) fn verify_image(path: &Path) -> Result<()> {
    let mut header = [0_u8; 64];
    fs::File::open(path)?.read_exact(&mut header)?;
    ensure!(
        &header[..4] == b"\x7fELF" && &header[8..11] == b"AI\x02",
        "Update is not a type-2 AppImage"
    );
    ensure!(
        header[4] == 2 && header[5] == 1 && header[18..20] == [62, 0],
        "AppImage is not Linux x86_64"
    );
    Ok(())
}

pub(super) fn prepare_image(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    verify_image(path)?;
    // The library's streamed raw-file staging starts non-executable.
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn current_appimage() -> Result<Option<PathBuf>> {
    resolve_appimage(
        &std::env::current_exe()?,
        std::env::var_os("APPIMAGE").as_deref(),
        std::env::var_os("APPDIR").as_deref(),
    )
}

fn resolve_appimage(
    executable: &Path,
    image: Option<&OsStr>,
    appdir: Option<&OsStr>,
) -> Result<Option<PathBuf>> {
    let Some(image) = image else {
        return Ok(None);
    };
    let image = Path::new(image);
    let appdir = Path::new(appdir.context("APPIMAGE is set without APPDIR")?);
    ensure!(
        image.is_absolute() && appdir.is_absolute(),
        "AppImage runtime paths must be absolute"
    );
    let image = image.canonicalize()?;
    let appdir = appdir.canonicalize()?;
    ensure!(
        executable.canonicalize()?.starts_with(&appdir) && !image.starts_with(&appdir),
        "Invalid AppImage runtime paths"
    );
    verify_image(&image)?;
    Ok(Some(image))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_replaces_the_appimage_file_without_touching_the_mounted_tree() {
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("ApiFuck.AppImage");
        fs::write(&target, "old image").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        let mounted = root.path().join("mounted-executable");
        fs::write(&mounted, "unchanged").unwrap();
        let mut bytes = [0_u8; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[8..11].copy_from_slice(b"AI\x02");
        bytes[18] = 62;
        let file = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        fs::write(file.path(), bytes).unwrap();
        fs::set_permissions(file.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let download = super::super::DownloadedUpdate {
            path: file.into_temp_path(),
            release: super::super::Release {
                version: "99.0.0".into(),
                notes: String::new(),
                url: "https://github.com/tyuan511/apifuck/releases/download/v99.0.0/app.AppImage"
                    .into(),
                sha256: format!("{:x}", Sha256::digest(bytes)),
                size: 64,
                bundle: false,
            },
        };
        super::super::installer::install(&download, None, Some(&target), "0.1.0", |file| {
            prepare_image(file)
                .map_err(|error| self_update::Error::verification_rejected(error.to_string()))
        })
        .unwrap();
        verify_image(&target).unwrap();
        assert!(fs::metadata(&target).unwrap().permissions().mode() & 0o111 != 0);
        assert_eq!(fs::read_to_string(mounted).unwrap(), "unchanged");
    }
    #[test]
    fn appimage_updates_target_the_outer_image_not_the_mounted_executable() {
        let temp = tempfile::tempdir().unwrap();
        let mount = temp.path().join("mount");
        fs::create_dir(&mount).unwrap();
        let exe = mount.join("apifuck-desktop");
        fs::write(&exe, "mounted executable").unwrap();
        let image = temp.path().join("ApiFuck.AppImage");
        let mut bytes = [0_u8; 64];
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[8..11].copy_from_slice(b"AI\x02");
        bytes[18] = 62;
        fs::write(&image, bytes).unwrap();
        assert_eq!(
            resolve_appimage(&exe, Some(image.as_os_str()), Some(mount.as_os_str())).unwrap(),
            Some(image.canonicalize().unwrap())
        );
        assert!(resolve_appimage(&exe, None, None).unwrap().is_none());
        assert!(resolve_appimage(&exe, Some(image.as_os_str()), None).is_err());
        assert!(
            resolve_appimage(&image, Some(image.as_os_str()), Some(mount.as_os_str())).is_err()
        );
        fs::write(&image, "not an AppImage").unwrap();
        assert!(verify_image(&image).is_err());
    }
}
