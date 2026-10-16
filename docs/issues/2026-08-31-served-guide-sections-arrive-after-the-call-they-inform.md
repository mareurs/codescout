---
kind: bug
status: fixed
tags:
- cluster/gate-keyed-on-unobservable-event
- guides
- section-grain-delivery
- tool-surface
- ack-handle
- progressive-disclosure
closed: 2026-10-16
opened: 2026-08-31
owner: marius
related: []
severity: medium
unverified: Agent ack-reflex risk is unmeasured - whether a caller reads the held section or reflexively re-issues needs an eval arm; and no live-session reproduction on a rebuilt binary has been run (tests exercise the server in-process).
---

# BUG: a `serves:`-declared guide section is delivered with the RESPONSE, so it cannot prevent the first-call mistake it exists to prevent

## Summary

Section-grain guide delivery injects a `<!-- serves: tool.action -->` section into the
**response** of the first matching call. For a read that is fine — you can re-read and
retry. For a **destructive** call it is structurally too late: the guidance that would
have stopped the mistake arrives attached to the result of having made it.

The consequence is not a broken call today; it is a **ceiling on the tool-surface budget**.
Guidance that prevents a first-call error can never be moved off the always-present schema
surface, no matter how long it is, because moving it would silently convert "expensive but
protective" into "cheap and useless".

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live; decision owed.** A
subagent's first `librarian(action="doctor")` call received its served sections AFTER the result, with
`_guide_hint: "selected for this call"`. At the bytes: `src/tools/core/types.rs:1383` runs `self.call(...)`
before `run_post` (`:1476`) → `emit_guide_sections` → `guide_blocks_for` (`src/engines/emitters.rs:98`); there is
no `run_pre` anywhere in `src/`; `parse_declarations` (`guide_index.rs:219`) still parses only `serves:` and
`requires:`. The `write_ack` capture/replay machinery exists and is wired to nothing guide-related. What is owed
is the choice `## Resume` already names: a `blocks:` declaration versus a call-property trigger. Verified by a
read-only Opus subagent dispatched by sessionId `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92`
(`deep-agent-context-observations:DCX-3`).

## Symptom (Effect)

Observed ~8 times in one session (2026-08-31) across `artifact`, `librarian` and
`get_guide` topics. Every injection arrived inside a tool result, after execution. The
envelope names the ordering itself:

```
"_guide_hint": "Section(s) of 'librarian' auto-injected below, selected for this call."
```

*for this call* — the call that already ran. A concrete pair from that session:

```
artifact(action="update", id=..., patch={body_edits: [...]})   → succeeded
   ...and the response carried:
   get_guide('librarian') § Choosing a mode — anti-patterns
   "Avoid this anti-pattern (caused a real ~600-line tracker body loss)"
```

The section documenting a 600-line data loss is delivered *after* the write that could
have caused it.

## Reproduction

```
git rev-parse HEAD                     # d94dd53d at filing
# any first-in-session call to a served shape:
librarian(action="doctor")
```

Read the response: the served section is in the returned content blocks, not in anything
that preceded the call.

## Environment

codescout 0.15.0, branch `experiments`, MCP stdio transport, project `codescout`.
Mechanism: `src/prompts/guide_index.rs` (`serves:` / `requires:` parsing),
`src/server.rs` (routing + the `SECTION_WAIVERS` coverage test at :3272).

## Root cause

Not a defect in the parser or the router — both are correct and well guarded. The
limitation is in **where the injection point sits relative to execution**: the section is
attached during response post-processing, so no ordering exists in which it precedes the
side effect.

Measured 2026-08-31: eight injections in one session, all in tool results, none before a
call. Not inferred from source — observed at runtime.

## Evidence

### It is already costing surface budget

While compacting the tool surface the same day (`52712759`, `d94dd53d`), this property is
what decided which descriptions could move to a guide and which could not:

| stayed inline, only because of this bug | bytes |
|---|---|
| `artifact.patch` — wrong mode wipes a body | 1068 |
| `edit_markdown.action` — `replace` destroys a section | 578 |
| `artifact.new_rel_path` — mints a new id, dangles every citation | 509 |

**~2155B of always-present surface** is pinned in place by delivery timing alone. Every
one of those is reference material a caller needs *once*; none of it could move, because
each is the thing that stops a destructive first call.

By contrast `librarian.fix` (987B) *could* move — not because it is shorter, but because
every mode is a dry run until `confirm=true`, so its first call is harmless.

## Hypotheses tried

1. **Hypothesis:** the section might be delivered on some pre-call path for write shapes.
   **Test:** read every injection received in one session's transcript and check its
   position relative to the tool result.
   **Verdict:** rejected — all eight arrived inside the result.

## Fix

Applied 2026-10-16 as decision B (see `### Implemented 2026-10-16` below). Original proposal, from Marius, 2026-08-31 — **block the first call, serve
the guide, and let the agent acknowledge**:

1. First call to a served shape is **not executed**. The full input is stashed.
2. The response carries the guide section plus a handle.
3. If the agent still judges the call correct, it re-invokes with the handle alone; the
   server replays the **stored** input.

This inverts the ordering without a second round of composition — the agent does not
re-emit the call, so the cost is one cheap acknowledgement rather than a full retry.

**The machinery already exists and is tested** — it is wired to a different trigger
(write-scope approval), not to guides. `src/tools/core/write_ack.rs`:

- `WriteOutcome::Pending` — captured, not executed.
- `resolve_write_or_capture` — *"On an outside-root rejection, stash the full input and
  return a `pending_ack` envelope instead of failing."*
- `maybe_replay_ack` — *"if `input["path"]` is an `@ack_*` write handle, approve its
  directory for the session and return the original stored input."*
- Guarded by `replay_approves_dir_and_returns_stored_input`,
  `replay_cross_tool_handle_rejected`, `replay_unknown_handle_errors`.

So this is a **second trigger on proven machinery**, not new machinery. The same
`@ack_*` handle kind already covers dangerous commands and out-of-scope writes
(`get_guide("progressive-disclosure")` § The @ref buffer).

### Open design questions, none blocking

- **Scope it, or every first call pays a stall.** Almost certainly only shapes whose
  section is marked as *protective* — a new declaration (`<!-- blocks: tool.action -->`
  beside `serves:`) rather than upgrading all 12 existing ones, so the cost lands only
  where a first-call mistake is destructive.
- **Idempotent and read-only calls should never block.** A gate on `librarian(doctor)`
  with no `fix=` buys nothing and costs a round trip.
- **The stash needs a TTL and a size bound**, as `output_buffer` handles do.
- **Non-interactive callers** (CLI, scripts) need an opt-out, or a first call in a
  pipeline hangs on an acknowledgement nobody will send.
- **Does the block itself become the thing agents learn to ack reflexively?** If so it
  degrades to today's behaviour plus a round trip. Worth an eval arm before shipping
  broadly — `prompt-engineering` can measure whether the ack is read or reflexive.

### Adoption is per-TOPIC and all-or-nothing — measured 2026-08-31

A constraint found while scoping a candidate move, and it bounds how any of the above
can be rolled out incrementally.

`GuideIndex::declares()` (`src/prompts/guide_index.rs`) is a **phase switch**, in its own
words: *"Whether this topic has opted into section-grain delivery. Topics with no
declarations keep the whole-topic path."* So the **first** `serves:` added anywhere in a
guide flips that entire topic from whole-topic delivery to section-grain — and every tool
already routing to it drops to **preamble only** unless it also gains a declaration.

Concretely, the reason `edit_code.at_line` (493B) was NOT moved:

- `edit_code` has no `relevant_guide_topic` at all, so it routes to no topic and a
  `serves: edit_code.*` section would never fire, wherever it were written.
- Wiring it to `symbol-navigation` would add the first declaration to that guide, flipping
  it to section-grain — and `symbols`, `references` and `call_graph` all route there today
  with no declarations of their own, so all three would silently degrade.
- `server.rs:3272` catches this rather than letting it ship, so the failure is loud. But
  the work is then: one `relevant_guide_topic`, three new `serves:` declarations or
  waivers, plus the section — a delivery-model refactor of a working guide, to buy 493B.

**Implication for the gate.** A `blocks:` declaration must not inherit this switch. If
adding the first `blocks:` to a topic changed that topic's delivery for every other tool
routing to it, the gate could not be adopted one call at a time — which is the only safe
way to adopt something that stalls a call. Whatever the gate's declaration is, it needs to
compose additively with whole-topic delivery, or `librarian.md` (already section-grain) is
the only guide that can ever host one.

### Payoff if it lands

The ~2155B above becomes movable, and the rule "long reference material lives in guides"
stops having a destructive-tools exception. That is roughly 4% of the 56519-char budget
recovered from a single mechanism change.

### Implemented 2026-10-16 — decision B: a per-call property gate, not a declaration

Decision (user, via main): gate on a property of the CALL, not a new `blocks:` declaration. Design note, boundaries first.

**Boundary.** One new trait method, one early-return in `Tool::call_content`, one pure function. No new handle kind, no new declaration, no change to `GuideIndex::declares()`.

- `Tool::is_destructive(&self, input) -> bool` (`src/tools/core/types.rs`, beside `is_write`, default `false`). Input-derivable only, because the topic and section are chosen from the RESULT on the post path and no result exists yet. Opt-in per call shape. `LibrarianAdapter` answers `true` for `doc` `move` and `librarian` `merge_worktree` unless `dry_run` is true. Not widened to the dry-run-by-default shapes (`doc` `delete`/`graft`/`rekey_prefix`, `librarian` `fix`) nor `doc` `update` (shrink-guarded): a stall there buys no protection.
- Pre-phase: in `call_content`, after `selector_key` and before `self.call(input, ctx)` consumes `input`. If `is_destructive(&input)`, ask `first_use_guide_gate(selector, ledger)` (`src/tools/core/guide_emit.rs`). It resolves the topic with `GUIDE_INDEX.topic_declaring(selector, &Null)` and calls the existing `guide_blocks_for` with a null result, so a plain `tool.action` shape matches on the selector alone and a `path_contains` shape cannot gate, by construction.
- Returns: not the tool's result but `[ {executed:false, tool, action, reason}, <guide section block(s)> ]`. The caller re-issues the IDENTICAL call. No handle: nothing is stashed. The cost is one re-emission of the call's input, which for `doc.move` and `merge_worktree` is a few fields. A handle (replay the stored input, as `write_ack.rs` does) would need a NEW ack kind, because `maybe_replay_ack` is keyed on `input["path"]` and approves a directory. That is the design's one deliberate omission, to be revisited only if re-emission proves costly.
- No double serve, and the state is the ledger. `guide_blocks_for` stamps `topic#heading` before the call returns, so (a) the re-issue finds the key and executes, (b) the post-call emission finds it and ships nothing, (c) a fork's `seed_inherited` ledger already holds it and is never held (`docs/issues/2026-09-24-a-fork-child-is-re-served-every-guide-it-inherited.md`), (d) a post-compact re-arm clears it and the gate fires again. TTL, persistence and per-principal scoping are the ledger's; there is no stash to bound.
- `GuideIndex::declares()` is untouched: the gate reads the SAME `serves:` sections the post path does (`doc.move, doc.delete` is one section in `librarian.md`). It never holds without text to show: no declared section for the selector, or all already delivered, means execute.
- Non-interactive opt-out: the gate stands down for a ledger with no conversation key (`GuideLedger::key() == None`: the CLI, scripts, clients with no session identity). Nobody is there to re-issue, so those keep today's post-call delivery.
- Held responses skip the param-corrections advisory and the path-stripping; the call did not run.

**OPEN EVAL NEED, not built here:** the block degrades to a reflexive re-issue if callers learn to ack without reading. Whether the section is read is unmeasured and wants an eval arm (`prompt-engineering` can measure read vs reflexive). Until then the gate is two shapes wide on purpose.

## Tests added

Written red first, one assertion on the SIDE EFFECT, not the response shape:

- `server::guide_hint_tests::the_first_destructive_call_is_held_and_serves_its_guide_before_the_side_effect`: creates an artifact, issues `doc` `move`; the file has not moved, the response says `executed:false` and carries the `librarian` section; the identical re-issue moves it and does not repeat the section. It failed on the pre-gate code at the first assertion (the file had already moved).
- `tools::core::guide_emit::tests::the_gate_serves_a_declared_section_once_and_stamps_it`, `..._never_holds_a_call_for_a_shape_no_section_serves` (and stamps nothing), `..._stands_down_for_an_anonymous_ledger`, `..._does_not_hold_a_call_whose_section_the_session_already_holds`.
- `librarian::adapter::tests::only_move_and_a_real_merge_worktree_are_held_for_the_guide`: the discriminator rows, `delete`/`graft`/`rekey_prefix`/`update` and a dry-run `merge_worktree` are not held.

Not covered: a live session. `a_p50_session_stays_under_the_committed_emission_byte_ceiling` still passes.

## Workarounds

Keep first-call-protective guidance inline in the tool schema, and move only reference
material whose first call is harmless. That is the rule applied in `d94dd53d`, and the
discriminator to use is: *can a first call, made without this text, destroy something or
silently produce a wrong result?* If yes it stays inline; if it merely produces an error
or a dry run, it can move.

## Resume

Gate is implemented for two shapes (see `### Implemented 2026-10-16`). Remaining: (1) the ack-reflex eval; (2) widen shape by shape only on evidence that a first call damaged something; (3) a live reproduction on a rebuilt binary (`doc(action="move", ...)` first call in a fresh session must answer `executed:false`).

## References

- `src/prompts/guide_index.rs`: `serves:` / `requires:` parsing; unchanged.
- `src/server.rs`: `SECTION_WAIVERS` test asserting every routed shape has a serving section.
- `src/tools/core/guide_emit.rs::first_use_guide_gate`, `src/tools/core/types.rs` (`Tool::is_destructive`, the pre-call block in `call_content`), `src/librarian/adapter.rs` (`LibrarianAdapter::is_destructive`): the implementation.
- `src/tools/core/write_ack.rs`: the capture/replay machinery that was NOT reused (see the no-handle note above).
- `52712759`, `d94dd53d`: the tool-surface compaction whose stopping point this defines.
