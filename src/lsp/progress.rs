//! What the server says about its own start-up work (`$/progress`), and the decision it feeds:
//! how long a `null` answer is worth re-asking in the cold-start window.
//!
//! `request_until_answered` re-asks a not-answered request for a fixed budget (5 s cold). On a repo
//! with a lot of files rust-analyzer answers `documentSymbol` with `null` for much longer than that
//! while it scans (8 to 54 s measured, see
//! docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md).
//! The server announces that work, so a `null` is worth re-asking while it is working.
//!
//! The rule is deliberately NOT keyed on a phase name. Measured on a cold rust-analyzer:
//! - Progress events can be 17.7 s apart inside the null window, so "recent activity" ends too soon.
//! - A token stays open (`Indexing`, cache priming) long after symbols work, so "no token open" is
//!   far too strict; but the wait only runs while the answer is still `null`, so that is harmless.
//! - Between two scans there was a 0.28 s moment with no token open, so a token that closed a
//!   moment ago still counts ([`PROGRESS_DEBOUNCE`]).

use std::collections::HashSet;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// A token that closed less than this ago still counts as work in progress: one phase ends and the
/// next begins a moment later.
// cap-class: NOT_A_CAP — debounce between two progress phases; never part of a tool result
pub(crate) const PROGRESS_DEBOUNCE: Duration = Duration::from_secs(1);

/// The most a `null` answer is re-asked while the server reports progress. Below the server's 60 s
/// tool timeout, which also has to fit the rest of the call.
// cap-class: NOT_A_CAP — wait ceiling; exhaustion surfaces as the same "answered null" error as before
pub(crate) const PROGRESS_CEILING: Duration = Duration::from_secs(40);

#[derive(Default)]
struct Inner {
    /// Tokens that began (or were seen reporting) and have not ended.
    open: HashSet<String>,
    /// When a token last ended.
    last_closed: Option<Instant>,
}

/// The server's announced work, as seen on the `$/progress` notifications.
#[derive(Default)]
pub(crate) struct ProgressTracker {
    inner: StdMutex<Inner>,
}

impl ProgressTracker {
    /// Record `msg` if it is a `$/progress` notification; ignore anything else.
    pub(crate) fn observe(&self, msg: &Value, now: Instant) {
        if msg.get("method").and_then(Value::as_str) != Some("$/progress") {
            return;
        }
        let params = &msg["params"];
        // A token is a string or a number; keep them apart (`7` is not `"7"`).
        let Some(token) = params
            .get("token")
            .filter(|t| t.is_string() || t.is_number())
        else {
            return;
        };
        let token = token.to_string();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match params["value"]["kind"].as_str() {
            // A `report` for a token never seen to begin counts as open: a client that joins a
            // mux mid-scan sees the rest of a token whose `begin` went to an earlier client.
            Some("begin") | Some("report") => {
                inner.open.insert(token);
            }
            Some("end") => {
                inner.open.remove(&token);
                inner.last_closed = Some(now);
            }
            _ => {}
        }
    }

    /// True while some token is open, or one closed less than [`PROGRESS_DEBOUNCE`] ago.
    pub(crate) fn busy(&self, now: Instant) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        !inner.open.is_empty()
            || inner
                .last_closed
                .is_some_and(|closed| now.saturating_duration_since(closed) < PROGRESS_DEBOUNCE)
    }
}

/// Whether `request_until_answered` should ask again. Within `budget` it always does. Past it, it
/// does only for a `null` answer (an empty array is an answer), while the server reports work, and
/// before [`PROGRESS_CEILING`].
pub(crate) fn keep_waiting(
    elapsed: Duration,
    budget: Duration,
    was_null: bool,
    busy: bool,
) -> bool {
    elapsed < budget || (was_null && busy && elapsed < PROGRESS_CEILING)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn progress(token: Value, kind: &str) -> Value {
        json!({"jsonrpc": "2.0", "method": "$/progress",
               "params": {"token": token, "value": {"kind": kind}}})
    }

    #[test]
    fn a_begun_token_is_busy_until_it_ends_and_a_moment_after() {
        let tracker = ProgressTracker::default();
        let t0 = Instant::now();
        assert!(!tracker.busy(t0), "nothing announced yet");
        tracker.observe(&progress(json!("scan"), "begin"), t0);
        assert!(
            tracker.busy(t0 + Duration::from_secs(30)),
            "open for as long as it is open"
        );
        tracker.observe(
            &progress(json!("scan"), "end"),
            t0 + Duration::from_secs(30),
        );
        assert!(
            tracker.busy(t0 + Duration::from_millis(30_500)),
            "a token that closed 0.5 s ago still counts: the next phase begins a moment later"
        );
        assert!(
            !tracker.busy(t0 + Duration::from_millis(31_100)),
            "and 1.1 s after the last close nothing is left"
        );
    }

    #[test]
    fn a_report_for_a_token_never_seen_to_begin_counts_as_open() {
        // A client that connects to a mux mid-scan sees `report` and `end` for tokens whose
        // `begin` went to an earlier client.
        let tracker = ProgressTracker::default();
        let t0 = Instant::now();
        tracker.observe(&progress(json!("scan"), "report"), t0);
        assert!(tracker.busy(t0 + Duration::from_secs(10)));
        tracker.observe(
            &progress(json!("scan"), "end"),
            t0 + Duration::from_secs(10),
        );
        assert!(!tracker.busy(t0 + Duration::from_secs(12)));
    }

    #[test]
    fn string_and_number_tokens_are_distinct_and_an_unknown_end_is_harmless() {
        let tracker = ProgressTracker::default();
        let t0 = Instant::now();
        tracker.observe(&progress(json!(7), "begin"), t0);
        tracker.observe(&progress(json!("7"), "begin"), t0);
        tracker.observe(&progress(json!(7), "end"), t0);
        assert!(
            tracker.busy(t0 + Duration::from_secs(5)),
            "the string token \"7\" is still open"
        );
        tracker.observe(&progress(json!("never-begun"), "end"), t0);
        tracker.observe(&progress(json!("7"), "end"), t0);
        assert!(!tracker.busy(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn a_message_that_is_not_a_progress_notification_is_ignored() {
        let tracker = ProgressTracker::default();
        let t0 = Instant::now();
        for msg in [
            json!({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"message": "x"}}),
            json!({"jsonrpc": "2.0", "id": 1, "result": null}),
            json!({"jsonrpc": "2.0", "method": "$/progress", "params": {"value": {"kind": "begin"}}}),
        ] {
            tracker.observe(&msg, t0);
        }
        assert!(
            !tracker.busy(t0),
            "nothing above is a well-formed begin/report/end"
        );
    }

    #[test]
    fn keep_waiting_decides_inside_and_past_the_budget() {
        let budget = Duration::from_secs(5);
        let s = Duration::from_secs;
        // Inside the budget it always asks again, whatever else is true.
        assert!(keep_waiting(s(1), budget, false, false));
        // Past it: a null answer while the server works, before the ceiling.
        assert!(keep_waiting(s(6), budget, true, true));
        assert!(keep_waiting(s(39), budget, true, true));
        // Past it but the server reports no work: give up as before.
        assert!(!keep_waiting(s(6), budget, true, false));
        // Past it with an answer that is not null (an empty array): never extended.
        assert!(!keep_waiting(s(6), budget, false, true));
        // At the ceiling: stop, so the call stays under the server's tool timeout.
        assert!(!keep_waiting(PROGRESS_CEILING, budget, true, true));
        assert!(PROGRESS_CEILING < Duration::from_secs(60));
    }
}
