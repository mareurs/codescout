---
id: '4b2b061b1b7a5bc1'
kind: bug
status: open
title: The committed-scripts gate scans the filesystem, so an untracked file reds it for everyone
owners:
- marius
tags:
- cluster/unclassified
topic: gate population and selector naming
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

Not attempted — the choice is a judgement the owner should make, and both options are one-liners:

- **Filter the population by tracked status**, which makes the name true: keep the `read_dir` walk
  and intersect it with `git ls-files`, or drive the walk from `git ls-files` directly.
- **Or widen the name and message** to match the population — *"scripts under `scripts/`"* rather
  than *"committed scripts"* — if scanning untracked files is wanted, on the argument that a
  hardcoded home path is worth catching before it is committed rather than after.

They are not equivalent and the difference is the point. The second keeps a shared gate red for
everyone whenever anyone has a scratch script open, which is the cost actually being paid today.
The first cannot catch a bad path until it is staged. A third option — walk untracked files but
report them at a lower severity, or name them as untracked in the message — costs more code and
removes the misdirection, which is the half that hurts most.

## Tests added

None; no fix was made. The reproduction above needs no fixture beyond one untracked file and is
deterministic.

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
