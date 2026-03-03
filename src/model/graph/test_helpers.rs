//! Shared test helpers for graph module tests.

use std::path::PathBuf;

use crate::model::types::*;
use crate::parser::SupportedLanguage;

/// Build a minimal code model for testing graph construction and analysis.
///
/// Contains:
/// - 3 symbols: handler, getUser, validate
/// - 1 interface: GET /api/users
/// - 1 data model: User (class)
/// - 5 references: handler→getUser, getUser→validate, getUser→axios.get(ext),
///   AdminService extends UserService, routes.ts imports services.ts
/// - 2 modules: routes (depends on services), services
pub(super) fn make_test_model() -> CodeModel {
    CodeModel {
        version: "1.0".into(),
        project_name: "test".into(),
        components: vec![Component {
            name: "test".into(),
            language: SupportedLanguage::TypeScript,
            interfaces: vec![Interface {
                method: HttpMethod::Get,
                path: "/api/users".into(),
                auth: None,
                anchor: SourceAnchor::from_line(PathBuf::from("src/routes.ts"), 10),
                parameters: vec![],
                handler_name: None,
                request_body_type: None,
            }],
            dependencies: vec![],
            sinks: vec![],
            symbols: vec![
                Symbol {
                    name: "handler".into(),
                    kind: SymbolKind::Function,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/routes.ts"), 5, 20),
                    doc: None,
                    signature: None,
                    visibility: Some(Visibility::Public),
                    parent: None,
                    is_test: false,
                },
                Symbol {
                    name: "getUser".into(),
                    kind: SymbolKind::Method,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/services.ts"), 10, 30),
                    doc: None,
                    signature: None,
                    visibility: Some(Visibility::Public),
                    parent: Some("UserService".into()),
                    is_test: false,
                },
                Symbol {
                    name: "validate".into(),
                    kind: SymbolKind::Function,
                    anchor: SourceAnchor::from_line_range(PathBuf::from("src/utils.ts"), 1, 10),
                    doc: None,
                    signature: None,
                    visibility: None,
                    parent: None,
                    is_test: false,
                },
            ],
            imports: vec![],
            references: vec![
                // handler -> getUser -> validate
                Reference {
                    source_symbol: "handler".into(),
                    source_file: PathBuf::from("src/routes.ts"),
                    source_line: 12,
                    target_symbol: "getUser".into(),
                    target_file: Some(PathBuf::from("src/services.ts")),
                    target_line: Some(10),
                    reference_kind: ReferenceKind::Call,
                    confidence: 0.95,
                    resolution_method: ResolutionMethod::ImportBased,
                    is_test_reference: false,
                },
                Reference {
                    source_symbol: "getUser".into(),
                    source_file: PathBuf::from("src/services.ts"),
                    source_line: 15,
                    target_symbol: "validate".into(),
                    target_file: Some(PathBuf::from("src/utils.ts")),
                    target_line: Some(1),
                    reference_kind: ReferenceKind::Call,
                    confidence: 0.90,
                    resolution_method: ResolutionMethod::SameFile,
                    is_test_reference: false,
                },
                // External call
                Reference {
                    source_symbol: "getUser".into(),
                    source_file: PathBuf::from("src/services.ts"),
                    source_line: 20,
                    target_symbol: "axios.get".into(),
                    target_file: None,
                    target_line: None,
                    reference_kind: ReferenceKind::Call,
                    confidence: 0.0,
                    resolution_method: ResolutionMethod::Unresolved,
                    is_test_reference: false,
                },
                // Type hierarchy: AdminService extends UserService
                Reference {
                    source_symbol: "AdminService".into(),
                    source_file: PathBuf::from("src/admin.ts"),
                    source_line: 1,
                    target_symbol: "UserService".into(),
                    target_file: Some(PathBuf::from("src/services.ts")),
                    target_line: Some(1),
                    reference_kind: ReferenceKind::Extends,
                    confidence: 0.80,
                    resolution_method: ResolutionMethod::GlobalUnique,
                    is_test_reference: false,
                },
                // Import
                Reference {
                    source_symbol: "getUser".into(),
                    source_file: PathBuf::from("src/routes.ts"),
                    source_line: 1,
                    target_symbol: "UserService".into(),
                    target_file: Some(PathBuf::from("src/services.ts")),
                    target_line: Some(1),
                    reference_kind: ReferenceKind::Import,
                    confidence: 0.95,
                    resolution_method: ResolutionMethod::ImportBased,
                    is_test_reference: false,
                },
            ],
            data_models: vec![DataModel {
                name: "User".into(),
                model_kind: DataModelKind::Class,
                fields: vec![FieldInfo {
                    name: "email".into(),
                    field_type: Some("string".into()),
                    line: 3,
                    visibility: Some(Visibility::Public),
                }],
                anchor: SourceAnchor::from_line_range(PathBuf::from("src/models.ts"), 1, 10),
                parent_type: None,
                implemented_interfaces: vec![],
            }],
            module_boundaries: vec![
                ModuleBoundary {
                    name: "routes".into(),
                    files: vec![PathBuf::from("src/routes.ts")],
                    exported_symbols: vec!["handler".into()],
                    depends_on: vec!["services".into()],
                },
                ModuleBoundary {
                    name: "services".into(),
                    files: vec![
                        PathBuf::from("src/services.ts"),
                        PathBuf::from("src/utils.ts"),
                    ],
                    exported_symbols: vec!["getUser".into()],
                    depends_on: vec![],
                },
            ],
            env_dependencies: vec![],
        }],
        stats: CodeModelStats {
            files_analyzed: 4,
            total_interfaces: 1,
            total_dependencies: 0,
            total_sinks: 0,
            total_symbols: 3,
            total_imports: 0,
            total_references: 5,
            total_data_models: 1,
            total_modules: 2,
            resolved_references: 0,
            avg_resolution_confidence: 0.0,
            ..Default::default()
        },
        file_tree: None,
    }
}
