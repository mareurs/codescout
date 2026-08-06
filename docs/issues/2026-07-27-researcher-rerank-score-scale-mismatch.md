---
status: open
opened: 2026-07-27
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md
tags: [researcher, rerank, calibration, cross-repo]
kind: bug
---

# BUG: researcher's rerank score scale is uncalibrated — min_score was dead under TEI, weight blend now distorted under llama-server

*Scope note: this bug is in `/home/marius/work/claude/researcher`, not codescout.
Filed here because codescout's `docs/issues/` is the tracker actually queried and
researcher has no issues directory. Move it if researcher grows one.*

## Summary

`RERANK_MIN_SCORE` is documented as operating on a logit scale, but the TEI
backend researcher used returned sigmoid-normalised 0..1 scores — so the filter
never fired. Switching `RERANK_BASE_URL` to the llama-server reranker (2026-07-27)
makes it fire as documented, but simultaneously breaks the `combined_score`
weight blend, because raw logits span roughly ±11 while the other two terms are
bounded by 0.2 and 0.1.

## Symptom (Effect)

Same request, two backends. `src/embeddings/reranker.rs` posts
`{query, texts}` to `{base}/rerank` and parses `Vec<{index, score}>`; both
backends answer that shape, so nothing errors.

```
query: "how do I parse a configuration file"
docs:  ["fn parse_config(path: &str) -> Config { }",
        "fn main() { println!(\"hello\"); }",
        "The mitochondria is the powerhouse of the cell."]

TEI :30083  (cross-encoder/ms-marco-MiniLM-L-6-v2)
  [{"index":0,"score":0.0008443969},
   {"index":1,"score":0.000013336498},
   {"index":2,"score":0.000011461376}]          ← sigmoid, 0..1

llama-server :48083  (bge-reranker-v2-m3-Q4_K_M)
  [{"index":0,"score":1.3913599252700806},
   {"index":1,"score":-8.401334762573242},
   {"index":2,"score":-10.944034576416016}]     ← raw logits, ~±11
```

Two consequences:

1. **`min_score` was dead code under TEI.** Default is `-5.0`
   (`src/config.rs:175`). Every sigmoid score is `> -5.0`, so
   `relevance_score < min_score` (`src/embeddings/reranker.rs:~97`) never
   dropped a source. The documented behaviour — "drops clearly off-topic
   results" — has never happened.
2. **The weight blend is now distorted.** `combined_score` is
   `relevance*0.7 + authority*0.2 + quality*0.1`
   (`src/embeddings/reranker.rs:~105-110`). Under TEI, relevance contributed at
   most 0.7 against authority's 0.2 — a ~3.5:1 design ratio. Under raw logits it
   contributes ±7.7, so `domain_authority` and `quality_score` become numerical
   noise and ranking is effectively pure cross-encoder.

Neither surfaces as an error. Rerank failure is already soft — `pipeline.rs`
catches it with `warn!("cross-encoder rerank failed, using dedup order")` — and
a *mis-scaled* result isn't a failure at all.

## Reproduction

```
curl -s :30083/rerank -H 'content-type: application/json' \
  -d '{"query":"parse config","texts":["fn parse_config()","fn main()"]}'
curl -s :48083/rerank -H 'content-type: application/json' \
  -d '{"query":"parse config","texts":["fn parse_config()","fn main()"]}'
```

Compare the score magnitudes.

## Environment

- researcher @ `/home/marius/work/claude/researcher`, MCP config `.mcp.json`
- Was: `RERANK_BASE_URL=http://localhost:30083` (TEI `cpu-1.8`, ai-infra project)
- Now: `RERANK_BASE_URL=http://localhost:48083` (llama-server `server-cuda`,
  codescout-retrieval project, `bge-reranker-v2-m3-Q4_K_M.gguf`)

## Root cause

`RerankerClient` treats the reranker's score as a scale-free number. It neither
requests a normalisation mode nor normalises on receipt, so the meaning of
`score` is set entirely by the backend:

- TEI's `/rerank` has a `raw_scores` flag defaulting to `false` → sigmoid.
  `RerankRequest` (`src/embeddings/reranker.rs:10-13`) has only `query` and
  `texts`, so it always gets the default.
- llama-server has no such flag and always returns raw logits.

`min_score` and the three weights are then calibrated against a scale that the
client does not pin down.

Worth noting: codescout hit the *transport* half of this same problem and solved
it with an explicit `Protocol` enum (`src/retrieval/reranker.rs:12-27`). Neither
codebase pins the *score scale*, which is the half that fails silently.

## Evidence

Config defaults, `src/config.rs:157-176`:

```rust
#[arg(long, env = "RERANK_BASE_URL", default_value = "")]
pub rerank_base_url: String,
#[arg(long, env = "RERANK_RELEVANCE_WEIGHT", default_value = "0.7")]
pub rerank_relevance_weight: f32,
#[arg(long, env = "RERANK_AUTHORITY_WEIGHT", default_value = "0.2")]
pub rerank_authority_weight: f32,
#[arg(long, env = "RERANK_QUALITY_WEIGHT", default_value = "0.1")]
pub rerank_quality_weight: f32,
/// Minimum raw cross-encoder relevance score to keep a source (logit scale).
/// Sources below this are dropped after reranking. -5.0 drops clearly off-topic results.
#[arg(long, env = "RERANK_MIN_SCORE", default_value = "-5.0")]
pub rerank_min_score: f32,
```

The doc comment is explicit that logits were intended. The backend was not
delivering them.

## Hypotheses tried

1. **Hypothesis:** the two backends need different request shapes, so the swap
   needs a client patch.
   **Test:** posted researcher's exact `{query, texts}` body to both.
   **Verdict:** rejected. llama-server mirrors the request dialect — `texts`
   yields TEI's `[{index, score}]`, `documents` yields Cohere's
   `{results:[{relevance_score}]}`. No transport change was needed; only the
   score *scale* differs.

## Fix

Decided 2026-07-27: **keep raw logits, do not patch the client.** Rationale —
this is what `min_score`'s doc comment always specified, and a reranking stage
being relevance-dominant is defensible. The switch makes a documented feature
work for the first time.

Deferred, for tuning once there is real usage data. Pick one:

- Drop `RERANK_RELEVANCE_WEIGHT` to ~0.05 via env, restoring the intended ~3.5:1
  relevance:authority ratio without touching code.
- Or sigmoid-normalise in `RerankerClient`, retune `RERANK_MIN_SCORE` to a 0..1
  value, and correct the "logit scale" doc comment.

Either way the real fix is to make the scale explicit rather than inherited:
have `RerankerClient` declare which scale it expects, the way codescout's
`Protocol` enum declares which wire shape it expects.

## Tests added

None. Would need a scale assertion — e.g. a test that fails if any returned
score falls outside the range the configured `min_score` implies.

## Workarounds

Revert to the previous backend by setting `RERANK_BASE_URL=http://localhost:30083`
in `researcher/.mcp.json`. The `tei-rerank` container is still running and
healthy, so this is a one-line revert plus a Claude Code restart.

## Resume

Run several real `/research-web` queries against the new reranker and inspect
Langfuse's `rerank` span (`in`/`out` counts). If `out` is dropping far more
sources than expected, `min_score -5.0` is too aggressive for this model — the
observed off-topic band was −8 to −11, so −5.0 has margin, but that is one
sample. Then decide between the two tuning options under "Fix".

## References

- `researcher/src/embeddings/reranker.rs:10-19` — request/response types
- `researcher/src/embeddings/reranker.rs:~90-115` — min_score filter and
  combined_score blend
- `researcher/src/config.rs:157-176` — weights and threshold defaults
- `researcher/src/researcher/pipeline.rs:438-460` — soft-failure path
- `researcher/infra/docker-compose.yml` — `tei-rerank`, the previous backend
- `docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md` — why :48083 exists
