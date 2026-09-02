//! The post-phase body of each registered engine, moved out of
//! `Tool::call_content` unchanged.
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
        assert!(matches!(
            emit_session_opener(&ctx(&v, Some("t.a")), &mut ledger),
            Emitted::Claimed(_)
        ));
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
