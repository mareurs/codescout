---
id: '986e8146fc44d17d'
kind: bug
status: fixed
title: 'BUG: the worktree read notice prescribes two calls a peer-serve client is forbidden from making'
tags:
- cluster/hint-composed-without-the-request
- peer-serve
- worktree
- notice
- unactionable-remedy
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md
- docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md
severity: low
verified: '2026-09-19 — the observable claim (a served read emits the notice) is now OBSERVED, not inferred. Reproduced end-to-end before implementing, per this file''s own instruction: linked worktree fabricated via a .git/worktrees/<name>/gitdir file, peer socket stood up, tool.call for `tree` sent; the notice fired verbatim, naming both calls peer dispatch denies. The fixture survives as `worktree_notice_on_a_served_read_never_prescribes_an_activate_or_pin_call` in src/peer/server.rs.'
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
`docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`,
which the one-shot was silencing in the state it exists to report — makes it one per served
read. **The change is right and this consequence is real**: the same removal that makes the
notice useful to a session that can act on it makes it repetitive for one that cannot. Named
here rather than absorbed, because the tell for the whole class is CLAUDE.md's *"name the
observer who acts on what it emits"* — and here that observer was never checked.

## Fix

**FIXED 2026-09-19** — `e208bc258a79ceed561a7ba852ba691328448dfc`, patch-id `0bf2dc88fe776ad5dea07e90129b6e5767d06854`.

**The premise is now OBSERVED, not inferred** — the thing this file's `unverified:` field asked for, done before implementing. A linked worktree was fabricated via a `.git/worktrees/<name>/gitdir` file, a peer socket stood up, and a `tool.call` for `tree` sent: the notice fires on a served read, verbatim, including both unreachable calls.

Option 1 shipped, not option 2: the DISCLOSURE is kept and only the PRESCRIPTION swapped, because a served agent needs to know which tree answered more than an interactive one does — no human is reading the envelope. Peer-serve now gets the banner plus "this connection is served against a fixed workspace and cannot be repinned from here"; every other caller is unchanged.

The discriminator is set AT the seam this file names — `call_tool_by_name`, peer-serve's only production caller — as a `tokio::task_local!` scoped around the single future it drives, read by `worktree_read_notice`. **Not a `ToolContext` field, and the deviation is disclosed:** that struct is built by bare literal at ~20 sites with no `Default`, so a required field breaks all of them. Verified rather than assumed: no `tokio::spawn` breaks the task between scope and `call_content`, and `unwrap_or(false)` degrades to today's behaviour rather than to a new failure. Option 3 remains struck.

Not implemented, but the blocking unknown is now **resolved**: a discriminator exists, and this
file's first draft was wrong to say otherwise.

### The discriminator exists, one frame above where it was looked for

The first draft searched *state* — `ToolContext.peer`, `home_root` — and correctly found
nothing that separates peer-serve from an ordinary session's startup fallback. `codescout-17`
proposed looking at the **action** instead, on the general form *"the discriminator lives at
the action that creates the ambiguity, not in the state it leaves behind"*, and offered it
explicitly as an unchecked hypothesis. Checked 2026-09-02, and it holds — at a site one frame
better than the one proposed.

17 suggested stamping a boolean where `handle_tool_call_inner` strips the `workspace` argument.
That works, but the cleaner seam is `CodeScoutServer::call_tool_by_name`
(`src/server.rs:927`), whose own doc comment reads *"Used by the peer-serve endpoint"* and
whose `cfg_attr` calls it *"peer-serve dispatch entry"*. It is `pub(crate)`, and peer dispatch
is its only production caller — it exists **for** peer-serve and for nothing else. It already
passes `None, None` down to `call_tool_inner`, which is where the `ToolContext` is built
(`src/server.rs:~600`, `workspace_override: None`).

So the flag is a parameter threaded through a function that already means "this is peer-serve",
not a fact inferred downstream from state that cannot carry it. **That also retires option (3)
below**: nothing needs to write to the stripped argument, so the pin-escape guard is never
near the change.

### The disclosure and the prescription are separable — and only the prescription is wrong

Also `codescout-17`'s, and it reframes the question this file was going to ask. The first draft
treated the choice as *suppress the notice for peer-serve, or leave it*. Both are wrong,
because they bundle two things the notice does:

- **which tree answered** — the fact the notice exists to convey. A served agent needs this
  *more* than an MCP session does, not less: it is the case with **no human reading the
  envelope**, so a wrong-tree read is acted on rather than noticed.
- **what to do about it** — `activate` / `workspace=`, both unavailable to it.

Suppressing removes the half that is useful to keep the half that is useless. So the peer-serve
variant likely wants **the banner minus the remedies**, naming the served root and saying the
tree is fixed by the server. With the discriminator above, that split costs one branch.

### Remaining options

1. **Peer-serve variant of the notice** — banner without the prescription, gated on a flag
   threaded from `call_tool_by_name`. *Recommended.* Preserves the disclosure, drops only the
   part the recipient cannot act on.
2. **Suppress entirely for served calls.** Cheapest, and it discards the fact the notice
   exists to convey, in the one setting where nobody is watching for it.
3. ~~**Set `workspace_override` to the served root at peer dispatch.**~~ **Struck** — it was
   only attractive while the discriminator seemed absent, and it writes to the very argument
   `handle_tool_call_inner` strips to fix
   `docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md`. Kept struck rather than
   deleted, because it is the option a reader arrives at on their own.

**Still not reproduced.** Everything above is read at the bytes; peer-serve was never stood
up. Per CLAUDE.md, run the reproduction before implementing — the plan is a hypothesis about
it, and the specific thing to check is whether a served read emits at all, since the whole
file rests on that inference.
## Fix provenance

- **SHA:** `e208bc258a79ceed561a7ba852ba691328448dfc` (`experiments`)
- **patch-id:** `0bf2dc88fe776ad5dea07e90129b6e5767d06854`

Formalised 2026-09-23 from a pair this record already stated in § *Fix* prose. `doctor`'s
`terminal_status_without_fix_anchor` cannot parse a hash in running text, so the record read as
anchored while nothing resolved it -- which is the failure mode that check exists to name. The
pair was not guessed: the SHA resolves and is an ancestor of `experiments`, and its patch-id
recomputed from the diff equals the one this file already carried. The SHA is positional and
dies on the next rebase of `experiments`; the patch-id is a content hash of the diff and
survives rebase and cherry-pick, which is why both are recorded rather than either.

## Resume

Unclaimed. The decision above is the whole of the work; the code is small under any of the
three. Anyone taking it should read `2026-06-01-peer-workspace-arg-pin-escape.md` first if
they are drawn to option 3 — that bug is what the strip is for.
