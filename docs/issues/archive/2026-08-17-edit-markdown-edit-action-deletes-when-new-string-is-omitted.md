---
id: f05be046c6c373d8
kind: bug
status: fixed
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

**Correction (2026-08-17, at implementation time): there are THREE read sites, not
two.** The original entry below enumerated two, from a grep scoped to
`edit_markdown.rs`. The third lives in a different file and is a separate
implementation rather than a wrapper:

| Site | Path |
|---|---|
| single-edit | `src/tools/markdown/edit_markdown.rs:1283` |
| batch (`edits[]`) | `src/tools/markdown/edit_markdown.rs:600` |
| **`body_edits[]`** | **`src/librarian/tools/update.rs:224`** — `apply_body_edits` |

All three read `edit["new_string"].as_str().unwrap_or("")`. The third is the worst
of them: it is the `artifact(update, patch={body_edits: […]})` path, so it edits
trackers and bug files — the artifacts least likely to have their content asserted
by any test. A search scoped to the tool that *names* the parameter finds two of
three; the population was only visible from a repo-wide grep for the expression.

`""` is then a legitimate replacement all the way down `plan_scoped_edit`, so a
deletion is indistinguishable from an intended one. Deleting via `edit` IS a real
use case, which is why the default looks harmless in isolation.

What makes it a defect rather than a sharp edge is the **asymmetry** with the
sibling action. `edit_markdown.rs:98`:

```rust
.ok_or_else(|| anyhow::anyhow!("content is required for the 'replace' action \
  (it overwrites the whole section body); for a scoped text swap pass \
  action='edit' with old_string + new_string"))?;
```

and `update.rs:218-221` carries the same guard for its own path. So `replace`
without `content` is refused *with a pointer to `edit`'s shape*, and `old_string`
missing is refused too. The authors clearly considered the pairing at every site.
Only `edit` without `new_string` fell through, at all three, into the destructive
branch.

`content` was not rejected either: it is a declared key used by `replace` /
`insert_*`, so passing it to `edit` was schema-valid and silently ignored. The call
was therefore well-formed by every check the tool applied.

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

Implemented — both directions from the original plan, plus the third site.

One shared helper, `edit_markdown::require_new_string(edit, prefix)`
(`src/tools/markdown/edit_markdown.rs`), now serves all three call sites:

- **Requires the key to be present**, rather than defaulting to `""`. Presence, not
  emptiness, is the test — so `new_string: ""` still deletes, and deliberate
  deletion stays reachable. That explicit empty string is exactly the distinction
  the old default could not make.
- **Names the wrong key when it sees it.** If `content` is present instead, the
  refusal says so and tells the caller to rename it, rather than emitting a generic
  missing-parameter error that invites re-sending `content`.
- **Refuses `new_string` and `content` together.** Both present means the caller is
  describing two different actions at once; silently picking one is how the original
  defect stayed invisible.
- `prefix` locates the entry per path — `""`, `"edits[3]: "`, `"body_edits[0]: "` —
  so the batch paths keep their own addressing rather than borrowing each other's.

Modelled on `subsection_guard_error`'s shape (`RecoverableError::with_hint` plus an
optional batch index), so it reads like the guards already in that file.

Deliberately **not** done: the envelope change. `edit_markdown` still returns
`{"status": "ok"}` with no `replacements` count or byte delta, and every one of the
five bad calls would also have been caught by that. It is a wider fix covering the
whole silent-no-op/over-op class rather than this key, so it belongs in its own
change — see § Resume.

Fix SHA: this commit, on `experiments`. `master` is a strict ancestor at fix time,
so the promotion path is fast-forward and this SHA is already the master SHA.
## Tests added

Seven, and the split across files is the point: three independent read sites need
three independent guards, or a revert at one site passes on the strength of the
others' coverage.

`src/tools/markdown/tests.rs`:

| Test | Mutation it catches |
|---|---|
| `edit_action_with_content_instead_of_new_string_is_refused_and_changes_nothing` | the single-edit site reverting to `unwrap_or("")` |
| `edit_action_without_any_replacement_key_is_refused` | narrowing the guard to only the wrong-key case |
| `edit_action_with_explicit_empty_new_string_still_deletes` | over-tightening into requiring a non-empty replacement, which would remove a real capability |
| `edit_action_rejects_both_new_string_and_content` | re-admitting the ambiguous pair |
| `batch_edit_action_requires_new_string` | the `edits[]` site specifically — a second read site with its own copy of the default |

`src/librarian/tools/update.rs`:

| Test | Mutation it catches |
|---|---|
| `body_edits_edit_action_with_content_is_refused_and_changes_nothing` | the `apply_body_edits` site, and that its error uses `body_edits[0]` rather than `edits[0]` |
| `body_edits_edit_action_with_explicit_empty_new_string_still_deletes` | deliberate deletion through the artifact path |

The load-bearing assertion in the file-based tests is not that the call is refused —
it is that **the file is byte-identical afterwards**. Refusing is the mechanism;
leaving the text alone is the property that was violated.

**Mutation-verified.** Reverting `update.rs`'s call site alone to
`edit["new_string"].as_str().unwrap_or("")` turns
`body_edits_edit_action_with_content_is_refused_and_changes_nothing` red, and the
failure prints the defect verbatim: the call **succeeded** and returned
`"## Foo\n"` — the sentence gone. Its sibling deliberate-deletion test stayed green
through the mutation, confirming the two assert different things.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test` 4017 passed / 0 failed / 45 ignored.
## Workarounds

Use `new_string` for `action="edit"` and `content` for `action="replace"` /
`insert_before` / `insert_after`. Since the success envelope reports nothing
about the change, **read back any section edited via `action="edit"`** —
`read_markdown(path, heading=…)` — rather than trusting `{"status": "ok"}`.

## Resume

N/A for this defect — all three sites are guarded, mutation-verified, and confirmed
on the wire after `cargo rb` + `/mcp`. The original failing call, replayed verbatim
against a scratch file:

```
edit_markdown(action="edit", heading="## Section A",
              old_string="keep this sentence", content="replaced sentence")
→ ok: false
  error: new_string is required for action="edit"
  hint:  Rename content to new_string — 'edit' performs a scoped old_string ->
         new_string swap and never reads content (that key belongs to 'replace' /
         'insert_before' / 'insert_after'). To DELETE the matched text, pass
         new_string="" explicitly.
```

and a read-back confirmed the file still contains `keep this sentence` — the
property that was actually violated, not merely the refusal.

One deliberate follow-up, worth its own bug file rather than reopening this one:
**`edit_markdown` reports nothing about what it changed.** The success envelope is
`{"status": "ok"}` plus a hint about unread sections. `artifact(update)` already
emits `prev_bytes` / `new_bytes` on its `field_patch` event for exactly this reason.
A `replacements: N` count or a byte delta would have caught all five bad calls in
this incident independently of the key check, and would cover the wider class —
edits that matched nothing, or matched more than intended — which no guard on
parameter names can reach.
## References

- `src/tools/markdown/edit_markdown.rs:1283` — single-edit read site
- `src/tools/markdown/edit_markdown.rs:600` — batch read site
- `src/tools/markdown/edit_markdown.rs:98` — the guard that exists for the mirror case
- `179c48a7`, `a8fdf055`, `9cdb2f50` — the commits carrying and repairing the damage
- `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md` — the bug being fixed when this one surfaced
