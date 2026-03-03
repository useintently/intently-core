//! Shared helpers for real-world validation tests.
//!
//! Used by both `real_world_validation.rs` and `diverse_validation.rs`.
//! All functions are `pub` so they're accessible from `mod common;` in test files.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use intently_core::{ExtractionResult, IntentlyEngine, WorkspaceKind};
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════
//  Clone & Analyze
// ═══════════════════════════════════════════════════════════════════

/// Clone a GitHub repo (shallow, single branch) into a temp directory.
///
/// Returns `(TempDir, analysis_path)` where `analysis_path` is the subdir
/// to analyze (or the repo root if `subdir` is `None`).
/// `TempDir` ownership ensures automatic cleanup on drop.
pub fn clone_repo(url: &str, subdir: Option<&str>) -> (TempDir, PathBuf) {
    let tmpdir = TempDir::new().expect("failed to create temp directory");

    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            url,
            tmpdir.path().join("repo").to_str().unwrap(),
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
        .expect("failed to execute git clone");

    assert!(
        status.success(),
        "git clone failed for {url} (exit code: {:?})",
        status.code()
    );

    let analysis_path = match subdir {
        Some(sub) => tmpdir.path().join("repo").join(sub),
        None => tmpdir.path().join("repo"),
    };

    assert!(
        analysis_path.exists(),
        "analysis path does not exist: {}",
        analysis_path.display()
    );

    (tmpdir, analysis_path)
}

/// Run full analysis on a cloned repo with a timeout guard.
pub fn analyze_repo(path: &Path, max_duration_secs: u64) -> ExtractionResult {
    let start = Instant::now();
    let mut engine = IntentlyEngine::new(path.to_path_buf());
    let result = engine
        .full_analysis()
        .expect("full_analysis() should not error on real-world repo");

    let elapsed = start.elapsed().as_secs();
    assert!(
        elapsed <= max_duration_secs,
        "analysis took {}s, exceeding {max_duration_secs}s timeout",
        elapsed
    );

    result
}

/// Run full analysis and return both the engine and the result.
///
/// Unlike `analyze_repo`, this preserves the engine for downstream operations
/// (incremental updates, workspace layout, extractions, sources).
pub fn analyze_repo_with_engine(
    path: &Path,
    max_duration_secs: u64,
) -> (IntentlyEngine, ExtractionResult) {
    let start = Instant::now();
    let mut engine = IntentlyEngine::new(path.to_path_buf());
    let result = engine
        .full_analysis()
        .expect("full_analysis() should not error on real-world repo");

    let elapsed = start.elapsed().as_secs();
    assert!(
        elapsed <= max_duration_secs,
        "analysis took {}s, exceeding {max_duration_secs}s timeout",
        elapsed
    );

    (engine, result)
}

// ═══════════════════════════════════════════════════════════════════
//  Assertion Helpers
// ═══════════════════════════════════════════════════════════════════

/// Basic invariants that should hold for any non-empty repo.
pub fn assert_basic_invariants(result: &ExtractionResult, name: &str) {
    assert!(
        result.files_analyzed > 0,
        "[{name}] expected files_analyzed > 0, got {}",
        result.files_analyzed
    );
    assert!(result.timing.total_ms > 0, "[{name}] expected total_ms > 0");
}

/// Assert the repo produces at least `min` routes.
pub fn assert_has_routes(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_interfaces;
    assert!(
        count >= min,
        "[{name}] expected >= {min} routes, got {count}"
    );
}

/// Assert the repo produces at least `min` symbols.
pub fn assert_has_symbols(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_symbols;
    assert!(
        count >= min,
        "[{name}] expected >= {min} symbols, got {count}"
    );
}

/// Assert at least one symbol has a signature (enriched extraction).
pub fn assert_has_enriched_symbols(result: &ExtractionResult, name: &str) {
    let has_sig = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.symbols)
        .any(|s| s.signature.is_some());
    assert!(
        has_sig,
        "[{name}] expected at least one symbol with signature"
    );
}

/// Assert the repo produces at least `min` sinks.
pub fn assert_has_sinks(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_sinks;
    assert!(
        count >= min,
        "[{name}] expected >= {min} sinks, got {count}"
    );
}

/// Assert the repo produces at least `min` imports.
pub fn assert_has_imports(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_imports;
    assert!(
        count >= min,
        "[{name}] expected >= {min} imports, got {count}"
    );
}

/// Assert the repo produces at least `min` data models.
pub fn assert_has_data_models(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_data_models;
    assert!(
        count >= min,
        "[{name}] expected >= {min} data_models, got {count}"
    );
}

/// Assert the repo produces at least `min` references.
pub fn assert_has_references(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_references;
    assert!(
        count >= min,
        "[{name}] expected >= {min} references, got {count}"
    );
}

/// Assert at least `min` references are resolved (confidence > 0).
pub fn assert_has_resolved_references(result: &ExtractionResult, name: &str, min: usize) {
    let resolved = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.references)
        .filter(|r| r.confidence > 0.0)
        .count();
    assert!(
        resolved >= min,
        "[{name}] expected >= {min} resolved references (confidence > 0), got {resolved}"
    );
}

/// Assert module boundaries are inferred.
pub fn assert_has_module_boundaries(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_modules;
    assert!(
        count >= min,
        "[{name}] expected >= {min} module boundaries, got {count}"
    );
}

/// Assert graph stats are present and non-trivial.
pub fn assert_has_graph_stats(result: &ExtractionResult, name: &str) {
    let stats = result
        .graph_stats
        .as_ref()
        .unwrap_or_else(|| panic!("[{name}] expected graph_stats to be Some"));
    assert!(stats.total_nodes > 0, "[{name}] expected graph nodes > 0");
    assert!(stats.total_edges > 0, "[{name}] expected graph edges > 0");
}

/// Assert SourceAnchors are valid (line > 0, non-empty file) on routes.
pub fn assert_anchors_valid(result: &ExtractionResult, name: &str) {
    for comp in &result.model.components {
        for route in &comp.interfaces {
            assert!(
                route.anchor.line > 0,
                "[{name}][{}] route anchor should have line > 0",
                comp.name
            );
        }
        for symbol in &comp.symbols {
            assert!(
                symbol.anchor.line > 0,
                "[{name}][{}] symbol '{}' anchor should have line > 0",
                comp.name,
                symbol.name
            );
        }
    }
}

/// Assert CodeModelStats fields that were previously unchecked.
pub fn assert_stats_populated(result: &ExtractionResult, name: &str) {
    let stats = &result.model.stats;
    assert!(
        stats.total_estimated_tokens > 0,
        "[{name}] expected total_estimated_tokens > 0"
    );
    assert!(
        !stats.file_roles.is_empty(),
        "[{name}] expected file_roles to be non-empty"
    );
    // Guard confidence checks on resolved_references, not total_references:
    // some module systems (CommonJS) produce references with confidence = 0
    if stats.resolved_references > 0 {
        assert!(
            stats.avg_resolution_confidence > 0.0,
            "[{name}] expected avg_resolution_confidence > 0.0 when resolved_references={} > 0",
            stats.resolved_references
        );
        assert!(
            stats.avg_resolution_confidence <= 1.0,
            "[{name}] expected avg_resolution_confidence <= 1.0, got {}",
            stats.avg_resolution_confidence
        );
    }
}

/// Assert all FileExtractions have a content hash set.
pub fn assert_content_hashes_present(engine: &IntentlyEngine, name: &str) {
    for (path, extraction) in engine.extractions() {
        assert!(
            extraction.content_hash.is_some(),
            "[{name}] content_hash missing for {}",
            path.display()
        );
    }
}

/// Assert the repo produces at least `min` env dependency references (Phase 3).
pub fn assert_has_env_dependencies(result: &ExtractionResult, name: &str, min: usize) {
    let count: usize = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.env_dependencies)
        .count();
    assert!(
        count >= min,
        "[{name}] expected >= {min} env_dependencies, got {count}"
    );
}

/// Assert the repo produces at least `min` test symbols (`is_test: true`) (Phase 3).
pub fn assert_has_test_symbols(result: &ExtractionResult, name: &str, min: usize) {
    let count = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.symbols)
        .filter(|s| s.is_test)
        .count();
    assert!(
        count >= min,
        "[{name}] expected >= {min} test symbols, got {count}"
    );
}

/// Assert the repo produces at least `min` enriched routes (handler name or parameters) (Phase 3).
pub fn assert_has_enriched_routes(result: &ExtractionResult, name: &str, min: usize) {
    let count = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.interfaces)
        .filter(|r| r.handler_name.is_some() || !r.parameters.is_empty())
        .count();
    assert!(
        count >= min,
        "[{name}] expected >= {min} enriched routes (handler_name or parameters), got {count}"
    );
}

/// Assert at least `min` references resolved via explicit import statements.
pub fn assert_has_import_based_references(result: &ExtractionResult, name: &str, min: usize) {
    let count = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.references)
        .filter(|r| r.resolution_method.as_str() == "import_based")
        .count();
    assert!(
        count >= min,
        "[{name}] expected >= {min} import_based references, got {count}"
    );
}

/// Assert at least `min` references classified as external (stdlib/third-party).
pub fn assert_has_external_references(result: &ExtractionResult, name: &str, min: usize) {
    let count = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.references)
        .filter(|r| r.resolution_method.as_str() == "external")
        .count();
    assert!(
        count >= min,
        "[{name}] expected >= {min} external references, got {count}"
    );
}

/// Assert the resolution method distribution contains a specific key.
pub fn assert_resolution_distribution_has(result: &ExtractionResult, name: &str, key: &str) {
    let dist = &result.model.stats.resolution_method_distribution;
    assert!(
        dist.contains_key(key),
        "[{name}] expected resolution_method_distribution to contain '{key}', got keys: {:?}",
        dist.keys().collect::<Vec<_>>()
    );
}

/// Assert workspace layout detection with expected kind and minimum packages.
pub fn assert_workspace_layout(
    engine: &IntentlyEngine,
    name: &str,
    expected_kind: WorkspaceKind,
    min_packages: usize,
) {
    let layout = engine
        .workspace_layout()
        .unwrap_or_else(|| panic!("[{name}] expected workspace_layout to be Some"));
    assert_eq!(
        layout.kind, expected_kind,
        "[{name}] workspace kind mismatch"
    );
    assert!(
        layout.packages.len() >= min_packages,
        "[{name}] expected >= {min_packages} packages, got {}",
        layout.packages.len()
    );
}

/// Run graph analysis pipeline on an engine after extraction.
pub fn run_graph_analysis_on(path: &Path, name: &str) {
    let mut engine = IntentlyEngine::new(path.to_path_buf());
    let result = engine
        .full_analysis()
        .expect("full_analysis should succeed");

    assert_has_graph_stats(&result, name);

    if let Some(ctx) = engine.run_graph_analysis() {
        eprintln!(
            "  [{name}] graph analysis: centrality={} entry_points={} flows={} cycles={}",
            ctx.degree_centrality.len(),
            ctx.entry_points.len(),
            ctx.process_flows.len(),
            ctx.cycles.len(),
        );
    } else {
        eprintln!("  [{name}] graph analysis: no graph available");
    }
}

/// Print a multi-line diagnostic report for a project.
pub fn print_report(result: &ExtractionResult, name: &str) {
    let stats = &result.model.stats;
    let authed = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.interfaces)
        .filter(|r| r.auth.is_some())
        .count();
    let pii_sinks = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.sinks)
        .filter(|s| s.contains_pii)
        .count();
    let enriched = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.symbols)
        .filter(|s| s.signature.is_some())
        .count();
    let resolved = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.references)
        .filter(|r| r.confidence > 0.0)
        .count();
    let graph_nodes = result.graph_stats.as_ref().map_or(0, |g| g.total_nodes);
    let graph_edges = result.graph_stats.as_ref().map_or(0, |g| g.total_edges);

    eprintln!(
        "  {:<28} files={:<4} routes={:<4} auth={:<3} sinks={:<4} pii={:<3} symbols={:<5} enriched={:<4} imports={:<4} refs={:<4} resolved={:<4} models={:<3} modules={:<3} components={:<2} graph={}n/{}e time={}ms",
        name,
        result.files_analyzed,
        stats.total_interfaces,
        authed,
        stats.total_sinks,
        pii_sinks,
        stats.total_symbols,
        enriched,
        stats.total_imports,
        stats.total_references,
        resolved,
        stats.total_data_models,
        stats.total_modules,
        result.model.components.len(),
        graph_nodes,
        graph_edges,
        result.timing.total_ms,
    );
}
