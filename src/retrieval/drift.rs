use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub chunk_id: String,
    pub content_hash: String,
    /// Forward-slashed project-relative path, as stored in the payload's
    /// `file_path`. Present so the dirty-set derivation can find paths that exist
    /// in an index but not on disk without parsing `chunk_id`.
    pub file_path: String,
}

#[derive(Debug, Default)]
pub struct DriftAction {
    pub to_upsert: Vec<String>,
    pub to_delete: Vec<String>,
}

pub fn diff_chunks(server: &[ChunkRef], local: &[ChunkRef]) -> DriftAction {
    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let local_ids: HashSet<&str> = local.iter().map(|c| c.chunk_id.as_str()).collect();
    let to_upsert = local
        .iter()
        .filter(|c| !server_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let to_delete = server
        .iter()
        .filter(|c| !local_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    DriftAction {
        to_upsert,
        to_delete,
    }
}

/// One chunk as it exists on disk right now.
#[derive(Debug, Clone)]
pub struct LocalChunk {
    /// Forward-slashed, project-relative -- identical in form to
    /// `ChunkRef.file_path`. `dirty_paths` compares this by exact string
    /// equality against `ChunkRef.file_path`; a platform-separator or
    /// absolute-path value here would make every file compare unequal, so the
    /// whole worktree is classified dirty AND the resulting `exclude_paths`
    /// match nothing in main's payloads -- main keeps serving stale chunks
    /// alongside the delta's fresh ones instead of failing cleanly.
    pub file_path: String,
    pub content_hash: String,
}

/// Which paths a worktree must not inherit from the main index, and which local
/// chunks belong in the worktree's delta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtySet {
    /// Paths to pass as `exclude_paths` when querying main's `project_id`.
    pub paths: BTreeSet<String>,
    /// Indices into the `local` slice: the chunks to embed under the delta id.
    pub to_embed: Vec<usize>,
}

/// Derive the worktree's dirty set by content, not by git.
///
/// A path is dirty when any of its `(file_path, content_hash)` pairs is on one
/// side and not the other -- present locally but not in main (added/modified),
/// or present in main but not locally (deleted). The check is symmetric on
/// purpose: a path-only check on the main side would miss a file that keeps one
/// surviving chunk while losing a sibling, since the surviving chunk's path is
/// still present on disk even though that exact chunk is gone. A file is served
/// by exactly one source -- main or the delta -- never both and never neither,
/// so a partially-changed file is embedded whole.
///
/// `main_refs` must come from `chunk_refs(collection, main_project_id)`; comparison
/// is on `(file_path, content_hash)` so it needs no base commit and inherits no
/// staleness window.
///
/// A `ChunkRef` whose payload lacked `file_path` (see `qdrant.rs`'s
/// `unwrap_or_default()` on that key) surfaces here as `file_path == ""`, which is
/// not a path that can exist on disk. Such a ref is skipped in the deletion check
/// below: unknown, never deleted. Classifying it as deleted would silently exclude
/// an arbitrary chunk from main's results with no signal of what happened.
pub fn dirty_paths(main_refs: &[ChunkRef], local: &[LocalChunk]) -> DirtySet {
    let main_pairs: HashSet<(&str, &str)> = main_refs
        .iter()
        .map(|r| (r.file_path.as_str(), r.content_hash.as_str()))
        .collect();
    let local_pairs: HashSet<(&str, &str)> = local
        .iter()
        .map(|c| (c.file_path.as_str(), c.content_hash.as_str()))
        .collect();

    let mut paths: BTreeSet<String> = BTreeSet::new();

    // Any local chunk whose exact bytes are not in main dirties its file.
    for c in local {
        if !main_pairs.contains(&(c.file_path.as_str(), c.content_hash.as_str())) {
            paths.insert(c.file_path.clone());
        }
    }
    // Any main ref whose exact bytes are not on disk dirties its file too -- this
    // is the symmetric check. A path-only check here (main path present on disk?)
    // is not enough: it misses a surviving chunk's sibling being deleted from the
    // same file, since the path alone can't tell that hB vanished while hA stayed.
    // Skip refs with an empty file_path -- unknown, not deleted (see doc comment
    // above).
    for r in main_refs {
        if !r.file_path.is_empty()
            && !local_pairs.contains(&(r.file_path.as_str(), r.content_hash.as_str()))
        {
            paths.insert(r.file_path.clone());
        }
    }
    // Every chunk of a dirty file goes to the delta, so the delta owns it whole.
    let to_embed = local
        .iter()
        .enumerate()
        .filter(|(_, c)| paths.contains(&c.file_path))
        .map(|(i, _)| i)
        .collect();

    DirtySet { paths, to_embed }
}

#[cfg(test)]
mod dirty_tests {
    use super::*;

    fn r(path: &str, hash: &str) -> ChunkRef {
        ChunkRef {
            chunk_id: format!("main:{path}:{hash}"),
            content_hash: hash.into(),
            file_path: path.into(),
        }
    }
    fn l(path: &str, hash: &str) -> LocalChunk {
        LocalChunk {
            file_path: path.into(),
            content_hash: hash.into(),
        }
    }

    #[test]
    fn unchanged_file_is_clean() {
        let d = dirty_paths(&[r("src/a.rs", "h1")], &[l("src/a.rs", "h1")]);
        assert!(
            d.paths.is_empty(),
            "byte-identical content must reuse main's vector"
        );
        assert!(d.to_embed.is_empty());
    }

    #[test]
    fn modified_file_is_dirty_and_queued() {
        let d = dirty_paths(&[r("src/a.rs", "h1")], &[l("src/a.rs", "h2")]);
        assert!(d.paths.contains("src/a.rs"));
        assert_eq!(
            d.to_embed,
            vec![0],
            "changed content must be embedded into the delta"
        );
    }

    #[test]
    fn file_absent_from_main_is_dirty_and_queued() {
        let d = dirty_paths(&[], &[l("src/new.rs", "h1")]);
        assert!(d.paths.contains("src/new.rs"));
        assert_eq!(d.to_embed, vec![0]);
    }

    #[test]
    fn file_in_main_but_absent_from_worktree_is_dirty_and_queues_nothing() {
        // The deletion case. Without this branch main keeps serving a file the
        // worktree deleted -- the exact confidently-stale outcome the design exists
        // to prevent, arriving through the back door.
        let d = dirty_paths(&[r("src/gone.rs", "h1")], &[]);
        assert!(
            d.paths.contains("src/gone.rs"),
            "a path in main but not on disk must be excluded from main's results"
        );
        assert!(
            d.to_embed.is_empty(),
            "there is nothing to embed for a deleted file"
        );
    }

    #[test]
    fn one_changed_chunk_dirties_the_whole_file() {
        // A file is served by exactly one source. If any chunk differs, the delta
        // owns the file, so every chunk of it must be embedded.
        let main = [r("src/a.rs", "h1"), r("src/a.rs", "h2")];
        let local = [l("src/a.rs", "h1"), l("src/a.rs", "hX")];
        let d = dirty_paths(&main, &local);
        assert!(d.paths.contains("src/a.rs"));
        assert_eq!(
            d.to_embed,
            vec![0, 1],
            "a partially-changed file must be embedded whole"
        );
    }

    #[test]
    fn deleted_sibling_chunk_dirties_the_file_even_when_a_chunk_survives() {
        // Mismatched granularity: main holds two chunks of the same file; the
        // worktree deletes the trailing symbol, leaving only the first chunk's
        // exact bytes on disk. A path-only check on the main side sees
        // "src/a.rs" still present locally and stops there -- it never notices
        // that main's second chunk (h2) has no counterpart on disk. That is the
        // "confidently stale" outcome this design exists to prevent: main would
        // keep serving the deleted symbol's chunk with full confidence.
        let main = [r("src/a.rs", "h1"), r("src/a.rs", "h2")];
        let local = [l("src/a.rs", "h1")];
        let d = dirty_paths(&main, &local);
        assert!(
            d.paths.contains("src/a.rs"),
            "a file missing one of main's chunks must be dirty even though another chunk survives"
        );
        assert_eq!(
            d.to_embed,
            vec![0],
            "the surviving local chunk must be re-embedded into the delta"
        );
    }

    #[test]
    fn empty_file_path_is_not_classified_as_deleted() {
        // A ChunkRef whose payload lacked `file_path` (qdrant.rs's
        // `unwrap_or_default()`) surfaces here as file_path == "". "" is not a
        // path that can exist on disk -- classifying it as deleted would exclude
        // an arbitrary chunk from main's results with no signal of what chunk it
        // was. Unknown must not be classified as deleted.
        let d = dirty_paths(&[r("", "h1")], &[]);
        assert!(
            !d.paths.contains(""),
            "an empty file_path must never be classified as deleted"
        );
        assert!(d.to_embed.is_empty());
    }
}
