//! Analyze all test fixtures and output structured JSON + summary table.
//!
//! Usage:
//!   cargo run --example analyze_fixtures                     # All fixtures
//!   cargo run --example analyze_fixtures -- express_ecommerce # Single fixture
//!   cargo run --example analyze_fixtures -- --summary        # Summary table only

use std::path::PathBuf;
use std::time::Instant;

use intently_core::IntentlyEngine;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures_dir = manifest_dir.join("tests/fixtures");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let summary_only = args.contains(&"--summary".to_string());
    let filter: Option<&str> = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str());

    let mut fixture_dirs: Vec<PathBuf> = match std::fs::read_dir(&fixtures_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect(),
        Err(e) => {
            eprintln!("Failed to read fixtures directory: {e}");
            std::process::exit(1);
        }
    };
    fixture_dirs.sort();

    if let Some(name) = filter {
        fixture_dirs.retain(|d| {
            d.file_name()
                .map(|n| n.to_string_lossy().contains(name))
                .unwrap_or(false)
        });
    }

    if fixture_dirs.is_empty() {
        eprintln!("No fixtures found matching filter.");
        std::process::exit(1);
    }

    let mut results = serde_json::Map::new();

    // Header for summary table
    eprintln!(
        "{:<28} {:>5} {:>6} {:>5} {:>5} {:>7} {:>8} {:>12}",
        "Fixture", "Files", "Routes", "Deps", "Sinks", "Symbols", "Time(ms)", "Workspace"
    );
    eprintln!("{}", "-".repeat(88));

    for fixture_path in &fixture_dirs {
        let fixture_name = fixture_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let start = Instant::now();
        let mut engine = IntentlyEngine::new(fixture_path.clone());

        let result = match engine.full_analysis() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{:<28} ERROR: {}", fixture_name, e);
                continue;
            }
        };
        let elapsed_ms = start.elapsed().as_millis();

        let stats = &result.model.stats;
        let workspace_info = engine
            .workspace_layout()
            .map(|l| format!("{:?}({})", l.kind, l.packages.len()))
            .unwrap_or_else(|| "none".to_string());

        let total_routes: usize = result
            .model
            .components
            .iter()
            .map(|c| c.interfaces.len())
            .sum();
        let total_deps: usize = result
            .model
            .components
            .iter()
            .map(|c| c.dependencies.len())
            .sum();
        let total_sinks: usize = result.model.components.iter().map(|c| c.sinks.len()).sum();
        let total_symbols: usize = result
            .model
            .components
            .iter()
            .map(|c| c.symbols.len())
            .sum();

        // Summary line to stderr
        eprintln!(
            "{:<28} {:>5} {:>6} {:>5} {:>5} {:>7} {:>8} {:>12}",
            fixture_name,
            stats.files_analyzed,
            total_routes,
            total_deps,
            total_sinks,
            total_symbols,
            elapsed_ms,
            workspace_info,
        );

        if !summary_only {
            let component_summaries: Vec<serde_json::Value> = result
                .model
                .components
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "interfaces": c.interfaces.len(),
                        "dependencies": c.dependencies.len(),
                        "sinks": c.sinks.len(),
                        "symbols": c.symbols.len(),
                        "data_models": c.data_models.len(),
                        "references": c.references.len(),
                        "imports": c.imports.len(),
                    })
                })
                .collect();

            let fixture_json = serde_json::json!({
                "files_analyzed": stats.files_analyzed,
                "total_interfaces": stats.total_interfaces,
                "total_dependencies": stats.total_dependencies,
                "total_sinks": stats.total_sinks,
                "total_symbols": stats.total_symbols,
                "total_references": stats.total_references,
                "total_data_models": stats.total_data_models,
                "total_imports": stats.total_imports,
                "total_estimated_tokens": stats.total_estimated_tokens,
                "workspace": engine.workspace_layout().map(|l| {
                    serde_json::json!({
                        "kind": format!("{:?}", l.kind),
                        "packages": l.packages.iter().map(|p| {
                            serde_json::json!({
                                "name": p.name,
                                "root": p.root.strip_prefix(fixture_path).unwrap_or(&p.root).display().to_string(),
                            })
                        }).collect::<Vec<_>>(),
                    })
                }),
                "components": component_summaries,
                "timing": {
                    "parse_extract_ms": result.timing.parse_extract_ms,
                    "model_build_ms": result.timing.model_build_ms,
                    "total_ms": result.timing.total_ms,
                },
                "graph_stats": result.graph_stats.as_ref().map(|g| {
                    serde_json::json!({
                        "total_nodes": g.total_nodes,
                        "total_edges": g.total_edges,
                    })
                }),
            });

            results.insert(fixture_name, fixture_json);
        }
    }

    eprintln!("{}", "-".repeat(88));
    eprintln!("Total fixtures analyzed: {}", fixture_dirs.len());

    if !summary_only {
        let output = serde_json::to_string_pretty(&serde_json::Value::Object(results))
            .expect("JSON serialization should not fail");
        println!("{output}");
    }
}
