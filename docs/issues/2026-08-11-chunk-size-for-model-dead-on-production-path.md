---
id: '7438a064c8d61f86'
kind: bug
status: investigating
title: 'BUG: chunk_size_for_model''s model-derived chunk budget is dead code on the production indexing path'
tags:
- chunking
- embeddings
- dead-code
- retrieval
closed: null
opened: 2026-08-11
owner: marius
related: []
severity: high
---

## Summary

`codescout_embed::chunk_size_for_model` computes a chunk-size budget derived from each embedding model's documented max sequence length. Its only non-test consumer is `EmbeddingsSection::effective_chunk_size` (`src/config/project.rs:350-357`), and `references(effective_chunk_size)` returns only that function's own unit tests — no production code path calls it. Real indexing goes `sync_project` → `STACK_CHUNK_TARGET = 1200` (`src/retrieval/sync.rs:259-263`, overridable via `CODESCOUT_CHUNK_TARGET`) → `split_file`, capped at `AST_CHUNK_TARGET = 3000` (`src/embed/ast_chunker.rs:953,976`). Nothing on that path consults the configured embedding model at all.

Pre-existing — not introduced by the `feat/local-onnx-query-path` branch — but that branch's Task 4 F2 fix (the branch's headline "silent-correctness bug", with measured before/after chunk-size figures 652/1305/20889 chars) hardens a function no production caller reaches.

## Symptom (Effect)

```
references(symbol="EmbeddingsSection/effective_chunk_size", path="src/config/project.rs")
  → 5 hits, ALL in src/config/project.rs: the definition (:350) and 4 unit tests (:894, :907, :920, :933).
    No hit outside this one file.
```

Separately: `crates/codescout-embed/src/lib.rs:120` documents `all-minilm`/`minilm-l6` (this project's own default model) at 256 max tokens. Production chunking ships 1200 chars (~300–480 tokens at 3–4 chars/token) per chunk regardless of the configured model — larger than that model's own documented budget, and closer to its actual truncation point at 512 tokens than to the 256 the crate documents.

## Reproduction

```
references(symbol="EmbeddingsSection/effective_chunk_size", path="src/config/project.rs")
  → 5 hits, all local to src/config/project.rs (1 definition + 4 tests)

grep(pattern="STACK_CHUNK_TARGET", path="src/retrieval/sync.rs")
  → src/retrieval/sync.rs:259: const STACK_CHUNK_TARGET: usize = 1200;

grep(pattern="AST_CHUNK_TARGET", path="src/embed/ast_chunker.rs")
  → :953  pub const AST_CHUNK_TARGET: usize = 3000;
  → :976  let target = chunk_size.min(AST_CHUNK_TARGET);
```

Neither constant, nor the `chunk_size` value that reaches `split_file`, is derived from `effective_chunk_size()` or `chunk_size_for_model()` anywhere in the call chain.

## Environment

codescout `feat/local-onnx-query-path` @ this branch's tip; static trace via `references`/`grep`, not runtime-measured.

## Root cause

`EmbeddingsSection::effective_chunk_size` (`src/config/project.rs:350-357`) was built as the model-aware chunk-budget API, but `RetrievalClient::sync_project` (`src/retrieval/sync.rs`) never calls it — it hardcodes `STACK_CHUNK_TARGET = 1200` (env-overridable, not model-aware) and passes that straight to `split_file` (`src/embed/ast_chunker.rs`), which only ever applies its own fixed `AST_CHUNK_TARGET = 3000` as an upper *cap*. The two chunk-sizing systems were never wired together. *Read at the bytes (`references` + both const definitions); not measured at runtime.*

## Evidence

- `references(symbol="EmbeddingsSection/effective_chunk_size", path="src/config/project.rs")` → 5 hits, all local to the file (definition + 4 tests).
- `src/retrieval/sync.rs:256-263` — `STACK_CHUNK_TARGET` definition and the `CODESCOUT_CHUNK_TARGET` env override, no reference to `effective_chunk_size` or `chunk_size_for_model` anywhere in `sync_project`.
- `src/embed/ast_chunker.rs:953,974-976` — `AST_CHUNK_TARGET` and its use as `chunk_size.min(AST_CHUNK_TARGET)`, a ceiling only.
- `crates/codescout-embed/src/lib.rs:120` — `"allminilml6v2q" | "allminilml6v2" => 256` inside `chunk_size_for_model`'s `local:` arm — this project's own default model's documented token ceiling.


### 2026-08-14 — premise confirmed, and the units say this is not only dead code

**Premise verified exactly.** `references(EmbeddingsSection/effective_chunk_size)` → 5
hits: one definition (`src/config/project.rs:350`) and four of its own unit tests
(`:894`, `:907`, `:920`, `:933`). **Zero production callers.** The chain
`chunk_size_for_model` → `effective_chunk_size` → nothing is real.

But the units turn this from a cleanup question into a quality question.
`chunk_size_for_model` returns **characters**, not tokens
(`crates/codescout-embed/src/lib.rs:69-71`):

```rust
fn from_tokens(n: usize) -> usize {
    (n as f64 * 0.85 * 3.0) as usize
}
```

So for this project's default model:

| Quantity | Value |
|---|---|
| `AllMiniLML6V2Q` documented max sequence | 256 tokens |
| × 0.85 headroom × 3 chars/token | **652 chars** ← the model-aware budget |
| `STACK_CHUNK_TARGET` actually used | **1200 chars** |
| Ratio | **1.84× over budget** |

The 652 figure is independently corroborated: it is one of the three measured
chunk-size numbers (652 / 1305 / 20889 chars) recorded in
`docs/issues/archive/2026-07-27-ast-chunker-no-minimum-chunk-size.md`.

And the disconnected function's own doc comment states what the factor is *for*
(`lib.rs:55-63`):

> The 0.85 factor leaves 15 % headroom for tokenisation variance and control tokens
> (BOS/EOS). Code tokenises at roughly 3–4 chars/token; 3 is the conservative lower
> bound, ensuring chunks stay within the context window … chunks will be smaller than
> necessary but **will never be truncated**.

At 3 chars/token 1200 chars ≈ 400 tokens; at an optimistic 4 chars/token, 300 tokens.
**Either way above the model's 256.** The mechanism built to prevent truncation was
wired to nothing, and the value production actually uses exceeds what it would have
allowed.

**Not verified — the load-bearing unknown.** Whether the embedder *silently truncates*
at max sequence length or errors. There is no `max_length` / truncation handling
anywhere in `crates/codescout-embed/src` (grepped), so the behaviour belongs to
`fastembed` and its tokenizer, which was not read. If it truncates silently, the tail of
every over-budget chunk is embedded as nothing and retrieval quality degrades with no
signal. That single question decides the direction of the fix, so it must be answered
first — see *Resume*.
## Hypotheses tried

None — found via static tracing (`references`), not runtime measurement. Filed for tracking per explicit instruction not to attempt a fix on this branch (pre-existing, out of scope for `feat/local-onnx-query-path`).

## Fix

Still not implemented, and **the decision is not the one this file originally framed.**

As filed, the choice was: wire it up (real fix) *or* delete it as superseded (cleanup),
either being acceptable. That framing assumed the two systems merely disagreed about
*who owns* chunk sizing. The arithmetic above says they disagree about *what is safe*,
and production is on the unsafe side by 1.84× for the default model.

If the embedder truncates silently, **"delete as superseded" is the wrong direction** —
it would remove the only artefact in the tree that records the safe bound, while leaving
production over it. The cleanup would look like tidying and would be the opposite.

The three options, re-framed:

1. **Wire it through, keeping the env override.** `sync_project`'s `chunk_target`
   resolves via `EmbeddingsSection::effective_chunk_size()`, so the default model gets
   652 and an 8192-token model gets ~20 889. Correct by construction, and it is what the
   dead code was built for. Cost: chunk sizes change for every existing index — a
   full `index(force=true)` reindex, and the retrieval benchmark should be re-run,
   because 1200 was itself a *measured* choice (`docs/research/2026-05-06-retrieval-stack-benchmark.md`,
   the chunk×model matrix).
2. **Keep 1200 and delete the model-aware path** — valid *only* if truncation is proven
   not to happen, or is proven acceptable. Then 1200 is a deliberate,
   benchmark-backed constant and the dead code is genuinely superseded. Record the
   truncation measurement in the deleting commit so the next reader does not re-derive
   this whole question.
3. **Cap at the model budget, keep 1200 as the ceiling.** `chunk_target =
   STACK_CHUNK_TARGET.min(effective_chunk_size())`. Smallest behavioural change that
   removes the over-budget case; still changes chunk sizes for the default model, so it
   carries option 1's reindex cost without option 1's benefit for large-context models.

Unchanged from the original filing: leaving both systems live and disconnected is the
actual defect.
## Tests added

N/A — no fix attempted; filing only, per explicit instruction not to fix this out-of-scope finding on this branch.

## Workarounds

None needed for correctness — indexing still runs; the chunk size is just not model-tuned. Operators who want a different chunk size can already reach for `CODESCOUT_CHUNK_TARGET`, which works today — it just isn't what `effective_chunk_size()` would have computed for the configured model.

## Resume

**Do not start by choosing between wire-it-up and delete-it.** Answer one question
first, because it decides which of those is correct:

> Does the embedder silently truncate input above the model's max sequence length?

Build with `--features local-embed`, embed a string comfortably over 256 tokens and one
under it that shares a prefix, and compare the vectors. If the over-length vector equals
the truncated-prefix vector, truncation is silent and confirmed. `crates/codescout-embed`
has no `max_length` handling of its own, so this is `fastembed`'s behaviour — read its
tokenizer config or measure it; do not infer it.

With that answered:

- **truncates silently** → this is a retrieval-quality bug, not dead code. Option 1 or 3
  under *Fix*. Deleting the model-aware path becomes the wrong move.
- **does not truncate** (pads, errors, or handles long input) → option 2 is available and
  1200 stands on its benchmark evidence. Record the measurement in the commit.

Either way the premise is already verified and needs no re-checking:
`effective_chunk_size` has zero production callers, `chunk_size_for_model` returns
characters via `n × 0.85 × 3.0`, and 652 vs 1200 is the gap.
## References

- `src/config/project.rs:350-357` — `EmbeddingsSection::effective_chunk_size`
- `crates/codescout-embed/src/lib.rs:68-144` — `chunk_size_for_model`
- `src/retrieval/sync.rs:256-263` — `STACK_CHUNK_TARGET`, `CODESCOUT_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:953,974-976` — `AST_CHUNK_TARGET`, `split_file`'s use as ceiling
- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/` — Task 4's F2 fix (branch's headline silent-correctness bug, guarding this dead-on-production-path function)
