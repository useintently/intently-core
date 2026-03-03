//! Type definitions for the knowledge graph.
//!
//! Contains node types, edge types, weighted edges, result types, and
//! the internal `NodeKey` used for deduplication during construction.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::types::{DataModelKind, HttpMethod, SymbolKind};

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

    /// Whether this node represents a code symbol (function, class, method, etc.).
    pub fn is_symbol(&self) -> bool {
        matches!(self, Self::Symbol { .. })
    }

    /// Whether this node represents an HTTP endpoint.
    pub fn is_interface(&self) -> bool {
        matches!(self, Self::Interface { .. })
    }

    /// Whether this node represents an external/unresolved dependency.
    pub fn is_external(&self) -> bool {
        matches!(self, Self::External { .. })
    }

    /// The node type as a string for stats grouping.
    pub(super) fn type_name(&self) -> &'static str {
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
    pub(super) fn type_name(&self) -> &'static str {
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
// Weighted edge (kind + confidence)
// ---------------------------------------------------------------------------

/// An edge carrying both its semantic kind and a confidence score.
///
/// Structural edges (`Defines`, `Contains`, `Exposes`, `DependsOn`) always
/// have `confidence: 1.0` because they are derived from syntax, not heuristic
/// resolution. Reference-derived edges (`Calls`, `Extends`, `Implements`,
/// `UsesType`, `Imports`) inherit the resolver's confidence score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeightedEdge {
    /// The semantic edge kind.
    pub kind: GraphEdge,
    /// Resolution confidence in \[0.0, 1.0\].
    pub confidence: f64,
}

impl WeightedEdge {
    /// Create a structural edge with perfect confidence (1.0).
    pub fn structural(kind: GraphEdge) -> Self {
        Self {
            kind,
            confidence: 1.0,
        }
    }

    /// Create an edge from a reference with its resolution confidence.
    pub fn from_reference(kind: GraphEdge, confidence: f64) -> Self {
        Self { kind, confidence }
    }
}

// ---------------------------------------------------------------------------
// Node identity for deduplication
// ---------------------------------------------------------------------------

/// Identity key for deduplicating nodes during construction.
///
/// Two references to the same symbol converge on one `NodeIndex`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum NodeKey {
    File(PathBuf),
    Symbol(String, PathBuf),
    Interface(String, String, PathBuf), // method+path+file
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
    /// Cumulative confidence along the path from root to this node.
    ///
    /// Computed as the product of all edge confidences along the BFS path.
    /// A value of 1.0 means all edges were structural; lower values indicate
    /// heuristic resolution was involved.
    pub confidence: f64,
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
