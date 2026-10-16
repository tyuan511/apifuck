use crate::{i18n::tr, icons::AppIcon};
use apifuck_core::cookies::{self, CookieRow};
use gpui_kit::component::{
    ActiveTheme, Disableable,
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
    confirm_clear: bool,
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
            confirm_clear: false,
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
        self.confirm_clear = false;
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
        div()
            .flex()
            .flex_col()
            .gap_3()
            .h_96()
            .child(format!(
                "{}: {}",
                tr("环境"),
                self.environment.as_deref().unwrap_or(tr("未选择环境"))
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("Cookie 按项目和环境隔离，包含会话 Cookie；关闭后仍保留")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("cookies-refresh")
                            .ghost()
                            .label(tr("刷新"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.update_store(None, false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("cookies-reveal")
                            .ghost()
                            .label(tr(if self.reveal {
                                "隐藏值"
                            } else {
                                "显示值"
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reveal = !this.reveal;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("cookies-clear")
                            .label(tr(if self.confirm_clear {
                                "确认清空 Cookie"
                            } else {
                                "清空 Cookie"
                            }))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.confirm_clear {
                                    this.update_store(None, true, window, cx);
                                } else {
                                    this.confirm_clear = true;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .id("cookies-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.rows.is_empty(), |list| list.child(tr("暂无 Cookie")))
                    .children(self.rows.iter().enumerate().map(|(index, row)| {
                        let cookie = row.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().truncate().child(format!(
                                        "{} = {}",
                                        row.name,
                                        if self.reveal {
                                            row.value.as_str()
                                        } else {
                                            "••••"
                                        }
                                    )))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!(
                                                "{}{} {}",
                                                row.domain,
                                                row.path,
                                                if row.secure { "Secure" } else { "" }
                                            )),
                                    ),
                            )
                            .child(
                                Button::new(("cookie-remove", index))
                                    .ghost()
                                    .icon(AppIcon::Trash)
                                    .tooltip(tr("删除 Cookie"))
                                    .accessibility_label(tr("删除 Cookie"))
                                    .disabled(self.busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.update_store(Some(cookie.clone()), false, window, cx)
                                    })),
                            )
                    })),
            )
    }
}
