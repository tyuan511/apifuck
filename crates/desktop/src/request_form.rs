use crate::editor_tools::{LanguageBinding, format_button};
use crate::i18n::tr;
use crate::icons::AppIcon;
use crate::key_value_editor::{Changed, KeyValueEditor};
use crate::languages::Language;
use crate::surfaces::{panel_editor, panel_toolbar};
use crate::variable_input::EditorVariablePreview;
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    form::{Field, Form},
    input::{EditorState, Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    resizable::{resizable_panel, v_resizable},
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FormKind {
    BaseUrl,
    Params,
    Headers,
    Variables,
    Auth,
    Body,
    Settings,
    Graphql,
    Assertions,
}
pub struct RequestForm {
    kind: FormKind,
    variables: crate::variable_input::Variables,
    original: Value,
    mode: String,
    fields: Vec<(String, String, Entity<InputState>)>,
    table: Entity<KeyValueEditor>,
    paths: Entity<KeyValueEditor>,
    editor: Entity<EditorState>,
    gql_variables: Entity<EditorState>,
    editor_variable_preview: Entity<EditorVariablePreview>,
    gql_variable_preview: Entity<EditorVariablePreview>,
    _subscriptions: Vec<Subscription>,
    _language_bindings: Vec<Entity<LanguageBinding>>,
    field_subscriptions: Vec<Subscription>,
    mode_cache: std::collections::BTreeMap<String, Value>,
}
impl EventEmitter<Changed> for RequestForm {}
impl RequestForm {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let table = cx.new(|_| KeyValueEditor::new(tr("查询参数"), "value"));
        let paths = cx.new(|_| KeyValueEditor::new(tr("路径参数"), "value"));
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
        });
        let gql_variables = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .default_value("{}")
        });
        let editor_variable_preview = EditorVariablePreview::attach(&editor, cx);
        let gql_variable_preview = EditorVariablePreview::attach(&gql_variables, cx);
        let subscriptions = vec![
            cx.subscribe(&gql_variables, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.emit(Changed);
                    cx.notify();
                }
            }),
            cx.subscribe(&table, |_, _, _: &Changed, cx| {
                cx.emit(Changed);
                cx.notify();
            }),
            cx.subscribe(&paths, |_, _, _: &Changed, cx| {
                cx.emit(Changed);
                cx.notify();
            }),
            cx.subscribe(&editor, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.emit(Changed);
                    cx.notify();
                }
            }),
        ];
        let language_bindings = vec![
            LanguageBinding::attach(&editor, window, cx),
            LanguageBinding::attach(&gql_variables, window, cx),
        ];
        Self {
            kind: FormKind::Params,
            variables: Default::default(),
            original: Value::Null,
            mode: String::new(),
            fields: vec![],
            table,
            paths,
            editor,
            gql_variables,
            editor_variable_preview,
            gql_variable_preview,
            _subscriptions: subscriptions,
            _language_bindings: language_bindings,
            field_subscriptions: vec![],
            mode_cache: Default::default(),
        }
    }
    pub fn set_variables(
        &mut self,
        variables: crate::variable_input::Variables,
        cx: &mut Context<Self>,
    ) {
        self.variables = variables.clone();
        self.table.update(cx, |table, cx| {
            table.set_variables(variables.clone());
            cx.notify();
        });
        self.paths.update(cx, |table, cx| {
            table.set_variables(variables);
            cx.notify();
        });
        cx.notify();
    }
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.table
            .update(cx, |table, cx| table.refresh_locale(window, cx));
        self.paths
            .update(cx, |table, cx| table.refresh_locale(window, cx));
        cx.notify();
    }
    pub fn load(
        &mut self,
        kind: FormKind,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        self.original = value.clone();
        self.fields.clear();
        self.field_subscriptions.clear();
        self.table.update(cx, |t, _| {
            t.set_multipart(false);
            t.set_assertions(kind == FormKind::Assertions);
        });
        self.table.update(cx, |t, _| {
            t.set_title(match kind {
                FormKind::Headers => tr("请求头"),
                FormKind::Assertions => tr("断言"),
                FormKind::Variables => tr("变量"),
                FormKind::Body => tr("表单字段"),
                _ => tr("查询参数"),
            })
        });
        match kind {
            FormKind::BaseUrl => self.add_field(
                "base_url",
                tr("Base URL（留空继承上级）"),
                value.as_str().unwrap_or(""),
                window,
                cx,
            ),
            FormKind::Graphql => {
                self.editor.update(cx, |s, cx| {
                    s.set_value(value["query"].as_str().unwrap_or("").to_owned(), window, cx);
                    s.set_highlighter("graphql", cx);
                });
                self.gql_variables.update(cx, |s, cx| {
                    s.set_highlighter("json", cx);
                    s.set_value(
                        value["variables"].as_str().unwrap_or("{}").to_owned(),
                        window,
                        cx,
                    )
                });
            }
            FormKind::Params => {
                let rows = value.as_array().cloned().unwrap_or_default();
                self.table.update(cx, |t, cx| {
                    t.set_value(
                        &json!(
                            rows.iter()
                                .filter(|p| p["type"] != "path")
                                .collect::<Vec<_>>()
                        ),
                        window,
                        cx,
                    )
                });
                self.paths.update(cx, |t, cx| {
                    t.set_value(
                        &json!(
                            rows.iter()
                                .filter(|p| p["type"] == "path")
                                .collect::<Vec<_>>()
                        ),
                        window,
                        cx,
                    )
                });
            }
            FormKind::Headers | FormKind::Variables | FormKind::Assertions => self
                .table
                .update(cx, |t, cx| t.set_value(value, window, cx)),
            FormKind::Auth => {
                self.mode = if value == "inherit" {
                    "inherit".into()
                } else {
                    value["type"].as_str().unwrap_or("none").into()
                };
                self.load_auth(window, cx);
            }
            FormKind::Body => {
                self.mode = value["type"].as_str().unwrap_or("none").into();
                self.load_body(window, cx);
            }
            FormKind::Settings => {
                for (key, label, default) in [
                    (
                        "timeout",
                        tr("超时（毫秒，0 表示不限；inherit 表示继承）"),
                        "inherit",
                    ),
                    (
                        "maxRedirects",
                        tr("最大重定向次数（inherit 表示继承）"),
                        "inherit",
                    ),
                    ("followRedirects", tr("跟随重定向"), "inherit"),
                    ("encodeUrl", tr("编码 URL"), "inherit"),
                ] {
                    let value = value
                        .get(key)
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| v.to_string())
                        })
                        .unwrap_or(default.into());
                    self.add_field(key, label, &value, window, cx);
                }
            }
        }
        cx.notify();
    }
    fn add_field(
        &mut self,
        key: &str,
        label: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value.to_owned())
                .masked(matches!(key, "password" | "clientSecret"))
        });
        self.field_subscriptions
            .push(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.emit(Changed);
                    cx.notify();
                }
            }));
        self.fields.push((key.into(), label.into(), input));
    }
    fn load_auth(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.fields.clear();
        self.field_subscriptions.clear();
        let fields: &[(&str, &str)] = match self.mode.as_str() {
            "basic" | "digest" | "ntlm" | "wsse" => {
                &[("username", tr("用户名")), ("password", tr("密码"))]
            }
            "bearer" => &[("token", tr("令牌"))],
            "apikey" => &[
                ("key", tr("键名")),
                ("value", tr("值")),
                ("placement", tr("添加位置")),
            ],
            "oauth2" => &[
                ("flow", tr("授权流程")),
                ("authorizationUrl", tr("授权地址")),
                ("callbackUrl", tr("回调地址")),
                ("refreshTokenUrl", tr("刷新令牌地址（可选）")),
                ("credentialPlacement", tr("客户端凭据位置")),
                ("accessTokenUrl", tr("访问令牌地址")),
                ("clientId", tr("客户端 ID")),
                ("clientSecret", tr("客户端密钥")),
                ("scope", tr("权限范围")),
            ],
            _ => &[],
        };
        for (key, label) in fields {
            let source = if self.mode == "oauth2" && matches!(*key, "clientId" | "clientSecret") {
                &self.original["credentials"][*key]
            } else if *key == "credentialPlacement" {
                &self.original["credentials"]["placement"]
            } else {
                &self.original[*key]
            };
            let value = source
                .as_str()
                .unwrap_or(match *key {
                    "placement" => "header",
                    "credentialPlacement" => "body",
                    "flow" => "client_credentials",
                    "callbackUrl" => "http://127.0.0.1:0/oauth/callback",
                    _ => "",
                })
                .to_owned();
            self.add_field(key, label, &value, window, cx);
        }
    }
    fn load_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.table
            .update(cx, |t, _| t.set_multipart(self.mode == "multipart-form"));
        self.fields.clear();
        self.field_subscriptions.clear();
        match self.mode.as_str() {
            "form-urlencoded" | "multipart-form" => self
                .table
                .update(cx, |t, cx| t.set_value(&self.original["data"], window, cx)),
            "file" => {
                let file = self.original["data"]
                    .as_array()
                    .and_then(|a| a.iter().find(|f| f["selected"] == true))
                    .cloned()
                    .unwrap_or(json!({}));
                self.add_field(
                    "filePath",
                    tr("文件"),
                    file["filePath"].as_str().unwrap_or(""),
                    window,
                    cx,
                );
                self.add_field(
                    "contentType",
                    tr("内容类型"),
                    file["contentType"]
                        .as_str()
                        .unwrap_or("application/octet-stream"),
                    window,
                    cx,
                );
            }
            _ => {
                let text = self.original["data"].as_str().unwrap_or("").to_owned();
                self.editor.update(cx, |s, cx| {
                    s.set_value(text, window, cx);
                    s.set_highlighter(
                        if self.mode == "json" {
                            "json"
                        } else if self.mode == "xml" {
                            "xml"
                        } else if self.mode == "sparql" {
                            "sparql"
                        } else {
                            "text"
                        },
                        cx,
                    );
                });
            }
        }
    }
    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("选择上传文件").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = this.update_in(cx, |this, window, cx| {
                    if let Some((_, _, input)) =
                        this.fields.iter().find(|(key, _, _)| key == "filePath")
                    {
                        input.update(cx, |s, cx| s.set_value(path, window, cx));
                        cx.emit(Changed);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
    fn fields_view(&self, window: &Window, cx: &mut Context<Self>) -> Form {
        let authorization_code = self
            .fields
            .iter()
            .find(|(key, _, _)| key == "flow")
            .is_some_and(|(_, _, input)| input.read(cx).value() == "authorization_code");
        Form::new().children(
            self.fields
                .iter()
                .filter(|(key, _, _)| {
                    authorization_code
                        || !matches!(key.as_str(), "authorizationUrl" | "callbackUrl")
                })
                .map(|(key, label, input)| {
                    let options: Option<Vec<&'static str>> = match key.as_str() {
                        "placement" => Some(vec!["header", "query"]),
                        "flow" => Some(vec!["client_credentials", "authorization_code"]),
                        "credentialPlacement" => Some(vec!["body", "basic_auth_header"]),
                        "followRedirects" | "encodeUrl" => Some(vec!["inherit", "true", "false"]),
                        _ => None,
                    };
                    let control = if let Some(options) = options {
                        let input = input.clone();
                        let weak = cx.entity().downgrade();
                        Button::new(SharedString::from(format!("field:{key}")))
                            .icon(crate::icons::value_kind(&input.read(cx).value()))
                            .outline()
                            .justify_start()
                            .label(mode_label(&input.read(cx).value()).to_owned())
                            .dropdown_caret(true)
                            .dropdown_menu(move |menu, _, _| {
                                let mut menu = menu;
                                for option in &options {
                                    let option = *option;
                                    let input = input.clone();
                                    let weak = weak.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(mode_label(option))
                                            .icon(crate::icons::value_kind(option))
                                            .on_click(move |_, window, cx| {
                                                input.update(cx, |s, cx| {
                                                    s.set_value(option, window, cx)
                                                });
                                                let _ = weak.update(cx, |_, cx| {
                                                    cx.emit(Changed);
                                                    cx.notify();
                                                });
                                            }),
                                    );
                                }
                                menu
                            })
                            .into_any_element()
                    } else if key == "filePath" {
                        div()
                            .flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(input)))
                            .child(
                                Button::new("browse-file")
                                    .icon(AppIcon::FolderOpen)
                                    .outline()
                                    .label(tr("选择文件…"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_file(window, cx)
                                    })),
                            )
                            .into_any_element()
                    } else {
                        crate::variable_input::input(
                            input,
                            Input::new(input)
                                .when(matches!(key.as_str(), "password" | "clientSecret"), |i| {
                                    i.mask_toggle()
                                }),
                            &self.variables,
                            window,
                            cx,
                        )
                        .into_any_element()
                    };
                    Field::new().label(tr(label).to_owned()).child(control)
                }),
        )
    }
    fn raw_snapshot(&self, cx: &App) -> Value {
        json!({
            "original": self.original, "mode": self.mode,
            "fields": self.fields.iter().map(|(key, _, input)| (key.clone(), json!(input.read(cx).value().to_string()))).collect::<serde_json::Map<_, _>>(),
            "table": self.table.read(cx).snapshot(cx), "paths": self.paths.read(cx).snapshot(cx),
            "editor": self.editor.read(cx).value().to_string(),
            "variables": self.gql_variables.read(cx).value().to_string(),
        })
    }
    pub fn snapshot(&self, cx: &App) -> Value {
        json!({"kind":self.kind, "value":self.value(cx), "raw": self.raw_snapshot(cx), "modes": self.mode_cache})
    }
    fn restore_raw(&mut self, raw: &Value, window: &mut Window, cx: &mut Context<Self>) {
        self.original = raw["original"].clone();
        self.mode = raw["mode"].as_str().unwrap_or("").into();
        match self.kind {
            FormKind::Auth => self.load_auth(window, cx),
            FormKind::Body => self.load_body(window, cx),
            _ => {}
        }
        for (key, _, input) in &self.fields {
            if let Some(value) = raw["fields"][key].as_str() {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_owned(), window, cx)
                });
            }
        }
        self.table
            .update(cx, |table, cx| table.restore(&raw["table"], window, cx));
        self.paths
            .update(cx, |table, cx| table.restore(&raw["paths"], window, cx));
        for (input, key) in [(&self.editor, "editor"), (&self.gql_variables, "variables")] {
            if let Some(value) = raw[key].as_str() {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_owned(), window, cx)
                });
            }
        }
        cx.notify();
    }
    pub fn restore(&mut self, state: &Value, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(kind) = serde_json::from_value(state["kind"].clone()) {
            self.load(kind, &state["value"], window, cx);
        }
        if let Some(raw) = state.get("raw") {
            self.restore_raw(raw, window, cx);
        }
        if let Some(modes) = state.get("modes") {
            self.mode_cache = serde_json::from_value(modes.clone()).unwrap_or_default();
        }
    }
    fn select_mode(&mut self, mode: &str, window: &mut Window, cx: &mut Context<Self>) {
        let prefix = if self.kind == FormKind::Auth {
            "auth"
        } else {
            "body"
        };
        self.mode_cache
            .insert(format!("{prefix}:{}", self.mode), self.raw_snapshot(cx));
        self.mode = mode.into();
        if let Some(raw) = self.mode_cache.get(&format!("{prefix}:{mode}")).cloned() {
            self.restore_raw(&raw, window, cx);
        } else {
            self.original = json!({});
            if self.kind == FormKind::Auth {
                self.load_auth(window, cx);
            } else {
                self.load_body(window, cx);
            }
        }
        cx.emit(Changed);
        cx.notify();
    }
    pub fn value(&self, cx: &App) -> Value {
        match self.kind {
            FormKind::BaseUrl => json!(self.fields[0].2.read(cx).value().to_string()),
            FormKind::Graphql => {
                json!({"query":self.editor.read(cx).value().to_string(),"variables":self.gql_variables.read(cx).value().to_string()})
            }
            FormKind::Params => {
                let mut query = self
                    .table
                    .read(cx)
                    .value(cx)
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                for row in &mut query {
                    row["type"] = json!("query");
                }
                let mut paths = self
                    .paths
                    .read(cx)
                    .value(cx)
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                for row in &mut paths {
                    row["type"] = json!("path");
                }
                query.extend(paths);
                json!(query)
            }
            FormKind::Headers | FormKind::Variables | FormKind::Assertions => {
                self.table.read(cx).value(cx)
            }
            FormKind::Auth => {
                if self.mode == "none" {
                    Value::Null
                } else if self.mode == "inherit" {
                    json!("inherit")
                } else {
                    let mut value = if self.original["type"] == self.mode {
                        self.original.clone()
                    } else {
                        json!({})
                    };
                    value["type"] = json!(self.mode);
                    if self.mode == "oauth2" {
                        return oauth_form_value(
                            value,
                            self.fields.iter().map(|(key, _, field)| {
                                (key.as_str(), field.read(cx).value().to_string())
                            }),
                        );
                    }
                    for (key, _, field) in &self.fields {
                        value[key] = json!(field.read(cx).value().to_string());
                    }
                    value
                }
            }
            FormKind::Body => match self.mode.as_str() {
                "none" => Value::Null,
                "file" => {
                    let mut file = json!({"selected":true});
                    for (key, _, field) in &self.fields {
                        file[key] = json!(field.read(cx).value().to_string());
                    }
                    json!({"type":"file","data":[file]})
                }
                "form-urlencoded" | "multipart-form" => {
                    let mut data = self.table.read(cx).value(cx);
                    if self.mode == "multipart-form" {
                        for row in data.as_array_mut().unwrap() {
                            if row.get("type").is_none() {
                                row["type"] = json!("text");
                            }
                        }
                    }
                    json!({"type":self.mode,"data":data})
                }
                _ => json!({"type":self.mode,"data":self.editor.read(cx).value().to_string()}),
            },
            FormKind::Settings => {
                let mut value = if self.original.is_object() {
                    self.original.clone()
                } else {
                    json!({})
                };
                for (key, _, input) in &self.fields {
                    let text = input.read(cx).value().to_string();
                    value[key] = serde_json::from_str(&text).unwrap_or(json!(text));
                }
                value
            }
        }
    }
}
impl Render for RequestForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.editor_variable_preview.update(cx, |preview, cx| {
            preview.set_variables(
                (self.kind == FormKind::Body && self.mode == "json").then_some(&self.variables),
                cx,
            );
        });
        self.gql_variable_preview.update(cx, |preview, cx| {
            preview.set_variables(
                (self.kind == FormKind::Graphql).then_some(&self.variables),
                cx,
            );
        });
        let weak = cx.entity().downgrade();
        let content = match self.kind {
            FormKind::Graphql => v_resizable("graphql-editors")
                .child(
                    resizable_panel().child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .min_h_0()
                            .child(
                                panel_toolbar(cx)
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr("查询"))
                                    .child(div().flex_1())
                                    .child(format_button(
                                        "format-query",
                                        &self.editor,
                                        Language::Graphql,
                                    )),
                            )
                            .child(
                                div().flex_1().min_h_0().child(
                                    panel_editor(&self.editor).aria_label(tr("GraphQL 查询")),
                                ),
                            ),
                    ),
                )
                .child(
                    resizable_panel().child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .min_h_0()
                            .child(
                                panel_toolbar(cx)
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr("变量"))
                                    .child(div().flex_1())
                                    .child(format_button(
                                        "format-variables",
                                        &self.gql_variables,
                                        Language::Json,
                                    )),
                            )
                            .child(div().flex_1().min_h_0().child(
                                panel_editor(&self.gql_variables).aria_label(tr("GraphQL 变量")),
                            )),
                    ),
                )
                .into_any_element(),
            FormKind::Params => div()
                .flex()
                .flex_col()
                .gap_6()
                .p_4()
                .child(self.table.clone())
                .child(self.paths.clone())
                .into_any_element(),
            FormKind::Headers | FormKind::Variables | FormKind::Assertions => {
                div().p_4().child(self.table.clone()).into_any_element()
            }
            FormKind::Auth => div()
                .flex()
                .flex_col()
                .p_4()
                .gap_5()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(div().w_32().text_sm().child(tr("认证类型")))
                        .child(
                            Button::new("auth-type")
                                .icon(crate::icons::value_kind(&self.mode))
                                .outline()
                                .w_40()
                                .justify_between()
                                .label(mode_label(&self.mode).to_owned())
                                .dropdown_caret(true)
                                .dropdown_menu(move |menu, _, _| {
                                    let mut menu = menu;
                                    for mode in
                                        ["inherit", "none", "basic", "bearer", "apikey", "oauth2"]
                                    {
                                        let weak = weak.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(mode_label(mode))
                                                .icon(crate::icons::value_kind(mode))
                                                .on_click(move |_, window, cx| {
                                                    let _ = weak.update(cx, |this, cx| {
                                                        this.select_mode(mode, window, cx);
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }),
                        ),
                )
                .when(self.fields.is_empty(), |d| {
                    d.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.mode == "inherit" {
                                tr("认证信息继承自上级目录或集合。")
                            } else {
                                tr("此请求不使用认证。")
                            }),
                    )
                })
                .child(self.fields_view(window, cx))
                .into_any_element(),
            FormKind::Body => div()
                .flex()
                .flex_col()
                .size_full()
                .min_h_0()
                .child(
                    panel_toolbar(cx)
                        .child(
                            div()
                                .text_color(cx.theme().muted_foreground)
                                .child(tr("正文类型")),
                        )
                        .child(
                            Button::new("body-type")
                                .icon(crate::icons::value_kind(&self.mode))
                                .ghost()
                                .small()
                                .label(mode_label(&self.mode).to_owned())
                                .dropdown_caret(true)
                                .flex_shrink_0()
                                .dropdown_menu(move |menu, _, _| {
                                    let mut menu = menu;
                                    for mode in [
                                        "none",
                                        "json",
                                        "text",
                                        "xml",
                                        "sparql",
                                        "form-urlencoded",
                                        "multipart-form",
                                        "file",
                                    ] {
                                        let weak = weak.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(mode_label(mode))
                                                .icon(crate::icons::value_kind(mode))
                                                .on_click(move |_, window, cx| {
                                                    let _ = weak.update(cx, |this, cx| {
                                                        this.select_mode(mode, window, cx);
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }),
                        )
                        .child(div().flex_1())
                        .when(self.mode == "json", |bar| {
                            bar.child(format_button("format-body", &self.editor, Language::Json))
                        }),
                )
                .child(match self.mode.as_str() {
                    "none" => div()
                        .p_4()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr("此请求不发送正文。"))
                        .into_any_element(),
                    "file" => div()
                        .p_4()
                        .child(self.fields_view(window, cx))
                        .into_any_element(),
                    "form-urlencoded" | "multipart-form" => {
                        div().p_4().child(self.table.clone()).into_any_element()
                    }
                    _ => div()
                        .flex_1()
                        .min_h_0()
                        .child(panel_editor(&self.editor).aria_label(tr("请求正文")))
                        .into_any_element(),
                })
                .into_any_element(),
            FormKind::Settings | FormKind::BaseUrl => div()
                .p_4()
                .child(self.fields_view(window, cx))
                .into_any_element(),
        };
        let embedded_editor = self.kind == FormKind::Graphql
            || (self.kind == FormKind::Body
                && !matches!(
                    self.mode.as_str(),
                    "none" | "file" | "form-urlencoded" | "multipart-form"
                ));
        div()
            .id("request-form")
            .size_full()
            .min_w_0()
            .min_h_0()
            .when(embedded_editor, |d| d.overflow_hidden())
            .when(!embedded_editor, |d| d.overflow_y_scroll())
            .child(content)
    }
}

fn mode_label(mode: &str) -> &str {
    match mode {
        "true" => tr("开启"),
        "false" => tr("关闭"),
        "header" => tr("请求头"),
        "query" => tr("查询参数"),
        "inherit" => tr("继承上级"),
        "none" => tr("无"),
        "basic" => tr("基本认证"),
        "bearer" => tr("Bearer 令牌"),
        "apikey" => tr("API 密钥"),
        "oauth2" => "OAuth 2.0",
        "client_credentials" => tr("客户端凭据"),
        "authorization_code" => tr("授权码 + PKCE"),
        "body" => tr("请求正文"),
        "basic_auth_header" => "Basic Authorization",
        "json" => "JSON",
        "text" => tr("文本"),
        "xml" => "XML",
        "file" => tr("文件"),
        "form-urlencoded" => tr("URL 编码表单"),
        "multipart-form" => tr("多部分表单"),
        _ => mode,
    }
}

fn oauth_form_value<'a>(
    mut value: Value,
    fields: impl Iterator<Item = (&'a str, String)>,
) -> Value {
    for (key, text) in fields {
        match key {
            "clientId" | "clientSecret" => {
                if !value["credentials"].is_object() {
                    value["credentials"] = json!({});
                }
                value["credentials"][key] = json!(text);
            }
            "credentialPlacement" => {
                if !value["credentials"].is_object() {
                    value["credentials"] = json!({});
                }
                value["credentials"]["placement"] = json!(text);
            }
            _ => {
                if text.is_empty() {
                    value.as_object_mut().unwrap().remove(key);
                } else {
                    value[key] = json!(text);
                }
            }
        }
    }
    for key in ["clientId", "clientSecret", "credentialPlacement"] {
        value.as_object_mut().unwrap().remove(key);
    }
    if value["flow"] == "client_credentials" {
        for key in ["authorizationUrl", "callbackUrl", "state", "pkce"] {
            value.as_object_mut().unwrap().remove(key);
        }
        if let Some(extras) = value
            .get_mut("additionalParameters")
            .and_then(Value::as_object_mut)
        {
            extras.remove("authorizationRequest");
        }
    }
    value
}
#[cfg(test)]
mod oauth_tests {
    use super::oauth_form_value;
    use serde_json::json;
    #[test]
    fn oauth_form_matches_schema_and_keeps_nested_configuration() {
        for flow in ["client_credentials", "authorization_code"] {
            let value = oauth_form_value(
                json!({"type":"oauth2","tokenConfig":{"placement":{"header":"Authorization"}}}),
                [
                    ("flow", flow),
                    ("clientId", "client"),
                    ("clientSecret", "secret"),
                    ("credentialPlacement", "body"),
                    ("accessTokenUrl", "https://example.com/token"),
                    ("authorizationUrl", "https://example.com/authorize"),
                    ("callbackUrl", "http://127.0.0.1:0/callback"),
                ]
                .into_iter()
                .map(|(key, value)| (key, value.into())),
            );
            assert_eq!(value["credentials"]["clientId"], "client");
            assert!(value.get("clientId").is_none());
            let document = json!({"info":{"name":"OAuth","type":"http"},"http":{"method":"GET","url":"https://example.com","auth":value}});
            apifuck_core::Document::parse(
                "oauth.yml".into(),
                serde_yaml::to_string(&document).unwrap(),
            )
            .unwrap();
        }
    }
}
