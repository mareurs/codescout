---
status: open
opened: 2026-08-29
closed:
severity: high
owner: marius
related:
  - docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md
tags:
  - testing
  - test-isolation
  - embeddings
  - timeout
  - hang
kind: bug
unverified: Hypothesis 2 (that the ET-2 working-tree change is not the cause) is rejected on structural grounds only — remote-embed is a default feature so nothing is gated out, the edits are semantically identical, and the hung tests are in a module the change never touches. NOT confirmed by an independent run at HEAD, which needs a full rebuild in a clean tree. See Resume.
---

# BUG: `EmbedderHttp` sets no request timeout, so a wedged embed server hangs `cargo test` forever instead of failing

## Summary

`EmbedderHttp`'s `reqwest::Client` is built with `Client::new()` — no request
timeout — so a server that **accepts the connection but never responds** blocks
the caller indefinitely. Because 16 `tools::memory::tests` resolve their embedder
from ambient environment config, a wedged local llama-server turns `cargo test`
into an unbounded hang with no failure, no output, and no diagnosis.

## Symptom (Effect)

`cargo test` runs past 900s with **no test binary completing**. The log shows
exactly 16 slow tests, all in one module:

```
test tools::memory::tests::memory_write_and_read_via_dispatch has been running for over 60 seconds
test tools::memory::tests::memory_read_missing_topic_embeds_available_and_suggestions has been running for over 60 seconds
... 14 more, all tools::memory::tests::*
```

`grep 'test result:' /tmp/cs-test.log` → **empty**. `grep -c '^test .* FAILED'` →
**0**. Nothing fails; nothing finishes.

The tell that this is a wedge and not slowness: no test outside
`tools::memory::tests` is affected. Retrieval's own fixtures point at
`http://unused.invalid`, which fails fast on connect-refused.

## Reproduction

With a local embed server that is listening but not answering:

```
$ curl -s -m 30 -o /dev/null -w '%{http_code} in %{time_total}s\n' \
    -X POST http://127.0.0.1:48081/v1/embeddings \
    -H 'Content-Type: application/json' \
    -d '{"input":["hello world"],"model":"CodeRankEmbed"}'
000 in 30.002397s
```

`000` = no HTTP response received. The port is open:

```
$ ss -ltn | grep 48081
LISTEN 0  4096  127.0.0.1:48081  0.0.0.0:*
```

Then `cargo test` hangs. Commit: `dde7491b` plus the ET-2 working tree. Observed
2026-08-29.

## Environment

Linux, branch `experiments`. Ambient config present in the shell:

```
CODESCOUT_EMBEDDER_URL=http://127.0.0.1:48081     ← listening, wedged
CODESCOUT_SPARSE_EMBEDDER_URL=http://127.0.0.1:48084  ← not listening
CODESCOUT_RERANKER_URL=http://127.0.0.1:48083     ← listening
CODESCOUT_QDRANT_URL=http://127.0.0.1:6334        ← listening
CODESCOUT_EMBEDDER_PROTOCOL=llama-server
CODESCOUT_RETRIEVAL_PROFILE=gpu
```

CI does not set these, which is why CI has never seen it.

## Root cause

Two independent defects that only bite together.

**1. No request timeout.** `src/retrieval/embedder.rs:368`:

```rust
client: reqwest::Client::new(),
```

`reqwest::Client::new()` is `ClientBuilder::new().build()`, whose `timeout` and
`connect_timeout` both default to `None`. A peer that completes the TCP handshake
and then never writes a response leaves the request pending forever.

This also makes the error branch at `embedder.rs:441` largely unreachable:

```rust
if e.is_connect() || e.is_timeout() {
```

`is_timeout()` cannot fire for a request-level stall when no timeout is
configured, so the helpful "the dense embedder is unreachable (connect/timeout).
Check CODESCOUT_EMBEDDER_URL" message is only produced for connect-refused — the
case that was already obvious. The wedge case, which is the confusing one, gets
no message at all.

**Measured 2026-08-29:** `curl -m 30` → `000`, full 30s elapsed, against a port
`ss` confirms is LISTENing.

**2. Tests resolve the embedder from ambient config.** `tools::memory::tests`
build a real `Agent`; `src/tools/memory/tests.rs:42-46` states plainly that
`test_ctx_with_project_raw()` resolves `semantic_memory_store()` and
`memory_embedder()` "however the environment resolves them". The prior bug
`docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`
addressed the **pollution** consequence of that (fixtures cross-embedding into
the live `memories` collection). It did not address the **availability**
consequence: the same ambient resolution makes the suite's runtime a function of
whether a local GPU service happens to be healthy.

## Evidence

The codebase already knows `EmbedderHttp` can hang forever, and guards exactly
one test against it — `src/retrieval/embedder.rs:1325-1328`:

```rust
/// failure — since the mock has nothing else to fall through to, it keeps
/// answering 500 forever, so the call never returns. Wrapped in an outer
/// `tokio::time::timeout` so that hang surfaces as a clean assertion
/// failure instead of wedging the test binary.
```

"instead of wedging the test binary" is the observed outcome here, in the
un-guarded case. The hazard is documented at one call site and unmitigated at the
type.

## Hypotheses tried

1. **Hypothesis:** general machine load (three peer worktrees building, disk 85%).
   **Test:** checked which tests were slow.
   **Verdict:** rejected — load slows tests across all modules; here the 16 slow
   tests are all in one module and every other test is unaffected.

2. **Hypothesis:** caused by the ET-2 feature-gating change in the same working
   tree.
   **Test:** `remote-embed` is a default feature, so every gated item is compiled
   in for `cargo test`; the three structural edits
   (`build_embedder_for_url`, `rerank_or_passthrough`, the
   `install_default_crypto_provider` body) are semantically identical to what
   they replaced, and `cargo clippy --all-targets` passes. The hung tests are in
   `tools::memory`; the change is confined to `src/retrieval/` plus a cfg'd
   `lib.rs` body.
   **Verdict:** rejected on structural grounds. **Not** independently confirmed by
   a run at `HEAD` — doing so needs a full rebuild in a clean tree, deferred.
   See `unverified` note below.

3. **Hypothesis:** the server is merely slow (GPU-bound, ~6.6-12.1s/request per
   `embedder.rs`'s own `DEFAULT_INFLIGHT` doc).
   **Test:** `curl -m 30` against it.
   **Verdict:** rejected — `000` after the full 30s is no response at all, not a
   slow one.

## Fix

**Fix 1 (the type):** give the client an explicit timeout.

```rust
client: reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(/* configurable */ 120))
    .build()
    .expect("static reqwest client config is valid"),
```

Make it configurable via `[embeddings]`/env rather than hardcoded — a cold GPU
model load can legitimately exceed 60s, so too tight a default trades a hang for
a spurious failure. Landing this also makes the existing `e.is_timeout()` branch
at `embedder.rs:441` reachable, so the wedge case starts producing the
already-written diagnostic.

**Fix 2 (the tests):** stop `tools::memory::tests` resolving the embedder from
ambient config. The isolation helper already exists for the pollution case; extend
the same treatment so no test depends on a live local service.

Neither is applied in this record.

## Tests added

None yet. A regression test for Fix 1 is straightforward and should assert the
*timeout*, not merely that a fast case works: bind a listener that accepts and
never writes, point `EmbedderHttp` at it, and assert the call returns `Err`
within the configured bound. Wrap in `tokio::time::timeout` at a longer bound so
a broken fix fails rather than wedging the suite — the pattern already used at
`embedder.rs:1352`.

## Workarounds

Run the suite with the ambient service config cleared:

```
env -u CODESCOUT_EMBEDDER_URL -u CODESCOUT_SPARSE_EMBEDDER_URL \
    -u CODESCOUT_RERANKER_URL -u CODESCOUT_QDRANT_URL \
    -u LIBRARIAN_EMBED_URL -u LIBRARIAN_EMBED_MODEL \
    cargo test
```

Or restart the wedged embed server (`./scripts/retrieval-stack.sh`).

## Resume

Confirm hypothesis 2 properly: `git stash` the ET-2 tree, run
`cargo test tools::memory::` against the wedged server at `HEAD`, confirm the
same 16 tests hang, unstash. Then implement Fix 1 in
`src/retrieval/embedder.rs:368` with the timeout sourced from config.

## References

- `src/retrieval/embedder.rs:368` — the untimed client
- `src/retrieval/embedder.rs:441` — the `is_timeout()` branch this makes unreachable
- `src/retrieval/embedder.rs:1325-1354` — the one guarded call site, and its comment
- `src/tools/memory/tests.rs:42-46` — ambient resolution, stated in the doc comment
- `docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`
  — the pollution half of the same ambient-resolution root cause
