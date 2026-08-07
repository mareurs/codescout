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


### 2026-08-07 — both halves confirmed by probing the endpoints directly; no Langfuse needed

The previous Resume proposed running real queries and reading Langfuse's `rerank` span in/out
counts. A direct probe answers it outright, because the defect is about the score **scale**, not
the drop **count**. One identical two-document probe — one on-topic, one deliberately off-topic —
posted to each live endpoint in the exact TEI shape the client uses (`{query, texts}` →
`[{index, score}]`, `src/embeddings/reranker.rs`):

| endpoint | on-topic | off-topic | scale |
|---|---|---|---|
| `localhost:30083` (TEI) | `0.9965708` | `0.0000107` | sigmoid, **[0,1]** |
| `localhost:48083` (llama-server) | `5.3375511` | `-11.0001240` | raw **logits** |

**`min_score = -5.0` is provably dead under TEI — not rarely triggered, unsatisfiable.** The
filter is `if relevance_score < min_score` (`src/embeddings/reranker.rs:101`) and TEI scores are
bounded below by 0, so no source can ever be dropped. Sixteen of the seventeen live servers point
at 30083.

**Under llama-server it is live and well placed** — off-topic at −11.0 against on-topic at +5.34,
so −5.0 sits cleanly between. That is a second sample for the −8..−11 off-topic band this file
recorded from one.

**And that is precisely where the blend breaks, now quantified.**
`combined = relevance*0.7 + domain_authority*0.2 + quality*0.1`. With relevance in [0,1] the three
terms are commensurate. With logits in [−11, +5.3] the relevance term spans about [−7.7, +3.7]
while authority and quality are confined to [0, 0.2] and [0, 0.1] — a **~40× span mismatch**, so
relevance dominates and the other two weights are decorative.

So the two configurations fail in **opposite** directions from identical code and identical
defaults: **TEI gives a dead filter with a sane blend; llama-server gives a live filter with a
broken blend.** No single `min_score` fixes both, because the scales are incommensurable.

### Configuration provenance is unresolved — and it is NOT drift

Worth pinning, because the obvious inference is wrong. The 17 live servers split 16 on 30083 and
one (pid 373104) on 48083 — the latter the only one also carrying an explicit
`RERANK_MIN_SCORE=-5.0`. Researcher's own `.env` sets **no** `RERANK_BASE_URL`, and the compiled
default is `""`, which disables reranking entirely: the whole rerank block in
`src/researcher/pipeline.rs` is gated on `!cfg.rerank_base_url.is_empty()`.

The tempting conclusion — live servers carry a stale config, new ones get reranking disabled — is
**false.** A server launched **1.3 h ago** still has 30083, alongside ones at 84.8 h and 94.1 h. So
the value comes from a live source. Ruled out by inspection: researcher's `.env`; `~/.bashrc`,
`~/.zshrc`, `~/.profile`, `~/.zshenv`; `systemctl --user show-environment`;
`~/.config/environment.d/`; `~/.claude.json` and the `.claude*/` profile JSONs; codescout's
`.mcp.json`; and the `claude daemon` process env. The value appears in `/proc/<pid>/environ`, so it
was present at **exec** time and is therefore not a dotenv-loaded variable (runtime `setenv` does
not alter that snapshot). Recorded so the next reader does not re-walk the same eight negatives.

### RESOLVED same day — it is the user-scope MCP config, identical in all three profiles

`RERANK_BASE_URL` is injected by Claude Code from the **user-scope MCP server `env` block**, not
inherited from any shell. The discriminator is one comparison the eight negatives above never made:
the parent `claude` processes of servers 1345614, 878507 and 35489 have **no** `RERANK_*` at all,
while their children do. A variable present in the child and absent in the parent can only have
been injected at spawn.

`claude mcp get researcher` reports *"Scope: User config (available in all your projects)"*, and
the value is set identically in **all three** profiles:

```
.claude/.claude.json        {'RERANK_BASE_URL': 'http://localhost:30083'}
.claude-sdd/.claude.json    {'RERANK_BASE_URL': 'http://localhost:30083'}
.claude-kat/.claude.json    {'RERANK_BASE_URL': 'http://localhost:30083'}
```

**Why the earlier search missed it, which is worth recording.** The grep was
`/home/marius/.claude-sdd/*.json` — and `*` does not match a leading dot, so `.claude.json` was
never examined. Identical false-negative class to the `grep` hidden-path bug fixed the same day in
`624f7f05`, encountered in the shell rather than in the tool.

**And the drift runs the other way.** No profile sets `RERANK_MIN_SCORE`, so the code default
−5.0 applies everywhere. The one server on 48083 *with* an explicit `RERANK_MIN_SCORE=-5.0`
(pid 373104, 23.5 h old) has neither variable in its parent `claude --continue` nor in its
grandparent `bash` — so it came from a **superseded revision of the profile config**. It is the
stale one; the sixteen on 30083 are current.

### What that settles

The canonical configuration — now and going forward — is **TEI on 30083 with `min_score` at the
−5.0 code default**. Combined with the measured scale, that means:

- **The off-topic filter does nothing at all, in the configuration that actually ships.** Not
  "mis-tuned": inert. TEI scores are bounded below by 0 and the threshold is −5.0.
- **The weight blend is fine** under TEI — relevance in [0,1] is commensurate with
  `domain_authority` and the quality score, so 0.7/0.2/0.1 mean what they appear to mean.
- **The llama-server distortion is a latent hazard, not a current defect.** It bites only if the
  URL is repointed at a logit backend, which is exactly what one stale server did.

So the immediate fix is much smaller than the *Fix* section's two options imply: express
`min_score` in TEI units. Measured anchors from the probe — off-topic `0.0000107`, on-topic
`0.9965708` — leave a threshold anywhere in roughly `0.001`–`0.05` dropping the off-topic band with
four orders of magnitude of margin. The normalisation-plus-protocol-flag work remains the right
shape for making a backend swap safe, but it is no longer needed to make the filter function.
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

Both the measurement and the provenance question are **done** — see the 2026-08-07 Evidence
subsections. Nothing left to investigate.

**The canonical configuration is TEI on `localhost:30083`**, set identically in all three profiles'
user-scope MCP `env`, with no `RERANK_MIN_SCORE` so the −5.0 code default applies. TEI returns
sigmoid scores in [0,1], so **the off-topic filter is inert in the shipping configuration** — and
the weight blend is fine there, because relevance is commensurate with authority and quality.

**Fix, in priority order:**

1. **Set `min_score` in TEI units.** Anywhere in `0.001`–`0.05` drops the measured off-topic band
   (`0.0000107`) with orders of magnitude of margin below the measured on-topic score
   (`0.9965708`). One-line change; makes a dead knob work. Worth widening the sample past two
   documents before picking the exact value.
2. **Add an explicit protocol/normalisation setting** so a backend swap cannot silently change the
   score scale underneath the blend. `RerankerClient` hardcodes the TEI request/response shape
   while being pointed at an arbitrary URL, which is how the 48083 server ended up with logits
   feeding a blend calibrated for [0,1]. Codescout solved the same fork with
   `CODESCOUT_RERANKER_PROTOCOL` selecting `Protocol::Infinity` in `src/retrieval/reranker.rs`, and
   `.env.gpu` warns that omitting it silently falls back to the TEI shape.
3. **Optional housekeeping:** the 48083 server (pid 373104) runs a superseded config revision.
   Restarting that session realigns it; nothing in the code needs to change for it.

Only step 1 needs a decision, and only about the exact constant.
## References

- `researcher/src/embeddings/reranker.rs:10-19` — request/response types
- `researcher/src/embeddings/reranker.rs:~90-115` — min_score filter and
  combined_score blend
- `researcher/src/config.rs:157-176` — weights and threshold defaults
- `researcher/src/researcher/pipeline.rs:438-460` — soft-failure path
- `researcher/infra/docker-compose.yml` — `tei-rerank`, the previous backend
- `docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md` — why :48083 exists
