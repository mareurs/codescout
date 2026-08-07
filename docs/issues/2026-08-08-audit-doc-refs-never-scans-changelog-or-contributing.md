---
id: f85d79f54a12f7a1
kind: bug
status: open
title: 'BUG: the Audit Doc Refs gate never scans CHANGELOG.md or CONTRIBUTING.md — 18 broken refs and 8 high findings hide there'
owners:
- marius
tags:
- audit-doc-refs
- ci-gate
- coverage-gap
- docs
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

Two steps, in this order. Step 1 alone turns the gate red.

1. **Reconcile the 18 refs first.** The 8 high ones are historical release sections. Decide
   between: (a) correcting each path to where the file lives now — wrong, it falsifies what
   that release shipped; (b) a severity drop for released-version sections, the changelog
   analogue of `matches_archive`, leaving `[Unreleased]` at full severity; (c) scoping the
   changelog scan to the `[Unreleased]` section only. **(b) is the recommendation** — it
   matches the existing `archive_drop` rationale exactly (a historical record citing a moved
   path carries no live drift signal) and keeps the section that actually gates.
2. **Then** add `CHANGELOG.md` and `CONTRIBUTING.md` to `DEFAULT_AUDIT_GLOBS`.
3. **Separately** (not bundled) add `src/prompts/**` to `DEFAULT_AUDIT_EXCLUDES` — see
   § Evidence: those 26 broken refs are teaching placeholders and one accepted
   classifier-pattern FP, so the fix there is to make the silence explicit rather than to
   start gating on it.

Not fixed in this session: it is a gate-severity change landing on the eve of a
474-commit `experiments` → `master` promotion, and the two CHANGELOG refs that made it
visible were repaired by hand.

## Tests added

None yet — the fix is not implemented. The regression test to write with step 2 is a
default-glob assertion in the shape of the existing
`default_scan_excludes_docs_agents` (`mod.rs:1367`): assert `CHANGELOG.md` and
`CONTRIBUTING.md` **are** in the default file set, so a future glob edit cannot silently
drop them again.

## Workarounds

Audit them explicitly:

```
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high --json \
  --project . --paths CHANGELOG.md --paths CONTRIBUTING.md
```

## Resume

Implement § Fix step 1 option (b): add a `matches_released_changelog`-style drop to
`src/librarian/tools/audit_doc_refs/severity.rs` next to `matches_archive`
(`severity.rs:183`), keyed on the ref's line falling under a released `## [x.y.z]` heading
rather than `## [Unreleased]` — note this needs section context the resolver does not
currently carry, so check whether `RefCandidate` (`src/librarian/tools/audit_doc_refs/parser.rs`)
can supply the enclosing heading before committing to the approach. Then step 2, plus the
glob-membership test named above. Verify with the `--paths` command in § Workarounds
reaching exit 0, then a default run still at exit 0.

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
