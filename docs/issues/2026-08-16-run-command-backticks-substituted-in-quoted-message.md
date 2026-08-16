---
id: e04115d9477d280b
kind: bug
status: open
title: 'BUG: run_command passes the command to `sh -c` verbatim, so backticks in a commit message are substituted — and the diagnostic names the wrong cause'
owners:
- marius
tags:
- run_command
- shell
- misleading-error
- commit-workflow
- repair-and-continue
---

---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related: [2026-05-19-run-command-eval-backtick-eof.md]
tags: [run_command, shell, misleading-error, commit-workflow]
kind: bug
---

# BUG: `run_command` passes the command to `sh -c` verbatim, so backticks in a quoted commit message are substituted — and the diagnostic names the wrong cause

## Summary

`run_command` wraps its input as `sh -c "$cmd"` (`src/platform/unix.rs:65`). A command of
the shape `git commit -m "…"` whose message contains backticks — the normal shape of a
commit message in this repo, where the house style cites symbols and paths in backticks —
has those backticks evaluated as **command substitution** before `git` ever runs.

The failure does not say so. It surfaces as three unrelated shell errors and a fourth,
actively misleading one.

## Symptom (Effect)

Observed 2026-08-16 while committing the `GF-N` tracker. The command was a normal
conventional-commit message citing `is_unbounded_lhs`, `UNBOUNDED_PREFIXES` and
`self.call(...).await?` in backticks:

```
exit_code: 126
Usage: grep [OPTION]... PATTERNS [FILE]...
Try 'grep --help' for more information.
sh: command substitution: line 1: syntax error near unexpected token `...'
sh: line 1: `self.call(...).await?'
sh: line 1: ?: command not found
sh: line 1: /usr/bin/git: Argument list too long
```

Four errors, none of which is *"your commit message contained backticks and the shell
substituted them."* The reader must reconstruct that from `sh: command substitution:`
buried in the middle.

**The last line is the actively harmful one.** `Argument list too long` is a plausible,
self-consistent explanation on its own — the message *was* long — and acting on it means
shortening the commit body, which fixes nothing and loses content. That is the
misleading-diagnostic class this repo already treats as a defect in its own right (cf.
`docs/issues/archive/2026-06-04-edit-file-old-string-miss-no-closest-match.md`: a bare
"not found" is a defect *because the tool holds the information needed to help*).

## Reproduction

```bash
run_command(command: 'git commit -m "fix: teach `my_fn` the new shape"')
```

The backticked `my_fn` is executed as a command. With unbalanced backticks the shell
reports `unexpected EOF while looking for matching \``; with balanced ones it silently
substitutes, which is worse — the commit may succeed with a mangled message.

Minimal, non-mutating probe:

```bash
run_command(command: 'echo "a `date` b"')      # substitutes
run_command(command: "echo 'a \`date\` b'")    # literal
```

## Environment

- Linux, `sh -c` wrapper (`src/platform/unix.rs:65`)
- codescout MCP server, release build, any client
- Reproduced live during commit of `92fee6a5`

## Root cause

Two independent contributors:

1. **No shape analysis on the command string.** `run_command` hands the string to the
   shell unexamined. Backticks inside a `-m` / `--message` argument are, in practice,
   never intended as substitution — but nothing looks.
2. **The house style manufactures the trap.** This project's commit convention (memory
   `conventions`) asks for symbol and path citations, and effectively every commit in the
   repo uses backticks for them. The single most common multi-line `run_command` in this
   workflow is the one most likely to contain the hazard.

`shell_tokens` already exists in `src/util/path_security.rs` and is used by
`is_unbounded_lhs` to reason about command shape — the substrate for detection is present
and already trusted for gating decisions.

## Evidence

- Live failure above, exit 126, during this session.
- `docs/issues/archive/2026-05-19-run-command-eval-backtick-eof.md` § Hypotheses tried #4
  is still **deferred**: *"an earlier `run_command` call wrapped its command in a way that
  bash saw an unbalanced backtick … need to identify the originating call."* This incident
  is a live instance of that mechanism, though not necessarily the one that produced the
  buffer bytes in that report (that error string contained `eval:`, which codescout's
  wrapper never emits — so treat the link as plausible, not established).

## Hypotheses tried

1. **Hypothesis:** the message really was too long (`Argument list too long`).
   **Test:** re-sent byte-identical content via `git commit -F <file>`.
   **Verdict:** rejected — committed cleanly as `92fee6a5`. The length was never the
   problem; the substitution expanded the argv.

## Fix

**Do not auto-repair.** Escaping backticks silently would break legitimate command
substitution, and the repair-and-continue law (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`)
sets a higher bar for writes: accept an explicit target, never guess one. A commit is a write.

Proposed: a narrow, high-precision **detection + teaching refusal** —

- Fire when a `-m` / `--message` argument (or any double-quoted argv element of a `git
  commit`) contains an unescaped `` ` `` or `$(`.
- Return `RecoverableError` naming the cause and both runnable corrections:
  - `git commit -F <file>` — write the message to a file first;
  - single-quote the message, escaping inner single quotes.

Cheaper alternative worth measuring first: leave the gate alone and fix only the
**diagnostic**, by detecting the `sh: command substitution:` marker in stderr and prefixing
a one-line cause. That closes the misleading-`Argument list too long` half at near-zero risk.

## Tests added

None yet.

## Workarounds

- `git commit -F <path>` with the message written via a quoted heredoc (`<<'EOF'`, quoted
  delimiter suppresses all expansion). This is what unblocked `92fee6a5`.
- Single-quote the whole `-m` argument.

## Resume

Undecided between the two fixes above. The diagnostic-only fix is cheap and strictly
positive; the detection gate needs a false-positive estimate first — measure how often a
`git commit -m` in the corpus legitimately wants substitution (expected: never).

Note `run_command`'s IL-3 gate already does exactly this kind of command-shape analysis, so
if the detection route is taken it belongs beside `is_unbounded_lhs`, not in a new module —
see `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` (GF-N), which is auditing that
same function's firing correctness.

## References

- `src/platform/unix.rs:65` — the `sh -c "$cmd"` wrapper
- `src/util/path_security.rs` — `shell_tokens`, `is_unbounded_lhs`
- `docs/issues/archive/2026-05-19-run-command-eval-backtick-eof.md` — deferred hypothesis #4
- `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` — why not auto-repair
- `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` — the sibling audit of `run_command` gate shape

