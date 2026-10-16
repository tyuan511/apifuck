//! Product copy lives in the bundled catalogs. Protocol names and user data bypass translation.
use std::{collections::BTreeMap, sync::OnceLock};

fn english() -> &'static BTreeMap<String, String> {
    static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/en.json")).expect("invalid English catalog")
    })
}

pub fn set_language(language: &str) {
    gpui_kit::component::set_locale(if language == "en" { "en" } else { "zh-CN" });
}

pub fn translate<'a>(language: &str, text: &'a str) -> &'a str {
    if language == "en" {
        english().get(text).map(String::as_str).unwrap_or(text)
    } else {
        // Some entity-owned labels were constructed before a live language switch.
        english()
            .iter()
            .find(|(_, value)| value.as_str() == text)
            .map(|(key, _)| key.as_str())
            .unwrap_or(text)
    }
}
pub fn tr(text: &str) -> &str {
    translate(&gpui_kit::component::locale(), text)
}

/// Substitute once, so braces in response values or file paths remain untouched.
pub fn format(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::new();
    let mut rest = template;
    let mut position = 0;
    while !rest.is_empty() {
        if rest.starts_with("{{") || rest.starts_with("}}") {
            out.push(rest.chars().next().unwrap());
            rest = &rest[2..];
        } else if let Some(after_open) = rest.strip_prefix('{') {
            let Some(end) = after_open.find('}') else {
                out.push_str(rest);
                break;
            };
            let placeholder = &after_open[..end];
            let key = if placeholder.is_empty() {
                let key = position.to_string();
                position += 1;
                key
            } else {
                placeholder.to_owned()
            };
            if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
                out.push_str(value);
            } else {
                out.push('{');
                out.push_str(placeholder);
                out.push('}');
            }
            rest = &after_open[end + 1..];
        } else {
            let ch = rest.chars().next().unwrap();
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_languages_and_live_entity_labels_are_supported() {
        assert_eq!(translate("en", "保存设置"), "Save settings");
        assert_eq!(translate("zh-CN", "Save settings"), "保存设置");
        assert_eq!(translate("en", "{{base_url}}/users"), "{{base_url}}/users");
        assert_eq!(translate("zh-CN", "GET"), "GET");
    }
    #[test]
    fn translated_templates_keep_arguments_and_do_not_reinterpret_values() {
        let text = format(
            translate("en", "保存失败：{error:#}"),
            &[("error:#", "bad {path}".into())],
        );
        assert_eq!(text, "Unable to save: bad {path}");
        assert_eq!(
            format("{{body}} {}", &[("0", "{data}".into())]),
            "{body} {data}"
        );
        assert_eq!(
            format(
                translate("en", "HTTP {} · {} ms · {} 字节"),
                &[("0", "200".into()), ("1", "5".into()), ("2", "128".into())]
            ),
            "HTTP 200 · 5 ms · 128 bytes"
        );
    }
    #[test]
    fn catalog_preserves_all_format_placeholders() {
        fn placeholders(text: &str) -> Vec<String> {
            let mut result = Vec::new();
            let mut rest = text;
            while let Some(start) = rest.find('{') {
                rest = &rest[start + 1..];
                let end = rest.find('}').expect("unclosed placeholder");
                result.push(rest[..end].to_owned());
                rest = &rest[end + 1..];
            }
            result.sort();
            result
        }
        for (source, translation) in english() {
            assert!(!translation.is_empty(), "{source}");
            assert_eq!(placeholders(source), placeholders(translation), "{source}");
        }
    }

    #[test]
    fn catalog_covers_ui_literals() {
        for agent in apifuck_core::mcp::guides::Agent::ALL {
            assert!(english().contains_key(agent.instruction()));
        }
        for source in [
            include_str!("main.rs"),
            include_str!("workbench.rs"),
            include_str!("update_view.rs"),
            include_str!("document_tab.rs"),
            include_str!("settings_view.rs"),
            include_str!("mcp_view.rs"),
            include_str!("runner_view.rs"),
            include_str!("history_view.rs"),
            include_str!("cookies_view.rs"),
            include_str!("network_editor.rs"),
            include_str!("certificates_editor.rs"),
            include_str!("request_form.rs"),
            include_str!("key_value_editor.rs"),
            include_str!("variable_input.rs"),
            include_str!("editor_tools.rs"),
        ] {
            for fragment in source.split("tr(\"").skip(1) {
                let key = fragment.split('"').next().unwrap();
                assert!(english().contains_key(key), "missing English copy: {key}");
            }
        }
    }
}
