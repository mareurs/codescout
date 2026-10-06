---
id: '2df804b3435e2300'
kind: bug
status: open
title: 'BUG: the worktree write guard keys on the doc action name, so it refuses dry-run previews and malformed calls that could never write'
tags:
- librarian
- worktree
- write-guard
- codescout-tool
- cluster/unclassified
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-03-the-worktree-write-guard-covers-file-writes-and-no-doc-action.md
severity: low
---

# BUG: the worktree write guard keys on the doc action name, so it refuses dry-run previews and malformed calls that could never write

## Summary

`LibrarianAdapter::call` runs `guard_worktree_write` for every `doc` action in `is_mutating_doc_action`, before the tool reads its arguments. The guard tests the action NAME as a proxy for "this call will write bytes". Two kinds of call match the name and never write: a dry-run preview, and a call that fails argument validation. In a repo with linked worktrees and no `workspace(action="activate")`, both get "Write blocked" instead of the preview or the validation error.

## Symptom (Effect)

Session with linked worktrees present, no project chosen, and no `workspace=` pin on the call.

A malformed call. The tool's own corrected-call hint never shows:

```
doc(action="move")
→ {"ok": false, "error": "Write blocked: git worktrees detected but workspace(action='activate') has not been called. Worktrees: [...]", "hint": "Call workspace(action='activate', path=\"/home/marius/work/claude/codescout\") to write to the main repo, or pass workspace=\"<abs path>\" on this call ..."}
```

The same call with `workspace="/home/marius/work/claude/codescout"`:

```
doc(action="move", workspace=...)
→ {"ok": false, "error": "doc(action=\"move\") requires 'id' and 'new_rel_path': missing field `id`", "hint": "e.g. doc(action=\"move\", id=\"<16-hex>\", new_rel_path=\"docs/archive/foo.md\"). ..."}
```

A dry-run preview. `delete` writes nothing without `force=true`, and the id here does not exist:

```
doc(action="delete", id="0000000000000000")
→ {"ok": false, "error": "Write blocked: git worktrees detected but workspace(action='activate') has not been called. ..."}
```

## Reproduction

```
git rev-parse HEAD    # 7976c0a8, branch experiments (git log -1, 2026-10-06 ~15:04)
```

1. Use a build of `target/release/codescout` from 2026-10-06 15:01 or later, in a repo with linked worktrees (here `codescout.worktrees/res2-a`, `res2-b`, `hold-minors`).
2. Start a session. Do not call `workspace(action="activate")`. Do not pin.
3. Call `doc(action="move")`, then `doc(action="delete", id="0000000000000000")`. Both return "Write blocked".
4. Repeat step 3 with `workspace="/home/marius/work/claude/codescout"`. The first call returns the missing-field error with its `e.g.` hint. The second returns the dry-run preview.

## Environment

Linux 7.2.8-zen1, codescout MCP over stdio, project `codescout`, branch `experiments`, three linked worktrees, no project chosen this session.

## Root cause

Mechanism, read from the code and confirmed by the three calls above (measured 2026-10-06):

- `src/librarian/adapter.rs` `LibrarianAdapter::call` reads `input["action"]` and, when `is_mutating_doc_action(action)` is true, awaits `guard_worktree_write(ctx)` and returns its error. It does this before it dispatches to the inner tool.
- `is_mutating_doc_action` (`src/librarian/adapter.rs:1039-1054`) lists `create`, `update`, `move`, `delete`, `graft`, `link`, `append_entry`, `update_entry`, `rekey_prefix`, `event_create`, `augment`. It never looks at `force`, `dry_run` or the other arguments.
- `delete`, `graft` and `rekey_prefix` are dry runs unless `force=true`. A preview writes nothing, yet the guard refuses it. Measured for `delete` only. `graft` and `rekey_prefix` are inferred from the list and their documented dry-run default, not measured.
- The inner tool reads its arguments after the guard. For `move`, that is `serde_json::from_value` at `src/librarian/tools/mv.rs:136`, which routes a missing field through `deser_error`. A malformed call therefore cannot reach its own validation error until the guard passes.
- The guard returns `Ok` at once when `ctx.workspace_override` is set (`src/tools/core/guards.rs:37`). So `workspace=` is the only per-call remedy.

The comment above the guard call says the adapter is the one place that sees both the core `ToolContext` and the `action` argument. That explains why the guard is there. It does not explain why it runs before argument parsing.

## Evidence

The three calls in "Symptom" ran in this session after the reconnect to the build of 15:01. The `delete` preview used a non-existent id so nothing could change.

## Hypotheses tried

1. **Hypothesis:** the guard runs after validation, and the first call failed for another reason. **Test:** the same `doc(action="move")` with and without `workspace=`. **Verdict:** rejected. Only the pin changed the error.
2. **Hypothesis:** only `move` is affected. **Test:** `doc(action="delete", id="0000000000000000")`, no pin. **Verdict:** rejected. `delete` is refused too, and a preview of it is the stronger case.

## Fix

Not started. Two options, not exclusive:

- For the three dry-run actions, run the guard only when the call will apply (`force=true`). The set would need a per-action predicate, not a name list.
- Run the guard after the argument check, so a malformed call returns its own error. This needs the guard inside each tool, or a parse step in the adapter. Today only the adapter sees both `ctx` and `action`.

Whichever is chosen, the guard must still run before the first write.

## Tests added

N/A — not fixed. A fix needs two tests that assert on the error TEXT, not only `is_err()`: a malformed `move` and a `delete` preview, each unpinned in a worktree repo (see `docs/issues/archive/2026-09-18-the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy.md` on why `is_err()` alone proved nothing about the hint).

## Workarounds

Pass `workspace="<abs path of the main repo>"` on the call, or call `workspace(action="activate", path=...)` once. Both are what the error already prescribes. They cost one failed call per session.

## Resume

Decide between the two fix options. Then add the two tests above. The adapter change is small. The open question is whether a preview should ever be refused.

## References

- Sibling, not a duplicate: `docs/issues/archive/2026-09-03-the-worktree-write-guard-covers-file-writes-and-no-doc-action.md`. That bug found the guard too NARROW (no `doc` action covered). Its fix added `is_mutating_doc_action`. This bug is the same list being too WIDE for two shapes of call.
- Cluster: filed as `cluster/unclassified`. IC-14 (guard narrower than its name) is the nearest class but names the opposite direction. IC-2 (gate keyed on an event it cannot observe) fits the proxy idea, but its claim is that the proxy fails silently, and this guard fails loudly. Neither was forced.
