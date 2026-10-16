//! Shared workbench surfaces. Panels own separators; embedded editors do not.
use crate::i18n::tr;
use gpui_kit::component::{
    ActiveTheme, WindowExt,
    dialog::DialogButtonProps,
    input::{Editor, EditorState},
};
use gpui_kit::*;

pub fn panel_editor(state: &Entity<EditorState>) -> Editor {
    Editor::new(state)
        .bordered(false)
        .rounded_none()
        .size_full()
}

pub fn panel_toolbar(cx: &App) -> Div {
    div()
        .flex()
        .items_center()
        .justify_start()
        .flex_shrink_0()
        .min_w_0()
        .h_9()
        .px_4()
        .gap_2()
        .bg(cx.theme().background)
        .border_b_1()
        .border_color(cx.theme().border)
        .text_xs()
}

/// Colored method marker shared by request lists; color comes from `appearance`.
pub fn method_badge(method: &str, cx: &App) -> Div {
    let color = crate::appearance::method_color(method, cx);
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(rems(3.25))
        .h(px(20.))
        .px(px(3.))
        .flex_shrink_0()
        .rounded(px(4.))
        .bg(color.opacity(if cx.theme().is_dark() { 0.14 } else { 0.09 }))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(div().truncate().child(method.to_owned()))
}

/// Small muted caption above a control, matching form field labels.
pub fn field_caption(text: &str, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}

/// Centered, muted placeholder for empty lists and detail panes.
pub fn empty_state(text: &str, cx: &App) -> Div {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .p_4()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_owned())
}

/// Destructive actions share one confirmation dialog instead of inline two-step buttons.
pub fn confirm(
    title: &'static str,
    message: &'static str,
    ok: &'static str,
    window: &mut Window,
    cx: &mut App,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    let on_ok = std::rc::Rc::new(on_ok);
    window.open_dialog(cx, move |dialog, _, _| {
        let on_ok = on_ok.clone();
        dialog
            .title(tr(title))
            .child(tr(message))
            .button_props(
                DialogButtonProps::default()
                    .ok_text(tr(ok))
                    .cancel_text(tr("取消"))
                    .show_cancel(true),
            )
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

pub fn human_size(bytes: usize) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KiB", bytes as f64 / 1024.),
        _ => format!("{:.1} MiB", bytes as f64 / 1_048_576.),
    }
}
