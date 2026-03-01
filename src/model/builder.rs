//! CodeModel builder: aggregates per-file extractions into a CodeModel.
//!
//! MVP simplification: all files belong to a single Component.
//! The builder sorts all collections for deterministic JSON output.
//!
//! `CodeModelBuilder` supports incremental updates: when a file changes,
//! call `set_file` to replace its contributions without rebuilding
//! the entire code model from scratch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::*;
use crate::parser::SupportedLanguage;

/// Tracks what a single file contributed to the code model.
#[derive(Debug, Clone)]
struct FileContribution {
    language: SupportedLanguage,
    interfaces: Vec<Interface>,
    dependencies: Vec<Dependency>,
    sinks: Vec<Sink>,
    symbols: Vec<Symbol>,
    imports: Vec<ImportInfo>,
    references: Vec<Reference>,
    data_models: Vec<DataModel>,
}

/// Incremental code model builder that tracks per-file contributions.
///
/// Instead of rebuilding the entire code model on every change, `CodeModelBuilder`
/// maintains a map of what each file contributed (interfaces, deps, sinks,
/// symbols). When a file changes, only its contributions are replaced.
///
/// During `build()`, post-processing runs import resolution and module
/// boundary inference across all aggregated contributions.
pub struct CodeModelBuilder {
    project_name: String,
    project_root: PathBuf,
    contributions: HashMap<PathBuf, FileContribution>,
}

impl CodeModelBuilder {
    /// Create a new empty builder.
    pub fn new(project_name: &str) -> Self {
        Self {
            project_name: project_name.to_string(),
            project_root: PathBuf::new(),
            contributions: HashMap::new(),
        }
    }

    /// Create a new builder with a project root for import resolution.
    pub fn with_root(project_name: &str, project_root: &Path) -> Self {
        Self {
            project_name: project_name.to_string(),
            project_root: project_root.to_path_buf(),
            contributions: HashMap::new(),
        }
    }

    /// Initialize from a batch of extractions (used during full_analysis).
    pub fn from_extractions(extractions: &[FileExtraction], project_name: &str) -> Self {
        let mut builder = Self::new(project_name);
        for ext in extractions {
            builder.set_file(ext);
        }
        builder
    }

    /// Add or replace a file's contributions.
    ///
    /// If the file was previously tracked, its old contributions are removed
    /// before the new ones are added.
    pub fn set_file(&mut self, extraction: &FileExtraction) {
        let contribution = FileContribution {
            language: extraction.language,
            interfaces: extraction.interfaces.clone(),
            dependencies: extraction.dependencies.clone(),
            sinks: extraction.sinks.clone(),
            symbols: extraction.symbols.clone(),
            imports: extraction.imports.clone(),
            references: extraction.references.clone(),
            data_models: extraction.data_models.clone(),
        };
        self.contributions
            .insert(extraction.file.clone(), contribution);
    }

    /// Remove a file's contributions from the builder.
    pub fn remove_file(&mut self, path: &Path) {
        self.contributions.remove(path);
    }

    /// Produce a sorted, deterministic CodeModel snapshot.
    ///
    /// After aggregating per-file contributions, runs two post-processing steps:
    /// 1. **Import resolution** — resolves relative imports to target files/symbols
    /// 2. **Module inference** — groups files into logical modules with dependencies
    pub fn build(&self) -> CodeModel {
        let mut interfaces = Vec::new();
        let mut dependencies = Vec::new();
        let mut sinks = Vec::new();
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut references = Vec::new();
        let mut data_models = Vec::new();
        let mut lang_counts: HashMap<SupportedLanguage, usize> = HashMap::new();

        // Build per-file maps for post-processing
        let mut file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();
        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        for (path, contribution) in &self.contributions {
            interfaces.extend(contribution.interfaces.iter().cloned());
            dependencies.extend(contribution.dependencies.iter().cloned());
            sinks.extend(contribution.sinks.iter().cloned());
            symbols.extend(contribution.symbols.iter().cloned());
            imports.extend(contribution.imports.iter().cloned());
            references.extend(contribution.references.iter().cloned());
            data_models.extend(contribution.data_models.iter().cloned());
            *lang_counts.entry(contribution.language).or_insert(0) += 1;

            file_imports.insert(path.clone(), contribution.imports.clone());
            file_symbols.insert(path.clone(), contribution.symbols.clone());
        }

        // Post-processing: resolve imports to cross-file references
        let import_refs = super::import_resolver::resolve_imports(
            &file_imports,
            &file_symbols,
            &self.project_root,
        );
        references.extend(import_refs);

        // Post-processing: infer module boundaries
        let module_boundaries = super::module_inference::infer_module_boundaries(
            &file_symbols,
            &file_imports,
            &self.project_root,
        );

        // Sort for deterministic output
        interfaces.sort_by(|a, b| {
            (&a.path, &a.method.to_string()).cmp(&(&b.path, &b.method.to_string()))
        });
        dependencies.sort_by(|a, b| a.target.cmp(&b.target));
        sinks.sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));
        symbols.sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));
        imports.sort_by(|a, b| (&a.source, a.line).cmp(&(&b.source, b.line)));
        references.sort_by(|a, b| {
            (&a.source_file, a.source_line).cmp(&(&b.source_file, b.source_line))
        });
        data_models.sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));

        let language = dominant_language_from_counts(&lang_counts);

        let stats = CodeModelStats {
            files_analyzed: self.contributions.len(),
            total_interfaces: interfaces.len(),
            total_dependencies: dependencies.len(),
            total_sinks: sinks.len(),
            total_symbols: symbols.len(),
            total_imports: imports.len(),
            total_references: references.len(),
            total_data_models: data_models.len(),
            total_modules: module_boundaries.len(),
        };

        let component = Component {
            name: self.project_name.clone(),
            language,
            interfaces,
            dependencies,
            sinks,
            symbols,
            imports,
            references,
            data_models,
            module_boundaries,
        };

        CodeModel {
            version: "1.0".into(),
            project_name: self.project_name.clone(),
            components: vec![component],
            stats,
        }
    }
}

/// Build a CodeModel from a set of per-file extractions.
///
/// Convenience wrapper around `CodeModelBuilder::from_extractions(...).build()`.
/// All extractions are merged into a single Component (MVP assumption:
/// one project = one service). Collections are sorted for determinism.
pub fn build_code_model(extractions: &[FileExtraction], project_name: &str) -> CodeModel {
    CodeModelBuilder::from_extractions(extractions, project_name).build()
}

/// Determine the most common language from a frequency map.
///
/// Ties are broken by language name (alphabetically) for determinism.
/// Returns TypeScript as fallback for empty maps.
fn dominant_language_from_counts(
    counts: &HashMap<SupportedLanguage, usize>,
) -> SupportedLanguage {
    counts
        .iter()
        .max_by(|(lang_a, count_a), (lang_b, count_b)| {
            count_a
                .cmp(count_b)
                .then_with(|| lang_a.to_string().cmp(&lang_b.to_string()))
        })
        .map(|(lang, _)| *lang)
        .unwrap_or(SupportedLanguage::TypeScript)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn make_extraction(
        file: &str,
        interfaces: Vec<Interface>,
        sinks: Vec<Sink>,
    ) -> FileExtraction {
        FileExtraction {
            file: PathBuf::from(file),
            language: SupportedLanguage::TypeScript,
            interfaces,
            dependencies: vec![],
            sinks,
            imports: vec![],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        }
    }

    #[test]
    fn builds_model_from_single_extraction() {
        let ext = make_extraction(
            "src/index.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/health".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/index.ts"), 5),
            }],
            vec![],
        );

        let model = build_code_model(&[ext], "test-project");

        assert_eq!(model.project_name, "test-project");
        assert_eq!(model.components.len(), 1);
        assert_eq!(model.components[0].interfaces.len(), 1);
        assert_eq!(model.stats.files_analyzed, 1);
        assert_eq!(model.stats.total_interfaces, 1);
    }

    #[test]
    fn aggregates_multiple_extractions() {
        let ext1 = make_extraction(
            "src/routes.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/api/users".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/routes.ts"), 3),
            }],
            vec![],
        );
        let ext2 = make_extraction(
            "src/payments.ts",
            vec![Interface {
                method: HttpMethod::Post,
                path: "/api/payments".into(),
                auth: Some(AuthKind::Middleware("auth".into())),
                anchor: SourceAnchor::from_line(PathBuf::from("src/payments.ts"), 10),
            }],
            vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("src/payments.ts"), 15),
                text: "console.log('paid')".into(),
                contains_pii: false,
            }],
        );

        let model = build_code_model(&[ext1, ext2], "my-app");

        assert_eq!(model.stats.files_analyzed, 2);
        assert_eq!(model.stats.total_interfaces, 2);
        assert_eq!(model.stats.total_sinks, 1);
    }

    #[test]
    fn empty_extractions_produce_empty_model() {
        let model = build_code_model(&[], "empty");

        assert_eq!(model.components.len(), 1);
        assert!(model.components[0].interfaces.is_empty());
        assert_eq!(model.stats.files_analyzed, 0);
    }

    #[test]
    fn deterministic_output_regardless_of_input_order() {
        let ext_a = make_extraction(
            "src/b.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/z-route".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/b.ts"), 1),
            }],
            vec![],
        );
        let ext_b = make_extraction(
            "src/a.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/a-route".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/a.ts"), 1),
            }],
            vec![],
        );

        let twin1 = build_code_model(&[ext_a.clone(), ext_b.clone()], "proj");
        let twin2 = build_code_model(&[ext_b, ext_a], "proj");

        let json1 = serde_json::to_string(&twin1).unwrap();
        let json2 = serde_json::to_string(&twin2).unwrap();
        assert_eq!(
            json1, json2,
            "output must be deterministic regardless of input order"
        );
    }

    #[test]
    fn stats_are_accurate() {
        let ext = FileExtraction {
            file: PathBuf::from("src/app.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![
                Interface {
                    method: HttpMethod::Get,
                    path: "/a".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                },
                Interface {
                    method: HttpMethod::Post,
                    path: "/b".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 5),
                },
            ],
            dependencies: vec![Dependency {
                target: "fetch(...)".into(),
                dependency_type: DependencyType::HttpCall,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 10),
            }],
            sinks: vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 15),
                text: "console.log('test')".into(),
                contains_pii: false,
            }],
            imports: vec![],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };

        let model = build_code_model(&[ext], "proj");

        assert_eq!(model.stats.files_analyzed, 1);
        assert_eq!(model.stats.total_interfaces, 2);
        assert_eq!(model.stats.total_dependencies, 1);
        assert_eq!(model.stats.total_sinks, 1);
    }

    // --- CodeModelBuilder incremental tests ---

    #[test]
    fn model_builder_set_file_adds_contributions() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext = make_extraction(
            "src/a.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/api/a".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/a.ts"), 1),
            }],
            vec![],
        );
        builder.set_file(&ext);
        let model = builder.build();

        assert_eq!(model.components[0].interfaces.len(), 1);
        assert_eq!(model.stats.files_analyzed, 1);
    }

    #[test]
    fn model_builder_set_file_replaces_old_contributions() {
        let mut builder = CodeModelBuilder::new("proj");

        // Initial extraction with route /old
        let ext_v1 = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/old".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
            }],
            vec![],
        );
        builder.set_file(&ext_v1);

        // Updated extraction with route /new
        let ext_v2 = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Post,
                path: "/new".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
            }],
            vec![],
        );
        builder.set_file(&ext_v2);

        let model = builder.build();
        assert_eq!(model.components[0].interfaces.len(), 1);
        assert_eq!(model.components[0].interfaces[0].path, "/new");
        assert_eq!(model.stats.files_analyzed, 1);
    }

    #[test]
    fn model_builder_remove_file_clears_contributions() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext = make_extraction(
            "src/gone.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/gone".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/gone.ts"), 1),
            }],
            vec![],
        );
        builder.set_file(&ext);
        assert_eq!(builder.build().components[0].interfaces.len(), 1);

        builder.remove_file(Path::new("src/gone.ts"));
        let model = builder.build();
        assert!(model.components[0].interfaces.is_empty());
        assert_eq!(model.stats.files_analyzed, 0);
    }

    #[test]
    fn model_builder_matches_build_code_model_output() {
        let ext1 = make_extraction(
            "src/a.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/a".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/a.ts"), 1),
            }],
            vec![],
        );
        let ext2 = make_extraction(
            "src/b.ts",
            vec![Interface {
                method: HttpMethod::Post,
                path: "/b".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/b.ts"), 5),
            }],
            vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("src/b.ts"), 10),
                text: "console.log('hello')".into(),
                contains_pii: false,
            }],
        );

        let from_build_code_model = build_code_model(&[ext1.clone(), ext2.clone()], "proj");
        let from_builder =
            CodeModelBuilder::from_extractions(&[ext1, ext2], "proj").build();

        let json1 = serde_json::to_string(&from_build_code_model).unwrap();
        let json2 = serde_json::to_string(&from_builder).unwrap();
        assert_eq!(json1, json2, "CodeModelBuilder must produce identical output to build_code_model");
    }

    #[test]
    fn model_builder_symbols_tracked_incrementally() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext = FileExtraction {
            file: PathBuf::from("src/lib.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![],
            symbols: vec![
                Symbol {
                    name: "UserService".into(),
                    kind: SymbolKind::Class,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/lib.ts"), 1, 20),
                    doc: None,
                    signature: None,
                    visibility: None,
                    parent: None,
                },
                Symbol {
                    name: "getUser".into(),
                    kind: SymbolKind::Method,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/lib.ts"), 5, 10),
                    doc: None,
                    signature: None,
                    visibility: None,
                    parent: None,
                },
            ],
            references: vec![],
            data_models: vec![],
        };
        builder.set_file(&ext);
        let model = builder.build();

        assert_eq!(model.stats.total_symbols, 2);
        assert_eq!(model.components[0].symbols.len(), 2);

        builder.remove_file(Path::new("src/lib.ts"));
        let model = builder.build();
        assert_eq!(model.stats.total_symbols, 0);
        assert!(model.components[0].symbols.is_empty());
    }

    #[test]
    fn model_builder_imports_preserved_and_sorted() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext = FileExtraction {
            file: PathBuf::from("src/app.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![
                ImportInfo {
                    source: "express".into(),
                    specifiers: vec!["express".into()],
                    line: 1,
                },
                ImportInfo {
                    source: "axios".into(),
                    specifiers: vec!["axios".into()],
                    line: 2,
                },
            ],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };
        builder.set_file(&ext);
        let model = builder.build();

        assert_eq!(model.stats.total_imports, 2);
        assert_eq!(model.components[0].imports.len(), 2);
        // Sorted by (source, line): axios before express
        assert_eq!(model.components[0].imports[0].source, "axios");
        assert_eq!(model.components[0].imports[1].source, "express");
    }

    #[test]
    fn model_builder_imports_removed_on_file_delete() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext = FileExtraction {
            file: PathBuf::from("src/a.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![ImportInfo {
                source: "lodash".into(),
                specifiers: vec!["_".into()],
                line: 1,
            }],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };
        builder.set_file(&ext);
        assert_eq!(builder.build().stats.total_imports, 1);

        builder.remove_file(Path::new("src/a.ts"));
        let model = builder.build();
        assert_eq!(model.stats.total_imports, 0);
        assert!(model.components[0].imports.is_empty());
    }

    #[test]
    fn model_builder_imports_aggregated_from_multiple_files() {
        let mut builder = CodeModelBuilder::new("proj");
        let ext1 = FileExtraction {
            file: PathBuf::from("src/a.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![ImportInfo {
                source: "express".into(),
                specifiers: vec!["express".into()],
                line: 1,
            }],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };
        let ext2 = FileExtraction {
            file: PathBuf::from("src/b.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![ImportInfo {
                source: "axios".into(),
                specifiers: vec!["axios".into()],
                line: 1,
            }],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };
        builder.set_file(&ext1);
        builder.set_file(&ext2);
        let model = builder.build();

        assert_eq!(model.stats.total_imports, 2);
        assert_eq!(model.components[0].imports.len(), 2);
    }
}
