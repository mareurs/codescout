---
kind: bug
status: taken
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-26
owner: marius
related: []
severity: high
---

# BUG: `call_tool_inner` reads `_meta` from a params field rmcp never fills on the wire

## Summary

Both `_meta` reads in `call_tool_inner` (`src/server.rs`) take the value from
`CallToolRequestParams.meta`. rmcp 1.3.0's inbound deserializer never populates
that field: it moves the request's `_meta` into `Request.extensions`, which the
serve loop then hands to the handler as `RequestContext.meta`. So on every live
call `tool_use_id_from_meta` and `conversation_from_meta` receive `None`, and
usage.db records `tool_use_id` NULL, with no error anywhere.

## Symptom (Effect)

The system1 base-rate measurement's Part A live exact-join check (plan Task 3
Step 2) failed. The rebuilt server (started 2026-09-26 11:22:16 UTC, Claude
Code 2.1.283) wrote `tool_use_id` NULL on every row of session `3c5b02df`.
`deliveries_json`, shipped in the same Part A, is populated on the same rows,
so the rows came from the new binary: the column exists, it is written, and
the value is absent.

The same defect silently disables the pre-existing conversation tier
(`conversation_from_meta`). Its doc comment says it "ships ahead of its sender"
and returns `None` today because no client sends the key. That is true, but
even once a client does send it, the value would still never arrive.

## Reproduction

A scratch crate depending on `rmcp = "=1.3.0"` deserializes this wire message
as a `ClientJsonRpcMessage`:

    {"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"grep",
     "arguments":{"pattern":"x"},"_meta":{"claudecode/toolUseId":"toolu_PROBE","progressToken":5}}}

Output:

    params.meta          = None
    extensions Meta      = Some(Meta({"claudecode/toolUseId": String("toolu_PROBE"), "progressToken": Number(5)}))
    arguments (control)  = Some({"pattern": String("x")})

The control matters: the arguments survive, and the whole `_meta`, progress
token included, is present, just not where `call_tool_inner` looks.

## Environment

codescout `experiments` at 84dba2bd, rmcp 1.3.0, Claude Code 2.1.283, Linux.

## Root cause

In rmcp 1.3.0 (its own model/serde_impl.rs), `Request<M, P>` deserializes
through a `WithMeta<P>` proxy that has a named `_meta` field beside
`#[serde(flatten)] _rest: P`. serde hands a flattened field only the keys no
named field claimed, so `_meta` never reaches `P = CallToolRequestParams`, whose
own `meta` field keeps its `default` of `None`. The proxy then inserts the Meta
into `Request.extensions`, and the serve loop (its service.rs, the
`std::mem::swap(&mut meta, request.get_meta_mut())` beside "swap meta firstly,
otherwise progress token will be lost") moves it into `RequestContext.meta`.

`call_tool` already reads the progress token from `req_ctx.meta`, and its
comment says this mirrors rmcp's own `get_meta().get_progress_token()`. The two
`_meta` reads added later went to `req.meta` instead, because `call_tool_inner`
takes the params and not the context.

**Why no test saw it.** Every server test builds `CallToolRequestParams` by
hand and calls `call_tool_inner` directly, so rmcp's deserializer is never on
the tested path. `call_tool_inner_records_the_meta_tool_use_id` sets
`req.meta` itself, which is exactly the one thing the wire never does, and so
it passes against a production path that records NULL.

## Evidence

- usage.db rows for session `3c5b02df` (parent and its subagents share it),
  window 11:22:16–11:40 UTC, read 2026-09-26: `tool_use_id` NULL on **49 of
  49**. `deliveries_json` NULL on 5, all `recoverable_error`, and on 0
  successes, which is the pinned "fan-out never ran" case.
- The wire probe above.
- Raw capture kept in the SDD workspace (gitignored) as
  probes/part-a-live-meta.txt.

## Hypotheses tried

1. *Claude Code 2.1.283 stopped sending `claudecode/toolUseId`* (the key was
   observed on the wire on 2026-09-13 against 2.1.270). Not excluded by the
   probe, which uses a synthetic message. It does not matter for the fix,
   though: with the current code the value is dropped whether or not it is
   sent. The post-fix live check discriminates the two, because non-NULL rows
   prove both.

## Fix

In `call_tool`, fold `req_ctx.meta` into `req.meta` before `call_tool_inner`,
with a params-level key keeping precedence, the direction rmcp's own serializer
merges in. The test drives `call_tool` through rmcp's real deserializer over
an in-memory transport, so it covers the call site and not a hand-built struct.

## Tests added

(pending)

## Workarounds

None. The Part B pipeline falls back to the heuristic join (spec § join) for
every row recorded before the fix ships.

## Resume

Fix dispatched from the system1 base-rate measurement SDD run (session
`3c5b02df-b6ce-45f5-9d03-1194e38465c0`). After it lands: `./scripts/rb.sh`,
the operator's `/mcp`, then re-run the plan Task 3 Step 2 live check.

## References

- `src/server.rs` — `call_tool`, `call_tool_inner`
- `src/tools/session_key.rs` — `tool_use_id_from_meta`, `conversation_from_meta`
- `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md` — Task 3 Step 2
