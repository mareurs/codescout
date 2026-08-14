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

### 2026-08-14 — the truncation question, measured

Answered by reading the dependency and the model files on disk, no `local-embed` build
needed (that build is blocked on this host anyway —
`docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`).

**Does fastembed truncate silently? Yes — but at 512 tokens, not 256.**

The chain, each link read:

| Step | Location | Value |
|---|---|---|
| codescout builds options | `crates/codescout-embed/src/local.rs:153` | `InitOptions::new(model)` — **never calls `with_max_length`** |
| fastembed's per-model constant | `fastembed-5.13.4/src/text_embedding/init.rs:15-17` | `impl HasMaxLength for EmbeddingModel { const MAX_LENGTH = DEFAULT_MAX_LENGTH }` |
| that default | `fastembed-5.13.4/src/text_embedding/mod.rs:6` | `DEFAULT_MAX_LENGTH = 512` |
| clamp against the tokenizer | `fastembed-5.13.4/src/common.rs:91` | `max_length.min(model_max_length)` |
| the model's own value | `~/.cache/huggingface/hub/models--sentence-transformers--all-MiniLM-L6-v2/…/tokenizer_config.json` | `"model_max_length": 512` |
| the truncation itself | `fastembed-5.13.4/src/common.rs:106-109` | `with_truncation(Some(TruncationParams { max_length, .. }))` — truncates, does not error |

Effective truncation point: `min(512, 512)` = **512 tokens**.

Production chunks are 1200 chars ≈ 300–400 tokens at the 3–4 chars/token range the
`chunk_size_for_model` doc comment itself uses. **300–400 < 512, so no truncation occurs.**
The silent-data-loss hypothesis is **refuted**.

**Two different "max sequence length" values exist for this model, and both are real:**

```
tokenizer_config.json      "model_max_length": 512   ← fastembed reads this
sentence_bert_config.json  "max_seq_length":  256   ← codescout's table uses this
```

fastembed never reads `sentence_bert_config.json`. So codescout's 256 is the
sentence-transformers *tuned* sequence length (a quality boundary) while 512 is the hard
truncation point. Production at ~300–400 tokens sits **between** them: above the tuned
regime, below any data loss.
## Hypotheses tried

None — found via static tracing (`references`), not runtime measurement. Filed for tracking per explicit instruction not to attempt a fix on this branch (pre-existing, out of scope for `feat/local-onnx-query-path`).

## Fix

Still not implemented. The measurement changed which option is correct — **and it is not
the one this file has recommended twice.**

### Wiring it up would be actively harmful

`HasMaxLength for EmbeddingModel` sets `MAX_LENGTH = DEFAULT_MAX_LENGTH = 512` for
**every** model — fastembed does not vary its default by model. Combined with
`max_length.min(model_max_length)`, any fastembed-hosted local model truncates at **at
most 512 tokens**, whatever context length the model advertises.

Now compare what `chunk_size_for_model` would hand it:

| Model | `chunk_size_for_model` | ≈ tokens | fastembed truncates at | Discarded |
|---|---|---|---|---|
| `local:AllMiniLML6V2Q` | 652 chars | ~200 | 512 | none |
| `local:BGESmallENV15Q` | 1305 chars | ~400 | 512 | none |
| `local:NomicEmbedTextV15Q` | **20 889 chars** | ~6 500 | **512** | **~92 %** |
| `local:JinaEmbeddingsV2BaseCode` | **20 889 chars** | ~6 500 | **512** | **~92 %** |

So **option 1 (wire `sync_project` through `effective_chunk_size`) would introduce the
very bug this file feared**, for exactly the models it was meant to serve: 20 889-char
chunks against a 512-token tokenizer means roughly nine tenths of each chunk embedded as
nothing. Today's fixed 1200 accidentally protects against that.

`chunk_size_for_model` is not wrong in general — for `openai:` / `ollama:` / bare-name
remote endpoints there is no fastembed tokenizer in the path and 8192 is genuinely
available. It is wrong **for the `local:` arm**, which is the arm feeding the only
backend that truncates.

### Revised options

1. ~~**Wire it through as-is.**~~ **Rejected on evidence** — see the table. Would
   over-chunk large-context local models by ~13× against a hard 512-token ceiling.
2. **Keep 1200; delete `effective_chunk_size` and narrow `chunk_size_for_model`.** Now
   the defensible default. 1200 is benchmark-backed
   (`docs/research/2026-05-06-retrieval-stack-benchmark.md`, the chunk×model matrix), sits
   safely under 512 tokens for every local model, and no production caller is lost
   because there are none. If `chunk_size_for_model` is kept for the remote arms, its
   `local:` table needs a `.min(512-token equivalent)` and a comment naming fastembed's
   uniform default as the reason.
3. **Wire it through *and* clamp to fastembed's ceiling.** `chunk_target =
   STACK_CHUNK_TARGET.min(effective_chunk_size()).min(FASTEMBED_MAX_CHARS)`. Most correct
   in principle; buys nothing today, since 1200 is already under every local model's
   ceiling. Only pays off if codescout ever passes `with_max_length` to raise fastembed's
   512.
4. **Raise fastembed's `max_length` deliberately.** Independent of this bug and worth its
   own decision: codescout could call `with_max_length` to use a large-context local
   model's real window instead of 512. That would make option 1 or 3 meaningful. It is a
   capability change, not a fix.

### The quality question that remains open

1200 chars ≈ 300–400 tokens is above `all-MiniLM-L6-v2`'s tuned `max_seq_length` of 256
though below its 512 hard limit. Whether that costs retrieval quality is measurable with
the existing benchmark and is *not* a correctness issue — do not conflate the two. The
chunk×model matrix already chose 1200 empirically, which is evidence, if indirect, that
it does not hurt on this corpus.
## Tests added

N/A — no fix attempted; filing only, per explicit instruction not to fix this out-of-scope finding on this branch.

## Workarounds

None needed for correctness — indexing still runs; the chunk size is just not model-tuned. Operators who want a different chunk size can already reach for `CODESCOUT_CHUNK_TARGET`, which works today — it just isn't what `effective_chunk_size()` would have computed for the configured model.

## Resume

**The measurement is done. Do not re-run it.** fastembed truncates silently at
`min(DEFAULT_MAX_LENGTH=512, tokenizer_config.model_max_length)`, which is 512 tokens for
`all-MiniLM-L6-v2`. Production's 1200-char chunks are ~300–400 tokens, so nothing is
truncated today. Read from `fastembed-5.13.4/src/common.rs:91,106` and
`text_embedding/{init.rs:15-17,mod.rs:6}`, plus `model_max_length: 512` and
`max_seq_length: 256` in the cached model's own config files.

**What remains is a scoping decision, and option 1 is now off the table** — wiring
`effective_chunk_size` into `sync_project` unchanged would over-chunk
`NomicEmbedTextV15Q` / `JinaEmbeddingsV2BaseCode` by ~13× against fastembed's uniform
512-token ceiling, discarding ~92 % of each chunk. Option 2 (keep 1200, delete the dead
path, narrow the `local:` table) is the defensible default; option 3 if you want the
belt-and-braces clamp; option 4 is a separate capability question.

Whatever lands, put fastembed's uniform `DEFAULT_MAX_LENGTH = 512` in a comment next to
any chunk-size constant. It is the non-obvious fact that makes a model's advertised 8192
context irrelevant on the local path, and it is not discoverable from codescout's source.
## References

- `src/config/project.rs:350-357` — `EmbeddingsSection::effective_chunk_size`
- `crates/codescout-embed/src/lib.rs:68-144` — `chunk_size_for_model`
- `src/retrieval/sync.rs:256-263` — `STACK_CHUNK_TARGET`, `CODESCOUT_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:953,974-976` — `AST_CHUNK_TARGET`, `split_file`'s use as ceiling
- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/` — Task 4's F2 fix (branch's headline silent-correctness bug, guarding this dead-on-production-path function)
