---
id: e1cc4bacb962df38
kind: bug
status: open
title: 'BUG: guard_worktree_write refuses a correctly-pinned write, so the remedy the docs prefer over re-activating does not work'
tags:
- cluster/guard-narrower-than-its-name
- worktree
- workspace-pin
- guard
- write-path
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md
- docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md
- docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md
severity: medium
unverified: 'Mechanism read at the bytes AND reproduced live in one session (pinned edit_code refused, identical unpinned call refused, activate then succeeded) — so the symptom is observed, not inferred. What is NOT established is the fix''s safety: whether an early return on workspace_override is sound on the WRITE path depends on where the pin is validated, and this file checked only the peer-dispatch strip. The in-process subagent path is unexamined. Do not add the early return without that check — the read-side twin (7a3aee93) could early-return safely because its worst case is silence, and a write guard''s is not.'
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

Not implemented. The change is one early return, mirroring the notice:

```rust
if ctx.workspace_override.is_some() {
    return Ok(());
}
```

**Placed BEFORE the `is_project_chosen_this_session()` check** — it is the cheaper test and does
not await.

**Two things a fix must not assume, both of which need checking first:**

1. **That the pin is validated elsewhere.** The read notice could early-return safely because
   its worst case is staying quiet. A *write* guard early-returning on an attacker- or
   accident-supplied `workspace` is a different risk class. Peer dispatch strips the argument
   (`handle_tool_call_inner`, the fix for
   `docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md`), so the peer path is
   covered — but that is one caller, and this needs the in-process subagent path checked too
   before the return is added.
2. **That `is_project_chosen_this_session()` is the only other gate.** `docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md`
   records this guard being gated on the *wrong* flag for months while reading as live. A
   second wrong flag is the failure mode with precedent here.

The regression test must drive a **write** tool with a pin and assert it succeeds, and must be
paired with an unpinned call on the same context that is refused — otherwise the success is
satisfiable by a guard that does nothing at all.

## Resume

Unclaimed. Noticed while it blocked this session's own work during the `issue-clusters` split
(`ece4f21a`), and filed late: the notice went into a chat message — *"filing that after"* — and
an hour of unrelated work passed before it became a record. An intention stated in prose is not
queryable and nothing checks it, which is `OB-15`'s shape from the other side and the reason
CLAUDE.md phrases the rule as capture **on notice** rather than at task end.

