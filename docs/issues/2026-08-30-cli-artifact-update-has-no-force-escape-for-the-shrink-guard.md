---
id: '6c2713ed241381c5'
kind: bug
status: open
title: 'BUG: the CLI''s `artifact update` has no `--force`, so a caller who hits the body-shrink guard has no escape hatch'
owners:
- marius
tags:
- cli
- librarian
- shrink-guard
- parity
closed: null
opened: 2026-08-30
severity: medium
---

## Summary

The MCP `artifact(action="update")` tool accepts `force: true` to bypass the
body-shrink guard. The `codescout artifact update` CLI subcommand does not
expose it at all. A CLI caller whose write is refused — including a legitimate,
intentional shrink such as a full rewrite or archiving stale sections — has no
way to proceed through the CLI.

## Symptom (Effect)

```
$ codescout artifact update <id> --body @/tmp/truncated.md
Error: body-shrink guard: write to <path> would reduce 7108 → 6118 bytes (14%)
and 108 → 18 lines (84%) — over the threshold on lines (hint: ... If the
shrinkage is intentional (e.g. archiving stale sections, full rewrite), re-call
with force=true.)

$ codescout artifact update <id> --body @/tmp/truncated.md --force
Usage: codescout artifact update --body <BODY> <ID>
For more information, try '--help'.
```

**The hint names a remedy the surface it was printed on does not have.** That is
the sharp edge: the error text is correct for the MCP tool and actively
misleading on the CLI.

`codescout artifact update --help` lists `--title`, `--status`, `--owners`,
`--tags`, `--topic`, `--body`, `--patch-params`, `--commit-refresh`,
`--project`, `--json`, `--no-color`. No `--force`.

## Reproduction

Measured 2026-08-30 against the live binary at `a65598be`.

1. `artifact(action="create", kind="spec", rel_path="docs/…/tmp.md", body="seed")`
2. Write a 100-line body whose first 10 lines are 600 chars and whose last 90 are
   10 chars: `codescout artifact update <id> --body @full.md`
3. Write back only the 10 fat lines: `codescout artifact update <id> --body @trunc.md`
   → refused (86% of bytes kept, 10% of lines kept — the line arm fires)
4. Retry with `--force` → clap usage error, not a forced write

## Environment

Linux, `experiments`, codescout 0.15.0, CLI subcommand `artifact update`.

## Root cause

Not yet read at the source. **Inferred from the CLI's `--help` output and not
confirmed in code**: the clap `Args` struct behind `artifact update` has no
`force` field, so the value never reaches the shared update path that already
honours it. The MCP tool's schema does declare `force`
(`src/librarian/tools/update.rs`, the `Args` struct's `force: bool`), and the
guard reads `a.force`, so the enforcement layer is surface-agnostic — only the
CLI's argument parsing is missing the field.

Whether the CLI and MCP share one `Args` type or two is exactly the thing to
check first; if they share one, this is stranger than it looks.

## Evidence

The guard itself is surface-agnostic and behaves correctly — the refusal in the
Symptom above left the file **byte-identical at 108 lines**, verified with `wc`.
This is purely a missing argument, not a broken guard.

## Hypotheses tried

1. **Hypothesis:** `--force` exists but is undocumented in `--help`.
   **Test:** passed it anyway; clap rejected it as an unknown argument.
   **Verdict:** rejected — it is absent, not hidden.

## Fix

Not implemented. Add `--force` to the CLI's `artifact update` args and thread it
into the same field the MCP path sets. Mirror the flag's documentation from the
MCP schema so both surfaces describe the same escape.

**Check the sibling surfaces in the same pass** rather than fixing only the one
that bit: `edit_markdown` also has a `force` in its MCP schema and the same
guard. Any CLI subcommand that can trip a shrink guard needs the same escape,
and this is a "declared law leaks at call sites" shape — the guard was made
surface-agnostic while the *escape* was not.

## Tests added

None yet. A regression test should assert the CLI accepts `--force` and that a
forced shrink writes, since the failure mode here is an absent argument that no
existing test would notice.

## Workarounds

Use the MCP tool, which has the flag:
`artifact(action="update", id=…, force=true, patch={body: …})`.

## Resume

Read the clap definition behind `codescout artifact update` (start at the CLI
module's `artifact` subcommand enum) and confirm whether it shares an `Args`
type with the MCP path or defines its own. Then add the flag and sweep the
sibling write surfaces named under Fix.

## References

- `src/librarian/tools/update.rs` — the guard and the MCP-side `force`
- `docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md` —
  the bug whose fix widened the guard to lines, making this gap more reachable

