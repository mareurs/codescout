---
id: ea152af988811fa1
kind: bug
status: open
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

## Tests added

None — nothing is fixed. The honest note is that this class is hard to gate: a test asserting
"README mentions the p50 ceiling" is monotone under the ceiling being renamed, and pinning prose
reds on every rewording. The cheap shape, if one is wanted, is asserting that each **budget
constant or budget-enforcing test name** appears somewhere under `src/prompts/README.md` — a
name-to-documentation check rather than a prose pin, which reds exactly when a fourth budget is
added and not documented.

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
