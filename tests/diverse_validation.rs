//! Diverse real-world validation suite for the Intently extraction engine.
//!
//! Tests 130+ GitHub repos across 13 language groups (10+ per language).
//! Each repo is cloned into a `TempDir` that auto-deletes after each iteration,
//! keeping disk usage minimal (~50MB peak for a single shallow clone).
//!
//! ALL tests use `#[ignore]` — they require network access and are slow.
//!
//! ```bash
//! # Run all diverse suites
//! cargo test --test diverse_validation -- --ignored --nocapture
//!
//! # Run one language suite
//! cargo test --test diverse_validation suite_typescript -- --ignored --nocapture
//! cargo test --test diverse_validation suite_python -- --ignored --nocapture
//! ```

#[allow(dead_code)]
mod common;

// ═══════════════════════════════════════════════════════════════════
//  Table-Driven Suite Infrastructure
// ═══════════════════════════════════════════════════════════════════

struct RepoSpec {
    name: &'static str,
    url: &'static str,
    subdir: Option<&'static str>,
    timeout_secs: u64,
    min_symbols: usize,
    min_files: usize,
}

/// Run a language validation suite: clone each repo, analyze, assert, report.
///
/// Uses `catch_unwind` per repo so one failure doesn't abort the suite.
/// `TempDir` drops at end of each iteration → disk freed automatically.
fn run_language_suite(suite_name: &str, specs: &[RepoSpec]) {
    eprintln!(
        "\n{}",
        "=".repeat(70)
    );
    eprintln!("  Suite: {suite_name} ({} repos)", specs.len());
    eprintln!("{}", "=".repeat(70));

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for spec in specs {
        let spec_name = spec.name;
        let spec_url = spec.url;
        let spec_subdir = spec.subdir;
        let spec_timeout = spec.timeout_secs;
        let spec_min_files = spec.min_files;
        let spec_min_symbols = spec.min_symbols;

        let result = std::panic::catch_unwind(|| {
            let (_tmp, path) = common::clone_repo(spec_url, spec_subdir);
            let result = common::analyze_repo(&path, spec_timeout);
            common::print_report(&result, spec_name);

            // Core assertions that apply to every repo
            common::assert_basic_invariants(&result, spec_name);
            assert!(
                result.files_analyzed >= spec_min_files,
                "[{spec_name}] expected >= {spec_min_files} files, got {}",
                result.files_analyzed
            );
            common::assert_has_symbols(&result, spec_name, spec_min_symbols);
            common::assert_has_graph_stats(&result, spec_name);
            common::assert_anchors_valid(&result, spec_name);
        });

        match result {
            Ok(()) => {
                passed += 1;
                eprintln!("  [PASS] {spec_name}");
            }
            Err(e) => {
                failed += 1;
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    (*s).to_string()
                } else {
                    "unknown panic".to_string()
                };
                eprintln!("  [FAIL] {spec_name}: {msg}");
                failures.push(format!("{spec_name}: {msg}"));
            }
        }
    }

    eprintln!(
        "\n  Suite {suite_name}: {passed} passed, {failed} failed out of {}",
        specs.len()
    );

    if !failures.is_empty() {
        eprintln!("\n  Failures:");
        for f in &failures {
            eprintln!("    - {f}");
        }
    }

    assert_eq!(
        failed, 0,
        "{suite_name}: {failed} repo(s) failed — see details above"
    );
}

// ═══════════════════════════════════════════════════════════════════
//  1. TypeScript / JavaScript
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_typescript() {
    let specs = [
        RepoSpec {
            name: "astro",
            url: "https://github.com/withastro/astro",
            subdir: Some("packages/astro"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "payload-cms",
            url: "https://github.com/payloadcms/payload",
            subdir: Some("packages/payload"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "directus",
            url: "https://github.com/directus/directus",
            subdir: Some("api"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "strapi",
            url: "https://github.com/strapi/strapi",
            subdir: Some("packages/core/strapi"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "supabase-js",
            url: "https://github.com/supabase/supabase-js",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "trpc",
            url: "https://github.com/trpc/trpc",
            subdir: Some("packages/server"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "hono",
            url: "https://github.com/honojs/hono",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "express",
            url: "https://github.com/expressjs/express",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "fastify",
            url: "https://github.com/fastify/fastify",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "zod",
            url: "https://github.com/colinhacks/zod",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 3,
        },
    ];

    run_language_suite("TypeScript/JavaScript", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  2. Python
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_python() {
    let specs = [
        RepoSpec {
            name: "fastapi",
            url: "https://github.com/fastapi/fastapi",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 30,
            min_files: 10,
        },
        RepoSpec {
            name: "flask",
            url: "https://github.com/pallets/flask",
            subdir: Some("src/flask"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 5,
        },
        RepoSpec {
            name: "celery",
            url: "https://github.com/celery/celery",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "black",
            url: "https://github.com/psf/black",
            subdir: Some("src/black"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 5,
        },
        RepoSpec {
            name: "poetry",
            url: "https://github.com/python-poetry/poetry",
            subdir: Some("src/poetry"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "pydantic",
            url: "https://github.com/pydantic/pydantic",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 10,
        },
        RepoSpec {
            name: "requests",
            url: "https://github.com/psf/requests",
            subdir: Some("src/requests"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 3,
        },
        RepoSpec {
            name: "scrapy",
            url: "https://github.com/scrapy/scrapy",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "ansible-core",
            url: "https://github.com/ansible/ansible",
            subdir: Some("lib/ansible"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "click",
            url: "https://github.com/pallets/click",
            subdir: Some("src/click"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 3,
        },
    ];

    run_language_suite("Python", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  3. Java
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_java() {
    let specs = [
        RepoSpec {
            name: "spring-framework",
            url: "https://github.com/spring-projects/spring-framework",
            subdir: Some("spring-core"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "guava",
            url: "https://github.com/google/guava",
            subdir: Some("guava"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "mybatis-3",
            url: "https://github.com/mybatis/mybatis-3",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "jackson-databind",
            url: "https://github.com/FasterXML/jackson-databind",
            subdir: None,
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "mockito",
            url: "https://github.com/mockito/mockito",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "junit5",
            url: "https://github.com/junit-team/junit5",
            subdir: Some("junit-jupiter-api"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "apache-kafka-clients",
            url: "https://github.com/apache/kafka",
            subdir: Some("clients"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "elasticsearch-server",
            url: "https://github.com/elastic/elasticsearch",
            subdir: Some("server/src"),
            timeout_secs: 600,
            min_symbols: 200,
            min_files: 100,
        },
        RepoSpec {
            name: "dagger",
            url: "https://github.com/google/dagger",
            subdir: Some("java/dagger"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "retrofit",
            url: "https://github.com/square/retrofit",
            subdir: Some("retrofit"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
    ];

    run_language_suite("Java", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  4. Kotlin
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_kotlin() {
    let specs = [
        RepoSpec {
            name: "ktor-server-core",
            url: "https://github.com/ktorio/ktor",
            subdir: Some("ktor-server/ktor-server-core"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "okhttp",
            url: "https://github.com/square/okhttp",
            subdir: Some("okhttp"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "coroutines",
            url: "https://github.com/Kotlin/kotlinx.coroutines",
            subdir: Some("kotlinx-coroutines-core"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "exposed",
            url: "https://github.com/JetBrains/Exposed",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "detekt-core",
            url: "https://github.com/detekt/detekt",
            subdir: Some("detekt-core"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "arrow-core",
            url: "https://github.com/arrow-kt/arrow",
            subdir: Some("arrow-libs/core"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "koin-core",
            url: "https://github.com/InsertKoinIO/koin",
            subdir: Some("projects/core/koin-core"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "moshi",
            url: "https://github.com/square/moshi",
            subdir: Some("moshi"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "coil-core",
            url: "https://github.com/coil-kt/coil",
            subdir: Some("coil-core"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "ktlint-core",
            url: "https://github.com/pinterest/ktlint",
            subdir: Some("ktlint-core"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
    ];

    run_language_suite("Kotlin", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  5. C#
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_csharp() {
    let specs = [
        RepoSpec {
            name: "aspnetcore-http",
            url: "https://github.com/dotnet/aspnetcore",
            subdir: Some("src/Http"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "efcore",
            url: "https://github.com/dotnet/efcore",
            subdir: Some("src/EFCore"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "orleans-core",
            url: "https://github.com/dotnet/orleans",
            subdir: Some("src/Orleans.Core"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "newtonsoft-json",
            url: "https://github.com/JamesNK/Newtonsoft.Json",
            subdir: Some("Src/Newtonsoft.Json"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "automapper",
            url: "https://github.com/AutoMapper/AutoMapper",
            subdir: Some("src/AutoMapper"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "mediatr",
            url: "https://github.com/jbogard/MediatR",
            subdir: Some("src/MediatR"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "polly-core",
            url: "https://github.com/App-vNext/Polly",
            subdir: Some("src/Polly.Core"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "serilog",
            url: "https://github.com/serilog/serilog",
            subdir: Some("src/Serilog"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "fluentvalidation",
            url: "https://github.com/FluentValidation/FluentValidation",
            subdir: Some("src/FluentValidation"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "dapper",
            url: "https://github.com/DapperLib/Dapper",
            subdir: Some("Dapper"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 3,
        },
    ];

    run_language_suite("C#", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  6. Go
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_go() {
    let specs = [
        RepoSpec {
            name: "gin",
            url: "https://github.com/gin-gonic/gin",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "fiber",
            url: "https://github.com/gofiber/fiber",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "cobra",
            url: "https://github.com/spf13/cobra",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "viper",
            url: "https://github.com/spf13/viper",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "mux",
            url: "https://github.com/gorilla/mux",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "zap",
            url: "https://github.com/uber-go/zap",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "prometheus-client-go",
            url: "https://github.com/prometheus/client_golang",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "grpc-go",
            url: "https://github.com/grpc/grpc-go",
            subdir: None,
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "chi",
            url: "https://github.com/go-chi/chi",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "echo",
            url: "https://github.com/labstack/echo",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
    ];

    run_language_suite("Go", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  7. PHP
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_php() {
    let specs = [
        RepoSpec {
            name: "laravel-framework",
            url: "https://github.com/laravel/framework",
            subdir: Some("src/Illuminate"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "symfony-http-foundation",
            url: "https://github.com/symfony/symfony",
            subdir: Some("src/Symfony/Component/HttpFoundation"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "composer",
            url: "https://github.com/composer/composer",
            subdir: Some("src/Composer"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "guzzle",
            url: "https://github.com/guzzle/guzzle",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "phpunit",
            url: "https://github.com/sebastianbergmann/phpunit",
            subdir: Some("src"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "monolog",
            url: "https://github.com/Seldaek/monolog",
            subdir: Some("src/Monolog"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "phpstan",
            url: "https://github.com/phpstan/phpstan-src",
            subdir: Some("src"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "pest",
            url: "https://github.com/pestphp/pest",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "filament-forms",
            url: "https://github.com/filamentphp/filament",
            subdir: Some("packages/forms/src"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "livewire",
            url: "https://github.com/livewire/livewire",
            subdir: Some("src"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
    ];

    run_language_suite("PHP", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  8. Ruby
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_ruby() {
    let specs = [
        RepoSpec {
            name: "rails-actionpack",
            url: "https://github.com/rails/rails",
            subdir: Some("actionpack"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "devise",
            url: "https://github.com/heartcombo/devise",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "sidekiq",
            url: "https://github.com/sidekiq/sidekiq",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "rspec-core",
            url: "https://github.com/rspec/rspec-core",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "rubocop",
            url: "https://github.com/rubocop/rubocop",
            subdir: Some("lib"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "grape",
            url: "https://github.com/ruby-grape/grape",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "pundit",
            url: "https://github.com/varvet/pundit",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "jekyll",
            url: "https://github.com/jekyll/jekyll",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "dry-validation",
            url: "https://github.com/dry-rb/dry-validation",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "hanami",
            url: "https://github.com/hanami/hanami",
            subdir: Some("lib"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
    ];

    run_language_suite("Ruby", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  9. Rust
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_rust() {
    let specs = [
        RepoSpec {
            name: "tokio",
            url: "https://github.com/tokio-rs/tokio",
            subdir: Some("tokio"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "serde",
            url: "https://github.com/serde-rs/serde",
            subdir: Some("serde"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "clap",
            url: "https://github.com/clap-rs/clap",
            subdir: Some("clap_builder"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "axum",
            url: "https://github.com/tokio-rs/axum",
            subdir: Some("axum"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "rayon-core",
            url: "https://github.com/rayon-rs/rayon",
            subdir: Some("rayon-core"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "reqwest",
            url: "https://github.com/seanmonstar/reqwest",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "anyhow",
            url: "https://github.com/dtolnay/anyhow",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "tracing",
            url: "https://github.com/tokio-rs/tracing",
            subdir: Some("tracing"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "warp",
            url: "https://github.com/seanmonstar/warp",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "actix-web",
            url: "https://github.com/actix/actix-web",
            subdir: Some("actix-web"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
    ];

    run_language_suite("Rust", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  10. C
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_c() {
    let specs = [
        RepoSpec {
            name: "redis",
            url: "https://github.com/redis/redis",
            subdir: Some("src"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "curl",
            url: "https://github.com/curl/curl",
            subdir: Some("lib"),
            timeout_secs: 180,
            min_symbols: 50,
            min_files: 20,
        },
        RepoSpec {
            name: "jq",
            url: "https://github.com/jqlang/jq",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "sds",
            url: "https://github.com/antirez/sds",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 3,
            min_files: 1,
        },
        RepoSpec {
            name: "zstd",
            url: "https://github.com/facebook/zstd",
            subdir: Some("lib"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "libuv",
            url: "https://github.com/libuv/libuv",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "raylib",
            url: "https://github.com/raysan5/raylib",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 5,
        },
        RepoSpec {
            name: "openssl-crypto",
            url: "https://github.com/openssl/openssl",
            subdir: Some("crypto"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "mbedtls",
            url: "https://github.com/Mbed-TLS/mbedtls",
            subdir: Some("library"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "cJSON",
            url: "https://github.com/DaveGamble/cJSON",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 1,
        },
    ];

    run_language_suite("C", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  11. C++
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_cpp() {
    let specs = [
        RepoSpec {
            name: "nlohmann-json",
            url: "https://github.com/nlohmann/json",
            subdir: Some("include"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "fmt",
            url: "https://github.com/fmtlib/fmt",
            subdir: Some("include"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "spdlog",
            url: "https://github.com/gabime/spdlog",
            subdir: Some("include"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "catch2",
            url: "https://github.com/catchorg/Catch2",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "grpc-core",
            url: "https://github.com/grpc/grpc",
            subdir: Some("src/core"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "abseil-strings",
            url: "https://github.com/abseil/abseil-cpp",
            subdir: Some("absl/strings"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "folly",
            url: "https://github.com/facebook/folly",
            subdir: Some("folly"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "imgui",
            url: "https://github.com/ocornut/imgui",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 3,
        },
        RepoSpec {
            name: "leveldb",
            url: "https://github.com/google/leveldb",
            subdir: None,
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "benchmark",
            url: "https://github.com/google/benchmark",
            subdir: Some("src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
    ];

    run_language_suite("C++", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  12. Swift
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_swift() {
    let specs = [
        RepoSpec {
            name: "vapor",
            url: "https://github.com/vapor/vapor",
            subdir: Some("Sources/Vapor"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "alamofire",
            url: "https://github.com/Alamofire/Alamofire",
            subdir: Some("Source"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "swift-nio-core",
            url: "https://github.com/apple/swift-nio",
            subdir: Some("Sources/NIOCore"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "kingfisher",
            url: "https://github.com/onevcat/Kingfisher",
            subdir: Some("Sources"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "swift-argument-parser",
            url: "https://github.com/apple/swift-argument-parser",
            subdir: Some("Sources/ArgumentParser"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "snapkit",
            url: "https://github.com/SnapKit/SnapKit",
            subdir: Some("Sources"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
        RepoSpec {
            name: "rxswift",
            url: "https://github.com/ReactiveX/RxSwift",
            subdir: Some("RxSwift"),
            timeout_secs: 120,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "swift-log",
            url: "https://github.com/apple/swift-log",
            subdir: Some("Sources/Logging"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 2,
        },
        RepoSpec {
            name: "swift-collections",
            url: "https://github.com/apple/swift-collections",
            subdir: Some("Sources"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "swiftyJSON",
            url: "https://github.com/SwiftyJSON/SwiftyJSON",
            subdir: Some("Source"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 1,
        },
    ];

    run_language_suite("Swift", &specs);
}

// ═══════════════════════════════════════════════════════════════════
//  13. Scala
// ═══════════════════════════════════════════════════════════════════

#[test]
#[ignore]
fn suite_scala() {
    let specs = [
        RepoSpec {
            name: "akka-actor",
            url: "https://github.com/akka/akka",
            subdir: Some("akka-actor/src"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "playframework-core",
            url: "https://github.com/playframework/playframework",
            subdir: Some("core/play/src"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "spark-core",
            url: "https://github.com/apache/spark",
            subdir: Some("core/src"),
            timeout_secs: 300,
            min_symbols: 100,
            min_files: 50,
        },
        RepoSpec {
            name: "cats-core",
            url: "https://github.com/typelevel/cats",
            subdir: Some("core/src"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "zio-core",
            url: "https://github.com/zio/zio",
            subdir: Some("core/shared/src"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "circe-core",
            url: "https://github.com/circe/circe",
            subdir: Some("modules/core/shared/src"),
            timeout_secs: 120,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "http4s-core",
            url: "https://github.com/http4s/http4s",
            subdir: Some("core/shared/src"),
            timeout_secs: 180,
            min_symbols: 10,
            min_files: 5,
        },
        RepoSpec {
            name: "scalatest",
            url: "https://github.com/scalatest/scalatest",
            subdir: Some("jvm/core/src"),
            timeout_secs: 180,
            min_symbols: 30,
            min_files: 15,
        },
        RepoSpec {
            name: "slick",
            url: "https://github.com/slick/slick",
            subdir: Some("slick/src"),
            timeout_secs: 180,
            min_symbols: 20,
            min_files: 10,
        },
        RepoSpec {
            name: "fs2-core",
            url: "https://github.com/typelevel/fs2",
            subdir: Some("core/shared/src"),
            timeout_secs: 120,
            min_symbols: 5,
            min_files: 3,
        },
    ];

    run_language_suite("Scala", &specs);
}
