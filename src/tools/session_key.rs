//! Conversation-identity resolution for the guide ledger.
//!
//! Deliberately free of I/O and `std::env`: the resolution ORDER is the part
//! worth testing, and tests must never mutate `environ` (see `ServerEnv`).

/// Which link produced the id. Carried for logging, not behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// `CODESCOUT_SESSION_ID` — the operator asserting an identity.
    Explicit,
    /// A known harness variable, e.g. `CLAUDE_CODE_SESSION_ID`.
    Harness,
}

/// A conversation identity, or the documented absence of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKey {
    /// Tier 1: persist the ledger under `id`.
    Keyed { id: String, source: KeySource },
    /// Tier 2: no identity exists. In-process only, bounded by an idle TTL.
    Anonymous,
}

impl SessionKey {
    /// The id to key storage by, if any.
    pub fn id(&self) -> Option<&str> {
        match self {
            SessionKey::Keyed { id, .. } => Some(id),
            SessionKey::Anonymous => None,
        }
    }
}

/// Harness variables probed, in order. Extend by adding one entry; nothing else
/// in the chain changes. Probed unconditionally — never gated on `clientInfo`.
pub const HARNESS_SESSION_VARS: &[&str] = &["CLAUDE_CODE_SESSION_ID"];

/// First non-empty wins: explicit, then each harness var in order, then
/// `Anonymous`. Values are trimmed; whitespace-only counts as absent.
pub fn resolve<I>(explicit: Option<String>, harness: I) -> SessionKey
where
    I: IntoIterator<Item = (&'static str, String)>,
{
    fn clean(v: String) -> Option<String> {
        let t = v.trim();
        (!t.is_empty()).then(|| t.to_string())
    }

    if let Some(id) = explicit.and_then(clean) {
        return SessionKey::Keyed {
            id,
            source: KeySource::Explicit,
        };
    }
    for (_name, value) in harness {
        if let Some(id) = clean(value) {
            return SessionKey::Keyed {
                id,
                source: KeySource::Harness,
            };
        }
    }
    SessionKey::Anonymous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_id_wins_over_every_harness_var() {
        let key = resolve(
            Some("explicit-1".to_string()),
            [("CLAUDE_CODE_SESSION_ID", "harness-1".to_string())],
        );
        assert_eq!(
            key,
            SessionKey::Keyed {
                id: "explicit-1".to_string(),
                source: KeySource::Explicit
            }
        );
    }

    #[test]
    fn a_harness_var_is_used_when_no_explicit_id_is_set() {
        let key = resolve(None, [("CLAUDE_CODE_SESSION_ID", "harness-1".to_string())]);
        assert_eq!(
            key,
            SessionKey::Keyed {
                id: "harness-1".to_string(),
                source: KeySource::Harness
            }
        );
    }

    #[test]
    fn harness_vars_are_probed_in_order_and_the_first_non_empty_wins() {
        // Kills a mutation that collects into a set, or takes the LAST match.
        let key = resolve(
            None,
            [("FIRST", "a".to_string()), ("SECOND", "b".to_string())],
        );
        assert_eq!(key.id(), Some("a"));
    }

    #[test]
    fn a_blank_or_whitespace_value_counts_as_absent_at_every_rank() {
        // Kills a mutation checking `is_some()` rather than non-empty-after-trim.
        let key = resolve(
            Some("   ".to_string()),
            [
                ("A", String::new()),
                ("B", "\t\n".to_string()),
                ("C", "real".to_string()),
            ],
        );
        assert_eq!(key.id(), Some("real"));
    }

    #[test]
    fn an_id_is_trimmed_before_use() {
        // A trailing newline is what a file-written id looks like; keying on the
        // untrimmed form silently splits one conversation across two ledgers.
        let key = resolve(Some("  abc-123\n".to_string()), []);
        assert_eq!(key.id(), Some("abc-123"));
    }

    #[test]
    fn no_source_at_all_is_anonymous_not_a_generated_id() {
        // Kills the old `unwrap_or_else(|| Uuid::new_v4())` tail, which persisted
        // a ledger under a key nothing could ever match again.
        assert_eq!(resolve(None, []), SessionKey::Anonymous);
    }
}
