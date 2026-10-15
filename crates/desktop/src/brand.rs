//! Reuse the existing ApiFuck artwork, embedded so packaged builds need no source checkout.
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;
pub const LOGO: &str = "brand/apifuck.png";
gpui_kit::assets::icon_assets!(
    UiAssets,
    [
        Folder,
        FolderOpen,
        Layers,
        FolderPlus,
        FilePlus,
        FileInput,
        Save,
        Send,
        Square,
        Settings,
        Globe,
        Palette,
        Network,
        Info,
        Languages,
        Sun,
        Moon,
        Monitor,
        Code,
        Ellipsis,
        Copy,
        Trash,
        Pencil,
        ArrowLeft,
        RefreshCw,
        FileCode,
        KeyRound,
        ShieldCheck,
        FileText,
        File,
        Check,
        Search,
        Braces,
        Link,
        Plug,
        SlidersHorizontal,
        CircleCheck,
        CircleX,
        ChevronDown,
        ChevronRight,
        Ban
    ]
);
pub struct BrandAssets;
impl AssetSource for BrandAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == LOGO {
            Ok(Some(Cow::Borrowed(include_bytes!(
                "../../../assets/apifuck.png"
            ))))
        } else if let Some(icon) = UiAssets.load(path)? {
            Ok(Some(icon))
        } else {
            gpui_kit::assets::Assets.load(path)
        }
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(UiAssets.list(path)?);
        paths.sort();
        paths.dedup();
        if LOGO.starts_with(path) {
            paths.push(LOGO.into());
        }
        Ok(paths)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_ui_icons_are_embedded_without_external_files() {
        let paths = UiAssets.list("icons/").unwrap();
        assert!(!paths.is_empty());
        for path in paths {
            let bytes = BrandAssets.load(&path).unwrap().expect("missing UI icon");
            assert!(
                std::str::from_utf8(&bytes).unwrap().contains("<svg"),
                "{path}"
            );
        }
    }

    #[test]
    fn original_logo_is_embedded() {
        let bytes = BrandAssets.load(LOGO).unwrap().unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            bytes.as_ref(),
            include_bytes!("../../../assets/apifuck.png")
        );
    }

    #[test]
    fn folder_states_have_distinct_embedded_icons() {
        use gpui_kit::assets::IconName;
        let closed = BrandAssets.load(&IconName::Folder.path()).unwrap().unwrap();
        let open = BrandAssets
            .load(&IconName::FolderOpen.path())
            .unwrap()
            .unwrap();
        assert_ne!(closed, open);
        for svg in [closed, open] {
            assert!(std::str::from_utf8(&svg).unwrap().contains("<svg"));
        }
    }
}
