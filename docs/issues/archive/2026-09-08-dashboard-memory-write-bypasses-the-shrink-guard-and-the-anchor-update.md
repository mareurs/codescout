---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-11
opened: 2026-09-08
owner: marius
related: []
severity: high
---

# BUG: the dashboard's memory editor bypasses both the shrink guard and the anchor update, so a UI edit silently leaves a topic stale and unguarded

## Summary

`POST /api/memories/{topic}` writes memory content through `MemoryStore::write` directly.
The MCP `memory(action="write")` path wraps that same call with two protections — a
shrink guard and an automatic anchor re-stamp. The dashboard route has neither. A memory
edited in the UI can be truncated to a fraction of its size with no refusal, and stays
reported `stale` afterwards even though its content was just brought current.

## Symptom (Effect)

Two distinct effects from one omission, and both are silent — no error, no warning, a
`200 OK` either way:

1. **No shrink guard.** Replacing a 39,000-byte memory with 200 bytes through the UI
   succeeds. The same content through `memory(action="write")` is refused with
   `shrink{old_bytes,new_bytes,byte_pct,...}`.
2. **No anchor update.** After a UI write, `workspace(action="status")` still lists the
   topic under `memory_staleness.stale`, because the `.anchors.toml` sidecar was never
   re-hashed. A user who has just corrected a memory is told it is out of date.

The second is the more misleading of the two: it inverts the meaning of the staleness
report for exactly the person who acted on it.

## Reproduction

At `09ecb58d` (`git rev-parse HEAD`), with the dashboard running:

```
curl -X POST localhost:<port>/api/memories/gotchas \
     -H 'Content-Type: application/json' \
     -d '{"content":"# gotchas\n\nreplaced\n"}'
```

Expect `200 OK` and a 39 KB memory reduced to ~20 bytes. Then
`workspace(action="status")` — `gotchas` is still listed stale, and its
`.codescout/memories/gotchas.anchors.toml` is untouched.

The MCP path refuses the identical write.

## Environment

Linux, Rust edition 2021, codescout 0.15.0, branch `experiments`, axum dashboard.
Not transport-specific: the MCP tool path and the HTTP path are different call sites in
the same binary.

## Root cause

Two protections live at the **tool** layer, not in `MemoryStore::write`, and the
dashboard calls the store directly:

- `src/dashboard/api/memories.rs:58` — `write_memory` calls
  `store.write(&topic, &body.content)` and nothing else.
- Route is live: `src/dashboard/routes.rs:48`,
  `.route("/api/memories/{topic}", post(api::memories::write_memory))`.
- The MCP path guards the same call twice: shrink check at
  `src/tools/memory/mod.rs:769` (private store) and `:781` (project store), then
  `update_anchors_on_write` at `:808`.
- `MemoryStore::write` (`src/memory/mod.rs:112-120`) is deliberately unguarded — its
  docstring at `:124-136` records that wholesale replacement is specified behaviour that
  `src/tools/onboarding.rs` depends on. So the omission is not in the store; it is that
  a second caller was added without the wrapper.
- `MemoryStore::shrink_check` is a separate method (`src/memory/mod.rs:137-143`), which
  is what makes it skippable by construction.

Measured 2026-09-08: read `write_memory` in full and confirmed the route registration at
`routes.rs:48` — the handler is reachable, not dead code. The guard call sites were read
at `src/tools/memory/mod.rs`. The runtime effect is **inferred from these three sites,
not observed** — the `curl` above has not been run.

## Evidence

`src/dashboard/api/memories.rs`, the whole handler:

```rust
pub async fn write_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
    Json(body): Json<WriteMemoryBody>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (write)", e),
    };
    match store.write(&topic, &body.content) {
        Ok(()) => (StatusCode::OK, Json(json!("ok"))),
        Err(e) => internal_error("memory write", e),
    }
}
```

Compare `delete_memory` immediately below it, which has the same shape — worth checking
whether deletion has an equivalent gap, though the blast radius differs.

## Hypotheses tried

1. **Hypothesis:** the handler is registered nowhere and therefore unreachable, which
   would make this documentation debt rather than a defect.
   **Test:** `references(symbol="write_memory")` returned 0 with a warning that the
   symbol appears in `src/dashboard/routes.rs`; corroborated with `grep`.
   **Verdict:** rejected — `routes.rs:48` registers it on `post`.
   **Note:** the LSP reference index returning 0 here is itself worth remembering; the
   tool's own warning is what prevented reading that zero as "unused".

## Fix

Both protections now live in `crate::memory::guarded`, and the **MCP path was rewired through it
too** — the shape § Fix asked for, rather than two calls added to the dashboard handler. A second
copy of the wrapper is the same defect waiting for a third caller, and the two callers now compose
nothing themselves.

**The ordering constraint § Fix warned about is why the composition is not optional.** The shrink
guard needs the OLD content so it must run before the write; the anchor update needs the NEW
content so it must run after. Those point in opposite directions, which is exactly what makes a
naive fix writable backwards and still compilable — guarding nothing while looking guarded. A
caller can no longer order them.

**Anchors are seeded beside the file actually written** (`store.dir()`) rather than from a
re-resolved memories directory. The MCP path re-resolved it after the write; re-resolving is how
the store's directory and the sidecar's could disagree.

**`force` was added to the dashboard's write payload, and it is part of the fix rather than a
convenience.** A guard with no escape converts *"the UI can silently destroy a memory"* into *"the
UI cannot legitimately replace one"* — a different defect, not a repair. The MCP path has carried
`force` since the guard was written. The refusal is **409, not 400**: the request is well-formed
and the refusal is about state on disk, which is what `force` overrides; a 400 would tell the UI to
fix a payload that is fine. It renders through `ShrinkReport::describe()` rather than a local
`format!`, because `shrink_guard`'s own header records that a forked message decays exactly the way
a forked predicate does.

**§ Resume asked whether `delete_memory` had a matching gap. It did**, and it is fixed here.
`store.delete` left the `.anchors.toml` sidecar orphaned, where the MCP delete removes it so a
deleted topic stops surfacing in staleness scans — a topic that no longer exists, reported stale
forever, with nothing to bring current. Same mechanism, second operation, which is why it belonged
in this change rather than a follow-up.

**§ Resume's other question — whether the guard belongs inside `MemoryStore::write` — is answered
no, and the store's own docstring was right.** Wholesale replacement is `write`'s specified
behaviour and `tools/onboarding.rs` depends on it. What failed was not that policy sat with the
caller; it was that there was no *shared* caller-side layer, so the policy lived inline in one
caller and a second was added beside it with none. `memory::guarded` is that layer, and the store's
primitives are untouched.

Fix SHA: 0cac0eaf
Patch-id: 67603e2be31ece266dca5885a1f406d39a93ee21
## Tests added

Six, in `src/dashboard/api/memories.rs`, and they drive the **handlers** rather than
`memory::guarded`. That is the whole point: the helper being correct was never the defect — the
route not calling it was, and a test of the helper passes identically whether the route reaches it
or not.

| test | guards |
|---|---|
| `a_shrinking_ui_edit_is_refused_and_the_old_content_survives` | the filed defect; asserts the old content is still on disk, not merely that a 409 came back |
| `force_true_applies_the_same_shrinking_edit` | the escape exists, so the guard did not remove a capability |
| `a_first_write_is_never_refused` | topic creation still works — a first write destroys nothing |
| `a_ui_write_stamps_the_anchor_sidecar` | the second half of the filed defect |
| `a_ui_delete_removes_the_anchor_sidecar` | the gap § Resume asked about |
| `a_ui_delete_tolerates_a_missing_sidecar` | most topics never grow one, so absence is not an error |

**Mutation, on the production path.** Reverting `write_memory` to `store.write` and
`delete_memory` to `store.delete` reds exactly **3 of 6** — shrink-refusal, anchor-stamp,
delete-sidecar — and leaves the other 3 green. A suite that redded on all six would not have been
discriminating between the defect and its surroundings.

Two fixture details are load-bearing and annotated at their lines: the shrink fixture is long
enough to clear `SHRINK_GUARD_MIN_BYTES` (under the floor the guard declines to object, so a short
fixture passes against the unfixed handler), and the anchor fixture creates the file it cites
(`seed_anchors` skips a path it cannot stat, so without it the sidecar is legitimately absent and
the assertion would be asserting the bug).

**These tests needed a lane before they meant anything.** `dashboard` is in no default or test
lane, and CI ran `cargo check --features dashboard` — which compiles a test target and never runs
it. The lane is now `cargo test --features dashboard --all-targets`. That gap is a class in its own
right: `every_declared_feature_has_a_lane_or_a_reason` cannot tell `cargo check --features X` from
`cargo test --features X`, filed as
`docs/issues/2026-09-11-the-feature-lane-gate-counts-a-compile-only-lane-as-coverage.md`.
## Workarounds

Use `memory(action="write")` rather than the dashboard editor. If a memory has already
been edited in the UI, run `memory(action="refresh_anchors", topic=...)` afterwards —
but only after reviewing that the content is actually correct, since `refresh_anchors`
clears the staleness flag without reading anything.

## Resume

Fixed and archived. Both § Resume questions were answered rather than deferred: the guard does
**not** belong in `MemoryStore::write` (the docstring's argument holds; what was missing was a
shared caller-side layer), and `delete_memory` **did** have a matching gap, now closed in the same
change.

One thread continues elsewhere:
`docs/issues/2026-09-11-the-feature-lane-gate-counts-a-compile-only-lane-as-coverage.md` — the
reason these tests would have been green by never running, and the gate that let that state exist.

Worth knowing for anything else in this area: a tool-scoped shrink-guard gate exists
(`every_content_bearing_tool_is_shrink_guarded_or_has_a_reason`, `src/server.rs`) and it would
**not** have caught this. Its population is registered MCP tools, and an HTTP route is outside that
by construction. Its own doc comment also states that it proves a named module CONTAINS a
`shrink_guard::check` call, never that the call runs on the right operands or is reachable — so a
centralised wrapper keeps it green whether or not any caller invokes it. Per-surface behavioural
tests remain owed by each surface.
## References

- `src/dashboard/api/memories.rs`, `src/dashboard/routes.rs:48`
- `src/tools/memory/mod.rs:769`, `:781`, `:808`
- `src/memory/mod.rs:112-143`, `src/util/shrink_guard.rs:100-129`
- `src/memory/anchors.rs:291` (`update_anchors_on_write`)
- Found while re-deriving all 16 stale memories, `09ecb58d`
