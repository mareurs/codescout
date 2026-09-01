---
kind: bug
status: fixed
tags:
- cli
- librarian
- parity
- silent-default
- cluster/declared-not-wired
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related: []
severity: low
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

### Performed 2026-08-30 — and there WAS something to run

The line above says *"there is nothing to run"*. That is a claim, and running it is
cheap, so it was run first:

```
$ codescout artifact update --help    # shipped binary
$ codescout artifact create --help
```

Neither listed `--time-scope` or `--extra`; `--force` **was** present, which confirms
the binary postdated `19289b1f` and that the absence was current rather than an
artefact of a stale build. That second fact is the one struct-reading could not have
supplied, and it is the difference between "the flags are missing" and "the flags are
missing *from the code that ships today*".

After the fix, the same two commands list both flags on both subcommands.
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

### Shipped 2026-08-30 — `0c4931ef` on `experiments`, patch-id `a0a4a3b4d0ea3f1b1d52e9299b9809dad98fcf05`

`--time-scope` and `--extra` added to both `CreateArgs` and `UpdateArgs`, and
marshalled at the depth each tool expects: nested in `patch` for update, top level
for create. `--extra` is parsed as JSON with `null` preserved, because `null` is how
`extra` deletes a key; a non-JSON value fails loudly naming the flag rather than being
dropped.

**`build_create_tool_args` extracted from `run_create`,** mirroring what `19289b1f`
did for update. That commit's doc comment already states this bug's mechanism — *"a
field can exist on the struct and never reach the tool … only testable if the
translation is reachable without a catalog"* — and the create side never received the
same treatment, which is why the same defect could recur there unobserved. Both halves
are now testable without a catalog.
### Re-verified against the shipped release binary, 2026-08-30

The `unverified:` key on this file said the fix had been checked only through a debug
`cargo run`, and asked for a re-check at the next rebuild anyone performed. That rebuild
happened; the key is now **removed** rather than left standing, because nothing is
outstanding.

```
$ codescout artifact update --help   → --time-scope ✓   --extra ✓
$ codescout artifact create --help   → --time-scope ✓   --extra ✓
$ readlink -f $(which codescout)     → target/release/codescout, inode 6442149, 13:25:19
```

The inode is recorded deliberately. `which codescout` resolves through a symlink, and a
mtime on the target says nothing about which image answered — the same confusion that had
to be corrected in the sibling bug
(`docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`).
Matching the inode against `/proc/<server-pid>/exe` is what makes "the shipped binary has
the flags" a claim about the running artefact rather than about a path.
## Tests added

Eight, in `src/cli/artifact.rs`, split the way the `--force` tests are: one per
independent failure mode, because the parser can reject a flag *or* accept it and the
marshalling can drop it, and only the second is silent.

All eight went **compile-error → green**, so per `bug-fix-session-log:W-73` none had
ever run its assertions against a wrong world. Three deliberate breaks, each killed:

| mutation | test that died | symptom |
|---|---|---|
| drop `time_scope` from the update marshalling | `update_time_scope_and_extra_reach_the_patch` | `left: None` |
| marshal create's `extra` under a wrong key | `create_time_scope_and_extra_reach_the_tool_args` | `left: None` |
| drop `augment.prompt` from the extracted builder | `create_still_marshals_every_pre_existing_field` | `left: None` |

The third is the one worth keeping past this bug: it is a characterization guard
proving the ~60-line extraction was behaviour-**preserving** rather than merely
compiling. Nothing else in 4819 tests would have caught a field silently lost in that
move — which is the same class of loss this bug is about, arriving via refactor
instead of via omission.

The assertions pin **depth**, not just presence, because a value marshalled to the
wrong depth is silently defaulted by the tool. That is the defect, so a test that only
checked presence somewhere in the payload would pass on the bug's own shape.
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
