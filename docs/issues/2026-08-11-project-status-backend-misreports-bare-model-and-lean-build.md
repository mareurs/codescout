---
id: c7ba92f5bb519482
kind: bug
status: open
title: 'BUG: ProjectStatus''s embedding backend classifier misreports two configs that backend_is_local doesn''t recognize'
tags:
- embedding
- status
- diagnostics
- retrieval
---

---
status: open
opened: 2026-08-11
closed:
severity: low
owner: marius
related: []
tags:
  - embedding
  - status
  - diagnostics
  - retrieval
kind: bug
---

# BUG: ProjectStatus's embedding backend classifier misreports two configs that backend_is_local doesn't recognize

## Summary
`ProjectStatus`'s backend classifier (`src/tools/config/mod.rs`) routes through
`RetrievalClient::backend_is_local`, which recognizes only `local:`/`local-dir:`
model prefixes. Two real configurations fall outside that recognition and get
the wrong `embedding_backend` string in `workspace(action="status")` output.
Status-string only — neither changes which embedder a query actually uses.

## Symptom (Effect)
1. **Bare unprefixed model name, `local-embed` compiled in.**
   `create_embedder_with_config` arm 6 (`crates/codescout-embed/src/lib.rs:265-271`)
   successfully resolves a bare model name (no `local:`/`local-dir:`/`ollama:`/
   `openai:`/`custom:` prefix) as a local ONNX model via `LocalEmbedder::new(model)`
   — the embedder actually constructed IS local. But `backend_is_local`
   (`src/retrieval/client.rs:37-40`) only tests
   `config.model.starts_with("local:") || config.model.starts_with("local-dir:")`,
   so a bare name never matches, and `ProjectStatus` reports
   `"embedding_backend": "remote-http"` for a config that is actually running
   the local ONNX backend.
2. **`--no-default-features` build, `ollama:`/`openai:` model, no url.** In this
   CI matrix row (`.github/workflows/ci.yml:74` and `:76`), `remote-embed` is
   NOT compiled (it's a default feature — `Cargo.toml:175`,
   `default = ["remote-embed", "http", "librarian"]` — disabled by
   `--no-default-features`), so `create_embedder_with_config`'s arms 3/4 (the
   `ollama:`/`openai:` prefixes) don't even compile in; construction falls
   through to the final `anyhow::bail!("Unknown model ...")` — nothing can
   actually be built. But `backend_is_local` returns `false` for an
   `ollama:`/`openai:`-prefixed model regardless of compiled features (it only
   checks the model-string prefix, never which features are compiled), so
   `ProjectStatus` falls into its `else` branch and reports
   `"embedding_backend": "remote-http"` — as if the config would work.

Both are status-string-only: the actual embedder construction (which
determines what `semantic_search`/`memory(recall)` really do) goes through
`create_embedder_with_config` directly, never through `backend_is_local`.

## Reproduction
- Case 1: build with `local-embed` compiled in, set
  `[embeddings].model = "AllMiniLML6V2Q"` (no `local:` prefix), no `url`. Call
  `workspace(action="status")` → `embedding_backend` reads `"remote-http"`,
  even though `create_embedder_with_config` would build a `LocalEmbedder` for
  that exact model string.
- Case 2: build with `cargo build --no-default-features` (or
  `--features local-embed --no-default-features`), set
  `[embeddings].model = "ollama:nomic-embed-text"`, no `url`. Call
  `workspace(action="status")` → `embedding_backend` reads `"remote-http"`,
  even though `create_embedder_with_config` has no compiled path that can
  build anything for that model (arms 3/4 need `remote-embed`, which is off)
  and would `bail!` with "Unknown model" if actually invoked.

Not run end-to-end in this session — derived from reading the two functions
against each other (see Root cause).

## Environment
Branch `feat/local-onnx-query-path`, commit `4962126ee10947e5b874bd64949b8e75c7078ca3`
at time of filing. Applies to any codescout build; case 2 is specifically a
`--no-default-features` build (a real CI matrix row, not hypothetical).

## Root cause
`ProjectStatus::call`'s embedding-backend section (`src/tools/config/mod.rs:297-351`)
derives `embedding_backend` from `RetrievalClient::backend_is_local(&retrieval_config)`
(`src/retrieval/client.rs:37-40`), which is a **narrower** recognizer than
`create_embedder_with_config`'s actual resolution ladder
(`crates/codescout-embed/src/lib.rs:154-292`; doc comment at `:154-162` lists
all 6 arms). `backend_is_local` only tests `local:`/`local-dir:` prefixes; it
has no branch for arm 6 (bare name resolves local when `local-embed` is
compiled), and it never reads compiled features at all (so it can't detect
that arms 3/4 are unreachable in a `--no-default-features` build).

**Why the obvious fix is wrong.** The natural first instinct is to patch this
inside `ProjectStatus` — e.g. special-case a bare model name, or check
`cfg!(feature = "remote-embed")` before trusting an `ollama:`/`openai:`
prefix. That would re-create exactly the class of bug that finding I1 (the
review that made `backend_is_local` the single source of truth — see the
comment at `src/tools/config/mod.rs:317-326` and the regression test
`status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`,
`src/tools/config/tests.rs:356-418`) removed: two independent places computing
"is this local" that can silently drift apart. The correct fix is inside
`backend_is_local` itself (`src/retrieval/client.rs:37-40`) — but that
function is not status-reporting-only. `guard_sparse`
(`src/retrieval/client.rs:47-59`, reads it at line 48) and `dense_only`
(`src/retrieval/client.rs:67-69`, reads it at line 68) both consult it to
decide whether the **live query path** runs dense-only and whether the hybrid
sparse leg is guarded. Widening `backend_is_local` to recognize a bare local
model name, or to consult compiled features, changes what those two functions
decide for real queries — strictly riskier than a status-string residual,
since a mistake there is a live behavior regression, not a diagnostic
misprint. This file exists to record the residual rather than "fix" it in
either the wrong (status-local) or risky (shared-classifier) place.

*Inferred from `crates/codescout-embed/src/lib.rs:154-292` and
`src/retrieval/client.rs:37-69` — not runtime-measured beyond the manual
repro steps above; no test executed for case 1/2 in this session.*

## Evidence
### `create_embedder_with_config`'s resolution order (doc comment)
`crates/codescout-embed/src/lib.rs:154-162`:
```
/// Resolution order:
/// 1. `url` set → RemoteEmbedder targeting that URL
/// 2. `model` starts with `local-dir:` → local ONNX loaded from a directory, no network;
///    or starts with `local:` → local ONNX via fastembed
/// 3. `model` starts with `ollama:` → Ollama (errors loudly if unreachable)
/// 4. `model` starts with `openai:` → OpenAI API
/// 5. `model` starts with `custom:` → hard error with migration hint
/// 6. No url, no known prefix → try `model` as a bare local model name; else
///    the unknown-model error (there is no silent default)
```

### Arm 6 — bare name resolves local
`crates/codescout-embed/src/lib.rs:265-271`:
```rust
// 6. No prefix — try as local model name
#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
{
    // Try parsing as a local model name directly
    if local::LocalEmbedder::new(model).await.is_ok() {
        return Ok(Box::new(local::LocalEmbedder::new(model).await?));
    }
}
```

### `backend_is_local` — narrower recognizer, shared with the live query path
`src/retrieval/client.rs:37-40`:
```rust
pub(crate) fn backend_is_local(config: &RetrievalConfig) -> bool {
    config.embedder_url.is_none()
        && (config.model.starts_with("local:") || config.model.starts_with("local-dir:"))
}
```
No branch for a bare name, no branch reading compiled features. Called by
`guard_sparse` (`:48`) and `dense_only` (`:68`), which decide real
query-path behavior — this is why widening it is riskier than the residual.

### CI's `--no-default-features` matrix rows
`.github/workflows/ci.yml:71-76`:
```yaml
- name: default
  flags: ""
- name: local-embed
  flags: "--features local-embed --no-default-features"
- name: no-features
  flags: "--no-default-features"
```
Both the `local-embed` row (`:74`) and the `no-features` row (`:76`) disable
the default `remote-embed` feature (`Cargo.toml:175`).

## Hypotheses tried
N/A — root cause fully determined by reading the resolution ladder against
`backend_is_local`; no dead ends worked in this session.

## Fix
Not implemented. Recorded as a known residual — see Root cause § *Why the
obvious fix is wrong* for why neither the easy (status-local) nor the
shared-classifier fix is being applied here. Two real options if this is ever
prioritized:
- Teach `backend_is_local` about arm 6 and compiled features, and add
  regression tests proving `guard_sparse`/`dense_only` live-query behavior is
  unchanged for every existing case (extend the `selection_tests` module,
  `src/retrieval/client.rs:409-664`).
- Or give `ProjectStatus` its own richer classifier that mirrors
  `create_embedder_with_config`'s full ladder (including compiled-feature
  awareness) without going through `backend_is_local` at all — accepting the
  "two places to keep in sync" risk finding I1 removed, in exchange for
  isolating the blast radius to the status tool.

## Tests added
None — this file documents a known, deliberately-unfixed residual (see Root
cause). No regression test exists for either case. `selection_tests`
(`src/retrieval/client.rs:409-664`) and
`status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`
(`src/tools/config/tests.rs:356-418`) are existing tests cited above as
evidence of the adjacent, already-correct contract — not new tests for this
bug.

## Workarounds
None needed — no query-path effect. If the reported `embedding_backend`
string is confusing, cross-check it against `embedding_compiled_in` and the
actual `[embeddings].model` value in `.codescout/project.toml`.

## Resume
If picked up: start from the "Fix" section's two options. Before touching
`backend_is_local`, run
`references(symbol="backend_is_local", path="src/retrieval/client.rs")` to
get the full call-site list, and re-read `guard_sparse`/`dense_only`'s doc
comments (`src/retrieval/client.rs:47-59`, `:67-69`) — both explicitly
document that they rely on `backend_is_local` being the single source of
truth for "does this backend need dense-only / does it produce sparse". Any
widening must keep both callers' invariants intact, verified by the existing
`selection_tests` module.

## References
- `src/tools/config/mod.rs:251-455` — `ProjectStatus::call`; embedding backend
  section at `:297-351`
- `src/retrieval/client.rs:37-69` — `backend_is_local`, `guard_sparse`, `dense_only`
- `crates/codescout-embed/src/lib.rs:154-292` — `create_embedder_with_config`,
  the full resolution ladder
- `.github/workflows/ci.yml:71-76` — the `--no-default-features` CI matrix rows
- `src/tools/config/tests.rs:356-418` —
  `status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends`,
  the sibling test covering the adjacent (already-fixed) misreport this
  residual sits next to
- `docs/superpowers/plans/2026-08-11-local-onnx-embedding-query-path.md` —
  the plan that introduced `backend_is_local` as the shared classifier

