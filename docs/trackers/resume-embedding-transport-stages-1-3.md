---
id: d24bf146cf7c789f
kind: tracker
status: active
title: Resume queue — Embedding Transport Consolidation Stages 1–3 (ET-N)
owners:
- marius
tags:
- resume-queue
- embeddings
- retrieval
- codescout-embed
- dependencies
topic: embedding transport boundary
entry_high_water_ET: 8
entry_prefix: ET
---

# Resume queue — Embedding Transport Consolidation Stages 1–3 (ET-N)

**ADR:** `docs/adrs/2026-07-25-embedding-transport-boundary.md`
**Plan:** `docs/plans/2026-07-25-embedding-transport-consolidation.md` (`0da5bd672ef60dfc`)

**Goal.** `codescout-embed` owns remote embedding transport. `src/retrieval`
keeps the domain vocabulary and a `server-stack`-only sparse + rerank client.
Root drops `reqwest`, `rustls`, and `install_default_crypto_provider`.

## State, verified at the bytes 2026-08-28

**Stage 0 is DONE.** Its gate was "all three contracts written into the ADR's
*Consequences* section" — confirmed present at
`docs/adrs/2026-07-25-embedding-transport-boundary.md:149+`, all three
(connect-error marker, dimension contract, query prefix). The `.env` profile
field-note fix also landed: all four active profiles now have
`CODESCOUT_QUERY_PREFIX` commented out with the benchmark citation.

**Stages 1–3 are untouched:**

- `Cargo.toml:89` — `reqwest` is an **unconditional** dependency.
- `Cargo.toml:92` — `rustls` is an **unconditional** dependency.
- `Cargo.toml:198` — `server-stack = ["dep:qdrant-client"]` only; the HTTP half
  was never gated.
- `src/retrieval/embedder.rs` was never split.

## How to use this queue

**To act:** ET-1 first — it pins an invariant everything else rests on. Then
ET-2, which banks the entire measured payoff on its own. ET-3/ET-4 are optional
follow-through.

**To append:** one call, from the main checkout —

```
artifact(action="append_entry", id="<this artifact's id>", id_prefix="ET",
         anchor_heading="## Template for new entries", title=…, body=…)
```

**Deliberately unaugmented** — see `docs/conventions/cross-machine-catalog-resume.md`.

## Provenance

Opened 2026-08-28 from a full-surface partial-implementation sweep. Stage 0's
completion and Stages 1–3's absence were both checked against `Cargo.toml` and
the ADR rather than inferred from the plan's `draft` status.

## ET-1 — Pin the load-bearing invariant with a test, before anything depends on it

**Status:** DONE 2026-08-28 — `2bd3415b` (`experiments`), patch-id `9e434457`
**Valid:** invariant — now pinned by a test, mutation-verified 3/3

The whole consolidation rests on one property, currently true and currently
maintained by **three files agreeing with each other by accident**. Nothing pins
it:

> `!cfg(server-stack)` ⟹ `lite == true` ⟹ (`dense_only == true` ∧ reranker never invoked)

Proof chain, each link read 2026-07-25:

1. `src/retrieval/client.rs:65-71` — under `#[cfg(not(feature = "server-stack"))]`,
   `qdrant_code_store` is an `anyhow::bail!`.
2. `src/retrieval/client.rs:77` — `from_config_only` is itself
   `#[cfg(feature = "server-stack")]`, so `from_env` is the only constructor in a
   lean build.
3. `from_env:31-36` routes `VectorBackend::Qdrant` to that bail, so a lean build
   can only reach `VectorBackend::SqliteVec` → `lite = true` (`:32`).
4. `src/retrieval/client.rs:43` — `dense_only = lite || config.disable_sparse` → `true`.
5. `src/retrieval/search.rs:84` — returns before `self.reranker.rerank(...)` at `:90`.

**Consequence, and why it makes Stage 1 safe:** in a lean build the sparse leg
(`embed_batch:353-440`) and the entire reranker are **already dead code**. Gating
them is a no-op at runtime.

**Done.** Two tests in `src/retrieval/client.rs::selection_tests`:
`a_lean_build_cannot_construct_a_non_lite_client` (links 1–3) and
`lite_alone_forces_dense_only_and_vetoes_the_reranker` (links 4–5, each half behind
its own negative guard so neither can pass vacuously). Mutation-verified with three
probes, each killing its test on its own named assertion.

**The line numbers HAD drifted, and two links moved in substance — re-read them
before citing the chain above:**

- `dense_only` is now `lite || disable_sparse || backend_is_local(config)`
  (`src/retrieval/client.rs:194-196`). The third term is new. Harmless to the
  implication — it is an OR — but the expression this entry quotes is stale.
- Link 5's inline `if !opts.rerank || self.lite || ...` was extracted into a named
  `should_rerank(caller_wants, operator_enabled, lite, n_candidates)`
  (`src/retrieval/search.rs:21-28`). That refactor is why this was cheap to pin.

**Gate at commit time:** clippy green, `--no-default-features` green, 4617 tests
pass. One failure, `server::tests::tool_surface_under_budget`, and two `fmt` diffs —
all three owned by a concurrent session's uncommitted work, none reachable from a
test-only change. Detail in the commit message.

**ET-2 is now unblocked.**

## ET-2 — Stage 1: split the module, gate the HTTP half — banks the whole win alone

**Status:** shipped 2026-08-29 — `2c6f2677` on `experiments`, patch-id
`07366d4b5d24c784f31fe56aa32c8a6e95411c61`. Record the patch-id, not just the
SHA: `experiments` is rebased after every ship and the SHA dies with it.
**Valid:** dated 2026-08-29

**Measured on landing: `bare 274 → 226 (−48)`, `remote-embed +1 → +49`.** The
predicted figure held exactly, which also closes `ET-6`.

**Read `ET-7` before touching `ET-3`/`ET-4`.** The plan's Stage 1 design table
was wrong in six places and the corrected gate is `remote-embed` throughout —
not `server-stack`, which breaks both the lean and the default build. The file
split described below did **not** happen: the HTTP items are interleaved, so
they were gated in place instead.

**This stage banks the entire 48-crate win on its own.** If Stages 2–3 never
happen, Stage 1 alone is still worth having.

**Measured payoff:** `--no-default-features` goes 274 → 226 crates (−48: the whole
`hyper`/`h2`/`tower`/`rustls`/`ring` stack). **Six of nine test cells** stop
compiling a TLS stack they never call — the matrix at
`.github/workflows/ci.yml:70-76` runs `no-features` *and* `local-embed
--no-default-features` across three OSes. (This line said "two CI lanes" citing
`:46`/`:48` until 2026-08-29; those are the clippy job, not the test matrix.)
Default build is unchanged at 339 — **the win is CI time and manifest honesty,
not shipped binary size. Do not oversell this to reviewers.**

**Next:** split `src/retrieval/embedder.rs` so the HTTP half sits behind a
feature gate, then move `reqwest`/`rustls` in `Cargo.toml` from unconditional to
optional under it. ET-1's test is what makes the gate provably safe.

## ET-3 — Stage 2: swap the dense leg to `RemoteEmbedder` — blocked on a three-state query prefix

**Status:** blocked on a design change to `codescout-embed`
**Valid:** dated 2026-07-25
**Rests on:** ADR § *The three contracts*, item 3

Root and crate derive the query prefix from **different sources of truth**, and
all three disagreements fail as degraded recall, never as an error:

| model | `CODESCOUT_QUERY_PREFIX` | root today | crate | effect of a naive swap |
|---|---|---|---|---|
| CodeRank | unset | *nothing* | prefix | **regression** |
| CodeRank | custom | custom | hardcoded | **regression** |
| other | set deliberately | applied | *nothing* | **regression** |

Row 1 is the counter-intuitive one and was inverted in an earlier draft of the
plan. `docs/manual/src/concepts/retrieval-stack.md` benchmarks CodeRankEmbed
**Q4_K_M with no prefix at 37 (champion)** vs f16+prefix at 34 — *"Q4 loses
asymmetric subspace if a prefix is forced."* So root applying **nothing** is
correct on the default model, and the crate's unconditional model-derived prefix
is the defect.

**The blocker:** `Option<String>` cannot express "explicitly no prefix" distinctly
from "derive from model name", and on Q4 the former is what we want.
`RemoteEmbedder` needs **three** states: *derive*, *explicit value*, *explicitly
suppressed*. Root maps unset `CODESCOUT_QUERY_PREFIX` → **suppressed**.

**Also in this stage:** `dense_model_name` defaults to the **empty string**
(`new():126`), so root sends `{"input": […], "model": ""}` today — tolerated by
llama-server, rejected by stricter gateways. Decide whether the crate's
required-model contract is adopted (preferred) or an empty model stays legal.

**Next:** add the three-state prefix to `codescout-embed` first; the swap is
mechanical after that. Hold batch size at 8 so the change is behaviour-preserving.

## ET-4 — Stage 3: delete the duplicates and the root manifest entries

**Status:** open — blocked on ET-3. **Re-framed 2026-08-29: this is a
correctness item, not trailing cleanup.**
**Valid:** dated 2026-08-29

### The duplicates have measurably drifted, and only in one direction

Two instances found on 2026-08-29, both in the same file pair
(`src/retrieval/embedder.rs` vs `crates/codescout-embed/src/remote.rs`), both
verified at the bytes:

| duplicated item | crate (`remote.rs`) | root (`embedder.rs`) |
|---|---|---|
| `is_https_or_loopback` | 11 host-spoofing assertions (`:549-563`) | **zero tests** |
| HTTP client timeout | `.timeout(300s)` + rationale (`:95-101`) | **none**, until `9f4debc3` |

Neither is a stylistic difference. The first means a mutation to root's
cleartext-API-key guard passes the entire suite
(`docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md`
— the coverage half is closed by `28bb6e8a`; the duplication is this entry's job).
The second let a wedged embedder hang `cargo test` indefinitely
(`docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`).

**The crate's doc comment names the exact trigger that occurred:**

> *"Build a reqwest client with a per-request timeout so that a hung embedding
> server (e.g. Ollama during GPU discovery failure) doesn't block
> `index_project` forever."*

The host's GPU driver failed a suspend/resume on 2026-08-28 19:10; the dense
llama-server accepted TCP and never answered for 15 hours. The crate had
anticipated that class in a comment. Root's copy had not.

### The project already proved the remedy, and stated why

`normalize_embedder_url` (`src/retrieval/config.rs:292`) is the one piece of this
surface that **was** consolidated, and its doc comment gives the reason:

> *"shared with `RemoteEmbedder::from_url`'s identical three-branch logic rather
> than duplicated here, so the two conventions cannot drift apart."*

That is the hypothesis. The two rows above are its confirmation: the siblings
that were left duplicated drifted, in the predicted direction — root's copy is
the one missing the guard, both times. So Stage 3 is not tidying; it is closing a
demonstrated defect channel, and its remaining surface should be audited pairwise
rather than deleted wholesale.

### Known asymmetry to resolve DURING the swap, not after

The two timeouts are not the same instrument, and the crate's is the weaker one:

- root (`transport::client`): `.read_timeout(120s)` — gap between bytes, resets
  on every successful read
- crate (`RemoteEmbedder::http_client`): `.timeout(300s)` — whole request

A total-request bound can cut off a legitimately slow large batch;
`embedder.rs`'s own `DEFAULT_INFLIGHT` measurements record 32-input GPU batches
at 23-33s end to end, and CPU-backed batches are ~4x slower again. Swapping
root's dense leg to `RemoteEmbedder` as-is would therefore **regress** the
guard root just gained. Port `read_timeout` into the crate as part of ET-3.

Remove root's now-unused dense transport code, `reqwest`, `rustls`,
`install_default_crypto_provider`, and — added 2026-08-29 — `is_https_or_loopback`
and `src/retrieval/transport.rs`, both of which duplicate a crate original.

**Audit each pair before deleting.** Root's copy is not always the stale one: the
`read_timeout`-vs-`timeout` row below is a case where root is *ahead* of the
crate, and a wholesale delete would silently drop the better guard.

**Verification is the feature-delta measurement**, not a passing test suite:
re-run the crate counts and confirm `--no-default-features` actually dropped. The
plan's § *Verification: the feature-delta measurement* has the method.

**Next:** blocked on ET-3.

## ET-5 — The connect-error marker becomes a cross-crate string contract

**Status:** open — lands with ET-3, decided in Stage 0
**Valid:** dated 2026-07-25

`src/retrieval/embedder.rs:221` emits `"dense embed connect failed: {url} — …"`.
`src/tools/semantic/semantic_search.rs:46` matches
`err_str.contains("embed connect failed")` to route the user to the embedder hint
instead of the misleading "check qdrant logs" fallback. Both sides carry
regression tests (`src/retrieval/embedder.rs:486-499`,
`src/tools/semantic/semantic_search.rs:495`).

Moving the producer into `codescout-embed` promotes a substring contract to a
**crate boundary, where nothing makes the two tests fail together**.

**Resolution already decided in the ADR:** the crate publishes the contract —
a typed `EmbedError::Connect { url }`, or at minimum a documented stable marker
with its own crate-side test. **Constraint:** the dependency points one way;
`codescout-embed` must not learn that `semantic_search.rs` exists.

**Next:** implement as part of ET-3, not after it.

## ET-6 — Re-measure the −48 crate figure before quoting it

**Status:** closed 2026-08-29 — re-measured during ET-2 (`2c6f2677`). The
month-old figure held **exactly**: `bare 274 → 226 (−48)`.
**Valid:** dated 2026-08-29

The 274 → 226 figure was measured 2026-07-25. The dependency tree has moved since
(`codescout-embed` gained the `local` ONNX module, among others). Quoting a
month-old crate delta in a PR description is exactly the decay this repo's
tracker discipline exists to catch.

**Measured 2026-08-29**, immediately before committing ET-2:

| feature | crates | delta vs bare |
|---|---|---|
| *(bare)* | **226** | — |
| `remote-embed` | 275 | **+49** |
| `server-stack` | 324 | +98 |
| `librarian` | 275 | +49 |
| `http` | 259 | +33 |

The headline number was safe to quote after all. Two sibling rows moved for
reasons worth knowing, and neither is a regression:

- **`remote-embed` +1 → +49** is the whole point — that `+1` was the bug the plan's
  own baseline table flagged. Root forwarded the feature while keeping its own
  `reqwest`/`rustls` unconditional, so enabling it cost one crate because the
  stack was already paid for.
- **`http` +17 → +33** is not growth. `http` (axum/tower-http) previously
  **freeloaded** on the always-compiled `reqwest`'s hyper/h2 stack; now that
  `reqwest` is optional it pulls that stack itself. A default build has the same
  total crates as before — only the attribution changed, which is the manifest
  honesty this stage was for.
- **`server-stack` +98** is `qdrant-client` (+49) plus the newly-declared
  `remote-embed` implication (+49) — a dependency it always had and never stated.

**Method note for whoever re-runs this:** do not compose per-package closures.
`cargo tree -p X --no-default-features` applies the flag to `X`, not to the root,
and returns degenerate results.

## ET-7 — Stage 1's design table was wrong in six places; the corrected gate is `remote-embed` throughout

**Observed:** 2026-08-28, executing ET-2 against the approved plan
`docs/plans/2026-07-25-embedding-transport-consolidation.md` § Stage 1.

**Outcome: shipped.** `bare 274 → 226 (−48)`, `remote-embed +1 → +49`. Gate green
(`fmt`, `clippy --workspace --all-targets --features local-embed -D warnings`,
`check --no-default-features`), plus four lean matrix configs.

**The plan's Stage 1 could not be executed as written.** Six defects, found in this
order. The first was already corrected in the plan's own preamble; the other five
were not, and #2 would have broken the build in two configurations.

1. **1.2's `server-stack` gate on `EmbedderHttp`** — already corrected to
   `remote-embed` in the plan preamble. Correct.

2. **The same error one table row down, uncorrected: `RerankerHttp` → `server-stack`.**
   Justified as "invariant: never invoked in a lean build". That is a *runtime*
   invariant — `should_rerank` gates on `lite`, which is what ET-1 pinned — and a
   runtime guard does not delete a code path. `search.rs:154` hard-references
   `self.reranker` from ungated `search_in`, so it must typecheck in **every**
   configuration. Gating `pub mod reranker` on `server-stack` breaks the lean build
   *and* the default build, and would silently drop reranking from the default build
   — a behaviour regression, not a manifest cleanup. This is CLAUDE.md's
   *"'already fails loudly' is a claim about a code path, not about a feature"*.

3. **Task 6's "add `dep:reqwest`/`dep:rustls` to BOTH features" is wrong.**
   `server-stack = ["dep:qdrant-client"]` is tonic/gRPC and does not use `reqwest`
   at all. Correct shape: both deps under `remote-embed` only, and
   `server-stack = ["dep:qdrant-client", "remote-embed"]` — because
   `from_config_only` is `server-stack`-gated yet constructs **both** `EmbedderHttp`
   and `RerankerHttp`. Without the implication, `--no-default-features --features
   server-stack` compiles those call sites against configured-out types. That
   configuration is not in the plan's verification list; it is now verified.

4. **1.1's "entire surface" list omits five items.** The plan's own note caught three
   (`CodeEmbedder`, `CodeDenseAdapter`, `CodeEmbedderAdapter`). Two more: `DEFAULT_INFLIGHT`
   and `embed_chunks_ordered` are HTTP-only in practice — sole callers are
   `EmbedderHttp::{resolve_inflight, embed_batch}` — so both become `dead_code` and
   fail `-D warnings`.

5. **Imports are unmentioned and all three are fully unused in a lean build** —
   `anyhow::{anyhow, Context, Result}`, `futures::stream::{StreamExt, TryStreamExt}`,
   `serde::{Deserialize, Serialize}`. The ungated traits use fully-qualified
   `anyhow::Result` in their signatures, so nothing else needs them.

6. **`is_https_or_loopback` does NOT stay ungated.** The plan says it does, "used by
   ungated `guarded_api_key`". `guarded_api_key`'s only caller is `build_http_embedder`,
   so once that is gated the whole API-key-over-cleartext guard chain is dead code.
   Both are now `remote-embed`.

**Mechanism deviation, deliberate.** Task 1 prescribed a physical
`embedder/mod.rs` + `embedder/http.rs` split. The HTTP items are **interleaved**, not
contiguous — `impl CodeEmbedder for EmbedderHttp` (75-92) sits between `CodeEmbedder`
(41-72) and `CodeDenseAdapter` (96) — so a split meant hand-moving ~1,300 lines,
transcription risk no gate catches. Gated in place with 15 `#[cfg]` attributes
instead: same compile-time outcome, and `cargo check --no-default-features` proves
completeness (rustc names each configured-out item). The plan's *goal* is manifest
honesty, not file layout; the split was a means.

**Two paired-definition helpers were extracted**, mirroring `qdrant_code_store`'s
existing lean arm, because `#[cfg]` on a tail expression is unstable:
`RetrievalClient::build_embedder_for_url` (client.rs) and
`RetrievalClient::rerank_or_passthrough` (search.rs). The latter's lean arm is a
reachable degradation, not `unreachable!()` — a lean non-lite stack with
`rerank = true` still arrives there and gets vector-query order, exactly what the
remote-embed version's `Err` arm returns when the reranker is unreachable.

**Known cost, accepted deliberately.** Eleven test items are now `remote-embed`-gated
because their fixtures build an inert `EmbedderHttp` placeholder
(`sync.rs::test_retrieval_client` + 5 callers + `SlowEnsureStore`;
`search.rs::client_with_store` + 2 callers + `effective_model_dim_falls_back_when_nothing_is_known`),
plus `tests/retrieval_integration.rs` file-level. `FixedDimEmbedder` exists in both
modules and would preserve the coverage — **not** taken: `effective_model_dim_falls_back_when_nothing_is_known`
turns on what an embedder reports when its dim is unknown, and a placeholder chosen
for a different assertion can be unreachable-by-construction for this one. Gating
cannot make a test lie; swapping can. Retrofitting a lean-safe inert embedder is
follow-up work, not Stage 1.

**Also found, out of scope:** `rendezvous_poll_for_test` (`server.rs:988`) is dead
under `--no-default-features --all-targets` because `guide_hint_tests` is
`librarian`-gated. **Pre-existing** — CLAUDE.md's lean gate omits `--all-targets`, so
it was never surfaced. Not touched.

**Severity:** high — defect #2 alone would have failed `cargo build` in the default
configuration, and the "fix" for that failure most people would reach for (gate
`search_in`'s rerank block on `server-stack` too) silently removes reranking from
every default deployment.

**Status:** fixed-verified

**Valid:** dated 2026-08-28

True of the gate topology at this commit. Re-derive if `search_in` stops referencing
`self.reranker`, or if `from_config_only` loses its `server-stack` gate.

**Rests on:** `should_rerank` gating on `lite` rather than on a feature (ET-1 pins
this), and `from_config_only` being `server-stack`-gated while constructing both HTTP
types.

## ET-8 — Ordered execution plan for ET-3/4/5 and the findings around them

**Observed:** 2026-08-29, after ET-2 shipped. Reviewed with the architecture
(snow-lion) and refactoring (yak) lenses.

**Status:** open — **A and B1 DONE. B2 is BLOCKED on a user decision** (below);
do not start it unilaterally.

| phase | state | commit | patch-id |
|---|---|---|---|
| A | done | `28bb6e8a` | `52cb00b5b67d80de322ccc0c9f5a6166d1860fb0` |
| B1 | done | `ffdf1b09` | `9bee6603a61ef66ba6aaf3b999896d64bceb68d2` |
| B2 | **blocked** | — | — |
| B3, B4, C, D | not started | — | — |

**Why B2 is blocked, and what unblocks it.** B2 is not plumbing — it decides
whether an unset `CODESCOUT_QUERY_PREFIX` means *suppressed* or *derive from the
model name*, and that is benchmark-visible retrieval quality. The operator's live
config currently disagrees with the repo's own measurement:

- `~/.claude/settings.json` sets `CODESCOUT_QUERY_PREFIX = "Represent this query
  for searching relevant code: "` alongside
  `CODESCOUT_EMBEDDER_MODEL_NAME = CodeRankEmbed-Q4_K_M.gguf`.
- The repo's `.env` has that exact line **commented out**, because "Q4_K_M is
  benchmarked best with NO query prefix (37, champion) — forcing the prefix drops
  to the f16+prefix tier (34)".

So the machine is sitting on ET-3's row 2 (*CodeRank + custom prefix →
regression*). Two questions to put to the user before writing code: (1) does unset
mean suppressed — ET-3's table says yes; (2) should the `settings.json` prefix be
removed to match the benchmark, or was it deliberate? Answering (1) is enough to
start B2; (2) is separable and is theirs regardless.

**Also landed alongside B1, not part of the plan:** `21174425` (patch-id
`9885fb29d5499e85b27532de50688cbf59d1c942`) removed a race in the wedged-peer
test that `9f4debc3` had shipped — `embed_one_batch` drives both legs through
`tokio::try_join!`, which returns whichever errors first, so a dense-specific
message assertion against two wedged bases was a coin flip. `dense_only(true)`
never applied; `embed_one_batch` does not consult it. Fixed by calling
`dense_batch` directly. Found by a peer session running the suite under
contention; it passes in isolation, which is why its authoring session missed it.

**Valid:** dated 2026-08-29

**Valid:** dated 2026-08-29

**Rests on:** the dependency direction measured below, and the crate visibility
table. Both re-check in one command each; do so before resuming.

### The architectural finding: there is no boundary to move

Measured 2026-08-29 — `codescout_embed::` is referenced **44 times across 10
root files**; `crates/codescout-embed/src/lib.rs` imports nothing from root. The
dependency is already one-way and already points inward.

So this is **not** an architecture task and no new wall is warranted — proposing
one would be a boundary with no change scenario behind it. It is a **DRY
violation with measured drift**: root re-implements what it already depends on,
and the copies diverged, twice, always with root missing the guard
(`ET-4`). The remedy is deletion toward an existing concrete, not a new
abstraction.

### What gates what — the crate's surface

| crate fn | visibility | consequence |
|---|---|---|
| `normalize_embeddings_base` (`lib.rs:39`) | **`pub`** | already shared; the precedent that worked |
| `is_https_or_loopback` (`remote.rs:48`) | private | blocks deleting root's copy |
| `http_client` (`remote.rs:95`) | private | blocks sharing the timeout policy |
| `query_prefix_for` (`remote.rs:105`) | private | this is ET-3's actual blocker |

Three private fns must become `pub` before any root duplicate can go. That is
the real precondition, and it is cheap — but it is a crate API change, so it
belongs in its own phase ahead of the consumer swap.

### Order

Each phase ends green and is independently revertable. Baseline to hold:
**4637 passed, 0 failed** (`cargo test`, ambient CODESCOUT_* unset).

**Phase A — safety net. No behaviour change.**

- **A1.** Port the 11 host-spoofing assertions from
  `remote.rs:549-563` to root's `is_https_or_loopback`.

  This is **not** an alternative to deleting root's copy, which is how
  `docs/issues/archive/2026-08-28-root-is-https-or-loopback-has-no-test-coverage.md`
  originally framed it — corrected there when A1 landed. It is the **characterization test that makes the
  deletion in D1 verifiable** — delete an untested function and nothing proves
  the replacement equivalent. Verify the tests bind by mutating root's host
  parse to the unanchored form and confirming red before committing green.

**Phase B — crate API. Contract changes, ahead of any consumer.**

- **B1. DONE** (`ffdf1b09`). Ported `read_timeout` into
  `RemoteEmbedder::http_client`, keeping the existing total `timeout(300s)` — the
  two bound opposite failures and neither subsumes the other. Note for later
  phases: `http_client` and `with_read_timeout` now share one `build_client`, and
  that sharing is load-bearing rather than tidy. Written as two separate builder
  chains, the test exercised only the injectable one and would have passed with
  the shipped path's bound removed entirely. The DRY violation and the vacuous
  test were the same defect. The crate
  currently uses `.timeout(300s)` (whole request); root uses `.read_timeout(120s)`
  (gap between bytes). Swapping the consumer first would regress the guard root
  gained in `9f4debc3`. Do this before C1, not after.
- **B2.** Three-state query prefix on `RemoteEmbedder` — *derive* / *explicit* /
  *explicitly suppressed*. `Option<String>` cannot express the third, and on the
  default Q4 model suppressed is the correct state. This is ET-3's blocker; see
  ET-3 for the three-row regression table.
- **B3.** Typed connect error (`EmbedError::Connect { url }`) replacing the
  `"embed connect failed"` substring contract. See ET-5 — it must land with this
  phase, not after, or the contract crosses a crate boundary as a bare string
  with nothing making both sides' tests fail together.
- **B4.** Export `is_https_or_loopback` (and whatever else D-phase needs) as
  `pub`.

**Phase C — consumer migration.** ET-3 proper: swap root's dense leg to
`RemoteEmbedder`. Hold batch size at 8 so the change is behaviour-preserving.

**Phase D — delete duplicates.** ET-4. Audit each pair first; root is not
reliably the stale side (B1 is the counterexample). D1 root's
`is_https_or_loopback` (A1's tests prove equivalence), D2 root's
`transport.rs` + wire structs, D3 drop `reqwest`/`rustls` from the root manifest
and re-measure the crate delta.

**Phase E — independent, no dependency on A-D.** Can be picked up by any session
at any time: test isolation for `tools::memory::tests` (the unfixed half of
`docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`);
retrofit a lean-safe inert embedder to restore the 11 test items ET-2 gated;
`rendezvous_poll_for_test` dead under `--no-default-features --all-targets`
(pre-existing, `server.rs:988`); `init: true` on the llama.cpp compose services
and healthchecks that exercise `/v1/embeddings` rather than `/health`.

### Why this order and not the plan's

The plan sequences by stage number. That ordering puts the consumer swap (C)
before the crate is ready (B), and never mentions A at all — so the deletion in
D would land against an untested function with no way to show equivalence. The
order above is derived from what gates what, not from stage numbering.

## Template for new entries

```
## ET-N — <one-line title>

**Status:** open | in-progress | done | blocked | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened

Seeded ET-1..ET-6. Stage 0 confirmed complete against the ADR's *Consequences*
section; Stages 1–3 confirmed absent against `Cargo.toml:89`, `:92`, `:198`.
