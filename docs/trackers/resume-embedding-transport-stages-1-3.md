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
entry_high_water_ET: 10
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
**Valid:** invariant
**Rests on:** `a_lean_build_cannot_construct_a_non_lite_client` and
`lite_alone_forces_dense_only_and_vetoes_the_reranker` in
`src/retrieval/client.rs::selection_tests` — cited by name, not by line, because
this entry's own proof chain below has already drifted once.

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

> **Corrected 2026-08-30 — the heading above is false, and the caveat at the foot
> of this entry is the main event.** Drift is **bidirectional**. This entry's two
> samples were root-deficient; three instances found on 2026-08-30 are all
> **crate**-deficient, and in each the crate was shipping the hazard to every
> external consumer while one root-side check hid it from exactly one caller:
>
> | instance | guarded in | missing in |
> |---|---|---|
> | status-error hoisted above the collection bucket | root | **crate** |
> | dense-response arity check (a **panic**, not an error) | root | **crate** |
> | crypto-provider install — an eager **panic** in `ClientBuilder::build()`, at any scheme (`probe_ollama`) | root | **crate** |
>
> The first two surfaced during T6 and are archived under `docs/issues/archive/`;
> the third was found by `codescout-ae` enumerating call sites of
> `install_default_crypto_provider` and noticing which client-builder was absent
> — a **pairwise** audit nearly missed it, because `probe_ollama` has no root
> twin to be compared against.
>
> **The third row above was understated until 2026-08-30, and the correction is
> the point.** It read "crypto-provider install before a TLS handshake", which is
> what the bug file claimed and why it was filed **severity low** — seemingly
> needing an operator with a TLS Ollama host. `codescout-ae`'s reproduction
> inverted both halves: `reqwest` is built with `rustls-no-provider`, so
> `default_rustls_crypto_provider()` is a literal `panic!("No provider set")` that
> runs **eagerly inside `ClientBuilder::build()`**, before any request and
> **regardless of scheme**. So it *panics* rather than returning `Err`, which means
> the "Ollama is not reachable" branch the bug file blamed never executes and that
> message is never printed — and it fires on plain `http://localhost:11434`, the
> zero-configuration default. Not an operator edge case: **every external consumer
> calling `create_embedder("ollama:…")`, with no configuration at all.**
>
> That is `CLAUDE.md`'s *run the reproduction before reading the fix plan* rule
> paying out again — the filing was a hypothesis about the reproduction, and the
> reproduction moved both the mechanism and the severity.
>
> It also sharpens what this class *is*. Root's `main.rs:253` installs the provider
> at startup, so **codescout itself is shielded and the crate ships the defect to
> everyone else** — which is why no root-side test could ever have caught it. And
> root's `transport.rs` states the invariant as already holding at *"every
> construction site"*: true of root, false of the crate, in a comment that reads as
> a repo-wide guarantee. A doc-vs-code drift whose scope claim is the drift.
>
> Verified with a positive control in a separate process — two `tests/` files,
> because the install is process-global and nothing weaker isolates it. Same three
> probes with the provider installed: `Err(connection refused)`, `Ok`, `Ok`. The
> provider is the discriminator, not a harness artefact.
>
> So the sentence to carry forward is the one this entry files as a caveat:
> **audit each pair before deleting; root's copy is not always the stale one.**
> On a crate-deficient pair, deleting root's copy removes the only guarded
> version. The `read_timeout` row below was already the counterexample — it now
> has three more.
>
> T7 followed the caveat rather than the heading, which is why it was safe:
> `roots_loopback_guard_agrees_with_the_crates_on_every_case` pinned the two
> copies against each other across 16 urls *before* root's was deleted, so the
> deletion rested on measured agreement rather than on this entry's predicted
> direction.

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

**Status:** fixed-verified — **shipped 2026-08-30 at `6be58840`; closed 2026-09-02 by the verify-open sweep.** Both halves of the requested resolution landed, and more: `crates/codescout-embed/src/embedder.rs:42` defines `pub enum EmbedError` with `Connect { url, detail }`, `CONNECT_FAILED_MARKER` is a published `pub const` at `:19` exported from `lib.rs:24`, and — the part this entry said was missing — `semantic_search.rs:1158` constructs the **producer's real** `EmbedError::Connect` and drives it through the consumer's classifier (`the_crates_own_connect_error_routes_where_roots_does`), so the two sides now fail together across the crate boundary. The one-way dependency constraint holds: `codescout-embed` has no knowledge of `semantic_search.rs`.

**Why this sat open for 33 days, which is the sweep's actual finding:** the fixing commit's subject is `fix(embed): publish the connect-failure contract as a type (T4 / ET-5)` — **it names this entry**. So this is not the zombie-open CLAUDE.md predicts ("a fix shipping under a message that does not name the tracker entry"). It is the *stronger* case: naming the entry in the commit subject changes nothing, because nothing reads commit subjects back into ledger status. A citation is not a write.

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

> **The live board is `ET-9`.** This entry holds the *reasoning* — why this order,
> what gates what, why Phase D audits rather than deletes. `ET-9` holds the
> *state*: 15 numbered tasks, their blockers, and the three decisions only the
> operator can make. Strike rows there; revise the argument here. If they ever
> disagree, this entry is the one to re-derive from.

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
at any time. Status as of 2026-08-30:

- ~~test isolation for `tools::memory::tests`~~ **DONE.** The unfixed half of
  `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`.
  Took two rounds: round 1 added the embedder/store stubs to 5 fixtures that
  bypassed the documented helpers; round 2 (after a concurrent session
  independently built `Agent::code_search`/`set_code_search_for_test`) added
  that override too, since its `NoCodeSearch` default only auto-installs inside
  `test_ctx_with_project_raw`. Verified by reproduction (a wedge listener
  logging hits, not just wall-clock time): 0 connections after the fix, vs. 4
  and 2 before it. Gate green (fmt, clippy, `cargo test` 4810/0, `--no-default-features`).
  See `bug-fix-session-log:W-75` for the full account.
- ~~`init: true` on the llama.cpp compose services and healthchecks that
  exercise `/v1/embeddings` rather than `/health`~~ **DONE**, commit `9360be99`
  ("fix(compose): healthcheck the inference path, not /health (T13, T14)"),
  patch-id `47ca28a05d9e5b5fa962b4ba43b9b16d68b52a9d`. This is the same fix
  `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`'s
  own "Fix idea" pointed at — see that entry for the live re-verification
  (a real `POST /v1/embeddings` in 26ms).
- retrofit a lean-safe inert embedder to restore the 11 test items ET-2 gated —
  still open.
- `rendezvous_poll_for_test` dead under `--no-default-features --all-targets`
  (pre-existing, `server.rs:988`) — still open.

### Why this order and not the plan's

The plan sequences by stage number. That ordering puts the consumer swap (C)
before the crate is ready (B), and never mentions A at all — so the deletion in
D would land against an untested function with no way to show equivalence. The
order above is derived from what gates what, not from stage numbering.

## ET-9 — Task board — every outstanding item from this work stream, with its blocker

**Observed:** 2026-08-29. The live board. `ET-8` holds the *reasoning* (why this
order, what gates what); this holds the *state*. Strike rows here as they land;
do not restate the rationale.

**Status:** open — **T1 through T8 are all closed.** What remains is T9 and its
precondition T16, and both are **discretionary rather than queued**: they buy
manifest honesty and **zero crates**, because root and the crate declare the same
`reqwest` and cargo unifies them. Verified at the bytes 2026-08-30, not inferred.
So the consolidation's measurable payoff is fully banked (`ET-2`), its two measured
drifts are closed (T1/T2/T5), and the duplication that produced them is gone (T6).

**A reader picking this up should decide whether to continue at all**, and the
honest input to that decision is `ET-4`'s corrected heading: drift is
bidirectional, and three of the five known instances had the crate as the
deficient side. That is an argument for auditing the remaining pairs — which
`codescout-ae` did on 2026-08-30, finding them clean plus one non-pair instance
(`probe_ollama`, `BL-66`) — rather than for grinding out T9.

**That bug is now fixed, and its filing was wrong in a way worth keeping** —
`docs/issues/archive/2026-08-30-sparse-status-errors-never-match-their-classifier-arm.md`,
fixed in `5dfa5051`, patch-id `189e55a8656e357ea80186eeeb372de277a1b08e`.

The wording mismatch was real; the impact claim on this board ("every sparse HTTP
failure is routed to the generic Qdrant bucket") was **not**.
`classify_search_error` is reached from `semantic_search.rs` only, and search embeds
its query through `EmbedderHttp::embed`, whose wording *did* match — the mismatched
producer was `embed_one_batch`, on the indexing path, which never reaches that
classifier. Filed from one producer grepped against one consumer, without asking
which producer the consumer sees.

What was actually broken on the path the classifier *does* see is worse and was not
filed at all: `error_for_status()` discarded the sparse server's response body, so a
422 read `422 Unprocessable Entity` instead of `batch size 40 > maximum allowed 32`.
The identical defect had been fixed for the **dense** leg in the same function
(`2026-08-26-dense-embedder-slot-context-drops-large-embeds`), and the sparse leg
beside it was left — a fourth instance of the one-side-of-a-pair shape `ET-4` now
records.

**Both stale rows are now struck by their owner**, resolving the hand-off the
previous session left here — and there were **two**, not one. That session
verified T5 at the bytes, correctly declined to strike another lane's row, and
recorded the evidence so it could not be lost. That protocol worked and is worth
keeping. What it did not catch is that **T4 was stale by the same mechanism**
(landed `6be58840`, row still reading *ready now*), because the check was aimed
at one row rather than swept across the table.

The generalisation, since this is now two instances in one table: a row's *State*
column is a claim about the past that nothing re-verifies. Sweep the whole
**ready now** column against `git log` when picking work up, not just the row you
came for — a stale row costs a session picking up finished work, which is the one
failure this board exists to prevent.

**Valid:** dated 2026-08-30

### Sequenced — embedding transport (order derived in `ET-8`)

| # | Phase | Task | State | Waits on |
|---|---|---|---|---|
| ~~T1~~ | A | Port the 11 host-spoofing assertions to root's `is_https_or_loopback` | **done** `28bb6e8a` | — |
| ~~T2~~ | B1 | Port `read_timeout` into `RemoteEmbedder::http_client` | **done** `ffdf1b09` | — |
| ~~T3~~ | B2 | Three-state query prefix on `RemoteEmbedder` — *derive* / *explicit* / *suppressed* | **done** `64c65248`, patch-id `72781e6b15e10b4edb56e8af39773db230555b5f`. `QueryPrefix` enum + `with_query_prefix`; constructors keep `Derive` so nothing observable changed yet. Two mutations run: `Suppressed` falling through to `derive_for` dies (the failure prints the offending wire body — literally the 34-point config), and `embed_query` ignoring the policy dies. **The pure resolver unit test survived the second mutation** — policy and its use are separate claims | — |
| ~~T4~~ | B3 | Typed `EmbedError::Connect { url }` replacing the `"embed connect failed"` substring contract (`ET-5`) | **done** `6be58840`, patch-id `07a8ea7c676e197bc862da3639bf63c19787d248`. Closes `ET-5`. Reproducing first showed the risk was **already realised**: `RemoteEmbedder` surfaced connect failures as reqwest's own `error sending request for url (…)`, which matched no arm of `classify_search_error` and fell through to the generic Qdrant fallback — live on the `ollama:`/`openai:` resolver path. Mutating `CONNECT_FAILED_MARKER` now fails root's two tests while the crate's pass, which is ET-5's "nothing makes the two fail together" turned false | — |
| ~~T5~~ | B4 | Export `is_https_or_loopback` (and whatever else Phase D needs) as `pub` | **done** `16dc28a5`, patch-id `dcf74187a08c2cf01399481c8a201f31a0ec2196`. `QueryPrefix::derive_for` shipped `pub` with T3. **`http_client` is NOT needed** — `ET-8`'s table assumed root's `transport.rs` dies in Phase D, but `reranker.rs` uses it at `:67`, `:83`, `:99` and the reranker is outside this consolidation, so **T8 should be re-scoped to the wire structs only**. Carries a differential pinning root's copy against the crate's on 16 urls, six named by neither fixed-expectation test; delete it with root's copy in T7 | — |
| ~~T6~~ | C | Swap root's dense leg to `RemoteEmbedder` | **done** in four commits — A `8097c2d6` / `18922aa3cc9f4be601e26f53ee68c9c483fec01b`, B `4fd4e5f4` / `d377f8ab6086f9d7137b4f4fc10d4628a26aa01c`, C `f9a205a9` / `469480c898f8db59c3a2e49acd12b628d9d824af`, D `797dd023` / `095ae63248a236e74a2135f101fa416cffb643dc`. **The seam is `dense_batch`, not the call site** — `RemoteEmbedder` is dense-only and root's two legs are fused per sub-batch, so a call-site swap reaches only the lite stack (`ET-10`). Root keeps chunking, both escape hatches, the concurrent sparse leg and the positional alignment; the crate owns the wire. D1's mapping landed **load-bearing**: `dense_query` prefixes via the crate's `embed_query`, so omitting `with_query_prefix` kills a test — root-side concatenation would have left that setting unfalsifiable. Root's empty predicate moved to `trim().is_empty()` to match the crate's, without which a whitespace-only chunk becomes a hard arity error. Two defects fixed en route, both the same shape (a hazard handled on root's side of the pair and never on the crate's): the collection-bucket hijack, and an index-out-of-bounds **panic** on a truncating server | — |
| ~~T7~~ | D1 | Delete root's `is_https_or_loopback` | **done** `c24d2d60`, patch-id `d7e1f42fa6e68e0922a0197cdd234694f034364f`. Both copies and both callers were already `remote-embed`-gated, so the lean build was never at risk. *(Corrected: an earlier draft of this row blamed “`ET-2`'s design table” for calling the function ungated. That claim was the **plan's**, and `ET-7` item 6 had already overturned it in writing — “`is_https_or_loopback` does NOT stay ungated. The plan says it does.” The ledger was right before I got here; I cited the stale source and credited it to the ledger.)* **Diverged from this row's "re-point T1's test, do not delete it"**: re-pointed, it was a literal duplicate of the crate's `is_https_or_loopback_matches_host_exactly`, same ten assertions on the same function. Replaced instead with `guarded_api_key_drops_the_key_for_a_host_that_only_looks_like_loopback`, which runs those inputs against **root's own behaviour** — a different claim, and one root had no coverage for at all (all five existing guard tests use a plainly non-loopback host). Mutation: a plausible `contains("localhost")` guard leaves all four old tests **green** and kills only the new one | — |
| ~~T8~~ | D2 | Delete the duplicated **dense wire structs** — NOT `transport.rs`, which `reranker.rs` keeps alive (`ET-5`) | **already done** by T6 step D (`797dd023`): `OpenAiEmbedReq` / `OpenAiEmbedResp` / `OpenAiEmbedItem` deleted, confirmed dead by the compiler rather than by inspection. `EmbedReq` + `SparseEntry` stay — those are the sparse wire | — |
| T9 | D3 | Drop `reqwest` / `rustls` from the **root** manifest | **BLOCKED, verified at the bytes 2026-08-30** — and worth **zero crates** even once unblocked. Four live uses remain: `EmbedderHttp.client` (the sparse leg + the `/info` probe), `transport.rs::client` (the shared builder), `RerankerHttp.client`, and `lib.rs`'s `rustls::crypto::ring` install. Root's `reqwest` sits under `remote-embed` (`Cargo.toml:89`), and `EmbedderHttp` — which owns sparse — is `remote-embed`-gated, so the dep cannot move until sparse does. **And the payoff is not crates**: the crate declares the same `reqwest` under its own `remote-embed` and cargo unifies them, so every configuration compiles it either way. The −48 was measured on the *bare* lean build and `ET-2` already banked it. Pursue this for manifest honesty or not at all | T16 |
| T16 | — | Gate root's **sparse leg** on `server-stack` rather than `remote-embed` — the precondition T9 actually has | **open, unscoped.** Sparse is only reachable when a sparse server is configured, which is the server-stack deployment; `dense_only` is true for every other. Moving it (with `resolve_batch_size`'s `/info` probe) would leave `EmbedderHttp` a thin orchestrator over `RemoteEmbedder` with no `reqwest` of its own, and `dep:reqwest`/`dep:rustls` could then sit under `server-stack` alone. Read `ET-10` finding 3 and T9's row before starting: this buys manifest honesty, not compile time | — |

Phase D is *audit each pair, then delete* — not delete-root's-copy. `ET-4` has the
counterexample where root was ahead of the crate.

### Free — no dependency on the sequence above

| # | Task | Why it matters | Pointer |
|---|---|---|---|
| ~~T10~~ | Stop `tools::memory::tests` resolving the retrieval stack from ambient config | **done** `fd638c76`, patch-id `2afe9f1378e8dece47fea600ecf840c57a215ab0`. **Co-authored** — the five-fixture isolation is session `2f584bf5`'s (`bug-fix-session-log:W-75`); the `CodeChunkSearch` trait, `Agent::code_search` seam, call site and regression test are mine. **This row understated the task**: the coupling was not "the embedder" but the whole `RetrievalClient` — `create_semantic_anchors` embeds through the seam and then searches code through a client *no seam covered*, so stubbing both documented seams still left every `write` test talking to the developer's live stack. Measured on libtest's clock: 0.88s live / 20.35s wedged before, 0.04–0.08s across live / wedged / refused / no-config after. Spread 19.47s → 0.04s. Regression test mutation-checked: restoring `RetrievalClient::from_env` at the call site turns it red | `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`, `unverified:` field |
| ~~T11~~ | Retrofit a lean-safe inert embedder; restore the test items `ET-2` gated on `remote-embed` | **done** `4b7cd31e`, patch-id `d26a28fcd9acb8013822071098746fbbf8796123`. The fake is `UnknownDimEmbedder` (`known_dim() -> None`), duplicated per-module beside `FixedDimEmbedder`. **`ET-7`'s refusal was right for a sharper reason than it states**: `effective_model_dim` is `known_dim().or(config.model_dim).unwrap_or(fallback)`, so a `Some(_)` embedder **shadows the operator's pin** — `FixedDimEmbedder` would not merely break the fallback test, it would silently neuter `client_with_store`'s two callers, which pass `Some(model_dim)` expecting it consulted. Measured: lean `retrieval::sync` 33→39, `retrieval::search` 5→8, +9 total and nothing else moved — lean now equals default. Mutation-checked ×3, not merely green: `.unwrap_or(fallback)`→`.unwrap_or(0)` kills the fallback test (0 vs 999); `known_dim` `None`→`Some(384)` kills it in search.rs (384 vs 999) and kills two sync.rs tests (`Some((999, 384))` vs `Some((999, 3))`), so **both duplicated copies are protected**. **Honest limit:** under that mutation only ONE of three search.rs tests died — the other two stay green for the wrong reason (with `known_dim` shadowing at 384 their stored dims still mismatch, so the guard still fires), so they do not discriminate "pin consulted" from "embedder answered". Pre-existing, but correctness rests on one test | `ET-7` |

**Two corrections to `ET-7`'s T11 inventory, both measured 2026-08-30.**
`test_retrieval_client` has **six** callers, not the five recorded — `references`
finds 3186/3290/3335/3389/3429/3514, where the original count was grep-derived.
And `tests/retrieval_integration.rs` is **correctly gated forever**, not retrofit
debt: every test there drives `EmbedderHttp` against a mockito server, so they
test the HTTP transport itself. It was left alone. Zero test items remain
`remote-embed`-gated in `sync.rs`/`search.rs`; every surviving gate there is
structural (`rerank_or_passthrough`'s paired definition, the `RerankerHttp`
import, three `reranker:` field initialisers).

**Found while executing T11, and now fixed by others:** `2c6f2677` had left
CI's `no-features` **test** lane red since 2026-08-29 — three ungated tests
asserting a call a lean build cannot satisfy. Fixed `f3dbfdf4`, patch-id
`ff3ffccb37031041c8f9ed64cb2baaa35ef2004a`; record archived at
`docs/issues/archive/2026-08-30-et2-gating-left-the-lean-test-lane-red.md`
(`6cc63559`). It is the **third** instance of the `ET-7`/T12 family and the
first T12's `--all-targets` remedy cannot reach, because compiling is not
running. CLAUDE.md's gate now says `cargo test --workspace --no-default-features`
rather than `cargo check` (`6764eb18`) — so the lean **run** is a documented
gate from here on, and this board's future rows should be verified against it.
| ~~T12~~ | `rendezvous_poll_for_test` was dead under `--no-default-features --all-targets` | **done** `141b69a3` — gate is now `all(test, feature = "librarian")`, matching its three callers in `guide_hint_tests`. Verified both ways: too wide leaves the lean warning, too narrow drops the 44 tests. Was hidden because CLAUDE.md's lean gate omits `--all-targets`, so no routine command builds lean *test* targets | `server.rs:988` |
| ~~T13~~ | `init: true` on the three GPU compose services | **done** `9360be99` — as hygiene, **not** as F-2's remedy. Docker's kill path does name `--init` on a Z-state PID, but that zombie held 394 MiB of VRAM and a reaped process holds none, so the driver's fd release was stuck and tini's `wait(2)` would have blocked in the same place. F-2's fix idea #1 claims more than its own evidence supports | `embedder-stack-ops-session-log:F-2` |
| ~~T14~~ | Healthchecks probe each service's **own** inference endpoint | **done** `9360be99`. Not one shared path — the three servers speak three APIs (`/v1/embeddings`, `/v1/rerank`, `/embed_sparse`), so T14 as written was only literally right for dense-gpu. Measured on this host: `/health` 0.8 ms vs inference 23 ms, a 29x gap no forward pass fits in | `embedder-stack-ops-session-log:F-2` |
| ~~T15~~ | `edit_markdown`'s frontmatter write never touches the catalog | **DONE by `codescout-ae`, not by me** — `518549d6`, patch-id `c424f89f8aeb67eaa692eeda4a9812a13820041c`, closed as `open-issue-work-queue:BL-48`. Chain is four links and **three** are covered: (2) `edit_markdown` calls the hook when frontmatter changed and (3) the call reaches the installed slot, both pinned by `tests/edit_markdown_catalog_sync.rs` — one integration binary on purpose, since the slot is process-wide and the unit-test binary contests it; (4) the syncer moves the row, checked against a real `Catalog`. All three mutation-confirmed, not merely green. **Residual, measured not assumed:** link (1), `server.rs:374` installing the syncer, is covered by nothing — deleting the install line leaves all 8 tests passing. Same gap exists for `librarian_guard`'s own install, so it is a shared shape rather than this change's defect | `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` (id `013458f0acdb88b8`, re-minted from `92d619d7a115617b` by the archive move), `open-issue-work-queue:BL-48` |

T13 and T14 landed together in `9360be99` (patch-id
`47ca28a05d9e5b5fa962b4ba43b9b16d68b52a9d`). Verified by mutation rather than by
a green run: each resolved command ran inside its live container (exit 0), then
against a wrong path and an unreachable port (all six mutants non-zero), so none
of the three can pass vacuously.

**Not applied to the running containers.** `healthcheck` and `init` are
creation-time properties, so they need a `docker compose --profile gpu up -d`
recreate — deliberately left to the operator, the stack having been restored by
reboot only the day before. Note also that `restart: unless-stopped` does **not**
react to health: the gain is that `docker ps` says `(unhealthy)` within ~150 s
instead of never, not that the container heals itself.

### Decisions only the operator can make

| # | Question | Blocks |
|---|---|---|
| ~~D1~~ | **ANSWERED 2026-08-30 — suppressed**, with `Derive` kept as an explicit opt-in rather than a default. Upholds the ADR § *The three contracts*. The deciding evidence: `derive_for` matches on `coderank` but is **quantization-blind**, and quantization is the axis that decides — `CodeRankEmbed-Q4_K_M.gguf` carries the answer in the same string it matches on and matches the wrong half, so deriving yields the 34-point config on the champion 37-point deployment | unblocked T3, and with it T4–T9 |
| D2 | `~/.claude/settings.json` sets the prefix while the running model is `CodeRankEmbed-Q4_K_M`, for which the repo's `.env` records "NO query prefix (37, champion) … forcing the prefix drops to 34". Remove it to match, or was it deliberate? | nothing — separable from D1 |
| D3 | 28 unpushed commits on `experiments`. Push? | nothing |

D1 is answered, so **nothing on this board is blocked**. D2 remains a live
retrieval-quality question and is now the sharper of the two: the running shell
sets the prefix while serving Q4_K_M, which is the 34-point configuration rather
than the 37-point champion. Post-fix, documents are stored unprefixed either way
(`docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`), so
flipping it moves only the query side — no re-index required, which makes it
cheap to test both ways.

### Resume — T6, cold

> **HISTORICAL — T6 shipped 2026-08-30.** Kept because its corrections (a)/(b)/(c)
> and the scout handover below are the reasoning the swap rested on, and because
> its opening sentence was wrong in an instructive way. For what is actually left,
> read *Resume — what is left* at the end of this entry.

Everything T6 needs now exists; it is a consumer swap, not a design task.

**Do first, before reading further:** sweep the *State* column against `git log`.
Two rows on this board were stale simultaneously (see the Status block). The
board is a claim about the past.

**What T6 is.** Replace root's `EmbedderHttp` dense leg with
`codescout_embed::remote::RemoteEmbedder` in
`RetrievalClient::build_http_embedder` / `build_embedder_for_url`
(`src/retrieval/client.rs:231`, `:257`).

> **Three of this block's premises were checked at the bytes on 2026-08-30 and
> two are false. Read the corrections below before designing the swap** — they
> remove one decision entirely and replace another. This is `ET-8`'s own "a
> plan's reference code is a sketch" arriving on this entry.
>
> **(a) "Hold batch size at 8" does not name root's dense batch.** Root has no
> fixed 8. `EmbedderHttp::resolve_batch_size` (`src/retrieval/embedder.rs:756`)
> *discovers* the cap from the **sparse** server's `/info`
> (`max_client_batch_size`), with `const FALLBACK: usize = 8` only when that
> server does not answer — and the **dense-only** path never probes it at all,
> using **32** (`embed_batch`, `:806-809`). `RemoteEmbedder`'s hardcoded
> `BATCH_SIZE = 32` (`crates/codescout-embed/src/remote.rs:384`) therefore
> already equals root's dense-only batch. The 8 is a *sparse*-derived fallback,
> and `resolve_batch_size`'s own doc records that the prior `const BATCH = 8`
> "was justified by a comment citing a cap that only `sparse-amd` ever imposed,
> and it silently survived that service's removal". Holding the dense leg at 8
> would not preserve behaviour — it would *change* it.
>
> **(b) "`RemoteEmbedder` requires a model" is false, so contract 3's decision
> evaporates.** `from_url` stores `model.to_string()` with no validation
> (`remote.rs:331-350`) and `embed` serializes `model: &self.model` straight onto
> the wire (`:400-405`). An empty model yields `{"model": "", "input": […]}` —
> **byte-identical to what root sends today**. `ET-3`'s "decide whether the
> crate's required-model contract is adopted (preferred)" describes a contract
> that does not exist anywhere in the crate. The swap is model-neutral; adopting
> a required-model rule is separate, optional hardening that would mean *adding*
> validation, not inheriting it — and it is operator-facing, since a url-set /
> model-unset deployment works today.
>
> **(c) A real contract this block does not name: the swap ADDS retry to a
> dense leg that has none.** Root's `EmbedderHttp::dense_batch`
> (`src/retrieval/embedder.rs:441`) issues a single `req.send().await` — no
> attempt counter, no backoff. A dead dense endpoint fails on the first try
> today. `RemoteEmbedder::embed` carries `MAX_RETRIES = 3` with a 500 ms doubling
> backoff (`remote.rs:384-395`), so the swap introduces ~1.5 s of retry per
> sub-batch where there was none.
>
> *Corrected 2026-08-30, same day, by the session that wrote this block's first
> version: it said "the retry ladder **doubles**", which is false. Root's
> 8-attempt `100ms * 2^min(attempt,6)` ladder is on the **sparse** leg only —
> every match for `retry` in `embedder.rs` is `/embed_sparse` — and the sparse
> leg stays in root, untouched by this swap. "Doubles" implies removing
> redundancy; the true finding asks whether the dense leg should retry **at
> all**, which is a different question with a different answer.*
>
> And this project has a prior on it, which is why the question is not
> rhetorical: the sparse ladder's cap is documented as "the only thing standing
> between a sparse server stuck returning a retryable status and an unbounded
> retry — and since Stage 1, that loop now runs while the per-project index lock
> is held for the whole `sync_project` pass: unbounded retry there means the lock
> never releases, wedging every subsequent index for that project"
> (`embedder.rs:1481-1487`). Dense retry would run under that same lock.
>
> **DECIDED 2026-08-30 by the operator: the dense leg stays fail-fast**, and the
> knob that makes it expressible has landed — `RemoteEmbedder::with_max_attempts`,
> `3587d823`, patch-id `1fa302e86eaf1476d6cda752ec64f6996b3229b2`. The swap must
> call `.with_max_attempts(1)` at the construction site. Default stays 3, so no
> existing caller moved.
>
> Two things about it the implementer should not have to rediscover. It is
> **attempts, not retries**: the loop was always `0..n` and the error text always
> said "attempts", so only the old `MAX_RETRIES` constant was misnamed — a
> `with_max_retries` would have carried that off-by-one to every call site. And
> its tests **count requests on the wire**, not the field, because a test
> asserting `max_attempts == 1` passes for an `embed` that stores the value and
> ignores it; mutating the loop to a hardcoded 3 kills three of them, each
> reporting `left: 3` as observed by the server.
>
> The knob is **neutral to `ET-10`'s A/B fork**. Whichever way that resolves —
> split on `dense_only`, or extract the sparse leg first — both branches need a
> fail-fast dense embedder, and neither is preempted by it.
>
> Contract 1 (query prefix) is unaffected and still exactly right — confirmed at
> the bytes: both `from_url` and `custom` hardcode
> `query_prefix: QueryPrefix::Derive`, so omitting `.with_query_prefix(…)` is the
> silent 37→34 regression this block warns about.
>
> The same wrong 8 appears a third time, in
> `remote.rs::tests::ollama_large_batch_exceeding_batch_size` ("BATCH_SIZE is 8;
> send 20 texts to exercise the chunking logic"). That test has been vacuous
> since it was written — 20 < 32 is one chunk — and `git log -S` finds no commit
> where the constant was ever 8. **FIXED 2026-08-30** (`236f31a4`, patch-id
> `7f066d41f254df6428d99ae17908e031f9d8c95a`): replaced with a CI-running loopback
> test asserting the split's shape, `[32, 32, 6]` for 70 inputs. Archived as
> `docs/issues/archive/2026-08-30-ollama-large-batch-test-never-exceeded-the-batch-size.md`.

**The three contracts it must carry across, all now built:**

1. **Query prefix (D1's root-side half).** Map unset `CODESCOUT_QUERY_PREFIX` →
   `QueryPrefix::Suppressed`, a set value → `QueryPrefix::Explicit(v)`. Do **not**
   let it fall to `Derive` — `RemoteEmbedder`'s constructors default to `Derive`,
   so this is an explicit `.with_query_prefix(…)` at the construction site and
   forgetting it is a silent 37→34 regression on the default model. Expose
   `Derive` as an opt-in sentinel if you want zero-config; that was the ruling.
2. **Connect errors.** Already typed — `EmbedError::Connect` renders
   `CONNECT_FAILED_MARKER`, and `classify_search_error` matches the constant. The
   swap should need no change here; `the_crates_own_connect_error_routes_where_roots_does`
   is the test that will tell you if it does.
3. **Model name.** Root's `dense_model_name` defaults to the **empty string**, so
   it sends `{"model": ""}` today — tolerated by llama-server, rejected by stricter
   gateways. `RemoteEmbedder` requires a model. Decide during the swap, not after
   (`ET-3`).

**The asymmetry to watch.** Root's `EmbedderHttp` fetches dense **and sparse**;
`RemoteEmbedder` is dense-only. The sparse leg and its retry ladder stay in root.
Do not delete them with the dense duplicate.

**After T6, Phase D (T7–T9) is deletion**, and two of its three rows already carry
corrections: T7 must **re-point** `roots_loopback_guard_agrees_with_the_crates_on_every_case`
and delete it with root's copy — it exists to make that deletion checkable and
outliving it is worse than useless; T8 is **mis-scoped** and should narrow to the
wire structs, since `reranker.rs` keeps `transport.rs` alive.

**Verify with a lean RUN**, not a check: `cargo test --workspace
--no-default-features` is a documented gate as of `6764eb18`, and `2c6f2677`
showed a check cannot see a lean runtime failure.

#### Handover — five things a scout already checked at the bytes (2026-08-30)

Recorded by the session that built the retry knob and then stood down from T6, so
the implementing session spends its budget on `ET-10`'s A/B fork rather than
re-deriving these. **Four diverge and need a decision; one is a parity already
confirmed — do not re-check that one.**

(It was written as three-and-two. DIVERGES 4 was drafted as a confirmed parity and
demoted before this was committed, after a concurrent session filed the bug that
contradicts it. Its footnote keeps the reasoning that produced the wrong call,
because the shape recurs.)

**DIVERGES 1 — the api-key cleartext policy is drop-and-warn in root and `bail!`
in the crate.** Both are safe (neither leaks a key), but the outcomes differ and
the difference is operator-visible.

| | plaintext non-loopback url + api_key set |
|---|---|
| root, `EmbedderHttp::new` (`EMBED_API_KEY`) | `tracing::warn!`, drop the key, **carry on unauthenticated** |
| root, `RetrievalClient::guarded_api_key` (`[embeddings].api_key`) | same — warn, drop, carry on |
| crate, `RemoteEmbedder::from_url` / `custom` | **`bail!`** — construction fails |

So an operator with `EMBED_API_KEY` set against a plaintext internal gateway
starts codescout today (and gets a 401 from the server, or not); after a naive
swap `build_embedder` returns `Err` and the retrieval client never constructs at
all. Whichever policy wins, it is a **choice**, not an inheritance.

Beware one doc line while deciding: root's `is_https_or_loopback` says it "Mirrors
the codescout-embed `RemoteEmbedder` guard". That is true of the **predicate** and
false of the **policy** — and since T7 deletes root's predicate, the policy
difference survives that deletion unnoticed unless it is decided here.

**DIVERGES 2 — a mutation-verified test binds root's policy to the real
construction path, and cannot survive unchanged.**
`build_http_embedder_never_sends_a_configured_key_over_plaintext_http`
(`src/retrieval/client.rs:919`) asserts `http.api_key_for_test() == None`, and its
own comment records the mutation that kills it. Under the crate's `bail!` policy
there is no constructed embedder to inspect, so the assertion has no object.
**Re-point it at whichever policy wins — do not delete it.** That is exactly the
treatment T7's row already prescribes for the loopback-guard test, and for the
same reason: it exists to make the change checkable, so outliving it is worse than
useless. Note it depends on `EmbedderHttp::api_key_for_test`, a `#[cfg(test)]`
accessor (`src/retrieval/embedder.rs:390`); fork branch A must keep an
inspectable construction, branch B must supply an equivalent seam.

**DIVERGES 3 — root resolves six ambient inputs the crate does not.**
`EmbedderHttp::new` reads `CODESCOUT_EMBEDDER_MODEL_NAME`,
`CODESCOUT_QUERY_PREFIX`, `EMBED_API_KEY`, `CODESCOUT_EMBED_BATCH`,
`CODESCOUT_EMBED_INFLIGHT`, and `transport::read_timeout_from_env()`.
`RemoteEmbedder` reads only the last of those. **`CODESCOUT_EMBED_BATCH` and
`CODESCOUT_EMBED_INFLIGHT` have no `RemoteEmbedder` equivalent at all** — the
batch size is the hardcoded `BATCH_SIZE = 32` and there is no inflight concept.
Either the swap keeps root's `embed_chunks_ordered` driving the crate per
sub-batch, or those two operator escape hatches silently stop working.

**PARITY 1, CONFIRMED — read timeout.** Same variable
(`CODESCOUT_HTTP_READ_TIMEOUT_SECS`), same default (**120s** on both sides), same
"zero or unparseable falls back rather than erroring" filter. T2 landed this and
it holds. Nothing to carry across.

**DIVERGES 4 — error bodies do NOT route the same, and this one nearly went into
this handover as a confirmed parity.** Both sides do put the response body in the
error text, and the **context-size** class specifically does still route: those
four patterns (`larger than the max context size`, `exceed_context_size`,
`input is too large`, `physical batch size`) are hoisted to the very front of
`classify_search_error`, so the archived
`2026-08-26-dense-embedder-slot-context-drops-large-embeds` class does not reopen.
That much holds.

But it does not generalise, and generalising it is the error. Root's producer
emits `dense openai status {code}: {body}` and **that arm was deliberately hoisted
above the collection bucket**, its comment naming the reason: the message "carries
the embedder's RESPONSE BODY, which is arbitrary remote text. A body containing
'not found' or 'Collection' would hijack the collection bucket." The crate's
producer emits `HTTP {status} from embedding server: {body}` and was **never given
the same protection** — so an ordinary embedder 404 whose body reads
`model 'coderank' not found` is classified as a **missing Qdrant collection**, and
the operator is told to re-index a collection that is fine.

The hazard was identified and fixed for one of two producers. It *was* live on the
`ollama:`/`openai:` resolver path, and the swap would have carried it onto the dense
leg. Filed by a concurrent session as
`docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md`;
its sibling,
`docs/issues/archive/2026-08-30-sparse-status-errors-never-match-their-classifier-arm.md`,
covers root's sparse leg — **also fixed 2026-08-30** (`5dfa5051`), and its filed
impact claim corrected in the same pass; see the ET-9 status block.

**Closed 2026-08-30 by T6 steps A and B** — `8097c2d6` (patch-id
`18922aa3cc9f4be601e26f53ee68c9c483fec01b`) publishes `EmbedError::Status` +
`STATUS_FAILED_MARKER`; `4fd4e5f4` (patch-id
`d377f8ab6086f9d7137b4f4fc10d4628a26aa01c`) matches the constant in root's hoisted
arm. The scout's correction below is what the fix rests on, and it holds: mutating
the arm away reproduces `Qdrant collection is missing` for an embedder 404, while
the pre-existing guard for *root's* producer passes unchanged — the two were
covered independently, which is exactly the shape "fixed for one of two producers"
predicts. The **sibling is still open**; branch B moves the sparse leg and will
meet it.

*Written first, by this same scout, as "PARITY 2, CONFIRMED — error bodies still
route", and corrected before it was committed. The reasoning was: the context-size
arms are hoisted, they match body text, therefore body-carrying errors route. True
of the arms I checked and false of the class I inferred — I generalised from the
one producer that had been fixed. The tell was available and not taken: root's
hoisting comment exists precisely because arbitrary bodies hijack buckets, so a
second producer with the same shape and no hoisted arm was the question, not the
reassurance.*

**Rests on:** `ET-8` for the ordering argument and `ET-4` for why Phase D audits
rather than deletes. If either is revised, re-derive this board rather than
patching rows.

### Resume — what is left (cold, 2026-08-30)

**This stream is done in every sense that was measurable.** T1–T8 are closed. What
remains is T9 and its precondition T16, and the honest summary is that a reader
should decide whether to continue *at all* rather than pick up the next row.

**Do first, as always:** sweep the *State* column against `git log`. Every SHA in
this entry was re-verified on 2026-08-30; that fact decays.

**The three reasons to stop here, each measured rather than argued:**

1. The **crate-count payoff is banked** (`ET-2`, bare lean build). T9 buys **zero**
   further crates — root and `codescout-embed` declare the same `reqwest` under
   their own `remote-embed`, and cargo unifies them, so no configuration stops
   compiling it.
2. The **measured drift is closed.** `ET-4`'s two instances were fixed by T1/T2/T5.
3. The **mechanism that produced them is gone.** T6 left one dense implementation.

**The one reason to continue, which is real:** `ET-4`'s heading was corrected on
2026-08-30 — drift is **bidirectional**, and three of the five known instances had
the *crate* as the deficient side. `codescout-ae` audited the remaining pairs and
found them clean, plus one non-pair instance (`probe_ollama`, `BL-66`) that a
pairwise sweep could not have found. So the audit is done and came back clean; that
is an argument for confidence, not for more deletion.

**If you do continue, T16 before T9** — gating root's sparse leg on `server-stack`
is the precondition T9 actually has, and T9 alone cannot be done. Read `ET-10`
finding 3 first.

**Three things not to re-derive:**

- The seam is `EmbedderHttp::dense_batch`, not `build_embedder_for_url`. The legs
  are fused per sub-batch; they are not composable at the `CodeEmbedder` boundary.
- `classify_search_error` is reached from `semantic_search.rs` **only**. Anything
  about indexing-path error wording that cites it is wrong — that mistake is on
  record in the archived sparse bug file.
- Root and the crate now disagree about nothing on the dense path, and four
  markers (`CONNECT_FAILED_MARKER`, `STATUS_FAILED_MARKER`, `SPARSE_MARKER`,
  `SPARSE_STATUS_MARKER`) are the contract. Match the constants, never a literal.

**Open, and not this stream's to close:**

- **D2** — the running shell sets `CODESCOUT_QUERY_PREFIX` while serving
  `CodeRankEmbed-Q4_K_M`, which is the 34-point configuration rather than the
  37-point champion. Flipping it moves only the query side, so no re-index is
  needed and it is cheap to test both ways. Operator decision, still live.
- **D3** — unpushed commits on `experiments`. Operator decision.
- **`reconnaissance-patterns:R-129`** — its Promote-when **FIRED** on 2026-08-30
  and is deliberately unapplied, pending an operator call between a CLAUDE.md rule
  and per-session worktrees. This stream produced the instance that fired it.

  The two options are not equivalent in cost, and the difference is measurable
  rather than a matter of taste. A CLAUDE.md rule is one edit and changes no
  workflow. Per-session worktrees would dissolve `R-90` and `R-129` together — the
  larger, more permanent fix — but `append_entry` **refuses id allocation from a
  worktree** by design (an entry id is ledger-wide state and must key to the main
  tracker), so every `F-N` / `W-N` / `R-N` / `BL-N` append becomes write-locally,
  then fold in from the main checkout after the merge. That is a real recurring
  tax on the surface this project uses most, and it is the number the decision
  turns on. Datapoint from `codescout-ae`, who priced it while I was arguing the
  two framings without it.

**Valid:** dated 2026-08-30

### The criterion both of this stream's test fixes were verified under

Added 2026-08-30. Two test-integrity fixes came out of this work stream —
`614b1271` (the concurrency guard) and `236f31a4` (the chunking guard). Both were
mutation-verified, and the rule that makes that verification *mean* something was
developed with `codescout-ae` the same afternoon and lives in their log:
**`bug-fix-session-log:W-84`**. Cited rather than restated, so there is one
definition.

The short form, because a citation nobody follows is not a link:

> A test cannot detect a change its assertion is monotone under.

Absence assertions are monotone under **removal** — kill the mechanism and the
silence still holds. Existence assertions (`contains`, `> 0`, `is_some`) are
monotone under **widening** — return a superset and the predicate still holds.
Equality against an exact expected shape is monotone under **nothing**.

Two consequences this stream actually depended on:

1. **"Did the test die?" is the wrong question.** *Did it die on the assertion
   under test?* is the right one. `all_empty_chunk_sends_no_requests_at_all` dies
   under a removal mutation — via a panic at `.expect("embed_one_batch")`, which
   says nothing about whether its `.expect(0)` has any power. It dies under the
   forbidden-act mutation with `Expected 0 request(s)… but received 1`. Both red,
   one informative.
2. **The fixture has to discriminate.** The property is only observable when the
   correct and mutated behaviours *disagree*; a fixture that does not separate them
   cannot detect the mutation however strict the assertion is.

**Both this stream's fixes pass the criterion, and by habit rather than by
argument** — worth recording as a near miss rather than a win. They assert
`met == 2` and `requests == [32, 32, 6]`. Had they been written `met > 0` and
`requests > 1` — both entirely natural phrasings — each would have been monotone in
exactly the direction its mutation moves, and each would have survived it while
looking guarded.
#### And one of the two is not run by the documented gate at all

Measured 2026-08-30, after `codescout-ae` reported the same for a file of theirs.
Filter `an_oversize_batch_is_split_into_batch_size_requests`:

| lane | result |
|---|---|
| `cargo test` — **gate command 3** | `0 passed` (root package targets only) |
| `cargo test --workspace --no-default-features` — **gate command 4** | `0 passed` (`remote-embed` off → compiles to nothing) |
| `cargo test --workspace` — **not in the gate** | **`1 passed`** |

So `236f31a4`'s chunking test is covered by **CI's `default` matrix lane and by no
command in `CLAUDE.md`'s four-command gate.** The commit's "gate: cargo test 4877/0,
lean 3385/0" is accurate about what was run and misleading about what it proves — the
mutation verification stands on its own (the test was run explicitly with
`--features remote-embed` and killed by `chunks(usize::MAX)`), but a **future**
regression in the chunking loop is locally invisible.

Worth naming plainly, because it is the same shape as the defect that commit fixed:
the old test was concealed by `#[ignore]` **and** a wrong premise. The new one is
CI-covered but locally invisible. Strictly better, and still not what the commit
message implies.

**This is a fourth blind spot in the gate**, alongside the three `CLAUDE.md` already
documents (bare clippy missing test targets; an ungated module reaching a gated one;
`check` not being `test`). The pattern it adds: **a workspace member's
feature-gated tests are invisible to both test commands** — command 3 does not
build the member, command 4 builds it with the feature off.

**Sized 2026-08-30, via `-- --list` so nothing is executed:**

| | tests |
|---|---|
| `-p codescout-embed --features remote-embed` | **56** |
| `-p codescout-embed --no-default-features` (what gate 4 reaches) | 19 |
| **run by neither test gate command** | **37** |

`codescout-ae` measured **33** (52 vs 19). I first explained the gap as "the tree
gained tests between us" — **wrong, and invented rather than checked.** They re-ran
and got the same 52; the tree had not changed. The gap is **passed-vs-total**,
verified here: `51 passed; 4 ignored` in the lib plus 1 integration test. The four
are the `ollama_*` family, `#[ignore]`d pending a live server.

So both numbers are right about different questions, and the entry keeps both:

| number | question it answers |
|---|---|
| **37** | tests not compiled by either gate command |
| **33** | live guards actually lost (the other 4 are `#[ignore]`d, so not coverage either way) |

**The comfortable explanation is the finding.** "The tree changed between us" costs
nothing to say, retires the discrepancy, and teaches nothing — and it would have
buried the fact that an `#[ignore]`d test was padding a coverage count. That is the
same `#[ignore]` that concealed the vacuity in
`ollama_large_batch_exceeding_batch_size` this morning: first it hid a test that
could not fail, now it inflates a number measuring what is guarded. **A discrepancy
reconciled by a plausible story nobody checked is worse than an open one**, because
the open one still has someone looking at it.

**The 33 include this session's own regression guards** —
`an_oversize_batch_is_split_into_batch_size_requests`,
`a_short_response_errors_instead_of_panicking`, and
`a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever` — the
tests written today to stop today's bugs recurring. That is the argument for a fifth
gate command, and it is an operator call rather than ours.

**Getting that number wrong first is itself the lesson.** The first attempt passed
`--list` before `--`, where cargo rejects it as an unknown argument; with stderr
suppressed it printed `0`, `0`, `0` and `exit_code 1`. Three clean zeros that ran
nothing, in the exact shape this entry already documents twice. The tell was the
exit code, which `2>/dev/null` had not hidden — and which a reader scanning the
numbers would not have looked at.

Not proposing a `CLAUDE.md` edit from here — that is the most load-bearing document
in the repo and the gate already runs ~20 s; adding a fifth command is an operator
call, not a drive-by. Recorded so the next person to add a crate test knows to run
`cargo test --workspace` explicitly.

**How `codescout-ae` nearly published the opposite**, worth carrying because the
error is in the *instrument*: their `ci.yml` grep matched only lines containing
`--features` or `--no-default-features`, which returned the two configs that have
them and silently dropped the third — whose flags are the **empty string**. They
almost reported "no gate lane runs it at all." A filter is a hypothesis about where
the thing lives, and an empty value is precisely what a pattern-based filter cannot
see. Third time in one day that a search's **scope**, not its pattern, was the thing
that was wrong (`include_str!` in the binary probe; hidden paths in the citation
sweep; this).
## ET-10 — T6 is a design task, not a consumer swap — and T9 is blocked by two surfaces outside this stream

**Status:** open
**Valid:** dated 2026-08-30

**Observed.** 2026-08-30, at the bytes, *before* writing any T6 code —
`ET-9`'s resume block says T6 "is a consumer swap, not a design task". Three
checks say otherwise. This is a fourth correction to that block, alongside its
own (a)/(b)/(c).

**1 — The swap's documented blocker is already gone, and the doc still asserts
it.** `build_embedder`'s doc comment (`src/retrieval/client.rs:303`) states that
routing through `EmbedderHttp` "keeps the connect-error marker
`src/tools/semantic/semantic_search.rs` matches on." T4 (`6be58840`) removed that
constraint: `EmbedError::Connect` renders `CONNECT_FAILED_MARKER`
(`crates/codescout-embed/src/embedder.rs:45`) and `classify_search_error` matches
the shared constant (`src/tools/semantic/semantic_search.rs:143`), so both
producers now land in the same bucket. The comment names the very reason T6 was
deferred, and it is now false. Correct it in the same commit as the swap —
otherwise the next reader re-derives a blocker that no longer exists.

**2 — The swap cannot reach the ordinary deployment.** `RemoteEmbedder` is
dense-only, and `CodeEmbedderAdapter::wrap` (`src/retrieval/embedder.rs:2099`)
returns an empty `SparseVector`, so *anything* routed through that adapter emits
no sparse leg. `build_embedder_for_url` is reached with
`dense_only = lite || config.disable_sparse || backend_is_local(config)`
(`client.rs:198`), and `disable_sparse` defaults **false** while
`sparse_embedder_url` carries a default (`src/retrieval/config.rs:210`, `:194`).
So on a normal server-stack deployment `dense_only == false` and `EmbedderHttp`
must stay. T6 as scoped reaches only the **lite** stack,
`CODESCOUT_DISABLE_SPARSE=1`, and a `local:` model paired with a url.

**3 — T9 is blocked by sparse and the reranker, neither of which is in this
stream.** Root's reqwest is declared under `remote-embed`
(`Cargo.toml:89`; `remote-embed = ["codescout-embed/remote-embed", "dep:reqwest",
"dep:rustls"]`), and `EmbedderHttp` — which *owns* the sparse leg — is
`remote-embed`-gated. `server-stack = ["dep:qdrant-client", "remote-embed"]`, and
that line's own comment states the reason in the manifest: "this stack's own
reranker leg is HTTP over reqwest". Dropping root's `reqwest`/`rustls` therefore
requires `EmbedderHttp` **deleted** and `RerankerHttp` **re-homed**; T6–T8
deliver neither. Note also that root's reqwest is already redundant in
crate-count terms — the crate pulls the same version under its own
`remote-embed` and cargo unifies — so **T9's payoff is manifest honesty, not
crates**. The −48 was measured on the *bare* lean build and `ET-2` already
banked it.

**Next.** Decide the fork before writing code:

- **(A) Split `build_embedder_for_url` on `dense_only`** — `RemoteEmbedder` when
  true, `EmbedderHttp` when false. Small and reviewable, but leaves **two dense
  implementations** live permanently (the duplication `ET-4` exists to remove)
  and makes dense behaviour differ by config: retry vs no-retry, and two
  different batch caps.
- **(B) Extract root's sparse leg into its own type first, then swap dense
  wholesale.** The only path that ends with one dense implementation and lets
  Phase D delete anything.

`ET-9`'s correction (a) lands squarely on this fork: the **combined** path
derives its batch cap from the sparse server's `/info`
(`resolve_batch_size`, `src/retrieval/embedder.rs:756`, `FALLBACK = 8`) while the
**dense-only** path never probes and uses 32. Splitting the legs means deciding
whether dense keeps honouring a cap the sparse server advertises.

**Resolved 2026-08-30 — branch B, at a seam neither branch named.** The operator
chose B. Executing it moved the seam again: root's dense and sparse legs are fused
*per sub-batch* (one batch size drives both, they run concurrently under
`try_join!`, outputs are positionally re-aligned), so "extract the sparse leg" is
not available either — the legs are not independently composable at the
`CodeEmbedder` level. The seam that **is** available is one function lower,
`EmbedderHttp::dense_batch`: root keeps the orchestration, the crate takes the
wire. Same outcome B wanted — one dense implementation — with `EmbedderHttp`
surviving as a hybrid orchestrator rather than being deleted.

Landed as A–D; SHAs and patch-ids on `ET-9`'s T6 row. Findings 1 and 2 of this
entry are discharged. **Finding 3 stands unchanged**: T9 is still blocked by the
sparse leg and the reranker, and T6 did not touch either.

One correction to this entry's own *Next*, worth keeping because the reasoning
recurs: it proposed "split on `dense_only`" and "extract the sparse leg" as the
fork, and both were framed at the `CodeEmbedder` boundary because that is where
`build_embedder_for_url` chooses. The choice point and the seam were different
places, and reading only the call site is what hid that.

**Rests on:** `ET-4` (why Phase D audits rather than deletes) and `ET-8` (the
ordering argument). Finding 3 contradicts `ET-8`'s implied terminal state, so
re-derive Phase D rather than patching T9's row.

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
