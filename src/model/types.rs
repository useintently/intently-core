//! Core data types for the CodeModel intermediate representation.
//!
//! These types model a codebase at a semantic level — services, APIs,
//! dependencies, and observable sinks — rather than at the file/line level.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::parser::SupportedLanguage;

// ---------------------------------------------------------------------------
// Resolution confidence types
// ---------------------------------------------------------------------------

/// How a reference target was resolved to a concrete file/symbol.
///
/// Ordered by decreasing confidence. Downstream consumers can filter
/// on resolution method to select only high-quality edges.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionMethod {
    /// Resolved via an explicit import statement (highest confidence).
    ImportBased,
    /// Resolved to a symbol in the same file.
    SameFile,
    /// Resolved to a globally unique symbol name.
    GlobalUnique,
    /// Multiple global matches — resolved to best heuristic pick.
    GlobalAmbiguous,
    /// Could not be resolved to any known symbol.
    #[default]
    Unresolved,
}

/// Precise source location anchoring a semantic fact to the CST.
///
/// Every extracted artifact (route, dependency, sink, symbol, data model) carries
/// a `SourceAnchor` that captures the full tree-sitter node position. This enables:
/// - Code context retrieval (anchor → source text for LLMs)
/// - AST rewriting (anchor → exact node for deterministic patches)
/// - Stable navigation (byte offsets survive line-number drift)
///
/// See ADR-002 for design rationale.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceAnchor {
    /// File containing the anchored node.
    pub file: PathBuf,
    /// 1-based start line.
    pub line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// Byte offset of the node's first byte in the source file.
    pub start_byte: usize,
    /// Byte offset past the node's last byte in the source file.
    pub end_byte: usize,
    /// Tree-sitter CST node type (e.g. `"call_expression"`, `"decorator"`).
    pub node_kind: String,
}

impl SourceAnchor {
    /// Create a minimal anchor from a file and line number.
    ///
    /// Sets `end_line = line`, byte offsets to 0, and `node_kind` to empty.
    /// Useful in tests and consumers that lack tree-sitter node data.
    pub fn from_line(file: PathBuf, line: usize) -> Self {
        Self {
            file,
            line,
            end_line: line,
            start_byte: 0,
            end_byte: 0,
            node_kind: String::new(),
        }
    }

    /// Create an anchor with a line range but no byte-level data.
    ///
    /// Useful for Symbol and DataModel constructions in tests.
    pub fn from_line_range(file: PathBuf, line: usize, end_line: usize) -> Self {
        Self {
            file,
            line,
            end_line,
            start_byte: 0,
            end_byte: 0,
            node_kind: String::new(),
        }
    }
}

/// The CodeModel: a semantic snapshot of the entire codebase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeModel {
    pub version: String,
    pub project_name: String,
    pub components: Vec<Component>,
    pub stats: CodeModelStats,
}

/// A logical component (service, library, module) in the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub name: String,
    pub language: SupportedLanguage,
    pub interfaces: Vec<Interface>,
    pub dependencies: Vec<Dependency>,
    pub sinks: Vec<Sink>,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<ImportInfo>,
    pub references: Vec<Reference>,
    pub data_models: Vec<DataModel>,
    pub module_boundaries: Vec<ModuleBoundary>,
}

/// An HTTP endpoint exposed by a component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interface {
    pub method: HttpMethod,
    pub path: String,
    pub auth: Option<AuthKind>,
    /// Source location of the route definition in the CST.
    #[serde(flatten)]
    pub anchor: SourceAnchor,
}

/// HTTP methods supported by the extractor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Head,
    All,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Patch => write!(f, "PATCH"),
            Self::Delete => write!(f, "DELETE"),
            Self::Options => write!(f, "OPTIONS"),
            Self::Head => write!(f, "HEAD"),
            Self::All => write!(f, "ALL"),
        }
    }
}

/// Kind of authentication detected on an endpoint.
///
/// Different frameworks express auth in different ways:
/// - Express/Gin/Rails: middleware functions in the route handler chain
/// - FastAPI/Flask/Django: decorators on route handler functions
/// - Spring Boot: annotations on controller methods
/// - ASP.NET Core: attributes on action methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// Express/Gin/Rails: `app.get('/x', authMiddleware, handler)`
    Middleware(String),
    /// Python: `@login_required`, `@jwt_required`
    Decorator(String),
    /// Java/Kotlin: `@PreAuthorize`, `@Secured`
    Annotation(String),
    /// C#: `[Authorize]`, `[Authorize(Roles="admin")]`
    Attribute(String),
}

/// An external dependency (HTTP call, DB connection, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    pub target: String,
    pub dependency_type: DependencyType,
    /// Source location of the dependency call in the CST.
    #[serde(flatten)]
    pub anchor: SourceAnchor,
}

/// Type of external dependency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    HttpCall,
}

/// A logging or output sink detected in the source code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Sink {
    pub sink_type: SinkType,
    /// Source location of the log/sink call in the CST.
    #[serde(flatten)]
    pub anchor: SourceAnchor,
    pub text: String,
    pub contains_pii: bool,
}

/// Type of sink.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SinkType {
    Log,
}

/// Visibility/access modifier of a code symbol.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

/// A code symbol (class, function, method, trait, etc.) extracted from source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Source location spanning the full symbol definition in the CST.
    #[serde(flatten)]
    pub anchor: SourceAnchor,
    pub doc: Option<String>,
    /// Full signature text, e.g. `fn foo(x: i32) -> bool`.
    /// LLMs read these natively — structured params would add
    /// complexity for zero value.
    pub signature: Option<String>,
    /// Access modifier. `None` means the language default applies.
    pub visibility: Option<Visibility>,
    /// Enclosing class, module, trait, or impl block name.
    pub parent: Option<String>,
}

/// Kind of code symbol.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Class,
    Function,
    Method,
    Module,
    Interface,
    Trait,
    Enum,
    Struct,
}

// ---------------------------------------------------------------------------
// Knowledge graph types
// ---------------------------------------------------------------------------

/// A reference between two symbols (call, extends, implements, etc.).
///
/// References form the edges of the knowledge graph, connecting symbols
/// across files and modules. The `source_symbol` is the origin (caller,
/// subclass) and `target_symbol` is the destination (callee, superclass).
///
/// Each reference carries a `confidence` score (0.0–1.0) and a
/// `resolution_method` indicating how the target was resolved.
/// Downstream consumers can filter low-confidence edges to reduce noise.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reference {
    /// Enclosing symbol at the call/usage site (e.g., the function that
    /// contains a call expression). Empty string if at module level.
    pub source_symbol: String,
    /// File containing the reference site.
    pub source_file: PathBuf,
    /// 1-based line of the reference site.
    pub source_line: usize,
    /// Target symbol name (callee, parent type, imported name).
    pub target_symbol: String,
    /// File where the target is defined (`None` if external/unresolved).
    pub target_file: Option<PathBuf>,
    /// 1-based line of the target definition (`None` if unresolved).
    pub target_line: Option<usize>,
    /// What kind of relationship this reference represents.
    pub reference_kind: ReferenceKind,
    /// Confidence that this reference is correctly resolved (0.0–1.0).
    ///
    /// 0.0 = unresolved, 1.0 = certain. Import-based: 0.95, same-file: 0.90,
    /// global-unique: 0.80, global-ambiguous: 0.40.
    #[serde(default)]
    pub confidence: f64,
    /// How this reference's target was resolved.
    #[serde(default)]
    pub resolution_method: ResolutionMethod,
}

/// Classification of a reference relationship.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// Function or method call (`foo()`, `obj.bar()`).
    Call,
    /// Class inheritance (`class Foo extends Bar`).
    Extends,
    /// Interface/trait implementation (`implements Baz`, `impl Trait for`).
    Implements,
    /// Type used as parameter, return type, or field type.
    TypeUsage,
    /// Import/require statement (`import { Foo } from './bar'`).
    Import,
}

/// A data model (class, struct, interface) with its fields.
///
/// Data models are the "nouns" of the system. Extracting them with
/// field-level detail lets LLMs understand data shapes without reading
/// the full source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataModel {
    /// Name of the type (e.g., `User`, `OrderItem`).
    pub name: String,
    /// What kind of data model this is.
    pub model_kind: DataModelKind,
    /// Fields/properties of the model.
    pub fields: Vec<FieldInfo>,
    /// Source location spanning the full type definition in the CST.
    #[serde(flatten)]
    pub anchor: SourceAnchor,
    /// Parent type (extends/inherits from).
    pub parent_type: Option<String>,
    /// Implemented interfaces or traits.
    pub implemented_interfaces: Vec<String>,
}

/// Classification of a data model type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DataModelKind {
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Record,
}

/// A single field within a data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldInfo {
    /// Field name (e.g., `email`, `order_id`).
    pub name: String,
    /// Type annotation if present (e.g., `String`, `Option<i32>`).
    pub field_type: Option<String>,
    /// 1-based line number.
    pub line: usize,
    /// Access modifier if detected.
    pub visibility: Option<Visibility>,
}

/// A logical module boundary inferred from directory structure and exports.
///
/// Module boundaries help LLMs understand the high-level architecture:
/// which files belong together, what each module exports, and how
/// modules depend on each other.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleBoundary {
    /// Module name (typically the directory name).
    pub name: String,
    /// Files belonging to this module.
    pub files: Vec<PathBuf>,
    /// Public symbols exported by this module.
    pub exported_symbols: Vec<String>,
    /// Names of modules this one depends on (via imports).
    pub depends_on: Vec<String>,
}

// ---------------------------------------------------------------------------
// Aggregate statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about the code model.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CodeModelStats {
    pub files_analyzed: usize,
    pub total_interfaces: usize,
    pub total_dependencies: usize,
    pub total_sinks: usize,
    pub total_symbols: usize,
    pub total_imports: usize,
    pub total_references: usize,
    pub total_data_models: usize,
    pub total_modules: usize,
    /// Number of references that were resolved to a concrete target.
    #[serde(default)]
    pub resolved_references: usize,
    /// Average confidence across all references (0.0 if no references).
    #[serde(default)]
    pub avg_resolution_confidence: f64,
}

/// Extraction results from a single source file, prior to aggregation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileExtraction {
    pub file: PathBuf,
    pub language: SupportedLanguage,
    pub interfaces: Vec<Interface>,
    pub dependencies: Vec<Dependency>,
    pub sinks: Vec<Sink>,
    pub imports: Vec<ImportInfo>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,
    pub data_models: Vec<DataModel>,
}

/// An import/require statement found in a source file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportInfo {
    pub source: String,
    pub specifiers: Vec<String>,
    pub line: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_model_round_trip_serialization() {
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "test-project".into(),
            components: vec![Component {
                name: "test-service".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![Interface {
                    method: HttpMethod::Get,
                    path: "/api/health".into(),
                    auth: None,
                    anchor: SourceAnchor::from_line(PathBuf::from("src/index.ts"), 10),
                }],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![],
                imports: vec![],
                references: vec![],
                data_models: vec![],
                module_boundaries: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 1,
                total_interfaces: 1,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 0,
                total_imports: 0,
                total_references: 0,
                total_data_models: 0,
                total_modules: 0,
                resolved_references: 0,
                avg_resolution_confidence: 0.0,
            },
        };

        let json = serde_json::to_string(&model).unwrap();
        let deserialized: CodeModel = serde_json::from_str(&json).unwrap();
        assert_eq!(model, deserialized);
    }

    #[test]
    fn file_extraction_round_trip_serialization() {
        let extraction = FileExtraction {
            file: PathBuf::from("src/server.ts"),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![Interface {
                method: HttpMethod::Post,
                path: "/api/users".into(),
                auth: Some(AuthKind::Middleware("authMiddleware".into())),
                anchor: SourceAnchor::from_line(PathBuf::from("src/server.ts"), 15),
            }],
            dependencies: vec![Dependency {
                target: "fetch(\"https://api.example.com\")".into(),
                dependency_type: DependencyType::HttpCall,
                anchor: SourceAnchor::from_line(PathBuf::from("src/server.ts"), 20),
            }],
            sinks: vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("src/server.ts"), 25),
                text: "console.log(user.email)".into(),
                contains_pii: true,
            }],
            imports: vec![ImportInfo {
                source: "express".into(),
                specifiers: vec!["express".into()],
                line: 1,
            }],
            symbols: vec![],
            references: vec![],
            data_models: vec![],
        };

        let json = serde_json::to_string(&extraction).unwrap();
        let deserialized: FileExtraction = serde_json::from_str(&json).unwrap();
        assert_eq!(extraction, deserialized);
    }

    #[test]
    fn interface_with_auth_serialization() {
        let iface = Interface {
            method: HttpMethod::Delete,
            path: "/api/users/:id".into(),
            auth: Some(AuthKind::Middleware("jwtAuth".into())),
            anchor: SourceAnchor::from_line(PathBuf::from("routes.ts"), 42),
        };

        let json = serde_json::to_string(&iface).unwrap();
        assert!(json.contains("DELETE"));
        assert!(json.contains("jwtAuth"));

        let deserialized: Interface = serde_json::from_str(&json).unwrap();
        assert_eq!(iface, deserialized);
    }

    #[test]
    fn http_method_display() {
        assert_eq!(HttpMethod::Get.to_string(), "GET");
        assert_eq!(HttpMethod::Post.to_string(), "POST");
        assert_eq!(HttpMethod::Delete.to_string(), "DELETE");
    }

    #[test]
    fn auth_kind_decorator_serialization() {
        let iface = Interface {
            method: HttpMethod::Post,
            path: "/api/users".into(),
            auth: Some(AuthKind::Decorator("login_required".into())),
            anchor: SourceAnchor::from_line(PathBuf::from("views.py"), 10),
        };

        let json = serde_json::to_string(&iface).unwrap();
        assert!(json.contains("login_required"));
        let deserialized: Interface = serde_json::from_str(&json).unwrap();
        assert_eq!(iface, deserialized);
    }

    #[test]
    fn auth_kind_annotation_serialization() {
        let iface = Interface {
            method: HttpMethod::Get,
            path: "/api/orders".into(),
            auth: Some(AuthKind::Annotation("PreAuthorize".into())),
            anchor: SourceAnchor::from_line(PathBuf::from("OrderController.java"), 25),
        };

        let json = serde_json::to_string(&iface).unwrap();
        assert!(json.contains("PreAuthorize"));
        let deserialized: Interface = serde_json::from_str(&json).unwrap();
        assert_eq!(iface, deserialized);
    }

    #[test]
    fn auth_kind_attribute_serialization() {
        let iface = Interface {
            method: HttpMethod::Delete,
            path: "/api/items/{id}".into(),
            auth: Some(AuthKind::Attribute("Authorize".into())),
            anchor: SourceAnchor::from_line(PathBuf::from("ItemsController.cs"), 30),
        };

        let json = serde_json::to_string(&iface).unwrap();
        assert!(json.contains("Authorize"));
        let deserialized: Interface = serde_json::from_str(&json).unwrap();
        assert_eq!(iface, deserialized);
    }

    #[test]
    fn symbol_round_trip_with_all_fields() {
        let symbol = Symbol {
            name: "process_payment".into(),
            kind: SymbolKind::Method,
            anchor: SourceAnchor::from_line_range(PathBuf::from("src/payments.rs"), 42, 60),
            doc: Some("Process a payment transaction.".into()),
            signature: Some("pub fn process_payment(&self, amount: f64) -> Result<Receipt>".into()),
            visibility: Some(Visibility::Public),
            parent: Some("PaymentService".into()),
        };

        let json = serde_json::to_string(&symbol).unwrap();
        let deserialized: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(symbol, deserialized);

        // Verify serde rename_all works
        assert!(json.contains("\"public\""));
        assert!(json.contains("\"method\""));
    }

    #[test]
    fn symbol_round_trip_with_none_fields() {
        let symbol = Symbol {
            name: "helper".into(),
            kind: SymbolKind::Function,
            anchor: SourceAnchor::from_line_range(PathBuf::from("utils.ts"), 1, 5),
            doc: None,
            signature: None,
            visibility: None,
            parent: None,
        };

        let json = serde_json::to_string(&symbol).unwrap();
        let deserialized: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(symbol, deserialized);
    }

    #[test]
    fn visibility_all_variants_serialization() {
        for (vis, expected) in [
            (Visibility::Public, "\"public\""),
            (Visibility::Private, "\"private\""),
            (Visibility::Protected, "\"protected\""),
            (Visibility::Internal, "\"internal\""),
        ] {
            let json = serde_json::to_string(&vis).unwrap();
            assert_eq!(json, expected);
            let back: Visibility = serde_json::from_str(&json).unwrap();
            assert_eq!(vis, back);
        }
    }

    #[test]
    fn sink_with_pii_serialization() {
        let sink = Sink {
            sink_type: SinkType::Log,
            anchor: SourceAnchor::from_line(PathBuf::from("handler.ts"), 99),
            text: "logger.info(req.body.password)".into(),
            contains_pii: true,
        };

        let json = serde_json::to_string(&sink).unwrap();
        let deserialized: Sink = serde_json::from_str(&json).unwrap();
        assert_eq!(sink, deserialized);
        assert!(deserialized.contains_pii);
    }

    // --- Knowledge graph type tests ---

    #[test]
    fn reference_round_trip_serialization() {
        let reference = Reference {
            source_symbol: "handle_request".into(),
            source_file: PathBuf::from("src/handler.rs"),
            source_line: 42,
            target_symbol: "validate".into(),
            target_file: Some(PathBuf::from("src/validation.rs")),
            target_line: Some(10),
            reference_kind: ReferenceKind::Call,
            confidence: 0.0,
            resolution_method: ResolutionMethod::Unresolved,
        };

        let json = serde_json::to_string(&reference).unwrap();
        assert!(json.contains("\"call\""));
        let deserialized: Reference = serde_json::from_str(&json).unwrap();
        assert_eq!(reference, deserialized);
    }

    #[test]
    fn reference_kind_all_variants_serialization() {
        for (kind, expected) in [
            (ReferenceKind::Call, "\"call\""),
            (ReferenceKind::Extends, "\"extends\""),
            (ReferenceKind::Implements, "\"implements\""),
            (ReferenceKind::TypeUsage, "\"type_usage\""),
            (ReferenceKind::Import, "\"import\""),
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, expected);
            let back: ReferenceKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn reference_with_unresolved_target() {
        let reference = Reference {
            source_symbol: "main".into(),
            source_file: PathBuf::from("src/main.ts"),
            source_line: 5,
            target_symbol: "axios.get".into(),
            target_file: None,
            target_line: None,
            reference_kind: ReferenceKind::Call,
            confidence: 0.0,
            resolution_method: ResolutionMethod::Unresolved,
        };

        let json = serde_json::to_string(&reference).unwrap();
        assert!(json.contains("null"));
        let deserialized: Reference = serde_json::from_str(&json).unwrap();
        assert_eq!(reference, deserialized);
    }

    #[test]
    fn data_model_round_trip_serialization() {
        let model = DataModel {
            name: "User".into(),
            model_kind: DataModelKind::Class,
            fields: vec![
                FieldInfo {
                    name: "id".into(),
                    field_type: Some("number".into()),
                    line: 3,
                    visibility: Some(Visibility::Public),
                },
                FieldInfo {
                    name: "email".into(),
                    field_type: Some("string".into()),
                    line: 4,
                    visibility: Some(Visibility::Private),
                },
            ],
            anchor: SourceAnchor::from_line_range(PathBuf::from("src/models/user.ts"), 2, 10),
            parent_type: Some("BaseEntity".into()),
            implemented_interfaces: vec!["Serializable".into()],
        };

        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("\"class\""));
        assert!(json.contains("BaseEntity"));
        let deserialized: DataModel = serde_json::from_str(&json).unwrap();
        assert_eq!(model, deserialized);
    }

    #[test]
    fn data_model_kind_all_variants_serialization() {
        for (kind, expected) in [
            (DataModelKind::Class, "\"class\""),
            (DataModelKind::Struct, "\"struct\""),
            (DataModelKind::Interface, "\"interface\""),
            (DataModelKind::Trait, "\"trait\""),
            (DataModelKind::Enum, "\"enum\""),
            (DataModelKind::Record, "\"record\""),
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, expected);
            let back: DataModelKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn module_boundary_round_trip_serialization() {
        let module = ModuleBoundary {
            name: "payments".into(),
            files: vec![
                PathBuf::from("src/payments/handler.ts"),
                PathBuf::from("src/payments/service.ts"),
            ],
            exported_symbols: vec!["PaymentService".into(), "processPayment".into()],
            depends_on: vec!["users".into(), "orders".into()],
        };

        let json = serde_json::to_string(&module).unwrap();
        let deserialized: ModuleBoundary = serde_json::from_str(&json).unwrap();
        assert_eq!(module, deserialized);
    }

    #[test]
    fn field_info_with_no_type_or_visibility() {
        let field = FieldInfo {
            name: "data".into(),
            field_type: None,
            line: 7,
            visibility: None,
        };

        let json = serde_json::to_string(&field).unwrap();
        let deserialized: FieldInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(field, deserialized);
    }
}
