---
status: open
opened: 2026-08-15
closed:
severity: low
owner: marius
related: []
tags: [run_command, path-security, friction, external-report]
kind: bug
---

# BUG: read-only metadata commands (`wc`, `ls`, `stat`) are blocked on source paths with no carve-out

## Summary

`run_command("wc -lc src/foo.rs")` is refused by the source-path gate. `wc`, `ls` and
`stat` return pure metadata and cannot disclose more than `symbols` already does, so
the block buys no safety while nudging the agent toward *guessing* file size rather
than measuring it. Filed as a decision — carve out read-only metadata, or declare the
blanket block intentional and say so in the guide.

## Symptom (Effect)

```
run_command("wc -lc src/…")
→ shell access to source files is blocked
```

Reproduced this session with a different command in the same family:

```
run_command("ls -1 src/tools/read_file.rs … && sed -n '322p' src/tools/core/types.rs")
→ shell access to source files is blocked
   hint: use read_file(path, start_line, end_line), symbols(path),
   symbols(name=..., include_body=true), or grep(regex) instead.
   Re-run with acknowledge_risk: true if you need raw shell access.
```

## Reproduction

Any `run_command` naming a source path, at `821f9d0d`. The `acknowledge_risk: true`
override works, so this is friction, not a wall.

## Environment

Reported on macOS against `experiments @ d7988aca`; reproduced on Linux at `821f9d0d`.

## Root cause

**Working as coded — the reporter's diagnosis was wrong, and that is itself part of the
finding.** He concluded `wc` was "in the block list". There is no command list.
`src/tools/run_command/inner.rs:305-315` calls
`crate::util::path_security::check_source_file_access(resolved_command)`, a **path**
predicate: any resolved command naming a source file is refused regardless of which
binary it invokes or how bounded its output is.

He was led to the command-list model by the guide, which claims a bounded-file
exception that does not exist — filed separately as
`docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`.

*Measured 2026-08-15: the `ls`/`sed` command above was executed against the live server
and returned the quoted error.*

## Evidence

### The cost is a wrong mental model, not a blocked task

The override exists and works. What the gate actually cost here was an outside user
spending part of an investigation on a "block list" that does not exist, and writing a
design section (his D3) against the wrong mechanism.

### The nudge-to-guess concern

Both the reporter and the gate's own hint route the agent to `symbols` / `read_file`.
Neither answers "how many bytes is this file", so the practical effect is an agent that
estimates size instead of measuring it — the exact habit
`docs/issues/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md`
shows going wrong.

## Hypotheses tried

1. **Hypothesis:** `wc` is specifically enumerated in a blocklist (the reporter's
   model). **Test:** read `src/tools/run_command/inner.rs` Step 2.5 and look for a
   command allowlist/blocklist. **Verdict:** rejected — the predicate is
   `check_source_file_access(resolved_command)`, path-based, with no per-command branch.

## Fix

Not yet decided. Two coherent outcomes:

- **A — carve out read-only metadata.** Permit a small allowlist (`wc`, `ls`, `stat`,
  `du`, `file`) when the command neither reads content to stdout nor writes. Narrow,
  and it removes the guess-instead-of-measure nudge.
- **B — declare the blanket block intentional.** Keep the gate, and fix the guide to
  stop implying an exception. Close this `wontfix` with that rationale.

**B is the minimum**, because the guide is wrong either way. A is optional on top.

## Tests added

N/A — no code change until the decision is made. Option A would need a test asserting
`wc` on a source path succeeds while `cat` on the same path is still refused.

## Workarounds

`run_command(..., acknowledge_risk=true)` — the reporter confirmed this executes `wc`
successfully. Or read `line_count` from a `read_file` summary, which reports it without
shell access.

## Resume

Decide A or B. If B: close this `wontfix` and fold the rationale into the guide edit
tracked by `docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`.
If A: add the metadata allowlist inside `check_source_file_access`
(`src/util/path_security.rs`), called from `src/tools/run_command/inner.rs:305-315`.

## References

- `docs/trackers/bistriceanu/index.md` § B-6
- `docs/trackers/bistriceanu/full-read-fidelity-design.md` § D3 — the reporter's writeup, against the wrong mechanism
