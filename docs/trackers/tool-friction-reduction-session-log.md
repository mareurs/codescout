# Session Log — Tool Friction Reduction (2026-07-09)

> Work stream: implementing `docs/superpowers/plans/2026-07-09-tool-friction-reduction.md`
> via subagent-driven-development. Copied from `docs/templates/session-log.md`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-09 | med | codescout-tool | fixed-verified | Task 2's plan text omits `read_only=false` on the claude-plugins activation, which would have left the workspace read-only for the implementer's writes |

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

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
