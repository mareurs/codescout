---
kind: bug
status: open
tags:
- librarian
- worktree
- write-guard
- codescout-tool
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-09-03
owner: marius
related:
- docs/trackers/bug-fix-session-log.md
severity: medium
---

# BUG: the worktree write guard covers the five file-write tools and no `doc` action, so the librarian writes to a tree `edit_file` refuses

## Summary

With linked worktrees present and no `workspace(action="activate")` called,
`edit_file` refuses to write a file — while `doc(action="append_entry")` had
already written to **that same file**, in the same session, seconds earlier,
without complaint. `guard_worktree_write` is wired at the five file-write tools
and at no librarian mutation. The inconsistency is the defect twice over: the
librarian is the write path whose catalog is keyed on absolute path
(`id = sha256(abs_path)`), so resolving to the wrong tree there mints a different
id — and the refusal an agent *does* get teaches it a model of write safety that
is false for the tool it just used.

## Symptom (Effect)

Observed 2026-09-03 ~23:33, one session, no `activate` between the two calls:

```
doc(action="append_entry", id="2dd9d90bc83f9f49", ...)
  -> {"id": "F-110", "section_written": true,
      "wrote_to": "/home/marius/work/claude/codescout"}      # WROTE

edit_file(path="docs/trackers/bug-fix-session-log.md", ...)  # SAME FILE
  -> {"ok": false,
      "error": "Write blocked: git worktrees detected but workspace(action='activate')
                has not been called. Worktrees:
                [/home/marius/work/claude/codescout/.worktrees/tool-collapse,
                 /home/marius/work/claude/codescout/.worktrees/result-cap-marker-gate]"}
```

Calling `workspace(action="activate", path="/home/marius/work/claude/codescout")`
and re-running the identical `edit_file` succeeded.

## Reproduction

```
git rev-parse HEAD    # ea03814e, branch experiments
# preconditions: >=1 linked worktree exists, and this MCP session has NOT
# called workspace(action="activate") — a fresh /mcp reconnect satisfies the
# second, since post_compact=true runs `status`, not `activate`.

doc(action="append_entry", id=<any tracker>, ...)        # succeeds
edit_file(path=<that tracker's rel_path>, ...)           # refused
```

## Environment

Linux 7.1.9-zen1-2-zen, `experiments` @ `ea03814e`, MCP stdio. Server pid 938147
started 23:30:10 from a 23:28:11 release build (`/proc/938147/exe` not
`(deleted)`). Two linked worktrees under `.worktrees/`.

## Root cause

`guard_worktree_write` (`src/tools/core/guards.rs:20-47`) is called from exactly
five places, all file-write tools:

- `src/tools/create_file.rs`
- `src/tools/edit_file/mod.rs`
- `src/tools/markdown/edit_markdown.rs`
- `src/tools/symbol/edit_code.rs` (2 sites)
- `src/tools/approve_write.rs`

A `grep` for the symbol across `src/` returns **no hit under `src/librarian/`**.
The librarian's own guards answer different questions:

- `temp_write_guard::guard_temp_workspace_write`
  (`src/librarian/tools/temp_write_guard.rs:62`) refuses a write whose root is
  under the **OS temp dir** while the catalog is the real one. Temp-dir, not
  worktree.
- `is_linked_worktree` is consulted by `index_repo_sync`
  (`src/librarian/indexer.rs`) to skip **walking** a linked worktree. That is the
  read/walk path; it gates no write.

So every `doc` mutation — `create`, `update`, `move`, `delete`, `append_entry`,
`update_entry`, `augment`, `event_create`, `link`, `graft` — writes with the
ambiguity `guard_worktree_write` exists to refuse. It resolves to *a* tree and
discloses which in `wrote_to`, which is good disclosure and not the same thing as
a gate.

*Read at the bytes this session:* the five call sites, the two librarian guards,
and the absence of the symbol under `src/librarian/`. *Measured 2026-09-03:* the
succeed-then-refuse pair quoted above, and the identical call succeeding after
`activate`.

## Evidence

### The gate already knows it has exactly four or five customers

```rust
// src/usage/db.rs:2298-2313
// The single largest member — 23 hits across four write tools, one gate.
("edit_markdown", "Write blocked: git worktrees detected but workspace(...) ...",
 Some("worktree_activate_required")),
("create_file",   "Write blocked: git worktrees detected but workspace(...) ...",
 Some("worktree_activate_required")),
```

The error-family normalizer enumerates the population as *"four write tools"* —
so the guard's own accounting surface already records that its customers are the
file-write tools, and nothing there is a `doc` action. The scope is visible in
the enforcement layer and nowhere a `doc` caller reads it.

### The repair machinery for the failure this would prevent already exists

`librarian(action="merge_worktree")` folds a worktree's shadow rows onto their
main twins; `librarian(action="doctor", fix="reseat_worktree")` reseats
worktree-scoped catalog rows. CLAUDE.md documents both. A repair path is
evidence the failure occurs — and the guard that would have prevented it does not
cover the tool that causes it.

## Hypotheses tried

1. **Hypothesis:** the guard is new in the 23:28 build, so this is a
   not-yet-wired new feature rather than a coverage gap.
   **Test:** `src/usage/db.rs:2298` names it as *"the single largest member — 23
   hits across four write tools"* of a 2026-08-20 error-family classification.
   **Verdict:** **rejected** — the guard predates this by weeks and already has a
   measured hit population.

2. **Hypothesis:** the librarian has an equivalent guard under another name.
   **Test:** grep `src/librarian/**` for `worktree.*guard|guard.*worktree|is_linked_worktree|guard_temp_workspace_write`.
   **Verdict:** **rejected** — the two hits answer different questions (temp-dir
   writes; skipping a worktree during the walk).

3. **Hypothesis:** `doc` does not need it because it resolves deterministically.
   **Verdict:** **deferred, and it is the real design question.** It does resolve
   and does report `wrote_to`. But `id = sha256(abs_path)`, so "resolves to a
   tree" is precisely the decision the guard exists to make the caller state.
   Whichever way this lands, the two tools should not disagree about the same
   file.

## Fix

*Plan only.*

Two coherent answers, and the bug is that neither has been chosen:

- **Extend the gate.** Call `guard_worktree_write` from the librarian mutation
  entry point. Cost: every `doc` write in a worktree-bearing repo now needs an
  `activate` first, including in sessions that never touch a worktree.
- **Retire the ambiguity instead.** If `doc`'s resolution is correct without
  activation, then `edit_file`'s refusal is over-strict for the same reason, and
  the fix is to make the file-write tools resolve the way `doc` does and report
  `wrote_to` rather than refuse.

Do not "fix" this by adding a `doc`-specific message. The value at stake is that
one file has one write target regardless of which tool addresses it.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None yet. When fixed, the discriminating test is **cross-tool**: same path, same
session, no activation, assert `doc` and `edit_file` agree — either both refuse
or both write to the same resolved root. A per-tool test on either side passes
today and is what let the two drift.

## Workarounds

Call `workspace(action="activate", path="<main repo abs path>")` at session
start in any repo with linked worktrees. `workspace(post_compact=true)` is NOT
sufficient — it runs `status`, not `activate`, which is how this session reached
the state.

## Resume

`src/tools/core/guards.rs:20-47` (the gate), its five call sites, and
`src/librarian/tools/` (which has none). `src/usage/db.rs:2298` records the
population the gate was measured against.

## References

- `docs/trackers/bug-fix-session-log.md` § `F-110` — the recon pass that hit this
  incidentally.
- CLAUDE.md § *Session Intelligence Trackers* — `id = sha256(abs_path)`, and why
  a hand-move orphans a catalog row's events and augmentation. Same arithmetic
  makes a wrong-tree write mint a wrong id.

### Cluster adjudication

Tagged `cluster/guard-narrower-than-its-name` (`IC-14`). `guard_worktree_write`
names the class of writes it protects — worktree writes — and covers only the
file-write subset, leaving the librarian's writes, which are the ones whose
identity is derived from the path.

Near miss rejected: `IC-3` (`declared-not-wired`). The guard *is* wired, at five
sites, and fires correctly there. The defect is the boundary of its coverage, not
its reachability.
