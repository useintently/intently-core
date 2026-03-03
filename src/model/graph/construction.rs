//! Knowledge graph construction from a `CodeModel`.
//!
//! Implements the 7-step construction algorithm that converts flat extraction
//! data into a directed graph with O(1) adjacency lookups.

use std::collections::HashMap;
use std::path::Path;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::model::types::{CodeModel, ReferenceKind, SymbolKind};

use super::types::{GraphEdge, GraphNode, NodeKey, WeightedEdge};

// ---------------------------------------------------------------------------
// KnowledgeGraph struct
// ---------------------------------------------------------------------------

/// A directed graph derived from the CodeModel.
///
/// Provides O(1) adjacency lookups for callers, callees, type hierarchy,
/// and impact analysis. Supports structural analysis (cycle detection via
/// Tarjan's SCC) for ARC-001 policy enforcement.
pub struct KnowledgeGraph {
    pub(super) graph: DiGraph<GraphNode, WeightedEdge>,
    pub(super) node_index: HashMap<NodeKey, NodeIndex>,
}

impl KnowledgeGraph {
    /// Build a knowledge graph from a CodeModel.
    ///
    /// The 7-step construction algorithm:
    /// 1. Create Module nodes from ModuleBoundary data
    /// 2. Create File nodes from module boundaries + symbol files
    /// 3. Create Symbol nodes with Defines edges from parent files
    /// 4. Create Interface nodes with Defines edges
    /// 5. Create DataModel nodes with Defines edges
    /// 6. Process References → Calls, Extends, Implements, UsesType, Imports edges
    /// 7. Process ModuleBoundaries → Contains, Exposes, DependsOn edges
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use intently_core::{IntentlyEngine, KnowledgeGraph};
    ///
    /// let mut engine = IntentlyEngine::new(PathBuf::from("/path/to/project"));
    /// let result = engine.full_analysis().expect("extraction failed");
    ///
    /// // The graph is built automatically during full_analysis, but you can
    /// // also construct one manually from any CodeModel:
    /// let graph = KnowledgeGraph::from_code_model(&result.model);
    /// let stats = graph.stats();
    /// println!("{} nodes, {} edges", stats.total_nodes, stats.total_edges);
    /// ```
    pub fn from_code_model(model: &CodeModel) -> Self {
        let mut kg = Self {
            graph: DiGraph::new(),
            node_index: HashMap::new(),
        };

        for component in &model.components {
            // Step 1: Module nodes
            for module in &component.module_boundaries {
                kg.ensure_node(NodeKey::Module(module.name.clone()), || GraphNode::Module {
                    name: module.name.clone(),
                });
            }

            // Step 2: File nodes from modules
            for module in &component.module_boundaries {
                for file in &module.files {
                    kg.ensure_node(NodeKey::File(file.clone()), || GraphNode::File {
                        path: file.clone(),
                    });
                }
            }

            // Step 3: Symbol nodes + Defines edges
            for symbol in &component.symbols {
                let file_idx = kg.ensure_node(NodeKey::File(symbol.anchor.file.clone()), || {
                    GraphNode::File {
                        path: symbol.anchor.file.clone(),
                    }
                });
                let sym_idx = kg.ensure_node(
                    NodeKey::Symbol(symbol.name.clone(), symbol.anchor.file.clone()),
                    || GraphNode::Symbol {
                        name: symbol.name.clone(),
                        kind: symbol.kind,
                        file: symbol.anchor.file.clone(),
                        line: symbol.anchor.line,
                    },
                );
                kg.graph.add_edge(
                    file_idx,
                    sym_idx,
                    WeightedEdge::structural(GraphEdge::Defines),
                );
            }

            // Step 4: Interface nodes + Defines edges
            for iface in &component.interfaces {
                let file_idx = kg.ensure_node(NodeKey::File(iface.anchor.file.clone()), || {
                    GraphNode::File {
                        path: iface.anchor.file.clone(),
                    }
                });
                let iface_idx = kg.ensure_node(
                    NodeKey::Interface(
                        iface.method.to_string(),
                        iface.path.clone(),
                        iface.anchor.file.clone(),
                    ),
                    || GraphNode::Interface {
                        method: iface.method,
                        path: iface.path.clone(),
                        file: iface.anchor.file.clone(),
                        line: iface.anchor.line,
                    },
                );
                kg.graph.add_edge(
                    file_idx,
                    iface_idx,
                    WeightedEdge::structural(GraphEdge::Defines),
                );
            }

            // Step 5: DataModel nodes + Defines edges
            for model in &component.data_models {
                let file_idx = kg.ensure_node(NodeKey::File(model.anchor.file.clone()), || {
                    GraphNode::File {
                        path: model.anchor.file.clone(),
                    }
                });
                let model_idx = kg.ensure_node(
                    NodeKey::DataModel(model.name.clone(), model.anchor.file.clone()),
                    || GraphNode::DataModel {
                        name: model.name.clone(),
                        kind: model.model_kind,
                        file: model.anchor.file.clone(),
                        line: model.anchor.line,
                    },
                );
                kg.graph.add_edge(
                    file_idx,
                    model_idx,
                    WeightedEdge::structural(GraphEdge::Defines),
                );
            }

            // Step 6: Process References
            for reference in &component.references {
                match reference.reference_kind {
                    ReferenceKind::Call => {
                        let source_idx = kg.ensure_symbol(
                            &reference.source_symbol,
                            &reference.source_file,
                            reference.source_line,
                        );
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_symbol(
                                &reference.target_symbol,
                                tf,
                                reference.target_line.unwrap_or(0),
                            ),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(
                            source_idx,
                            target_idx,
                            WeightedEdge::from_reference(GraphEdge::Calls, reference.confidence),
                        );
                    }
                    ReferenceKind::Extends => {
                        let source_idx = kg.ensure_symbol(
                            &reference.source_symbol,
                            &reference.source_file,
                            reference.source_line,
                        );
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_symbol(
                                &reference.target_symbol,
                                tf,
                                reference.target_line.unwrap_or(0),
                            ),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(
                            source_idx,
                            target_idx,
                            WeightedEdge::from_reference(GraphEdge::Extends, reference.confidence),
                        );
                    }
                    ReferenceKind::Implements => {
                        let source_idx = kg.ensure_symbol(
                            &reference.source_symbol,
                            &reference.source_file,
                            reference.source_line,
                        );
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_symbol(
                                &reference.target_symbol,
                                tf,
                                reference.target_line.unwrap_or(0),
                            ),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(
                            source_idx,
                            target_idx,
                            WeightedEdge::from_reference(
                                GraphEdge::Implements,
                                reference.confidence,
                            ),
                        );
                    }
                    ReferenceKind::TypeUsage => {
                        let source_idx = kg.ensure_symbol(
                            &reference.source_symbol,
                            &reference.source_file,
                            reference.source_line,
                        );
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_symbol(
                                &reference.target_symbol,
                                tf,
                                reference.target_line.unwrap_or(0),
                            ),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(
                            source_idx,
                            target_idx,
                            WeightedEdge::from_reference(GraphEdge::UsesType, reference.confidence),
                        );
                    }
                    ReferenceKind::Import => {
                        let source_file_idx =
                            kg.ensure_node(NodeKey::File(reference.source_file.clone()), || {
                                GraphNode::File {
                                    path: reference.source_file.clone(),
                                }
                            });
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_node(NodeKey::File(tf.clone()), || {
                                GraphNode::File { path: tf.clone() }
                            }),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(
                            source_file_idx,
                            target_idx,
                            WeightedEdge::from_reference(GraphEdge::Imports, reference.confidence),
                        );
                    }
                }
            }

            // Step 7: Module boundaries → Contains, Exposes, DependsOn
            for module in &component.module_boundaries {
                let module_idx = kg.node_index[&NodeKey::Module(module.name.clone())];

                // Contains: module → file
                for file in &module.files {
                    if let Some(&file_idx) = kg.node_index.get(&NodeKey::File(file.clone())) {
                        kg.graph.add_edge(
                            module_idx,
                            file_idx,
                            WeightedEdge::structural(GraphEdge::Contains),
                        );
                    }
                }

                // Exposes: module → symbol (find matching symbols in module files)
                for sym_name in &module.exported_symbols {
                    // Find any symbol node matching this name in the module's files
                    for file in &module.files {
                        let key = NodeKey::Symbol(sym_name.clone(), file.clone());
                        if let Some(&sym_idx) = kg.node_index.get(&key) {
                            kg.graph.add_edge(
                                module_idx,
                                sym_idx,
                                WeightedEdge::structural(GraphEdge::Exposes),
                            );
                            break;
                        }
                    }
                }

                // DependsOn: module → module
                for dep_name in &module.depends_on {
                    let dep_idx =
                        kg.ensure_node(NodeKey::Module(dep_name.clone()), || GraphNode::Module {
                            name: dep_name.clone(),
                        });
                    kg.graph.add_edge(
                        module_idx,
                        dep_idx,
                        WeightedEdge::structural(GraphEdge::DependsOn),
                    );
                }
            }
        }

        kg
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Get or create a node, returning its index.
    fn ensure_node(&mut self, key: NodeKey, make_node: impl FnOnce() -> GraphNode) -> NodeIndex {
        if let Some(&idx) = self.node_index.get(&key) {
            return idx;
        }
        let node = make_node();
        let idx = self.graph.add_node(node);
        self.node_index.insert(key, idx);
        idx
    }

    /// Get or create a Symbol node.
    fn ensure_symbol(&mut self, name: &str, file: &Path, line: usize) -> NodeIndex {
        self.ensure_node(
            NodeKey::Symbol(name.to_string(), file.to_path_buf()),
            || GraphNode::Symbol {
                name: name.to_string(),
                kind: SymbolKind::Function, // default kind for reference-only symbols
                file: file.to_path_buf(),
                line,
            },
        )
    }

    /// Get or create an External node.
    fn ensure_external(&mut self, name: &str) -> NodeIndex {
        self.ensure_node(NodeKey::External(name.to_string()), || {
            GraphNode::External {
                name: name.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_test_model;
    use super::*;
    use crate::model::types::*;
    use crate::parser::SupportedLanguage;

    #[test]
    fn constructs_from_empty_model() {
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "empty".into(),
            components: vec![Component {
                name: "empty".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![],
                imports: vec![],
                references: vec![],
                data_models: vec![],
                module_boundaries: vec![],
                env_dependencies: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 0,
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
        };

        let kg = KnowledgeGraph::from_code_model(&model);
        assert_eq!(kg.graph.node_count(), 0);
        assert_eq!(kg.graph.edge_count(), 0);
    }

    #[test]
    fn constructs_with_symbols_and_references() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        // Should have nodes for: files, symbols, interface, data model, modules, externals
        assert!(kg.graph.node_count() > 0);
        assert!(kg.graph.edge_count() > 0);

        let stats = kg.stats();
        assert!(stats.node_counts.contains_key("symbol"));
        assert!(stats.node_counts.contains_key("file"));
        assert!(stats.node_counts.contains_key("module"));
        assert!(stats.edge_counts.contains_key("calls"));
        assert!(stats.edge_counts.contains_key("defines"));
    }

    #[test]
    fn deduplicates_nodes_from_multiple_references() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        // getUser appears in two references as source, but should be one node
        let getuser_nodes = kg.find_symbol_nodes("getUser");
        assert_eq!(
            getuser_nodes.len(),
            1,
            "getUser should be deduplicated to one node"
        );
    }

    #[test]
    fn creates_external_nodes_for_unresolved_targets() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert!(
            stats.node_counts.get("external").copied().unwrap_or(0) > 0,
            "should have external nodes for unresolved targets"
        );
    }

    #[test]
    fn creates_module_nodes_with_contains_edges() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert_eq!(
            stats.node_counts.get("module").copied().unwrap_or(0),
            2,
            "should have 2 module nodes"
        );
        assert!(
            stats.edge_counts.get("contains").copied().unwrap_or(0) > 0,
            "should have contains edges"
        );
    }

    #[test]
    fn creates_defines_edges_for_symbols() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("defines").copied().unwrap_or(0) >= 3,
            "should have at least 3 defines edges (3 symbols)"
        );
    }

    #[test]
    fn creates_depends_on_edges_between_modules() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("depends_on").copied().unwrap_or(0) > 0,
            "should have depends_on edge (routes -> services)"
        );
    }

    #[test]
    fn exposes_edge_links_module_to_exported_symbol() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("exposes").copied().unwrap_or(0) > 0,
            "should have exposes edges for exported symbols"
        );
    }

    #[test]
    fn imports_edge_connects_files() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("imports").copied().unwrap_or(0) > 0,
            "should have imports edges"
        );
    }

    #[test]
    fn weighted_edge_structural_has_full_confidence() {
        let edge = WeightedEdge::structural(GraphEdge::Defines);
        assert_eq!(edge.confidence, 1.0);
        assert_eq!(edge.kind, GraphEdge::Defines);
    }

    #[test]
    fn weighted_edge_from_reference_carries_confidence() {
        let edge = WeightedEdge::from_reference(GraphEdge::Calls, 0.85);
        assert_eq!(edge.confidence, 0.85);
        assert_eq!(edge.kind, GraphEdge::Calls);
    }
}
