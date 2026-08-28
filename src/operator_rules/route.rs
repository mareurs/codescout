//! Selecting the `triggered` rules a call should receive.
//!
//! A second corpus fed to the section-grain matcher — not a second matcher.

use super::corpus::OPERATOR_RULES;
use super::rule::{Binding, Rule, Status};
use serde_json::Value;

/// Ledger key for a delivered `triggered` rule.
///
/// Spec § 5. `GuideLedger` stores opaque `String` keys, so a third namespace
/// needs no on-disk format change. Guide keys are `<topic>` and
/// `<topic>#<heading>`; the `op:` prefix keeps this disjoint from both, and
/// `op_keys_collide_with_no_guide_key` asserts it rather than trusting it.
pub fn ledger_key(id: &str) -> String {
    format!("op:{id}")
}

/// The `triggered`, `active` rules whose selector matches this call.
///
/// `always` rules are excluded unconditionally: they are resident in the
/// profile by construction, so routing one would deliver it twice and stamping
/// one would assert a per-session delivery event that did not occur.
///
/// `retired` rules are excluded on the same predicate `render_block` and
/// `check_budget` use, so a retirement takes effect on every path at once.
pub fn route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule> {
    OPERATOR_RULES
        .iter()
        .filter(|r| r.binding == Binding::Triggered && r.status == Status::Active)
        .filter(|r| r.serves.iter().any(|s| s.matches(sel, result)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_matching_selector_routes_its_rule() {
        // OP-3 declares `**Serves:** memory.write`.
        let hit = route(Some("memory.write"), &json!({"status": "ok"}));
        assert!(
            hit.iter().any(|r| r.id == "OP-3"),
            "memory.write must route OP-3; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_non_matching_selector_routes_nothing() {
        let hit = route(Some("grep"), &json!({"status": "ok"}));
        assert!(
            hit.is_empty(),
            "grep serves no rule; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_tool_that_opted_out_of_selector_key_routes_nothing() {
        // `Shape::matches` treats `None` as "cannot match" deliberately. A
        // wildcard here would deliver every triggered rule on every call from
        // every tool that has not opted in — the opposite of just-in-time.
        assert!(route(None, &json!({"status": "ok"})).is_empty());
    }

    #[test]
    fn always_rules_are_never_routed() {
        // OP-1 is `always`. It is resident in CLAUDE.md; routing it would
        // deliver it twice and contradict spec § 5.
        for sel in [
            "memory.write",
            "Agent",
            "Task",
            "edit_file",
            "artifact.update",
        ] {
            let hit = route(Some(sel), &json!({"abs_path": "/home/u/.claude/CLAUDE.md"}));
            assert!(
                !hit.iter().any(|r| r.id == "OP-1"),
                "OP-1 is `always` and must never route; fired on {sel}"
            );
        }
    }

    #[test]
    fn retired_rules_are_never_routed() {
        assert!(
            OPERATOR_RULES.iter().any(|r| r.status == Status::Retired),
            "this test is vacuous unless the ledger has at least one retired rule — \
             OP-5 was retired 2026-08-28; if it is gone, retire another or delete this"
        );
        for r in OPERATOR_RULES
            .iter()
            .filter(|r| r.status == Status::Retired)
        {
            for s in &r.serves {
                let sel = match &s.action {
                    Some(a) => format!("{}.{}", s.tool, a),
                    None => s.tool.clone(),
                };
                let hit = route(Some(&sel), &json!({"abs_path": "/home/u/.claude/x.md"}));
                assert!(
                    !hit.iter().any(|h| h.id == r.id),
                    "retired rule {} routed on its own selector {sel}",
                    r.id
                );
            }
        }
    }

    /// Gate 5, asserted directly.
    ///
    /// `GuideIndex.topics` is a private field, so this asserts against
    /// `GUIDE_INDEX.ledger_keys()` — an accessor added to `guide_index.rs`
    /// for exactly this test — rather than the field itself.
    #[test]
    fn op_keys_collide_with_no_guide_key() {
        use crate::prompts::guide_index::GUIDE_INDEX;
        let op_keys: Vec<String> = OPERATOR_RULES.iter().map(|r| ledger_key(&r.id)).collect();
        assert!(!op_keys.is_empty(), "no rules — the corpus failed to load");
        let guide_keys = GUIDE_INDEX.ledger_keys();
        assert!(
            !guide_keys.is_empty(),
            "no guide keys — the guide corpus failed to load"
        );
        for gk in &guide_keys {
            assert!(
                !op_keys.iter().any(|k| k == gk),
                "an op: key collides with guide key {gk}"
            );
        }
    }
}
