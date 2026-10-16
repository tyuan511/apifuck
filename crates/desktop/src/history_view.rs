use crate::{i18n::tr, icons::AppIcon, surfaces::panel_editor};
use apifuck_core::history::{self, HistoryEntry};
use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    input::EditorState,
    notification::Notification,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

pub enum HistoryEvent {
    Open(PathBuf),
}
pub struct HistoryView {
    root: PathBuf,
    entries: Vec<HistoryEntry>,
    selected: Option<usize>,
    details: Entity<EditorState>,
    busy: bool,
    confirm_clear: bool,
}
impl EventEmitter<HistoryEvent> for HistoryView {}
impl HistoryView {
    pub fn new(root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            root,
            entries: vec![],
            selected: None,
            details: cx.new(|cx| EditorState::new(window, cx).line_number(false)),
            busy: false,
            confirm_clear: false,
        };
        view.refresh(false, window, cx);
        view
    }
    fn refresh(&mut self, clear: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.confirm_clear = false;
        let root = self.root.clone();
        let task = cx.background_spawn(async move {
            if clear {
                history::clear(&root)?;
            }
            history::load(&root)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(entries) => {
                        this.entries = entries;
                        this.selected = None;
                        this.details
                            .update(cx, |editor, cx| editor.set_value("", window, cx));
                    }
                    Err(error) => {
                        crate::feedback::show(Notification::error(format!("{error:#}")), window, cx)
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(index);
        let entry = &self.entries[index];
        let text = format!(
            "{}\n{}\n{} {}\n{}: {}\n{}: {}\n{} · {} ms · {} bytes\n{}\n\n{}{}",
            entry.name,
            entry.path.display(),
            entry.method,
            entry.url,
            tr("环境"),
            entry.environment.as_deref().unwrap_or(tr("未选择环境")),
            tr("时间"),
            history::timestamp(entry.timestamp_ms),
            entry
                .status
                .map(|status| format!("HTTP {status}"))
                .unwrap_or_else(|| tr(if entry.cancelled {
                    "已取消"
                } else if entry.error.is_some() {
                    "失败"
                } else {
                    "已完成"
                })
                .into()),
            entry.elapsed_ms,
            entry.bytes,
            entry.error.as_deref().unwrap_or(""),
            entry.body_preview,
            if entry.truncated {
                tr("响应预览已截断（32 KiB）")
            } else {
                ""
            }
        );
        self.details
            .update(cx, |editor, cx| editor.set_value(text, window, cx));
        cx.notify();
    }
}
impl Render for HistoryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_lg().flex_1().child(tr("请求历史")))
                    .child(
                        Button::new("history-refresh")
                            .ghost()
                            .icon(AppIcon::RefreshCw)
                            .label(tr("刷新"))
                            .disabled(self.busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.refresh(false, window, cx)),
                            ),
                    )
                    .child(
                        Button::new("history-open")
                            .label(tr("打开原请求"))
                            .disabled(self.selected.is_none() || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(index) = this.selected {
                                    cx.emit(HistoryEvent::Open(this.entries[index].path.clone()));
                                }
                            })),
                    )
                    .child(
                        Button::new("history-clear")
                            .label(tr(if self.confirm_clear {
                                "确认清空历史"
                            } else {
                                "清空历史"
                            }))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.confirm_clear {
                                    this.refresh(true, window, cx);
                                } else {
                                    this.confirm_clear = true;
                                    cx.notify();
                                }
                            })),
                    )
                    .when(self.confirm_clear, |row| {
                        row.child(
                            Button::new("history-keep")
                                .ghost()
                                .label(tr("取消"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirm_clear = false;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(div().text_color(cx.theme().muted_foreground).child(tr(
                "本项目最近 200 次执行；响应预览保存在本机，URL 查询值已隐藏",
            )))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        div()
                            .id("history-list")
                            .w_80()
                            .h_full()
                            .overflow_y_scroll()
                            .when(self.entries.is_empty(), |list| {
                                list.child(tr("暂无请求历史"))
                            })
                            .children(self.entries.iter().enumerate().map(|(index, entry)| {
                                div()
                                    .id(("history-entry", index))
                                    .p_2()
                                    .cursor_pointer()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .when(self.selected == Some(index), |row| {
                                        row.bg(cx.theme().secondary)
                                    })
                                    .child(format!("{} · {}", entry.method, entry.name))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{} · {} ms",
                                                history::timestamp(entry.timestamp_ms),
                                                entry.elapsed_ms
                                            )),
                                    )
                                    .child(
                                        entry
                                            .status
                                            .map(|status| format!("HTTP {status}"))
                                            .unwrap_or_else(|| {
                                                tr(if entry.cancelled {
                                                    "已取消"
                                                } else if entry.error.is_some() {
                                                    "失败"
                                                } else {
                                                    "已完成"
                                                })
                                                .into()
                                            }),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(index, window, cx)
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(panel_editor(&self.details).readonly(true)),
                    ),
            )
    }
}
