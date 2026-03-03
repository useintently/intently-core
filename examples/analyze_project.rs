//! Analyze any project directory and output structured extraction results.
//!
//! Usage:
//!   cargo run --example analyze_project -- /path/to/project
//!   cargo run --features git --example analyze_project -- /path/to/project
//!   cargo run --example analyze_project -- /path/to/project --json  # Full JSON to stdout

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use intently_core::IntentlyEngine;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_mode = args.contains(&"--json".to_string());

    let project_path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Usage: cargo run --example analyze_project -- /path/to/project [--json]");
            std::process::exit(1);
        });

    if !project_path.exists() {
        eprintln!("Path does not exist: {}", project_path.display());
        std::process::exit(1);
    }

    eprintln!("Analyzing: {}", project_path.display());
    eprintln!();

    let start = Instant::now();
    let mut engine = IntentlyEngine::new(project_path.clone());

    let result = match engine.full_analysis() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Analysis failed: {e}");
            std::process::exit(1);
        }
    };
    let elapsed = start.elapsed();

    let stats = &result.model.stats;

    // ── Summary ──────────────────────────────────────────────────────
    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!("  PROJECT ANALYSIS SUMMARY");
    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!();
    eprintln!("  Project:        {}", result.model.project_name);
    eprintln!("  Components:     {}", result.model.components.len());
    eprintln!("  Files analyzed: {}", stats.files_analyzed);
    eprintln!("  Total time:     {:.2}s", elapsed.as_secs_f64());
    eprintln!();

    // ── Timing breakdown ─────────────────────────────────────────────
    eprintln!("─── Timing ─────────────────────────────────────────────");
    eprintln!("  Parse + Extract: {}ms", result.timing.parse_extract_ms);
    eprintln!("  Model Build:     {}ms", result.timing.model_build_ms);
    eprintln!("  Total pipeline:  {}ms", result.timing.total_ms);
    eprintln!();

    // ── Core extraction stats ────────────────────────────────────────
    eprintln!("─── Extraction ─────────────────────────────────────────");
    eprintln!("  Interfaces (routes):    {}", stats.total_interfaces);
    eprintln!("  Dependencies (calls):   {}", stats.total_dependencies);
    eprintln!("  Sinks (logs):           {}", stats.total_sinks);
    eprintln!("  Symbols:                {}", stats.total_symbols);
    eprintln!("  Data models:            {}", stats.total_data_models);
    eprintln!("  Imports:                {}", stats.total_imports);
    eprintln!("  References:             {}", stats.total_references);
    eprintln!(
        "  Resolved references:    {} ({:.1}%)",
        stats.resolved_references,
        if stats.total_references > 0 {
            stats.resolved_references as f64 / stats.total_references as f64 * 100.0
        } else {
            0.0
        }
    );
    eprintln!(
        "  Avg confidence:         {:.3}",
        stats.avg_resolution_confidence
    );
    if !stats.resolution_method_distribution.is_empty() {
        let mut methods: Vec<(&str, &usize)> = stats
            .resolution_method_distribution
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        methods.sort_by(|a, b| b.1.cmp(a.1));
        for (method, count) in &methods {
            eprintln!("    {:<22} {}", method, count);
        }
    }
    eprintln!("  Modules:                {}", stats.total_modules);
    eprintln!(
        "  Est. tokens:            {}",
        format_number(stats.total_estimated_tokens as usize)
    );
    eprintln!();

    // ── Phase 3 features ─────────────────────────────────────────────
    eprintln!("─── Phase 3: New Capabilities ──────────────────────────");
    eprintln!("  Test symbols:           {}", stats.total_test_symbols);
    eprintln!("  Env dependencies:       {}", stats.total_env_dependencies);

    // API schema enrichment
    let mut routes_with_params = 0usize;
    let mut routes_with_handler = 0usize;
    let mut routes_with_body_type = 0usize;
    let mut total_params = 0usize;
    for comp in &result.model.components {
        for iface in &comp.interfaces {
            if !iface.parameters.is_empty() {
                routes_with_params += 1;
                total_params += iface.parameters.len();
            }
            if iface.handler_name.is_some() {
                routes_with_handler += 1;
            }
            if iface.request_body_type.is_some() {
                routes_with_body_type += 1;
            }
        }
    }
    eprintln!(
        "  Routes with params:     {} ({} params total)",
        routes_with_params, total_params
    );
    eprintln!("  Routes with handler:    {}", routes_with_handler);
    eprintln!("  Routes with body type:  {}", routes_with_body_type);

    // Git stats
    if let Some(ref git) = stats.git_stats {
        eprintln!("  Git authors:            {}", git.total_authors);
        eprintln!("  Git commits (walked):   {}", git.total_commits);
        eprintln!("  Avg commits/file:       {:.1}", git.avg_commits_per_file);
        if !git.hottest_files.is_empty() {
            eprintln!("  Top 5 hottest files:");
            for (path, count) in git.hottest_files.iter().take(5) {
                let display = path.strip_prefix(&project_path).unwrap_or(path).display();
                eprintln!("    {count:>4} commits  {display}");
            }
        }
    } else {
        eprintln!("  Git metadata:           (disabled or not a git repo)");
    }
    eprintln!();

    // ── File role breakdown ──────────────────────────────────────────
    eprintln!("─── Analyzed File Roles ────────────────────────────────");
    let mut roles: Vec<(&str, &usize)> = stats
        .file_roles
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    roles.sort_by(|a, b| b.1.cmp(a.1));
    for (role, count) in &roles {
        eprintln!("  {:<20} {}", role, count);
    }
    eprintln!();

    // ── Graph stats ──────────────────────────────────────────────────
    if let Some(ref gs) = result.graph_stats {
        eprintln!("─── Knowledge Graph ────────────────────────────────────");
        eprintln!("  Nodes: {}", gs.total_nodes);
        eprintln!("  Edges: {}", gs.total_edges);
        eprintln!();
    }

    // ── Per-component breakdown ──────────────────────────────────────
    if result.model.components.len() > 1 {
        eprintln!("─── Components ─────────────────────────────────────────");
        eprintln!(
            "  {:<30} {:>6} {:>6} {:>6} {:>7} {:>5} {:>5}",
            "Name", "Files", "Routes", "Deps", "Symbols", "Tests", "EnvD"
        );
        for comp in &result.model.components {
            let test_count = comp.symbols.iter().filter(|s| s.is_test).count();
            // Count unique files from symbols
            let mut files: std::collections::HashSet<&PathBuf> = std::collections::HashSet::new();
            for s in &comp.symbols {
                files.insert(&s.anchor.file);
            }
            eprintln!(
                "  {:<30} {:>6} {:>6} {:>6} {:>7} {:>5} {:>5}",
                truncate(&comp.name, 30),
                files.len(),
                comp.interfaces.len(),
                comp.dependencies.len(),
                comp.symbols.len(),
                test_count,
                comp.env_dependencies.len(),
            );
        }
        eprintln!();
    }

    // ── Workspace info ───────────────────────────────────────────────
    if let Some(ws) = engine.workspace_layout() {
        eprintln!("─── Workspace ──────────────────────────────────────────");
        eprintln!("  Kind: {:?}", ws.kind);
        eprintln!("  Packages: {}", ws.packages.len());
        for pkg in ws.packages.iter().take(20) {
            eprintln!("    - {}", pkg.name);
        }
        if ws.packages.len() > 20 {
            eprintln!("    ... and {} more", ws.packages.len() - 20);
        }
        eprintln!();
    }

    // ── Sample env dependencies ──────────────────────────────────────
    let all_env: Vec<_> = result
        .model
        .components
        .iter()
        .flat_map(|c| c.env_dependencies.iter())
        .collect();
    if !all_env.is_empty() {
        eprintln!("─── Environment Variables (sample) ─────────────────────");
        let mut seen = HashMap::new();
        for env in &all_env {
            *seen.entry(env.var_name.as_str()).or_insert(0usize) += 1;
        }
        let mut sorted: Vec<_> = seen.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (name, count) in sorted.iter().take(15) {
            eprintln!("  {count:>3}x  {name}");
        }
        if sorted.len() > 15 {
            eprintln!("  ... and {} more unique vars", sorted.len() - 15);
        }
        eprintln!();
    }

    // ── Sample interfaces with parameters ────────────────────────────
    let enriched: Vec<_> = result
        .model
        .components
        .iter()
        .flat_map(|c| c.interfaces.iter())
        .filter(|i| !i.parameters.is_empty() || i.handler_name.is_some())
        .collect();
    if !enriched.is_empty() {
        eprintln!("─── API Routes (sample enriched) ───────────────────────");
        for iface in enriched.iter().take(10) {
            let handler = iface.handler_name.as_deref().unwrap_or("?");
            let params: Vec<_> = iface
                .parameters
                .iter()
                .map(|p| {
                    let loc = format!("{:?}", p.location);
                    if let Some(ref t) = p.param_type {
                        format!("{}:{} ({})", p.name, t, loc)
                    } else {
                        format!("{} ({})", p.name, loc)
                    }
                })
                .collect();
            let body = iface
                .request_body_type
                .as_deref()
                .map(|b| format!(" body={b}"))
                .unwrap_or_default();
            eprintln!(
                "  {:?} {} → {}  params=[{}]{}",
                iface.method,
                iface.path,
                handler,
                params.join(", "),
                body,
            );
        }
        if enriched.len() > 10 {
            eprintln!("  ... and {} more enriched routes", enriched.len() - 10);
        }
        eprintln!();
    }

    eprintln!("═══════════════════════════════════════════════════════════");

    // ── Full JSON output ─────────────────────────────────────────────
    if json_mode {
        let json = serde_json::to_string_pretty(&result.model)
            .expect("JSON serialization should not fail");
        println!("{json}");
    }
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
