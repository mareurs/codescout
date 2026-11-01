//! What the red-attribution engine did on one `run_command` call — record-only.
//!
//! `usage.db`'s `attribution_outcome` and `attribution_ms` columns say whether the engine
//! started, how it ended and how long it took. A timeout is silent in the response on
//! purpose (a missing hint must never read as "nobody wrote it"), so without this sink a
//! reader of the database cannot tell a scan that ran out of time from a red that named no
//! dirty file. See `docs/superpowers/specs/2026-11-01-attribution-outcome-in-usage-db-design.md`.
//!
//! A value is a statement about WHAT THE ENGINE DID. It is never a statement about who wrote
//! a file, and nothing reads it back into behavior.
//!
//! The channel is a task-local rather than a `ToolContext` field for the reason
//! `observed::OBSERVED_SINK` records: `ToolContext` is built by bare struct literal at well
//! over a hundred sites. Like that sink it holds only while no `tokio::spawn` separates
//! `UsageRecorder::record_content` from the tool body; a spawn there would make [`record`] a
//! silent no-op and every row NULL.

use std::cell::RefCell;

/// How the engine ended. NULL in the column (no value recorded) means it did not start:
/// exit 0 with nothing failure-shaped in the output, or the opt-out. Any NON-ZERO exit starts
/// the engine whatever the text says, so a failed `ls` or a `grep` with no match is `Silent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The engine finished and printed a hint: any text other than its own failure notice.
    Hinted,
    /// The engine finished and printed nothing. That covers a clean tree, a red that names no
    /// path, a red whose named paths are not dirty (each an early exit of about 0.1 s), and a
    /// full scan with no output. A non-zero exit starts the engine whatever the text says, so a
    /// failed `ls` or a `grep` with no match lands here too. `attribution_ms` separates the cases.
    Silent,
    /// The ceiling passed before the engine finished. The response says nothing.
    TimedOut,
    /// The engine exited non-zero, or the wait on it failed, or it exited 0 with its own
    /// failure notice ("red attribution is unavailable").
    ToolingFailure,
    /// The engine could not be extracted or spawned.
    SpawnFailed,
}

impl Outcome {
    /// The exact text stored in `tool_calls.attribution_outcome`. A contract with every
    /// reader of the database: change a value and the readers' queries go quiet.
    pub(crate) fn as_column(self) -> &'static str {
        match self {
            Outcome::Hinted => "hinted",
            Outcome::Silent => "silent",
            Outcome::TimedOut => "timed_out",
            Outcome::ToolingFailure => "tooling_failure",
            Outcome::SpawnFailed => "spawn_failed",
        }
    }
}

/// One call's recorded engine outcome and the wall time it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Recorded {
    pub outcome: Outcome,
    pub elapsed_ms: i64,
}

tokio::task_local! {
    /// Scoped around exactly one tool call's future by `UsageRecorder::record_content` and
    /// drained there. Inside a scope `None` means the engine did not start, which becomes
    /// the columns' NULL.
    pub(crate) static ATTRIBUTION_SINK: RefCell<Option<Recorded>>;
}

/// Record how the engine ended and how long it took.
///
/// Replaces, never merges: the last writer owns the call's record. Outside a scope it does
/// nothing, so a test calling the engine directly must not panic.
pub(crate) fn record(outcome: Outcome, elapsed_ms: i64) {
    let _ = ATTRIBUTION_SINK.try_with(|sink| {
        *sink.borrow_mut() = Some(Recorded {
            outcome,
            elapsed_ms,
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Outside a scope `record` is inert, and does not panic, which a bare
    /// `ATTRIBUTION_SINK.with` would. Every direct `wip_author_diagnostic*` test that
    /// runs without a recorder takes this path.
    #[test]
    fn record_outside_a_scope_is_a_silent_no_op() {
        record(Outcome::TimedOut, 5);
    }

    /// Inside a scope the sink starts empty, holds what was recorded, and a later record
    /// replaces an earlier one.
    #[tokio::test]
    async fn a_later_record_replaces_an_earlier_one_and_an_unscoped_sink_starts_empty() {
        let seen = ATTRIBUTION_SINK
            .scope(RefCell::new(None), async {
                let before = ATTRIBUTION_SINK.with(|s| *s.borrow());
                record(Outcome::Hinted, 7);
                let first = ATTRIBUTION_SINK.with(|s| *s.borrow());
                record(Outcome::TimedOut, 31_000);
                let second = ATTRIBUTION_SINK.with(|s| *s.borrow());
                (before, first, second)
            })
            .await;
        assert_eq!(
            seen,
            (
                None,
                Some(Recorded {
                    outcome: Outcome::Hinted,
                    elapsed_ms: 7
                }),
                Some(Recorded {
                    outcome: Outcome::TimedOut,
                    elapsed_ms: 31_000
                }),
            )
        );
    }

    /// The column's five values are a contract with every reader of `usage.db`: exact
    /// text, five distinct members, in the order the design lists them.
    #[test]
    fn the_five_columns_values_are_exact_and_distinct() {
        let all = [
            Outcome::Hinted,
            Outcome::Silent,
            Outcome::TimedOut,
            Outcome::ToolingFailure,
            Outcome::SpawnFailed,
        ];
        let texts: Vec<&str> = all.iter().map(|o| o.as_column()).collect();
        assert_eq!(
            texts,
            [
                "hinted",
                "silent",
                "timed_out",
                "tooling_failure",
                "spawn_failed"
            ]
        );
        let distinct: std::collections::HashSet<_> = texts.iter().collect();
        assert_eq!(distinct.len(), 5);
    }

    /// Two calls in flight on one runtime each keep their own value: a task-local is
    /// per future, so an interleaving at an await point cannot mix them.
    #[tokio::test]
    async fn two_concurrent_scopes_keep_their_own_value() {
        let one = ATTRIBUTION_SINK.scope(RefCell::new(None), async {
            record(Outcome::Hinted, 1);
            tokio::task::yield_now().await;
            ATTRIBUTION_SINK.with(|s| *s.borrow())
        });
        let two = ATTRIBUTION_SINK.scope(RefCell::new(None), async {
            record(Outcome::SpawnFailed, 2);
            tokio::task::yield_now().await;
            ATTRIBUTION_SINK.with(|s| *s.borrow())
        });
        let (a, b) = tokio::join!(one, two);
        assert_eq!(
            a,
            Some(Recorded {
                outcome: Outcome::Hinted,
                elapsed_ms: 1
            })
        );
        assert_eq!(
            b,
            Some(Recorded {
                outcome: Outcome::SpawnFailed,
                elapsed_ms: 2
            })
        );
    }
}
