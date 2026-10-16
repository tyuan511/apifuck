use crate::editor_tools::{LanguageBinding, format_button};
use crate::i18n::tr;
use crate::icons::AppIcon;
use crate::languages::{self, Language};
use crate::request_form::{FormKind, RequestForm};
use crate::surfaces::{panel_editor, panel_toolbar};
use anyhow::{Context as _, Result, ensure};
use apifuck_core::settings::RequestLayout;
use apifuck_core::{
    Collection, Document, DocumentKind, Event, NetworkSettings, RequestHead, Response, exchange,
    execute, prepare, websocket,
};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::{EditorState, Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    notification::Notification,
    resizable::{h_resizable, resizable_panel, v_resizable},
    tab::{Tab, TabBar},
    text::TextView,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum Section {
    BaseUrl,
    Source,
    Params,
    Headers,
    Auth,
    Body,
    Before,
    After,
    Message,
    Variables,
    Assertions,
    Tests,
    Docs,
    Settings,
}
impl Section {
    fn title(self) -> &'static str {
        match self {
            Self::BaseUrl => tr("Base URL"),
            Self::Source => tr("源码"),
            Self::Params => tr("参数"),
            Self::Headers => tr("请求头"),
            Self::Auth => tr("认证"),
            Self::Body => tr("正文"),
            Self::Before => tr("脚本"),
            Self::After => tr("脚本"),
            Self::Message => tr("消息"),
            Self::Variables => tr("变量"),
            Self::Assertions => tr("断言"),
            Self::Tests => tr("测试"),
            Self::Docs => tr("文档"),
            Self::Settings => tr("请求选项"),
        }
    }
    fn icon(self) -> AppIcon {
        match self {
            Self::BaseUrl => AppIcon::Globe,
            Self::Source | Self::Before | Self::After => AppIcon::Code,
            Self::Params | Self::Headers => AppIcon::SlidersHorizontal,
            Self::Auth => AppIcon::KeyRound,
            Self::Body => AppIcon::FileText,
            Self::Message => AppIcon::Send,
            Self::Variables => AppIcon::Braces,
            Self::Assertions => AppIcon::ShieldCheck,
            Self::Tests => AppIcon::CircleCheck,
            Self::Docs => AppIcon::BookOpen,
            Self::Settings => AppIcon::Settings,
        }
    }
    fn form(self) -> Option<FormKind> {
        match self {
            Self::BaseUrl => Some(FormKind::BaseUrl),
            Self::Params => Some(FormKind::Params),
            Self::Headers => Some(FormKind::Headers),
            Self::Auth => Some(FormKind::Auth),
            Self::Body => Some(FormKind::Body),
            Self::Variables => Some(FormKind::Variables),
            Self::Settings => Some(FormKind::Settings),
            Self::Assertions => Some(FormKind::Assertions),
            _ => None,
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Params => "params",
            Self::Headers => "headers",
            Self::Auth => "auth",
            Self::Body => "body",
            Self::Message => "message",
            _ => "",
        }
    }
}
enum Delivery {
    AuthorizeUrl(String),
    Authorized(Result<(), String>),
    Event(Event),
    Done(Result<Option<Response>, String>),
}
struct DocumentLayout {
    default: RequestLayout,
    local: Option<RequestLayout>,
}
impl DocumentLayout {
    fn new(default: RequestLayout) -> Self {
        Self {
            default,
            local: None,
        }
    }
    fn current(&self) -> RequestLayout {
        self.local.unwrap_or(self.default)
    }
    fn toggle(&mut self) {
        self.local = Some(match self.current() {
            RequestLayout::Vertical => RequestLayout::Horizontal,
            RequestLayout::Horizontal => RequestLayout::Vertical,
        });
    }
    fn restore(&mut self, state: &Value) {
        self.local = serde_json::from_value(state["requestLayout"].clone()).ok();
    }
}
pub struct DocumentTab {
    document: Document,
    variables: crate::variable_input::Variables,
    breadcrumb: String,
    source: Entity<EditorState>,
    section_editor: Entity<EditorState>,
    response_editor: Entity<EditorState>,
    sent_headers_editor: Entity<EditorState>,
    sent_requests: Vec<RequestHead>,
    sent_request_index: usize,
    show_sent_headers: bool,
    form: Entity<RequestForm>,
    form_source: Value,
    form_drafts: std::collections::BTreeMap<String, Value>,
    layout: DocumentLayout,
    docs_preview: bool,
    response_pretty: bool,
    timeline: Vec<String>,
    socket_messages: Vec<String>,
    stream_elapsed_ms: u128,
    method: Entity<InputState>,
    url: Entity<InputState>,
    message: Entity<InputState>,
    name: Entity<InputState>,
    original_name: String,
    original_method: String,
    original_url: String,
    section: Section,
    section_source: String,
    status: String,
    dirty: bool,
    response: Option<Response>,
    response_view: usize,
    stream: String,
    stream_headers: Vec<(String, String)>,
    stream_status: Option<u16>,
    received_bytes: usize,
    started_at: Option<std::time::Instant>,
    response_scroll: ScrollHandle,
    running: bool,
    connected: bool,
    cancel: Option<Arc<AtomicBool>>,
    commands: Option<tokio::sync::mpsc::UnboundedSender<websocket::Command>>,
    incoming: Option<mpsc::Receiver<Delivery>>,
    _subscriptions: Vec<Subscription>,
    _language_bindings: Vec<Entity<LanguageBinding>>,
    _poll: Task<()>,
}
impl DocumentTab {
    pub fn new(
        document: Document,
        draft: Option<String>,
        layout: RequestLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = draft.unwrap_or_else(|| document.source().to_owned());
        let parsed = Document::parse(document.path().to_owned(), text.clone())
            .unwrap_or_else(|_| document.clone());
        let key = parsed.protocol_key();
        let original_method = parsed.method().to_owned();
        let original_url = apifuck_core::url_editor::display(&parsed.value()[key]);
        let source = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .line_number(true)
                .default_value(text.clone())
        });
        let section_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("yaml")
                .line_number(true)
        });
        let response_editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
        });
        let method =
            cx.new(|cx| InputState::new(window, cx).default_value(original_method.clone()));
        let sent_headers_editor = cx.new(|cx| EditorState::new(window, cx).line_number(false));
        let url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr("请求 URL，或直接粘贴 cURL"))
                .default_value(original_url.clone())
        });
        let message = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr("发送 WebSocket 消息"))
                .default_value(
                    parsed.value()[key]["message"]["data"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned(),
                )
        });
        let original_name = parsed.name().to_owned();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(original_name.clone())
                .placeholder(tr("名称"))
        });
        let form = cx.new(|cx| RequestForm::new(window, cx));
        let mut subscriptions = vec![cx.subscribe_in(
            &form,
            window,
            |this, _, _: &crate::key_value_editor::Changed, window, cx| {
                this.dirty = true;
                if this.section == Section::Params
                    && let Ok(source) = this.draft(cx)
                    && let Ok(value) = serde_yaml::from_str::<Value>(&source)
                {
                    let url =
                        apifuck_core::url_editor::display(&value[this.document.protocol_key()]);
                    this.url.update(cx, |s, cx| s.set_value(url, window, cx));
                }
                cx.notify();
            },
        )];
        for input in [&source, &section_editor] {
            subscriptions.push(cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.dirty = true;
                    cx.notify();
                }
            }));
        }
        subscriptions.push(
            cx.subscribe_in(&url, window, |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if this.document.kind() == DocumentKind::Http
                        && exchange::is_curl(&input.read(cx).value())
                    {
                        if let Err(error) = this.import_curl(window, cx) {
                            this.set_error(error.to_string(), window, cx);
                        }
                        return;
                    }
                    if this.section == Section::Params
                        && let Ok(source) = this.draft(cx)
                        && let Ok(mut value) = serde_yaml::from_str::<Value>(&source)
                    {
                        let key = this.document.protocol_key();
                        apifuck_core::url_editor::apply_url(
                            &mut value[key],
                            &input.read(cx).value(),
                        );
                        let params = apifuck_core::url_editor::parameters(&value[key]);
                        this.form.update(cx, |form, cx| {
                            form.load(FormKind::Params, &params, window, cx)
                        });
                    }
                    this.dirty = true;
                    cx.notify();
                }
            }),
        );
        for input in [&method, &name] {
            subscriptions.push(cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.dirty = true;
                    cx.notify();
                }
            }));
        }
        subscriptions.push(cx.subscribe_in(
            &message,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.send_message(window, cx);
                }
                cx.notify();
            },
        ));
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| this.drain(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let breadcrumb = document
            .path()
            .parent()
            .map(|p| p.to_string_lossy().replace('/', " / "))
            .unwrap_or_default();
        let language_bindings = vec![
            LanguageBinding::attach(&source, window, cx),
            LanguageBinding::attach(&section_editor, window, cx),
        ];
        let mut this = Self {
            dirty: text != document.source(),
            document,
            variables: Default::default(),
            breadcrumb,
            source,
            section_editor,
            response_editor,
            sent_headers_editor,
            sent_requests: vec![],
            sent_request_index: 0,
            show_sent_headers: false,
            form,
            form_source: Value::Null,
            form_drafts: Default::default(),
            layout: DocumentLayout::new(layout),
            docs_preview: true,
            response_pretty: true,
            timeline: vec![],
            socket_messages: vec![],
            stream_elapsed_ms: 0,
            method,
            url,
            message,
            name,
            original_name,
            original_method,
            original_url,
            section: Section::Source,
            section_source: String::new(),
            status: tr("就绪").into(),
            response: None,
            response_view: 0,
            stream: String::new(),
            stream_headers: vec![],
            stream_status: None,
            received_bytes: 0,
            started_at: None,
            response_scroll: ScrollHandle::new(),
            running: false,
            connected: false,
            cancel: None,
            commands: None,
            incoming: None,
            _subscriptions: subscriptions,
            _language_bindings: language_bindings,
            _poll: poll,
        };
        if !this.document.is_request()
            && matches!(
                this.document.kind(),
                DocumentKind::Environment | DocumentKind::Collection | DocumentKind::Folder
            )
        {
            this.section = Section::Variables;
            let _ = this.load_section(&parsed, window, cx);
        }
        if this.document.is_request() {
            this.section = if this.document.kind() == DocumentKind::WebSocket {
                Section::Headers
            } else if this.document.kind() == DocumentKind::Graphql {
                Section::Body
            } else {
                Section::Params
            };
            let _ = this.load_section(&parsed, window, cx);
        }
        this
    }
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.url.update(cx, |input, cx| {
            input.set_placeholder(tr("请求 URL，或直接粘贴 cURL"), window, cx)
        });
        self.message.update(cx, |input, cx| {
            input.set_placeholder(tr("发送 WebSocket 消息"), window, cx)
        });
        self.name.update(cx, |input, cx| {
            input.set_placeholder(tr("名称"), window, cx)
        });
        self.form
            .update(cx, |form, cx| form.refresh_locale(window, cx));
        cx.notify();
    }
    pub fn set_variables(
        &mut self,
        variables: crate::variable_input::Variables,
        cx: &mut Context<Self>,
    ) {
        if self.variables != variables {
            self.variables = variables.clone();
            self.form
                .update(cx, |form, cx| form.set_variables(variables, cx));
            cx.notify();
        }
    }
    pub fn set_default_layout(&mut self, layout: RequestLayout, cx: &mut Context<Self>) {
        self.layout.default = layout;
        cx.notify();
    }
    pub fn request_header(&self, commands: Div, cx: &Context<Self>) -> Div {
        let vertical = self.layout.current() == RequestLayout::Vertical;
        let layout_label = if vertical {
            tr("切换为左右布局")
        } else {
            tr("切换为上下布局")
        };
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .min_w_0()
            .px_4()
            .pt_3()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().min_w_0().truncate().child(self.breadcrumb.clone()))
                    .child("/")
                    .child(
                        div().flex_1().min_w_0().max_w(rems(28.)).child(
                            Input::new(&self.name)
                                .small()
                                .bordered(false)
                                .aria_label(tr("请求名称")),
                        ),
                    ),
            )
            .child(commands)
            .child(
                Button::new("request-layout")
                    .small()
                    .ghost()
                    .label(tr("切换布局"))
                    .flex_shrink_0()
                    .icon(if vertical {
                        AppIcon::Columns2
                    } else {
                        AppIcon::Rows2
                    })
                    .accessibility_label(layout_label)
                    .tooltip(layout_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.layout.toggle();
                        cx.notify();
                    })),
            )
    }
    pub fn editor_snapshot(&self, cx: &App) -> Value {
        json!({"baseSource":self.document.source(), "dirty":self.dirty, "message":self.message.read(cx).value().to_string(),
            "requestLayout":self.layout.local,
            "formDraft":self.form.read(cx).snapshot(cx), "formDrafts": self.form_drafts,
            "source":self.raw_source(cx), "section":self.section, "sectionSource":self.section_source,
            "sectionText":self.section_editor.read(cx).value().to_string(),
            "method":self.method.read(cx).value().to_string(), "url":self.url.read(cx).value().to_string(),
            "originalMethod":self.original_method,"originalUrl":self.original_url,"formValue":self.form.read(cx).value(cx),"formSource":self.form_source,"name":self.name.read(cx).value().to_string(),"originalName":self.original_name})
    }
    pub fn restore_editor(&mut self, state: &Value, window: &mut Window, cx: &mut Context<Self>) {
        // Local presentation state survives even when a clean document changed externally.
        self.layout.restore(state);
        if let Some(message) = state["message"].as_str() {
            self.message.update(cx, |input, cx| {
                input.set_value(message.to_owned(), window, cx)
            });
        }
        // A clean UI snapshot must not roll back changes made to the file elsewhere.
        if !should_restore_document_draft(state, self.document.source()) {
            cx.notify();
            return;
        }
        if let Some(source) = state["source"].as_str() {
            self.source
                .update(cx, |s, cx| s.set_value(source.to_owned(), window, cx));
        }
        if let Ok(section) = serde_json::from_value(state["section"].clone()) {
            self.section = if self.document.kind() == DocumentKind::WebSocket
                && matches!(
                    section,
                    Section::Message | Section::Before | Section::After | Section::Body
                ) {
                Section::Headers
            } else {
                section
            };
        }
        self.section_source = state["sectionSource"].as_str().unwrap_or("").into();
        self.section_editor.update(cx, |s, cx| {
            s.set_value(
                state["sectionText"].as_str().unwrap_or("").to_owned(),
                window,
                cx,
            )
        });
        self.method.update(cx, |s, cx| {
            s.set_value(
                state["method"].as_str().unwrap_or("").to_owned(),
                window,
                cx,
            )
        });
        self.url.update(cx, |s, cx| {
            s.set_value(state["url"].as_str().unwrap_or("").to_owned(), window, cx)
        });
        self.original_url = state["originalUrl"].as_str().unwrap_or("").into();
        self.original_method = state["originalMethod"].as_str().unwrap_or("").into();
        if let Some(kind) = self.section.form().map(|kind| {
            if self.section == Section::Body && self.document.kind() == DocumentKind::Graphql {
                FormKind::Graphql
            } else {
                kind
            }
        }) && let Some(value) = state.get("formValue")
        {
            self.form
                .update(cx, |f, cx| f.load(kind, value, window, cx));
            self.form_source = state["formSource"].clone();
        }
        if let Some(name) = state["name"].as_str() {
            self.name
                .update(cx, |s, cx| s.set_value(name.to_owned(), window, cx));
            self.original_name = state["originalName"]
                .as_str()
                .unwrap_or(self.document.name())
                .into();
        }
        if let Some(raw) = state.get("formDraft") {
            self.form
                .update(cx, |form, cx| form.restore(raw, window, cx));
        }
        self.form_drafts = serde_json::from_value(state["formDrafts"].clone()).unwrap_or_default();
        self.dirty = state["dirty"].as_bool().unwrap_or(true);
        cx.notify();
    }
    fn is_scope(&self) -> bool {
        matches!(
            self.document.kind(),
            DocumentKind::Collection | DocumentKind::Folder
        )
    }
    fn editing_key(&self) -> &'static str {
        if self.is_scope() {
            "request"
        } else {
            self.document.protocol_key()
        }
    }
    fn script_key(&self) -> &'static str {
        if self.is_scope() {
            "request"
        } else {
            "runtime"
        }
    }
    pub fn set_breadcrumb(&mut self, value: String, cx: &mut Context<Self>) {
        self.breadcrumb = value;
        cx.notify();
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn reordered(
        &mut self,
        document: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if document.value()["info"]["seq"] == self.document.value()["info"]["seq"] {
            return Ok(());
        }
        let source = self.raw_source(cx);
        let mut value: Value = serde_yaml::from_str(&source)?;
        value["info"]["seq"] = document.value()["info"]["seq"].clone();
        let text = apifuck_core::editing::update(&source, &value)?;
        self.source
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.document = document;
        cx.notify();
        Ok(())
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn name(&self, cx: &App) -> String {
        let name = self.name.read(cx).value();
        if name.trim().is_empty() {
            self.document.name().to_owned()
        } else {
            name.to_string()
        }
    }
    pub fn focus_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.name.update(cx, |name, cx| {
            name.focus(window, cx);
            name.select_all(window, cx);
        });
    }
    pub fn raw_source(&self, cx: &App) -> String {
        self.source.read(cx).value().to_string()
    }
    pub fn draft(&self, cx: &App) -> Result<String> {
        let source = self.source.read(cx).value().to_string();
        let mut value: Value = serde_yaml::from_str(&source)?;
        let mut changed = false;
        let key = self.editing_key();
        let name = self.name.read(cx).value().to_string();
        if name != self.original_name {
            if self.document.kind() == DocumentKind::Environment {
                value["name"] = json!(name);
            } else {
                value["info"]["name"] = json!(name);
            }
            changed = true;
        }
        if self.document.is_request() {
            let url = self.url.read(cx).value().to_string();
            let method = self.method.read(cx).value().to_string();
            if url != self.original_url {
                apifuck_core::url_editor::apply_url(&mut value[key], &url);
                changed = true;
            }
            if method != self.original_method && key != "websocket" {
                value[key]["method"] = json!(method);
                changed = true;
            }
        }
        if self.section.form().is_some() {
            let data = self.form.read(cx).value(cx);
            if data != self.form_source {
                changed = true;
                match self.section {
                    Section::BaseUrl => {
                        let scope = self.script_key();
                        if !value[scope].is_object() {
                            value[scope] = json!({});
                        }
                        apifuck_core::url_editor::set_base_url(
                            &mut value[scope]["variables"],
                            data.as_str().unwrap_or(""),
                        );
                    }
                    Section::Assertions => {
                        if !value["runtime"].is_object() {
                            value["runtime"] = json!({});
                        }
                        value["runtime"]["assertions"] = data;
                    }
                    Section::Variables => {
                        if self.document.kind() == DocumentKind::Environment {
                            value["variables"] = data;
                        } else {
                            let scope = self.script_key();
                            if !value[scope].is_object() {
                                value[scope] = json!({});
                            }
                            value[scope]["variables"] = data;
                        }
                    }
                    Section::Settings => value["settings"] = data,
                    Section::Params => {
                        apifuck_core::url_editor::apply_parameters(&mut value[key], data)
                    }
                    _ => {
                        let field = self.section.key();
                        if data.is_null() {
                            value[key]
                                .as_object_mut()
                                .context(tr("请求配置无效"))?
                                .remove(field);
                        } else {
                            value[key][field] = data;
                        }
                    }
                }
            }
        } else if self.section != Section::Source {
            let text = self.section_editor.read(cx).value().to_string();
            if text != self.section_source {
                changed = true;
                match self.section {
                    Section::Before | Section::After | Section::Tests => {
                        let phase = if self.section == Section::Before {
                            "before-request"
                        } else if self.section == Section::Tests {
                            "tests"
                        } else {
                            "after-response"
                        };
                        let owner = self.script_key();
                        if !value[owner].is_object() {
                            value[owner] = json!({});
                        }
                        if !value[owner]["scripts"].is_array() {
                            value[owner]["scripts"] = json!([]);
                        }
                        let hooks = value[owner]["scripts"].as_array_mut().unwrap();
                        hooks.retain(|s| s["type"] != phase);
                        if !text.trim().is_empty() {
                            hooks.push(json!({"type":phase,"code":text}));
                        }
                    }
                    Section::Docs => {
                        value["docs"] = json!(text);
                    }
                    Section::Assertions => {
                        if !value["runtime"].is_object() {
                            value["runtime"] = json!({});
                        }
                        value["runtime"]["assertions"] =
                            serde_yaml::from_str(&text).context(tr("断言格式无效"))?;
                    }
                    section => {
                        let field = section.key();
                        if text.trim().is_empty() {
                            value[key]
                                .as_object_mut()
                                .context(tr("请求配置无效"))?
                                .remove(field);
                        } else {
                            value[key][field] =
                                serde_yaml::from_str(&text).context(tr("当前配置的 YAML 无效"))?;
                        }
                    }
                }
            }
        }
        if changed {
            apifuck_core::editing::update(&source, &value)
        } else {
            Ok(source)
        }
    }
    fn commit_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<Document> {
        let raw_form = self.section.form().map(|_| self.form.read(cx).snapshot(cx));
        let text = self.draft(cx)?;
        let mut value: Value = serde_yaml::from_str(&text)?;
        if self.document.is_request() {
            let key = self.document.protocol_key();
            value[key]["url"] = json!(apifuck_core::url_editor::normalize(
                value[key]["url"].as_str().unwrap_or("")
            ));
        }
        let text = apifuck_core::editing::update(&text, &value)?;
        let document = Document::parse(self.document.path().to_owned(), text.clone())?;
        let key = document.protocol_key();
        self.original_name = document.name().into();
        if self.name.read(cx).value().as_ref() != self.original_name {
            self.name.update(cx, |s, cx| {
                s.set_value(self.original_name.clone(), window, cx)
            });
        }
        self.original_method = document.method().to_owned();
        self.original_url = apifuck_core::url_editor::display(&document.value()[key]);
        if self.source.read(cx).value().as_ref() != text {
            self.source
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
        if self.method.read(cx).value().as_ref() != self.original_method {
            self.method.update(cx, |state, cx| {
                state.set_value(self.original_method.clone(), window, cx)
            });
        }
        if self.url.read(cx).value().as_ref() != self.original_url {
            self.url.update(cx, |state, cx| {
                state.set_value(self.original_url.clone(), window, cx)
            });
        }
        self.load_section(&document, window, cx)?;
        if let Some(raw) = raw_form
            && raw["value"] == self.form.read(cx).value(cx)
        {
            self.form
                .update(cx, |form, cx| form.restore(&raw, window, cx));
        }
        Ok(document)
    }
    fn load_section(
        &mut self,
        document: &Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if self.section == Section::Source {
            return Ok(());
        }
        let key = self.editing_key();
        if let Some(kind) = self.section.form().map(|kind| {
            if self.section == Section::Body && self.document.kind() == DocumentKind::Graphql {
                FormKind::Graphql
            } else {
                kind
            }
        }) {
            let parameters = apifuck_core::url_editor::parameters(&document.value()[key]);
            let value = match self.section {
                Section::BaseUrl => &json!(apifuck_core::url_editor::base_url(
                    &document.value()[self.script_key()]["variables"]
                )),
                Section::Params => &parameters,
                Section::Assertions => &document.value()["runtime"]["assertions"],
                Section::Variables => {
                    if document.kind() == DocumentKind::Environment {
                        &document.value()["variables"]
                    } else {
                        &document.value()[self.script_key()]["variables"]
                    }
                }
                Section::Settings => &document.value()["settings"],
                section => &document.value()[key][section.key()],
            };
            if self.form_source != *value || self.section_source != self.section.title() {
                self.form
                    .update(cx, |form, cx| form.load(kind, value, window, cx));
            }
            self.form_source = self.form.read(cx).value(cx);
            self.section_source = self.section.title().into();
            return Ok(());
        }
        let text = match self.section {
            Section::Before | Section::After | Section::Tests => {
                let phase = if self.section == Section::Before {
                    "before-request"
                } else if self.section == Section::Tests {
                    "tests"
                } else {
                    "after-response"
                };
                document.value()[self.script_key()]["scripts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|s| s["type"] == phase)
                    .filter_map(|s| s["code"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Section::Docs => document.value()["docs"].as_str().unwrap_or("").to_owned(),
            Section::Assertions => serde_yaml::to_string(
                &document.value()["runtime"]["assertions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            )?,
            section => {
                let value = &document.value()[key][section.key()];
                if value.is_null() {
                    String::new()
                } else {
                    serde_yaml::to_string(value)?
                }
            }
        };
        self.section_editor.update(cx, |state, cx| {
            crate::script_completion::configure(
                state,
                match self.section {
                    Section::Before => Some(false),
                    Section::After | Section::Tests => Some(true),
                    _ => None,
                },
                cx,
            );
            state.set_highlighter(
                if matches!(
                    self.section,
                    Section::Before | Section::After | Section::Tests
                ) {
                    "javascript"
                } else if self.section == Section::Docs {
                    "markdown"
                } else {
                    "yaml"
                },
                cx,
            )
        });
        self.section_source = text.clone();
        if self.section_editor.read(cx).value().as_ref() != text {
            self.section_editor
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
        Ok(())
    }
    fn select_section(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.section.form().is_some() {
            let mut raw = self.form.read(cx).snapshot(cx);
            // Mode history belongs to the whole editor, not an older section snapshot.
            raw.as_object_mut().unwrap().remove("modes");
            self.form_drafts.insert(
                serde_json::to_string(&self.section).unwrap(),
                json!({
                    "value": self.form.read(cx).value(cx), "raw": raw
                }),
            );
        }
        match self.commit_editors(window, cx) {
            Ok(document) => {
                self.section = section;
                if let Err(error) = self.load_section(&document, window, cx) {
                    self.set_error(error.to_string(), window, cx);
                }
                if self.section.form().is_some()
                    && let Some(saved) = self
                        .form_drafts
                        .get(&serde_json::to_string(&section).unwrap())
                    && saved["value"] == self.form.read(cx).value(cx)
                {
                    self.form
                        .update(cx, |form, cx| form.restore(&saved["raw"], window, cx));
                }
            }
            Err(e) => self.set_error(format!("{e:#}"), window, cx),
        };
        cx.notify();
    }
    pub fn save(
        &mut self,
        collection: &Collection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let document = self.commit_editors(window, cx)?;
        ensure!(
            !document.is_request() || !document.name().trim().is_empty(),
            "{}",
            tr("请填写请求名称")
        );
        self.document = collection.save(&self.document, document.source())?;
        self.dirty = false;
        cx.notify();
        Ok(())
    }
    pub fn reload(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.source.update(cx, |s, cx| {
            s.set_value(document.source().to_owned(), window, cx)
        });
        self.document = document;
        self.section = Section::Source;
        self.original_url = self.url.read(cx).value().to_string();
        self.original_method = self.method.read(cx).value().to_string();
        let _ = self.commit_editors(window, cx);
        self.dirty = false;
        cx.notify();
    }
    pub fn authorize_oauth(
        &mut self,
        collection: &Collection,
        environment: Option<&str>,
        network: NetworkSettings,
        runtime: &Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(!self.running, "{}", tr("请求正在运行"));
        let document = self.commit_editors(window, cx)?;
        let execution = prepare(collection, &document, environment, network)?;
        self.cancel = Some(execution.cancellation());
        self.running = true;
        self.status = tr("正在获取 OAuth 令牌…").into();
        let (delivery, incoming) = mpsc::channel();
        self.incoming = Some(incoming);
        let (urls, mut urls_rx) = tokio::sync::mpsc::unbounded_channel();
        let forward = delivery.clone();
        runtime.spawn(async move {
            while let Some(url) = urls_rx.recv().await {
                let _ = forward.send(Delivery::AuthorizeUrl(url));
            }
        });
        runtime.spawn(async move {
            let result = apifuck_core::oauth::authorize(execution, urls)
                .await
                .map_err(|error| format!("{error:#}"));
            let _ = delivery.send(Delivery::Authorized(result));
        });
        cx.notify();
        Ok(())
    }
    pub fn send(
        &mut self,
        collection: &Collection,
        env: Option<&str>,
        network: NetworkSettings,
        runtime: &Arc<tokio::runtime::Runtime>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        ensure!(!self.running, "{}", tr("请求正在运行"));
        self.sent_requests.clear();
        self.sent_request_index = 0;
        self.show_sent_headers = false;
        self.sent_headers_editor
            .update(cx, |editor, cx| editor.set_value("", window, cx));
        cx.notify();
        let document = self.commit_editors(window, cx)?;
        let execution = prepare(collection, &document, env, network)?;
        self.cancel = Some(execution.cancellation());
        self.stream.clear();
        self.stream_status = None;
        self.received_bytes = 0;
        self.started_at = Some(std::time::Instant::now());
        self.stream_headers.clear();
        self.timeline.clear();
        self.socket_messages.clear();
        self.stream_elapsed_ms = 0;
        self.timeline.push(format!(
            "{} {}",
            document.method(),
            execution.request()["url"].as_str().unwrap_or("")
        ));
        self.response = None;
        self.running = true;
        self.connected = false;
        self.status = tr("连接中…").into();
        self.response_editor
            .update(cx, |state, cx| state.set_value("", window, cx));
        let (delivery, incoming) = mpsc::channel();
        self.incoming = Some(incoming);
        let (events, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let forward = delivery.clone();
        runtime.spawn(async move {
            while let Some(event) = rx.recv().await {
                if forward.send(Delivery::Event(event)).is_err() {
                    break;
                }
            }
        });
        if execution.is_websocket() {
            let (tx, commands) = tokio::sync::mpsc::unbounded_channel();
            self.commands = Some(tx);
            runtime.spawn(async move {
                let result = websocket::connect(execution, commands, events)
                    .await
                    .map(|_| None)
                    .map_err(|e| format!("{e:#}"));
                let _ = delivery.send(Delivery::Done(result));
            });
        } else {
            runtime.spawn(async move {
                let result = execute(execution, Some(events))
                    .await
                    .map(Some)
                    .map_err(|e| format!("{e:#}"));
                let _ = delivery.send(Delivery::Done(result));
            });
        }
        cx.notify();
        Ok(())
    }
    fn update_sent_headers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .sent_requests
            .get(self.sent_request_index)
            .map(|request| {
                let mut lines = vec![format!("{} {}", request.method, request.url), String::new()];
                lines.extend(
                    request
                        .headers
                        .iter()
                        .map(|(name, value)| format!("{name}: {value}")),
                );
                lines.join("\n")
            })
            .unwrap_or_default();
        self.sent_headers_editor
            .update(cx, |editor, cx| editor.set_value(text, window, cx));
    }
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(commands) = &self.commands {
            let _ = commands.send(websocket::Command::Close);
        }
        self.status = tr("正在停止…").into();
        cx.notify();
    }
    fn drain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let messages: Vec<_> = self
            .incoming
            .as_ref()
            .map(|rx| rx.try_iter().take(256).collect())
            .unwrap_or_default();
        if messages.is_empty() {
            return;
        }
        for message in messages {
            match message {
                Delivery::AuthorizeUrl(url) => {
                    if self.running
                        && self
                            .cancel
                            .as_ref()
                            .is_some_and(|cancel| !cancel.load(Ordering::Relaxed))
                    {
                        cx.open_url(&url);
                    }
                }
                Delivery::Authorized(result) => {
                    self.running = false;
                    self.cancel = None;
                    match result {
                        Ok(()) => {
                            self.status = tr("OAuth 令牌已保存").into();
                            crate::feedback::show(
                                Notification::success(tr("OAuth 令牌已保存")),
                                window,
                                cx,
                            );
                        }
                        Err(error) => {
                            self.status = tr("OAuth 授权失败").into();
                            crate::feedback::show(Notification::error(error), window, cx);
                        }
                    }
                }
                Delivery::Event(Event::Request { request }) => {
                    self.sent_requests.push(request);
                    self.sent_request_index = self.sent_requests.len() - 1;
                    self.show_sent_headers = true;
                    self.update_sent_headers(window, cx);
                }
                Delivery::Event(Event::Started { status, headers }) => {
                    self.stream_headers = headers;
                    self.stream_status = Some(status);
                    self.timeline.push(crate::i18n::format(
                        tr("收到 HTTP {status}"),
                        &[("status", format!("{status}"))],
                    ));
                    self.status = crate::i18n::format(
                        tr("HTTP {status} · 接收中…"),
                        &[("status", format!("{status}"))],
                    )
                }
                Delivery::Event(Event::Chunk {
                    text,
                    received_bytes,
                }) => {
                    self.received_bytes = received_bytes;
                    if self.stream.len() < 2 * 1024 * 1024 {
                        self.stream.push_str(&text);
                    }
                    self.status = crate::i18n::format(
                        tr("已接收 {received_bytes} 字节"),
                        &[("received_bytes", format!("{received_bytes}"))],
                    );
                }
                Delivery::Event(Event::Connected) => {
                    self.connected = true;
                    self.status = tr("WebSocket 已连接").into();
                    self.record_socket(tr("系统"), tr("WebSocket 已连接"));
                }
                Delivery::Event(Event::Message { direction, text }) => {
                    self.received_bytes += text.len();
                    self.record_socket(
                        if direction == "inbound" {
                            tr("响应")
                        } else {
                            tr("请求")
                        },
                        &text,
                    );
                }
                Delivery::Event(Event::Error { message }) => {
                    if self.document.kind() == DocumentKind::WebSocket {
                        self.record_socket(tr("错误"), &message);
                    }
                    crate::feedback::show(Notification::error(message), window, cx);
                }
                Delivery::Event(Event::Finished) => {
                    if self.document.kind() == DocumentKind::WebSocket {
                        self.record_socket(tr("系统"), tr("连接已关闭"));
                    }
                }
                Delivery::Done(result) => {
                    self.stream_elapsed_ms = self
                        .started_at
                        .map(|t| t.elapsed().as_millis())
                        .unwrap_or(0);
                    self.timeline.push(match &result {
                        Ok(Some(response)) => crate::i18n::format(
                            tr("已完成 · {} 毫秒 · {} 字节"),
                            &[
                                ("0", format!("{}", response.elapsed_ms)),
                                ("1", format!("{}", response.bytes)),
                            ],
                        ),
                        Ok(None) => tr("连接已关闭").into(),
                        Err(error) => crate::i18n::format(
                            tr("失败：{error}"),
                            &[("error", error.to_string())],
                        ),
                    });
                    self.running = false;
                    self.connected = false;
                    self.commands = None;
                    self.cancel = None;
                    match result {
                        Ok(Some(response)) => {
                            self.status = crate::i18n::format(
                                tr("HTTP {} · {} ms · {} 字节"),
                                &[
                                    ("0", format!("{}", response.status)),
                                    ("1", format!("{}", response.elapsed_ms)),
                                    ("2", format!("{}", response.bytes)),
                                ],
                            );
                            self.response = Some(response);
                        }
                        Ok(None) => self.status = tr("连接已关闭").into(),
                        Err(e) => {
                            self.status = tr("请求失败").into();
                            if self.document.kind() == DocumentKind::WebSocket {
                                self.record_socket(tr("错误"), &e);
                            }
                            self.set_error(e, window, cx);
                        }
                    }
                }
            }
        }
        self.update_response(window, cx);
        self.response_scroll.scroll_to_bottom();
        cx.notify();
    }
    fn record_socket(&mut self, kind: &str, text: &str) {
        if self.stream.len() > 2 * 1024 * 1024 {
            self.stream.clear();
            self.socket_messages.clear();
            self.timeline.clear();
        }
        let elapsed = self
            .started_at
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.);
        let message = format!("+{elapsed:.3}s {kind}: {text}");
        self.stream.push_str(&message);
        self.stream.push('\n');
        self.timeline.push(message.clone());
        self.socket_messages.push(message);
    }
    fn update_response(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = if self.response_view == 4 {
            self.response_info()
                .into_iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(response) = &self.response {
            match self.response_view {
                1 => response
                    .headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\n"))
                    .collect(),
                2 => self.timeline.join("\n"),
                3 => response
                    .tests
                    .iter()
                    .map(|test| {
                        format!(
                            "{} {}{}",
                            if test.passed {
                                tr("通过")
                            } else {
                                tr("失败")
                            },
                            test.name,
                            test.error
                                .as_ref()
                                .map(|e| format!("\n  {e}"))
                                .unwrap_or_default()
                        )
                    })
                    .chain(response.logs.iter().cloned())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ if !self.response_pretty => response.body.clone(),
                _ => languages::format(Language::Json, &response.body)
                    .unwrap_or_else(|_| response.body.clone()),
            }
        } else {
            match self.response_view {
                1 => self
                    .stream_headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
                2 => self.timeline.join("\n"),
                3 => String::new(),
                _ => self.stream.clone(),
            }
        };
        self.response_editor
            .update(cx, |s, cx| s.set_value(text, window, cx));
    }
    fn response_headers(&self) -> &[(String, String)] {
        self.response
            .as_ref()
            .map(|r| r.headers.as_slice())
            .unwrap_or(&self.stream_headers)
    }
    fn is_event_stream(&self) -> bool {
        crate::response_view::is_event_stream(self.response_headers())
    }
    fn response_info(&self) -> Vec<(String, String)> {
        let response = self.response.as_ref();
        vec![
            (
                tr("状态码").into(),
                response
                    .map(|r| r.status)
                    .or(self.stream_status)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "—".into()),
            ),
            (
                tr("耗时").into(),
                format!(
                    "{} ms",
                    response
                        .map(|r| r.elapsed_ms)
                        .unwrap_or_else(|| if self.running {
                            self.started_at
                                .map(|t| t.elapsed().as_millis())
                                .unwrap_or(0)
                        } else {
                            self.stream_elapsed_ms
                        })
                ),
            ),
            (
                tr("大小").into(),
                format!(
                    "{} B",
                    response.map(|r| r.bytes).unwrap_or(self.received_bytes)
                ),
            ),
            (
                tr("内容类型").into(),
                self.response_headers()
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "—".into()),
            ),
            (
                tr("响应头数量").into(),
                self.response_headers().len().to_string(),
            ),
            (tr("请求状态").into(), self.status.clone()),
            (
                tr("视图类型").into(),
                if self.is_event_stream() {
                    tr("事件流")
                } else if self.document.kind() == DocumentKind::WebSocket {
                    "WebSocket"
                } else {
                    tr("响应正文")
                }
                .into(),
            ),
        ]
    }
    fn response_rows(&self, rows: Vec<(String, String)>, cx: &App) -> impl IntoElement {
        div()
            .id("response-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .children(rows.into_iter().map(|(key, value)| {
                div()
                    .flex()
                    .min_w_0()
                    .p_3()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().w(rems(12.)).flex_shrink_0().child(key))
                    .child(div().flex_1().min_w_0().child(value))
            }))
    }
    pub fn export(
        &mut self,
        collection: &Collection,
        env: Option<&str>,
        network: NetworkSettings,
        language: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String> {
        let document = self.commit_editors(window, cx)?;
        exchange::export(&prepare(collection, &document, env, network)?, language)
    }
    pub fn import_curl(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Result<()> {
        ensure!(
            self.document.kind() == DocumentKind::Http,
            "cURL 导入仅支持 HTTP 请求"
        );
        let command = self.url.read(cx).value().to_string();
        self.import_curl_source(&command, window, cx)
    }
    fn import_curl_source(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let text = exchange::import_curl_into(&self.draft(cx)?, command, &self.name(cx))?;
        let parsed = Document::parse(self.document.path().to_owned(), text.clone())?;
        self.original_method = parsed.method().into();
        self.method.update(cx, |s, cx| {
            s.set_value(self.original_method.clone(), window, cx)
        });
        self.original_name = parsed.name().into();
        self.source
            .update(cx, |s, cx| s.set_value(text, window, cx));
        self.original_url = self.url.read(cx).value().to_string();
        self.section = Section::Source;
        self.commit_editors(window, cx)?;
        self.form_drafts.clear();
        self.section = Section::Params;
        self.load_section(&parsed, window, cx)?;
        self.dirty = true;
        cx.notify();
        Ok(())
    }
    fn paste_url(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let before = self.url.read(cx).value();
        let cursor = self.url.read(cx).cursor();
        let clipboard = cx.read_from_clipboard_async();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Some(clipboard)) = clipboard.await else {
                return;
            };
            let Some(text) = clipboard.text() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if this.url.read(cx).value() != before || this.url.read(cx).cursor() != cursor {
                    return;
                }
                if exchange::is_curl(&text) && this.document.kind() == DocumentKind::Http {
                    if let Err(error) = this.import_curl_source(&text, window, cx) {
                        this.url
                            .update(cx, |input, cx| input.set_value(text, window, cx));
                        this.dirty = true;
                        this.set_error(format!("{error:#}"), window, cx);
                    }
                } else {
                    this.url.update(cx, |input, cx| {
                        input.replace(text.replace(['\r', '\n'], ""), window, cx)
                    });
                }
            });
        })
        .detach();
    }
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        if let Some(tx) = &self.commands {
            match tx.send(websocket::Command::Send(
                self.message.read(cx).value().to_string(),
            )) {
                Ok(()) => self.message.update(cx, |s, cx| s.set_value("", window, cx)),
                Err(error) => self.set_error(error.to_string(), window, cx),
            }
        }
        cx.notify();
    }
    pub fn set_error(&mut self, error: String, window: &mut Window, cx: &mut Context<Self>) {
        crate::feedback::show(Notification::error(error), window, cx);
        cx.notify();
    }
}
impl Drop for DocumentTab {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}
impl Render for DocumentTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_request = self.document.is_request();
        let is_ws = self.document.kind() == DocumentKind::WebSocket;
        let sections = if !is_request {
            if self.document.kind() == DocumentKind::Environment {
                vec![Section::Variables, Section::Source]
            } else if self.is_scope() {
                vec![
                    Section::BaseUrl,
                    Section::Headers,
                    Section::Auth,
                    Section::Variables,
                    Section::Before,
                    Section::Tests,
                    Section::Docs,
                    Section::Source,
                ]
            } else {
                vec![Section::Source]
            }
        } else if is_ws {
            vec![
                Section::Headers,
                Section::Auth,
                Section::BaseUrl,
                Section::Variables,
                Section::Source,
            ]
        } else {
            vec![
                Section::Params,
                Section::BaseUrl,
                Section::Auth,
                Section::Headers,
                Section::Body,
                Section::Variables,
                Section::Before,
                Section::Assertions,
                Section::Tests,
                Section::Docs,
                Section::Settings,
                Section::Source,
            ]
        };
        let sections = if self.document.kind() == DocumentKind::Graphql {
            sections
                .into_iter()
                .filter(|s| *s != Section::Params)
                .collect::<Vec<_>>()
        } else {
            sections
        };
        let selected = sections
            .iter()
            .position(|s| {
                *s == self.section || (*s == Section::Before && self.section == Section::After)
            })
            .unwrap_or(0);
        let weak = cx.entity().downgrade();
        let editor = if self.section == Section::Source {
            &self.source
        } else {
            &self.section_editor
        };
        let request_pane = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                TabBar::new("request-sections")
                    .underline()
                    .px_4()
                    .gap_4()
                    .flex_shrink_0()
                    .bg(cx.theme().background)
                    .selected_index(selected)
                    .menu(true)
                    .children(sections.iter().map(|s| {
                        Tab::new().prefix(Icon::new(s.icon()).size_3p5()).label(
                            if *s == Section::Body && self.document.kind() == DocumentKind::Graphql
                            {
                                tr("查询")
                            } else {
                                s.title()
                            },
                        )
                    }))
                    .on_click(cx.listener(move |this, ix, window, cx| {
                        this.select_section(sections[*ix], window, cx)
                    })),
            )
            .when(
                matches!(self.section, Section::Before | Section::After),
                |d| {
                    d.child(
                        panel_toolbar(cx)
                            .child(
                                TabBar::new("script-phases")
                                    .pill()
                                    .small()
                                    .selected_index(usize::from(self.section == Section::After))
                                    .children([
                                        Tab::new().label(tr("请求前")),
                                        Tab::new().label(tr("响应后")),
                                    ])
                                    .on_click(cx.listener(|this, ix, window, cx| {
                                        this.select_section(
                                            if *ix == 0 {
                                                Section::Before
                                            } else {
                                                Section::After
                                            },
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                            .child(div().flex_1())
                            .child(format_button(
                                "format-script",
                                &self.section_editor,
                                Language::JavaScript,
                            )),
                    )
                },
            )
            .when(
                matches!(
                    self.section,
                    Section::Source | Section::Tests | Section::Message
                ),
                |d| {
                    let language = if self.section == Section::Tests {
                        Language::JavaScript
                    } else {
                        Language::Yaml
                    };
                    d.child(panel_toolbar(cx).child(div().flex_1()).child(format_button(
                        "format-section",
                        editor,
                        language,
                    )))
                },
            )
            .when(
                self.section == Section::Headers && !self.sent_requests.is_empty(),
                |d| {
                    let headers_weak = cx.entity().downgrade();
                    d.child(
                        panel_toolbar(cx)
                            .child(
                                TabBar::new("request-headers-mode")
                                    .pill()
                                    .small()
                                    .selected_index(usize::from(self.show_sent_headers))
                                    .children([
                                        Tab::new().label(tr("编辑配置")),
                                        Tab::new().label(tr("实际发送")),
                                    ])
                                    .on_click(cx.listener(|this, index, _, cx| {
                                        this.show_sent_headers = *index == 1;
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1())
                            .when(
                                self.show_sent_headers && self.sent_requests.len() > 1,
                                |bar| {
                                    bar.child(
                                        Button::new("sent-request-hop")
                                            .small()
                                            .ghost()
                                            .label(format!("#{}", self.sent_request_index + 1))
                                            .tooltip(tr("选择重定向请求"))
                                            .dropdown_caret(true)
                                            .dropdown_menu(move |mut menu, _, cx| {
                                                let Some(tab) = headers_weak.upgrade() else {
                                                    return menu;
                                                };
                                                for (index, request) in
                                                    tab.read(cx).sent_requests.iter().enumerate()
                                                {
                                                    let handle = headers_weak.clone();
                                                    menu = menu.item(
                                                        PopupMenuItem::new(format!(
                                                            "#{} {} {}",
                                                            index + 1,
                                                            request.method,
                                                            request.url
                                                        ))
                                                        .checked(
                                                            index
                                                                == tab.read(cx).sent_request_index,
                                                        )
                                                        .on_click(move |_, window, cx| {
                                                            let _ =
                                                                handle.update(cx, |this, cx| {
                                                                    this.sent_request_index = index;
                                                                    this.update_sent_headers(
                                                                        window, cx,
                                                                    );
                                                                    cx.notify();
                                                                });
                                                        }),
                                                    );
                                                }
                                                menu
                                            }),
                                    )
                                },
                            )
                            .when(self.show_sent_headers, |bar| {
                                bar.child(
                                    Button::new("copy-sent-headers")
                                        .ghost()
                                        .small()
                                        .icon(AppIcon::Copy)
                                        .label(tr("复制"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            crate::feedback::copy(
                                                this.sent_headers_editor
                                                    .read(cx)
                                                    .value()
                                                    .to_string(),
                                                tr("已复制请求头").to_owned(),
                                                window,
                                                cx,
                                            );
                                        })),
                                )
                            }),
                    )
                },
            )
            .when(self.section == Section::Docs, |d| {
                d.child(
                    panel_toolbar(cx).child(
                        TabBar::new("documentation-mode")
                            .pill()
                            .small()
                            .selected_index(usize::from(self.docs_preview))
                            .children([Tab::new().label(tr("编辑")), Tab::new().label(tr("预览"))])
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.docs_preview = *index == 1;
                                cx.notify();
                            })),
                    ),
                )
            })
            .child(div().flex_1().min_h_0().child(
                if self.section == Section::Docs && self.docs_preview {
                    div()
                        .id("docs-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .p_4()
                        .child(TextView::markdown(
                            SharedString::from(format!("docs:{}", self.document.path().display())),
                            self.section_editor.read(cx).value(),
                        ))
                        .into_any_element()
                } else if self.section == Section::Headers && self.show_sent_headers {
                    panel_editor(&self.sent_headers_editor)
                        .readonly(true)
                        .aria_label(tr("实际发送的请求头"))
                        .into_any_element()
                } else if self.section.form().is_some() {
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .when(
                            is_request
                                && self.section == Section::Auth
                                && (self.form.read(cx).value(cx)["type"] == "oauth2"
                                    || self.form.read(cx).value(cx) == "inherit"),
                            |view| {
                                view.child(
                                    div().px_4().py_2().child(
                                        Button::new("oauth-authorize")
                                            .label(tr("获取 OAuth 令牌"))
                                            .disabled(self.running)
                                            .on_click(|_, window, cx| {
                                                window.dispatch_action(
                                                    Box::new(crate::workbench::AuthorizeOAuth),
                                                    cx,
                                                )
                                            }),
                                    ),
                                )
                            },
                        )
                        .child(
                            div()
                                .id("form-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .child(self.form.clone()),
                        )
                        .into_any_element()
                } else {
                    panel_editor(editor)
                        .aria_label(tr("请求编辑器"))
                        .into_any_element()
                },
            ));
        let response_pane = div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                TabBar::new("response-sections")
                    .menu(true)
                    .underline()
                    .px_4()
                    .gap_4()
                    .flex_shrink_0()
                    .bg(cx.theme().background)
                    .selected_index(self.response_view)
                    .children([
                        Tab::new()
                            .prefix(Icon::new(AppIcon::FileText).size_3p5())
                            .label(tr("响应正文")),
                        Tab::new()
                            .prefix(Icon::new(AppIcon::SlidersHorizontal).size_3p5())
                            .label(crate::i18n::format(
                                tr("响应头（{}）"),
                                &[(
                                    "0",
                                    format!(
                                        "{}",
                                        self.response
                                            .as_ref()
                                            .map(|r| r.headers.len())
                                            .unwrap_or(self.stream_headers.len())
                                    ),
                                )],
                            )),
                        Tab::new()
                            .prefix(Icon::new(AppIcon::Network).size_3p5())
                            .label(tr("时间线")),
                        Tab::new()
                            .prefix(Icon::new(AppIcon::CircleCheck).size_3p5())
                            .label(
                                self.response
                                    .as_ref()
                                    .map(|r| {
                                        let failed = r.tests.iter().filter(|t| !t.passed).count();
                                        if failed > 0 {
                                            crate::i18n::format(
                                                tr("测试 · {failed} 项失败"),
                                                &[("failed", format!("{failed}"))],
                                            )
                                        } else if !r.tests.is_empty() {
                                            crate::i18n::format(
                                                tr("测试 · {} 项通过"),
                                                &[("0", format!("{}", r.tests.len()))],
                                            )
                                        } else {
                                            tr("测试").into()
                                        }
                                    })
                                    .unwrap_or_else(|| tr("测试").into()),
                            ),
                        Tab::new().label(tr("基本信息")),
                    ])
                    .on_click(cx.listener(|this, ix, window, cx| {
                        this.response_view = *ix;
                        this.update_response(window, cx);
                        cx.notify();
                    })),
            )
            .when(
                self.response.is_some() || self.running || !self.stream.is_empty(),
                |d| {
                    d.child(
                        panel_toolbar(cx)
                            .when(self.running, |bar| {
                                bar.child(gpui_kit::component::spinner::Spinner::new().small())
                            })
                            .justify_between()
                            .child(if let Some(response) = &self.response {
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if response.status < 400 {
                                                cx.theme().success
                                            } else {
                                                cx.theme().danger
                                            })
                                            .child(format!("{}", response.status)),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{} ms", response.elapsed_ms)),
                                    )
                                    .child(div().text_color(cx.theme().muted_foreground).child(
                                        if response.bytes < 1024 {
                                            format!("{} B", response.bytes)
                                        } else {
                                            format!("{:.1} KB", response.bytes as f64 / 1024.)
                                        },
                                    ))
                                    .into_any_element()
                            } else {
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(&self.status).to_owned())
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(
                                        Button::new("pretty-response")
                                            .icon(AppIcon::Braces)
                                            .ghost()
                                            .xsmall()
                                            .label(tr("格式化"))
                                            .disabled(
                                                self.response_view != 0
                                                    || self.response.is_none()
                                                    || self.running,
                                            )
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                if let Some(response) = &this.response {
                                                    match languages::format(
                                                        Language::Json,
                                                        &response.body,
                                                    ) {
                                                        Ok(text) => {
                                                            this.response_pretty = true;
                                                            this.response_editor.update(
                                                                cx,
                                                                |s, cx| {
                                                                    s.set_value(text, window, cx)
                                                                },
                                                            );
                                                            cx.notify();
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
                                                }
                                            })),
                                    )
                                    .when(
                                        self.response_view == 0 && self.response.is_some(),
                                        |bar| {
                                            bar.child(
                                                Button::new("raw-response")
                                                    .ghost()
                                                    .xsmall()
                                                    .label(tr("原始内容"))
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.response_pretty = false;
                                                            this.update_response(window, cx);
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                        },
                                    )
                                    .child(
                                        Button::new("copy-response")
                                            .icon(AppIcon::Copy)
                                            .ghost()
                                            .xsmall()
                                            .label(tr("复制"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                crate::feedback::copy(
                                                    this.response_editor
                                                        .read(cx)
                                                        .value()
                                                        .to_string(),
                                                    tr("已复制响应内容").to_owned(),
                                                    window,
                                                    cx,
                                                );
                                            })),
                                    ),
                            ),
                    )
                },
            )
            .when(is_ws, |d| {
                d.child(
                    div()
                        .flex()
                        .p_3()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Input::new(&self.message).aria_label(tr("WebSocket 消息"))),
                        )
                        .child(
                            Button::new("send-message")
                                .icon(AppIcon::Send)
                                .label(tr("发送消息"))
                                .disabled(!self.connected)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.send_message(window, cx)
                                    }),
                                ),
                        ),
                )
            })
            .child(
                if self.response_view == 0
                    && self.response.is_none()
                    && !self.running
                    && self.stream.is_empty()
                {
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .items_center()
                        .gap_3()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            img(crate::brand::LOGO)
                                .size_12()
                                .object_fit(ObjectFit::Contain),
                        )
                        .child(div().text_sm().child(tr("发送请求后在此查看响应")))
                        .child(div().text_xs().child("⌘ / Ctrl + Enter"))
                        .into_any_element()
                } else if self.response_view == 1 {
                    self.response_rows(self.response_headers().to_vec(), cx)
                        .into_any_element()
                } else if self.response_view == 4 {
                    self.response_rows(self.response_info(), cx)
                        .into_any_element()
                } else if self.response_view == 0 && (is_ws || self.is_event_stream()) {
                    let entries = if is_ws {
                        self.socket_messages.clone()
                    } else {
                        crate::response_view::event_data(
                            self.response
                                .as_ref()
                                .map(|r| r.body.as_str())
                                .unwrap_or(&self.stream),
                        )
                    };
                    div()
                        .id("response-events")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.response_scroll)
                        .children(entries.into_iter().enumerate().map(|(ix, text)| {
                            div()
                                .id(("event", ix))
                                .p_3()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .hover(|d| d.bg(cx.theme().muted))
                                .child(text)
                        }))
                        .into_any_element()
                } else {
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(
                            panel_editor(&self.response_editor)
                                .readonly(true)
                                .aria_label(tr("响应内容")),
                        )
                        .into_any_element()
                },
            );
        let split = if !is_request {
            request_pane.into_any_element()
        } else if self.layout.current() == RequestLayout::Vertical {
            v_resizable("request-response-vertical")
                .child(resizable_panel().child(request_pane))
                .child(resizable_panel().child(response_pane))
                .into_any_element()
        } else {
            h_resizable("request-response-horizontal")
                .child(resizable_panel().child(request_pane))
                .child(resizable_panel().child(response_pane))
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .text_sm()
            .when(!is_request, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_3()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if self.document.kind() == DocumentKind::Environment {
                                    tr("环境")
                                } else {
                                    tr("集合 / 目录")
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(&self.name).aria_label(tr("名称"))),
                        )
                        .child(
                            Button::new("save-scope")
                                .icon(AppIcon::Save)
                                .outline()
                                .label(tr("保存"))
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(crate::workbench::Save), cx)
                                }),
                        ),
                )
            })
            .when(is_request, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .px_4()
                        .py_3()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .flex_1()
                                .min_w_0()
                                .border_1()
                                .border_color(cx.theme().input)
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().background)
                                .child(
                                    Button::new("method")
                                        .ghost()
                                        .w_24()
                                        .flex_shrink_0()
                                        .dropdown_caret(true)
                                        .text_color(crate::appearance::method_color(
                                            &self.method.read(cx).value(),
                                            cx,
                                        ))
                                        .label(self.method.read(cx).value())
                                        .disabled(is_ws)
                                        .dropdown_menu(move |menu, _, _| {
                                            let mut menu = menu;
                                            for method in [
                                                "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD",
                                                "OPTIONS", "CONNECT", "TRACE",
                                            ] {
                                                let weak = weak.clone();
                                                menu =
                                                    menu.item(PopupMenuItem::new(method).on_click(
                                                        move |_, window, cx| {
                                                            let _ = weak.update(cx, |this, cx| {
                                                                this.method.update(cx, |s, cx| {
                                                                    s.set_value(method, window, cx)
                                                                });
                                                                this.dirty = true;
                                                                cx.notify();
                                                            });
                                                        },
                                                    ));
                                            }
                                            menu
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .border_l_1()
                                        .border_color(cx.theme().border)
                                        .capture_action(cx.listener(
                                            |this,
                                             _: &gpui_kit::component::input::Paste,
                                             window,
                                             cx| {
                                                cx.stop_propagation();
                                                this.paste_url(window, cx);
                                            },
                                        ))
                                        .child(crate::variable_input::input(
                                            &self.url,
                                            Input::new(&self.url)
                                                .bordered(false)
                                                .rounded_none()
                                                .aria_label(tr("请求地址")),
                                            &self.variables,
                                            window,
                                            cx,
                                        )),
                                ),
                        )
                        .child(
                            Button::new("send")
                                .icon(if self.running {
                                    AppIcon::Square
                                } else if is_ws {
                                    AppIcon::Plug
                                } else {
                                    AppIcon::Send
                                })
                                .primary()
                                .label(if self.running {
                                    tr("取消")
                                } else if is_ws {
                                    tr("连接")
                                } else {
                                    tr("发送")
                                })
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(crate::workbench::Send), cx)
                                }),
                        )
                        .child(
                            Button::new("save")
                                .icon(AppIcon::Save)
                                .ghost()
                                .label(tr("保存"))
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(crate::workbench::Save), cx)
                                }),
                        ),
                )
            })
            .child(div().flex_1().min_h_0().child(split))
    }
}

fn should_restore_document_draft(state: &Value, source: &str) -> bool {
    state["dirty"].as_bool().unwrap_or(true)
        || state["baseSource"]
            .as_str()
            .or_else(|| state["source"].as_str())
            == Some(source)
}

#[cfg(test)]
mod draft_tests {
    use super::{DocumentLayout, should_restore_document_draft};
    use apifuck_core::settings::{RequestLayout, WorkspaceState};
    use serde_json::json;

    #[test]
    fn request_layout_is_independent_and_survives_workspace_recovery() {
        let mut first = DocumentLayout::new(RequestLayout::Vertical);
        let second = DocumentLayout::new(RequestLayout::Vertical);
        first.toggle();
        assert_eq!(first.current(), RequestLayout::Horizontal);
        assert_eq!(second.current(), RequestLayout::Vertical);

        let mut workspace = WorkspaceState::default();
        workspace.editor_drafts.insert(
            "first.yml".into(),
            json!({"requestLayout": first.local, "dirty": false, "baseSource": "old file"}),
        );
        workspace
            .editor_drafts
            .insert("second.yml".into(), json!({"requestLayout": second.local}));
        let saved = serde_json::to_string(&workspace).unwrap();
        let restored: WorkspaceState = serde_json::from_str(&saved).unwrap();
        let snapshot = &restored.editor_drafts["first.yml"];
        assert!(!should_restore_document_draft(snapshot, "external edit"));
        let mut first = DocumentLayout::new(RequestLayout::Vertical);
        first.restore(snapshot);
        assert_eq!(first.current(), RequestLayout::Horizontal);
        first.toggle();
        first.default = RequestLayout::Horizontal;
        assert_eq!(first.current(), RequestLayout::Vertical);

        let mut second = DocumentLayout::new(RequestLayout::Horizontal);
        second.restore(&restored.editor_drafts["second.yml"]);
        assert_eq!(second.current(), RequestLayout::Horizontal);
        second.default = RequestLayout::Vertical;
        assert_eq!(second.current(), RequestLayout::Vertical);
    }

    #[test]
    fn legacy_or_invalid_layout_drafts_follow_the_default() {
        for snapshot in [json!({}), json!({"requestLayout": "unknown"})] {
            let mut layout = DocumentLayout::new(RequestLayout::Horizontal);
            layout.restore(&snapshot);
            assert_eq!(layout.current(), RequestLayout::Horizontal);
            layout.default = RequestLayout::Vertical;
            assert_eq!(layout.current(), RequestLayout::Vertical);
        }
    }

    #[test]
    fn dirty_and_legacy_drafts_survive_external_file_changes() {
        assert!(should_restore_document_draft(
            &json!({"dirty":true,"source":"unfinished: ["}),
            "new file"
        ));
        assert!(should_restore_document_draft(
            &json!({"source":"old-format draft"}),
            "new file"
        ));
    }

    #[test]
    fn clean_snapshots_do_not_revert_external_edits() {
        let raw = json!({"dirty":false,"baseSource":"saved","source":"saved","message":"unsent"});
        assert!(should_restore_document_draft(&raw, "saved"));
        assert!(!should_restore_document_draft(&raw, "externally changed"));
    }
}
