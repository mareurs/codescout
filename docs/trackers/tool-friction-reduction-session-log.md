# Session Log — Tool Friction Reduction (2026-07-09)

> Work stream: implementing `docs/superpowers/plans/2026-07-09-tool-friction-reduction.md`
> via subagent-driven-development. Copied from `docs/templates/session-log.md`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-09 | med | codescout-tool | fixed-verified | Task 2's plan text omits `read_only=false` on the claude-plugins activation, which would have left the workspace read-only for the implementer's writes |
| F-2 | 2026-07-09 | high | architectural | mitigated | Controller's foreign-workspace activation raced a live background implementer subagent's MCP calls |
| F-3 | 2026-07-09 | med | architectural | open | `subagent-driven-development`'s `task-N-brief.md`/`task-N-report.md` scratch paths collide across unrelated plans |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|

---

## Category conventions

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

---

## Status vocabulary

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. |
| `promoted-to-bug-tracker` | Moved to a formal tracker. The session log keeps the pointer. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. |
| `archived` | Pattern no longer load-bearing. |

---

## F-1 — Task 2's plan text omits `read_only=false` on the claude-plugins activation

**Observed:** 2026-07-09, pre-dispatch reconnaissance for Task 2 of the tool-friction-reduction
plan (subagent-driven-development mode), while Task 1's implementer was running in the
background in codescout.

**When:** Scouting the claude-plugins seam (`buddy/skills/codescout-pika/sql/queries.sql`,
`buddy/skills/codescout-pika/SKILL.md`) before dispatching Task 2's implementer.

**Expected (plan):** Task 2 Step 1 reads `workspace(action="activate",
path="/home/marius/work/claude/claude-plugins")` — no `read_only` param — implying the
implementer can write immediately after.

**Got (scouted reality):** `workspace(activate, path=claude-plugins)` with no `read_only`
param returned `"read_only": true`. Per `get_guide("workspace-state")` § "The home/foreign
distinction": the **first** project activated in an MCP session is home (`read_only=false`
default); every subsequent activation to a different path is foreign (`read_only=true`
default) — writes are blocked at the agent layer unless `read_only=false` is passed
explicitly. codescout (this session's home) correctly defaulted to `false`; claude-plugins
(foreign) defaulted to `true`, confirmed by re-activating with `read_only=false` explicit
and observing the hint flip from "Browsing ... (read-only)" to "Switched project
(read-write)".

**Probable cause:** The plan's Task 2 Step 1 was written assuming `workspace(activate)`'s
generic tool-schema default (`read_only: false`) applies uniformly, without accounting for
the home/foreign-specific default documented in `get_guide("workspace-state")`. This is
documented, intentional server behavior (a safety default against accidental cross-repo
writes) — not a codescout bug — but the plan text didn't carry it forward.

**Workaround:** None needed — caught pre-dispatch. Plan Task 2 Step 1 revised in place to
`workspace(action="activate", path="/home/marius/work/claude/claude-plugins",
read_only=false)`.

**Severity:** med — without this catch, Task 2's implementer subagent would have hit a
`RecoverableError` on its first `edit_file`/`edit_markdown` write in claude-plugins, then had
to self-diagnose the read-only gate and re-activate correctly before retrying — one failed
tool call plus a self-diagnosis round-trip that the controller would otherwise absorb as an
implementer BLOCKED/NEEDS_CONTEXT report.

**Status:** fixed-verified — plan edit landed before any subagent ran; re-activation with
`read_only=false` confirmed writable via the `"Switched project (read-write)"` hint.

**Fix idea / Pointer:** Plan Task 2, Step 1, this session (`experiments`, pre-Task-2-dispatch).

---

## F-2 — Controller's foreign-workspace activation raced a live background implementer subagent's MCP calls

**Observed:** 2026-07-09, immediately after dispatching Task 1's implementer (background
agent) and while it was still running, the controller (this session) ran
`workspace(activate, path="/home/marius/work/claude/claude-plugins")` to pre-scout Task 2's
seam.

**When:** Task 1's implementer was mid-task, using the same shared codescout MCP server
session as the controller (per `get_guide("workspace-state")` § "Subagent semantics":
background/async subagents dispatched via the `Agent` tool share the parent's MCP server,
including the one shared active-project slot).

**Expected:** The controller assumed a foreign-workspace excursion during a background
subagent's run was harmless housekeeping, since the subagent had "already been briefed" and
should be self-contained.

**Got (scouted reality, from the implementer's own report):** The implementer's Task 1
report independently flagged: "the active workspace project was also independently observed
to flip to an unrelated project (`claude-plugins`) mid-task from what must have been a
concurrent session, requiring re-activation of `codescout` before continuing." This is a
first-hand account of exactly the hazard `get_guide("workspace-state")` names: "parallel
subagents that activate different workspaces race — last writer wins." Here it wasn't two
subagents racing — it was the controller's own recon excursion racing a background
implementer that shares the same server session.

**Probable cause:** `workspace(activate)` flips one global, session-wide active-project
slot. A background-dispatched subagent (async `Agent` call) is not isolated from this slot
just because it's "a different subagent" — it shares the literal same MCP server connection
as the controller. The controller's mental model ("subagents are isolated context, so my
tool calls can't affect them") is true for conversational/token context but false for MCP
server-side state.

**Workaround:** None applied retroactively — the implementer happened to notice the drift
(via a stale/wrong-project response) and self-corrected by re-activating `codescout` before
its next write. This was luck, not design: had the implementer been mid-`edit_file`/
`edit_code` write when the slot flipped, the write could have silently targeted the wrong
repo with no error (same shared session, different active root).

**Severity:** high — silent wrong-repo write is a realistic outcome of this race, not just
a failed call; this instance was caught only because the implementer's own defensive
re-check happened to fire before any write landed.

**Status:** mitigated — going-forward practice for the rest of this work stream: do not call
`workspace(action="activate", path=<foreign>)` on the controller side while any background
subagent is in flight. Use per-call `workspace=<path>` pinning (see
`get_guide("workspace-state")` § "Per-call workspace pinning") for controller-side
cross-repo reads instead of a shared activate, or wait for the subagent to report before
activating. Root cause (one shared active-project slot per MCP session) is architectural,
not something to fix in this session.

**Fix idea / Pointer:** Process discipline only, this session. Candidate `H-N` hookify /
`R-N` reconnaissance-pattern promotion if this recurs: "controller must not `activate` a
foreign workspace while a background subagent is in flight — pin per call instead."

---

## F-3 — `subagent-driven-development`'s `task-N-brief.md`/`task-N-report.md` scratch paths collide across unrelated plans

**Observed:** 2026-07-09, Task 2 implementer's completion report (background subagent),
flagged as an aside while reporting Task 2 done.

**When:** Implementer wrote its report to `.superpowers/sdd/task-2-report.md` per the
controller's dispatch instructions, and independently noticed the file already contained
stale content before it overwrote it.

**Expected:** `.superpowers/sdd/task-2-report.md` should either not exist yet (fresh plan)
or contain this work stream's own prior content.

**Got (scouted reality, via the implementer's own observation):** The pre-existing content
was from a *different, unrelated, already-merged* plan — the constitution-tracker work's
"Plan 2A Task 2: `find_matching_rules`" (visible in `.superpowers/sdd/progress.md`'s earlier
section, confirmed by the controller from having read that ledger section pre-dispatch).
`scripts/task-brief` and the implementer-dispatch convention both key report/brief filenames
purely off task index (`task-<N>-brief.md`, `task-<N>-report.md`) with no plan-name
component, so any two plans executed against the same repo checkout — even sequentially,
weeks apart — silently share and overwrite the same scratch files.

**Probable cause:** `.superpowers/sdd/` is a flat, plan-agnostic scratch directory; the
skill's scripts (`task-brief`, `review-package`) don't namespace by plan slug or session.

**Workaround:** None needed this time — the implementer overwrote cleanly and the stale
content was fully irrelevant, so no confusion resulted. The controller also always
regenerates `task-N-brief.md` immediately before each dispatch (never reads a pre-existing
one blindly), which happens to sidestep the read-side risk for this session specifically.

**Severity:** med — this time it was harmless (overwrite, no read-before-write of stale
data), but the failure mode is real: a controller resuming a session after compaction, or
running two plans interleaved, could read a stale `task-N-report.md` believing it reflects
the current plan's task, or a concurrent second SDD session in the same checkout could
race a write. Either produces silently wrong context fed to a reviewer or the controller
itself.

**Status:** open — not fixed; this is upstream tooling (the `superpowers:subagent-driven-
development` skill's scripts), not something to patch mid-plan in this repo.

**Fix idea / Pointer:** Candidate upstream fix: namespace `.superpowers/sdd/` scratch
files by plan slug (e.g. `task-<plan-slug>-<N>-brief.md`) instead of bare task index.
No action taken this session — flagging for whoever next touches the
`subagent-driven-development` skill's `scripts/task-brief` / `scripts/review-package`.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
