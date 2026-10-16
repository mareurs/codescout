---
id: '604bfbcecf4602d5'
kind: bug
status: fixed
title: 'RESIDUAL: Build a check that flags a **Mechanism status:** worklist field contradicted by a later commit'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md
severity: low
unverified: 'STANDING — a falsifier is a tripwire on a name its author guessed: a resource with no declared falsifier stays invisible, and a mechanism built under a name the glob did not anticipate does not flip it. 23 of 24 fields are not migrated, by decision.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Build a check that flags a **Mechanism status:** worklist field contradicted by a later commit.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md` (status `fixed`):

> No regression test, and § *Tests added* records that as a deliberate finding rather than an omission: a test would have to assert that no `Mechanism status` field in the corpus contradicts a later commit, which is the unbuilt mechanism itself and not a test of this fix. Recurrence of the class is therefore unguarded — the corrected field is pinned, the mechanism that would keep it correct is not.

## Fix

**Fixed 2026-10-16, option A (declared falsifier rows), decided by the user after the design was put to them with evidence.**

Why a design decision was needed: the parent's § Resume left open whether `Mechanism status` should carry declared resource rows, and the originating instance defeats the cheaper options. The stale field cited no path for its absence claim, and the falsifying commit `99d5acac` touched only `scripts/` and hook config, so a check keyed on cited paths or the field's date (option B) cannot see it, and the basis gate cannot either, because the field had a basis.

What shipped:

- **Grammar, defined once** in `docs/trackers/issue-clusters.md` § *Falsifier rows*: a single-backtick span reading exactly `falsifier: <glob> expect=absent|present` inside a `**Mechanism status:**` field (that line through the line before the next `**Label:**` field or heading, fences skipped). The glob is repo-relative, matched against the git index; `*` and `?` stay inside a segment, a whole `**` segment spans segments.
- **Rust** (`tests/issue_clusters.rs`): `mechanism_falsifier_scan`, `glob_match`, `falsifier_problems`; hook-owed test `no_mechanism_status_falsifier_is_violated`.
- **Python twin** (`scripts/pre-commit-ledger-counts.py`): the same functions, CHECK 8, and `HOOK_RULES` entry, so the commit that adds the file a field said did not exist is the commit refused. `the_hook_script_agrees_on_the_mechanism_falsifier_scan` compares the full problem strings across the two on one fixture.
- **`IC-17` migrated** as the first field: five falsifiers (three `present` on the owning scripts, two `absent` on `scripts/*unstaged*` and `scripts/*high*water*`). The other 23 fields are not migrated, by decision.
- **Parsers-over-a-namespace**: escape (a fenced block, or a span holding only the bare word, is a mention) and disambiguator (every finding is keyed `IC-N` and quotes the span). Malformed is loud, never skipped: wrong case or spacing, a bad `expect`, `[ ] { } \\` in the glob, a `..` segment, a leading `/`, `**` fused to other characters, and a bare `expect=` outside a span. A glob anchored under a directory with no tracked file is refused as dead, because `expect=absent` over an empty population holds whatever the tree contains.

**Limit, standing and not fixable by any action:** a falsifier is a tripwire on a name its author guessed. A resource nobody declared one for stays invisible, which is how the original field went stale, and a mechanism built under a name the glob did not anticipate does not flip it.

## References

- `docs/issues/archive/2026-09-02-a-worklist-field-announcing-an-absence-outlives-the-mechanism.md` — parent
- `docs/trackers/issue-clusters.md` § *Falsifier rows* — the grammar
- `tests/issue_clusters.rs`, `scripts/pre-commit-ledger-counts.py` — the two implementations

## Tests added

- `a_falsifier_would_have_flipped_on_the_original_stale_field` — fixture trees cut from the `0dea2246` and `99d5acac` `scripts/` listings; the falsifier holds on the first and flips on the second naming `scripts/post-index-change-stage-log.sh`, and the basis gate is asserted blind to the original field.
- `the_mechanism_falsifier_scan_discriminates` — glob semantics, and a fixture with known answers: 3 holding declarations, 1 violated, 1 dead, 7 malformed shapes, and the escapes (fence, mention, text after the field, text after a heading) that must declare nothing.
- `the_hook_script_agrees_on_the_mechanism_falsifier_scan` — cross-language parity.
- `no_mechanism_status_falsifier_is_violated` — the live gate, with a non-vacuity assertion that `IC-17` declares one.

Red first: the four tests failed against stubs; after implementing, the live gate stayed red until `IC-17` declared its falsifiers. Mutation: flipping one `IC-17` falsifier to the wrong expectation reds the live test and makes the Python hook exit 1 naming `IC-17`; restored, both are green.
