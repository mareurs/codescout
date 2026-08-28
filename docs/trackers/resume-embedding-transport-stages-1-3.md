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
entry_high_water_ET: 6
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

**Status:** open — the highest-value single piece in this stream
**Valid:** dated 2026-08-28

**This stage banks the entire 48-crate win on its own.** If Stages 2–3 never
happen, Stage 1 alone is still worth having.

**Measured payoff:** `--no-default-features` goes 274 → 226 crates (−48: the whole
`hyper`/`h2`/`tower`/`rustls`/`ring` stack). Two CI lanes
(`.github/workflows/ci.yml:46`, `:48`) stop compiling a TLS stack they never call.
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

**Status:** open — trailing cleanup, blocked on ET-3
**Valid:** dated 2026-08-28

Remove root's now-unused dense transport code, `reqwest`, `rustls`, and
`install_default_crypto_provider`.

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

**Status:** open — cheap, do it alongside ET-2
**Valid:** dated 2026-07-25

The 274 → 226 figure was measured 2026-07-25. The dependency tree has moved since
(`codescout-embed` gained the `local` ONNX module, among others). Quoting a
month-old crate delta in a PR description is exactly the decay this repo's
tracker discipline exists to catch.

**Next:** re-run `cargo tree --no-default-features` counts before and after ET-2
and record both here.

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
