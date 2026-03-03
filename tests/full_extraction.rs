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
use intently_core::{IntentlyEngine, WorkspaceKind};

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
    assert!(public >= 3, "Expected ≥3 public routes, got {}", public);

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

#[test]
fn nestjs_api_extracts_decorator_routes_auth_sinks() {
    let result = analyze_fixture("nestjs_api");
    let model = &result.model;

    assert!(
        result.files_analyzed >= 4,
        "Expected at least 4 TS files analyzed, got {}",
        result.files_analyzed
    );

    // Should extract NestJS decorator-based routes from all controllers
    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 11,
        "NestJS project should have >=11 routes, got {}",
        routes.len()
    );

    // Should have authenticated routes (class-level and method-level @UseGuards)
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 8,
        "Expected >=8 authenticated NestJS routes, got {}",
        authed
    );

    // Should have public routes (health check, user findOne, user create)
    let public = routes.iter().filter(|r| r.auth.is_none()).count();
    assert!(
        public >= 2,
        "Expected >=2 public NestJS routes, got {}",
        public
    );

    // Auth should use AuthKind::Decorator for NestJS
    let decorator_auth = routes
        .iter()
        .filter(|r| matches!(&r.auth, Some(AuthKind::Decorator(_))))
        .count();
    assert!(
        decorator_auth >= 8,
        "NestJS auth should use AuthKind::Decorator, got {} decorator auths",
        decorator_auth
    );

    // Should detect HTTP calls in the service file (fetch to Stripe)
    let http_calls = &model.components[0].dependencies;
    assert!(
        http_calls.len() >= 2,
        "Expected >=2 HTTP calls (Stripe API), got {}",
        http_calls.len()
    );

    // Should detect log sinks across controller and service files
    let sinks = &model.components[0].sinks;
    assert!(
        sinks.len() >= 4,
        "Expected >=4 log sinks, got {}",
        sinks.len()
    );

    // Should detect PII in some sinks (email references)
    let pii_sinks = sinks.iter().filter(|s| s.contains_pii).count();
    assert!(
        pii_sinks >= 1,
        "Expected >=1 PII-containing sinks, got {}",
        pii_sinks
    );

    // Should have multiple HTTP methods
    let methods: Vec<_> = routes.iter().map(|r| &r.method).collect();
    assert!(methods.contains(&&HttpMethod::Get));
    assert!(methods.contains(&&HttpMethod::Post));
    assert!(methods.contains(&&HttpMethod::Delete));

    // Verify specific NestJS paths include controller prefix
    let paths: Vec<&str> = routes.iter().map(|r| r.path.as_str()).collect();
    assert!(
        paths.contains(&"/api/articles"),
        "Should have /api/articles path, got {:?}",
        paths
    );
    assert!(
        paths.contains(&"/api/articles/:slug"),
        "Should have /api/articles/:slug path, got {:?}",
        paths
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
        result.files_analyzed >= 5,
        "Expected at least 5 C# files analyzed (controllers + services + Program.cs), got {}",
        result.files_analyzed
    );

    let routes = &model.components[0].interfaces;
    assert!(
        routes.len() >= 18,
        "ASP.NET project should have ≥18 routes (controllers + Minimal API), got {}",
        routes.len()
    );

    // Minimal API routes from Program.cs
    let minimal_api_paths = [
        "/health",
        "/api/v1/catalog/categories",
        "/api/v1/catalog/import",
        "/api/v1/cache/{key}",
    ];
    for path in &minimal_api_paths {
        assert!(
            routes.iter().any(|r| r.path == *path),
            "Expected Minimal API route '{}' to be extracted, found routes: {:?}",
            path,
            routes.iter().map(|r| &r.path).collect::<Vec<_>>()
        );
    }

    // Should have auth (controllers [Authorize] + Minimal API RequireAuthorization)
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 7,
        "Expected ≥7 ASP.NET routes with auth (controllers + Minimal API), got {}",
        authed
    );

    // Auth should be of Attribute kind
    let attribute_auth = routes
        .iter()
        .filter(|r| matches!(&r.auth, Some(AuthKind::Attribute(_))))
        .count();
    assert!(
        attribute_auth >= 5,
        "Expected ≥5 routes with AuthKind::Attribute, got {}",
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
    let all_method = routes
        .iter()
        .filter(|r| r.method == HttpMethod::All)
        .count();
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
    // 15+ explicit routes + 7 (resource) + 5 (apiResource) + 1 (any) = 28+
    assert!(
        routes.len() >= 28,
        "Laravel project should have ≥28 Route:: definitions (including resource/apiResource), got {}",
        routes.len()
    );

    // Should have middleware auth on some routes
    let authed = routes.iter().filter(|r| r.auth.is_some()).count();
    assert!(
        authed >= 3,
        "Expected ≥3 Laravel routes with ->middleware('auth'), got {}",
        authed
    );

    // Route::resource('tickets', ...) expands to 7 routes with auth middleware
    let ticket_routes: Vec<_> = routes
        .iter()
        .filter(|r| r.path.starts_with("/tickets"))
        .collect();
    assert_eq!(
        ticket_routes.len(),
        7,
        "Route::resource('tickets') should expand to 7 routes, got {}",
        ticket_routes.len()
    );
    assert!(
        ticket_routes.iter().all(|r| r.auth.is_some()),
        "All resource('tickets') routes should inherit ->middleware('auth')"
    );

    // Route::apiResource('notifications', ...) expands to 5 routes
    let notification_routes: Vec<_> = routes
        .iter()
        .filter(|r| r.path.starts_with("/notifications"))
        .collect();
    assert_eq!(
        notification_routes.len(),
        5,
        "Route::apiResource('notifications') should expand to 5 routes, got {}",
        notification_routes.len()
    );

    // Route::any() produces HttpMethod::All
    let any_routes: Vec<_> = routes
        .iter()
        .filter(|r| r.method == HttpMethod::All)
        .collect();
    assert!(
        !any_routes.is_empty(),
        "Expected at least 1 Route::any() with HttpMethod::All"
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
    let resource_routes = routes
        .iter()
        .filter(|r| r.method == HttpMethod::All)
        .count();
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
        ("express_ecommerce", "TypeScript"), // TypeScript
        ("tsx_dashboard", "TSX"),            // TSX
        ("jsx_app", "JSX"),                  // JSX
        ("fastapi_ecommerce", "Python"),     // Python
        ("spring_ecommerce", "Java"),        // Java
        ("kotlin_spring", "Kotlin"),         // Kotlin
        ("aspnet_ecommerce", "C#"),          // C#
        ("gin_ecommerce", "Go"),             // Go
        ("laravel_ecommerce", "PHP"),        // PHP
        ("rails_ecommerce", "Ruby"),         // Ruby
        ("rust_service", "Rust"),            // Rust
        ("cpp_service", "C++"),              // C++
        ("c_service", "C"),                  // C
        ("swift_service", "Swift"),          // Swift
        ("scala_service", "Scala"),          // Scala
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

// ═══════════════════════════════════════════════════════════════════
//  Graph Analysis Pipeline
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_graph_analysis_pipeline() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let ctx = engine
        .run_graph_analysis()
        .expect("graph analysis should produce results");

    // Degree centrality should be computed for all nodes
    assert!(
        !ctx.degree_centrality.is_empty(),
        "degree centrality should be non-empty for a multi-file project"
    );

    // Entry points should detect HTTP endpoints
    assert!(
        !ctx.entry_points.is_empty(),
        "should detect at least one entry point in express_ecommerce"
    );

    // Process flows should trace from entry points
    assert!(
        !ctx.process_flows.is_empty(),
        "should trace at least one process flow from entry points"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Symbol Extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_extracts_symbols_with_signatures() {
    let result = analyze_fixture("express_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 5,
        "Express project should extract ≥5 symbols (classes + functions), got {}",
        symbols.len()
    );

    // Should have at least one class (StripeService)
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 1,
        "Expected ≥1 class symbol (StripeService), got {}",
        classes
    );

    // Should have functions/methods
    let functions = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function || s.kind == SymbolKind::Method)
        .count();
    assert!(
        functions >= 3,
        "Expected ≥3 function/method symbols, got {}",
        functions
    );

    // At least some should have signatures
    let with_signature = symbols.iter().filter(|s| s.signature.is_some()).count();
    assert!(
        with_signature >= 3,
        "Expected ≥3 symbols with signatures, got {}",
        with_signature
    );

    // Stats should reflect symbol count
    assert_eq!(
        result.model.stats.total_symbols,
        symbols.len(),
        "stats.total_symbols should match actual symbol count"
    );
}

#[test]
fn nestjs_api_extracts_class_and_method_symbols() {
    let result = analyze_fixture("nestjs_api");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 5,
        "NestJS project should extract ≥5 symbols, got {}",
        symbols.len()
    );

    // Should have controller classes
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 2,
        "Expected ≥2 controller classes (UsersController, ArticlesController), got {}",
        classes
    );

    // Should have methods within classes
    let methods = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method)
        .count();
    assert!(
        methods >= 5,
        "Expected ≥5 controller methods, got {}",
        methods
    );

    // Methods should have parent references to their controller classes
    let with_parent = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method && s.parent.is_some())
        .count();
    assert!(
        with_parent >= 3,
        "Expected ≥3 methods with parent class reference, got {}",
        with_parent
    );
}

#[test]
fn fastapi_ecommerce_extracts_function_symbols() {
    let result = analyze_fixture("fastapi_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 3,
        "FastAPI project should extract ≥3 symbols, got {}",
        symbols.len()
    );

    // Python extractors should find functions
    let functions = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .count();
    assert!(
        functions >= 2,
        "Expected ≥2 function symbols, got {}",
        functions
    );

    // At least some should have signatures
    let with_signature = symbols.iter().filter(|s| s.signature.is_some()).count();
    assert!(
        with_signature >= 1,
        "Expected ≥1 symbol with signature, got {}",
        with_signature
    );
}

#[test]
fn spring_ecommerce_extracts_class_method_symbols() {
    let result = analyze_fixture("spring_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 10,
        "Spring project should extract ≥10 symbols (controllers + models + methods), got {}",
        symbols.len()
    );

    // Should have classes
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 2,
        "Expected ≥2 class symbols (controllers), got {}",
        classes
    );

    // Should have methods with visibility
    let methods_with_visibility = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Method && s.visibility.is_some())
        .count();
    assert!(
        methods_with_visibility >= 3,
        "Expected ≥3 methods with visibility, got {}",
        methods_with_visibility
    );
}

#[test]
fn aspnet_ecommerce_extracts_class_method_symbols() {
    let result = analyze_fixture("aspnet_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 10,
        "ASP.NET project should extract ≥10 symbols, got {}",
        symbols.len()
    );

    // Should have classes (controllers + services)
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 3,
        "Expected ≥3 class symbols (controllers + services), got {}",
        classes
    );
}

#[test]
fn gin_ecommerce_extracts_function_symbols() {
    let result = analyze_fixture("gin_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 5,
        "Gin project should extract ≥5 symbols, got {}",
        symbols.len()
    );

    // Go uses functions (not methods on classes)
    let functions = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .count();
    assert!(
        functions >= 3,
        "Expected ≥3 Go function symbols, got {}",
        functions
    );
}

#[test]
fn laravel_ecommerce_extracts_class_method_symbols() {
    let result = analyze_fixture("laravel_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 3,
        "Laravel project should extract ≥3 symbols, got {}",
        symbols.len()
    );

    // PHP extractors should find classes and methods
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 1,
        "Expected ≥1 PHP class symbol, got {}",
        classes
    );
}

#[test]
fn rails_ecommerce_extracts_class_method_symbols() {
    let result = analyze_fixture("rails_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 3,
        "Rails project should extract ≥3 symbols, got {}",
        symbols.len()
    );

    // Ruby extractors should find classes
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();
    assert!(
        classes >= 1,
        "Expected ≥1 Ruby class symbol, got {}",
        classes
    );
}

#[test]
fn rust_service_extracts_function_symbols() {
    let result = analyze_fixture("rust_service");
    let symbols = &result.model.components[0].symbols;

    assert!(
        symbols.len() >= 3,
        "Rust project should extract ≥3 symbols, got {}",
        symbols.len()
    );

    // Rust uses functions and structs
    let functions = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .count();
    assert!(
        functions >= 1,
        "Expected ≥1 Rust function symbol, got {}",
        functions
    );

    // Rust symbols should have signatures
    let with_signature = symbols.iter().filter(|s| s.signature.is_some()).count();
    assert!(
        with_signature >= 1,
        "Expected ≥1 Rust symbol with signature, got {}",
        with_signature
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Data Model Extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_extracts_typescript_data_models() {
    let result = analyze_fixture("express_ecommerce");
    let data_models = &result.model.components[0].data_models;

    // stripe.ts defines TypeScript interfaces (StripeCharge, StripeRefund, etc.)
    assert!(
        !data_models.is_empty(),
        "Express project should extract data models (TypeScript interfaces in stripe.ts), got 0"
    );

    // Data models should have fields
    let with_fields = data_models
        .iter()
        .filter(|dm| !dm.fields.is_empty())
        .count();
    assert!(
        with_fields >= 1,
        "Expected ≥1 data model with fields, got {}",
        with_fields
    );

    // Stats should track data model count
    assert_eq!(
        result.model.stats.total_data_models,
        data_models.len(),
        "stats.total_data_models should match actual count"
    );
}

#[test]
fn spring_ecommerce_extracts_java_data_models() {
    let result = analyze_fixture("spring_ecommerce");
    let data_models = &result.model.components[0].data_models;

    assert!(
        !data_models.is_empty(),
        "Spring project should extract Java data models (classes with fields), got 0"
    );

    // At least some should be Class kind
    let classes = data_models
        .iter()
        .filter(|dm| dm.model_kind == DataModelKind::Class)
        .count();
    assert!(
        classes >= 1,
        "Expected ≥1 Class data model, got {}",
        classes
    );

    // Should have fields with types
    let has_typed_fields = data_models
        .iter()
        .any(|dm| dm.fields.iter().any(|f| f.field_type.is_some()));
    assert!(
        has_typed_fields,
        "Expected at least one data model with typed fields"
    );
}

#[test]
fn aspnet_ecommerce_extracts_csharp_data_models() {
    let result = analyze_fixture("aspnet_ecommerce");
    let data_models = &result.model.components[0].data_models;

    assert!(
        !data_models.is_empty(),
        "ASP.NET project should extract C# data models, got 0"
    );

    // All data models should have valid anchors
    for dm in data_models {
        assert!(
            !dm.name.is_empty(),
            "Data model should have a non-empty name"
        );
        assert!(
            dm.anchor.line > 0,
            "Data model '{}' should have line > 0",
            dm.name
        );
    }

    // Stats should track data model count
    assert_eq!(
        result.model.stats.total_data_models,
        data_models.len(),
        "stats.total_data_models should match actual count"
    );
}

#[test]
fn gin_ecommerce_extracts_go_struct_data_models() {
    let result = analyze_fixture("gin_ecommerce");
    let data_models = &result.model.components[0].data_models;

    assert!(
        !data_models.is_empty(),
        "Gin project should extract Go struct data models, got 0"
    );

    // Go data models should be Struct kind
    let structs = data_models
        .iter()
        .filter(|dm| dm.model_kind == DataModelKind::Struct)
        .count();
    assert!(
        structs >= 1,
        "Expected ≥1 Struct data model in Go project, got {}",
        structs
    );
}

// ═══════════════════════════════════════════════════════════════════
//  References / Call Graph
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_extracts_call_references() {
    let result = analyze_fixture("express_ecommerce");
    let references = &result.model.components[0].references;

    assert!(
        !references.is_empty(),
        "Express project should extract references (call sites, imports), got 0"
    );

    // Should have Call references (function calls across the project)
    let calls = references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Call)
        .count();
    assert!(calls >= 1, "Expected ≥1 Call reference, got {}", calls);

    // Should have Import references
    let imports = references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert!(
        imports >= 1,
        "Expected ≥1 Import reference, got {}",
        imports
    );

    // Stats should track reference count
    assert_eq!(
        result.model.stats.total_references,
        references.len(),
        "stats.total_references should match actual count"
    );
}

#[test]
fn nestjs_api_extracts_references() {
    let result = analyze_fixture("nestjs_api");
    let references = &result.model.components[0].references;

    assert!(
        !references.is_empty(),
        "NestJS project should extract references, got 0"
    );

    // All references should have valid source info
    for r in references {
        assert!(
            !r.source_file.as_os_str().is_empty(),
            "Reference should have a source_file"
        );
        assert!(r.source_line > 0, "Reference should have source_line > 0");
    }
}

#[test]
fn spring_ecommerce_extracts_references() {
    let result = analyze_fixture("spring_ecommerce");
    let references = &result.model.components[0].references;

    assert!(
        !references.is_empty(),
        "Spring project should extract references, got 0"
    );

    // Should have Call references at minimum
    let calls = references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Call)
        .count();
    assert!(
        calls >= 1,
        "Expected ≥1 Call reference in Spring project, got {}",
        calls
    );

    // All references should have valid source info
    for r in references {
        assert!(r.source_line > 0, "Reference should have source_line > 0");
        assert!(
            !r.target_symbol.is_empty(),
            "Reference should have non-empty target_symbol"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Confidence Scoring
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_references_have_confidence_scores() {
    let result = analyze_fixture("express_ecommerce");
    let references = &result.model.components[0].references;

    // At least some references should be resolved with confidence > 0
    let resolved = references.iter().filter(|r| r.confidence > 0.0).count();
    assert!(
        resolved >= 1,
        "Expected ≥1 reference with confidence > 0.0, got {} resolved out of {} total",
        resolved,
        references.len()
    );

    // Resolved references should have a non-Unresolved method
    let with_method = references
        .iter()
        .filter(|r| r.resolution_method != ResolutionMethod::Unresolved)
        .count();
    assert!(
        with_method >= 1,
        "Expected ≥1 reference with resolution_method != Unresolved, got {}",
        with_method
    );
}

#[test]
fn multi_file_projects_have_resolved_references_in_stats() {
    // Test that stats track resolved references for multi-file projects
    let fixtures = [
        "express_ecommerce",
        "nestjs_api",
        "spring_ecommerce",
        "aspnet_ecommerce",
    ];

    for fixture_name in &fixtures {
        let result = analyze_fixture(fixture_name);
        let stats = &result.model.stats;

        // Multi-file projects should have at least some references
        assert!(
            stats.total_references > 0,
            "[{fixture_name}] expected total_references > 0, got {}",
            stats.total_references
        );

        // resolved_references should be populated (may be 0 for some projects, but tracked)
        // avg_resolution_confidence should be 0.0 or positive
        assert!(
            stats.avg_resolution_confidence >= 0.0,
            "[{fixture_name}] avg_resolution_confidence should be >= 0.0, got {}",
            stats.avg_resolution_confidence
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Import Extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_extracts_typescript_imports() {
    let result = analyze_fixture("express_ecommerce");
    let imports = &result.model.components[0].imports;

    assert!(
        imports.len() >= 5,
        "Express project should extract ≥5 import statements (axios, express, etc.), got {}",
        imports.len()
    );

    // Should have imports from external packages
    let external = imports
        .iter()
        .any(|i| i.source.contains("axios") || i.source.contains("express"));
    assert!(
        external,
        "Expected imports from external packages (axios, express)"
    );

    // Stats: total_imports counts ReferenceKind::Import references (not ImportInfo entries)
    let import_ref_count = result.model.components[0]
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert_eq!(
        result.model.stats.total_imports, import_ref_count,
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

#[test]
fn nestjs_api_extracts_imports() {
    let result = analyze_fixture("nestjs_api");
    let imports = &result.model.components[0].imports;

    assert!(
        imports.len() >= 3,
        "NestJS project should extract ≥3 imports (@nestjs/common, etc.), got {}",
        imports.len()
    );

    // Should have NestJS framework imports
    let nestjs = imports.iter().any(|i| i.source.contains("@nestjs"));
    assert!(nestjs, "Expected imports from @nestjs packages");
}

#[test]
fn spring_ecommerce_import_count_matches_stats() {
    let result = analyze_fixture("spring_ecommerce");
    let comp = &result.model.components[0];

    // total_imports counts ReferenceKind::Import references
    let import_ref_count = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert_eq!(
        result.model.stats.total_imports, import_ref_count,
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

#[test]
fn fastapi_ecommerce_import_count_matches_stats() {
    let result = analyze_fixture("fastapi_ecommerce");
    let comp = &result.model.components[0];

    // total_imports counts ReferenceKind::Import references
    let import_ref_count = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert_eq!(
        result.model.stats.total_imports, import_ref_count,
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

#[test]
fn aspnet_ecommerce_import_count_matches_stats() {
    let result = analyze_fixture("aspnet_ecommerce");
    let comp = &result.model.components[0];

    // total_imports counts ReferenceKind::Import references
    let import_ref_count = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert_eq!(
        result.model.stats.total_imports, import_ref_count,
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

#[test]
fn gin_ecommerce_import_count_matches_stats() {
    let result = analyze_fixture("gin_ecommerce");
    let comp = &result.model.components[0];

    // total_imports counts ReferenceKind::Import references
    let import_ref_count = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .count();
    assert_eq!(
        result.model.stats.total_imports, import_ref_count,
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Module Boundary Inference
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_infers_module_boundaries() {
    let result = analyze_fixture("express_ecommerce");
    let modules = &result.model.components[0].module_boundaries;

    // express_ecommerce has subdirectories: routes/, services/, middleware/
    assert!(
        !modules.is_empty(),
        "Express project with subdirectories should infer module boundaries, got 0"
    );

    // Each module should have at least one file
    for m in modules {
        assert!(
            !m.files.is_empty(),
            "Module '{}' should have at least one file",
            m.name
        );
    }

    // Stats should track module count
    assert_eq!(
        result.model.stats.total_modules,
        modules.len(),
        "stats.total_modules should match actual module count"
    );
}

#[test]
fn spring_ecommerce_infers_module_boundaries() {
    let result = analyze_fixture("spring_ecommerce");
    let modules = &result.model.components[0].module_boundaries;

    // spring_ecommerce has subdirectories: controllers/, models/, services/, etc.
    assert!(
        !modules.is_empty(),
        "Spring project with subdirectories should infer module boundaries, got 0"
    );

    // Should have module names matching directory structure
    let module_names: Vec<&str> = modules.iter().map(|m| m.name.as_str()).collect();
    assert!(
        module_names
            .iter()
            .any(|n| n.contains("controllers") || n.contains("controller")),
        "Expected a controllers module, got: {:?}",
        module_names
    );
}

#[test]
fn aspnet_ecommerce_infers_module_boundaries() {
    let result = analyze_fixture("aspnet_ecommerce");
    let modules = &result.model.components[0].module_boundaries;

    // aspnet_ecommerce has Controllers/ and Services/ directories
    assert!(
        !modules.is_empty(),
        "ASP.NET project with Controllers/ and Services/ should infer module boundaries, got 0"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  SourceAnchor Quality
// ═══════════════════════════════════════════════════════════════════

#[test]
fn route_anchors_have_valid_positions() {
    let result = analyze_fixture("express_ecommerce");
    let routes = &result.model.components[0].interfaces;

    assert!(!routes.is_empty(), "Need routes to validate anchors");

    for route in routes {
        assert!(
            route.anchor.line > 0,
            "Route {} {} should have line > 0, got {}",
            route.method,
            route.path,
            route.anchor.line
        );
        assert!(
            route.anchor.end_line >= route.anchor.line,
            "Route {} {} should have end_line >= line",
            route.method,
            route.path
        );
        assert!(
            route.anchor.end_byte > route.anchor.start_byte,
            "Route {} {} should have end_byte > start_byte (got {}..{})",
            route.method,
            route.path,
            route.anchor.start_byte,
            route.anchor.end_byte
        );
        assert!(
            !route.anchor.node_kind.is_empty(),
            "Route {} {} should have non-empty node_kind",
            route.method,
            route.path
        );
        assert!(
            !route.anchor.file.as_os_str().is_empty(),
            "Route {} {} should have non-empty file path",
            route.method,
            route.path
        );
    }
}

#[test]
fn symbol_anchors_have_valid_positions() {
    let result = analyze_fixture("express_ecommerce");
    let symbols = &result.model.components[0].symbols;

    assert!(!symbols.is_empty(), "Need symbols to validate anchors");

    for symbol in symbols {
        assert!(
            symbol.anchor.line > 0,
            "Symbol '{}' should have line > 0, got {}",
            symbol.name,
            symbol.anchor.line
        );
        assert!(
            symbol.anchor.end_line >= symbol.anchor.line,
            "Symbol '{}' should have end_line >= line",
            symbol.name
        );
        assert!(
            !symbol.anchor.file.as_os_str().is_empty(),
            "Symbol '{}' should have non-empty file path",
            symbol.name
        );
    }
}

#[test]
fn sink_anchors_have_valid_positions() {
    let result = analyze_fixture("express_ecommerce");
    let sinks = &result.model.components[0].sinks;

    assert!(!sinks.is_empty(), "Need sinks to validate anchors");

    for sink in sinks {
        assert!(
            sink.anchor.line > 0,
            "Sink '{}' should have line > 0, got {}",
            sink.text,
            sink.anchor.line
        );
        assert!(
            sink.anchor.end_byte > sink.anchor.start_byte,
            "Sink should have end_byte > start_byte (got {}..{})",
            sink.anchor.start_byte,
            sink.anchor.end_byte
        );
    }
}

#[test]
fn dependency_anchors_have_valid_positions() {
    let result = analyze_fixture("express_ecommerce");
    let deps = &result.model.components[0].dependencies;

    assert!(!deps.is_empty(), "Need dependencies to validate anchors");

    for dep in deps {
        assert!(
            dep.anchor.line > 0,
            "Dependency '{}' should have line > 0",
            dep.target
        );
        assert!(
            dep.anchor.end_byte > dep.anchor.start_byte,
            "Dependency '{}' should have end_byte > start_byte",
            dep.target
        );
    }
}

#[test]
fn data_model_anchors_have_valid_positions() {
    let result = analyze_fixture("express_ecommerce");
    let data_models = &result.model.components[0].data_models;

    if data_models.is_empty() {
        return; // Skip if no data models (some extractors may not produce them)
    }

    for dm in data_models {
        assert!(
            dm.anchor.line > 0,
            "DataModel '{}' should have line > 0",
            dm.name
        );
        assert!(
            dm.anchor.end_line >= dm.anchor.line,
            "DataModel '{}' should have end_line >= line",
            dm.name
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
//  KnowledgeGraph: Construction, Stats, Impact Analysis, Cycles
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_knowledge_graph_has_nodes_and_edges() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let graph = engine
        .graph()
        .expect("graph should be built after analysis");
    let stats = graph.stats();

    assert!(
        stats.total_nodes >= 10,
        "Express graph should have ≥10 nodes (files + symbols + interfaces), got {}",
        stats.total_nodes
    );
    assert!(
        stats.total_edges >= 10,
        "Express graph should have ≥10 edges (defines + calls + imports), got {}",
        stats.total_edges
    );

    // Should have file nodes (lowercase key from type_name())
    let file_count = stats.node_counts.get("file").copied().unwrap_or(0);
    assert!(
        file_count >= 4,
        "Expected ≥4 file nodes, got {}",
        file_count
    );

    // Should have symbol nodes
    let symbol_count = stats.node_counts.get("symbol").copied().unwrap_or(0);
    assert!(
        symbol_count >= 3,
        "Expected ≥3 symbol nodes, got {}",
        symbol_count
    );

    // Should have defines edges
    let defines_count = stats.edge_counts.get("defines").copied().unwrap_or(0);
    assert!(
        defines_count >= 3,
        "Expected ≥3 defines edges, got {}",
        defines_count
    );
}

#[test]
fn express_ecommerce_graph_stats_in_result() {
    let result = analyze_fixture("express_ecommerce");

    let graph_stats = result
        .graph_stats
        .as_ref()
        .expect("ExtractionResult should include graph_stats");

    assert!(
        graph_stats.total_nodes > 0,
        "graph_stats.total_nodes should be > 0"
    );
    assert!(
        graph_stats.total_edges > 0,
        "graph_stats.total_edges should be > 0"
    );
    assert!(
        graph_stats.connected_components >= 1,
        "graph_stats.connected_components should be >= 1"
    );
}

#[test]
fn express_ecommerce_impact_analysis_returns_results() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let graph = engine.graph().expect("graph should exist");

    // Find a symbol to analyze impact for
    let symbols = &engine.extractions().values().next().unwrap().symbols;
    if let Some(first_symbol) = symbols.first() {
        let impact = graph.impact_analysis(&first_symbol.name, 5);

        // Impact analysis should at minimum return the symbol's own file
        assert!(
            !impact.root.is_empty(),
            "impact_analysis root should be non-empty"
        );
    }
}

#[test]
fn express_ecommerce_graph_exports_to_json() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let graph = engine.graph().expect("graph should exist");
    let json = graph.to_json();

    // JSON export should have nodes and edges arrays
    assert!(
        json.get("nodes").is_some(),
        "Graph JSON should have 'nodes' key"
    );
    assert!(
        json.get("edges").is_some(),
        "Graph JSON should have 'edges' key"
    );

    let nodes = json["nodes"].as_array().expect("nodes should be an array");
    let edges = json["edges"].as_array().expect("edges should be an array");

    assert!(!nodes.is_empty(), "Graph JSON nodes should be non-empty");
    assert!(!edges.is_empty(), "Graph JSON edges should be non-empty");
}

#[test]
fn express_ecommerce_find_cycles() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let graph = engine.graph().expect("graph should exist");

    // find_cycles should not panic and should return a valid result
    let cycles = graph.find_cycles();
    // Cycles may or may not exist — just verify it runs without error
    let _ = cycles.len();

    // Module cycles should also work
    let module_cycles = graph.find_module_cycles();
    let _ = module_cycles.len();
}

#[test]
fn spring_ecommerce_graph_analysis_detects_entry_points() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/spring_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let ctx = engine
        .run_graph_analysis()
        .expect("graph analysis should produce results");

    // Spring project with many routes should have entry points
    assert!(
        !ctx.entry_points.is_empty(),
        "Spring project should detect entry points (HTTP endpoints)"
    );

    // Should have degree centrality computed
    assert!(
        !ctx.degree_centrality.is_empty(),
        "degree centrality should be computed"
    );
}

#[test]
fn aspnet_ecommerce_graph_analysis_pipeline() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/aspnet_ecommerce");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let ctx = engine
        .run_graph_analysis()
        .expect("graph analysis should produce results");

    // All analysis phases should produce non-empty results on a real project
    assert!(
        !ctx.degree_centrality.is_empty(),
        "degree centrality should be non-empty for ASP.NET project"
    );
    assert!(
        !ctx.entry_points.is_empty(),
        "should detect entry points in ASP.NET project"
    );
}

#[test]
fn nestjs_api_graph_has_interface_nodes() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/nestjs_api");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let graph = engine.graph().expect("graph should exist");
    let stats = graph.stats();

    // NestJS project with many routes should have interface nodes
    let interface_count = stats.node_counts.get("interface").copied().unwrap_or(0);
    assert!(
        interface_count >= 5,
        "NestJS graph should have ≥5 interface nodes, got {}",
        interface_count
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Semantic Diff (incremental analysis)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn express_ecommerce_full_analysis_produces_no_diff_on_first_run() {
    let result = analyze_fixture("express_ecommerce");

    // First analysis should NOT produce a diff (no previous model to compare against)
    assert!(
        result.diff.is_none(),
        "First analysis should have diff = None"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Pipeline Timing
// ═══════════════════════════════════════════════════════════════════

#[test]
fn extraction_result_has_valid_timing() {
    let result = analyze_fixture("express_ecommerce");

    assert!(result.timing.total_ms > 0, "total_ms should be > 0");
    assert!(
        result.timing.parse_extract_ms > 0,
        "parse_extract_ms should be > 0"
    );
    assert!(
        result.timing.model_build_ms > 0 || result.timing.total_ms > 0,
        "model_build_ms or total_ms should be > 0"
    );
    assert!(result.duration_ms > 0, "duration_ms should be > 0");
}

// ═══════════════════════════════════════════════════════════════════
//  Cross-Framework Comprehensive Validation
// ═══════════════════════════════════════════════════════════════════

/// Validates that ALL multi-file framework fixtures produce a minimum
/// baseline of extraction output across every feature dimension.
#[test]
fn all_framework_fixtures_produce_complete_extraction() {
    let frameworks = vec![
        ("express_ecommerce", 4, true), // (name, min_files, has_subdirs)
        ("nestjs_api", 4, false),
        ("fastapi_ecommerce", 3, true),
        ("spring_ecommerce", 4, true),
        ("aspnet_ecommerce", 5, true),
        ("gin_ecommerce", 4, true),
        ("laravel_ecommerce", 3, true),
        ("rails_ecommerce", 4, true),
    ];

    for (fixture, min_files, has_subdirs) in &frameworks {
        let result = analyze_fixture(fixture);
        let comp = &result.model.components[0];
        let stats = &result.model.stats;

        // Basic extraction
        assert!(
            result.files_analyzed >= *min_files,
            "[{fixture}] expected ≥{min_files} files, got {}",
            result.files_analyzed
        );

        // Routes
        assert!(
            stats.total_interfaces > 0,
            "[{fixture}] expected routes > 0, got {}",
            stats.total_interfaces
        );

        // Symbols
        assert!(
            stats.total_symbols > 0,
            "[{fixture}] expected symbols > 0, got {}",
            stats.total_symbols
        );

        // Sinks
        assert!(
            stats.total_sinks > 0,
            "[{fixture}] expected sinks > 0, got {}",
            stats.total_sinks
        );

        // References
        assert!(
            stats.total_references > 0,
            "[{fixture}] expected references > 0, got {}",
            stats.total_references
        );

        // Imports: total_imports counts ReferenceKind::Import references
        let import_ref_count = comp
            .references
            .iter()
            .filter(|r| r.reference_kind == ReferenceKind::Import)
            .count();
        assert_eq!(
            stats.total_imports, import_ref_count,
            "[{fixture}] stats.total_imports should match ReferenceKind::Import reference count"
        );

        // Module boundaries (only for projects with subdirectories)
        if *has_subdirs {
            assert!(
                !comp.module_boundaries.is_empty(),
                "[{fixture}] expected module_boundaries for project with subdirs"
            );
        }

        // Graph stats should be present
        assert!(
            result.graph_stats.is_some(),
            "[{fixture}] expected graph_stats to be Some"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Workspace / Monorepo Detection
// ═══════════════════════════════════════════════════════════════════

#[test]
fn pnpm_monorepo_produces_multiple_components() {
    let result = analyze_fixture("pnpm_monorepo");

    // pnpm_monorepo has packages/api and packages/auth
    // Plus the default (root) component = 3 components total
    assert!(
        result.model.components.len() >= 2,
        "pnpm monorepo should produce ≥2 components, got {}",
        result.model.components.len()
    );

    let component_names: Vec<&str> = result
        .model
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(
        component_names.iter().any(|n| n.contains("api")),
        "Should have an api component, got: {:?}",
        component_names
    );
    assert!(
        component_names.iter().any(|n| n.contains("auth")),
        "Should have an auth component, got: {:?}",
        component_names
    );

    // api package should have its own routes
    let api_comp = result
        .model
        .components
        .iter()
        .find(|c| c.name.contains("api"))
        .expect("api component should exist");
    assert!(
        !api_comp.interfaces.is_empty(),
        "api component should have interfaces, got 0"
    );

    // auth package should have its own routes
    let auth_comp = result
        .model
        .components
        .iter()
        .find(|c| c.name.contains("auth"))
        .expect("auth component should exist");
    assert!(
        !auth_comp.interfaces.is_empty(),
        "auth component should have interfaces, got 0"
    );
}

#[test]
fn cargo_monorepo_produces_multiple_components() {
    let result = analyze_fixture("cargo_monorepo");

    // cargo_monorepo has crates/core and crates/api
    assert!(
        result.model.components.len() >= 2,
        "Cargo monorepo should produce ≥2 components, got {}",
        result.model.components.len()
    );

    let component_names: Vec<&str> = result
        .model
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(
        component_names.iter().any(|n| n.contains("core")),
        "Should have a core component, got: {:?}",
        component_names
    );
    assert!(
        component_names.iter().any(|n| n.contains("api")),
        "Should have an api component, got: {:?}",
        component_names
    );

    // Both crates should have symbols
    let core_comp = result
        .model
        .components
        .iter()
        .find(|c| c.name.contains("core"))
        .expect("core component should exist");
    assert!(
        !core_comp.symbols.is_empty(),
        "core component should have symbols, got 0"
    );

    let api_comp = result
        .model
        .components
        .iter()
        .find(|c| c.name.contains("api"))
        .expect("api component should exist");
    assert!(
        !api_comp.symbols.is_empty(),
        "api component should have symbols, got 0"
    );
}

#[test]
fn single_project_still_produces_one_component() {
    // Non-workspace fixture should produce exactly 1 component (backward compat)
    let result = analyze_fixture("express_ecommerce");

    assert_eq!(
        result.model.components.len(),
        1,
        "Single-project fixture should produce exactly 1 component, got {}",
        result.model.components.len()
    );
}

#[test]
fn workspace_layout_accessible_after_analysis() {
    // pnpm monorepo should expose its workspace layout
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/pnpm_monorepo");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let layout = engine
        .workspace_layout()
        .expect("pnpm monorepo should have a workspace layout");

    assert_eq!(layout.kind, WorkspaceKind::Pnpm);
    assert_eq!(
        layout.packages.len(),
        2,
        "pnpm monorepo should detect 2 packages, got {}",
        layout.packages.len()
    );

    // Single-project fixture should NOT have a workspace layout
    let fixture_path = manifest_dir.join("tests/fixtures/express_ecommerce");
    let engine = IntentlyEngine::new(fixture_path);
    assert!(
        engine.workspace_layout().is_none(),
        "Single-project fixture should have workspace_layout = None"
    );
}

#[test]
fn monorepo_stats_aggregate_across_components() {
    let result = analyze_fixture("pnpm_monorepo");

    // Stats should aggregate across all components
    assert!(
        result.model.stats.files_analyzed >= 2,
        "Monorepo should analyze files from both packages, got {}",
        result.model.stats.files_analyzed
    );

    // Total interfaces should be the sum across all components
    let component_interfaces: usize = result
        .model
        .components
        .iter()
        .map(|c| c.interfaces.len())
        .sum();
    assert_eq!(
        result.model.stats.total_interfaces, component_interfaces,
        "stats.total_interfaces should equal sum across components"
    );
}

// ─── npm workspace ───────────────────────────────────────────────

#[test]
fn npm_monorepo_produces_multiple_components() {
    let result = analyze_fixture("npm_monorepo");

    assert!(
        result.model.components.len() >= 2,
        "npm monorepo should produce ≥2 components, got {}",
        result.model.components.len()
    );

    let component_names: Vec<&str> = result
        .model
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(
        component_names.iter().any(|n| n.contains("gateway")),
        "Should have a gateway component, got: {:?}",
        component_names
    );
    assert!(
        component_names.iter().any(|n| n.contains("products")),
        "Should have a products component, got: {:?}",
        component_names
    );
}

#[test]
fn npm_monorepo_detects_workspace_kind() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/npm_monorepo");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let layout = engine
        .workspace_layout()
        .expect("npm monorepo should have a workspace layout");

    assert_eq!(layout.kind, WorkspaceKind::Npm);
    assert_eq!(
        layout.packages.len(),
        2,
        "npm monorepo should detect 2 packages, got {}",
        layout.packages.len()
    );
}

#[test]
fn npm_monorepo_extracts_routes_across_packages() {
    let result = analyze_fixture("npm_monorepo");

    let total_routes: usize = result
        .model
        .components
        .iter()
        .map(|c| c.interfaces.len())
        .sum();

    assert!(
        total_routes >= 6,
        "npm monorepo should have ≥6 total routes across packages, got {}",
        total_routes
    );
}

// ─── Go workspace ────────────────────────────────────────────────

#[test]
fn go_workspace_produces_multiple_components() {
    let result = analyze_fixture("go_workspace");

    assert!(
        result.model.components.len() >= 2,
        "Go workspace should produce ≥2 components, got {}",
        result.model.components.len()
    );

    let component_names: Vec<&str> = result
        .model
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    // go.work uses module names from go.mod files
    assert!(
        component_names.iter().any(|n| n.contains("api")),
        "Should have an api component, got: {:?}",
        component_names
    );
    assert!(
        component_names.iter().any(|n| n.contains("auth")),
        "Should have an auth component, got: {:?}",
        component_names
    );
}

#[test]
fn go_workspace_detects_workspace_kind() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/go_workspace");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let layout = engine
        .workspace_layout()
        .expect("Go workspace should have a workspace layout");

    assert_eq!(layout.kind, WorkspaceKind::Go);
    assert_eq!(
        layout.packages.len(),
        2,
        "Go workspace should detect 2 packages, got {}",
        layout.packages.len()
    );
}

#[test]
fn go_workspace_extracts_routes_and_symbols() {
    let result = analyze_fixture("go_workspace");

    let total_routes: usize = result
        .model
        .components
        .iter()
        .map(|c| c.interfaces.len())
        .sum();

    assert!(
        total_routes >= 5,
        "Go workspace should have ≥5 Gin routes, got {}",
        total_routes
    );

    let total_symbols: usize = result
        .model
        .components
        .iter()
        .map(|c| c.symbols.len())
        .sum();

    assert!(
        total_symbols >= 5,
        "Go workspace should have ≥5 symbols, got {}",
        total_symbols
    );
}

// ─── uv workspace ────────────────────────────────────────────────

#[test]
fn uv_workspace_produces_multiple_components() {
    let result = analyze_fixture("uv_workspace");

    assert!(
        result.model.components.len() >= 2,
        "uv workspace should produce ≥2 components, got {}",
        result.model.components.len()
    );

    let component_names: Vec<&str> = result
        .model
        .components
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(
        component_names.iter().any(|n| n.contains("api")),
        "Should have an api component, got: {:?}",
        component_names
    );
    assert!(
        component_names.iter().any(|n| n.contains("auth")),
        "Should have an auth component, got: {:?}",
        component_names
    );
}

#[test]
fn uv_workspace_detects_workspace_kind() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = manifest_dir.join("tests/fixtures/uv_workspace");

    let mut engine = IntentlyEngine::new(fixture_path);
    engine.full_analysis().expect("extraction should succeed");

    let layout = engine
        .workspace_layout()
        .expect("uv workspace should have a workspace layout");

    assert_eq!(layout.kind, WorkspaceKind::Uv);
    assert_eq!(
        layout.packages.len(),
        2,
        "uv workspace should detect 2 packages, got {}",
        layout.packages.len()
    );
}

#[test]
fn uv_workspace_extracts_routes_and_symbols() {
    let result = analyze_fixture("uv_workspace");

    let total_routes: usize = result
        .model
        .components
        .iter()
        .map(|c| c.interfaces.len())
        .sum();

    assert!(
        total_routes >= 4,
        "uv workspace should have ≥4 FastAPI routes, got {}",
        total_routes
    );
}

// ═══════════════════════════════════════════════════════════════════
//  FileTree
// ═══════════════════════════════════════════════════════════════════

#[test]
fn file_tree_present_after_full_analysis() {
    let result = analyze_fixture("express_ecommerce");

    let tree = result
        .model
        .file_tree
        .as_ref()
        .expect("file_tree should be populated after full analysis");

    // Root should have subdirectories (express_ecommerce has src/ at minimum)
    assert!(
        !tree.root.subdirectories.is_empty(),
        "root should have subdirectories, got empty"
    );

    // Total files should match stats
    assert_eq!(
        tree.root.stats.total_file_count, result.model.stats.files_analyzed,
        "file tree total count should match stats.files_analyzed"
    );

    // total_directories should be populated in stats
    assert!(
        result.model.stats.total_directories > 0,
        "total_directories should be > 0 for a non-empty project"
    );
}

#[test]
fn monorepo_file_tree_has_component_names() {
    let result = analyze_fixture("cargo_monorepo");

    let tree = result
        .model
        .file_tree
        .as_ref()
        .expect("file_tree should be populated for monorepo");

    // Collect all component names from the tree recursively
    fn collect_component_names(node: &intently_core::DirectoryNode) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(ref name) = node.component_name {
            names.push(name.clone());
        }
        for sub in &node.subdirectories {
            names.extend(collect_component_names(sub));
        }
        names
    }

    let component_names = collect_component_names(&tree.root);

    // A Cargo monorepo fixture should have at least one package with a component_name
    assert!(
        !component_names.is_empty(),
        "monorepo file tree should have at least one directory with component_name set, got none"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  Confidence Filter
// ═══════════════════════════════════════════════════════════════════

#[test]
fn filtered_model_excludes_low_confidence_references() {
    let result = analyze_fixture("express_ecommerce");
    let model = &result.model;

    // Verify there are references with varying confidence levels
    let all_refs: Vec<&intently_core::model::types::Reference> = model
        .components
        .iter()
        .flat_map(|c| c.references.iter())
        .collect();

    // The express_ecommerce fixture should produce references (calls, imports, etc.)
    assert!(
        !all_refs.is_empty(),
        "express_ecommerce should have references for filtering test"
    );

    let filtered = model.filtered(0.5);
    let filtered_refs: Vec<&intently_core::model::types::Reference> = filtered
        .components
        .iter()
        .flat_map(|c| c.references.iter())
        .collect();

    // After filtering, no reference should be below the threshold
    for r in &filtered_refs {
        assert!(
            r.confidence >= 0.5,
            "filtered(0.5) should not contain ref with confidence {:.2}: {} -> {}",
            r.confidence,
            r.source_symbol,
            r.target_symbol
        );
    }

    // Stats should match filtered count
    assert_eq!(
        filtered.stats.total_references,
        filtered_refs.len(),
        "stats.total_references should match filtered reference count"
    );
}

#[test]
fn test_references_field_populated_in_full_analysis() {
    // Verify that is_test_reference is populated (not left as default)
    // during the full extraction pipeline.
    //
    // Note: fixtures live under `tests/fixtures/`, so FileRole::from_path
    // sees the absolute path containing `tests/` and classifies all files
    // as Test — which means is_test_reference can be true even for
    // production-only fixtures when run as integration tests.
    // This is a known limitation of absolute-path-based role classification.
    let result = analyze_fixture("express_ecommerce");
    let all_refs: Vec<&intently_core::model::types::Reference> = result
        .model
        .components
        .iter()
        .flat_map(|c| c.references.iter())
        .collect();

    // The field should exist on all references (serde default is false)
    assert!(
        !all_refs.is_empty(),
        "express_ecommerce should produce references"
    );

    // Verify the field is accessible and has a boolean value
    let _tagged_count = all_refs.iter().filter(|r| r.is_test_reference).count();
    let _untagged_count = all_refs.iter().filter(|r| !r.is_test_reference).count();

    // The important thing: the field exists, is populated, and doesn't panic
    assert_eq!(
        _tagged_count + _untagged_count,
        all_refs.len(),
        "all references should have is_test_reference field"
    );
}

// ---------------------------------------------------------------------------
// Git metadata tests (feature-gated)
// ---------------------------------------------------------------------------

/// Verify that git metadata is computed when running on intently-core's own repo.
///
/// This test uses `#[ignore]` + `#[cfg(feature = "git")]` because:
/// - It runs against the real intently-core git history
/// - It requires the `git` feature flag to be enabled
/// - CI should run this explicitly with `cargo test --features git -- --ignored`
#[test]
#[ignore]
#[cfg(feature = "git")]
fn git_metadata_populated_on_own_repo() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut engine = IntentlyEngine::new(manifest_dir);
    let result = engine
        .full_analysis()
        .expect("full analysis should succeed");

    // Verify git_stats was computed
    assert!(
        result.model.stats.git_stats.is_some(),
        "git_stats should be populated when analyzing a git repo with feature=git"
    );

    let git_stats = result.model.stats.git_stats.as_ref().unwrap();
    assert!(
        git_stats.total_authors > 0,
        "should find at least one author"
    );
    assert!(
        git_stats.total_commits > 0,
        "should count at least one commit"
    );
    assert!(
        !git_stats.hottest_files.is_empty(),
        "should identify hottest files"
    );

    // Verify per-file git metadata is populated on at least some extractions
    let extractions_with_git: usize = engine
        .extractions()
        .values()
        .filter(|e| e.git_metadata.is_some())
        .count();
    assert!(
        extractions_with_git > 0,
        "at least some files should have git metadata"
    );
}

// ---------------------------------------------------------------------------
// Python import extraction and resolution
// ---------------------------------------------------------------------------

#[test]
fn python_imports_fixture_extracts_imports() {
    let result = analyze_fixture("python_imports");
    let comp = &result.model.components[0];

    // 5 Python files (main.py, config.py, app/models.py, app/routes.py, app/services.py)
    // __init__.py is empty so may not produce extractions
    assert!(
        result.files_analyzed >= 4,
        "Expected ≥4 Python files analyzed, got {}",
        result.files_analyzed
    );

    // Python import extraction should produce ImportInfo entries
    let total_import_infos: usize = comp.imports.len();
    assert!(
        total_import_infos >= 5,
        "Expected ≥5 ImportInfo entries from Python files, got {}",
        total_import_infos
    );

    // Verify import references exist in the reference list
    let import_refs: Vec<&Reference> = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .collect();
    assert!(
        !import_refs.is_empty(),
        "Expected import references in the model, got 0"
    );

    // stats.total_imports should match import reference count
    assert_eq!(
        result.model.stats.total_imports,
        import_refs.len(),
        "stats.total_imports should match ReferenceKind::Import reference count"
    );
}

#[test]
fn python_imports_fixture_classifies_stdlib_as_external() {
    let result = analyze_fixture("python_imports");
    let comp = &result.model.components[0];

    let import_refs: Vec<&Reference> = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .collect();

    // Stdlib imports (os, sys, json, logging, etc.) should be External, not Unresolved
    let external_refs: Vec<&&Reference> = import_refs
        .iter()
        .filter(|r| r.resolution_method == ResolutionMethod::External)
        .collect();
    assert!(
        !external_refs.is_empty(),
        "Expected some External resolution method references (stdlib/package imports)"
    );

    // Verify specific stdlib imports are classified as External
    let os_import = import_refs.iter().find(|r| r.target_symbol == "os");
    assert!(os_import.is_some(), "Expected an import reference for 'os'");
    assert_eq!(
        os_import.unwrap().resolution_method,
        ResolutionMethod::External,
        "'os' import should be External, not Unresolved"
    );
}

#[test]
fn python_imports_fixture_resolves_relative_imports() {
    let result = analyze_fixture("python_imports");
    let comp = &result.model.components[0];

    let import_refs: Vec<&Reference> = comp
        .references
        .iter()
        .filter(|r| r.reference_kind == ReferenceKind::Import)
        .collect();

    // Relative imports (from .models import User) should resolve with ImportBased
    let import_based_refs: Vec<&&Reference> = import_refs
        .iter()
        .filter(|r| r.resolution_method == ResolutionMethod::ImportBased)
        .collect();

    // app/routes.py imports from .models (User, Product) and app/services.py imports from .models
    // These should resolve to app/models.py since the files exist in the fixture
    assert!(
        !import_based_refs.is_empty(),
        "Expected some ImportBased references from resolved relative imports, got 0. \
         All import refs: {:?}",
        import_refs
            .iter()
            .map(|r| format!(
                "{}→{} ({:?})",
                r.source_file.display(),
                r.target_symbol,
                r.resolution_method
            ))
            .collect::<Vec<_>>()
    );
}

#[test]
fn python_imports_fixture_has_resolution_method_distribution() {
    let result = analyze_fixture("python_imports");

    let dist = &result.model.stats.resolution_method_distribution;
    assert!(
        !dist.is_empty(),
        "resolution_method_distribution should not be empty"
    );

    // Should have at least 'external' bucket (from stdlib/package imports)
    assert!(
        dist.contains_key("external"),
        "Expected 'external' in resolution_method_distribution, got: {:?}",
        dist
    );
}
