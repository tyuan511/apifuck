use gpui_kit::component::{
    ActiveTheme, Sizable, ThemeStyled,
    button::{Button, ButtonVariants},
    input::{
        EditorState, HoverProvider, Input, InputState, Rope, RopeExt, TextDecoration,
        TextDecorationCollection,
    },
    tooltip::Tooltip,
};
use gpui_kit::{prelude::FluentBuilder, *};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};
use std::rc::Rc;

const HOVER_LINE_BREAK: &str = "<br>";

// Encode literal text before placing it in the editor's Markdown hover. This
// preserves whitespace and prevents values from creating links or other markup.
fn hover_literal(text: &str) -> String {
    use std::fmt::Write;
    let mut literal = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            // GPUI collapses newlines inside Markdown text nodes. An owned
            // inline break keeps multiline values intact without a code block.
            literal.push_str(HOVER_LINE_BREAK);
        } else if ch.is_ascii_punctuation() || ch.is_whitespace() {
            write!(literal, "&#{};", ch as u32).unwrap();
        } else {
            literal.push(ch);
        }
    }
    literal
}

fn completion(text: &str, cursor: usize, name: &str) -> Option<String> {
    let before = text.get(..cursor)?;
    let start = before.rfind("{{")?;
    let prefix = before[start + 2..].trim();
    if prefix.contains(['{', '}', '\n']) || !name.starts_with(prefix) {
        return None;
    }
    let suffix = &text[cursor..];
    let suffix = suffix
        .find("}}")
        .filter(|end| !suffix[..*end].contains(['{', '}', '\n']))
        .map(|end| &suffix[end + 2..])
        .unwrap_or(suffix);
    Some(format!("{}{{{{{name}}}}}{suffix}", &text[..start]))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Variables {
    names: Vec<String>,
    values: serde_json::Value,
    environment: Option<String>,
    unavailable: bool,
}

#[derive(Debug, PartialEq)]
enum PreviewValue {
    Value(String),
    Unset,
    Invalid,
}

impl Variables {
    pub fn new(
        names: Vec<String>,
        values: anyhow::Result<serde_json::Value>,
        environment: Option<String>,
    ) -> Self {
        let unavailable = values.is_err();
        Self {
            names,
            values: values.unwrap_or_default(),
            environment,
            unavailable,
        }
    }

    fn environment_label(&self) -> String {
        crate::i18n::format(
            crate::i18n::tr("当前环境：{environment}"),
            &[(
                "environment",
                self.environment
                    .clone()
                    .unwrap_or_else(|| crate::i18n::tr("未选择环境").into()),
            )],
        )
    }

    fn tooltip(&self, name: &str, value: &PreviewValue) -> String {
        format!("{}\n{name} = {}", self.environment_label(), value.label())
    }

    fn resolve(&self, name: &str) -> PreviewValue {
        if self.unavailable {
            return PreviewValue::Invalid;
        }
        match apifuck_core::engine::interpolate(&format!("{{{{{name}}}}}"), &self.values) {
            Ok(value) => PreviewValue::Value(value),
            Err(error) if error.to_string().starts_with("未定义变量：") => {
                PreviewValue::Unset
            }
            Err(_) => PreviewValue::Invalid,
        }
    }

    fn hover_at(&self, text: &Rope, offset: usize) -> Option<Hover> {
        let source = text.to_string();
        let range = crate::syntax::template_ranges(&source)
            .into_iter()
            .find(|range| range.contains(&offset))?;
        let name = source[range.start + 2..range.end - 2].trim();
        // GPUI's level-six heading uses small, muted text. Keep the assignment
        // in ordinary text with a strong name, avoiding code-block chrome.
        let content = format!(
            "###### {}\n\n**{}** = {}",
            hover_literal(&self.environment_label()),
            hover_literal(name),
            hover_literal(&self.resolve(name).label()),
        );
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            // Match GPUI's own Rope conversion when it maps the hover back to bytes.
            range: Some(lsp_types::Range::new(
                text.offset_to_position(range.start),
                text.offset_to_position(range.end),
            )),
        })
    }

    fn decorations(&self, text: &str, primary: Hsla, danger: Hsla) -> Vec<TextDecoration> {
        crate::syntax::template_ranges(text)
            .into_iter()
            .map(|range| {
                let name = text[range.start + 2..range.end - 2].trim();
                let color = match self.resolve(name) {
                    PreviewValue::Value(_) => primary,
                    _ => danger,
                };
                TextDecoration::new(
                    range,
                    HighlightStyle {
                        color: Some(color),
                        background_color: Some(color.alpha(0.1)),
                        ..Default::default()
                    },
                )
            })
            .collect()
    }
}

impl HoverProvider for Variables {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        _: &mut App,
    ) -> Task<anyhow::Result<Option<Hover>>> {
        Task::ready(Ok(self.hover_at(text, offset)))
    }
}

/// Presentation-only editor support. Observe loads, typing and undo so a pending
/// hover can never outlive the text it describes; only decorations touch the editor.
pub struct EditorVariablePreview {
    editor: WeakEntity<EditorState>,
    variables: Option<Variables>,
    text: SharedString,
    language: SharedString,
    locale: String,
    colors: Option<(Hsla, Hsla)>,
    highlights: TextDecorationCollection,
    _observe: Subscription,
}

impl EditorVariablePreview {
    pub fn attach(editor: &Entity<EditorState>, cx: &mut App) -> Entity<Self> {
        let highlights = editor.update(cx, |state, cx| {
            state.create_decorations_collection(vec![], cx)
        });
        cx.new(|cx| Self {
            editor: editor.downgrade(),
            variables: None,
            text: editor.read(cx).value(),
            language: editor.read(cx).language_name(),
            locale: String::new(),
            colors: None,
            highlights,
            _observe: cx.observe(editor, |this: &mut Self, _, cx| this.refresh(false, cx)),
        })
    }

    pub fn set_variables(&mut self, variables: Option<&Variables>, cx: &mut Context<Self>) {
        let changed = self.variables.as_ref() != variables;
        if changed {
            self.variables = variables.cloned();
        }
        self.refresh(changed, cx);
    }

    fn refresh(&mut self, variables_changed: bool, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.upgrade() else {
            return;
        };
        let text = editor.read(cx).value();
        let language = editor.read(cx).language_name();
        let locale = gpui_kit::component::locale().to_string();
        let colors = (cx.theme().primary, cx.theme().danger);
        if !variables_changed
            && self.text == text
            && self.language == language
            && self.locale == locale
            && self.colors == Some(colors)
        {
            return;
        }
        self.text = text;
        self.language = language;
        self.locale = locale;
        self.colors = Some(colors);

        let variables = self
            .variables
            .as_ref()
            .filter(|_| self.language.as_ref() == "json");
        editor.update(cx, |state, cx| {
            state.clear_hover_state(cx);
            state.lsp_mut().hover_provider = variables
                .cloned()
                .map(|variables| Rc::new(variables) as Rc<dyn HoverProvider>);
        });
        self.highlights.set(
            variables
                .map(|variables| variables.decorations(&self.text, colors.0, colors.1))
                .unwrap_or_default(),
            cx,
        );
    }
}

impl PreviewValue {
    fn label(&self) -> String {
        match self {
            Self::Value(value) if value.is_empty() => crate::i18n::tr("空字符串").into(),
            Self::Value(value) => value.clone(),
            Self::Unset => crate::i18n::tr("未设置").into(),
            Self::Invalid => crate::i18n::tr("无法解析").into(),
        }
    }
}

#[derive(IntoElement)]
pub struct VariableInput {
    state: Entity<InputState>,
    control: Input,
    variables: Variables,
}

pub fn input(
    state: &Entity<InputState>,
    control: Input,
    variables: &Variables,
    _window: &Window,
    _cx: &App,
) -> VariableInput {
    VariableInput {
        state: state.clone(),
        control,
        variables: variables.clone(),
    }
}

impl RenderOnce for VariableInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Keep focus/blur and programmatic loads observable without changing the
        // editing buffer or adding presentation-only operations to undo history.
        window.use_keyed_state(("variable-preview", self.state.entity_id()), cx, |_, cx| {
            cx.observe(&self.state, |_, _, cx| cx.notify())
        });
        let text = self.state.read(cx).value();
        let cursor = self.state.read(cx).cursor();
        let focused = self.state.read(cx).focus_handle(cx).is_focused(window);
        let ranges = crate::syntax::template_ranges(&text);
        let show_preview =
            !focused && !ranges.is_empty() && !self.state.read(cx).presentation().is_masked();
        let suggestions = self
            .variables
            .names
            .iter()
            .filter_map(|name| {
                focused
                    .then(|| completion(&text, cursor, name))
                    .flatten()
                    .map(|value| (name.clone(), value))
            })
            .take(8)
            .collect::<Vec<_>>();
        let mut preview = div()
            .id(("variable-badges", self.state.entity_id()))
            .absolute()
            .inset_0()
            .px_3()
            .flex()
            .items_center()
            .overflow_hidden()
            .text_sm()
            .cursor_text()
            .on_mouse_down(MouseButton::Left, {
                let state = self.state.clone();
                move |_, window, cx| {
                    cx.stop_propagation();
                    state.update(cx, |state, cx| state.focus(window, cx));
                }
            });
        let mut offset = 0;
        for range in ranges {
            if range.start > offset {
                preview = preview.child(
                    div()
                        .flex_shrink_0()
                        .child(text[offset..range.start].to_owned()),
                );
            }
            let name = text[range.start + 2..range.end - 2].trim();
            let value = self.variables.resolve(name);
            let color = match value {
                PreviewValue::Value(_) => cx.theme().primary,
                _ => cx.theme().danger,
            };
            let tooltip = self.variables.tooltip(name, &value);
            preview = preview.child(
                div()
                    .id(("variable-badge", range.start))
                    .flex_shrink_0()
                    .px_1()
                    .rounded_md()
                    .border_1()
                    .border_color(color.alpha(0.25))
                    .bg(color.alpha(0.1))
                    .text_color(color)
                    .child(text[range.clone()].to_owned())
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
            );
            offset = range.end;
        }
        if offset < text.len() {
            preview = preview.child(div().flex_shrink_0().child(text[offset..].to_owned()));
        }
        let has_suggestions = !suggestions.is_empty();
        let menu = div()
            .id(("variable-completions", self.state.entity_id()))
            .occlude()
            .popover_style(cx)
            .p_1()
            .w_72()
            .max_h_64()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                // Keep the input focused while choosing a completion.
                window.prevent_default();
            })
            .children(
                suggestions
                    .into_iter()
                    .enumerate()
                    .map(|(ix, (name, value))| {
                        let state = self.state.clone();
                        let tooltip = self
                            .variables
                            .tooltip(&name, &self.variables.resolve(&name));
                        Button::new(("variable", ix))
                            .ghost()
                            .small()
                            .w_full()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .w_full()
                                    .text_left()
                                    .truncate()
                                    .child(format!("{{{{{name}}}}}")),
                            )
                            .tooltip(tooltip)
                            .on_click(move |_, window, cx| {
                                state.update(cx, |state, cx| {
                                    state.replace_all(value.clone(), window, cx);
                                    state.focus(window, cx);
                                })
                            })
                    }),
            );
        div()
            .relative()
            .min_w_0()
            .child(self.control.when(show_preview, |control| {
                control.text_color(cx.theme().foreground.alpha(0.))
            }))
            .when(show_preview, |this| this.child(preview))
            .when(has_suggestions, |this| {
                // Absolute positioning keeps the panel out of the table's layout;
                // deferred painting lets it escape scrolling/clipping ancestors.
                this.child(
                    div().absolute().top_full().left_0().child(
                        deferred(
                            anchored()
                                .offset(point(px(0.), px(4.)))
                                .snap_to_window_with_margin(px(8.))
                                .child(menu),
                        )
                        .with_priority(1),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{HOVER_LINE_BREAK, PreviewValue, Variables, completion};
    use gpui_kit::component::input::{Rope, RopeExt};
    use lsp_types::{Hover, HoverContents};
    use serde_json::json;

    fn hover_text(hover: Hover) -> String {
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected literal hover content");
        };
        let mut content = String::new();
        let mut headings = 0;
        let mut names = 0;
        for event in pulldown_cmark::Parser::new(&markup.value) {
            match event {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Heading {
                    level: pulldown_cmark::HeadingLevel::H6,
                    ..
                }) => headings += 1,
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => names += 1,
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph)
                | pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {}
                pulldown_cmark::Event::Text(text) => content.push_str(&text),
                pulldown_cmark::Event::InlineHtml(html) if html.as_ref() == HOVER_LINE_BREAK => {
                    content.push('\n')
                }
                pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Heading(_))
                | pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                    content.push('\n')
                }
                event => panic!("hover interpreted user data as markup: {event:?}"),
            }
        }
        assert_eq!(headings, 1);
        assert_eq!(names, 1);
        content
    }

    #[test]
    fn json_editor_hover_matches_full_tokens_after_unicode_and_ignores_plain_text() {
        let source = "{\n  \"🦊名字\": \"{{ 用户 }}\", \"id\": {{created_user_id}}, \"draft\": \"{{unfinished\"\n}";
        let text = Rope::from(source);
        let variables = Variables::new(
            vec![],
            Ok(json!({"用户": "Ada", "created_user_id": 3})),
            Some("local".into()),
        );
        for (token, assignment) in [
            ("{{ 用户 }}", "用户 = Ada"),
            ("{{created_user_id}}", "created_user_id = 3"),
        ] {
            let start = source.find(token).unwrap();
            for (relative, _) in token.char_indices() {
                let hover = variables.hover_at(&text, start + relative).unwrap();
                let range = hover.range.unwrap();
                assert_eq!(text.position_to_offset(&range.start), start);
                assert_eq!(text.position_to_offset(&range.end), start + token.len());
                let content = hover_text(hover);
                assert!(content.contains("local"));
                assert!(content.ends_with(&format!("{assignment}\n")));
            }
            assert!(variables.hover_at(&text, start + token.len()).is_none());
        }
        for offset in [
            0,
            source.find("🦊").unwrap(),
            source.find("unfinished").unwrap(),
            source.len(),
        ] {
            assert!(variables.hover_at(&text, offset).is_none());
        }
    }

    #[test]
    fn json_editor_hover_keeps_markdown_and_multiline_values_literal() {
        let value =
            "  user_name = [link](https://example.test) <b>value</b>\n```\n````text\n*literal*  ";
        let variables = Variables::new(
            vec![],
            Ok(json!({"payload": value})),
            Some("qa_*`环境`*".into()),
        );
        let text = Rope::from(r#"{"data":"{{payload}}"}"#);
        let hover = variables.hover_at(&text, 12).unwrap();
        assert_eq!(
            hover_text(hover),
            format!(
                "{}\n",
                variables.tooltip("payload", &variables.resolve("payload"))
            )
        );
    }

    #[test]
    fn json_editor_hover_tracks_environment_and_resolution_states() {
        let text = Rope::from("{{value}}");
        for values in [
            json!({"value": ""}),
            json!({}),
            json!({"value": "{{value}}"}),
            json!({"value": "{{nested}}", "nested": 3}),
        ] {
            let variables = Variables::new(vec![], Ok(values), None);
            assert_eq!(
                hover_text(variables.hover_at(&text, 3).unwrap()),
                format!(
                    "{}\n",
                    variables.tooltip("value", &variables.resolve("value"))
                )
            );
        }
        let local = Variables::new(vec![], Ok(json!({"value": 3})), Some("local".into()));
        let staging = Variables::new(vec![], Ok(json!({"value": 3})), Some("staging".into()));
        assert_ne!(
            hover_text(local.hover_at(&text, 3).unwrap()),
            hover_text(staging.hover_at(&text, 3).unwrap())
        );
    }

    #[test]
    fn previews_resolve_nested_typed_and_empty_values_without_confusing_unset() {
        let variables = Variables::new(
            vec![],
            Ok(json!({
                "host": "https://example.test", "url": "{{host}}/v1",
                "empty": "", "count": 0, "enabled": false, "nil": null,
                "object": {"a": 1}, "missing_nested": "{{missing}}",
                "cycle_a": "{{cycle_b}}", "cycle_b": "{{cycle_a}}"
            })),
            None,
        );
        for (name, value) in [
            ("url", "https://example.test/v1"),
            ("empty", ""),
            ("count", "0"),
            ("enabled", "false"),
            ("nil", "null"),
            ("object", r#"{"a":1}"#),
        ] {
            assert_eq!(variables.resolve(name), PreviewValue::Value(value.into()));
        }
        assert_eq!(variables.resolve("missing"), PreviewValue::Unset);
        assert_eq!(variables.resolve("missing_nested"), PreviewValue::Unset);
        assert_eq!(variables.resolve("cycle_a"), PreviewValue::Invalid);
        let invalid = Variables::new(vec![], Err(anyhow::anyhow!("invalid environment")), None);
        assert_eq!(invalid.resolve("host"), PreviewValue::Invalid);
    }

    #[test]
    fn previews_change_when_environment_values_change_without_name_changes() {
        let names = vec!["host".into()];
        let first = Variables::new(
            names.clone(),
            Ok(json!({"host":"local"})),
            Some("local".into()),
        );
        let second = Variables::new(
            names.clone(),
            Ok(json!({"host":"production"})),
            Some("production".into()),
        );
        assert_ne!(first, second);
        assert_eq!(
            second.resolve("host"),
            PreviewValue::Value("production".into())
        );
        let same_values =
            Variables::new(names, Ok(json!({"host":"local"})), Some("staging".into()));
        assert_ne!(first, same_values);
        assert_ne!(
            first.tooltip("host", &first.resolve("host")),
            same_values.tooltip("host", &same_values.resolve("host"))
        );
    }

    #[test]
    fn completes_open_tokens_without_duplicating_braces_or_losing_suffix() {
        assert_eq!(
            completion("Bearer {{tok}}/v1", 12, "token"),
            Some("Bearer {{token}}/v1".into())
        );
        assert_eq!(
            completion("{{ host", 7, "hostname"),
            Some("{{hostname}}".into())
        );
        assert_eq!(completion("{{done}}", 8, "done"), None);
        assert_eq!(completion("{{other", 7, "token"), None);
        assert_eq!(
            completion("中文{{token}}/v1", "中文{{tok".len(), "token"),
            Some("中文{{token}}/v1".into())
        );
    }
}
