---
status: open
opened: 2026-08-23
closed:
severity: high
owner: marius
related: []
tags: [workspace, subagents, concurrency, write-guard]
kind: bug
unverified: 'Root cause only half established — the global-state mutation is measured, but the write-authorization rule that made the session scratchpad unwritable under the foreign project is NOT: both projects declare identical file_write_enabled=true / extra_write_roots=[], so the differentiator is unread code, not config.'
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

**Established:** active-project selection is server-global, not per-caller. Subagents
share the parent's MCP server connection, so `activate` is a mutation visible to every
concurrent caller. This is the defect.

**Not established:** the specific write-authorization rule that refused the scratchpad
path under the foreign project. E-3 rules out per-project config as the differentiator,
so the remaining candidate is home-vs-foreign project semantics in the write guard —
inferred from `CLAUDE.md`'s existing warning ("After `workspace(activate, foreign)` →
activate home before turn end") and the `get_guide("workspace-state")` topic covering
"home/foreign", **not** measured. Do not treat the second paragraph as a finding.

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
   **Test:** not yet run — requires reading the guard that emits the error string.
   **Verdict:** deferred; currently the only surviving candidate.

## Fix

Not yet planned — hypothesis 3 must be settled first, because it decides whether the fix
is one change or two.

The **global-state** defect has a clear direction regardless: activation should be
per-caller, or subagents should be unable to mutate the parent's selection. The
per-call `workspace` parameter already exists on every tool and is documented for
exactly this case ("Absolute workspace path to resolve this call against; omit for the
active project. For concurrent subagents in different workspaces."), so the machinery
for caller-scoped resolution is present — `activate` is the surface that bypasses it.

Note this is the **inverse** of a family of already-fixed bugs (`edit_code`,
`memory`, `references`/`symbol_at`/`call_graph`, `artifact(find)` all once ignored the
`workspace=` pin). Those were "the pin is not honoured"; this is "the pin is honoured
but a peer can move the default underneath you". Fixing it should not regress them.

Secondary, same file: the error message should name the actual condition — that the
active project is `<X>` and the path lies outside its writable roots — rather than
asserting a read-only mode that may never have been set.

## Tests added

None — filed, not fixed. A regression test should assert that an `activate` issued on
one logical caller does not change the project a second concurrent caller's writes
resolve against.

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
- `get_guide("workspace-state")` — home/foreign activation semantics; the authoritative
  account hypothesis 3 needs checking against.
- `CLAUDE.md` § *Companion Plugin* — concurrent-multi-workspace rules.
- Workflow `wf_f348d3a6-a64`, agent `a2ae67537bf864aec` — the incident's trigger.
