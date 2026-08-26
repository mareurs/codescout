---
kind: bug
status: fixed
tags:
- server
- compaction
- progressive-disclosure
closed: 2026-08-26
opened: 2026-08-21
owner: marius
related: []
severity: low
unverified: 'The downstream harm is still INFERRED, not measured: no session has been observed mis-resolving a project-relative path against its own cwd. The mechanism, the reproduction and the fix are all measured; the consequence that motivated the filing is not, and in most sessions cwd IS the project root, which is why severity stayed low.'
---

# BUG: the path-relative banner is not re-armed after compaction, so an agent loses the note that response paths are project-relative

## Summary

`[codescout] paths are relative to <root>` is emitted once per activation and lives in the
conversation, which is exactly what `/compact` discards. Its novelty flag is reset only on
`workspace(action="activate")` — **not** on `workspace(post_compact=true)` — so after a
compaction the note is gone from context and nothing brings it back for the rest of the
session unless the agent happens to re-activate. The sibling mechanism for guide hints
*does* re-arm on `post_compact`; these two disagree, and only one of them is right.

## Symptom (Effect)

After `/compact`, every subsequent tool response carries project-relative paths in
allowlisted path fields with no statement anywhere in context that they are relative. An
agent that reads `src/prompts/mod.rs` out of a response and resolves it against its own cwd
— rather than against the project root — is resolving against the wrong base.

Not observed in the wild as a wrong write; filed on mechanism, which is why severity is
`low` and not higher.

## Reproduction

**Reproduced end-to-end 2026-08-26**, in a live session rather than by reading code.
This file previously said "not yet reproduced" — that was true of the recipe it had
written for itself, not of the bug.

The session was resumed from a `/compact`, called `workspace(post_compact=true)` as
its first action, and then made roughly sixty tool calls. **None carried the
banner.** It reappeared only after an `/mcp` reconnect, which re-constructs the
server and so resets the flag through its initializer rather than through any
re-arm path:

```
1. Session resumes post-compaction.
2. workspace(post_compact=true)   → guide hints re-arm; NO banner
3. ~60 tool calls                 → NO banner on any of them
4. /mcp reconnect                 → banner returns (fresh server, flag starts false)
5. workspace(activate)            → banner (the one re-arm that worked)
```

Step 2 is the discriminator: the guide-hint ledger re-injects there, so a reader
seeing guides come back may reasonably assume the banner did too. It did not.

Also reproducible as a unit test through `call_tool_inner` — see § Tests added.
## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio transport, Claude Code client
with the codescout-companion SessionStart hook (which is what issues the `post_compact`
call at all).

## Root cause

Two novelty gates for two once-per-context facts, re-armed on different events.

- `path_note_emitted_since_activation` is an `AtomicBool` on `CodeScoutServer`
  (`src/server.rs:230`), consumed by `post_process` (`src/server.rs:731-742`) and reset in
  exactly one place: `call_tool_inner`'s `req.name == "workspace" && action == "activate"`
  branch (`src/server.rs:1085-1091`).
- `guide_hints_emitted` is cleared by `ProjectStatus::call`'s `post_compact` arm
  (`src/tools/config/mod.rs:323`), with a comment naming the reason: *"compaction
  summarized the guide bodies out of context, so allow them to re-inject."*

That reason appears to apply verbatim to the banner. **It does not, and this paragraph
originally said the opposite** — see § Hypotheses tried #2. The flag's own doc comment at
`src/server.rs:222-229` argues the banner is redundant once `build_server_instructions`
carries the root as system-prompt content, which compaction preserves. What survives is
narrower: those lines state the root, never the relative-path convention.

**Measured 2026-08-21:** `grep` over `src/**/*.rs` for `post_compact` returns 24 matches in
5 files, and none of them touches `path_note_emitted_since_activation` — the only writes to
that flag are its initializer (`src/server.rs:454`), the `activate` reset, and
`post_process`'s own `swap`. **Inferred, not measured:** that an agent actually
mis-resolves a path as a result. The gate's behaviour is established from the code; the
downstream harm is not.

A structural note that outlives this bug: the flag is reachable from `CodeScoutServer` but
not from `ToolContext`, while the `post_compact` handler has only `ctx`. That is why the
existing `activate` reset lives in `call_tool_inner`, matched on request shape — and it is
the natural place for the `post_compact` reset too.

## Evidence

`src/server.rs:1085-1091` — the sole reset, `activate` only:

```rust
if req.name == "workspace"
    && input_for_record.get("action").and_then(|v| v.as_str()) == Some("activate")
{
    self.path_note_emitted_since_activation
        .store(false, std::sync::atomic::Ordering::Relaxed);
}
```

`src/tools/config/mod.rs:314-323` — the sibling, which does re-arm:

```rust
if parse_bool_param(&input["post_compact"]) {
    ctx.lsp.shutdown_all().await;
    // Re-arm guide hints: compaction summarized the guide bodies out
    // of context, so allow them to re-inject.
    ctx.guide_hints_emitted.lock().clear();
```

## Hypotheses tried

1. **Hypothesis:** the banner survives compaction because it is in the system prompt.
   **Test:** read `post_process` (`src/server.rs:718-742`) — it pushes `Content::text` onto
   the `CallToolResult`. **Verdict:** rejected. That is conversation content, not system
   prompt. `server_instructions` is the system-prompt surface; the banner is not part of it.

2. **Hypothesis:** the asymmetry with `guide_hints_emitted` is an omission — nobody
   revisited the flag when `post_compact` was added.
   **Test:** read the flag's own doc comment, `src/server.rs:222-229`, which this file
   should have read before asserting an omission.
   **Verdict: REJECTED — a rationale is documented, and it covers this case.** It reads:
   *"the cold-reader signal U-23 protected … is now carried by the **Active project** +
   **Worktree** lines in `build_server_instructions`, which compaction preserves as
   system-prompt content. The per-response annotation becomes redundant after the first
   eligible call."* The argument is that the banner's job is done once a persistent surface
   carries the root, so not re-arming it is consistent rather than accidental.

   **This downgrades the bug and it is recorded rather than quietly deleted**, because the
   filing mistake generalises: a rationale sitting in a doc comment next to the code is a
   claim about current state, and it is the cheapest thing in the world to read before
   filing against it. Not doing so produced a file whose § Root cause said *"the asymmetry
   reads as an omission rather than a decision"* about a decision written down twenty lines
   above the flag.

## What is left of it

One gap survives hypothesis 2, and it is narrower than the original filing:

**The persistent lines state the ROOT; they never state the CONVENTION.** `- **Active
project:** codescout at `/path`` tells an agent where the project is. It does not say that
allowlisted path fields in tool responses are rendered relative to it. The banner is the
only surface that says so in push form, and after a compaction it is gone.

The convention IS documented in `get_guide("progressive-disclosure")` § Path-relative
annotation, and that ledger *does* re-arm on `post_compact` — so an agent that triggers
that topic again recovers it. Whether that is sufficient is a judgement about how reliably
the topic re-triggers, not a mechanism question, and this file does not have the
measurement to settle it.

**Severity dropped to `low` and status left `open` on that narrow question**, not on the
original "the flag was forgotten" framing, which is refuted.
## Fix

**Shipped 2026-08-26** — `dd4dcad6` on `experiments`, patch-id
`343ec8234b7e089c39d1612ae39d99a2a65e6d3e`.

Implemented in the shape this section prescribed, including the part about the two
flags: `call_tool_inner`'s existing request-shape match now resets
`path_note_emitted_since_activation` **and**
`status_block_emitted_since_activation` from a single `if is_activate ||
is_post_compact` branch, rather than from two branches one clause apart. Resetting
them together is the actual recurrence guard — the divergence, not the missing
clause, is what this bug was.

The timing condition in the original plan is satisfied: the Project-Status carrier
work it wanted to land alongside (`statement-validity-session-log` F-9) has already
shipped as `status_block_emitted_since_activation`, so there was nothing left to
wait for.

Both field doc comments were corrected in the same commit, because both stated the
old design as intentional:

- `path_note_emitted_since_activation` argued the banner goes redundant once
  `build_server_instructions` carries the root. Now says what § What is left of it
  established — that holds for the ROOT and not for the CONVENTION.
- `status_block_emitted_since_activation` described itself as "deliberately a
  SEPARATE flag with a WIDER reset". The reset is no longer wider; the flags stay
  separate because they are *consumed* against different facts. "Reset them
  together, consume them separately."

Leaving those two comments in place would have re-argued the old design to the next
reader, which is how the divergence survived in the first place.
## Tests added

`compaction_rearms_the_path_relative_banner` (`src/server.rs`), driven through
`call_tool_inner` so it exercises the request-shape match rather than
`post_process` in isolation.

It asserts a **count**, not a particular response: the banner must appear exactly
once across the `post_compact` reply and the next two ordinary calls. The contract
is "compaction brings the fact back, once" — not "it rides on the post_compact
reply". Without the fix the count is 0; a double-arming regression of the kind
`activation_and_the_next_two_calls_carry_the_banner_exactly_once` guards would make
it 2.

It loops over **both shapes** of the compaction signal — bare `{post_compact: true}`
(what the companion hook sends) and `{action: "status", post_compact: true}` (what
the guide-hint sibling sends). Now that both gates share one `if`, covering the
second shape looks redundant with
`both_shapes_of_the_compaction_signal_rearm_the_status_block`. It is not, and this
was checked rather than argued: adding `action.is_none()` to the reset condition —
the exact trap the code comment warns about — fails the second shape with 0 banners
instead of 1. "Covered by construction" is precisely the argument a mutation
defeats, and that sibling test was itself written from a surviving mutation.
## Workarounds

Call `workspace(action="activate", path=<same root>)` after a compaction — it re-arms the
banner as a side effect. The companion hook already prescribes `workspace(post_compact=true)`
at that moment, so this is one extra call in a place an agent is already making one.

## Resume

Apply the fix in `src/server.rs:1085-1091`: widen the condition to
`action == "activate" || parse_bool_param(&input_for_record["post_compact"])`. Confirm
`workspace(post_compact=true)` reaches `call_tool_inner` with `req.name == "workspace"` —
note that `post_compact` without an `action` infers `action="status"`
(`src/tools/config/mod.rs:57`), so the match must not require `action` to be absent. Then
write the regression test named in § Tests added.

## References

- `docs/trackers/statement-validity-session-log.md` — F-9, the Project Status carrier work
  that surfaced this
- `src/server.rs:5979` — `post_compact_rearms_guide_hints`, the sibling that behaves correctly
- `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md` — the bug that
  established the re-arm-on-compaction rule for guide hints
