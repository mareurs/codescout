---
id: 0e623bae045db6b5
kind: bug
status: fixed
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

### Two live conversations, two DIFFERENT frozen states

`397ca32b` confirmed this bug from a second session using a two-marker method, and reported
that session's instructions in **state 1**. Read against the same table, the session that
filed this bug is in **state 2** — a different one:

| marker | state 1 (pre-`391fdcdc`) | state 2 (post-`391fdcdc`) | state 3 (post-`d2cf4449`) | filing session |
|---|---|---|---|---|
| IL-1 overlap clause | present | absent | present | **absent** |
| `## Workspace gate` section | present | absent | absent | **absent** |
| quickref `docs/trackers` row | present | present | absent | **present** |

Three markers, one consistent reading: state 2, which is the surface compiled into
`3d7f13ce` — the binary that served this conversation's first connection, and the one
`usage.db` recorded for its earliest calls.

This is stronger than either observation alone, and it rules out one more alternative.
A host that re-read `initialize.instructions` late, or served them from any shared or
global cache, would have to show the **same** text in both conversations. It does not: two
sessions running against the same repo, at the same minute, hold instructions compiled from
two different builds — each one the build that was live when *that* conversation first
connected.

So the freeze is **per-conversation and at first connect**, not per-host and not
time-based. Which also means the staleness silently grows with conversation age: the longer
a session runs, the further its instructions drift from the binary answering its calls. A
long-lived session is the *most* likely to be reasoning from a surface nobody ships any
more, and nothing in its context marks the text as old.
### Independent confirmation, and a two-marker fingerprint that dates the live text

Added 2026-08-17 by the session that authored `d2cf4449` and `8d44ee08` — the two commits
this bug names. Confirmed, with a sharper instrument than "the text did not change".

A single marker only shows staleness. **Two markers that moved in opposite directions
pin which build the live instructions came from**, because the surface passed through three
distinguishable states in two days:

| State | `overlaps a symbol` | `## Workspace gate` |
|---|:---:|:---:|
| before `391fdcdc` (2026-08-16) | present | present |
| after `391fdcdc` (the 1900-char refit) | **absent** | **absent** |
| after `d2cf4449` (clause restored, section stays cut) | present | absent |

The shipped surface right now, from the fixture that CI checks:

```
$ grep -c 'overlaps a symbol' tests/fixtures/prompt_surfaces/server_instructions.md
1
$ grep -c 'Workspace gate' tests/fixtures/prompt_surfaces/server_instructions.md
0
```

So the surface is in the third state. The instructions **in this session's context** carry
the overlap clause *and* a full `## Workspace gate` section with its own heading and body —
which is the first state, from a build no later than 2026-08-16.

That session had by then rebuilt with `cargo rb` four times and reconnected `/mcp` four
times, including once immediately after `d2cf4449`. The live text moved not at all, and it
is not merely stale-by-one — it predates a refit that deleted an entire section.

**Why this is worth more than a second datapoint.** It rules out the most comfortable
alternative reading, that the reconnect refreshes instructions but the *specific edit* had
not landed in the binary yet. A section deleted the previous day is still present, so the
text cannot have been rebuilt from any of today's binaries. Whatever the host is doing, it
is not re-reading `initialize.instructions` on reconnect.

It also gives the next author a cheap self-check that needs no tooling: pick any **two**
surface markers whose presence differs between the last two states, and read them off the
live block. One marker cannot distinguish "not refreshed" from "not yet built"; two can.
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

Documentation, not code — the host behaviour is not ours to change. Fixed in
`src/prompts/README.md` § Versioning:

1. Corrected the section's own instance of the claim this bug falsifies
   ("each `/mcp` connect re-reads the sliced text... changes are live on next
   connect") — spelled out explicitly that a same-conversation `/mcp`
   reconnect does **not** rebuild the system prompt, only a genuinely new
   conversation's first connect does.
2. Added the corrected two-line split this bug proposed, verbatim:
   *"Tool code and schemas: `cargo rb` + `/mcp`. Prompt surfaces: not
   observable in this session; verified by fixture + `prompt_surfaces`
   tests, and eyeballable only in a NEW conversation."*
3. Tightened the summary table's `server_instructions` row from the bare
   "live on next connect" to name which "next connect" it means.

Left the two historical plan/spec docs that repeat the old phrasing
(`docs/superpowers/plans/2026-06-08-vdi-reliability-hardening.md`,
`docs/superpowers/specs/2026-05-03-librarian-progressive-disclosure.md`)
untouched — frozen design snapshots, not the canonical surface this bug
named.
## Tests added

None — nothing here is codescout behaviour to pin. The existing `prompt_surfaces` snapshot
tests already cover *what gets compiled*; this bug is about what a session *observes*, which
no test in this repo can reach.

## Workarounds

Verify prompt-surface edits against the fixture and the `prompt_surfaces` tests. Never
claim a surface change is live from inside the session that authored it — the session is
structurally the one observer that cannot see it.

Concretely, the check that *is* authoritative:

```
grep -c '<the new text>' tests/fixtures/prompt_surfaces/server_instructions.md
cargo test --lib prompt_surfaces
```

The fixture is regenerated with `UPDATE_PROMPT_SNAPSHOTS=1`, so it is what the build
ships; the live block in the authoring session is not evidence either way.

And a correction to the phrasing this bug flags, from the session that wrote it: the
`NOTE:` line on a mixed commit needs to split the two, because `cargo rb` + `/mcp` is the
right instruction for one half and misleading for the other —

> Tool code and schemas: `cargo rb` + `/mcp`.
> Prompt surfaces: not observable in this session; verified by fixture + `prompt_surfaces`
> tests, and eyeballable only in a NEW conversation.
## Resume

Fixed and closed. The doc fix landed in `src/prompts/README.md` only — no
separate commit-message convention note was added; the README's own
corrected text is the canonical statement CLAUDE.md already points readers
at. The optional composition-vs-cache experiment this Resume floated (start
a fresh session against the same running binary) was not run — the
two-marker fingerprint in § Evidence already pins the mechanism precisely
enough that the distinction doesn't change what an author does differently.
## References

- `src/prompts/README.md` — prompt-surface rules, `ONBOARDING_VERSION`, the character cap.
- `tests/fixtures/prompt_surfaces/server_instructions.md` — the compiled snapshot.
- `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md` — the sibling
  reconnect-semantics bug, about guide bodies rather than instructions; its fix made
  `guide_hints_emitted` persist across `/mcp` restarts within one conversation.
