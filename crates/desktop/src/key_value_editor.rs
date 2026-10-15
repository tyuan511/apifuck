use crate::i18n::tr;
use crate::icons::AppIcon;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{Value, json};

pub struct Changed;
pub struct KeyValueEditor {
    rows: Vec<Row>,
    variables: crate::variable_input::Variables,
    next_id: usize,
    title: String,
    value_key: String,
    multipart: bool,
    assertions: bool,
    key_key: String,
}
struct Row {
    id: usize,
    original: Value,
    name: Entity<InputState>,
    value: Entity<InputState>,
    description: Entity<InputState>,
    enabled: bool,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<Changed> for KeyValueEditor {}
impl KeyValueEditor {
    pub fn new(title: &str, value_key: &str) -> Self {
        Self {
            rows: vec![],
            variables: Default::default(),
            next_id: 0,
            title: title.into(),
            value_key: value_key.into(),
            multipart: false,
            assertions: false,
            key_key: "name".into(),
        }
    }
    pub fn set_variables(&mut self, variables: crate::variable_input::Variables) {
        self.variables = variables;
    }
    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for row in &self.rows {
            for (input, label) in [
                (&row.name, "名称"),
                (&row.value, "值"),
                (&row.description, "说明"),
            ] {
                input.update(cx, |input, cx| input.set_placeholder(tr(label), window, cx));
            }
        }
        cx.notify();
    }
    pub fn set_assertions(&mut self, enabled: bool) {
        self.assertions = enabled;
        self.key_key = if enabled { "expression" } else { "name" }.into();
    }
    pub fn set_multipart(&mut self, enabled: bool) {
        self.multipart = enabled;
    }
    pub fn set_title(&mut self, title: &str) {
        self.title = title.into();
    }
    pub fn set_value(&mut self, value: &Value, window: &mut Window, cx: &mut Context<Self>) {
        self.rows.clear();
        for value in value.as_array().into_iter().flatten() {
            self.insert(value.clone(), window, cx);
        }
        if self.rows.is_empty() {
            self.insert(json!({}), window, cx);
        }
        cx.notify();
    }
    fn insert(&mut self, original: Value, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(original[&self.key_key].as_str().unwrap_or("").to_owned())
                .placeholder(tr("名称"))
        });
        let value = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(
                    original[&self.value_key]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            if original[&self.value_key].is_null() {
                                String::new()
                            } else {
                                original[&self.value_key].to_string()
                            }
                        }),
                )
                .placeholder(tr("值"))
        });
        let description = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(original["description"].as_str().unwrap_or("").to_owned())
                .placeholder(tr("说明"))
        });
        let subscriptions = [&name, &value, &description]
            .into_iter()
            .map(|field| {
                cx.subscribe(field, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.emit(Changed);
                        cx.notify();
                    }
                })
            })
            .collect();
        let enabled = original["disabled"] != true;
        self.rows.push(Row {
            id,
            original,
            name,
            value,
            description,
            enabled,
            _subscriptions: subscriptions,
        });
    }
    fn choose_file(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let choice = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("选择上传文件").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = choice.await
                && let Some(path) = paths.first()
            {
                let value = path.to_string_lossy().into_owned();
                let _ = this.update_in(cx, |this, window, cx| {
                    if let Some(row) = this.rows.iter_mut().find(|r| r.id == id) {
                        row.original["type"] = json!("file");
                        row.value.update(cx, |s, cx| s.set_value(value, window, cx));
                        cx.emit(Changed);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
    // Drafts keep raw rows, including empty/disabled rows and their original types.
    pub fn snapshot(&self, cx: &App) -> Value {
        json!(
            self.rows
                .iter()
                .map(|row| json!({
                    "original": row.original,
                    "name": row.name.read(cx).value().to_string(),
                    "value": row.value.read(cx).value().to_string(),
                    "description": row.description.read(cx).value().to_string(),
                    "enabled": row.enabled,
                }))
                .collect::<Vec<_>>()
        )
    }
    pub fn restore(&mut self, snapshot: &Value, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rows) = snapshot.as_array() else {
            return;
        };
        self.rows.clear();
        for saved in rows {
            self.insert(saved["original"].clone(), window, cx);
            let row = self.rows.last_mut().unwrap();
            for (input, key) in [
                (&row.name, "name"),
                (&row.value, "value"),
                (&row.description, "description"),
            ] {
                input.update(cx, |input, cx| {
                    input.set_value(saved[key].as_str().unwrap_or("").to_owned(), window, cx)
                });
            }
            row.enabled = saved["enabled"].as_bool().unwrap_or(true);
        }
        cx.notify();
    }
    pub fn value(&self, cx: &App) -> Value {
        json!(
            self.rows
                .iter()
                .filter_map(|row| {
                    let name = row.name.read(cx).value().to_string();
                    let value = row.value.read(cx).value().to_string();
                    let description = row.description.read(cx).value().to_string();
                    if name.is_empty() && value.is_empty() && description.is_empty() {
                        return None;
                    }
                    let mut entry = row.original.clone();
                    entry[&self.key_key] = json!(name);
                    if self.assertions && entry.get("operator").is_none() {
                        entry["operator"] = json!("eq");
                    }
                    if self.multipart && entry.get("type").is_none() {
                        entry["type"] = json!("text");
                    }
                    let original_value = entry[&self.value_key]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            if entry[&self.value_key].is_null() {
                                String::new()
                            } else {
                                entry[&self.value_key].to_string()
                            }
                        });
                    if entry["secret"] != true
                        && (original_value != value || entry.get(&self.value_key).is_none())
                    {
                        entry[&self.value_key] = json!(value);
                    }
                    if description != row.original["description"].as_str().unwrap_or("") {
                        entry["description"] = json!(description);
                    }
                    if !row.enabled || entry.get("disabled").is_some() {
                        entry["disabled"] = json!(!row.enabled);
                    }
                    Some(entry)
                })
                .collect::<Vec<_>>()
        )
    }
}
impl Render for KeyValueEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empty_variables = crate::variable_input::Variables::default();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(&self.title).to_owned()),
                    )
                    .child(
                        Button::new("add-row")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .label(tr("添加"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.insert(json!({}), window, cx);
                                cx.emit(Changed);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h_8()
                            .bg(cx.theme().muted)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(div().w_9().flex_shrink_0())
                            .when(self.multipart || self.assertions, |d| {
                                d.child(div().w(rems(7.)).flex_shrink_0().px_2().child(
                                    if self.assertions {
                                        tr("运算符")
                                    } else {
                                        tr("类型")
                                    },
                                ))
                            })
                            .child(div().flex_1().min_w_0().px_2().child(tr("名称")))
                            .child(div().flex_1().min_w_0().px_2().child(tr("值")))
                            .child(div().flex_1().min_w_0().px_2().child(tr("说明")))
                            .child(div().w_8().flex_shrink_0()),
                    )
                    .children(self.rows.iter().map(|row| {
                        let id = row.id;
                        let weak = cx.entity().downgrade();
                        let assertions = self.assertions;
                        div()
                            .id(("row", id))
                            .flex()
                            .items_center()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(
                                div().w_9().flex_shrink_0().flex().justify_center().child(
                                    Checkbox::new(("enabled", id))
                                        .checked(row.enabled)
                                        .on_change(cx.listener(move |this, value, _, cx| {
                                            if let Some(row) =
                                                this.rows.iter_mut().find(|r| r.id == id)
                                            {
                                                row.enabled = *value;
                                            }
                                            cx.emit(Changed);
                                            cx.notify();
                                        })),
                                ),
                            )
                            .when(self.multipart || self.assertions, |d| {
                                d.child(
                                    div().w(rems(7.)).flex_shrink_0().child(
                                        Button::new(("part-type", id))
                                            .dropdown_caret(true)
                                            .ghost()
                                            .small()
                                            .icon(if assertions {
                                                AppIcon::SlidersHorizontal
                                            } else {
                                                crate::icons::value_kind(
                                                    row.original["type"].as_str().unwrap_or("text"),
                                                )
                                            })
                                            .label(
                                                match row.original
                                                    [if assertions { "operator" } else { "type" }]
                                                .as_str()
                                                .unwrap_or(if assertions { "eq" } else { "text" })
                                                {
                                                    "text" => tr("文本"),
                                                    "file" => tr("文件"),
                                                    value => value,
                                                }
                                                .to_owned(),
                                            )
                                            .dropdown_menu(move |menu, _, _| {
                                                let mut menu = menu;
                                                for mode in if assertions {
                                                    apifuck_core::testing::OPERATORS.to_vec()
                                                } else {
                                                    vec!["text", "file"]
                                                } {
                                                    let weak = weak.clone();
                                                    menu = menu.item(
                                                        PopupMenuItem::new(match mode {
                                                            "text" => tr("文本"),
                                                            "file" => tr("文件"),
                                                            _ => mode,
                                                        })
                                                        .icon(crate::icons::value_kind(mode))
                                                        .on_click(move |_, window, cx| {
                                                            let _ = weak.update(cx, |this, cx| {
                                                                if let Some(row) = this
                                                                    .rows
                                                                    .iter_mut()
                                                                    .find(|r| r.id == id)
                                                                {
                                                                    row.original[if assertions {
                                                                        "operator"
                                                                    } else {
                                                                        "type"
                                                                    }] = json!(mode);
                                                                }
                                                                if !assertions && mode == "file" {
                                                                    this.choose_file(
                                                                        id, window, cx,
                                                                    );
                                                                }
                                                                cx.emit(Changed);
                                                                cx.notify();
                                                            });
                                                        }),
                                                    );
                                                }
                                                menu
                                            }),
                                    ),
                                )
                            })
                            .child(
                                div().flex_1().min_w_0().child(crate::variable_input::input(
                                    &row.name,
                                    Input::new(&row.name)
                                        .small()
                                        .bordered(false)
                                        .rounded_none()
                                        .aria_label(tr("名称")),
                                    &self.variables,
                                    window,
                                    cx,
                                )),
                            )
                            .child(
                                div().flex_1().min_w_0().child(crate::variable_input::input(
                                    &row.value,
                                    Input::new(&row.value)
                                        .readonly(row.original["secret"] == true)
                                        .small()
                                        .bordered(false)
                                        .rounded_none()
                                        .aria_label(tr("值")),
                                    if row.original["secret"] == true {
                                        &empty_variables
                                    } else {
                                        &self.variables
                                    },
                                    window,
                                    cx,
                                )),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    Input::new(&row.description)
                                        .small()
                                        .bordered(false)
                                        .rounded_none()
                                        .aria_label(tr("说明")),
                                ),
                            )
                            .child(
                                div().w_8().flex_shrink_0().child(
                                    Button::new(("remove-row", id))
                                        .ghost()
                                        .xsmall()
                                        .icon(AppIcon::Trash)
                                        .accessibility_label(tr("删除行"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.rows.retain(|r| r.id != id);
                                            cx.emit(Changed);
                                            cx.notify();
                                        })),
                                ),
                            )
                    })),
            )
    }
}
