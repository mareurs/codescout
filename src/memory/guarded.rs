//! The protections a memory mutation owes, in one place both callers reach.
//!
//! **Why this module exists rather than a method on [`MemoryStore`].** The store's
//! primitives are deliberately unguarded — `MemoryStore::write`'s own doc comment
//! records that wholesale replacement is specified behaviour that
//! `crate::tools::onboarding` depends on, and puts policy with "the caller that has a
//! user to warn and a `force` flag to offer". That reasoning is sound and is not what
//! failed. What failed is that there was no *shared* caller-side layer, so the policy
//! lived inline in one caller and a second caller was added beside it with none:
//!
//! - `POST /api/memories/{topic}` called `store.write` directly — no shrink guard, so a
//!   UI edit could truncate a memory to a fraction of its size with no refusal, and no
//!   anchor re-stamp, so the topic stayed reported `stale` right after being brought
//!   current.
//! - `DELETE /api/memories/{topic}` called `store.delete` directly, leaving the
//!   `.anchors.toml` sidecar orphaned — which the MCP path removes precisely so it stops
//!   surfacing in staleness scans.
//!
//! (`docs/issues/archive/2026-09-08-dashboard-memory-write-bypasses-the-shrink-guard-and-the-anchor-update.md`)
//!
//! Adding the missing calls to the dashboard handler would have fixed both symptoms and
//! left the shape that produced them: two hand-assembled copies, waiting for a third
//! caller to be added without either.
//!
//! **The ordering is the substance, not the grouping.** The shrink guard needs the OLD
//! content, so it must run *before* the write; the anchor update needs the NEW content,
//! so it must run *after*. Those constraints point in opposite directions, which is why
//! "call both from the handler" is a fix that can be written backwards and still compile
//! — guarding nothing while looking guarded. Here they cannot be reordered by a caller.

use crate::memory::{MemoryStore, ShrinkReport};
use std::path::Path;

/// Why a guarded write did not happen.
///
/// The refusal is returned rather than rendered because the two callers owe different
/// remedies: the MCP tool names its `force` parameter, the dashboard answers HTTP. A
/// message composed here would be wrong for one of them.
#[derive(Debug)]
pub enum GuardedWriteError {
    /// The shrink guard refused. Carries the report so the caller can show the numbers.
    Shrink(Box<ShrinkReport>),
    /// The write itself failed.
    Failed(anyhow::Error),
}

impl std::fmt::Display for GuardedWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shrink(r) => write!(f, "shrink guard refused: {r:?}"),
            Self::Failed(e) => write!(f, "{e}"),
        }
    }
}

/// Shrink-check, write, then re-stamp path anchors — the full contract of a memory write.
///
/// `anchor_root` is the project root anchors resolve against, and `None` skips the anchor
/// step. That is not an escape hatch: the private store has no anchors, and a caller that
/// cannot resolve a project root has nothing to resolve them against. It is spelled as an
/// option rather than a bool so a caller cannot ask for anchors without supplying what
/// they need.
///
/// Anchors are seeded beside the file that was actually written (`store.dir()`) rather
/// than from a re-resolved directory. Re-resolving is how the two could disagree.
///
/// Returns non-fatal warnings. Anchor failure does not fail the write — the content is
/// already on disk by then, so reporting it as a failed write would be a lie — but it is
/// returned rather than only logged, because a caller who asked to save something is
/// entitled to know a part of it silently degraded.
pub fn guarded_write(
    store: &MemoryStore,
    anchor_root: Option<&Path>,
    topic: &str,
    content: &str,
    force: bool,
) -> Result<Vec<String>, GuardedWriteError> {
    if !force {
        if let Some(report) = store.shrink_check(topic, content) {
            return Err(GuardedWriteError::Shrink(Box::new(report)));
        }
    }

    store
        .write(topic, content)
        .map_err(GuardedWriteError::Failed)?;

    let mut warnings = Vec::new();
    if let Some(root) = anchor_root {
        if let Err(e) =
            crate::memory::anchors::update_anchors_on_write(root, store.dir(), topic, content)
        {
            tracing::warn!("anchor update failed (non-fatal): {e}");
            warnings.push(format!("anchor update failed: {e}"));
        }
    }
    Ok(warnings)
}

/// Delete a memory and the anchor sidecar that outlives it.
///
/// The sidecar is the whole reason this is not `store.delete`. Left behind it orphans and
/// keeps surfacing in staleness scans — a topic that no longer exists, reported stale
/// forever, with nothing to bring current.
///
/// A missing sidecar is success, not an error: most topics never grow one.
pub fn guarded_delete(store: &MemoryStore, topic: &str) -> anyhow::Result<()> {
    store.delete(topic)?;

    let sidecar = crate::memory::anchors::anchor_path_for_topic(store.dir(), topic);
    match std::fs::remove_file(&sidecar) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!("failed to remove anchor sidecar {}: {e}", sidecar.display());
        }
    }
    Ok(())
}
