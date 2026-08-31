---
id: a5054d135acacbe3
kind: bug
status: open
title: workspace(post_compact=true) clears the whole guide ledger without checking that a compaction happened — ~49 KB re-delivered on one mistaken call, and the flag name is what misleads
tags:
- guides
- guide-ledger
- workspace
- affordance
- doc-vs-code
---

## Symptom

`workspace(post_compact=true)` unconditionally clears `guide_hints_emitted`. It does not
and cannot check that a compaction actually occurred, so a caller who has just **`/mcp`
reconnected** — not compacted — silently discards the conversation's entire dedup ledger
and re-receives every guide topic it subsequently touches.

## Measured, 2026-08-31, on the author of this file

Called after a plain `/mcp` reconnect with no compaction. The live ledger went from
**13 topics to 1** (only `project-activation-bootstrap`, re-added by the call itself).
Immediate re-delivery, both observed in the same session:

| topic | bytes |
|---|---|
| `workspace-state` | 10,355 |
| `tracker-conventions` | 38,870 |
| **total so far** | **~49 KB** |

That is one call. The remaining 11 topics re-deliver on next touch.

## This is an affordance problem, not a knowledge problem

The correct behaviour is documented unambiguously in `get_guide("workspace-state")`
§ *Per-session state reset*: the keyed ledger **survives** `/mcp` restarts, and server
construction re-arms only the session-opening topic on reconnect. That guide was in the
caller's context at the time.

What misleads is the **parameter name**. `post_compact` reads as "post-reconnect
housekeeping" to someone who has just reconnected, and both events are followed by the
same visible thing — a fresh server process. The correct call after a reconnect is a plain
`workspace(action="status")`, which the same caller then made one call later.

Note the two effects are not equally expensive and are bundled:

- **LSP flush** (`ctx.lsp.shutdown_all()`) — cheap, idempotent, harmless when spurious.
- **Ledger clear** — expensive, silent, and unrecoverable within the conversation.

Only the second needs a gate. A fix that refuses the whole call would be worse than one
that keeps flushing and skips the clear.

## The obvious fix is REFUTED — recorded so nobody re-proposes it

The natural design is "gate on whether a `SessionStart` fired recently", using
`Rendezvous`'s `Entry.hook_at`. **It does not work, and the refutation is already in the
tree**, at `codescout-companion:hooks/lib.mjs:333`:

> `hook_at` gets old — was refuted by measurement: the companion stamps ONLY on
> SessionStart, so `hook_at` records "when did this conversation last start" […] deployed
> a while, `hook_at` age becomes **time-since-last-proof-of-life**

`lib.mjs:348` is a `Refresh hook_at` helper precisely for that liveness role. Confirmed
live: this session's slot (`~/.local/state/codescout/servers/316305.json`) reported
`hook_at` **0 minutes old** with no `SessionStart` having fired for hours. A staleness
check on it would measure proof-of-life and fire essentially at random.

**And `rendezvous.rs`'s own doc comment on `Entry.hook_at` is stale on exactly this
point** — it still says the companion hook is the only writer, which is what makes the
refuted design look sound to a reader who starts there. That doc-vs-code drift is the
part most likely to mislead the next person, and is worth fixing whether or not the
guard below is ever built.

## The one viable design

The discriminating signal exists and is thrown away one line from where it is needed.
`codescout-companion:hooks/session-start.mjs` reads `input.source` — Claude Code's enum is
`{startup, resume, clear, compact}` — and already gates its own POST-COMPACT message on
`source === 'compact'` (`:336`). But when it stamps the slot it writes only
`e.hook_at = stampedAt` (`:76`), never the source.

So:

1. **Companion**: write the source alongside the stamp (e.g. `hook_source: 'compact'`).
2. **codescout**: `post_compact=true` reads the slot for this session. If it positively
   knows the last `SessionStart` was **not** a compaction, skip the ledger clear and say
   so in the response; still perform the LSP flush.
3. **Degrade to today's behaviour on absence.** No slot, no source, hookless client ⇒
   clear as now. This mirrors `inherited_stamp`'s existing philosophy — *"absent a
   companion the scan finds nothing, so a hookless client keeps the blunt behaviour
   exactly as before"* — and never refuses on missing evidence.

## Not fixed, deliberately

One datapoint. The cost per occurrence is measured (~49 KB) but the **rate** is not: the
correct usage is driven by a hook that fires only on compaction and explicitly instructs
the model to make the call, so a spurious call requires the model to invoke it without
that instruction in context. That happened once, here, and nothing establishes how often
it happens generally.

A two-repo change on n=1 is not obviously worth it. What *is* worth doing independently
is correcting the stale `Entry.hook_at` doc comment, since it is what makes the refuted
design look correct.

