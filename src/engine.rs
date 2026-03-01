//! IntentlyEngine: the stateful orchestrator for realtime code extraction.
//!
//! Maintains in-memory caches for parsed files and extractions.
//! On file change, re-parses only the changed file, rebuilds the twin
//! from all cached extractions, and builds the knowledge graph.
//!
//! This engine is purely extractive — it answers "what does this codebase
//! contain?" Policy evaluation, health scores, and governance are the
//! responsibility of downstream consumers.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use tracing::{debug, info, instrument, warn};
use walkdir::WalkDir;

use crate::error::{IntentlyError, Result};
use crate::parser;
use crate::twin::builder::TwinBuilder;
use crate::twin::diff::{self, SemanticDiff};
use crate::twin::extractors;
use crate::twin::graph::{GraphStats, KnowledgeGraph};
use crate::twin::types::{FileExtraction, SystemTwin};

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

/// Per-stage timing breakdown of the extraction pipeline.
///
/// Enables the IDE to display pipeline performance and identify bottlenecks.
/// All values are in milliseconds.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PipelineTiming {
    /// Time spent parsing and extracting semantic information from source files.
    pub parse_extract_ms: u64,
    /// Time spent building the System Twin from per-file contributions.
    pub twin_build_ms: u64,
    /// Total wall-clock time for the entire extraction pipeline.
    pub total_ms: u64,
}

/// Result of a full or incremental extraction pass.
///
/// Contains the System Twin, semantic diff, knowledge graph stats,
/// and timing information. Does NOT contain policy or health data —
/// those are governance concerns for downstream consumers.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtractionResult {
    pub twin: SystemTwin,
    pub diff: Option<SemanticDiff>,
    pub timing: PipelineTiming,
    pub graph_stats: Option<GraphStats>,
    pub duration_ms: u64,
    pub files_analyzed: usize,
}

/// The stateful Intently extraction engine.
///
/// Manages parsing, extraction, twin building, and knowledge graph
/// construction with incremental caching for real-time performance.
pub struct IntentlyEngine {
    project_root: PathBuf,
    project_name: String,
    source_cache: HashMap<PathBuf, String>,
    extraction_cache: HashMap<PathBuf, FileExtraction>,
    previous_twin: Option<SystemTwin>,
    /// Cached tree-sitter CSTs for incremental re-parsing.
    /// When a file is re-parsed, the old tree enables tree-sitter to reuse
    /// unchanged nodes, reducing parse time from O(file_size) to O(edit_size).
    tree_cache: HashMap<PathBuf, tree_sitter::Tree>,
    /// Incremental twin builder tracking per-file contributions.
    /// Instead of rebuilding from all cached extractions, updates are applied
    /// as deltas (set_file/remove_file) for O(1) incremental changes.
    twin_builder: TwinBuilder,
    /// Derived knowledge graph built from the SystemTwin after each extraction.
    /// Provides O(1) adjacency lookups for callers/callees/impact analysis
    /// and structural analysis (cycle detection).
    knowledge_graph: Option<KnowledgeGraph>,
}

impl IntentlyEngine {
    /// Create a new engine for the given project root.
    pub fn new(project_root: PathBuf) -> Self {
        let project_name = project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let twin_builder = TwinBuilder::with_root(&project_name, &project_root);
        Self {
            project_root,
            project_name,
            source_cache: HashMap::new(),
            extraction_cache: HashMap::new(),
            previous_twin: None,
            tree_cache: HashMap::new(),
            twin_builder,
            knowledge_graph: None,
        }
    }

    /// Run a full extraction: discover all files, parse, extract, build twin.
    #[instrument(skip(self), fields(project = %self.project_name))]
    pub fn full_analysis(&mut self) -> Result<ExtractionResult> {
        let start = Instant::now();

        let files = self.discover_files()?;
        info!(file_count = files.len(), "discovered source files");

        // Parse and extract all files in parallel
        let parse_start = Instant::now();
        let results: Vec<(PathBuf, String, FileExtraction, tree_sitter::Tree)> = files
            .par_iter()
            .filter_map(|path| {
                match self.parse_and_extract(path, None) {
                    Ok(result) => Some(result),
                    Err(e) => {
                        warn!(?path, error = %e, "skipping file");
                        None
                    }
                }
            })
            .collect();
        let parse_extract_ms = parse_start.elapsed().as_millis() as u64;

        // Populate caches and twin builder
        self.source_cache.clear();
        self.extraction_cache.clear();
        self.tree_cache.clear();
        self.twin_builder = TwinBuilder::with_root(&self.project_name, &self.project_root);
        for (path, source, extraction, tree) in results {
            self.source_cache.insert(path.clone(), source);
            self.twin_builder.set_file(&extraction);
            self.extraction_cache.insert(path.clone(), extraction);
            self.tree_cache.insert(path, tree);
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
                self.twin_builder.set_file(&extraction);
                self.extraction_cache.insert(path.clone(), extraction);
                self.tree_cache.insert(path, tree);
            }
            Err(e) => {
                warn!(error = %e, "failed to re-analyze, removing from cache");
                self.source_cache.remove(&abs_path);
                self.extraction_cache.remove(&abs_path);
                self.tree_cache.remove(&abs_path);
                self.twin_builder.remove_file(&abs_path);
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
        self.twin_builder.remove_file(&abs_path);

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
                    self.twin_builder.set_file(&extraction);
                    self.extraction_cache.insert(p.clone(), extraction);
                    self.tree_cache.insert(p, tree);
                }
                Err(e) => {
                    warn!(file = %abs_path.display(), error = %e, "skipping in batch");
                    self.source_cache.remove(&abs_path);
                    self.extraction_cache.remove(&abs_path);
                    self.tree_cache.remove(&abs_path);
                    self.twin_builder.remove_file(&abs_path);
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

    /// Discover all supported source files under the project root.
    fn discover_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for entry in WalkDir::new(&self.project_root)
            .into_iter()
            .filter_entry(|e| !is_ignored_dir(e))
        {
            let entry = entry?;
            if entry.file_type().is_file()
                && parser::detect_language(entry.path()).is_some()
            {
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

        let language = parser::detect_language(path)
            .ok_or_else(|| IntentlyError::UnsupportedLanguage {
                path: path.to_path_buf(),
            })?;

        let parsed = parser::parse_source(path, &source, language, old_tree)?;

        let extraction = extractors::extract(path, &source, &parsed.tree, language);

        Ok((path.to_path_buf(), source, extraction, parsed.tree))
    }

    /// Build the extraction result from current caches.
    fn build_result(
        &mut self,
        start: Instant,
        parse_extract_ms: u64,
    ) -> Result<ExtractionResult> {
        let twin_start = Instant::now();
        let twin = self.twin_builder.build();
        let twin_build_ms = twin_start.elapsed().as_millis() as u64;

        // Build knowledge graph from twin (derived view for traversal)
        let graph = KnowledgeGraph::from_twin(&twin);
        let graph_stats = Some(graph.stats());
        self.knowledge_graph = Some(graph);

        let semantic_diff = self
            .previous_twin
            .as_ref()
            .map(|prev| diff::compute_diff(prev, &twin));

        let total_ms = start.elapsed().as_millis() as u64;

        let timing = PipelineTiming {
            parse_extract_ms,
            twin_build_ms,
            total_ms,
        };

        let files_analyzed = self.extraction_cache.len();

        debug!(
            parse_extract_ms,
            twin_build_ms,
            total_ms,
            "pipeline timing"
        );
        info!(duration_ms = total_ms, files_analyzed, "extraction complete");

        self.previous_twin = Some(twin.clone());

        Ok(ExtractionResult {
            twin,
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
        let dir = create_project(&[
            ("src/index.ts", r#"
import express from 'express';
const app = express();
app.get('/health', (req, res) => res.json({ ok: true }));
"#),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        assert_eq!(result.files_analyzed, 1);
        assert_eq!(result.twin.components[0].interfaces.len(), 1);
        assert!(result.diff.is_none(), "first analysis has no diff");
    }

    #[test]
    fn incremental_analysis_on_file_change() {
        let dir = create_project(&[
            ("src/app.ts", r#"
const app = require('express')();
app.get('/health', (req, res) => res.json({ ok: true }));
"#),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let first = engine.full_analysis().unwrap();
        assert_eq!(first.twin.components[0].interfaces.len(), 1);

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

        let second = engine
            .on_file_changed(Path::new("src/app.ts"))
            .unwrap();

        assert_eq!(second.twin.components[0].interfaces.len(), 2);
        assert!(second.diff.is_some(), "second analysis should have a diff");

        let diff = second.diff.unwrap();
        assert!(
            !diff.interface_changes.is_empty(),
            "should detect the new endpoint"
        );
    }

    #[test]
    fn file_deletion_removes_from_twin() {
        let dir = create_project(&[
            ("src/a.ts", r#"
const app = require('express')();
app.get('/a', (req, res) => res.json({}));
"#),
            ("src/b.ts", r#"
const router = require('express').Router();
router.get('/b', (req, res) => res.json({}));
"#),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();
        assert_eq!(result.twin.components[0].interfaces.len(), 2);

        let result = engine
            .on_file_deleted(&dir.path().join("src/b.ts"))
            .unwrap();
        assert_eq!(result.twin.components[0].interfaces.len(), 1);
        assert_eq!(result.twin.components[0].interfaces[0].path, "/a");
    }

    #[test]
    fn skips_node_modules() {
        let dir = create_project(&[
            ("src/app.ts", r#"app.get('/real', (req, res) => res.json({}));"#),
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
            .on_files_changed(&[
                dir.path().join("src/a.ts"),
                dir.path().join("src/b.ts"),
            ])
            .unwrap();

        let paths: Vec<&str> = result.twin.components[0]
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
        assert!(result.twin.components[0].interfaces.is_empty());
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
        let extraction = engine
            .analyze_single_file(Path::new("src/app.ts"))
            .unwrap();

        assert_eq!(extraction.interfaces.len(), 1);
        assert_eq!(extraction.interfaces[0].path, "/health");
        // Engine caches remain empty — no side effects
        assert!(engine.extraction_cache.is_empty());
        assert!(engine.tree_cache.is_empty());
    }

    #[test]
    fn extraction_result_serializes_to_json() {
        let dir = create_project(&[
            ("index.ts", r#"app.get('/test', (req, res) => res.json({}));"#),
        ]);

        let mut engine = IntentlyEngine::new(dir.path().to_path_buf());
        let result = engine.full_analysis().unwrap();

        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("twin"));
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
        assert!(engine.sources().values().next().unwrap().contains("app.get"));
    }
}
