---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- mux
- lsp
- memory
- unbounded-growth
topic: null
time_scope: null
closed: '2026-07-05'
opened: '2026-06-23'
owner: marius
related: []
severity: low
---

# BUG: mux `MuxState.cached_capabilities` grows unbounded (pushed per `client/registerCapability`, never cleared or capped)

## Summary
The LSP mux caches every `client/registerCapability` request the language server
sends, in `MuxState.cached_capabilities: Vec<Value>`, so it can replay them to
clients that connect later. The vec is **never cleared, deduped, or capped** — it
only grows for the lifetime of the mux process. Surfaced while auditing
68-GB-capable allocations for `docs/issues/2026-06-19-mcp-server-oom-68gb.md`.

## Symptom (Effect)
No observed failure. This is a latent unbounded-growth smell found by code audit,
not a reproduced OOM. **It is explicitly NOT the cause of the 2026-06-19 68 GB OOM:**
that victim was a `codescout start` **server** process, whereas `cached_capabilities`
lives in `MuxState`, which is owned by the **separate `codescout mux` process**. And
each registration message is a few KB, so reaching tens of GB would need millions of
registrations — not a realistic path. Logged for hygiene, not as an OOM lead.

## Reproduction
Not reproduced (no functional symptom). To exercise the growth: drive a mux against a
language server that re-emits `client/registerCapability` many times (e.g. repeated
workspace reconfiguration) and watch `cached_capabilities.len()` climb without bound.

## Environment
codescout `experiments`; the mux is spawned as `codescout mux --socket … --cwd …`
(a distinct process from the MCP `start` server). Any LSP that uses dynamic capability
registration (rust-analyzer, kotlin-lsp).

## Root cause
`src/lsp/mux/process.rs:36` declares `cached_capabilities: Vec<Value>`. The
`client/registerCapability` arm (`:628-631`) does `st.cached_capabilities.push(msg.clone())`
on every such request with no dedup/cap. The vec is read at `:365` to seed new clients'
init message (`"registered_capabilities": st.cached_capabilities`). Nothing ever removes
entries, so re-registration of the same capability appends a duplicate rather than
replacing it, and distinct registrations accumulate forever.

## Evidence
```rust
// src/lsp/mux/process.rs:36
cached_capabilities: Vec<Value>,
// :628-631 — only mutation site, append-only
"client/registerCapability" => {
    let mut st = state.lock().await;
    st.cached_capabilities.push(msg.clone());   // never deduped / capped / cleared
}
// :365 — replayed to each new client
"registered_capabilities": st.cached_capabilities,
```

## Hypotheses tried
1. **Hypothesis:** this is the 68 GB server OOM. **Test:** check which process owns
   `MuxState` and the per-message size. **Verdict:** rejected — it's in the mux process
   (not the `start` server victim) and each message is KB-scale; 68 GB needs millions of
   registrations. **Evidence:** mux process model (`codescout mux` is a separate process)
   + the append site above.

## Fix

Implemented **option 1 (dedup by registration id)** in `src/lsp/mux/process.rs`.
Two pure free functions added before `handle_server_request`:
- `registration_ids(msg)` — extracts `params.registrations[].id`.
- `cache_registration(cache, msg)` — before pushing, drops any cached message whose
  registration ids are **all** superseded by the incoming one (keeps entries still
  carrying a live id), then pushes. Repeated identical re-registration now replaces
  instead of accumulating, bounding the growth; distinct live capabilities are preserved.

The `client/registerCapability` arm now calls
`cache_registration(&mut st.cached_capabilities, msg)` instead of the append-only
`st.cached_capabilities.push(msg.clone())`. New-client init replay therefore carries
each live capability once (fewer duplicate registrations to late-joining clients).
## Tests added

Three pure unit tests in the new `src/lsp/mux/process.rs` `mod tests` (no async /
`SharedWriter` mock needed — the dedup logic is a free function):
- `cache_registration_dedups_repeated_identical_registrations` — 100× identical registration → len 1.
- `cache_registration_keeps_distinct_and_replaces_superseded` — distinct ids retained; re-registering an id replaces (len stays 2) with the newest registerOptions.
- `cache_registration_supersedes_prior_entry_when_batch_covers_its_ids` — a batch covering a prior entry's ids supersedes it.

All green; clippy `-D warnings` clean; full lib suite green (2882 passed).
## Workarounds
None needed — no functional impact observed. A mux restart (idle-timeout or new session)
resets the vec.

## Resume

Fixed via dedup-by-registration-id (option 1). Verified: 3 new tests pass, full
`--features server-stack` lib suite green. Latent hygiene bug closed.
## References
- `src/lsp/mux/process.rs:36,365,628-631`
- Sibling: `docs/issues/2026-06-19-mcp-server-oom-68gb.md` (the audit that surfaced this; this is explicitly *not* its cause)
- `docs/trackers/bug-fix-session-log.md` (OOM-instrumentation work stream)
