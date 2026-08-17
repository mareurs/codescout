---
id: e04115d9477d280b
kind: bug
status: mitigated
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

**The diagnostic half is implemented; the detection gate is deliberately not.** The bug
offered the gate as the main proposal and the diagnostic as a *"cheaper alternative worth
measuring first"* — that alternative is what shipped, because it needs no
false-positive estimate and closes the actively-harmful half on its own.

`substitution_diagnostic(command, stderr)` in `src/tools/run_command/output.rs` returns a
one-line cause when the shell reports a substitution failure. Three properties, each
chosen against a way this could have gone wrong:

- **Anchored on the shell's own marker** (`command substitution:` in stderr), not on
  command shape. A command that genuinely wanted substitution and got it emits no marker,
  so this cannot fire on working substitution — which is exactly the false-positive risk
  that made the gate need measuring first.
- **Only claims a cause it can point at.** The command must also contain a backtick or
  `$(`; the marker alone (from a nested script or alias) yields nothing, because naming a
  cause not visible in the caller's own string is a guess.
- **Disowns the misleading line.** When `Argument list too long` co-occurs, the message
  says it is a CONSEQUENCE, not the cause. That line is the actual damage in the reported
  incident: plausible, self-consistent, and wrong, and acting on it means shortening the
  commit body for no benefit.

No repair is attempted, per the bug's own reasoning and
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md` — a commit is a write, and a
write never has its target guessed.

**Attached so it actually arrives.** The value rides on `result["shell_cause"]`, computed
*before* the buffer/summarise branch (one arm moves `raw_stderr` into the response) and
attached on **both** exit paths, including the `buffer_only` early return. `format_run_command`
appends it after all branch logic, in the same position and for the same reason as
`timeout_hint`: `format_compact` is what `call_content` renders, and a field that function
does not read reaches nobody — the defect filed as
`docs/issues/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`.

Status `mitigated`, not `fixed`: the substitution still happens, and a message whose
backticks *parse* is still silently mangled rather than refused. What is fixed is that a
failure now names its cause. The detection gate remains open as the root-cause fix — see
§ Resume.

Fix SHA: this commit, on `experiments`.
## Tests added

Four in `src/tools/run_command/tests.rs`, over a `SUBSTITUTION_STDERR` const holding the
reported incident's stderr verbatim — all four errors, in order, so the fixture is the
real input rather than a paraphrase of it.

| Test | Mutation it catches |
|---|---|
| `substitution_diagnostic_names_the_cause_and_disowns_the_misleading_line` | dropping the `Argument list too long` clause, or the runnable `git commit -F` correction |
| `substitution_diagnostic_is_silent_when_substitution_worked` | switching detection from the shell marker to command shape — which would fire on every backtick-bearing command, including working ones |
| `substitution_diagnostic_is_silent_when_the_command_shows_no_substitution` | claiming a cause not visible in the caller's string |
| `format_compact_surfaces_the_shell_cause_on_every_output_shape` | the boundary — asserted through **both** the short-output and buffered shapes, which render from different branches |

**Mutation-verified on the boundary test**, which is the one worth checking because a
correct field that never renders is indistinguishable from no fix at all. Removing the
`format_run_command` append turns it red with the compact render reduced to
`✗ exit 126 · 0 lines` — cause invisible, exactly the shape of the
`frontmatter_max`-dropped-at-the-boundary defect.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
4030 passed / 0 failed / 45 ignored.
## Workarounds

- `git commit -F <path>` with the message written via a quoted heredoc (`<<'EOF'`, quoted
  delimiter suppresses all expansion). This is what unblocked `92fee6a5`.
- Single-quote the whole `-m` argument.

## Resume

The diagnostic is in. What remains is the **detection gate**, and its blocker is unchanged:
measure how often a `git commit -m` in the corpus legitimately wants substitution (expected:
never) before refusing on shape. `.codescout/usage.db` `tool_calls.input_json` holds the
commands — the same source the IL-gate firing audit used.

Two notes for whoever takes it:

- It belongs beside `is_unbounded_lhs` in `src/util/path_security.rs`, not in a new module.
  `run_command`'s IL-3 gate already does this class of command-shape analysis, and
  `shell_tokens` is already trusted for gating decisions.
- The gate would catch the case this fix cannot: a message whose backticks **parse** as a
  command, so the shell substitutes successfully and silently mangles the commit body with
  no marker in stderr and no error at all. That is the more dangerous half and the
  diagnostic is blind to it by construction.
## References

- `src/platform/unix.rs:65` — the `sh -c "$cmd"` wrapper
- `src/util/path_security.rs` — `shell_tokens`, `is_unbounded_lhs`
- `docs/issues/archive/2026-05-19-run-command-eval-backtick-eof.md` — deferred hypothesis #4
- `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` — why not auto-repair
- `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` — the sibling audit of `run_command` gate shape
