---
status: open
opened: 2026-06-24
closed:
severity: high
owner: marius
related:
  - "agents/system repo: VPN_DOCKER_RECURRING_ISSUE.md (the env trigger that surfaced this)"
  - docs/issues/2026-06-19-mcp-server-oom-68gb.md
tags: [qdrant, startup, mcp, robustness, timeout, server-stack, hang, server-stack]
kind: bug
---

# BUG: a reachable-but-hung Qdrant wedges codescout MCP startup (120 s op timeout ≫ host init budget) → session sees codescout "fail / can't restart"

## Summary
When the Qdrant daemon is **reachable-but-unresponsive** (TCP accepts, no reply — e.g. Docker
port-forwarding broken by a VPN, or Qdrant internally wedged), codescout's first Qdrant call on the
startup path **blocks up to 120 s** (the `QdrantWrap` op timeout). That is ~4× the MCP host's
(~30 s) initialize/first-request budget, so Claude Code kills the codescout server before the call
returns. The user-visible symptom is **"codescout failed in a session and can't restart"** — new
instances start, go idle, die at ~30 s; already-running instances (which connected while Qdrant was
healthy) keep working. The failure is opaque: nothing in codescout's own logs reports an error.

## Symptom (Effect)
Per-instance diagnostic logs (`<project>/.codescout/diagnostic-<id>.log`) of a failing start:
```
… Starting codescout MCP server (transport=stdio)
… codescout MCP server ready (stdio)
… current project resolved: abs_path=… git_root=…
… heartbeat instance=<id> uptime_secs=30 …        # exactly ONE heartbeat
(no further lines; process gone shortly after)
```
The shared `debug.log` shows the instance connecting to Qdrant gRPC `127.0.0.1:6334`
immediately after "current project resolved", sending a request, then **no response** before death.
No `ERROR`/panic/OOM is logged.

## Reproduction
Observed in `~/work/stefanini/southpole/MRV-poc` on 2026-06-24 (codescout sha `46f48231`). The
environmental trigger was a Cisco VPN reconnect after suspend that broke host→container Docker
forwarding (tracked + fixed in the `agents/system` repo, `VPN_DOCKER_RECURRING_ISSUE.md`), so the
qdrant container was reachable at the TCP layer (docker-proxy accepted) but no gRPC response came back.

Deterministic standalone repro (no VPN needed): point codescout at a **black-hole** Qdrant — a
listener that accepts connections and never responds — and start the server:
```
# terminal 1: accept-and-hang on the qdrant gRPC port
socat TCP-LISTEN:6334,fork,reuseaddr SYSTEM:'sleep 600'    # or `nc -k -l 6334`
# terminal 2:
CODESCOUT_VECTOR_BACKEND=qdrant <start codescout against a project>
```
Expected (bug): startup blocks on the first Qdrant call for up to 120 s; under an MCP host with a
~30 s init timeout, the server is killed and "can't restart".

## Environment
- codescout ≥ 0.15.0, `server-stack` build (Qdrant backend), MCP stdio transport (Claude Code).
- Any state where Qdrant is reachable at TCP but unresponsive at the application layer.

## Root cause
`QdrantWrap::connect` (`src/retrieval/qdrant.rs:24`) builds the client with a **120 s** op timeout:
```rust
Qdrant::from_url(url).timeout(std::time::Duration::from_secs(120)).build()
```
`build()` is lazy (no network); the **first actual Qdrant operation** does the round-trip, bounded by
that 120 s. On the startup path, `build_tool_context` (`src/librarian/mod.rs:~118`) constructs the
artifact vector store during tool-context setup; its Qdrant branch runs `QdrantWrap::connect` +
`QdrantArtifactStore::new` (collection bootstrap — a network call).

That site already degrades on **connect error** (`match connected { Ok => …, Err => … }`), but it
does **not** defend against a **hang**: a reachable-but-unresponsive Qdrant produces no error — the
gRPC call simply blocks up to 120 s. 120 s ≫ the MCP host's ~30 s init/first-request budget, so the
host reaps codescout before the call returns. Net effect: a degraded dependency takes down the whole
server instead of being skipped.

Same 120 s exposure exists at the other production connect sites (lazy, so lower startup risk):
- `src/agent/mod.rs:1587` — semantic memory store (`get_or_try_init`, lazy; first semantic call).
- `src/retrieval/client.rs:59` — code vector store.

## Evidence
- `src/retrieval/qdrant.rs:24-30` — the 120 s timeout (verified by read).
- `src/librarian/mod.rs:112-128` — artifact-store init on the tool-context path; `Err`-only degrade.
- MRV-poc `.codescout/diagnostic-389b.log` / `-b340.log` (2026-06-24): start → ready → resolve →
  one 30 s heartbeat → death, no error; `debug.log` shows the Qdrant `:6334` connect + request with
  no response. Five older instances on the same project (connected pre-break) stayed alive.

## Hypotheses tried
1. **codescout crash/panic/OOM on startup.** Test: grep all diagnostic + debug logs for
   panic/SIGKILL/OOM. Verdict: **rejected** — none; the server logs "ready" and a healthy heartbeat.
2. **Stale binary missing a prior fix.** Test: compare running sha vs fix commits. Verdict:
   **rejected** — running sha `46f48231` carries the fixes; the same start path works once Qdrant is reachable.
3. **Qdrant op blocks past the host timeout because the codescout-side timeout (120 s) ≫ host (~30 s).**
   Test: read `QdrantWrap::connect` timeout + the `build_tool_context` Qdrant path; confirm error-only
   degrade. Verdict: **confirmed.**

## Fix
*(Proposed — not yet implemented.)*
1. **Bound init-path Qdrant work with a short wall-clock ceiling** well under the host budget — wrap
   the artifact-store (and any startup-critical) Qdrant call in `tokio::time::timeout(~5 s, …)`, or
   add a short *connect/bootstrap* timeout distinct from the 120 s *operation* timeout.
2. **Treat a timeout exactly like the existing connect-error branch** — degrade (artifact store
   `None` / semantic unavailable), log a clear one-line WARN ("Qdrant unreachable; semantic/artifact
   features disabled this session"), and **lazily retry** on later calls so it self-heals once Qdrant
   returns.
3. **Keep all Qdrant I/O off the MCP `initialize` critical path** so `initialize` always completes
   fast; defer Qdrant touches to first actual semantic/artifact use, bounded + degrading per (1)/(2).
4. Consider lowering the global 120 s op timeout, or splitting connect vs. long-scroll timeouts.

## Tests added
N/A — open proposal, no fix yet. When implemented, add a test that points the artifact/semantic
store at a black-hole listener (accept-and-hang) and asserts tool-context construction returns in
< a few seconds with the feature marked unavailable (not blocked/erroring the server).

## Workarounds
- Ensure Qdrant is reachable before starting/reconnecting codescout (`curl -m3 127.0.0.1:6333/readyz`).
- If Qdrant is wedged by Docker/VPN networking, run the `agents/system` `fix-vpn-docker.sh`, then `/mcp` reconnect.
- Lite stack escape hatch: `CODESCOUT_VECTOR_BACKEND=sqlite-vec` (no Qdrant daemon).

## Resume
Implement the bounded-timeout + degrade in `build_tool_context` (`src/librarian/mod.rs`) first (the
startup-critical path), then mirror it at `agent/mod.rs:1587` and `retrieval/client.rs:59`. Add the
black-hole-listener regression test. Verify a fresh start against an unresponsive Qdrant completes
init quickly with semantic/artifact features disabled rather than the server being killed.

## References
- `src/retrieval/qdrant.rs:24` (`QdrantWrap::connect`, 120 s timeout)
- `src/librarian/mod.rs:112-128` (`build_tool_context` artifact-store init, error-only degrade)
- `src/agent/mod.rs:1565-1600` (lazy semantic memory store), `src/retrieval/client.rs:59` (code store)
- `agents/system` repo: `VPN_DOCKER_RECURRING_ISSUE.md` (the environment trigger + its permanent fix)
