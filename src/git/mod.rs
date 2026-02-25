//! Git integration using `git2`.

pub mod blame;

use anyhow::Result;
use std::path::Path;

/// Open the git repository at or above `path`.
pub fn open_repo(path: &Path) -> Result<git2::Repository> {
    git2::Repository::discover(path)
        .map_err(|e| anyhow::anyhow!("No git repository found at {}: {}", path.display(), e))
}

/// Get the short commit hash of HEAD.
pub fn head_short_sha(repo: &git2::Repository) -> Result<String> {
    let head = repo.head()?;
    let commit = head.peel_to_commit()?;
    let id = commit.id();
    Ok(format!("{:.8}", id))
}

/// List the last `limit` commits for a file path.
pub fn file_log(
    repo: &git2::Repository,
    file: &Path,
    limit: usize,
) -> Result<Vec<CommitSummary>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut commits = vec![];
    for oid in revwalk.take(limit * 10) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        // Check if this commit touches the file
        if commit_touches_file(repo, &commit, file)? {
            commits.push(CommitSummary {
                sha: format!("{:.8}", commit.id()),
                message: commit
                    .summary()
                    .unwrap_or("<no message>")
                    .to_string(),
                author: commit.author().name().unwrap_or("unknown").to_string(),
                timestamp: commit.time().seconds(),
            });
            if commits.len() >= limit {
                break;
            }
        }
    }
    Ok(commits)
}

fn commit_touches_file(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    file: &Path,
) -> Result<bool> {
    let tree = commit.tree()?;
    let file_str = file.to_string_lossy();

    if commit.parent_count() == 0 {
        // Initial commit — check if file exists in tree
        return Ok(tree.get_path(file).is_ok());
    }

    let parent = commit.parent(0)?;
    let parent_tree = parent.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

    for delta in diff.deltas() {
        let touches = delta
            .new_file()
            .path()
            .map(|p| p == file)
            .unwrap_or(false)
            || delta
                .old_file()
                .path()
                .map(|p| p == file)
                .unwrap_or(false);
        if touches {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitSummary {
    pub sha: String,
    pub message: String,
    pub author: String,
    /// Unix timestamp
    pub timestamp: i64,
}
