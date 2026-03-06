//! Runtime Python import resolver.
//!
//! Spawns a Python subprocess to resolve dotted import paths to actual file
//! locations by leveraging Python's own `importlib` + `inspect` machinery.
//! This follows re-export chains that static analysis cannot trace: e.g.,
//! `torch.nn.Module` is re-exported through `__init__.py` files but actually
//! defined in `torch/nn/modules/module.py`.
//!
//! Gated by `#[cfg(feature = "python-resolver")]`. When the feature is disabled
//! or Python is not available, all functions gracefully return empty results.
//!
//! # Requirements
//!
//! - `python3` must be on `PATH`
//! - The project should be importable (dependencies installed or source on
//!   `PYTHONPATH`). The resolver adds `project_root` to `sys.path` automatically.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use tracing::{debug, info, warn};

/// A symbol resolved by the Python runtime to its actual defining file.
#[derive(Debug, Clone)]
pub struct PythonResolved {
    /// Absolute file path where the symbol is defined.
    pub file: PathBuf,
    /// 1-based line number of the definition (if available).
    pub line: Option<usize>,
}

/// Response entry from the Python resolver script.
#[derive(Deserialize)]
struct ResolvedEntry {
    file: String,
    line: Option<usize>,
}

/// Batch-resolve Python import symbols using the Python runtime.
///
/// Takes a list of `(module_path, symbols)` queries and spawns a single
/// Python subprocess that uses `importlib.import_module` + `inspect.getfile`
/// to locate where each symbol is actually defined.
///
/// Returns a map from `"module.Symbol"` keys to `PythonResolved` values.
/// Only symbols that resolve to files within `project_root` are included
/// (external packages are filtered out since they're not in the project).
///
/// Returns an empty map if:
/// - `queries` is empty
/// - `python3` is not available on PATH
/// - The Python subprocess fails
/// - No symbols resolve to project-internal files
pub fn batch_resolve(
    project_root: &Path,
    queries: &[(String, Vec<String>)],
) -> HashMap<String, PythonResolved> {
    if queries.is_empty() {
        return HashMap::new();
    }

    let total_symbols: usize = queries.iter().map(|(_, syms)| syms.len().max(1)).sum();
    info!(
        modules = queries.len(),
        symbols = total_symbols,
        "running Python runtime resolver"
    );

    // Build JSON input for the Python script
    let json_queries: Vec<serde_json::Value> = queries
        .iter()
        .map(|(module, symbols)| {
            serde_json::json!({
                "module": module,
                "symbols": symbols,
            })
        })
        .collect();

    let input = match serde_json::to_string(&json_queries) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to serialize Python resolver queries");
            return HashMap::new();
        }
    };

    // Spawn Python subprocess
    let child = Command::new("python3")
        .arg("-c")
        .arg(RESOLVER_SCRIPT)
        .arg(project_root.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            debug!(error = %e, "python3 not available — skipping runtime resolution");
            return HashMap::new();
        }
    };

    // Write queries to stdin
    if let Some(ref mut stdin) = child.stdin {
        if let Err(e) = stdin.write_all(input.as_bytes()) {
            warn!(error = %e, "failed to write to Python resolver stdin");
            return HashMap::new();
        }
    }
    // Drop stdin to signal EOF
    drop(child.stdin.take());

    // Wait for output
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "Python resolver process failed");
            return HashMap::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            status = ?output.status,
            stderr = %stderr,
            "Python resolver exited with error"
        );
        return HashMap::new();
    }

    parse_output(&output.stdout, project_root)
}

/// Parse the JSON output from the Python resolver script.
///
/// Filters results to only include files within `project_root`.
fn parse_output(stdout: &[u8], project_root: &Path) -> HashMap<String, PythonResolved> {
    let text = String::from_utf8_lossy(stdout);
    let entries: HashMap<String, ResolvedEntry> = match serde_json::from_str(&text) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "failed to parse Python resolver JSON output");
            return HashMap::new();
        }
    };

    let mut results = HashMap::with_capacity(entries.len());
    let mut in_project = 0usize;
    let mut external = 0usize;

    for (key, entry) in entries {
        let abs_path = PathBuf::from(&entry.file);
        if abs_path.starts_with(project_root) {
            in_project += 1;
            results.insert(
                key,
                PythonResolved {
                    file: abs_path,
                    line: entry.line,
                },
            );
        } else {
            external += 1;
        }
    }

    info!(
        in_project,
        external,
        total = in_project + external,
        "Python resolver completed"
    );

    results
}

/// Inline Python script for runtime import resolution.
///
/// The script:
/// 1. Adds `project_root` (argv[1]) to `sys.path`
/// 2. Reads JSON queries from stdin: `[{"module": "torch.nn", "symbols": ["Module"]}]`
/// 3. For each module+symbol, uses `importlib.import_module` + `inspect.getfile`
/// 4. Outputs JSON to stdout: `{"torch.nn.Module": {"file": "/path/to/module.py", "line": 27}}`
///
/// Symbols that fail to import or inspect are silently skipped.
const RESOLVER_SCRIPT: &str = r#"
import sys, json, importlib, inspect

project_root = sys.argv[1]
sys.path.insert(0, project_root)

queries = json.loads(sys.stdin.read())
results = {}

for q in queries:
    module_path = q["module"]
    symbols = q.get("symbols", [])
    try:
        mod = importlib.import_module(module_path)
        if not symbols:
            # Module-level resolution only
            mod_file = getattr(mod, "__file__", None)
            if mod_file:
                results[module_path] = {"file": mod_file, "line": None}
            continue
        for sym_name in symbols:
            obj = getattr(mod, sym_name, None)
            if obj is None:
                continue
            try:
                file_path = inspect.getfile(obj)
                try:
                    _, line_no = inspect.getsourcelines(obj)
                    results[f"{module_path}.{sym_name}"] = {"file": file_path, "line": line_no}
                except (OSError, TypeError):
                    results[f"{module_path}.{sym_name}"] = {"file": file_path, "line": None}
            except TypeError:
                pass
    except ImportError:
        pass
    except Exception:
        pass

json.dump(results, sys.stdout)
"#;

/// Collect unique Python module+symbol queries from file imports.
///
/// Scans all imports from `.py` / `.pyi` files, collects non-relative
/// (absolute) imports, and deduplicates into `(module, [symbols])` pairs
/// suitable for [`batch_resolve`].
pub fn collect_python_queries(
    file_imports: &HashMap<PathBuf, Vec<crate::model::types::ImportInfo>>,
) -> Vec<(String, Vec<String>)> {
    let mut queries: HashMap<String, Vec<String>> = HashMap::new();

    for (path, imports) in file_imports {
        let is_python = path
            .extension()
            .is_some_and(|ext| ext == "py" || ext == "pyi");
        if !is_python {
            continue;
        }

        for import in imports {
            let is_relative = import.source.starts_with("./")
                || import.source.starts_with("../")
                || import.source.starts_with('.');
            if is_relative {
                continue;
            }

            let entry = queries.entry(import.source.clone()).or_default();
            for spec in &import.specifiers {
                if !entry.contains(spec) {
                    entry.push(spec.clone());
                }
            }
        }
    }

    queries.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::ImportInfo;

    #[test]
    fn collect_python_queries_extracts_absolute_imports() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/train.py"),
            vec![
                ImportInfo {
                    source: "torch.nn".into(),
                    specifiers: vec!["Module".into(), "Conv2d".into()],
                    line: 1,
                    aliases: vec![],
                },
                ImportInfo {
                    source: "./utils".into(),
                    specifiers: vec!["helper".into()],
                    line: 2,
                    aliases: vec![],
                },
                ImportInfo {
                    source: "numpy".into(),
                    specifiers: vec!["array".into()],
                    line: 3,
                    aliases: vec![],
                },
            ],
        );
        file_imports.insert(
            PathBuf::from("src/model.py"),
            vec![ImportInfo {
                source: "torch.nn".into(),
                specifiers: vec!["Linear".into(), "Module".into()],
                line: 1,
                aliases: vec![],
            }],
        );

        let queries = collect_python_queries(&file_imports);

        // Should have 2 unique modules: torch.nn and numpy
        assert_eq!(queries.len(), 2);

        let torch_nn = queries.iter().find(|(m, _)| m == "torch.nn").unwrap();
        // Should deduplicate Module, collect Conv2d and Linear
        assert!(torch_nn.1.contains(&"Module".to_string()));
        assert!(torch_nn.1.contains(&"Conv2d".to_string()));
        assert!(torch_nn.1.contains(&"Linear".to_string()));

        let numpy = queries.iter().find(|(m, _)| m == "numpy").unwrap();
        assert_eq!(numpy.1, vec!["array".to_string()]);
    }

    #[test]
    fn collect_python_queries_skips_non_python_files() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/app.ts"),
            vec![ImportInfo {
                source: "express".into(),
                specifiers: vec!["Router".into()],
                line: 1,
                aliases: vec![],
            }],
        );

        let queries = collect_python_queries(&file_imports);
        assert!(queries.is_empty());
    }

    #[test]
    fn collect_python_queries_skips_relative_imports() {
        let mut file_imports = HashMap::new();
        file_imports.insert(
            PathBuf::from("src/main.py"),
            vec![
                ImportInfo {
                    source: ".utils".into(),
                    specifiers: vec!["helper".into()],
                    line: 1,
                    aliases: vec![],
                },
                ImportInfo {
                    source: "..models".into(),
                    specifiers: vec!["User".into()],
                    line: 2,
                    aliases: vec![],
                },
            ],
        );

        let queries = collect_python_queries(&file_imports);
        assert!(queries.is_empty());
    }

    #[test]
    fn parse_output_filters_external_files() {
        let json = r#"{"torch.nn.Module": {"file": "/project/torch/nn/modules/module.py", "line": 27}, "numpy.array": {"file": "/usr/lib/python3/numpy/core/multiarray.py", "line": 100}}"#;
        let results = parse_output(json.as_bytes(), Path::new("/project"));

        // Only torch (inside /project) should be included
        assert_eq!(results.len(), 1);
        assert!(results.contains_key("torch.nn.Module"));
        assert!(!results.contains_key("numpy.array"));

        let resolved = &results["torch.nn.Module"];
        assert_eq!(
            resolved.file,
            PathBuf::from("/project/torch/nn/modules/module.py")
        );
        assert_eq!(resolved.line, Some(27));
    }

    #[test]
    fn parse_output_handles_empty_json() {
        let results = parse_output(b"{}", Path::new("/project"));
        assert!(results.is_empty());
    }

    #[test]
    fn parse_output_handles_malformed_json() {
        let results = parse_output(b"not json", Path::new("/project"));
        assert!(results.is_empty());
    }

    #[test]
    fn batch_resolve_returns_empty_for_no_queries() {
        let results = batch_resolve(Path::new("/tmp"), &[]);
        assert!(results.is_empty());
    }
}
