---
id: '98375fa1124ab1ca'
kind: tracker
status: active
title: Archived-bug SHA reconciliation queue (experiments → master)
tags:
- release
- bug-tracking
- bookkeeping
topic: Reconciling experiments-side fix SHAs to master in archived bug files after the 2026-07-28 archive-gate change
---

# Archived-bug SHA reconciliation queue (experiments → master)

**Why this exists.** On 2026-07-28 the bug-archive gate changed from *"archive after
the fix reaches `master`"* to *"archive once the fix is verified on `experiments`"*
(`dd56423e`), and 135 terminal-status bug files were archived in one batch. The old
gate was silently guaranteeing something the new one does not: while archiving waited
for `master`, every archived file necessarily carried a **master** SHA. Archiving
earlier means files land in `docs/issues/archive/` holding an `experiments` SHA that
the next `git rebase master` orphans — `git branch --contains <orphan-sha>` then
returns empty, which is the exact failure `docs/RELEASE.md` § *After cherry-pick*
documents.

Nothing re-reads `archive/`. This file is the queue so that debt is visible instead of
rotting there. `docs/RELEASE.md` step 4 is the consumer.

**Scale at creation:** `experiments` was **365 commits ahead of `master`**, so this is
not a handful of stragglers.

## How to work it

For each cherry-pick that lands on `master`:

1. Find the row below whose SHA you just cherry-picked.
2. In each listed file (now under `docs/issues/archive/`), replace the
   `experiments`-side SHA with the **master-side** SHA that `cherry-pick` assigned,
   and delete the "master-side SHA still to be recorded" line from its `## Resume`.
3. Confirm `git branch --contains <master-sha>` shows `master`.
4. Strike the row here. When the table is empty, delete this tracker — it is a finite
   queue, not a living ledger.

Edit archived files through the catalog (`artifact(action="update", id=…)`), not by
path — `id = sha256(abs_path)` and these rows were just re-keyed by the move.

## Accuracy caveats — read before trusting a row

The SHA column was derived mechanically: strip YAML frontmatter, take every 7–40 char
hex token in the body, keep the first that `git cat-file -e <tok>^{commit}` validates,
then test it with `git branch --contains`. Three known noise sources:

- **A file may cite several commits** (its own fix, a related bug's fix, a commit named
  in Evidence). "First validating token" is not guaranteed to be the *fix* SHA.
- **40-char entries are probably provenance, not fixes.** `52fcaf0118d9…`,
  `61a800af6570…` and `a92c734fde3e…` look like librarian `head_commit` metadata that
  leaked into the body. Note `52fcaf01` also appears as its own 8-char row — same
  commit, cited twice in different forms.
- **A first pass of this census was wrong and is worth recording.** It matched the
  artifact `id:` out of the frontmatter and fed a 16-hex non-commit to
  `git branch --contains`, which fails for everything — producing a confident
  "80 experiments-only / 33 master / 22 unknown" that was pure artifact. The corrected
  run gives 49 / 45 / 41. Any future recount must skip frontmatter and validate that
  the token is actually a commit.

## Census (135 archived files)

| provenance of first validating SHA | count | action |
|---|---:|---|
| **`experiments`-only** | **49** | in the queue below |
| already on `master` | 45 | nothing owed |
| no validating commit SHA in body | 41 | needs manual inspection; listed at the bottom |

## Queue — 49 files across 44 distinct SHAs

> Four more files were archived after this census was taken — see *Added after the census*
> below. Total outstanding is 53 files, not 49.

| experiments SHA | files | archived bug file(s) |
|---|---:|---|
| `06946ae3` | 1 | `2026-07-06-constitution-rule-malformed-glob-silent-fail-open.md` |
| `0cefd1f3` | 1 | `2026-07-10-memory-cross-embed-ignores-workspace-pin.md` |
| `0d62b2ec` | 1 | `2026-07-10-silent-cap-missing-overflow-signals-audit.md` |
| `0ef6c7bc` | 1 | `2026-07-12-activate-index-status-stale-probe-cache-false-negative.md` |
| `10e5ed0e` | 1 | `2026-07-17-catalog-dead-rows-no-gc.md` |
| `14bd8b55` | 1 | `2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md` |
| `19fb6b88` | 1 | `2026-07-10-artifact-filter-inversion-misleading-hint.md` |
| `1a3b6fc2` | 3 | `2026-07-07-activate-hint-home-path-not-forward-slash-normalized.md`<br>`2026-07-07-doctor-ads-colon-false-positive-windows-verbatim-prefix.md`<br>`2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md` |
| `1b40776a` | 1 | `2026-07-17-artifact-find-ignores-workspace-pin.md` |
| `1c04046b` | 1 | `2026-07-13-truncated-lsp-range-repair-test-fails-on-windows.md` |
| `1d10c072` | 1 | `2026-07-05-context-anchor-starves-neighbors.md` |
| `1d489b3d` | 1 | `2026-06-19-edit-markdown-fenced-comment-section-truncation.md` |
| `21662112` | 1 | `2026-07-09-artifact-get-heading-exact-match-only.md` |
| `23ea7e9c` | 1 | `2026-07-05-link-scan-yaml-metadata-block-swallows-headings.md` |
| `27176006` | 1 | `2026-07-17-worktree-cites-refusal-materializes-shadow-fork.md` |
| `2f9d446d` | 1 | `2026-07-06-no-default-features-build-broken.md` |
| `2fbcff80` | 1 | `2026-07-10-path-security-doc-promises-recoverable-but-bails.md` |
| `33eca3e2` | 1 | `2026-07-07-windows-glob-overview-path-separator-test-mismatch.md` |
| `3af52f1e` | 1 | `2026-07-10-read-file-buffer-refs-silently-drop-navigation-params.md` |
| `3fca32db` | 1 | `2026-07-09-edit-code-write-path-ignores-workspace-pin.md` |
| `439a9c7a` | 1 | `2026-07-05-librarian-guide-8hex-id-doc-error.md` |
| `4d13e673` | 1 | `2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md` |
| `50842163` | 1 | `2026-07-25-concurrent-index-no-project-lock.md` |
| `51f9e6fb` | 1 | `2026-07-10-lsp-shutdown-all-holds-clients-lock-across-await.md` |
| `52cc35a8` | 1 | `2026-07-09-edit-code-replace-drops-visibility-modifier.md` |
| `52fcaf01` | 2 | `2026-07-25-compose-gpu-profile-ampere-only.md`<br>`2026-07-25-reindex-reembed-noop-without-force.md` |
| `52fcaf0118d9…` (40-char; same commit, likely provenance) | 1 | `2026-07-25-coderankembed-gguf-source-404.md` |
| `5c9ba0dd` | 1 | `2026-06-12-windows-runcmd-tempfile-leak-spawn-error.md` |
| `61a800af` | 1 | `2026-07-13-artifact-update-frontmatter-null-churn.md` |
| `61a800af6570…` (40-char; same commit, likely provenance) | 1 | `2026-07-13-artifact-create-drops-topic.md` |
| `62457959` | 1 | `2026-07-07-display-audit-scope-gap-non-to-string-sites.md` |
| `7141ac6e` | 1 | `2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md` |
| `798119ad` | 1 | `2026-07-19-recoverableerror-display-doc-contradicts-code.md` |
| `83430da8` | 2 | `2026-07-10-librarian-context-silent-empty-no-embedder.md`<br>`2026-07-10-librarian-semantic-no-like-fallback-doc-drift.md` |
| `97a36905` | 1 | `2026-07-10-preview-headings-silent-cap-20.md` |
| `a5743870` | 1 | `2026-07-13-artifact-update-phantom-schema-fields.md` |
| `a92c734fde3e…` (40-char; likely provenance) | 2 | `2026-07-07-artifact-get-full-body-silent-truncation.md`<br>`2026-07-07-doctor-ads-colon-verbatim-prefix-false-positive.md` |
| `b33ad329` | 1 | `2026-07-10-monitor-bg-ref-tail-f-pipeline-error.md` |
| `c76937ee` | 1 | `2026-07-09-read-markdown-heading-not-found-quote-mismatch.md` |
| `d531ee76` | 1 | `2026-07-28-index-lock-tests-pollute-runtime-dir.md` |
| `d5ee0464` | 1 | `2026-07-19-edit-code-insert-after-lands-mid-statement.md` |
| `e68f43ae` | 1 | `2026-07-18-tree-strip-bare-root-not-stripped.md` |
| `edb44a9b` | 1 | `2026-07-07-list-overview-remaining-display-path-separator-sites.md` |
| `ef45b6e` | 1 | `2026-04-18-memory-leak-x-session-freeze.md` |

## Added after the census — 4 files (2026-07-29)

The census above is a point-in-time snapshot of the 135-file batch archive. These were
archived *later*, under the same gate, so they carry the same debt and are listed
separately rather than folded in — otherwise the "135 archived files" count stops being
true of the thing it counts.

Work them exactly like the queue rows above.

| experiments SHA | files | archived bug file(s) |
|---|---:|---|
| `3fbfbe2a` | 1 | `2026-06-14-librarian-artifact-index-port-to-qdrant.md` |
| `79cd1428`, `65440388`, `af3be4ab` | 1 | `2026-07-28-edit-code-reindent-shifts-string-literal-contents.md` — **three** SHAs: the fix, the raw-newline widening, and the live-verification fixture. All three must land or the file's own claims outrun the code. |
| `d668927e` | 1 | `2026-07-28-memory-sections-filter-matches-h3-only.md` |

And one for the no-SHA bucket below:

- `2026-07-27-reranker-gpu-tei-cuda-oom.md` — compose/`.env.gpu`/`fetch-models.sh` change
  with no Rust surface. Its `## Fix` names the changed files and records a live round-trip
  with measured VRAM, but never names a commit, so there is nothing to reconcile until
  someone identifies the commit that carried those file changes.

## No validating commit SHA in body — 41 files

These cite no token that resolves to a commit in this repo. Most are older bugs whose
Fix sections describe the change in prose, or whose SHA has already been rebased away.
Each needs a manual look before it can be called reconciled or exempt. Low urgency —
an absent SHA cannot orphan.

`2026-05-26-edit-markdown-scoped-edit-fuses-heading.md`,
`2026-05-28-path-annotation-spam.md`,
`2026-06-01-kotlin-lsp-analyzer-index-unbounded-disk.md`,
`2026-06-01-librarian-adapter-stale-is-write.md`,
`2026-06-02-markdown-chunks-collection-vestigial.md`,
`2026-06-03-get-guide-description-over-budget.md`,
`2026-06-04-call-graph-ts-arrow-const-callees.md`,
`2026-06-04-docstring-extractors-lag-symbol-coverage.md`,
`2026-06-04-go-ast-generic-receiver-name-path.md`,
`2026-06-04-kotlin-ast-drops-nested-classes.md`,
`2026-06-04-rust-ast-drops-assoc-items-macros.md`,
`2026-06-04-ts-extractor-drops-arrow-fn-consts.md`,
`2026-06-05-lsp-failed-starts-not-recorded.md`,
`2026-06-09-edit-tools-escaped-newline-multiline-friction.md`,
`2026-06-09-onboarding-prompt-uses-project-not-project-id.md`,
`2026-06-09-references-false-zero-stale-graph.md`,
`2026-06-09-windows-test-suite-preexisting-failures.md`,
`2026-06-14-get-guide-reinjects-on-mcp-restart.md`,
`2026-06-14-pika-outcome-vocab-miscalibration.md`,
`2026-06-14-progress-notifications-unsolicited-token.md`,
`2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`,
`2026-06-18-artifact-create-no-custom-frontmatter.md`,
`2026-06-23-mux-cached-capabilities-unbounded.md`,
`2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md`,
`2026-07-05-audit-doc-refs-fail-on-doc-mismatch.md`,
`2026-07-05-audit-doc-refs-lsp-stubbed-off.md`,
`2026-07-05-audit-doc-refs-scope-param-ignored.md`,
`2026-07-05-v6-migration-cascade-deletes-child-rows.md`,
`2026-07-07-orphan-cleanup-deletes-walk-excluded-existing-files.md`,
`2026-07-07-reindex-reports-success-but-catalog-find-get-empty.md`,
`2026-07-09-artifact-get-full-true-body-silent-truncation.md`,
`2026-07-10-edit-code-impl-method-selection-range-refusal.md`,
`2026-07-10-extract-toml-key-branch-order-mixed-files-unreachable.md`,
`2026-07-10-librarian-recoverable-error-downcast-never-matches.md`,
`2026-07-10-oom-blast-radius-cgroup-cap.md`,
`2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md`,
`2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`,
`2026-07-17-artifact-extra-quotes-frontmatter-scalars.md`,
`2026-07-17-edit-markdown-scoped-edit-no-crlf-tolerance.md`,
`2026-07-20-append-entry-id-drift-params-vs-body.md`,
`2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md`

## References

- `dd56423e` — the archive-gate change across five surfaces
- `docs/RELEASE.md` § *Standard Ship Sequence* step 4 — the consumer of this queue
- `docs/RELEASE.md` § *After cherry-pick: cite the master SHA* — why orphaning happens
- `get_guide("tracker-conventions")` § *Bug files* — the current archive rule
