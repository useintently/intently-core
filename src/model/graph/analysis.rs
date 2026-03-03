//! Graph traversal and structural analysis algorithms.
//!
//! Implements callers/callees traversal, impact analysis with confidence-weighted
//! BFS, type hierarchy traversal, cycle detection (Tarjan's SCC), statistics,
//! and JSON export for visualization tools.

use std::collections::HashMap;

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use super::construction::KnowledgeGraph;
use super::types::{
    AffectedNode, Cycle, GraphEdge, GraphNode, GraphStats, HierarchyDirection, ImpactResult,
    NodeKey, TraversalEntry, WeightedEdge,
};

impl KnowledgeGraph {
    // -----------------------------------------------------------------------
    // Traversal methods
    // -----------------------------------------------------------------------

    /// Find direct and transitive callers of a symbol via BFS.
    ///
    /// Follows `Calls` edges in the **incoming** direction.
    /// Returns entries ordered by BFS depth (nearest first).
    pub fn callers(&self, symbol_name: &str, max_depth: usize) -> Vec<TraversalEntry> {
        self.bfs_by_edge(
            symbol_name,
            max_depth,
            Direction::Incoming,
            &[GraphEdge::Calls],
        )
    }

    /// Find direct and transitive callees of a symbol via BFS.
    ///
    /// Follows `Calls` edges in the **outgoing** direction.
    pub fn callees(&self, symbol_name: &str, max_depth: usize) -> Vec<TraversalEntry> {
        self.bfs_by_edge(
            symbol_name,
            max_depth,
            Direction::Outgoing,
            &[GraphEdge::Calls],
        )
    }

    /// Analyze the blast radius of changing a symbol.
    ///
    /// Multi-edge-type BFS: traverses Calls (incoming), Extends, Implements,
    /// and UsesType edges to find everything that could break if this symbol
    /// changes.
    ///
    /// Paths where the product of edge confidences drops below the internal
    /// threshold (0.1) are pruned, reducing noise from chains of heuristic
    /// resolutions.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use intently_core::{IntentlyEngine, KnowledgeGraph};
    ///
    /// let mut engine = IntentlyEngine::new(PathBuf::from("/path/to/project"));
    /// let result = engine.full_analysis().expect("extraction failed");
    /// let graph = KnowledgeGraph::from_code_model(&result.model);
    ///
    /// // Find everything affected by changing "UserService" (up to 5 hops)
    /// let impact = graph.impact_analysis("UserService", 5);
    /// println!("Blast radius: {} affected nodes across {} files",
    ///     impact.total_affected, impact.affected_files.len());
    ///
    /// for node in &impact.affected_nodes {
    ///     println!("  depth={} confidence={:.2} {:?}",
    ///         node.depth, node.confidence, node.node.display_name());
    /// }
    /// ```
    ///
    /// Minimum cumulative confidence for a path to be included in impact analysis.
    const IMPACT_CONFIDENCE_THRESHOLD: f64 = 0.1;

    pub fn impact_analysis(&self, symbol_name: &str, max_depth: usize) -> ImpactResult {
        let start_indices = self.find_symbol_nodes(symbol_name);
        if start_indices.is_empty() {
            return ImpactResult {
                root: symbol_name.to_string(),
                affected_nodes: vec![],
                affected_files: vec![],
                affected_modules: vec![],
                total_affected: 0,
            };
        }

        let impact_edges = [
            GraphEdge::Calls,
            GraphEdge::Extends,
            GraphEdge::Implements,
            GraphEdge::UsesType,
        ];

        // BFS queue carries (node_index, depth, cumulative_confidence)
        let mut visited: HashMap<NodeIndex, bool> = HashMap::new();
        let mut queue: std::collections::VecDeque<(NodeIndex, usize, f64)> =
            std::collections::VecDeque::new();
        let mut affected: Vec<AffectedNode> = Vec::new();

        for &idx in &start_indices {
            visited.insert(idx, true);
            queue.push_back((idx, 0, 1.0));
        }

        while let Some((current, depth, cumulative_conf)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // Check incoming edges (who depends on this node?)
            for edge in self.graph.edges_directed(current, Direction::Incoming) {
                if !impact_edges.contains(&edge.weight().kind) {
                    continue;
                }
                let neighbor = edge.source();
                if visited.contains_key(&neighbor) {
                    continue;
                }

                // Accumulate confidence along the path
                let path_confidence = cumulative_conf * edge.weight().confidence;

                // Prune low-confidence paths
                if path_confidence < Self::IMPACT_CONFIDENCE_THRESHOLD {
                    continue;
                }

                visited.insert(neighbor, true);
                affected.push(AffectedNode {
                    node: self.graph[neighbor].clone(),
                    depth: depth + 1,
                    edge_type: edge.weight().kind.clone(),
                    confidence: path_confidence,
                });
                queue.push_back((neighbor, depth + 1, path_confidence));
            }
        }

        // Collect unique affected files and modules
        let mut files: Vec<std::path::PathBuf> = affected
            .iter()
            .filter_map(|a| a.node.file_path().cloned())
            .collect();
        files.sort();
        files.dedup();

        let mut modules: Vec<String> = Vec::new();
        for a in &affected {
            if let GraphNode::Module { name } = &a.node {
                modules.push(name.clone());
            }
        }
        modules.sort();
        modules.dedup();

        let total = affected.len();
        ImpactResult {
            root: symbol_name.to_string(),
            affected_nodes: affected,
            affected_files: files,
            affected_modules: modules,
            total_affected: total,
        }
    }

    /// Traverse the type hierarchy for a given type.
    ///
    /// `Ancestors` follows Extends/Implements edges outgoing from the type.
    /// `Descendants` follows Extends/Implements edges incoming to the type.
    pub fn type_hierarchy(
        &self,
        type_name: &str,
        direction: HierarchyDirection,
    ) -> Vec<TraversalEntry> {
        let hierarchy_edges = [GraphEdge::Extends, GraphEdge::Implements];
        let mut results = Vec::new();

        if direction == HierarchyDirection::Ancestors || direction == HierarchyDirection::Both {
            results.extend(self.bfs_by_edge(
                type_name,
                10, // reasonable max depth for inheritance chains
                Direction::Outgoing,
                &hierarchy_edges,
            ));
        }

        if direction == HierarchyDirection::Descendants || direction == HierarchyDirection::Both {
            results.extend(self.bfs_by_edge(type_name, 10, Direction::Incoming, &hierarchy_edges));
        }

        results
    }

    // -----------------------------------------------------------------------
    // Structural analysis
    // -----------------------------------------------------------------------

    /// Detect circular dependencies using Tarjan's SCC algorithm.
    ///
    /// Returns only cycles (SCCs with size > 1). Each cycle includes
    /// the participating nodes and any module names involved.
    pub fn find_cycles(&self) -> Vec<Cycle> {
        let sccs = tarjan_scc(&self.graph);
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let nodes: Vec<GraphNode> =
                    scc.iter().map(|&idx| self.graph[idx].clone()).collect();
                let modules: Vec<String> = nodes
                    .iter()
                    .filter_map(|n| {
                        if let GraphNode::Module { name } = n {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Cycle { nodes, modules }
            })
            .collect()
    }

    /// Detect module-level circular dependencies only.
    ///
    /// Builds a subgraph of Module nodes + DependsOn edges, then runs
    /// Tarjan's SCC. More focused than `find_cycles()` for ARC-001.
    pub fn find_module_cycles(&self) -> Vec<Cycle> {
        // Build a subgraph of only Module nodes and DependsOn edges
        let mut sub = DiGraph::<GraphNode, WeightedEdge>::new();
        let mut sub_index: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for idx in self.graph.node_indices() {
            if matches!(self.graph[idx], GraphNode::Module { .. }) {
                let new_idx = sub.add_node(self.graph[idx].clone());
                sub_index.insert(idx, new_idx);
            }
        }

        for edge in self.graph.edge_references() {
            if edge.weight().kind == GraphEdge::DependsOn {
                if let (Some(&src), Some(&tgt)) =
                    (sub_index.get(&edge.source()), sub_index.get(&edge.target()))
                {
                    sub.add_edge(src, tgt, WeightedEdge::structural(GraphEdge::DependsOn));
                }
            }
        }

        let sccs = tarjan_scc(&sub);
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let nodes: Vec<GraphNode> = scc.iter().map(|&idx| sub[idx].clone()).collect();
                let modules: Vec<String> = nodes
                    .iter()
                    .filter_map(|n| {
                        if let GraphNode::Module { name } = n {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Cycle { nodes, modules }
            })
            .collect()
    }

    /// Compute graph-wide statistics.
    pub fn stats(&self) -> GraphStats {
        let mut node_counts: HashMap<String, usize> = HashMap::new();
        for idx in self.graph.node_indices() {
            *node_counts
                .entry(self.graph[idx].type_name().to_string())
                .or_default() += 1;
        }

        let mut edge_counts: HashMap<String, usize> = HashMap::new();
        for edge in self.graph.edge_references() {
            *edge_counts
                .entry(edge.weight().kind.type_name().to_string())
                .or_default() += 1;
        }

        let connected_components = petgraph::algo::connected_components(&self.graph);
        let cycles_detected = self.find_cycles().len();

        GraphStats {
            total_nodes: self.graph.node_count(),
            total_edges: self.graph.edge_count(),
            node_counts,
            edge_counts,
            connected_components,
            cycles_detected,
        }
    }

    /// Export the graph as JSON suitable for visualization tools (D3, Sigma.js).
    ///
    /// Produces a standard `{ nodes: [...], edges: [...] }` format.
    pub fn to_json(&self) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = self
            .graph
            .node_indices()
            .map(|idx| {
                let node = &self.graph[idx];
                serde_json::json!({
                    "id": idx.index(),
                    "label": node.display_name(),
                    "data": node,
                })
            })
            .collect();

        let edges: Vec<serde_json::Value> = self
            .graph
            .edge_references()
            .map(|edge| {
                serde_json::json!({
                    "source": edge.source().index(),
                    "target": edge.target().index(),
                    "type": edge.weight().kind.type_name(),
                    "confidence": edge.weight().confidence,
                })
            })
            .collect();

        let stats = self.stats();

        serde_json::json!({
            "nodes": nodes,
            "edges": edges,
            "stats": {
                "total_nodes": stats.total_nodes,
                "total_edges": stats.total_edges,
                "node_counts": stats.node_counts,
                "edge_counts": stats.edge_counts,
                "connected_components": stats.connected_components,
                "cycles_detected": stats.cycles_detected,
            },
        })
    }

    // -----------------------------------------------------------------------
    // Crate-internal accessors for graph_analysis
    // -----------------------------------------------------------------------

    /// Iterate over all (node_index, node) pairs.
    pub(crate) fn node_iter(&self) -> impl Iterator<Item = (NodeIndex, &GraphNode)> {
        self.graph
            .node_indices()
            .map(move |idx| (idx, &self.graph[idx]))
    }

    /// Get the in-degree and out-degree of a node.
    pub(crate) fn node_degree(&self, idx: NodeIndex) -> (usize, usize) {
        let in_deg = self.graph.edges_directed(idx, Direction::Incoming).count();
        let out_deg = self.graph.edges_directed(idx, Direction::Outgoing).count();
        (in_deg, out_deg)
    }

    /// Get a node by index.
    pub(crate) fn node(&self, idx: NodeIndex) -> &GraphNode {
        &self.graph[idx]
    }

    /// Iterate over edges from a node, filtered by edge kind.
    pub(crate) fn edges_filtered(
        &self,
        idx: NodeIndex,
        direction: Direction,
        kinds: &[GraphEdge],
    ) -> Vec<(NodeIndex, &WeightedEdge)> {
        self.graph
            .edges_directed(idx, direction)
            .filter(|e| kinds.contains(&e.weight().kind))
            .map(|e| {
                let neighbor = match direction {
                    Direction::Outgoing => e.target(),
                    Direction::Incoming => e.source(),
                };
                (neighbor, e.weight())
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Find all symbol NodeIndices whose name matches (case-sensitive).
    pub(super) fn find_symbol_nodes(&self, symbol_name: &str) -> Vec<NodeIndex> {
        self.node_index
            .iter()
            .filter_map(|(key, &idx)| match key {
                NodeKey::Symbol(name, _) if name == symbol_name => Some(idx),
                _ => None,
            })
            .collect()
    }

    /// BFS traversal following specific edge types in a given direction.
    fn bfs_by_edge(
        &self,
        symbol_name: &str,
        max_depth: usize,
        direction: Direction,
        edge_types: &[GraphEdge],
    ) -> Vec<TraversalEntry> {
        let start_indices = self.find_symbol_nodes(symbol_name);
        if start_indices.is_empty() {
            return vec![];
        }

        let mut visited: HashMap<NodeIndex, bool> = HashMap::new();
        let mut queue: std::collections::VecDeque<(NodeIndex, usize)> =
            std::collections::VecDeque::new();
        let mut results: Vec<TraversalEntry> = Vec::new();

        for &idx in &start_indices {
            visited.insert(idx, true);
            queue.push_back((idx, 0));
        }

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            for edge in self.graph.edges_directed(current, direction) {
                if !edge_types.contains(&edge.weight().kind) {
                    continue;
                }
                let neighbor = match direction {
                    Direction::Outgoing => edge.target(),
                    Direction::Incoming => edge.source(),
                };
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, true);
                results.push(TraversalEntry {
                    node: self.graph[neighbor].clone(),
                    depth: depth + 1,
                });
                queue.push_back((neighbor, depth + 1));
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::super::test_helpers::make_test_model;
    use super::*;
    use crate::model::types::*;
    use crate::parser::SupportedLanguage;

    // --- Callers tests ---

    #[test]
    fn callers_finds_direct_caller() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let callers = kg.callers("getUser", 1);
        assert!(!callers.is_empty(), "getUser should have callers");
        assert!(
            callers
                .iter()
                .any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "handler")),
            "handler should be a caller of getUser"
        );
    }

    #[test]
    fn callers_finds_transitive_caller() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        // validate is called by getUser, which is called by handler
        let callers = kg.callers("validate", 2);
        let names: Vec<String> = callers
            .iter()
            .filter_map(|e| match &e.node {
                GraphNode::Symbol { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(
            names.contains(&"getUser".to_string()),
            "getUser should be a transitive caller"
        );
        assert!(
            names.contains(&"handler".to_string()),
            "handler should be a transitive caller"
        );
    }

    #[test]
    fn callers_respects_max_depth() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let callers = kg.callers("validate", 1);
        // At depth 1, only getUser should be found (direct caller)
        assert!(
            callers.iter().all(|e| e.depth == 1),
            "depth-1 results should all be at depth 1"
        );
    }

    #[test]
    fn callers_returns_empty_for_unknown_symbol() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let callers = kg.callers("nonexistent", 5);
        assert!(callers.is_empty());
    }

    // --- Callees tests ---

    #[test]
    fn callees_finds_direct_callee() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let callees = kg.callees("handler", 1);
        assert!(
            callees
                .iter()
                .any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "getUser")),
            "getUser should be a callee of handler"
        );
    }

    #[test]
    fn callees_finds_transitive_callees() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let callees = kg.callees("handler", 3);
        let names: Vec<String> = callees
            .iter()
            .filter_map(|e| match &e.node {
                GraphNode::Symbol { name, .. } => Some(name.clone()),
                GraphNode::External { name } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"getUser".to_string()));
        assert!(names.contains(&"validate".to_string()));
        assert!(
            names.contains(&"axios.get".to_string()),
            "should reach external callees"
        );
    }

    // --- Impact analysis tests ---

    #[test]
    fn impact_analysis_finds_affected_nodes() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let impact = kg.impact_analysis("getUser", 3);
        assert!(
            impact.total_affected > 0,
            "changing getUser should affect something"
        );
        assert!(
            impact
                .affected_nodes
                .iter()
                .any(|a| matches!(&a.node, GraphNode::Symbol { name, .. } if name == "handler")),
            "handler should be affected (it calls getUser)"
        );
    }

    #[test]
    fn impact_analysis_collects_affected_files() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let impact = kg.impact_analysis("getUser", 3);
        assert!(
            !impact.affected_files.is_empty(),
            "should report affected files"
        );
    }

    #[test]
    fn impact_analysis_returns_empty_for_unknown_symbol() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let impact = kg.impact_analysis("nonexistent", 5);
        assert_eq!(impact.total_affected, 0);
    }

    // --- Type hierarchy tests ---

    #[test]
    fn type_hierarchy_finds_ancestors() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let ancestors = kg.type_hierarchy("AdminService", HierarchyDirection::Ancestors);
        assert!(
            ancestors.iter().any(
                |e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "UserService")
            ),
            "UserService should be an ancestor of AdminService"
        );
    }

    #[test]
    fn type_hierarchy_finds_descendants() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let descendants = kg.type_hierarchy("UserService", HierarchyDirection::Descendants);
        assert!(
            descendants.iter().any(
                |e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "AdminService")
            ),
            "AdminService should be a descendant of UserService"
        );
    }

    #[test]
    fn type_hierarchy_both_directions() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let both = kg.type_hierarchy("UserService", HierarchyDirection::Both);
        // Should find ancestors and descendants
        assert!(
            !both.is_empty(),
            "UserService should have hierarchy entries"
        );
    }

    // --- Cycle detection tests ---

    #[test]
    fn find_cycles_returns_empty_when_no_cycles() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let cycles = kg.find_cycles();
        // Our test model has no cycles
        assert!(cycles.is_empty(), "test model should have no cycles");
    }

    #[test]
    fn find_cycles_detects_mutual_recursion() {
        // Build a code model with a call cycle: A -> B -> A
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "cyclic".into(),
            components: vec![Component {
                name: "cyclic".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![
                    Symbol {
                        name: "funcA".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("a.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
                        is_test: false,
                    },
                    Symbol {
                        name: "funcB".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("b.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
                        is_test: false,
                    },
                ],
                imports: vec![],
                references: vec![
                    Reference {
                        source_symbol: "funcA".into(),
                        source_file: PathBuf::from("a.ts"),
                        source_line: 5,
                        target_symbol: "funcB".into(),
                        target_file: Some(PathBuf::from("b.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
                        confidence: 0.0,
                        resolution_method: ResolutionMethod::Unresolved,
                        is_test_reference: false,
                    },
                    Reference {
                        source_symbol: "funcB".into(),
                        source_file: PathBuf::from("b.ts"),
                        source_line: 5,
                        target_symbol: "funcA".into(),
                        target_file: Some(PathBuf::from("a.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
                        confidence: 0.0,
                        resolution_method: ResolutionMethod::Unresolved,
                        is_test_reference: false,
                    },
                ],
                data_models: vec![],
                module_boundaries: vec![],
                env_dependencies: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 2,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 2,
                total_imports: 0,
                total_references: 2,
                total_data_models: 0,
                total_modules: 0,
                resolved_references: 0,
                avg_resolution_confidence: 0.0,
                ..Default::default()
            },
            file_tree: None,
        };

        let kg = KnowledgeGraph::from_code_model(&model);
        let cycles = kg.find_cycles();
        assert!(!cycles.is_empty(), "should detect the A -> B -> A cycle");
    }

    #[test]
    fn find_module_cycles_detects_circular_module_deps() {
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "cyclic-modules".into(),
            components: vec![Component {
                name: "cyclic-modules".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![],
                imports: vec![],
                references: vec![],
                data_models: vec![],
                module_boundaries: vec![
                    ModuleBoundary {
                        name: "auth".into(),
                        files: vec![],
                        exported_symbols: vec![],
                        depends_on: vec!["users".into()],
                    },
                    ModuleBoundary {
                        name: "users".into(),
                        files: vec![],
                        exported_symbols: vec![],
                        depends_on: vec!["auth".into()],
                    },
                ],
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
                total_modules: 2,
                resolved_references: 0,
                avg_resolution_confidence: 0.0,
                ..Default::default()
            },
            file_tree: None,
        };

        let kg = KnowledgeGraph::from_code_model(&model);
        let cycles = kg.find_module_cycles();
        assert!(!cycles.is_empty(), "should detect auth <-> users cycle");
        assert!(
            cycles[0].modules.contains(&"auth".to_string())
                || cycles[0].modules.contains(&"users".to_string()),
            "cycle should mention auth or users module"
        );
    }

    // --- Stats tests ---

    #[test]
    fn stats_counts_are_accurate() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let stats = kg.stats();
        assert_eq!(stats.total_nodes, kg.graph.node_count());
        assert_eq!(stats.total_edges, kg.graph.edge_count());
        assert!(stats.total_nodes > 0);
        assert!(stats.total_edges > 0);
    }

    // --- JSON export tests ---

    #[test]
    fn to_json_produces_valid_structure() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let json = kg.to_json();
        assert!(json.get("nodes").is_some(), "should have nodes array");
        assert!(json.get("edges").is_some(), "should have edges array");
        assert!(json.get("stats").is_some(), "should have stats object");

        let nodes = json["nodes"].as_array().unwrap();
        let edges = json["edges"].as_array().unwrap();
        assert!(!nodes.is_empty());
        assert!(!edges.is_empty());

        // Each node should have id, label, data
        let first_node = &nodes[0];
        assert!(first_node.get("id").is_some());
        assert!(first_node.get("label").is_some());
        assert!(first_node.get("data").is_some());

        // Each edge should have source, target, type
        let first_edge = &edges[0];
        assert!(first_edge.get("source").is_some());
        assert!(first_edge.get("target").is_some());
        assert!(first_edge.get("type").is_some());
    }

    // --- Edge case tests ---

    #[test]
    fn handles_self_referencing_symbol() {
        // A recursive function calls itself
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "self-ref".into(),
            components: vec![Component {
                name: "self-ref".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![Symbol {
                    name: "fibonacci".into(),
                    kind: SymbolKind::Function,
                    anchor: SourceAnchor::from_line(PathBuf::from("math.ts"), 1),
                    doc: None,
                    signature: None,
                    visibility: None,
                    parent: None,
                    is_test: false,
                }],
                imports: vec![],
                references: vec![Reference {
                    source_symbol: "fibonacci".into(),
                    source_file: PathBuf::from("math.ts"),
                    source_line: 3,
                    target_symbol: "fibonacci".into(),
                    target_file: Some(PathBuf::from("math.ts")),
                    target_line: Some(1),
                    reference_kind: ReferenceKind::Call,
                    confidence: 0.0,
                    resolution_method: ResolutionMethod::Unresolved,
                    is_test_reference: false,
                }],
                data_models: vec![],
                module_boundaries: vec![],
                env_dependencies: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 1,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 1,
                total_imports: 0,
                total_references: 1,
                total_data_models: 0,
                total_modules: 0,
                resolved_references: 0,
                avg_resolution_confidence: 0.0,
                ..Default::default()
            },
            file_tree: None,
        };

        let kg = KnowledgeGraph::from_code_model(&model);
        // Should not infinite loop
        let callers = kg.callers("fibonacci", 5);
        // Self-call: fibonacci calls fibonacci, but visited set prevents infinite loop
        // The callers of fibonacci include fibonacci itself
        assert!(
            callers.len() <= 1,
            "should handle self-reference without infinite loop"
        );
    }

    // --- Confidence tests ---

    #[test]
    fn impact_filters_low_confidence() {
        // Build a model where A calls B (high confidence) and B calls C (very low confidence).
        // Impact of C should reach B but NOT A, because cumulative confidence drops below threshold.
        let model = CodeModel {
            version: "1.0".into(),
            project_name: "low-conf".into(),
            components: vec![Component {
                name: "low-conf".into(),
                language: SupportedLanguage::TypeScript,
                interfaces: vec![],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![
                    Symbol {
                        name: "funcA".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("a.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
                        is_test: false,
                    },
                    Symbol {
                        name: "funcB".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("b.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
                        is_test: false,
                    },
                    Symbol {
                        name: "funcC".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("c.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
                        is_test: false,
                    },
                ],
                imports: vec![],
                references: vec![
                    Reference {
                        source_symbol: "funcA".into(),
                        source_file: PathBuf::from("a.ts"),
                        source_line: 5,
                        target_symbol: "funcB".into(),
                        target_file: Some(PathBuf::from("b.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
                        confidence: 0.05, // very low — below threshold
                        resolution_method: ResolutionMethod::GlobalAmbiguous,
                        is_test_reference: false,
                    },
                    Reference {
                        source_symbol: "funcB".into(),
                        source_file: PathBuf::from("b.ts"),
                        source_line: 5,
                        target_symbol: "funcC".into(),
                        target_file: Some(PathBuf::from("c.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
                        confidence: 0.90,
                        resolution_method: ResolutionMethod::SameFile,
                        is_test_reference: false,
                    },
                ],
                data_models: vec![],
                module_boundaries: vec![],
                env_dependencies: vec![],
            }],
            stats: CodeModelStats {
                files_analyzed: 3,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 3,
                total_imports: 0,
                total_references: 2,
                total_data_models: 0,
                total_modules: 0,
                resolved_references: 1,
                avg_resolution_confidence: 0.475,
                ..Default::default()
            },
            file_tree: None,
        };

        let kg = KnowledgeGraph::from_code_model(&model);
        let impact = kg.impact_analysis("funcC", 5);

        // funcB calls funcC with 0.90 confidence — should appear
        assert!(
            impact
                .affected_nodes
                .iter()
                .any(|a| { matches!(&a.node, GraphNode::Symbol { name, .. } if name == "funcB") }),
            "funcB should be affected (high confidence call to funcC)"
        );

        // funcA calls funcB with 0.05 confidence — cumulative = 0.90 * 0.05 = 0.045 < 0.1
        assert!(
            !impact
                .affected_nodes
                .iter()
                .any(|a| { matches!(&a.node, GraphNode::Symbol { name, .. } if name == "funcA") }),
            "funcA should be pruned (cumulative confidence below threshold)"
        );
    }

    #[test]
    fn impact_cumulative_confidence() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        // In the test model: handler calls getUser (0.95), getUser calls validate (0.90)
        // Impact of validate: getUser (conf=0.90), handler (conf=0.90*0.95=0.855)
        let impact = kg.impact_analysis("validate", 5);
        assert!(impact.total_affected > 0);

        // All affected nodes should have confidence > 0
        for node in &impact.affected_nodes {
            assert!(
                node.confidence > 0.0,
                "affected node {:?} should have positive confidence",
                node.node.display_name()
            );
        }

        // handler's cumulative confidence should be less than getUser's
        let handler_conf = impact
            .affected_nodes
            .iter()
            .find(|a| matches!(&a.node, GraphNode::Symbol { name, .. } if name == "handler"))
            .map(|a| a.confidence);
        let getuser_conf = impact
            .affected_nodes
            .iter()
            .find(|a| matches!(&a.node, GraphNode::Symbol { name, .. } if name == "getUser"))
            .map(|a| a.confidence);

        if let (Some(h), Some(g)) = (handler_conf, getuser_conf) {
            assert!(
                h < g,
                "handler confidence ({h}) should be less than getUser confidence ({g})"
            );
        }
    }

    #[test]
    fn to_json_includes_edge_confidence() {
        let model = make_test_model();
        let kg = KnowledgeGraph::from_code_model(&model);

        let json = kg.to_json();
        let edges = json["edges"].as_array().unwrap();

        // Every edge should have a "confidence" field
        for edge in edges {
            assert!(
                edge.get("confidence").is_some(),
                "edge should have confidence field"
            );
            let conf = edge["confidence"].as_f64().unwrap();
            assert!(conf >= 0.0 && conf <= 1.0, "confidence should be in [0, 1]");
        }
    }
}
