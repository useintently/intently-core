//! FileTree: hierarchical directory structure with role classification,
//! aggregated statistics, and inter-directory dependencies.
//!
//! Reconstructs the physical directory hierarchy from the flat list of
//! analyzed files, enriching each node with:
//! - Role classification (Source, Test, Config, etc.) via majority vote
//! - Aggregated stats (file count, token estimate, language breakdown)
//! - Component affiliation from workspace layout (for monorepos)
//!
//! Inter-directory dependencies are derived from resolved `Reference` data,
//! grouping references by source/target directory and computing confidence
//! metrics.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::types::{FileRole, Reference};
use crate::parser::SupportedLanguage;
use crate::workspace::WorkspaceLayout;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Hierarchical view of the project's directory structure.
///
/// Contains a recursive tree of directories and files, plus
/// inter-directory dependency edges derived from resolved references.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileTree {
    /// Root directory node (name = "", path = "").
    pub root: DirectoryNode,
    /// Inter-directory dependencies sorted by (source_dir, target_dir).
    pub directory_dependencies: Vec<DirectoryDependency>,
}

/// A directory in the project hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectoryNode {
    /// Directory name (e.g. "src", "utils"). Root = "".
    pub name: String,
    /// Path relative to project root. Root = "".
    pub path: PathBuf,
    /// Direct child files, sorted by name.
    pub files: Vec<FileEntry>,
    /// Recursive subdirectories, sorted by name.
    pub subdirectories: Vec<DirectoryNode>,
    /// Inferred role based on majority vote of direct file roles.
    pub role: DirectoryRole,
    /// Aggregated statistics (recursive).
    pub stats: DirectoryStats,
    /// Workspace package name, if this directory is within a workspace package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_name: Option<String>,
}

/// Lightweight file summary within a directory node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    /// Filename only (no directory path).
    pub name: String,
    /// Path relative to project root.
    pub path: PathBuf,
    /// File role classification.
    pub role: FileRole,
    /// Detected programming language.
    pub language: SupportedLanguage,
    /// Estimated token count (bytes / 4).
    pub estimated_tokens: u64,
}

/// Classification of a directory's primary role.
///
/// Inferred by majority vote of direct child file roles. When there are
/// no direct files but subdirectories have differing roles, falls back
/// to `Mixed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryRole {
    Source,
    Test,
    Config,
    Documentation,
    Generated,
    Build,
    Mixed,
}

/// Aggregated statistics for a directory node (recursive).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectoryStats {
    /// Number of direct child files.
    pub direct_file_count: usize,
    /// Total file count including all descendants.
    pub total_file_count: usize,
    /// Total estimated tokens across all descendants.
    pub total_estimated_tokens: u64,
    /// Deduplicated, sorted list of languages found in descendants.
    pub languages: Vec<SupportedLanguage>,
    /// Depth in the tree (root = 0).
    pub depth: usize,
}

/// An inter-directory dependency edge.
///
/// Derived from resolved `Reference` data by grouping references whose
/// source and target files reside in different directories.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DirectoryDependency {
    /// Directory containing the reference source (relative to project root).
    pub source_dir: PathBuf,
    /// Directory containing the reference target (relative to project root).
    pub target_dir: PathBuf,
    /// Number of references between these directories.
    pub reference_count: usize,
    /// Average confidence across the references.
    pub avg_confidence: f64,
    /// Maximum confidence across the references.
    pub max_confidence: f64,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// File metadata collected from builder contributions.
pub(crate) struct FileMeta {
    pub path: PathBuf,
    pub role: FileRole,
    pub language: SupportedLanguage,
    pub estimated_tokens: u64,
}

/// Build a `FileTree` from file metadata and references.
///
/// This is the main entry point called from `CodeModelBuilder::build()`.
pub(crate) fn build_file_tree(
    all_files: &[FileMeta],
    all_references: &[&Reference],
    project_root: &Path,
    workspace_layout: Option<&WorkspaceLayout>,
) -> FileTree {
    // Phase 1: Build the recursive directory tree
    let root = build_tree_from_files(all_files, project_root, workspace_layout);

    // Phase 2: Aggregate directory dependencies from references
    let directory_dependencies = aggregate_directory_dependencies(all_references, project_root);

    FileTree {
        root,
        directory_dependencies,
    }
}

/// Intermediate directory node: contains files and child directories.
#[derive(Default)]
struct IntermediateDir {
    files: BTreeMap<String, FileEntry>,
    dirs: BTreeMap<String, IntermediateDir>,
}

/// Build the recursive `DirectoryNode` tree from flat file paths.
fn build_tree_from_files(
    all_files: &[FileMeta],
    project_root: &Path,
    workspace_layout: Option<&WorkspaceLayout>,
) -> DirectoryNode {
    let mut root = IntermediateDir::default();

    for meta in all_files {
        let relative = meta.path.strip_prefix(project_root).unwrap_or(&meta.path);

        let components: Vec<&str> = relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        if components.is_empty() {
            continue;
        }

        // Navigate/create intermediate directories
        let mut current = &mut root;
        for &dir_name in &components[..components.len() - 1] {
            current = current.dirs.entry(dir_name.to_string()).or_default();
        }

        // Insert the file at the leaf
        let file_name = components[components.len() - 1];
        let entry = FileEntry {
            name: file_name.to_string(),
            path: relative.to_path_buf(),
            role: meta.role,
            language: meta.language,
            estimated_tokens: meta.estimated_tokens,
        };
        current.files.insert(file_name.to_string(), entry);
    }

    // Convert the intermediate tree into DirectoryNode tree
    build_node_recursive(
        "",
        &PathBuf::new(),
        &root,
        0,
        workspace_layout,
        project_root,
    )
}

/// Recursively convert an `IntermediateDir` into a `DirectoryNode`.
fn build_node_recursive(
    name: &str,
    path: &Path,
    intermediate: &IntermediateDir,
    depth: usize,
    workspace_layout: Option<&WorkspaceLayout>,
    project_root: &Path,
) -> DirectoryNode {
    // Files are already sorted by name (BTreeMap iteration order)
    let files: Vec<FileEntry> = intermediate.files.values().cloned().collect();

    // Recurse into subdirectories (also sorted by BTreeMap key order)
    let subdirectories: Vec<DirectoryNode> = intermediate
        .dirs
        .iter()
        .map(|(child_name, child_dir)| {
            let child_path = if path.as_os_str().is_empty() {
                PathBuf::from(child_name)
            } else {
                path.join(child_name)
            };
            build_node_recursive(
                child_name,
                &child_path,
                child_dir,
                depth + 1,
                workspace_layout,
                project_root,
            )
        })
        .collect();

    let role = infer_directory_role(&files, &subdirectories);
    let stats = compute_directory_stats(&files, &subdirectories, depth);

    // Resolve component name from workspace layout using the absolute path
    let component_name = workspace_layout.and_then(|layout| {
        let abs_path = project_root.join(path);
        layout.component_for_path(&abs_path).map(|s| s.to_string())
    });

    DirectoryNode {
        name: name.to_string(),
        path: path.to_path_buf(),
        files,
        subdirectories,
        role,
        stats,
        component_name,
    }
}

/// Infer a directory's role from its direct files via majority vote.
///
/// - Counts each `FileRole` among direct files.
/// - Returns the most common role.
/// - Tie-break: `Source` wins (the project's default assumption).
/// - No direct files: if subdirectories exist and all share the same role,
///   propagate that role. Otherwise `Mixed`.
fn infer_directory_role(files: &[FileEntry], subdirs: &[DirectoryNode]) -> DirectoryRole {
    if files.is_empty() {
        // No direct files — infer from subdirectories
        if subdirs.is_empty() {
            return DirectoryRole::Source; // empty dir defaults to source
        }
        let first = subdirs[0].role;
        if subdirs.iter().all(|d| d.role == first) {
            return first;
        }
        return DirectoryRole::Mixed;
    }

    let mut counts: HashMap<FileRole, usize> = HashMap::new();
    for f in files {
        *counts.entry(f.role).or_insert(0) += 1;
    }

    let max_count = counts.values().copied().max().unwrap_or(0);

    // Collect all roles with the max count (ties)
    let mut candidates: Vec<FileRole> = counts
        .iter()
        .filter(|(_, &c)| c == max_count)
        .map(|(&role, _)| role)
        .collect();
    candidates.sort_by_key(|r| r.as_str().to_string());

    // Tie-break: prefer Implementation (maps to Source)
    let winner = if candidates.contains(&FileRole::Implementation) {
        FileRole::Implementation
    } else {
        candidates[0]
    };

    file_role_to_directory_role(winner)
}

/// Map a `FileRole` to the corresponding `DirectoryRole`.
fn file_role_to_directory_role(role: FileRole) -> DirectoryRole {
    match role {
        FileRole::Implementation => DirectoryRole::Source,
        FileRole::Test => DirectoryRole::Test,
        FileRole::Config => DirectoryRole::Config,
        FileRole::Documentation => DirectoryRole::Documentation,
        FileRole::Generated => DirectoryRole::Generated,
        FileRole::Build => DirectoryRole::Build,
        FileRole::Other => DirectoryRole::Source, // default to source
    }
}

/// Compute recursive statistics for a directory node.
fn compute_directory_stats(
    files: &[FileEntry],
    subdirs: &[DirectoryNode],
    depth: usize,
) -> DirectoryStats {
    let direct_file_count = files.len();

    let mut total_file_count = direct_file_count;
    let mut total_estimated_tokens: u64 = files.iter().map(|f| f.estimated_tokens).sum();
    let mut lang_set: HashMap<String, SupportedLanguage> = HashMap::new();

    for f in files {
        lang_set.insert(f.language.to_string(), f.language);
    }

    for sub in subdirs {
        total_file_count += sub.stats.total_file_count;
        total_estimated_tokens += sub.stats.total_estimated_tokens;
        for &lang in &sub.stats.languages {
            lang_set.insert(lang.to_string(), lang);
        }
    }

    let mut languages: Vec<SupportedLanguage> = lang_set.into_values().collect();
    languages.sort_by_key(|l| l.to_string());

    DirectoryStats {
        direct_file_count,
        total_file_count,
        total_estimated_tokens,
        languages,
        depth,
    }
}

/// Aggregate inter-directory dependencies from resolved references.
///
/// Groups references by (source_dir, target_dir) and computes count,
/// average confidence, and max confidence. Same-directory references
/// are excluded.
fn aggregate_directory_dependencies(
    refs: &[&Reference],
    project_root: &Path,
) -> Vec<DirectoryDependency> {
    let mut groups: BTreeMap<(PathBuf, PathBuf), Vec<f64>> = BTreeMap::new();

    for reference in refs {
        let target_file = match &reference.target_file {
            Some(f) => f,
            None => continue, // unresolved — skip
        };

        let source_dir = reference
            .source_file
            .strip_prefix(project_root)
            .unwrap_or(&reference.source_file)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();

        let target_dir = target_file
            .strip_prefix(project_root)
            .unwrap_or(target_file)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();

        // Same-directory references don't produce edges
        if source_dir == target_dir {
            continue;
        }

        groups
            .entry((source_dir, target_dir))
            .or_default()
            .push(reference.confidence);
    }

    groups
        .into_iter()
        .map(|((source_dir, target_dir), confidences)| {
            let reference_count = confidences.len();
            let sum: f64 = confidences.iter().sum();
            let avg_confidence = if reference_count > 0 {
                sum / reference_count as f64
            } else {
                0.0
            };
            let max_confidence = confidences.iter().copied().fold(0.0_f64, f64::max);

            DirectoryDependency {
                source_dir,
                target_dir,
                reference_count,
                avg_confidence,
                max_confidence,
            }
        })
        .collect()
}

/// Count total directories in a `FileTree` (including root).
pub(crate) fn count_directories(tree: &FileTree) -> usize {
    count_dirs_recursive(&tree.root)
}

fn count_dirs_recursive(node: &DirectoryNode) -> usize {
    1 + node
        .subdirectories
        .iter()
        .map(count_dirs_recursive)
        .sum::<usize>()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::ReferenceKind;

    fn meta(path: &str, role: FileRole, lang: SupportedLanguage, tokens: u64) -> FileMeta {
        FileMeta {
            path: PathBuf::from(path),
            role,
            language: lang,
            estimated_tokens: tokens,
        }
    }

    fn make_ref(source_file: &str, target_file: Option<&str>, confidence: f64) -> Reference {
        Reference {
            source_symbol: String::new(),
            source_file: PathBuf::from(source_file),
            source_line: 1,
            target_symbol: "target".into(),
            target_file: target_file.map(PathBuf::from),
            target_line: Some(1),
            reference_kind: ReferenceKind::Call,
            confidence,
            resolution_method: Default::default(),
            is_test_reference: false,
        }
    }

    #[test]
    fn builds_tree_from_single_flat_directory() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                200,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);

        assert_eq!(tree.root.files.len(), 2);
        assert_eq!(tree.root.files[0].name, "a.ts");
        assert_eq!(tree.root.files[1].name, "b.ts");
        assert!(tree.root.subdirectories.is_empty());
    }

    #[test]
    fn builds_tree_from_nested_directories() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/src/utils/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                50,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);

        assert_eq!(tree.root.subdirectories.len(), 1);
        let src = &tree.root.subdirectories[0];
        assert_eq!(src.name, "src");
        assert_eq!(src.files.len(), 1);
        assert_eq!(src.files[0].name, "a.ts");
        assert_eq!(src.subdirectories.len(), 1);

        let utils = &src.subdirectories[0];
        assert_eq!(utils.name, "utils");
        assert_eq!(utils.files.len(), 1);
        assert_eq!(utils.files[0].name, "b.ts");
    }

    #[test]
    fn empty_contributions_return_empty_root() {
        let tree = build_file_tree(&[], &[], Path::new("/project"), None);

        assert!(tree.root.files.is_empty());
        assert!(tree.root.subdirectories.is_empty());
        assert_eq!(tree.root.stats.total_file_count, 0);
    }

    #[test]
    fn files_sorted_by_name() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/z.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/m.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        let names: Vec<&str> = tree.root.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.ts", "m.ts", "z.ts"]);
    }

    #[test]
    fn subdirectories_sorted_by_name() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/zoo/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/alpha/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/mid/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        let names: Vec<&str> = tree
            .root
            .subdirectories
            .iter()
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "mid", "zoo"]);
    }

    #[test]
    fn directory_role_majority_vote() {
        let files = vec![
            FileEntry {
                name: "a.test.ts".into(),
                path: "a.test.ts".into(),
                role: FileRole::Test,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "b.test.ts".into(),
                path: "b.test.ts".into(),
                role: FileRole::Test,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "c.test.ts".into(),
                path: "c.test.ts".into(),
                role: FileRole::Test,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "helper.ts".into(),
                path: "helper.ts".into(),
                role: FileRole::Implementation,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
        ];

        let role = infer_directory_role(&files, &[]);
        assert_eq!(role, DirectoryRole::Test);
    }

    #[test]
    fn directory_role_tie_breaks_to_source() {
        let files = vec![
            FileEntry {
                name: "a.test.ts".into(),
                path: "a.test.ts".into(),
                role: FileRole::Test,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "b.test.ts".into(),
                path: "b.test.ts".into(),
                role: FileRole::Test,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "c.ts".into(),
                path: "c.ts".into(),
                role: FileRole::Implementation,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
            FileEntry {
                name: "d.ts".into(),
                path: "d.ts".into(),
                role: FileRole::Implementation,
                language: SupportedLanguage::TypeScript,
                estimated_tokens: 10,
            },
        ];

        let role = infer_directory_role(&files, &[]);
        assert_eq!(role, DirectoryRole::Source, "tie should break to Source");
    }

    #[test]
    fn directory_role_mixed_when_subdirs_differ() {
        let test_dir = DirectoryNode {
            name: "tests".into(),
            path: "tests".into(),
            files: vec![],
            subdirectories: vec![],
            role: DirectoryRole::Test,
            stats: DirectoryStats {
                direct_file_count: 0,
                total_file_count: 0,
                total_estimated_tokens: 0,
                languages: vec![],
                depth: 1,
            },
            component_name: None,
        };
        let src_dir = DirectoryNode {
            name: "src".into(),
            path: "src".into(),
            files: vec![],
            subdirectories: vec![],
            role: DirectoryRole::Source,
            stats: DirectoryStats {
                direct_file_count: 0,
                total_file_count: 0,
                total_estimated_tokens: 0,
                languages: vec![],
                depth: 1,
            },
            component_name: None,
        };

        let role = infer_directory_role(&[], &[test_dir, src_dir]);
        assert_eq!(role, DirectoryRole::Mixed);
    }

    #[test]
    fn stats_direct_count() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/src/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                200,
            ),
            meta(
                "/project/src/sub/c.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                50,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        let src = &tree.root.subdirectories[0];

        assert_eq!(src.stats.direct_file_count, 2, "src/ has 2 direct files");
        assert_eq!(
            src.subdirectories[0].stats.direct_file_count, 1,
            "src/sub/ has 1 direct file"
        );
    }

    #[test]
    fn stats_total_count_recursive() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/src/sub/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                50,
            ),
            meta(
                "/project/src/sub/deep/c.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                25,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        let src = &tree.root.subdirectories[0];

        assert_eq!(
            src.stats.total_file_count, 3,
            "src/ has 3 files total (recursive)"
        );
        assert_eq!(
            tree.root.stats.total_file_count, 3,
            "root has 3 files total"
        );
    }

    #[test]
    fn stats_tokens_recursive() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/src/sub/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                200,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        let src = &tree.root.subdirectories[0];

        assert_eq!(src.stats.total_estimated_tokens, 300);
        assert_eq!(tree.root.stats.total_estimated_tokens, 300);
    }

    #[test]
    fn stats_languages_deduplicated() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/c.py",
                FileRole::Implementation,
                SupportedLanguage::Python,
                10,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);

        assert_eq!(
            tree.root.stats.languages.len(),
            2,
            "should have 2 unique languages"
        );
    }

    #[test]
    fn dependency_aggregation() {
        let root = Path::new("/project");
        let ref1 = make_ref("/project/src/a.ts", Some("/project/lib/b.ts"), 0.9);
        let ref2 = make_ref("/project/src/c.ts", Some("/project/lib/d.ts"), 0.7);
        let refs: Vec<&Reference> = vec![&ref1, &ref2];

        let deps = aggregate_directory_dependencies(&refs, root);

        assert_eq!(deps.len(), 1, "both refs go src → lib");
        assert_eq!(deps[0].source_dir, PathBuf::from("src"));
        assert_eq!(deps[0].target_dir, PathBuf::from("lib"));
        assert_eq!(deps[0].reference_count, 2);
        assert!((deps[0].avg_confidence - 0.8).abs() < 0.01);
        assert!((deps[0].max_confidence - 0.9).abs() < 0.01);
    }

    #[test]
    fn dependency_same_dir_ignored() {
        let root = Path::new("/project");
        let ref1 = make_ref("/project/src/a.ts", Some("/project/src/b.ts"), 0.9);
        let refs: Vec<&Reference> = vec![&ref1];

        let deps = aggregate_directory_dependencies(&refs, root);

        assert!(deps.is_empty(), "same-dir refs should not produce edges");
    }

    #[test]
    fn component_name_from_workspace() {
        use crate::workspace::{WorkspaceKind, WorkspaceLayout, WorkspacePackage};

        let root = Path::new("/project");
        let layout = WorkspaceLayout {
            kind: WorkspaceKind::Pnpm,
            workspace_root: root.to_path_buf(),
            packages: vec![
                WorkspacePackage {
                    name: "api".into(),
                    root: PathBuf::from("/project/packages/api"),
                },
                WorkspacePackage {
                    name: "web".into(),
                    root: PathBuf::from("/project/packages/web"),
                },
            ],
        };

        let files = vec![
            meta(
                "/project/packages/api/src/index.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/packages/web/src/app.tsx",
                FileRole::Implementation,
                SupportedLanguage::Tsx,
                200,
            ),
            meta(
                "/project/README.md",
                FileRole::Documentation,
                SupportedLanguage::TypeScript,
                10,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, Some(&layout));

        // packages/api should have component_name = "api"
        let packages = &tree
            .root
            .subdirectories
            .iter()
            .find(|d| d.name == "packages")
            .unwrap();
        let api_dir = packages
            .subdirectories
            .iter()
            .find(|d| d.name == "api")
            .unwrap();
        assert_eq!(api_dir.component_name.as_deref(), Some("api"));

        let web_dir = packages
            .subdirectories
            .iter()
            .find(|d| d.name == "web")
            .unwrap();
        assert_eq!(web_dir.component_name.as_deref(), Some("web"));

        // Root README should not have a component name
        assert!(tree.root.component_name.is_none());
    }

    #[test]
    fn serde_round_trip() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                100,
            ),
            meta(
                "/project/src/b.py",
                FileRole::Implementation,
                SupportedLanguage::Python,
                50,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);

        let json = serde_json::to_string(&tree).expect("serialize");
        let deserialized: FileTree = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(tree, deserialized, "round-trip must preserve all data");
    }

    #[test]
    fn count_directories_accurate() {
        let root = Path::new("/project");
        let files = vec![
            meta(
                "/project/src/a.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/src/sub/b.ts",
                FileRole::Implementation,
                SupportedLanguage::TypeScript,
                10,
            ),
            meta(
                "/project/tests/c.ts",
                FileRole::Test,
                SupportedLanguage::TypeScript,
                10,
            ),
        ];

        let tree = build_file_tree(&files, &[], root, None);
        // root, src, src/sub, tests = 4 directories
        assert_eq!(count_directories(&tree), 4);
    }

    #[test]
    fn depth_tracked_correctly() {
        let root = Path::new("/project");
        let files = vec![meta(
            "/project/a/b/c/d.ts",
            FileRole::Implementation,
            SupportedLanguage::TypeScript,
            10,
        )];

        let tree = build_file_tree(&files, &[], root, None);

        assert_eq!(tree.root.stats.depth, 0);
        assert_eq!(tree.root.subdirectories[0].stats.depth, 1); // a
        assert_eq!(tree.root.subdirectories[0].subdirectories[0].stats.depth, 2); // b
        assert_eq!(
            tree.root.subdirectories[0].subdirectories[0].subdirectories[0]
                .stats
                .depth,
            3
        ); // c
    }
}
