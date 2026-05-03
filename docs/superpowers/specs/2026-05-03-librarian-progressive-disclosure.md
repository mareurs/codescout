# Librarian Progressive Disclosure & Tracker Terminology

**Date:** 2026-05-03
**Status:** approved

## Goal

Close two gaps in the librarian tool surface:

1. **Cold-start orientation** — an LLM calling `artifact` for the first time has no idea how many
   artifacts exist, what kinds are in play, or that trackers are special. There is no load info.
2. **Tracker/artifact terminology** — "create a tracker for X" is natural user language, but the
   mapping to `kind=tracker` + `librarian(tracker_design)` prerequisite lives only in
   `server_instructions.md`, not in tool descriptions or response hints.

## Changes

### 1. `artifact` tool description

Append one sentence to the existing description:

> Trackers are artifacts with `kind=tracker` — augmented documents that auto-refresh their body
> via a persistent prompt; call `librarian(tracker_design)` before creating one.

**File:** `crates/librarian-mcp/src/tools/artifact.rs`
`impl Tool for Artifact / description`

---

### 2. `artifact(find)` — catalog summary on cold call

**Trigger:** `find` called with none of: `filter`, `semantic`, `kind`, `status` set (i.e. no
narrowing params — the natural "what's here?" orientation call). `scope` does not count as a
narrowing param and does not suppress the catalog summary.

**Inject** into the response alongside the normal `items` array:

```json
"catalog": {
  "total": 12,
  "by_kind": { "tracker": 3, "plan": 4, "spec": 5 },
  "augmented": 3
}
```

- `total` — count of non-archived artifacts in the active scope
- `by_kind` — map of kind → count, non-archived only, scoped same as `total`
- `augmented` — count of artifacts with an augmentation row, same scope

**File:** `crates/librarian-mcp/src/tools/find.rs`  
Inject after building hints, gated on the "cold call" condition.

**Why not always:** Focused finds (filtered, semantic) don't need catalog noise. Cold finds are
the orientation moment — that's when the summary pays off.

---

### 3. `artifact(create, kind=tracker)` without `augment` — warning hint

When `create` is called with `kind=tracker` and no `augment` field:

- **Do not fail.** Creation proceeds normally.
- **Append to response:**

```json
"tracker_hint": "Tracker created without augmentation. Call librarian(tracker_design) to pick an archetype and attach a refresh prompt via artifact_augment."
```

**File:** `crates/librarian-mcp/src/tools/create.rs`

---

### 4. `server_instructions.md` — augmentation + tracker 2-liner

In the "When to use artifact tools" block, after the entry-point line (currently line ~160),
add:

> Artifacts can carry **augmentation** — a persistent prompt that auto-refreshes their body as
> the codebase evolves. **Trackers** (`kind=tracker`) are the canonical augmented artifact:
> living documents for issue lists, ADR logs, experiment records, and similar multi-entry state.

**File:** `src/prompts/server_instructions.md`

---

## What does NOT change

- `tracker_design` workflow — unchanged; the hint in `create` points to it
- `librarian(context)` — unchanged; still the "orient by topic" call
- `artifact_augment` — unchanged
- `ONBOARDING_VERSION` — no bump needed; only `server_instructions.md` changes (live on next connect)

## Testing

- Unit test in `find.rs`: cold call (no params) returns `catalog` field; focused call (with filter) does not
- Unit test in `create.rs`: `kind=tracker` without `augment` returns `tracker_hint`; with `augment` does not
- Existing tests must continue to pass unchanged
