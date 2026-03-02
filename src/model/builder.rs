//! CodeModel builder: aggregates per-file extractions into a CodeModel.
//!
//! Supports multi-component builds: each workspace package becomes a
//! separate `Component` in the `CodeModel`, with its own import resolution
//! root and module boundary inference.
//!
//! For single-project repos, all files belong to a single default Component
//! (backward compatible with the original MVP behavior).
//!
//! `CodeModelBuilder` supports incremental updates: when a file changes,
//! call `set_file` to replace its contributions without rebuilding
//! the entire code model from scratch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::file_tree::{self, FileMeta};
use super::types::*;
use crate::error::Result;
use crate::parser::SupportedLanguage;
use crate::workspace::WorkspaceLayout;

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
    env_dependencies: Vec<EnvDependency>,
    file_role: FileRole,
    estimated_tokens: u64,
}

/// Per-component state within the builder.
///
/// Each component has its own root path (for import resolution) and
/// its own set of file contributions.
#[derive(Debug, Clone)]
struct ComponentState {
    root: PathBuf,
    contributions: HashMap<PathBuf, FileContribution>,
}

/// Incremental code model builder that tracks per-file contributions
/// across one or more components.
///
/// Instead of rebuilding the entire code model on every change, `CodeModelBuilder`
/// maintains a map of what each file contributed (interfaces, deps, sinks,
/// symbols). When a file changes, only its contributions are replaced.
///
/// During `build()`, post-processing runs import resolution and module
/// boundary inference per component, each using its own root path.
pub struct CodeModelBuilder {
    project_name: String,
    project_root: PathBuf,
    components: HashMap<String, ComponentState>,
    default_component: String,
    /// Optional workspace layout for component-aware file tree construction.
    workspace_layout: Option<WorkspaceLayout>,
}

impl CodeModelBuilder {
    /// Create a new empty builder.
    pub fn new(project_name: &str) -> Self {
        let default_name = project_name.to_string();
        let mut components = HashMap::new();
        components.insert(
            default_name.clone(),
            ComponentState {
                root: PathBuf::new(),
                contributions: HashMap::new(),
            },
        );

        Self {
            project_name: project_name.to_string(),
            project_root: PathBuf::new(),
            components,
            default_component: default_name,
            workspace_layout: None,
        }
    }

    /// Create a new builder with a project root for import resolution.
    pub fn with_root(project_name: &str, project_root: &Path) -> Self {
        let default_name = project_name.to_string();
        let mut components = HashMap::new();
        components.insert(
            default_name.clone(),
            ComponentState {
                root: project_root.to_path_buf(),
                contributions: HashMap::new(),
            },
        );

        Self {
            project_name: project_name.to_string(),
            project_root: project_root.to_path_buf(),
            components,
            default_component: default_name,
            workspace_layout: None,
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

    /// Set the root path for a named component.
    ///
    /// If the component doesn't exist yet, creates it with an empty
    /// contribution set. The root path is used for per-component import
    /// resolution and module boundary inference.
    pub fn set_component_root(&mut self, component_name: &str, root: &Path) {
        self.components
            .entry(component_name.to_string())
            .and_modify(|state| state.root = root.to_path_buf())
            .or_insert_with(|| ComponentState {
                root: root.to_path_buf(),
                contributions: HashMap::new(),
            });
    }

    /// Set the workspace layout for component-aware file tree construction.
    ///
    /// When set, the file tree builder uses this layout to assign
    /// `component_name` to directory nodes based on longest-prefix matching.
    pub fn set_workspace_layout(&mut self, layout: WorkspaceLayout) {
        self.workspace_layout = Some(layout);
    }

    /// Add or replace a file's contributions in the default component.
    ///
    /// If the file was previously tracked, its old contributions are removed
    /// before the new ones are added.
    pub fn set_file(&mut self, extraction: &FileExtraction) {
        // Delegate to update_file with an infallible closure.
        // The unwrap is safe because the closure never returns Err.
        let _ = self.update_file(extraction, |new| Ok(new.clone()));
    }

    /// Add or replace a file's contributions in a specific named component.
    ///
    /// If the component doesn't exist yet, creates it with the project root
    /// as its root path. Use `set_component_root()` beforehand for correct
    /// per-package import resolution.
    pub fn set_file_in_component(&mut self, extraction: &FileExtraction, component_name: &str) {
        let contribution = extraction_to_contribution(extraction);
        let state = self
            .components
            .entry(component_name.to_string())
            .or_insert_with(|| ComponentState {
                root: self.project_root.clone(),
                contributions: HashMap::new(),
            });
        state
            .contributions
            .insert(extraction.file.clone(), contribution);
    }

    /// Atomically update a file's contributions using a closure.
    ///
    /// The closure `f` receives the new extraction and returns the
    /// `FileExtraction` to store. If the closure returns `Err`, the
    /// previous state is preserved — no partial update occurs.
    ///
    /// This enables callers to perform load-modify-save patterns where a
    /// mid-way failure does not corrupt the builder state.
    pub fn update_file<F>(&mut self, extraction: &FileExtraction, f: F) -> Result<()>
    where
        F: FnOnce(&FileExtraction) -> Result<FileExtraction>,
    {
        let resolved = f(extraction)?;

        let contribution = extraction_to_contribution(&resolved);
        let state = self
            .components
            .get_mut(&self.default_component)
            .expect("default component always exists");
        state
            .contributions
            .insert(resolved.file.clone(), contribution);
        Ok(())
    }

    /// Check whether the builder has contributions for a given file.
    pub fn has_file(&self, path: &Path) -> bool {
        self.components
            .values()
            .any(|state| state.contributions.contains_key(path))
    }

    /// Remove a file's contributions from the builder.
    ///
    /// Searches all components for the file path and removes from
    /// whichever component owns it.
    pub fn remove_file(&mut self, path: &Path) {
        for state in self.components.values_mut() {
            if state.contributions.remove(path).is_some() {
                return;
            }
        }
    }

    /// Produce a sorted, deterministic CodeModel snapshot.
    ///
    /// Iterates over all components, building a `Component` for each.
    /// Per-component post-processing runs:
    /// 1. **Import resolution** — resolves relative imports using the component's root
    /// 2. **Module inference** — groups files into modules relative to the component root
    ///
    /// Stats are aggregated across all components.
    pub fn build(&self) -> CodeModel {
        let mut all_components = Vec::new();

        // Accumulate stats across all components
        let mut total_files = 0;
        let mut total_interfaces = 0;
        let mut total_dependencies = 0;
        let mut total_sinks = 0;
        let mut total_symbols = 0;
        let mut total_imports = 0;
        let mut total_references = 0;
        let mut total_data_models = 0;
        let mut total_modules = 0;
        let mut total_resolved_references = 0;
        let mut total_confidence_sum: f64 = 0.0;
        let mut total_ref_count = 0;
        let mut file_roles: HashMap<String, usize> = HashMap::new();
        let mut total_estimated_tokens: u64 = 0;
        let mut total_test_symbols: usize = 0;
        let mut total_env_dependencies: usize = 0;

        // Sort component names for deterministic output
        let mut component_names: Vec<&String> = self.components.keys().collect();
        component_names.sort();

        for name in component_names {
            let state = &self.components[name];
            let component = self.build_component(name, state);

            // Aggregate stats
            total_files += state.contributions.len();
            total_interfaces += component.interfaces.len();
            total_dependencies += component.dependencies.len();
            total_sinks += component.sinks.len();
            total_symbols += component.symbols.len();
            total_imports += component.imports.len();
            total_references += component.references.len();
            total_data_models += component.data_models.len();
            total_modules += component.module_boundaries.len();
            total_test_symbols += component.symbols.iter().filter(|s| s.is_test).count();
            total_env_dependencies += component.env_dependencies.len();
            total_resolved_references += component
                .references
                .iter()
                .filter(|r| r.confidence > 0.0)
                .count();
            total_confidence_sum += component
                .references
                .iter()
                .map(|r| r.confidence)
                .sum::<f64>();
            total_ref_count += component.references.len();

            for contrib in state.contributions.values() {
                let role_key = contrib.file_role.as_str().to_string();
                *file_roles.entry(role_key).or_insert(0) += 1;
                total_estimated_tokens += contrib.estimated_tokens;
            }

            all_components.push(component);
        }

        let avg_resolution_confidence = if total_ref_count == 0 {
            0.0
        } else {
            total_confidence_sum / total_ref_count as f64
        };

        // Build file tree from all contributions across all components
        let all_file_metas: Vec<FileMeta> = self
            .components
            .values()
            .flat_map(|state| {
                state.contributions.iter().map(|(path, contrib)| FileMeta {
                    path: path.clone(),
                    role: contrib.file_role,
                    language: contrib.language,
                    estimated_tokens: contrib.estimated_tokens,
                })
            })
            .collect();

        let all_refs: Vec<&Reference> = all_components
            .iter()
            .flat_map(|c| c.references.iter())
            .collect();

        let file_tree = file_tree::build_file_tree(
            &all_file_metas,
            &all_refs,
            &self.project_root,
            self.workspace_layout.as_ref(),
        );
        let total_directories = file_tree::count_directories(&file_tree);

        let stats = CodeModelStats {
            files_analyzed: total_files,
            total_interfaces,
            total_dependencies,
            total_sinks,
            total_symbols,
            total_imports,
            total_references,
            total_data_models,
            total_modules,
            resolved_references: total_resolved_references,
            avg_resolution_confidence,
            file_roles,
            total_estimated_tokens,
            total_directories,
            total_test_symbols,
            total_env_dependencies,
        };

        CodeModel {
            version: "1.0".into(),
            project_name: self.project_name.clone(),
            components: all_components,
            stats,
            file_tree: Some(file_tree),
        }
    }

    /// Build a single `Component` from a `ComponentState`.
    ///
    /// Runs post-processing (import resolution, module inference) using
    /// the component's own root path — critical for monorepos where each
    /// package has its own relative import context.
    fn build_component(&self, name: &str, state: &ComponentState) -> Component {
        let mut interfaces = Vec::new();
        let mut dependencies = Vec::new();
        let mut sinks = Vec::new();
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut references = Vec::new();
        let mut data_models = Vec::new();
        let mut env_dependencies = Vec::new();
        let mut lang_counts: HashMap<SupportedLanguage, usize> = HashMap::new();

        // Build per-file maps for post-processing
        let mut file_imports: HashMap<PathBuf, Vec<ImportInfo>> = HashMap::new();
        let mut file_symbols: HashMap<PathBuf, Vec<Symbol>> = HashMap::new();

        for (path, contribution) in &state.contributions {
            interfaces.extend(contribution.interfaces.iter().cloned());
            dependencies.extend(contribution.dependencies.iter().cloned());
            sinks.extend(contribution.sinks.iter().cloned());
            symbols.extend(contribution.symbols.iter().cloned());
            imports.extend(contribution.imports.iter().cloned());
            references.extend(contribution.references.iter().cloned());
            data_models.extend(contribution.data_models.iter().cloned());
            env_dependencies.extend(contribution.env_dependencies.iter().cloned());
            *lang_counts.entry(contribution.language).or_insert(0) += 1;

            file_imports.insert(path.clone(), contribution.imports.clone());
            file_symbols.insert(path.clone(), contribution.symbols.clone());
        }

        // Post-processing: resolve ALL references (imports + calls + hierarchy)
        // via the two-level symbol table for confident cross-file resolution.
        // Uses the component's root, not the workspace root.
        let all_resolved = super::import_resolver::resolve_all_references(
            &file_imports,
            &file_symbols,
            &references,
            &state.root,
        );
        references = all_resolved;

        // Tag test→production references: a reference is a "test reference"
        // when the source file is a test file and the target is NOT a test file.
        // This enables downstream consumers to separate test coupling edges
        // from production architecture.
        for reference in &mut references {
            let source_is_test = state
                .contributions
                .get(&reference.source_file)
                .is_some_and(|c| c.file_role == FileRole::Test);
            let target_is_test = reference
                .target_file
                .as_ref()
                .and_then(|tf| state.contributions.get(tf))
                .is_some_and(|c| c.file_role == FileRole::Test);
            reference.is_test_reference = source_is_test && !target_is_test;
        }

        // Post-processing: infer module boundaries relative to the component root.
        let module_boundaries = super::module_inference::infer_module_boundaries(
            &file_symbols,
            &file_imports,
            &state.root,
        );

        // Sort for deterministic output
        interfaces.sort_by(|a, b| {
            (&a.path, &a.method.to_string()).cmp(&(&b.path, &b.method.to_string()))
        });
        dependencies.sort_by(|a, b| a.target.cmp(&b.target));
        sinks.sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));
        symbols
            .sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));
        imports.sort_by(|a, b| (&a.source, a.line).cmp(&(&b.source, b.line)));
        references
            .sort_by(|a, b| (&a.source_file, a.source_line).cmp(&(&b.source_file, b.source_line)));
        data_models
            .sort_by(|a, b| (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line)));

        env_dependencies.sort_by(|a, b| {
            (&a.anchor.file, a.anchor.line).cmp(&(&b.anchor.file, b.anchor.line))
        });

        let language = dominant_language_from_counts(&lang_counts);

        Component {
            name: name.to_string(),
            language,
            interfaces,
            dependencies,
            sinks,
            symbols,
            imports,
            references,
            data_models,
            module_boundaries,
            env_dependencies,
        }
    }
}

/// Convert a `FileExtraction` into a `FileContribution`.
fn extraction_to_contribution(extraction: &FileExtraction) -> FileContribution {
    FileContribution {
        language: extraction.language,
        interfaces: extraction.interfaces.clone(),
        dependencies: extraction.dependencies.clone(),
        sinks: extraction.sinks.clone(),
        symbols: extraction.symbols.clone(),
        imports: extraction.imports.clone(),
        references: extraction.references.clone(),
        data_models: extraction.data_models.clone(),
        env_dependencies: extraction.env_dependencies.clone(),
        file_role: extraction.file_role,
        estimated_tokens: extraction.estimated_tokens,
    }
}

/// Build a CodeModel from a set of per-file extractions.
///
/// Convenience wrapper around `CodeModelBuilder::from_extractions(...).build()`.
/// All extractions are merged into a single Component. Collections are sorted
/// for determinism.
pub fn build_code_model(extractions: &[FileExtraction], project_name: &str) -> CodeModel {
    CodeModelBuilder::from_extractions(extractions, project_name).build()
}

/// Determine the most common language from a frequency map.
///
/// Ties are broken by language name (alphabetically) for determinism.
/// Returns TypeScript as fallback for empty maps.
fn dominant_language_from_counts(counts: &HashMap<SupportedLanguage, usize>) -> SupportedLanguage {
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

    fn make_extraction(file: &str, interfaces: Vec<Interface>, sinks: Vec<Sink>) -> FileExtraction {
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                    parameters: vec![],
                    handler_name: None,
                    request_body_type: None,
                },
                Interface {
                    method: HttpMethod::Post,
                    path: "/b".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 5),
                    parameters: vec![],
                    handler_name: None,
                    request_body_type: None,
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
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
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("src/b.ts"), 10),
                text: "console.log('hello')".into(),
                contains_pii: false,
            }],
        );

        let from_build_code_model = build_code_model(&[ext1.clone(), ext2.clone()], "proj");
        let from_builder = CodeModelBuilder::from_extractions(&[ext1, ext2], "proj").build();

        let json1 = serde_json::to_string(&from_build_code_model).unwrap();
        let json2 = serde_json::to_string(&from_builder).unwrap();
        assert_eq!(
            json1, json2,
            "CodeModelBuilder must produce identical output to build_code_model"
        );
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
                    is_test: false,
                },
                Symbol {
                    name: "getUser".into(),
                    kind: SymbolKind::Method,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/lib.ts"), 5, 10),
                    doc: None,
                    signature: None,
                    visibility: None,
                    parent: None,
                    is_test: false,
                },
            ],
            references: vec![],
            data_models: vec![],
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
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
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 0,
            content_hash: None,
        };
        builder.set_file(&ext1);
        builder.set_file(&ext2);
        let model = builder.build();

        assert_eq!(model.stats.total_imports, 2);
        assert_eq!(model.components[0].imports.len(), 2);
    }

    // --- Atomic update tests ---

    #[test]
    fn test_builder_atomic_update_on_error() {
        use crate::error::IntentlyError;

        let mut builder = CodeModelBuilder::new("proj");

        // Set initial state with route /original
        let original = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/original".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        builder.set_file(&original);

        // Verify initial state
        let model_before = builder.build();
        assert_eq!(model_before.components[0].interfaces.len(), 1);
        assert_eq!(model_before.components[0].interfaces[0].path, "/original");

        // Attempt an update that fails mid-way — closure returns Err
        let new_extraction = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Post,
                path: "/should-not-appear".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 5),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );

        let result = builder.update_file(&new_extraction, |_new| {
            Err(IntentlyError::ExtractionFailed {
                path: PathBuf::from("src/app.ts"),
                reason: "simulated extraction error".into(),
            })
        });

        // update_file should return Err
        assert!(result.is_err());

        // Previous state must be intact — /original still present
        let model_after = builder.build();
        assert_eq!(model_after.components[0].interfaces.len(), 1);
        assert_eq!(model_after.components[0].interfaces[0].path, "/original");
    }

    #[test]
    fn test_builder_update_file_success_replaces_contributions() {
        let mut builder = CodeModelBuilder::new("proj");

        let original = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/v1".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        builder.set_file(&original);

        // Successful update replaces the contribution
        let updated = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/v2".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );

        let result = builder.update_file(&updated, |new| Ok(new.clone()));
        assert!(result.is_ok());

        let model = builder.build();
        assert_eq!(model.components[0].interfaces.len(), 1);
        assert_eq!(model.components[0].interfaces[0].path, "/v2");
    }

    #[test]
    fn test_builder_update_file_has_file_check() {
        let mut builder = CodeModelBuilder::new("proj");

        // Initially no file tracked
        assert!(!builder.has_file(Path::new("src/app.ts")));

        let ext = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/existing".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        builder.set_file(&ext);

        // Now it should be tracked
        assert!(builder.has_file(Path::new("src/app.ts")));

        // And we can update it successfully
        let new_ext = make_extraction("src/app.ts", vec![], vec![]);
        let result = builder.update_file(&new_ext, |new| Ok(new.clone()));
        assert!(result.is_ok());
    }

    // --- Multi-component tests ---

    #[test]
    fn multi_component_build_produces_n_components() {
        let mut builder = CodeModelBuilder::with_root("monorepo", Path::new("/repo"));
        builder.set_component_root("api", Path::new("/repo/packages/api"));
        builder.set_component_root("auth", Path::new("/repo/packages/auth"));

        let api_ext = make_extraction(
            "/repo/packages/api/src/index.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/health".into(),
                auth: None,
                anchor: SourceAnchor::from_line(
                    PathBuf::from("/repo/packages/api/src/index.ts"),
                    1,
                ),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        let auth_ext = make_extraction(
            "/repo/packages/auth/src/index.ts",
            vec![Interface {
                method: HttpMethod::Post,
                path: "/login".into(),
                auth: None,
                anchor: SourceAnchor::from_line(
                    PathBuf::from("/repo/packages/auth/src/index.ts"),
                    1,
                ),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );

        builder.set_file_in_component(&api_ext, "api");
        builder.set_file_in_component(&auth_ext, "auth");

        let model = builder.build();

        // 3 components: api, auth, and the default (monorepo) which is empty
        assert_eq!(model.components.len(), 3);

        let api_comp = model.components.iter().find(|c| c.name == "api").unwrap();
        assert_eq!(api_comp.interfaces.len(), 1);
        assert_eq!(api_comp.interfaces[0].path, "/health");

        let auth_comp = model.components.iter().find(|c| c.name == "auth").unwrap();
        assert_eq!(auth_comp.interfaces.len(), 1);
        assert_eq!(auth_comp.interfaces[0].path, "/login");

        assert_eq!(model.stats.files_analyzed, 2);
        assert_eq!(model.stats.total_interfaces, 2);
    }

    #[test]
    fn set_file_routes_to_default_component() {
        let mut builder = CodeModelBuilder::new("proj");
        builder.set_component_root("other", Path::new("/other"));

        let ext = make_extraction(
            "src/app.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/default".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/app.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        builder.set_file(&ext);

        let model = builder.build();
        let default = model.components.iter().find(|c| c.name == "proj").unwrap();
        assert_eq!(default.interfaces.len(), 1);
        assert_eq!(default.interfaces[0].path, "/default");
    }

    #[test]
    fn remove_file_finds_across_components() {
        let mut builder = CodeModelBuilder::with_root("mono", Path::new("/repo"));
        builder.set_component_root("pkg", Path::new("/repo/pkg"));

        let ext = make_extraction(
            "/repo/pkg/src/lib.ts",
            vec![Interface {
                method: HttpMethod::Get,
                path: "/in-pkg".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("/repo/pkg/src/lib.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            vec![],
        );
        builder.set_file_in_component(&ext, "pkg");

        assert!(builder.has_file(Path::new("/repo/pkg/src/lib.ts")));

        builder.remove_file(Path::new("/repo/pkg/src/lib.ts"));

        assert!(!builder.has_file(Path::new("/repo/pkg/src/lib.ts")));
        let pkg = builder
            .build()
            .components
            .into_iter()
            .find(|c| c.name == "pkg")
            .unwrap();
        assert!(pkg.interfaces.is_empty());
    }

    #[test]
    fn multi_component_stats_aggregate_correctly() {
        let mut builder = CodeModelBuilder::with_root("mono", Path::new("/repo"));
        builder.set_component_root("a", Path::new("/repo/a"));
        builder.set_component_root("b", Path::new("/repo/b"));

        let ext_a = FileExtraction {
            file: PathBuf::from("/repo/a/src/lib.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![Interface {
                method: HttpMethod::Get,
                path: "/a".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("/repo/a/src/lib.ts"), 1),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            dependencies: vec![],
            sinks: vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("/repo/a/src/lib.ts"), 5),
                text: "console.log('a')".into(),
                contains_pii: false,
            }],
            imports: vec![],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 100,
            content_hash: None,
        };
        let ext_b = FileExtraction {
            file: PathBuf::from("/repo/b/src/lib.ts"),
            language: SupportedLanguage::Python,
            interfaces: vec![
                Interface {
                    method: HttpMethod::Post,
                    path: "/b1".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("/repo/b/src/lib.ts"), 1),
                    parameters: vec![],
                    handler_name: None,
                    request_body_type: None,
                },
                Interface {
                    method: HttpMethod::Get,
                    path: "/b2".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("/repo/b/src/lib.ts"), 10),
                    parameters: vec![],
                    handler_name: None,
                    request_body_type: None,
                },
            ],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
            env_dependencies: vec![],
            file_role: FileRole::Implementation,
            estimated_tokens: 200,
            content_hash: None,
        };

        builder.set_file_in_component(&ext_a, "a");
        builder.set_file_in_component(&ext_b, "b");

        let model = builder.build();

        assert_eq!(model.stats.files_analyzed, 2);
        assert_eq!(model.stats.total_interfaces, 3);
        assert_eq!(model.stats.total_sinks, 1);
        assert_eq!(model.stats.total_estimated_tokens, 300);
    }

    // --- is_test_reference tagging tests ---

    /// Helper: build a FileExtraction with a specific file role and references.
    fn extraction_with_refs(
        file: &str,
        role: FileRole,
        refs: Vec<Reference>,
        symbols: Vec<Symbol>,
    ) -> FileExtraction {
        FileExtraction {
            file: PathBuf::from(file),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![],
            dependencies: vec![],
            sinks: vec![],
            imports: vec![],
            symbols,
            references: refs,
            data_models: vec![],
            env_dependencies: vec![],
            file_role: role,
            estimated_tokens: 0,
            content_hash: None,
        }
    }

    #[test]
    fn test_reference_tagged_when_source_is_test_and_target_is_impl() {
        let mut builder = CodeModelBuilder::new("proj");

        // Production file with a symbol
        let prod = extraction_with_refs(
            "src/service.ts",
            FileRole::Implementation,
            vec![],
            vec![Symbol {
                name: "UserService".into(),
                kind: SymbolKind::Class,
                anchor: SourceAnchor::from_line(PathBuf::from("src/service.ts"), 1),
                doc: None,
                signature: None,
                visibility: None,
                parent: None,
                is_test: false,
            }],
        );

        // Test file calling the production symbol
        let test = extraction_with_refs(
            "tests/service.test.ts",
            FileRole::Test,
            vec![Reference {
                source_symbol: "testService".into(),
                source_file: PathBuf::from("tests/service.test.ts"),
                source_line: 5,
                target_symbol: "UserService".into(),
                target_file: Some(PathBuf::from("src/service.ts")),
                target_line: Some(1),
                reference_kind: ReferenceKind::Call,
                confidence: 0.90,
                resolution_method: ResolutionMethod::ImportBased,
                is_test_reference: false, // will be tagged by builder
            }],
            vec![],
        );

        builder.set_file(&prod);
        builder.set_file(&test);
        let model = builder.build();

        let test_refs: Vec<&Reference> = model.components[0]
            .references
            .iter()
            .filter(|r| r.source_file == PathBuf::from("tests/service.test.ts"))
            .collect();

        assert!(
            !test_refs.is_empty(),
            "should have references from test file"
        );
        for r in &test_refs {
            if r.target_file == Some(PathBuf::from("src/service.ts")) {
                assert!(
                    r.is_test_reference,
                    "test→production reference should be tagged"
                );
            }
        }
    }

    #[test]
    fn test_to_test_not_tagged_as_test_reference() {
        let mut builder = CodeModelBuilder::new("proj");

        // Two test files referencing each other
        let test1 = extraction_with_refs(
            "tests/a.test.ts",
            FileRole::Test,
            vec![Reference {
                source_symbol: "testA".into(),
                source_file: PathBuf::from("tests/a.test.ts"),
                source_line: 3,
                target_symbol: "helperB".into(),
                target_file: Some(PathBuf::from("tests/b.test.ts")),
                target_line: Some(1),
                reference_kind: ReferenceKind::Call,
                confidence: 0.80,
                resolution_method: ResolutionMethod::GlobalUnique,
                is_test_reference: false,
            }],
            vec![],
        );
        let test2 = extraction_with_refs(
            "tests/b.test.ts",
            FileRole::Test,
            vec![],
            vec![Symbol {
                name: "helperB".into(),
                kind: SymbolKind::Function,
                anchor: SourceAnchor::from_line(PathBuf::from("tests/b.test.ts"), 1),
                doc: None,
                signature: None,
                visibility: None,
                parent: None,
                is_test: false,
            }],
        );

        builder.set_file(&test1);
        builder.set_file(&test2);
        let model = builder.build();

        let test_to_test: Vec<&Reference> = model.components[0]
            .references
            .iter()
            .filter(|r| {
                r.source_file == PathBuf::from("tests/a.test.ts")
                    && r.target_file == Some(PathBuf::from("tests/b.test.ts"))
            })
            .collect();

        for r in &test_to_test {
            assert!(
                !r.is_test_reference,
                "test→test reference should NOT be tagged"
            );
        }
    }

    #[test]
    fn impl_to_impl_not_tagged_as_test_reference() {
        let mut builder = CodeModelBuilder::new("proj");

        let prod1 = extraction_with_refs(
            "src/a.ts",
            FileRole::Implementation,
            vec![Reference {
                source_symbol: "funcA".into(),
                source_file: PathBuf::from("src/a.ts"),
                source_line: 5,
                target_symbol: "funcB".into(),
                target_file: Some(PathBuf::from("src/b.ts")),
                target_line: Some(1),
                reference_kind: ReferenceKind::Call,
                confidence: 0.95,
                resolution_method: ResolutionMethod::ImportBased,
                is_test_reference: false,
            }],
            vec![],
        );
        let prod2 = extraction_with_refs(
            "src/b.ts",
            FileRole::Implementation,
            vec![],
            vec![Symbol {
                name: "funcB".into(),
                kind: SymbolKind::Function,
                anchor: SourceAnchor::from_line(PathBuf::from("src/b.ts"), 1),
                doc: None,
                signature: None,
                visibility: None,
                parent: None,
                is_test: false,
            }],
        );

        builder.set_file(&prod1);
        builder.set_file(&prod2);
        let model = builder.build();

        for r in &model.components[0].references {
            assert!(
                !r.is_test_reference,
                "impl→impl reference should NOT be tagged"
            );
        }
    }
}
