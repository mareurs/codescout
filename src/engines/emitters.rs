//! The post-phase body of each registered engine — copied out of the
//! inlined logic that still lives, unchanged, in `Tool::call_content`
//! (`src/tools/core/types.rs`).
//!
//! **Not live in production yet.** `call_content` runs its own copy of this
//! logic directly; nothing on any production path calls these functions —
//! only `ENGINES`' `emit_post` pointers (reachable via `run_post_in`, which
//! itself has no live caller outside `cfg(test)` today) and this file's own
//! tests do. The two copies **must stay byte-identical** until
//! `docs/superpowers/plans/2026-09-02-layer-2a-3-wiring-and-one-budget.md`
//! (Plan 3) deletes `call_content`'s inlined branch and calls `run_post` in
//! its place. Until then, a fix applied to one side and not the other is a
//! silent behavior fork nothing here can catch, because these functions are
//! not on the path production actually runs.
//!
//! Each function answers one question — *"does my trigger fire on this call,
//! and if so what do I ship?"* — and answers nothing about ordering. Ordering
//! and corpus exclusivity belong to [`super::coordinator`].

use super::coordinator::{Emission, Emitted, PostCtx};
use crate::tools::guide_emit::{guide_block, guide_blocks_for, GuideDeliveryShape};
use crate::tools::guide_ledger::GuideLedger;
use rmcp::model::Content;

/// Engine `session-opener`. Fires whenever the bootstrap topic specifically is
/// absent from the ledger — not merely whenever the ledger is empty, which
/// diverges the moment `GuideLedger::re_arm` removes just that topic.
pub(crate) fn emit_session_opener(_ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    let topic = crate::prompts::SESSION_OPENING_GUIDE;
    if ledger.contains(topic) {
        return Emitted::Declined;
    }
    match guide_block(topic) {
        Some(block) => {
            // The key is burned only once the block actually builds: an
            // unregistered or empty topic must not consume the slot on silence.
            ledger.insert(topic.to_string());
            Emitted::Claimed(Emission {
                hint: Some((topic.to_string(), GuideDeliveryShape::Whole)),
                blocks: vec![block],
            })
        }
        // CLAIMED, not Declined. The pre-refactor `if/else` returned
        // `(None, Vec::new())` here rather than falling through to
        // guide-sections, and that fail-safe survives the move.
        None => Emitted::Claimed(Emission::default()),
    }
}

/// Engine `guide-sections`. Two ways to name a topic: the tool's own
/// `relevant_guide_topic` reads the RESULT and goes first because it encodes
/// what the call TOUCHED; `topic_declaring` asks which section was written for
/// this call's shape and is a fallthrough. Letting declarations win outright
/// would starve `tracker-conventions` as thoroughly as the old order starved
/// the sections — the same defect with the sign flipped.
pub(crate) fn emit_guide_sections(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    let Some(content_topic) = ctx.content_topic else {
        return Emitted::Declined;
    };
    let mut candidates: Vec<&str> = vec![content_topic];
    if let Some(t) =
        crate::prompts::guide_index::GUIDE_INDEX.topic_declaring(ctx.selector, ctx.value)
    {
        if t != content_topic {
            candidates.push(t);
        }
    }
    for topic in candidates {
        // `progressive-disclosure` is the one topic conditional on the
        // response actually overflowing — either the default path buffered a
        // large JSON, or the tool pre-buffered and returned an `output_id`.
        let should = match topic {
            "progressive-disclosure" => ctx.overflowing,
            _ => true,
        };
        if !should {
            continue;
        }
        let (blocks, shape) = guide_blocks_for(topic, ctx.selector, ctx.value, ledger);
        // `shape.is_none()` iff `blocks.is_empty()` — every return path in
        // `guide_blocks_for` keeps the two in lockstep.
        if let Some(shape) = shape {
            return Emitted::Claimed(Emission {
                hint: Some((topic.to_string(), shape)),
                blocks,
            });
        }
    }
    Emitted::Claimed(Emission::default())
}

/// Engine `operator-rules`. Append-only: it produces no hint, because the
/// primary block's single `_guide_hint` field belongs to the guide corpus.
///
/// A tool that opted out of `selector_key` declines rather than matching
/// everything. That is deliberate — a wildcard here would deliver every
/// triggered rule on every call from every un-opted-in tool.
pub(crate) fn emit_operator_rules(ctx: &PostCtx<'_>, ledger: &mut GuideLedger) -> Emitted {
    if ctx.selector.is_none() {
        return Emitted::Declined;
    }
    let mut blocks = Vec::new();
    for r in crate::operator_rules::route::route(ctx.selector, ctx.value) {
        let key = crate::operator_rules::route::ledger_key(&r.id);
        // `contains` then `insert` rather than `insert`'s return value: a
        // repeat must not refresh the stamp `expire_idle`'s TTL reads, nor pay
        // `persist()`'s unconditional disk write, for a call delivering nothing.
        if ledger.contains(&key) {
            continue;
        }
        ledger.insert(key);
        blocks.push(Content::text(format!(
            "<!-- operator-rule {} — delivered once this session for this call \
             shape; see docs/trackers/operator-rules.md -->\n{}",
            r.id, r.imperative
        )));
    }
    Emitted::Claimed(Emission { hint: None, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx<'a>(value: &'a serde_json::Value, sel: Option<&'a str>) -> PostCtx<'a> {
        PostCtx {
            selector: sel,
            value,
            content_topic: None,
            overflowing: false,
        }
    }

    #[test]
    fn the_opener_declines_once_its_topic_is_stamped() {
        let mut ledger = GuideLedger::default();
        let v = json!({});
        let topic = crate::prompts::SESSION_OPENING_GUIDE;

        let Emitted::Claimed(first) = emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger)
        else {
            panic!("the opener must claim on its first call");
        };
        let (hint_topic, shape) = first.hint.expect("the opener must produce a hint");
        assert_eq!(
            hint_topic, topic,
            "the hint must name the opener's own bare topic, not a section key"
        );
        assert!(
            matches!(shape, GuideDeliveryShape::Whole),
            "the opener always delivers Whole, never a section shape"
        );
        assert_eq!(first.blocks.len(), 1, "got {} block(s)", first.blocks.len());
        let got_text = first.blocks[0]
            .as_text()
            .map(|t| t.text.clone())
            .expect("the opener's block must be text content");
        let want_text = guide_block(topic)
            .and_then(|b| b.as_text().map(|t| t.text.clone()))
            .expect("the opener's own topic must be registered and text-shaped");
        assert_eq!(
            got_text, want_text,
            "the opener's block must be exactly guide_block(topic)'s bytes"
        );

        assert!(matches!(
            emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger),
            Emitted::Declined
        ));
    }

    /// Not a restatement of the opener test: this asserts the ledger key is
    /// the BARE topic name. Keying it finer would desync the trigger from what
    /// `GuideLedger::re_arm` re-arms, which no delivery assertion can see.
    #[test]
    fn the_opener_stamps_the_bare_topic_name() {
        let mut ledger = GuideLedger::default();
        let v = json!({});
        let _ = emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger);
        assert!(ledger.contains(crate::prompts::SESSION_OPENING_GUIDE));
    }

    #[test]
    fn guide_sections_declines_when_the_tool_names_no_topic() {
        let v = json!({});
        assert!(matches!(
            emit_guide_sections(&ctx(&v, Some("t.a")), &mut GuideLedger::default()),
            Emitted::Declined
        ));
    }

    /// `progressive-disclosure` is gated on overflow. With `overflowing:
    /// false` the candidate is skipped, so the emitter claims and ships
    /// nothing — the topic must NOT arrive on a small response.
    #[test]
    fn progressive_disclosure_is_withheld_when_nothing_overflowed() {
        let v = json!({});
        let mut c = ctx(&v, Some("t.a"));
        c.content_topic = Some("progressive-disclosure");
        let Emitted::Claimed(e) = emit_guide_sections(&c, &mut GuideLedger::default()) else {
            panic!("a named content topic must claim");
        };
        assert!(e.is_empty(), "got {} block(s)", e.blocks.len());
    }

    #[test]
    fn progressive_disclosure_ships_when_the_response_overflowed() {
        let v = json!({});
        let mut c = ctx(&v, Some("t.a"));
        c.content_topic = Some("progressive-disclosure");
        c.overflowing = true;
        let Emitted::Claimed(e) = emit_guide_sections(&c, &mut GuideLedger::default()) else {
            panic!("a named content topic must claim");
        };
        assert!(!e.is_empty(), "the overflow path must deliver the topic");
        let (hint_topic, shape) = e.hint.expect("the overflow path must hint its own topic");
        assert_eq!(
            hint_topic, "progressive-disclosure",
            "the hint must name the topic actually delivered, not the tool's content topic"
        );
        assert!(
            matches!(shape, GuideDeliveryShape::Whole),
            "progressive-disclosure is a non-declaring topic, so its shape is always Whole"
        );
    }

    #[test]
    fn operator_rules_declines_for_a_tool_that_opted_out_of_selector_key() {
        let v = json!({"status": "ok"});
        assert!(matches!(
            emit_operator_rules(&ctx(&v, None), &mut GuideLedger::default()),
            Emitted::Declined
        ));
    }

    /// OP-3 declares `**Serves:** memory.write` in the shipped ledger.
    #[test]
    fn operator_rules_delivers_a_matching_rule_once_then_dedups() {
        let mut ledger = GuideLedger::default();
        let v = json!({"status": "ok"});
        let Emitted::Claimed(first) =
            emit_operator_rules(&ctx(&v, Some("memory.write")), &mut ledger)
        else {
            panic!("a selector-bearing call must claim")
        };
        assert_eq!(first.blocks.len(), 1, "OP-3 must route on memory.write");
        let text = first.blocks[0]
            .as_text()
            .map(|t| t.text.clone())
            .expect("the rule block must be text content");
        assert!(
            text.starts_with("<!-- operator-rule OP-3"),
            "the wrapper comment must survive byte-for-byte for Plan 3; got {text:?}"
        );
        assert!(first.hint.is_none(), "the rule corpus owns no _guide_hint");

        let Emitted::Claimed(second) =
            emit_operator_rules(&ctx(&v, Some("memory.write")), &mut ledger)
        else {
            panic!("a selector-bearing call must claim")
        };
        assert!(
            second.is_empty(),
            "a delivered rule must not fire twice in a session"
        );
    }
}
