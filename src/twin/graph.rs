//! Knowledge graph backed by petgraph.
//!
//! `KnowledgeGraph` is a **derived view** over the `SystemTwin`. It converts
//! flat `Vec<T>` data (symbols, references, imports, modules) into a directed
//! graph with O(1) adjacency lookups. This replaces hand-rolled BFS with
//! petgraph's standard traversal algorithms and enables structural analysis
//! (Tarjan's SCC for cycle detection → ARC-001).
//!
//! See ADR-003 for design rationale.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use super::types::{
    DataModelKind, HttpMethod, ReferenceKind, SymbolKind, SystemTwin,
};

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// A node in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphNode {
    /// A source file in the project.
    File { path: PathBuf },
    /// A code symbol (function, class, method, etc.).
    Symbol {
        name: String,
        kind: SymbolKind,
        file: PathBuf,
        line: usize,
    },
    /// An HTTP endpoint.
    Interface {
        method: HttpMethod,
        path: String,
        file: PathBuf,
        line: usize,
    },
    /// A data model (class, struct, interface, etc.).
    DataModel {
        name: String,
        kind: DataModelKind,
        file: PathBuf,
        line: usize,
    },
    /// A logical module inferred from directory structure.
    Module { name: String },
    /// An external/unresolved dependency (npm package, stdlib, etc.).
    External { name: String },
}

impl GraphNode {
    /// Human-readable display name for cycle reporting and JSON output.
    pub fn display_name(&self) -> String {
        match self {
            Self::File { path } => path.display().to_string(),
            Self::Symbol { name, .. } => name.clone(),
            Self::Interface { method, path, .. } => format!("{method} {path}"),
            Self::DataModel { name, .. } => name.clone(),
            Self::Module { name } => format!("module:{name}"),
            Self::External { name } => format!("ext:{name}"),
        }
    }

    /// The file path associated with this node, if any.
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            Self::File { path } => Some(path),
            Self::Symbol { file, .. } => Some(file),
            Self::Interface { file, .. } => Some(file),
            Self::DataModel { file, .. } => Some(file),
            Self::Module { .. } | Self::External { .. } => None,
        }
    }

    /// The node type as a string for stats grouping.
    fn type_name(&self) -> &'static str {
        match self {
            Self::File { .. } => "file",
            Self::Symbol { .. } => "symbol",
            Self::Interface { .. } => "interface",
            Self::DataModel { .. } => "data_model",
            Self::Module { .. } => "module",
            Self::External { .. } => "external",
        }
    }
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

/// An edge in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdge {
    /// Function or method call.
    Calls,
    /// Class inheritance (extends).
    Extends,
    /// Interface/trait implementation.
    Implements,
    /// Import relationship (file → file or file → external).
    Imports,
    /// Type used as parameter, field, or return type.
    UsesType,
    /// File defines a symbol, interface, or data model.
    Defines,
    /// Module contains a file.
    Contains,
    /// Module exposes a public symbol.
    Exposes,
    /// Module depends on another module.
    DependsOn,
}

impl GraphEdge {
    /// The edge type as a string for stats grouping.
    fn type_name(&self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::Extends => "extends",
            Self::Implements => "implements",
            Self::Imports => "imports",
            Self::UsesType => "uses_type",
            Self::Defines => "defines",
            Self::Contains => "contains",
            Self::Exposes => "exposes",
            Self::DependsOn => "depends_on",
        }
    }
}

// ---------------------------------------------------------------------------
// Node identity for deduplication
// ---------------------------------------------------------------------------

/// Identity key for deduplicating nodes during construction.
///
/// Two references to the same symbol converge on one `NodeIndex`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NodeKey {
    File(PathBuf),
    Symbol(String, PathBuf),
    Interface(String, String, PathBuf),   // method+path+file
    DataModel(String, PathBuf),
    Module(String),
    External(String),
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of an impact analysis traversal.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    /// The root symbol analyzed.
    pub root: String,
    /// Nodes affected, ordered by distance from root.
    pub affected_nodes: Vec<AffectedNode>,
    /// Unique files containing affected nodes.
    pub affected_files: Vec<PathBuf>,
    /// Unique modules containing affected nodes.
    pub affected_modules: Vec<String>,
    /// Total number of affected nodes.
    pub total_affected: usize,
}

/// A node affected by a change, with distance and relationship type.
#[derive(Debug, Clone, Serialize)]
pub struct AffectedNode {
    /// The affected node.
    pub node: GraphNode,
    /// BFS depth from the root.
    pub depth: usize,
    /// The edge type through which this node was reached.
    pub edge_type: GraphEdge,
}

/// A cycle detected by Tarjan's SCC algorithm.
#[derive(Debug, Clone, Serialize)]
pub struct Cycle {
    /// Nodes forming the cycle.
    pub nodes: Vec<GraphNode>,
    /// Module names involved (for ARC-001 reporting).
    pub modules: Vec<String>,
}

/// A traversal result entry: a node with its BFS depth.
#[derive(Debug, Clone, Serialize)]
pub struct TraversalEntry {
    /// The graph node found.
    pub node: GraphNode,
    /// BFS depth from the start.
    pub depth: usize,
}

/// Direction for type hierarchy traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HierarchyDirection {
    /// Find what this type extends/implements.
    Ancestors,
    /// Find what extends/implements this type.
    Descendants,
    /// Both directions.
    Both,
}

/// Graph-wide statistics.
#[derive(Debug, Clone, Serialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub node_counts: HashMap<String, usize>,
    pub edge_counts: HashMap<String, usize>,
    pub connected_components: usize,
    pub cycles_detected: usize,
}

// ---------------------------------------------------------------------------
// KnowledgeGraph
// ---------------------------------------------------------------------------

/// A directed graph derived from the SystemTwin.
///
/// Provides O(1) adjacency lookups for callers, callees, type hierarchy,
/// and impact analysis. Supports structural analysis (cycle detection via
/// Tarjan's SCC) for ARC-001 policy enforcement.
pub struct KnowledgeGraph {
    graph: DiGraph<GraphNode, GraphEdge>,
    node_index: HashMap<NodeKey, NodeIndex>,
}

impl KnowledgeGraph {
    /// Build a knowledge graph from a SystemTwin.
    ///
    /// The 7-step construction algorithm:
    /// 1. Create Module nodes from ModuleBoundary data
    /// 2. Create File nodes from module boundaries + symbol files
    /// 3. Create Symbol nodes with Defines edges from parent files
    /// 4. Create Interface nodes with Defines edges
    /// 5. Create DataModel nodes with Defines edges
    /// 6. Process References → Calls, Extends, Implements, UsesType, Imports edges
    /// 7. Process ModuleBoundaries → Contains, Exposes, DependsOn edges
    pub fn from_twin(twin: &SystemTwin) -> Self {
        let mut kg = Self {
            graph: DiGraph::new(),
            node_index: HashMap::new(),
        };

        for component in &twin.components {
            // Step 1: Module nodes
            for module in &component.module_boundaries {
                kg.ensure_node(NodeKey::Module(module.name.clone()), || {
                    GraphNode::Module { name: module.name.clone() }
                });
            }

            // Step 2: File nodes from modules
            for module in &component.module_boundaries {
                for file in &module.files {
                    kg.ensure_node(NodeKey::File(file.clone()), || {
                        GraphNode::File { path: file.clone() }
                    });
                }
            }

            // Step 3: Symbol nodes + Defines edges
            for symbol in &component.symbols {
                let file_idx = kg.ensure_node(
                    NodeKey::File(symbol.anchor.file.clone()),
                    || GraphNode::File { path: symbol.anchor.file.clone() },
                );
                let sym_idx = kg.ensure_node(
                    NodeKey::Symbol(symbol.name.clone(), symbol.anchor.file.clone()),
                    || GraphNode::Symbol {
                        name: symbol.name.clone(),
                        kind: symbol.kind,
                        file: symbol.anchor.file.clone(),
                        line: symbol.anchor.line,
                    },
                );
                kg.graph.add_edge(file_idx, sym_idx, GraphEdge::Defines);
            }

            // Step 4: Interface nodes + Defines edges
            for iface in &component.interfaces {
                let file_idx = kg.ensure_node(
                    NodeKey::File(iface.anchor.file.clone()),
                    || GraphNode::File { path: iface.anchor.file.clone() },
                );
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
                kg.graph.add_edge(file_idx, iface_idx, GraphEdge::Defines);
            }

            // Step 5: DataModel nodes + Defines edges
            for model in &component.data_models {
                let file_idx = kg.ensure_node(
                    NodeKey::File(model.anchor.file.clone()),
                    || GraphNode::File { path: model.anchor.file.clone() },
                );
                let model_idx = kg.ensure_node(
                    NodeKey::DataModel(model.name.clone(), model.anchor.file.clone()),
                    || GraphNode::DataModel {
                        name: model.name.clone(),
                        kind: model.model_kind,
                        file: model.anchor.file.clone(),
                        line: model.anchor.line,
                    },
                );
                kg.graph.add_edge(file_idx, model_idx, GraphEdge::Defines);
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
                        kg.graph.add_edge(source_idx, target_idx, GraphEdge::Calls);
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
                        kg.graph.add_edge(source_idx, target_idx, GraphEdge::Extends);
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
                        kg.graph.add_edge(source_idx, target_idx, GraphEdge::Implements);
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
                        kg.graph.add_edge(source_idx, target_idx, GraphEdge::UsesType);
                    }
                    ReferenceKind::Import => {
                        let source_file_idx = kg.ensure_node(
                            NodeKey::File(reference.source_file.clone()),
                            || GraphNode::File { path: reference.source_file.clone() },
                        );
                        let target_idx = match &reference.target_file {
                            Some(tf) => kg.ensure_node(
                                NodeKey::File(tf.clone()),
                                || GraphNode::File { path: tf.clone() },
                            ),
                            None => kg.ensure_external(&reference.target_symbol),
                        };
                        kg.graph.add_edge(source_file_idx, target_idx, GraphEdge::Imports);
                    }
                }
            }

            // Step 7: Module boundaries → Contains, Exposes, DependsOn
            for module in &component.module_boundaries {
                let module_idx = kg.node_index[&NodeKey::Module(module.name.clone())];

                // Contains: module → file
                for file in &module.files {
                    if let Some(&file_idx) = kg.node_index.get(&NodeKey::File(file.clone())) {
                        kg.graph.add_edge(module_idx, file_idx, GraphEdge::Contains);
                    }
                }

                // Exposes: module → symbol (find matching symbols in module files)
                for sym_name in &module.exported_symbols {
                    // Find any symbol node matching this name in the module's files
                    for file in &module.files {
                        let key = NodeKey::Symbol(sym_name.clone(), file.clone());
                        if let Some(&sym_idx) = kg.node_index.get(&key) {
                            kg.graph.add_edge(module_idx, sym_idx, GraphEdge::Exposes);
                            break;
                        }
                    }
                }

                // DependsOn: module → module
                for dep_name in &module.depends_on {
                    let dep_idx = kg.ensure_node(
                        NodeKey::Module(dep_name.clone()),
                        || GraphNode::Module { name: dep_name.clone() },
                    );
                    kg.graph.add_edge(module_idx, dep_idx, GraphEdge::DependsOn);
                }
            }
        }

        kg
    }

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

        let mut visited: HashMap<NodeIndex, bool> = HashMap::new();
        let mut queue: std::collections::VecDeque<(NodeIndex, usize)> =
            std::collections::VecDeque::new();
        let mut affected: Vec<AffectedNode> = Vec::new();

        for &idx in &start_indices {
            visited.insert(idx, true);
            queue.push_back((idx, 0));
        }

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // Check incoming edges (who depends on this node?)
            for edge in self.graph.edges_directed(current, Direction::Incoming) {
                if !impact_edges.contains(edge.weight()) {
                    continue;
                }
                let neighbor = edge.source();
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, true);
                affected.push(AffectedNode {
                    node: self.graph[neighbor].clone(),
                    depth: depth + 1,
                    edge_type: edge.weight().clone(),
                });
                queue.push_back((neighbor, depth + 1));
            }
        }

        // Collect unique affected files and modules
        let mut files: Vec<PathBuf> = affected
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
            results.extend(self.bfs_by_edge(
                type_name,
                10,
                Direction::Incoming,
                &hierarchy_edges,
            ));
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
        let mut sub = DiGraph::<GraphNode, GraphEdge>::new();
        let mut sub_index: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for idx in self.graph.node_indices() {
            if matches!(self.graph[idx], GraphNode::Module { .. }) {
                let new_idx = sub.add_node(self.graph[idx].clone());
                sub_index.insert(idx, new_idx);
            }
        }

        for edge in self.graph.edge_references() {
            if *edge.weight() == GraphEdge::DependsOn {
                if let (Some(&src), Some(&tgt)) =
                    (sub_index.get(&edge.source()), sub_index.get(&edge.target()))
                {
                    sub.add_edge(src, tgt, GraphEdge::DependsOn);
                }
            }
        }

        let sccs = tarjan_scc(&sub);
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                let nodes: Vec<GraphNode> =
                    scc.iter().map(|&idx| sub[idx].clone()).collect();
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
                .entry(edge.weight().type_name().to_string())
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
                    "type": edge.weight().type_name(),
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
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Get or create a node, returning its index.
    fn ensure_node(
        &mut self,
        key: NodeKey,
        make_node: impl FnOnce() -> GraphNode,
    ) -> NodeIndex {
        if let Some(&idx) = self.node_index.get(&key) {
            return idx;
        }
        let node = make_node();
        let idx = self.graph.add_node(node);
        self.node_index.insert(key, idx);
        idx
    }

    /// Get or create a Symbol node.
    fn ensure_symbol(
        &mut self,
        name: &str,
        file: &Path,
        line: usize,
    ) -> NodeIndex {
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
            GraphNode::External { name: name.to_string() }
        })
    }

    /// Find all symbol NodeIndices whose name matches (case-sensitive).
    fn find_symbol_nodes(&self, symbol_name: &str) -> Vec<NodeIndex> {
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
                if !edge_types.contains(edge.weight()) {
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
    use super::*;
    use crate::parser::SupportedLanguage;
    use crate::twin::types::*;

    /// Build a minimal twin for testing graph construction.
    fn make_test_twin() -> SystemTwin {
        SystemTwin {
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
                }],
                dependencies: vec![],
                sinks: vec![],
                symbols: vec![
                    Symbol {
                        name: "handler".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line_range(
                            PathBuf::from("src/routes.ts"), 5, 20,
                        ),
                        doc: None,
                        signature: None,
                        visibility: Some(Visibility::Public),
                        parent: None,
                    },
                    Symbol {
                        name: "getUser".into(),
                        kind: SymbolKind::Method,
                        anchor: SourceAnchor::from_line_range(
                            PathBuf::from("src/services.ts"), 10, 30,
                        ),
                        doc: None,
                        signature: None,
                        visibility: Some(Visibility::Public),
                        parent: Some("UserService".into()),
                    },
                    Symbol {
                        name: "validate".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line_range(
                            PathBuf::from("src/utils.ts"), 1, 10,
                        ),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
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
                    },
                    Reference {
                        source_symbol: "getUser".into(),
                        source_file: PathBuf::from("src/services.ts"),
                        source_line: 15,
                        target_symbol: "validate".into(),
                        target_file: Some(PathBuf::from("src/utils.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
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
                    anchor: SourceAnchor::from_line_range(
                        PathBuf::from("src/models.ts"), 1, 10,
                    ),
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
            }],
            stats: TwinStats {
                files_analyzed: 4,
                total_interfaces: 1,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 3,
                total_imports: 0,
                total_references: 5,
                total_data_models: 1,
                total_modules: 2,
            },
        }
    }

    // --- Construction tests ---

    #[test]
    fn constructs_from_empty_twin() {
        let twin = SystemTwin {
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
            }],
            stats: TwinStats {
                files_analyzed: 0,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 0,
                total_imports: 0,
                total_references: 0,
                total_data_models: 0,
                total_modules: 0,
            },
        };

        let kg = KnowledgeGraph::from_twin(&twin);
        assert_eq!(kg.graph.node_count(), 0);
        assert_eq!(kg.graph.edge_count(), 0);
    }

    #[test]
    fn constructs_with_symbols_and_references() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

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
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        // getUser appears in two references as source, but should be one node
        let getuser_nodes = kg.find_symbol_nodes("getUser");
        assert_eq!(getuser_nodes.len(), 1, "getUser should be deduplicated to one node");
    }

    #[test]
    fn creates_external_nodes_for_unresolved_targets() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert!(
            stats.node_counts.get("external").copied().unwrap_or(0) > 0,
            "should have external nodes for unresolved targets"
        );
    }

    #[test]
    fn creates_module_nodes_with_contains_edges() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

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
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("defines").copied().unwrap_or(0) >= 3,
            "should have at least 3 defines edges (3 symbols)"
        );
    }

    #[test]
    fn creates_depends_on_edges_between_modules() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("depends_on").copied().unwrap_or(0) > 0,
            "should have depends_on edge (routes -> services)"
        );
    }

    // --- Callers tests ---

    #[test]
    fn callers_finds_direct_caller() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let callers = kg.callers("getUser", 1);
        assert!(!callers.is_empty(), "getUser should have callers");
        assert!(
            callers.iter().any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "handler")),
            "handler should be a caller of getUser"
        );
    }

    #[test]
    fn callers_finds_transitive_caller() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        // validate is called by getUser, which is called by handler
        let callers = kg.callers("validate", 2);
        let names: Vec<String> = callers
            .iter()
            .filter_map(|e| match &e.node {
                GraphNode::Symbol { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"getUser".to_string()), "getUser should be a transitive caller");
        assert!(names.contains(&"handler".to_string()), "handler should be a transitive caller");
    }

    #[test]
    fn callers_respects_max_depth() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let callers = kg.callers("validate", 1);
        // At depth 1, only getUser should be found (direct caller)
        assert!(callers.iter().all(|e| e.depth == 1), "depth-1 results should all be at depth 1");
    }

    #[test]
    fn callers_returns_empty_for_unknown_symbol() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let callers = kg.callers("nonexistent", 5);
        assert!(callers.is_empty());
    }

    // --- Callees tests ---

    #[test]
    fn callees_finds_direct_callee() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let callees = kg.callees("handler", 1);
        assert!(
            callees.iter().any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "getUser")),
            "getUser should be a callee of handler"
        );
    }

    #[test]
    fn callees_finds_transitive_callees() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

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
        assert!(names.contains(&"axios.get".to_string()), "should reach external callees");
    }

    // --- Impact analysis tests ---

    #[test]
    fn impact_analysis_finds_affected_nodes() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let impact = kg.impact_analysis("getUser", 3);
        assert!(
            impact.total_affected > 0,
            "changing getUser should affect something"
        );
        assert!(
            impact.affected_nodes.iter().any(|a| matches!(&a.node, GraphNode::Symbol { name, .. } if name == "handler")),
            "handler should be affected (it calls getUser)"
        );
    }

    #[test]
    fn impact_analysis_collects_affected_files() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let impact = kg.impact_analysis("getUser", 3);
        assert!(
            !impact.affected_files.is_empty(),
            "should report affected files"
        );
    }

    #[test]
    fn impact_analysis_returns_empty_for_unknown_symbol() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let impact = kg.impact_analysis("nonexistent", 5);
        assert_eq!(impact.total_affected, 0);
    }

    // --- Type hierarchy tests ---

    #[test]
    fn type_hierarchy_finds_ancestors() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let ancestors = kg.type_hierarchy("AdminService", HierarchyDirection::Ancestors);
        assert!(
            ancestors.iter().any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "UserService")),
            "UserService should be an ancestor of AdminService"
        );
    }

    #[test]
    fn type_hierarchy_finds_descendants() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let descendants = kg.type_hierarchy("UserService", HierarchyDirection::Descendants);
        assert!(
            descendants.iter().any(|e| matches!(&e.node, GraphNode::Symbol { name, .. } if name == "AdminService")),
            "AdminService should be a descendant of UserService"
        );
    }

    #[test]
    fn type_hierarchy_both_directions() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let both = kg.type_hierarchy("UserService", HierarchyDirection::Both);
        // Should find ancestors and descendants
        assert!(!both.is_empty(), "UserService should have hierarchy entries");
    }

    // --- Cycle detection tests ---

    #[test]
    fn find_cycles_returns_empty_when_no_cycles() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let cycles = kg.find_cycles();
        // Our test twin has no cycles
        assert!(cycles.is_empty(), "test twin should have no cycles");
    }

    #[test]
    fn find_cycles_detects_mutual_recursion() {
        // Build a twin with a call cycle: A -> B -> A
        let twin = SystemTwin {
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
                    },
                    Symbol {
                        name: "funcB".into(),
                        kind: SymbolKind::Function,
                        anchor: SourceAnchor::from_line(PathBuf::from("b.ts"), 1),
                        doc: None,
                        signature: None,
                        visibility: None,
                        parent: None,
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
                    },
                    Reference {
                        source_symbol: "funcB".into(),
                        source_file: PathBuf::from("b.ts"),
                        source_line: 5,
                        target_symbol: "funcA".into(),
                        target_file: Some(PathBuf::from("a.ts")),
                        target_line: Some(1),
                        reference_kind: ReferenceKind::Call,
                    },
                ],
                data_models: vec![],
                module_boundaries: vec![],
            }],
            stats: TwinStats {
                files_analyzed: 2,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 2,
                total_imports: 0,
                total_references: 2,
                total_data_models: 0,
                total_modules: 0,
            },
        };

        let kg = KnowledgeGraph::from_twin(&twin);
        let cycles = kg.find_cycles();
        assert!(!cycles.is_empty(), "should detect the A -> B -> A cycle");
    }

    #[test]
    fn find_module_cycles_detects_circular_module_deps() {
        let twin = SystemTwin {
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
            }],
            stats: TwinStats {
                files_analyzed: 0,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 0,
                total_imports: 0,
                total_references: 0,
                total_data_models: 0,
                total_modules: 2,
            },
        };

        let kg = KnowledgeGraph::from_twin(&twin);
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
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert_eq!(stats.total_nodes, kg.graph.node_count());
        assert_eq!(stats.total_edges, kg.graph.edge_count());
        assert!(stats.total_nodes > 0);
        assert!(stats.total_edges > 0);
    }

    // --- JSON export tests ---

    #[test]
    fn to_json_produces_valid_structure() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

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
        let twin = SystemTwin {
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
                }],
                data_models: vec![],
                module_boundaries: vec![],
            }],
            stats: TwinStats {
                files_analyzed: 1,
                total_interfaces: 0,
                total_dependencies: 0,
                total_sinks: 0,
                total_symbols: 1,
                total_imports: 0,
                total_references: 1,
                total_data_models: 0,
                total_modules: 0,
            },
        };

        let kg = KnowledgeGraph::from_twin(&twin);
        // Should not infinite loop
        let callers = kg.callers("fibonacci", 5);
        // Self-call: fibonacci calls fibonacci, but visited set prevents infinite loop
        // The callers of fibonacci include fibonacci itself
        assert!(callers.len() <= 1, "should handle self-reference without infinite loop");
    }

    #[test]
    fn exposes_edge_links_module_to_exported_symbol() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("exposes").copied().unwrap_or(0) > 0,
            "should have exposes edges for exported symbols"
        );
    }

    #[test]
    fn imports_edge_connects_files() {
        let twin = make_test_twin();
        let kg = KnowledgeGraph::from_twin(&twin);

        let stats = kg.stats();
        assert!(
            stats.edge_counts.get("imports").copied().unwrap_or(0) > 0,
            "should have imports edges"
        );
    }
}
