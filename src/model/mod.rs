//! The CodeModel intermediate representation and its supporting modules.
//!
//! This module contains the core data structures, builders, and analyzers
//! that form the semantic IR (intermediate representation) of a codebase.
//!
//! # Module overview
//!
//! - [`types`] — Core data types: `CodeModel`, `FileExtraction`, `Component`,
//!   `Interface`, `Symbol`, `Reference`, `DataModel`, and supporting types.
//! - [`builder`] — `CodeModelBuilder` with incremental per-file updates.
//! - [`diff`] — Semantic diffing between two `CodeModel` states.
//! - [`graph`] — `KnowledgeGraph` (petgraph): impact analysis, cycle detection.
//! - [`graph_analysis`] — Composable analysis pipeline (`GraphAnalyzer` trait).
//! - [`extractors`] — Per-language semantic extractors (16 languages).
//! - [`symbol_table`] — Two-level symbol table for cross-file resolution.
//! - [`import_resolver`] — Cross-file import resolution with confidence scoring.
//! - [`module_inference`] — Module boundary detection from directory structure.
//! - [`file_tree`] — Hierarchical directory structure with role inference.
//! - [`patterns`] — Shared cross-language extraction patterns.

pub mod builder;
pub mod diff;
pub mod extractors;
pub mod file_tree;
pub mod graph;
pub mod graph_analysis;
pub mod import_resolver;
pub mod module_inference;
pub mod patterns;
pub mod symbol_table;
pub mod types;
