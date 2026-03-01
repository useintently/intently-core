//! Go semantic extraction from tree-sitter CSTs.
//!
//! Handles the Go language family, covering major web frameworks:
//! - **Gin**: `r.GET("/path", handler)`, `r.POST("/path", authMw, handler)`
//! - **Echo**: `e.GET("/path", handler)`
//! - **net/http**: `http.HandleFunc("/path", handler)`
//!
//! Extracts:
//! - Route definitions from framework call patterns
//! - Middleware auth detection (Gin/Echo middleware args)
//! - External HTTP calls (http.Get, http.Post, client.Do)
//! - Log sinks with PII detection

use std::path::Path;

use tree_sitter::{Node, Tree};

use crate::parser::SupportedLanguage;
use crate::model::patterns;
use crate::model::types::*;

use super::common::{self, anchor_from_node, extract_string_value, node_text, truncate_call_text};

/// Go HTTP route methods (uppercase — Gin/Echo convention).
const GO_ROUTE_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "Any",
];

/// Extract semantic information from a Go source file.
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
    if node.kind() == "call_expression" {
        try_extract_route(node, source, file_path, extraction);
        try_extract_http_call(node, source, file_path, extraction);
        common::try_extract_log_sink(node, source, file_path, extraction);
    }

    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            extract_recursive(&child, source, file_path, extraction);
        }
    }
}

/// Try to extract a route from Gin/Echo/net-http patterns.
///
/// Handles:
/// - `r.GET("/users", handler)` — Gin/Echo
/// - `r.POST("/orders", authMiddleware, handler)` — Gin with middleware
/// - `http.HandleFunc("/path", handler)` — net/http
fn try_extract_route(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let function = match node.child_by_field_name("function") {
        Some(f) => f,
        None => return,
    };

    if function.kind() == "selector_expression" {
        // Gin/Echo pattern: r.GET("/path", handler)
        try_extract_gin_route(node, &function, source, file_path, extraction);
    }
}

/// Try to extract a Gin/Echo route: `r.GET("/path", handler)`.
fn try_extract_gin_route(
    node: &Node,
    function: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let field = match function.child_by_field_name("field") {
        Some(f) => f,
        None => return,
    };

    let method_name = node_text(&field, source);

    // Check for HandleFunc (net/http)
    if method_name == "HandleFunc" || method_name == "Handle" {
        try_extract_net_http_route(node, source, file_path, extraction);
        return;
    }

    // Check for Gin/Echo route methods
    if !GO_ROUTE_METHODS.contains(&method_name.as_str()) {
        return;
    }

    let http_method = if method_name == "Any" {
        HttpMethod::All
    } else {
        match common::parse_http_method(&method_name) {
            Some(m) => m,
            None => return,
        }
    };

    let args = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    // First argument is the path
    let mut arg_texts = Vec::new();
    for i in 0..args.named_child_count() {
        if let Some(child) = args.named_child(i as u32) {
            arg_texts.push(node_text(&child, source));
        }
    }

    if arg_texts.is_empty() {
        return;
    }

    let route_path = match extract_string_value(&arg_texts[0]) {
        Some(p) => p,
        None => return,
    };

    // Check for auth middleware in middle arguments (Gin pattern:
    // r.POST("/path", authMw, handler) — last arg is the handler)
    let auth = if arg_texts.len() > 2 {
        detect_go_middleware_auth(&arg_texts[1..arg_texts.len() - 1])
    } else {
        None
    };

    extraction.interfaces.push(Interface {
        method: http_method,
        path: route_path,
        auth,
        anchor: anchor_from_node(node, file_path),
    });
}

/// Try to extract a net/http route: `http.HandleFunc("/path", handler)`.
fn try_extract_net_http_route(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let args = match node.child_by_field_name("arguments") {
        Some(a) => a,
        None => return,
    };

    // First argument is the path
    if let Some(first_arg) = args.named_child(0) {
        let text = node_text(&first_arg, source);
        if let Some(path) = extract_string_value(&text) {
            extraction.interfaces.push(Interface {
                method: HttpMethod::All,
                path,
                auth: None,
                anchor: anchor_from_node(node, file_path),
            });
        }
    }
}

/// Detect auth middleware in Go function arguments.
fn detect_go_middleware_auth(arg_texts: &[String]) -> Option<AuthKind> {
    for arg in arg_texts {
        if patterns::is_auth_indicator(arg) {
            return Some(AuthKind::Middleware(arg.clone()));
        }
    }
    None
}

/// Try to extract an HTTP call from `http.Get(url)`, `http.Post(url)`, etc.
fn try_extract_http_call(
    node: &Node,
    source: &str,
    file_path: &Path,
    extraction: &mut FileExtraction,
) {
    let function = match node.child_by_field_name("function") {
        Some(f) if f.kind() == "selector_expression" => f,
        _ => return,
    };

    let operand = match function.child_by_field_name("operand") {
        Some(o) => node_text(&o, source),
        None => return,
    };

    let field = match function.child_by_field_name("field") {
        Some(f) => node_text(&f, source),
        None => return,
    };

    // http.Get, http.Post, http.Head
    let is_std_http = operand == "http"
        && matches!(field.as_str(), "Get" | "Post" | "Head" | "NewRequest");

    // client.Do, client.Get, etc.
    let is_client = (operand.ends_with("client") || operand.ends_with("Client"))
        && matches!(field.as_str(), "Do" | "Get" | "Post" | "Head");

    // resty.R().Get, etc.
    let is_resty = operand == "resty" || field == "Execute";

    if !is_std_http && !is_client && !is_resty {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::parser;

    fn extract_go(source: &str) -> FileExtraction {
        let path = PathBuf::from("main.go");
        let parsed =
            parser::parse_source(&path, source, SupportedLanguage::Go, None).unwrap();
        extract(&path, source, &parsed.tree, SupportedLanguage::Go)
    }

    #[test]
    fn extracts_gin_get_route() {
        let ext = extract_go(r#"
package main

func main() {
    r := gin.Default()
    r.GET("/users", listUsers)
}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Get);
        assert_eq!(ext.interfaces[0].path, "/users");
    }

    #[test]
    fn extracts_gin_post_with_auth_middleware() {
        let ext = extract_go(r#"
package main

func main() {
    r := gin.Default()
    r.POST("/api/orders", authMiddleware, createOrder)
}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::Post);
        assert!(ext.interfaces[0].auth.is_some());
    }

    #[test]
    fn extracts_net_http_handle_func() {
        let ext = extract_go(r#"
package main

import "net/http"

func main() {
    http.HandleFunc("/api/data", dataHandler)
}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert_eq!(ext.interfaces[0].method, HttpMethod::All);
        assert_eq!(ext.interfaces[0].path, "/api/data");
    }

    #[test]
    fn extracts_http_get_call() {
        let ext = extract_go(r#"
package main

import "net/http"

func fetch() {
    resp, err := http.Get("https://api.example.com/data")
    _ = resp
    _ = err
}
"#);
        assert_eq!(ext.dependencies.len(), 1);
        assert_eq!(ext.dependencies[0].dependency_type, DependencyType::HttpCall);
    }

    #[test]
    fn no_auth_when_no_middleware() {
        let ext = extract_go(r#"
package main

func main() {
    r := gin.Default()
    r.GET("/health", healthCheck)
}
"#);
        assert_eq!(ext.interfaces.len(), 1);
        assert!(ext.interfaces[0].auth.is_none());
    }

    #[test]
    fn no_false_positives_on_regular_selectors() {
        let ext = extract_go(r#"
package main

func main() {
    config.Set("key", "value")
    db.Query("SELECT 1")
}
"#);
        assert!(ext.interfaces.is_empty());
        assert!(ext.dependencies.is_empty());
    }

    #[test]
    fn detects_pii_in_log() {
        let ext = extract_go(r#"
package main

import "log"

func handle() {
    log.Printf("User email: %s", user.email)
}
"#);
        assert!(ext.sinks.iter().any(|s| s.contains_pii));
    }

    #[test]
    fn realistic_gin_server() {
        let ext = extract_go(r#"
package main

import (
    "log"
    "net/http"
    "github.com/gin-gonic/gin"
)

func main() {
    r := gin.Default()

    r.GET("/health", func(c *gin.Context) {
        c.JSON(200, gin.H{"status": "ok"})
    })

    r.POST("/api/payments", authMiddleware, func(c *gin.Context) {
        log.Printf("Processing payment for: %s", c.email)
        resp, _ := http.Post("https://stripe.api/charge", "application/json", nil)
        _ = resp
    })

    r.GET("/api/users", listUsers)
}
"#);
        assert_eq!(ext.interfaces.len(), 3);
        assert!(ext.interfaces[0].auth.is_none()); // /health
        assert!(ext.interfaces[1].auth.is_some()); // /api/payments
        assert_eq!(ext.dependencies.len(), 1); // http.Post
        assert!(ext.sinks.len() >= 1); // log.Printf
    }
}
