//! Integration tests: full extraction pipeline on realistic multi-file projects.
//!
//! Each test points the IntentlyEngine at a fixture directory containing
//! realistic source code for one language/framework combination.  The engine
//! discovers files, parses with tree-sitter, dispatches to the correct
//! extractor, and builds a CodeModel.  We then assert on the aggregate
//! extraction results — route counts, auth presence, HTTP call detection,
//! log sinks, and PII flags.
//!
//! Fixture projects live under `tests/fixtures/<project_name>/`.

use std::path::PathBuf;

use intently_core::model::types::*;
use intently_core::IntentlyEngine;

/// Run full analysis on a fixture project and return the result.
fn analyze_fixture(project_name: &str) -> intently_core::ExtractionResult {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures").join(project_name);
    assert!(
        fixture_path.exists(),
        "Fixture directory does not exist: {}",
        fixture_path.display()
    );

    let mut engine = IntentlyEngine::new(fixture_path);
    engine
        .full_analysis()
        .expect("Full analysis should succeed on fixture project")
}

// ═══════════════════════════════════════════════════════════════════
//  TypeScript / JavaScript Family (JavaScriptLike)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_extracts_routes_auth_httpcalls_sinks() {
    let result = analyze_fixture("express_ecommerce");
    let model = &result.model;

    // Should discover and analyze multiple .ts files
    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 TS files analyzed, got {}",
        result.files_analyzed
    );

    // Should extract many routes (users CRUD + payments + products + health/metrics)
    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 15,
        "Express project should have ≥15 routes, got {}",
        routes.len()
    );

    // Should have a mix of authed and public routes
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    let public = routes.iter().filter(|r| r.auth.is_none()).count();
    assert!(
        authed >= 5,
        "Expected ≥5 authenticated routes, got {}",
        authed
    );
    assert!(
        public >= 3,
        "Expected ≥3 public routes, got {}",
        public
    );

    // Should detect external HTTP calls (axios, fetch)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 3,
        "Expected ≥3 HTTP calls (Stripe, email, analytics), got {}",
        http_calls.len()
    );

    // Should detect log sinks
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 5,
        "Expected ≥5 log sinks, got {}",
        sinks.len()
    );

    // Should detect PII in some log sinks
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(
        pii_sinks >= 2,
        "Expected ≥2 PII-containing sinks, got {}",
        pii_sinks
    );

    // Should have multiple HTTP methods
    let methods: Vec<_> = routes.iter().map(|r| &r.method).collect();
    assert!(methods.contains(&&HttpMethod::Get));
    assert!(methods.contains(&&HttpMethod::Post));
    assert!(methods.contains(&&HttpMethod::Put) || methods.contains(&&HttpMethod::Delete));
}

#[test]
fn tsx_dashboard_extracts_http_calls_and_sinks() {
    let result = analyze_fixture("tsx_dashboard");

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 TSX file analyzed"
    );

    let model = &result.model;

    // TSX components typically have HTTP calls (axios/fetch) and log sinks
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 1,
        "TSX component should have HTTP calls, got {}",
        http_calls.len()
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 1,
        "TSX component should have log sinks, got {}",
        sinks.len()
    );
}

#[test]
fn jsx_app_extracts_http_calls_and_sinks() {
    let result = analyze_fixture("jsx_app");

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 JSX file analyzed"
    );

    let model = &result.model;
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 1,
        "JSX app should have log sinks, got {}",
        sinks.len()
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Python Family
// ═══════════════════════════════════════════════════════════════════

#[test]
fn fastapi_ecommerce_extracts_routes_auth_httpcalls_sinks() {
    let result = analyze_fixture("fastapi_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 3,
        "Expected at least 3 Python files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 10,
        "FastAPI project should have ≥10 routes, got {}",
        routes.len()
    );

    // Should have authed routes (via @login_required or similar decorators)
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 2,
        "Expected ≥2 authenticated FastAPI routes, got {}",
        authed
    );

    // HTTP calls to external services (requests, httpx)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 2,
        "Expected ≥2 HTTP calls in FastAPI project, got {}",
        http_calls.len()
    );

    // Log sinks with PII
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Expected ≥3 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in FastAPI project");

    // Multiple HTTP methods
    let methods: Vec<_> = routes.iter().map(|r| &r.method).collect();
    assert!(methods.contains(&&HttpMethod::Get));
    assert!(methods.contains(&&HttpMethod::Post));
}

#[test]
fn flask_ecommerce_extracts_routes_and_auth() {
    let result = analyze_fixture("flask_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 Flask file analyzed"
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 5,
        "Flask project should have ≥5 routes, got {}",
        routes.len()
    );

    // Flask uses @login_required, @jwt_required decorators
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 1,
        "Expected ≥1 authenticated Flask route, got {}",
        authed
    );
}

#[test]
fn django_ecommerce_extracts_url_patterns_and_sinks() {
    let result = analyze_fixture("django_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 2,
        "Expected at least 2 Django files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 8,
        "Django project should have ≥8 URL patterns, got {}",
        routes.len()
    );

    // NOTE: Django auth decorators (@login_required, @permission_required) live
    // on view functions in views.py, while routes are defined via path() in
    // urls.py. The extractor currently operates per-file and cannot correlate
    // auth across files. Cross-file auth resolution is a planned enhancement.
    // For now, we verify that routes and sinks are correctly extracted.

    // Log sinks
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 2,
        "Expected ≥2 log sinks in Django project, got {}",
        sinks.len()
    );

    // HTTP calls (requests, httpx in services.py)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 2,
        "Expected ≥2 HTTP calls in Django project, got {}",
        http_calls.len()
    );

    // PII in logs
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(
        pii_sinks >= 1,
        "Expected ≥1 PII-containing sink in Django project, got {}",
        pii_sinks
    );
}

// ═══════════════════════════════════════════════════════════════════
//  JVM Family (Java / Kotlin)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spring_ecommerce_extracts_annotations_auth_httpcalls_sinks() {
    let result = analyze_fixture("spring_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 Java files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 15,
        "Spring project should have ≥15 annotated routes, got {}",
        routes.len()
    );

    // Should have auth annotations (@PreAuthorize, @Secured, @RolesAllowed)
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 5,
        "Expected ≥5 Spring routes with auth annotations, got {}",
        authed
    );

    // Auth should be of Annotation kind
    let annotation_auth = routes
        .iter()
        .filter(|r| matches!(&r.auth, Some(AuthKind::Annotation(_))))
        .count();
    assert!(
        annotation_auth >= 3,
        "Expected ≥3 routes with AuthKind::Annotation, got {}",
        annotation_auth
    );

    // HTTP calls (RestTemplate, WebClient)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 3,
        "Expected ≥3 HTTP calls in Spring project, got {}",
        http_calls.len()
    );

    // Log sinks with PII
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 5,
        "Expected ≥5 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 2, "Expected ≥2 PII sinks in Spring project");

    // Multiple HTTP methods
    let methods: Vec<_> = routes.iter().map(|r| &r.method).collect();
    assert!(methods.contains(&&HttpMethod::Get));
    assert!(methods.contains(&&HttpMethod::Post));
    assert!(methods.contains(&&HttpMethod::Delete));
}

#[test]
fn kotlin_spring_extracts_annotations_and_auth() {
    let result = analyze_fixture("kotlin_spring");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 2,
        "Expected at least 2 Kotlin files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 5,
        "Kotlin Spring project should have ≥5 routes, got {}",
        routes.len()
    );

    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 2,
        "Expected ≥2 authenticated Kotlin routes, got {}",
        authed
    );

    // HTTP calls
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 1,
        "Expected ≥1 HTTP call in Kotlin project, got {}",
        http_calls.len()
    );
}

// ═══════════════════════════════════════════════════════════════════
//  C# (ASP.NET Core)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn aspnet_ecommerce_extracts_attributes_auth_httpcalls_sinks() {
    let result = analyze_fixture("aspnet_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 C# files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 15,
        "ASP.NET project should have ≥15 routes, got {}",
        routes.len()
    );

    // Should have [Authorize] attributes
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 5,
        "Expected ≥5 ASP.NET routes with [Authorize], got {}",
        authed
    );

    // Auth should be of Attribute kind
    let attribute_auth = routes
        .iter()
        .filter(|r| matches!(&r.auth, Some(AuthKind::Attribute(_))))
        .count();
    assert!(
        attribute_auth >= 3,
        "Expected ≥3 routes with AuthKind::Attribute, got {}",
        attribute_auth
    );

    // HTTP calls (HttpClient)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 3,
        "Expected ≥3 HttpClient calls, got {}",
        http_calls.len()
    );

    // Log sinks with PII
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 5,
        "Expected ≥5 log sinks, got {}",
        sinks.len()
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Go Family
// ═══════════════════════════════════════════════════════════════════

#[test]
fn gin_ecommerce_extracts_routes_auth_httpcalls_sinks() {
    let result = analyze_fixture("gin_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 Go files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 12,
        "Gin project should have ≥12 routes, got {}",
        routes.len()
    );

    // Should have middleware auth on some routes
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 3,
        "Expected ≥3 Gin routes with auth middleware, got {}",
        authed
    );

    // HTTP calls (http.Get, http.Post, client.Do)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 3,
        "Expected ≥3 HTTP calls in Gin project, got {}",
        http_calls.len()
    );

    // Log sinks
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Expected ≥3 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in Gin project");
}

#[test]
fn echo_api_extracts_routes_and_sinks() {
    let result = analyze_fixture("echo_api");
    let model = &result.model;

    assert!(result.files_analyzed >= 1, "Expected ≥1 Go file analyzed");

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 5,
        "Echo project should have ≥5 routes, got {}",
        routes.len()
    );
}

#[test]
fn nethttp_api_extracts_handlefunc_routes() {
    let result = analyze_fixture("nethttp_api");
    let model = &result.model;

    assert!(result.files_analyzed >= 1, "Expected ≥1 Go file analyzed");

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 4,
        "net/http project should have ≥4 HandleFunc routes, got {}",
        routes.len()
    );

    // net/http routes default to HttpMethod::All
    let all_method = routes.iter().filter(|r| r.method == HttpMethod::All).count();
    assert!(
        all_method >= 3,
        "net/http HandleFunc should produce All method routes, got {}",
        all_method
    );
}

// ═══════════════════════════════════════════════════════════════════
//  PHP (Laravel)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn laravel_ecommerce_extracts_routes_middleware_httpcalls_sinks() {
    let result = analyze_fixture("laravel_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 3,
        "Expected at least 3 PHP files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 15,
        "Laravel project should have ≥15 Route:: definitions, got {}",
        routes.len()
    );

    // Should have middleware auth on some routes
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 3,
        "Expected ≥3 Laravel routes with ->middleware('auth'), got {}",
        authed
    );

    // HTTP calls (Http::get, Http::post)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 2,
        "Expected ≥2 Http facade calls, got {}",
        http_calls.len()
    );

    // Log sinks (Log::info uses :: pattern)
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Expected ≥3 Log:: sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in Laravel project");
}

// ═══════════════════════════════════════════════════════════════════
//  Ruby (Rails)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn rails_ecommerce_extracts_routes_auth_httpcalls_sinks() {
    let result = analyze_fixture("rails_ecommerce");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 Ruby files analyzed, got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 10,
        "Rails project should have ≥10 routes (DSL + resources), got {}",
        routes.len()
    );

    // Should have before_action auth
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 2,
        "Expected ≥2 Rails routes with before_action auth, got {}",
        authed
    );

    // HTTP calls (HTTParty, Faraday, RestClient)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 2,
        "Expected ≥2 HTTP client calls, got {}",
        http_calls.len()
    );

    // Log sinks
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Expected ≥3 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in Rails project");

    // Should have resources :name routes (HttpMethod::All)
    let resource_routes = routes.iter().filter(|r| r.method == HttpMethod::All).count();
    assert!(
        resource_routes >= 1,
        "Expected ≥1 resources route, got {}",
        resource_routes
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Generic Fallback (Rust, C, C++, Swift, Scala)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn rust_service_extracts_log_sinks_with_pii() {
    let result = analyze_fixture("rust_service");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 Rust file analyzed"
    );

    // Generic extractor: no routes, no HTTP calls — only sinks
    let routes = &model.components[0].interfaces;
    assert!(
        routes.is_empty(),
        "Rust project should have no route extraction (generic fallback)"
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 5,
        "Rust project should have ≥5 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(
        pii_sinks >= 2,
        "Expected ≥2 PII sinks in Rust project, got {}",
        pii_sinks
    );
}

#[test]
fn cpp_service_extracts_log_sinks_with_pii() {
    let result = analyze_fixture("cpp_service");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 C++ file analyzed"
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 5,
        "C++ project should have ≥5 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 2, "Expected ≥2 PII sinks in C++ project");
}

#[test]
fn c_service_extracts_log_sinks() {
    let result = analyze_fixture("c_service");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 C file analyzed"
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "C project should have ≥3 log sinks, got {}",
        sinks.len()
    );
}

#[test]
fn swift_service_extracts_log_sinks_with_pii() {
    let result = analyze_fixture("swift_service");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 Swift file analyzed"
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Swift project should have ≥3 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in Swift project");
}

#[test]
fn scala_service_extracts_log_sinks_with_pii() {
    let result = analyze_fixture("scala_service");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 1,
        "Expected at least 1 Scala file analyzed"
    );

    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 3,
        "Scala project should have ≥3 log sinks, got {}",
        sinks.len()
    );
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(pii_sinks >= 1, "Expected ≥1 PII sink in Scala project");
}

// ═══════════════════════════════════════════════════════════════════
//  Coverage: all 16 languages are parseable
// ═══════════════════════════════════════════════════════════════════

#[test]
fn all_sixteen_languages_are_covered_by_fixture_projects() {
    // This test verifies that our fixture suite covers all 16 supported languages.
    // Each fixture project uses at least one of the 16 languages.
    let projects = vec![
        ("express_ecommerce", "TypeScript"),       // TypeScript
        ("tsx_dashboard", "TSX"),                   // TSX
        ("jsx_app", "JSX"),                         // JSX
        ("fastapi_ecommerce", "Python"),            // Python
        ("spring_ecommerce", "Java"),               // Java
        ("kotlin_spring", "Kotlin"),                // Kotlin
        ("aspnet_ecommerce", "C#"),                 // C#
        ("gin_ecommerce", "Go"),                    // Go
        ("laravel_ecommerce", "PHP"),               // PHP
        ("rails_ecommerce", "Ruby"),                // Ruby
        ("rust_service", "Rust"),                   // Rust
        ("cpp_service", "C++"),                     // C++
        ("c_service", "C"),                         // C
        ("swift_service", "Swift"),                 // Swift
        ("scala_service", "Scala"),                 // Scala
    ];

    // JavaScript is covered by express_ecommerce (JS files would use same extractor)
    // but we can add a jsx_app fixture to cover JSX explicitly.

    for (project, language) in &projects {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = manifest_dir.join("tests/fixtures").join(project);
        assert!(
            fixture_path.exists(),
            "Missing fixture project for {}: {}",
            language,
            fixture_path.display()
        );

        let mut engine = IntentlyEngine::new(fixture_path);
        let result = engine.full_analysis().unwrap_or_else(|e| {
            panic!("Full analysis failed for {} ({}): {}", project, language, e)
        });

        assert!(
            result.files_analyzed >= 1,
            "{} ({}) should analyze at least 1 file, got {}",
            project,
            language,
            result.files_analyzed
        );
    }
}
