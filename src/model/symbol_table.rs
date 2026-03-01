//! Two-level symbol table for cross-file reference resolution.
//!
//! Provides a per-file exact lookup (Level 1) and a global fuzzy lookup
//! (Level 2) to resolve call references that extractors leave as unresolved.
//!
//! Resolution chain (in priority order):
//! 1. **Import-based** (0.95) — symbol resolved via explicit import
//! 2. **Same-file** (0.90) — symbol found in the same file
//! 3. **Global unique** (0.80) — exactly one global match
//! 4. **Global same-directory** (0.60) — prefer match in same directory
//! 5. **Global ambiguous** (0.40) — multiple matches, pick first deterministically
//! 6. **Unresolved** (0.0) — no match found

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::model::types::{ResolutionMethod, Symbol, SymbolKind};

/// Location of a symbol in the codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLocation {
    /// File containing the symbol definition.
    pub file: PathBuf,
    /// 1-based line number of the definition.
    pub line: usize,
    /// Kind of symbol (function, class, method, etc.).
    pub kind: SymbolKind,
    /// Enclosing parent name (class, module, impl block).
    pub parent: Option<String>,
}

/// Two-level symbol table for name resolution.
///
/// Level 1: per-file exact lookup `(file, name) -> SymbolLocation`
/// Level 2: global fuzzy lookup `name -> Vec<SymbolLocation>`
pub struct SymbolTable {
    /// Level 1: exact file-scoped lookup.
    per_file: HashMap<(PathBuf, String), SymbolLocation>,
    /// Level 2: global name -> all locations.
    global: HashMap<String, Vec<SymbolLocation>>,
}

/// Result of resolving a symbol name.
pub struct ResolveResult {
    /// The resolved location, if found.
    pub location: Option<SymbolLocation>,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// How the resolution was achieved.
    pub method: ResolutionMethod,
}

impl SymbolTable {
    /// Build a symbol table from per-file symbol lists.
    pub fn from_file_symbols(file_symbols: &HashMap<PathBuf, Vec<Symbol>>) -> Self {
        let mut per_file = HashMap::new();
        let mut global: HashMap<String, Vec<SymbolLocation>> = HashMap::new();

        for (file, symbols) in file_symbols {
            for symbol in symbols {
                let location = SymbolLocation {
                    file: file.clone(),
                    line: symbol.anchor.line,
                    kind: symbol.kind,
                    parent: symbol.parent.clone(),
                };

                per_file.insert((file.clone(), symbol.name.clone()), location.clone());

                global
                    .entry(symbol.name.clone())
                    .or_default()
                    .push(location);
            }
        }

        // Sort global entries by file path for deterministic resolution
        for locations in global.values_mut() {
            locations.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        }

        Self { per_file, global }
    }

    /// Level 1: exact lookup by (file, name).
    pub fn resolve_in_file(&self, file: &Path, name: &str) -> Option<&SymbolLocation> {
        self.per_file.get(&(file.to_path_buf(), name.to_string()))
    }

    /// Level 2: global lookup by name — returns all matches.
    pub fn resolve_global(&self, name: &str) -> &[SymbolLocation] {
        self.global.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Resolve a symbol name using the full heuristic chain.
    ///
    /// Resolution order:
    /// 1. Import-based: check `imported_symbols` map, resolve in target file
    /// 2. Same-file: check `source_file`
    /// 3. Global unique: exactly one global match
    /// 4. Global same-directory: prefer match in same directory as source
    /// 5. Global ambiguous: multiple matches, pick first deterministically
    /// 6. Unresolved: no match
    pub fn resolve(
        &self,
        name: &str,
        source_file: &Path,
        imported_symbols: &HashMap<String, PathBuf>,
    ) -> ResolveResult {
        let bare_name = extract_bare_name(name);

        // 1. Import-based resolution (0.95)
        if let Some(target_file) = imported_symbols.get(bare_name) {
            if let Some(loc) = self.resolve_in_file(target_file, bare_name) {
                return ResolveResult {
                    location: Some(loc.clone()),
                    confidence: 0.95,
                    method: ResolutionMethod::ImportBased,
                };
            }
        }

        // 2. Same-file resolution (0.90)
        if let Some(loc) = self.resolve_in_file(source_file, bare_name) {
            return ResolveResult {
                location: Some(loc.clone()),
                confidence: 0.90,
                method: ResolutionMethod::SameFile,
            };
        }

        // 3-5. Global resolution
        let global_matches = self.resolve_global(bare_name);
        match global_matches.len() {
            0 => ResolveResult {
                location: None,
                confidence: 0.0,
                method: ResolutionMethod::Unresolved,
            },
            1 => ResolveResult {
                location: Some(global_matches[0].clone()),
                confidence: 0.80,
                method: ResolutionMethod::GlobalUnique,
            },
            _ => {
                // Prefer match in same directory as source file
                let source_dir = source_file.parent();
                if let Some(dir) = source_dir {
                    if let Some(loc) = global_matches.iter().find(|l| l.file.parent() == Some(dir))
                    {
                        return ResolveResult {
                            location: Some(loc.clone()),
                            confidence: 0.60,
                            method: ResolutionMethod::GlobalAmbiguous,
                        };
                    }
                }

                // Fall back to first match (deterministic since sorted)
                ResolveResult {
                    location: Some(global_matches[0].clone()),
                    confidence: 0.40,
                    method: ResolutionMethod::GlobalAmbiguous,
                }
            }
        }
    }
}

/// Build an import index mapping imported names to their source files.
///
/// For each file, examines its imports and resolves them to target files
/// using the `file_symbols` map (same logic as `import_resolver`).
pub fn build_import_index(
    file: &Path,
    imports: &[crate::model::types::ImportInfo],
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    project_root: &Path,
) -> HashMap<String, PathBuf> {
    let mut index = HashMap::new();

    for import in imports {
        let is_relative = import.source.starts_with("./") || import.source.starts_with("../");
        if !is_relative {
            continue; // External imports can't be resolved to project files
        }

        // Resolve the import path to a file using the same logic as import_resolver
        if let Some(resolved_file) =
            resolve_relative_path_for_index(file, &import.source, file_symbols, project_root)
        {
            let specifiers = if import.specifiers.is_empty() {
                vec![import.source.clone()]
            } else {
                import.specifiers.clone()
            };

            for specifier in specifiers {
                index.insert(specifier, resolved_file.clone());
            }
        }
    }

    index
}

/// Resolve a relative import path to a concrete file.
///
/// Reuses the same resolution strategy as `import_resolver::resolve_relative_path`:
/// 1. Try exact path
/// 2. Try path + known extensions
/// 3. Try path as directory + index files
fn resolve_relative_path_for_index(
    importing_file: &Path,
    source: &str,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    project_root: &Path,
) -> Option<PathBuf> {
    let base_dir = importing_file.parent()?;
    let raw_path = base_dir.join(source);
    let normalized = normalize_path(&raw_path);
    let relative = normalized
        .strip_prefix(project_root)
        .map(|p| p.to_path_buf())
        .unwrap_or(normalized);

    // 1. Exact path
    if file_symbols.contains_key(&relative) {
        return Some(relative);
    }

    // 2. Try extensions
    const RESOLVE_EXTENSIONS: &[&str] = &[
        ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".rs", ".java", ".cs", ".rb", ".php",
    ];
    for ext in RESOLVE_EXTENSIONS {
        let mut s = relative.as_os_str().to_os_string();
        s.push(ext);
        let with_ext = PathBuf::from(s);
        if file_symbols.contains_key(&with_ext) {
            return Some(with_ext);
        }
    }

    // 3. Try index files
    const INDEX_FILES: &[&str] = &["index.ts", "index.js"];
    for index_name in INDEX_FILES {
        let index_path = relative.join(index_name);
        if file_symbols.contains_key(&index_path) {
            return Some(index_path);
        }
    }

    None
}

/// Extract the bare function/method name from a qualified callee string.
///
/// Examples:
/// - `"user.save"` → `"save"`
/// - `"Cls::method"` → `"method"`
/// - `"validate"` → `"validate"`
/// - `"a.b.c"` → `"c"`
fn extract_bare_name(target_symbol: &str) -> &str {
    // Try :: separator first (Rust, PHP, C++)
    if let Some(pos) = target_symbol.rfind("::") {
        return &target_symbol[pos + 2..];
    }
    // Try . separator (JS/TS, Python, Java, Go)
    if let Some(pos) = target_symbol.rfind('.') {
        return &target_symbol[pos + 1..];
    }
    // Try -> separator (PHP)
    if let Some(pos) = target_symbol.rfind("->") {
        return &target_symbol[pos + 2..];
    }
    target_symbol
}

/// Normalize a path by resolving `.` and `..` components lexically.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if let Some(std::path::Component::Normal(_)) = components.last() {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::SourceAnchor;

    fn make_symbol(name: &str, file: &str, line: usize, kind: SymbolKind) -> Symbol {
        Symbol {
            name: name.into(),
            kind,
            anchor: SourceAnchor::from_line_range(PathBuf::from(file), line, line + 10),
            doc: None,
            signature: None,
            visibility: None,
            parent: None,
        }
    }

    fn make_file_symbols() -> HashMap<PathBuf, Vec<Symbol>> {
        let mut fs = HashMap::new();
        fs.insert(
            PathBuf::from("src/handler.ts"),
            vec![
                make_symbol("handleRequest", "src/handler.ts", 5, SymbolKind::Function),
                make_symbol("validate", "src/handler.ts", 20, SymbolKind::Function),
            ],
        );
        fs.insert(
            PathBuf::from("src/service.ts"),
            vec![
                make_symbol("UserService", "src/service.ts", 1, SymbolKind::Class),
                make_symbol("getUser", "src/service.ts", 10, SymbolKind::Method),
            ],
        );
        fs.insert(
            PathBuf::from("src/utils/helpers.ts"),
            vec![make_symbol(
                "validate",
                "src/utils/helpers.ts",
                1,
                SymbolKind::Function,
            )],
        );
        fs
    }

    // --- SymbolTable construction ---

    #[test]
    fn from_file_symbols_builds_both_levels() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        // Level 1: exact lookup
        let loc = table
            .resolve_in_file(Path::new("src/handler.ts"), "handleRequest")
            .unwrap();
        assert_eq!(loc.line, 5);
        assert_eq!(loc.kind, SymbolKind::Function);

        // Level 2: global lookup
        let globals = table.resolve_global("UserService");
        assert_eq!(globals.len(), 1);
        assert_eq!(globals[0].file, PathBuf::from("src/service.ts"));
    }

    #[test]
    fn resolve_in_file_not_found() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        assert!(table
            .resolve_in_file(Path::new("src/handler.ts"), "nonexistent")
            .is_none());
    }

    #[test]
    fn resolve_global_unique() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let globals = table.resolve_global("UserService");
        assert_eq!(globals.len(), 1);
    }

    #[test]
    fn resolve_global_multiple_matches() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        // "validate" exists in both handler.ts and utils/helpers.ts
        let globals = table.resolve_global("validate");
        assert_eq!(globals.len(), 2);
    }

    #[test]
    fn resolve_global_not_found() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let globals = table.resolve_global("doesNotExist");
        assert!(globals.is_empty());
    }

    // --- Heuristic resolution chain ---

    #[test]
    fn resolve_via_import_based() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let mut imports = HashMap::new();
        imports.insert("UserService".to_string(), PathBuf::from("src/service.ts"));

        let result = table.resolve("UserService", Path::new("src/handler.ts"), &imports);

        assert!(result.location.is_some());
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.method, ResolutionMethod::ImportBased);
        assert_eq!(
            result.location.unwrap().file,
            PathBuf::from("src/service.ts")
        );
    }

    #[test]
    fn resolve_via_same_file() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new(); // no imports

        let result = table.resolve("validate", Path::new("src/handler.ts"), &imports);

        assert!(result.location.is_some());
        assert_eq!(result.confidence, 0.90);
        assert_eq!(result.method, ResolutionMethod::SameFile);
    }

    #[test]
    fn resolve_via_global_unique() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new();

        // "UserService" is globally unique and not in the source file
        let result = table.resolve("UserService", Path::new("src/utils/helpers.ts"), &imports);

        assert!(result.location.is_some());
        assert_eq!(result.confidence, 0.80);
        assert_eq!(result.method, ResolutionMethod::GlobalUnique);
    }

    #[test]
    fn resolve_via_global_ambiguous_prefers_same_directory() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new();

        // "validate" is ambiguous (handler.ts + utils/helpers.ts)
        // Source file is in src/, so src/handler.ts should be preferred
        let result = table.resolve(
            "validate",
            Path::new("src/routes.ts"), // same dir as handler.ts
            &imports,
        );

        assert!(result.location.is_some());
        assert_eq!(result.method, ResolutionMethod::GlobalAmbiguous);
        assert_eq!(result.confidence, 0.60);
        assert_eq!(
            result.location.unwrap().file,
            PathBuf::from("src/handler.ts")
        );
    }

    #[test]
    fn resolve_via_global_ambiguous_fallback() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new();

        // "validate" is ambiguous, source file is in a different directory
        let result = table.resolve("validate", Path::new("other/dir/file.ts"), &imports);

        assert!(result.location.is_some());
        assert_eq!(result.method, ResolutionMethod::GlobalAmbiguous);
        assert_eq!(result.confidence, 0.40);
    }

    #[test]
    fn resolve_unresolved() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new();

        let result = table.resolve("nonexistent", Path::new("src/handler.ts"), &imports);

        assert!(result.location.is_none());
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.method, ResolutionMethod::Unresolved);
    }

    // --- Bare name extraction ---

    #[test]
    fn extract_bare_name_from_dot_qualified() {
        assert_eq!(extract_bare_name("user.save"), "save");
        assert_eq!(extract_bare_name("a.b.c"), "c");
    }

    #[test]
    fn extract_bare_name_from_scope_qualified() {
        assert_eq!(extract_bare_name("Cls::method"), "method");
        assert_eq!(extract_bare_name("std::io::read"), "read");
    }

    #[test]
    fn extract_bare_name_from_arrow_qualified() {
        assert_eq!(extract_bare_name("$obj->method"), "method");
    }

    #[test]
    fn extract_bare_name_simple() {
        assert_eq!(extract_bare_name("validate"), "validate");
    }

    // --- Import index builder ---

    #[test]
    fn build_import_index_resolves_relative_imports() {
        let fs = make_file_symbols();
        let imports = vec![crate::model::types::ImportInfo {
            source: "./service".into(),
            specifiers: vec!["UserService".into()],
            line: 1,
        }];

        let index = build_import_index(Path::new("src/handler.ts"), &imports, &fs, Path::new(""));

        assert_eq!(
            index.get("UserService"),
            Some(&PathBuf::from("src/service.ts"))
        );
    }

    #[test]
    fn build_import_index_skips_external_imports() {
        let fs = make_file_symbols();
        let imports = vec![crate::model::types::ImportInfo {
            source: "express".into(),
            specifiers: vec!["express".into()],
            line: 1,
        }];

        let index = build_import_index(Path::new("src/handler.ts"), &imports, &fs, Path::new(""));

        assert!(index.is_empty());
    }

    // --- Resolve with qualified names (integration test) ---

    #[test]
    fn resolve_strips_receiver_for_method_call() {
        let fs = make_file_symbols();
        let table = SymbolTable::from_file_symbols(&fs);

        let imports = HashMap::new();

        // "user.getUser" should strip to "getUser" and find it
        let result = table.resolve("user.getUser", Path::new("src/handler.ts"), &imports);

        assert!(result.location.is_some());
        assert_eq!(
            result.location.unwrap().file,
            PathBuf::from("src/service.ts")
        );
    }
}
