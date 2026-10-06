---
id: 1912bb6e54144e18
kind: bug
status: mitigated
title: 'BUG: a guide auto-injected into a result the harness saves to disk is marked delivered, and the model sees only a 2 KB preview'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-10-06
opened: 2026-09-24
related:
- docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md
severity: medium
unverified: 'Root cause not addressed: the ledger still stamps on push, so a guide block the harness drops is still marked delivered. Only the oversize-whole-topic case is mitigated (Fix 1). Fix 2 (serves: sections for tracker-conventions) and Fix 3 (shrink the guide) are not done. The 16 KiB bound is not mutation-checked (cap_probe row is Deferred) and the harness save threshold is still not determined. A pointer block lost to a saved-to-disk result spends its key and nothing re-points.'
---

# BUG: a guide auto-injected into a result the harness saves to disk is marked delivered, and the model sees only a 2 KB preview

## Summary

codescout marks a guide topic delivered the moment its block is pushed onto a tool response. Claude Code may then decide — client-side, after the response has left the server — that the result is too large to inline, save it to `tool-results/<id>.json`, and show the model a ~2 KB preview of the **first** block. The guide is always a later block, so it is never in the preview, and the ledger suppresses it for the rest of the session. The model is not told it missed anything. `tracker-conventions` is the one that trips this: 59,381 bytes, shipped whole, it can by itself push a small answer over the line.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live, and one clause
understates it.** Reproduced by a subagent: a narrow `doc(action="find", …)` came back as a harness-saved
61 KB result whose second block (59,593 B) was `auto-injected get_guide('tracker-conventions')`, and its
ledger then held `"tracker-conventions"`. **The model is not merely left untold — it is told the opposite:**
the answer was small enough that the saved preview carried the `_guide_hint` "Full guide auto-injected …
do not re-call get_guide". At the bytes: `guide_emit.rs:136-146` stamps on push with no size bound;
`tracker-conventions.md` is 59,381 B with **0** `serves:` declarations (confirmed independently by the
coordinator), so that one guide alone overflows any response. **Reproduction step 2 as written no longer
reproduces** — the 50-row open-bug list now overflows first and `LibrarianAdapter::relevant_guide_topic`
(`adapter.rs:408-474`) routes it to `progressive-disclosure`; use a narrow filter. Fix 1 stands.

## Symptom (Effect)

Measured 2026-09-24T13:33:48Z, session `774ba049-d97c-443a-b31d-f662a9cb6a1e`, first `doc(action="find", kind="bug", …)` after a compaction. The response carried two blocks:

```
block 0  text   4,777 B   the query answer
block 1  text  59,137 B   <!-- auto-injected get_guide('tracker-conventions') — first call this session … -->
```

What the model received, verbatim head:

```
<persisted-output>
Output too large (64.2KB). Full output saved to: ~/.claude/projects/-home-marius-work-claude-codescout/774ba049-…/tool-results/toolu_01A37T1KKGj3Lmssf3mNDMLV.json

Preview (first 2KB):
[ { "type": "text", "text": "{\n  \"count\": 9, …
```

The session transcript stores that 2,552-byte preview and no guide marker. The ledger (`~/.local/state/codescout/guide_hints/774ba049-….json`) holds `"tracker-conventions":"2026-09-24T13:33:48.497994075Z"`. The answer alone was 4.8 KB; the guide is what made the result 64 KB.

## Reproduction

1. Fresh codescout session (or `workspace(post_compact=true)` after a compaction, which clears the ledger).
2. Make any call that triggers `tracker-conventions` and returns a result a few KB long — `doc(action="find", kind="bug", filter={"status":{"in":["open","taken","investigating","zombie"]}})` did.
3. Observe the `<persisted-output>` preview; confirm the guide is in the saved file (`blocks: 2`, marker in block 1) and in the ledger, and absent from the transcript.

`experiments` @ `db6a5f0e`.

## Environment

Linux, Claude Code interactive, profile `~/.claude`, codescout release binary over stdio.

## Root cause

- **The gate is the push.** `guide_blocks_for` (`src/tools/core/guide_emit.rs:142-145`) calls `emitted.insert(topic)` when the block builds, and its doc comment states the rule: a key is inserted *"ONLY when its block is actually pushed"*. Pushing onto the MCP response is the last point the server can observe. Whether the model reads the block is decided afterwards by the client, outside that boundary — so "pushed" stands in for "received", and the stand-in fails silently.
- **The block that goes missing is always the guide.** A guide is block 2+ by design (`src/tools/core/types.rs:1257-1261`: the answer stays first), and the preview covers the head of block 0 only.
- **Why this topic.** `tracker-conventions` declares no `serves:` sections, so it takes the whole-topic branch (`guide_emit.rs:136-146`) and ships `src/prompts/guides/tracker-conventions.md` entire: 59,381 bytes — 2.3× the next-largest guide (`librarian.md`, 25,266 B), which *does* declare sections and ships only the matching one.

measured 2026-09-24: the saved file parsed as JSON — 2 blocks, marker in block 1; the transcript's `tool_result` for that call — 2,552 B, no marker; the ledger file — the stamp above.

## Evidence

### How often

Files under `~/.claude/projects/-home-marius-work-claude-codescout/*/tool-results/` containing an auto-injection marker, 2026-09-24 ~15:10Z — **one profile, one project directory**:

```
tracker-conventions     36
progressive-disclosure   1
(all other topics)       0
```

Each is a delivery the ledger recorded and the model received as a preview — unless the model later opened the saved file, which was not measured. The transcript stores only the preview for a saved result (checked on the instance above), so these 36 are disjoint from inline deliveries.

### No single size threshold

Across the same directory's transcripts: 64 saved results, the smallest `Output too large (29.4KB)`; 45,056 inline results, the largest 91,643 B. The corpus mixes native and MCP tools and Claude Code versions, so the cut-off is not one number here and is **not determined** — which matters for any size-based fix (see Fix).

## Hypotheses tried

1. **The model saw the guide and the ledger is right.** Rejected: the transcript's `tool_result` is the 2,552-byte preview and carries no marker.

## Fix

**Fix 1 shipped; Fixes 2 and 3 did not.** Status is `mitigated` because the root cause (the ledger stamps at push, before the client decides what the model sees) is unchanged.

1. **Do not stamp what may not arrive. DONE (mitigation).** `guide_blocks_for` no longer auto-injects a whole-topic guide larger than `MAX_AUTO_INJECT_GUIDE_BYTES` (16 KiB, `src/tools/core/guide_emit.rs`). It ships a one-line `get_guide("<topic>")` pointer carrying the size, does not stamp the bare topic, and stamps a separate `<topic>#<pointer>` key once, so the pointer cannot starve a declared section for the same call (`emit_guide_sections` stops at the first candidate that ships). An explicit `get_guide("<topic>")` still returns the full body and stamps the bare topic, so the pointer cannot loop. Today only `tracker-conventions` (~59 KB) is over the bound; the next-largest non-declaring guide (`iron-laws-detail`) is ~14.7 KB. `fda10a31` classifies the new constant as `RESULT_CAP guide_emit.auto_inject_bound` and adds a `Coverage::Deferred` row in `src/tools/core/cap_probe.rs`: it is NOT mutation-checked and no row is certified against that table's marker grammar.
2. **Make `tracker-conventions` declare `serves:` sections. NOT DONE.** High risk: it trips `SECTION_WAIVERS`. Until then auto-injection of that topic is always the pointer, never a section.
3. **Shrink the guide. NOT DONE.**

Residual risks, not fixed:

- If the pointer block is itself lost because the primary block was saved to disk, the `<topic>#<pointer>` key is already spent and nothing re-points. The model is then told nothing (the same silent-loss class, now for a ~1-line block).
- Doc drift: the `<topic>#<pointer>` key is missing from the ledger-key tables in `src/engines/mod.rs` and from the `ledger_keys()` docs in `src/tools/guide_index.rs`, and the test doc comment at `src/server.rs:14051` still says `tracker-conventions` "ships WHOLE" (checked 2026-10-06: a grep for `#<pointer>` over `src/` finds the key only in `guide_emit.rs`).

## Tests added

Added by `5d4ee239`, asserted on the ledger and not only on response shape:

- `src/server.rs` `guide_hint_tests::an_oversize_whole_topic_guide_ships_a_pointer_and_is_not_stamped` (end to end through a `doc` create; also asserts the explicit fetch returns the whole body).
- `src/tools/core/guide_emit.rs` tests: `an_oversize_whole_topic_guide_ships_a_pointer_and_does_not_stamp_the_topic`, `a_whole_topic_guide_within_the_bound_ships_whole_and_stamps` (positive twin), `a_spent_pointer_ships_nothing_and_still_leaves_the_topic_unstamped`, `an_explicitly_fetched_oversize_guide_is_not_pointed_at_again`.

`fda10a31` adds no behavioural test; it adds the cap-class annotation and the Deferred `cap_probe.rs` row so `every_cap_constant_is_classified` passes. Test names are taken from the commit diff; this bookkeeping pass did not re-run them.

## Workarounds

When a `<persisted-output>` preview arrives from a codescout call, read the saved file's later blocks (`Read` on the path shown). For `tracker-conventions` specifically, read `src/prompts/guides/tracker-conventions.md` by heading.

## Resume

Mitigated, not fixed. Fix 1 is in on `experiments` (local, not pushed at the time of writing). Remaining, needs a human decision: Fix 2 (give `tracker-conventions` `serves:` sections, high risk against `SECTION_WAIVERS`) versus Fix 3 (shrink the 59 KB guide); and whether a pointer lost to a saved-to-disk result needs a re-point path. Small follow-up: add the `<topic>#<pointer>` key to the ledger-key tables in `src/engines/mod.rs` and the `ledger_keys()` docs in `src/tools/guide_index.rs`, and fix the "ships WHOLE" comment at `src/server.rs:14051`.

## Fix provenance

- **SHA:** `5d4ee239` (`experiments`)
- **patch-id:** `a160bc04f6d668401d1e1713f7910630b72c795f`
- **SHA:** `fda10a31` (`experiments`)
- **patch-id:** `7cef622a70e375294f89c2cc41e0edf79dce3947`

## References

- `docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md` — found in the same probe
- `docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md` — another delivery-timing defect in the same emitter
