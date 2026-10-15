//! Product palette: literal colors are confined to this theme boundary.
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, Window, rgb};
pub fn apply(mode: &str, accent: &str, window: &mut Window, cx: &mut App) {
    match mode {
        "light" => Theme::change(ThemeMode::Light, Some(window), cx),
        "dark" => Theme::change(ThemeMode::Dark, Some(window), cx),
        _ => Theme::sync_system_appearance(Some(window), cx),
    }
    Theme::update(cx, |theme| {
        let dark = theme.is_dark();
        theme.highlight_theme = crate::syntax::highlight_theme(dark);
        // Neutral workbench surfaces; only actions and protocol markers carry color.
        theme.colors.background = rgb(if dark { 0x17191d } else { 0xffffff }).into();
        theme.colors.foreground = rgb(if dark { 0xe8eaed } else { 0x20242a }).into();
        theme.colors.sidebar = rgb(if dark { 0x1d2025 } else { 0xf8f9fb }).into();
        theme.colors.muted = rgb(if dark { 0x242830 } else { 0xf2f4f7 }).into();
        theme.colors.muted_foreground = rgb(if dark { 0x9199a6 } else { 0x737b87 }).into();
        theme.colors.border = rgb(if dark { 0x30353e } else { 0xe4e7ec }).into();
        theme.colors.tab_bar = theme.colors.muted;
        theme.colors.tab_active = theme.colors.background;
        theme.colors.tab_foreground = theme.colors.muted_foreground;
        theme.colors.tab_active_foreground = theme.colors.foreground;
        theme.colors.sidebar_foreground = theme.colors.foreground;
        theme.colors.sidebar_border = theme.colors.border;
        theme.colors.list_hover = rgb(if dark { 0x272c34 } else { 0xeff1f5 }).into();

        let primary = accent_color(accent, dark);
        let foreground = rgb(if dark { 0x0f172a } else { 0xffffff }).into();
        let mut hover = primary;
        hover.l *= 0.92;
        let mut active = primary;
        active.l *= 0.82;
        theme.colors.primary = primary;
        theme.colors.primary_foreground = foreground;
        theme.colors.primary_hover = hover;
        theme.colors.primary_active = active;
        theme.colors.button_primary = primary;
        theme.colors.button_primary_foreground = foreground;
        theme.colors.button_primary_hover = hover;
        theme.colors.button_primary_active = active;
        theme.colors.ring = primary;
        if accent.is_empty() || accent == "amber" {
            let brand = rgb(0xf4b41a).into();
            theme.colors.primary = rgb(if dark { 0xfbbf24 } else { 0xa16207 }).into();
            theme.colors.primary_foreground = rgb(0x1c1917).into();
            theme.colors.button_primary = brand;
            theme.colors.button_primary_foreground = rgb(0x1c1917).into();
            theme.colors.button_primary_hover = rgb(0xe8a30d).into();
            theme.colors.button_primary_active = rgb(0xd99108).into();
            theme.colors.ring = theme.colors.primary;
        }
        theme.colors.list_active = theme.colors.primary.opacity(if dark { 0.16 } else { 0.08 });
        theme.colors.list_active_border = theme.colors.primary.opacity(0.);
    });
}

pub fn method_color(method: &str, cx: &App) -> Hsla {
    use gpui_kit::component::ActiveTheme;
    let dark = cx.theme().is_dark();
    let color = match (method, dark) {
        ("GET", false) => 0x087f5b,
        ("GET", true) => 0x6ee7b7,
        ("POST", false) => 0x2563eb,
        ("POST", true) => 0x93c5fd,
        ("PUT" | "PATCH", false) => 0xa16207,
        ("PUT" | "PATCH", true) => 0xfcd34d,
        ("DELETE", false) => 0xc2410c,
        ("DELETE", true) => 0xfca5a5,
        ("GQL" | "WS", false) => 0x7c3aed,
        ("GQL" | "WS", true) => 0xc4b5fd,
        _ => return cx.theme().muted_foreground,
    };
    rgb(color).into()
}

/// Shared by theme actions and accent swatches.
pub fn accent_color(accent: &str, dark: bool) -> Hsla {
    rgb(match (accent, dark) {
        ("blue", false) => 0x2563eb,
        ("blue", true) => 0x60a5fa,
        ("green", false) => 0x15803d,
        ("green", true) => 0x4ade80,
        ("amber", false) => 0xb45309,
        ("amber", true) => 0xfbbf24,
        ("rose", false) => 0xbe123c,
        ("rose", true) => 0xfb7185,
        ("violet", false) => 0x6d28d9,
        ("violet", true) => 0xa78bfa,
        (_, false) => 0x334155,
        (_, true) => 0xcbd5e1,
    })
    .into()
}
