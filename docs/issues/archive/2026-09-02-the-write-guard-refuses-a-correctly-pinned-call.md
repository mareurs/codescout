---
id: df352add066ea79e
kind: bug
status: fixed
title: 'BUG: guard_worktree_write refuses a correctly-pinned write, so the remedy the docs prefer over re-activating does not work'
tags:
- cluster/guard-narrower-than-its-name
- worktree
- workspace-pin
- guard
- write-path
fix_patch_id: e4759a8996fabd88a8b2352b70d543677fe80032
fix_sha: 7320d27d281750169252145e66def00073c989ea
fixed: 2026-09-04
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md
- docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md
- docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md
severity: medium
---

## Summary

`guard_worktree_write` (`src/tools/core/guards.rs`) decides on
`Agent::is_project_chosen_this_session()` alone. It reads `ctx.workspace_override` **only** to
resolve a root for the refusal message — never for the decision. So a call that names its own
tree explicitly is refused exactly as a bare one is.

That matters because per-call pinning is the remedy the docs prescribe **in preference to
re-activating**: `get_guide("workspace-state")` § *Per-call workspace pinning* says *"When a peer
has activated read-only, pin — do not re-activate"*, on the grounds that activation is
process-wide and flipping it clears your block by moving the substrate under whoever set it,
mid-task. Following that advice on a write does not work.

## Symptom (Effect)

Observed live 2026-09-02, in this order, within about a minute:

| call | result |
|---|---|
| `edit_code(path=…, …)` after a peer flipped the shared slot | refused — *"worktrees detected but `workspace(action='activate')` has not been called"* |
| the same call **with `workspace="/home/marius/work/claude/codescout"`** | **refused identically** |
| `workspace(action="activate", path=…)`, then the same call unpinned | succeeded |

The pin is the documented, peer-safe remedy; the only thing that actually cleared the block was
the process-wide mutation the guide tells you to avoid.

## Reproduction

1. In a checkout with a linked worktree, ensure no `activate` has run this session (a `/mcp`
   reconnect does this, or a peer's activation of a different project).
2. Call any write tool with `workspace=<abs path of the main checkout>`.
3. Refused, naming a call the caller has deliberately avoided making.

## Root cause

```rust
pub async fn guard_worktree_write(ctx: &ToolContext) -> anyhow::Result<()> {
    if ctx.agent.is_project_chosen_this_session().await {
        return Ok(());
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())   // <- the ONLY use
        .await?;
    let worktrees = crate::util::path_security::list_git_worktrees(&root);
    if worktrees.is_empty() {
        return Ok(());
    }
    …refuse, listing `worktrees`
}
```

`workspace_override` reaches `require_project_root_for`, which picks *which root's worktrees get
enumerated in the message*. The decision above it does not consult the pin at all.

**The two are answering different questions and only one is asked.** The flag means *"did
someone choose a tree for this session?"*; the pin means *"this call names its tree."* Both
resolve the ambiguity the guard exists to protect against — a pinned write cannot land in the
wrong tree — but only the first is checked.

## Evidence

### It is the exact mirror of a defect fixed the same day, one surface over

`7a3aee93` gave `worktree_read_notice` an early return on `ctx.workspace_override.is_some()`,
with the reasoning that a caller who named their tree resolved nothing by default and so has no
default to be warned about. That reasoning applies unchanged to the write guard: a caller who
named their tree has no ambiguity to be refused over. The notice learned it; the guard did not.

Pinned by `a_pinned_read_gets_no_worktree_notice_even_though_the_tree_is_unchosen`
(`src/tools/core/tests.rs`). No sibling test exists on the write side — which is why the two
drifted apart in one commit without anything reporting it.

### The prescribed remedy is unavailable, so the guard trains callers to the unsafe one

Refusing a pinned write leaves exactly two routes: `workspace(action="activate")` — process-wide,
which `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
records disrupting a running implementer — or give up. A guard whose only satisfiable remedy is
the one the docs warn against is worse than one that simply lacks a remedy, because the caller
who followed the guidance is the one who gets refused.

## Fix

**Fixed 2026-09-04** — `7320d27d281750169252145e66def00073c989ea`, patch-id
`e4759a8996fabd88a8b2352b70d543677fe80032`. One early return in `guard_worktree_write`
(`src/tools/core/guards.rs`), placed before the `is_project_chosen_this_session()` await
exactly as this section prescribed:

```rust
if ctx.workspace_override.is_some() {
    return Ok(());
}
```

**Both prerequisite checks this section demanded were run first, and both cleared.**

1. **The pin decides where the bytes land, not merely what the refusal message says.**
   `resolve_write_or_capture` (`src/tools/core/write_ack.rs`) derives all three inputs to
   `classify_write_path` — root, security config, session write roots — through
   `*_for(ctx.workspace_override)`. Above it, `call_tool_inner` acquires the **pinned**
   project's `write_lock`/`file_lock` (`src/server.rs`, comment: *"pin the write guard to the
   SAME workspace the tool body will resolve"*) and gates `check_tool_access` on the pinned
   security config. The pin is not advisory on the write path; it *is* the write path.

2. **The pin cannot be smuggled, and the "in-process subagent path" flagged here is not a
   separate path at all.** Established positively, by enumerating the namespace rather than
   failing to find a counterexample: `Server::build_context` is the **only** production
   constructor of a core `ToolContext` — every one of the ~50 other in-tree constructions is
   inside `#[cfg(test)]` — and it sets `workspace_override` solely from
   `extract_workspace_override(&input)` on that same call. A subagent's calls arrive through
   the same `call_tool_inner`, so its pin is its own per-call argument and never inherited
   state. Peer dispatch strips a smuggled one
   (`peer_tool_call_ignores_smuggled_workspace_override`, `src/peer/server.rs`).

3. **`is_project_chosen_this_session()` was still the only other gate** — the function body
   has exactly two decision points, that flag and `worktrees.is_empty()`. The second-wrong-flag
   failure mode this section warned about had not recurred.

**A precedent this file did not know about, and the strongest single argument for the change:**
`call_tool_inner` *already* treats a write-tool pin as consent, deliberately and in writing.
`src/server.rs` grants the pinned root write residency on first residency **because** "the pin
itself already is the caller's consent", and explicitly rejects `activate` as the alternative
because it "would clear every other resident workspace, defeating the point of pinning". The
guard was refusing what the layer directly above it had already accepted as consent — so this
was an inconsistency inside one call path, not a new policy.

Note on the pin being per-call: it resolves the ambiguity **more narrowly** than `activate`
does, not less. A pin cannot be flipped mid-task by a peer or subagent the way the process-wide
session flag can (`docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`),
which is why `get_guide("workspace-state")` prescribes it in preference to re-activating.
## Tests added

`create_file_pinned_to_its_tree_is_allowed_while_the_same_call_unpinned_is_refused`
(`src/tools/core/tests.rs`), beside the three existing guard tests.

Driven through `CreateFile::call` rather than `guard_worktree_write` directly, for the reason
this file's `## Fix` gave: the guard is one `?` at the top of the tool, so an `Ok(())` in
isolation is a claim about a return value and not about where a write lands. The pinned half
therefore asserts `root.join("pinned.txt").exists()`.

Three assertions exist only to stop the pinned half passing vacuously, and each was observed
holding in the RED run rather than merely written down:

- the **unpinned** call on the same context must be refused, and refused with `"Write blocked"`
  specifically — a guard gutted to `Ok(())` satisfies the pinned half on its own;
- the refused call must leave no file behind;
- `is_project_chosen_this_session()` must still be `false` **after** the refused call —
  otherwise the pinned half could be passing through the pre-existing session flag rather than
  through the pin.

Observed RED before the fix: half 1 green, the flag re-assertion green, half 2 failing at the
pinned `create_file` with the exact production message — *"Write blocked: git worktrees detected
but workspace(action='activate') has not been called"*. Observed GREEN after, in **both** test
lanes (`tools::core::tests::` compiles in the lean lane too), alongside the three pre-existing
guard tests — which are what shows the early return did not loosen the guard where it must still
refuse.
## Resume

Closed 2026-09-04 — fixed, gate-green in both test lanes, regression test in place. Nothing
owed: the SHA and its patch-id are recorded in `## Fix`, and the two prerequisite checks this
file refused to be fixed without are recorded there as findings rather than as assurances.

The original entry stands as filed, and is worth keeping: noticed while it blocked this
session's own work during the `issue-clusters` split (`ece4f21a`), and filed late — the notice
went into a chat message, *"filing that after"*, and an hour of unrelated work passed before it
became a record. An intention stated in prose is not queryable and nothing checks it, which is
`OB-15`'s shape from the other side and the reason CLAUDE.md phrases the rule as capture **on
notice** rather than at task end.

One thing the fix session would add to that: the `unverified` field's prohibition (*"do not add
the early return without checking the in-process subagent path"*) was the most valuable line in
the file and also the one that came closest to costing the most. It is a claim about current
state written at the moment someone decided to stop, and it read as a settled judgement. Both
halves of it cleared in about ten minutes of reading — and the check turned up a precedent
(`call_tool_inner` already treating a write pin as consent, in a comment saying so) that made
the fix obviously right rather than merely defensible. Re-cost a deferral rationale; do not
inherit it.
