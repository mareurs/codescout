---
id: d5566f4c24ceb601
kind: bug
status: open
title: Workspace activation is process-wide, so a subagent's read-only activate disables the controller's writes
tags:
- cluster/shared-resource-carries-no-owner
closed: ''
opened: 2026-09-01
owner: marius
severity: low
---

## Summary
`workspace(action="activate", read_only=…)` sets a **process-wide** default that every caller
sharing the MCP process inherits — including a controller whose subagent activated it. During
an SDD run the Task 1 implementer activated the worktree read-only; the controller's next
write to a file in a *different* checkout was refused, naming an active project the
controller never chose. Nothing owns the setting, so nothing can arbitrate between them.

The tool's own error text already recognises the shape — *"If that is not the project you
expected to be in, something else sharing this process activated it — a subagent, or another
caller on this session — because activation replaces the default for everyone in the
process."* — so the diagnosis is shipped; what is missing is a remedy that does not require
taking the substrate away from the other party.

## Symptom (Effect)
```
File writes are disabled: the active project is
/home/marius/work/claude/codescout/.worktrees/audit-shards-t7 and it was
activated read-only.
```
…on a path under `/home/marius/work/claude/codescout`, which is neither read-only nor the
project the caller was working in.

The remedy the message offers — re-activate with `read_only: false` — is **the one move the
controller must not make**, because it is equally process-wide and would flip the substrate
under a running implementer mid-task.

## Reproduction
1. Start an SDD run; create a worktree.
2. Dispatch a subagent that calls `workspace(action="activate", path=<worktree>,
   read_only=true)`.
3. From the controller, `edit_markdown` or `create_file` any path in the **main** checkout.
4. Refused, naming the worktree.

## Environment
codescout `experiments` @ `0ed6cb18` (+ worktree `audit-shards-t7`). Observed 2026-09-01
during the T-7 SDD run, between the Task 1 dispatch and its report.

## Root cause
Activation state is a single process-global with no owner and no scope. Two independent
callers in one process each hold a legitimate, different answer to "what is the active
project", and the data structure can represent only one. Note this is the *general* form of
a hazard CLAUDE.md already documents for the shared `target/` directory and for the shared
git index — same process, same resource, no ownership channel.

## Evidence
### The escape hatch exists but is not in the redirect chain
Every mutating codescout tool takes a per-call `workspace` parameter, documented as *"For
concurrent subagents in different workspaces"* — which resolves this exactly. The controller
used it and proceeded without disturbing the implementer.

But nothing in the failure path names it. The refusal message offers only re-activation; and
the native fallbacks are both gated toward codescout:

- native `Edit` → *"codescout's edit_code is the safer path for structural source edits"*
  (fired on a **markdown** file, which `edit_code` does not handle)
- native `Write` → *"codescout's create_file is the tracked path for new source files"*

So a caller following the redirects lands on tools that are themselves read-only, and the
only correct move is a parameter none of the three messages mentions.

## Hypotheses tried
None — the `workspace=` parameter worked first try, so no debugging was needed.

## Fix
Two candidates, not mutually exclusive:

1. **Name the parameter at the refusal site.** The read-only error should offer
   `workspace="<path>"` as the non-destructive option *before* it offers re-activation, and
   should say why: re-activation is process-wide and will affect other callers.
2. **Record who activated.** Store the activating agent id alongside the active project so
   the message can say *"activated read-only by subagent <id> 4 minutes ago"* rather than
   *"something else sharing this process"*. That converts an unattributable shared resource
   into an attributable one, which is the `IC-17` remedy shape.

Deliberately **not** proposed: per-agent activation state. That is a much larger change and
the per-call parameter already covers the need.

## Tests added

Regression test `read_only_refusal_offers_the_per_call_pin_before_reactivation`
(`src/util/path_security.rs`). **The assertion is ORDER, not presence** — it takes
`find()` positions and requires the `workspace=` offer to precede `read_only: false`.
Two `contains` checks would pass with the pin advice appended at the end, and that
arrangement is this bug, not its fix: a caller takes the first remedy that fits. Do not
relax it to `contains`.

Verified before writing the message that the advice actually works, rather than trusting
the field report: `check_tool_access(name, workspace_override)` →
`Agent::security_config_for` (`src/agent/mod.rs:712`) → `with_project_at(override)` → that
project's own `project_security_config`. A pinned write genuinely does not consult the
read-only project's config.
## Workarounds
Pass `workspace="<abs path>"` on every mutating call while a subagent may hold the
activation. Do **not** re-activate to fix it mid-run.

## Resume

**Fix 1 is implemented (uncommitted, `experiments` working tree, 2026-09-02).** Both
affected arms of `check_tool_access` now offer `workspace='<absolute path>'` *first* and
label re-activation as process-wide: the `ActivatedReadOnly` arm, and the `None`
(unattributed) arm, which hedges toward the same read-only cause and had the same gap.
The `ConfiguredOff` arm was deliberately left alone — its cause is config, not activation,
and its own test exists to stop it offering remedies that do not apply.

The durable guidance went to `get_guide("workspace-state")` § *Per-call workspace
pinning*, which already had a pinning section and now carries the read-only-collision
case. It could **not** go on the `server_instructions` slice: that surface has ~200 chars
of headroom under `STATIC_SLICE_CHAR_BUDGET`, and a one-line quickref addition tripped
`the_tier_split_leaves_real_headroom_in_the_persistent_channel`, which reserves ≥120 chars
for the dynamic `## Project Status` block. The slice carries a two-word pointer instead
(`activate, home/foreign, pinning, reset`). No `ONBOARDING_VERSION` bump —
`server_instructions` is live-on-connect and `builders.rs` was untouched.

**Still open: Fix 2** (record *who* activated, so the message can name the subagent and
the time instead of "something else sharing this process"). That is the durable one and
the root cause — ownerless shared state — is untouched; weigh it against `IC-17`'s other
members rather than building it for this bug alone. Flip this file to `mitigated` with a
`closed:` date, fix SHA and patch-id when the Fix 1 change is committed.
## References
- `.superpowers/sdd/2026-09-01-committed-audit-shards/progress.md` § Observations during the run
- `docs/trackers/catalog-audit-trail-session-log.md` (T-7 run)
- Cluster: `IC-17` — a shared resource carries no owner, so enumerating the peer does not help
