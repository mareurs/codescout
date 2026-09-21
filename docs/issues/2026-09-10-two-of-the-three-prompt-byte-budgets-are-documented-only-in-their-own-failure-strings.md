---
id: ea152af988811fa1
kind: bug
status: fixed
title: 'BUG: two of the three prompt-surface byte budgets are documented only in the failure strings of the tests that enforce them'
owners:
- marius
tags:
- cluster/unclassified
- prompts
- docs
---

## Summary

Three tests bound how many bytes a prompt/guide edit may add:

| gate | bound on | where it lives |
|---|---|---|
| `prompts::guide_index::tests::declared_sections_are_within_the_size_cap` | 2500 B per declared guide section | `src/prompts/guide_index.rs` |
| `server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling` | bytes a p50 session emits | `src/server.rs` |
| `server::tests::tool_surface_under_budget` | `TOOL_SURFACE_CHAR_BUDGET` over all advertised tools | `src/server.rs` |

**`src/prompts/README.md` — the page whose entire job is telling an author what the prompt surfaces
cost — documents only the third.** Verified 2026-09-10: it has a whole § *The tool-surface budget*
with the right instruction (*"do not raise it — find the bytes"*, ratchet down), and `grep -E
'p50|emission|guide_hint|declared_sections|size_cap|2500'` over it returns **0**.

So an author adding a paragraph to a guide file gets no warning from the surface that exists to
warn them, and the guide files themselves carry none either.

## Symptom (Effect)

A documentation edit reds the build in three places at once, in tests whose names name neither the
file edited nor the guide system, and the author's diff cannot contain the fact that explains it.

That is `observer-blindness:OB-25` — *"docs-only" is a category imported from other repos, and the
diff cannot carry the fact that refutes it* — and `OB-1` position 3: a bound published only in the
failure string of the test that enforces it is published to an audience that reads it **after**
tripping over it.

## Reproduction

2026-09-10, while adding ~8 lines to `src/prompts/guides/librarian.md` § *Archiving / Moving
Trackers* and ~45 characters to `artifact.rs`'s `new_rel_path` schema, to document two new
`doc(move)` response fields:

- `declared_sections_are_within_the_size_cap` — *"librarian § Archiving / Moving Trackers is 2727 B,
  over the 2500 B cap"*
- `a_p50_session_stays_under_the_committed_emission_byte_ceiling` — *"emitted 12913 B … ceiling
  12244 B, margin 0 B"*
- `tool_surface_under_budget` — *"56812 chars across 21 tools; budget is 56492"*, i.e. **47 over
  from a 47-character edit**

All three were repaired by making the edits net-neutral. No ceiling was raised.

## Environment

`experiments`, 2026-09-10, at `38f7490e`.

## Root cause

The bounds live in Rust — a const, two test modules — and the failure strings are excellent
(each names its remedy, and the p50 one names a prior misdiagnosis to rule out first). What is
missing is any pointer on the **read** surface an author of a `.md` guide actually opens.

`src/prompts/README.md` covers the tool surface because that budget was *designed* and written up
(`docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`). The two guide budgets arrived
with the guide-section feature and their documentation stayed inside the enforcement layer.

## Evidence

- `src/prompts/README.md` § *The tool-surface budget* — the third gate, documented well.
- Same file, zero matches for `p50`, `emission`, `guide_hint`, `declared_sections`, `size_cap`,
  `2500`.
- Three live failure strings, quoted above.
- `38f7490e`'s commit message records the incident but a commit message is not a read surface.

**Deliberately NOT recorded: the current headroom.** It was effectively zero on two of the three at
the moment of measurement, and that is exactly the kind of number `CLAUDE.md` says to derive rather
than cite — it moves on every prompt-surface commit, and publishing it would go stale the same day.
The durable claim is *"assume no slack; make edits net-neutral or measure"*, not a byte count.

## Hypotheses tried

- **Document it in the guides themselves?** Rejected, and the reason is the bug: a guide is the
  thing under the cap, so the warning would consume the budget it warns about, and it would be
  emitted to every session at runtime cost rather than read once by an editor.
- **Raise the ceilings so ordinary edits fit?** Rejected. `TOOL_SURFACE_CHAR_BUDGET`'s own failure
  message allows a raise when the bytes are owed, and that allowance is not for buying room the
  author could have found — in the incident above the whole 47 characters were recoverable by
  rewording.
- **`cluster/gate-keyed-on-unobservable-event` (`IC-2`)?** Rejected. The gates observe exactly the
  right thing and fail loudly and correctly; only their *bound* is unpublished.

## Fix

Not applied. Add a short subsection to `src/prompts/README.md` — beside § *The tool-surface budget*,
which is the precedent and the right neighbour — naming the two guide gates, where their bounds
live, and the standing rule: **assume no slack, make guide edits net-neutral, and never raise a
ceiling to fit prose.**

**Include the two-state diagnostic**, because it is what separates the two causes cheaply and it is
not obvious: when the p50 ceiling reds, revert the guide file to `HEAD`'s bytes and re-run. Green
means the bytes are yours; still red means the instrument (its own message names
`docs/issues/2026-09-08-the-emission-byte-ceiling-measures-the-fixtures-tempdir-path-length.md`).
One test run, and it settles by observation what reading the normalisation code would only produce
an opinion about.

`src/prompts/README.md` is a reader doc, not an `include_str!`'d payload — verified — so adding to
it costs no budget. It is gated by `prompts::tests::reader_docs_contain_no_retired_call_forms` and
by `audit_doc_refs`, so cited paths and tool names must resolve.

**Correction, 2026-09-20 — "Not applied" was stale by nine days, and this section is where a reader checks.** The prescribed subsection shipped as `## Three byte budgets, and two of the margins are invisible until you exceed them` (`src/prompts/README.md:66`), immediately after `## The tool-surface budget` (`:52`) — the exact neighbour named above. It landed in `faf1dc99` (2026-09-10) and was corrected in place by `cd8917ca` (2026-09-11), which withdrew a "zero headroom" figure that was really 262 characters and a "every failing test prints its margin" claim false for two of the three. Both are ancestors of HEAD, verified with `git merge-base --is-ancestor`.

**What was genuinely left undone was the test**, and only that. It is now `063b4722`.

The generalisable half, because this campaign had just adopted the opposite rule: a `## Fix` section holds **two kinds of claim with one heading over them**. Its *prescription* — do this, not that; this neighbour, this tone — is a **decision**, and decisions do not decay. Its *status* — "Not applied", "Not implemented", "Not yet fixed" — is a **claim about the tree**, and decays exactly like any other. This campaign's round-2 lesson was "build the brief from the bug file's own `## Fix`, not from a triage sketch", and that lesson is right about the prescription and wrong about the status: here the frontmatter `status:` and the prose both said not-fixed, and the tree disagreed with both. **Read the section for its ruling; verify its state against the code.**

## Fix provenance

Three commits, because the prose and the gate landed separately and the prose was corrected once:

- **SHA:** `faf1dc99` — the subsection itself (2026-09-10).
- **patch-id:** `6a37fdd355cf9dbc9dcd1b68dbefd9b516608d72`
- **SHA:** `cd8917ca` — retracts two false claims inside it (2026-09-11).
- **patch-id:** `f2c3a685409c9a98378c1287813dd1dccf882529`
- **SHA:** `063b4722` — the name-to-documentation gate (2026-09-20).
- **patch-id:** `fe452b01a8123a6c8e4239ac44eacaeaaafa5a36`

SHAs are positional and do not survive a rebase of `experiments`; the patch-ids are content hashes of each diff and survive rebase and cherry-pick. All three derived through a file, never a pipe from `git show` — the command buffer is capped and a hash of a truncated prefix is a valid-looking WRONG digest.

## Tests added

None — nothing is fixed. The honest note is that this class is hard to gate: a test asserting
"README mentions the p50 ceiling" is monotone under the ceiling being renamed, and pinning prose
reds on every rewording. The cheap shape, if one is wanted, is asserting that each **budget
constant or budget-enforcing test name** appears somewhere under `src/prompts/README.md` — a
name-to-documentation check rather than a prose pin, which reds exactly when a fourth budget is
added and not documented.

**Added 2026-09-20:** `prompts::tests::byte_budget_gates_are_named_in_the_prompts_readme` (`src/prompts/mod.rs`) — the name-to-documentation shape this section specified, asserting each known budget's constant or enforcing-test name appears somewhere in `src/prompts/README.md`. It survives a rewording and reds on the regression that actually happened.

**Red corroborated two ways, the second without a build.** The implementing agent observed it in an isolated `git worktree` with the README swapped to its pre-`faf1dc99` bytes: FAILED, naming `MAX_DECLARED_SECTION_BYTES` and `a_p50_session_stays_under_the_committed_emission_byte_ceiling`. That was then re-derived here straight from git — token counts in `src/prompts/README.md` at `faf1dc99^` versus HEAD are **0 → 1**, **0 → 1**, and **1 → 2**. The third line is the **control**: `TOOL_SURFACE_CHAR_BUDGET` was documented both before and after, so it is never in the missing list, which is what shows the test discriminates *documented from undocumented* rather than merely *README changed*.

**Its ceiling, recorded here so nobody credits it with the coverage this file asked for.** The section above asked for a check that "reds exactly when a fourth budget is added and not documented". This test does **not** do that: `BUDGET_GATES` is a hand-maintained list, so a fourth gate added without a matching entry is invisible to it. The test's own doc comment says so rather than implying completeness. The reason it cannot be derived today is concrete — two of the three bounds (`CEILING`, `TOOL_SURFACE_CHAR_BUDGET`) are constants **local to their test functions**, and one of them is named `CEILING`, carrying no marker a scan could recognise. There is no attribute or naming convention to enumerate.

**And this repo has already measured what a hand-maintained list costs.** `src/config/embedding_env.rs`'s `all_names()` carries the prediction in its own doc comment — *"A list that must be kept in step by hand is the same shape as the eight scattered `env::var` calls this module exists to replace"* — and three tests in `tests/retrieval_unit.rs` went red on exactly that, fixed 2026-09-19 at `e635d4da` by deriving the population instead of retyping it. So the durable remedy here is the same move one layer up: give budget gates a **marker** (a registry, or a scannable attribute) so the population becomes derivable, and let this test enumerate rather than recite. That is a change to `src/server.rs` and `src/prompts/guide_index.rs`, outside this bug, and is left as the named follow-up rather than done as a drive-by — `src/server.rs` was concurrently held dirty by another session throughout this work.

## Workarounds

Run the four-command gate on documentation changes. That is already the rule; this file exists
because the rule is the only thing standing between an author and three surprising reds.

## Resume

Write the subsection. Keep it short — the neighbouring § *The tool-surface budget* is the model for
tone and length.

## References

- `docs/trackers/observer-blindness.md` — `OB-25` (docs-only is an imported category), `OB-1`
  position 3 (a bound published to an audience that never reads it)
- `src/prompts/README.md` § *The tool-surface budget* — the precedent, and the neighbour to write beside
- `docs/issues/2026-09-08-the-emission-byte-ceiling-measures-the-fixtures-tempdir-path-length.md` —
  the alternative cause the p50 failure asks you to rule out first
- `38f7490e` — the commit whose gate run produced all three
