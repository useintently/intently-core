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

#[allow(dead_code)]
mod common;

use intently_core::parser::SupportedLanguage;
use intently_core::search::{SearchPattern, StructuralSearch};
use intently_core::WorkspaceKind;

// ═══════════════════════════════════════════════════════════════════
//  TypeScript / JavaScript (JavaScriptLike)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn typescript_express_realworld() {
    let name = "express-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/node-express-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 5);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_sinks(&result, name, 1);
    common::assert_has_imports(&result, name, 5);
    common::assert_has_references(&result, name, 1);
    // Note: Express uses CommonJS require() — resolver produces 0 resolved refs (confidence = 0)
    common::assert_has_module_boundaries(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);

    // Graph analysis on real TypeScript project
    common::run_graph_analysis_on(&path, name);
}

#[test]
#[ignore]
fn typescript_nestjs_starter() {
    let name = "nestjs-starter";
    let (_tmp, path) = common::clone_repo("https://github.com/nestjs/typescript-starter", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // NestJS starter is minimal — just validate parsing works
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_imports(&result, name, 5);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Python
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn python_fastapi_fullstack() {
    let name = "fastapi-fullstack";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/fastapi/full-stack-fastapi-template",
        Some("backend"),
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_sinks(&result, name, 1);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);

    // Graph analysis on real Python project
    common::run_graph_analysis_on(&path, name);
}

#[test]
#[ignore]
fn python_flask_realworld() {
    let name = "flask-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/flask-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

#[test]
#[ignore]
fn python_django_styleguide() {
    let name = "django-styleguide";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/HackSoftware/Django-Styleguide-Example",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Java (Spring Boot)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn java_spring_petclinic() {
    let name = "spring-petclinic";
    let (_tmp, path) =
        common::clone_repo("https://github.com/spring-projects/spring-petclinic", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 5);
    common::assert_has_symbols(&result, name, 10);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_sinks(&result, name, 1);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_data_models(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);

    // Graph analysis on real Java project
    common::run_graph_analysis_on(&path, name);
}

#[test]
#[ignore]
fn java_spring_realworld() {
    let name = "spring-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/spring-boot-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 5);
    common::assert_has_symbols(&result, name, 10);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  C# (ASP.NET Core)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn csharp_aspnet_realworld() {
    let name = "aspnet-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/aspnetcore-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 5);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);

    // Graph analysis
    common::run_graph_analysis_on(&path, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Go (Gin / Echo)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn go_gin_examples() {
    let name = "gin-examples";
    let (_tmp, path) = common::clone_repo("https://github.com/gin-gonic/examples", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 5);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_sinks(&result, name, 1);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn go_echo_realworld() {
    let name = "echo-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/xesina/golang-echo-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 5);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  PHP (Laravel)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn php_laravel_realworld() {
    let name = "laravel-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/laravel-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_sinks(&result, name, 1);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Ruby (Rails)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn ruby_rails_realworld() {
    let name = "rails-realworld";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/rails-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_routes(&result, name, 3);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_resolved_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn ruby_administrate() {
    let name = "administrate";
    let (_tmp, path) = common::clone_repo("https://github.com/thoughtbot/administrate", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 3);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Rust (generic fallback + symbols)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn rust_miniserve() {
    let name = "miniserve";
    let (_tmp, path) = common::clone_repo("https://github.com/svenstaro/miniserve", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Rust uses generic fallback — no route extraction
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Rust should have 0 routes (generic fallback)"
    );
    common::assert_has_symbols(&result, name, 10);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_has_references(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Kotlin (JvmLike → java extractor)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn kotlin_spring_boot() {
    let name = "kotlin-spring";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/spring-guides/tut-spring-boot-kotlin",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Kotlin Spring should extract routes via java extractor
    common::assert_has_routes(&result, name, 1);
    common::assert_has_graph_stats(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Swift (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn swift_log() {
    let name = "swift-log";
    let (_tmp, path) = common::clone_repo("https://github.com/apple/swift-log", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // No routes for Swift (generic fallback)
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Swift should have 0 routes"
    );
    common::assert_has_graph_stats(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  C (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn c_cjson() {
    let name = "cJSON";
    let (_tmp, path) = common::clone_repo("https://github.com/DaveGamble/cJSON", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] C should have 0 routes"
    );
    common::assert_has_graph_stats(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  C++ (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn cpp_leveldb() {
    let name = "leveldb";
    let (_tmp, path) = common::clone_repo("https://github.com/google/leveldb", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] C++ should have 0 routes"
    );
    common::assert_has_graph_stats(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Scala (generic fallback)
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn scala_play_seed() {
    let name = "play-scala-seed";
    let (_tmp, path) =
        common::clone_repo("https://github.com/playframework/play-scala-seed.g8", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    assert_eq!(
        result.model.stats.total_interfaces, 0,
        "[{name}] Scala should have 0 routes"
    );
    common::assert_has_graph_stats(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Workspace / Monorepo Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn workspace_cargo_tokio_console() {
    let name = "tokio-console";
    let (_tmp, path) = common::clone_repo("https://github.com/tokio-rs/console", None);

    let (engine, result) = common::analyze_repo_with_engine(&path, 180);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);

    // Workspace detection
    let layout = engine
        .workspace_layout()
        .expect("[tokio-console] expected workspace_layout to be Some");
    assert_eq!(
        layout.kind,
        WorkspaceKind::Cargo,
        "[{name}] expected Cargo workspace"
    );
    assert!(
        layout.packages.len() >= 2,
        "[{name}] expected >= 2 workspace packages, got {}",
        layout.packages.len()
    );

    // Multi-component model
    assert!(
        result.model.components.len() >= 2,
        "[{name}] expected >= 2 components, got {}",
        result.model.components.len()
    );

    common::assert_has_symbols(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_content_hashes_present(&engine, name);

    eprintln!(
        "  [{name}] workspace: kind={:?} packages={} components={}",
        layout.kind,
        layout.packages.len(),
        result.model.components.len(),
    );
}

#[test]
#[ignore]
fn workspace_pnpm_drizzle_orm() {
    let name = "drizzle-orm";
    let (_tmp, path) = common::clone_repo("https://github.com/drizzle-team/drizzle-orm", None);

    let (engine, result) = common::analyze_repo_with_engine(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);

    // Workspace detection
    let layout = engine
        .workspace_layout()
        .expect("[drizzle-orm] expected workspace_layout to be Some");
    assert_eq!(
        layout.kind,
        WorkspaceKind::Pnpm,
        "[{name}] expected Pnpm workspace"
    );
    assert!(
        layout.packages.len() >= 3,
        "[{name}] expected >= 3 workspace packages, got {}",
        layout.packages.len()
    );

    // Multi-component model
    assert!(
        result.model.components.len() >= 2,
        "[{name}] expected >= 2 components, got {}",
        result.model.components.len()
    );

    common::assert_has_symbols(&result, name, 50);
    common::assert_has_imports(&result, name, 20);
    common::assert_has_graph_stats(&result, name);
    common::assert_content_hashes_present(&engine, name);

    eprintln!(
        "  [{name}] workspace: kind={:?} packages={} components={}",
        layout.kind,
        layout.packages.len(),
        result.model.components.len(),
    );
}

#[test]
#[ignore]
fn workspace_npm_cal_com() {
    let name = "cal.com";
    let (_tmp, path) = common::clone_repo("https://github.com/calcom/cal.com", None);

    let (engine, result) = common::analyze_repo_with_engine(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_workspace_layout(&engine, name, WorkspaceKind::Npm, 5);

    // Multi-component model
    assert!(
        result.model.components.len() >= 5,
        "[{name}] expected >= 5 components, got {}",
        result.model.components.len()
    );

    common::assert_has_symbols(&result, name, 50);
    common::assert_has_imports(&result, name, 100);
    common::assert_has_routes(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_content_hashes_present(&engine, name);
    common::assert_has_enriched_symbols(&result, name);

    // Phase 3: enriched routes on a large TypeScript monorepo
    common::assert_has_enriched_routes(&result, name, 5);

    eprintln!(
        "  [{name}] workspace: kind=Npm packages={} components={}",
        engine.workspace_layout().map_or(0, |l| l.packages.len()),
        result.model.components.len(),
    );
}

// NOTE: Go workspace (go.work) and uv workspace ([tool.uv.workspace]) tests are omitted.
//
// go.work is a LOCAL development tool — virtually never committed to public repos.
// Checked 10+ major Go projects (terraform, gitea, pulumi, cosmos-sdk, etc.): none had go.work.
// Go workspace detection is validated via controlled fixture (tests/fixtures/go_workspace/).
//
// uv workspaces are too new — no reliable public repo with [tool.uv.workspace] found.
// uv workspace detection is validated via controlled fixture (tests/fixtures/uv_workspace/).

// ═══════════════════════════════════════════════════════════════════
//  Medium-to-Large Repository Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn python_saleor_large() {
    let name = "saleor";
    let (_tmp, path) = common::clone_repo("https://github.com/saleor/saleor", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 500);
    common::assert_has_imports(&result, name, 200);
    common::assert_has_sinks(&result, name, 10);
    common::assert_has_references(&result, name, 100);
    common::assert_has_resolved_references(&result, name, 50);
    common::assert_has_module_boundaries(&result, name, 5);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn java_design_patterns_large() {
    let name = "java-design-patterns";
    let (_tmp, path) = common::clone_repo("https://github.com/iluwatar/java-design-patterns", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 200);
    common::assert_has_data_models(&result, name, 10);
    common::assert_has_references(&result, name, 50);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_anchors_valid(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Diverse Framework Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn typescript_nextjs_cal_platform() {
    // Next.js + TypeScript monorepo — tests extraction on a React/Next.js codebase.
    // cal.com platform is a large Next.js app with API routes, tRPC, and React components.
    let name = "nextjs-cal-platform";
    let (_tmp, path) = common::clone_repo("https://github.com/calcom/cal.com", Some("apps/web"));

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 50);
    common::assert_has_imports(&result, name, 100);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn typescript_nextjs_t3_app() {
    // create-t3-app — Next.js + tRPC + Prisma + Tailwind starter.
    let name = "create-t3-app";
    let (_tmp, path) = common::clone_repo("https://github.com/t3-oss/create-t3-app", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 5);
    common::assert_has_imports(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

#[test]
#[ignore]
fn typescript_vite_core() {
    // Vite — build tool with extensive TypeScript codebase.
    let name = "vite";
    let (_tmp, path) = common::clone_repo("https://github.com/vitejs/vite", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 100);
    common::assert_has_imports(&result, name, 100);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn typescript_remix_indie_stack() {
    // Remix — full-stack React framework with loader/action patterns.
    let name = "remix-indie-stack";
    let (_tmp, path) = common::clone_repo("https://github.com/remix-run/indie-stack", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 5);
    common::assert_has_imports(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_anchors_valid(&result, name);
}

#[test]
#[ignore]
fn go_gitea_large() {
    // Gitea — large Go project (self-hosted Git service).
    // Tests Go extraction at scale without workspace (single go.mod).
    let name = "gitea";
    let (_tmp, path) = common::clone_repo("https://github.com/go-gitea/gitea", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 500);
    common::assert_has_sinks(&result, name, 50);
    common::assert_has_references(&result, name, 100);
    common::assert_has_resolved_references(&result, name, 50);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn python_django_large() {
    // Django framework itself — large Python project with complex imports.
    let name = "django";
    let (_tmp, path) = common::clone_repo("https://github.com/django/django", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 1000);
    common::assert_has_imports(&result, name, 500);
    common::assert_has_sinks(&result, name, 10);
    common::assert_has_references(&result, name, 500);
    common::assert_has_resolved_references(&result, name, 200);
    common::assert_has_module_boundaries(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_anchors_valid(&result, name);
    common::assert_stats_populated(&result, name);

    // Phase 3: Django should have env deps and test symbols
    common::assert_has_env_dependencies(&result, name, 5);
    common::assert_has_test_symbols(&result, name, 50);

    // Import resolution distribution
    common::assert_resolution_distribution_has(&result, name, "import_based");
    common::assert_resolution_distribution_has(&result, name, "external");
}

#[test]
#[ignore]
fn csharp_aspnet_ecommerce() {
    // eShopOnWeb — Microsoft's reference ASP.NET Core architecture.
    let name = "eshop-on-web";
    let (_tmp, path) =
        common::clone_repo("https://github.com/dotnet-architecture/eShopOnWeb", None);

    let result = common::analyze_repo(&path, 180);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 20);
    common::assert_has_data_models(&result, name, 5);
    common::assert_has_references(&result, name, 10);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_anchors_valid(&result, name);
}

#[test]
#[ignore]
fn php_laravel_large() {
    // Laravel framework itself — large PHP project.
    let name = "laravel-framework";
    let (_tmp, path) = common::clone_repo("https://github.com/laravel/framework", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 200);
    common::assert_has_references(&result, name, 50);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_stats_populated(&result, name);
}

#[test]
#[ignore]
fn ruby_rails_large() {
    // Rails framework itself — large Ruby project.
    let name = "rails-framework";
    let (_tmp, path) = common::clone_repo("https://github.com/rails/rails", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 500);
    common::assert_has_references(&result, name, 100);
    common::assert_has_graph_stats(&result, name);
    common::assert_has_enriched_symbols(&result, name);
    common::assert_stats_populated(&result, name);
}

// ═══════════════════════════════════════════════════════════════════
//  Python Import Stress Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn python_rich_import_resolution() {
    let name = "rich";
    let (_tmp, path) = common::clone_repo("https://github.com/Textualize/rich", None);

    let result = common::analyze_repo(&path, 180);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 100);
    common::assert_has_imports(&result, name, 200);

    // Rich uses heavy relative imports (.console, .style, etc.)
    common::assert_has_import_based_references(&result, name, 50);
    // stdlib (typing, os, io, etc.) + third-party classified as External
    common::assert_has_external_references(&result, name, 10);
    common::assert_has_resolved_references(&result, name, 50);

    // Resolution method distribution should include both categories
    common::assert_resolution_distribution_has(&result, name, "import_based");
    common::assert_resolution_distribution_has(&result, name, "external");

    let dist = &result.model.stats.resolution_method_distribution;
    eprintln!("  [{name}] resolution distribution: {dist:?}");
}

#[test]
#[ignore]
fn python_httpie_import_resolution() {
    let name = "httpie";
    let (_tmp, path) = common::clone_repo("https://github.com/httpie/cli", None);

    let result = common::analyze_repo(&path, 180);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    common::assert_has_symbols(&result, name, 50);
    common::assert_has_imports(&result, name, 50);

    // httpie has moderate internal imports
    common::assert_has_import_based_references(&result, name, 20);
    // stdlib + third-party
    common::assert_has_external_references(&result, name, 5);
    common::assert_resolution_distribution_has(&result, name, "import_based");
    common::assert_resolution_distribution_has(&result, name, "external");

    let dist = &result.model.stats.resolution_method_distribution;
    eprintln!("  [{name}] resolution distribution: {dist:?}");
}

// ═══════════════════════════════════════════════════════════════════
//  Phase 3 Feature Validation
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn phase3_env_dependencies_python() {
    let name = "phase3-env-django";
    // Django framework uses os.environ[] and os.getenv() directly for config.
    // NOTE: FastAPI template was tested first but uses pydantic-settings (BaseSettings
    // abstraction) instead of raw stdlib calls — our extractor doesn't detect that pattern.
    let (_tmp, path) = common::clone_repo("https://github.com/django/django", None);

    let result = common::analyze_repo(&path, 300);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Django uses os.environ[] and os.getenv() extensively for config
    common::assert_has_env_dependencies(&result, name, 5);

    let env_deps: Vec<&str> = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.env_dependencies)
        .map(|e| e.var_name.as_str())
        .collect();
    eprintln!(
        "  [{name}] env_dependencies ({} total): {:?}",
        env_deps.len(),
        &env_deps[..env_deps.len().min(20)]
    );
}

#[test]
#[ignore]
fn phase3_test_symbols_java() {
    let name = "phase3-test-petclinic";
    let (_tmp, path) =
        common::clone_repo("https://github.com/spring-projects/spring-petclinic", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Spring PetClinic has JUnit @Test annotated methods
    common::assert_has_test_symbols(&result, name, 1);

    let test_symbols: Vec<&str> = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.symbols)
        .filter(|s| s.is_test)
        .map(|s| s.name.as_str())
        .collect();
    eprintln!(
        "  [{name}] test symbols ({} total): {:?}",
        test_symbols.len(),
        &test_symbols[..test_symbols.len().min(20)]
    );
}

#[test]
#[ignore]
fn phase3_enriched_routes_express() {
    let name = "phase3-routes-express";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/node-express-realworld-example-app",
        None,
    );

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Express routes should have handler names and/or path parameters
    common::assert_has_enriched_routes(&result, name, 1);

    let enriched: Vec<String> = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.interfaces)
        .filter(|r| r.handler_name.is_some() || !r.parameters.is_empty())
        .map(|r| {
            format!(
                "{:?} {} (handler={:?}, params={})",
                r.method,
                r.path,
                r.handler_name,
                r.parameters.len()
            )
        })
        .collect();
    eprintln!(
        "  [{name}] enriched routes ({} total): {:?}",
        enriched.len(),
        &enriched[..enriched.len().min(10)]
    );
}

#[test]
#[ignore]
fn phase3_enriched_routes_spring() {
    let name = "phase3-routes-spring";
    let (_tmp, path) =
        common::clone_repo("https://github.com/spring-projects/spring-petclinic", None);

    let result = common::analyze_repo(&path, 120);
    common::print_report(&result, name);

    common::assert_basic_invariants(&result, name);
    // Spring @RequestMapping with handler method names
    common::assert_has_enriched_routes(&result, name, 1);

    let enriched: Vec<String> = result
        .model
        .components
        .iter()
        .flat_map(|c| &c.interfaces)
        .filter(|r| r.handler_name.is_some() || !r.parameters.is_empty())
        .map(|r| {
            format!(
                "{:?} {} (handler={:?}, params={})",
                r.method,
                r.path,
                r.handler_name,
                r.parameters.len()
            )
        })
        .collect();
    eprintln!(
        "  [{name}] enriched routes ({} total): {:?}",
        enriched.len(),
        &enriched[..enriched.len().min(10)]
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Incremental Pipeline & Semantic Diff
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn incremental_pipeline_with_semantic_diff() {
    let name = "incremental-express";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/node-express-realworld-example-app",
        None,
    );

    // Full analysis — first run should have no diff
    let (mut engine, initial_result) = common::analyze_repo_with_engine(&path, 120);
    common::assert_basic_invariants(&initial_result, name);
    assert!(
        initial_result.diff.is_none(),
        "[{name}] first full_analysis() should have diff = None"
    );

    // Find a route file to modify
    let target_file = engine
        .sources()
        .keys()
        .find(|p| {
            let s = p.to_string_lossy();
            s.contains("route") && (s.ends_with(".js") || s.ends_with(".ts"))
        })
        .cloned()
        .expect("[{name}] expected at least one route file in sources");

    // Append a new route to the file on disk
    let original_content = std::fs::read_to_string(&target_file).expect("should read route file");
    let modified_content = format!(
        "{}\nrouter.get('/test-incremental', function(req, res) {{ res.json({{ok: true}}); }});\n",
        original_content
    );
    std::fs::write(&target_file, &modified_content).expect("should write modified route file");

    // Incremental update
    let incremental_result = engine
        .on_file_changed(&target_file)
        .expect("[{name}] on_file_changed() should succeed");

    // Semantic diff should detect the change
    let diff = incremental_result
        .diff
        .as_ref()
        .expect("[{name}] incremental result should have diff = Some");

    let total_changes =
        diff.interface_changes.len() + diff.dependency_changes.len() + diff.sink_changes.len();
    assert!(
        total_changes > 0,
        "[{name}] expected at least 1 semantic change, got 0"
    );

    eprintln!(
        "  [{name}] incremental diff: interfaces={} deps={} sinks={} risk=({:?}/{:?})",
        diff.interface_changes.len(),
        diff.dependency_changes.len(),
        diff.sink_changes.len(),
        diff.risk_summary.security,
        diff.risk_summary.reliability,
    );

    common::assert_content_hashes_present(&engine, name);
}

// ═══════════════════════════════════════════════════════════════════
//  ast-grep Structural Search
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn ast_grep_structural_search_on_real_code() {
    let name = "ast-grep-express";
    let (_tmp, path) = common::clone_repo(
        "https://github.com/gothinkster/node-express-realworld-example-app",
        None,
    );

    let (engine, _result) = common::analyze_repo_with_engine(&path, 120);

    // Search for require() calls — common in Express/Node.js
    let patterns = vec![SearchPattern {
        id: "require-call".into(),
        language: SupportedLanguage::JavaScript,
        pattern_text: "require($MODULE)".into(),
        kind: "call_site".into(),
    }];
    let search = StructuralSearch::from_patterns(&patterns)
        .expect("[{name}] StructuralSearch::from_patterns should succeed");

    let mut total_matches = 0;
    for (file_path, source) in engine.sources() {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "js" || ext == "ts" {
            let matches = search.search_file(source, SupportedLanguage::JavaScript, file_path);
            total_matches += matches.len();
        }
    }

    assert!(
        total_matches >= 1,
        "[{name}] expected >= 1 require() match, got {total_matches}"
    );

    eprintln!("  [{name}] ast-grep: {total_matches} require() matches found");
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
        "{:<24} {:<10} {:>5} {:>6} {:>4} {:>5} {:>6} {:>5} {:>4} {:>5} {:>5} {:>5} {:>7}",
        "Project",
        "Language",
        "Files",
        "Routes",
        "Auth",
        "Sinks",
        "Syms",
        "Impts",
        "Refs",
        "Rslvd",
        "DModl",
        "Graph",
        "TimeMs"
    );
    eprintln!("{}", "─".repeat(120));

    let mut all_ok = true;

    for spec in SUMMARY_PROJECTS {
        let result = std::panic::catch_unwind(|| {
            let (_tmp, path) = common::clone_repo(spec.url, spec.subdir);
            common::analyze_repo(&path, 120)
        });

        match result {
            Ok(analysis) => {
                let stats = &analysis.model.stats;
                let authed = analysis
                    .model
                    .components
                    .iter()
                    .flat_map(|c| &c.interfaces)
                    .filter(|r| r.auth.is_some())
                    .count();
                let resolved = analysis
                    .model
                    .components
                    .iter()
                    .flat_map(|c| &c.references)
                    .filter(|r| r.confidence > 0.0)
                    .count();
                let graph_nodes = analysis.graph_stats.as_ref().map_or(0, |g| g.total_nodes);

                eprintln!(
                    "{:<24} {:<10} {:>5} {:>6} {:>4} {:>5} {:>6} {:>5} {:>4} {:>5} {:>5} {:>5} {:>7}",
                    spec.name,
                    spec.language,
                    analysis.files_analyzed,
                    stats.total_interfaces,
                    authed,
                    stats.total_sinks,
                    stats.total_symbols,
                    stats.total_imports,
                    stats.total_references,
                    resolved,
                    stats.total_data_models,
                    graph_nodes,
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
                eprintln!("{:<24} {:<10} FAILED: {}", spec.name, spec.language, msg);
                all_ok = false;
            }
        }
    }

    eprintln!("{}", "─".repeat(120));

    assert!(all_ok, "one or more projects failed analysis");
}

// ═══════════════════════════════════════════════════════════════════
//  Summary: Workspace Formats
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn summary_workspace_formats() {
    struct WorkspaceSpec {
        name: &'static str,
        url: &'static str,
    }

    let specs = [
        WorkspaceSpec {
            name: "tokio-console (Cargo)",
            url: "https://github.com/tokio-rs/console",
        },
        WorkspaceSpec {
            name: "drizzle-orm (Pnpm)",
            url: "https://github.com/drizzle-team/drizzle-orm",
        },
        WorkspaceSpec {
            name: "cal.com (Npm)",
            url: "https://github.com/calcom/cal.com",
        },
        // Go (go.work) and uv ([tool.uv.workspace]) omitted — these formats
        // are rarely committed to public repos. See workspace section comments.
    ];

    eprintln!();
    eprintln!(
        "{:<28} {:<8} {:>5} {:>8} {:>6} {:>5} {:>5} {:>7}",
        "Project", "Kind", "Pkgs", "Comps", "Syms", "Impts", "Graph", "TimeMs"
    );
    eprintln!("{}", "─".repeat(90));

    let mut all_ok = true;

    for spec in &specs {
        let result = std::panic::catch_unwind(|| {
            let (_tmp, path) = common::clone_repo(spec.url, None);
            common::analyze_repo_with_engine(&path, 300)
        });

        match result {
            Ok((engine, analysis)) => {
                let layout = engine.workspace_layout();
                let kind = layout.map_or("none".to_string(), |l| format!("{:?}", l.kind));
                let pkgs = layout.map_or(0, |l| l.packages.len());
                let graph_nodes = analysis.graph_stats.as_ref().map_or(0, |g| g.total_nodes);

                eprintln!(
                    "{:<28} {:<8} {:>5} {:>8} {:>6} {:>5} {:>5} {:>7}",
                    spec.name,
                    kind,
                    pkgs,
                    analysis.model.components.len(),
                    analysis.model.stats.total_symbols,
                    analysis.model.stats.total_imports,
                    graph_nodes,
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
                eprintln!("{:<28} FAILED: {}", spec.name, msg);
                all_ok = false;
            }
        }
    }

    eprintln!("{}", "─".repeat(90));

    assert!(all_ok, "one or more workspace projects failed analysis");
}

// ═══════════════════════════════════════════════════════════════════
//  Summary: Phase 3 Feature Coverage
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn summary_phase3_features() {
    struct Phase3Spec {
        name: &'static str,
        url: &'static str,
        subdir: Option<&'static str>,
        language: &'static str,
    }

    let specs = [
        Phase3Spec {
            name: "fastapi-fullstack",
            url: "https://github.com/fastapi/full-stack-fastapi-template",
            subdir: Some("backend"),
            language: "Python",
        },
        Phase3Spec {
            name: "spring-petclinic",
            url: "https://github.com/spring-projects/spring-petclinic",
            subdir: None,
            language: "Java",
        },
        Phase3Spec {
            name: "express-realworld",
            url: "https://github.com/gothinkster/node-express-realworld-example-app",
            subdir: None,
            language: "TypeScript",
        },
        Phase3Spec {
            name: "gin-examples",
            url: "https://github.com/gin-gonic/examples",
            subdir: None,
            language: "Go",
        },
    ];

    eprintln!();
    eprintln!(
        "{:<24} {:<10} {:>7} {:>8} {:>10} {:>7}",
        "Project", "Language", "EnvDep", "TestSym", "EnrichRte", "TimeMs"
    );
    eprintln!("{}", "─".repeat(80));

    let mut all_ok = true;

    for spec in &specs {
        let result = std::panic::catch_unwind(|| {
            let (_tmp, path) = common::clone_repo(spec.url, spec.subdir);
            common::analyze_repo(&path, 180)
        });

        match result {
            Ok(analysis) => {
                let env_deps: usize = analysis
                    .model
                    .components
                    .iter()
                    .flat_map(|c| &c.env_dependencies)
                    .count();
                let test_syms = analysis
                    .model
                    .components
                    .iter()
                    .flat_map(|c| &c.symbols)
                    .filter(|s| s.is_test)
                    .count();
                let enriched_routes = analysis
                    .model
                    .components
                    .iter()
                    .flat_map(|c| &c.interfaces)
                    .filter(|r| r.handler_name.is_some() || !r.parameters.is_empty())
                    .count();

                eprintln!(
                    "{:<24} {:<10} {:>7} {:>8} {:>10} {:>7}",
                    spec.name,
                    spec.language,
                    env_deps,
                    test_syms,
                    enriched_routes,
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
                eprintln!("{:<24} {:<10} FAILED: {}", spec.name, spec.language, msg);
                all_ok = false;
            }
        }
    }

    eprintln!("{}", "─".repeat(80));

    assert!(
        all_ok,
        "one or more Phase 3 validation projects failed analysis"
    );
}
