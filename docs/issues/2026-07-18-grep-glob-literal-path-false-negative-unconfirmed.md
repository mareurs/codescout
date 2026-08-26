---
kind: bug
status: investigating
title: 'BUG: `grep`''s `glob` param reported to miss real matches on literal (non-wildcard) file paths — not reproducible on immediate re-test'
tags:
- grep
- glob
- override
- false-negative
- recurring
last_observed: 2026-07-18
opened: 2026-07-18
owner: marius
related: []
reopened: 2026-07-18
severity: medium
---

# BUG: `grep`'s `glob` param reported to miss real matches on literal (non-wildcard) file paths — not reproducible on immediate re-test

## Summary

A Mercury BOM project session reported that `grep`'s `glob` param silently
returned 0 matches (or matched the wrong file) when given a literal,
wildcard-free file path, while the equivalent `path` param on the same file
and pattern found the matches correctly. If real, this is a **silent false
negative in a core search tool** — worse than a crash, since a caller can
conclude "this file doesn't contain X" when it does.

This doc files the report faithfully, but flags a critical caveat up front:
**re-running the exact same calls against the same target repo in this
session did NOT reproduce the reported zero-match / wrong-file symptoms.**
`glob` correctly found matches for a literal single-file path and a
literal two-file array in re-test. Filed as `zombie` (observed once, not
currently reproducible, root cause unconfirmed) rather than `open`, per
`get_guide("tracker-conventions")`. A genuine test-coverage gap was
confirmed regardless (see Evidence/Fix) — existing `glob` tests only cover
wildcard extension patterns (`"*.rs"`), never a literal non-wildcard path,
singular or array.

## Symptom (Effect)

As reported by the originating session (Mercury BOM project,
`c:\Users\MAILINCA.BRN.002\work\Mercury BOM`):

```
grep(glob="app/graph_api.py", pattern="def explode_by_family|family_label|engine_families", context_lines=15)
→ 0 matches
```
immediately followed by:
```
grep(path="app/graph_api.py", pattern="family", ignore_case=true)
→ 8 matches, including lines that directly satisfy the first query
   (598: def explode_by_family(...), 599: docstring w/ family_label, 603: family_label call site)
```

And separately:
```
grep(glob=["scripts/bom_graph.py", "scripts/bom_analytics.py"], ignore_case=true,
     pattern="engine.planning.famil|family_to_sku|expand_family|BOM-0013|family_skus")
→ 1 match, reported inside scripts/bom_config.py — a file NOT in the glob array
```
with a direct file read confirming `scripts/bom_graph.py` genuinely contains
`component_engine_families`, `explode_by_family`, and multiple `family_label`
occurrences (lines ~733, 754, 764, 768) that were never surfaced while
`bom_graph.py` was in a `glob` array.

## Reproduction

### Repro 1 (as reported, Mercury BOM session)
```
mcp_rmcp_grep(context_lines=15, glob="app/graph_api.py",
              pattern="def explode_by_family|family_label|engine_families")
```
Reported result: **0 matches**.

### Repro 2 (as reported, Mercury BOM session)
```
mcp_rmcp_grep(glob=["scripts/bom_graph.py", "scripts/bom_analytics.py"], ignore_case=true,
              pattern="engine.planning.famil|family_to_sku|expand_family|BOM-0013|family_skus")
```
Reported result: **1 match**, inside `scripts/bom_config.py` (outside the requested list).

### Repro 3 — gap flagged by the reporting session, closed in this session

The reporting session did not independently re-run a plain
`grep(path="scripts/bom_graph.py", pattern="family", ignore_case=true)` to
confirm `path=` finds the known-present lines in that specific file. Closed
here (this session, read-only, via `workspace=` param pinned to Mercury BOM
without activating it — no changes made to that project):

```
mcp_rmcp_grep(path="scripts/bom_graph.py", pattern="family", ignore_case=true,
              workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM")
→ 21 matches (BOMGraph/engine_family_map, component_engine_families,
   explode_by_family, family_label, etc. — all real, confirmed present)
```

### Re-verification attempts THIS session (could not reproduce the reported defect)

1. Same file, using `glob=` (singular, literal, no wildcard) instead of `path=`:
   ```
   mcp_rmcp_grep(glob="scripts/bom_graph.py", ignore_case=true, pattern="family",
                 workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM")
   → 21 matches — same as path=, correctly scoped, correctly matched.
   ```

2. **Exact** Repro 1 call, same file/pattern/params as originally reported:
   ```
   mcp_rmcp_grep(context_lines=15, glob="app/graph_api.py",
                 pattern="def explode_by_family|family_label|engine_families",
                 workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM")
   → 3 matches (12 lines with context) — including the exact
     `def component_engine_families(engine: Engine, part: str)`,
     `def explode_by_family(engine: Engine, family_label: str)`, and
     `"engine_families": (engine.g.component_engine_families(item) ...)` lines.
     NOT the reported 0.
   ```

3. **Exact** Repro 2 call:
   ```
   mcp_rmcp_grep(glob=["scripts/bom_graph.py", "scripts/bom_analytics.py"], ignore_case=true,
                 pattern="engine.planning.famil|family_to_sku|expand_family|BOM-0013|family_skus",
                 workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM")
   → 0 matches. NOT the reported spurious 1-match-in-scripts/bom_config.py
     (bom_config.py is outside the array either way — but this run
     surfaced no match anywhere, not a wrong-file match).
   ```

**Net result:** none of the three re-tests reproduced the originally reported
symptom. The tool behaved correctly (matched, and scoped to the requested
files) in every re-test performed in this session.

## Environment

- Date originally observed: 2026-07-18 (Mercury BOM project session).
- Date re-tested: 2026-07-18 (this session, codescout project, target repo
  pinned via `workspace=` param — active project stayed `codescout`
  throughout; no writes made to Mercury BOM).
- Tool: `mcp_rmcp_grep` (`src/tools/grep.rs`, `Grep::call` + `parse_globs` +
  `ignore::overrides::OverrideBuilder`).
- Target files: `app/graph_api.py`, `scripts/bom_graph.py`,
  `scripts/bom_analytics.py`, `scripts/bom_config.py` in the Mercury BOM repo.

## Root cause

**Unconfirmed — could not be pinned down because the symptom did not
reproduce.** Source review of `src/tools/grep.rs` (`Grep::call`,
`parse_globs`, lines ~78-122) shows:

- `path` (or its aliases) resolves via `validate_read_path` to an absolute
  `search_path`, defaulting to the project root when omitted (`raw_path`
  defaults to `"."`).
- `glob` values are collected by `parse_globs` (single string or array) and
  fed into `ignore::overrides::OverrideBuilder::new(&search_path)` — i.e.
  override patterns are anchored to whatever `search_path` resolves to,
  **not necessarily the project root**, if a future/other call also passes
  an explicit `path` that diverges from root.
- No caching/index is involved — `WalkBuilder` + `Override` run fresh on
  every call, so a transient/non-deterministic result would be unusual for
  a pure filesystem walk. This makes an **environment/anchor mismatch** (a
  `path`/`search_path` that silently differed from the project root at the
  time of the original report — e.g. a stale `workspace_override`, or a
  narrower resolved root left over from prior context in that session) a
  more plausible explanation than a bug in the glob-matching logic itself,
  but this is **speculation, not confirmed** — the exact `ToolContext`
  state at the time of the original report is not recoverable.
- A genuine, independent finding regardless of root cause: existing tests
  (`glob_filters_by_extension`, `glob_and_ignore_case_compose`,
  `tests::glob_...`) only exercise wildcard extension globs (`"*.rs"`).
  **No test exercises a literal, wildcard-free file-path glob** (singular
  or array) anchored at a multi-segment relative path like
  `"app/graph_api.py"` or `"scripts/bom_graph.py"`. This is a real coverage
  gap worth closing even though the specific reported symptom didn't
  reproduce.

## Evidence

- `parse_globs` (`src/tools/grep.rs:589-598`): collects `glob` as
  `Vec<String>` from either a JSON string or array — no special-casing of
  wildcard-free values.
- `Grep::call` (`src/tools/grep.rs:~105-122`): builds
  `ignore::overrides::OverrideBuilder::new(&search_path)`, adds each glob via
  `ob.add(g)`, and installs it on the `WalkBuilder`. `search_path` is the
  resolved `path` param (project root when `path` is omitted).
- Existing tests only cover extension-wildcard globs — confirmed via
  `symbols(path="src/tools/grep.rs")`: `tests/glob_filters_by_extension`,
  `tests/glob_and_ignore_case_compose` both use `"glob": "*.rs"`, never a
  literal path.
- Re-test transcript above (Reproduction § "Re-verification attempts")
  shows all three re-tests behaving correctly against the same target repo
  and files named in the original report.

## Hypotheses tried

1. **Hypothesis:** `glob`'s wildcard-free literal-path matching is broken in
   `ignore::overrides::OverrideBuilder` (e.g. gitignore-style anchoring
   semantics not doing what a caller expects for a bare relative path).
   **Test:** re-ran the exact singular-glob and array-glob reproductions
   against the same files. **Verdict:** DISPROVEN for this specific
   scenario — both worked correctly on re-test (21/21 matches via `glob=`
   vs `path=` for `bom_graph.py`; 3 matches for the exact Repro-1 call
   against `app/graph_api.py`; 0/0 matches — not a wrong-file hit — for the
   exact Repro-2 array call).
2. **Hypothesis:** the original session's `path`/`search_path` silently
   diverged from the project root (stale `workspace_override`, leftover
   scoping from an earlier call in that session), causing the override
   anchor to mismatch the glob's assumed root. **Test:** not verifiable
   post hoc — the original session's exact `ToolContext` state is gone.
   **Verdict:** deferred, most plausible remaining explanation.
3. **Hypothesis:** transient/non-deterministic walker behavior (race,
   filesystem timing). **Test:** source shows `WalkBuilder`/`Override` are
   built fresh per call with no shared mutable state or cache.
   **Verdict:** unlikely, no supporting mechanism found in source.
4. **Hypothesis (added 2026-07-18, second recurrence):** the false negative
   is a real, low-frequency, session-timing-dependent defect — not glob-
   specific (the second recurrence's 4 calls used `glob=` array, `path=`
   single-file, `path=` directory, and no path/glob at all, i.e. every
   scoping mode) and not confined to one target file/string. Two
   independent human-verified sightings (`family_label`/`app/graph_api.py`
   on the first pass; `is_subassembly`/`subassembly`/`scripts/bom_graph.py`
   on the second, different session) each failed to recur on immediate
   retest — by two different investigators, in two different sessions,
   against the same target project (Mercury BOM). **Test:** re-ran the
   second recurrence's exact 4 calls verbatim, pinned via `workspace=`
   against Mercury BOM (2026-07-18, this session). **Verdict:** DID NOT
   reproduce — all 4 calls correctly found `is_subassembly`/`subassembly`
   matches in `scripts/bom_graph.py` (and correctly widened to
   `bom_config.py`/`bom_analytics.py`/`tests/test_bom_graph.py` once scope
   was widened to `scripts` and project-root). This is now the **third**
   re-test in a row (1 for the first recurrence, 1 for this one) that
   failed to reproduce a real, credibly-reported symptom — strengthening
   rather than weakening the case that the defect is real but requires a
   trigger condition neither re-test session hit. Leading candidate
   triggers (untested, unconfirmed): (a) first grep call immediately after
   a `workspace(activate)` project switch — both original reports may have
   been early in their session, before any other grep had run against that
   project; (b) index/LSP warm-up staleness right after activation, self-
   healing after the first call or two; (c) something specific to the
   *reporting* session's tool-call sequence (e.g. a prior call in the same
   turn/session leaving stale scoping state) that a fresh, isolated re-test
   call can never trigger because the re-test IS the first call of its
   session against that project. (c) is now the best-supported theory
   because both re-tests were run as early, isolated calls and both failed
   to reproduce — consistent with the defect only firing under a specific
   prior-call sequence that no re-test session has yet replicated.

## Fix

No code change made — the specific reported symptom is unconfirmed and
could not be reproduced. Recommended next steps for whoever re-opens this:

1. If the symptom recurs, capture (before doing anything else): the exact
   `path` value passed (or its absence), the active project /
   `workspace_override` at the time, and ideally a log/trace of the
   resolved `search_path` the tool actually used — to test Hypothesis 2
   directly.
2. Regardless of recurrence, add regression tests to
   `src/tools/grep.rs::tests` for a literal, wildcard-free `glob` value
   (both singular string and array) against a multi-segment relative path
   (e.g. `"sub/dir/file.rs"`), asserting matches are found AND that files
   outside an array glob are never included in results. This closes the
   confirmed test-coverage gap independent of whether the original
   anchor-mismatch hypothesis is ever confirmed.

## Tests added

Added (2026-07-19), per Fix § 2, in `src/tools/grep.rs::tests`:
- `glob_matches_literal_multi_segment_path` — literal, wildcard-free singular
  `glob` value against a multi-segment relative path; asserts the match is
  found and correctly attributed to the nested file.
- `glob_array_matches_literal_multi_segment_path_only` — array-form literal
  `glob`; asserts a file outside the array is never included.

Both pass against current `main`/`experiments`, confirming `glob`'s literal-path
handling is correct in the general case — the closed test-coverage gap, not a
production fix (no production code was changed for this issue).

## Workarounds

Prefer `path=` over `glob=` when searching a single, already-known literal
file path — `path=` was confirmed correct in every test performed (both in
the original report and in this session's re-verification). Reserve `glob=`
for actual wildcard patterns (`"*.py"`, `"**/*.rs"`) where its behavior is
covered by existing tests.

## Resume

Filed as **zombie**: observed once by a credible, detailed report with
verbatim tool calls, but not reproducible on immediate re-test with the
identical calls against the identical target files. **Re-open trigger:** the
same symptom (glob literal path finds 0 matches that `path=` finds, or a
glob array returns a match from a file outside the array) recurs — at that
moment, capture the exact `path` param value, active project, and
`workspace_override` state before anything else changes, per Fix § 1.

## References

- Reported during a Mercury BOM project session (2026-07-18), investigating
  `app/graph_api.py` / `scripts/bom_graph.py` family/OOP-related code.
- Repro 3 gap closed and Repro 1/2 re-verified in this session (codescout
  project, 2026-07-18) via `grep(..., workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM")`
  without activating that project — no changes made there.


## 2026-07-18 (later) — second independent recurrence, different string/file

**Reported (Mercury BOM project session, human-verified):** the same
*category* of false negative recurred independently — different search
string, different file, different session — from the original report
above. Ground truth: `is_subassembly` (and the bare substring
`subassembly`) is present in `scripts/bom_graph.py`'s `explode()` method
(confirmed via direct `read_file`, lines ~519-600, not a grep result):

```python
"is_subassembly": self.G.out_degree(item) > 0,
...
cols = ["item", "description", "depth", "is_subassembly", "parent", "total_qty"]
```

Yet these four `mcp_rmcp_grep` calls, run in sequence in that session
immediately before/after the confirming file read, ALL reportedly returned
**0 matches**:

1. `grep(glob=["scripts/bom_graph.py", "scripts/bom_exploded.py"], ignore_case=true, pattern="is_subassembly|subassembly")` → reported 0 matches
2. `grep(ignore_case=true, path="scripts/bom_graph.py", pattern="subassembly")` → reported 0 matches
3. `grep(ignore_case=true, path="scripts", pattern="subassembly")` → reported 0 matches
4. `grep(ignore_case=true, pattern="is_subassembly")` (no path/glob — defaults to project root) → reported 0 matches

All four calls used different scoping (glob array, `path=`file, `path=`
directory, no scope at all) and all four reportedly missed a string
unambiguously present in the file. Notably this recurrence is **not**
glob-specific — call 2 used `path=` (a single file), which the *first*
recurrence's Repro 3/re-verification had shown working correctly — and
call 4 used neither `path=` nor `glob=` at all. This is a stronger,
independently-reproduced signal than the original zombie report: different
string, different file, different session, same target project (Mercury
BOM, active project at the time for the reporting session).

### This session's own reproduction attempt

Re-ran the exact 4 calls verbatim, pinned via `workspace="c:\Users\MAILINCA.BRN.002\work\Mercury BOM"`
(active project stayed `codescout` throughout; no writes made to Mercury BOM):

1. `grep(glob=["scripts/bom_graph.py", "scripts/bom_exploded.py"], ignore_case=true, pattern="is_subassembly|subassembly", workspace=...)` → **7 matches** in `scripts/bom_graph.py` (lines 531, 580, 584, 611, 625, 815, 819 — all real `is_subassembly` occurrences in `BOMGraph.explode`/`common_parts`). NOT the reported 0.
2. `grep(ignore_case=true, path="scripts/bom_graph.py", pattern="subassembly", workspace=...)` → same **7 matches**. NOT the reported 0.
3. `grep(ignore_case=true, path="scripts", pattern="subassembly", workspace=...)` → **16 matches in 3 files** (`bom_config.py` ×8, `bom_graph.py` ×7, `bom_analytics.py` ×1) — correctly widened scope, correctly found everything. NOT the reported 0.
4. `grep(ignore_case=true, pattern="is_subassembly", workspace=...)` (no path/glob) → **9 matches in 2 files** (`bom_graph.py` ×7, `tests/test_bom_graph.py` ×2, including `test_explode_is_subassembly_flag`). NOT the reported 0.

**Net result: did not reproduce, again.** All 4 calls behaved correctly on
this attempt — matched, correctly scoped, correctly widened as scope
widened. This is now two-for-two: every attempt (across two different
investigators, two different sessions, two different original repro
strings/files) to reproduce a reported grep false-negative on demand has
failed, while the original reports themselves were detailed, verbatim, and
came from credible human-verified sessions with direct-read ground truth.
See Hypotheses tried § 4 for the refined theory this supports: the defect
is real but session-sequence-dependent in a way that an isolated,
first-call-of-session re-test cannot trigger.

**Status change:** `zombie` → `investigating`. A single unreproducible
sighting is a zombie; two independent, differently-shaped sightings of the
same *category* of defect (silent false negative, ground-truth-confirmed
absence, across varied scoping params) — even though this session's own
re-test also failed to reproduce — is enough signal to keep this actively
watched rather than let it lapse back to zombie. Re-open trigger from the
original filing (capture `path`/`workspace` state and call sequence the
moment it recurs) still stands and is now the single highest-value next
data point.


### Bookkeeping correction — 2026-08-26 zombie-verify pass

The `Status change: zombie → investigating` line above was written 2026-07-18, same day as
the second recurrence, but only as prose — the frontmatter `status:` field was never
actually flipped, so every `artifact(find, status="zombie")`-style query kept reporting
this as a settled, no-action zombie for over five weeks. Caught during a routine
verify-open sweep of this project's three zombie bug files; corrected to match what the
file itself already concluded. No new evidence gathered — this is a catalog-vs-prose
reconciliation, not a re-investigation. The re-open trigger from the original filing
(capture `path`/`workspace` state and call sequence at the moment of recurrence) is still
the next useful action if this fires again.
