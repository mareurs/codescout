---
kind: bug
status: fixed
tags:
- testing
- test-isolation
- embeddings
- timeout
- hang
closed: 2026-08-29
opened: 2026-08-29
owner: marius
related:
- docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md
severity: high
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

**Fix 1 (the type) — APPLIED.** `src/retrieval/transport.rs` (new) states the
timeout policy once, for both remote legs:

```rust
reqwest::Client::builder()
    .read_timeout(read_timeout)
    .build()
```

`read_timeout`, **not** `timeout` — the distinction is load-bearing and was the
main design decision here. `timeout` bounds the whole request, and this crate's
own measurements (`DEFAULT_INFLIGHT` in `retrieval::embedder`) record legitimate
32-input GPU batches at 6.6-12.1s of inference and 23-33s end to end, so a
total-request timeout tight enough to catch a wedge would cut off real work.
`read_timeout` applies per read operation and resets after every successful read.
reqwest's own doc: *"more appropriate for detecting stalled connections."*

Default **120s**, matching the Qdrant client's timeout in
`retrieval::client::from_config_only`, and deliberately generous — a cold GGUF
load can accept a connection well before it can answer. The goal is a **bounded**
wait, not a short one. Override with `CODESCOUT_HTTP_READ_TIMEOUT_SECS`; a zero
or unparseable value falls back to the default rather than erroring, so an
operator typo cannot restore the unbounded-wait behaviour.

Put in one module rather than copied into both legs, deliberately: the same
session filed `docs/issues/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md`
about a duplicated predicate whose two copies have different test coverage, and
there was no reason to create a second instance of that shape.

Env is read only in `EmbedderHttp::new` / `RerankerHttp::new`, never in their
`with_config` / `with_protocol` siblings — those are the explicit-control paths
the tests use. The new `with_read_timeout` builders **rebuild** the client rather
than mutating it, mirroring `with_inflight_override`, so no test needs to touch
real process env (`EnvGuard` and `serial_test` are banned crate-wide).

Side effect worth naming: this makes the `e.is_timeout()` arm of the dense
`send()` error map reachable for the first time. It could never fire while no
timeout was configured anywhere — so the wedge case, the confusing one, produced
no message, while the obvious connect-refused case got the helpful one.

**Fix commit:** `9f4debc3` on `experiments`
**patch-id:** `447d6f36dedcdbfb855d572535c00d6139e91e9c`

The patch-id is the durable half of the pair: `experiments` is rebased after
every ship, so the SHA is positional and dies, while the patch-id is a content
hash of the diff and survives both rebase and cherry-pick.

**Fix 2 (the tests) — APPLIED 2026-08-30.** Two rounds, because the first missed
a third resolution path:

- Round 1: audited every `tools::memory::tests` fixture that builds a real
  `Agent` (not just the two named `test_ctx_with_project*` helpers) — found 5
  more direct `Agent::new(...)` construction sites
  (`test_ctx_no_project`, `multi_project_ctx`, `memory_write_routes_to_project_dir`,
  `memory_write_accepts_project_alias_for_project_id`, `workspace_ctx_with_sub_project`),
  2 of which (the two `memory_write_*` tests) reach a REAL, successful "write"
  action with no isolation at all. Added `set_memory_embedder_for_test` +
  `set_semantic_memory_store_for_test` to all 5.
- Round 2: a concurrent session (same day) independently found and fixed a
  THIRD resolution path this seam never covered — `create_semantic_anchors`'s
  code-chunk search built its own `RetrievalClient::from_env` rather than going
  through the embedder/store seams (see `Agent::code_search` +
  `Agent::set_code_search_for_test`, and the regression test
  `a_memory_write_reaches_code_search_through_the_seam`). That fix only
  auto-installs its `NoCodeSearch` default inside `test_ctx_with_project_raw`,
  so it did not automatically cover the 5 fixtures from round 1 that bypass
  that helper — added `set_code_search_for_test(NoCodeSearch)` to all 5.

**Verified by reproduction, not by reading the fix.** Pointed `CODESCOUT_EMBEDDER_URL`
at a listener that accepts and never answers (matching this bug's own repro
shape) and confirmed, before fixing: `memory_write_routes_to_project_dir` took
12.25s and hit the wedge 4 times; `memory_write_accepts_project_alias_for_project_id`
took 6.40s and hit it twice. After fixing all three seams: both run in
0.03–0.04s with **zero** connections to the wedge — not merely fast, genuinely
hermetic. Full suite: `cargo fmt`, `cargo clippy --workspace --all-targets
--features local-embed -D warnings`, `cargo test` (4810 passed, 0 failed, 46
ignored), `cargo check --no-default-features` — all green.

Not yet committed as of this update — landed in a working tree a concurrent
session was also actively editing (`src/agent/mod.rs`,
`src/tools/memory/mod.rs`); see that session's own commit for the `code_search`
seam itself.
## Tests added

`a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever`
— `src/retrieval/embedder.rs`, in the `remote-embed`-gated `tests` module.

Binds a listener that accepts connections and **holds them open and silent**.
Deliberately not a closed port: that fails on connect, which already worked and
already produced a clear error. Holding rather than dropping the streams is also
load-bearing — a dropped stream closes the socket, which the client reports
promptly as a clean EOF, and the test would pass with no timeout configured at
all and prove nothing.

**Verified by mutation, not by passing.** Swapping `read_timeout` for
`connect_timeout` — the realistic wrong-instrument mistake, since a wedged peer
connects fine — moves the test from `ok in 0.56s` to `FAILED in 30.16s`, via the
outer `tokio::time::timeout` firing with `Elapsed(())`. That outer bound is what
makes the failure a red test rather than a wedged binary, the same guard and the
same reason as `sparse_retry_cap_stops_at_exactly_8_attempts`.

**End-to-end, against the still-wedged live server:** a memory test that
previously hung past 900s completed in **5.22s** under
`CODESCOUT_HTTP_READ_TIMEOUT_SECS=5`, with the embedder on `127.0.0.1:48081`
still returning `HTTP 000`.

Gate at fix time: fmt, clippy `--workspace --all-targets --features local-embed
-D warnings`, test (4637 passed, 0 failed), and four lean configurations.
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

N/A — both fixes are done as of 2026-08-30. See the updated § Fix for what
changed and how it was verified (reproduction against a wedge listener, zero
connections after the fix, full gate green). Not yet committed — landed in a
working tree a concurrent session was also actively editing.
## References

- `src/retrieval/embedder.rs:368` — the untimed client
- `src/retrieval/embedder.rs:441` — the `is_timeout()` branch this makes unreachable
- `src/retrieval/embedder.rs:1325-1354` — the one guarded call site, and its comment
- `src/tools/memory/tests.rs:42-46` — ambient resolution, stated in the doc comment
- `docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`
  — the pollution half of the same ambient-resolution root cause
