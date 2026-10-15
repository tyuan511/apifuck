use crate::i18n::tr;
use crate::icons::AppIcon;
use crate::network_editor::{NetworkEditor, NetworkSection};
use apifuck_core::settings::{AppPreferences, RequestLayout};
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    notification::Notification,
    switch::Switch,
    text::TextView,
};
use gpui_kit::{prelude::FluentBuilder, *};

fn request_layout_preview(layout: RequestLayout, cx: &App) -> Div {
    let line =
        |width: f32, color: Hsla| div().w(relative(width)).h(px(3.)).rounded_full().bg(color);
    let panel = |label, request: bool| {
        let color = if request {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .p(px(6.))
            .gap(px(4.))
            .rounded_sm()
            .bg(color.opacity(0.06))
            .child(
                div()
                    .text_size(px(10.))
                    .line_height(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(line(0.8, color.opacity(0.24)))
            .child(line(0.55, color.opacity(0.16)))
            .child(line(0.68, color.opacity(0.16)))
    };
    div()
        .flex()
        .flex_col()
        .w_full()
        .h(px(128.))
        .flex_shrink_0()
        .p(px(6.))
        .gap(px(6.))
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .h(px(12.))
                .flex_shrink_0()
                .child(
                    div()
                        .w(px(20.))
                        .h(px(6.))
                        .rounded_sm()
                        .bg(cx.theme().primary.opacity(0.3)),
                )
                .child(div().flex_1().h(px(6.)).rounded_sm().bg(cx.theme().muted))
                .child(
                    div()
                        .w(px(16.))
                        .h(px(8.))
                        .rounded_sm()
                        .bg(cx.theme().primary.opacity(0.5)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .gap(px(4.))
                .when(layout == RequestLayout::Vertical, |view| view.flex_col())
                .child(panel(tr("请求"), true))
                .child(panel(tr("响应"), false)),
        )
}

#[derive(Clone, Copy, PartialEq)]
enum Category {
    Appearance,
    Network,
    Proxy,
    About,
}
impl Category {
    fn title(self) -> &'static str {
        match self {
            Self::Appearance => tr("外观与语言"),
            Self::Network => tr("网络请求"),
            Self::Proxy => tr("代理"),
            Self::About => tr("关于与更新"),
        }
    }
    fn icon(self) -> AppIcon {
        match self {
            Self::Appearance => AppIcon::Palette,
            Self::Network => AppIcon::Network,
            Self::Proxy => AppIcon::Globe,
            Self::About => AppIcon::Info,
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Appearance => tr("调整主题与强调色，让工作台符合你的使用习惯。"),
            Self::Network => tr("设置所有项目共用的请求超时、重定向和安全选项。"),
            Self::Proxy => tr("配置应用发送请求时使用的代理服务器。"),
            Self::About => tr("查看应用版本并检查可用更新。"),
        }
    }
}
pub enum SettingsEvent {
    Save(AppPreferences),
    CheckUpdates,
}
pub struct SettingsView {
    category: Category,
    language: String,
    appearance: String,
    accent: String,
    request_layout: RequestLayout,
    network: Entity<NetworkEditor>,
    notes: String,
    auto_check_updates: bool,
    _network_subscription: Subscription,
}
impl EventEmitter<SettingsEvent> for SettingsView {}
impl SettingsView {
    pub fn new(preferences: AppPreferences, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let network = cx.new(|cx| NetworkEditor::new(preferences.network, window, cx));
        let subscription = cx.observe(&network, |_, _, cx| cx.notify());
        let mut this = Self {
            _network_subscription: subscription,
            category: Category::Appearance,
            language: if preferences.language == "en" {
                "en".into()
            } else {
                "zh-CN".into()
            },
            appearance: if preferences.appearance.is_empty() {
                "system".into()
            } else {
                preferences.appearance
            },
            accent: if preferences.accent.is_empty() {
                "amber".into()
            } else {
                preferences.accent
            },
            network,
            request_layout: preferences.request_layout,
            notes: tr("正在加载版本介绍…").into(),
            auto_check_updates: preferences.auto_check_updates,
        };
        match AppPreferences::draft_path()
            .and_then(|path| apifuck_core::settings::load_raw_draft(&path))
        {
            Ok(Some(raw)) => {
                for (target, key) in [
                    (&mut this.language, "language"),
                    (&mut this.appearance, "appearance"),
                    (&mut this.accent, "accent"),
                ] {
                    if let Some(value) = raw[key].as_str() {
                        *target = value.into();
                    }
                }
                if let Ok(layout) = serde_json::from_value(raw["requestLayout"].clone()) {
                    this.request_layout = layout;
                }
                if let Some(enabled) = raw["autoCheckUpdates"].as_bool() {
                    this.auto_check_updates = enabled;
                }
                this.network.update(cx, |network, cx| {
                    network.restore(&raw["network"], window, cx)
                });
            }
            Ok(None) => {}
            Err(error) => {
                crate::feedback::show(Notification::error(format!("{error:#}")), window, cx);
            }
        }
        this
    }
    pub fn snapshot(&self, cx: &App) -> serde_json::Value {
        serde_json::json!({"language":self.language, "appearance":self.appearance, "accent":self.accent, "requestLayout":self.request_layout, "autoCheckUpdates":self.auto_check_updates, "network":self.network.read(cx).snapshot(cx)})
    }
    pub fn set_notes(&mut self, notes: String, cx: &mut Context<Self>) {
        self.notes = notes;
        cx.notify();
    }
    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.network.read(cx).value(cx) {
            Ok(network) => cx.emit(SettingsEvent::Save(AppPreferences {
                language: self.language.clone(),
                appearance: self.appearance.clone(),
                accent: self.accent.clone(),
                request_layout: self.request_layout,
                auto_check_updates: self.auto_check_updates,
                network,
            })),
            Err(error) => {
                crate::feedback::show(Notification::error(format!("{error:#}")), window, cx)
            }
        }
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = div()
            .w(rems(16.))
            .flex_shrink_0()
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child(tr("设置")),
            )
            .children(
                [
                    Category::Appearance,
                    Category::Network,
                    Category::Proxy,
                    Category::About,
                ]
                .into_iter()
                .enumerate()
                .map(|(ix, category)| {
                    Button::new(("settings-category", ix))
                        .ghost()
                        .w_full()
                        .selected(self.category == category)
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    Icon::new(category.icon())
                                        .size_4()
                                        .flex_shrink_0()
                                        .text_color(if self.category == category {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().muted_foreground
                                        }),
                                )
                                .child(div().flex_1().min_w_0().child(category.title())),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.category = category;
                            this.network.update(cx, |network, cx| {
                                network.set_section(
                                    if category == Category::Proxy {
                                        NetworkSection::Proxy
                                    } else {
                                        NetworkSection::Network
                                    },
                                    cx,
                                )
                            });
                            cx.notify();
                        }))
                }),
            );
        let detail = match self.category {
            Category::Appearance => div()
                .flex()
                .flex_col()
                .gap_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().font_weight(FontWeight::MEDIUM).child(tr("界面语言")))
                        .child(div().flex().gap_2().children(
                            [("zh-CN", "简体中文"), ("en", "English")].into_iter().map(
                                |(language, name)| {
                                    Button::new(language)
                                        .icon(AppIcon::Languages)
                                        .outline()
                                        .selected(self.language == language)
                                        .label(name)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.language = language.into();
                                            cx.notify();
                                        }))
                                },
                            ),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().font_weight(FontWeight::MEDIUM).child(tr("主题")))
                        .child(
                            div().flex().flex_wrap().gap_2().children(
                                [
                                    ("system", tr("跟随系统")),
                                    ("light", tr("浅色")),
                                    ("dark", tr("深色")),
                                ]
                                .into_iter()
                                .map(|(value, label)| {
                                    Button::new(value)
                                        .icon(match value {
                                            "light" => AppIcon::Sun,
                                            "dark" => AppIcon::Moon,
                                            _ => AppIcon::Monitor,
                                        })
                                        .outline()
                                        .selected(self.appearance == value)
                                        .label(label)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.appearance = value.into();
                                            cx.notify();
                                        }))
                                }),
                            ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().font_weight(FontWeight::MEDIUM).child(tr("强调色")))
                        .child(
                            div().flex().flex_wrap().gap_2().children(
                                [
                                    ("amber", tr("琥珀")),
                                    ("slate", tr("石板")),
                                    ("blue", tr("蓝色")),
                                    ("green", tr("绿色")),
                                    ("rose", tr("玫瑰")),
                                    ("violet", tr("紫色")),
                                ]
                                .into_iter()
                                .map(|(value, label)| {
                                    Button::new(value)
                                        .outline()
                                        .small()
                                        .selected(self.accent == value)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(div().size_3().rounded_full().bg(
                                                    crate::appearance::accent_color(
                                                        value,
                                                        cx.theme().is_dark(),
                                                    ),
                                                ))
                                                .child(label),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.accent = value.into();
                                            cx.notify();
                                        }))
                                }),
                            ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr("默认请求布局")),
                        )
                        .child(
                            div().flex().flex_wrap().gap_3().children(
                                [
                                    (
                                        "layout-vertical",
                                        RequestLayout::Vertical,
                                        tr("上下布局"),
                                    ),
                                    (
                                        "layout-horizontal",
                                        RequestLayout::Horizontal,
                                        tr("左右布局"),
                                    ),
                                ]
                                .into_iter()
                                .map(
                                    |(id, layout, label)| {
                                        let selected = self.request_layout == layout;
                                        Button::new(id)
                                            .outline()
                                            .selected(selected)
                                            .toggled(selected)
                                            .accessibility_label(label)
                                            .w(px(216.))
                                            .h(px(180.))
                                            .p_3()
                                            .rounded_lg()
                                            .child(
                                                div().flex().flex_col().w_full().gap_2()
                                                    .child(request_layout_preview(layout, cx))
                                                    .child(
                                                        div().flex().items_center().justify_between()
                                                            .child(label)
                                                            .child(
                                                                div().flex().items_center().justify_center()
                                                                    .size_4().rounded_full().border_1()
                                                                    .border_color(if selected { cx.theme().primary } else { cx.theme().border })
                                                                    .when(selected, |view| {
                                                                        view.bg(cx.theme().primary)
                                                                            .child(Icon::new(IconName::Check).size(px(10.)).text_color(cx.theme().primary_foreground))
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.request_layout = layout;
                                                cx.notify();
                                            }))
                                    },
                                ),
                            ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(tr("统一设置所有请求的请求与响应面板排列方式。")),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr("语言、主题、强调色和布局在保存后生效。")),
                )
                .into_any_element(),
            Category::Network | Category::Proxy => self.network.clone().into_any_element(),
            Category::About => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            img(crate::brand::LOGO)
                                .size_10()
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("ApiFuck"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(crate::i18n::format(
                                            tr("版本 {}"),
                                            &[("0", env!("CARGO_PKG_VERSION").to_owned())],
                                        )),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .child(tr("基于 Rust 与 GPUI 的本地 API 调试工作台。")),
                )
                .child(div().flex().items_center().justify_between().gap_4()
                    .child(tr("自动检查更新"))
                    .child(Switch::new("auto-check-updates").checked(self.auto_check_updates)
                        .on_click(cx.listener(|this, enabled, _, cx| {
                            this.auto_check_updates = *enabled;
                            cx.notify();
                        }))))
                .child(
                    Button::new("check-updates")
                        .icon(AppIcon::RefreshCw)
                        .outline()
                        .label(tr("检查更新"))
                        .on_click(cx.listener(|_, _, _, cx| {
                            cx.emit(SettingsEvent::CheckUpdates);
                        })),
                )
                .child(div().font_weight(FontWeight::MEDIUM).child(tr("版本介绍")))
                .child(TextView::markdown("release-notes", self.notes.clone()))
                .into_any_element(),
        };
        div()
            .flex()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(menu)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(
                        div()
                            .flex_shrink_0()
                            .px_6()
                            .py_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.category.title()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.category.description()),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-details")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_6()
                            .child(div().max_w(rems(48.)).child(detail)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_4()
                            .px_6()
                            .py_3()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr("设置保存在此设备，切换分类不会丢失输入。")),
                            )
                            .child(
                                Button::new("save-settings")
                                    .primary()
                                    .icon(AppIcon::Save)
                                    .label(tr("保存设置"))
                                    .flex_shrink_0()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
