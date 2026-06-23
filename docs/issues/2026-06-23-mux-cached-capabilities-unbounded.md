---
status: open
opened: 2026-06-23
closed:
severity: low
owner: marius
related: []
tags:
  - mux
  - lsp
  - memory
  - unbounded-growth
kind: bug
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
Not yet implemented. Options, safest first:
1. **Dedup by registration id** — `registerCapability` params carry
   `registrations[].id`; replace an existing entry with the same id instead of appending.
   Correct per LSP semantics (re-registration supersedes) and bounds the duplicate case.
2. **High cap + `warn!`** — keep the most recent N (e.g. 4096) and warn past it. Bounds
   memory unconditionally; effectively never drops a real capability (no LSP registers
   thousands of distinct capabilities). Risk: theoretically drops an old distinct
   capability for a late-joining client.
Deferred out of the OOM-instrumentation change to keep that commit focused; this is a
separate, non-urgent hygiene fix.

## Tests added
N/A — not yet fixed. A fix should add a mux test asserting `cached_capabilities` does not
grow on repeated identical registrations (dedup) or stays ≤ cap.

## Workarounds
None needed — no functional impact observed. A mux restart (idle-timeout or new session)
resets the vec.

## Resume
Decide between dedup-by-id (option 1, preferred) and cap+warn (option 2) for
`src/lsp/mux/process.rs:628-631`; add the mux regression test; verify new-client init
still receives each live capability. Low priority — schedule behind the OOM-forensics
instrumentation and the actual leak hunt.

## References
- `src/lsp/mux/process.rs:36,365,628-631`
- Sibling: `docs/issues/2026-06-19-mcp-server-oom-68gb.md` (the audit that surfaced this; this is explicitly *not* its cause)
- `docs/trackers/bug-fix-session-log.md` (OOM-instrumentation work stream)
