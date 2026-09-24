---
id: '05959bffb7b4fd7d'
kind: bug
status: open
title: 'BUG: on Windows the cluster-parser parity test finds guard-narrower-than-its-name and repro-env-diverges-from-gate-env missing from one side'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-25
severity: medium
---

# BUG: on Windows, `the_hook_script_agrees_on_the_cluster_parsers` finds two clusters missing from one side

## Summary

`tests/issue_clusters.rs`'s parity test compares the cluster counts derived by the Rust gate with those derived by `scripts/pre-commit-ledger-counts.py`. It fails on the native Windows lanes because **two whole clusters are absent from one side**, while every other count agrees. It passes on Linux and macOS. Not diagnosed.

## Symptom (Effect)

Run `36058985076` (head `87001799`), in `Test (windows-latest / no-features)` (job `107832995378`) and `Test (windows-latest / local-embed)` (job `107832995190`):

```
panicked at tests\issue_clusters.rs:1846:9:
assertion `left == right` failed: `actual` disagrees between this gate and scripts/pre-commit-ledger-counts.py.
Reproduce: python3 scripts/pre-commit-ledger-counts.py --source=index --json
```

`left` holds `"guard-narrower-than-its-name": 59` and `"repro-env-diverges-from-gate-env": 27`, and `right` has neither key. The other 23 keys are identical, including `unclassified: 51`. **`left` is Rust and `right` is the Python script**, read from `tests/issue_clusters.rs:1846`: `assert_eq!(mine, theirs, …)`, where `mine` is `actual_counts(&valid)` and `theirs` is the script's `--json` output. So **the Python side drops the two clusters**. The reading was supplied by session `938e2953-de0e-4241-a543-9b761a70326a` and checked against the file.

## Reproduction

Native `windows-latest` CI lanes. It is not in the wine lane's results, because that lane runs `--lib` only.

## History

- Observed at `ea972b40` (run `36041448098`, 2026-09-24T18:28Z), where it was that job's only failure. So it predates `1ea1d36b`'s references test, and it is **not** caused by the roster edits in `0ac67b35`/`a13845ad`.
- Between it and `87001799`, native `no-features` stopped earlier at the lib failure of `fed1c5731a623c30`, and `cargo test` does not run integration targets after a lib failure. So it was hidden rather than absent.
- The job was last green at `baaafe0a` (2026-09-17). The red at `a31ed03a` (09-18) and `f918548c` (09-24) was a different lib test, `tools::core::tests::guard_worktree_write_hint_names_the_main_repo_not_an_arbitrary_worktree`, so this test's first failing commit is **not established**.

## Root cause

Unknown. The shape is two clusters missing whole, not miscounted, which points at a whole class file or section that one parser does not read on Windows.

**Candidate, not verified:** in `scripts/pre-commit-ledger-counts.py`, `_git()` (`:113`) and `read()`'s per-file fallbacks (`git show HEAD:path` at `:262`, `git show :path` at `:267`) call `subprocess.run(…, capture_output=True, text=True)` with no `encoding=`. On Windows that decodes with the locale codepage (cp1252), not UTF-8. The worktree `open()` at `:270` passes `encoding="utf-8"` explicitly. The absence of `encoding=` at those three lines is confirmed. Whether either fallback is reached for these two slugs, and whether their files hold bytes that cp1252 decodes differently, is **not** known. Candidate raised by session `938e2953`.

## Resume

Unowned. Test the candidate in Root cause first. Run `python3 scripts/pre-commit-ledger-counts.py --source=index --json` with `PYTHONUTF8=0` and a cp1252 locale (or on Windows), and see whether passing `encoding="utf-8"` at `:113`/`:262`/`:267` restores the two slugs.
