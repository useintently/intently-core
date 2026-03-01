pub mod engine;
pub mod error;
pub mod model;
pub mod parser;
pub mod search;
pub mod workspace;

pub use engine::{ExtractionResult, IntentlyEngine, PipelineTiming};
pub use error::{IntentlyError, Result};
pub use model::file_tree::{
    DirectoryDependency, DirectoryNode, DirectoryRole, DirectoryStats, FileEntry, FileTree,
};
pub use model::graph::{
    AffectedNode, Cycle, GraphEdge, GraphNode, GraphStats, HierarchyDirection, ImpactResult,
    KnowledgeGraph, TraversalEntry, WeightedEdge,
};
pub use model::graph_analysis::{
    AnalysisContext, AnalysisPipeline, DegreeCentrality, EntryPoint, EntryPointReason, FlowStep,
    GraphAnalyzer, ProcessFlow,
};
pub use model::types::{estimate_tokens, FileExtraction, FileRole, ResolutionMethod, TokenBudget};
pub use workspace::{WorkspaceKind, WorkspaceLayout, WorkspacePackage};
