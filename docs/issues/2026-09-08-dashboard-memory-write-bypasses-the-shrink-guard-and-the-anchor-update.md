---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
closed: null
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

Not started. The obvious shape is to move both protections behind a single guarded
entry point that both callers use, rather than adding two calls to the dashboard handler
— a second copy of the wrapper is the same defect waiting for a third caller.

Note the ordering constraint: the shrink guard needs the OLD content, so it must run
before the write; `update_anchors_on_write` needs the NEW content, so it must run after.
A naive "call both from the handler" fix that gets this backwards will silently guard
nothing.

## Tests added

None yet. A regression test should assert the dashboard route refuses an
under-half write and that the sidecar's hash changes after a successful one — asserting
only that the handler *calls* something is the shape this repo has been burned by
(an assertion over a re-implementation rather than the shipped path).

## Workarounds

Use `memory(action="write")` rather than the dashboard editor. If a memory has already
been edited in the UI, run `memory(action="refresh_anchors", topic=...)` afterwards —
but only after reviewing that the content is actually correct, since `refresh_anchors`
clears the staleness flag without reading anything.

## Resume

Read `src/memory/mod.rs:112-143` and decide whether the guard belongs inside
`MemoryStore::write` (which its own docstring argues against, citing
`src/tools/onboarding.rs`) or in a new guarded wrapper both callers take. Then check
whether `delete_memory` at `src/dashboard/api/memories.rs` has a matching gap.

## References

- `src/dashboard/api/memories.rs`, `src/dashboard/routes.rs:48`
- `src/tools/memory/mod.rs:769`, `:781`, `:808`
- `src/memory/mod.rs:112-143`, `src/util/shrink_guard.rs:100-129`
- `src/memory/anchors.rs:291` (`update_anchors_on_write`)
- Found while re-deriving all 16 stale memories, `09ecb58d`
