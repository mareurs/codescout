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
/// Thin wrapper over `route_in`, bound to the live ledger. Kept as a
/// stable entry point — Task 5's render path already depends on this
/// exact signature — while the actual filter logic lives in `route_in`
/// below, where it can be tested against a synthetic corpus.
pub fn route(sel: Option<&str>, result: &Value) -> Vec<&'static Rule> {
    route_in(&OPERATOR_RULES, sel, result)
}

/// `route`'s filter logic, generic over the input slice's lifetime.
///
/// `always` rules are excluded unconditionally: they are resident in the
/// profile by construction, so routing one would deliver it twice and stamping
/// one would assert a per-session delivery event that did not occur.
///
/// `retired` rules are excluded on the same predicate `render_block` and
/// `check_budget` use, so a retirement takes effect on every path at once.
///
/// Split out from `route` so both filters can be exercised against a
/// synthetic corpus: the real ledger cannot produce a non-empty `serves`
/// on an `always` rule (`validate`, Gate 6, forbids it) or a retired
/// `triggered` rule with a matching selector (none exists in the ledger
/// today) — see `route_in_excludes_an_always_rule_even_when_its_selector_matches`
/// and `route_in_excludes_a_retired_triggered_rule` below.
fn route_in<'a>(rules: &'a [Rule], sel: Option<&str>, result: &Value) -> Vec<&'a Rule> {
    rules
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
    fn a_rule_with_several_selectors_routes_on_any_one_of_them() {
        // OP-2 declares `**Serves:** Agent, Task` — the ledger's only rule
        // with more than one selector. `serves.iter().any(...)` is what
        // routes it on either one; mutating that `.any` to `.all` would
        // require a single call's selector to equal both at once (never
        // true) and silently stop OP-2 from ever firing, while every other
        // test in this module stays green — no other rule has a second
        // selector to expose the bug. Both selectors are exercised here, not
        // just the first, so the second position in the list is confirmed
        // reachable too.
        for sel in ["Agent", "Task"] {
            let hit = route(Some(sel), &json!({"status": "ok"}));
            assert!(
                hit.iter().any(|r| r.id == "OP-2"),
                "{sel} must route OP-2; got {:?}",
                hit.iter().map(|r| &r.id).collect::<Vec<_>>()
            );
        }
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
        //
        // This documents intent against the real corpus, but it is not what
        // proves the `Binding::Triggered` filter load-bearing: OP-1 has an
        // empty `serves` (Gate 6 forbids an `always` rule from having any),
        // so the selector-match filter alone already excludes it — deleting
        // the binding filter leaves this test green. The mutation is caught
        // by `route_in_excludes_an_always_rule_even_when_its_selector_matches`,
        // which builds a synthetic `always` rule with a matching `serves` —
        // a combination `validate` forbids the real ledger from ever holding.
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
        // This documents intent against the real corpus, but it is not what
        // proves the `Status::Active` filter load-bearing: the ledger's only
        // retired rule, OP-5, is also `always` and so has an empty `serves`
        // — the inner loop below runs zero times for it, and the assertion
        // never executes. Deleting the status filter leaves this test green.
        // The mutation is caught by `route_in_excludes_a_retired_triggered_rule`,
        // which builds a synthetic retired `triggered` rule with a matching
        // `serves` — a combination the ledger does not contain today.
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

    /// A synthetic rule with a non-empty `serves` regardless of `binding` or
    /// `status` — a combination `validate` (Gate 6) forbids the real ledger
    /// from ever holding for `Binding::Always`, and one the ledger simply
    /// does not contain today for a retired `Binding::Triggered` rule. Exists
    /// so `route_in`'s binding/status filters can be tested in isolation from
    /// that constraint, rather than through the corpus that happens to make
    /// them coincide with the selector filter.
    fn synthetic_rule(id: &str, binding: Binding, status: Status) -> Rule {
        use crate::operator_rules::rule::{Evidence, Shape};
        Rule {
            id: id.into(),
            title: "synthetic".into(),
            imperative: "Do the synthetic thing.".into(),
            binding,
            shape: Shape::Imperative,
            covers: format!("synthetic-{id}"),
            serves: vec![crate::prompts::guide_index::parse_shape("route_in_test_tool").unwrap()],
            evidence: Evidence::Unmeasured,
            rests_on: None,
            status,
        }
    }

    #[test]
    fn route_in_excludes_an_always_rule_even_when_its_selector_matches() {
        let rules = vec![synthetic_rule(
            "SYN-ALWAYS",
            Binding::Always,
            Status::Active,
        )];
        let hit = route_in(&rules, Some("route_in_test_tool"), &json!({"status": "ok"}));
        assert!(
            hit.is_empty(),
            "an always rule must never route even when its selector matches; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn route_in_excludes_a_retired_triggered_rule() {
        let rules = vec![synthetic_rule(
            "SYN-RETIRED",
            Binding::Triggered,
            Status::Retired,
        )];
        let hit = route_in(&rules, Some("route_in_test_tool"), &json!({"status": "ok"}));
        assert!(
            hit.is_empty(),
            "a retired triggered rule must never route even when its selector matches; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn route_in_includes_a_triggered_active_rule() {
        // Positive control: without this, the two absence assertions above
        // would be equally satisfied by a `route_in` that always returns
        // nothing, regardless of its filters.
        let rules = vec![synthetic_rule(
            "SYN-ACTIVE",
            Binding::Triggered,
            Status::Active,
        )];
        let hit = route_in(&rules, Some("route_in_test_tool"), &json!({"status": "ok"}));
        assert!(
            hit.iter().any(|r| r.id == "SYN-ACTIVE"),
            "a triggered, active rule with a matching selector must route; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
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

    /// `OP-4`'s `path~` predicate cannot fire against a real write response.
    ///
    /// Pinned rather than fixed: write tools answer with the no-echo `"ok"`
    /// convention, and `names_path_containing` scans only `abs_path`/`rel_path`
    /// (top level and `items[]`) plus `violations[].path`. Giving writes a path
    /// field is a change to the no-echo convention, not a bug fix — see
    /// docs/issues/2026-08-28-op-4-path-predicate-can-never-fire.md
    ///
    /// **When this test starts failing, that is the fix landing.** Delete it and
    /// assert delivery instead; close the bug file.
    #[test]
    fn op_4s_path_predicate_cannot_fire_against_a_write_response_today() {
        let observed = json!({"status": "ok", "wrote_to": "/home/u/work/claude/codescout"});
        let hit = route(Some("edit_file"), &observed);
        assert!(
            !hit.iter().any(|r| r.id == "OP-4"),
            "OP-4 fired — the write-response shape gained a path field. This is the \
             GOOD failure: delete this test, assert delivery, and close the bug file."
        );
    }

    /// The same rule DOES fire once a response names the path — so the defect is
    /// the response shape, not the selector or the matcher.
    ///
    /// Without this cell the test above is indistinguishable from "OP-4's selector
    /// is malformed", which is a different bug with a different fix.
    #[test]
    fn op_4s_predicate_is_itself_sound_given_a_path_bearing_response() {
        let hit = route(
            Some("edit_file"),
            &json!({"abs_path": "/home/u/.claude/CLAUDE.md"}),
        );
        assert!(
            hit.iter().any(|r| r.id == "OP-4"),
            "OP-4's selector is broken independently of the response shape; got {:?}",
            hit.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }
}
