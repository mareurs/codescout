---
id: '1dea5090a3a2f5ad'
kind: bug
status: fixed
title: 'BUG: the Audit Doc Refs gate never scans CHANGELOG.md or CONTRIBUTING.md — 18 broken refs and 8 high findings hide there'
owners:
- marius
tags:
- audit-doc-refs
- ci-gate
- coverage-gap
- docs
- cluster/selector-narrower-than-its-population
---

## Summary

`DEFAULT_AUDIT_GLOBS` is `["docs/**/*.md", "CLAUDE.md", "**/CLAUDE.md", "**/README.md"]`
(`src/librarian/tools/audit_doc_refs/mod.rs:149`). Exactly two tracked root-level markdown
files fall outside it: **`CHANGELOG.md`** and **`CONTRIBUTING.md`**. Scanned explicitly they
hold **18 broken refs, 8 of them `high`** — findings that would fail `--fail-on high` today
if the glob set reached them. The gate has never looked.

`CHANGELOG.md` is the worst possible file to leave unaudited: per memory
`experimental-docs-lifecycle`, every unreleased manual page deliberately links to
`CHANGELOG.md` `[Unreleased]` as *the one canonical cohort list*. The hub of the citation
graph is the one markdown surface with no lint.

## Symptom (Effect)

Default scan reports the file count that excludes both, and never names them:

```
$ ./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json --project .
"n_files_scanned": 887,
"exit_code": 0
```

Scanned explicitly, the same tree fails:

```
$ ./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json \
    --project . --paths CHANGELOG.md --paths CONTRIBUTING.md
"n_files_scanned": 2,
"n_refs_found": 87,
"n_refs_resolved": 50,
"n_refs_broken": 18,
"n_refs_unknown": 19,
"exit_code": 1
```

The 8 `high` findings, all `verdict: missing` / `severity_reason: policy_default`:

| md_line | raw_ref |
|---|---|
| CHANGELOG.md:249 | `scripts/chunk-model-matrix.py` |
| CHANGELOG.md:481 | `src/embed/index.rs`, `src/embed/drift.rs`, `src/embed/bm25.rs` |
| CHANGELOG.md:482 | `src/embed/chunker.rs`, `src/embed/local.rs`, `src/embed/remote.rs` |
| CHANGELOG.md:519 | `src/tools/github.rs` |

## Reproduction

`git rev-parse HEAD` → `f244ad17` (branch `experiments`). Run the two commands above.
The delta between `n_files_scanned: 887` and the two extra files is the whole bug.

## Environment

Linux, codescout 0.15.0 on `experiments`, release binary built with `cargo rb`.
`LIBRARIAN_WORKSPACE` unset (local workspace). Reproduces identically in CI —
run `31218234007`'s Audit Doc Refs job also reports `n_files_scanned: 887`.

## Root cause

`run_audit` picks its file set at `src/librarian/tools/audit_doc_refs/mod.rs:212-223`:
`args.paths` when given, otherwise `DEFAULT_AUDIT_GLOBS` + `DEFAULT_AUDIT_EXCLUDES`. The
glob list enumerates `docs/**` plus three named/`**`-anchored filenames. It was written to
cover *the docs tree* and two conventional filenames; root-level project docs that are
neither `CLAUDE.md` nor `README.md` were never enumerated, so they were never in scope —
there is no exclusion to point at, only an absence.

`DEFAULT_AUDIT_EXCLUDES` (`mod.rs:182`) is unrelated: it holds only `docs/agents/**` and
`docs/lessons/**`.

**measured 2026-08-08:** the `--paths CHANGELOG.md --paths CONTRIBUTING.md` run above —
87 refs, 18 broken, 8 high, exit 1. The glob constant was read at `mod.rs:149`; the
`887 → 2` file-count split is the direct observation that the default run excludes them.

## Evidence

### The 8 high findings are *historical* release sections, not live drift

Every `src/embed/*.rs` ref sits in a released-version section describing a module layout
that was correct when that version shipped. `src/embed/` has since been reorganised. So
these are the changelog equivalent of an archived bug file: a true statement about the past
whose paths no longer resolve.

That is what makes this more than a one-line fix — see § Fix.

### Why it surfaced now

Archiving three bug files on 2026-08-07 (`9886773e`, `2feeabf5`) left 25 dangling
`docs/issues/<slug>.md` citations, two of them in `CHANGELOG.md`. Fixing all five *live*
markdown citations moved the audit's broken count by only **3** (9080 → 9077), not 5. The
missing 2 were the CHANGELOG pair — a tally that did not move is what exposed the gap.

### `src/prompts/**/*.md` is also unscanned — but wants the OPPOSITE treatment

The same glob gap covers the prompt-guide surface. **measured 2026-08-08:**

```
$ ./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json \
    --project . --paths 'src/prompts/**/*.md'
"n_files_scanned": 14, "n_refs_broken": 26, "exit_code": 1     # 9 high
```

`src/prompts/README.md` *is* scanned (it matches `**/README.md`); the 13 guide/source
files beside it are not. But unlike `CHANGELOG.md`, **none of these 9 highs is real drift**:

- **8 are teaching placeholders** — `src/foo.rs` (`guides/iron-laws-detail.md:99`),
  `docs/plans/foo.md`, `docs/trackers/foo.md`, `docs/archive/foo.md`,
  `docs/trackers/archive/foo.md`, `docs/specs/`. These are illustrative example paths in
  agent-facing instructions — exactly the rationale already written for
  `DEFAULT_AUDIT_EXCLUDES`' `docs/agents/**` entry (`mod.rs:179-182`): *"the files there
  describe reader-side paths ... and produce only FPs against the audited project."*
- **1 is an accepted false positive** — `src/prompts/source.md` → `docs/ARCHITECTURE.md`,
  which is a built-in **classifier pattern**, not a citation. Already analysed and accepted
  in `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`
  (§ *"a configuration value that looks like a path — the doc is right; the pattern need
  not resolve"*), and `docs/manual/src/concepts/librarian-embedded.md:89` carries an
  explicit `<!-- audit-doc-refs:ignore -->` for the same value.

So the correct routing for this subtree is **`src/prompts/**` added to
`DEFAULT_AUDIT_EXCLUDES`, not to `DEFAULT_AUDIT_GLOBS`** — making the existing de-facto
silence explicit and intentional, so a future glob widening cannot accidentally pull 26
placeholder refs into the gate. That is a different fix from the CHANGELOG one, and the two
should not be bundled.

The general lesson: `DEFAULT_AUDIT_GLOBS` being an allow-list means every unscanned surface
is silent for the *same* reason, whether that silence is right (prompt guides) or wrong
(CHANGELOG). Silence carries no signal about which — that is the defect.
## Hypotheses tried

1. **Hypothesis:** the CHANGELOG refs resolve via the basename fallback.
   **Test:** read `try_basename_fallback` (`resolver.rs:82`).
   **Verdict:** rejected — `if raw_ref.contains('/') { return None }`; every ref here has a
   separator, so the fallback cannot apply to any of them.
2. **Hypothesis:** a severity drop rule demotes them below the gate.
   **Test:** read `apply_drops` (`severity.rs:145`) — drops key on `archive/`, memory globs,
   `docs/issues/`, `DEFAULT_HISTORICAL_DIRS`. **Verdict:** rejected — `CHANGELOG.md` matches
   none, which the explicit-paths run confirms by reporting them at `policy_default` / `high`.
3. **Hypothesis:** the 50-finding cap hid them.
   **Test:** the exit code iterates `findings.iter()`, not `shown_findings`
   (`mod.rs:770-783`), and the `by_file` overflow map lists every file with findings —
   `CHANGELOG.md` appears in neither. **Verdict:** rejected.
4. **Hypothesis:** the glob set omits them. **Verdict:** confirmed — `mod.rs:149`.

## Fix

Shipped in `130c93a5`, following this file's own sequencing (reconcile first, then
widen the globs). The recommendation — option (b), a severity drop for released
sections — was right. The *classification* it rested on was not.

**Re-measured on HEAD (2026-08-15): 10 highs, not 8, and they are three different
things.** This file said "the 8 high findings are historical release sections" and
"every `src/embed/*.rs` ref sits in a released-version section". The second is
true; the first is not.

| Class | Count | Disposition |
|---|---|---|
| Historical release sections | 7 | `cap_released_history` — drop one band |
| **Live drift, genuinely broken** | 1 | **repaired** |
| Correct-as-written, needing neither | 2 | ignore marker / display form |

**The one that mattered.** `CHANGELOG.md` cited
`docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`
after that bug file was archived to `docs/issues/archive/`. The very next bullet
cites an archived file *correctly*, so it was a plain miss — and the gate caught
it the instant it could see the file. That single finding is the argument for
keeping `[Unreleased]` at full severity instead of forgiving the whole file, and
it is why the implementation is a line-boundary cap rather than a path rule.

**The two that needed no severity change at all.** Both would have been quietly
mis-forgiven by a blanket rule:

- `scripts/chunk-model-matrix.py` sits under `### Removed`. **A removal entry
  cites the path it removed, so its refs dangle by construction** — that is the
  entry being accurate, not drift. The existing `<!-- audit-doc-refs:ignore -->`
  marker is scoped to exactly one section, which is exactly the right scope, so
  the block carries one. No new mechanism.
- `` `src/foo.rs :: impl Bar :: fn baz(…)` `` illustrates a *generated header
  format*. It now uses the repo's established double-backtick display form, which
  `is_markup_display` already skips outright.

**Implementation.** `severity::released_history_boundary` + `cap_released_history`,
applied once per file in `run_audit` where both the text and each ref's `md_line`
are already in scope — so no struct or signature changed. One boundary suffices
because a Keep-a-Changelog file is reverse-chronological and appends at the top:
the first non-`[Unreleased]` version heading divides live claims from history.

Then `CHANGELOG.md` and `CONTRIBUTING.md` joined `DEFAULT_AUDIT_GLOBS`.

**Step 3 (`src/prompts/**` → excludes) is deliberately still not bundled**, as
this file directed. Those paths are not in the glob list either, so the silence is
real but undocumented; making it explicit is its own change.
## Tests added

Three in `src/librarian/tools/audit_doc_refs/mod.rs`:

- `default_scan_covers_changelog_and_contributing` — the coverage gap itself.
- `changelog_released_sections_drop_below_the_gate_but_unreleased_does_not` — the
  load-bearing one. Both arms in one fixture, because **a cap that swallowed both
  would look identical to a correct one** from either arm alone. Its fixture also
  creates a real `src/` directory: without it `cap_inferred_path` caps both refs
  to `med` first and the test passes for the wrong reason. That is not
  hypothetical — it failed exactly that way on the first run.
- `released_history_boundary_only_applies_to_changelogs` — pins the `None` cases:
  a non-changelog with `## [x]` headings must get no boundary (otherwise any doc
  using that heading style silently stops gating), and a changelog with nothing
  released yet has no history to forgive.

**End-to-end verification with the real CI command**, not just unit tests:
`./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high
--json --project .` — **exit 1 before, exit 0 after**, across 943 files.

Gate: `cargo test --workspace` → 3808 passed / 0 failed / 50 ignored; clippy
`--workspace --all-targets -D warnings` clean.
## Workarounds

Audit them explicitly:

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json \
  --project . --paths CHANGELOG.md --paths CONTRIBUTING.md
```

## Resume

Closed, and the one surviving follow-up is now discharged too —
`experiments:325f4387` added the `src/prompts` payloads to
`DEFAULT_AUDIT_EXCLUDES`, not bundled, as this file instructed.

**It shipped narrower than this file prescribed, and the correction came from
this file's own evidence.** The prescription above says `src/prompts/**`; the
section two headings up records that `src/prompts/README.md` *is* scanned via
`**/README.md`. Those two facts do not compose. Excludes are matched after the
include set with no re-include and `globset` has no negation, so
`src/prompts/**` would not have documented a silence — it would have created
one, costing the 28 refs (24 resolved, 0 high) on the only file in that
directory whose refs are real citations rather than teaching placeholders. The
placeholder-vs-citation discriminator this file drew cuts *between* README and
its neighbours, not around the directory, so the four payload patterns are
enumerated: `src/prompts/guides/**`, `src/prompts/source.md`,
`src/prompts/memory-templates.md`, `src/prompts/workspace_onboarding_prompt.md`.

The entries are **inert** under today's `DEFAULT_AUDIT_GLOBS` — verified, not
assumed: the default scan with the new constant compiled in reports the same
945 files / 50134 refs / exit 0, with README still contributing its 28. Their
only job is to survive a future glob widening, so the test
(`prompt_payload_excludes_survive_a_glob_widening_but_spare_the_readme`)
simulates that widening; a default-scan assertion would have passed with the
four entries deleted.

The transferable lesson is about the *shape of the reconciliation*, not the gate.
This file grouped 8 findings under one label ("historical release sections") after
verifying that label against the `src/embed/*` subset. The generalisation held for
7 of 10 and hid the only finding that was a real bug — which is the worst possible
place for a summary to be slightly wrong, because the summary is what decides
whether anyone looks at the individual rows. **A count and a category are two
claims; verifying the category on a sample does not verify it on the count.**
## References

- `src/librarian/tools/audit_doc_refs/mod.rs:149` — `DEFAULT_AUDIT_GLOBS`
- `src/librarian/tools/audit_doc_refs/mod.rs:182` — `DEFAULT_AUDIT_EXCLUDES`
- `src/librarian/tools/audit_doc_refs/severity.rs:145` — `apply_drops`
- `src/librarian/tools/audit_doc_refs/severity.rs:183` — `matches_archive`, the model for the proposed drop
- `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` — the
  reconciliation pass that made the gate green; it worked the `docs/**` tree only, so these
  two files were never in its scope either
- CI run 31218234007 — the Audit Doc Refs failure that started this investigation (a
  different, live finding: `docs/manual/src/concepts/retrieval-stack.md:275`)
