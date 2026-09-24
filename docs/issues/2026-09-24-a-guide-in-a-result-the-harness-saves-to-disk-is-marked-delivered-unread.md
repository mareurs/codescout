---
id: '1dce65964572186a'
kind: bug
status: open
title: 'BUG: a guide auto-injected into a result the harness saves to disk is marked delivered, and the model sees only a 2 KB preview'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-24
related:
- docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md
severity: medium
---

# BUG: a guide auto-injected into a result the harness saves to disk is marked delivered, and the model sees only a 2 KB preview

## Summary

codescout marks a guide topic delivered the moment its block is pushed onto a tool response. Claude Code may then decide — client-side, after the response has left the server — that the result is too large to inline, save it to `tool-results/<id>.json`, and show the model a ~2 KB preview of the **first** block. The guide is always a later block, so it is never in the preview, and the ledger suppresses it for the rest of the session. The model is not told it missed anything. `tracker-conventions` is the one that trips this: 59,381 bytes, shipped whole, it can by itself push a small answer over the line.

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

Not started. In order of how much they close:

1. **Do not stamp what may not arrive.** When the answer plus guide blocks would exceed a conservative bound, ship a one-line pointer (`get_guide("<topic>")`, with its size) instead of the body, and leave the topic unstamped. The bound must sit below the smallest observed saved result (29.4 KB) since the threshold is not known. This closes the class for any future large guide.
2. **Make `tracker-conventions` declare `serves:` sections**, as `librarian` does, so auto-injection ships a section. Fixes the instance that actually occurs; an explicit `get_guide("tracker-conventions")` would still return 59 KB and still be saved.
3. Shrink the guide.

## Tests added

None — this record opens the defect. A fix under (1) wants a test that a guide which would push the response past the bound is NOT stamped and that a pointer ships instead — asserted on the ledger, since a response-shape assertion alone is satisfied by a stamped-and-dropped block.

## Workarounds

When a `<persisted-output>` preview arrives from a codescout call, read the saved file's later blocks (`Read` on the path shown). For `tracker-conventions` specifically, read `src/prompts/guides/tracker-conventions.md` by heading.

## Resume

Decide between Fix 1 and Fix 2; 1 is the class fix.

## References

- `docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md` — found in the same probe
- `docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md` — another delivery-timing defect in the same emitter
