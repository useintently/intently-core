//! Semantic diff computation between two CodeModel states.
//!
//! The diff captures behavioral changes — added/removed endpoints,
//! new dependencies, changed sinks — not textual file changes.

use serde::{Deserialize, Serialize};

use super::types::{CodeModel, Dependency, Interface, Sink};

/// Semantic diff between two CodeModel snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticDiff {
    pub interface_changes: Vec<InterfaceChange>,
    pub dependency_changes: Vec<DependencyChange>,
    pub sink_changes: Vec<SinkChange>,
    pub risk_summary: RiskSummary,
}

/// A change to an HTTP interface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterfaceChange {
    pub change_type: ChangeType,
    pub interface: Interface,
}

/// A change to an external dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyChange {
    pub change_type: ChangeType,
    pub dependency: Dependency,
}

/// A change to a log/output sink.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SinkChange {
    pub change_type: ChangeType,
    pub sink: Sink,
}

/// Type of change in the diff.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Removed,
}

/// Risk assessment derived from the semantic diff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskSummary {
    pub security: RiskLevel,
    pub reliability: RiskLevel,
}

/// Qualitative risk level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Compute the semantic diff between two CodeModel states.
///
/// Matching strategy:
/// - Interfaces matched by (method, path)
/// - Dependencies matched by (target, dependency_type)
/// - Sinks matched by (file, line, text)
pub fn compute_diff(old: &CodeModel, new: &CodeModel) -> SemanticDiff {
    let old_interfaces = collect_interfaces(old);
    let new_interfaces = collect_interfaces(new);

    let interface_changes = diff_by_key(
        &old_interfaces,
        &new_interfaces,
        |i| (i.method, i.path.clone()),
        |change_type, i| InterfaceChange {
            change_type,
            interface: i.clone(),
        },
    );

    let old_deps = collect_dependencies(old);
    let new_deps = collect_dependencies(new);

    let dependency_changes = diff_by_key(
        &old_deps,
        &new_deps,
        |d| (d.target.clone(), d.dependency_type),
        |change_type, d| DependencyChange {
            change_type,
            dependency: d.clone(),
        },
    );

    let old_sinks = collect_sinks(old);
    let new_sinks = collect_sinks(new);

    let sink_changes = diff_by_key(
        &old_sinks,
        &new_sinks,
        |s| (s.anchor.file.clone(), s.anchor.line, s.text.clone()),
        |change_type, s| SinkChange {
            change_type,
            sink: s.clone(),
        },
    );

    let risk_summary = compute_risk(&interface_changes, &sink_changes);

    SemanticDiff {
        interface_changes,
        dependency_changes,
        sink_changes,
        risk_summary,
    }
}

fn collect_interfaces(model: &CodeModel) -> Vec<&Interface> {
    model
        .components
        .iter()
        .flat_map(|c| &c.interfaces)
        .collect()
}

fn collect_dependencies(model: &CodeModel) -> Vec<&Dependency> {
    model
        .components
        .iter()
        .flat_map(|c| &c.dependencies)
        .collect()
}

fn collect_sinks(model: &CodeModel) -> Vec<&Sink> {
    model.components.iter().flat_map(|c| &c.sinks).collect()
}

/// Generic diff computation: items present in `new` but not `old` are Added,
/// items present in `old` but not `new` are Removed.
fn diff_by_key<T, K, C, F, M>(old: &[&T], new: &[&T], key_fn: F, make_change: M) -> Vec<C>
where
    K: Eq + std::hash::Hash,
    F: Fn(&T) -> K,
    M: Fn(ChangeType, &T) -> C,
{
    use std::collections::HashSet;

    let old_keys: HashSet<K> = old.iter().map(|item| key_fn(item)).collect();
    let new_keys: HashSet<K> = new.iter().map(|item| key_fn(item)).collect();

    let mut changes = Vec::new();

    for item in new {
        let k = key_fn(item);
        if !old_keys.contains(&k) {
            changes.push(make_change(ChangeType::Added, item));
        }
    }

    for item in old {
        let k = key_fn(item);
        if !new_keys.contains(&k) {
            changes.push(make_change(ChangeType::Removed, item));
        }
    }

    changes
}

fn compute_risk(interface_changes: &[InterfaceChange], sink_changes: &[SinkChange]) -> RiskSummary {
    let has_unauthed_new_endpoint = interface_changes
        .iter()
        .any(|ic| ic.change_type == ChangeType::Added && ic.interface.auth.is_none());

    let has_new_pii_sink = sink_changes
        .iter()
        .any(|sc| sc.change_type == ChangeType::Added && sc.sink.contains_pii);

    let security = if has_new_pii_sink || has_unauthed_new_endpoint {
        RiskLevel::High
    } else if !interface_changes.is_empty() || !sink_changes.is_empty() {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    let reliability = if !interface_changes.is_empty() {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };

    RiskSummary {
        security,
        reliability,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::model::types::*;
    use crate::parser::SupportedLanguage;

    fn make_model(
        interfaces: Vec<Interface>,
        dependencies: Vec<Dependency>,
        sinks: Vec<Sink>,
    ) -> CodeModel {
        CodeModel {
            version: "1.0".into(),
            project_name: "test".into(),
            components: vec![Component {
                name: "svc".into(),
                language: SupportedLanguage::TypeScript,
                interfaces,
                dependencies,
                sinks,
                symbols: vec![],
                imports: vec![],
                references: vec![],
                data_models: vec![],
                module_boundaries: vec![],
                env_dependencies: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 1,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 0,
                total_imports: 0,
                total_references: 0,
                total_data_models: 0,
                total_modules: 0,
                resolved_references: 0,
                avg_resolution_confidence: 0.0,
                ..Default::default()
            },
            file_tree: None,
        }
    }

    fn endpoint(method: HttpMethod, path: &str, auth: Option<AuthKind>) -> Interface {
        Interface {
            method,
            path: path.into(),
            auth,
            anchor: SourceAnchor::from_line(PathBuf::from("src/index.ts"), 1),
            parameters: vec![],
            handler_name: None,
            request_body_type: None,
        }
    }

    #[test]
    fn no_change_produces_empty_diff() {
        let model = make_model(
            vec![endpoint(HttpMethod::Get, "/health", None)],
            vec![],
            vec![],
        );
        let diff = compute_diff(&model, &model);

        assert!(diff.interface_changes.is_empty());
        assert!(diff.dependency_changes.is_empty());
        assert!(diff.sink_changes.is_empty());
        assert_eq!(diff.risk_summary.security, RiskLevel::Low);
    }

    #[test]
    fn detects_added_endpoint() {
        let old = make_model(vec![], vec![], vec![]);
        let new = make_model(
            vec![endpoint(HttpMethod::Post, "/api/users", None)],
            vec![],
            vec![],
        );
        let diff = compute_diff(&old, &new);

        assert_eq!(diff.interface_changes.len(), 1);
        assert_eq!(diff.interface_changes[0].change_type, ChangeType::Added);
        assert_eq!(diff.interface_changes[0].interface.path, "/api/users");
    }

    #[test]
    fn detects_removed_endpoint() {
        let old = make_model(
            vec![endpoint(HttpMethod::Delete, "/api/users/:id", None)],
            vec![],
            vec![],
        );
        let new = make_model(vec![], vec![], vec![]);
        let diff = compute_diff(&old, &new);

        assert_eq!(diff.interface_changes.len(), 1);
        assert_eq!(diff.interface_changes[0].change_type, ChangeType::Removed);
    }

    #[test]
    fn new_pii_sink_raises_security_risk() {
        let old = make_model(vec![], vec![], vec![]);
        let new = make_model(
            vec![],
            vec![],
            vec![Sink {
                sink_type: SinkType::Log,
                anchor: SourceAnchor::from_line(PathBuf::from("handler.ts"), 10),
                text: "console.log(user.email)".into(),
                contains_pii: true,
            }],
        );
        let diff = compute_diff(&old, &new);

        assert_eq!(diff.sink_changes.len(), 1);
        assert_eq!(diff.risk_summary.security, RiskLevel::High);
    }

    #[test]
    fn unauthed_new_endpoint_raises_security_risk() {
        let old = make_model(vec![], vec![], vec![]);
        let new = make_model(
            vec![endpoint(HttpMethod::Post, "/api/payments", None)],
            vec![],
            vec![],
        );
        let diff = compute_diff(&old, &new);

        assert_eq!(diff.risk_summary.security, RiskLevel::High);
    }

    #[test]
    fn semantic_diff_round_trip_serialization() {
        let diff = SemanticDiff {
            interface_changes: vec![InterfaceChange {
                change_type: ChangeType::Added,
                interface: endpoint(HttpMethod::Get, "/new", None),
            }],
            dependency_changes: vec![],
            sink_changes: vec![],
            risk_summary: RiskSummary {
                security: RiskLevel::Medium,
                reliability: RiskLevel::Low,
            },
        };

        let json = serde_json::to_string(&diff).unwrap();
        let deserialized: SemanticDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(diff, deserialized);
    }
}
