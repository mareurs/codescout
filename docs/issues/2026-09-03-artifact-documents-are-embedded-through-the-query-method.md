---
kind: bug
status: open
tags:
- librarian
- embedding
- retrieval
- qdrant
- cluster/repro-env-diverges-from-gate-env
closed: null
opened: 2026-09-03
owner: marius
related:
- docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md
- docs/trackers/retrieval-benchmark.md
severity: medium
---

# BUG: artifact documents are embedded through `embed_query`, so every stored vector carries the query prefix — the exact inversion `remote.rs` warns about

## Summary

`EmbeddingService::embed_artifact` embeds **documents** by calling
`self.embedder.embed_query(&text)` (`src/librarian/embedding.rs:14-17`). On the
remote embedder that method applies the configured `QueryPrefix`
(`crates/codescout-embed/src/remote.rs:596-609`), and this deployment sets
`CODESCOUT_QUERY_PREFIX="Represent this query for searching relevant code: "`.
Every artifact chunk in Qdrant is therefore stored in **query space**, which is
the inversion `remote.rs`'s own doc comment names as the thing not to do, and a
second-subsystem recurrence of
`docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`.

**Measured effect is small** — see § *What this does NOT explain*. Filed because
it is unambiguously wrong, silent, and cheap to get backwards again; not because
it is the cause of any current benchmark number.

## Symptom (Effect)

No error, no log line. The only observable is that a stored vector is bit-identical
to the prefixed embedding of its own text:

```
chunk 9c864963-3b9a-49b0-b19f-419b4eaebd19  entry=W-81
cos(stored, embedded WITHOUT prefix) = 0.993610
cos(stored, embedded WITH    prefix) = 1.000000
VERDICT: documents are stored PREFIXED (query-space)
```

## Reproduction

```
git rev-parse HEAD    # ae82cf10, branch experiments
# 1. confirm the prefix is live in the running server
tr '\0' '\n' < /proc/<server-pid>/environ | grep CODESCOUT_QUERY_PREFIX
#    -> CODESCOUT_QUERY_PREFIX=Represent this query for searching relevant code:

# 2. take any chunk row, rebuild the document text the indexer built:
#      text = "{entry_token}\n\n{content}" if token set and content doesn't start with '#'
#      doc  = "{artifact.title}\n\n{text}"
# 3. embed doc twice (with and without the prefix) against the same embedder
# 4. scroll the chunk's point out of artifact_chunks_<project>_<hash> with_vector=true
# 5. cosine both against the stored vector -> the prefixed one is 1.000000
```

Script used: `prefix_probe.py`, reproduced 2026-09-03 ~23:55.

## Environment

Linux 7.1.9-zen1-2-zen, `experiments` @ `ae82cf10`. Embedder
`CodeRankEmbed-Q4_K_M.gguf` over llama-server at `127.0.0.1:48081`.
`CODESCOUT_QUERY_PREFIX` set in the MCP server's environment. Collection
`artifact_chunks_codescout_dc6a871595179329`, 29,077 points.

## Root cause

```rust
// src/librarian/embedding.rs:14-17
pub async fn embed_artifact(&self, title: Option<&str>, body: &str) -> Result<Vec<f32>> {
    let text = format!("{}\n\n{}", title.unwrap_or(""), body);
    self.embedder.embed_query(&text).await     // <-- document, sent through the QUERY method
}
```

```rust
// crates/codescout-embed/src/remote.rs:590-596
/// Embed a single **query**, applying the configured [`QueryPrefix`].
///
/// The document side ([`Embedder::embed`]) never prefixes — that asymmetry is
/// the entire point of an asymmetric model, and getting it backwards strands
/// stored vectors in query-space. See
/// `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`.
async fn embed_query(&self, text: &str) -> Result<Embedding> {
```

The warning and the violation are four files apart, and the violating call site
reads as correct — `embed_query` is also the *only* method on the trait that
returns a single vector, so reaching for it from a function that embeds one
document is the natural move. `Embedder::embed(&[text])` is the correct call and
requires unwrapping a batch of one.

**Why the default hid it.** `QueryPrefix::Suppressed` is the documented default
for an unset `CODESCOUT_QUERY_PREFIX` (`remote.rs:103-119`, ruled 2026-08-30,
worth ~3 benchmark points). Under that default `embed_query == embed` and this
bug is inert. It only becomes real when a deployment sets the variable — which
this one does, in the environment rather than in any tracked config, so reading
the source alone gives you the wrong answer. *Measured, not inferred:* I rejected
this hypothesis once from the documented default, then re-confirmed it by reading
`/proc/<pid>/environ`.

## Evidence

### The prefix moves a document vector by 0.64%

`cos(plain, prefixed) = 0.993610` on a real 2,023-byte chunk. That is the bound
on this bug's ranking impact, produced by the same probe that confirmed it.

## Hypotheses tried

1. **Hypothesis:** documents are unprefixed because the shipped default is
   `Suppressed`.
   **Test:** read `/proc/938147/environ`.
   **Verdict:** **rejected** — `CODESCOUT_QUERY_PREFIX` is set in the live server.
   The source default is not the deployed behaviour.

2. **Hypothesis:** this explains the artifact benchmark's 3/12.
   **Test:** raw kNN ranks of the 11 scorable targets; the 0.64% bound above.
   **Verdict:** **rejected** — see below.

## What this does NOT explain

Filed with its own negative result so nobody spends a day on it expecting a
benchmark move. Target chunks for the failing cases rank 15, 25, 27, 71 and >300
in raw kNN. A 0.64% cosine shift cannot move a rank-249 result into a top-5 page.
The artifact benchmark's misses were attributed 2026-09-03 to page policy
(`max_per_artifact`), mis-specified ground truth, and two genuine ranking losses
— *not* to this. Fixing it is correctness, not a score.

## Fix

*Plan only.*

`embed_artifact` should call `Embedder::embed(&[text])` and take the single
vector, leaving `embed_query` to queries. One line, plus the batch unwrap.

**It requires a full re-embed to take effect**, and that is the expensive part,
not the edit: every stored artifact vector is currently in query-space, so a
mixed corpus would be worse than a consistent wrong one. Land the code change and
the re-embed together, or not at all.

The durable fix is that the trait lets a document be embedded by the query
method at all. A `embed_document`/`embed_query` pair with no general-purpose
single-text method would make this unrepresentable rather than merely
discouraged — which is what the four-files-away comment is currently substituting
for.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None yet. The discriminating test does not assert on a call — it asserts on the
**stored bytes**: embed a document through the service with a prefix configured,
and assert the result equals the *unprefixed* embedding. A test that mocks the
embedder and checks "was `embed` called" passes against a mock that ignores the
prefix, which is the same shape as the bug.

## Workarounds

Unset `CODESCOUT_QUERY_PREFIX` — which is the documented default and was measured
in 2026-08-30's D2 ruling as the *better* configuration for
`CodeRankEmbed-Q4_K_M` anyway (37 with no prefix vs 34 with it). That makes the
bug inert without touching code, at the cost of a re-embed to normalise the
existing vectors.

## Resume

`src/librarian/embedding.rs:14-17` (the call), `crates/codescout-embed/src/remote.rs:590-609`
(the warning), `remote.rs:103-133` (why the default hides it).

## References

- `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md` —
  the same inversion in the memory subsystem. That fix did not generalise; this
  is the second subsystem to do it.
- `docs/trackers/retrieval-benchmark.md` — the 2026-08-30 D2 prefix ruling, and
  the 2026-09-03 attribution of the artifact benchmark's misses.

### Cluster adjudication

Tagged `cluster/repro-env-diverges-from-gate-env` (`IC-5`). The behaviour is
decided by an environment variable that no tracked config records: the source
default says `Suppressed` and the deployed process says otherwise, so reading the
repo gives a confident wrong answer and only `/proc/<pid>/environ` gives the
right one. I rejected this bug once on exactly that basis before re-checking at
runtime.

Considered and rejected: `IC-11` (`doc-contradicted-by-code`) — the doc comment
at `remote.rs:592` is *correct* and the code contradicts it, which is the reverse
of that class; and `IC-14` (`guard-narrower-than-its-name`) — there is no guard
here, only a comment.
