---
kind: bug
status: fixed
tags:
- tests
- vacuous-assertion
- codescout-embed
- embeddings
- cluster/assertion-that-cannot-fail
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related:
- docs/trackers/resume-embedding-transport-stages-1-3.md
severity: low
---

# BUG: `ollama_large_batch_exceeding_batch_size` never exceeded the batch size — vacuous since the day it was written

## Summary

`crates/codescout-embed/src/remote.rs::tests::ollama_large_batch_exceeding_batch_size`
exists to exercise `RemoteEmbedder::embed`'s chunking loop. It sends 20 texts,
on the stated premise that `BATCH_SIZE` is 8. `BATCH_SIZE` is **32**, and
`git log -S` finds no commit in which it was ever 8. 20 < 32, so the loop runs
exactly one chunk and the chunking logic the test names is never entered.

It is also `#[ignore]`d (requires a live Ollama), so it does not run in CI either
way. The vacuity is therefore invisible from two directions at once.

## Symptom (Effect)

`crates/codescout-embed/src/remote.rs:626-635`:

```rust
#[tokio::test]
#[ignore = "requires running Ollama"]
async fn ollama_large_batch_exceeding_batch_size() {
    // BATCH_SIZE is 8; send 20 texts to exercise the chunking logic
    let embedder = make_embedder();
    let texts: Vec<String> = (0..20)
        .map(|i| format!("fn function_{i}() -> i32 {{ {i} }}"))
        .collect();
```

against `crates/codescout-embed/src/remote.rs:384`:

```rust
const BATCH_SIZE: usize = 32;
```

No failure is observable — the test passes (when run at all), for the wrong
reason. That is the defect: it reports a green chunking guard that has never
chunked.

## Reproduction

Read the two lines above. Or, to make the vacuity fail loudly, mutate the
chunking away entirely:

```rust
// in RemoteEmbedder::embed, replace
for batch in filtered.chunks(BATCH_SIZE) {
// with
for batch in filtered.chunks(usize::MAX) {
```

`ollama_large_batch_exceeding_batch_size` still passes with a live Ollama: 20
texts were already one chunk.

## Environment

Any. This is a compile-time constant vs. a literal in the same file — not
platform-, feature-, or config-dependent.

## Root cause

The comment's premise was **wrong when written**, not drifted into wrongness.

**measured 2026-08-30:**

```
git log -4 --format='%h %ad %s' --date=short -S 'const BATCH_SIZE: usize = 32' \
  -- crates/codescout-embed/src/remote.rs
  aa6bff1d 2026-04-21 feat: promote all ready features from experiments to master

git log -4 --format='%h %ad %s' --date=short -S 'const BATCH_SIZE: usize = 8' \
  -- crates/codescout-embed/src/remote.rs
  (no output)
```

The second command returning nothing is the finding: there is no commit in this
file's history where `BATCH_SIZE` was 8. The `-S` pairing matters — one command
alone would only show when 32 arrived, which is consistent with a later change
*from* 8 and would support the opposite (and wrong) "stale comment" reading.

Where the 8 likely came from: root's `EmbedderHttp::resolve_batch_size` uses
`const FALLBACK: usize = 8`, and its own doc records that a previous
`const BATCH = 8` there "was justified by a comment citing a cap that only
`sparse-amd` ever imposed, and it silently survived that service's removal"
(`src/retrieval/embedder.rs:750-755`). An 8 from the *other* crate's *sparse*
path appears to have been carried into a comment about this crate's *dense*
constant. Plausible, and explicitly **not** measured — recorded as the likely
origin, not as established fact.

## Evidence

### The two directions the vacuity hides in

1. **Wrong premise** — the assertion count (20) was chosen against a constant
   that never held, so the branch it targets is unreachable by construction.
2. **`#[ignore]`** — needing a live Ollama, it never runs in CI, so no run has
   ever had the chance to reveal (1) through a surprising pass.

Neither alone is unusual. Together they mean nothing in the repository, and no
run anywhere, could report this.

### Why it surfaced now

Found while scouting `resume-embedding-transport-stages-1-3:ET-9` T6, which
instructs "hold batch size at **8** so the change is behaviour-preserving". That
sent a reader to `BATCH_SIZE` expecting 8 and finding 32 — see that entry's own
correction, recorded separately. The test comment is a third site repeating the
same wrong number.

## Hypotheses tried

1. **Hypothesis:** the comment is stale — `BATCH_SIZE` used to be 8 and was
   raised to 32 without updating the test.
   **Test:** `git log -S 'const BATCH_SIZE: usize = 8'` on the file.
   **Verdict:** rejected — no such commit. It was 32 from `aa6bff1d` onward and
   never 8. The comment was wrong at authoring time.

## Fix

Not applied — filed on notice, per CLAUDE.md's capture-on-notice rule, while
working a different task (ET-9 T6).

Two candidate fixes, and they are not equivalent:

- **Raise the input count** past 32 (e.g. 70 texts → 3 chunks) and correct the
  comment. Keeps the test's stated intent and makes it real. Still `#[ignore]`d,
  so it only helps someone running the live suite deliberately.
- **Rewrite it as a non-`#[ignore]` unit test** against a mock endpoint,
  asserting the *number of HTTP requests* for a given input count. That is what
  actually pins chunking, runs in CI, and does not need Ollama. `remote.rs`
  already has mockito-based tests to model it on.

The second is preferable and is a slightly larger change. Whichever is chosen,
**verify by mutation** — `chunks(BATCH_SIZE)` → `chunks(usize::MAX)` must turn
the test red. A fix that does not is the same defect with a bigger number.

## Fix provenance

**Applied 2026-08-30 on `experiments`.**

- SHA: `236f31a4` (`experiments` — orphans on the next rebase)
- patch-id: `7f066d41f254df6428d99ae17908e031f9d8c95a` (survives rebase and cherry-pick)

Took the second of the two candidate fixes, as this file recommended: a loopback
test that runs in CI, not a bigger number on an `#[ignore]`d one.

**The assertion is the split's shape, not its existence.** 70 inputs against the
32 cap must arrive as exactly `[32, 32, 6]`, in order. `requests > 1` would have
passed for any chunk size that is not 70 — a laxer assertion reproducing this very
defect with a bigger number, which is what the file warned against.

**The mock had to echo the request's own arity.** `embed` rejects a response whose
length disagrees with the batch it sent, so a fixed-size answer fails the request
instead of exercising the loop. The helper reads headers then exactly
`Content-Length` bytes — a 70-input body does not fit one read, and counting a
truncated body would under-report the batch size, which is the same defect class
the test exists to catch.

**The old test was deleted, not renumbered.** Its chunking purpose is now covered
properly, and its other assertion — consistent dimensions across a batch — already
duplicates `ollama_batch_consistent_dimensions` two tests above it.

**Mutation-verified as this file demanded:** `chunks(BATCH_SIZE)` →
`chunks(usize::MAX)` gives `left: [70]`, `right: [32, 32, 6]`. The old test passes
unchanged under that same mutation — which is the entire finding, now demonstrated
rather than argued.

## A second vacuity, found while fixing the first

`cargo test -p codescout-embed an_oversize_batch_is_split_into_batch_size_requests`
reports **`0 passed; 19 filtered out`, exit 0**. The `remote` module sits behind
the `remote-embed` feature, so without `--features remote-embed` the test does not
exist to be run. That zero is character-identical to a pass and had to be caught by
reading the count rather than the exit code — the same shape as this bug one level
out, and the same shape as the `0/N` checker-exec-bit trap `CLAUDE.md` records for
the prompt-eval harness.

The test itself is CI-safe: `.github/workflows/ci.yml` builds the workspace with
features, and the lean lane (`--no-default-features`, 3385 passed) simply does not
compile the module. Verified green in both.
## Tests added

None yet — the fix is the test.

## Workarounds

N/A — no runtime impact. `RemoteEmbedder::embed`'s chunking is believed correct;
what is missing is evidence for it, not the behaviour.

## Resume

Decide between the two fixes in § Fix (recommend the mockito request-count
version). Implement in `crates/codescout-embed/src/remote.rs`, modelled on the
existing mockito tests in that file's `tests` module. Then run the
`chunks(usize::MAX)` mutation and confirm the new test fails; a green mutation
means the rewrite reproduced the original defect.

## References

- `crates/codescout-embed/src/remote.rs:384` — `const BATCH_SIZE: usize = 32`.
- `crates/codescout-embed/src/remote.rs:626-635` — the test and its wrong comment.
- `src/retrieval/embedder.rs:750-755` — root's `resolve_batch_size` doc, and the
  likely origin of the 8.
- `docs/trackers/resume-embedding-transport-stages-1-3.md` — `ET-3` / `ET-9` T6,
  which repeat the same 8 and are corrected there.
