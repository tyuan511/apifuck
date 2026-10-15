//! Editor grammar and palette overrides for the bundled GPUI version.
use gpui_kit::component::highlighter::{HighlightTheme, LanguageRegistry};
use std::{
    ops::Range,
    sync::{Arc, Once},
};

pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let registry = LanguageRegistry::singleton();
        let mut graphql = registry
            .language("graphql")
            .expect("GraphQL grammar is enabled");
        // GPUI 0.7.3 supplies the grammar with an empty highlights query.
        graphql.highlights = include_str!("queries/graphql.scm").into();
        registry.register("graphql", &graphql);
    });
}

pub fn highlight_theme(dark: bool) -> Arc<HighlightTheme> {
    let mut theme = if dark {
        HighlightTheme::default_dark()
    } else {
        HighlightTheme::default_light()
    };
    let syntax = &mut Arc::make_mut(&mut theme).style.syntax;
    // JSON keys and GraphQL fields should be distinct from ordinary text.
    syntax.property = syntax.type_.or(syntax.keyword);
    theme
}

/// Byte ranges used by editor decorations, including templates in invalid JSON.
pub fn template_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    while let Some(start) = text[offset..].find("{{") {
        let start = offset + start;
        let Some(end) = text[start + 2..].find("}}") else {
            break;
        };
        let end = start + 2 + end;
        let name = &text[start + 2..end];
        if !name.trim().is_empty() && !name.contains(['{', '}']) {
            ranges.push(start..end + 2);
        }
        offset = end + 2;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::{highlight_theme, init, template_ranges};
    use gpui_kit::component::highlighter::SyntaxHighlighter;

    fn assert_colored(language: &str, source: &str, tokens: &[&str]) {
        init();
        let mut highlighter = SyntaxHighlighter::new(language);
        let mut text = highlighter.text().clone();
        text.insert(0, source);
        assert!(highlighter.update(None, &text, None));
        for dark in [false, true] {
            let theme = highlight_theme(dark);
            let styles = highlighter.styles(&(0..source.len()), theme.as_ref());
            for token in tokens {
                let start = source.find(token).unwrap();
                assert!(
                    styles.iter().any(|(range, style)| {
                        range.start <= start
                            && range.end >= start + token.len()
                            && style
                                .color
                                .is_some_and(|color| color.a > 0. && color.s > 0.1)
                    }),
                    "{language}: {token} has no visible syntax color (dark={dark})"
                );
            }
        }
    }

    #[test]
    fn graphql_queries_and_fragments_have_highlights_in_both_themes() {
        assert_colored(
            "graphql",
            "query GetUser($id: Int!) { user(id: $id) { id name email } }",
            &["query", "GetUser", "$id", "Int", "user"],
        );
        assert_colored(
            "graphql",
            "# comment\nfragment UserFields on User { name @include(if: true) }",
            &["fragment", "UserFields", "User", "name", "true"],
        );
    }

    #[test]
    fn json_variables_keep_key_colors_with_template_values() {
        assert_colored(
            "json",
            r#"{"id": 42, "name": "test", "active": true}"#,
            &["\"id\"", "42", "\"test\"", "true"],
        );
        assert_colored("json", r#"{"id": {{user_id}}}"#, &["\"id\""]);
    }

    #[test]
    fn template_highlights_use_utf8_ranges_and_ignore_unfinished_variables() {
        let source = r#"{"名字":"{{ user_name }}", "id":{{user_id}}, "draft":{{unfinished}"#;
        let tokens: Vec<_> = template_ranges(source)
            .into_iter()
            .map(|range| &source[range])
            .collect();
        assert_eq!(tokens, ["{{ user_name }}", "{{user_id}}"]);
        assert!(template_ranges(r#"{"id": 42, "empty":"{{ }}"}"#).is_empty());
    }
}
