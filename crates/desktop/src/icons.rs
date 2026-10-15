//! One Lucide vocabulary for actions, menus, and editors.
pub use gpui_kit::assets::IconName as AppIcon;

pub fn document_kind(kind: &str) -> AppIcon {
    match kind {
        "folder" => AppIcon::FolderPlus,
        "environment" => AppIcon::Globe,
        "graphql" => AppIcon::Braces,
        "websocket" => AppIcon::Plug,
        _ => AppIcon::FilePlus,
    }
}

pub fn value_kind(kind: &str) -> AppIcon {
    match kind {
        "inherit" => AppIcon::Link,
        "none" | "false" => AppIcon::Ban,
        "basic" | "bearer" | "apikey" => AppIcon::KeyRound,
        "json" | "xml" => AppIcon::FileCode,
        "file" => AppIcon::File,
        "text" => AppIcon::FileText,
        "true" => AppIcon::Check,
        _ => AppIcon::SlidersHorizontal,
    }
}
