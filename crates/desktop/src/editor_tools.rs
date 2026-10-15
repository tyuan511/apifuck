use crate::{
    i18n::tr,
    icons::AppIcon,
    language_server,
    languages::{self, Language},
};
use gpui_kit::component::{
    Sizable,
    button::{Button, ButtonVariants},
    input::EditorState,
    notification::Notification,
};
use gpui_kit::*;
use std::time::{Duration, Instant};

/// Keep this entity with its editor. Programmatic loads, undo and language changes
/// are observed as well as keystrokes; discarded drafts cannot receive old results.
pub struct LanguageBinding {
    editor: WeakEntity<EditorState>,
    document: language_server::Document,
    text: SharedString,
    language: SharedString,
    version: i32,
    pending: Option<Instant>,
    _observe: Subscription,
    _poll: Task<()>,
}
impl LanguageBinding {
    pub fn attach(editor: &Entity<EditorState>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let observe = cx.observe(editor, |this: &mut Self, editor, cx| {
                let state = editor.read(cx);
                let text = state.value();
                let language = state.language_name();
                if this.text != text || this.language != language {
                    this.text = text;
                    this.language = language;
                    this.version += 1;
                    this.pending = Some(Instant::now());
                    // A language switch with unchanged content also invalidates diagnostics.
                    editor.update(cx, |state, cx| {
                        if let Some(set) = state.diagnostics_mut() {
                            set.clear();
                        }
                        cx.notify();
                    });
                }
            });
            let poll = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                        break;
                    }
                }
            });
            Self {
                editor: editor.downgrade(),
                document: language_server::Document::new(),
                text: editor.read(cx).value(),
                language: editor.read(cx).language_name(),
                version: 1,
                pending: Some(Instant::now()),
                _observe: observe,
                _poll: poll,
            }
        })
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self
            .pending
            .is_some_and(|at| at.elapsed() >= Duration::from_millis(250))
        {
            self.document
                .change(&self.language, &self.text, self.version);
            self.pending = None;
        }
        for result in self.document.diagnostics.try_iter() {
            if result.version != Some(self.version) || self.pending.is_some() {
                continue;
            }
            let _ = self.editor.update(cx, |state, cx| {
                if state.value() != self.text || state.language_name() != self.language {
                    return;
                }
                if let Some(set) = state.diagnostics_mut() {
                    set.clear();
                    set.extend(result.diagnostics);
                }
                cx.notify();
            });
        }
    }
}

pub fn format_button(id: &'static str, editor: &Entity<EditorState>, language: Language) -> Button {
    let editor = editor.downgrade();
    Button::new(id)
        .icon(AppIcon::Braces)
        .ghost()
        .xsmall()
        .label(tr("格式化"))
        .on_click(move |_, window, cx| {
            let Some(editor) = editor.upgrade() else {
                return;
            };
            let before = editor.read(cx).value();
            let before_language = editor.read(cx).language_name();
            let input = before.to_string();
            let work = cx
                .background_executor()
                .spawn(async move { languages::format(language, &input) });
            let weak = editor.downgrade();
            window
                .spawn(cx, async move |cx| {
                    let result = work.await;
                    let _ = cx.update(|window, cx| {
                        let Some(editor) = weak.upgrade() else {
                            return;
                        };
                        if editor.read(cx).value() != before
                            || editor.read(cx).language_name() != before_language
                        {
                            crate::feedback::show(
                                Notification::info(tr("内容已变化，请重新格式化").to_owned()),
                                window,
                                cx,
                            );
                            return;
                        }
                        match result {
                            Ok(text) => {
                                if text != before.as_ref() {
                                    editor.update(cx, |state, cx| {
                                        state.replace_all(text, window, cx);
                                        state.focus(window, cx);
                                    });
                                }
                            }
                            Err(error) => crate::feedback::show(
                                Notification::error(format!(
                                    "{}: {error}",
                                    tr("格式化失败，原文已保留")
                                )),
                                window,
                                cx,
                            ),
                        }
                    });
                })
                .detach();
        })
}
