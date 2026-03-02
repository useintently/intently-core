//! IntentlyEngine: the stateful orchestrator for realtime code extraction.
//!
//! Maintains in-memory caches for parsed files and extractions.
//! On file change, re-parses only the changed file, rebuilds the code model
//! from all cached extractions, and builds the knowledge graph.
//!
//! This engine is purely extractive — it answers "what does this codebase
//! contain?" Policy evaluation, health scores, and governance are the
//! responsibility of downstream consumers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use tracing::{debug, info, info_span, instrument, warn};
use walkdir::WalkDir;

use crate::error::{IntentlyError, Result};
use crate::model::builder::CodeModelBuilder;
use crate::model::diff::{self, SemanticDiff};
use crate::model::extractors;
use crate::model::graph::{GraphStats, KnowledgeGraph};
use crate::model::graph_analysis::{AnalysisContext, AnalysisPipeline};
use crate::model::types::{CodeModel, FileExtraction};
use crate::parser;
use crate::workspace::{self, WorkspaceLayout};

/// Directories to skip during file discovery.
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "target",
    ".next",
    "coverage",
    ".turbo",
    "__pycache__",
];

/// Maximum number of files processed in a single parallel chunk during
/// `full_analysis`. Bounds peak memory to ~CHUNK_SIZE intermediate results
/// before they are drained into the sequential caches.
const CHUNK_SIZE: usize = 500;

/// Per-stage timing breakdown of the extraction pipeline.
///
/// Enables the IDE to display pipeline performance and identify bottlenecks.
/// All values are in milliseconds.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineTiming {
    /// Time spent parsing and extracting semantic information from source files.
    pub parse_extract_ms: u64,
    /// Time spent building the CodeModel from per-file contributions.
    pub model_build_ms: u64,
    /// Total wall-clock time for the entire extraction pipeline.
    pub total_ms: u64,
}

/// Result of a full or incremental extraction pass.
///
/// Contains the CodeModel, semantic diff, knowledge graph stats,
/// and timing information. Does NOT contain policy or health data —
/// those are governance concerns for downstream consumers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractionResult {
    pub model: CodeModel,
    pub diff: Option<SemanticDiff>,
    pub timing: PipelineTiming,
    pub graph_stats: Option<GraphStats>,
    pub duration_ms: u64,
    pub files_analyzed: usize,
}

/// The stateful Intently extraction engine.
///
/// Manages parsing, extraction, model building, and knowledge graph
/// construction with incremental caching for real-time performance.
pub struct IntentlyEngine {
    project_root: PathBuf,
    project_name: String,
    source_cache: HashMap<PathBuf, String>,
    extraction_cache: HashMap<PathBuf, FileExtraction>,
    previous_model: Option<CodeModel>,
    /// Cached tree-sitter CSTs for incremental re-parsing.
    /// When a file is re-parsed, the old tree enables tree-sitter to reuse
    /// unchanged nodes, reducing parse time from O(file_size) to O(edit_size).
    tree_cache: HashMap<PathBuf, tree_sitter::Tree>,
    /// Incremental code model builder tracking per-file contributions.
    /// Instead of rebuilding from all cached extractions, updates are applied
    /// as deltas (set_file/remove_file) for O(1) incremental changes.
    model_builder: CodeModelBuilder,
    /// Derived knowledge graph built from the CodeModel after each extraction.
    /// Provides O(1) adjacency lookups for callers/callees/impact analysis
    /// and structural analysis (cycle detection).
    knowledge_graph: Option<KnowledgeGraph>,
    /// Detected workspace layout for monorepo projects.
    /// When present, files are routed to per-package components during
    /// extraction, enabling per-package import resolution and module inference.
    workspace_layout: Option<WorkspaceLayout>,
    /// Cached git metadata from the most recent full_analysis.
    /// Used to compute GitStats and merge into the CodeModel.
    #[cfg(feature = "git")]
    git_metadata_cache:
        Option<std::collections::HashMap<PathBuf, crate::model::types::GitFileMetadata>>,
}

impl IntentlyEngine {
    /// Create a new engine for the given project root.
    ///
    /// Automatically detects workspace layouts (pnpm, npm, Cargo, Go, uv)
    /// to enable per-package component extraction.
    pub fn new(project_root: PathBuf) -> Self {
        let project_name = project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let workspace_layout = workspace::detect_workspace(&project_root);
        if let Some(ref layout) = workspace_layout {
            info!(
                kind = %layout.kind,
                packages = layout.packages.len(),
                "detected workspace"
            );
        }

        let model_builder = CodeModelBuilder::with_root(&project_name, &project_root);
        Self {
            project_root,
            project_name,
            source_cache: HashMap::new(),
            extraction_cache: HashMap::new(),
            previous_model: None,
            tree_cache: HashMap::new(),
            model_builder,
            knowledge_graph: None,
            workspace_layout,
            #[cfg(feature = "git")]
            git_metadata_cache: None,
        }
    }

    /// Run a full extraction: discover all files, parse, extract, build code model.
    ///
    /// Files are processed in chunks of [`CHUNK_SIZE`] to bound peak memory.
    /// Each chunk is extracted in parallel via rayon, then drained into the
    /// sequential caches before the next chunk starts.
    #[instrument(skip(self), fields(project = %self.project_name))]
    pub fn full_analysis(&mut self) -> Result<ExtractionResult> {
        let start = Instant::now();

        let files = {
            let _span = info_span!("discover_files").entered();
            let discovered = self.discover_files()?;
            info!(file_count = discovered.len(), "discovered source files");
            discovered
        };

        // Parse and extract files in chunks to bound peak memory
        let parse_start = Instant::now();
        let _parse_span = info_span!(
            "parse_and_extract",
            file_count = files.len(),
            chunk_size = CHUNK_SIZE,
        )
        .entered();

        self.source_cache.clear();
        self.extraction_cache.clear();
        self.tree_cache.clear();
        self.model_builder = CodeModelBuilder::with_root(&self.project_name, &self.project_root);

        // Configure per-package component roots and workspace layout for monorepos
        if let Some(ref layout) = self.workspace_layout {
            for pkg in &layout.packages {
                self.model_builder.set_component_root(&pkg.name, &pkg.root);
            }
            self.model_builder.set_workspace_layout(layout.clone());
        }

        for (chunk_index, chunk) in files.chunks(CHUNK_SIZE).enumerate() {
            info!(
                chunk_index,
                chunk_size = chunk.len(),
                total_chunks = files.len().div_ceil(CHUNK_SIZE),
                "processing chunk"
            );

            let results: Vec<(PathBuf, String, FileExtraction, tree_sitter::Tree)> = chunk
                .par_iter()
                .filter_map(|path| match self.parse_and_extract_cached(path) {
                    Ok(result) => Some(result),
                    Err(e) => {
                        warn!(?path, error = %e, "skipping file");
                        None
                    }
                })
                .collect();

            // Drain chunk results into caches (sequential)
            for (path, source, extraction, tree) in results {
                self.source_cache.insert(path.clone(), source);
                self.route_extraction_to_component(&extraction);
                self.extraction_cache.insert(path.clone(), extraction);
                self.tree_cache.insert(path, tree);
            }
        }

        drop(_parse_span);
        let parse_extract_ms = parse_start.elapsed().as_millis() as u64;

        // Merge git metadata into file extractions (when feature is enabled)
        #[cfg(feature = "git")]
        {
            let _git_span = info_span!("git_metadata").entered();
            if let Some(git_metadata) =
                crate::git::metadata::compute_git_metadata(&self.project_root)
            {
                info!(
                    files_with_metadata = git_metadata.len(),
                    "computed git metadata"
                );
                for (path, extraction) in &mut self.extraction_cache {
                    if let Some(meta) = git_metadata.get(path) {
                        extraction.git_metadata = Some(meta.clone());
                    }
                }
                // Re-route extractions that got git metadata into the model builder
                // (builder already has the extractions, but git_metadata is on FileExtraction
                // which the builder doesn't track — it's passed through to the output model
                // via the extraction_cache, so no re-routing needed)
                self.git_metadata_cache = Some(git_metadata);
            }
        }

        self.build_result(start, parse_extract_ms)
    }

    /// Incrementally analyze after a single file changed.
    ///
    /// If old source and old tree are cached, computes an `InputEdit`
    /// and applies it to the tree for incremental re-parsing.
    #[instrument(skip(self), fields(file = %path.display()))]
    pub fn on_file_changed(&mut self, path: &Path) -> Result<ExtractionResult> {
        let start = Instant::now();

        let abs_path = self.resolve_path(path);
        debug!(?abs_path, "re-analyzing changed file");

        let parse_start = Instant::now();
        let old_tree = self.prepare_incremental_tree(&abs_path);
        match self.parse_and_extract(&abs_path, old_tree.as_ref()) {
            Ok((path, source, extraction, tree)) => {
                self.source_cache.insert(path.clone(), source);
                self.route_extraction_to_component(&extraction);
                self.extraction_cache.insert(path.clone(), extraction);
                self.tree_cache.insert(path, tree);
            }
            Err(e) => {
                warn!(error = %e, "failed to re-analyze, removing from cache");
                self.source_cache.remove(&abs_path);
                self.extraction_cache.remove(&abs_path);
                self.tree_cache.remove(&abs_path);
                self.model_builder.remove_file(&abs_path);
            }
        }
        let parse_extract_ms = parse_start.elapsed().as_millis() as u64;

        self.build_result(start, parse_extract_ms)
    }

    /// Handle a file deletion.
    #[instrument(skip(self), fields(file = %path.display()))]
    pub fn on_file_deleted(&mut self, path: &Path) -> Result<ExtractionResult> {
        let start = Instant::now();

        let abs_path = self.resolve_path(path);
        info!(?abs_path, "file deleted, removing from caches");

        self.source_cache.remove(&abs_path);
        self.extraction_cache.remove(&abs_path);
        self.tree_cache.remove(&abs_path);
        self.model_builder.remove_file(&abs_path);

        // No parse/extract on deletion
        self.build_result(start, 0)
    }

    /// Handle a batch of file changes (for debouncing).
    #[instrument(skip(self), fields(count = paths.len()))]
    pub fn on_files_changed(&mut self, paths: &[PathBuf]) -> Result<ExtractionResult> {
        let start = Instant::now();

        let parse_start = Instant::now();
        for path in paths {
            let abs_path = self.resolve_path(path);
            let old_tree = self.prepare_incremental_tree(&abs_path);
            match self.parse_and_extract(&abs_path, old_tree.as_ref()) {
                Ok((p, source, extraction, tree)) => {
                    self.source_cache.insert(p.clone(), source);
                    self.route_extraction_to_component(&extraction);
                    self.extraction_cache.insert(p.clone(), extraction);
                    self.tree_cache.insert(p, tree);
                }
                Err(e) => {
                    warn!(file = %abs_path.display(), error = %e, "skipping in batch");
                    self.source_cache.remove(&abs_path);
                    self.extraction_cache.remove(&abs_path);
                    self.tree_cache.remove(&abs_path);
                    self.model_builder.remove_file(&abs_path);
                }
            }
        }
        let parse_extract_ms = parse_start.elapsed().as_millis() as u64;

        self.build_result(start, parse_extract_ms)
    }

    /// Analyze a single file on-demand without updating any caches.
    ///
    /// Used by integrations (e.g. MCP server) that need file-level detail
    /// without side-effecting the engine state.
    pub fn analyze_single_file(&self, path: &Path) -> Result<FileExtraction> {
        let abs_path = self.resolve_path(path);
        let (_path, _source, extraction, _tree) = self.parse_and_extract(&abs_path, None)?;
        Ok(extraction)
    }

    /// Get the cached source text for a file, if available.
    ///
    /// Returns `None` if the file has not been analyzed. Falls back to
    /// reading from disk if the file is not in the cache.
    pub fn get_source(&self, path: &Path) -> Option<String> {
        let abs_path = self.resolve_path(path);
        self.source_cache
            .get(&abs_path)
            .cloned()
            .or_else(|| std::fs::read_to_string(&abs_path).ok())
    }

    /// Get all cached sources.
    ///
    /// Useful for downstream consumers (e.g. policy engines) that need
    /// access to raw source text for scanning.
    pub fn sources(&self) -> &HashMap<PathBuf, String> {
        &self.source_cache
    }

    /// Get the cached extraction for a file, if available.
    pub fn get_extraction(&self, path: &Path) -> Option<&FileExtraction> {
        let abs_path = self.resolve_path(path);
        self.extraction_cache.get(&abs_path)
    }

    /// Get all cached extractions.
    pub fn extractions(&self) -> &HashMap<PathBuf, FileExtraction> {
        &self.extraction_cache
    }

    /// Get the knowledge graph, if available.
    ///
    /// The graph is built during each extraction pass and provides O(1)
    /// adjacency lookups for callers, callees, impact analysis, and
    /// cycle detection.
    pub fn graph(&self) -> Option<&KnowledgeGraph> {
        self.knowledge_graph.as_ref()
    }

    /// Inject a pre-built knowledge graph. Used by downstream crates in tests
    /// to set up graph state without running full analysis.
    #[doc(hidden)]
    pub fn set_knowledge_graph(&mut self, graph: KnowledgeGraph) {
        self.knowledge_graph = Some(graph);
    }

    /// Get the detected workspace layout, if any.
    ///
    /// Returns `Some` for monorepo projects with a recognized workspace manifest
    /// (pnpm, npm, Cargo, Go, uv). Returns `None` for single-project repos.
    pub fn workspace_layout(&self) -> Option<&WorkspaceLayout> {
        self.workspace_layout.as_ref()
    }

    /// Route a file extraction to the correct component based on workspace layout.
    ///
    /// If a workspace is detected and the file falls within a package root,
    /// routes to that package's component. Otherwise, routes to the default
    /// component via `set_file()`.
    fn route_extraction_to_component(&mut self, extraction: &FileExtraction) {
        if let Some(ref layout) = self.workspace_layout {
            if let Some(component_name) = layout.component_for_path(&extraction.file) {
                self.model_builder
                    .set_file_in_component(extraction, component_name);
                return;
            }
        }
        self.model_builder.set_file(extraction);
    }

    /// Run the standard graph analysis pipeline on the current knowledge graph.
    ///
    /// Executes: degree centrality → entry point detection → process flow
    /// tracing → cycle analysis. Returns `None` if no graph is available
    /// (i.e., no extraction has been run yet).
    pub fn run_graph_analysis(&self) -> Option<AnalysisContext> {
        let graph = self.knowledge_graph.as_ref()?;
        let pipeline = AnalysisPipeline::standard();
        Some(pipeline.run(graph))
    }

    /// Run a custom graph analysis pipeline on the current knowledge graph.
    ///
    /// Allows downstream consumers to compose their own analysis passes.
    /// Returns `None` if no graph is available.
    pub fn run_custom_analysis(&self, pipeline: AnalysisPipeline) -> Option<AnalysisContext> {
        let graph = self.knowledge_graph.as_ref()?;
        Some(pipeline.run(graph))
    }

    /// Discover all supported source files under the project root.
    fn discover_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in WalkDir::new(&self.project_root)
            .into_iter()
            .filter_entry(|e| !is_ignored_dir(e))
        {
            let entry = entry?;
            if entry.file_type().is_file() && parser::detect_language(entry.path()).is_some() {
                files.push(entry.path().to_path_buf());
            }
        }

        Ok(files)
    }

    /// Prepare an old tree for incremental parsing by applying InputEdit.
    ///
    /// Reads the new source from disk, compares with cached old source,
    /// and applies the computed edit to a clone of the cached tree.
    /// Returns `None` if no old tree/source is cached or sources are identical.
    fn prepare_incremental_tree(&self, abs_path: &Path) -> Option<tree_sitter::Tree> {
        let old_tree = self.tree_cache.get(abs_path)?;
        let old_source = self.source_cache.get(abs_path)?;
        let new_source = std::fs::read_to_string(abs_path).ok()?;

        let edit = parser::compute_input_edit(old_source, &new_source)?;

        let mut tree = old_tree.clone();
        tree.edit(&edit);
        Some(tree)
    }

    /// Parse and extract a single file, returning the tree for caching.
    ///
    /// The `old_tree` parameter is passed through to tree-sitter for
    /// incremental parsing. Callers must apply `InputEdit` to the old tree
    /// before passing it — otherwise tree-sitter assumes nothing changed
    /// and reuses the entire old CST (producing incorrect results).
    fn parse_and_extract(
        &self,
        path: &Path,
        old_tree: Option<&tree_sitter::Tree>,
    ) -> Result<(PathBuf, String, FileExtraction, tree_sitter::Tree)> {
        let source = std::fs::read_to_string(path).map_err(|e| IntentlyError::Io { source: e })?;

        let language =
            parser::detect_language(path).ok_or_else(|| IntentlyError::UnsupportedLanguage {
                path: path.to_path_buf(),
            })?;

        let parsed = parser::parse_source(path, &source, language, old_tree)?;

        let mut extraction = extractors::extract(path, &source, &parsed.tree, language);
        extraction.content_hash = Some(compute_sha256(source.as_bytes()));

        Ok((path.to_path_buf(), source, extraction, parsed.tree))
    }

    /// Parse and extract using thread-local cached parsers.
    ///
    /// Used by `full_analysis` for parallel processing where parser reuse
    /// within a rayon thread avoids repeated allocation.
    fn parse_and_extract_cached(
        &self,
        path: &Path,
    ) -> Result<(PathBuf, String, FileExtraction, tree_sitter::Tree)> {
        let source = std::fs::read_to_string(path).map_err(|e| IntentlyError::Io { source: e })?;

        let language =
            parser::detect_language(path).ok_or_else(|| IntentlyError::UnsupportedLanguage {
                path: path.to_path_buf(),
            })?;

        let parsed = parser::parse_source_cached(path, &source, language, None)?;

        let mut extraction = extractors::extract(path, &source, &parsed.tree, language);
        extraction.content_hash = Some(compute_sha256(source.as_bytes()));

        Ok((path.to_path_buf(), source, extraction, parsed.tree))
    }

    /// Build the extraction result from current caches.
    fn build_result(&mut self, start: Instant, parse_extract_ms: u64) -> Result<ExtractionResult> {
        let model_start = Instant::now();
        #[allow(unused_mut)]
        let mut model = {
            let _span = info_span!("build_model", files = self.extraction_cache.len(),).entered();
            self.model_builder.build()
        };
        let model_build_ms = model_start.elapsed().as_millis() as u64;

        // Compute and attach git stats when metadata is available
        #[cfg(feature = "git")]
        if let Some(ref git_metadata) = self.git_metadata_cache {
            model.stats.git_stats = Some(crate::git::metadata::compute_git_stats(git_metadata));
        }

        // Build knowledge graph from code model (derived view for traversal)
        let (graph_stats, graph) = {
            let _span = info_span!("build_graph", components = model.components.len(),).entered();
            let g = KnowledgeGraph::from_code_model(&model);
            let stats = Some(g.stats());
            (stats, g)
        };
        self.knowledge_graph = Some(graph);

        let semantic_diff = self
            .previous_model
            .as_ref()
            .map(|prev| diff::compute_diff(prev, &model));

        let total_ms = start.elapsed().as_millis() as u64;

        let timing = PipelineTiming {
            parse_extract_ms,
            model_build_ms,
            total_ms,
        };

        let files_analyzed = self.extraction_cache.len();

        debug!(
            parse_extract_ms,
            model_build_ms, total_ms, "pipeline timing"
        );
        info!(
            duration_ms = total_ms,
            files_analyzed, "extraction complete"
        );

        self.previous_model = Some(model.clone());

        Ok(ExtractionResult {
            model,
            diff: semantic_diff,
            timing,
            graph_stats,
            duration_ms: total_ms,
            files_analyzed,
        })
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        }
    }
}

/// Compute SHA-256 hash of file content for cache invalidation.
fn compute_sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn is_ignored_dir(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|name| IGNORED_DIRS.contains(&name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_project(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, content) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content).unwrap();
        }
        dir
    }

    #[test]
    fn full_analysis_discovers_and_analyzes_files() {
        let dir = create_project(&[(
            "src/index.ts",
            r#"
import express from 'express';
const app = express();
app.get('/health', (req, res) => res.json({ ok: true }));
"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        assert_eq!(result.files_analyzed, 1);
        assert_eq!(result.model.components[0].interfaces.len(), 1);
        assert!(result.diff.is_none(), "first analysis has no diff");
    }

    #[test]
    fn incremental_analysis_on_file_change() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"
const app = require('express')();
app.get('/health', (req, res) => res.json({ ok: true }));
"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let first = engine.full_analysis().unwrap();
        assert_eq!(first.model.components[0].interfaces.len(), 1);

        // Add a new endpoint
        std::fs::write(
            dir.path().join("src/app.ts"),
            r#"
const app = require('express')();
app.get('/health', (req, res) => res.json({ ok: true }));
app.post('/api/users', (req, res) => res.status(201).json({}));
"#,
        )
        .unwrap();

        let second = engine.on_file_changed(Path::new("src/app.ts")).unwrap();

        assert_eq!(second.model.components[0].interfaces.len(), 2);
        assert!(second.diff.is_some(), "second analysis should have a diff");

        let diff = second.diff.unwrap();
        assert!(
            !diff.interface_changes.is_empty(),
            "should detect the new endpoint"
        );
    }

    #[test]
    fn file_deletion_removes_from_code_model() {
        let dir = create_project(&[
            (
                "src/a.ts",
                r#"
const app = require('express')();
app.get('/a', (req, res) => res.json({}));
"#,
            ),
            (
                "src/b.ts",
                r#"
const router = require('express').Router();
router.get('/b', (req, res) => res.json({}));
"#,
            ),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();
        assert_eq!(result.model.components[0].interfaces.len(), 2);

        let result = engine
            .on_file_deleted(&dir.path().join("src/b.ts"))
            .unwrap();
        assert_eq!(result.model.components[0].interfaces.len(), 1);
        assert_eq!(result.model.components[0].interfaces[0].path, "/a");
    }

    #[test]
    fn skips_node_modules() {
        let dir = create_project(&[
            (
                "src/app.ts",
                r#"app.get('/real', (req, res) => res.json({}));"#,
            ),
            (
                "node_modules/express/index.ts",
                r#"app.get('/fake', (req, res) => {});"#,
            ),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        assert_eq!(result.files_analyzed, 1);
    }

    #[test]
    fn batch_file_changes() {
        let dir = create_project(&[
            ("src/a.ts", r#"app.get('/a', (req, res) => res.json({}));"#),
            ("src/b.ts", r#"app.get('/b', (req, res) => res.json({}));"#),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        engine.full_analysis().unwrap();

        // Update both files
        std::fs::write(
            dir.path().join("src/a.ts"),
            r#"app.get('/a-new', (req, res) => res.json({}));"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/b.ts"),
            r#"app.get('/b-new', (req, res) => res.json({}));"#,
        )
        .unwrap();

        let result = engine
            .on_files_changed(&[dir.path().join("src/a.ts"), dir.path().join("src/b.ts")])
            .unwrap();

        let paths: Vec<&str> = result.model.components[0]
            .interfaces
            .iter()
            .map(|i| i.path.as_str())
            .collect();
        assert!(paths.contains(&"/a-new"));
        assert!(paths.contains(&"/b-new"));
    }

    #[test]
    fn handles_empty_project() {
        let dir = create_project(&[]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        assert_eq!(result.files_analyzed, 0);
        assert!(result.model.components[0].interfaces.is_empty());
    }

    #[test]
    fn analyze_single_file_returns_extraction_without_cache_mutation() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"
const app = require('express')();
app.get('/health', (req, res) => res.json({ ok: true }));
"#,
        )]);

        let engine = IntentlyEngine::new(dir.path().to_path_buf());
        let extraction = engine.analyze_single_file(Path::new("src/app.ts")).unwrap();

        assert_eq!(extraction.interfaces.len(), 1);
        assert_eq!(extraction.interfaces[0].path, "/health");
        // Engine caches remain empty — no side effects
        assert!(engine.extraction_cache.is_empty());
        assert!(engine.tree_cache.is_empty());
    }

    #[test]
    fn extraction_result_serializes_to_json() {
        let dir = create_project(&[(
            "index.ts",
            r#"app.get('/test', (req, res) => res.json({}));"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("model"));
        assert!(json.contains("timing"));
    }

    #[test]
    fn sources_accessor_returns_cached_sources() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"app.get('/test', (req, res) => res.json({}));"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        engine.full_analysis().unwrap();

        assert_eq!(engine.sources().len(), 1);
        assert!(engine
            .sources()
            .values()
            .next()
            .unwrap()
            .contains("app.get"));
    }

    #[test]
    fn extractions_have_content_hash() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"app.get('/test', (req, res) => res.json({}));"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        engine.full_analysis().unwrap();

        for extraction in engine.extractions().values() {
            assert!(
                extraction.content_hash.is_some(),
                "all extractions should have a content hash after analysis"
            );
        }
    }

    #[test]
    fn content_hash_changes_when_file_changes() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"app.get('/v1', (req, res) => res.json({}));"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        engine.full_analysis().unwrap();

        let hash_before = engine.extractions().values().next().unwrap().content_hash;

        // Modify file content
        std::fs::write(
            dir.path().join("src/app.ts"),
            r#"app.get('/v2', (req, res) => res.json({}));"#,
        )
        .unwrap();

        engine.on_file_changed(Path::new("src/app.ts")).unwrap();

        let hash_after = engine.extractions().values().next().unwrap().content_hash;

        assert_ne!(
            hash_before, hash_after,
            "content hash should change when file is modified"
        );
    }

    #[test]
    fn file_role_and_tokens_populated_after_analysis() {
        let dir = create_project(&[(
            "src/app.ts",
            r#"app.get('/test', (req, res) => res.json({}));"#,
        )]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        for extraction in engine.extractions().values() {
            assert_eq!(
                extraction.file_role,
                crate::model::types::FileRole::Implementation,
                "src/*.ts files should be classified as Implementation"
            );
            assert!(
                extraction.estimated_tokens > 0,
                "non-empty files should have estimated tokens > 0"
            );
        }

        assert!(
            result.model.stats.total_estimated_tokens > 0,
            "total estimated tokens should be aggregated in stats"
        );
        assert!(
            !result.model.stats.file_roles.is_empty(),
            "file roles breakdown should be populated in stats"
        );
    }
}
