---
id: '986e8146fc44d17d'
kind: bug
status: open
title: 'BUG: the worktree read notice prescribes two calls a peer-serve client is forbidden from making'
tags:
- cluster/gate-keyed-on-unobservable-event
- peer-serve
- worktree
- notice
- unactionable-remedy
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md
- docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md
severity: low
unverified: 'Filed from a bytes-level read of two mechanisms, not from an end-to-end reproduction: peer-serve is an explicit `codescout peer serve` surface this session did not stand up, so the observable claim (a served read emits the notice) is INFERRED from the strip + call_content path rather than seen. What IS established at the bytes: `workspace` is absent from PEER_EXPOSED_TOOLS and named in the deny comment; handle_tool_call_inner removes the `workspace` argument before dispatch; the notice body names both calls. Anyone fixing this should reproduce first — CLAUDE.md''s rule that the plan is a hypothesis about the reproduction applies with full force here, because the fix option a reader finds most attractive (option 3) is the one that writes to the argument the strip exists to control.'
---

## Summary

`worktree_read_notice` (`src/tools/core/types.rs`) tells the caller to run
`workspace(action='activate', …)`, or to pass `workspace="<abs path>"` on a single call.
A peer-serve client can do **neither**, by construction:

- `workspace` is not in `PEER_EXPOSED_TOOLS` (`src/peer/server.rs:23`) and is named in the
  deny-by-default comment at `:311`. Calling it returns `AccessDenied` before dispatch.
- `handle_tool_call_inner` **strips** any caller-supplied `workspace` argument
  (`src/peer/server.rs`, "Scope every peer call to the SERVED workspace"), deliberately —
  that strip is the fix for `docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md`
  and must not be relaxed.

So a served read that trips the notice receives advice it is structurally forbidden from
following. The notice's **information** is correct — results really do describe the served
root while linked worktrees exist — but its **remedy** names two calls the recipient cannot
make.

## Symptom (Effect)

A peer-served agent reading a worktree-bearing checkout gets, on every unpinned read:

    Reads are resolving against "<served root>". … Call workspace(action='activate',
    path="<worktree>") to pin the tree you mean, or pass workspace="<abs path>" on a
    single call.

Both suggestions fail: the first with `AccessDenied`, the second silently, because the
argument is removed before the tool sees it. A silent failure is the worse of the two — the
served agent has no way to learn its remedy was dropped.

## Reproduction

Not reproduced end-to-end; established by reading the two mechanisms at the bytes
2026-09-02 (`PEER_EXPOSED_TOOLS` at `src/peer/server.rs:23`, the strip in
`handle_tool_call_inner`, and the notice body in `src/tools/core/types.rs`). Filed at that
strength deliberately rather than at "confirmed", because peer-serve is an explicit
`codescout peer serve` surface this session did not stand up.

## Root cause

The notice is written for the MCP-session caller, who holds both remedies. Peer-serve
reaches the same code with neither. `ToolContext` carries no marker distinguishing the two:
its `peer` field is the MCP `Peer<RoleServer>` handle, not a peer-serve flag, and
`home_root` cannot discriminate either — peer-serve makes the served root the home, but so
does an ordinary session's `current_dir()` startup fallback, which is precisely the case the
notice exists to fire on.

## Prior state, and why this is newly worth filing

The notice used to be one-shot per conversation (`notice_once`), so peer-serve saw one dead
message and nobody noticed. Removing that gate — the fix for
`docs/issues/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`,
which the one-shot was silencing in the state it exists to report — makes it one per served
read. **The change is right and this consequence is real**: the same removal that makes the
notice useful to a session that can act on it makes it repetitive for one that cannot. Named
here rather than absorbed, because the tell for the whole class is CLAUDE.md's *"name the
observer who acts on what it emits"* — and here that observer was never checked.

## Fix

Not implemented. Three candidates, none free:

1. **Give `ToolContext` a peer-serve marker** and suppress the notice for served calls. Least
   code, but adds a field whose only reader is a notice, and the suppression hides a fact the
   served agent might still want.
2. **Keep the notice, change the remedy for served calls** — name the served root and say the
   pin is fixed by the server rather than suggesting an uncallable tool. Preserves the
   information, costs a second message form.
3. **Set `workspace_override` to the served root at peer dispatch**, after the strip. It is
   semantically exact — peer-serve *was* given its root — and silences the notice through the
   existing pinned-call check with no new field. But it writes to the very argument the strip
   exists to control, so it needs a test proving a caller-supplied value still cannot survive.

(2) preserves the most and asserts the least; (3) is the tidiest and the riskiest. Decide
before writing code.

## Resume

Unclaimed. The decision above is the whole of the work; the code is small under any of the
three. Anyone taking it should read `2026-06-01-peer-workspace-arg-pin-escape.md` first if
they are drawn to option 3 — that bug is what the strip is for.

