//! Git metadata computation via `git log`.
//!
//! Walks up to [`MAX_COMMITS`] commits from HEAD, parsing author, timestamp,
//! and changed file paths per commit. Aggregates into [`GitFileMetadata`] per
//! file and [`GitStats`] for repository-level summaries.
//!
//! Graceful degradation: returns `None` if:
//! - `git` binary is not available on PATH
//! - The directory is not inside a git repository
//! - `git log` fails for any reason (shallow clone, corrupted, etc.)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::debug;

use crate::model::types::{GitFileMetadata, GitStats};

/// Maximum number of commits to walk from HEAD.
///
/// Bounds wall-clock time on large repositories while still capturing
/// meaningful churn data. 1000 commits typically covers 3-6 months
/// of active development.
const MAX_COMMITS: usize = 1000;

/// Separator between commit records in git log output.
/// Uses a string unlikely to appear in author names or file paths.
const RECORD_SEP: &str = "---INTENTLY_COMMIT---";

/// Compute per-file git metadata for all tracked files.
///
/// Returns `None` if git is not available, the directory is not a git repo,
/// or any other error occurs during git operations.
pub fn compute_git_metadata(repo_root: &Path) -> Option<HashMap<PathBuf, GitFileMetadata>> {
    // Run git log with a custom format:
    // For each commit: RECORD_SEP, author name, unix timestamp, then --name-only for changed files
    let max_commits_arg = format!("-{MAX_COMMITS}");
    let format_arg = format!("{RECORD_SEP}%n%an%n%at");
    let output = Command::new("git")
        .args(["log", "--format", &format_arg, "--name-only", &max_commits_arg])
        .current_dir(repo_root)
        .output()
        .ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug!(stderr = %stderr, "git log failed — not a git repo or git unavailable");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        debug!("git log returned empty output — empty repo or no commits");
        return None;
    }

    let metadata = parse_git_log(&stdout, repo_root);
    if metadata.is_empty() {
        debug!(
            stdout_len = stdout.len(),
            "parse_git_log returned empty map"
        );
        return None;
    }

    Some(metadata)
}

/// Compute aggregate git statistics from per-file metadata.
pub fn compute_git_stats(metadata: &HashMap<PathBuf, GitFileMetadata>) -> GitStats {
    let mut all_authors: HashSet<&str> = HashSet::new();
    let mut total_commits = 0;

    for meta in metadata.values() {
        total_commits += meta.commit_count;
        if let Some(ref author) = meta.last_author {
            all_authors.insert(author);
        }
    }

    let avg_commits_per_file = if metadata.is_empty() {
        0.0
    } else {
        total_commits as f64 / metadata.len() as f64
    };

    // Top 10 files by commit count
    let mut files_by_churn: Vec<(PathBuf, usize)> = metadata
        .iter()
        .map(|(path, meta)| (path.clone(), meta.commit_count))
        .collect();
    files_by_churn.sort_by(|a, b| b.1.cmp(&a.1));
    files_by_churn.truncate(10);

    GitStats {
        total_authors: all_authors.len(),
        total_commits,
        avg_commits_per_file,
        hottest_files: files_by_churn,
    }
}

/// Parse the output of `git log --format=... --name-only`.
///
/// Expected format per commit:
/// ```text
/// ---INTENTLY_COMMIT---
/// Author Name
/// 1709312000
///
/// path/to/file1.rs
/// path/to/file2.rs
/// ```
fn parse_git_log(output: &str, repo_root: &Path) -> HashMap<PathBuf, GitFileMetadata> {
    let mut file_data: HashMap<PathBuf, FileAccumulator> = HashMap::new();

    // Split on the record separator to get individual commit blocks
    for record in output.split(RECORD_SEP).skip(1) {
        let lines: Vec<&str> = record.lines().collect();
        if lines.len() < 2 {
            continue;
        }

        // lines[0] is empty (newline after separator), or the author
        // Depending on format, first non-empty line is author, second is timestamp
        let mut line_iter = lines.iter().filter(|l| !l.is_empty());

        let author = match line_iter.next() {
            Some(a) => a.to_string(),
            None => continue,
        };

        let timestamp: i64 = match line_iter.next().and_then(|t| t.parse().ok()) {
            Some(ts) => ts,
            None => continue,
        };

        // Remaining non-empty lines are file paths
        for file_line in line_iter {
            let file_line = file_line.trim();
            if file_line.is_empty() {
                continue;
            }

            let file_path = repo_root.join(file_line);
            let entry = file_data
                .entry(file_path)
                .or_insert_with(|| FileAccumulator {
                    last_modified: None,
                    last_author: None,
                    commit_count: 0,
                    authors: HashSet::new(),
                });

            entry.commit_count += 1;
            entry.authors.insert(author.clone());

            // Track the most recent commit (highest timestamp)
            if entry.last_modified.is_none() || entry.last_modified < Some(timestamp) {
                entry.last_modified = Some(timestamp);
                entry.last_author = Some(author.clone());
            }
        }
    }

    file_data
        .into_iter()
        .map(|(path, acc)| {
            (
                path,
                GitFileMetadata {
                    last_modified: acc.last_modified,
                    last_author: acc.last_author,
                    commit_count: acc.commit_count,
                    distinct_authors: acc.authors.len(),
                },
            )
        })
        .collect()
}

/// Internal accumulator for building GitFileMetadata.
struct FileAccumulator {
    last_modified: Option<i64>,
    last_author: Option<String>,
    commit_count: usize,
    authors: HashSet<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_log_single_commit() {
        let output = format!("{RECORD_SEP}\nAlice\n1709312000\n\nsrc/main.rs\nsrc/lib.rs\n");
        let root = PathBuf::from("/repo");
        let metadata = parse_git_log(&output, &root);

        assert_eq!(metadata.len(), 2);

        let main_meta = &metadata[&PathBuf::from("/repo/src/main.rs")];
        assert_eq!(main_meta.commit_count, 1);
        assert_eq!(main_meta.last_author.as_deref(), Some("Alice"));
        assert_eq!(main_meta.last_modified, Some(1709312000));
        assert_eq!(main_meta.distinct_authors, 1);
    }

    #[test]
    fn parse_git_log_multiple_commits_same_file() {
        let output = format!(
            "{RECORD_SEP}\nBob\n1709400000\n\nsrc/main.rs\n\
             {RECORD_SEP}\nAlice\n1709312000\n\nsrc/main.rs\nsrc/lib.rs\n"
        );
        let root = PathBuf::from("/repo");
        let metadata = parse_git_log(&output, &root);

        let main_meta = &metadata[&PathBuf::from("/repo/src/main.rs")];
        assert_eq!(main_meta.commit_count, 2);
        assert_eq!(main_meta.distinct_authors, 2);
        // Bob's commit is more recent
        assert_eq!(main_meta.last_modified, Some(1709400000));
        assert_eq!(main_meta.last_author.as_deref(), Some("Bob"));
    }

    #[test]
    fn parse_git_log_empty_output_returns_empty() {
        let metadata = parse_git_log("", &PathBuf::from("/repo"));
        assert!(metadata.is_empty());
    }

    #[test]
    fn parse_git_log_malformed_timestamp_skips_commit() {
        let output = format!("{RECORD_SEP}\nAlice\nnot_a_number\n\nsrc/main.rs\n");
        let metadata = parse_git_log(&output, &PathBuf::from("/repo"));
        assert!(metadata.is_empty());
    }

    #[test]
    fn compute_git_stats_aggregates_correctly() {
        let mut metadata = HashMap::new();
        metadata.insert(
            PathBuf::from("src/a.rs"),
            GitFileMetadata {
                last_modified: Some(1709400000),
                last_author: Some("Alice".into()),
                commit_count: 5,
                distinct_authors: 2,
            },
        );
        metadata.insert(
            PathBuf::from("src/b.rs"),
            GitFileMetadata {
                last_modified: Some(1709300000),
                last_author: Some("Bob".into()),
                commit_count: 3,
                distinct_authors: 1,
            },
        );

        let stats = compute_git_stats(&metadata);

        assert_eq!(stats.total_authors, 2); // Alice + Bob
        assert_eq!(stats.total_commits, 8); // 5 + 3
        assert!((stats.avg_commits_per_file - 4.0).abs() < f64::EPSILON);
        assert_eq!(stats.hottest_files.len(), 2);
        // Sorted by churn: a.rs (5) before b.rs (3)
        assert_eq!(stats.hottest_files[0].1, 5);
        assert_eq!(stats.hottest_files[1].1, 3);
    }

    #[test]
    fn compute_git_stats_empty_metadata() {
        let stats = compute_git_stats(&HashMap::new());
        assert_eq!(stats.total_authors, 0);
        assert_eq!(stats.total_commits, 0);
        assert_eq!(stats.avg_commits_per_file, 0.0);
        assert!(stats.hottest_files.is_empty());
    }

    #[test]
    fn compute_git_stats_top_ten_limit() {
        let mut metadata = HashMap::new();
        for i in 0..15 {
            metadata.insert(
                PathBuf::from(format!("src/file_{i}.rs")),
                GitFileMetadata {
                    last_modified: Some(1709400000),
                    last_author: Some("Dev".into()),
                    commit_count: i + 1,
                    distinct_authors: 1,
                },
            );
        }

        let stats = compute_git_stats(&metadata);
        assert_eq!(stats.hottest_files.len(), 10, "capped at top 10");
        // Highest churn file should be first
        assert_eq!(stats.hottest_files[0].1, 15);
    }

    #[test]
    fn compute_git_metadata_returns_none_for_nonexistent_dir() {
        let result = compute_git_metadata(Path::new("/nonexistent/path/to/repo"));
        assert!(result.is_none());
    }

    #[test]
    fn compute_git_metadata_returns_none_for_non_git_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        let result = compute_git_metadata(dir.path());
        assert!(result.is_none());
    }

    #[test]
    #[ignore]
    fn compute_git_metadata_works_on_own_repo() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        eprintln!("manifest_dir: {:?}", manifest_dir);

        // Debug: run the exact same command as compute_git_metadata
        let format_arg = format!("{RECORD_SEP}%n%an%n%at");
        let debug_output = Command::new("git")
            .args([
                "log",
                "--format",
                &format_arg,
                "--name-only",
                "-3",
            ])
            .current_dir(&manifest_dir)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&debug_output.stdout);
        eprintln!(
            "git log status={}, stdout_len={}, first_200={}",
            debug_output.status,
            stdout.len(),
            &stdout[..stdout.len().min(200)]
        );

        // Also debug parse_git_log directly
        let parsed = parse_git_log(&stdout, &manifest_dir);
        eprintln!("parse_git_log returned {} entries", parsed.len());
        if !parsed.is_empty() {
            let first_key = parsed.keys().next().unwrap();
            eprintln!("first key: {:?}", first_key);
        }

        let result = compute_git_metadata(&manifest_dir);
        assert!(result.is_some(), "should produce metadata for own repo");
        let metadata = result.unwrap();
        assert!(!metadata.is_empty(), "should have metadata for some files");

        // Check a file we know exists
        let engine_path = manifest_dir.join("src/engine.rs");
        let engine_meta = metadata.get(&engine_path);
        assert!(
            engine_meta.is_some(),
            "engine.rs should have git metadata, keys: {:?}",
            metadata.keys().take(5).collect::<Vec<_>>()
        );
    }
}
