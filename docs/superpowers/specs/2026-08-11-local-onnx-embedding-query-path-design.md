---
id: '04ec1893cec034d3'
kind: spec
status: active
title: Design — Local ONNX embedding reaches the query path
owners:
- marius
tags:
- embeddings
- retrieval
- codescout-embed
- low-resource
- offline
- architecture
- cross-crate-boundary
topic: embedding-backend-selection
---

# Design — Local ONNX embedding reaches the query path

**Supersedes:** the spec and plan that live on branch `feat/local-onnx-embedder`
(PR #13) — 2026-08-08-local-onnx-embedder-design.md and
docs/superpowers/plans/2026-08-08-local-onnx-embedder.md. Those two paths, and
the three bug files named under § References, exist only on that branch and not
on `experiments`; they are written without code spans deliberately, because
`audit_doc_refs` resolves refs against the current checkout and would read a
code-spanned branch-only path as a stale link. That work is partially salvaged;
see § What survives from PR #13.

**Related ADR:** `docs/adrs/2026-07-25-embedding-transport-boundary.md`
(`codescout-embed` owns remote embedding transport). This design honours its
boundary for *selection* and explicitly defers its *transport* migration.

## Goal

A user with no GPU, no embedding server, and no budget can run
`semantic_search` and `memory(recall)` against a local ONNX AllMiniLM model —
including on a host with no network reachability at all. Restricted
environments (corporate VDI, air-gapped) are the hardest case this must
satisfy, not the only one it targets.

## Non-goals

- Prebuilt release binaries. That is a separate sub-project with its own spec;
  this one delivers the capability a release lane would later ship. The
  ordering is forced — publishing binaries for a capability the query path
  cannot select would compile the current contradiction into an artifact.
- Retiring `EmbedderHttp`. That is the ADR's transport migration, deferred here
  on purpose (§ Deferred).
- Changing default cargo features. `local-embed` stays opt-in; the honest
  reporting in Unit 7 is how a lean binary stops lying about it.

## Context — three facts that shape everything

**1. The resolver already exists, and retrieval never calls it.**
`create_embedder_with_config` (`crates/codescout-embed/src/lib.rs:125`) already
resolves `local:` → fastembed ONNX, plus `ollama:`, `openai:`, and an explicit
url, defaulting to `local:AllMiniLML6V2Q`. `src/librarian/mod.rs:98` and `:367`
call it. `grep codescout_embed:: src/` returns 22 hits across librarian, embed,
and config — and **zero** in `src/retrieval/`. Retrieval has its own selection:
`RetrievalConfig.embedder_url` → `EmbedderHttp`, HTTP only.

**2. The default config names a model the default binary cannot load.**
`src/config/project.rs:385` makes `local:AllMiniLML6V2Q` the default embedding
model. `default = ["remote-embed", "http", "librarian"]` excludes every local
backend, so on a default build each `local:` branch of the resolver is
`#[cfg]`'d out and the call falls through to
`"Local embedding requires the 'local-embed' feature."` A fresh clone has no
working embedder unless the user sets a url.

**3. The session ownership already lives in the crate.**
`crates/codescout-embed/src/local.rs:13` holds
`Arc<Mutex<fastembed::TextEmbedding>>`; `:82` carries the comment
*"fastembed 5 changed embed() to &mut self — Mutex serializes access across
spawn_blocking tasks"*, which is the exact constraint PR #13's plan got wrong.
`Cargo.toml:115` states *"HTTP client and local ONNX embeddings are now
provided by the codescout-embed crate."*

## Decision

**Decision:** `src/retrieval` selects its dense embedder through
`codescout_embed::create_embedder_with_config` whenever no url is configured,
and `codescout-embed` gains directory-loaded weights. `EmbedderHttp` remains
the url-path implementation, unchanged.

**Context:** one config grammar must serve both indexing and search for the
capability to be usable; the crate already owns the ONNX session; the ADR
already assigns local ONNX to the crate.

**Alternatives considered:**

- *Add a local branch to retrieval only* (PR #13's Task 4). Rejected:
  `RetrievalConfig` has no `model` field, so this requires inventing a second
  selection grammar. `[embeddings] model` would keep working for the librarian
  and not for search — the first thing a new low-resource user hits.
- *Route all backends through the resolver.* Rejected for this spec: the
  resolver returns `RemoteEmbedder` when a url is present, which silently
  retires `EmbedderHttp` and drags in ADR contracts 1 and 3. That is a
  migration, and it deserves its own review.
- *A new `LocalOnnxEmbedder` type in the root crate* (PR #13 as built).
  Rejected: adds `fastembed` as a direct root dependency 26 lines above the
  comment assigning local ONNX to the crate, and lands in a file no test lane
  compiles.

**Consequences:**

- now easier: `[embeddings] model` becomes the single source of truth for
  indexing and search; the local backend fixes `memory(recall)` for free
  through the shared instance; new local code lands in a crate three CI lanes
  already compile.
- now harder: retrieval's config gains a project-config input alongside env;
  two dense implementations coexist behind one grammar until the ADR's
  transport stage lands.

**Change scenarios absorbed:** swap the local ONNX backend; add a second
weights source; add an embedding provider (one resolver, not two); build with
no local backend (one feature edge).

**Revisit-when:** the url path moves to the resolver (then contract 3's
three-state prefix becomes mandatory); librarian needs directory-loaded
weights (then `local-dir:` belongs in its documented grammar too).

**Confidence:** high on the boundary; medium on deferring the transport
migration rather than paying it once.

## Architecture

`codescout-embed` owns *how to make an embedder*. `src/retrieval` owns *what an
embedding means here* — `EmbedOutput`, `SparseVector`, the dim contract, the
hybrid query path.

```
[embeddings] model/url/api_key  ──┐
                                  ├──► selection ──► Arc<dyn CodeEmbedder>
CODESCOUT_EMBEDDER_* env vars   ──┘   (override)

url present?  ── yes ──►  EmbedderHttp                      (root, unchanged)
              └─ no  ──►  create_embedder_with_config(model, None, api_key)
                            ├─ "local:<Variant>"  ► LocalEmbedder::new
                            └─ "local-dir:<path>" ► LocalEmbedder::from_dir
                          └─► CodeEmbedderAdapter { inner, expected_dim }
```

**Config precedence.** `[embeddings]` in the project's .codescout/project.toml
is the base (un-spanned: .codescout/ is gitignored, so a code span would resolve
in a working tree and fail in a clean checkout — see R-58);
`CODESCOUT_EMBEDDER_*` env vars override it. Benchmark matrix cells keep
working unchanged (they set env); the documented config finally reaches search.

**The `8081` default is removed.** `embedder_url` currently defaults to
`http://127.0.0.1:8081` when unset (`src/retrieval/config.rs:69`) — a
fabricated URL for a server that may not exist. An *unset* url now means
"resolve from the model" rather than "assume 8081". An explicitly-set
`CODESCOUT_EMBEDDER_URL` is untouched. Users who ran a server at 8081 without
setting the variable will switch to the local backend and fail **loudly** on
the dim check (768 configured vs 384 produced), not silently.

**Local implies dense-only.** A local ONNX backend produces no sparse vector.
Selecting one sets `dense_only`, a third reason beside `lite` and
`disable_sparse`. `server-stack` compiled in *and* sparse expected *and* a
local backend selected is an error, not a downgrade.

## Components

| # | Unit | File | Responsibility |
|---|---|---|---|
| 1 | `LocalEmbedder::from_dir(dir)` | `crates/codescout-embed/src/local.rs` | Load weights from a directory via fastembed's `try_new_from_user_defined` — no hf-hub, no network. Reuses the existing `Arc<Mutex<TextEmbedding>>` + `spawn_blocking` discipline and probe-based dim discovery. |
| 2 | `local-dir:<path>` grammar | `crates/codescout-embed/src/lib.rs` | One arm in `create_embedder_with_config`, beside `local:` / `ollama:` / `openai:`. Unknown-model error text gains the new form. |
| 3 | `CodeEmbedderAdapter` | `src/retrieval/embedder.rs` | Wraps `Box<dyn Embedder>` into `BatchEmbedder` + `CodeEmbedder`. Dense-only: `sparse` always empty. Holds `expected_dim`, raises the dim-mismatch error. |
| 4 | Config merge | `src/retrieval/config.rs` | `RetrievalConfig` gains `model` and `api_key`; `embedder_url` becomes `Option<String>`. Construction takes an optional `&EmbeddingsSection` base with env override. `from_env()` keeps working for callers with no project. |
| 5 | Selection branch | `src/retrieval/client.rs` | The diagram above; `dense_only = lite \|\| disable_sparse \|\| backend_is_local`; the sparse-expected error. |
| 6 | `CodeVectorStore` dim read-back | `src/retrieval/code_store.rs` | Report an existing collection's dense dimension (`None` when the collection does not exist). **No default method** — all five implementors answer explicitly so the compiler enforces coverage: `QdrantWrap` (`:131`), `SqliteVecCodeStore` (`src/retrieval/sqlite_code_store.rs:141`), and the test doubles `InMemoryCodeStore` (`:238`), `RecordingStore` (`src/retrieval/sync.rs:433`), `SlowEnsureStore` (`:842`). |
| 7 | Honest reporting | `workspace(status)` | Which backends are *compiled in* and which is *live*. Without it, a user on a lean binary reads `local:AllMiniLML6V2Q` in their config with no way to learn the binary cannot honour it. |
| 8 | Manual page | `docs/manual/src/` | The low-resource path: which feature to build with, where weights come from, the offline directory layout. |

## Data flow

**Selection** happens once, at client construction, and yields one
`Arc<dyn CodeEmbedder>` shared by search, indexing, and memory recall.

**Index path.** `sync_project` hands the trait object to the chunk pipeline,
which calls `BatchEmbedder::embed_batch_dyn`. The adapter forwards to
`Embedder::embed` and wraps each vector as `EmbedOutput { dense, sparse: ∅ }`.

**Query path.** `semantic_search` → `CodeEmbedder::embed_one` → adapter →
`Embedder::embed`. The dim check fires on the first non-empty result.

**Memory recall.** Reaches the same `Arc` through the dense adapter (PR #13's
Task 2). One selection, one instance, both surfaces — which is why the local
backend fixes `memory(recall)` without a second selection path.

**Weights resolution, in priority order:**

1. `local-dir:/abs/path` — read from disk. No hf-hub, no network, no cache
   layout. **This is the restricted-environment contract.**
2. `local:AllMiniLML6V2Q` with a populated cache — hf-hub's `CacheRepo::get`
   reads refs/&lt;revision&gt;, resolves snapshots/&lt;hash&gt;/&lt;file&gt;, and returns it
   with no network call; `ApiRepo::get` only downloads on a cache miss.
   fastembed reads `HF_HOME` for the cache root and maps `AllMiniLML6V2Q` to
   the Xenova/all-MiniLM-L6-v2 repo. **Documented as a convenience, depended on
   by nothing** — the layout is an internal convention of a transitive
   dependency. Verify by execution before documenting; if it fails, delete the
   documentation, not the design.
3. `local:AllMiniLML6V2Q` cold — fastembed fetches ~22 MB on first use. The
   ordinary low-resource path.

## Error handling

| Failure | Treatment | Text must name |
|---|---|---|
| Weights dir missing / unreadable | `RecoverableError` | The path tried, all five required files, and a reachable source for the bundle |
| ONNX session init fails | `RecoverableError` | `ORT_DYLIB_PATH` for dylib-load failures, and that an `os error 5` is application control denying the load, not a missing file |
| Dim mismatch | `RecoverableError` | Both dimensions, the weights dir, and that `vec0` bakes dimension at table creation — delete and rebuild, never migrate |
| `local:` configured, no local backend compiled in | `RecoverableError` | Both escapes — the rebuild command *and* setting a url. Today this is an `anyhow::bail!` naming only the rebuild, stranding a caller that could have switched endpoint. |
| `server-stack` sparse expected + local backend | `RecoverableError` | An operator-fixable config conflict, not a bug: name both fixes — `CODESCOUT_DISABLE_SPARSE=1` for dense-only, or an embedder url that serves both dense and sparse |

**Repair-and-continue: cache-root paths.** `local-dir:` pointed at the cache
root rather than the snapshot directory is the mistake this layout invites.
When the given directory contains exactly one `snapshots/<hash>/` holding the
expected files, descend into it and load. Exactly one correct reading, and it is
a read — the write-path caveat does not apply. Two candidates is not one correct
reading, so it is left alone.

**Corrected 2026-08-11 (ruling during planning):** an earlier draft called for an
MCP `corrections` note here. `corrections` rides on tool responses, and the
repair happens during embedder construction, which is not a tool-response
boundary — surfacing it would mean threading an advisory out of
`RetrievalClient::from_env` through every caller. The repair emits
`tracing::info!` naming the given and resolved paths instead. If the note is
wanted later, the honest shape is a general advisory channel for client
construction, not an ad-hoc one for this case.

**Dimension defaulting.** `model_dim` defaults to `768` today, which is wrong
by construction for a local AllMiniLM user. Explicitly set → unchanged, it *is*
the contract. Unset → discovered from `Embedder::dimensions()` and used for
both `ensure_collection` and validation. A model switch against an existing
index is caught by Unit 6, naming both dimensions.

**Corrected 2026-08-11 (ruling during planning):** an earlier draft placed that
check at client construction. `SqliteVecCodeStore::conn_for`
(`src/retrieval/sqlite_code_store.rs:70`) keys on **`project_id`**, which
`RetrievalClient::from_env` does not have — the store is per-project and the
client is not. The guard therefore runs at the entry to `sync_project` and
`search_in`, the first points where `project_id` exists. Same check, same
message, one call later. `collection_dim` takes `(collection, project_id)` for
the same reason.

## Testing

Where tests live is a design decision. `.github/workflows/ci.yml:62` runs
`--features local-embed` on ubuntu, macOS, and windows. Tests for
`LocalEmbedder::from_dir` live in `crates/codescout-embed/src/local.rs` and are
covered on three OSes from the first commit. **Nothing new is gated on
`local-embed-dynamic`**, which no test lane enables — it gets `cargo check`
only (`ci.yml:157`). This is the mechanism that let PR #13 reach CI with two
compile errors.

| # | Test | Guards |
|---|---|---|
| 1 | `from_dir` on a missing directory names the path and all five expected files | The operator text, which is the value of the error |
| 2 | `from_dir` produces a stable 384-d vector; same input same output; not all-zero | Wrong tokenizer or wrong pooling — both produce a correctly-shaped, silently-wrong vector |
| 3 | Cache-root path repaired to snapshot dir, a `tracing::info!` note naming the given and resolved paths present | That the repair fires *and* teaches |
| 4 | url set → `EmbedderHttp`; url unset + local model → adapter; explicit url beats any model | The `8081` removal — the behaviour change gets the direct test |
| 5 | `CODESCOUT_EMBEDDER_*` overrides `[embeddings]`, exercised as pure functions (`EmbedEnv`, `merge_embed_config`/`resolve_embed_fields_with`) taking env as a plain value — never `EnvGuard` + `#[serial]`, banned crate-wide by `docs/conventions/test-env-isolation.md` | Config precedence, which benchmark cells depend on |
| 6 | Model dim ≠ stored collection dim → `RecoverableError` naming both | Unit 6, at the point it exists to serve |
| 7 | `server-stack` + sparse expected + local backend → error | A downgrade that would otherwise surface as degraded recall, never as a failure |

**Skip only on explicit opt-out.** Test 2 skips on `CODESCOUT_SKIP_ONNX_TESTS=1`
and **never** on a missing weights file. A skip keyed on file presence is
indistinguishable from a pass.

**CI fetches the model.** The `local-embed` lane downloads the ~22 MB model
once (runners have network; the VDI does not) and caches it, so test 2 executes
on three platforms instead of only on a machine that already has weights.
Without this, the strongest test in the design is documentation.

**Mutation-verify tests 2 and 6.** Reintroduce the defect, watch that specific
test die, record which siblings stayed green, revert. The green set measures
what the suite is blind to.

## What survives from PR #13

**Kept:** commit `25c0a175` (the `CodeEmbedder` trait object) with its review
fixes `bc79f98c` and `b3aaf820`, and the Task 2 memory-recall sharing. The seam
is right and now has two implementors. The three `docs/issues/` files stay.

**Dropped:** commit `1eceba32` — src/retrieval/local_onnx.rs (branch-only, and
deleted by this design, so un-spanned), the root
`fastembed` dependency (`Cargo.toml`), and the `local-embed-dynamic` module
gate in `src/retrieval/mod.rs`. Its operator-facing error text is preserved by
moving it into Unit 1.

**Superseded:** the branch's spec and plan.

## Deferred

- **ADR contract 1** — the connect-error substring `EmbedderHttp` emits, matched
  at `src/tools/semantic/semantic_search.rs:46`. Untouched here because
  `EmbedderHttp` stays on the url path; becomes mandatory when the url path
  moves to the resolver.
- **ADR contract 3** — `RemoteEmbedder`'s query prefix needs three states
  (*derive* / *explicit* / *suppressed*), with unset mapping to **suppressed**.
  Root defaults `CODESCOUT_QUERY_PREFIX` to empty; the crate derives from the
  model name. The benchmark in `docs/manual/src/concepts/retrieval-stack.md`
  § Dense embedder rates Q4 no-prefix the champion at 37 against 34 with a
  forced prefix. Getting this wrong costs ~3 points silently. Listed as
  deferred, not absent, so the next person does not rediscover it expensively.
- **`EmbedderHttp` retirement**, and with it root dropping `reqwest`/`rustls`
  and two lean CI lanes dropping 48 crates.
- **Prebuilt release lane** — sub-project 2.

## Risks

- **The VDI cannot verify any of this.** CyberArk EPM kills `ort-sys`'s build
  script before codescout's own code compiles (filed on the PR branch as
  docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md). This
  design does not fix that; it ensures CI covers what the VDI cannot.
- **384-d AllMiniLM may retrieve worse** than the current 768-d vectors,
  worst for exact identifier matches. Benchmark with
  `scripts/run-tc-benchmark.sh` before treating it as a permanent default.
- **Reindex is destructive** — `vec0` bakes dimension at table creation, so a
  dimension change deletes and rebuilds the index.
- **Weights-path resolution 2 is unverified by execution.** Read from source
  only; prove it before it reaches the manual.

## References

- ADR: `docs/adrs/2026-07-25-embedding-transport-boundary.md`
- Session log: `docs/trackers/pr-review-session-log.md` F-5, F-6, W-4
- Recon ledger: `docs/trackers/reconnaissance-patterns.md` R-70, R-71, R-72
- Bug files carried from PR #13 — branch-resident, absent from `experiments`,
  so named without code spans (see the note under the title):
  docs/issues/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md,
  docs/issues/2026-08-08-clippy-pre-existing-drift-stable-gnu-toolchain.md,
  docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md
