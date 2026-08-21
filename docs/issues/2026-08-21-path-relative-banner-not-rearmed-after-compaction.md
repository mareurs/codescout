---
status: open
opened: 2026-08-21
closed:
severity: low
owner: marius
related: []
tags: [server, compaction, progressive-disclosure]
kind: bug
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

Not yet reproduced end-to-end — the mechanism is read out of the code, see § Root cause for
what is and is not measured.

```
1. Start a session; make any tool call.        → response carries the banner
2. /compact
3. workspace(post_compact=true)                 → guide hints re-arm; banner does not
4. Make any tool call.                          → NO banner, and none for the rest
                                                  of the session
```

Step 3 is the discriminator: the guide-hint ledger re-injects after it, so a reader seeing
guides come back may reasonably assume the banner did too.

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

Not implemented. The one-line shape: extend `call_tool_inner`'s existing request-shape
match so the reset also fires on `post_compact: true`, alongside `action == "activate"`.

Worth doing as part of, or immediately after, the Project-Status carrier work in
`docs/trackers/statement-validity-session-log.md` F-9 — that work adds a second
response-carried block with the same persistence property and the same need to re-arm, and
the two flags should be reset by one branch rather than drift apart the way these two did.

## Tests added

None yet — not fixed. The regression test to write mirrors `post_compact_rearms_guide_hints`
(`src/server.rs:5979`): emit the banner, issue `workspace(post_compact=true)`, assert the
next eligible response carries it again. `responses_emit_paths_relative_annotation_once_per_activation`
(`src/server.rs:3667`) is the existing test that pins the once-per-activation half and must
keep passing.

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
