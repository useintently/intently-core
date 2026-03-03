//! Git metadata extraction for churn and ownership analysis.
//!
//! Gated behind the `git` Cargo feature flag. When enabled, walks commit
//! history (up to 1000 commits from HEAD) and produces per-file metadata
//! (commit count, distinct authors, last modified timestamp).
//!
//! Uses `git log` via `std::process::Command` for simplicity and zero
//! additional dependencies. Requires `git` to be available on PATH.

#[cfg(feature = "git")]
pub mod metadata;
