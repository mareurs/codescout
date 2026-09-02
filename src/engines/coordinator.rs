//! The fan-out that decides what a tool response carries, Layer 2 of
//! `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`.
//!
//! Post phase only. This module owns **ordering**, **corpus exclusivity** and
//! the **idle re-arm tick**; it owns no knowledge of guides, rules, or the
//! `Tool` trait. Each engine's body lives in [`super::emitters`].

use super::{Corpus, EngineDecl, ENGINES};
use crate::tools::guide_emit::GuideDeliveryShape;
use crate::tools::guide_ledger::GuideLedger;
use rmcp::model::Content;
use serde_json::Value;

/// Everything a post-phase engine may read about the call.
///
/// `content_topic` is resolved by the caller rather than by asking the tool,
/// for two reasons: the coordinator must not depend on the `Tool` trait, and
/// a default trait method cannot coerce `&self` to `&dyn Tool` anyway.
pub(crate) struct PostCtx<'a> {
    pub selector: Option<&'a str>,
    pub value: &'a Value,
    /// `Tool::relevant_guide_topic(value)`.
    pub content_topic: Option<&'a str>,
    /// Whether the progressive-disclosure gate fires:
    /// `exceeds_inline_limit(&json) || output_id is a STRING`, exactly as
    /// computed where `call_content` builds this struct — the `overflowing:`
    /// field of its `PostCtx { … }` literal. Precomputed because deciding it
    /// requires the serialised JSON, which the coordinator does not hold.
    ///
    /// Read `is a string` literally: the gate is `.and_then(|v| v.as_str())`,
    /// so a present-but-non-string `output_id` does **not** fire it. No
    /// producer emits a non-string today, which is what makes transcribing
    /// this as `get("output_id").is_some()` a divergence nothing would catch.
    ///
    /// **Not** the same condition as the separate buffering decision —
    /// `call_content`'s `let primary = if exceeds_inline_limit(&json) &&
    /// !self.force_inline()`, a few lines below the construction. That one
    /// carries a `force_inline` term, this gate does not. A `force_inline`
    /// tool whose JSON exceeds the inline limit is never buffered, but this
    /// field must still be `true` for it, because the disjunction above never
    /// consults `force_inline` either. Latent today (the only `force_inline`
    /// tool, `get_guide`, declares no `relevant_guide_topic`, so
    /// `emit_guide_sections` never reaches the check this field feeds) —
    /// computing `overflowing` from the buffering decision instead is a
    /// silent byte diff with no dedicated detector.
    ///
    /// Both references above name an **expression**, not a line. This doc
    /// carried three `types.rs:<line>` citations until Plan 3, and the commit
    /// that wired the coordinator invalidated all three — one of them
    /// (`:1162`) pointing past a file that is now 1112 lines long. A line
    /// number is a claim about a revision; the expression survives the edit.
    pub overflowing: bool,
}

/// One engine's contribution to one response.
#[derive(Default)]
pub(crate) struct Emission {
    /// Drives the single legacy `_guide_hint` field on the primary block.
    /// There is exactly one such field, so at most one hint survives a
    /// response — see `run_post_in`.
    pub hint: Option<(String, GuideDeliveryShape)>,
    pub blocks: Vec<Content>,
}

impl Emission {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by emitters::tests to assert on an Emission's emptiness now \
                    that Plan 2 wired the emitters; run_post_in itself never calls it \
                    — it checks claimed-and-empty through Emitted::Claimed's own match \
                    arm, not this method — so it stays a test-only helper until some \
                    future caller reads it in production"
        )
    )]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Whether an engine's trigger fired — which is **not** the same question as
/// whether it produced bytes.
///
/// The distinction is load-bearing and pre-dates this module. The session
/// opener's pre-refactor branch returned `(None, Vec::new())` when its topic
/// failed to build, rather than falling through to guide-sections: a fired
/// trigger that ships nothing still spends the call. Collapsing these two into
/// `Vec<Content>` would silently restore the fall-through.
pub(crate) enum Emitted {
    /// Trigger did not fire. Later engines in the same corpus may still run.
    Declined,
    /// Trigger fired. No later engine in the same corpus runs, even if this
    /// emission is empty.
    Claimed(Emission),
}

/// `run_post`'s logic, generic over the engine slice.
///
/// Split out so the ordering rules can be exercised against synthetic engines
/// — the live registry cannot produce a claimed-but-empty first engine, and
/// that is exactly the case worth pinning. Same idiom, same reason, as
/// `operator_rules::route::route_in`.
pub(crate) fn run_post_in(
    engines: &[EngineDecl],
    ctx: &PostCtx<'_>,
    ledger: &mut GuideLedger,
) -> Emission {
    // Anonymous-tier idle re-arm, coordinator-level and FIRST. The session
    // opener's trigger reads the ledger this may re-arm, so running the tick
    // inside any one engine would order it against that engine alone.
    let rearmed = ledger.tick();
    if rearmed > 0 {
        tracing::debug!("anonymous guide ledger idle TTL re-armed {rearmed} topic(s)");
    }

    let mut out = Emission::default();
    let mut claimed: Vec<Corpus> = Vec::new();
    for engine in engines {
        let Some(emit) = engine.emit_post else {
            continue;
        };
        if claimed.contains(&engine.corpus) {
            continue;
        }
        let Emitted::Claimed(e) = emit(ctx, ledger) else {
            continue;
        };
        claimed.push(engine.corpus);
        if out.hint.is_none() {
            out.hint = e.hint;
        }
        out.blocks.extend(e.blocks);
    }
    out
}

/// The live fan-out, bound to [`ENGINES`].
pub(crate) fn run_post(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emission {
    run_post_in(ENGINES, ctx, ledger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::{Mode, RetrievalKey};
    use crate::tools::guide_ledger::GuideLedger;
    use rmcp::model::Content;
    use serde_json::json;

    fn owns_nothing(_k: &str) -> bool {
        false
    }

    fn decl(
        id: &'static str,
        corpus: Corpus,
        emit: fn(&PostCtx<'_>, &mut GuideLedger) -> Emitted,
    ) -> EngineDecl {
        EngineDecl {
            id,
            key: RetrievalKey::CallShape,
            corpus,
            mode: Mode::Push,
            writes_at: &[],
            owns_key: owns_nothing,
            emit_post: Some(emit),
        }
    }

    fn claims_a_block(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission {
            hint: Some(("first".into(), GuideDeliveryShape::Whole)),
            blocks: vec![Content::text("FIRST")],
        })
    }
    fn claims_nothing(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission::default())
    }
    fn declines(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Declined
    }
    fn claims_second(_c: &PostCtx<'_>, _l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission {
            hint: Some(("second".into(), GuideDeliveryShape::Whole)),
            blocks: vec![Content::text("SECOND")],
        })
    }

    fn reports_ledger_state(_c: &PostCtx<'_>, l: &mut GuideLedger) -> Emitted {
        Emitted::Claimed(Emission {
            hint: None,
            blocks: vec![Content::text(if l.contains("stale") {
                "STILL-THERE"
            } else {
                "REARMED"
            })],
        })
    }

    fn ctx<'a>(v: &'a serde_json::Value) -> PostCtx<'a> {
        PostCtx {
            selector: Some("t.a"),
            value: v,
            content_topic: None,
            overflowing: false,
        }
    }

    fn texts(e: &Emission) -> Vec<String> {
        e.blocks
            .iter()
            .filter_map(|b| b.as_text().map(|t| t.text.clone()))
            .collect()
    }

    /// The load-bearing one. A CLAIMED-but-empty engine still spends its
    /// corpus: the pre-refactor `if/else` returned `(None, Vec::new())` when
    /// the opener's `guide_block` came back `None`, rather than falling
    /// through to guide-sections. Mutating `Emitted::Claimed(e) if
    /// e.is_empty() => continue` reds this test and only this test.
    #[test]
    fn a_claim_spends_its_corpus_even_when_it_emits_nothing() {
        let v = json!({});
        let engines = [
            decl("empty-claimer", Corpus::CompiledGuides, claims_nothing),
            decl("would-emit", Corpus::CompiledGuides, claims_a_block),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert!(texts(&out).is_empty(), "got {:?}", texts(&out));
        assert!(out.hint.is_none());
    }

    #[test]
    fn a_decline_passes_the_corpus_to_the_next_engine() {
        let v = json!({});
        let engines = [
            decl("decliner", Corpus::CompiledGuides, declines),
            decl("would-emit", Corpus::CompiledGuides, claims_a_block),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["FIRST".to_string()]);
    }

    #[test]
    fn engines_in_different_corpora_both_emit_in_registry_order() {
        let v = json!({});
        let engines = [
            decl("guides", Corpus::CompiledGuides, claims_a_block),
            decl("rules", Corpus::OperatorLedger, claims_second),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["FIRST".to_string(), "SECOND".to_string()]);
    }

    /// The primary block carries exactly one `_guide_hint` field, so a second
    /// hint has nowhere to go. The rule is FIRST NON-`None` HINT WINS, not
    /// strictly "first claimant": a claim whose `Emission::hint` is `None`
    /// (an operator-rules claim carrying no guide hint, for instance) does
    /// NOT suppress a later engine's hint — `run_post_in` only skips the
    /// assignment once `out.hint` already holds `Some`. Both of this test's
    /// emitters carry a hint, so it cannot distinguish "first claimant wins"
    /// from "first non-`None` hint wins"; see `run_post_in` for the actual
    /// rule and its rationale.
    #[test]
    fn only_the_first_hint_survives() {
        let v = json!({});
        let engines = [
            decl("guides", Corpus::CompiledGuides, claims_a_block),
            decl("rules", Corpus::OperatorLedger, claims_second),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(out.hint.map(|(t, _)| t), Some("first".to_string()));
    }

    /// FIRST NON-`None` HINT WINS, not "first claimant wins" — the two are
    /// indistinguishable when every claimant carries a hint, which is why
    /// `only_the_first_hint_survives` above cannot pin the actual rule. Here
    /// the first claimant (`claims_nothing`, a different corpus so it does
    /// not block the second engine from running) carries no hint at all, so
    /// only the SECOND engine's hint can produce a `Some` below.
    ///
    /// The isolating mutation is at the assignment gate itself
    /// (`coordinator.rs`'s `run_post_in`): changing `if out.hint.is_none()`
    /// to `if claimed.len() == 1` — NOT breaking the loop, just changing
    /// which claim's hint is allowed to land. Both conditions agree on every
    /// *other* test in this module (each has at most one claim whose hint
    /// matters, so "first claim" and "hint not yet set" coincide), which is
    /// exactly why this mutation is isolating: verified by hand-mutating
    /// `run_post_in` to that gate and re-running this module's tests —
    /// `a_hintless_claim_does_not_block_a_later_hint` reds (`out.hint` comes
    /// back `None` instead of `Some("first")`) while `only_the_first_hint_survives`
    /// and `engines_in_different_corpora_both_emit_in_registry_order` both
    /// still pass. (An earlier report of this mutation described inserting a
    /// loop-`break` on `claimed.len() == 1` instead — that is a different,
    /// blunter mutation that also reds `engines_in_different_corpora_both_emit_in_registry_order`,
    /// so it proved nothing about this test's unique contribution.)
    #[test]
    fn a_hintless_claim_does_not_block_a_later_hint() {
        let v = json!({});
        let engines = [
            decl("rules", Corpus::OperatorLedger, claims_nothing),
            decl("guides", Corpus::CompiledGuides, claims_a_block),
        ];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(out.hint.map(|(t, _)| t), Some("first".to_string()));
    }

    /// An engine with no emitter is skipped, not treated as a decline that
    /// claims nothing — `craft-skills` is `Mode::Unmanaged` and must not be
    /// able to spend a corpus it does not draw from.
    #[test]
    fn an_engine_without_an_emitter_is_skipped() {
        let v = json!({});
        let mut unwired = decl("unwired", Corpus::CompiledGuides, claims_a_block);
        unwired.emit_post = None;
        let engines = [unwired, decl("real", Corpus::CompiledGuides, claims_second)];
        let out = run_post_in(&engines, &ctx(&v), &mut GuideLedger::default());
        assert_eq!(texts(&out), vec!["SECOND".to_string()]);
    }

    /// The idle re-arm tick is coordinator-level and FIRST, not merely
    /// present somewhere in `run_post_in` — the session opener's trigger
    /// reads the ledger this tick may re-arm (see `run_post_in`'s doc
    /// comment), so running it after even one engine reorders it against
    /// that engine's own read. A topic backdated past the TTL must already
    /// be gone by the time the first engine's emitter observes the ledger.
    #[test]
    fn the_idle_tick_runs_before_any_engine_observes_the_ledger() {
        let v = json!({});
        let mut ledger = GuideLedger::anonymous(Some(std::time::Duration::from_secs(60)));
        ledger.insert("stale".to_string());
        ledger.backdate_for_test("stale", chrono::Duration::seconds(120));
        let engines = [decl(
            "reporter",
            Corpus::CompiledGuides,
            reports_ledger_state,
        )];
        let out = run_post_in(&engines, &ctx(&v), &mut ledger);
        assert_eq!(texts(&out), vec!["REARMED".to_string()]);
    }
}
