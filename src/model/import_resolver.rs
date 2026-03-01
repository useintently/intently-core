//! Cross-file import resolution for the CodeModel.
//!
//! This module performs a post-processing step after per-file extraction:
//! it takes the aggregated imports and symbols from all files and resolves
//! relative imports to concrete file paths and symbol definitions. Each
//! resolved import produces a [`Reference`] with [`ReferenceKind::Import`].
//!
//! Resolution strategy:
//! - **Relative imports** (`./foo`, `../bar`): resolve to a filesystem path
//!   by trying common extensions and index file conventions, then match
//!   specifiers against the target file's exported symbols.
//! - **Package imports** (`express`, `lodash`): marked as external with
//!   `target_file: None` since the definition lives outside the project.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::{ImportInfo, Reference, ReferenceKind, Symbol};

/// File extensions to try when resolving a relative import path.
///
/// Ordered by frequency in typical polyglot codebases. When a relative
/// import like `./user-service` is encountered, we try appending each
/// extension until a file is found in `file_symbols`.
const RESOLVE_EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".rs", ".java", ".cs", ".rb", ".php",
];

/// Index file names to try for directory imports (JS/TS convention).
///
/// When `./utils` resolves to a directory, these are tried in order:
/// `./utils/index.ts`, `./utils/index.js`.
const INDEX_FILES: &[&str] = &["index.ts", "index.js"];

/// Resolve imports to cross-file references.
///
/// Takes a map of file -> (imports) and a map of file -> (symbols), plus
/// the project root, then tries to resolve each import to a concrete file
/// and symbol.
///
/// Resolution strategy:
/// 1. Relative imports (`./foo`, `../bar`) -- resolve filesystem path,
///    look up symbols in target file.
/// 2. Named imports (`{ Foo } from './bar'`) -- find `Foo` in target file's symbols.
/// 3. Package imports (`express`, `lodash`) -- mark as external (`target_file: None`).
///
/// Returns a `Vec<Reference>` with `ReferenceKind::Import` for each resolved import.
pub fn resolve_imports(
    file_imports: &HashMap<PathBuf, Vec<ImportInfo>>,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    project_root: &Path,
) -> Vec<Reference> {
    // Build symbol index: name -> Vec<(file, line)> for fast lookup.
    let symbol_index = build_symbol_index(file_symbols);

    let mut references = Vec::new();

    for (importing_file, imports) in file_imports {
        for import in imports {
            let resolved_refs = resolve_single_import(
                importing_file,
                import,
                file_symbols,
                &symbol_index,
                project_root,
            );
            references.extend(resolved_refs);
        }
    }

    // Sort for deterministic output: by (source_file, source_line, target_symbol).
    references.sort_by(|a, b| {
        (&a.source_file, a.source_line, &a.target_symbol).cmp(&(
            &b.source_file,
            b.source_line,
            &b.target_symbol,
        ))
    });

    references
}

/// Build an index mapping symbol names to their (file, line) locations.
fn build_symbol_index(
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
) -> HashMap<String, Vec<(PathBuf, usize)>> {
    let mut index: HashMap<String, Vec<(PathBuf, usize)>> = HashMap::new();
    for (file, symbols) in file_symbols {
        for symbol in symbols {
            index
                .entry(symbol.name.clone())
                .or_default()
                .push((file.clone(), symbol.anchor.line));
        }
    }
    index
}

/// Resolve a single import statement into zero or more references.
///
/// Creates one `Reference` per specifier in the import. For imports without
/// explicit specifiers (e.g., `import express from 'express'`), the source
/// string itself is used as the target symbol.
fn resolve_single_import(
    importing_file: &Path,
    import: &ImportInfo,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    symbol_index: &HashMap<String, Vec<(PathBuf, usize)>>,
    project_root: &Path,
) -> Vec<Reference> {
    let is_relative = import.source.starts_with("./") || import.source.starts_with("../");

    if is_relative {
        resolve_relative_import(importing_file, import, file_symbols, project_root)
    } else {
        resolve_external_import(importing_file, import, symbol_index)
    }
}

/// Resolve a relative import (starts with `./` or `../`).
///
/// Attempts to find the target file by resolving the path relative to the
/// importing file's directory, trying common extensions and index files.
/// For each specifier, looks up the symbol in the resolved file.
fn resolve_relative_import(
    importing_file: &Path,
    import: &ImportInfo,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    project_root: &Path,
) -> Vec<Reference> {
    let resolved_path =
        resolve_relative_path(importing_file, &import.source, file_symbols, project_root);

    let specifiers = effective_specifiers(import);

    specifiers
        .iter()
        .map(|specifier| {
            let (target_file, target_line) = match &resolved_path {
                Some(path) => {
                    let line = find_symbol_in_file(specifier, path, file_symbols);
                    (Some(path.clone()), line)
                }
                None => (None, None),
            };

            Reference {
                source_symbol: String::new(),
                source_file: importing_file.to_path_buf(),
                source_line: import.line,
                target_symbol: specifier.clone(),
                target_file,
                target_line,
                reference_kind: ReferenceKind::Import,
            }
        })
        .collect()
}

/// Resolve a package/external import (no `./` or `../` prefix).
///
/// External imports cannot be resolved to a file within the project, so
/// `target_file` is always `None`.
fn resolve_external_import(
    importing_file: &Path,
    import: &ImportInfo,
    _symbol_index: &HashMap<String, Vec<(PathBuf, usize)>>,
) -> Vec<Reference> {
    let specifiers = effective_specifiers(import);

    specifiers
        .iter()
        .map(|specifier| Reference {
            source_symbol: String::new(),
            source_file: importing_file.to_path_buf(),
            source_line: import.line,
            target_symbol: specifier.clone(),
            target_file: None,
            target_line: None,
            reference_kind: ReferenceKind::Import,
        })
        .collect()
}

/// Return the effective specifiers for an import.
///
/// If the import has explicit specifiers, use those. Otherwise, use the
/// source module name as the specifier (e.g., `import express from 'express'`
/// uses `"express"` as the target symbol).
fn effective_specifiers(import: &ImportInfo) -> Vec<String> {
    if import.specifiers.is_empty() {
        vec![import.source.clone()]
    } else {
        import.specifiers.clone()
    }
}

/// Resolve a relative import source to a concrete file path.
///
/// Tries the following in order:
/// 1. Exact path (already has extension and exists in `file_symbols`)
/// 2. Path + each extension in `RESOLVE_EXTENSIONS`
/// 3. Path as directory + each entry in `INDEX_FILES`
///
/// All paths are canonicalized relative to the importing file's parent
/// directory and then made relative to `project_root` for consistency
/// with the keys in `file_symbols`.
fn resolve_relative_path(
    importing_file: &Path,
    source: &str,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    project_root: &Path,
) -> Option<PathBuf> {
    let base_dir = importing_file.parent()?;
    let raw_path = base_dir.join(source);

    // Normalize the path (resolve `.` and `..` components) without
    // requiring the path to exist on the filesystem. We use a simple
    // component-based normalization instead of `canonicalize()` because
    // the files may only exist in the `file_symbols` map (e.g., in tests).
    let normalized = normalize_path(&raw_path);

    // Make it relative to project_root if possible.
    let relative = make_relative(&normalized, project_root);

    // 1. Try the exact path.
    if file_symbols.contains_key(&relative) {
        return Some(relative);
    }

    // 2. Try appending each known extension.
    for ext in RESOLVE_EXTENSIONS {
        let with_ext = append_extension(&relative, ext);
        if file_symbols.contains_key(&with_ext) {
            return Some(with_ext);
        }
    }

    // 3. Try as directory with index files.
    for index_name in INDEX_FILES {
        let index_path = relative.join(index_name);
        if file_symbols.contains_key(&index_path) {
            return Some(index_path);
        }
    }

    None
}

/// Find a symbol by name in a specific file's symbol list.
///
/// Returns the line number if found, `None` otherwise.
fn find_symbol_in_file(
    symbol_name: &str,
    file: &Path,
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
) -> Option<usize> {
    file_symbols
        .get(file)?
        .iter()
        .find(|s| s.name == symbol_name)
        .map(|s| s.anchor.line)
}

/// Normalize a path by resolving `.` and `..` components lexically.
///
/// Unlike `std::fs::canonicalize`, this does not require the path to exist
/// on the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => { /* skip `.` */ }
            std::path::Component::ParentDir => {
                // Pop the last normal component if possible.
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

/// Make a path relative to `root` if it starts with `root`.
///
/// Returns the path unchanged if it is not under `root`.
fn make_relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Append an extension string (e.g., `".ts"`) to a path.
fn append_extension(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(ext);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::{SourceAnchor, SymbolKind};

    /// Helper: create a minimal Symbol for testing.
    fn make_symbol(name: &str, file: &str, line: usize) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Class,
            anchor: SourceAnchor::from_line_range(PathBuf::from(file), line, line + 10),
            doc: None,
            signature: None,
            visibility: None,
            parent: None,
        }
    }

    #[test]
    fn relative_import_resolves_to_correct_file() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/handler.ts"),
            vec![ImportInfo {
                source: "./service".into(),
                specifiers: vec!["UserService".into()],
                line: 1,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/service.ts"),
            vec![make_symbol("UserService", "src/service.ts", 5)],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_symbol, "UserService");
        assert_eq!(r.target_file, Some(PathBuf::from("src/service.ts")));
        assert_eq!(r.target_line, Some(5));
        assert_eq!(r.reference_kind, ReferenceKind::Import);
        assert_eq!(r.source_file, PathBuf::from("src/handler.ts"));
        assert_eq!(r.source_line, 1);
        assert!(r.source_symbol.is_empty());
    }

    #[test]
    fn named_import_finds_matching_symbol_in_target_file() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "./utils/auth".into(),
                specifiers: vec!["verifyToken".into()],
                line: 3,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/utils/auth.ts"),
            vec![
                make_symbol("verifyToken", "src/utils/auth.ts", 10),
                make_symbol("generateToken", "src/utils/auth.ts", 25),
            ],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_symbol, "verifyToken");
        assert_eq!(r.target_file, Some(PathBuf::from("src/utils/auth.ts")));
        assert_eq!(r.target_line, Some(10));
    }

    #[test]
    fn package_import_stays_unresolved() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "express".into(),
                specifiers: vec!["express".into()],
                line: 1,
            }],
        );

        let file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_symbol, "express");
        assert!(r.target_file.is_none());
        assert!(r.target_line.is_none());
        assert_eq!(r.reference_kind, ReferenceKind::Import);
    }

    #[test]
    fn import_with_multiple_specifiers_creates_multiple_references() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/handler.ts"),
            vec![ImportInfo {
                source: "./models".into(),
                specifiers: vec!["User".into(), "Order".into(), "Product".into()],
                line: 2,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/models.ts"),
            vec![
                make_symbol("User", "src/models.ts", 1),
                make_symbol("Order", "src/models.ts", 20),
                make_symbol("Product", "src/models.ts", 40),
            ],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 3);
        // All references point to the same source file and line.
        for r in &refs {
            assert_eq!(r.source_file, PathBuf::from("src/handler.ts"));
            assert_eq!(r.source_line, 2);
            assert_eq!(r.target_file, Some(PathBuf::from("src/models.ts")));
            assert_eq!(r.reference_kind, ReferenceKind::Import);
        }
        // Sorted by target_symbol.
        let target_symbols: Vec<&str> = refs.iter().map(|r| r.target_symbol.as_str()).collect();
        assert_eq!(target_symbols, vec!["Order", "Product", "User"]);
    }

    #[test]
    fn nonexistent_relative_import_creates_reference_with_target_file_none() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "./does-not-exist".into(),
                specifiers: vec!["Foo".into()],
                line: 5,
            }],
        );

        let file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_symbol, "Foo");
        assert!(r.target_file.is_none());
        assert!(r.target_line.is_none());
    }

    #[test]
    fn path_resolution_handles_multiple_extensions() {
        // The import says `./service` and the file is `src/service.js`
        // (not `.ts`). Extension resolution should still find it.
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "./service".into(),
                specifiers: vec!["createApp".into()],
                line: 1,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/service.js"),
            vec![make_symbol("createApp", "src/service.js", 3)],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_file, Some(PathBuf::from("src/service.js")));
        assert_eq!(r.target_line, Some(3));
    }

    #[test]
    fn empty_imports_produce_no_references() {
        let file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();
        let file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert!(refs.is_empty());
    }

    #[test]
    fn external_imports_without_dot_prefix_mark_as_external() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/main.ts"),
            vec![
                ImportInfo {
                    source: "lodash".into(),
                    specifiers: vec!["debounce".into()],
                    line: 1,
                },
                ImportInfo {
                    source: "@nestjs/common".into(),
                    specifiers: vec!["Controller".into(), "Get".into()],
                    line: 2,
                },
            ],
        );

        let file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 3);
        for r in &refs {
            assert!(
                r.target_file.is_none(),
                "external import should have no target_file"
            );
            assert!(
                r.target_line.is_none(),
                "external import should have no target_line"
            );
        }

        let target_symbols: Vec<&str> = refs.iter().map(|r| r.target_symbol.as_str()).collect();
        assert!(target_symbols.contains(&"debounce"));
        assert!(target_symbols.contains(&"Controller"));
        assert!(target_symbols.contains(&"Get"));
    }

    // --- Internal helper tests ---

    #[test]
    fn normalize_path_resolves_dot_and_dotdot() {
        let path = Path::new("src/handlers/../utils/./auth");
        let normalized = normalize_path(path);
        assert_eq!(normalized, PathBuf::from("src/utils/auth"));
    }

    #[test]
    fn index_file_resolution_for_directory_imports() {
        // Import `./components` should resolve to `src/components/index.ts`
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "./components".into(),
                specifiers: vec!["Button".into()],
                line: 1,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/components/index.ts"),
            vec![make_symbol("Button", "src/components/index.ts", 2)],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(
            r.target_file,
            Some(PathBuf::from("src/components/index.ts"))
        );
        assert_eq!(r.target_line, Some(2));
    }

    #[test]
    fn parent_directory_relative_import_resolves() {
        // `src/handlers/user.ts` imports `../models/user`
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/handlers/user.ts"),
            vec![ImportInfo {
                source: "../models/user".into(),
                specifiers: vec!["UserModel".into()],
                line: 1,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/models/user.ts"),
            vec![make_symbol("UserModel", "src/models/user.ts", 8)],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.target_file, Some(PathBuf::from("src/models/user.ts")));
        assert_eq!(r.target_line, Some(8));
    }

    #[test]
    fn specifier_not_found_in_resolved_file_has_none_target_line() {
        // The file resolves, but the specific symbol is not in it.
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "./service".into(),
                specifiers: vec!["NonexistentSymbol".into()],
                line: 1,
            }],
        );

        let mut file_symbols = HashMap::new();
        file_symbols.insert(
            PathBuf::from("src/service.ts"),
            vec![make_symbol("ActualSymbol", "src/service.ts", 5)],
        );

        let refs = resolve_imports(&file_imports, &file_symbols, Path::new(""));

        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        // File resolves, but symbol does not exist in it.
        assert_eq!(r.target_file, Some(PathBuf::from("src/service.ts")));
        assert!(r.target_line.is_none());
    }
}
