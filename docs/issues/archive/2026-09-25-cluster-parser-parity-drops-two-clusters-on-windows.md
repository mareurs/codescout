---
id: 8efcf7dba0d8a327
kind: bug
status: archived
title: 'BUG: on Windows the cluster-parser parity test finds guard-narrower-than-its-name and repro-env-diverges-from-gate-env missing from one side'
owners:
- marius
tags:
- cluster/unclassified
claimed_at: 2026-09-25
claimed_by: ebf651ec-5ab7-42d9-a526-dcf9758692e1
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

**Measured 2026-09-25, in three steps that close the chain.**

1. **The cluster files always take the encoding-free fallback, on every platform.** A run with `subprocess.run` forced to cp1252 wherever `text=True` gives no `encoding=` crashed at `read_ledger` → `read()` → the `git show :path` fallback. So class files are not in `_prime_index`'s cache: its one caller, the member-count loop, primes `bug_files()` only, and every class file reaches the `text=True` fallback.
2. **Exactly the two missing clusters are the only files that decode differently.** Scanning every class file's index bytes for the five bytes cp1252 leaves undefined (`0x81 0x8D 0x8F 0x90 0x9D`): 2 of 24 contain one, `IC-14-guard-narrower-than-its-name.md` (`0x9d`, strict decode fails at byte 27720) and `IC-5-repro-env-diverges-from-gate-env.md` (`0x9d`, at 16265). Those are precisely the two slugs Windows drops.
3. **Why Windows exits 0 where Linux would crash.** This is read from CPython's own `subprocess.py` (3.14, whose `_mswindows` branch ships in the same file). On Windows, `text=True` wraps the pipe in a `TextIOWrapper` using the locale encoding. `communicate()` reads it in `_readerthread` (`buffer.append(fh.read())`), and a decode error raised there is printed by the thread excepthook, never re-raised. `_communicate` then returns `stdout = stdout[0] if stdout else None`, so `None`, while `git` itself exited 0. `read()` returns `r.stdout`, which is `None`, and `read_ledger` skips a `None` without a word. The class text for IC-14 and IC-5 never enters the ledger, so their slugs are not valid and neither is counted. On POSIX the decode runs in the calling thread and raises, which is why Linux shows nothing: its locale is UTF-8 and never reaches this.

Step 3 is established by reading the interpreter, not by running Windows Python. The CI result after the fix is the confirmation owed.

The defect is therefore two things: a locale-dependent decode, and a read path that turns "could not decode" into "absent" silently.

## Fix

`scripts/pre-commit-ledger-counts.py`: a new `_run_git(args)` captures bytes and decodes them in the calling thread with `"utf-8", "replace"`, the same call `_prime_index` makes, so the fallback decodes exactly what the fast path would. It replaces every `text=True` call: `_git()` (path lists), `read()`'s `HEAD:` and index fallbacks (file bodies, the defect), and `_corpus_paths_diverged` (path list, refusal path only). `stdout` can no longer be `None`, so the silent "absent" arm cannot be reached by a decode failure.

On Linux the change is behaviour-neutral. The committed script and the fixed one, run back-to-back on the same tree, give byte-identical `--json` in `index`, `head` and `worktree` modes. The cp1252 simulation that crashed before the fix gives the same output as a normal run after it.

Test: `tests/issue_clusters.rs` `the_hook_script_decodes_git_output_itself_never_through_the_locale` runs the script in all three modes under a guard that refuses any text-mode `Popen`. It asserts the invariant, so it holds or fails on every platform and does not depend on the corpus containing a bad byte.

## Resume

**Mutation results, 2026-09-25**, through `scripts/mutation-probe.sh` against `the_hook_script_decodes_git_output_itself_never_through_the_locale`:

- P1, `_run_git` back to `text=True` (the whole pre-fix behaviour, so this is the observed red): **KILLED**. `--source=index` refused `git show :docs/trackers/issue-clusters.md`.
- P2, the `read()` `HEAD:` fallback restored: **KILLED**, in `--source=head`.
- P3, the `read()` index fallback restored (the site the Windows bug went through): **KILLED**.
- P4, the `_git()` text-mode call restored: **KILLED**, on `git ls-files docs/trackers/issue-clusters`.
- P5, the `_corpus_paths_diverged` text-mode call restored: **SURVIVED**, because a `--json` run never reaches it. That function runs only when the hook refuses, to annotate the refusal. It is changed for uniformity, and it is the lowest-risk of the five: it decodes a `git diff --name-only` path list, and git quotes non-ASCII paths by default (`core.quotepath`). No test covers it.

**Confirmed on CI, 2026-09-25, run `36096588131` (head `ffff1dfb`, which contains `23027269`).** In `Test (windows-latest / no-features)` (job `107950021793`), `the_hook_script_agrees_on_the_cluster_parsers ... ok` and `the_hook_script_decodes_git_output_itself_never_through_the_locale ... ok`, with no failing test result in the job. That was the job's first green since 2026-09-17. Root cause step 3, read from the interpreter, is thereby confirmed by the platform it describes.

## Fix provenance

- **SHA:** `23027269` (on `experiments`). Positional, so it does not survive a rebase of `experiments`.
- **patch-id:** `9c6a6924f64741365c07f18406b6a68a304e712b`. A content hash of the diff, so it survives rebase and cherry-pick.

`fix(ledger-counts): decode git output as UTF-8 in-thread, never via text=True, so Windows stops dropping two clusters`

**Archived 2026-09-25**, after CI run `36096588131` ran `23027269` with `the_hook_script_agrees_on_the_cluster_parsers` green on native `windows-latest / no-features` (see Resume). The id citations in `scripts/pre-commit-ledger-counts.py`, `tests/issue_clusters.rs` and the archived references bug were re-pointed in the same commit.
