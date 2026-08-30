---
id: 5312b1e4e86f1bfd
kind: bug
status: fixed
title: 'BUG: the CLI''s `artifact update` has no `--force`, so a caller who hits the body-shrink guard has no escape hatch'
owners:
- marius
tags:
- cli
- librarian
- shrink-guard
- parity
closed: 2026-08-30
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

**Confirmed at the source** (was inferred from `--help` when this was filed).

The CLI does **not** share an `Args` type with the MCP path — it defines its
own clap struct, `UpdateArgs` (`src/cli/artifact.rs`), and `run_update`
hand-marshals each field into a `serde_json::Map` before calling
`librarian::tools::update::call`. `UpdateArgs` had no `force` field, so the
key was never inserted and the tool's `Args::force` fell to its serde default.

That is the sharp part, and it is worse than a missing flag: `Args` carries no
`deny_unknown_fields`, and `force` is `#[serde(default)]`. A field the CLI
never sends is therefore **defaulted in silence** — not rejected — so had the
flag been added to the struct without a matching insert, `--force` would have
parsed cleanly, been ignored, and the write would still have reported
`updated: true`. The two halves fail independently, which is why the fix pins
both.

The enforcement layer was already surface-agnostic: `crate::util::shrink_guard::check`
is called from `update.rs:565` and reads `a.force` at `:564`, regardless of
caller. Only the CLI's argument parsing and marshalling were missing.
## Evidence

The guard itself is surface-agnostic and behaves correctly — the refusal in the
Symptom above left the file **byte-identical at 108 lines**, verified with `wc`.
This is purely a missing argument, not a broken guard.

**Sibling-surface sweep (the one the Fix section asked for).**
`references(symbol="check", path="src/util/shrink_guard.rs")` returns three
production call sites: `librarian/tools/update.rs:565`, `memory/mod.rs:142`,
and `tools/markdown/edit_markdown.rs:1454`. Of those, **only `artifact update`
has a CLI subcommand at all** — `src/cli/` contains `artifact`,
`artifact_augment`, `artifact_event`, `artifact_refresh`,
`constitution_check`, `doctor` and `audit_doc_refs`, and nothing for
`edit_markdown` or `memory`. So the sweep closes here rather than widening:
the original Fix note guessed `edit_markdown` would need the same escape, and
it does not, because it is not reachable from the CLI.
## Hypotheses tried

1. **Hypothesis:** `--force` exists but is undocumented in `--help`.
   **Test:** passed it anyway; clap rejected it as an unknown argument.
   **Verdict:** rejected — it is absent, not hidden.

## Fix

**Fixed on `experiments`** — `19289b1f834a6d359dbc8167c478c16603279550`,
patch-id `0dcfe354914363bcdc62a67d3e5f0d99ba79fe29`.

1. Added `force: bool` to `UpdateArgs` with `#[arg(long)]`, doc-commented from
   the MCP schema so both surfaces describe the same escape.
2. Extracted `build_update_tool_args(&UpdateArgs) -> Result<Value>` from
   `run_update`, mirroring how `compile_filter` is split out of `run_find`.
   The marshalling was previously inline in an `async fn` that opens a catalog,
   so the translation step — the half that fails silently — was not reachable
   from a test. It is now.
3. Marshalled `force` as a **top-level** key, sibling to `patch`, matching
   where the tool reads it (`a.force`, not `patch.force`), and only when set.

**Live verification** at the rebuilt release binary, against the Reproduction
above: the unforced shrinking write is still refused (exit 1, file unchanged at
108 lines / 7079 bytes), and the identical write with `--force` succeeds
(exit 0, `updated: true`, 18 lines / 6089 bytes). The guard was not weakened —
only given the door its own error message advertises.
## Tests added

Three, in `src/cli/artifact.rs`'s `mod tests` — one per independent failure
mode, because the flag can break at the parser *or* at the marshalling and the
second break is invisible from the first:

| Test | Guards against |
|---|---|
| `update_parser_accepts_force_flag` | clap rejecting `--force` |
| `update_force_flag_reaches_the_tool_args` | parsed-then-dropped (the shipped defect) |
| `update_omits_force_when_unset` | marshalling it unconditionally, which would disable the guard for **every** CLI update |

**Mutation-verified — the matrix is the result, not the three green ticks.**
Each mutation killed exactly one test and left the other two green:

| Mutation | Kills |
|---|---|
| `#[arg(long)]` → `#[arg(long = "forcibly")]` | parser test only |
| drop the `if args.force { … }` insert | marshalling test only |
| make the insert unconditional | omission test only |

The second mutation reproduces the shipped defect exactly, and under it
`--force` still parses and the write still reports success — which is precisely
what a parser-only test cannot see.

That `force: true` actually bypasses the guard is pinned tool-side by
`librarian::tools::update::tests::body_shrink_guard_allows_with_force`; these
three close the CLI's half of the path.
## Workarounds

None needed as of `19289b1f`. Before the fix: use the MCP tool, which has the
flag — `artifact(action="update", id=…, force=true, patch={body: …})`.
## Resume

Nothing outstanding. The fix is on `experiments` with its SHA and patch-id
recorded above, and the sibling-surface sweep the Fix section asked for is
complete and recorded under Evidence — it closed at one surface rather than
widening.

One **adjacent** gap was noticed while reading the clap definition and is
filed separately rather than fixed here, since it is a different consequence of
the same hand-marshalling mechanism: `time_scope` and `extra` are MCP params on
both `artifact create` and `artifact update` that no CLI flag reaches. See
`docs/issues/2026-08-30-cli-artifact-drops-time-scope-and-extra.md`.
## References

- `src/librarian/tools/update.rs` — the guard and the MCP-side `force`
- `docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md` —
  the bug whose fix widened the guard to lines, making this gap more reachable
