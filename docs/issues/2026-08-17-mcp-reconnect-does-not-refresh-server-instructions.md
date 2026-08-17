---
id: '64fb2d6846f49865'
kind: bug
status: open
title: 'BUG: /mcp reconnect refreshes tool schemas but not server_instructions — a prompt-surface change cannot be verified in the session that made it'
tags:
- prompt-surfaces
- verification
- mcp
- tooling
closed: ''
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md
severity: medium
---

## Summary

`cargo rb` + `/mcp` makes a live session pick up new **tool schemas and tool behaviour**,
but **not** the `server_instructions` block. That text is composed into the model's system
prompt when the conversation first connects, and a mid-session reconnect does not rebuild
it. So an edit to `src/prompts/source.md` is invisible to the session that made it, no
matter how many times it rebuilds and reconnects.

This matters because the project's commit convention says the opposite. Both `d2cf4449`
and `8d44ee08` close with *"Needs cargo rb + /mcp before a live session sees them."* That is
correct for compiled code and wrong for the instructions surface.

## Symptom (Effect)

After `cargo rb` and **three** `/mcp` reconnects in one conversation, with the serving
binary confirmed at `0364c23a` (which contains `d2cf4449`):

| | Iron Law 1 tail | quickref `docs/trackers` row |
|---|---|---|
| Compiled surface at HEAD (`tests/fixtures/prompt_surfaces/server_instructions.md`) | `— refused only when the range overlaps a symbol; force=true reads it anyway.` | absent |
| `instructions` block in the model's context | `; force=true overrides.` | present |

The context copy is the **pre-`d2cf4449`** text — i.e. still the surface compiled into
`3d7f13ce`, the binary that served the conversation's *first* connection.

Tool schemas, by contrast, **did** refresh across the same reconnects: `librarian` gained
its `archetype` parameter (`99c71710`), and `edit_markdown`'s `new_string` guard
(`8d44ee08`) went from absent to active and was confirmed by probe.

## Reproduction

1. In a live session, note the `instructions` text (e.g. Iron Law 1's wording).
2. Edit `src/prompts/source.md` inside the `server_instructions` slice.
3. `cargo rb`, then `/mcp`.
4. Confirm the new binary is serving — a fresh tool-schema field, or
   `sqlite3 .codescout/usage.db "select codescout_sha from tool_calls order by id desc limit 1"`.
5. Re-read the `instructions` text. It is unchanged.

## Environment

- Claude Code 2.1.233, MCP stdio transport, codescout `experiments`.
- Observed 2026-08-17 across binaries `3d7f13ce` → `66487591` → `0364c23a`, three reconnects.

## Root cause

**Measured 2026-08-17:** the table above, plus the serving-binary trail read from
`usage.db.tool_calls.codescout_sha` (the column added for build provenance). Schemas
refreshed; instructions did not. Both facts come from the same three reconnects.

**Inferred, NOT verified against Claude Code's source:** MCP delivers `instructions` once,
in the `initialize` response. The host composes it into the system prompt at conversation
start. A `/mcp` reconnect re-runs `initialize` against a new child process and re-reads the
tool list, but the system prompt — already built, and already prompt-cached — is not
rebuilt from it. A prompt-cache boundary would produce the same observable, and this bug
does not distinguish the two. It does not need to: the operational consequence is identical.

## Evidence

### Schemas refreshed, instructions did not, in the same session

- `librarian` schema before reconnect: no `archetype` key. After: `archetype` present with
  its full enum. `99c71710` is the oldest commit in the range, so the pre-reconnect server
  predated it.
- `edit_markdown(action="edit", old_string=…, new_string omitted)` before: not probed.
  After: refused with `new_string is required for action="edit"`, and the target file
  verified byte-identical (54 bytes, `DELETE-ME` intact).
- `instructions` across all three reconnects: byte-identical, and matching neither of the
  two newer binaries.

### The claim this falsifies

`d2cf4449`: *"NOTE: source.md is compiled in. Needs cargo rb + /mcp before a live session
sees the restored clause."* Necessary, not sufficient.

## Hypotheses tried

1. **Hypothesis:** the reconnect did not take, so nothing refreshed.
   **Test:** re-fetch the `librarian` tool schema; query `codescout_sha` for a marker call.
   **Verdict:** rejected. Schema gained `archetype`; `codescout_sha` moved `3d7f13ce` →
   `66487591` → `0364c23a`.

2. **Hypothesis:** the compiled surface does not actually contain the restored clause, so
   there is nothing to see.
   **Test:** read `tests/fixtures/prompt_surfaces/server_instructions.md` at HEAD.
   **Verdict:** rejected. The fixture carries the overlap condition and has dropped the
   quickref row `d2cf4449` sold to pay for it.

## Fix

Documentation, not code — the host behaviour is not ours to change.

`src/prompts/README.md` should state plainly: a `server_instructions` or
`onboarding_prompt` change is **not** observable in the authoring session. `cargo rb` +
`/mcp` refreshes tool schemas and tool behaviour only. Verification requires either

- the snapshot fixture (`tests/fixtures/prompt_surfaces/server_instructions.md`) plus the
  `prompt_surfaces` tests — the authoritative check, and the one CI runs; or
- a **new** conversation, if the live-injected text itself must be eyeballed.

The commit-message `NOTE: … needs cargo rb + /mcp` line should split code from surface, so
the next author does not inherit the wrong expectation.

## Tests added

None — nothing here is codescout behaviour to pin. The existing `prompt_surfaces` snapshot
tests already cover *what gets compiled*; this bug is about what a session *observes*, which
no test in this repo can reach.

## Workarounds

Verify prompt-surface edits against the fixture and the `prompt_surfaces` tests. Never
claim a surface change is live from inside the session that authored it — the session is
structurally the one observer that cannot see it.

## Resume

Decide whether the doc fix belongs in `src/prompts/README.md` alone or also in a
commit-message convention note. Optionally distinguish host-composition from prompt-cache
by starting a fresh session against the *same* running binary: if the new session shows the
restored clause, composition-at-start is confirmed and caching is ruled out.

## References

- `src/prompts/README.md` — prompt-surface rules, `ONBOARDING_VERSION`, the character cap.
- `tests/fixtures/prompt_surfaces/server_instructions.md` — the compiled snapshot.
- `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md` — the sibling
  reconnect-semantics bug, about guide bodies rather than instructions; its fix made
  `guide_hints_emitted` persist across `/mcp` restarts within one conversation.

