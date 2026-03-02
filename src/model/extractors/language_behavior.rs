//! Language-specific behavioral traits.
//!
//! Defines the [`LanguageBehavior`] trait that encapsulates language-specific
//! conventions: module separators, source directory roots, visibility parsing,
//! signature extraction, doc comment extraction, parent name resolution, and
//! call node identification.
//!
//! Each language family gets a unit struct implementing the trait. The factory
//! function [`behavior_for`] maps [`SupportedLanguage`] variants to the correct
//! behavior instance, enabling downstream consumers to work polymorphically
//! without language-specific dispatch logic.

use tree_sitter::Node;

use crate::model::types::Visibility;
use crate::parser::SupportedLanguage;

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Language-specific behavioral conventions.
///
/// Provides default implementations where a sensible cross-language default
/// exists. Language-specific structs override only the methods that differ
/// from the defaults.
pub trait LanguageBehavior: Send + Sync {
    /// The separator used between module/namespace segments.
    ///
    /// Examples: `"."` for JavaScript, `"::"` for Rust, `"\\"` for PHP.
    fn module_separator(&self) -> &'static str {
        "."
    }

    /// Common source directory roots for this language.
    ///
    /// Used by module inference to strip prefix paths. For example,
    /// Java projects typically place source in `src/main/java/`.
    fn source_roots(&self) -> &[&str] {
        &["src"]
    }

    /// Extract visibility from a tree-sitter CST node.
    ///
    /// Returns `None` when the language has no visibility concept for the
    /// given node or when the visibility cannot be determined.
    fn parse_visibility(&self, _node: &Node, _source: &str) -> Option<Visibility> {
        None
    }

    /// The character that opens a function/method body.
    ///
    /// Used by [`extract_signature`](LanguageBehavior::extract_signature) to
    /// truncate the declaration at the body boundary. Returns `None` for
    /// languages where signature extraction uses a different strategy
    /// (e.g., Ruby takes the first line).
    fn signature_body_opener(&self) -> Option<char> {
        Some('{')
    }

    /// Extract the declaration signature from a definition node.
    ///
    /// Default: truncates the node text at [`signature_body_opener`](LanguageBehavior::signature_body_opener).
    fn extract_signature(&self, node: &Node, source: &str) -> Option<String> {
        let node_text = node.utf8_text(source.as_bytes()).ok()?;

        let truncated = match self.signature_body_opener() {
            Some(opener) => truncate_at_char(node_text, opener),
            None => node_text.lines().next().map(|l| l.to_string()),
        };

        let sig = truncated.as_deref().unwrap_or(node_text).trim().to_string();

        if sig.is_empty() {
            None
        } else {
            Some(sig)
        }
    }

    /// Extract a doc comment above the given node.
    ///
    /// Default: looks for `/** ... */`, `///`, or `//` comment siblings
    /// preceding the node (C-family convention).
    fn extract_doc_comment(&self, node: &Node, source: &str) -> Option<String> {
        extract_block_or_line_comment(node, source)
    }

    /// Find the name of the enclosing class, module, trait, or impl block.
    ///
    /// Default: walks up the CST looking for enclosing type definition nodes.
    fn find_parent_name(&self, node: &Node, source: &str) -> Option<String> {
        find_parent_generic(node, source)
    }

    /// CST node kinds that represent function/method calls.
    ///
    /// Used by call graph extraction to identify call sites.
    fn call_node_kinds(&self) -> &[&str] {
        &["call_expression"]
    }

    /// Determine whether a symbol definition node represents a test function.
    ///
    /// Language-specific detection patterns include:
    /// - **Naming conventions:** `test_*` (Python, Ruby, PHP), `Test*` (Go)
    /// - **Annotations/attributes:** `@Test` (Java/Kotlin), `[Test]`/`[Fact]`/`[Theory]` (C#),
    ///   `#[test]` (Rust), `#[Test]` (PHP)
    /// - **File heuristic:** TS/JS functions named `test*` in test files
    ///
    /// Returns `false` by default (GenericBehavior and languages without test patterns).
    fn is_test_symbol(&self, _node: &Node, _source: &str, _symbol_name: &str) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Behavior implementations
// ---------------------------------------------------------------------------

/// Behavior for TypeScript, TSX, JavaScript, and JSX.
pub(crate) struct TypeScriptBehavior;

impl LanguageBehavior for TypeScriptBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["src", "lib", "app"]
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        let text = node.utf8_text(source.as_bytes()).ok()?;
        if text.starts_with("export") {
            return Some(Visibility::Public);
        }
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return Some(Visibility::Public);
            }
        }
        None
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call_expression"]
    }

    /// TS/JS: functions named `test*` are considered test symbols.
    ///
    /// This is a name-prefix heuristic. BDD-style `describe`/`it` blocks
    /// are call_expressions, not function_declarations — not detected here.
    fn is_test_symbol(&self, _node: &Node, _source: &str, symbol_name: &str) -> bool {
        symbol_name.starts_with("test")
    }
}

/// Behavior for Python.
pub(crate) struct PythonBehavior;

impl LanguageBehavior for PythonBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["src", "app"]
    }

    fn signature_body_opener(&self) -> Option<char> {
        Some(':')
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        // Python uses naming convention — extract name from the node.
        // Both `_private` (convention) and `__mangled` (name-mangling)
        // are treated as Private in our model.
        let name = extract_name_from_node(node, source)?;
        if name.starts_with('_') {
            Some(Visibility::Private)
        } else {
            Some(Visibility::Public)
        }
    }

    fn extract_doc_comment(&self, node: &Node, source: &str) -> Option<String> {
        extract_python_docstring(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call"]
    }

    /// Python: `def test_*` or methods inside `unittest.TestCase` subclasses.
    fn is_test_symbol(&self, _node: &Node, _source: &str, symbol_name: &str) -> bool {
        symbol_name.starts_with("test_") || symbol_name.starts_with("test")
    }
}

/// Behavior for Java and Kotlin.
pub(crate) struct JavaBehavior;

impl LanguageBehavior for JavaBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["src/main/java", "src"]
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        extract_visibility_modifier_child(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["method_invocation"]
    }

    /// Java/Kotlin: check for `@Test` annotation on the preceding sibling.
    fn is_test_symbol(&self, node: &Node, source: &str, _symbol_name: &str) -> bool {
        has_preceding_annotation(node, source, &["Test"])
    }
}

/// Behavior for C#.
pub(crate) struct CSharpBehavior;

impl LanguageBehavior for CSharpBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["src", "Controllers", "Services"]
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        // C# uses `modifier` child nodes (includes `internal`)
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32) {
                if child.kind() == "modifier" {
                    let mod_text = match child.utf8_text(source.as_bytes()) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    if let Some(vis) = parse_visibility_keyword(mod_text) {
                        return Some(vis);
                    }
                }
            }
        }
        // Fallback: check first word of node text
        let text = node.utf8_text(source.as_bytes()).ok()?;
        let first_word = text.split_whitespace().next()?;
        parse_visibility_keyword(first_word)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["invocation_expression"]
    }

    /// C#: check for `[Test]`, `[Fact]`, or `[Theory]` attributes.
    fn is_test_symbol(&self, node: &Node, source: &str, _symbol_name: &str) -> bool {
        has_preceding_attribute(node, source, &["Test", "Fact", "Theory"])
    }
}

/// Behavior for Go.
pub(crate) struct GoBehavior;

impl LanguageBehavior for GoBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["cmd", "internal", "pkg"]
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        // Go uses capitalization convention
        let name = extract_name_from_node(node, source)?;
        let first_char = name.chars().next()?;
        if first_char.is_uppercase() {
            Some(Visibility::Public)
        } else {
            Some(Visibility::Private)
        }
    }

    fn find_parent_name(&self, node: &Node, source: &str) -> Option<String> {
        // Go methods have a receiver type — extract it from method_declaration
        if node.kind() == "method_declaration" {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i as u32) {
                    if child.kind() == "parameter_list" {
                        let text = child.utf8_text(source.as_bytes()).ok()?;
                        let cleaned = text.trim_matches(|c| c == '(' || c == ')');
                        let type_name = cleaned.split_whitespace().last()?.trim_start_matches('*');
                        return Some(type_name.to_string());
                    }
                }
            }
        }
        find_parent_generic(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call_expression"]
    }

    /// Go: `func Test*(t *testing.T)` — name starts with `Test` and has
    /// a `testing.T` or `testing.B` or `testing.M` parameter.
    fn is_test_symbol(&self, node: &Node, source: &str, symbol_name: &str) -> bool {
        if !symbol_name.starts_with("Test")
            && !symbol_name.starts_with("Benchmark")
            && !symbol_name.starts_with("Fuzz")
        {
            return false;
        }
        // Verify the function signature contains testing.T/B/M/F
        let text = match node.utf8_text(source.as_bytes()) {
            Ok(t) => t,
            Err(_) => return false,
        };
        text.contains("testing.T")
            || text.contains("testing.B")
            || text.contains("testing.M")
            || text.contains("testing.F")
    }
}

/// Behavior for PHP.
pub(crate) struct PhpBehavior;

impl LanguageBehavior for PhpBehavior {
    fn module_separator(&self) -> &'static str {
        "\\"
    }

    fn source_roots(&self) -> &[&str] {
        &["src", "app"]
    }

    fn parse_visibility(&self, node: &Node, source: &str) -> Option<Visibility> {
        extract_visibility_modifier_child(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &[
            "member_call_expression",
            "function_call_expression",
            "scoped_call_expression",
        ]
    }

    /// PHP: `function test*()` name prefix or `#[Test]` attribute.
    fn is_test_symbol(&self, node: &Node, source: &str, symbol_name: &str) -> bool {
        if symbol_name.starts_with("test") {
            return true;
        }
        has_preceding_attribute(node, source, &["Test"])
    }
}

/// Behavior for Ruby.
pub(crate) struct RubyBehavior;

impl LanguageBehavior for RubyBehavior {
    fn module_separator(&self) -> &'static str {
        "::"
    }

    fn source_roots(&self) -> &[&str] {
        &["app", "lib"]
    }

    fn signature_body_opener(&self) -> Option<char> {
        // Ruby: take the first line as the signature
        None
    }

    fn extract_doc_comment(&self, node: &Node, source: &str) -> Option<String> {
        extract_hash_comment(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call", "method_call"]
    }

    /// Ruby: `def test_*` naming convention.
    fn is_test_symbol(&self, _node: &Node, _source: &str, symbol_name: &str) -> bool {
        symbol_name.starts_with("test_")
    }
}

/// Behavior for Rust.
pub(crate) struct RustBehavior;

impl LanguageBehavior for RustBehavior {
    fn module_separator(&self) -> &'static str {
        "::"
    }

    fn source_roots(&self) -> &[&str] {
        &["src"]
    }

    fn parse_visibility(&self, node: &Node, _source: &str) -> Option<Visibility> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i as u32) {
                if child.kind() == "visibility_modifier" {
                    return Some(Visibility::Public);
                }
            }
        }
        Some(Visibility::Private)
    }

    fn extract_doc_comment(&self, node: &Node, source: &str) -> Option<String> {
        extract_rust_doc_comment(node, source)
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call_expression"]
    }

    /// Rust: `#[test]` or `#[tokio::test]` attribute on the function.
    fn is_test_symbol(&self, node: &Node, source: &str, _symbol_name: &str) -> bool {
        has_preceding_rust_attribute(node, source, &["test", "tokio::test", "rstest"])
    }
}

/// Fallback behavior for C, C++, Swift, and Scala.
pub(crate) struct GenericBehavior;

impl LanguageBehavior for GenericBehavior {
    fn module_separator(&self) -> &'static str {
        "."
    }

    fn source_roots(&self) -> &[&str] {
        &["src"]
    }

    fn call_node_kinds(&self) -> &[&str] {
        &["call_expression"]
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

// Static instances for each behavior (unit structs, zero-cost).
static TYPESCRIPT_BEHAVIOR: TypeScriptBehavior = TypeScriptBehavior;
static PYTHON_BEHAVIOR: PythonBehavior = PythonBehavior;
static JAVA_BEHAVIOR: JavaBehavior = JavaBehavior;
static CSHARP_BEHAVIOR: CSharpBehavior = CSharpBehavior;
static GO_BEHAVIOR: GoBehavior = GoBehavior;
static PHP_BEHAVIOR: PhpBehavior = PhpBehavior;
static RUBY_BEHAVIOR: RubyBehavior = RubyBehavior;
static RUST_BEHAVIOR: RustBehavior = RustBehavior;
static GENERIC_BEHAVIOR: GenericBehavior = GenericBehavior;

/// Return the [`LanguageBehavior`] implementation for a given language.
///
/// Languages that share a grammar family map to the same behavior:
/// - TypeScript, TSX, JavaScript, JSX -> [`TypeScriptBehavior`]
/// - Java, Kotlin, Scala -> [`JavaBehavior`]
/// - C, C++, Swift -> [`GenericBehavior`]
pub fn behavior_for(language: SupportedLanguage) -> &'static dyn LanguageBehavior {
    match language {
        SupportedLanguage::TypeScript
        | SupportedLanguage::Tsx
        | SupportedLanguage::JavaScript
        | SupportedLanguage::Jsx => &TYPESCRIPT_BEHAVIOR,
        SupportedLanguage::Python => &PYTHON_BEHAVIOR,
        SupportedLanguage::Java | SupportedLanguage::Kotlin => &JAVA_BEHAVIOR,
        SupportedLanguage::CSharp => &CSHARP_BEHAVIOR,
        SupportedLanguage::Go => &GO_BEHAVIOR,
        SupportedLanguage::Php => &PHP_BEHAVIOR,
        SupportedLanguage::Ruby => &RUBY_BEHAVIOR,
        SupportedLanguage::Rust => &RUST_BEHAVIOR,
        SupportedLanguage::Swift | SupportedLanguage::C | SupportedLanguage::Cpp => {
            &GENERIC_BEHAVIOR
        }
        SupportedLanguage::Scala => &JAVA_BEHAVIOR,
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (used by trait implementations)
// ---------------------------------------------------------------------------

/// Truncate text at the first occurrence of `ch`, trimming whitespace.
fn truncate_at_char(text: &str, ch: char) -> Option<String> {
    text.find(ch).map(|pos| text[..pos].trim().to_string())
}

/// Parse a visibility keyword string into [`Visibility`].
fn parse_visibility_keyword(keyword: &str) -> Option<Visibility> {
    match keyword.trim() {
        "public" => Some(Visibility::Public),
        "private" => Some(Visibility::Private),
        "protected" => Some(Visibility::Protected),
        "internal" => Some(Visibility::Internal),
        _ => None,
    }
}

/// Look for visibility modifier keywords in child nodes (Java/PHP pattern).
fn extract_visibility_modifier_child(node: &Node, source: &str) -> Option<Visibility> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            let kind = child.kind();
            if kind == "modifiers" || kind == "modifier" || kind == "visibility_modifier" {
                let mod_text = match child.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if let Some(vis) = parse_visibility_keyword(mod_text) {
                    return Some(vis);
                }
            }
            // Direct keyword nodes (some grammars use these)
            if let Some(vis) = parse_visibility_keyword(kind) {
                return Some(vis);
            }
        }
    }
    // Fallback: check if text starts with a visibility keyword
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let first_word = text.split_whitespace().next()?;
    parse_visibility_keyword(first_word)
}

/// Extract the name identifier from a node (looks for common name child kinds).
fn extract_name_from_node(node: &Node, source: &str) -> Option<String> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i as u32) {
            let kind = child.kind();
            if kind == "identifier"
                || kind == "type_identifier"
                || kind == "name"
                || kind == "constant"
                || kind == "property_identifier"
                || kind == "field_identifier"
            {
                return child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

/// Generic parent finder: walk up looking for class/module/trait/impl nodes.
fn find_parent_generic(node: &Node, source: &str) -> Option<String> {
    let mut current = node.parent()?;
    loop {
        let kind = current.kind();
        if is_enclosing_type(kind) {
            return extract_name_child(&current, source);
        }
        current = current.parent()?;
    }
}

/// Check if a CST node kind represents an enclosing type definition.
fn is_enclosing_type(kind: &str) -> bool {
    matches!(
        kind,
        "class_declaration"
            | "class_definition"
            | "class"
            | "record_declaration"
            | "interface_declaration"
            | "trait_item"
            | "trait_declaration"
            | "impl_item"
            | "struct_declaration"
            | "struct_item"
            | "enum_declaration"
            | "enum_item"
            | "module"
            | "mod_item"
    )
}

/// Extract the `name:` child text from an enclosing type node.
fn extract_name_child(node: &Node, source: &str) -> Option<String> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i as u32) {
            let kind = child.kind();
            if kind == "identifier"
                || kind == "type_identifier"
                || kind == "name"
                || kind == "constant"
                || kind == "property_identifier"
            {
                return child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Test detection helpers
// ---------------------------------------------------------------------------

/// Check for a Java/Kotlin annotation (`@Name`) on a method_declaration node.
///
/// Java tree-sitter grammar nests annotations inside `modifiers` child nodes
/// of the declaration. We walk the node's children looking for `modifiers`
/// containing `marker_annotation` or `annotation` nodes, and also check
/// direct preceding siblings (some grammar versions place them there).
fn has_preceding_annotation(node: &Node, source: &str, names: &[&str]) -> bool {
    // Strategy 1: Check child `modifiers` node (Java grammar nests annotations here)
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            let kind = child.kind();
            if kind == "modifiers" {
                // Walk inside modifiers for annotations
                for j in 0..child.child_count() {
                    if let Some(mod_child) = child.child(j as u32) {
                        if check_annotation_node(&mod_child, source, names) {
                            return true;
                        }
                    }
                }
            }
            // Direct annotation child (some grammar versions)
            if check_annotation_node(&child, source, names) {
                return true;
            }
        }
    }

    // Strategy 2: Check preceding siblings (fallback)
    let mut sib = node.prev_named_sibling();
    while let Some(s) = sib {
        if check_annotation_node(&s, source, names) {
            return true;
        }
        let kind = s.kind();
        if kind != "marker_annotation"
            && kind != "annotation"
            && kind != "modifiers"
            && kind != "modifier"
        {
            break;
        }
        sib = s.prev_named_sibling();
    }

    false
}

/// Check if a single CST node is an annotation matching one of the target names.
fn check_annotation_node(node: &Node, source: &str, names: &[&str]) -> bool {
    let kind = node.kind();
    if kind == "marker_annotation" || kind == "annotation" {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            let ann_name = text.trim_start_matches('@');
            for name in names {
                if ann_name == *name || ann_name.starts_with(&format!("{name}(")) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check for a C# attribute (`[Name]`) on a declaration node.
///
/// C# tree-sitter grammar nests attributes as child `attribute_list` nodes
/// of the declaration. We check both child nodes and preceding siblings.
fn has_preceding_attribute(node: &Node, source: &str, names: &[&str]) -> bool {
    // Strategy 1: Check child nodes (C# grammar nests attributes as children)
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            let kind = child.kind();
            if kind == "attribute_list" || kind == "attribute" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    for name in names {
                        if text.contains(name) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    // Strategy 2: Check preceding siblings (fallback)
    let mut sib = node.prev_named_sibling();
    while let Some(s) = sib {
        let kind = s.kind();
        if kind == "attribute_list" || kind == "attribute" {
            if let Ok(text) = s.utf8_text(source.as_bytes()) {
                for name in names {
                    if text.contains(name) {
                        return true;
                    }
                }
            }
        }
        if kind != "attribute_list" && kind != "attribute" && kind != "modifier" {
            break;
        }
        sib = s.prev_named_sibling();
    }
    false
}

/// Check for a Rust `#[name]` attribute_item preceding the function.
///
/// Rust tree-sitter grammar uses `attribute_item` nodes as siblings before
/// function_item nodes.
fn has_preceding_rust_attribute(node: &Node, source: &str, names: &[&str]) -> bool {
    let mut sib = node.prev_named_sibling();
    while let Some(s) = sib {
        if s.kind() == "attribute_item" {
            if let Ok(text) = s.utf8_text(source.as_bytes()) {
                // text looks like `#[test]` or `#[tokio::test]`
                let inner = text.trim_start_matches("#[").trim_end_matches(']');
                for name in names {
                    if inner == *name || inner.starts_with(&format!("{name}(")) {
                        return true;
                    }
                }
            }
        }
        // Attribute items can be stacked — keep walking
        if s.kind() != "attribute_item" && s.kind() != "line_comment" {
            break;
        }
        sib = s.prev_named_sibling();
    }
    false
}

// ---------------------------------------------------------------------------
// Doc comment helpers
// ---------------------------------------------------------------------------

/// Find a comment node immediately preceding the given node.
fn find_preceding_comment<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    // Named sibling first
    if let Some(sib) = node.prev_named_sibling() {
        let kind = sib.kind();
        if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
            return Some(sib);
        }
    }
    // Walk unnamed siblings (comments are unnamed in some grammars like Java)
    let mut sib = node.prev_sibling();
    while let Some(s) = sib {
        let kind = s.kind();
        if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
            return Some(s);
        }
        if !s.is_named() {
            sib = s.prev_sibling();
            continue;
        }
        break;
    }
    None
}

/// C-family languages: look for `/** ... */`, `///`, or `//` comments preceding the node.
fn extract_block_or_line_comment(node: &Node, source: &str) -> Option<String> {
    let sibling = find_preceding_comment(node)?;
    let kind = sibling.kind();
    if kind != "comment" && kind != "line_comment" && kind != "block_comment" {
        return None;
    }
    let text = sibling.utf8_text(source.as_bytes()).ok()?;

    // JSDoc/JavaDoc style: /** ... */
    if text.starts_with("/**") {
        return Some(clean_block_comment(text));
    }
    // Triple-slash style: ///
    if text.starts_with("///") {
        let mut comments = vec![text.trim_start_matches("///").trim().to_string()];
        let mut sib = sibling.prev_named_sibling();
        while let Some(s) = sib {
            if s.kind() == "comment" || s.kind() == "line_comment" {
                let t = match s.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => break,
                };
                if t.starts_with("///") {
                    comments.push(t.trim_start_matches("///").trim().to_string());
                    sib = s.prev_named_sibling();
                    continue;
                }
            }
            break;
        }
        comments.reverse();
        return Some(comments.join("\n"));
    }
    // Go-style: single-line // comments
    if text.starts_with("//") {
        let mut comments = vec![text.trim_start_matches("//").trim().to_string()];
        let mut sib = sibling.prev_named_sibling();
        while let Some(s) = sib {
            if s.kind() == "comment" {
                let t = match s.utf8_text(source.as_bytes()) {
                    Ok(t) => t,
                    Err(_) => break,
                };
                if t.starts_with("//") {
                    comments.push(t.trim_start_matches("//").trim().to_string());
                    sib = s.prev_named_sibling();
                    continue;
                }
            }
            break;
        }
        comments.reverse();
        return Some(comments.join("\n"));
    }

    None
}

/// Python: extract docstring from the first expression_statement in the body.
fn extract_python_docstring(node: &Node, source: &str) -> Option<String> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i as u32) {
            if child.kind() == "block" {
                if let Some(first_stmt) = child.named_child(0) {
                    if first_stmt.kind() == "expression_statement" {
                        if let Some(str_node) = first_stmt.named_child(0) {
                            if str_node.kind() == "string"
                                || str_node.kind() == "concatenated_string"
                            {
                                let text = str_node.utf8_text(source.as_bytes()).ok()?;
                                return Some(clean_python_docstring(text));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Rust: look for `///` line comments preceding the node.
fn extract_rust_doc_comment(node: &Node, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut sibling = find_preceding_comment(node);
    while let Some(sib) = sibling {
        let kind = sib.kind();
        if kind == "line_comment" || kind == "comment" {
            let text = match sib.utf8_text(source.as_bytes()) {
                Ok(t) => t,
                Err(_) => break,
            };
            if text.starts_with("///") {
                comments.push(text.trim_start_matches("///").trim().to_string());
                sibling = find_preceding_comment(&sib);
                continue;
            }
        }
        break;
    }
    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    Some(comments.join("\n"))
}

/// Ruby: look for `#` comments preceding the node.
fn extract_hash_comment(node: &Node, source: &str) -> Option<String> {
    let mut comments = Vec::new();
    let mut sibling = find_preceding_comment(node);
    while let Some(sib) = sibling {
        if sib.kind() == "comment" {
            let text = match sib.utf8_text(source.as_bytes()) {
                Ok(t) => t,
                Err(_) => break,
            };
            if text.starts_with('#') {
                comments.push(text.trim_start_matches('#').trim().to_string());
                sibling = find_preceding_comment(&sib);
                continue;
            }
        }
        break;
    }
    if comments.is_empty() {
        return None;
    }
    comments.reverse();
    Some(comments.join("\n"))
}

/// Clean a `/** ... */` block comment.
fn clean_block_comment(text: &str) -> String {
    let trimmed = text.trim_start_matches("/**").trim_end_matches("*/").trim();
    trimmed
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clean a Python docstring (`"""..."""` or `'''...'''`).
fn clean_python_docstring(text: &str) -> String {
    let inner = text
        .trim_start_matches("\"\"\"")
        .trim_start_matches("'''")
        .trim_end_matches("\"\"\"")
        .trim_end_matches("'''")
        .trim();
    inner
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // module_separator
    // =======================================================================

    #[test]
    fn typescript_module_separator_is_dot() {
        assert_eq!(TypeScriptBehavior.module_separator(), ".");
    }

    #[test]
    fn python_module_separator_is_dot() {
        assert_eq!(PythonBehavior.module_separator(), ".");
    }

    #[test]
    fn java_module_separator_is_dot() {
        assert_eq!(JavaBehavior.module_separator(), ".");
    }

    #[test]
    fn csharp_module_separator_is_dot() {
        assert_eq!(CSharpBehavior.module_separator(), ".");
    }

    #[test]
    fn go_module_separator_is_dot() {
        assert_eq!(GoBehavior.module_separator(), ".");
    }

    #[test]
    fn php_module_separator_is_backslash() {
        assert_eq!(PhpBehavior.module_separator(), "\\");
    }

    #[test]
    fn ruby_module_separator_is_double_colon() {
        assert_eq!(RubyBehavior.module_separator(), "::");
    }

    #[test]
    fn rust_module_separator_is_double_colon() {
        assert_eq!(RustBehavior.module_separator(), "::");
    }

    #[test]
    fn generic_module_separator_is_dot() {
        assert_eq!(GenericBehavior.module_separator(), ".");
    }

    // =======================================================================
    // source_roots
    // =======================================================================

    #[test]
    fn typescript_source_roots() {
        let roots = TypeScriptBehavior.source_roots();
        assert!(roots.contains(&"src"));
        assert!(roots.contains(&"lib"));
        assert!(roots.contains(&"app"));
    }

    #[test]
    fn python_source_roots() {
        let roots = PythonBehavior.source_roots();
        assert!(roots.contains(&"src"));
        assert!(roots.contains(&"app"));
    }

    #[test]
    fn java_source_roots() {
        let roots = JavaBehavior.source_roots();
        assert!(roots.contains(&"src/main/java"));
        assert!(roots.contains(&"src"));
    }

    #[test]
    fn csharp_source_roots() {
        let roots = CSharpBehavior.source_roots();
        assert!(roots.contains(&"src"));
        assert!(roots.contains(&"Controllers"));
        assert!(roots.contains(&"Services"));
    }

    #[test]
    fn go_source_roots() {
        let roots = GoBehavior.source_roots();
        assert!(roots.contains(&"cmd"));
        assert!(roots.contains(&"internal"));
        assert!(roots.contains(&"pkg"));
    }

    #[test]
    fn php_source_roots() {
        let roots = PhpBehavior.source_roots();
        assert!(roots.contains(&"src"));
        assert!(roots.contains(&"app"));
    }

    #[test]
    fn ruby_source_roots() {
        let roots = RubyBehavior.source_roots();
        assert!(roots.contains(&"app"));
        assert!(roots.contains(&"lib"));
    }

    #[test]
    fn rust_source_roots() {
        let roots = RustBehavior.source_roots();
        assert!(roots.contains(&"src"));
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn generic_source_roots() {
        let roots = GenericBehavior.source_roots();
        assert!(roots.contains(&"src"));
    }

    // =======================================================================
    // call_node_kinds
    // =======================================================================

    #[test]
    fn typescript_call_node_kinds() {
        let kinds = TypeScriptBehavior.call_node_kinds();
        assert!(kinds.contains(&"call_expression"));
    }

    #[test]
    fn python_call_node_kinds() {
        let kinds = PythonBehavior.call_node_kinds();
        assert!(kinds.contains(&"call"));
    }

    #[test]
    fn java_call_node_kinds() {
        let kinds = JavaBehavior.call_node_kinds();
        assert!(kinds.contains(&"method_invocation"));
    }

    #[test]
    fn csharp_call_node_kinds() {
        let kinds = CSharpBehavior.call_node_kinds();
        assert!(kinds.contains(&"invocation_expression"));
    }

    #[test]
    fn go_call_node_kinds() {
        let kinds = GoBehavior.call_node_kinds();
        assert!(kinds.contains(&"call_expression"));
    }

    #[test]
    fn php_call_node_kinds() {
        let kinds = PhpBehavior.call_node_kinds();
        assert!(kinds.contains(&"member_call_expression"));
        assert!(kinds.contains(&"function_call_expression"));
        assert!(kinds.contains(&"scoped_call_expression"));
    }

    #[test]
    fn ruby_call_node_kinds() {
        let kinds = RubyBehavior.call_node_kinds();
        assert!(kinds.contains(&"call"));
        assert!(kinds.contains(&"method_call"));
    }

    #[test]
    fn rust_call_node_kinds() {
        let kinds = RustBehavior.call_node_kinds();
        assert!(kinds.contains(&"call_expression"));
    }

    #[test]
    fn generic_call_node_kinds() {
        let kinds = GenericBehavior.call_node_kinds();
        assert!(kinds.contains(&"call_expression"));
    }

    // =======================================================================
    // behavior_for factory
    // =======================================================================

    #[test]
    fn factory_maps_typescript_family() {
        let b = behavior_for(SupportedLanguage::TypeScript);
        assert_eq!(b.module_separator(), ".");
        assert!(b.source_roots().contains(&"lib"));

        // All JS-like languages should get the same behavior
        let tsx = behavior_for(SupportedLanguage::Tsx);
        assert_eq!(tsx.module_separator(), ".");
        assert!(tsx.source_roots().contains(&"lib"));

        let js = behavior_for(SupportedLanguage::JavaScript);
        assert_eq!(js.module_separator(), ".");

        let jsx = behavior_for(SupportedLanguage::Jsx);
        assert_eq!(jsx.module_separator(), ".");
    }

    #[test]
    fn factory_maps_python() {
        let b = behavior_for(SupportedLanguage::Python);
        assert_eq!(b.module_separator(), ".");
        assert!(b.call_node_kinds().contains(&"call"));
    }

    #[test]
    fn factory_maps_java_and_kotlin() {
        let java = behavior_for(SupportedLanguage::Java);
        assert!(java.call_node_kinds().contains(&"method_invocation"));

        let kotlin = behavior_for(SupportedLanguage::Kotlin);
        assert!(kotlin.call_node_kinds().contains(&"method_invocation"));
    }

    #[test]
    fn factory_maps_csharp() {
        let b = behavior_for(SupportedLanguage::CSharp);
        assert!(b.call_node_kinds().contains(&"invocation_expression"));
    }

    #[test]
    fn factory_maps_go() {
        let b = behavior_for(SupportedLanguage::Go);
        assert!(b.source_roots().contains(&"cmd"));
    }

    #[test]
    fn factory_maps_php() {
        let b = behavior_for(SupportedLanguage::Php);
        assert_eq!(b.module_separator(), "\\");
    }

    #[test]
    fn factory_maps_ruby() {
        let b = behavior_for(SupportedLanguage::Ruby);
        assert_eq!(b.module_separator(), "::");
    }

    #[test]
    fn factory_maps_rust() {
        let b = behavior_for(SupportedLanguage::Rust);
        assert_eq!(b.module_separator(), "::");
    }

    #[test]
    fn factory_maps_generic_languages() {
        for lang in [
            SupportedLanguage::C,
            SupportedLanguage::Cpp,
            SupportedLanguage::Swift,
        ] {
            let b = behavior_for(lang);
            assert_eq!(b.module_separator(), ".");
            assert!(b.call_node_kinds().contains(&"call_expression"));
        }
    }

    #[test]
    fn factory_maps_scala_to_java_behavior() {
        let b = behavior_for(SupportedLanguage::Scala);
        assert!(b.call_node_kinds().contains(&"method_invocation"));
        assert!(b.source_roots().contains(&"src/main/java"));
    }

    // =======================================================================
    // signature_body_opener
    // =======================================================================

    #[test]
    fn python_body_opener_is_colon() {
        assert_eq!(PythonBehavior.signature_body_opener(), Some(':'));
    }

    #[test]
    fn ruby_body_opener_is_none() {
        assert_eq!(RubyBehavior.signature_body_opener(), None);
    }

    #[test]
    fn c_family_body_opener_is_brace() {
        assert_eq!(TypeScriptBehavior.signature_body_opener(), Some('{'));
        assert_eq!(JavaBehavior.signature_body_opener(), Some('{'));
        assert_eq!(CSharpBehavior.signature_body_opener(), Some('{'));
        assert_eq!(GoBehavior.signature_body_opener(), Some('{'));
        assert_eq!(RustBehavior.signature_body_opener(), Some('{'));
        assert_eq!(GenericBehavior.signature_body_opener(), Some('{'));
    }

    // =======================================================================
    // truncate_at_char helper
    // =======================================================================

    #[test]
    fn truncate_at_char_finds_first_occurrence() {
        assert_eq!(
            truncate_at_char("fn main() {", '{'),
            Some("fn main()".to_string())
        );
    }

    #[test]
    fn truncate_at_char_returns_none_when_absent() {
        assert_eq!(truncate_at_char("no opener here", '{'), None);
    }

    #[test]
    fn truncate_at_char_trims_whitespace() {
        assert_eq!(
            truncate_at_char("def foo()  :", ':'),
            Some("def foo()".to_string())
        );
    }
}
