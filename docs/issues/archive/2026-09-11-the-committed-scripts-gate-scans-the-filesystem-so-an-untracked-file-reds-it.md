---
id: 79b0a012716ff390
kind: bug
status: fixed
title: The committed-scripts gate scans the filesystem, so an untracked file reds it for everyone
owners:
- marius
tags:
- cluster/unclassified
topic: gate population and selector naming
closed: 2026-09-11
opened: 2026-09-11
owner: marius
severity: medium
---

## Summary

`no_committed_script_hardcodes_a_personal_home_path` (`tests/committed_paths.rs:105`) is named
for, and reports on, **committed** scripts. Its population is built by `text_files`
(`tests/committed_paths.rs:38-49`), which walks the filesystem with `std::fs::read_dir` and never
consults git. So any **untracked, in-progress** script under `scripts/` — a scratch probe another
session is mid-way through writing — reds the shared gate for every session in the checkout, with a
message telling the reader their committed scripts are broken.

The selector is **wider than its name**. That is the inverse of `IC-14`
(`guard-narrower-than-its-name`), and it is the **second** instance of the candidate class the
`cluster/unclassified` roster already named when the first one landed.

## Symptom (Effect)

Verbatim, from `cargo test --workspace`:

```
---- no_committed_script_hardcodes_a_personal_home_path stdout ----
committed scripts hardcode machine-specific home paths:
  scripts/architecture-boundary-probe.py:1210 — /home/marius

Derive the value instead — the repo root is `$(cd "$(dirname "$0")/.." && pwd)` — or take it
from an environment variable with a portable fallback. If the account is genuinely universal
(a CI runner image), add it to UNIVERSAL_ACCOUNTS with the reason.
```

`scripts/architecture-boundary-probe.py` is untracked. `git ls-files --error-unmatch` on it exits
**1**, `git status --short` reports `??`, and `git log -- <path>` is empty: the file has never been
committed. Every noun in the message is wrong about it.

## Reproduction

Measured 2026-09-11 at `git rev-parse HEAD` = `638b1e02` on `experiments`.

1. In a clean checkout with a green gate, create any untracked file under `scripts/` containing an
   absolute path beginning with a personal home directory — one line is enough.
2. `cargo test --workspace`.
3. `no_committed_script_hardcodes_a_personal_home_path` fails, naming that file.
4. `git ls-files --error-unmatch scripts/<that file>` exits 1, confirming the file the gate calls
   "committed" is not tracked.

No coordination with another session is needed to reproduce it; a peer's untracked file is simply
the way it shows up in practice.

## Environment

Linux, codescout `experiments`, shared checkout with several concurrent Claude sessions. The
observed instance belongs to another session, which is incidental to the defect.

## Root cause

```
tests/committed_paths.rs:33-35   scripts_dir() -> CARGO_MANIFEST_DIR + "/scripts"
tests/committed_paths.rs:38-49   text_files() -> std::fs::read_dir, recursive
tests/committed_paths.rs:105     fn no_committed_script_hardcodes_a_personal_home_path
tests/committed_paths.rs:109     "committed scripts hardcode machine-specific home paths"
```

`text_files` is a filesystem walk with one filter, `SKIP_DIRS` (`__pycache__`, `node_modules`).
Nothing in the chain asks git whether a file is tracked, so the population is *"every text file on
disk under `scripts/`"* while the test name and message both claim *"committed"*.

Measured 2026-09-11 by reading both symbols and running `git ls-files --error-unmatch` against the
named file; not inferred from the failure text.

## Evidence

### The named file is untracked

```
$ git ls-files --error-unmatch scripts/architecture-boundary-probe.py; echo $?
1
$ git status --short -- scripts/architecture-boundary-probe.py
?? scripts/architecture-boundary-probe.py
$ git log -2 -- scripts/architecture-boundary-probe.py
(no output)
```

### Line drift across three runs, minutes apart

| run | reported line |
|---|---|
| `cargo test --workspace --no-default-features` | `:1115` |
| `cargo test --workspace` | `:1128` |
| `cargo test --workspace --no-fail-fast` | `:1210` |

Three readings of one citation, disagreeing, in one gate pass. This is the line-drift tell: the
**corpus moved** — the file is being actively written — rather than any reading being wrong. Worth
recording because the natural first reading of three disagreeing line numbers is that the
instrument is broken.

### It is the only failure in the workspace

`cargo test --workspace --no-fail-fast`: **5767 passed, 1 failed, 52 ignored.** Without
`--no-fail-fast` the run aborts at this test and never reaches `tests/symbol_lsp.rs`, so an
unrelated session's scratch file can hide the results of every integration target ordered after
`committed_paths`.

## Hypotheses tried

1. **Hypothesis:** the file was committed by a peer between my runs, making the gate correct.
   **Test:** `git ls-files --error-unmatch`, `git status --short`, `git log -- <path>`.
   **Verdict:** rejected — untracked on all three, never committed.

2. **Hypothesis:** the three different line numbers mean one of the runs misreported.
   **Test:** compared the readings against the file's tracked status and its growth.
   **Verdict:** rejected — the numbers increase monotonically and the file is being written; the
   corpus moved. Recorded because the competing reading ("the instrument is broken") sends the
   reader to debug a working tool.

## Fix

**Fixed on `experiments` at `0e01d43d`**, patch-id `96322830a5ca2467e85f06879d9dad7cb3dd5448`.
Recorded as a pair because they fail differently: the SHA is positional and dies when
`experiments` is rebased, the patch-id is a content hash of the diff and survives rebase and
cherry-pick alike.

The population is now `git ls-files -- scripts`, and the test is renamed
`no_tracked_script_hardcodes_a_personal_home_path`. Name, message and population agree.

**Narrowing loses no coverage**, which is the objection this fix turns on. `git ls-files` reports
INDEX entries, so a file enters the population the moment it is `git add`-ed — strictly before any
commit exists. A hardcoded home path is still caught before it can be committed; what is excluded
is exactly the file that is not in the repo at all.

The message now states that out loud — *"An untracked file is not scanned, so a scratch script in
your working tree cannot be the cause"* — because the old one sent the reader to look for a
tracked-file problem that did not exist. That half is the misdirection, and it cost more than the
false positive.

The option NOT taken, recorded because it is defensible: widening the name to match the filesystem
population. Rejected because it keeps a shared gate red for everyone whenever anyone has a scratch
script open, and the `git add` timing above means the narrow form gives that up for nothing.
## Tests added

`tests/committed_paths.rs`:

- `an_untracked_script_is_excluded_and_a_tracked_one_is_not` — the fix, tested directly.
- `the_tracked_population_is_not_vacuous` — the guard on the guard.

Three mutations, three observed REDs:

- disable the tracked filter → `an_untracked_script_is_excluded…` RED, **and**
  `no_tracked_script…` reds on the same untracked file as before the fix. The original bug,
  reproduced on the same tree, which is what makes the before/after a measurement rather than a
  claim — the offending untracked file was never removed.
- typo the `ls-files` pathspec → `the_tracked_population_is_not_vacuous` RED, **while the main gate
  PASSES**. `git ls-files` exits 0 with empty output on a wrong cwd, a pathspec typo, or an
  unreadable repo, and none of those is an error at the call site — so one wrong word silently
  disarms the whole gate while showing green. That is the entire reason the vacuity guard exists.

The exclusion fixture writes the **same offending line** into both files, tracked and untracked, so
tracked-ness is the only variable; differing content would let the test pass while no longer
discriminating. It runs against a tempdir rather than `scripts/` because creating a real untracked
file there to test the exclusion would place it in every concurrent session's `git status` — the
exact cost this filter removes.

Verified by name in both gate lanes, not from either lane's total. Gate green: fmt 0, clippy 0,
lean 3751, default 5772.
## Workarounds

Run `cargo test --workspace --no-fail-fast` to see past it: the abort is what makes this expensive,
because it hides every integration target ordered after `committed_paths`. The failure itself is
safe to disregard **once you have confirmed the named file is untracked** — `git ls-files
--error-unmatch <path>` exiting 1 is the check, and it takes one command.

Do not "fix" the named file if it is not yours. It is another session's work in progress.

## Resume

Decide between narrowing the population and widening the name (both sketched under **Fix**), then
make the test's name, its message, and its population agree. Whichever is chosen, the message
should name what it scanned, so the next reader is not sent to look for a committed file that does
not exist.

## References

- First instance of the same candidate class, filed under the same escape hatch:
  `docs/issues/archive/2026-09-03-worktree-guard-word-boundary-blocks-read-only-git-plumbing.md` —
  `git-worktree-guard.mjs` ends each destructive verb with `\b`, so `merge\b` matches
  `git merge-base` and six read-only plumbing commands are refused as destructive mutations.
- `docs/trackers/issue-clusters.md` — `IC-14` (`guard-narrower-than-its-name`) is this class's
  exact inverse, and `IC-18` (`selector-narrower-than-its-population`) is the inverse of the
  selector half. Both run the opposite direction: they MISS work, this one REFUSES it.
