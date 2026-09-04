---
kind: bug
status: fixed
tags:
- librarian
- worktree
- write-guard
- codescout-tool
- cluster/guard-narrower-than-its-name
claimed_at: 2026-09-04
claimed_by: 4a2f34f7-0669-487d-9ce9-39b77881642f
closed: 2026-09-04
opened: 2026-09-03
owner: marius
related:
- docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md
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

**Chosen (deliberately, by the project owner, not by whoever picked up the bug): extend the gate.** Every `doc` write now needs `activate` first in a worktree-bearing repo, consistent with the five file-write tools — accepting the broadened friction over resolving `doc`'s ambiguity by trusting it.

**Real cost found while scouting, materially bigger than "wire the existing function into one more call site":** `guard_worktree_write(ctx: &ToolContext)` reads `ctx.agent` — but `doc()`'s dispatch (`Artifact::call` in `artifact.rs`) receives a COMPLETELY DIFFERENT `ToolContext` type (`crate::librarian::tools::ToolContext`, defined in `src/librarian/tools/mod.rs`), which has no `agent` field at all. It's built once at server boot (`crate::librarian::build_tool_context`) and shared read-only across every session (`Arc<LibToolContext>`), not derived per-call from session state the way the core `ToolContext` is. So the guard could not simply be called from `Artifact::call`.

The actual seam: `LibrarianAdapter::call` (`src/librarian/adapter.rs`) is the ONE place that receives BOTH the core `crate::tools::ToolContext` (with `.agent`, as its own `ctx` parameter) and the raw `input` JSON (with the `action` string) before deriving the librarian's own per-call `ToolContext` via `self.derive_ctx(...)`. The guard now runs there, gated on `self.inner.name() == "doc"` and a new `is_mutating_doc_action(action)` classifier covering exactly the ten actions this bug named (`create`, `update`, `move`, `delete`, `graft`, `link`, `append_entry`, `update_entry`, `event_create`, `augment`). The seven read actions (`find`, `get`, `graph`, `state_at`, `event_list`, `gather`, `list_stale`) are exempt and verified not to be gated (see § *Tests added*).

Fixed at `05b785ac445130a4facfeae5fe5cdd3dc8cf87f3` on `codescout` `experiments`, patch-id `d2f60345d6a21d278e9dc03f8fb71257fb78afb3`.
## Tests added

Three new `#[tokio::test]`s in `src/librarian/adapter.rs`'s own `tests` module (mirroring `guard_worktree_write_refuses_when_only_resolved_at_startup` / `..._allows_after_explicit_activate` in `src/tools/core/tests.rs`, and `adapter_for_test()` for the `LibrarianAdapter` construction):

- `doc_mutation_is_blocked_when_worktrees_exist_and_not_activated` — `doc(action="create")` through `LibrarianAdapter::call` with linked worktrees and no `activate()` must fail with `"Write blocked"` specifically (not some other validation error).
- `doc_mutation_allowed_after_explicit_activate` — the same call succeeds once `ctx.agent.activate(root, None)` has run.
- `doc_read_is_not_blocked_by_worktree_guard` — `doc(action="find")` under the same worktree/no-activate conditions must never be refused by this guard.

All three ran RED first (2 of 3 failing for the expected reason: the guard did not exist and the fixture-payload gap on `body`), then GREEN after the fix. Full workspace gate (fmt, clippy -D warnings, both test lanes) run clean for this file; two unrelated failures seen in the same `cargo test --workspace` run (`parse_create_table_columns_extracts_artifact_columns`, `index_repo_sync_embeds_content_stamped_by_a_run_that_did_not_embed_it`) belong to a concurrent peer session's uncommitted work on a766aad35b0b7610 in `catalog/*.rs` and `indexer.rs` — confirmed via `git status` before committing, and this commit stages only `src/librarian/adapter.rs`.
## Workarounds

Call `workspace(action="activate", path="<main repo abs path>")` at session
start in any repo with linked worktrees. `workspace(post_compact=true)` is NOT
sufficient — it runs `status`, not `activate`, which is how this session reached
the state.

## Resume

Done. Fixed and verified on `codescout` `experiments` at `05b785ac445130a4facfeae5fe5cdd3dc8cf87f3` (patch-id `d2f60345d6a21d278e9dc03f8fb71257fb78afb3`). Direction chosen: extend the gate (not retire it) — see § *Fix* for why the actual wiring point is `LibrarianAdapter::call`, not the `Artifact::call` dispatch this bug originally pointed at.
## References

- `docs/trackers/bug-fix-session-log.md` § `F-110` — the recon pass that hit this
  incidentally.
- CLAUDE.md § *Session Intelligence Trackers* — `id = sha256(abs_path)`, and why
  a hand-move orphans a catalog row's events and augmentation. Same arithmetic
  makes a wrong-tree write mint a wrong id.

### The nearest sibling, and why this is not a duplicate of it

`docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`
(`93caba562c06a258`, **fixed** 2026-09-02) carries the same `IC-14` tag and is the
closest thing in the corpus. It is a different defect and that fix does not reach
this one:

| | `93caba562c06a258` | this |
|---|---|---|
| guard | `LibrarianAdapter::is_write` → cross-process write lock (mutex + `.codescout/write.lock`) | `guard_worktree_write` → activation gate |
| mechanism | the guard IS reached from the librarian; its **action enumeration** omitted five | the guard is **never reached** from the librarian, at any action |
| repair | add the five names to the arm | choose a resolution rule, then wire it |

Worth recording that this sibling was surfaced by `doc(action="find", semantic=…)`
ranked 2 for a query describing the mechanism, not by any grep I ran while
writing the file above — the two share almost no vocabulary (`is_write` /
`guard_worktree_write`, `write.lock` / `activate`). It is the clearest case this
session of chunk-grain retrieval doing work a keyword search could not.

### Cluster adjudication

Tagged `cluster/guard-narrower-than-its-name` (`IC-14`). `guard_worktree_write`
names the class of writes it protects — worktree writes — and covers only the
file-write subset, leaving the librarian's writes, which are the ones whose
identity is derived from the path.

Near miss rejected: `IC-3` (`declared-not-wired`). The guard *is* wired, at five
sites, and fires correctly there. The defect is the boundary of its coverage, not
its reachability.
