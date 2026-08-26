---
kind: bug
status: mitigated
tags:
- workspace
- subagents
- concurrency
- write-guard
closed: 2026-08-25
opened: 2026-08-23
owner: marius
related: []
severity: high
unverified: Root cause (global mutable default_workspace_root, no per-caller identity) is NOT addressed — options 2/3 in Fix remain open. This closes the specific incident trigger (a Workflow script briefed to call activate) by strengthening the existing, already-correct-but-underweighted guidance; a subagent that ignores the briefing can still reproduce the original failure.
---

# BUG: a background subagent's workspace(activate) mutates the PARENT session's active project, breaking the parent's writes mid-turn

## Summary

`workspace(action="activate")` sets process-global state in the codescout MCP server.
A background subagent that activates a sibling repo therefore switches the **parent
session's** active project out from under it, with no notice. The parent's next write
fails, mid-turn, with an error that misattributes the cause to read-only mode. Any
long-running fan-out that touches a second repo can break its own dispatcher at an
arbitrary point.

## Symptom (Effect)

Parent session called `create_file` against its own session scratchpad — a path it had
already written to five times successfully in the same turn — and got:

```
File writes are disabled for this project. If this project was activated in read-only
mode, call workspace(action='activate', read_only: false) to enable writes.
```

`workspace(action="status")` from the parent then returned a project it never activated:

```
"project_root": "/home/marius/work/claude/prompt-engineering"
```

The message is also **misleading**: read-only mode was never set by anyone (see E-2).
It names the one fix that was not the cause, and does not mention that the active
project changed — which is the actual fact the reader needs.

## Reproduction

Measured 2026-08-23, branch `experiments`, codescout MCP, Claude Code v2.1.241.

1. From a session with `codescout` active, write any file to the session scratchpad via
   `create_file`. It succeeds.
2. Launch a background `Workflow` whose subagent prompt instructs
   `workspace(action="activate", path="/home/marius/work/claude/prompt-engineering")`.
3. While that subagent is alive, `create_file` to the same scratchpad path from the
   parent. It fails with the error above.
4. `workspace(action="status")` from the parent reports the sibling repo as the active
   project.
5. `workspace(action="activate", path="<codescout>", read_only=false)` restores writes.

Race-dependent on timing only in *when* it fires, not *whether* — any activate by any
concurrent agent is sufficient.

## Environment

Linux, codescout MCP over stdio, Claude Code 2.1.241. Parent session home project
`codescout`; workflow `wf_f348d3a6-a64` (8 agents) with one agent briefed to work in
`/home/marius/work/claude/prompt-engineering`.

## Root cause

**Fully established, chain traced 2026-08-25.**

1. `Agent::activate` (`src/agent/mod.rs:542-567`) is unconditionally global and
   caller-blind — there is no MCP-level concept of “which agent called this”
   (confirmed already rejected as infeasible in the sibling plan below: “Per-actor
   map: MCP `RequestContext` has no per-subagent key → impossible”). Every call, parent
   or subagent, runs `inner.workspaces.clear(); inner.workspaces.insert(root, ws);
   inner.default_workspace_root = Some(root);` under one write lock — wiping every
   OTHER resident (pinned) workspace too, not just changing the default.
2. `check_tool_access` → `security_config_for(None)` → `with_project_at(None, ...)`
   (`src/agent/mod.rs:644-671`, `683-690`) resolves an **unpinned** call against
   `inner.default_workspace()` — whichever project the *last* `activate()` call, by
   anyone, installed. There is no separate “home” fallback consulted here.
3. `AgentInner::build_workspace` (`:156-249`) computes `is_home = (root ==
   self.home_root)` and defaults `read_only = true` for any non-home activation with no
   explicit `read_only` — exactly what the subagent's bare `activate(prompt-engineering)`
   triggered (E-2: it passed no `read_only`).
4. `project_security_config` (`:399-411`) folds that runtime `read_only` bit into
   `file_write_enabled`, **overriding** `project.toml`'s static setting — this is what
   E-3 correctly ruled out as the differentiator; the differentiator is the runtime flag,
   not the file.

Hypothesis 3 is CONFIRMED, not just surviving: the write guard does check “the active
project,” and that project's `read_only` bit is the foreign subagent's default — not
anything the parent set or can see without an extra `workspace(status)` call.

**This is not new territory — it is a known, tracked, partially-closed gap.**
`docs/plans/2026-05-30-per-request-workspace-pinning.md` (status: draft, last touched
2026-05-31) designed and shipped exactly the fix this bug needs, for the axis it covers:
Phases 0–3 (read-tool pinning) and Phase 4a (write-tool pinning, incl. `create_file`,
`edit_markdown`) are marked **COMPLETE**, confirmed live in current `experiments` by
direct read of `with_project_at`/`ensure_resident`/`security_config_for`. A caller that
passes the `workspace=` pin on every call is already immune to this bug today.

What is NOT fixed, and is the plan's own explicitly-flagged, never-resolved question
(“Risks / open questions”): **`default_workspace_root` under concurrency.** *“a subagent
that does NOT pin still races the default slot. Decide before Phase 5: is
unpinned-concurrent simply unsupported (documented), or does the warning guard survive
only for the unpinned `default_workspace_root` path? … resolve it explicitly, don't let
it default.”* Nobody returned to decide it. This bug's own reproduction is a live instance
of exactly that gap: the triggering workflow briefed its subagent to call
`workspace(activate=...)` — the one form that mutates shared state — instead of the
already-existing, already-shipped `workspace=` pin.

**Additional finding beyond E-1..E-5: `activate()`'s blast radius is wider than “the
default changes.”** `inner.workspaces.clear()` drops every OTHER resident workspace too,
including one a different concurrent caller had pinned and was relying on via
`ensure_resident`. That caller's next `with_project_at(Some(root), ...)` re-runs
`ensure_resident` and should self-heal, but can hit a transient “pinned workspace not
resident” error first if the timing lands inside that window. Not reproduced directly;
follows from reading `with_project_at`/`ensure_resident`/`activate` together.
## Evidence

### E-1 — exactly one subagent activated the sibling repo

`grep` over the workflow's agent transcripts at
`~/.claude-sdd/projects/…/subagents/workflows/wf_f348d3a6-a64/`, 2026-08-23:

```
1 "path":"/home/marius/work/claude/prompt-engineering"
1 "path":"/home/marius/work/claude/prompt-engineering/tests"
```

Traced to `agent-a2ae67537bf864aec.jsonl` (the harness-anatomy recon agent). The parent
session issued no such call.

### E-2 — no subagent passed `read_only`

Same grep pass, over the same transcripts:

```
grep -ho '"read_only"[[:space:]]*:[[:space:]]*[a-z]*' "$D"/*.jsonl | sort | uniq -c
→ (no output)
```

Zero matches. So the error message's read-only hypothesis is not what happened, and the
message is misdirecting.

### E-3 — per-project config is NOT the differentiator

`.codescout/project.toml` `[security]` for both projects, 2026-08-23:

| Key | codescout | prompt-engineering |
|---|---|---|
| `file_write_enabled` | `true` | `true` |
| `extra_write_roots` | `[]` | `[]` |

Identical. This falsifies the first hypothesis (below) and is why the root cause is only
half established.


### E-4 — recurs under a *same-repo* subagent that reported restoring the home project

**2026-08-23, SDD execution of the hidden-information eval plan** (this repo, branch
`experiments`). Two further occurrences, both during a subagent-driven run where the
subagents worked in the sibling `prompt-engineering` repo:

1. The Task 1 implementer's final message ended with *"Home project restored."* The
   parent's very next `edit_markdown` on its own SDD ledger — a file under
   `codescout/.superpowers/`, i.e. squarely inside the home project — was refused with
   `File writes are disabled for this project`. Recovered with
   `workspace(action="activate", path=<codescout>, read_only=false)`.
2. Same shape after the Task 1 reviewer returned.

Three things this adds to E-1..E-3:

- **A subagent's own "restored" claim is not a restoration of the write bit.** Whatever
  the subagent restored, `read_only` came back true. So the parent cannot delegate
  recovery to subagent discipline, and "brief subagents to restore" is not a sufficient
  workaround.
- **The failure lands arbitrarily far from its cause.** It surfaces on the parent's next
  *write*, which in a dispatch-heavy session can be many tool calls and one or more
  notifications after the offending `activate`. Nothing links the two in the transcript.
- **A read-only parent still reads fine**, so every diagnostic the parent runs before its
  next write succeeds — the session looks healthy right up to the refusal.

Practical consequence for any controller-style session: after **every** subagent
returns, treat the write bit as suspect. Cheapest guard is to re-assert it before the
first write of each turn rather than to detect the refusal and recover.

### E-5 — a **read-only** reviewer subagent flips it, mid-flight, four calls after the parent re-asserted the write bit

**2026-08-23, same SDD run, third recurrence.** The strongest form of the evidence so far,
because it removes every remaining explanation but one.

Sequence, exactly as it happened in the parent:

1. `workspace(activate, path=<codescout>, read_only=false)` → `read_only: false` confirmed
   in the response.
2. `read_markdown` on a plugin template — fine.
3. `run_command` writing a diff file via shell redirect — **succeeded** (shell writes do not
   go through the guard, so they are not a signal either way).
4. Dispatched a **re-reviewer** subagent whose prompt said, verbatim: *"Your review is
   read-only on this checkout. Do not mutate the working tree, the index, HEAD, or branch
   state in any way."*
5. `run_command` with two `ls` calls — fine (a read).
6. `edit_markdown` on the parent's own ledger → `File writes are disabled for this project`.

What this rules out:

- **Not caused by the subagent writing anything.** It was instructed not to, and a reviewer
  has no reason to. Merely *activating* a project to read it is sufficient.
- **Not caused by subagent exit or cleanup.** The refusal arrived while the re-reviewer was
  still running. The window opens at the subagent's `activate`, not at its return.
- **Not a stale parent activation.** The parent had re-asserted `read_only=false` four tool
  calls earlier and received an explicit `"read_only": false` in the response.
- **Not detectable by reading.** Steps 2 and 5 both succeeded; every read-shaped diagnostic
  a parent might run to check its own health passes while the write bit is off.

Consequence for the workaround section: "brief subagents not to mutate state" does not
help, because no mutation is required. The only reliable guard available to a controller
today is to re-assert `read_only=false` **immediately before each write**, treating the
bit as unowned for the whole duration of any dispatch — which is three occurrences in one
session, all recovered, none prevented.
## Hypotheses tried

1. **Hypothesis:** the subagent activated the sibling repo in read-only mode.
   **Test:** grep every agent transcript for `read_only`.
   **Verdict:** rejected — E-2, zero occurrences.
2. **Hypothesis:** the sibling repo's `project.toml` disables writes, or its
   `extra_write_roots` excludes the scratchpad while codescout's includes it.
   **Test:** read `[security]` from both `project.toml` files.
   **Verdict:** rejected — E-3, the two are identical.
3. **Hypothesis:** the write guard authorises paths against the **home** project and
   treats a foreign activated project as write-restricted, so the scratchpad falls
   outside the writable set once the active project is foreign.
   **Test:** read `check_tool_access` → `security_config_for` → `with_project_at` →
   `build_workspace` → `project_security_config` (`src/agent/mod.rs`,
   `src/util/path_security.rs`), 2026-08-25.
   **Verdict: CONFIRMED**, precisely — see Root cause. The guard checks “the active
   project's `read_only` flag,” and an unpinned call's “active project” is whichever
   project the last `activate()` call (by anyone, home or foreign) installed as the
   global default.

## Fix

Not yet chosen — hypothesis 3 is now settled, but settling it surfaced a **policy
decision already on record and never made**, not a fresh engineering question. See
`docs/plans/2026-05-30-per-request-workspace-pinning.md` § Risks / open questions, the
“`default_workspace_root` under concurrency” bullet. Three live options, not mutually
exclusive:

1. **Dispatch-discipline fix (cheapest, addresses the actual incident). — TAKEN
   2026-08-25.** This bug's own trigger was a workflow prompt briefing a subagent to call
   `workspace(activate=...)` instead of the already-shipped `workspace=` per-call pin.
   Investigated the three prompt surfaces first (`server_instructions`/`onboarding_prompt`
   in `src/prompts/source.md`, `builders.rs`) — found the correct guidance ALREADY present
   in `get_guide("workspace-state")` (pull-only, triggered on `workspace(activate)` calls)
   and in `docs/architecture/companion-plugin.md` § *Concurrent multi-workspace*. The gap
   wasn't missing policy, it was that nothing grounds the rule in a real cost or puts it in
   front of a session at the moment it writes a `Workflow` script's subagent prompt.
   Strengthened `docs/architecture/companion-plugin.md` § *Concurrent multi-workspace* with
   an explicit imperative ("never brief a subagent to call `activate`") and a citation of
   this incident as the concrete cost of getting it wrong — no change to the tight-budget
   `server_instructions` 1900-char slice was needed or attempted. Zero code risk; does NOT
   structurally prevent a subagent that ignores the briefing from reproducing this.
2. **Resolve the plan's open question by declaring unpinned-concurrent unsupported,
   documented.** Keep `concurrent_activation_warning` on the unpinned
   `default_workspace_root` path only (per the plan's own recommendation in its Phase-5
   progress note), retire it for pinned flows, and document plainly that a session doing
   concurrent multi-repo subagent work MUST pin every call or accept the race. Matches
   existing CLAUDE.md guidance (“After `workspace(activate, foreign)` → activate home
   before turn end”) but makes the unpinned-default hazard explicit for the subagent case
   specifically, which that guidance doesn't currently name.
3. **A further code guard on top of pinning (most costly, not yet designed).** E.g. make
   `activate()` itself refuse to run inside a subagent context, or scope it so a foreign
   `activate` cannot silently displace an already-active home default without an explicit
   flag. Blocked on the same constraint the plan already hit: MCP's `RequestContext` has
   no per-caller identity, so “is this call from a subagent” cannot be answered directly
   without inventing a new signal.

Separately, regardless of which of the above is chosen: **the error message should name
the actual condition** — the active project is `<X>` and its `read_only` bit is set —
rather than asserting a read-only mode that reads as user-configured when it is actually
a default inherited from someone else's activation.
## Tests added

None — the fix is doc-only (a briefing-guidance edit to
`docs/architecture/companion-plugin.md`), no behavior changed, nothing to regress
against. Options 2/3 in Fix, if taken later, would each need the concurrent-pinning
regression the sibling plan's Phase 3 already modeled
(`docs/plans/2026-05-30-per-request-workspace-pinning.md` § Phase 3: “the 5-subagent
scenario from the bug file”).
## Workarounds

**Brief subagents to pass the per-call `workspace` parameter instead of calling
`activate`.** Every codescout tool accepts it. This was a dispatch defect in the
briefing that produced this incident: the workflow prompt instructed agents to
`activate` the sibling repo, which is the one form that mutates shared state.

If a subagent must activate, the parent should re-assert its own project before writing:

```
workspace(action="activate", path="<home>", read_only=false)
```

`workspace(action="status")` is the cheap check — it reports the project that will
actually be used, which is the fact the failing error message omits.

## Resume

Find the guard that emits the error string and read its authorisation rule — that
single read settles hypothesis 3 and determines whether the fix is one change or two:

```
grep(pattern="File writes are disabled for this project", glob="src/**/*.rs")
```

Then `symbols(name=<the guard fn>, include_body=true)` and check whether it resolves the
writable set against the **home** project or the **active** one, and where the session
scratchpad enters that set.

## References

- `docs/issues/archive/2026-08-17-worktree-reads-resolve-against-the-old-project.md` —
  nearest neighbour: writes blocked until activate after `EnterWorktree`, reads not.
  Different trigger (worktree, not a peer agent), same underlying axis.
- `docs/issues/archive/2026-07-09-edit-code-write-path-ignores-workspace-pin.md` and
  siblings — the inverse defect family, all fixed.
- `docs/plans/2026-05-30-per-request-workspace-pinning.md` — the plan that built the
  pin-based fix this bug needs. Phases 0–4a (COMPLETE) already solve this for any pinned
  caller; § Risks / open questions names the exact unpinned-concurrency gap this bug hit,
  flagged and never resolved since 2026-05-31.
- `get_guide("workspace-state")` — home/foreign activation semantics; the authoritative
  account hypothesis 3 needs checking against.
- `CLAUDE.md` § *Companion Plugin* — concurrent-multi-workspace rules.
- Workflow `wf_f348d3a6-a64`, agent `a2ae67537bf864aec` — the incident's trigger.

## Fix provenance

- **SHA:** `001c0a91` (`experiments`)
- **patch-id:** `5dd3d86fd214d28b7c852cc692827dbe8867be1a`

`docs(architecture): ground the subagent-activate warning in the actual incident` — option 1
(dispatch discipline) applied to `docs/architecture/companion-plugin.md`
§ *Concurrent multi-workspace*, plus this file.

The pointer records a **mitigation, not a fix**, and the two must not be read as the same
thing. The root cause in § *Root cause* — a process-global `default_workspace_root` with no
per-caller identity in MCP's `RequestContext` — is untouched by this commit; `unverified:`
in the frontmatter says so where a query can read it. Options 2 and 3 remain open, and
either would carry the concurrent-pinning regression this commit does not
(`docs/plans/2026-05-30-per-request-workspace-pinning.md` § Phase 3).
