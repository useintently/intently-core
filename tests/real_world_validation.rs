//! Real-world validation harness for the Intently extraction engine.
//!
//! These tests clone real GitHub projects, run `full_analysis()`, and validate
//! that the engine extracts meaningful data across all 16 supported languages.
//!
//! ALL tests use `#[ignore]` — they require network access and are slow.
//!
//! ```bash
//! # Run all real-world tests
//! cargo test -p intently_core --test real_world_validation -- --ignored --nocapture
//!
//! # Run one language group
//! cargo test -p intently_core --test real_world_validation typescript -- --ignored --nocapture
//!
//! # Summary table only
//! cargo test -p intently_core --test real_world_validation summary -- --ignored --nocapture
//! ```
//!
//! Disk space: each test clones into a `TempDir` that auto-deletes on drop.
//! Peak usage ~40MB (8 concurrent clones × ~5MB average with `--depth 1`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use intently_core::{ExtractionResult, IntentlyEngine};
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════
//  Helper Functions
// ═══════════════════════════════════════════════════════════════════

/// Clone a GitHub repo (shallow, single branch) into a temp directory.
///
/// Returns `(TempDir, analysis_path)` where `analysis_path` is the subdir
/// to analyze (or the repo root if `subdir` is `None`).
/// `TempDir` ownership ensures automatic cleanup on drop.
fn clone_repo(url: &str, subdir: Option<&str>) -> (TempDir, PathBuf) {
    let tmpdir = TempDir::new().expect("failed to create temp directory");

    let status = Command::new("git")
        .args([
            "clone",
            "--depth", "1",
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
fn analyze_repo(path: &Path, max_duration_secs: u64) -> ExtractionResult {
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

// ─────────────────────────────────────────────────────────────────
//  Assertion Helpers
// ─────────────────────────────────────────────────────────────────

/// Basic invariants that should hold for any non-empty repo.
fn assert_basic_invariants(result: &ExtractionResult, name: &str) {
    assert!(
        result.files_analyzed > 0,
        "[{name}] expected files_analyzed > 0, got {}",
        result.files_analyzed
    );
    assert!(
        result.timing.total_ms > 0,
        "[{name}] expected total_ms > 0"
    );
}

/// Assert the repo produces at least `min` routes.
fn assert_has_routes(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_interfaces;
    assert!(
        count >= min,
        "[{name}] expected >= {min} routes, got {count}"
    );
}

/// Assert the repo produces at least `min` symbols.
fn assert_has_symbols(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_symbols;
    assert!(
        count >= min,
        "[{name}] expected >= {min} symbols, got {count}"
    );
}

/// Assert at least one symbol has a signature (enriched extraction).
fn assert_has_enriched_symbols(result: &ExtractionResult, name: &str) {
    let has_sig = result.model.components[0]
        .symbols
        .iter()
        .any(|s| s.signature.is_some());
    assert!(
        has_sig,
        "[{name}] expected at least one symbol with signature"
    );
}

/// Assert the repo produces at least `min` sinks.
fn assert_has_sinks(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_sinks;
    assert!(
        count >= min,
        "[{name}] expected >= {min} sinks, got {count}"
    );
}

/// Assert the repo produces at least `min` imports.
fn assert_has_imports(result: &ExtractionResult, name: &str, min: usize) {
    let count = result.model.stats.total_imports;
    assert!(
        count >= min,
        "[{name}] expected >= {min} imports, got {count}"
    );
}

/// Print a one-line diagnostic report for a project.
fn print_report(result: &ExtractionResult, name: &str) {
    let comp = &result.model.components[0];
    let authed = comp.interfaces.iter().filter(|r| r.auth.is_some()).count();
    let pii_sinks = comp.sinks.iter().filter(|s| s.contains_pii).count();
    let enriched = comp.symbols.iter().filter(|s| s.signature.is_some()).count();

    eprintln!(
        "  {:<28} files={:<5} routes={:<5} auth={:<4} sinks={:<5} pii={:<4} symbols={:<6} enriched={:<5} imports={:<5} time={}ms",
        name,
        result.files_analyzed,
        result.model.stats.total_interfaces,
        authed,
        result.model.stats.total_sinks,
        pii_sinks,
        result.model.stats.total_symbols,
        enriched,
        result.model.stats.total_imports,
        result.timing.total_ms,
    );
}

// ═══════════════════════════════════════════════════════════════════
//  TypeScript / JavaScript (JavaScriptLike)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn typescript_express_realworld() {
    let name = "express-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/node-express-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 5);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);
    assert_has_sinks(&result, name, 1);
    assert_has_imports(&result, name, 5);

    // Diagnostic: check auth presence
    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn typescript_nestjs_starter() {
    let name = "nestjs-starter";
    let (_tmp, path) = clone_repo(
        "https://github.com/nestjs/typescript-starter",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    // NestJS starter is minimal — just validate parsing works
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);
    assert_has_imports(&result, name, 5);
}

// ═══════════════════════════════════════════════════════════════════
//  Python
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn python_fastapi_fullstack() {
    let name = "fastapi-fullstack";
    let (_tmp, path) = clone_repo(
        "https://github.com/fastapi/full-stack-fastapi-template",
        Some("backend"),
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);
    assert_has_sinks(&result, name, 1);

    // Python extractor does not currently extract imports
    let imports = result.model.stats.total_imports;
    eprintln!("  [{name}] imports: {imports} (Python import extraction not yet implemented)");

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn python_flask_realworld() {
    let name = "flask-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/flask-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);

    // Flask-realworld may not have detectable log sinks — diagnostic only
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn python_django_styleguide() {
    let name = "django-styleguide";
    let (_tmp, path) = clone_repo(
        "https://github.com/HackSoftware/Django-Styleguide-Example",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);

    // Django routes are in urls.py — may or may not match our patterns
    let routes = result.model.stats.total_interfaces;
    eprintln!("  [{name}] routes found: {routes}");
}

// ═══════════════════════════════════════════════════════════════════
//  Java (Spring Boot)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn java_spring_petclinic() {
    let name = "spring-petclinic";
    let (_tmp, path) = clone_repo(
        "https://github.com/spring-projects/spring-petclinic",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 5);
    assert_has_symbols(&result, name, 10);
    assert_has_enriched_symbols(&result, name);
    assert_has_sinks(&result, name, 1);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn java_spring_realworld() {
    let name = "spring-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/spring-boot-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 5);
    assert_has_symbols(&result, name, 10);
    assert_has_enriched_symbols(&result, name);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

// ═══════════════════════════════════════════════════════════════════
//  C# (ASP.NET Core)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn csharp_aspnet_realworld() {
    let name = "aspnet-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/aspnetcore-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 5);
    assert_has_enriched_symbols(&result, name);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

// ═══════════════════════════════════════════════════════════════════
//  Go (Gin / Echo)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn go_gin_examples() {
    let name = "gin-examples";
    let (_tmp, path) = clone_repo(
        "https://github.com/gin-gonic/examples",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 5);
    assert_has_enriched_symbols(&result, name);
    assert_has_sinks(&result, name, 1);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn go_echo_realworld() {
    let name = "echo-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/xesina/golang-echo-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 5);
    assert_has_enriched_symbols(&result, name);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

// ═══════════════════════════════════════════════════════════════════
//  PHP (Laravel)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn php_laravel_realworld() {
    let name = "laravel-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/laravel-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);
    assert_has_sinks(&result, name, 1);

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

// ═══════════════════════════════════════════════════════════════════
//  Ruby (Rails)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn ruby_rails_realworld() {
    let name = "rails-realworld";
    let (_tmp, path) = clone_repo(
        "https://github.com/gothinkster/rails-realworld-example-app",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_routes(&result, name, 3);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);

    // Rails-realworld may not have detectable log sinks (Rails.logger pattern)
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");

    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");
}

#[test]
#[ignore]
fn ruby_administrate() {
    let name = "administrate";
    let (_tmp, path) = clone_repo(
        "https://github.com/thoughtbot/administrate",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_has_symbols(&result, name, 3);
    assert_has_enriched_symbols(&result, name);

    // Rails engine — check what we find
    let routes = result.model.stats.total_interfaces;
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] routes: {routes}, sinks: {sinks}");
}

// ═══════════════════════════════════════════════════════════════════
//  Rust (generic fallback + symbols)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn rust_miniserve() {
    let name = "miniserve";
    let (_tmp, path) = clone_repo(
        "https://github.com/svenstaro/miniserve",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    // Rust uses generic fallback — no route extraction
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Rust should have 0 routes (generic fallback)"
    );
    assert_has_symbols(&result, name, 10);
    assert_has_enriched_symbols(&result, name);

    // Rust projects use tracing macros (info!(), warn!()) which are macro invocations,
    // not object.method() calls. The generic fallback text-matcher may not detect them.
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks} (generic fallback — tracing macros may not match)");
}

// ═══════════════════════════════════════════════════════════════════
//  Kotlin (JvmLike → java extractor)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn kotlin_spring_boot() {
    let name = "kotlin-spring";
    let (_tmp, path) = clone_repo(
        "https://github.com/spring-guides/tut-spring-boot-kotlin",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    // Kotlin Spring should extract routes via java extractor
    let routes = result.model.stats.total_interfaces;
    eprintln!("  [{name}] routes found: {routes}");
    assert!(
        routes >= 1,
        "[{name}] expected >= 1 route from Kotlin Spring, got {routes}"
    );

    // Kotlin has no symbol queries — expect 0
    let symbols = result.model.stats.total_symbols;
    eprintln!("  [{name}] symbols: {symbols} (no query for Kotlin)");

    // Check auth
    let authed = result.model.components[0]
        .interfaces
        .iter()
        .filter(|r| r.auth.is_some())
        .count();
    eprintln!("  [{name}] authenticated routes: {authed}");

    // Check sinks
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");
}

// ═══════════════════════════════════════════════════════════════════
//  Swift (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn swift_log() {
    let name = "swift-log";
    let (_tmp, path) = clone_repo(
        "https://github.com/apple/swift-log",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    // No routes for Swift (generic fallback)
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Swift should have 0 routes"
    );
    // No symbol queries for Swift
    let symbols = result.model.stats.total_symbols;
    eprintln!("  [{name}] symbols: {symbols} (no query for Swift)");

    // swift-log defines logging abstractions — actual log.info() patterns
    // may or may not be detected by the generic text-matcher.
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks} (generic fallback)");
}

// ═══════════════════════════════════════════════════════════════════
//  C (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn c_cjson() {
    let name = "cJSON";
    let (_tmp, path) = clone_repo(
        "https://github.com/DaveGamble/cJSON",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] C should have 0 routes"
    );

    // No symbol queries for C
    let symbols = result.model.stats.total_symbols;
    eprintln!("  [{name}] symbols: {symbols} (no query for C)");

    // cJSON likely has printf/fprintf calls
    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");
}

// ═══════════════════════════════════════════════════════════════════
//  C++ (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn cpp_leveldb() {
    let name = "leveldb";
    let (_tmp, path) = clone_repo(
        "https://github.com/google/leveldb",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] C++ should have 0 routes"
    );

    // No symbol queries for C++
    let symbols = result.model.stats.total_symbols;
    eprintln!("  [{name}] symbols: {symbols} (no query for C++)");

    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");
}

// ═══════════════════════════════════════════════════════════════════
//  Scala (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn scala_play_seed() {
    let name = "play-scala-seed";
    let (_tmp, path) = clone_repo(
        "https://github.com/playframework/play-scala-seed.g8",
        None,
    );

    let result = analyze_repo(&path, 120);
    print_report(&result, name);

    assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Scala should have 0 routes"
    );

    // No symbol queries for Scala
    let symbols = result.model.stats.total_symbols;
    eprintln!("  [{name}] symbols: {symbols} (no query for Scala)");

    let sinks = result.model.stats.total_sinks;
    eprintln!("  [{name}] sinks: {sinks}");
}

// ═══════════════════════════════════════════════════════════════════
//  Summary: one project per language, formatted table
// ═══════════════════════════════════════════════════════════════════

/// Representative project per language for the summary table.
struct ProjectSpec {
    name: &'static str,
    url: &'static str,
    subdir: Option<&'static str>,
    language: &'static str,
}

const SUMMARY_PROJECTS: &[ProjectSpec] = &[
    ProjectSpec {
        name: "express-realworld",
        url: "https://github.com/gothinkster/node-express-realworld-example-app",
        subdir: None,
        language: "TypeScript",
    },
    ProjectSpec {
        name: "fastapi-fullstack",
        url: "https://github.com/fastapi/full-stack-fastapi-template",
        subdir: Some("backend"),
        language: "Python",
    },
    ProjectSpec {
        name: "spring-petclinic",
        url: "https://github.com/spring-projects/spring-petclinic",
        subdir: None,
        language: "Java",
    },
    ProjectSpec {
        name: "aspnet-realworld",
        url: "https://github.com/gothinkster/aspnetcore-realworld-example-app",
        subdir: None,
        language: "C#",
    },
    ProjectSpec {
        name: "gin-examples",
        url: "https://github.com/gin-gonic/examples",
        subdir: None,
        language: "Go",
    },
    ProjectSpec {
        name: "laravel-realworld",
        url: "https://github.com/gothinkster/laravel-realworld-example-app",
        subdir: None,
        language: "PHP",
    },
    ProjectSpec {
        name: "rails-realworld",
        url: "https://github.com/gothinkster/rails-realworld-example-app",
        subdir: None,
        language: "Ruby",
    },
    ProjectSpec {
        name: "miniserve",
        url: "https://github.com/svenstaro/miniserve",
        subdir: None,
        language: "Rust",
    },
    ProjectSpec {
        name: "kotlin-spring",
        url: "https://github.com/spring-guides/tut-spring-boot-kotlin",
        subdir: None,
        language: "Kotlin",
    },
    ProjectSpec {
        name: "swift-log",
        url: "https://github.com/apple/swift-log",
        subdir: None,
        language: "Swift",
    },
    ProjectSpec {
        name: "cJSON",
        url: "https://github.com/DaveGamble/cJSON",
        subdir: None,
        language: "C",
    },
    ProjectSpec {
        name: "leveldb",
        url: "https://github.com/google/leveldb",
        subdir: None,
        language: "C++",
    },
    ProjectSpec {
        name: "play-scala-seed",
        url: "https://github.com/playframework/play-scala-seed.g8",
        subdir: None,
        language: "Scala",
    },
];

#[test]
#[ignore]
fn summary_all_languages() {
    eprintln!();
    eprintln!(
        "{:<24} {:<14} {:>5} {:>7} {:>6} {:>6} {:>8} {:>8}",
        "Project", "Language", "Files", "Routes", "Auth", "Sinks", "Symbols", "TimeMs"
    );
    eprintln!("{}", "─".repeat(90));

    let mut all_ok = true;

    for spec in SUMMARY_PROJECTS {
        let result = std::panic::catch_unwind(|| {
            let (_tmp, path) = clone_repo(spec.url, spec.subdir);
            analyze_repo(&path, 120)
        });

        match result {
            Ok(analysis) => {
                let comp = &analysis.model.components[0];
                let authed = comp.interfaces.iter().filter(|r| r.auth.is_some()).count();

                eprintln!(
                    "{:<24} {:<14} {:>5} {:>7} {:>6} {:>6} {:>8} {:>8}",
                    spec.name,
                    spec.language,
                    analysis.files_analyzed,
                    analysis.model.stats.total_interfaces,
                    authed,
                    analysis.model.stats.total_sinks,
                    analysis.model.stats.total_symbols,
                    analysis.timing.total_ms,
                );
            }
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.as_str()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s
                } else {
                    "unknown error"
                };
                eprintln!(
                    "{:<24} {:<14} FAILED: {}",
                    spec.name, spec.language, msg
                );
                all_ok = false;
            }
        }
    }

    eprintln!("{}", "─".repeat(90));

    assert!(all_ok, "one or more projects failed analysis");
}
