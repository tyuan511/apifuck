use crate::i18n::tr;
use anyhow::{Context as _, Result, ensure};
use apifuck_core::NetworkSettings;
use gpui_kit::component::{
    ActiveTheme,
    form::{Field, Form},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::FluentBuilder, *};

#[derive(Clone, Copy, PartialEq)]
pub enum NetworkSection {
    Network,
    Proxy,
}

pub struct NetworkEditor {
    original: NetworkSettings,
    section: NetworkSection,
    timeout: Entity<InputState>,
    connect_timeout: Entity<InputState>,
    redirects: Entity<InputState>,
    proxy: Entity<InputState>,
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
        let subscriptions = [&timeout, &connect_timeout, &redirects, &proxy, &limit]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            _subscriptions: subscriptions,
            original: settings.clone(),
            section: NetworkSection::Network,
            timeout,
            connect_timeout,
            redirects,
            proxy,
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
        let proxy = self.proxy.read(cx).value().trim().to_owned();
        settings.proxy = (!proxy.is_empty()).then_some(proxy);
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
            .when(self.section == NetworkSection::Network, |view| view.child(
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
                    )
            ))
            .when(self.section == NetworkSection::Proxy, |view| view.child(
                Form::new().child(Field::new().label(tr("代理地址")).child(Input::new(&self.proxy)))
            ).child(div().text_sm().text_color(cx.theme().muted_foreground).child(tr("支持 HTTP、HTTPS 和 SOCKS5。留空时使用系统环境代理；填写地址后，应用内请求统一使用此代理。"))))
            .when(self.section == NetworkSection::Network, |view| view.child(
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
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("保存后应用于所有项目。请求文件中明确指定的超时和重定向选项优先。")),
            )
    }
}
