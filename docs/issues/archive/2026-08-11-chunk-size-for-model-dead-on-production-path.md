---
id: d2072751251dbf3e
kind: bug
status: fixed
title: 'BUG: chunk_size_for_model''s model-derived chunk budget is dead code on the production indexing path'
tags:
- chunking
- embeddings
- dead-code
- retrieval
closed: 2026-08-14
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

**Option 2 implemented 2026-08-14 on `experiments`.**

1. **`EmbeddingsSection::effective_chunk_size` deleted** (`src/config/project.rs`) — zero
   production callers, and the budget it computed was not one the local embedder honours.
2. **`[embeddings].chunk_size` retired to ignored-for-compat**, renamed
   `_chunk_size_ignored` with `#[serde(default, skip_serializing, rename = "chunk_size")]`,
   following the precedent already in the same struct for `chunk_overlap`. Deleting the
   field outright would have turned a silently-ignored key into a **hard parse error** on
   upgrade for anyone whose `project.toml` carries it; `skip_serializing` also stops a dead
   knob reappearing in files codescout generates.
3. **`chunk_size_for_model`'s `local:` arm clamped** to
   `FASTEMBED_DEFAULT_MAX_TOKENS = 512` (`crates/codescout-embed/src/lib.rs`), with the
   mechanism and its source lines in a comment. The function itself is kept: it is correct
   for the `openai:` / `ollama:` / bare-name arms, where no fastembed tokenizer is in the
   path.
4. `STACK_CHUNK_TARGET = 1200` unchanged — benchmark-backed, and under every local model's
   real ceiling.

### Correction to this file's own analysis

The previous revision claimed wiring option 1 would hand **20 889-char chunks** to a
512-token tokenizer and discard **~92 %**. **That was wrong, and it was mine.** It read
`chunk_size_for_model`'s raw output and attributed it to `effective_chunk_size`, which
applied its own `DEFAULT_CAP = 4096` — a cap whose doc comment gives exactly this reason
(*"nomic-embed, jina, bge-m3 would otherwise default to ~20k chars per chunk, which both
slows indexing and dilutes ranking signal"*). The author had already anticipated
over-chunking.

Corrected:

| | chars offered | ≈ tokens | fastembed ceiling | discarded |
|---|---|---|---|---|
| pre-fix `effective_chunk_size()`, 8192-token local model | 4096 | ~1365 | 512 (~1536 chars) | **~62 %** |

The conclusion holds — option 1 was still the wrong direction, and `DEFAULT_CAP` mitigated
without eliminating the problem — but the magnitude was overstated by 30 points. The
reasoning error is the instructive part: the table was built from the function named in
the *evidence* rather than the one named in the *option*.

### Still open, deliberately: raising fastembed's ceiling

codescout *could* call `with_max_length` in `local.rs` to give a large-context local model
its real window instead of 512. That would make a model-aware chunk budget meaningful
again and would be the point to reconsider a per-model `chunk_target`. A capability
change, not a bug fix; nothing currently depends on it.

### The quality question that remains open

1200 chars ≈ 300–400 tokens is above `all-MiniLM-L6-v2`'s tuned `max_seq_length` of 256
though below its 512 hard limit. Whether that costs retrieval quality is measurable with
the existing chunk×model benchmark and is **not** a correctness issue — do not conflate the
two. That benchmark already chose 1200 empirically, which is indirect evidence it does not
hurt on this corpus.
## Tests added

**`local_models_are_clamped_to_fastembeds_actual_token_ceiling`**
(`crates/codescout-embed/src/lib.rs`) — the three 8192-token `local:` entries must all
collapse to the 512-token budget (1305 chars); `AllMiniLML6V2Q` must **keep** its smaller
652 (the clamp is a maximum, not a floor that inflates small models); and
`ollama:nomic-embed-text` must stay **above** the ceiling, proving the clamp is scoped to
the `local:` arm. That last assertion is the one that catches a clamp applied too broadly —
the failure mode of this fix.

**`a_retained_chunk_size_key_still_deserialises_and_is_ignored`**
(`src/config/project.rs`) — replaces the four deleted tests with the only contract still
owed: an upgrade must not turn a silently-ignored key into a parse error. Also asserts the
key is not re-emitted on serialise.

**Removed:** `effective_chunk_size_none_uses_model_max`, `_user_value_below_cap_honored`,
`_user_value_above_cap_clamped`, `_zero_falls_back_to_model_max`. All four exercised a
function no production path called — four green tests over a disconnected component, which
is what let the drift persist unnoticed. `project_config_chunk_size_round_trip` was kept
and repointed at the ignored field; it tests deserialisation compatibility, which is
exactly what was preserved.

Gate: **3718 passed / 0 failed / 44 ignored** root-only (3721 − 4 removed + 1 added,
reconciling exactly), **3751 / 49** with `--workspace`, `clippy --workspace --all-targets
-D warnings` clean.
## Workarounds

None needed for correctness — indexing still runs; the chunk size is just not model-tuned. Operators who want a different chunk size can already reach for `CODESCOUT_CHUNK_TARGET`, which works today — it just isn't what `effective_chunk_size()` would have computed for the configured model.

## Resume

N/A — fixed and verified.

One fact a later session should not re-derive: **fastembed caps every `local:` model at
512 tokens.** `HasMaxLength for EmbeddingModel` returns `DEFAULT_MAX_LENGTH` regardless of
model (`fastembed-5.13.4/src/text_embedding/{init.rs:15-17,mod.rs:6}`), clamped again
against `tokenizer_config.model_max_length` (`common.rs:91`) and applied as silent
truncation (`common.rs:106`); `local.rs` never calls `with_max_length`. It is now recorded
in a comment beside the clamp, which is where it will actually be read.
## References

- `src/config/project.rs:350-357` — `EmbeddingsSection::effective_chunk_size`
- `crates/codescout-embed/src/lib.rs:68-144` — `chunk_size_for_model`
- `src/retrieval/sync.rs:256-263` — `STACK_CHUNK_TARGET`, `CODESCOUT_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:953,974-976` — `AST_CHUNK_TARGET`, `split_file`'s use as ceiling
- `.superpowers/sdd/2026-08-11-local-onnx-embedding-query-path/` — Task 4's F2 fix (branch's headline silent-correctness bug, guarding this dead-on-production-path function)
