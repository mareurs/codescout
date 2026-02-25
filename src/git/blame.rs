//! Line-level git blame.

use anyhow::Result;
use std::path::Path;

use super::open_repo;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BlameLine {
    pub line: usize,
    pub content: String,
    pub sha: String,
    pub author: String,
    pub timestamp: i64,
}

/// Return blame information for each line of `file` (relative to repo root).
pub fn blame_file(repo_path: &Path, file: &Path) -> Result<Vec<BlameLine>> {
    let repo = open_repo(repo_path)?;
    let blame = repo.blame_file(file, None)?;
    let source = std::fs::read_to_string(repo.workdir().unwrap_or(repo_path).join(file))?;

    let mut result = vec![];
    for (i, line_text) in source.lines().enumerate() {
        let hunk = blame
            .get_line(i + 1)
            .ok_or_else(|| anyhow::anyhow!("No blame hunk for line {}", i + 1))?;

        let sig = hunk.orig_signature();
        result.push(BlameLine {
            line: i + 1,
            content: line_text.to_string(),
            sha: format!("{:.8}", hunk.orig_commit_id()),
            author: sig.name().unwrap_or("unknown").to_string(),
            timestamp: sig.when().seconds(),
        });
    }
    Ok(result)
}
