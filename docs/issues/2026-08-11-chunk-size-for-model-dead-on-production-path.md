---
id: '7438a064c8d61f86'
kind: bug
status: open
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
severity: medium
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

## Hypotheses tried

None — found via static tracing (`references`), not runtime measurement. Filed for tracking per explicit instruction not to attempt a fix on this branch (pre-existing, out of scope for `feat/local-onnx-query-path`).

## Fix

Not implemented — deliberately out of scope for this branch. Candidate direction for a future task: either wire `sync_project`'s `chunk_target` through `EmbeddingsSection::effective_chunk_size()` (model-aware, still env-overridable), or remove `chunk_size_for_model`/`effective_chunk_size` if the fixed `STACK_CHUNK_TARGET`/`AST_CHUNK_TARGET` scheme is the intended permanent design and the model-aware path was superseded without being deleted.

## Tests added

N/A — no fix attempted; filing only, per explicit instruction not to fix this out-of-scope finding on this branch.

## Workarounds

None needed for correctness — indexing still runs; the chunk size is just not model-tuned. Operators who want a different chunk size can already reach for `CODESCOUT_CHUNK_TARGET`, which works today — it just isn't what `effective_chunk_size()` would have computed for the configured model.

## Resume

Decide, with the codebase owner, whether `chunk_size_for_model`/`effective_chunk_size` should be wired into `sync_project` (real fix) or deleted as superseded dead code (cleanup). Either resolves the drift; leaving both systems live and disconnected is the actual defect.

## References

- `src/config/project.rs:350-357` — `EmbeddingsSection::effective_chunk_size`
- `crates/codescout-embed/src/lib.rs:68-144` — `chunk_size_for_model`
- `src/retrieval/sync.rs:256-263` — `STACK_CHUNK_TARGET`, `CODESCOUT_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:953,974-976` — `AST_CHUNK_TARGET`, `split_file`'s use as ceiling
- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/` — Task 4's F2 fix (branch's headline silent-correctness bug, guarding this dead-on-production-path function)

