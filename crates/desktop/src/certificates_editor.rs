use crate::{i18n::tr, icons::AppIcon, surfaces::confirm};
use apifuck_core::{NetworkSettings, tls::ClientCertificate};
use gpui_kit::component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    form::{Field, Form},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{Value, json};
use std::path::PathBuf;

struct CertificateRow {
    domain: Entity<InputState>,
    certificate: Entity<InputState>,
    key: Entity<InputState>,
    disabled: bool,
}
pub struct CertificatesEditor {
    ca: Entity<InputState>,
    rows: Vec<CertificateRow>,
    subscriptions: Vec<Subscription>,
}
impl CertificatesEditor {
    pub fn new(settings: &NetworkSettings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let ca = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                settings
                    .custom_ca_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            )
        });
        let mut view = Self {
            ca,
            rows: vec![],
            subscriptions: vec![],
        };
        view.watch(view.ca.clone(), cx);
        for certificate in &settings.client_certificates {
            view.add(certificate, window, cx);
        }
        view
    }
    fn watch(&mut self, input: Entity<InputState>, cx: &mut Context<Self>) {
        self.subscriptions
            .push(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
    }
    fn add(&mut self, value: &ClientCertificate, window: &mut Window, cx: &mut Context<Self>) {
        let domain = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value.domain.clone())
                .placeholder("api.example.com")
        });
        let certificate = cx.new(|cx| {
            InputState::new(window, cx).default_value(value.certificate_file.display().to_string())
        });
        let key = cx.new(|cx| {
            InputState::new(window, cx).default_value(value.private_key_file.display().to_string())
        });
        for input in [&domain, &certificate, &key] {
            self.watch(input.clone(), cx);
        }
        self.rows.push(CertificateRow {
            domain,
            certificate,
            key,
            disabled: value.disabled,
        });
        cx.notify();
    }
    pub fn snapshot(&self, cx: &App) -> Value {
        json!({"ca":self.ca.read(cx).value().to_string(),"rows":self.rows.iter().map(|row|json!({"domain":row.domain.read(cx).value().to_string(),"certificate":row.certificate.read(cx).value().to_string(),"key":row.key.read(cx).value().to_string(),"disabled":row.disabled})).collect::<Vec<_>>()})
    }
    pub fn restore(&mut self, raw: &Value, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ca) = raw["ca"].as_str() {
            self.ca
                .update(cx, |input, cx| input.set_value(ca.to_owned(), window, cx));
        }
        if let Some(rows) = raw["rows"].as_array() {
            self.rows.clear();
            self.subscriptions.clear();
            self.watch(self.ca.clone(), cx);
            for row in rows {
                self.add(
                    &ClientCertificate {
                        domain: row["domain"].as_str().unwrap_or("").into(),
                        certificate_file: row["certificate"].as_str().unwrap_or("").into(),
                        private_key_file: row["key"].as_str().unwrap_or("").into(),
                        disabled: row["disabled"] == true,
                    },
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }
    pub fn apply(&self, settings: &mut NetworkSettings, cx: &App) {
        let ca = self.ca.read(cx).value();
        settings.custom_ca_path = (!ca.trim().is_empty()).then(|| PathBuf::from(ca.trim()));
        settings.client_certificates = self
            .rows
            .iter()
            .map(|row| ClientCertificate {
                domain: row.domain.read(cx).value().trim().to_owned(),
                certificate_file: PathBuf::from(row.certificate.read(cx).value().trim()),
                private_key_file: PathBuf::from(row.key.read(cx).value().trim()),
                disabled: row.disabled,
            })
            .collect();
    }
    fn browse(&mut self, input: Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("选择证书或私钥文件").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = choice.await else {
                return;
            };
            if let Some(path) = paths.first() {
                let value = path.display().to_string();
                let _ = this.update_in(cx, |_, window, cx| {
                    input.update(cx, |input, cx| input.set_value(value, window, cx))
                });
            }
        })
        .detach();
    }
    fn is_blank(row: &CertificateRow, cx: &App) -> bool {
        [&row.domain, &row.certificate, &row.key]
            .iter()
            .all(|input| input.read(cx).value().trim().is_empty())
    }
    fn remove_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.rows.get(index).is_none() {
            return;
        }
        if Self::is_blank(&self.rows[index], cx) {
            self.rows.remove(index);
            cx.notify();
            return;
        }
        let weak = cx.entity().downgrade();
        confirm(
            "删除证书配置",
            "将移除这条客户端证书配置，保存设置后生效。",
            "删除",
            window,
            cx,
            move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    if index < this.rows.len() {
                        this.rows.remove(index);
                        cx.notify();
                    }
                });
            },
        );
    }
    fn problem(&self, input: &Entity<InputState>, cx: &App) -> Option<Div> {
        let value = input.read(cx).value();
        let message = path_problem(value.trim())?;
        Some(
            div()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(tr(message).to_owned()),
        )
    }
    fn path_input(&self, id: String, input: &Entity<InputState>, cx: &mut Context<Self>) -> Div {
        let input = input.clone();
        let browse = input.clone();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&input)))
                    .child(
                        Button::new(SharedString::from(id))
                            .ghost()
                            .icon(AppIcon::FolderOpen)
                            .tooltip(tr("选择文件…"))
                            .accessibility_label(tr("选择文件…"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.browse(browse.clone(), window, cx)
                            })),
                    ),
            )
            .children(self.problem(&input, cx))
    }
}
impl Render for CertificatesEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr("TLS 证书")),
            )
            .child(
                Form::new().child(
                    Field::new()
                        .label(tr("自定义 CA（PEM / DER，可选）"))
                        .child(self.path_input("tls-ca".into(), &self.ca, cx)),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(
                        "保留内置信任根。客户端证书按域名从上到下匹配，支持未加密 PEM 证书和私钥。",
                    )),
            )
            .when(self.rows.is_empty(), |col| {
                col.child(
                    div()
                        .p_3()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr("尚未配置客户端证书")),
                )
            })
            .children(self.rows.iter().enumerate().map(|(index, row)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div().flex_1().child(
                                    Switch::new(("certificate-enabled", index))
                                        .label(tr("启用客户端证书"))
                                        .checked(!row.disabled)
                                        .on_change(cx.listener(
                                            move |this, value: &bool, _, cx| {
                                                if let Some(row) = this.rows.get_mut(index) {
                                                    row.disabled = !*value;
                                                }
                                                cx.notify();
                                            },
                                        )),
                                ),
                            )
                            .child(
                                Button::new(("certificate-remove", index))
                                    .ghost()
                                    .icon(AppIcon::Trash)
                                    .tooltip(tr("删除证书配置"))
                                    .accessibility_label(tr("删除证书配置"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.remove_row(index, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        Form::new()
                            .child(
                                Field::new()
                                    .label(tr("目标域名（可带端口或 *. 通配符）"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(Input::new(&row.domain))
                                            .when(
                                                row.domain.read(cx).value().trim().is_empty()
                                                    && !(row
                                                        .certificate
                                                        .read(cx)
                                                        .value()
                                                        .trim()
                                                        .is_empty()
                                                        && row
                                                            .key
                                                            .read(cx)
                                                            .value()
                                                            .trim()
                                                            .is_empty()),
                                                |col| {
                                                    col.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().danger)
                                                            .child(tr("请填写目标域名")),
                                                    )
                                                },
                                            ),
                                    ),
                            )
                            .child(Field::new().label(tr("客户端证书文件（PEM）")).child(
                                self.path_input(
                                    format!("certificate-file-{index}"),
                                    &row.certificate,
                                    cx,
                                ),
                            ))
                            .child(Field::new().label(tr("未加密私钥文件（PEM）")).child(
                                self.path_input(format!("certificate-key-{index}"), &row.key, cx),
                            )),
                    )
            }))
            .child(
                Button::new("certificate-add")
                    .icon(AppIcon::Plus)
                    .label(tr("添加客户端证书"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add(&ClientCertificate::default(), window, cx)
                    })),
            )
    }
}

/// Cheap per-render hint; the core still validates when the settings are saved.
fn path_problem(value: &str) -> Option<&'static str> {
    if value.is_empty() {
        return None;
    }
    let path = std::path::Path::new(value);
    if !path.is_absolute() {
        Some("路径必须是绝对路径")
    } else if !path.is_file() {
        Some("文件不存在")
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::path_problem;
    #[test]
    fn flags_relative_and_missing_paths_only() {
        assert_eq!(path_problem(""), None);
        assert_eq!(path_problem("relative/ca.pem"), Some("路径必须是绝对路径"));
        let missing = std::env::temp_dir().join("apifuck-missing-ca.pem");
        assert_eq!(
            path_problem(&missing.display().to_string()),
            Some("文件不存在")
        );
        assert_eq!(
            path_problem(&std::env::current_exe().unwrap().display().to_string()),
            None
        );
    }
}
