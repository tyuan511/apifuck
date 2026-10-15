//! Shared workbench surfaces. Panels own separators; embedded editors do not.
use gpui_kit::component::{
    ActiveTheme,
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
