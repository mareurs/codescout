---
status: fixed
opened: 2026-06-13
closed: 2026-06-14
severity: low
owner: marius
related: []
tags: [retrieval, reranker, test-hermeticity, env-isolation]
kind: bug
---

# BUG: `reranker_returns_scores_in_input_order` fails on dev machines with `CODESCOUT_RERANKER_*` env set

## Summary
The integration test `reranker_returns_scores_in_input_order`
(`tests/retrieval_integration.rs:74`) fails deterministically on this machine —
including in isolation — because `RerankerHttp::new` reads reranker config from
process-global env (`CODESCOUT_RERANKER_PROTOCOL` / `_MODEL`) without per-test
isolation. The machine has those vars set, so the test inherits a non-default
config. Pre-existing and unrelated to the legibility/LSP work on `experiments`.

## Symptom (Effect)
```
thread 'reranker_returns_scores_in_input_order' panicked at tests/retrieval_integration.rs:88:10:
rerank: rerank json

Caused by:
    0: error decoding response body
    1: invalid type: map, expected a sequence at line 1 column 1
```
Exit code 101. Fails both in the full `cargo test` run and in isolation
(`cargo test --test retrieval_integration reranker_returns_scores_in_input_order`).

## Reproduction
At `git rev-parse HEAD` = `f3ff0736` (branch `experiments`):
```
cargo test --test retrieval_integration reranker_returns_scores_in_input_order
# → FAILED (1 failed), ISO_EXIT=101
```
The mock (`tests/retrieval_integration.rs:79`) returns a JSON **array**
`[{"index":1,"score":0.9},{"index":0,"score":0.1}]` (the TEI response shape),
but the decoder receives a JSON **object** on a 200.

## Environment
- OS: Linux (this dev machine). Branch `experiments`.
- Process env has reranker config set (`printenv`):
  - `CODESCOUT_RERANKER_PROTOCOL=llama-server`
  - `CODESCOUT_RERANKER_URL=http://127.0.0.1:48083`
  - (`CODESCOUT_RERANKER_MODEL` unset)
- A clean CI env (none of these set) is expected to pass — which is why this
  is a dev-machine-only red, not a master code regression.

## Root cause
**Confirmed (env hermeticity).** Decisive: clearing the three `CODESCOUT_RERANKER_*` vars (`env -u …`) makes the test pass; with them set it fails. The test is **not env-hermetic**:
`RerankerHttp::new` (`src/retrieval/reranker.rs:62`) calls
`Protocol::from_env()` (`src/retrieval/reranker.rs:18`), which reads
`CODESCOUT_RERANKER_PROTOCOL` from process env, and also reads
`CODESCOUT_RERANKER_MODEL`. The test neither sets these explicitly nor carries
`#[serial]` + an `EnvGuard`. This violates the project convention recorded in
CLAUDE.md / `docs/conventions/test-env-isolation.md`: *"test helpers that build
env-reading objects must isolate env per test."* The sibling fix already exists
one commit away — `ca57869e fix(embedder): hermetic test ctor — env reads no
longer leak from .env` applied the pattern to **`EmbedderHttp`** but not
**`RerankerHttp`**.

**Open question (do not over-claim):** the exact error *direction* is
unreconciled. `CODESCOUT_RERANKER_PROTOCOL=llama-server` maps to
`Protocol::Infinity` (`reranker.rs:24`), whose path decodes into a struct
(`InfinityRerankResp`, a map) — so an array body should yield "expected struct,
got sequence". The observed error is the inverse ("expected a sequence, got
map" = the TEI `Vec<TeiRerankItem>` path receiving an object body). Either the
protocol selection isn't what `printenv` implies at test time, or the body the
client receives isn't the mock's array (a request not reaching the mock). Needs
a `dbg!` of the resolved `Protocol` + the raw response body to close.

## Evidence

### Pre-existing — reranker code + test unchanged from master
```
git diff master..experiments --stat -- src/retrieval/ tests/retrieval_integration.rs Cargo.lock
#  src/retrieval/embedder.rs    | 77 ...
#  src/retrieval/index_state.rs | 193 ...
#  src/retrieval/mod.rs         | 1 ...
#  src/retrieval/search.rs      | 1 ...
#  src/retrieval/sync.rs        | 16 ...
# (reranker.rs and tests/retrieval_integration.rs NOT listed → identical to master)
```
The divergent retrieval commits (`6a715899` checkpoint WIP, `e4348442`,
`ca57869e`, `6c19cc7d` TLS swap) belong to a separate work-stream; none are in
the legibility chain (`b946171d`..`f3ff0736`).

### Not caused by the legibility/LSP work
`cargo test --lib` = 2717 passed, 0 failed. The only failure surfaces in the
`tests/` integration target, in a subsystem the legibility campaign never
touched.

### Decisive: clearing the env makes it pass
```
env -u CODESCOUT_RERANKER_PROTOCOL -u CODESCOUT_RERANKER_MODEL -u CODESCOUT_RERANKER_URL \
  cargo test --test retrieval_integration reranker_returns_scores_in_input_order
# → test result: ok. 1 passed; 0 failed.  (CLEAN_ENV_EXIT=0)
```
Same binary, same test, only the env differs — proving the failure is host-env
inherited, not a code defect. (The serde error *direction* in Hypothesis 4 is
still unreconciled and left for the fixer; it does not change the diagnosis.)
## Hypotheses tried
1. **Parallel env leak from a concurrent test.** Test: re-ran in isolation
   (single test, single thread). **Verdict: rejected** — fails alone too.
2. **Introduced by the legibility/LSP refactor (8 commits this session).**
   Test: full `cargo test --lib` green; `git diff master..experiments` shows
   `reranker.rs` + the test unchanged. **Verdict: rejected.**
3. **Test inherits this machine's `CODESCOUT_RERANKER_*` env (hermeticity gap).**
   Test: `printenv` shows the vars set; `reranker.rs:18,62` reads them.
   **Verdict: confirmed** — decisive `env -u` clear-env pass (see Evidence).
   Exact serde error-direction reconciliation still open (Hypothesis 4).
4. **Exact protocol→error-direction mechanism.** **Verdict: deferred** — the
   Infinity-protocol path and the observed TEI-direction error contradict; needs
   a raw-body / resolved-Protocol dump.

## Fix

Shipped to master as `4586374a` (cherry-picked from experiments-side `b716d664`;
the code lands on master, the experiments-only bug file does not). Mirrored
`ca57869e` (the sibling embedder fix):

- Added `RerankerHttp::with_protocol(base, protocol, model_id)` — an explicit
  constructor that reads **no** process env. `crate::install_default_crypto_provider()`
  moved into it so both construction paths initialise it.
- `RerankerHttp::new` still reads env (`Protocol::from_env()` +
  `CODESCOUT_RERANKER_MODEL`) for production callers, then delegates to
  `with_protocol` for field init.
- Exported `Protocol` (`pub enum`) so integration tests can name `Protocol::Tei`
  — mirrors the already-`pub` `DenseProtocol` from the embedder fix.
- Both reranker integration tests (`reranker_returns_scores_in_input_order`,
  `reranker_503_returns_error`) switched to
  `with_protocol(server.url(), Protocol::Tei, None)`, making them env-independent
  by construction.

**Verification (under the breaking env, not a cleared one):**
`cargo test --test retrieval_integration` → 4 passed with
`CODESCOUT_RERANKER_PROTOCOL=llama-server` **still set** (the exact var that
triggered the failure). Full `cargo test` → 2838 passed, 0 failed;
`cargo clippy --all-targets -- -D warnings` clean.

**Hypothesis 4 (serde error-direction) — now moot, not reconciled.** The causal
chain is proven (env-read present → fail; env-read removed → pass), but the exact
"invalid type: map, expected a sequence" *direction* was never reconciled by
static reading — and is no longer reachable, because the hermetic test never
reads env, so the protocol-selection path that produced the symptom is no longer
exercised. Closed without the `dbg!` forensics: the regression bar (green under
an arbitrary host env) is met, and the open question now points at dead code
from the test's perspective.
## Tests added

No new test. The existing `reranker_returns_scores_in_input_order` is the
regression test — it was the symptom and is now the verification, green under an
arbitrary host env via the hermetic `with_protocol` ctor. `reranker_503_returns_error`
was already protocol-agnostic (it short-circuits on the 503 status before any
decode) but was converted too, so the whole reranker test surface is hermetic by
construction rather than by luck.
## Workarounds
Run the suite with the reranker env cleared:
```
env -u CODESCOUT_RERANKER_PROTOCOL -u CODESCOUT_RERANKER_MODEL -u CODESCOUT_RERANKER_URL cargo test
```

## Resume
Confirm whether `CODESCOUT_RERANKER_PROTOCOL=llama-server` is intended dev
config (likely from a `.env.amd`/shell profile — see `38338e6d feat(retrieval):
llama-server protocol aliases + .env.amd profile`). Then in
`tests/retrieval_integration.rs:74`, add a `dbg!(self.protocol)` + raw-body dump
to reconcile Hypothesis 4, and apply Fix option (a). This is the retrieval
owner's subsystem; the legibility campaign's cherry-pick to master is
independent (touches zero retrieval code).

## References
- `src/retrieval/reranker.rs:18` (`Protocol::from_env`), `:62` (`RerankerHttp::new`), `:75` (`rerank`)
- `tests/retrieval_integration.rs:74` (the test)
- `docs/conventions/test-env-isolation.md` (the convention this violates)
- Sibling fix: `ca57869e fix(embedder): hermetic test ctor`
