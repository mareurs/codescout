---
id: c752708c2757e139
kind: bug
status: investigating
title: Workspace read_only flipped to true twice mid-session with no activate call from this session, silently blocking every write
tags:
- workspace
- read-only
- concurrency
- multi-session
- tool-quirk
opened: 2026-08-26
owner: marius
related:
- '4574d18db7aacec8'
- '3be6b587a9c92a7a'
severity: high
unverified: MECHANISM confirmed for both symptom forms (read-only refusal AND silent wrong-project reads) at path:line. WHICH specific call triggers each occurrence is still not logged/pinned per-incident — occurrence 4 is the only one with a named trigger (SendMessage resume of a subagent).
---

## Summary

Twice in one long session the active project (`codescout`) went `read_only`, blocking every
write path, **without this session issuing any `workspace(activate)` call between the last
successful write and the failure.** Both times the remedy was
`workspace(action="activate", path="/home/marius/work/claude/codescout", read_only=false)`,
which succeeded immediately and restored writes.

The failure is loud at the call site but silent as a state change: nothing announces the
flip, so the first symptom is an unrelated-looking write refusal.

## Symptom (Effect)

```
File writes are disabled for this project. If this project was activated in
read-only mode, call workspace(action='activate', read_only: false) to enable writes.
```

The message is accurate and its hint is the correct remedy. The problem is not the message —
it is that the state changed underneath a session that never asked for it.

## Reproduction

Not reproduced on demand. Two observations from session `b02898c3` on 2026-08-26:

1. **Occurrence 1** — `create_file` to an absolute path in the session scratchpad
   (*outside* the project root). Immediately followed the completion of a background
   implementer subagent. Many `edit_markdown`, `artifact(update)` and `create_file` writes
   had succeeded earlier in the same session.
2. **Occurrence 2** — `edit_markdown` on `.superpowers/sdd/…/progress.md`, immediately after
   dispatching another implementer subagent. Between the two occurrences this session had
   completed several successful writes and two git commits.

In both cases the very next call — `workspace(activate, read_only=false)` — returned
`"read_only": false` and the retried write succeeded unchanged.

## Environment

- codescout MCP server shared across three Claude Code profiles (`~/.claude`,
  `~/.claude-sdd`, `~/.claude-kat`).
- **A peer session was live on this machine throughout** (`lang-pal-engine-3a`), and
  committed `20d5d43f` to codescout's `experiments` branch during this session — so it was
  not merely open, it was actively working in the same repo.
- This session had background subagents running against a *different* repo
  (`prompt-engineering`), pinned via the `workspace=` parameter rather than by activation.

## Hypothesis (NOT established)

The nearest prior art is `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md`
(`4574d18db7aacec8`, **fixed**): *"shared codescout server has one process-global active
project — concurrent activations silently cross-contaminate reads."* That fix addressed the
project **identity** being process-global. The plausible reading here is that the
`read_only` **flag** shares the same scope, so a peer session activating codescout read-only
flips it for every session on the process.

This is a hypothesis with a motive and an opportunity, and no evidence tying an activate call
to any particular caller. It should be checked, not assumed — see `unverified:`.


## Update (2026-08-26) — cross-session hypothesis refuted, cause narrowed

The cross-session/shared-server hypothesis above is **refuted**, not just unconfirmed.
`docs/manual/src/concepts/cross-process-write-serialization.md` documents that each Claude
Code session runs its **own separate** codescout server process; cross-session coordination
is a `.codescout/write.lock` flock for write ordering only, not shared memory. A different
OS process (e.g. a `lang-pal-engine-3a`-style peer) cannot mutate this session's in-memory
`read_only` field — there is no shared `AgentInner` across sessions.

The real mechanism is **within-session**, and it is not new: `Agent::activate`
(`src/agent/mod.rs:542-567`) still unconditionally runs `inner.workspaces.clear()` and
reassigns `default_workspace_root`, for any caller sharing this session's one process —
exactly the mechanism diagnosed in `3be6b587a9c92a7a`
(`docs/issues/archive/2026-08-23-subagent-activate-mutates-parent-active-project.md`,
status `mitigated`). That bug's own fix was documentation-only (a stronger imperative in
`docs/architecture/companion-plugin.md` § *Concurrent multi-workspace*) and its `unverified:`
field explicitly predicted this: "a subagent that ignores the briefing can still reproduce
the original failure." `explore-inject.mjs` was checked directly (source read) and is not
the cause — it correctly injects `workspace="<root>"` pin guidance, never an `activate` call.

This file is very likely a **rediscovery** of `3be6b587a9c92a7a`'s residual risk, one day
after that bug closed. The open question is no longer "what mechanism" (answered) but
"which specific call" — same as the archived bug's own unresolved gap. See that bug's Fix
section (options 1–3) for the real remediation choices; option 1 (docs) was already tried
and just failed to hold. Options 2 (declare unpinned-concurrent unsupported, documented) and
3 (structural code guard — blocked on MCP `RequestContext` having no per-caller identity)
remain undecided and are the actual open work, not anything specific to this file.

**Confirmation, not new discovery (downgraded 2026-08-26 after checking against
`get_guide("workspace-state")`, which already states the home/foreign read-only default
split verbatim in its § *The home/foreign distinction* — home: `read_only=false`, foreign:
`read_only=true`).** What's actually new here is tying that already-documented default to
the *parent-clobbering* mechanism above, not the default itself — codescout-8f's original
framing overstated it as a fresh finding. The byte-level trace still has standalone value as
proof the implementation matches the docs: `AgentInner::build_workspace`
(`src/agent/mod.rs:156-249`):

```rust
let is_home = self.home_root.as_ref().map(|h| h.as_path() == root).unwrap_or(true);
let effective_read_only = match read_only {
    Some(false) => false,
    _ if is_home => false,
    _ => true,
};
```

Activating any root that is not `home_root`, without explicitly passing `read_only=false`,
yields `effective_read_only = true` by design (a safety default for foreign roots). Combined
with `activate` clearing the registry and reassigning `default_workspace_root`, the full
mechanism is: a subagent calling `workspace(activate, path=<foreign project>)` without
`read_only=false` makes the **parent session's default workspace a read-only foreign
project** — not just a wrong one. This matches both occurrences (subagent lifecycle-adjacent,
subagents on non-home `prompt-engineering`) and the recovery (`activate(codescout,
read_only=false)` — the one arm that beats a non-home root). It also means the diagnosis cost
is worse than the archived bug's own framing: the write refusal names a cause ("activated in
read-only mode") the parent never enacted, inviting the wrong kind of debugging. A narrower,
cheaper partial mitigation this suggests: whether `activate` should infer `read_only` at all
when invoked with a foreign root, independent of the two structural options above.

**Occurrence 4 (codescout-8f, 2026-08-26, same day) — the more dangerous sibling form.**
Trigger precisely named this time: resuming a subagent via `SendMessage` (not dispatching
one). Between the parent's last codescout-relative call and the next, nothing else
happened — the subagent (working in `prompt-engineering`) had activated its own project,
and the parent inherited it. This produced the **wrong-project** form, not the read-only
form: `symbols`/`grep`/`read_file` on `src/agent/mod.rs` returned confident, correct-for-
`prompt-engineering` negative answers ("file not found", "0 matches") for a file that is
tracked and 131 KB in `codescout`. `workspace(action="status")` was the only thing that
surfaced the actual active root; nothing else pointed at it, and the negative results were
indistinguishable from a genuine absence until that one call was made.

**Why this raises severity (medium → high):** read-only *refuses* — loud, one wasted call,
obvious remedy. Wrong-project *succeeds against the wrong tree* — silent, and can manufacture
plausible-looking wrong findings about the very codebase under investigation. This session came
within one message of filing a false high-severity bug against codescout's own `grep`/
`read_file`/`symbols` ("blind to a tracked source file") before `workspace(status)` revealed
the real cause.

**Trigger list, broadened:** not just dispatching a subagent into a foreign root — *resuming*
one (`SendMessage` to an existing subagent) reproduces the identical mechanism, since nothing
about resumption changes which process/registry the subagent's tool calls land on.

**Diagnostic gap, independent of any structural fix:** `workspace(action="status")` resolves
both symptom forms instantly, and this project's OWN `get_guide("workspace-state")` already
documents the gap verbatim, pre-dating this bug: “Caller has no way to detect this without an
extra `workspace(status)` call.” Neither the read-only refusal message (“File writes are
disabled for this project...”) nor a silent wrong-project read points at it. Cheapest possible
fix, orthogonal to options 1–3 above: have the write-refusal hint suggest `workspace(action=
"status")` explicitly (“your active project may have been changed by a subagent”). Does not
help the silent wrong-project form (nothing errors to hang the hint on), but closes the
read-only form's diagnosis gap for near-zero cost.

**IMPLEMENTED 2026-08-26.** `check_tool_access` (`src/util/path_security.rs:568`) now appends
“If you didn't expect this, a subagent may have changed the active project — call
`workspace(action='status')` to check.” to the write-refusal message. Regression test
`file_write_disabled_message_points_to_workspace_status` (same file, added via TDD — watched
RED against the old message, GREEN after the change). Full gate green: `cargo fmt --check`,
`cargo clippy --lib -- -D warnings`, `cargo test --lib` (4360 passed, 0 failed, 8 ignored).
Committed `experiments` (label: **experiments**) sha `00948381d3ef06448e03552ed001d64e5499a1ab`,
patch-id `d7d6bc55f292fb3983613c57f7812dc74d6b880b`. The two structural options (2/3) remain
undecided and unimplemented — this only closes the diagnosis gap for the read-only form, not
the underlying clobber. Not archiving this file: root cause is still open.
## Hypotheses tried

- **"A subagent did it."** Plausible on timing — both occurrences sit next to subagent
  lifecycle events. But my subagents were told to pin with `workspace=`, not to activate, and
  they were operating on `prompt-engineering`, not codescout. Neither confirmed nor excluded.
- **"The scratchpad path is outside the project, so the guard is right."** Refuted by
  occurrence 2: `.superpowers/…` is inside the project root and was refused identically.

## Impact

Medium. Every write path refuses, and the refusal names a cause (`if this project was
activated in read-only mode`) that reads as speculative rather than diagnostic — so the
natural response is to doubt the path or the tool rather than the workspace state. The
recovery is one call and is stated in the hint, which is why this is not high severity. The
cost is a wasted call plus the risk that an agent mid-task interprets it as a permissions
problem and works around it, or a subagent silently gives up on a write it was asked to make.

## Fix

Not attempted. Two things worth checking before any change:

1. ~~Whether `read_only` is per-session or process-global in the current build.~~ **ANSWERED
   2026-08-26**: process-global-per-session (i.e. shared by everything on one session's
   process, not isolated per-caller). Not the unfinished half of `4574d18db7aacec8`
   (project *identity* resolution) — it's the already-known, still-unaddressed
   `default_workspace_root` clobber from `3be6b587a9c92a7a`. See the Update section above.
2. Whether the state change can be made **legible** regardless of scope — an activation that
   flips `read_only` for a session that did not request it should be visible in that
   session's next call, not discovered by a write refusal.

## Workarounds

`workspace(action="activate", path="<project root>", read_only=false)` restores writes
immediately. Cheap, and safe to issue whenever the refusal appears.

## References

- `docs/issues/archive/2026-05-30-shared-server-global-active-project-race.md` — the fixed
  process-global active-project race; same architecture, different field.
- `prompt-surface-measurement-session-log:F-3` — *"A subagent's `workspace(activate)` mutated
  the parent's active project"*, still `open`. Same surface, and the reason "a subagent did
  it" could not be dismissed out of hand here.
