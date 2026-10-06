---
id: '9278fa8a4e651b43'
kind: bug
status: fixed
title: 'RESIDUAL: Make grep''s hidden-paths completeness warning check that hidden pruning could explain the zero before asserting its remedy'
tags:
- cluster/unclassified
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make grep's hidden-paths completeness warning check that hidden pruning could explain the zero before asserting its remedy.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md` (status `fixed`):

> the second defect named in the title is NOT fixed in general: the hidden-paths completeness warning still asserts its remedy without checking that hidden pruning could explain the zero. The reported misattribution can no longer occur, because the glob case now errors before any walk runs, but the narrowing candidate in Fix remains unimplemented.

## Fix

`WalkAudit::completeness_warning` in `src/tools/grep.rs` now gates the hidden-paths clause on a new `WalkAudit::globs_can_reach_a_root_dot_entry(globs)`, in addition to the existing `include_hidden` check. That predicate is false only when every admitting glob is relative, contains a slash (a trailing slash does not count) and has a first segment that opens with a literal non-dot character (`docs/issues/*.md`, `src/**/*.rs`). It stays true, and the clause stays, for floating globs (no slash, such as `*.mjs`), first segments opening with `.`, `*`, `?`, `[`, `{` or `\`, absolute globs, negation-only lists and an empty list; this is a deliberate over-approximation. Only globs anchored under a visible directory drop the clause. The starved-zero message text is unchanged apart from source indentation.

**Scope, stated plainly:** the clause is dropped for any zero whose admitting globs are all anchored under a visible directory, not only for glob-starved ones. A pattern that sits under a deeper hidden path (for example `src/.gen/` under `src/**/*.rs`) therefore no longer gets the `include_hidden` remedy when the glob is anchored. The clause never named such paths (it lists only dot entries at the walk root), so this removes a cause that was asserted without being checked, not one that was ruled out.

## Premises of the filing that did not hold

Source: the fixing agent's triage on 2026-10-05; not re-derived while closing this file.

- "A starved glob cannot reach a dot-entry" is false for floating globs such as `*.mjs`, which match at any depth including under a dot directory. Only anchored globs can be ruled out, which is why the predicate keys on the first path segment rather than on starvation.
- The filing's second candidate narrowing (that `path` cannot lie under hidden entries) is vacuous and was not implemented.

Measurement, from the fixing commit's test doc comment (measured 2026-10-05 on this project's `usage.db`; not re-run here): 37 of the 242 grep zeros that carried the hidden clause were glob-starved, and 23 of those 37 used only globs anchored under a visible first segment (`docs/issues/*.md`, `src/**/*.rs`, `tests/cap_probe.rs`). The other 14 were dot-anchored (`.github/workflows/*.yml`), floating (`*.mjs`) or a buffer ref, and keep the clause. The fixing agent also reported 7,584 grep calls in all, of which 242 carried the hidden clause and 53 zeros carried "passed the glob filter" (37 of them also carried the hidden clause).

## Tests added

All in the test module of `src/tools/grep.rs`, over a fixture whose root holds `src/a.rs` and a `.github/` directory (`hidden_dir_holds_the_pattern`, `zero_warning_under_glob`):

- `a_glob_anchored_under_a_visible_dir_does_not_name_the_hidden_remedy`: a starved glob (`src/nope.rs`), an admitting glob that finds nothing (`src/*.rs`) and an array form (`["src/nope.rs", "src/b/*.rs"]`) all return a warning without `include_hidden` or `.github/`; the starved case first asserts it still says "passed the glob filter".
- `a_glob_that_can_reach_a_hidden_root_entry_still_names_the_remedy`: the positive twin. `.github/*.yml`, `*.yml`, `**/ci.yml`, an array with one reaching glob among anchored ones, and an absolute glob inside the root each still name `include_hidden` and `.github/`. A narrowing that fired on every glob would pass the test above and fail this one.
- `a_negation_only_glob_keeps_the_hidden_remedy`: `!src/skip.rs` admits nothing and narrows nothing, so the remedy stays.

No mutation run is recorded in the commit message.

## Fix provenance

- **SHA:** `3548d3fd` (`experiments`)
- **patch-id:** `46fe142bfb3a832c33a0819148599654e7bd2c6b`

## Resume

Closed on 2026-10-06. Residual follow-ups, listed and not filed:

- Globs under a dot directory such as `.codescout/...` still make the warning list unrelated root dot entries (for example `.github/`), because any dot-led first segment counts as reaching.
- Two recorded calls passed `@file_*` buffer refs as `glob`. That is a separate misuse and was not investigated.
- Over-approximated shapes (wildcard- or brace-led first segments, absolute globs) still name the clause even when they could not reach a dot entry; tightening them was not attempted.

## References

- `docs/issues/archive/2026-08-18-grep-absolute-glob-outside-project-returns-silent-zero.md` — parent
