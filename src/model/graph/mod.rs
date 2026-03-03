//! Knowledge graph backed by petgraph.
//!
//! `KnowledgeGraph` is a **derived view** over the `CodeModel`. It converts
//! flat `Vec<T>` data (symbols, references, imports, modules) into a directed
//! graph with O(1) adjacency lookups. This replaces hand-rolled BFS with
//! petgraph's standard traversal algorithms and enables structural analysis
//! (Tarjan's SCC for cycle detection → ARC-001).
//!
//! See ADR-003 for design rationale.
//!
//! # Module structure
//!
//! - `types` — Node types, edge types, weighted edges, result types
//! - `construction` — `KnowledgeGraph` struct and `from_code_model()` builder
//! - `analysis` — Traversal algorithms, impact analysis, cycle detection, stats

mod analysis;
mod construction;
mod types;

#[cfg(test)]
mod test_helpers;

// Re-export all public types at the `graph` module level.
// This preserves the existing import paths: `use model::graph::{KnowledgeGraph, ...}`
pub use construction::KnowledgeGraph;
pub use types::{
    AffectedNode, Cycle, GraphEdge, GraphNode, GraphStats, HierarchyDirection, ImpactResult,
    TraversalEntry, WeightedEdge,
};
