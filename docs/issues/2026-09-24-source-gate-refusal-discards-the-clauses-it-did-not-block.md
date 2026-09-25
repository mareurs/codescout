---
status: open
opened: 2026-09-24
closed:
severity: low
owner: marius
related: []
tags: [cluster/hint-composed-without-the-request]
kind: bug
---

# BUG: the source-file gate's refusal discards the clauses it did not block

## Summary
When one clause of a compound command reads project source, `run_command` refuses the whole
command (correct — see below) and reports how many other clauses did not run, but hands back
nothing to re-run. The caller reconstructs the batch by hand.

## Symptom (Effect)
```
echo one; ls src | head -2; sed -n '1,3p' src/lib.rs; echo three
→ shell access to source files is blocked
  offending clause: `sed -n '1,3p' src/lib.rs` (4 other clauses in this command were not run).
```

## Reproduction
Live binary at `506924f2`; `run_command` the command above.

## Root cause
`check_source_file_access` (`src/util/path_security.rs:1686`) stops at the first offender and
composes its hint from that clause alone. The refuse-in-full contract is deliberate and stays
(comment at the clause-note site: running the permitted clauses "is a worse contract — the
caller could not tell which side effects happened").
measured 2026-09-24: the refusal above.

**Rewriting is only sound for `;`/newline chains.** The splitter drops separators, and
re-joining `test -f x && rm y` with `;` would make the `rm` unconditional. A heredoc body is
stripped before the split, so a rewrite built from the stripped text would lose it.

## Fix

`check_source_file_access` (`src/util/path_security.rs`) now collects **every** offending run
(it used to `break` at the first), and `rerun_without_offenders` appends
`Re-run the rest as: \`<command minus offending runs>\`.` to the refusal — or, when a rewrite
would not be sound, a `No rerun offered: <reason>.` line naming why:

- a heredoc anywhere in the ORIGINAL command (the body is stripped before the split);
- `&&` / `||` between clauses (dropping one changes which of the others run);
- a run starting with a compound keyword, or holding unquoted `( ) { }` or a backtick
  (`has_unquoted_grouping`; quoted text is skipped so `awk '{print $1}'` keeps the rerun).

A stage inside a pipeline takes its whole run with it. The refuse-in-full contract is unchanged.

Fix SHA / patch-id: _recorded at commit time_.
## Tests added

Eight `source_gate_*` tests in `src/util/path_security.rs`, one per row of the rewrite table,
plus the quoted-braces case that guards the false-refusal direction. Mutation results are
recorded in the commit message.

Writing the compound-construct fixture exposed a separate pre-existing bypass — a run that
STARTS with `do`/`then`/`(` is not blocked at all — filed as
`docs/issues/2026-09-24-source-gate-is-bypassed-by-a-keyword-or-group-prefix.md`. The fixtures
use shapes that are blocked today.
## Workarounds
Re-type the permitted clauses.

## Resume

N/A once committed. Tag through the catalog after merge.
## References
- Source report: `codescout-lessons.md` § 6.3.
