use crate::{
    appearance::http_status_color,
    i18n::tr,
    icons::AppIcon,
    surfaces::{confirm, empty_state, human_size, method_badge, panel_editor, panel_toolbar},
};
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
    Close,
}
pub struct HistoryView {
    root: PathBuf,
    entries: Vec<HistoryEntry>,
    selected: Option<usize>,
    details: Entity<EditorState>,
    busy: bool,
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
        };
        view.refresh(false, window, cx);
        view
    }
    fn refresh(&mut self, clear: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
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
        self.details.update(cx, |editor, cx| {
            editor.set_value(entry.body_preview.clone(), window, cx)
        });
        cx.notify();
    }
    fn status_label(entry: &HistoryEntry) -> String {
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
            })
    }
    fn summary(&self, entry: &HistoryEntry, cx: &App) -> Div {
        let color = http_status_color(entry.status, entry.cancelled, cx);
        let muted = cx.theme().muted_foreground;
        let facts = [
            (
                tr("环境"),
                entry
                    .environment
                    .clone()
                    .unwrap_or_else(|| tr("未选择环境").into()),
            ),
            (tr("时间"), history::timestamp(entry.timestamp_ms)),
            (tr("耗时"), format!("{} ms", entry.elapsed_ms)),
            (tr("大小"), human_size(entry.bytes)),
            (tr("请求"), entry.path.display().to_string()),
        ];
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(method_badge(&entry.method, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(entry.url.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color)
                            .child(Self::status_label(entry)),
                    ),
            )
            .children(
                entry
                    .error
                    .as_ref()
                    .map(|error| div().text_color(cx.theme().danger).child(error.clone())),
            )
            .children(facts.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .gap_3()
                    .child(div().w_16().flex_shrink_0().text_color(muted).child(label))
                    .child(div().flex_1().min_w_0().truncate().child(value))
            }))
            .when(entry.truncated, |card| {
                card.child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(tr("响应预览已截断（32 KiB）")),
                )
            })
    }
}
impl Render for HistoryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let selected = self.selected.and_then(|index| self.entries.get(index));
        let summary = selected.map(|entry| self.summary(entry, cx));
        let rows: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let color = http_status_color(entry.status, entry.cancelled, cx);
                div()
                    .id(("history-entry", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .hover(|row| row.bg(cx.theme().list_hover))
                    .when(self.selected == Some(index), |row| {
                        row.bg(cx.theme().list_active)
                    })
                    .child(method_badge(&entry.method, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(entry.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} · {} ms",
                                        history::timestamp(entry.timestamp_ms),
                                        entry.elapsed_ms
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color)
                            .child(Self::status_label(entry)),
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.select(index, window, cx)),
                    )
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                panel_toolbar(cx)
                    .h_12()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr("请求历史")),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(
                                        "本项目最近 200 次执行；响应预览保存在本机，URL 查询值已隐藏",
                                    )),
                            ),
                    )
                    .child(
                        Button::new("history-open")
                            .outline()
                            .icon(AppIcon::ArrowLeft)
                            .label(tr("打开原请求"))
                            .disabled(self.selected.is_none() || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(index) = this.selected {
                                    cx.emit(HistoryEvent::Open(this.entries[index].path.clone()));
                                }
                            })),
                    )
                    .child(
                        Button::new("history-refresh")
                            .ghost()
                            .icon(AppIcon::RefreshCw)
                            .tooltip(tr("刷新"))
                            .accessibility_label(tr("刷新"))
                            .disabled(self.busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.refresh(false, window, cx)),
                            ),
                    )
                    .child(
                        Button::new("history-clear")
                            .ghost()
                            .icon(AppIcon::Trash)
                            .tooltip(tr("清空历史"))
                            .accessibility_label(tr("清空历史"))
                            .disabled(self.busy || self.entries.is_empty())
                            .on_click(cx.listener(move |_, _, window, cx| {
                                let weak = weak.clone();
                                confirm(
                                    "清空历史",
                                    "将删除本项目全部请求历史，此操作不可撤销。",
                                    "清空历史",
                                    window,
                                    cx,
                                    move |window, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.refresh(true, window, cx)
                                        });
                                    },
                                );
                            })),
                    )
                    .child(
                        Button::new("history-close")
                            .ghost()
                            .icon(AppIcon::Close)
                            .tooltip(tr("关闭页面"))
                            .accessibility_label(tr("关闭页面"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(HistoryEvent::Close))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_96()
                            .flex_shrink_0()
                            .h_full()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .when(self.entries.is_empty(), |list| {
                                list.child(empty_state(tr("暂无请求历史"), cx))
                            })
                            .when(!self.entries.is_empty(), |list| {
                                list.child(
                                    div()
                                        .id("history-list")
                                        .flex_1()
                                        .min_h_0()
                                        .overflow_y_scroll()
                                        .children(rows),
                                )
                            }),
                    )
                    .child(match summary {
                        Some(summary) => div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(summary)
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(panel_editor(&self.details).readonly(true)),
                            ),
                        None => div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .child(empty_state(tr("选择左侧记录查看详情"), cx)),
                    }),
            )
    }
}
