---
status: open
opened: 2026-08-31
closed:
severity: medium
owner: marius
related: []
tags: [guides, section-grain-delivery, tool-surface, ack-handle, progressive-disclosure]
kind: bug
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

Not applied. Proposed strategy, from Marius, 2026-08-31 — **block the first call, serve
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

### Payoff if it lands

The ~2155B above becomes movable, and the rule "long reference material lives in guides"
stops having a destructive-tools exception. That is roughly 4% of the 56519-char budget
recovered from a single mechanism change.

## Tests added

None — nothing implemented yet. A regression test for the fix should assert that the
first call to a blocking shape **did not execute** (observe the side effect's absence,
not the response shape), and that the replayed input is byte-identical to the stashed
one rather than re-parsed from the acknowledgement.

## Workarounds

Keep first-call-protective guidance inline in the tool schema, and move only reference
material whose first call is harmless. That is the rule applied in `d94dd53d`, and the
discriminator to use is: *can a first call, made without this text, destroy something or
silently produce a wrong result?* If yes it stays inline; if it merely produces an error
or a dry run, it can move.

## Resume

Decide whether to scope the gate with a new `<!-- blocks: tool.action -->` declaration in
`src/prompts/guide_index.rs::parse_declarations` (which already parses `serves:` and
`requires:` and rejects malformed shapes loudly), or to derive it from a property of the
call. Then wire `src/tools/core/write_ack.rs`'s capture/replay pair to that trigger —
read its four tests first, since they already pin the cross-tool and unknown-handle
rejections a guide gate would need identically.

## References

- `src/prompts/guide_index.rs` — `serves:` / `requires:` parsing; its doc comment on why a
  shape that parses but never matches is the failure mode this feature exists to prevent.
- `src/server.rs:3272` — the test asserting every routed shape has a serving section.
- `src/tools/core/write_ack.rs` — the capture/replay machinery this would reuse.
- `52712759`, `d94dd53d` — the tool-surface compaction whose stopping point this defines.
