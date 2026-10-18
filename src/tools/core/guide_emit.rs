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
/// read (`docs/issues/archive/2026-09-24-a-guide-in-a-result-the-harness-saves-to-disk-is-marked-delivered-unread.md`).
/// The cut-off is not known: the smallest harness-saved result measured was
/// 29.4 KB (64 saved results across one profile's transcripts, 2026-09-24).
/// The bound is chosen so that guide + a primary block at its own inline cap
/// (`TOOL_OUTPUT_BUFFER_THRESHOLD`, ~10 KB) still sits below that floor:
/// 16 KiB + 10 KB = ~26 KB < 29.4 KB. A guide over this is not delivered; a
/// pointer to `get_guide(topic)` ships instead and the topic stays unstamped.
/// No whole-topic guide in the corpus exceeds it today: `tracker-conventions` (~60 KB)
/// is the one that did, and it now declares `serves:` sections and ships those; the
/// largest guide still shipped whole (`iron-laws-detail`) is ~14.7 KB.
// cap-class: RESULT_CAP guide_emit.auto_inject_bound — probed
pub(crate) const MAX_AUTO_INJECT_GUIDE_BYTES: usize = 16 * 1024;

/// A text block that FOLLOWS another in one reply, built so that it starts on a line of
/// its own.
///
/// The MCP layer keeps a reply's content blocks apart, but a client that shows the reply
/// as text lays them end to end (Claude Code and the `mcp` CLI both do), and the tool's
/// own block ends on its last line with no newline. An appended block that opened with
/// `<!--` then sat on that line (`      }<!-- auto-injected ...`), and a line-oriented
/// reader that cut the guide at its marker (`sed '/auto-injected/,$d'`, `grep -v`, `head`)
/// took the tool's last line with it: one body line, one overview row. That produced two
/// false bug reports on 2026-10-17. A newline at the head of the appended block removes the
/// hazard for every such reader whatever joins the blocks.
/// docs/issues/2026-10-17-injected-guide-block-is-joined-to-the-tool-text-with-no-newline.md
///
/// Every block this crate appends behind a tool's answer is built here: the guide blocks
/// (whole, pointer, preamble, section), the operator rules, and the notices. Idempotent, so
/// a block that already opens with a newline (the paths banner, the status block, the build
/// notice) is not given a second one.
pub(crate) fn trailing_block(text: impl AsRef<str>) -> Content {
    let text = text.as_ref();
    if text.starts_with('\n') {
        Content::text(text)
    } else {
        Content::text(format!("\n{text}"))
    }
}

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
    Some(trailing_block(wrapped))
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
        vec![trailing_block(format!(
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
                    vec![trailing_block(format!(
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
        out.push(section_block(topic, sec));
    }
    let shape = if out.is_empty() {
        None
    } else {
        Some(GuideDeliveryShape::Section)
    };
    (out, shape)
}

/// The pre-call gate's blocks: the `serves:` section(s) for `selector`, served
/// BEFORE a destructive call runs, or empty when there is nothing to hold it for.
///
/// Empty means "execute": no declared section serves this selector, or every one
/// of them was already delivered this session, or the ledger belongs to no
/// conversation. The last is the non-interactive opt-out — an anonymous ledger
/// (the CLI, a script, a client with no session identity) has nobody to
/// acknowledge a held call, so it keeps today's behaviour and the post-call
/// delivery. Never blocks without text to show.
///
/// Input-derivable only. `topic_declaring` and `match_sections` are given a null
/// result: a `serves:` shape of the plain `tool.action` form matches on the
/// selector alone, which is all that exists before the call runs. A shape that
/// also needs the result (`path_contains`) cannot gate, by construction.
///
/// Stamps the ledger (inside [`guide_blocks_for`]), which is the whole of the
/// no-double-serve story: the post-call emission finds the keys present and ships
/// nothing, and the caller's re-issue of the identical call finds them present
/// here and executes. State lives in the ledger, so it inherits the ledger's
/// persistence, idle TTL, per-principal scoping and fork seeding
/// (`GuideLedger::seed_inherited`) for free; there is no stash to bound.
/// docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md
pub(crate) fn first_use_guide_gate(
    selector: Option<&str>,
    digest: &str,
    emitted: &mut crate::tools::guide_ledger::GuideLedger,
) -> Vec<Content> {
    use crate::prompts::guide_index::GUIDE_INDEX;

    if emitted.key().is_none() {
        return Vec::new();
    }
    let null = Value::Null;
    let Some(topic) = GUIDE_INDEX.topic_declaring(selector, &null) else {
        return Vec::new();
    };
    // A caller who fetched the WHOLE guide (`get_guide`, which stamps the bare topic
    // name) already holds every one of its sections; so does a fork whose seed carries
    // only that key. Holding them again buys a stall and no protection.
    if emitted.contains(topic) {
        return Vec::new();
    }
    let sections = GUIDE_INDEX.match_sections(topic, selector, &null);
    let keys: Vec<String> = sections.iter().map(|s| s.ledger_key()).collect();
    if keys.is_empty() {
        return Vec::new();
    }

    // First sight of a section: serve it (the stamp is `guide_blocks_for`'s) and hold
    // THIS call.
    if keys.iter().any(|k| !emitted.contains(k)) {
        let blocks = guide_blocks_for(topic, selector, &null, emitted).0;
        emitted.hold(&keys, digest);
        return blocks;
    }
    // Served, and nobody has yet proved they read it. Keying the decision on the
    // section alone let the SECOND of two parallel first calls through: a batch is
    // composed before any response is read, so every call in it is a first call.
    // Only the identical re-issue of a call that WAS held runs, and the first such
    // re-issue acknowledges the section for everyone; any other call is held in its
    // turn, and shown the text, since it too was composed without it.
    if !emitted.has_pending_hold(&keys) {
        return Vec::new();
    }
    if emitted.was_held(&keys, digest) {
        emitted.acknowledge(&keys);
        return Vec::new();
    }
    emitted.hold(&keys, digest);
    sections.iter().map(|s| section_block(topic, s)).collect()
}

/// A digest of a call's input, to recognise its identical re-issue. Taken after
/// parameter-alias normalisation, so an alias spelling and its canonical form agree.
/// Not a security boundary: it keys in-memory state in the caller's own session.
pub(crate) fn input_digest(input: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(input).unwrap_or_default();
    hex::encode(Sha256::digest(&bytes))[..16].to_string()
}

/// One served section, wrapped. The text a caller reads, shared by the post-call
/// delivery and the pre-call gate.
pub(crate) fn section_block(
    topic: &str,
    sec: &crate::prompts::guide_index::GuideSection,
) -> Content {
    trailing_block(format!(
        "<!-- auto-injected get_guide('{topic}') § {} — first call this \
             session that serves this section. Do NOT re-call get_guide for \
             it. -->\n\
             \n\
             {}\n\
             \n\
             <!-- end auto-injected get_guide('{topic}') § {} -->",
        sec.heading, sec.body, sec.heading
    ))
}

/// Test instrument: the blocks of one reply whose text would begin MID-LINE when a client
/// lays the blocks end to end, one entry per block, naming it. Empty means every block
/// after the first starts a line of its own: it begins with a newline, or the block before
/// it ends with one. A non-text block is a break by itself; an empty text block is skipped.
///
/// `pub(crate)` because the seam is crossed from several modules (the engines, the notice
/// carriers, the server's appended blocks) and one definition of "clean" is the point.
#[cfg(test)]
pub(crate) fn seam_defects(blocks: &[Content]) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev: Option<&str> = None;
    for (i, b) in blocks.iter().enumerate() {
        let Some(t) = b.as_text() else {
            prev = None;
            continue;
        };
        let text = t.text.as_str();
        // An empty block shows nothing: it neither has a seam of its own nor interrupts the
        // one between the text before it and the text after it.
        if text.is_empty() {
            continue;
        }
        if let Some(p) = prev {
            if !text.starts_with('\n') && !p.ends_with('\n') {
                let head: String = text.chars().take(48).collect();
                let tail: String = {
                    let n = p.chars().count();
                    p.chars().skip(n.saturating_sub(12)).collect()
                };
                out.push(format!(
                    "block {i} starts mid-line: {head:?} follows {tail:?}"
                ));
            }
        }
        prev = Some(text);
    }
    out
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

    /// The reply a caller reads is these blocks laid end to end, and the tool's own block
    /// ends on its last line with no newline. Putting one in front of the block under test
    /// is the whole fixture: whether the seam survives is decided by the block alone.
    fn behind_a_primary(blocks: Vec<Content>) -> Vec<Content> {
        let mut out = vec![Content::text("{\n  \"result\": 1\n}")];
        out.extend(blocks);
        out
    }

    /// The seam check is the instrument every test below reads through, so it is tested:
    /// a glued block must be named, a separated one passed. Without this, a check that
    /// returned nothing for every input would turn every test below into a pass.
    #[test]
    fn the_seam_check_names_a_glued_block_and_passes_a_separated_one() {
        let glued = vec![Content::text("}"), Content::text("<!-- auto-injected")];
        assert_eq!(seam_defects(&glued).len(), 1, "{:?}", seam_defects(&glued));
        let led = vec![Content::text("}"), Content::text("\n<!-- auto-injected")];
        assert!(seam_defects(&led).is_empty());
        let trailing = vec![Content::text("}\n"), Content::text("<!-- auto-injected")];
        assert!(seam_defects(&trailing).is_empty());
        // The first block has nothing in front of it.
        assert!(seam_defects(&[Content::text("<!-- first")]).is_empty());
        // An empty block shows nothing, so the block after it still follows the last text
        // on the line: `}` + `` + `<!-- auto-injected` reads `}<!-- auto-injected`.
        let across_empty = vec![
            Content::text("}"),
            Content::text(""),
            Content::text("<!-- auto-injected"),
        ];
        assert_eq!(
            seam_defects(&across_empty).len(),
            1,
            "{:?}",
            seam_defects(&across_empty)
        );
        let across_empty_led = vec![
            Content::text("}"),
            Content::text(""),
            Content::text("\n<!-- auto-injected"),
        ];
        assert!(seam_defects(&across_empty_led).is_empty());
    }

    /// The constructor itself: one newline added, never two, and the text otherwise
    /// untouched. The idempotence is what lets a block that already opens with a newline
    /// (the paths banner, the status block, the build notice) pass through unchanged.
    #[test]
    fn trailing_block_adds_exactly_one_leading_newline() {
        assert_eq!(text(&trailing_block("<!-- x -->")), "\n<!-- x -->");
        assert_eq!(text(&trailing_block("\n<!-- x -->")), "\n<!-- x -->");
        assert_eq!(text(&trailing_block("\n\n<!-- x -->")), "\n\n<!-- x -->");
        assert_eq!(text(&trailing_block("")), "\n");
    }

    /// docs/issues/2026-10-17-injected-guide-block-is-joined-to-the-tool-text-with-no-newline.md
    ///
    /// A client that lays the blocks end to end (Claude Code and the `mcp` CLI both do)
    /// puts the marker on the tool's last line, and a line-oriented reader that cuts the
    /// guide at the marker then loses that line with it. Whole-topic shape.
    #[test]
    fn a_whole_topic_guide_starts_on_its_own_line() {
        let topic = a_non_declaring_topic();
        let (blocks, shape) = guide_blocks_for(topic, None, &json!({}), &mut ledger());
        assert!(matches!(shape, Some(GuideDeliveryShape::Whole)));
        let reply = behind_a_primary(blocks);
        assert!(
            seam_defects(&reply).is_empty(),
            "{:?}",
            seam_defects(&reply)
        );
    }

    /// The same seam, oversize-pointer shape (`whole_topic_blocks` builds it separately).
    #[test]
    fn an_oversize_pointer_starts_on_its_own_line() {
        let (blocks, shape) = whole_topic_blocks(a_non_declaring_topic(), &mut ledger(), 100);
        assert!(matches!(shape, Some(GuideDeliveryShape::Pointer)));
        let reply = behind_a_primary(blocks);
        assert!(
            seam_defects(&reply).is_empty(),
            "{:?}",
            seam_defects(&reply)
        );
    }

    /// The same seam, preamble-fallback shape (a third, separate `format!`).
    #[test]
    fn a_preamble_fallback_starts_on_its_own_line() {
        let (blocks, shape) = guide_blocks_for(
            DECLARING,
            Some("no_such_tool.no_such_action"),
            &json!({}),
            &mut ledger(),
        );
        assert!(matches!(shape, Some(GuideDeliveryShape::Preamble)));
        let reply = behind_a_primary(blocks);
        assert!(
            seam_defects(&reply).is_empty(),
            "{:?}",
            seam_defects(&reply)
        );
    }

    /// The same seam, section shape (`section_block`, shared by the post-call delivery and
    /// the pre-call gate).
    #[test]
    fn a_served_section_starts_on_its_own_line() {
        let (blocks, shape) =
            guide_blocks_for(DECLARING, Some(SERVED_SELECTOR), &json!({}), &mut ledger());
        assert!(matches!(shape, Some(GuideDeliveryShape::Section)));
        let reply = behind_a_primary(blocks);
        assert!(
            seam_defects(&reply).is_empty(),
            "{:?}",
            seam_defects(&reply)
        );
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

    /// A ledger that belongs to a conversation, as every MCP session's does.
    /// `anonymous` has no key and the gate stands down for it by design.
    fn keyed_ledger() -> (GuideLedger, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let led = GuideLedger::load("gate-sess", Some(dir.path().join("guide_hints")));
        assert!(
            led.key().is_some(),
            "premise: this ledger names a conversation"
        );
        (led, dir)
    }

    #[test]
    fn the_gate_serves_a_declared_section_once_and_stamps_it() {
        let (mut led, _dir) = keyed_ledger();
        let blocks = first_use_guide_gate(Some("doc.move"), "d1", &mut led);

        assert!(!blocks.is_empty(), "a declared destructive shape is held");
        assert!(
            blocks.iter().any(|b| text(b).contains("§")),
            "what ships is a SECTION, not the preamble fallback"
        );
        // The re-issue of the identical call must find it spent and execute.
        assert!(
            first_use_guide_gate(Some("doc.move"), "d1", &mut led).is_empty(),
            "the identical re-issue runs"
        );
        // And so does ANY call once one re-issue has run (the section is acknowledged).
        assert!(
            first_use_guide_gate(Some("doc.move"), "d2", &mut led).is_empty(),
            "after one re-issue ran, nothing else is held for it"
        );
        // And the post-call emission must not serve it a second time.
        let (after, _) = guide_blocks_for("librarian", Some("doc.move"), &json!({}), &mut led);
        assert!(
            after.is_empty(),
            "the gate already delivered the section; the post-call path ships nothing"
        );
    }

    #[test]
    fn the_gate_never_holds_a_call_for_a_shape_no_section_serves() {
        let (mut led, _dir) = keyed_ledger();
        assert!(first_use_guide_gate(Some("doc.no_such_action"), "d", &mut led).is_empty());
        assert!(
            first_use_guide_gate(None, "d", &mut led).is_empty(),
            "a tool with no selector has no declared shape to gate on"
        );
        assert!(
            led.is_empty(),
            "a gate that held nothing must stamp nothing — a stamp with no delivery is starvation"
        );
    }

    #[test]
    fn the_gate_stands_down_for_an_anonymous_ledger() {
        // The non-interactive opt-out: no conversation, nobody to acknowledge.
        let mut led = ledger();
        assert!(first_use_guide_gate(Some("doc.move"), "d", &mut led).is_empty());
        assert!(led.is_empty());
    }

    #[test]
    fn the_gate_does_not_hold_a_call_whose_section_the_session_already_holds() {
        // A fork's seeded ledger, or a session that read the section on an earlier call.
        let (mut led, _dir) = keyed_ledger();
        let first = first_use_guide_gate(Some("doc.delete"), "d1", &mut led);
        assert!(!first.is_empty(), "premise: doc.delete shares the section");
        // Same section, DIFFERENT call, and the first one's hold is still pending: the
        // second must NOT run on the strength of the first's hold.
        assert!(
            !first_use_guide_gate(Some("doc.move"), "d2", &mut led).is_empty(),
            "a different call is held while another's hold is pending"
        );
        // Once a held call's re-issue has run, the section is acknowledged.
        assert!(first_use_guide_gate(Some("doc.delete"), "d1", &mut led).is_empty());
        assert!(
            first_use_guide_gate(Some("doc.move"), "d2", &mut led).is_empty(),
            "doc.move and doc.delete are served by one section; it was acknowledged"
        );
    }

    #[test]
    fn a_stamped_bare_topic_covers_every_section_of_it() {
        // `get_guide("librarian")` stamps the bare topic; a fork seeded with only that
        // key is the same case. Neither holds anything for a call it already read.
        let (mut led, _dir) = keyed_ledger();
        led.insert("librarian".to_string());
        assert!(first_use_guide_gate(Some("doc.move"), "d", &mut led).is_empty());
        assert!(first_use_guide_gate(Some("doc.delete"), "d", &mut led).is_empty());
        assert_eq!(led.key_set().len(), 1, "and it stamps nothing further");
    }
}
