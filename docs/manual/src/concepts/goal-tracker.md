# Goal-Tracker Archetype

> **Status:** experimental — see [Experimental Features](../experimental/index.md).

A **goal-tracker** is a tracker artifact (`kind=tracker`, `tags: ["goal"]`)
that names a single completion criterion and aggregates the state of typed
child trackers. It is the project's standing answer to "what are we trying
to finish, and how far along are we?"

## When to create one

Create a goal-tracker when you are starting work toward a stated objective
with a definable "done" line — a launch, a migration, an audit pass, a
metric target. The archetype is **not** appropriate for:

- Open-ended research → use a `reflective` tracker.
- A bare task list → use `task_list` directly.
- Anything without a clear completion criterion.

A goal requires **2+ child sub-trackers**. If a single criterion can be
checked directly (one failure table, one metric series), use the underlying
archetype instead.

## At most one active goal per project

Only one goal-tracker per project may have `status=active` at a time. If
multiple active goals exist simultaneously, the Stop hook fails open
(deferring) and the librarian context surfaces them in `created_at` order.
This is a soft guard, not a hard constraint — but it nudges projects toward
one named focus at a time.

## Discovery

The active goal is auto-surfaced — you do not have to remember to read it.

**Find the active goal directly:**

```text
artifact(action="find",
         kind="tracker",
         filter={"tags":{"in":["goal"]}, "status":{"eq":"active"}})
```

**Get richer context including active goals plus other project signal:**

```text
librarian(action="context")   # no anchor — auto-includes active goals
```

The no-anchor mode of `librarian(action="context")` is the canonical entry
point: it returns the active goal alongside other live tracker signal, so a
fresh session sees the goal without having to query for it.

## Creation flow

```text
# 1. Get the archetype teaching prompt + skeleton
librarian(action="tracker_design", intent="goal: <one-line objective>")

# 2. Create the artifact with augmentation in one call
artifact(action="create",
         kind="tracker",
         tags=["goal"],
         title="<human title>",
         augment={ prompt: "...", params: {...} })

# 3. Link child sub-trackers (one or more existing archetypes)
artifact(action="link",
         src_id="<goal-id>",
         dst_id="<child-id>",
         rel="child")
```

Children use existing archetypes — `failure_table`, `task_list`,
`metric_baseline`, `audit_issues`, `reflective`, `deployment_state`, or a
nested `goal` for multi-level objectives.

## Aggregation

A goal does not store progress directly; it aggregates the deterministic
status of its children. The Rust side (`goal_aggregation::child_status_pure`)
computes each child's status by archetype-specific rules; the augmentation
prompt consumes those statuses to render the goal's body. This split keeps
the source of truth in the children and the synthesis in one place.

## Why the constraints exist

- **2+ children:** a single criterion does not need a goal wrapper. The
  goal archetype's value is rolling up *multiple* signals into one
  completion line; with one child, the goal adds nothing the child does
  not already provide.
- **One active per project:** competing active goals split attention. The
  guard makes the chosen focus visible — if you genuinely have two parallel
  goals, the failure-open behaviour lets you proceed, but the context now
  surfaces both so the divergence is not silent.
- **Evidence anchored to commits:** the goal's `evidence_commits` field
  cites the git commits that moved each child forward. Anchoring keeps the
  goal time-travel-safe (`artifact_state_at`) and gives reviewers a path
  from "goal closed" back to the changes that closed it.

## Further reading

- Design spec: `docs/superpowers/specs/2026-05-16-goal-tracker-design.md`
- Amendment: `docs/superpowers/specs/2026-05-17-goal-tracker-amendment.md`
- Implementation plan: `docs/superpowers/plans/2026-05-16-goal-tracker-archetype.md`
