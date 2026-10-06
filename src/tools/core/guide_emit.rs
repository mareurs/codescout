//! Guide-block emission — what the guide engine ships on one call, and the
//! ledger bookkeeping for it.
//!
//! Extracted from `Tool::call_content` on 2026-09-02 (`GG-3`). Every item here
//! was already a free function nested inside that method: none touches `self`
//! or `ToolContext`, so the move is a pure relocation with visibility widened
//! to `pub(crate)`. Behaviour is unchanged and the doc comments are verbatim.
//!
//! Why it is its own module rather than more lines in `types.rs`: this is one
//! **retrieval engine's** emission path, and `operator_rules::route` is
//! another. Both are driven by the same `Tool::selector_key` and stamp the same
//! [`GuideLedger`](crate::tools::guide_ledger::GuideLedger) under disjoint key
//! namespaces (`<topic>#<heading>` here, `op:OP-N` there). Naming this one
//! makes the pair addressable — see
//! `docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`.

use rmcp::model::Content;
use serde_json::Value;

/// Which shape of guide content actually shipped in the second (or
/// later) content block, so the legacy `_guide_hint` JSON field can
/// describe THAT, not a fixed sentence written for the whole-topic
/// world. Before this existed, `inject_hint` always emitted "Full
/// guide auto-injected ... do not re-call get_guide" regardless of
/// what shipped — true for `Whole`, but false for `Section` (a
/// slice, not "the full guide") and actively backwards for
/// `Preamble` (the whole point of the preamble fallback is that the
/// model SHOULD re-call `get_guide(topic)`; telling it not to
/// neutered the fallback into a ~700-byte no-op).
#[derive(Clone, Copy)]
pub(crate) enum GuideDeliveryShape {
    /// Non-declaring topic: the entire topic body shipped, once.
    Whole,
    /// Declaring topic, at least one section matched: only the
    /// matched slice(s) shipped — other sections may still arrive
    /// on later, differently-shaped calls this session.
    Section,
    /// Declaring topic, no section matched this call's shape: the
    /// topic's preamble shipped, with an explicit pointer to call
    /// `get_guide(topic)` for the full body.
    Preamble,
    /// Non-declaring topic whose body is over
    /// [`MAX_AUTO_INJECT_GUIDE_BYTES`]: NO body shipped, only a one-line
    /// pointer to `get_guide(topic)`. The topic is not stamped delivered.
    Pointer,
}

/// Largest guide block (wrapper included) the server will auto-inject whole.
///
/// A guide is a later content block, and the harness may save an oversized
/// tool result to disk and show the model only a ~2 KB preview of block 0 — so
/// anything auto-injected into such a result is stamped delivered and never
/// read (`docs/issues/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md`).
/// The cut-off is not known: the smallest harness-saved result measured was
/// 29.4 KB (64 saved results across one profile's transcripts, 2026-09-24).
/// The bound is chosen so that guide + a primary block at its own inline cap
/// (`TOOL_OUTPUT_BUFFER_THRESHOLD`, ~10 KB) still sits below that floor:
/// 16 KiB + 10 KB = ~26 KB < 29.4 KB. A guide over this is not delivered; a
/// pointer to `get_guide(topic)` ships instead and the topic stays unstamped.
/// Of the corpus today only `tracker-conventions` (~59 KB) exceeds it; the next
/// largest non-declaring guide (`iron-laws-detail`) is ~14.7 KB.
// cap-class: RESULT_CAP guide_emit.auto_inject_bound — probed
pub(crate) const MAX_AUTO_INJECT_GUIDE_BYTES: usize = 16 * 1024;

pub(crate) fn inject_hint(val: &mut Value, topic: &str, shape: GuideDeliveryShape) {
    let text = match shape {
        GuideDeliveryShape::Whole => format!(
            "First call this session for topic '{topic}'. \
             Full guide auto-injected as a separate content \
             block below; do not re-call get_guide(\"{topic}\")."
        ),
        GuideDeliveryShape::Section => format!(
            "Section(s) of '{topic}' auto-injected below, selected for \
             this call. Other sections may arrive on later calls; \
             `get_guide(\"{topic}\")` returns the full topic."
        ),
        GuideDeliveryShape::Preamble => format!(
            "No section of '{topic}' declares this call shape; its \
             preamble is below. Call `get_guide(\"{topic}\")` for the \
             full topic."
        ),
        GuideDeliveryShape::Pointer => format!(
            "Guide '{topic}' is too large to auto-inject and was NOT \
             delivered. Call `get_guide(\"{topic}\")` to read it."
        ),
    };
    if let Some(obj) = val.as_object_mut() {
        obj.insert("_guide_hint".to_string(), Value::String(text));
    }
}

/// Build the second-block content for V2 hard-injection of a
/// non-declaring (whole-topic) guide. Returns `None` when the
/// topic's body is not registered. The caller (`guide_blocks_for`)
/// treats `None` as "nothing shipped": it does NOT burn the topic's
/// ledger key and does NOT set the `_guide_hint` field for it, so a
/// misregistered/renamed topic degrades to silence rather than a
/// false promise of content that never arrives, and a later call
/// against the same topic still gets a chance to succeed once the
/// registration is fixed. This should not happen for any topic
/// actually reachable via `relevant_guide_topic` in production; it
/// exists as a defensive guard against drift between that side and
/// the registered-topics table, not as a supported delivery path.
///
/// `pub(crate)` because it has **two** callers, not one: `guide_blocks_for`'s
/// non-declaring branch below, and `call_content`'s session-opener path, which
/// delivers `SESSION_OPENING_GUIDE` whole and deliberately bypasses
/// `guide_blocks_for` (that topic never declares sections in Phase 1, asserted
/// by `session_opening_guide_never_declares_sections`).
pub(crate) fn guide_block(topic: &str) -> Option<Content> {
    let body = crate::prompts::topic_body(topic)?;
    let wrapped = format!(
        "<!-- auto-injected get_guide('{topic}') — first call this session \
         that triggers the topic. Do NOT re-call get_guide for this topic. -->\n\
         \n\
         {body}\n\
         \n\
         <!-- end auto-injected get_guide('{topic}') -->"
    );
    Some(Content::text(wrapped))
}

/// The non-declaring (whole-topic) branch of [`guide_blocks_for`], with the
/// size bound passed in so a test can drive the oversize path without a guide
/// that happens to be large.
///
/// A body within `max_bytes` ships whole and stamps the bare topic. A body over
/// it ships a one-line pointer instead and does NOT stamp the bare topic: the
/// stamp means "the model has this", and an oversized block is the one most
/// likely to be saved to disk by the harness and never read. An explicit
/// `get_guide(topic)` always returns the full body and stamps the bare topic
/// itself, so the pointer leads somewhere and a deliberate read retires it.
///
/// The pointer is sent once per session, under its own `<topic>#<pointer>` key
/// (swept by `GuideLedger::re_arm` with the topic, owned by `owns_guide_key`).
/// Unkeyed it would claim every call that names the topic, and
/// `emit_guide_sections` stops at the first candidate that ships — a declared
/// section for the same call would then never be reached. A spent pointer
/// returns empty, the same as a spent whole guide, so that fallthrough survives.
fn whole_topic_blocks(
    topic: &str,
    emitted: &mut crate::tools::guide_ledger::GuideLedger,
    max_bytes: usize,
) -> (Vec<Content>, Option<GuideDeliveryShape>) {
    if emitted.contains(topic) {
        return (Vec::new(), None);
    }
    // Only burn a ledger key once the block actually builds —
    // `guide_block` returning `None` (topic not registered) must
    // not consume the slot on silence (fix for the fail-safe
    // inversion flagged in Task 8 review: an unregistered topic
    // used to burn its key with nothing shipped at all, which is
    // ambiguity resolving toward suppression).
    let Some(block) = guide_block(topic) else {
        return (Vec::new(), None);
    };
    let size = block.as_text().map_or(0, |t| t.text.len());
    if size <= max_bytes {
        emitted.insert(topic.to_string());
        return (vec![block], Some(GuideDeliveryShape::Whole));
    }
    let pointer_key = format!("{topic}#<pointer>");
    if emitted.contains(&pointer_key) {
        return (Vec::new(), None);
    }
    emitted.insert(pointer_key);
    (
        vec![Content::text(format!(
            "<!-- get_guide('{topic}') NOT auto-injected: {size} bytes is over the \
             {max_bytes}-byte auto-inject bound. -->\n\
             Call `get_guide(\"{topic}\")` to read it."
        ))],
        Some(GuideDeliveryShape::Pointer),
    )
}

/// Blocks to emit for a resolved topic, and the ledger bookkeeping for
/// them. A topic that does not declare any `serves:` sections
/// (`GUIDE_INDEX.declares`) keeps the exact pre-Task-8 whole-topic
/// behaviour: one block, bare-topic ledger key, byte-identical output
/// — this is the Phase 1 containment property, and every topic but
/// `librarian` takes this branch today. Except that a body over
/// [`MAX_AUTO_INJECT_GUIDE_BYTES`] ships a pointer, not the body, and does not
/// stamp the bare topic — see `whole_topic_blocks`.
///
/// A declaring topic instead resolves the call's shape
/// (`selector` + the typed result) against the topic's declared
/// sections. Sections already delivered this session (per
/// `GuideSection::ledger_key`, `"{topic}#{heading}"`) are skipped —
/// silently, not as a fallback trigger. Only when NO section
/// declares this shape at all does the preamble fallback fire
/// (`"{topic}#<preamble>"` key): a small preamble plus a
/// `get_guide(topic)` pointer, never the whole topic, never silence.
/// Falling back to the preamble is safe because a fixed shape
/// census bounds the number of distinct call shapes, and starvation
/// degrades to "delivered late" (a later matching call still fires
/// the section), never "never delivered".
///
/// A ledger key is inserted ONLY when its block is actually pushed —
/// never merely computed. Marking a key emitted before knowing
/// whether anything would be sent is the bug this replaces: a call
/// whose shape matched nothing used to burn the whole topic on
/// silence, and a declaring topic's bare name is never used as a key
/// at all (only `topic#heading` / `topic#<preamble>` are), so the
/// old "already emitted the bare topic" shortcut cannot be reused
/// here — each branch below does its own `emitted.insert` check at
/// the granularity it actually delivers.
pub(crate) fn guide_blocks_for(
    topic: &str,
    selector: Option<&str>,
    result: &Value,
    emitted: &mut crate::tools::guide_ledger::GuideLedger,
) -> (Vec<Content>, Option<GuideDeliveryShape>) {
    use crate::prompts::guide_index::GUIDE_INDEX;

    if !GUIDE_INDEX.declares(topic) {
        return whole_topic_blocks(topic, emitted, MAX_AUTO_INJECT_GUIDE_BYTES);
    }

    let matched = GUIDE_INDEX.match_sections(topic, selector, result);

    // No section declares this call's shape at all: preamble + a
    // pointer to the full topic, once per session.
    if matched.is_empty() {
        let key = format!("{topic}#<preamble>");
        // `contains` before `insert`, never `insert`'s return value as
        // the already-sent test. `GuideLedger::insert` refreshes the
        // stamp and persists on a repeat, so using it here bills a
        // call that delivers NOTHING for a staged write + rename
        // (identified tier, `path: Some`) or a deferred re-arm
        // (anonymous tier, whose `idle_ttl` is the only thing standing
        // between a second conversation and permanent starvation).
        // `op_content` below spells it this way for exactly this reason.
        if emitted.contains(&key) {
            return (Vec::new(), None);
        }
        return match GUIDE_INDEX.topic(topic) {
            Some(entry) => {
                // Stamp only once the block is built. Moving the insert
                // here also closes a latent burn-on-silence: an
                // unregistered topic used to consume the preamble slot
                // and ship nothing. Unreachable today — `declares(topic)`
                // already implies `topic(topic).is_some()` — but that is
                // an argument, and the fail-safe direction is free.
                emitted.insert(key);
                (
                    vec![Content::text(format!(
                        "<!-- auto-injected get_guide('{topic}') preamble — no section \
                     declares this call's shape. -->\n\
                     \n\
                     {}\n\
                     \n\
                     Call `get_guide(\"{topic}\")` for the full topic.\n\
                     \n\
                     <!-- end auto-injected get_guide('{topic}') preamble -->",
                        entry.preamble.trim()
                    ))],
                    Some(GuideDeliveryShape::Preamble),
                )
            }
            None => (Vec::new(), None),
        };
    }

    // At least one section declares this shape: deliver whichever of
    // those (plus their `requires:` closure, from `match_sections`)
    // have not already been sent this session. All-already-sent is
    // NOT "nothing declared" — it must return empty, never fall back
    // to the preamble (that would re-litigate an already-satisfied
    // shape as if it were unmatched).
    let mut out = Vec::new();
    for sec in matched {
        let key = sec.ledger_key();
        // `contains` then `insert`, per the preamble branch above: the
        // `if` here gated only the `push`, so an all-already-sent call
        // still refreshed and persisted one stamp per matched section
        // while returning empty.
        if emitted.contains(&key) {
            continue;
        }
        emitted.insert(key);
        out.push(Content::text(format!(
            "<!-- auto-injected get_guide('{topic}') § {} — first call this \
                 session that serves this section. Do NOT re-call get_guide for \
                 it. -->\n\
                 \n\
                 {}\n\
                 \n\
                 <!-- end auto-injected get_guide('{topic}') § {} -->",
            sec.heading, sec.body, sec.heading
        )));
    }
    let shape = if out.is_empty() {
        None
    } else {
        Some(GuideDeliveryShape::Section)
    };
    (out, shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::guide_index::GUIDE_INDEX;
    use crate::tools::guide_ledger::GuideLedger;
    use serde_json::json;

    fn ledger() -> GuideLedger {
        GuideLedger::anonymous(None)
    }

    fn text(c: &Content) -> String {
        c.as_text().expect("guide blocks are text").text.clone()
    }

    /// A topic that declares nothing, resolved at run time rather than named.
    ///
    /// **Load-bearing:** hard-coding one (`error-handling`, say) would break the
    /// moment `GG-2` declares it — reading as a regression in Phase 3's corpus
    /// edit rather than as this test outliving its fixture. The `expect`
    /// message is the handover note for whoever finishes `GG-2`.
    fn a_non_declaring_topic() -> &'static str {
        crate::prompts::GUIDE_TOPICS
            .iter()
            .copied()
            .find(|t| !GUIDE_INDEX.declares(t))
            .expect(
                "every guide topic now declares sections — GG-2/Phase 3 is complete. \
                 Rewrite the non-declaring branch's tests against a synthetic corpus \
                 (GuideIndex::from_str_for_test) instead of the live one.",
            )
    }

    /// A topic that declares, plus a selector at least one of its sections
    /// serves. `librarian` is the only declaring topic in Phase 1.
    const DECLARING: &str = "librarian";
    const SERVED_SELECTOR: &str = "doc.find";

    #[test]
    fn a_non_declaring_topic_ships_whole_and_stamps_the_bare_topic() {
        let topic = a_non_declaring_topic();
        let mut led = ledger();
        let (blocks, shape) = guide_blocks_for(topic, None, &json!({}), &mut led);

        assert_eq!(blocks.len(), 1, "whole-topic delivery is exactly one block");
        assert!(matches!(shape, Some(GuideDeliveryShape::Whole)));
        assert!(
            led.contains(topic),
            "the bare topic name is the ledger key for a non-declaring topic"
        );
    }

    #[test]
    fn a_repeat_of_a_non_declaring_topic_ships_nothing() {
        let topic = a_non_declaring_topic();
        let mut led = ledger();
        guide_blocks_for(topic, None, &json!({}), &mut led);
        let (blocks, shape) = guide_blocks_for(topic, None, &json!({}), &mut led);

        assert!(blocks.is_empty());
        assert!(shape.is_none());
    }
    /// Byte length of a topic's whole-topic block — the quantity the bound reads.
    fn whole_block_len(topic: &str) -> usize {
        text(&guide_block(topic).expect("registered topic")).len()
    }

    #[test]
    fn an_oversize_whole_topic_guide_ships_a_pointer_and_does_not_stamp_the_topic() {
        let topic = a_non_declaring_topic();
        let mut led = ledger();
        // A bound of 100 B makes any real guide "oversize" without depending on
        // which topic is large today.
        let (blocks, shape) = whole_topic_blocks(topic, &mut led, 100);

        assert_eq!(blocks.len(), 1);
        assert!(matches!(shape, Some(GuideDeliveryShape::Pointer)));
        let pointer = text(&blocks[0]);
        assert!(pointer.contains(&format!("get_guide(\"{topic}\")")));
        assert!(
            pointer.len() < 400 && pointer.len() < whole_block_len(topic),
            "a pointer is a line, not the body — got {} B",
            pointer.len()
        );
        assert!(
            !pointer.contains("<!-- end auto-injected"),
            "the body's wrapper must not ship"
        );
        // The load-bearing half: the stamp is what suppresses the guide for the
        // rest of the session, so an undelivered body must not leave one.
        assert!(
            !led.contains(topic),
            "an oversize guide must not be marked delivered"
        );
    }

    /// Positive twin of the oversize test: the same function, a bound the guide
    /// fits, and the opposite outcome — so a `max_bytes` that never compares, or
    /// a pointer that always ships, reds one of the two.
    #[test]
    fn a_whole_topic_guide_within_the_bound_ships_whole_and_stamps() {
        let topic = a_non_declaring_topic();
        let size = whole_block_len(topic);

        // Exactly at the bound is within it; one byte under is not.
        let mut at = ledger();
        let (blocks, shape) = whole_topic_blocks(topic, &mut at, size);
        assert!(matches!(shape, Some(GuideDeliveryShape::Whole)));
        assert_eq!(text(&blocks[0]).len(), size);
        assert!(at.contains(topic));
        assert!(!at.contains(&format!("{topic}#<pointer>")));

        let mut under = ledger();
        let (_, shape) = whole_topic_blocks(topic, &mut under, size - 1);
        assert!(matches!(shape, Some(GuideDeliveryShape::Pointer)));
        assert!(!under.contains(topic));
    }

    /// The pointer is once per session, and a spent one ships NOTHING — not
    /// another pointer. `emit_guide_sections` stops at the first candidate that
    /// ships, so a pointer on every call would starve any declared section for
    /// the same call (`a_declared_section_still_arrives_once_the_content_topic_is_spent`).
    #[test]
    fn a_spent_pointer_ships_nothing_and_still_leaves_the_topic_unstamped() {
        let topic = a_non_declaring_topic();
        let mut led = ledger();
        whole_topic_blocks(topic, &mut led, 100);
        let (blocks, shape) = whole_topic_blocks(topic, &mut led, 100);

        assert!(blocks.is_empty());
        assert!(shape.is_none());
        assert!(
            !led.contains(topic),
            "repeating the pointer must not stamp the topic either"
        );
    }

    /// The pointer cannot loop: once the model follows it, `get_guide` stamps the
    /// bare topic (`tools/guide.rs`), and an oversize guide is then silent rather
    /// than pointing again at something already read.
    #[test]
    fn an_explicitly_fetched_oversize_guide_is_not_pointed_at_again() {
        let topic = a_non_declaring_topic();
        let mut led = ledger();
        led.insert(topic.to_string()); // what an explicit get_guide does
        let (blocks, shape) = whole_topic_blocks(topic, &mut led, 100);

        assert!(blocks.is_empty());
        assert!(shape.is_none());
        assert!(!led.contains(&format!("{topic}#<pointer>")));
    }

    /// Pins the bound to the measurement it was derived from, so raising it is a
    /// decision rather than a drift: guide + a primary block at its own inline
    /// cap must stay under the smallest harness-saved result observed (29.4 KB).
    /// A `const` assertion, not a `#[test]`: both sides are constants, so a bad
    /// bound is a compile error in the test build.
    const _: () = {
        const SMALLEST_SAVED_RESULT_BYTES: usize = 29_400;
        assert!(
            MAX_AUTO_INJECT_GUIDE_BYTES + crate::tools::core::types::TOOL_OUTPUT_BUFFER_THRESHOLD
                < SMALLEST_SAVED_RESULT_BYTES
        );
    };

    #[test]
    fn an_unregistered_topic_ships_nothing_and_stamps_nothing() {
        // The fail-safe direction: a topic that resolves to no body must not
        // burn its ledger slot, so a later call gets another chance once the
        // registration is fixed. Asserting the *stamp* is the point — the
        // empty block list alone is also what a correctly-silent path returns.
        let mut led = ledger();
        let (blocks, shape) = guide_blocks_for("no-such-topic-exists", None, &json!({}), &mut led);

        assert!(blocks.is_empty());
        assert!(shape.is_none());
        assert!(
            led.is_empty(),
            "silence must not consume the slot, else the topic is starved for the session"
        );
    }

    #[test]
    fn an_unmatched_shape_ships_the_preamble_under_its_own_key() {
        let mut led = ledger();
        let (blocks, shape) = guide_blocks_for(
            DECLARING,
            Some("no_such_tool.no_such_action"),
            &json!({}),
            &mut led,
        );

        assert_eq!(blocks.len(), 1);
        assert!(matches!(shape, Some(GuideDeliveryShape::Preamble)));
        assert!(led.contains(&format!("{DECLARING}#<preamble>")));
        assert!(
            !led.contains(DECLARING),
            "a declaring topic must never stamp its bare name — that key is what \
             GuideLedger::re_arm and the whole-topic path use"
        );
        assert!(
            text(&blocks[0]).contains(&format!("get_guide(\"{DECLARING}\")")),
            "the preamble's whole job is to point at the full topic"
        );
    }

    #[test]
    fn a_matching_shape_ships_sections_each_under_its_own_key() {
        let mut led = ledger();
        let (blocks, shape) =
            guide_blocks_for(DECLARING, Some(SERVED_SELECTOR), &json!({}), &mut led);

        assert!(
            !blocks.is_empty(),
            "`{SERVED_SELECTOR}` is declared by librarian.md; if this reds, the \
             declaration was removed, not the matcher broken"
        );
        assert!(matches!(shape, Some(GuideDeliveryShape::Section)));
        let stamps: Vec<String> = led.stamps_for_test().into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            stamps.len(),
            blocks.len(),
            "one ledger key per block actually pushed, never per section considered"
        );
        for k in &stamps {
            assert!(
                k.starts_with(&format!("{DECLARING}#")),
                "section keys are `topic#heading`, got {k}"
            );
        }
    }

    #[test]
    fn all_sections_already_sent_returns_empty_and_does_not_fall_back_to_the_preamble() {
        // The invariant `guide_blocks_for`'s doc comment states and nothing
        // else asserts: "all-already-sent is NOT 'nothing declared'".
        //
        // Note which direction this discriminates. `blocks.is_empty()` alone is
        // monotone under removal — a `guide_blocks_for` gutted to `return
        // (vec![], None)` passes it. The assertion that carries the weight is
        // the preamble key's *absence* paired with the first call having
        // produced blocks: together they say the second call took the
        // all-sent path rather than the unmatched path, which are the two
        // ways to arrive at an empty result.
        let mut led = ledger();
        let (first, _) = guide_blocks_for(DECLARING, Some(SERVED_SELECTOR), &json!({}), &mut led);
        assert!(!first.is_empty(), "precondition: the first call delivers");

        let (second, shape) =
            guide_blocks_for(DECLARING, Some(SERVED_SELECTOR), &json!({}), &mut led);

        assert!(
            second.is_empty(),
            "a satisfied shape delivers nothing twice"
        );
        assert!(shape.is_none());
        assert!(
            !led.contains(&format!("{DECLARING}#<preamble>")),
            "re-litigating a satisfied shape as unmatched would ship the preamble — \
             the fallback exists for shapes no section declares, not for shapes \
             already served"
        );
    }

    #[test]
    fn inject_hint_tells_the_truth_about_what_shipped() {
        // Each shape's sentence must differ in the direction that matters:
        // `Whole` says do NOT re-call; `Preamble` says DO. A single shared
        // sentence is the bug this enum was introduced to fix, and it is not
        // caught by asserting any one shape in isolation.
        let mut v = json!({});
        inject_hint(&mut v, "t", GuideDeliveryShape::Whole);
        let whole = v["_guide_hint"].as_str().unwrap().to_string();

        let mut v = json!({});
        inject_hint(&mut v, "t", GuideDeliveryShape::Preamble);
        let preamble = v["_guide_hint"].as_str().unwrap().to_string();

        assert!(whole.contains("do not re-call"), "got: {whole}");
        assert!(
            preamble.contains("Call `get_guide(\"t\")`"),
            "the preamble fallback is a no-op if it tells the reader not to fetch: {preamble}"
        );
        assert_ne!(whole, preamble);

        // `Pointer` shipped no body: it must say so, and must not borrow the
        // `Whole` promise that the guide is already in context.
        let mut v = json!({});
        inject_hint(&mut v, "t", GuideDeliveryShape::Pointer);
        let pointer = v["_guide_hint"].as_str().unwrap().to_string();
        assert!(
            pointer.contains("Call `get_guide(\"t\")`"),
            "got: {pointer}"
        );
        assert!(pointer.contains("NOT delivered"), "got: {pointer}");
        assert!(!pointer.contains("do not re-call"), "got: {pointer}");
    }
}
