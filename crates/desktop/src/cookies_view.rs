use crate::{
    i18n::tr,
    icons::AppIcon,
    surfaces::{confirm, empty_state},
};
use apifuck_core::cookies::{self, CookieRow};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon,
    button::{Button, ButtonVariants},
    notification::Notification,
};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

pub struct CookiesView {
    root: PathBuf,
    environment: Option<String>,
    rows: Vec<CookieRow>,
    busy: bool,
    reveal: bool,
}
impl CookiesView {
    pub fn new(
        root: PathBuf,
        environment: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self {
            root,
            environment,
            rows: vec![],
            busy: false,
            reveal: false,
        };
        view.update_store(None, false, window, cx);
        view
    }
    fn update_store(
        &mut self,
        remove: Option<CookieRow>,
        clear: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        let root = self.root.clone();
        let environment = self.environment.clone();
        let task = cx.background_spawn(async move {
            if clear {
                cookies::clear(&root, environment.as_deref())?;
            }
            if let Some(row) = remove {
                cookies::remove(
                    &root,
                    environment.as_deref(),
                    &row.domain,
                    &row.path,
                    &row.name,
                )?;
            }
            cookies::list(&root, environment.as_deref())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(rows) => this.rows = rows,
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
}
impl Render for CookiesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let muted = cx.theme().muted_foreground;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .min_h(rems(16.))
            .max_h(rems(32.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(AppIcon::Globe).size_4().text_color(muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(
                                self.environment
                                    .clone()
                                    .unwrap_or_else(|| tr("未选择环境").into()),
                            ),
                    )
                    .child(
                        Button::new("cookies-refresh")
                            .ghost()
                            .icon(AppIcon::RefreshCw)
                            .label(tr("刷新"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.update_store(None, false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("cookies-reveal")
                            .ghost()
                            .icon(if self.reveal {
                                AppIcon::EyeOff
                            } else {
                                AppIcon::Eye
                            })
                            .label(tr(if self.reveal {
                                "隐藏值"
                            } else {
                                "显示值"
                            }))
                            .disabled(self.rows.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reveal = !this.reveal;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("cookies-clear")
                            .ghost()
                            .icon(AppIcon::Trash)
                            .label(tr("清空 Cookie"))
                            .disabled(self.busy || self.rows.is_empty())
                            .on_click(cx.listener(move |_, _, window, cx| {
                                let weak = weak.clone();
                                confirm(
                                    "清空 Cookie",
                                    "将删除当前环境保存的全部 Cookie，此操作不可撤销。",
                                    "清空 Cookie",
                                    window,
                                    cx,
                                    move |window, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.update_store(None, true, window, cx)
                                        });
                                    },
                                );
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(tr("Cookie 按项目和环境隔离，包含会话 Cookie；关闭后仍保留")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .when(self.rows.is_empty(), |list| {
                        list.child(empty_state(tr("暂无 Cookie"), cx))
                    })
                    .when(!self.rows.is_empty(), |list| {
                        list.child(
                            div()
                                .id("cookies-list")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .children(self.rows.iter().enumerate().map(|(index, row)| {
                                    let cookie = row.clone();
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_3()
                                        .py_2()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .child(
                                                    div()
                                                        .truncate()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(row.name.clone()),
                                                )
                                                .child(div().truncate().text_color(muted).child(
                                                    if self.reveal {
                                                        row.value.clone()
                                                    } else {
                                                        "••••••••".into()
                                                    },
                                                ))
                                                .child(
                                                    div()
                                                        .truncate()
                                                        .text_xs()
                                                        .text_color(muted)
                                                        .child(format!(
                                                            "{}{}",
                                                            row.domain, row.path
                                                        )),
                                                ),
                                        )
                                        .when(row.secure, |item| {
                                            item.child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_2()
                                                    .rounded(px(4.))
                                                    .text_xs()
                                                    .bg(cx.theme().success.opacity(0.12))
                                                    .text_color(cx.theme().success)
                                                    .child("Secure"),
                                            )
                                        })
                                        .child(
                                            Button::new(("cookie-remove", index))
                                                .ghost()
                                                .icon(AppIcon::Trash)
                                                .tooltip(tr("删除 Cookie"))
                                                .accessibility_label(tr("删除 Cookie"))
                                                .disabled(self.busy)
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.update_store(
                                                            Some(cookie.clone()),
                                                            false,
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                )),
                                        )
                                })),
                        )
                    }),
            )
    }
}
