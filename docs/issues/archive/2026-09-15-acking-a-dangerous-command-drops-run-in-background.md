---
id: e1aaab73c7d3ba5f
kind: bug
status: fixed
title: Acking a dangerous command silently drops run_in_background
owners:
- marius
tags:
- run-command
- ack-gate
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-16
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

**Fixed in `1f36fe26`** — patch-id `6e78e0e32346a3aa815454852772111c9e484924`. Direction chosen by the operator 2026-09-16: **honour the flag**, candidate 1 below.

What shipped: `PendingAckCommand` gains `run_in_background: bool`, `store_dangerous` takes it as a fourth argument, and the ack dispatch in `run_command/mod.rs` passes `stored.run_in_background` where it previously hardcoded `false`.

**Which half of candidate 1, and why the other half was rejected.** The option offered two mechanisms — add the field, or carry the whole input as `PendingAckWrite` does. The `Value` form is the more general shape and would survive future parameters, but it is not reachable here: `store_dangerous` is called from inside `run_command_inner`, which never receives the original input `Value`. Carrying it means threading an eleventh parameter through a ten-parameter function and re-parsing it at dispatch to recover one bool already in scope at the call site. **If a second parameter is ever dropped the same way, that is the signal to switch to the `Value`** — two concretes, where today there is one.

**A false doc claim corrected with it.** `store_dangerous`'s own comment read *"The handle carries the full execution context so the ack call needs no extra parameters."* That was false for exactly one parameter, and the comment is the surface a maintainer would have trusted. It now says when the claim was untrue rather than merely being made true.

## Tests added

Two, in `src/tools/run_command/tests.rs`, and they are a **pair on purpose** — either alone is monotone in a direction that hides a real regression.

- `ack_re_dispatch_honours_run_in_background` — stores an ack with `true`, dispatches the handle, asserts the response carries **no `exit_code`** and an `output_id` starting `@bg_`.
- `ack_re_dispatch_stays_foreground_when_not_requested` — stores with `false`, asserts `exit_code == 0` and the command's stdout. **Without this one, honouring the flag unconditionally — backgrounding every acked command — passes the first test and breaks every other ack caller.**

**Why the discriminator is the absence of `exit_code` rather than stdout.** A foreground `sleep 30` also returns stdout — empty — so a stdout assertion is satisfied by the broken behaviour and would not red under the mutation these tests exist to catch. Slice 1 (`f098069a`) made *"a spawn response carries no `exit_code` key"* true by construction, which is what makes that assertion available at all; before it, there was no observable difference to assert on.

The `true` argument in the first test's `store_dangerous` call is load-bearing and annotated as such on its line: with `false` the test asserts nothing, because a foreground response is then the correct one.

Gate green all four lanes at fix time: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Workarounds

After acking, re-issue as a background call if the gate does not re-trip, or
restructure so the dangerous step is not the backgrounded one.

## Resume

Decide between honouring the flag and documenting its loss. Low severity: it needs
a command that is both dangerous-gated and backgrounded, which is rare.
