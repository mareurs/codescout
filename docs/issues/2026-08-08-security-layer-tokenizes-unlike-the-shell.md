---
status: open
opened: 2026-08-08
closed:
severity: medium
owner: marius
related: []
tags: [security, run_command, windows, tokenizer]
kind: bug
---

# BUG: the dangerous-command gate tokenizes with `split_whitespace`, not with the shell's rules — and the tokenizer written for that job has no callers

## Summary

`run_command` now executes every command through a POSIX shell on both platforms
(`sh -c` on Unix, Git Bash `bash -c` on Windows, since WIN-32). The safety layer that
decides whether a command is dangerous still reads it with `split_whitespace()`. The
POSIX tokenizer written to close that gap — `crate::platform::posix_tokenize`, reached
through `platform::shell_tokenize` — has **zero production call sites**. So the layer
that judges a command and the shell that runs it can disagree about where the words are.

## Symptom (Effect)

No user-visible failure is filed against this. It is a latent divergence, and it is
recorded because the code asserted the opposite. `src/platform/mod.rs` carried:

```
/// One tokenizer here is load-bearing rather than tidy: this feeds the security layer's
/// dangerous-command and pipeline checks, so tokenizing with rules the executing shell
/// does not use would let a command read as safe here and then behave differently when
/// it actually runs.
```

That comment named the hazard precisely and claimed it was handled. It was not.

## Reproduction

```
git grep -n 'shell_tokenize\|posix_tokenize' -- 'src/**/*.rs'
```

Every hit is a definition, a delegation (`unix.rs` / `windows.rs` → `super::posix_tokenize`),
or one of the tokenizer's own 6 tests. Nothing calls `platform::shell_tokenize`.
Then read `is_dangerous_command` (`src/util/path_security.rs`): `split_whitespace()`.

## Environment

codescout 0.15.0, branch `fix/windows-paths-and-doctor` (PR #10) and `experiments` alike —
the divergence predates the PR on Unix; the PR makes it live on Windows by replacing
`cmd.exe` with bash.

## Root cause

Two independent readers of the same string, neither aware of the other:

- `src/util/path_security.rs` — `is_dangerous_command` and its helpers split on
  whitespace and run substring regexes over the raw command.
- `src/platform/mod.rs` — `posix_tokenize` implements quote/escape-aware splitting,
  and `shell_tokenize` exposes it per-platform. Nothing consumes either.

**measured 2026-08-08:** `git grep -n 'shell_tokenize\|posix_tokenize'` over
`refs/pull/10/merge` returned definitions, two delegations and 6 tests — no production
call site. Confirmed independently by two reviewers in the PR #10 review.

## Evidence

### The gate itself is NOT trivially evadable

Worth recording so a future session does not over-scope the fix: `is_dangerous_command`
matches substrings over the **raw** command, so `echo $(rm -rf x)` still trips it.
Command substitution does not hide a dangerous verb. The divergence is about word
boundaries, not about hiding text.

### Where it bites

`split_whitespace` sees `"a b"` as two words and `a\ b` as two words; bash sees one in
both cases. A path-shaped or verb-shaped token that the shell assembles from quoted
fragments is not the same token the gate inspected.

## Hypotheses tried

1. **Hypothesis:** `shell_tokenize` is called somewhere behind a trait or macro.
   **Test:** `git grep` across all of `src/`, plus a read of `path_security.rs`'s
   parsing helpers. **Verdict:** rejected — the only callers are its own tests.

## Fix

Two options, and the choice is a real one:

1. **Wire it in.** Replace `split_whitespace()` in `is_dangerous_command` and the
   pipeline checks with `platform::shell_tokenize`. This is a behaviour change to a
   security gate — it will reclassify some commands in both directions, and needs its
   own test pass. This is the fix the deleted comment claimed had already happened.
2. **Delete the tokenizer.** If the gate is deliberately a coarse substring matcher,
   `posix_tokenize` is 60 lines of dead code with 6 tests guarding nothing, and keeping
   it invites the same false comment to be written again.

Not attempted here. This file exists because option (1) was *documented* as done, and a
reviewer reading that comment would stop looking. The comment now states the truth and
points here.

## Tests added

None — no behaviour changed. The 6 existing `posix_tokenize` tests are correct about
the function and say nothing about whether anything calls it, which is the defect.

## Workarounds

None needed; no known exploit. `acknowledge_risk` remains the escape hatch for a command
the gate wrongly blocks, and the gate's substring matching means the common dangerous
verbs are still caught regardless of word boundaries.

## Resume

Decide between Fix option 1 and 2. If 1: start at `is_dangerous_command`
(`src/util/path_security.rs`), enumerate its `split_whitespace()` call sites, and write
the divergence tests FIRST — a quoted-path command that the two tokenizers classify
differently — so the change is observable. If 2: delete `platform::shell_tokenize`,
`unix::shell_tokenize`, `windows::shell_tokenize`, `posix_tokenize` and its tests in one
commit, and say in the message that the security layer is deliberately substring-based.

## References

- `src/platform/mod.rs` — `posix_tokenize`, and the corrected comment pointing here
- `src/util/path_security.rs` — `is_dangerous_command`
- PR #10 review, 2026-08-08 — found independently by the platform/security and
  test-rigor reviewers
- `docs/issues/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` — sibling defect in
  the same layer: the `is_buffer_only` path heuristic, which gates this check
