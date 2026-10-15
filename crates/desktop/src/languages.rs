//! Offline syntax services. No request data is sent to a remote language server.
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Json,
    JavaScript,
    Graphql,
    Yaml,
    Xml,
    Sparql,
    Markdown,
    Text,
}
impl Language {
    pub fn from_id(id: &str) -> Self {
        match id.to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            "javascript" | "js" => Self::JavaScript,
            "graphql" | "gql" => Self::Graphql,
            "yaml" | "yml" => Self::Yaml,
            "xml" => Self::Xml,
            "sparql" | "rq" => Self::Sparql,
            "markdown" | "md" => Self::Markdown,
            _ => Self::Text,
        }
    }
    pub fn can_format(self) -> bool {
        matches!(
            self,
            Self::Json | Self::JavaScript | Self::Graphql | Self::Yaml
        )
    }
}

pub fn position(text: &str, offset: usize) -> Position {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let prefix = &text[..offset];
    Position::new(
        prefix.bytes().filter(|b| *b == b'\n').count() as u32,
        prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count() as u32,
    )
}
fn byte_offset(text: &str, line: usize, column: usize, byte_column: bool) -> usize {
    let start = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let tail = text
        .get(start..)
        .unwrap_or("")
        .split('\n')
        .next()
        .unwrap_or("");
    start
        + if byte_column {
            column.saturating_sub(1).min(tail.len())
        } else {
            tail.char_indices()
                .nth(column.saturating_sub(1))
                .map(|(i, _)| i)
                .unwrap_or(tail.len())
        }
}
fn diagnostic(text: &str, start: usize, end: usize, message: impl Into<String>) -> Diagnostic {
    let mut start = start.min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    // Give EOF errors a visible range on the preceding character.
    if start == text.len() && start > 0 {
        start = text.char_indices().next_back().unwrap().0;
    }
    let end = end
        .max(
            start
                + text[start..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0),
        )
        .min(text.len());
    Diagnostic {
        range: Range::new(position(text, start), position(text, end)),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("ApiFuck syntax".into()),
        message: message.into(),
        ..Default::default()
    }
}

/// Replace template expressions with valid stand-ins while keeping byte offsets.
/// Templates inside quoted strings need no replacement. The formatter uses unique
/// string markers instead so it can restore expressions exactly after reflowing.
fn template_input(
    text: &str,
    language: Language,
    markers: bool,
) -> (String, Vec<(String, String)>) {
    if language == Language::Sparql {
        let mut input = text.as_bytes().to_vec();
        for range in crate::syntax::template_ranges(text) {
            let in_iri = text[..range.start]
                .rfind('<')
                .is_some_and(|open| !text[open..range.start].contains('>'));
            input[range.clone()].fill(if in_iri { b'x' } else { b' ' });
            if !in_iri {
                input[range.start] = b'0';
            }
        }
        return (
            String::from_utf8(input).expect("templates replaced at UTF-8 boundaries"),
            vec![],
        );
    }
    if !matches!(language, Language::Json | Language::Graphql) {
        return (text.into(), vec![]);
    }
    let mut result = String::new();
    let mut replacements = vec![];
    let mut i = 0;
    let mut quoted = false;
    let mut block = false;
    let mut comment = false;
    let mut prefix = "__apifuck_template_".to_string();
    while text.contains(&prefix) {
        prefix.push('_');
    }
    while i < text.len() {
        let rest = &text[i..];
        if !quoted && language == Language::Graphql && rest.starts_with('#') {
            comment = true;
        }
        if comment {
            let ch = rest.chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
            if ch == '\n' {
                comment = false;
            }
            continue;
        }
        if block && rest.starts_with("\\\"\"\"") {
            result.push_str(&rest[..4]);
            i += 4;
            continue;
        }
        if language == Language::Graphql && rest.starts_with("\"\"\"") {
            block = !block;
            result.push_str("\"\"\"");
            i += 3;
            continue;
        }
        if block {
            let ch = rest.chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if quoted && rest.starts_with('\\') {
            result.push('\\');
            i += 1;
            if let Some(ch) = text[i..].chars().next() {
                result.push(ch);
                i += ch.len_utf8();
            }
            continue;
        }
        if rest.starts_with('"') {
            quoted = !quoted;
            result.push('"');
            i += 1;
            continue;
        }
        if !quoted
            && rest.starts_with("{{")
            && let Some(end) = rest[2..].find("}}")
        {
            let original = &rest[..end + 4];
            if !original[2..original.len() - 2].trim().is_empty()
                && !original[2..original.len() - 2].contains(['{', '}', '\n', '\r'])
            {
                if markers {
                    let marker = format!("\"{prefix}{}__\"", replacements.len());
                    result.push_str(&marker);
                    replacements.push((marker, original.into()));
                } else {
                    result.push('0');
                    result.extend(std::iter::repeat_n(' ', original.len() - 1));
                }
                i += original.len();
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }
    (result, replacements)
}

pub fn diagnostics(language: Language, text: &str) -> Vec<Diagnostic> {
    if text.trim().is_empty() || language == Language::Text {
        return vec![];
    }
    let (input, _) = template_input(text, language, false);
    match language {
        Language::Json => {
            let mut de = serde_json::Deserializer::from_str(&input);
            // IgnoredAny validates without rounding numbers or dropping duplicate keys.
            match serde::de::IgnoredAny::deserialize(&mut de).and_then(|_| de.end()) {
                Ok(()) => vec![],
                Err(e) => {
                    let i = byte_offset(&input, e.line(), e.column(), true);
                    vec![diagnostic(text, i, i, e.to_string())]
                }
            }
        }
        Language::Graphql => {
            let tree = apollo_parser::Parser::new(&input).parse();
            tree.errors()
                .take(100)
                .map(|e| diagnostic(text, e.index(), e.index() + e.data().len(), e.message()))
                .collect()
        }
        Language::JavaScript => {
            // Hooks execute inside a synchronous arrow function (core/scripts.rs).
            // Match that grammar: return is allowed, import and bare await are not.
            const PREFIX: &str = "(() => {\n";
            let input = format!("{PREFIX}{input}\n}})()");
            let params = deno_ast::ParseParams {
                specifier: deno_ast::ModuleSpecifier::parse("file:///apifuck-hook.js").unwrap(),
                text: input.into(),
                media_type: deno_ast::MediaType::JavaScript,
                capture_tokens: false,
                scope_analysis: false,
                maybe_syntax: None,
            };
            let errors = match deno_ast::parse_script(params) {
                Ok(parsed) => parsed.diagnostics().clone(),
                Err(e) => vec![e],
            };
            errors
                .iter()
                .take(100)
                .map(|e| {
                    let range = e.range();
                    diagnostic(
                        text,
                        range
                            .start
                            .as_byte_index(deno_ast::StartSourcePos::START_SOURCE_POS)
                            .saturating_sub(PREFIX.len()),
                        range
                            .end
                            .as_byte_index(deno_ast::StartSourcePos::START_SOURCE_POS)
                            .saturating_sub(PREFIX.len()),
                        e.kind().msg().to_string(),
                    )
                })
                .collect()
        }
        Language::Yaml => {
            let result = serde_yaml::Deserializer::from_str(text)
                .try_for_each(|document| serde_yaml::Value::deserialize(document).map(|_| ()));
            match result {
                Ok(()) => vec![],
                Err(e) => {
                    let i = e.location().map(|l| l.index()).unwrap_or(0);
                    vec![diagnostic(text, i, i, e.to_string())]
                }
            }
        }
        Language::Xml => match roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        ) {
            Ok(_) => vec![],
            Err(e) => {
                let p = e.pos();
                let i = byte_offset(text, p.row as usize, p.col as usize, false);
                vec![diagnostic(text, i, i, e.to_string())]
            }
        },
        Language::Sparql => {
            let parser = spargebra::SparqlParser::new()
                .with_base_iri("https://apifuck.invalid/")
                .unwrap();
            if parser.clone().parse_query(&input).is_ok()
                || parser.clone().parse_update(&input).is_ok()
            {
                return vec![];
            }
            let error = if text.trim_start().to_uppercase().starts_with("INSERT")
                || text.trim_start().to_uppercase().starts_with("DELETE")
            {
                parser.parse_update(&input).unwrap_err().to_string()
            } else {
                parser.parse_query(&input).unwrap_err().to_string()
            };
            // spargebra's public error exposes its PEG location in Display only.
            let location = error
                .split(" at ")
                .nth(1)
                .and_then(|s| s.split_once(": "))
                .map(|(p, _)| p);
            let i = location
                .and_then(|p| p.split_once(':'))
                .and_then(|(l, c)| Some(byte_offset(text, l.parse().ok()?, c.parse().ok()?, false)))
                .unwrap_or(0);
            vec![diagnostic(text, i, i, error)]
        }
        Language::Markdown => markdown_diagnostics(text),
        Language::Text => vec![],
    }
}

fn markdown_diagnostics(text: &str) -> Vec<Diagnostic> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
    let mut language = Language::Text;
    let mut result = vec![];
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                language = Language::from_id(info.split_whitespace().next().unwrap_or(""))
            }
            Event::End(TagEnd::CodeBlock) => language = Language::Text,
            Event::Text(_) if language != Language::Text && language != Language::Markdown => {
                // Use original source slices to keep diagnostics mapped to document offsets.
                let code = &text[range.clone()];
                for mut d in diagnostics(language, code) {
                    let start = offset_from_position(code, d.range.start) + range.start;
                    let end = offset_from_position(code, d.range.end) + range.start;
                    d.range = Range::new(position(text, start), position(text, end));
                    result.push(d);
                }
            }
            _ => {}
        }
    }
    result
}
fn offset_from_position(text: &str, pos: Position) -> usize {
    let start = text
        .split_inclusive('\n')
        .take(pos.line as usize)
        .map(str::len)
        .sum::<usize>();
    let mut units = 0;
    for (i, ch) in text[start.min(text.len())..].char_indices() {
        if units >= pos.character {
            return start + i;
        }
        units += ch.len_utf16() as u32;
    }
    text.len()
}

pub fn format(language: Language, text: &str) -> anyhow::Result<String> {
    anyhow::ensure!(language.can_format(), "No formatter for this language");
    if text.trim().is_empty() {
        return Ok(text.into());
    }
    if let Some(error) = diagnostics(language, text).first() {
        anyhow::bail!(
            "{}:{}: {}",
            error.range.start.line + 1,
            error.range.start.character + 1,
            error.message
        );
    }
    let (input, markers) = template_input(text, language, true);
    let mut output = match language {
        Language::Json => dprint_plugin_json::format_text(
            Path::new("body.json"),
            &input,
            &dprint_plugin_json::configuration::ConfigurationBuilder::new()
                .indent_width(2)
                .use_tabs(false)
                // prefer_single_line(false) still retains short inline containers.
                // Force every non-empty object/array to expand, preserving tokens.
                .line_width(1)
                .prefer_single_line(false)
                .build(),
        )?
        .unwrap_or(input),
        Language::Graphql => pretty_graphql::format_text(&input, &Default::default())?,
        Language::Yaml => pretty_yaml::format_text(&input, &Default::default())?,
        Language::JavaScript => {
            dprint_plugin_typescript::format_text(dprint_plugin_typescript::FormatTextOptions {
                path: Path::new("hook.js"),
                extension: None,
                text: input.clone(),
                config: &dprint_plugin_typescript::configuration::ConfigurationBuilder::new()
                    .indent_width(2)
                    .build(),
                external_formatter: None,
            })?
            .unwrap_or(input)
        }
        _ => unreachable!(),
    };
    for (marker, original) in markers {
        output = output.replace(&marker, &original);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_invalid_inputs_for_every_structured_language() {
        for (language, good, bad) in [
            (
                Language::Json,
                r#"{"用户":"你好","active":true}"#,
                r#"{"id":}"#,
            ),
            (
                Language::JavaScript,
                "const user = { id: 1 }; test('ok', () => expect(user.id).to.equal(1));",
                "const user = ;",
            ),
            (
                Language::Graphql,
                "query Users($id: ID!) { user(id: $id) { ...User } } fragment User on User { id name }",
                "query Users { user( }",
            ),
            (
                Language::Yaml,
                "# preserved\nname: demo\nitems: [one, two]\n",
                "items: [one, two\n",
            ),
            (
                Language::Xml,
                "<?xml version=\"1.0\"?><root id=\"1\"><child>中文</child></root>",
                "<root><child></root>",
            ),
            (
                Language::Sparql,
                "SELECT ?s WHERE { ?s ?p ?o }",
                "SELECT WHERE {",
            ),
        ] {
            assert!(
                diagnostics(language, good).is_empty(),
                "{language:?}: {:?}",
                diagnostics(language, good)
            );
            assert!(
                !diagnostics(language, bad).is_empty(),
                "{language:?} accepted invalid syntax"
            );
        }
    }

    #[test]
    fn syntax_errors_do_not_execute_javascript() {
        assert!(
            diagnostics(
                Language::JavaScript,
                "throw new Error('must not run'); while (true) {} "
            )
            .is_empty()
        );
        assert!(diagnostics(Language::JavaScript, "return 1;").is_empty());
        assert!(
            !diagnostics(Language::JavaScript, "await fetch('https://example.com');").is_empty()
        );
        assert!(!diagnostics(Language::JavaScript, "import fs from 'fs';").is_empty());
    }

    #[test]
    fn strict_json_does_not_accept_json5_or_missing_commas() {
        for input in [
            "{a:1}",
            "{'a':1}",
            "{\"a\":1,}",
            "{\"a\":1 \"b\":2}",
            "{\"a\":NaN}",
            "/* comment */ {}",
        ] {
            assert!(!diagnostics(Language::Json, input).is_empty(), "{input}");
            assert!(format(Language::Json, input).is_err());
        }
    }

    #[test]
    fn json_formatting_expands_response_objects_and_arrays_with_two_spaces() {
        let input = r#"{"authenticated":true,"method":"bearer","permissions":["users:read","users:write"]}"#;
        let expected = concat!(
            "{\n",
            "  \"authenticated\": true,\n",
            "  \"method\": \"bearer\",\n",
            "  \"permissions\": [\n",
            "    \"users:read\",\n",
            "    \"users:write\"\n",
            "  ]\n",
            "}\n",
        );
        assert_eq!(format(Language::Json, input).unwrap(), expected);
        assert_eq!(format(Language::Json, expected).unwrap(), expected);
    }

    #[test]
    fn json_formatting_expands_short_nested_values_and_keeps_empty_containers() {
        for (input, expected) in [
            (r#"{"a":1}"#, "{\n  \"a\": 1\n}\n"),
            ("[1]", "[\n  1\n]\n"),
            (
                r#"[{"a":[1]},[],{}]"#,
                "[\n  {\n    \"a\": [\n      1\n    ]\n  },\n  [],\n  {}\n]\n",
            ),
            (r#"{"id":{{ user_id }}}"#, "{\n  \"id\": {{ user_id }}\n}\n"),
            ("{}", "{}\n"),
            ("[]", "[]\n"),
            ("true", "true\n"),
        ] {
            let output = format(Language::Json, input).unwrap();
            assert_eq!(output, expected, "{input}");
            assert_eq!(format(Language::Json, &output).unwrap(), output);
        }
    }

    #[test]
    fn formatting_preserves_json_numbers_duplicate_keys_order_and_templates() {
        let input =
            r#"{"z":123456789012345678901234567890,"z":1e400,"a":"{{用户}}","id":{{ user_id }}}"#;
        let output = format(Language::Json, input).unwrap();
        assert!(output.contains("123456789012345678901234567890"));
        assert!(output.contains("1e400"));
        assert_eq!(output.matches("\"z\"").count(), 2);
        assert!(output.find("\"z\"").unwrap() < output.find("\"a\"").unwrap());
        assert!(output.contains("{{ user_id }}"));
        assert!(output.contains("{{用户}}"));
        assert!(output.contains('\n'));
        assert_eq!(format(Language::Json, &output).unwrap(), output);
    }

    #[test]
    fn formatting_preserves_graphql_comments_fragments_and_block_strings() {
        let input = "# comment\nquery Q($id: ID!) { user(id: $id) { ...Fields } search(text: \"\"\"a { b }\"\"\") { id } } fragment Fields on User { id name }";
        let output = format(Language::Graphql, input).unwrap();
        assert!(output.contains("# comment"));
        assert!(output.contains("...Fields"));
        assert!(output.contains("a { b }"));
        assert_eq!(format(Language::Graphql, &output).unwrap(), output);
        let templated = "{ user(id: {{user_id}}) { id } }";
        assert!(
            format(Language::Graphql, templated)
                .unwrap()
                .contains("{{user_id}}")
        );
    }

    #[test]
    fn yaml_and_javascript_formatting_preserve_meaning_and_comments() {
        let yaml = "# keep me\ninfo: {name: 'demo', tags: [one,two]}\n";
        let formatted = format(Language::Yaml, yaml).unwrap();
        assert!(formatted.contains("# keep me"));
        assert_eq!(
            serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap(),
            serde_yaml::from_str::<serde_yaml::Value>(&formatted).unwrap()
        );
        let js = "// keep\nconst a={id:1};test('ok',()=>{expect(a.id).to.equal(1)});";
        let formatted = format(Language::JavaScript, js).unwrap();
        assert!(formatted.contains("// keep"));
        assert!(diagnostics(Language::JavaScript, &formatted).is_empty());
        assert_eq!(format(Language::JavaScript, &formatted).unwrap(), formatted);
    }

    #[test]
    fn template_masking_keeps_error_positions_after_unicode() {
        let source = "{\"id\":{{用户}},\"bad\":}";
        let errors = diagnostics(Language::Json, source);
        assert_eq!(errors.len(), 1);
        let expected = source.find(":}").unwrap() + 1;
        assert_eq!(errors[0].range.start, position(source, expected));
        assert!(
            diagnostics(
                Language::Json,
                r#"{"id":{{id}},"text":"escaped \"{{foo}}"}"#
            )
            .is_empty()
        );
        assert_eq!(position("中文😀x", "中文😀".len()), Position::new(0, 4));
    }

    #[test]
    fn markdown_checks_embedded_code_and_leaves_prose_alone() {
        assert!(
            diagnostics(
                Language::Markdown,
                "# Title\nArbitrary **prose and unfinished [links"
            )
            .is_empty()
        );
        let source = "# Example\n\n```json\n{\"id\":}\n```\n";
        let errors = diagnostics(Language::Markdown, source);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].range.start.line, 3);
        assert!(
            diagnostics(
                Language::Markdown,
                "```js\nfunction test() {\n return 1;\n}\n```\n"
            )
            .is_empty()
        );
        assert!(diagnostics(Language::Text, "{any unstructured text").is_empty());
    }

    #[test]
    fn sparql_update_and_relative_iris_are_supported() {
        assert!(
            diagnostics(Language::Sparql, "INSERT DATA { <test> <field> \"value\" }").is_empty()
        );
    }

    #[test]
    fn errors_are_bounded_ranges_and_empty_editors_are_quiet() {
        for language in [
            Language::Json,
            Language::JavaScript,
            Language::Graphql,
            Language::Yaml,
            Language::Xml,
            Language::Sparql,
            Language::Markdown,
        ] {
            assert!(diagnostics(language, "").is_empty());
            for error in diagnostics(language, "\n😀\n{") {
                assert!(error.range.start <= error.range.end);
                assert!(error.range.end <= position("\n😀\n{", "\n😀\n{".len()));
            }
        }
    }
    #[test]
    fn javascript_error_locations_match_original_hook_lines() {
        let text = "const name = '😀';\nconst id = ;";
        let error = diagnostics(Language::JavaScript, text).remove(0);
        assert_eq!(error.range.start.line, 1);
        assert_eq!(error.range.start.character, 11);
        assert!(format(Language::JavaScript, "return {id:1};").is_ok());
    }

    #[test]
    fn sparql_templates_and_internal_xml_doctype_are_valid() {
        assert!(
            diagnostics(
                Language::Sparql,
                "SELECT ?s WHERE { ?s <{{predicate}}> {{ value }} }"
            )
            .is_empty()
        );
        assert!(
            diagnostics(
                Language::Xml,
                "<!DOCTYPE root [<!ENTITY greeting 'hello'>]><root>&greeting;</root>"
            )
            .is_empty()
        );
    }
}
