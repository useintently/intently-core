//! Shared utilities used across all language-specific extractors.
//!
//! These functions extract text from tree-sitter nodes, parse arguments,
//! detect log sinks, and provide other cross-cutting helpers. By
//! centralizing them here we keep each extractor focused on its
//! framework-specific CST walking logic.

use std::path::Path;

use tree_sitter::Node;

use crate::model::patterns;
use crate::model::types::*;

/// Create a [`SourceAnchor`] from a tree-sitter node and file path.
///
/// Captures the full node position: start/end line, byte offsets, and
/// CST node kind. This is the standard way to create anchors inside
/// extractors — every `Interface`, `Dependency`, `Sink`, `Symbol`,
/// and `DataModel` construction should use this.
pub fn anchor_from_node(node: &Node, file_path: &Path) -> SourceAnchor {
    SourceAnchor {
        file: file_path.to_path_buf(),
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        node_kind: node.kind().to_string(),
    }
}

/// Extract the source text spanned by a tree-sitter node.
pub fn node_text(node: &Node, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
}

/// Extract the source text spanned by a tree-sitter node as a borrowed slice.
///
/// Zero-copy alternative to [`node_text`] — returns a `&str` tied to the
/// source lifetime instead of allocating a new `String`. Use this when
/// the text is only needed for comparisons, pattern matching, or other
/// read-only operations within the same scope.
pub fn node_text_ref<'a>(node: &Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

/// Strip surrounding quote characters (single, double, or backtick).
pub fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .to_string()
}

/// Strip surrounding quote characters without allocating.
///
/// Zero-copy alternative to [`strip_quotes`] — returns a `&str` slice
/// of the input with leading/trailing quote characters removed.
pub fn strip_quotes_ref(s: &str) -> &str {
    s.trim_matches(|c| c == '\'' || c == '"' || c == '`')
}

/// Try to parse a string literal from an argument's text.
///
/// Returns `Some(unquoted_value)` if the text is a quoted string,
/// `None` otherwise.
pub fn extract_string_value(text: &str) -> Option<String> {
    let text = text.trim();
    if (text.starts_with('\'') && text.ends_with('\''))
        || (text.starts_with('"') && text.ends_with('"'))
        || (text.starts_with('`') && text.ends_with('`'))
    {
        Some(strip_quotes_ref(text).to_string())
    } else {
        None
    }
}

/// A single argument extracted from a call's argument list.
pub struct ArgumentInfo {
    pub text: String,
}

/// Collect all non-punctuation children of an argument list node.
///
/// Skips `(`, `)`, and `,` tokens, returning the remaining children
/// as `ArgumentInfo` values with their source text.
pub fn collect_arguments(args_node: &Node, source: &str) -> Vec<ArgumentInfo> {
    let mut result = Vec::new();
    let count = args_node.child_count();
    for i in 0..count {
        if let Some(child) = args_node.child(i as u32) {
            if child.kind() == "(" || child.kind() == ")" || child.kind() == "," {
                continue;
            }
            result.push(ArgumentInfo {
                text: node_text(&child, source),
            });
        }
    }
    result
}

/// Parse an HTTP method name (case-insensitive) into an `HttpMethod`.
pub fn parse_http_method(name: &str) -> Option<HttpMethod> {
    match name.to_lowercase().as_str() {
        "get" => Some(HttpMethod::Get),
        "post" => Some(HttpMethod::Post),
        "put" => Some(HttpMethod::Put),
        "patch" => Some(HttpMethod::Patch),
        "delete" => Some(HttpMethod::Delete),
        "options" => Some(HttpMethod::Options),
        "head" => Some(HttpMethod::Head),
        "all" => Some(HttpMethod::All),
        _ => None,
    }
}

/// Check if a CST node kind represents a function/method call.
///
/// Each language family uses different node names:
/// - JS/TS/Go/Rust/C/C++/Swift/Scala: `call_expression`
/// - Python/Ruby: `call`
/// - Java/Kotlin: `method_invocation`
/// - C#: `invocation_expression`
/// - PHP: `member_call_expression`, `function_call_expression`, `scoped_call_expression`
/// - Ruby: `method_call`
#[deprecated(note = "Use LanguageBehavior::call_node_kinds() for language-specific call detection")]
pub fn is_call_node(kind: &str) -> bool {
    matches!(
        kind,
        "call_expression"
            | "call"
            | "method_invocation"
            | "invocation_expression"
            | "member_call_expression"
            | "function_call_expression"
            | "scoped_call_expression"
            | "method_call"
    )
}

/// Try to detect a log sink using text-based heuristic matching.
///
/// Checks if the call text contains an `object.method(` pattern where
/// `object` is a known log object and `method` is a known log method.
/// This is the generic detection shared by all extractors — framework-
/// specific extractors may also use their own CST-aware detection.
pub fn try_extract_log_sink(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    // Quick exit using zero-copy text: does the call text contain any log object name?
    let call_ref = node_text_ref(node, source);
    let call_lower = call_ref.to_lowercase();
    let has_log_object = patterns::LOG_OBJECTS
        .iter()
        .any(|obj| call_lower.contains(&obj.to_lowercase()));

    if !has_log_object {
        return;
    }

    // Check for object.method( and object::method( patterns
    // The `::` variant covers PHP (Log::info) and C++ (Logger::error)
    for obj in patterns::LOG_OBJECTS {
        for method in patterns::LOG_METHODS {
            let dot_pattern = format!("{obj}.{method}(");
            let scope_pattern = format!("{obj}::{method}(");
            if call_ref.contains(&dot_pattern) || call_ref.contains(&scope_pattern) {
                let pii = patterns::contains_pii(call_ref);
                extraction.sinks.push(Sink {
                    sink_type: SinkType::Log,
                    anchor: anchor_from_node(node, file_path),
                    text: call_ref.to_string(),
                    contains_pii: pii,
                });
                return;
            }
        }
    }
}

/// Truncate a call expression text to a maximum display length.
pub fn truncate_call_text(text: String, max_len: usize) -> String {
    if text.len() > max_len {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    } else {
        text
    }
}

/// Create a new empty `FileExtraction` for the given file and language.
pub fn new_extraction(
    file_path: &Path,
    language: crate::parser::SupportedLanguage,
) -> FileExtraction {
    FileExtraction {
        file: file_path.to_path_buf(),
        language,
        interfaces: Vec::new(),
        dependencies: Vec::new(),
        sinks: Vec::new(),
        imports: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
        data_models: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_call_text_works() {
        assert_eq!(truncate_call_text("short".into(), 100), "short");
        let long = "a".repeat(120);
        let truncated = truncate_call_text(long, 100);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 100);
    }

    #[test]
    fn strip_quotes_removes_all_quote_types() {
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("`hello`"), "hello");
        assert_eq!(strip_quotes("no_quotes"), "no_quotes");
        assert_eq!(strip_quotes("''"), "");
    }

    #[test]
    fn extract_string_value_from_quoted_text() {
        assert_eq!(
            extract_string_value("'/api/users'"),
            Some("/api/users".into())
        );
        assert_eq!(
            extract_string_value("\"/api/users\""),
            Some("/api/users".into())
        );
        assert_eq!(
            extract_string_value("`/api/users`"),
            Some("/api/users".into())
        );
        assert_eq!(extract_string_value("variable"), None);
        assert_eq!(extract_string_value("123"), None);
        assert_eq!(
            extract_string_value("  '/spaced'  "),
            Some("/spaced".into())
        );
    }

    #[test]
    fn parse_http_method_case_insensitive() {
        assert_eq!(parse_http_method("get"), Some(HttpMethod::Get));
        assert_eq!(parse_http_method("GET"), Some(HttpMethod::Get));
        assert_eq!(parse_http_method("Get"), Some(HttpMethod::Get));
        assert_eq!(parse_http_method("post"), Some(HttpMethod::Post));
        assert_eq!(parse_http_method("POST"), Some(HttpMethod::Post));
        assert_eq!(parse_http_method("put"), Some(HttpMethod::Put));
        assert_eq!(parse_http_method("delete"), Some(HttpMethod::Delete));
        assert_eq!(parse_http_method("patch"), Some(HttpMethod::Patch));
        assert_eq!(parse_http_method("options"), Some(HttpMethod::Options));
        assert_eq!(parse_http_method("head"), Some(HttpMethod::Head));
        assert_eq!(parse_http_method("all"), Some(HttpMethod::All));
        assert_eq!(parse_http_method("unknown"), None);
        assert_eq!(parse_http_method(""), None);
    }

    #[test]
    #[allow(deprecated)]
    fn is_call_node_matches_all_language_variants() {
        assert!(is_call_node("call_expression"));
        assert!(is_call_node("call"));
        assert!(is_call_node("method_invocation"));
        assert!(is_call_node("invocation_expression"));
        assert!(is_call_node("member_call_expression"));
        assert!(is_call_node("function_call_expression"));
        assert!(is_call_node("scoped_call_expression"));
        assert!(is_call_node("method_call"));
        assert!(!is_call_node("identifier"));
        assert!(!is_call_node("expression_statement"));
    }
}
