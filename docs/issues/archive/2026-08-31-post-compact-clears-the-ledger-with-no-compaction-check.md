---
id: f6a748bcbeee1652
kind: bug
status: archived
title: workspace(post_compact=true) clears the whole guide ledger without checking that a compaction happened — ~49 KB re-delivered on one mistaken call, and the flag name is what misleads
tags:
- cluster/gate-keyed-on-unobservable-event
- guides
- guide-ledger
- workspace
- affordance
- doc-vs-code
closed: 2026-09-24
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

**Attempted and REVERTED 2026-09-19** (`4b12032e`), recorded so the third proposal is cheaper than the second. The clear was gated on `rendezvous_active()`, converging it with `ActivateProject::call`'s adjacent branch — which reads as closing an asymmetry and is not. The two branches guard different EVENTS and only one is observable: that branch's is a `/clear`, which `Rendezvous::poll` genuinely sees (it "returns the new session id ONLY when it changed"), while a compaction leaves the session id identical, and the companion stamps only `hook_at`, never the source. Since the companion is always active here, the gating meant a GENUINE compaction never re-armed the ledger — an under-serve on every real compaction traded for this file's n=1 over-serve. The inverted test `post_compact_clears_even_when_the_rendezvous_is_active` now reds on re-proposal. This ruling stands: the design in § *The one viable design* is still the only viable one, and still not obviously worth a two-repo change at n=1.

One datapoint. The cost per occurrence is measured (~49 KB) but the **rate** is not: the
correct usage is driven by a hook that fires only on compaction and explicitly instructs
the model to make the call, so a spurious call requires the model to invoke it without
that instruction in context. That happened once, here, and nothing establishes how often
it happens generally.

A two-repo change on n=1 is not obviously worth it. What *is* worth doing independently
is correcting the stale `Entry.hook_at` doc comment, since it is what makes the refuted
design look correct.

**Superseded 2026-09-24 — the viable design above was implemented** (see *Fix*). The ruling in this section still stands for what it ruled on: gating on `rendezvous_active()` remains wrong, and `post_compact_clears_even_when_the_rendezvous_is_active` still pins it.


## Fix

The design in § *The one viable design*, plus one piece it missed. **The measured case — a mistaken call after a plain `/mcp` reconnect — starts a NEW server with a fresh slot, and no `SessionStart` fires on `/mcp`**, so a source stamped only by SessionStart never reaches it. The new server must inherit it, which `inherited_stamp` now does.

- **Companion** (`claude-plugins:cf5ea29c`, patch-id `8ee2e1081663975c179922093446f13d0fbe9088`): `session-start.mjs` writes `hook_source` + `hook_source_at`, and treats the source as part of "already current" — the old skip (`e.session === sessionId && e.hook_at`) swallowed exactly the compaction stamp, since a compaction keeps the session id. The liveness refresher rewrites the whole object, so it preserves both fields.
- **codescout** (`ba3a787ef5486a4825349d313b3fb64455dff1d1` on `experiments`, patch-id `538f3a2a525265310386d375197898931d7033db`): `Entry` gains both fields (serde-defaulted); `inherited_stamp` carries the LATEST session start's source across `/mcp`, ordered by `hook_source_at` — never by `hook_at`, which the liveness refresher keeps moving; `poll` tracks it; `call_tool_inner` copies it onto the live ledger AFTER principal adoption; `post_compact` skips the clear only on a positive non-`compact` source and says so (`ledger: "kept"` + `ledger_note`). Absent or `compact` → the blunt clear, exactly as before.

**Residual, accepted:** a compaction long ago followed by a later mistaken call still clears — the last session start really was a compaction. Closing it would need "this compaction was already honoured" state persisted across restarts.

## Tests added

- `tools::config::tests::post_compact_keeps_the_ledger_when_the_last_session_start_was_not_a_compaction` — observed RED on the behavioural assertion (ledger wiped), then GREEN; `…clears_when_the_last_session_start_was_a_compaction` is its sandwich half.
- `tools::rendezvous::tests::publish_inherits_the_session_start_source_from_a_predecessor_slot`, `…publish_inherits_the_newest_session_start_source` (carries a `hook_at` trap: the older session start's slot has the newest liveness stamp), `…poll_reports_the_session_start_source_the_hook_wrote` — all RED first.
- `server::guide_hint_tests::post_compact_after_a_plain_session_start_keeps_the_ledger_end_to_end` — the wiring through `call_tool_inner`; RED before the copy existed.
- Companion `session-start.test.sh`: source recorded; compaction re-stamps on an unchanged session; RFC3339 `hook_source_at` — all RED first.
- Mutations (`scripts/mutation-probe.sh`, isolated), each KILLED by exactly its intended test: gate keeps on `compact`; inheritance ordered by `hook_at`; `poll` drops the source; `publish` drops the inherited source.
- Gate `GATE_EXIT=0`; `claude-plugins` `tests/run-all.sh` green.

## Resume

**Verified live 2026-09-24, both halves**, on release binary built 10:48:41Z (contains `ba3a787e`) with `claude-plugins` working tree at `cf5ea29c`:

- **Clearing half, through the `/mcp` inheritance path.** Session `774ba049-d97c-443a-b31d-f662a9cb6a1e` (`~/.claude`):
  - `/compact` fired SessionStart(`compact`) at 10:48:37.803Z.
  - `/mcp` then started server 3393665 at 10:49:02.787Z. Its slot carried `"hook_source":"compact"` with `hook_source_at` 10:48:37.803Z. That's **25 s before the server existed**, so the value was inherited; no SessionStart runs on `/mcp`.
  - The liveness refresh at 10:49:47Z preserved both fields.
  - `workspace(post_compact=true)` returned `"ledger": "cleared"`, and the ledger file went from 14 entries to 1: `project-activation-bootstrap`, re-emitted by the same call.
- **Kept half.** A headless `claude -p` (`~/.claude`, SessionStart `startup`, session `1e4fc97a-de21-438c-80f5-bb8e4e07f7ff`) called `status` and then `post_compact=true`. It got `"kept"` (3 turns, no error). This pairs with the result above: same binary, same hook, only the source differs.
  - It exercised the direct-stamp path (a fresh server whose own slot was stamped), not the inheritance path. The clearing half covers inheritance.
  - I didn't separately read the probe's ledger file. The observation is the response's `ledger` field.
  - The same probe exposed `92deba12cd82aaf0`: its SessionStart also stamped *my* server's slot. Its own verdict came from its own server, whose slot read `startup`.

**Known limit: startup race (raised by sessionId `09093108-1425-4f6d-9695-a9e3bb98ea0d`).**

- **Mechanism.** On `startup`/`resume` the source only arrives if the new server's slot exists before SessionStart runs. The measured slot-to-stamp margins were 51 ms and 86 ms. That peer's resumed session holds a slot with no source; the cause isn't decided between no slot yet, an empty `source`, and old code loaded.
- **Consequence.** Losing the race degrades to `"cleared"`, the pre-fix behaviour, which is the safe direction.
- **Scope.** The measured case (a mistaken call after `/mcp`) is unaffected, because the source was stamped into the predecessor slot long before.
- **Observed so far (a sample, not a rate):** interactive sessions were stamped on **0 of 3** starts (two `--resume`s in `~/.claude-sdd`, the second on companion 1.20.14, plus one fresh startup in `~/.claude` whose slot never got a stamp at all). `claude -p` startups were stamped on 5 of 5. The details and the startup case are in `798f69a248d72298`.
- **Addressed 2026-09-24 by `claude-plugins:3a069d5d`** (`b586243d`'s fix: a detached late stamper). Verified live: an interactive startup was stamped `startup` 58 ms after its slot appeared, and an interactive `--resume` re-stamped the inherited `startup` to `resume`. So `"kept"` is reachable after an interactive start or resume. The two bullets above describe the state before that fix.
  - At 11:16:51Z the server published its slot at .394Z, and the `SessionStart:resume` attachment was recorded at .499Z. The slot was never stamped; its next write was the liveness refresher.
  - The peer ruled out an empty `source` (it was `resume`) and old code (1.20.14's `session-start.mjs` has the stamping). "No slot yet at scan time" survives, but it isn't proven, because the hook's start time is not recorded.
  - **So `"kept"` may be close to unreachable after a resume, rather than occasionally missed.** The measured `/mcp` case is unaffected. A remedy would decouple the source from slot timing, for example a per-session record the server reads at adoption: `798f69a248d72298`.

**Archived 2026-09-24**, in one pass with `6d671794cd4da970`, `92deba12cd82aaf0` and `798f69a248d72298`. Citations in both repos were re-pointed in the same pass.
