---
kind: bug
status: open
title: 'BUG: check_has_index scrolls every chunk of a project with full payloads to answer "are there any?", so on a real corpus it exceeds its 2s budget, reports the project unindexed, and re-does the whole scroll on every activation'
tags:
- retrieval
- qdrant
- activation
- performance
- silent-wrong-answer
closed: null
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

Not yet implemented. In preference order:

1. **Give existence its own query.** A `project_has_chunks(collection, project_id)` that
   scrolls with `limit(1)`, `with_payload(false)`, `with_vectors(false)` and returns
   whether the first page is non-empty. One round trip, no payloads, no pagination.
   `check_has_index` calls that instead. This is the actual fix; everything else is
   mitigation.
2. **Stop fetching payloads in `project_index_stats` that it does not need** — it uses
   only `file_path`, so `with_payload` should select that one key rather than `true`.
   Independently worth doing; it does not by itself make an O(corpus) call fit in 2 s.
3. **Do not raise `FIRST_PROBE_TIMEOUT`.** Tempting and wrong: it trades a wrong answer
   for a slow activation, and the number that "works" is a function of the biggest
   corpus anyone points at it — precisely the environment-specific constant memory
   `conventions` § *Environment-Agnostic Tuning* warns against.

## Tests added

None yet. The regression test needs the existence path asserted against a store double
that **counts round trips** — the defect is not "returns the wrong answer" but "does
O(corpus) work to return the right one", and only a call-count assertion discriminates
those. `RecordingStore` in `src/retrieval/sync.rs` is the existing pattern to copy.

`index_status_cache_serves_stale_then_refreshes` should also stop encoding "stack
offline in tests" as an unstated premise; with the fix its first probe completes on a
live stack, so it can assert the completed-and-cached path honestly.

## Workarounds

None needed for correctness of search — this affects only the `index.status` field in
the activation response and the hint derived from it. Indexing and querying are
unaffected. The visible cost is a misleading "not indexed" and a wasted scroll per
activation.

## Resume

Read `RetrievalClient::project_index_stats` (`src/retrieval/client.rs:101`) and check
whether any caller other than `check_has_index` needs only existence — `index(action=
"status")` and the dashboard both plausibly want the real counts, so the fix is a new
narrow method rather than changing the existing one. Then implement § Fix candidate 1
and give it the round-trip-counting test described above.

## References

- `src/tools/config/mod.rs:424-436` — `check_has_index`, the one-bit question
- `src/tools/config/mod.rs:450` — `FIRST_PROBE_TIMEOUT`, 2s
- `src/tools/config/mod.rs:480-491` — `resolve_first_probe`, the deliberate no-cache-on-timeout
- `src/retrieval/qdrant.rs:161-205` — `project_index_stats`, the exhaustive scroll
- `.cargo/config.toml` — `rb = "build --release --features server-stack"`, why this ships untested
