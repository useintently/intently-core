//! PHP semantic extraction from tree-sitter CSTs.
//!
//! Handles the PHP language family, focusing on Laravel:
//!
//! Extracts:
//! - `Route::get()`, `Route::post()`, etc. route definitions
//! - `->middleware('auth')` auth detection
//! - `Http::get()`, Guzzle HTTP client calls
//! - Log sinks with PII detection

use std::path::Path;

use tree_sitter::{Node, Tree};

use crate::parser::SupportedLanguage;
use crate::model::patterns;
use crate::model::types::*;

use super::common::{self, anchor_from_node, extract_string_value, node_text, truncate_call_text};

/// Extract semantic information from a PHP source file.
pub fn extract(
    file_path: &Path,
    source: &str,
    tree: &Tree,
    language: SupportedLanguage,
) -> FileExtraction {
    let root = tree.root_node();
    let mut extraction = common::new_extraction(file_path, language);

    extract_recursive(&root, source, file_path, &mut extraction);

    extraction
}

fn extract_recursive(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    match node.kind() {
        // Laravel routes: Route::get('/path', ...)
        "scoped_call_expression" => {
            try_extract_laravel_route(node, source, file_path, extraction);
            try_extract_http_facade_call(node, source, file_path, extraction);
            common::try_extract_log_sink(node, source, file_path, extraction);
        }
        // Method chains: ->middleware('auth')
        "member_call_expression" => {
            common::try_extract_log_sink(node, source, file_path, extraction);
        }
        "function_call_expression" => {
            common::try_extract_log_sink(node, source, file_path, extraction);
        }
        _ => {}
    }

    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            extract_recursive(&child, source, file_path, extraction);
        }
    }
}

/// Try to extract a Laravel route from `Route::get('/path', ...)`.
///
/// Handles:
/// ```php
/// Route::get('/users', [UserController::class, 'index']);
/// Route::post('/orders', [OrderController::class, 'store'])->middleware('auth');
/// ```
fn try_extract_laravel_route(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    // Check if this is Route::method(...)
    let scope = match node.child_by_field_name("scope") {
        Some(s) => s,
        None => return,
    };

    let scope_name = node_text(&scope, source);
    if scope_name != "Route" {
        return;
    }

    let method_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };

    let method_name = node_text(&method_node, source);

    // Handle Route::resource() — expands to 7 RESTful routes
    // Handle Route::apiResource() — expands to 5 RESTful routes (no create/edit)
    if method_name == "resource" || method_name == "apiResource" {
        let args = match node.child_by_field_name("arguments") {
            Some(a) => a,
            None => return,
        };
        if let Some(resource_path) = find_first_string_arg(&args, source) {
            let is_api = method_name == "apiResource";
            let auth = detect_middleware_chain(node, source);
            expand_resource_routes(&resource_path, is_api, file_path, node, auth, extraction);
        }
        return;
    }

    // Handle Route::any() — matches all HTTP methods
    if method_name == "any" {
        let args = match node.child_by_field_name("arguments") {
            Some(a) => a,
            None => return,
        };
        let route_path = match find_first_string_arg(&args, source) {
            Some(p) => p,
            None => return,
        };
        let auth = detect_middleware_chain(node, source);
        extraction.interfaces.push(Interface {
            method: HttpMethod::All,
            path: route_path,
            auth,
            anchor: anchor_from_node(node, file_path),
        });
        return;
    }

    let http_method = match common::parse_http_method(&method_name) {
        Some(m) => m,
        None => return,
    };

    // Extract path from first argument
    let args = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    let route_path = match find_first_string_arg(&args, source) {
        Some(p) => p,
        None => return,
    };

    // Check for ->middleware('auth') chain
    let auth = detect_middleware_chain(node, source);

    extraction.interfaces.push(Interface {
        method: http_method,
        path: route_path,
        auth,
        anchor: anchor_from_node(node, file_path),
    });
}

/// Expand `Route::resource()` or `Route::apiResource()` into individual routes.
///
/// Laravel resource routes follow a standard convention:
/// - `resource`: index, create, store, show, edit, update, destroy (7 routes)
/// - `apiResource`: index, store, show, update, destroy (5 routes — no create/edit)
fn expand_resource_routes(
    base_path: &str,
    api_only: bool,
    file_path: &Path,
    node: &Node,
    auth: Option<AuthKind>,
    extraction: &mut FileExtraction,
) {
    let base = base_path.trim_matches('/');
    // Infer singular form by stripping trailing 's' (simple heuristic)
    let singular = if base.ends_with('s') && base.len() > 1 {
        &base[..base.len() - 1]
    } else {
        base
    };
    let param = format!("{{{singular}}}");
    let anchor = anchor_from_node(node, file_path);

    // Standard resource routes
    let routes: &[(&str, &str)] = if api_only {
        &[
            ("get", ""),             // index
            ("post", ""),            // store
            ("get", &param),         // show
            ("put", &param),         // update
            ("delete", &param),      // destroy
        ]
    } else {
        &[
            ("get", ""),             // index
            ("get", "create"),       // create
            ("post", ""),            // store
            ("get", &param),         // show
            ("get", "edit"),         // edit (simplified — actual is {param}/edit)
            ("put", &param),         // update
            ("delete", &param),      // destroy
        ]
    };

    for (method_str, suffix) in routes {
        let method = common::parse_http_method(method_str).unwrap();
        let path = if suffix.is_empty() {
            format!("/{base}")
        } else {
            format!("/{base}/{suffix}")
        };

        extraction.interfaces.push(Interface {
            method,
            path,
            auth: auth.clone(),
            anchor: anchor.clone(),
        });
    }
}

/// Detect `->middleware('auth')` in the parent chain.
///
/// Laravel routes can chain middleware:
/// ```php
/// Route::post('/orders', ...)->middleware('auth');
/// ```
fn detect_middleware_chain(node: &Node, source: &str) -> Option<AuthKind> {
    // Check if this node is the object of a member_call_expression
    // that calls ->middleware('auth')
    let parent = node.parent()?;

    if parent.kind() == "member_call_expression" {
        let full_text = node_text(&parent, source);
        if full_text.contains("middleware") {
            // Extract the middleware name from the full text
            if let Some(mw_name) = extract_middleware_name(&full_text) {
                if patterns::is_auth_indicator(&mw_name) {
                    return Some(AuthKind::Middleware(mw_name));
                }
            }
        }
    }

    None
}

/// Extract middleware name from text like `->middleware('auth')`.
fn extract_middleware_name(text: &str) -> Option<String> {
    let idx = text.find("middleware(")?;
    let rest = &text[idx + "middleware(".len()..];
    // Find the first quoted string
    for quote in ['\'', '"'] {
        if let Some(start) = rest.find(quote) {
            if let Some(end) = rest[start + 1..].find(quote) {
                return Some(rest[start + 1..start + 1 + end].to_string());
            }
        }
    }
    None
}

/// Try to extract an HTTP call from `Http::get(url)`.
fn try_extract_http_facade_call(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let scope = match node.child_by_field_name("scope") {
        Some(s) => s,
        None => return,
    };

    let scope_name = node_text(&scope, source);
    if scope_name != "Http" && scope_name != "Guzzle" {
        return;
    }

    let method_node = match node.child_by_field_name("name") {
        Some(n) => n,
        None => return,
    };

    let method_name = node_text(&method_node, source);
    if !matches!(
        method_name.as_str(),
        "get" | "post" | "put" | "patch" | "delete" | "head" | "request"
    ) {
        return;
    }

    let call_text = node_text(node, source);
    let display_text = truncate_call_text(call_text, 100);

    extraction.dependencies.push(Dependency {
        target: display_text,
        dependency_type: DependencyType::HttpCall,
        anchor: anchor_from_node(node, file_path),
    });
}

/// Find the first string literal argument in a PHP argument list.
fn find_first_string_arg(args_node: &Node, source: &str) -> Option<String> {
    for i in 0..args_node.named_child_count() {
        if let Some(child) = args_node.named_child(i as u32) {
            // PHP argument node might wrap the expression
            let text = if child.kind() == "argument" {
                // Get the inner expression
                child
                    .named_child(0)
                    .map(|inner| node_text(&inner, source))
                    .unwrap_or_else(|| node_text(&child, source))
            } else {
                node_text(&child, source)
            };

            if let Some(value) = extract_string_value(&text) {
                return Some(value);
            }
            // PHP also has encapsed_string (double-quoted with interpolation)
            // For simple cases, try stripping quotes directly
            let trimmed = text.trim();
            if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
                || (trimmed.starts_with('"') && trimmed.ends_with('"'))
            {
                return Some(common::strip_quotes(trimmed));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::parser;

    fn extract_php(source: &str) -> FileExtraction {
        let path = PathBuf::from("routes.php");
        let parsed =
            parser::parse_source(&path, source, SupportedLanguage::Php, None).unwrap();
        extract(&path, source, &parsed.tree, SupportedLanguage::Php)
    }

    #[test]
    fn extracts_laravel_get_route() {
        let ext = extract_php(r#"<?php
Route::get('/users', [UserController::class, 'index']);
?>"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Get);
        assert_eq!(ext.interfaces[0].path, "/users");
    }

    #[test]
    fn extracts_laravel_post_route() {
        let ext = extract_php(r#"<?php
Route::post('/api/orders', [OrderController::class, 'store']);
?>"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Post);
        assert_eq!(ext.interfaces[0].path, "/api/orders");
    }

    #[test]
    fn detects_middleware_auth() {
        let ext = extract_php(r#"<?php
Route::post('/api/orders', [OrderController::class, 'store'])->middleware('auth');
?>"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert!(ext.interfaces[0].auth.is_some());
    }

    #[test]
    fn no_auth_when_no_middleware() {
        let ext = extract_php(r#"<?php
Route::get('/health', function () { return 'ok'; });
?>"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert!(ext.interfaces[0].auth.is_none());
    }

    #[test]
    fn extracts_http_facade_call() {
        let ext = extract_php(r#"<?php
$response = Http::get('https://api.example.com/data');
?>"#);
        assert_eq!(ext.dependencies.len(), 1);
        assert_eq!(ext.dependencies[0].dependency_type, DependencyType::HttpCall);
    }

    #[test]
    fn detects_pii_in_log() {
        let ext = extract_php(r#"<?php
Log::info("User email: " . $user->email);
?>"#);
        assert!(ext.sinks.iter().any(|s| s.contains_pii));
    }

    #[test]
    fn extracts_multiple_routes() {
        let ext = extract_php(r#"<?php
Route::get('/users', [UserController::class, 'index']);
Route::post('/users', [UserController::class, 'store']);
Route::delete('/users/{id}', [UserController::class, 'destroy']);
?>"#);
        assert_eq!(ext.interfaces.len(), 3);
    }

    // --- Resource routes ---

    #[test]
    fn extracts_laravel_resource_routes() {
        let ext = extract_php(r#"<?php
Route::resource('/photos', PhotoController::class);
?>"#);
        assert_eq!(ext.interfaces.len(), 7, "resource() expands to 7 routes");
    }

    #[test]
    fn extracts_laravel_api_resource_routes() {
        let ext = extract_php(r#"<?php
Route::apiResource('/posts', PostController::class);
?>"#);
        assert_eq!(ext.interfaces.len(), 5, "apiResource() expands to 5 routes");
    }

    #[test]
    fn extracts_laravel_any_route() {
        let ext = extract_php(r#"<?php
Route::any('/webhook', WebhookController::class);
?>"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::All);
        assert_eq!(ext.interfaces[0].path, "/webhook");
    }

    #[test]
    fn resource_routes_with_middleware() {
        let ext = extract_php(r#"<?php
Route::resource('/photos', PhotoController::class)->middleware('auth');
?>"#);
        assert_eq!(ext.interfaces.len(), 7, "resource() expands to 7 routes");
        assert!(
            ext.interfaces.iter().all(|i| i.auth.is_some()),
            "all resource routes inherit middleware auth"
        );
    }

    #[test]
    fn resource_route_paths_are_correct() {
        let ext = extract_php(r#"<?php
Route::resource('/photos', PhotoController::class);
?>"#);
        let paths: Vec<&str> = ext.interfaces.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"/photos"), "index");
        assert!(paths.contains(&"/photos/create"), "create");
        assert!(paths.contains(&"/photos/{photo}"), "show (singular param)");
        assert!(paths.contains(&"/photos/edit"), "edit");
    }

    #[test]
    fn realistic_laravel_routes() {
        let ext = extract_php(r#"<?php
use Illuminate\Support\Facades\Route;

Route::get('/health', function () {
    return response()->json(['status' => 'ok']);
});

Route::post('/api/payments', [PaymentController::class, 'charge'])->middleware('auth');

Route::get('/api/products', [ProductController::class, 'index']);

$response = Http::post('https://stripe.api/charge', $data);
Log::info("Processing payment for: " . $request->email);
?>"#);
        assert_eq!(ext.interfaces.len(), 3);
        assert!(ext.interfaces[0].auth.is_none()); // /health
        assert!(ext.interfaces[1].auth.is_some()); // /api/payments
        assert_eq!(ext.dependencies.len(), 1); // Http::post
        assert!(ext.sinks.len() >= 1); // Log::info
    }
}
