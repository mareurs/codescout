---
status: open
opened: 2026-08-30
closed:
severity: low
owner: marius
related: []
tags: [cli, librarian, parity, silent-default]
kind: bug
---

# BUG: the CLI's `artifact create` / `artifact update` silently drop `time_scope` and `extra`

## Summary

The MCP `artifact` tool accepts `time_scope` and `extra` on both `create` and
`update`. Neither CLI subcommand has a flag for either, and because the tool's
`Args` defaults missing fields rather than rejecting them, a CLI caller cannot
set them and gets no indication they are unreachable. Same mechanism as
`docs/issues/archive/2026-08-30-cli-artifact-update-has-no-force-escape-for-the-shrink-guard.md`,
different consequence.

## Symptom (Effect)

There is no error to quote — that is the whole problem. `--help` simply does
not list the flags:

```
$ codescout artifact update --help
Options:
      --title <TITLE>                New title
      --status <STATUS>              New status
      --owners <OWNERS>              Comma-separated owner list (replaces existing list)
      --tags <TAGS>                  Comma-separated tag list (replaces existing list)
      --topic <TOPIC>                New topic
      --body <BODY>                  Body content: `@<file>`, `-`, or literal
      --patch-params <PATCH_PARAMS>  RFC 7396 merge-patch on augmentation params
      --commit-refresh               Record a completed refresh cycle atomically
      --force                        Bypass the body-shrink guard
      ...
```

No `--time-scope`, no `--extra`. `--patch-params` is not a substitute: it maps
to `patch.params` (augmentation params), a different field from `extra`
(custom YAML frontmatter keys).

## Reproduction

Read the two struct pairs side by side; there is nothing to run.

- `src/cli/artifact.rs` — `UpdateArgs` and `CreateArgs`
- `src/librarian/tools/update.rs` — `Args` (`time_scope`, `extra`)
- `src/librarian/tools/create.rs` — the create-side equivalents

## Environment

Linux, `experiments`, codescout 0.15.0, at `19289b1f`.

## Root cause

Inferred from reading both clap structs and the MCP `Args` on 2026-08-30 —
mechanism read at the source, consequence not exercised at runtime.

The CLI defines its own clap structs and hand-marshals each field into the
tool's JSON (`build_update_tool_args`). `librarian::tools::update::Args`
carries no `deny_unknown_fields` and marks every optional field
`#[serde(default)]`, so a field the CLI never inserts is **defaulted in
silence**. There is no diagnostic at any layer: clap cannot warn about a flag
that was never declared, and serde cannot warn about a key that was never sent.

This is the same defect shape as the `--force` bug, and it is structural
rather than incidental: any MCP param added to `update`/`create` in future is
unreachable from the CLI by default, and nothing fails when it is.

## Evidence

`UpdateArgs` fields: `id, title, status, owners, tags, topic, body,
patch_params, commit_refresh, force, common`.

`update::Args` fields: `id, patch, status, title, owners, tags, topic,
time_scope, extra, commit_refresh, force`.

The set difference is exactly `{time_scope, extra}` (`patch` and `common` being
structural rather than user-facing). `CreateArgs` has the same two absences.

## Hypotheses tried

None — filed on notice while fixing the `--force` bug, not investigated
further.

## Fix

Not implemented, and deliberately not bundled with the `--force` fix: that one
closed a hint that pointed at a missing remedy, this one is metadata parity.

Sketch: add `--time-scope <STR>` and `--extra <JSON>` to both `UpdateArgs` and
`CreateArgs`, marshal `time_scope` into `patch` (its canonical home on update)
and `extra` as a parsed JSON object. `build_update_tool_args` already exists as
the testable seam for the update half; the create half would want the same
split before it is worth testing.

**Worth considering instead of adding two flags:** a test that asserts the
CLI's marshalled key set covers the tool's `Args` field set, so the next added
param cannot go missing silently. That addresses the mechanism rather than the
two instances of it.

## Tests added

None — not fixed.

## Workarounds

Use the MCP tool, which accepts both:
`artifact(action="update", id=…, patch={time_scope: "2026-W35"}, extra={…})`.

## Resume

Decide first whether to add the two flags or the coverage test described under
Fix — they are different bets, and the test is the one that scales. Then read
`src/librarian/tools/create.rs`'s `Args` to confirm the create-side field names
before mirroring them (this file asserts the create gap from `CreateArgs`
alone; the create-side `Args` was not read).

## References

- `docs/issues/archive/2026-08-30-cli-artifact-update-has-no-force-escape-for-the-shrink-guard.md`
  — same mechanism, fixed at `19289b1f`; its Evidence section carries the
  sibling-surface sweep that bounds how far this class reaches
- `src/cli/artifact.rs` — `UpdateArgs`, `CreateArgs`, `build_update_tool_args`
- `src/librarian/tools/update.rs` — the MCP-side `Args`
