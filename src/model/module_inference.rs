//! Infer logical module boundaries from project structure and symbol visibility.
//!
//! This module groups source files by their first-level subdirectory under the
//! project root, collects public symbols per group, and computes inter-module
//! dependencies from relative import sources.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::model::types::{ImportInfo, ModuleBoundary, Symbol, Visibility};

/// Name used for files that live directly in the project root with no
/// subdirectory structure.
const ROOT_MODULE_NAME: &str = "__root__";

/// Infer module boundaries from project structure.
///
/// Groups files by their first-level subdirectory under the project root,
/// collects public symbols per group, and computes inter-module dependencies
/// from import sources.
///
/// # Strategy
///
/// 1. Group all files by first-level directory (e.g., `src/users/handler.ts` -> `src/users`).
///    Files directly in the root go into a `"__root__"` module.
/// 2. For each group, collect symbols with `Visibility::Public`.
/// 3. Compute inter-module dependencies:
///    - For relative imports (`./`, `../`), resolve which module the target belongs to.
///    - A module depends on another if any of its files import from the other module.
///
/// Returns a sorted `Vec<ModuleBoundary>` (sorted by name for determinism).
pub fn infer_module_boundaries(
    file_symbols: &HashMap<PathBuf, Vec<Symbol>>,
    file_imports: &HashMap<PathBuf, Vec<ImportInfo>>,
    project_root: &Path,
) -> Vec<ModuleBoundary> {
    // Step 1: Group files by module name.
    // We use BTreeMap for deterministic iteration order.
    let mut module_files: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();

    // Gather all file paths from both maps (a file might appear in one but
    // not the other).
    let all_files: BTreeSet<&PathBuf> = file_symbols
        .keys()
        .chain(file_imports.keys())
        .collect();

    for file_path in &all_files {
        let module_name = module_name_for_file(file_path, project_root);
        module_files
            .entry(module_name)
            .or_default()
            .insert((*file_path).clone());
    }

    // Step 2 & 3: For each module, collect public symbols and compute dependencies.
    let mut boundaries: Vec<ModuleBoundary> = Vec::with_capacity(module_files.len());

    for (module_name, files) in &module_files {
        // Collect public symbols from all files in this module.
        let mut exported_symbols: BTreeSet<String> = BTreeSet::new();
        for file_path in files {
            if let Some(symbols) = file_symbols.get(file_path) {
                for symbol in symbols {
                    if symbol.visibility == Some(Visibility::Public) {
                        exported_symbols.insert(symbol.name.clone());
                    }
                }
            }
        }

        // Compute inter-module dependencies from relative imports.
        let mut depends_on: BTreeSet<String> = BTreeSet::new();
        for file_path in files {
            if let Some(imports) = file_imports.get(file_path) {
                for import in imports {
                    if !is_relative_import(&import.source) {
                        continue;
                    }

                    // Resolve the import relative to the importing file's directory.
                    let resolved = resolve_relative_import(file_path, &import.source);
                    let target_module = module_name_for_file(&resolved, project_root);

                    if &target_module != module_name {
                        depends_on.insert(target_module);
                    }
                }
            }
        }

        boundaries.push(ModuleBoundary {
            name: module_name.clone(),
            files: files.iter().cloned().collect(),
            exported_symbols: exported_symbols.into_iter().collect(),
            depends_on: depends_on.into_iter().collect(),
        });
    }

    // boundaries is already sorted because we iterate a BTreeMap.
    boundaries
}

/// Determine the module name for a file path relative to the project root.
///
/// Takes the first two path components after stripping the project root.
/// - `project/src/users/handler.ts` with root `project/` -> `src/users`
/// - `project/src/index.ts` with root `project/` -> `src`
/// - `project/main.go` with root `project/` -> `__root__`
fn module_name_for_file(file_path: &Path, project_root: &Path) -> String {
    let relative = file_path
        .strip_prefix(project_root)
        .unwrap_or(file_path);

    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    // We need at least 2 components for a real module (dir + file).
    // If we have 3+ (dir/subdir/file), take the first two as the module name.
    // If we have exactly 2 (dir/file), the module is the first component.
    // If we have 0 or 1 (just a filename or nothing), it belongs to __root__.
    match components.len() {
        0 | 1 => ROOT_MODULE_NAME.to_string(),
        2 => components[0].to_string(),
        _ => format!("{}/{}", components[0], components[1]),
    }
}

/// Check if an import source is relative (starts with `./` or `../`).
fn is_relative_import(source: &str) -> bool {
    source.starts_with("./") || source.starts_with("../")
}

/// Resolve a relative import path against the importing file's directory.
///
/// For example, if `file_path` is `src/users/handler.ts` and `import_source`
/// is `../orders/service`, the result is `src/orders/service`.
///
/// This performs logical path resolution (handling `..` components) without
/// requiring filesystem access.
fn resolve_relative_import(file_path: &Path, import_source: &str) -> PathBuf {
    let base_dir = file_path
        .parent()
        .unwrap_or(Path::new(""));

    let import_path = Path::new(import_source);
    let combined = base_dir.join(import_path);

    // Normalize the path by resolving `.` and `..` components.
    normalize_path(&combined)
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> PathBuf {
    let mut result = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {
                // Skip `.`
            }
            std::path::Component::ParentDir => {
                // Pop the last component for `..`
                result.pop();
            }
            other => {
                result.push(other.as_os_str().to_owned());
            }
        }
    }

    result.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::{SourceAnchor, SymbolKind};

    /// Helper to create a public symbol with minimal fields.
    fn public_symbol(name: &str, file: &str) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
            anchor: SourceAnchor::from_line_range(PathBuf::from(file), 1, 10),
            doc: None,
            signature: None,
            visibility: Some(Visibility::Public),
            parent: None,
        }
    }

    /// Helper to create a private symbol with minimal fields.
    fn private_symbol(name: &str, file: &str) -> Symbol {
        Symbol {
            name: name.into(),
            kind: SymbolKind::Function,
            anchor: SourceAnchor::from_line_range(PathBuf::from(file), 1, 10),
            doc: None,
            signature: None,
            visibility: Some(Visibility::Private),
            parent: None,
        }
    }

    /// Helper to create an import with minimal fields.
    fn import(source: &str) -> ImportInfo {
        ImportInfo {
            source: source.into(),
            specifiers: vec![],
            line: 1,
        }
    }

    #[test]
    fn groups_files_by_first_level_directory() {
        let root = Path::new("/project");

        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();
        file_symbols.insert(
            PathBuf::from("/project/src/users/handler.ts"),
            vec![public_symbol("handleUser", "/project/src/users/handler.ts")],
        );
        file_symbols.insert(
            PathBuf::from("/project/src/users/service.ts"),
            vec![public_symbol("UserService", "/project/src/users/service.ts")],
        );
        file_symbols.insert(
            PathBuf::from("/project/src/orders/handler.ts"),
            vec![public_symbol("handleOrder", "/project/src/orders/handler.ts")],
        );

        let file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();

        let boundaries = infer_module_boundaries(&file_symbols, &file_imports, root);

        assert_eq!(boundaries.len(), 2);

        let users_module = boundaries.iter().find(|m| m.name == "src/users").unwrap();
        assert_eq!(users_module.files.len(), 2);

        let orders_module = boundaries.iter().find(|m| m.name == "src/orders").unwrap();
        assert_eq!(orders_module.files.len(), 1);
    }

    #[test]
    fn collects_only_public_symbols_per_module() {
        let root = Path::new("/project");

        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();
        file_symbols.insert(
            PathBuf::from("/project/src/payments/service.ts"),
            vec![
                public_symbol("PaymentService", "/project/src/payments/service.ts"),
                private_symbol("internalHelper", "/project/src/payments/service.ts"),
                public_symbol("processPayment", "/project/src/payments/service.ts"),
            ],
        );

        let file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();

        let boundaries = infer_module_boundaries(&file_symbols, &file_imports, root);

        assert_eq!(boundaries.len(), 1);
        let module = &boundaries[0];
        assert_eq!(module.name, "src/payments");
        assert_eq!(module.exported_symbols, vec!["PaymentService", "processPayment"]);
    }

    #[test]
    fn computes_inter_module_dependencies_from_relative_imports() {
        let root = Path::new("/project");

        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();
        file_symbols.insert(
            PathBuf::from("/project/src/users/handler.ts"),
            vec![public_symbol("handleUser", "/project/src/users/handler.ts")],
        );
        file_symbols.insert(
            PathBuf::from("/project/src/orders/handler.ts"),
            vec![public_symbol("handleOrder", "/project/src/orders/handler.ts")],
        );

        let mut file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();
        file_imports.insert(
            PathBuf::from("/project/src/users/handler.ts"),
            vec![
                // Relative import pointing to another module.
                import("../orders/service"),
                // Package import — should be ignored.
                import("express"),
            ],
        );

        let boundaries = infer_module_boundaries(&file_symbols, &file_imports, root);

        let users_module = boundaries.iter().find(|m| m.name == "src/users").unwrap();
        assert_eq!(users_module.depends_on, vec!["src/orders"]);

        let orders_module = boundaries.iter().find(|m| m.name == "src/orders").unwrap();
        assert!(orders_module.depends_on.is_empty());
    }

    #[test]
    fn root_files_go_to_root_module() {
        let root = Path::new("/project");

        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();
        file_symbols.insert(
            PathBuf::from("/project/main.go"),
            vec![public_symbol("Main", "/project/main.go")],
        );
        file_symbols.insert(
            PathBuf::from("/project/config.go"),
            vec![public_symbol("LoadConfig", "/project/config.go")],
        );

        let file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();

        let boundaries = infer_module_boundaries(&file_symbols, &file_imports, root);

        assert_eq!(boundaries.len(), 1);
        let module = &boundaries[0];
        assert_eq!(module.name, "__root__");
        assert_eq!(module.files.len(), 2);
        assert_eq!(module.exported_symbols, vec!["LoadConfig", "Main"]);
    }
}
