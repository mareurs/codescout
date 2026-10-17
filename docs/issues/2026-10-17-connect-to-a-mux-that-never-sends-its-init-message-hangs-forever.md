---
kind: bug
status: open
title: 'BUG: connecting to a mux that has bound its socket but never sends its init message hangs forever, so the wait window and the wedge memory are never reached'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
- docs/issues/2026-10-17-mux-connect-path-lock-races.md
severity: low
---

# BUG: connecting to a mux that has bound its socket but never sends its init message hangs forever, so the wait window and the wedge memory are never reached

## Summary

`LspClient::connect` (`src/lsp/client.rs`) reads the mux's init message with no timeout. A mux that has bound its socket but never writes the init message (a mux wedged after `bind`, or one whose accept task is stuck) therefore blocks `connect` for good. `connect_to_mux` (`src/lsp/manager.rs`) only checks its deadline between attempts, so its window never expires, and the wedge memory (`LspManager::mux_wedged_at`) is never written: every later call hangs the same way. Found by the third review of the Kotlin symbols branch, 2026-10-17.

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: a bound on a wait that is enforced between steps but not inside the step that blocks, so the bound does not hold. The case differs from a wedged flock in that the socket answers the connect.

## Symptom (Effect)

A tool call that needs the mux never returns until the tool timeout (60 s) cuts it. Pre-existing before the peer-wait change, which kept the unbounded read.

## Reproduction

The ignored test `connect_to_mux_gives_up_on_a_socket_that_never_sends_its_init_message` in `src/lsp/manager.rs`:

```
scripts/with-slot.sh cargo test --lib -- --ignored lsp::manager::tests::connect_to_mux_gives_up
```

It binds a Unix listener that accepts and says nothing, then runs `connect_to_mux` with a 1 s window inside a 5 s outer timeout.

## Environment

Branch worktree-agent-af85b780928eaa450 at 92e76d3b plus this test; Linux.

## Root cause

measured 2026-10-17: `a 1 s window must end by itself; still waiting after 5.001611297s` (test above, 5.00 s run). Mechanism read from `LspClient::connect` (`transport::read_message(&mut buf_reader).await` for the init message with no `tokio::time::timeout`) and `connect_to_mux` (deadline tested only after `LspClient::connect` returns).

## Evidence

The measurement above. The same wait shape (a read with no bound) is not present in the direct-process path, which has `init_timeout`.

## Hypotheses tried

None beyond the reproduction.

## Fix

Unknown, not implemented. Option: wrap the connect attempt in `tokio::time::timeout(remaining window)` inside `connect_to_mux` (and treat the timeout as an unreachable socket so the wedge memory is written), or give `LspClient::connect` its own init-read timeout. Choosing between them is a decision about who owns the bound.

## Tests added

The ignored reproduction above (red under `--ignored`, not part of the default run).

## Workarounds

Retry after the tool timeout; kill the wedged mux (`fuser` the lock file).

## Resume

1. Pick the owner of the bound (connect_to_mux window or LspClient::connect). 2. Remove `#[ignore]` from `connect_to_mux_gives_up_on_a_socket_that_never_sends_its_init_message` and make it green.

## References

- `src/lsp/client.rs` (`LspClient::connect`)
- `src/lsp/manager.rs` (`connect_to_mux`)
