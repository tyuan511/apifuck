//! Offline Hook API completion. Catalogs describe the native QuickJS runtime,
//! rather than browser/Node APIs which scripts cannot call.
use gpui_kit::component::{
    highlighter::SyntaxHighlighter,
    input::{CompletionProvider, EditorState, HoverProvider, Rope, RopeExt},
};
use gpui_kit::{App, Context, Task, Window, px};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Documentation, Hover, HoverContents, MarkupContent, MarkupKind, Range, TextEdit,
};
use std::{cell::RefCell, rc::Rc};

pub fn configure(
    state: &mut EditorState,
    response_available: Option<bool>,
    cx: &mut Context<EditorState>,
) {
    state.dismiss_lsp_overlays(cx);
    let provider = response_available.map(|response_available| {
        Rc::new(ScriptCompletion {
            response_available,
            syntax: RefCell::new(SyntaxHighlighter::new("javascript")),
        })
    });
    state.lsp_mut().completion_provider = provider.clone().map(|p| p as Rc<dyn CompletionProvider>);
    state.lsp_mut().hover_provider = provider.map(|p| p as Rc<dyn HoverProvider>);
    state.lsp_mut().completion_menu.max_width = px(560.);
}

struct ScriptCompletion {
    response_available: bool,
    syntax: RefCell<SyntaxHighlighter>,
}

impl CompletionProvider for ScriptCompletion {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        Task::ready(Ok(CompletionResponse::Array(self.items(text, offset))))
    }

    fn is_completion_trigger(&self, _: usize, text: &str, _: &mut App) -> bool {
        !text.is_empty() && text.chars().all(|ch| identifier(ch) || ch == '.')
    }
}

impl HoverProvider for ScriptCompletion {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        _: &mut App,
    ) -> Task<anyhow::Result<Option<Hover>>> {
        Task::ready(Ok(self.documentation(text, offset)))
    }
}

impl ScriptCompletion {
    fn is_code(&self, text: &Rope, offset: usize) -> bool {
        if offset == 0 {
            return true;
        }
        let mut syntax = self.syntax.borrow_mut();
        if !syntax.update(None, text, None) {
            return false;
        }
        let Some(tree) = syntax.tree() else {
            return false;
        };
        // A small lexical state machine handles unfinished strings/comments while
        // typing; the syntax tree distinguishes regex literals from division.
        // Template substitutions return to code, including nested templates.
        enum Mode {
            Code(usize),
            Quote(u8),
            Template,
            LineComment,
            BlockComment,
        }
        let source = text.to_string();
        let bytes = source.as_bytes();
        let mut modes = vec![Mode::Code(0)];
        let mut i = 0;
        while i < offset {
            let ch = bytes[i];
            let next = bytes.get(i + 1).copied();
            let nested = modes.len() > 1;
            match modes.last_mut().unwrap() {
                Mode::Quote(quote) => {
                    if ch == b'\\' {
                        i += 1;
                    } else if ch == *quote {
                        modes.pop();
                    }
                }
                Mode::Template => {
                    if ch == b'\\' {
                        i += 1;
                    } else if ch == b'`' {
                        modes.pop();
                    } else if ch == b'$' && next == Some(b'{') {
                        modes.push(Mode::Code(0));
                        i += 1;
                    }
                }
                Mode::LineComment => {
                    if matches!(ch, b'\n' | b'\r') {
                        modes.pop();
                    }
                }
                Mode::BlockComment => {
                    if ch == b'*' && next == Some(b'/') {
                        modes.pop();
                        i += 1;
                    }
                }
                Mode::Code(depth) => match ch {
                    b'\'' | b'"' => modes.push(Mode::Quote(ch)),
                    b'`' => modes.push(Mode::Template),
                    b'{' => *depth += 1,
                    b'}' if *depth > 0 => *depth -= 1,
                    b'}' if nested => {
                        modes.pop();
                    }
                    b'/' if next == Some(b'/') => {
                        modes.push(Mode::LineComment);
                        i += 1;
                    }
                    b'/' if next == Some(b'*') => {
                        modes.push(Mode::BlockComment);
                        i += 1;
                    }
                    b'/' => {
                        let mut node = tree.root_node().descendant_for_byte_range(i, i + 1);
                        while let Some(current) = node {
                            if current.kind() == "regex" {
                                if offset <= current.end_byte() {
                                    return false;
                                }
                                i = current.end_byte() - 1;
                                break;
                            }
                            node = current.parent();
                        }
                    }
                    _ => {}
                },
            }
            i += 1;
        }
        matches!(modes.last(), Some(Mode::Code(_)))
    }

    fn items(&self, text: &Rope, offset: usize) -> Vec<CompletionItem> {
        let source = text.to_string();
        let Some(context) = completion_context(&source, offset) else {
            return vec![];
        };
        if !self.is_code(text, offset) {
            return vec![];
        }
        catalog(&context.owner, self.response_available)
            .into_iter()
            .filter(|entry| entry.name.starts_with(context.prefix))
            .map(|entry| CompletionItem {
                label: entry.name.into(),
                detail: Some(entry.signature.into()),
                kind: Some(entry.kind),
                filter_text: Some(context.prefix.into()),
                documentation: Some(Documentation::MarkupContent(markup(entry.signature))),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    // GPUI uses its Rope position conversion for both insertion and hover.
                    range: Range::new(
                        text.offset_to_position(context.start),
                        text.offset_to_position(context.end),
                    ),
                    new_text: entry.name.into(),
                })),
                ..Default::default()
            })
            .collect()
    }

    fn documentation(&self, text: &Rope, offset: usize) -> Option<Hover> {
        let source = text.to_string();
        let context = completion_context(&source, offset)?;
        if !self.is_code(text, offset) {
            return None;
        }
        let name = &source[context.start..context.end];
        let entry = catalog(&context.owner, self.response_available)
            .into_iter()
            .find(|entry| entry.name == name)?;
        Some(Hover {
            contents: HoverContents::Markup(markup(entry.signature)),
            range: Some(Range::new(
                text.offset_to_position(context.start),
                text.offset_to_position(context.end),
            )),
        })
    }
}

fn markup(signature: &str) -> MarkupContent {
    MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!("```typescript\n{signature}\n```"),
    }
}

fn identifier(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}

struct CompletionContextAt<'a> {
    owner: String,
    prefix: &'a str,
    start: usize,
    end: usize,
}

fn completion_context(source: &str, offset: usize) -> Option<CompletionContextAt<'_>> {
    let before = source.get(..offset)?;
    let start = before
        .char_indices()
        .rev()
        .find(|(_, ch)| !identifier(*ch))
        .map_or(0, |(i, ch)| i + ch.len_utf8());
    let end = source[offset..]
        .char_indices()
        .find(|(_, ch)| !identifier(*ch))
        .map_or(source.len(), |(i, _)| offset + i);
    let mut rest = source[..start].trim_end();
    let mut owners = Vec::new();
    while let Some(parent) = rest.strip_suffix('.') {
        // Also accept optional member access (`fuck.response?.json`).
        let parent = parent.strip_suffix('?').unwrap_or(parent).trim_end();
        let word_start = parent
            .char_indices()
            .rev()
            .find(|(_, ch)| !identifier(*ch))
            .map_or(0, |(i, ch)| i + ch.len_utf8());
        let name = &parent[word_start..];
        if name.is_empty() {
            return None;
        }
        owners.push(name);
        rest = parent[..word_start].trim_end();
    }
    owners.reverse();
    Some(CompletionContextAt {
        owner: owners.join("."),
        prefix: &source[start..offset],
        start,
        end,
    })
}

struct Entry {
    name: &'static str,
    signature: &'static str,
    kind: CompletionItemKind,
}
fn property(name: &'static str, signature: &'static str) -> Entry {
    Entry {
        name,
        signature,
        kind: CompletionItemKind::PROPERTY,
    }
}
fn method(name: &'static str, signature: &'static str) -> Entry {
    Entry {
        name,
        signature,
        kind: CompletionItemKind::FUNCTION,
    }
}

fn catalog(owner: &str, response_available: bool) -> Vec<Entry> {
    let owner = if let Some(suffix) = owner.strip_prefix("fuck") {
        format!("api{suffix}")
    } else {
        owner.to_owned()
    };
    let owner = match owner.as_str() {
        "$req" => "req",
        "$res" => "res",
        other => other,
    };
    if !response_available && matches!(owner, "api.response" | "res") {
        return vec![];
    }
    match owner {
        "" => {
            let mut entries = vec![
                property("fuck", "fuck: HookAPI = api"),
                property("api", "api: HookAPI"),
                property("req", "req: OpenCollectionRequest"),
                property("$req", "$req: OpenCollectionRequest = req"),
                property("bru", "bru: VariableAPI"),
                property("console", "console: { log, warn, error }"),
                property("JSON", "JSON: { parse, stringify }"),
                property("Math", "Math: Math"),
                property("Date", "Date: DateConstructor"),
                property("Object", "Object: ObjectConstructor"),
                property("Array", "Array: ArrayConstructor"),
                method("String", "String(value: unknown): string"),
                method("Number", "Number(value: unknown): number"),
                method("Boolean", "Boolean(value: unknown): boolean"),
                method(
                    "parseInt",
                    "parseInt(value: string, radix?: number): number",
                ),
                method("parseFloat", "parseFloat(value: string): number"),
                method(
                    "encodeURIComponent",
                    "encodeURIComponent(value: string): string",
                ),
                method(
                    "decodeURIComponent",
                    "decodeURIComponent(value: string): string",
                ),
                method("test", "test(name: string, callback: () => void): void"),
                method("expect", "expect(value: unknown): Chai.Assertion"),
                method("assert", "assert(value: unknown, message?: string): void"),
            ];
            if response_available {
                entries.push(property("res", "res: ResponseAPI"));
                entries.push(property("$res", "$res: ResponseAPI = res"));
            }
            entries.extend(
                [
                    "const",
                    "let",
                    "var",
                    "if",
                    "else",
                    "for",
                    "of",
                    "in",
                    "while",
                    "return",
                    "function",
                    "try",
                    "catch",
                    "finally",
                    "throw",
                    "new",
                    "typeof",
                    "instanceof",
                    "true",
                    "false",
                    "null",
                    "undefined",
                    "switch",
                    "case",
                    "break",
                    "continue",
                ]
                .into_iter()
                .map(|name| Entry {
                    name,
                    signature: name,
                    kind: CompletionItemKind::KEYWORD,
                }),
            );
            entries
        }
        "api" => {
            let mut entries = vec![
                property("env", "env: { get, set }"),
                property("vars", "vars: { get, set }"),
                property("config", "config: OpenCollectionRequest"),
                property("request", "request: OpenCollectionRequest = config"),
            ];
            if response_available {
                entries.push(property(
                    "response",
                    "response: { status, headers, body, time, json }",
                ));
            }
            entries
        }
        "api.env" | "api.vars" => vec![
            method("get", "get(key: string): unknown"),
            method("set", "set(key: string, value: unknown): void"),
        ],
        "api.config" | "api.request" | "req" => {
            let mut entries = vec![
                property("url", "url: string"),
                property("method", "method: string"),
                property(
                    "headers",
                    "headers: Array<{ name: string, value: string, disabled?: boolean }>",
                ),
                property(
                    "params",
                    "params: Array<{ name: string, value: string, type: 'query' | 'path' }>",
                ),
                property("body", "body: { type: string, data: unknown }"),
                property("auth", "auth: object | 'inherit' | 'none'"),
                property(
                    "variables",
                    "variables: Array<{ name: string, value: unknown }>",
                ),
            ];
            entries.extend([
                method("getUrl", "getUrl(): string"),
                method("setUrl", "setUrl(url: string): void"),
                method("getMethod", "getMethod(): string"),
                method("setMethod", "setMethod(method: string): void"),
                method("getHeader", "getHeader(name: string): string | undefined"),
                method("getHeaders", "getHeaders(): Record<string, string>"),
                method("setHeader", "setHeader(name: string, value: string): void"),
                method("deleteHeader", "deleteHeader(name: string): void"),
                method("getBody", "getBody(): unknown"),
                method("setBody", "setBody(value: unknown): void"),
            ]);
            entries
        }
        "api.config.body" | "api.request.body" | "req.body" => vec![
            property("type", "type: string"),
            property("data", "data: unknown"),
        ],
        "api.response" => vec![
            property("status", "status: number"),
            property("headers", "headers: Record<string, string>"),
            property("body", "body: string"),
            property("time", "time: number // ms"),
            method("json", "json(path?: string): unknown"),
        ],
        "bru" => vec![
            method("getEnvVar", "getEnvVar(key: string): unknown"),
            method("setEnvVar", "setEnvVar(key: string, value: unknown): void"),
            method("getVar", "getVar(key: string): unknown"),
            method("setVar", "setVar(key: string, value: unknown): void"),
            method("hasVar", "hasVar(key: string): boolean"),
            method("hasEnvVar", "hasEnvVar(key: string): boolean"),
            method("getAllVars", "getAllVars(): Record<string, unknown>"),
            method("getAllEnvVars", "getAllEnvVars(): Record<string, unknown>"),
        ],
        "res" => vec![
            property("status", "status: number"),
            property("headers", "headers: Record<string, string>"),
            property("body", "body: unknown"),
            property("responseTime", "responseTime: number // ms"),
            method("getStatus", "getStatus(): number"),
            method("getHeaders", "getHeaders(): Record<string, string>"),
            method("getHeader", "getHeader(name: string): string | undefined"),
            method("getBody", "getBody(): unknown"),
            method("setBody", "setBody(value: unknown): void"),
            method("jq", "jq(expression: string): unknown"),
        ],
        "console" => vec![
            method("log", "log(...values: unknown[]): void"),
            method("warn", "warn(...values: unknown[]): void"),
            method("error", "error(...values: unknown[]): void"),
        ],
        "JSON" => vec![
            method("parse", "parse(text: string): unknown"),
            method(
                "stringify",
                "stringify(value: unknown, replacer?: unknown, space?: number): string",
            ),
        ],
        "Object" => vec![
            method("keys", "keys(object: object): string[]"),
            method("values", "values(object: object): unknown[]"),
            method("entries", "entries(object: object): [string, unknown][]"),
            method(
                "assign",
                "assign(target: object, ...sources: object[]): object",
            ),
            method(
                "fromEntries",
                "fromEntries(entries: Iterable<[string, unknown]>): object",
            ),
            method("hasOwn", "hasOwn(object: object, key: string): boolean"),
        ],
        "Array" => vec![
            method("isArray", "isArray(value: unknown): boolean"),
            method("from", "from(value: Iterable<unknown>): unknown[]"),
            method("of", "of(...values: unknown[]): unknown[]"),
        ],
        "Date" => vec![
            method("now", "now(): number"),
            method("parse", "parse(value: string): number"),
        ],
        "Math" => vec![
            method("random", "random(): number"),
            method("floor", "floor(value: number): number"),
            method("ceil", "ceil(value: number): number"),
            method("round", "round(value: number): number"),
            method("abs", "abs(value: number): number"),
            method("min", "min(...values: number[]): number"),
            method("max", "max(...values: number[]): number"),
            property("PI", "PI: number"),
        ],
        "assert" => vec![
            method(
                "equal",
                "equal(actual: unknown, expected: unknown, message?: string): void",
            ),
            method(
                "deepEqual",
                "deepEqual(actual: unknown, expected: unknown, message?: string): void",
            ),
            method(
                "strictEqual",
                "strictEqual(actual: unknown, expected: unknown, message?: string): void",
            ),
            method("isTrue", "isTrue(value: unknown, message?: string): void"),
            method("isFalse", "isFalse(value: unknown, message?: string): void"),
        ],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::ScriptCompletion;
    use gpui_kit::component::{
        highlighter::SyntaxHighlighter,
        input::{Rope, RopeExt},
    };
    use lsp_types::CompletionTextEdit;
    use std::cell::RefCell;

    fn provider(response_available: bool) -> ScriptCompletion {
        ScriptCompletion {
            response_available,
            syntax: RefCell::new(SyntaxHighlighter::new("javascript")),
        }
    }

    fn labels(source: &str, response_available: bool) -> Vec<String> {
        provider(response_available)
            .items(&Rope::from(source), source.len())
            .into_iter()
            .map(|item| item.label)
            .collect()
    }

    #[test]
    fn hook_aliases_offer_runtime_members_and_parameter_details() {
        for source in ["fuck.env.", "api.env.", "fuck.vars.", "api.vars."] {
            assert_eq!(labels(source, false), ["get", "set"], "{source}");
        }
        let items = provider(false).items(&Rope::from("fuck.env.se"), 11);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "set");
        assert!(
            items[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("key: string, value: unknown")
        );
        assert!(labels("fuck.config.", false).contains(&"headers".into()));
        assert_eq!(labels("fuck.config.body.", false), ["type", "data"]);
        assert!(!labels("fuck.config.body.", false).contains(&"raw".into()));
        assert!(labels("req.", false).contains(&"setHeader".into()));
        assert!(labels("bru.", false).contains(&"getEnvVar".into()));
    }

    #[test]
    fn response_members_are_only_suggested_after_response_or_in_tests() {
        assert!(!labels("fuck.", false).contains(&"response".into()));
        assert!(labels("fuck.response.", false).is_empty());
        assert!(labels("res.", false).is_empty());
        assert!(labels("fuck.", true).contains(&"response".into()));
        assert_eq!(labels("fuck.response.j", true), ["json"]);
        assert!(labels("$res.", true).contains(&"getBody".into()));
    }

    #[test]
    fn completions_replace_identifier_suffix_and_preserve_unicode_and_member_path() {
        for source in [
            "fuck.env.g|et",
            "// 中文🙂\nconst 名 = '🙂'; fuck.env.g|et",
            "fuck . env . g|et",
            "fuck.response?.j|son",
        ] {
            let offset = source.find('|').unwrap();
            let source = source.replace('|', "");
            let rope = Rope::from(source.as_str());
            let items = provider(true).items(&rope, offset);
            let item = items.first().expect("completion");
            let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
                panic!("missing edit")
            };
            let range = rope.position_to_offset(&edit.range.start)
                ..rope.position_to_offset(&edit.range.end);
            let mut result = source.clone();
            result.replace_range(range, &edit.new_text);
            assert_eq!(
                result, source,
                "must not duplicate suffix or replace member path"
            );
        }
    }

    #[test]
    fn strings_comments_and_regex_do_not_offer_code_completions() {
        for source in [
            "// fuck.env.",
            "/* fuck.env.",
            "'fuck.env.",
            "\"fuck.env.",
            "`fuck.env.",
            "/fuck.env./",
            "const value = /fuck.env./; // api.",
        ] {
            assert!(labels(source, true).is_empty(), "{source}");
        }
        assert_eq!(labels("`token: ${fuck.env.g", true), ["get"]);
        assert_eq!(labels("/* comment */\nfuck.env.g", true), ["get"]);
        for source in [
            "const text = '/*'; fuck.env.g",
            "const text = `/*`; fuck.env.g",
            "/* nested /* marker */ fuck.env.g",
            r#"const pattern = /['"`]/; fuck.env.g"#,
            "const ratio = 10 / 2; fuck.env.g",
            "`outer ${`inner ${fuck.env.g",
            "`outer ${{value: 1}.value + fuck.env.g",
        ] {
            assert_eq!(labels(source, true), ["get"], "{source}");
        }
    }

    #[test]
    fn javascript_builtins_are_available_without_unsupported_host_apis() {
        assert_eq!(labels("cons", false), ["console", "const"]);
        assert_eq!(labels("JSON.p", false), ["parse"]);
        assert_eq!(labels("console.", false), ["log", "warn", "error"]);
        for unsupported in ["fetch", "window", "require", "process", "unknown."] {
            assert!(labels(unsupported, false).is_empty(), "{unsupported}");
        }
        assert!(labels("", true).contains(&"test".into()));
        assert!(labels("", true).contains(&"expect".into()));
    }

    #[test]
    fn advertised_api_members_exist_in_the_actual_hook_runtime() {
        let mut code = String::new();
        for owner in [
            "",
            "fuck",
            "api",
            "fuck.env",
            "fuck.vars",
            "fuck.config",
            "fuck.config.body",
            "fuck.response",
            "req",
            "$req",
            "res",
            "$res",
            "bru",
            "console",
            "JSON",
            "Math",
            "Date",
            "Object",
            "Array",
            "assert",
        ] {
            for entry in super::catalog(owner, true) {
                if entry.kind == lsp_types::CompletionItemKind::KEYWORD {
                    continue;
                }
                let expression = if owner.is_empty() {
                    entry.name.to_owned()
                } else {
                    format!("{owner}.{}", entry.name)
                };
                code.push_str(&format!("if (typeof {expression} === 'undefined') throw new Error('Missing {expression}');\n"));
            }
        }
        code.push_str("fuck.env.set('token', fuck.response.json('$.token')); fuck.vars.set('trace', 'example'); req.setHeader('X-Trace', fuck.vars.get('trace')); ");
        let mut request = serde_json::json!({"url":"https://example.com", "method":"GET", "headers":[], "params":[], "body":{"type":"json", "data":"{}"}, "auth":"none", "variables":[]});
        let mut response =
            serde_json::json!({"status":200, "headers":{}, "body":"{\"token\":\"abc\"}", "time":1});
        let mut env = serde_json::json!({});
        let mut vars = serde_json::json!({});
        apifuck_core::scripts::run(
            &code,
            &mut request,
            Some(&mut response),
            &mut env,
            &mut vars,
            &mut serde_json::json!({}),
            &mut apifuck_core::scripts::ScriptReport::default(),
        )
        .unwrap();
        assert_eq!(env["token"], "abc");
        assert_eq!(request["headers"][0]["value"], "example");
    }

    #[test]
    fn hover_describes_complete_symbol_at_caret() {
        let source = "fuck.response.json('$.token')";
        let hover = provider(true)
            .documentation(&Rope::from(source), source.find("json").unwrap() + 2)
            .unwrap();
        let lsp_types::HoverContents::Markup(content) = hover.contents else {
            panic!("missing signature")
        };
        assert!(content.value.contains("json(path?: string): unknown"));
        assert!(
            provider(false)
                .documentation(&Rope::from(source), source.find("json").unwrap() + 2)
                .is_none()
        );
    }
}
