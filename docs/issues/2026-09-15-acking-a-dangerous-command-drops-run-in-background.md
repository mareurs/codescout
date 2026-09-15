---
id: d72f6e0d771073bd
kind: bug
status: open
title: Acking a dangerous command silently drops run_in_background
owners:
- marius
tags:
- run-command
- ack-gate
- cluster/accepted-parameter-silently-dropped
opened: 2026-09-15
severity: low
---

# BUG: acking a dangerous command silently drops run_in_background

## Summary

`run_command(..., run_in_background: true)` on a command that trips the
dangerous-command gate returns an `@ack_` handle. Re-invoking with that handle runs
the command **in the foreground**: `run_in_background` is neither stored on the
pending ack nor replayed, and nothing tells the caller.

`PendingAckCommand` (`src/tools/output_buffer.rs`) holds three fields:

```rust
pub struct PendingAckCommand {
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_secs: u64,
}
```

`command`, `cwd` and `timeout_secs` survive the round trip. `run_in_background` has
nowhere to live, and the re-dispatch at `src/tools/run_command/mod.rs:200` passes a
hardcoded `false` with the comment *"ack re-dispatch is always foreground"*.

## Symptom (Effect)

Observed 2026-09-15 while MCP-verifying the background job-state work. A
backgrounded `sh -c 'kill -9 $$'` returned:

```json
{ "pending_ack": "@ack_a4d4a7a7", "reason": "kill -9 (SIGKILL)",
  "hint": "run_command(\"@ack_a4d4a7a7\") to execute" }
```

The hint names one action and no caveat. A caller who asked for a background job
and acks gets a foreground one — so a long-running dangerous command blocks the
call and can hit the foreground timeout, and no `@bg_` handle is ever minted.

The gate firing is correct and not at issue. The defect is that an accepted
parameter is dropped between the two halves of a two-call protocol, with the second
call's own hint the natural place to say so.

## Reproduction

```
run_command(command="sh -c 'kill -9 $$'", run_in_background=true)   -> @ack_ handle
run_command(command="@ack_<handle>")                                -> foreground
```

The first call is verified — its exact response is quoted above. The second was
**not** run: doing so executes a SIGKILL, and it was not needed to establish the
behaviour, which is readable at the two sites cited and is deliberate there.

## Environment

2026-09-15, live MCP after `cargo rb`, branch `experiments`. Read at `704f1f85`.

## Root cause

The pending-ack store models a dangerous command as three fields rather than as the
original request. Its **sibling variant in the same enum does the opposite** —
`PendingAckWrite` is documented as carrying *"the original tool input, verbatim"*
precisely so replay needs nothing re-sent. So the correct shape already exists next
door; the command variant predates it and was never brought along.

## Evidence

`src/tools/output_buffer.rs` — `PendingAckCommand` (3 fields) vs `PendingAckWrite`
(`input: serde_json::Value`, verbatim). `src/tools/run_command/mod.rs:200` — the
hardcoded `false` and its comment.

## Hypotheses tried

None; found by using the tool, not by a failing call.

## Fix

Not implemented, and it is a real decision rather than an oversight to undo. The
comment asserts foreground re-dispatch is intended, and there may be a reason
(an ack is a human-in-the-loop moment, and a backgrounded dangerous command
detaches immediately past the one gate that paused it). Two candidates:

1. **Honour the flag** — store `run_in_background` on `PendingAckCommand`, or carry
   the whole input as `PendingAckWrite` already does.
2. **Keep foreground and say so** — add the caveat to the ack hint, so the dropped
   parameter is a stated limitation rather than a silent one.

If (2), it belongs in the hint text, not only in the source comment: the caller
reads the hint and never the comment.

## Tests added

None.

## Workarounds

After acking, re-issue as a background call if the gate does not re-trip, or
restructure so the dangerous step is not the backgrounded one.

## Resume

Decide between honouring the flag and documenting its loss. Low severity: it needs
a command that is both dangerous-gated and backgrounded, which is rare.
