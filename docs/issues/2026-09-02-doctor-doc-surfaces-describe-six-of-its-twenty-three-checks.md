---
id: '88c3f36937d83ed6'
kind: bug
status: taken
title: 'BUG: every doc surface describes `doctor` as a catalog-drift scanner, hiding ~17 of its 23 checks'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
topic: doctor check discoverability
claimed_at: 2026-09-11
claimed_by: ec98641c-d5ba-456d-8e4a-10e24d52da16
closed: ''
opened: 2026-09-02
owner: marius
related: []
severity: medium
---

# BUG: every doc surface describes `doctor` as a catalog-drift scanner, hiding ~17 of its 23 checks

## Summary

`librarian(action="doctor")` runs **23** `scan_*` checks. **Five** surfaces that describe
it — `docs/PROBES.md`, the `librarian` MCP tool-description string, the served
`get_guide("librarian")` row, and the module rustdoc on both `doctor.rs` files — describe
it as a *catalog-drift* scanner over roughly six.
The entire statement-validity family and the entire bug-record family are undescribed
everywhere, so an agent routing by the documented capability never learns they exist.


## Re-derivation 2026-09-07 — the count moved, and the published command never produced it

The title and § *Summary* above are **superseded, not wrong**: `23` was exactly right at the
cited `c5798d2b`. They are left standing because what moved is the count, and the file is cited
by that number.

**Three corrections, in descending importance.**

**1. The reproduction command in § *Reproduction* does not reproduce `23`, and never did.**
It reads:

```
symbols(name="scan_", path="src/librarian/tools/doctor.rs", kind="function")  → 23
```

Run today it returns **32**; at `c5798d2b` it would have returned at least **26**. `name="scan_"`
is a substring match over *all* functions, so it also collects the test functions —
7 of the 32 today (`tests/scan_worktree_scoped_*`, `tests/call_wires_*scan_*`). The author
filtered those out mentally and published the unfiltered command, so a reader re-deriving gets a
number they cannot reconcile with the title and has no way to tell which of the two is wrong.
This is `CLAUDE.md` § *Observer Blindness*'s *"ship its derivation rather than its value"*
failing in its subtler direction: the derivation shipped, and it does not yield the value.

The form that does reproduce it excludes test functions — either by reading the `tests/` prefix
out of the `symbols` output, or:

```bash
git show <ref>:src/librarian/tools/doctor.rs | awk '
  /^\s*#\[(tokio::)?test\]/ {istest=1; next}
  /^\s*(pub )?(async )?fn scan_[a-z0-9_]+/ { if (istest) t++; else p++ }
  /^\s*fn |^\s*pub fn / {istest=0}
  END {print p}'
```

**2. In this file's own unit — production `scan_*` functions — the count is now 25.**

| ref | production `scan_*` | test-named `scan_*` |
|---|---:|---:|
| `c5798d2b` (cited at filing) | **23** | 3 |
| `087cabfb` | 24 | 3 |
| `4b30601c` (2026-09-07) | **25** | 3 |

The two additions are `7790f343 feat(doctor): report a live bug file cited from a source file
whose citation has settled` and one earlier in the same range. The `test-named` column is
constant at 3, which is why the unfiltered total tracked the real one and the discrepancy stayed
invisible.

**3. There is a second defensible unit, and it is nearly half again as large: 37 declared
check NAMES.** `declare_checks!` (`src/librarian/tools/doctor.rs:234`) has 37 arms, and
`Violation::new` carries a `debug_assert!` that panics on any name not in it — so it is
authoritative in a way a function count is not. One `scan_*` function emits several check names
(`scan_claim_liveness` alone emits four: `claim_held_by_dead_session`,
`claim_held_by_live_session`, `claim_unresolvable_here`, `claim_without_claimant`).

Confirmed independently at runtime: `librarian(action="doctor")`'s `summary.by_check` has exactly
**37** keys, because the report seeds every declared check including the zero ones. Two
instruments with genuinely different scopes — a source parse and a runtime report — agreeing.

**Which unit the fix should use.** The bug is about *discoverability*, and what an agent needs to
discover is the set of things `doctor` can tell it — which is the **37 check names**, not the 25
functions that emit them. § *Fix*'s Option A ("list all 23") should therefore read 37 and should
enumerate `declare_checks!`, which has the additional property of being one edit away from the
names the tool actually reports.

**The surfaces are still wrong, and by more than when this was filed.** The
`get_guide("librarian")` row served during this session reads: *"Read-only catalog drift scan
(forward-slash form, NTFS ADS colons, `..` segments, missing-on-disk files,
`abs_path_must_be_absolute`), plus `claim_liveness`"* — six named, against 37. Filing said
"~17 of 23 hidden"; the live figure is **31 of 37**.

**Derived by** sessionId `89d91024-cd66-4361-9300-c55b87b179ea`, 2026-09-07, while reconciling a
119-commit pull. Related friction: `docs/trackers/statement-validity-session-log.md` `F-10`.
## Symptom (Effect)

`docs/PROBES.md:173`, on the page `CLAUDE.md` calls *"a one-page index of every
measurement instrument"* and tells you to read *"before answering a question with a
number"*:

```
| `librarian(action="doctor")` | Catalog drift: `abs_path` form, ADS colons, `..` segments,
missing-on-disk files, worktree-scoped rows, frontmatter-id vs catalog-id mismatch |
Read-only unless you pass `fix=…`. Opt-in repairs are individually gated and dry-run by default |
```

`get_guide("librarian")` § *librarian(action=…) — Reference*:

```
| `doctor` | Read-only catalog drift scan (forward-slash form, NTFS ADS colons, `..`
segments, missing-on-disk files, `abs_path_must_be_absolute`). Manual — run after large
refactors or when downstream LIKE queries return empty. |
```

The `librarian` tool description carries the same six, phrased differently.

**Corrected 2026-09-02, same session: the count is five, not three.** The first sweep
searched for check *names* (`entry_dated_stale`, `non_terminal_status_with_fix_anchor`) and
so could only find surfaces that name a check — but these surfaces name none, they say
"catalog drift". Re-grepping the phrase returns the full population:

```
grep(pattern="catalog drift|Catalog drift",
     glob=["src/**/*.rs", "src/**/*.md", "docs/PROBES.md"])   → 6 matches in 5 files
```

| # | site | audience | text |
|---|---|---|---|
| 1 | `docs/PROBES.md:173` | agents + humans | "Catalog drift: `abs_path` form, ADS colons, …" |
| 2 | `src/librarian/tools/librarian.rs:30` (`Librarian/description`) | agents — **the tool description itself** | "doctor: catalog drift scanner (read-only by default)…" |
| 3 | `src/prompts/guides/librarian.md:286` | agents, via `get_guide` | "Read-only catalog drift scan (forward-slash form, NTFS ADS colons, …)" |
| 4 | `src/librarian/tools/doctor.rs:1` | developers, rustdoc | `//! Doctor — catalog drift scanner.` |
| 5 | `src/cli/doctor.rs:1` | developers, rustdoc | `//! … invoke the librarian catalog drift scanner.` |

#2 is the load-bearing one and the one the first sweep missed: it is the string an agent
reads to decide whether `doctor` is the right instrument for a question.

A sixth site states it a third way — `codescout doctor --help` describes the CLI as
"Read-only scan of the librarian catalog for invariant violations: non-forward-slash
separators, NTFS ADS colons, `..` segments, and missing files on disk" — four checks,
verified by running the binary 2026-09-02. It is generated from the same clap doc comment
as #5, so it is one site with two renders rather than a sixth independent surface.

## Reproduction

```
git rev-parse HEAD          # c5798d2b5d489d6496a411883112123081761365 at filing
```

1. Count the checks:
   `symbols(name="scan_", path="src/librarian/tools/doctor.rs", kind="function")` → **23**
   `scan_*` functions (excluding the `tests/` entries the same query returns).
2. Read any of the three surfaces above → **~6** described.
3. Grep the newest check's name:
   `grep(pattern="non_terminal_status_with_fix_anchor", mode="files")` → 7 files, **none**
   of them `docs/PROBES.md`, `CHANGELOG.md`, `CLAUDE.md`, or a prompt guide. It exists in
   `doctor.rs` and in bug/tracker prose only.
4. Contrast with an older check:
   `grep(pattern="entry_dated_stale", mode="files")` → 15 files, **including**
   `CLAUDE.md`, `docs/TAXONOMY.md`, `CHANGELOG.md`, `docs/manual/src/concepts/statement-validity.md`
   and `src/prompts/guides/tracker-conventions.md`.

The asymmetry between 3 and 4 is the finding: the describe-it-everywhere step happened
for some checks and then stopped, and nothing gates it.

## Environment

codescout on branch `experiments`, Linux, MCP over stdio. Not environment-sensitive —
this is a text-vs-code comparison.

## Root cause

**Mechanism.** Adding a `doctor` check requires exactly two edits to *function*:
the `scan_*` function itself, and one registration line at
`src/librarian/tools/doctor.rs:414` (`all_violations.extend(scan_…(ctx, &cat.conn)?)`).
Nothing in the build, the test suite, or the gate connects that set to the three prose
surfaces that enumerate checks. So a new check is fully working and fully undiscoverable,
and the author gets no signal — measured by inspection of `doctor.rs` and by the two greps
above, 2026-09-02.

**Why nobody notices — the observer structure.** The party who would have to write the
description is the check's author, and they are the one person for whom the omission costs
nothing: they know the check exists, they know `doctor` runs it, and they can see it in
`doctor.rs`. The party misled is a later agent choosing an instrument from `PROBES.md`,
who never talks to them. `CLAUDE.md` § *Observer Blindness*, and `OB-1` § *the third
position*.

**Why the doc surfaces agree with each other.** They are not five independent
confirmations — the tool-description string, the guide row, the PROBES row and the two
rustdoc headers appear to descend from one original phrasing written when `doctor`
genuinely was a catalog-drift scanner. Their agreement is one stale claim counted five
times, which is the failure mode `CLAUDE.md` § *Reaching a Peer Session* names as "check
independence, not agreement".

## Evidence

Check families present in `doctor.rs` and absent from all three surfaces:

- **statement-validity** — `scan_conditional_past_due`, `scan_dated_stale`,
  `scan_cited_but_undeclared`, `scan_validity_unparseable`,
  `scan_cited_prefix_with_no_definer`, `scan_undefined_entries`, `scan_entry_defined_twice`
- **bug-record** — `scan_terminal_status_with_caveat`,
  `scan_terminal_status_without_fix_anchor`, `scan_non_terminal_status_with_fix_anchor`,
  `scan_archived_fix_sha_unresolvable`
- **augmentation** — `scan_augmentation_declared_but_absent`, `scan_sidecar_shape_drift`,
  `scan_params_behind_body`, `scan_params_status_drift`
- **other** — `scan_snapshot_drift`, `scan_premature_archive_citation`,
  `scan_unterminated_fence`

Note that `CLAUDE.md` § *Session Intelligence Trackers* **does** cite three of the
statement-validity checks by name ("run `librarian(action="doctor")` before hand-rolling a
scan — `entry_dated_stale`, `entry_conditional_past_due` and
`entry_cited_from_outside_but_undeclared` already ship the machine-checkable half"). So the
project already depends on capabilities its own instrument index does not list.

## Hypotheses tried

1. **Hypothesis:** `PROBES.md` deliberately summarises rather than enumerates.
   **Test:** read its own admission test (lines 20-26) and the `index(action="verify")`
   row beside the `doctor` row.
   **Verdict:** rejected — the neighbouring rows enumerate specific measured fields
   (`expected_files`, `chunks_without_vectors`, `git_sync`, `empty_eligible_dirs`) at
   length. The `doctor` row is short because it is stale, not because the page summarises.

## Fix

Applied Option B (describe by family) as this file recommended, across all
five named sites. Re-scouted the check count before writing: it had already
drifted from 23 (filing time) to 26 `scan_*` functions — living proof, inside
this very fix, that Option A (enumerate) would have rotted immediately and
Option B was the only defensible choice.

- `docs/PROBES.md` — already partially updated since filing (mentioned
  bug-record + statement-validity); added the missing augmentation family.
- `src/prompts/guides/librarian.md` — already partially updated (mentioned
  `claim_liveness`); rewritten to name all four families explicitly, then
  trimmed to fit the guide's 2500 B declared-section cap (first attempt
  overflowed it by 164 B — caught by `declared_sections_are_within_the_size_cap`).
- `src/librarian/tools/librarian.rs` (the `Librarian` tool description) —
  rewritten to name all four families, funded within
  `TOOL_SURFACE_CHAR_BUDGET` (zero headroom at fix time) by trimming
  "JSON violation-count report. Opt-in repairs, each detailed under the `fix`
  param." to "JSON report; repairs are opt-in via `fix`."
- `src/librarian/tools/doctor.rs:1` and `src/cli/doctor.rs:1` (module rustdoc
  headers) — both changed from "catalog drift scanner" to name all four
  families.

NOT touched: the much larger `//! Checks (MVP)` numbered list further down in
`doctor.rs`'s module doc, which is its own, separate staleness (documents ~8
of 26 checks) — out of scope for the five sites this bug named.

Fix SHA: *(recorded at archive time — see Resume)*
Patch-id: *(recorded at archive time — see Resume)*
## Tests added

None new — doc/description-only correction. Re-ran green: `tool_surface_under_budget`,
`declared_sections_are_within_the_size_cap` (initially failed at 2506 B, fixed by
further trimming), `prompt_surfaces_reference_only_real_tools`, `doc_tool_refs`
(3 tests).
## Workarounds

Read `doctor`'s check list from the code, not the docs:

```
symbols(name="scan_", path="src/librarian/tools/doctor.rs", kind="function")
```

Or just run `librarian(action="doctor")` and read the per-check keys in the JSON report —
the report is accurate; only the descriptions are not.

## Resume

The `//! Checks (MVP)` numbered list in `doctor.rs` (currently documents ~8 of
26 checks) is a related but separate staleness, worth its own bug file if
someone wants to take it — not filed here since it wasn't in this bug's named
scope. Otherwise N/A — fixed.
## References

- `docs/trackers/bug-claim-liveness-session-log.md` — `F-1`, the scout that found this
- `docs/superpowers/plans/2026-09-02-bug-claim-liveness-design.md` § *Wiring* — the plan
  whose surface table was short by three because of this
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md`
- `CLAUDE.md` § *Observer Blindness*
