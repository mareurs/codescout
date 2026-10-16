//! The OBSERVED half of a `run_command` call's effect — record-only.
//!
//! `usage.db`'s `effect_class` column holds what the caller DECLARED (`read` or
//! `write`). A wrong declaration is invisible in that column alone; this sink carries the
//! one observation that can contradict it — "the working tree changed while a command
//! declared `read` ran" — to `observed_changed`.
//!
//! The channel is a task-local rather than a `ToolContext` field for the reason
//! `deliveries::DELIVERY_SINK` records: `ToolContext` is built by bare struct literal at
//! well over a hundred sites. Like that sink it holds only while no `tokio::spawn`
//! separates `UsageRecorder::record_content` from the tool body; a spawn there would make
//! [`record`] a silent no-op and every row NULL.
//!
//! What a value means is stated on the column's migration in `db::open_db`, because that
//! is where a reader of the database lands. In short: `true` is "something under the
//! project's git-visible tree changed during the call", NOT "this command wrote" — on a
//! checkout shared with peer sessions the change may be theirs.

use std::cell::RefCell;

tokio::task_local! {
    /// Scoped around exactly one tool call's future by `UsageRecorder::record_content`
    /// and drained there. Inside a scope `None` means no observation was made (the tool
    /// is not `run_command`, the call declared `write`, the snapshot failed, ...), which
    /// becomes the column's NULL. `Some(false)` is an observation of NO CHANGE IN WHAT THE
    /// SNAPSHOT SEES, which a wrong `read` declaration can still produce (gitignored paths,
    /// writes outside the repo, a reverted change); see `run_command::observe`.
    pub(crate) static OBSERVED_SINK: RefCell<Option<bool>>;
}

/// Record that the tree did (`true`) or did not (`false`) change during this call.
///
/// Replaces, never ORs: the last writer owns the call's observation. Outside a scope it
/// does nothing, so a test calling a tool directly must not panic.
pub(crate) fn record(changed: bool) {
    let _ = OBSERVED_SINK.try_with(|sink| *sink.borrow_mut() = Some(changed));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outside a scope `record` is inert — and does not panic, which a bare
    /// `OBSERVED_SINK.with` would. Every direct `RunCommand.call` test runs this path.
    #[test]
    fn record_outside_a_scope_is_a_silent_no_op() {
        record(true);
    }

    /// Inside a scope the value round-trips, `false` stays distinct from "nothing was
    /// recorded", and a later record replaces an earlier one.
    #[tokio::test]
    async fn the_sink_distinguishes_unchanged_from_unobserved() {
        let drained = OBSERVED_SINK
            .scope(RefCell::new(None), async {
                let before = OBSERVED_SINK.with(|s| *s.borrow());
                record(false);
                let unchanged = OBSERVED_SINK.with(|s| *s.borrow());
                record(true);
                let changed = OBSERVED_SINK.with(|s| *s.borrow());
                (before, unchanged, changed)
            })
            .await;
        assert_eq!(drained, (None, Some(false), Some(true)));
    }
}
