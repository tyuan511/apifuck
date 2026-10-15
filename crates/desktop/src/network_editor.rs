use crate::i18n::tr;
use anyhow::{Context as _, Result, ensure};
use apifuck_core::NetworkSettings;
use gpui_kit::component::{
    ActiveTheme,
    form::{Field, Form},
    input::{Input, InputEvent, InputState},
    select::{Select, SelectEvent, SelectItem, SelectState},
    switch::Switch,
};
use gpui_kit::{prelude::FluentBuilder, *};

#[derive(Clone, Copy, PartialEq)]
pub enum NetworkSection {
    Network,
    Proxy,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProxyMode {
    Disabled,
    System,
    Custom,
}

impl ProxyMode {
    fn from_proxy(proxy: Option<&str>) -> Self {
        match proxy {
            None => Self::System,
            Some("") => Self::Disabled,
            Some(_) => Self::Custom,
        }
    }

    fn from_address(address: &str) -> Self {
        if address.trim().is_empty() {
            Self::System
        } else {
            Self::Custom
        }
    }

    fn restore(raw: &serde_json::Value, fallback: Self) -> Self {
        serde_json::from_value(raw["proxyMode"].clone()).unwrap_or_else(|_| {
            raw["proxy"]
                .as_str()
                .map(Self::from_address)
                .unwrap_or(fallback)
        })
    }

    fn proxy(self, address: &str) -> Result<Option<String>> {
        match self {
            Self::Disabled => Ok(Some(String::new())),
            Self::System => Ok(None),
            Self::Custom => {
                let address = address.trim();
                ensure!(!address.is_empty(), "{}", tr("请输入自定义代理地址"));
                Ok(Some(address.to_owned()))
            }
        }
    }
}

impl SelectItem for ProxyMode {
    type Value = Self;

    fn title(&self) -> SharedString {
        tr(match self {
            Self::Disabled => "无代理",
            Self::System => "使用系统代理",
            Self::Custom => "自定义代理",
        })
        .into()
    }

    fn value(&self) -> &Self {
        self
    }
}

pub struct NetworkEditor {
    original: NetworkSettings,
    section: NetworkSection,
    timeout: Entity<InputState>,
    connect_timeout: Entity<InputState>,
    redirects: Entity<InputState>,
    proxy: Entity<InputState>,
    proxy_mode: ProxyMode,
    proxy_select: Entity<SelectState<Vec<ProxyMode>>>,
    limit: Entity<InputState>,
    follow: bool,
    invalid_certificates: bool,
    _subscriptions: Vec<Subscription>,
}
impl NetworkEditor {
    pub fn new(settings: NetworkSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let timeout =
            cx.new(|cx| InputState::new(window, cx).default_value(settings.timeout_ms.to_string()));
        let connect_timeout = cx.new(|cx| {
            InputState::new(window, cx).default_value(settings.connect_timeout_ms.to_string())
        });
        let redirects = cx.new(|cx| {
            InputState::new(window, cx).default_value(settings.max_redirects.to_string())
        });
        let proxy = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(settings.proxy.clone().unwrap_or_default())
                .placeholder("http://127.0.0.1:7890")
        });
        let limit = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value((settings.max_response_bytes / 1024 / 1024).to_string())
        });
        let proxy_mode = ProxyMode::from_proxy(settings.proxy.as_deref());
        let proxy_select = cx.new(|cx| {
            let mut select = SelectState::new(
                vec![ProxyMode::Disabled, ProxyMode::System, ProxyMode::Custom],
                None,
                window,
                cx,
            );
            select.set_selected_value(&proxy_mode, window, cx);
            select
        });
        let mut subscriptions: Vec<_> = [&timeout, &connect_timeout, &redirects, &proxy, &limit]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        subscriptions.push(cx.subscribe(
            &proxy_select,
            |this, _, event: &SelectEvent<Vec<ProxyMode>>, cx| {
                if let SelectEvent::Confirm(Some(mode)) = event {
                    this.proxy_mode = *mode;
                    cx.notify();
                }
            },
        ));
        Self {
            _subscriptions: subscriptions,
            original: settings.clone(),
            section: NetworkSection::Network,
            timeout,
            connect_timeout,
            redirects,
            proxy,
            proxy_mode,
            proxy_select,
            limit,
            follow: settings.follow_redirects,
            invalid_certificates: settings.allow_invalid_certificates,
        }
    }
    pub fn set_section(&mut self, section: NetworkSection, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }
    pub fn snapshot(&self, cx: &App) -> serde_json::Value {
        serde_json::json!({
            "timeout":self.timeout.read(cx).value().to_string(),
            "connectTimeout":self.connect_timeout.read(cx).value().to_string(),
            "redirects":self.redirects.read(cx).value().to_string(),
            "proxy":self.proxy.read(cx).value().to_string(),
            "proxyMode":self.proxy_mode,
            "limit":self.limit.read(cx).value().to_string(),
            "follow":self.follow, "invalidCertificates":self.invalid_certificates,
        })
    }
    pub fn restore(
        &mut self,
        raw: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.proxy_mode = ProxyMode::restore(raw, self.proxy_mode);
        self.proxy_select.update(cx, |select, cx| {
            select.set_selected_value(&self.proxy_mode, window, cx);
        });
        for (input, key) in [
            (&self.timeout, "timeout"),
            (&self.connect_timeout, "connectTimeout"),
            (&self.redirects, "redirects"),
            (&self.proxy, "proxy"),
            (&self.limit, "limit"),
        ] {
            if let Some(value) = raw[key].as_str() {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_owned(), window, cx)
                });
            }
        }
        self.follow = raw["follow"].as_bool().unwrap_or(self.follow);
        self.invalid_certificates = raw["invalidCertificates"]
            .as_bool()
            .unwrap_or(self.invalid_certificates);
        cx.notify();
    }
    pub fn value(&self, cx: &App) -> Result<NetworkSettings> {
        let mut settings = self.original.clone();
        settings.timeout_ms = self
            .timeout
            .read(cx)
            .value()
            .parse()
            .context(tr("请求超时必须是非负整数"))?;
        settings.connect_timeout_ms = self
            .connect_timeout
            .read(cx)
            .value()
            .parse()
            .context(tr("连接超时必须是正整数"))?;
        settings.max_redirects = self
            .redirects
            .read(cx)
            .value()
            .parse()
            .context(tr("重定向次数必须是非负整数"))?;
        let megabytes: usize = self
            .limit
            .read(cx)
            .value()
            .parse()
            .context(tr("响应大小必须是正整数"))?;
        ensure!(
            megabytes > 0 && megabytes <= 1024,
            "{}",
            tr("响应大小应在 1–1024 MiB 之间")
        );
        ensure!(
            settings.connect_timeout_ms > 0,
            "{}",
            tr("连接超时必须大于 0")
        );
        settings.max_response_bytes = megabytes * 1024 * 1024;
        settings.follow_redirects = self.follow;
        settings.allow_invalid_certificates = self.invalid_certificates;
        settings.proxy = self.proxy_mode.proxy(&self.proxy.read(cx).value())?;
        settings.validate().map_err(|error| {
            anyhow::anyhow!(
                "{}",
                error
                    .chain()
                    .map(|cause| tr(&cause.to_string()).to_owned())
                    .collect::<Vec<_>>()
                    .join(": ")
            )
        })?;
        Ok(settings)
    }
}
impl Render for NetworkEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .when(self.section == NetworkSection::Network, |view| {
                view.child(
                    Form::new()
                        .columns(2)
                        .child(
                            Field::new()
                                .label(tr("请求超时（ms，0 为不限）"))
                                .child(Input::new(&self.timeout)),
                        )
                        .child(
                            Field::new()
                                .label(tr("连接超时（ms）"))
                                .child(Input::new(&self.connect_timeout)),
                        )
                        .child(
                            Field::new()
                                .label(tr("最大重定向次数"))
                                .child(Input::new(&self.redirects)),
                        )
                        .child(
                            Field::new()
                                .label(tr("响应大小上限（MiB）"))
                                .child(Input::new(&self.limit)),
                        ),
                )
            })
            .when(self.section == NetworkSection::Proxy, |view| {
                view.child(
                    Form::new().child(
                        Field::new().label(tr("代理方式")).child(
                            Select::new(&self.proxy_select)
                                .accessibility_label(tr("代理方式"))
                                .w_full(),
                        ),
                    ),
                )
                .when(self.proxy_mode == ProxyMode::Custom, |view| {
                    view.child(
                        Form::new().child(
                            Field::new()
                                .label(tr("代理地址"))
                                .child(Input::new(&self.proxy)),
                        ),
                    )
                })
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(match self.proxy_mode {
                            ProxyMode::Disabled => tr("直接连接，不使用系统或自定义代理。"),
                            ProxyMode::System => {
                                tr("自动使用系统或环境变量中的代理配置，无需填写地址。")
                            }
                            ProxyMode::Custom => {
                                tr("请输入完整的代理地址，支持 HTTP、HTTPS、SOCKS5 和 SOCKS5H。")
                            }
                        }),
                )
            })
            .when(self.section == NetworkSection::Network, |view| {
                view.child(
                    Switch::new("follow-redirects")
                        .label(tr("跟随重定向"))
                        .checked(self.follow)
                        .on_change(cx.listener(|this, value, _, cx| {
                            this.follow = *value;
                            cx.notify();
                        })),
                )
                .child(
                    Switch::new("invalid-certificates")
                        .label(tr("允许无效 TLS 证书"))
                        .checked(self.invalid_certificates)
                        .on_change(cx.listener(|this, value, _, cx| {
                            this.invalid_certificates = *value;
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(
                        "保存后应用于所有项目。请求文件中明确指定的超时和重定向选项优先。",
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::ProxyMode;
    use apifuck_core::NetworkSettings;
    use serde_json::json;

    #[test]
    fn proxy_mode_restores_legacy_and_explicit_drafts() {
        assert_eq!(
            ProxyMode::restore(&json!({"proxy":"http://localhost:7890"}), ProxyMode::System),
            ProxyMode::Custom
        );
        assert_eq!(
            ProxyMode::restore(&json!({"proxy":"  "}), ProxyMode::Custom),
            ProxyMode::System
        );
        for (mode, address) in [
            (ProxyMode::Disabled, "http://"),
            (ProxyMode::System, "http://"),
            (ProxyMode::Custom, ""),
        ] {
            let raw = json!({"proxyMode":mode, "proxy":address});
            assert_eq!(ProxyMode::restore(&raw, ProxyMode::System), mode);
        }
        assert_eq!(
            ProxyMode::restore(&json!({}), ProxyMode::Custom),
            ProxyMode::Custom
        );
    }

    #[test]
    fn system_ignores_hidden_address_and_custom_requires_valid_address() {
        for address in ["", "http://", "http://localhost:7890"] {
            assert_eq!(ProxyMode::System.proxy(address).unwrap(), None);
            assert_eq!(
                ProxyMode::Disabled.proxy(address).unwrap(),
                Some(String::new())
            );
        }
        assert!(ProxyMode::Custom.proxy(" \t").is_err());
        let mut settings = NetworkSettings::default();
        settings.proxy = ProxyMode::Custom.proxy("http://").unwrap();
        assert!(settings.validate().is_err());
        settings.proxy = ProxyMode::Custom
            .proxy("  socks5h://localhost:1080  ")
            .unwrap();
        assert_eq!(settings.proxy.as_deref(), Some("socks5h://localhost:1080"));
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn saved_proxy_modes_remain_distinct_after_reload() {
        for mode in [ProxyMode::Disabled, ProxyMode::System, ProxyMode::Custom] {
            let mut settings = NetworkSettings::default();
            settings.proxy = mode.proxy("http://localhost:7890").unwrap();
            settings.validate().unwrap();
            let restored: NetworkSettings =
                serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
            assert_eq!(ProxyMode::from_proxy(restored.proxy.as_deref()), mode);
        }
    }
}
