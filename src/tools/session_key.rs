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

/// `_meta` keys probed, in order, for a per-request conversation identity.
///
/// A LIST rather than one key, for exactly the reason [`HARNESS_SESSION_VARS`]
/// is one: there is no reserved key to standardise on. `Mcp-Client-Session-Id`
/// via `params._meta` (modelcontextprotocol PR #2822) was closed for lacking
/// compelling use cases and redirected to `transports-wg#36`, so every sender
/// that exists is vendor-namespaced and an eventual standard key is one more
/// entry here rather than a replacement for this one. Extend by adding an
/// entry; nothing else in the chain changes.
///
/// **Two keys are deliberately NOT probed, both measured finer than a
/// conversation.** Note what this means: the list has no senders, but `_meta`
/// itself is populated in the wild.
///
/// - `claudecode/toolUseId` — Claude Code 2.1.270 sends it on EVERY `tools/call`,
///   beside `progressToken`. Identifies one tool call.
/// - `x-codex-turn-metadata` — Codex CLI (PR #15190, 2026-03-19). Identifies one
///   turn.
///
/// Keying the ledger on either would re-arm every topic on every call or every
/// turn — the over-delivery this ledger exists to prevent. A granularity
/// mismatch is worse than probing nothing, not a near-miss worth taking; both
/// are pinned in `no_sub_conversation_key_is_ever_probed`. Observed on the wire
/// 2026-09-13 against Claude Code 2.1.270 and Pi 0.85.1.
pub const CONVERSATION_META_KEYS: &[&str] = &["dev.codescout.mcp/conversationId"];

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

/// The conversation id a client asserted on this request's `_meta`, if any.
/// First non-empty key in [`CONVERSATION_META_KEYS`] wins, matching
/// [`resolve`]'s rule for the env chain.
///
/// `None` covers every "the client said nothing" shape — absent `_meta`, absent
/// key, a non-string value, or whitespace — and none of them is an error: a
/// client that sends nothing must behave exactly as it does today.
///
/// **No client populates these keys yet — and the precise claim matters, because
/// the shorter one is false.** `_meta` IS populated: Claude Code sends
/// `claudecode/toolUseId` and `progressToken` on every call; Codex CLI sends turn
/// metadata; Pi sends no `_meta` at all. What no client sends is a
/// CONVERSATION-scoped identifier. (Claude Code issue #76391 is the open ask for
/// one; #41836 is a neighbouring HTTP-scoped request that never mentions
/// `_meta`.) So this returns `None` on every live call today. It ships ahead of
/// its sender
/// because it is the only tier that can tell a subagent's call from its
/// parent's: they share the session id, the process and the connection, and
/// differ only in what a client could choose to stamp here.
pub fn conversation_from_meta(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<String> {
    let meta = meta?;
    CONVERSATION_META_KEYS.iter().find_map(|key| {
        let raw = meta.get(*key)?.as_str()?;
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
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

    fn meta_map(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn a_conversation_id_is_read_from_the_namespaced_meta_key() {
        let m = meta_map(&[(CONVERSATION_META_KEYS[0], serde_json::json!("conv-7"))]);
        assert_eq!(conversation_from_meta(Some(&m)), Some("conv-7".to_string()));
    }

    #[test]
    fn a_conversation_id_is_trimmed_like_every_other_rank() {
        let m = meta_map(&[(CONVERSATION_META_KEYS[0], serde_json::json!("  conv-7\n"))]);
        assert_eq!(conversation_from_meta(Some(&m)), Some("conv-7".to_string()));
    }

    #[test]
    fn every_shape_of_client_silence_yields_no_conversation() {
        // Five DISTINCT ways a client can say nothing. Asserted per-shape rather
        // than as one aggregate: a mutation that revives exactly one of them
        // (say, accepting a non-string) must still red, and an aggregate
        // "returns None somewhere" would absorb it.
        assert_eq!(conversation_from_meta(None), None, "no _meta at all");
        assert_eq!(
            conversation_from_meta(Some(&meta_map(&[]))),
            None,
            "_meta present but empty"
        );
        assert_eq!(
            conversation_from_meta(Some(&meta_map(&[(
                "progressToken",
                serde_json::json!("p")
            )]))),
            None,
            "_meta carries someone else's key, not ours"
        );
        assert_eq!(
            conversation_from_meta(Some(&meta_map(&[(
                CONVERSATION_META_KEYS[0],
                serde_json::json!(42)
            )]))),
            None,
            "our key, non-string value"
        );
        assert_eq!(
            conversation_from_meta(Some(&meta_map(&[(
                CONVERSATION_META_KEYS[0],
                serde_json::json!("   ")
            )]))),
            None,
            "our key, whitespace-only"
        );
    }

    #[test]
    fn the_conversation_meta_key_stays_vendor_namespaced() {
        // `_meta` is a SHARED protocol slot: a bare key would collide with the
        // next vendor to want one. Shape, not spelling — `transports-wg#36` may
        // yet rename ours, and that rename must not red.
        assert!(
            !CONVERSATION_META_KEYS.is_empty(),
            "an empty probe list makes conversation_from_meta dead code while \
             every other test here still passes — assert the inputs exist first"
        );
        for key in CONVERSATION_META_KEYS {
            assert!(
                key.contains('/') || key.contains('.') || key.contains('-'),
                "probe key must be vendor-namespaced, not bare: {key}"
            );
        }
    }

    #[test]
    fn no_sub_conversation_key_is_ever_probed() {
        // Keys OBSERVED on the wire (2026-09-13) that are finer than a
        // conversation. Probing any would re-arm every topic on every call or
        // turn — the over-delivery this ledger exists to prevent, and strictly
        // worse than probing nothing.
        //
        // A denylist rather than a substring heuristic, because
        // `claudecode/toolUseId` advertises nothing about its granularity in its
        // name — and it is the one someone would reach for, having looked at a
        // real `_meta` and correctly noticed it is populated on every call.
        const MEASURED_TOO_FINE: &[&str] = &[
            "claudecode/toolUseId",  // Claude Code 2.1.270 — per tool call
            "progressToken",         // per request
            "x-codex-turn-metadata", // Codex CLI — per turn
        ];
        for key in CONVERSATION_META_KEYS {
            assert!(
                !MEASURED_TOO_FINE.contains(key),
                "key is finer than a conversation and must not be probed: {key}"
            );
            assert!(
                !key.to_ascii_lowercase().contains("turn"),
                "a turn-granularity key must never be probed: {key}"
            );
        }
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
