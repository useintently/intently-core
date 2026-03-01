//! Python semantic extraction from tree-sitter CSTs.
//!
//! Handles the Python language family, covering three major frameworks:
//! - **FastAPI**: `@app.get("/path")`, `@router.post("/path")`
//! - **Flask**: `@app.route("/path")`, `@app.get("/path")` (Flask 2.0+)
//! - **Django**: `path("url/", views.handler)` in URL patterns
//!
//! Extracts:
//! - Route definitions from decorators and URL pattern calls
//! - Auth decorator detection (`@login_required`, `@jwt_required`, etc.)
//! - External HTTP calls (requests, httpx)
//! - Log sinks with PII detection

use std::path::Path;

use tree_sitter::{Node, Tree};

use crate::parser::SupportedLanguage;
use crate::model::patterns;
use crate::model::types::*;

use super::common::{self, anchor_from_node, extract_string_value, node_text, truncate_call_text};

/// Extract semantic information from a Python source file.
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
        "decorated_definition" => {
            try_extract_decorated_route(node, source, file_path, extraction);
        }
        "call" => {
            try_extract_django_path(node, source, file_path, extraction);
            try_extract_http_call(node, source, file_path, extraction);
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

/// Try to extract a route from a decorated function definition.
///
/// Handles FastAPI and Flask patterns:
/// ```python
/// @app.get("/users")         # FastAPI
/// @router.post("/orders")    # FastAPI
/// @app.route("/items")       # Flask
/// @login_required            # Auth decorator
/// def handler(): ...
/// ```
fn try_extract_decorated_route(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let mut route_info: Option<(HttpMethod, String, SourceAnchor)> = None;
    let mut auth: Option<AuthKind> = None;

    // Iterate over decorator children
    for i in 0..node.child_count() {
        let child = match node.child(i as u32) {
            Some(c) if c.kind() == "decorator" => c,
            _ => continue,
        };

        // The decorator expression is the first named child (after @ token)
        let expr = match child.named_child(0) {
            Some(e) => e,
            None => continue,
        };

        // Check for route decorator
        if let Some((method, path)) = try_parse_route_decorator(&expr, source) {
            route_info = Some((method, path, anchor_from_node(&child, file_path)));
        }

        // Check for auth decorator
        if auth.is_none() {
            if let Some(auth_kind) = try_parse_auth_decorator(&expr, source) {
                auth = Some(auth_kind);
            }
        }
    }

    if let Some((method, path, anchor)) = route_info {
        extraction.interfaces.push(Interface {
            method,
            path,
            auth,
            anchor,
        });
    }
}

/// Parse a decorator expression to extract route information.
///
/// Returns `Some((method, path))` for route decorators like:
/// - `@app.get("/users")` → (Get, "/users")
/// - `@app.route("/items")` → (All, "/items")
fn try_parse_route_decorator(expr: &Node, source: &str) -> Option<(HttpMethod, String)> {
    // Route decorators are always calls: @app.get("/path")
    if expr.kind() != "call" {
        return None;
    }

    let function = expr.child_by_field_name("function")?;

    // Must be an attribute access: app.get, router.post, etc.
    if function.kind() != "attribute" {
        return None;
    }

    let method_name = node_text(&function.child_by_field_name("attribute")?, source);

    // Determine HTTP method from decorator name
    let http_method = if method_name == "route" {
        // Flask's @app.route() — defaults to ALL
        HttpMethod::All
    } else {
        common::parse_http_method(&method_name)?
    };

    // Extract path from first argument
    let args = expr.child_by_field_name("arguments")?;
    let first_arg = find_first_string_arg(&args, source)?;

    Some((http_method, first_arg))
}

/// Parse a decorator expression to detect auth indicators.
///
/// Handles:
/// - `@login_required` (bare identifier)
/// - `@jwt_required()` (call with no args)
/// - `@permission_classes([IsAuthenticated])` (call with args)
fn try_parse_auth_decorator(expr: &Node, source: &str) -> Option<AuthKind> {
    let name = match expr.kind() {
        "identifier" => node_text(expr, source),
        "call" => {
            let function = expr.child_by_field_name("function")?;
            match function.kind() {
                "identifier" => node_text(&function, source),
                "attribute" => node_text(&function.child_by_field_name("attribute")?, source),
                _ => return None,
            }
        }
        "attribute" => node_text(&expr.child_by_field_name("attribute")?, source),
        _ => return None,
    };

    if patterns::is_auth_indicator(&name) {
        Some(AuthKind::Decorator(name))
    } else {
        None
    }
}

/// Try to extract a Django URL pattern from `path("url/", view)`.
fn try_extract_django_path(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let function = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return,
    };

    // Match `path(...)` or `re_path(...)`
    let func_name = match function.kind() {
        "identifier" => node_text(&function, source),
        _ => return,
    };

    if func_name != "path" && func_name != "re_path" {
        return;
    }

    let args = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    let url_path = match find_first_string_arg(&args, source) {
        Some(p) => {
            // Django paths don't start with / — normalize
            if p.starts_with('/') { p } else { format!("/{p}") }
        }
        None => return,
    };

    extraction.interfaces.push(Interface {
        method: HttpMethod::All,
        path: url_path,
        auth: None,
        anchor: anchor_from_node(node, file_path),
    });
}

/// Try to extract an HTTP call from `requests.get(url)` or `httpx.post(url)`.
fn try_extract_http_call(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let function = match node.child_by_field_name("function") {
        Some(f) if f.kind() == "attribute" => f,
        _ => return,
    };

    let object_name = match function.child_by_field_name("object") {
        Some(obj) if obj.kind() == "identifier" => node_text(&obj, source),
        _ => return,
    };

    let method_name = match function.child_by_field_name("attribute") {
        Some(attr) => node_text(&attr, source),
        None => return,
    };

    // Known HTTP client libraries
    let is_http_client = matches!(object_name.as_str(), "requests" | "httpx")
        && matches!(
            method_name.as_str(),
            "get" | "post" | "put" | "patch" | "delete" | "head" | "options"
        );

    if !is_http_client {
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

/// Find the first string literal argument in an argument list.
fn find_first_string_arg(args_node: &Node, source: &str) -> Option<String> {
    for i in 0..args_node.named_child_count() {
        if let Some(child) = args_node.named_child(i as u32) {
            // Skip keyword arguments — we want positional
            if child.kind() == "keyword_argument" {
                continue;
            }
            let text = node_text(&child, source);
            if let Some(value) = extract_string_value(&text) {
                return Some(value);
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

    fn extract_py(source: &str) -> FileExtraction {
        let path = PathBuf::from("test.py");
        let parsed =
            parser::parse_source(&path, source, SupportedLanguage::Python, None).unwrap();
        extract(&path, source, &parsed.tree, SupportedLanguage::Python)
    }

    #[test]
    fn extracts_fastapi_get_route() {
        let ext = extract_py(r#"
from fastapi import FastAPI
app = FastAPI()

@app.get("/users")
def list_users():
    return []
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Get);
        assert_eq!(ext.interfaces[0].path, "/users");
    }

    #[test]
    fn extracts_fastapi_post_route() {
        let ext = extract_py(r#"
@router.post("/api/orders")
def create_order(order: Order):
    return {"id": 1}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Post);
        assert_eq!(ext.interfaces[0].path, "/api/orders");
    }

    #[test]
    fn extracts_flask_route() {
        let ext = extract_py(r#"
from flask import Flask
app = Flask(__name__)

@app.route("/items")
def list_items():
    return jsonify([])
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::All);
        assert_eq!(ext.interfaces[0].path, "/items");
    }

    #[test]
    fn extracts_django_path() {
        let ext = extract_py(r#"
from django.urls import path
from . import views

urlpatterns = [
    path('api/users/', views.list_users),
    path('api/orders/', views.create_order),
]
"#);
        assert_eq!(ext.interfaces.len(), 2);
        assert_eq!(ext.interfaces[0].path, "/api/users/");
        assert_eq!(ext.interfaces[0].method, HttpMethod::All);
        assert_eq!(ext.interfaces[1].path, "/api/orders/");
    }

    #[test]
    fn detects_login_required_decorator() {
        let ext = extract_py(r#"
@app.get("/api/profile")
@login_required
def get_profile():
    return {"user": "me"}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(
            ext.interfaces[0].auth,
            Some(AuthKind::Decorator("login_required".into()))
        );
    }

    #[test]
    fn detects_jwt_required_decorator() {
        let ext = extract_py(r#"
@app.post("/api/orders")
@jwt_required()
def create_order():
    pass
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert!(ext.interfaces[0].auth.is_some());
    }

    #[test]
    fn no_auth_when_missing() {
        let ext = extract_py(r#"
@app.get("/health")
def health_check():
    return {"status": "ok"}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert!(ext.interfaces[0].auth.is_none());
    }

    #[test]
    fn extracts_requests_http_call() {
        let ext = extract_py(r#"
import requests
response = requests.get("https://api.example.com/data")
"#);
        assert_eq!(ext.dependencies.len(), 1);
        assert_eq!(ext.dependencies[0].dependency_type, DependencyType::HttpCall);
    }

    #[test]
    fn extracts_httpx_http_call() {
        let ext = extract_py(r#"
import httpx
response = httpx.post("https://payment.service/charge", json=payload)
"#);
        assert_eq!(ext.dependencies.len(), 1);
        assert_eq!(ext.dependencies[0].dependency_type, DependencyType::HttpCall);
    }

    #[test]
    fn detects_pii_in_log_sink() {
        let ext = extract_py(r#"
logging.info("User email: %s", user.email)
"#);
        assert_eq!(ext.sinks.len(), 1);
        assert!(ext.sinks[0].contains_pii);
    }

    #[test]
    fn extracts_multiple_routes() {
        let ext = extract_py(r#"
@app.get("/users")
def list_users():
    return []

@app.post("/users")
@auth_required
def create_user(user: User):
    return user

@app.delete("/users/{id}")
@auth_required
def delete_user(id: int):
    pass
"#);
        assert_eq!(ext.interfaces.len(), 3);
        assert!(ext.interfaces[0].auth.is_none());
        assert!(ext.interfaces[1].auth.is_some());
        assert!(ext.interfaces[2].auth.is_some());
    }

    #[test]
    fn realistic_fastapi_file() {
        let ext = extract_py(r#"
from fastapi import FastAPI, Depends
import requests

app = FastAPI()

@app.get("/health")
def health():
    return {"status": "ok"}

@app.post("/api/payments")
@jwt_required()
async def process_payment(payment: PaymentRequest):
    logging.info("Processing payment for: %s", payment.email)
    response = requests.post("https://stripe.api/charge", json=payment.dict())
    logger.info("Payment processed")
    return {"success": True}

@app.get("/api/users")
async def list_users():
    logging.info("Listing users")
    return []
"#);
        assert_eq!(ext.interfaces.len(), 3);
        assert!(ext.interfaces[0].auth.is_none()); // /health
        assert!(ext.interfaces[1].auth.is_some()); // /api/payments
        assert!(ext.interfaces[2].auth.is_none()); // /api/users
        assert_eq!(ext.dependencies.len(), 1); // requests.post
        assert!(ext.sinks.len() >= 2); // logging calls
        assert!(ext.sinks.iter().any(|s| s.contains_pii));
    }
}
