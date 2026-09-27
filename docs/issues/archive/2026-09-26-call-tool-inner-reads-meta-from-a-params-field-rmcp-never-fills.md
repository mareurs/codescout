---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-27
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

6f6349ca (patch-id `6bff20b066abbae151bbfd9762d1a4dad6b2b65b`) folds `req_ctx.meta` into `req.meta` inside `call_tool`. The params-level meta is the base and **context keys win on conflict**, the direction rmcp's own `WithMeta` serializer merges in ("params-level _meta as base, extensions-level _meta overwrites on conflict"). An empty merge stays `None`, so an absent `_meta` stays absent. The precedence decides nothing reachable: every production route (stdio and streamable HTTP) deserializes through `WithMeta`, so the params meta `call_tool` receives is always `None`, and peer-serve's `call_tool_by_name` bypasses `call_tool`. It is therefore deliberately untested. `call_tool_inner`'s signature is unchanged.

Same misconception, outside this fix: `src/tools/progress.rs` documents the progress token as coming from `CallToolRequestParams._meta.progressToken`, but it is read from `RequestContext.meta`.

## Tests added

- `call_tool_records_the_wire_meta_tool_use_id` (`src/server.rs`) serves `CodeScoutServer` over an in-memory duplex, sends NDJSON `initialize` → `initialized` → `tools/call` whose `_meta` carries the literal `claudecode/toolUseId`, and asserts the usage row. Deleting the fold was run in an isolated worktree (`scripts/mutation-probe.sh`): it compiles, and the test fails on the assertion (`None` vs `Some("toolu_wire1")`). That mutant is the pre-fix production state. The test lives in `guide_hint_tests`, so it runs only with the `librarian` feature, which the default lane and the shipped binary both enable.
- `call_tool_inner_records_the_meta_tool_use_id`'s doc comment now says what it pins (the params-level read), not "the production funnel".

## Workarounds

None. The Part B pipeline falls back to the heuristic join (spec § join) for
every row recorded before the fix ships.

## Resume

Fixed and verified live, 2026-09-27.

- Gate on HEAD ee412408 (includes 6f6349ca): FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0. The wire test ran in the default lane, and `cross_process_write_lock` (the only end-to-end run of the no-`_meta` path) ran in both lanes.
- Live exact join, from the first session to reconnect to the rebuilt binary (`0cbae2f0`, window 2026-09-27 04:58:01–05:55:38 UTC): 85 of 85 rows carry a non-NULL `tool_use_id`, 85 are distinct, and each occurs EXACTLY once as a `tool_use` id across that session's 7 transcript files (1 top-level, 6 subagent). `deliveries_json` is non-NULL on 83 of them; the other 2 are `recoverable_error`, the pinned "fan-out never ran" case.
- That also settles Hypothesis 1: Claude Code 2.1.283 does send `claudecode/toolUseId`, because the recorded values match the transcript's own ids.

Only calls served by a binary at or after 6f6349ca carry the id. Each session gains it at its own `/mcp`, so every earlier row joins heuristically (spec Amendment 4).

## References

- `src/server.rs` — `call_tool`, `call_tool_inner`
- `src/tools/session_key.rs` — `tool_use_id_from_meta`, `conversation_from_meta`
- `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md` — Task 3 Step 2
