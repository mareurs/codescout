---
id: a52dc618df71d995
kind: bug
status: open
title: 'BUG: edit_markdown action="edit" defaults a missing new_string to empty, so a content/new_string mix-up silently DELETES the matched text — 5 bad edits across 4 files in one session, 3 of them committed'
---

## Summary

`edit_markdown(action="edit", ...)` reads its replacement as
`input["new_string"].as_str().unwrap_or("")`. Omit `new_string` and the tool
deletes every match of `old_string` and reports `{"status": "ok"}`. The natural
way to omit it is to pass `content` instead — the correct key for `action="replace"`,
and a valid key in the same schema — so the mistake is a plausible one-word slip
with destructive, silent consequences.

The mirror-image mistake is guarded: `replace` without `content` errors and even
names this action's shape. Only this direction is silent.

## Symptom (Effect)

Five calls of this shape, all returning success, each deleting the matched text
and inserting nothing:

```
edit_markdown(path="CLAUDE.md", action="edit", heading="## Prompt Surface Consistency",
              old_string="the 2200-byte slice cap + …", content="the 1900-character …")
→ {"status": "ok", "hint": "14 unread sections in this file: …"}
```

Result on disk — the sentence simply ends:

```
Listing tools (`artifact(find)`, `librarian(context)`) default to the
active project and hide archived/superseded rows. Responses include a


**Umbrellas are user-declared** in `workspace.toml` …
```

Nothing in the response distinguishes this from a successful replacement. There
is no byte count, no `replaced` echo, no diff.

## Reproduction

`git rev-parse HEAD` → `66487591`, branch `experiments`.

1. Pick any markdown file with a section heading.
2. `edit_markdown(path=…, action="edit", heading="## Some Section", old_string="<exact text>", content="<replacement>")`
3. Response is `{"status": "ok"}`.
4. Read the section — `<exact text>` is gone and `<replacement>` was never written.

Passing `new_string` instead of `content` behaves correctly, which is the whole
difference.

## Environment

Linux, codescout `experiments` @ `66487591`, stdio MCP. Applies to both the
single-edit path and the `edits[]` batch path.

## Root cause

Two sites default the replacement to the empty string instead of requiring it:

- `src/tools/markdown/edit_markdown.rs:1283` (single-edit path) —
  `let new_string = input["new_string"].as_str().unwrap_or("");`
- `src/tools/markdown/edit_markdown.rs:600` (batch path) —
  `let new_string = edit["new_string"].as_str().unwrap_or("");`

`""` is then a legitimate replacement all the way down `plan_scoped_edit`, so a
deletion is indistinguishable from an intended one. Deleting via `edit` IS a
real use case, which is why the default looks harmless in isolation.

What makes it a defect rather than a sharp edge is the **asymmetry** with the
sibling action, at `edit_markdown.rs:98`:

```rust
.ok_or_else(|| anyhow::anyhow!("content is required for the 'replace' action \
  (it overwrites the whole section body); for a scoped text swap pass \
  action='edit' with old_string + new_string"))?;
```

So `replace` without `content` is refused *with a pointer to `edit`'s shape*, and
`old_string` missing is refused too (verified — the error is
`missing 'old_string' parameter`). The authors clearly considered the pairing.
Only `edit` without `new_string` falls through, and it falls through into the
destructive branch.

`content` is not rejected either: it is a declared key used by `replace` /
`insert_*`, so passing it to `edit` is schema-valid and silently ignored. The
call is therefore well-formed by every check the tool applies.

Measured 2026-08-17: 5 calls, 4 files, 3 committed before detection (see Evidence).

## Evidence

### The blast radius in one session

All five calls used `old_string` + `content`. All returned `ok`.

| File | Lost | Reached |
|---|---|---|
| `CLAUDE.md` | prompt-cap sentence | commit `a8fdf055` |
| `.codescout/memories/conventions.md` | prompt-cap line | commit `a8fdf055` |
| `src/prompts/guides/librarian-runtime.md` | the `hints`-block description | commit `9cdb2f50` |
| `src/librarian/prompts/companion_hint.md` | example JSON + the whole scope bullet list (×2 calls) | commit `9cdb2f50` |
| `src/prompts/source.md` | Iron Law 1's overlap clause | caught before commit |

Two of those files are **prompt surfaces shipped to every session**, and one is
the repo's always-loaded `CLAUDE.md`.

### Nothing downstream caught it

- **No test.** `librarian-runtime.md` and `companion_hint.md` have no
  content assertions — only registration (`GUIDE_TOPICS`, `topic_body`).
- **The body-shrink guard did not apply.** It refuses writes that cut a file by
  >50%; these removed 60–500 bytes from multi-KB files.
- **`cargo fmt` / `clippy` / `cargo test` were all green** across both commits.
- **Detection was incidental.** `source.md`'s loss was noticed only because the
  very next action measured the slice's character count and it had gone *down*
  by 61 when it should have gone *up* by 57. Without a numeric expectation to
  contradict, this would have shipped too.

### A peer session hit the same wall

`179c48a7` — *"docs(claude-md): finish `a8fdf055`'s cap correction — repair the
sentence, fix the sibling"* — is another session repairing `CLAUDE.md` after the
first of these calls. Its subject reads as a follow-up improvement, not as
damage control, which is how a silent deletion enters the record as ordinary
churn. It repaired `CLAUDE.md` only; `conventions.md` stayed broken.

## Hypotheses tried

1. **Hypothesis:** `content` is honored by `action="edit"` and something else
   removed the text.
   **Test:** re-ran the identical call with `new_string` instead of `content`.
   **Verdict:** rejected — the replacement lands correctly with `new_string`, so
   `content` is being ignored.
   **Evidence:** all four repairs in commit *(this commit)*.

2. **Hypothesis:** the tool errors on the missing param and the `ok` came from a
   different call.
   **Test:** read both read sites.
   **Verdict:** rejected — `.unwrap_or("")` at `edit_markdown.rs:1283` and `:600`
   makes the param optional by construction.

## Fix

Not implemented. The fix is small and the choice is about which of two rules to
apply:

1. **Require `new_string` for `action="edit"`.** Mirrors `edit_markdown.rs:98`
   exactly, including the cross-pointer in the message. Deleting text then needs
   an explicit `new_string=""`, which is the honest way to ask for a deletion.
   This is the recommended direction: it makes the destructive path opt-in and
   costs one line at each of the two sites.
2. **Reject keys the action does not use.** Passing `content` to `action="edit"`
   (or `new_string` to `replace`) is always a mistake about which action is
   running. Rejecting it catches this class rather than this instance, and would
   also have caught it in the `edits[]` batch path.

They compose, and doing both is still small. Direction 1 alone leaves
`content`-to-`edit` silently ignored; direction 2 alone permits a bare
`action="edit"` + `old_string` deletion with no replacement key at all.

Independently: **the response should report what changed.** Every one of these
five calls would have been caught immediately by a `bytes_before`/`bytes_after`
or `replacements: N` field in the `ok` envelope. `artifact(update)` already
emits `prev_bytes`/`new_bytes` on its `field_patch` event for exactly this
reason; `edit_markdown` returns `{"status": "ok"}` and a hint about unread
sections.

## Tests added

None yet. Three are needed, and the third is the one that matters:

| Test | Mutation it catches |
|---|---|
| `edit_action_requires_new_string` | restoring `.unwrap_or("")` at the single-edit site |
| `batch_edit_action_requires_new_string` | the same at `plan_batch` — the batch path is a separate read site and would otherwise keep the hole |
| `edit_action_rejects_content_key` | re-admitting the wrong-action key that made the slip silent |

A test asserting `new_string=""` still deletes should accompany them, so the
deliberate-deletion use case is pinned as intentional rather than left to be
re-discovered as a regression.

## Workarounds

Use `new_string` for `action="edit"` and `content` for `action="replace"` /
`insert_before` / `insert_after`. Since the success envelope reports nothing
about the change, **read back any section edited via `action="edit"`** —
`read_markdown(path, heading=…)` — rather than trusting `{"status": "ok"}`.

## Resume

Implement direction 1 + 2 at `src/tools/markdown/edit_markdown.rs:1283` and
`:600`, modelling the error text on the existing `:98` message so both
directions of the mix-up point at the other action. Add the four tests under
**Tests added**. Then consider the envelope change (`replacements` count or
byte delta) as a separate, wider fix — it covers the whole class of silent
no-op/over-op edits, not just this key.

## References

- `src/tools/markdown/edit_markdown.rs:1283` — single-edit read site
- `src/tools/markdown/edit_markdown.rs:600` — batch read site
- `src/tools/markdown/edit_markdown.rs:98` — the guard that exists for the mirror case
- `179c48a7`, `a8fdf055`, `9cdb2f50` — the commits carrying and repairing the damage
- `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md` — the bug being fixed when this one surfaced

