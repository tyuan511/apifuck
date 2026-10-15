use crate::{i18n::tr, icons::AppIcon, surfaces::panel_editor};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::EditorState,
    tab::{Tab, TabBar},
};
use gpui_kit::*;

pub struct ExportView {
    codes: Vec<(&'static str, Result<String, String>)>,
    selected: usize,
    editor: Entity<EditorState>,
}
impl ExportView {
    pub fn new(
        codes: Vec<(&'static str, Result<String, String>)>,
        language: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = codes
            .iter()
            .position(|(name, _)| *name == language)
            .unwrap_or(0);
        let editor = cx.new(|cx| EditorState::new(window, cx).line_number(true));
        let mut view = Self {
            codes,
            selected,
            editor,
        };
        view.select(selected, window, cx);
        view
    }
    fn select(&mut self, selected: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = selected;
        let (language, result) = &self.codes[selected];
        let text = result.as_ref().unwrap_or_else(|error| error).clone();
        self.editor.update(cx, |editor, cx| {
            editor.set_highlighter(
                if *language == "curl" {
                    "bash"
                } else {
                    language
                },
                cx,
            );
            editor.set_value(text, window, cx);
        });
        cx.notify();
    }
}
impl Render for ExportView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h(px(480.))
            .gap_2()
            .child(
                TabBar::new("code-language")
                    .selected_index(self.selected)
                    .children(
                        self.codes
                            .iter()
                            .map(|(language, _)| Tab::new().label(*language)),
                    )
                    .on_click(cx.listener(|this, ix, window, cx| this.select(*ix, window, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(panel_editor(&self.editor).readonly(true)),
            )
            .child(
                Button::new("copy-code")
                    .icon(AppIcon::Copy)
                    .primary()
                    .small()
                    .label(tr("复制代码"))
                    .disabled(self.codes[self.selected].1.is_err())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Ok(code) = &this.codes[this.selected].1 {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                        }
                    })),
            )
    }
}
