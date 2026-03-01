//! Structural code search using ast-grep patterns.
//!
//! Provides declarative pattern matching against source code using
//! ast-grep's metavariable syntax (`$VAR` for single captures,
//! `$$$ARGS` for multiple captures). Patterns operate on the CST,
//! enabling language-aware structural search beyond regex.

mod pattern_engine;

pub use pattern_engine::{PatternMatch, SearchPattern, StructuralSearch};
