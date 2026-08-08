---
kind: bug
status: fixed
title: 'BUG: check_has_index scrolls every chunk of a project with full payloads to answer "are there any?", so on a real corpus it exceeds its 2s budget, reports the project unindexed, and re-does the whole scroll on every activation'
tags:
- retrieval
- qdrant
- activation
- performance
- silent-wrong-answer
closed: 2026-08-08
opened: 2026-08-08
owner: marius
related:
- docs/issues/2026-08-08-server-stack-gated-tests-never-compiled-by-any-lane.md
severity: high
---

# BUG: the index-status probe enumerates the corpus to answer a yes/no question

## Summary

`check_has_index` asks one bit — does this project have **any** indexed chunks? It
answers it by calling `project_index_stats`, which scrolls **every chunk for the
project, 1000 at a time, with full payloads** (including `content`), to compute
`(chunk_count, file_count)`, and then compares `chunks > 0`.

The call is bounded by `FIRST_PROBE_TIMEOUT` = **2 seconds**. On any corpus large enough
to care about, the scroll cannot finish in that budget. The timeout is then handled
correctly-by-design — report `false`, do **not** cache — with the consequence that every
subsequent activation repeats the entire scroll and throws it away again.

Net effect on a real project: `index.status` says *not indexed* for a project with tens
of thousands of chunks indexed, the activation response surfaces a "build the index"
hint that is wrong, and each activation issues a full-corpus enumeration to produce that
wrong answer.

## Symptom (Effect)

`cargo test --features server-stack` (the shipped configuration — see § Environment):

```
---- tools::config::tests::index_status_cache_serves_stale_then_refreshes stdout ----
thread '...' panicked at src/tools/config/tests.rs:1554:5:
assertion `left == right` failed
  left: None
 right: Some(false)
```

The preceding `assert!(!check_has_index_cached(...))` passes — the probe *did* return
`false`. The failure is on the next line: nothing was cached. Uncached is precisely the
timeout branch.

## Reproduction

```
1. Have a Qdrant with a real corpus reachable (here: 579,311 points, codescout's own
   project 33,764 chunks).
2. cargo test --features server-stack tools::config::tests::index_status_cache_serves_stale_then_refreshes
```

Passes without `--features server-stack` (no Qdrant client compiled → `RetrievalClient::from_env`
errors immediately → definitive `false` → cached). Commit: `7c1d026e`.

## Environment

**This is the shipped configuration, not an exotic one.** `.cargo/config.toml` defines
`rb = "build --release --features server-stack"`, and `cargo rb` is the command CLAUDE.md
mandates for the live MCP release binary. CI compiles no `server-stack` lane, which is
why this has never been seen — see the related bug file.

Qdrant at `127.0.0.1:6334` (gRPC), collection `code_chunks`, 579,311 points across 21
project ids.

## Root cause

`src/tools/config/mod.rs:424-436` — `check_has_index` needs only existence:

```rust
client
    .project_index_stats(&coll, project_id)
    .await
    .map(|(chunks, _files)| chunks > 0)
    .unwrap_or(false)
```

`src/retrieval/qdrant.rs:161-205` — `project_index_stats` is an exhaustive paginated
scroll, and its doc comment says so plainly ("Scroll all chunks for a project"):

```rust
let mut builder = ScrollPointsBuilder::new(collection)
    .filter(filter.clone())
    .with_payload(true)      // <-- full payload, including `content`
    .with_vectors(false)
    .limit(1000u32);
```

For codescout's 33,764 chunks that is ~34 round trips carrying every chunk's source
text, to decide whether the count is greater than zero.

`src/tools/config/mod.rs:450` caps it at `FIRST_PROBE_TIMEOUT = 2s`, and
`resolve_first_probe` (`:480-491`) deliberately does not cache a timeout — correct in
isolation ("so the next activation re-probes instead of serving a poisoned negative"),
but it converts a slow probe into a *permanently* slow probe, repeated per activation.

The function is not wrong for its own name: `project_index_stats` genuinely needs
`file_count`, and distinct-file counting requires enumeration. The defect is the caller
using a stats function as an existence check.

*measured 2026-08-08: exact `count` calls against this same collection took 2.12 s,
2.27 s and 2.22 s — already over the 2 s budget, and a filtered count is strictly
cheaper than the full-payload scroll this path performs. A 58-page scroll of the whole
collection took ~4 minutes wall clock. Mechanism then read at the three sites cited
above.*

## Hypotheses tried

1. **Hypothesis:** the gated test rotted and needs updating; there is no product defect.
   **Test:** read `check_has_index` → `project_index_stats` → the scroll builder, and
   compare the probe's question against the callee's work.
   **Verdict:** rejected. The test's premise ("stack offline in tests => false") is what
   rotted, but the reason it now fails is a real timeout on a real query, and the
   timeout's consequence — a wrong `index.status` plus a repeated full scan — exists
   independently of any test.

## Fix

**Implemented in `feac9539` (`experiments`).** Promotion is by fast-forward, so this SHA
*is* the master SHA — there is no second one to record later.

1. **`CodeVectorStore::project_has_chunks`** — existence as its own trait method.
   Qdrant: one scroll, `limit(1)`, `with_payload(false)`, `with_vectors(false)`, constant
   work regardless of corpus size. sqlite: `SELECT EXISTS(SELECT 1 …)`, which stops at
   the first row. `check_has_index` calls that instead of `project_index_stats(..).0 > 0`.

   **Required, not defaulted.** A default delegating to `project_index_stats` would have
   been a one-line change touching no implementors — and would have made this exact
   defect the behaviour every future backend inherits by not thinking about it. Five
   implementors each answering the cheap question cheaply is the point.

2. **Two sibling payload over-fetches, found while fixing the first.** Both scrolls in
   `src/retrieval/qdrant.rs` passed `with_payload(true)` and read two or three keys:
   - `scroll_chunk_refs` reads only `chunk_id` and `content_hash`, and runs on **every
     sync** — `stream_index` diffs against it. It was pulling every chunk's `content`
     over the wire to compare hashes. Now `PayloadIncludeSelector{fields: [chunk_id,
     content_hash]}`.
   - `project_index_stats` reads only `file_path`. Now selects just that.

   Neither was in the original diagnosis; both are the same mistake as the headline bug
   (fetch everything, read a little) at different call sites. The sync-path one is
   plausibly the more expensive of the three in aggregate.

3. **`FIRST_PROBE_TIMEOUT` left at 2s**, as § Fix candidate 3 argued. The fix is to make
   the question cheap, not the deadline generous.

**Gate:** `fmt`; `clippy --all-targets -D warnings` on both default and `server-stack`;
`cargo test` 3581; `cargo test --features server-stack` **3586 passed / 0 failed** — the
first green run of the shipped configuration; `check --no-default-features --all-targets`
and `test --no-default-features` green.
## Tests added

- `contract_has_chunks_agrees_with_stats_and_costs_nothing_extra`
  (`src/retrieval/code_store.rs`). Pins `project_has_chunks` to
  `project_index_stats().0 > 0` in both the empty and populated state — the two answer
  the same question by different routes and can drift apart silently — plus a scoping
  case: another project's chunks must not make this one look indexed.
  *Mutation:* making the in-memory impl ignore `project_id` fails it **on the scoping
  assertion specifically**, not on the easy ones.

- `index_status_cache_serves_stale_then_refreshes` — rewritten hermetic. It was
  asserting, through a live probe, that a network round trip completes inside two
  seconds; that is not a property a unit test controls. After the fix it passed alone
  (1.43 s) and still failed under full-suite load, which is the tell. It now drives
  `resolve_first_probe` directly — already a pure function over the probe outcome — and
  covers a branch the old version never asserted at all: **a timed-out probe must not be
  cached**.

**What is not covered by a test, stated rather than implied:** that the Qdrant path does
O(1) work. That property is structural — `limit(1)`, no payload, no vectors — and
verifying it needs a store double counting round trips against a caller that cannot
currently be injected (`check_has_index` builds its client from env). The trait being
required-not-defaulted is the guard that a new backend cannot regress it by omission.
## Workarounds

None needed for correctness of search — this affects only the `index.status` field in
the activation response and the hint derived from it. Indexing and querying are
unaffected. The visible cost is a misleading "not indexed" and a wasted scroll per
activation.

## Resume

N/A — fixed 2026-08-08.

One thing deliberately left: `check_has_index` resolves its client via
`RetrievalClient::from_env()`, so no test can inject a store double and assert on call
counts. That is why the O(1) property is argued structurally above rather than pinned by
a test. If this area is touched again, making the client injectable is the change that
would let the cost property be asserted rather than reasoned about.
## References

- `src/tools/config/mod.rs:424-436` — `check_has_index`, the one-bit question
- `src/tools/config/mod.rs:450` — `FIRST_PROBE_TIMEOUT`, 2s
- `src/tools/config/mod.rs:480-491` — `resolve_first_probe`, the deliberate no-cache-on-timeout
- `src/retrieval/qdrant.rs:161-205` — `project_index_stats`, the exhaustive scroll
- `.cargo/config.toml` — `rb = "build --release --features server-stack"`, why this ships untested
