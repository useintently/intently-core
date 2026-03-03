//! Pattern engine wrapping ast-grep-core for structural code search.
//!
//! Implements the ast-grep `Language` and `LanguageExt` traits for our
//! `SupportedLanguage` enum, enabling pattern matching with metavariable
//! capture across all 16 supported languages.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};
use ast_grep_core::{Language, Pattern, PatternError};

use crate::parser::SupportedLanguage;

// ---------------------------------------------------------------------------
// ast-grep Language implementation for SupportedLanguage
// ---------------------------------------------------------------------------

/// Bridge between our `SupportedLanguage` and ast-grep's `Language` trait.
///
/// Each instance wraps a specific language and delegates tree-sitter
/// operations to our parser module. Languages where `$` is valid syntax
/// (PHP, Python, Rust) use `µ` as the expando character so the parser
/// treats metavariables as identifiers.
#[derive(Clone, Copy, Debug)]
pub struct AstGrepLang(pub SupportedLanguage);

impl Language for AstGrepLang {
    fn kind_to_id(&self, kind: &str) -> u16 {
        self.get_ts_language().id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.get_ts_language()
            .field_id_for_name(field)
            .map(|f| f.get())
    }

    fn build_pattern(
        &self,
        builder: &ast_grep_core::matcher::PatternBuilder,
    ) -> Result<Pattern, PatternError> {
        builder.build(|src| StrDoc::try_new(src, *self))
    }

    fn expando_char(&self) -> char {
        match self.0 {
            // JS/TS and Java allow $ in identifiers, so $VAR parses as
            // a valid identifier without any substitution needed.
            SupportedLanguage::TypeScript
            | SupportedLanguage::Tsx
            | SupportedLanguage::JavaScript
            | SupportedLanguage::Jsx
            | SupportedLanguage::Java => '$',
            // All other languages: $ is either invalid in identifiers
            // or has special meaning (PHP sigil, Ruby globals). Using µ
            // (a Unicode letter) as the expando char lets the parser
            // accept metavariables as regular identifiers.
            _ => 'µ',
        }
    }

    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        let meta = self.meta_var_char();
        let expando = self.expando_char();
        if meta == expando {
            Cow::Borrowed(query)
        } else {
            Cow::Owned(query.replace(meta, &expando.to_string()))
        }
    }
}

impl LanguageExt for AstGrepLang {
    fn get_ts_language(&self) -> TSLanguage {
        crate::parser::get_tree_sitter_language(self.0)
    }
}

// ---------------------------------------------------------------------------
// Public search types
// ---------------------------------------------------------------------------

/// A named pattern for structural matching.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchPattern {
    /// Unique identifier for this pattern.
    pub id: String,
    /// Language this pattern targets.
    pub language: SupportedLanguage,
    /// ast-grep pattern text (e.g., `console.$METHOD($$$ARGS)`).
    pub pattern_text: String,
    /// Descriptive category (e.g., "call_site", "type_declaration").
    pub kind: String,
}

/// A single match produced by structural search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatternMatch {
    /// Which pattern produced this match.
    pub pattern_id: String,
    /// File where the match was found.
    pub file: PathBuf,
    /// 1-based start line.
    pub line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Full text of the matched node.
    pub matched_text: String,
    /// Captured metavariables (e.g., `METHOD` → `log`).
    pub captured_vars: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// StructuralSearch engine
// ---------------------------------------------------------------------------

/// Engine for running ast-grep patterns against source code.
///
/// Holds a set of compiled patterns grouped by language. Each pattern
/// is compiled once and can be matched against multiple files.
pub struct StructuralSearch {
    /// Compiled patterns keyed by language → list of (pattern_id, kind, compiled_pattern).
    patterns: HashMap<SupportedLanguage, Vec<CompiledPattern>>,
}

struct CompiledPattern {
    id: String,
    #[allow(dead_code)]
    kind: String,
    pattern: Pattern,
}

impl StructuralSearch {
    /// Create a search engine from a list of pattern definitions.
    ///
    /// Each pattern is compiled against its target language. Returns an
    /// error if any pattern fails to compile.
    pub fn from_patterns(
        definitions: &[SearchPattern],
    ) -> Result<Self, crate::error::IntentlyError> {
        let mut patterns: HashMap<SupportedLanguage, Vec<CompiledPattern>> = HashMap::new();

        for def in definitions {
            let lang = AstGrepLang(def.language);
            let compiled = Pattern::new(&def.pattern_text, lang);
            patterns
                .entry(def.language)
                .or_default()
                .push(CompiledPattern {
                    id: def.id.clone(),
                    kind: def.kind.clone(),
                    pattern: compiled,
                });
        }

        Ok(Self { patterns })
    }

    /// Search a source file against all patterns for its language.
    ///
    /// Returns all matches with captured metavariables. The file is
    /// re-parsed by ast-grep (separate from our tree-sitter cache)
    /// since ast-grep owns its parse trees.
    pub fn search_file(
        &self,
        source: &str,
        language: SupportedLanguage,
        file_path: &Path,
    ) -> Vec<PatternMatch> {
        let compiled = match self.patterns.get(&language) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let lang = AstGrepLang(language);
        let sg = lang.ast_grep(source);
        let root = sg.root();
        let mut results = Vec::new();

        for cp in compiled {
            for m in root.find_all(&cp.pattern) {
                let start_pos = m.start_pos();
                let end_pos = m.end_pos();

                let mut captured_vars = HashMap::new();
                let env = m.get_env();
                for var in env.get_matched_variables() {
                    let var_name = match &var {
                        ast_grep_core::meta_var::MetaVariable::Capture(name, _) => name.clone(),
                        ast_grep_core::meta_var::MetaVariable::MultiCapture(name) => name.clone(),
                        ast_grep_core::meta_var::MetaVariable::Dropped(_)
                        | ast_grep_core::meta_var::MetaVariable::Multiple => continue,
                    };
                    if let Some(node) = env.get_match(&var_name) {
                        captured_vars.insert(var_name, node.text().to_string());
                    }
                }

                results.push(PatternMatch {
                    pattern_id: cp.id.clone(),
                    file: file_path.to_path_buf(),
                    line: start_pos.line() + 1,
                    end_line: end_pos.line() + 1,
                    matched_text: m.text().to_string(),
                    captured_vars,
                });
            }
        }

        results
    }

    /// Search a single source string against a single pattern.
    ///
    /// Convenience method for one-off searches without pre-compiling
    /// a full pattern set.
    pub fn search_single(
        source: &str,
        language: SupportedLanguage,
        pattern_text: &str,
        file_path: &Path,
    ) -> Vec<PatternMatch> {
        let lang = AstGrepLang(language);
        let compiled = Pattern::new(pattern_text, lang);
        let sg = lang.ast_grep(source);
        let root = sg.root();
        let mut results = Vec::new();

        for m in root.find_all(&compiled) {
            let start_pos = m.start_pos();
            let end_pos = m.end_pos();

            let mut captured_vars = HashMap::new();
            let env = m.get_env();
            for var in env.get_matched_variables() {
                let var_name = match &var {
                    ast_grep_core::meta_var::MetaVariable::Capture(name, _) => name.clone(),
                    ast_grep_core::meta_var::MetaVariable::MultiCapture(name) => name.clone(),
                    ast_grep_core::meta_var::MetaVariable::Dropped(_)
                    | ast_grep_core::meta_var::MetaVariable::Multiple => continue,
                };
                if let Some(node) = env.get_match(&var_name) {
                    captured_vars.insert(var_name, node.text().to_string());
                }
            }

            results.push(PatternMatch {
                pattern_id: String::new(),
                file: file_path.to_path_buf(),
                line: start_pos.line() + 1,
                end_line: end_pos.line() + 1,
                matched_text: m.text().to_string(),
                captured_vars,
            });
        }

        results
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.ts")
    }

    #[test]
    fn matches_console_log_in_typescript() {
        let source = r#"
function greet(name: string) {
    console.log("Hello", name);
    console.warn("debug");
}
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::TypeScript,
            "console.$METHOD($$$ARGS)",
            &test_path(),
        );
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].captured_vars.get("METHOD").unwrap(), "log");
        assert_eq!(matches[1].captured_vars.get("METHOD").unwrap(), "warn");
    }

    #[test]
    fn matches_function_calls_in_python() {
        let source = r#"
def main():
    result = process_data(items, batch_size=10)
    print(result)
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::Python,
            "print($ARG)",
            &PathBuf::from("test.py"),
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captured_vars.get("ARG").unwrap(), "result");
    }

    #[test]
    fn matches_express_routes_in_typescript() {
        let source = r#"
const app = express();
app.get('/health', (req, res) => res.json({ ok: true }));
app.post('/api/users', createUser);
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::TypeScript,
            "app.$METHOD($PATH, $$$HANDLER)",
            &test_path(),
        );
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].captured_vars.get("METHOD").unwrap(), "get");
        assert_eq!(matches[1].captured_vars.get("METHOD").unwrap(), "post");
    }

    #[test]
    fn returns_empty_for_no_matches() {
        let source = "const x = 42;";
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::TypeScript,
            "console.log($ARG)",
            &test_path(),
        );
        assert!(matches.is_empty());
    }

    #[test]
    fn returns_empty_for_wrong_language() {
        let patterns = vec![SearchPattern {
            id: "ts-console".into(),
            language: SupportedLanguage::TypeScript,
            pattern_text: "console.log($ARG)".into(),
            kind: "call_site".into(),
        }];
        let engine = StructuralSearch::from_patterns(&patterns).unwrap();

        // Search a Python file — no TypeScript patterns should match
        let matches = engine.search_file(
            "print('hello')",
            SupportedLanguage::Python,
            &PathBuf::from("t.py"),
        );
        assert!(matches.is_empty());
    }

    #[test]
    fn compiled_engine_matches_multiple_patterns() {
        let patterns = vec![
            SearchPattern {
                id: "console-log".into(),
                language: SupportedLanguage::TypeScript,
                pattern_text: "console.log($$$ARGS)".into(),
                kind: "call_site".into(),
            },
            SearchPattern {
                id: "console-error".into(),
                language: SupportedLanguage::TypeScript,
                pattern_text: "console.error($$$ARGS)".into(),
                kind: "call_site".into(),
            },
        ];
        let engine = StructuralSearch::from_patterns(&patterns).unwrap();
        let source = r#"
console.log("info");
console.error("oops");
console.warn("hmm");
"#;
        let matches = engine.search_file(source, SupportedLanguage::TypeScript, &test_path());
        assert_eq!(matches.len(), 2);

        let ids: Vec<&str> = matches.iter().map(|m| m.pattern_id.as_str()).collect();
        assert!(ids.contains(&"console-log"));
        assert!(ids.contains(&"console-error"));
    }

    #[test]
    fn match_lines_are_one_based() {
        let source = "const x = 1;\nconsole.log(x);\nconst y = 2;\n";
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::TypeScript,
            "console.log($ARG)",
            &test_path(),
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[0].end_line, 2);
    }

    #[test]
    fn matches_java_spring_annotation() {
        let source = r#"
@RestController
public class UserController {
    @GetMapping("/users")
    public List<User> getUsers() {
        return userService.findAll();
    }
}
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::Java,
            "userService.$METHOD()",
            &PathBuf::from("Controller.java"),
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captured_vars.get("METHOD").unwrap(), "findAll");
    }

    #[test]
    fn matches_go_variable_declarations() {
        // Go's grammar parses standalone `pkg.Func(...)` as type
        // conversions, not calls. Dot-notation call patterns don't work
        // without contextual wrapping. Variable/assignment patterns and
        // simple call patterns work fine.
        let source = r#"
package main

func compute() int {
    var result = 42
    return result
}
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::Go,
            "var $NAME = $VALUE",
            &PathBuf::from("main.go"),
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captured_vars.get("NAME").unwrap(), "result");
        assert_eq!(matches[0].captured_vars.get("VALUE").unwrap(), "42");
    }

    #[test]
    fn matches_rust_function_with_expando() {
        let source = r#"
fn process(items: &[Item]) -> Result<(), Error> {
    let result = compute(items);
    validate(result)?;
    Ok(())
}
"#;
        let matches = StructuralSearch::search_single(
            source,
            SupportedLanguage::Rust,
            "validate($ARG)",
            &PathBuf::from("lib.rs"),
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].captured_vars.get("ARG").unwrap(), "result");
    }
}
