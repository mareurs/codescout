# Phase 5 — Embed / Memory / Library

**Date:** 2026-04-24
**Scope:** `src/embed/`, `src/memory/`, `src/library/` (+ `crates/codescout-embed/`)
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Cross-check answers (Phase 1-4)

- **Phase 1 S6 (`EMBED_API_KEY` flow):** Confirmed clean at obvious sinks. After deserialize at `src/config/project.rs:73`, key flows through `Agent::get_or_create_embedder` (`src/agent/mod.rs:792-823`) → `RemoteEmbedder` → `req.bearer_auth(key)` at `crates/codescout-embed/src/remote.rs:189`. **No `tracing::*!` / `Display` / `Debug` impl touches `api_key`.** Cache key is `format!("{model}@{url}")` — no secret. **BUT** see S1 below: `from_url` lacks the HTTPS gate that `custom()` enforces.
- **Phase 2 C1 (`index_project` concurrency):** Project scope guarded by `Agent.indexing: Arc<Mutex<IndexingState>>` (`src/agent/mod.rs:61`); returns `"already_running"` (`src/tools/semantic.rs:540-553`). **BUT** `lib:<name>` branch (`src/tools/semantic.rs:371-468`) bypasses this guard entirely. → C2 below.
- **Phase 2 C3 (`register_library` concurrency):** Mutation + `save()` happen inside single `agent.inner.write().await` critical section (`src/tools/library.rs:147-160`, `src/library/auto_register.rs:38-69`). C3 concern does NOT reproduce.
- **Memory leak (`flat_texts` clone):** Partial fix landed (`src/embed/index.rs:1628` `drop(flat_texts);` after spawn loop). Per-batch clones at `:1614` still coexist briefly with `flat_texts` at spawn moment. Bigger structural mitigation (group-by-group via `file_group_size`) in place. ptmalloc2 arena retention orthogonal/unaddressed (env-only). Consistent with `project_memory_leak.md` "partial."

---

## Security (Ibex)

### S1 — MEDIUM — `RemoteEmbedder::from_url` allows API key over plaintext HTTP
- **Location:** `crates/codescout-embed/src/remote.rs:122-141` (and project mirror `src/embed/remote.rs:97-115`).
- **Evidence:** `from_url` is the production path (wired via `create_embedder_with_config` lib.rs:135). `custom()` (lib.rs:96-114) enforces HTTPS-or-bail, but `custom()` is dead-on-arrival per deprecation message at lib.rs:174-184. `from_url` sends `bearer_auth(key)` without scheme check.
- **Exploit:** User config `url = "http://embeddings.internal.corp/v1"` + `api_key` → `Authorization: Bearer …` sniffable on local segment.
- **Fix:** Mirror `custom()` check in `from_url`:
  ```rust
  if api_key.is_some() && !url.starts_with("https://") && !is_loopback(url) {
      bail!("HTTPS required when api_key is set ...");
  }
  ```
  Allow `http://localhost`/`127.0.0.1`/`[::1]` for Ollama.
- **Confidence:** high.

### S2 — LOW (real exploit path) — `register_library` accepts arbitrary absolute paths; library-scope index reads them
- **Location:** `src/tools/library.rs:103-167`; bypass at `src/tools/semantic.rs:371-468`.
- **Evidence:** Only checks `lib_path.exists() && is_dir()`. No canonicalize, no symlink check, no scope check vs project root. `check_index_scope` runs only for project-scope indexing — library branch skips it.
- **Exploit:** Prompt-injected MCP call: `register_library(name="secret", path="/etc")` → later `index_project(scope="lib:secret")` walks `/etc`, embeds every file, stores verbatim chunks in `.codescout/embed/lib_secret.sqlite`. Then `semantic_search(scope="lib:secret", query="aws")` returns secrets to LLM.
- **Fix:** (a) Run `check_index_scope` against registered library path before allowing it, OR (b) require lib path under one of `read_paths` scopes, OR (c) canonicalize + reject system-root paths in `classify_path` (`src/embed/preflight.rs:60-84`).
- **Confidence:** medium-high.

### S3 — INFO/QUESTION — Memory `topic` sanitization gap on Windows-style `\\` separators
- **Location:** `src/memory/mod.rs:131-146`.
- **Evidence:** On Linux `Path::new("..\\..\\etc\\passwd").components()` returns single `Normal("..\\..\\etc\\passwd")` (safe — `\` not a separator on Unix). `MemoryStore::list` `replace('\\', "/")` (line 99) creates fake hierarchy on read-back. Round-trip broken but not exploitable on Linux. Windows behavior unverified — see Q1.
- **Confidence:** low. Recategorized to question.

### S4 — LOW — `init_sqlite_vec` global extension registration via `transmute`
- **Location:** `src/embed/index.rs:328-349`.
- **Evidence:** `unsafe transmute` of `sqlite3_vec_init`. Comment says safe at C level; signature drift in upstream `sqlite-vec` crate would silently miscompile.
- **Fix:** `static_assertions::assert_type_eq_all!` or `const _: fn(...) = sqlite_vec::sqlite3_vec_init;` near the transmute → build asserts.
- **Confidence:** low.

### S5 — LOW (DOS-adjacent; pairs with S1) — Embedding API responses unbounded
- **Location:** `crates/codescout-embed/src/remote.rs:200` — `resp.json::<EmbedResponse>().await?`.
- **Evidence:** Reads entire body into memory. 300s timeout bounds duration not bytes. Hostile endpoint can stream gigabytes.
- **Fix:** Cap with `resp.bytes_with_max_size(...)` before json-decode (~16 MB). Worth fixing alongside S1.
- **Confidence:** medium.

---

## Critical (non-security)

### C1 — Memory write embedding dim mismatch silently swallowed → silent data loss
- **Location:** `src/embed/index.rs:660-696` (create), `:713-748` (insert), `:842-877` (upsert); caller at `src/tools/memory.rs:236-265`.
- **Issue:** `vec_memories` virtual table created with embedding dim from `meta`, but `insert_memory`/`upsert_memory_by_title` never re-validate that supplied embedding matches dim. Model change (e.g. 384→768 dim) before `build_index` re-runs causes sqlite-vec INSERT failure. Caller in `cross_embed_memory` downgrades to `tracing::debug!("cross-embed memory failed (non-fatal): {e}")`. User sees successful `memory(action="write")` response but memory absent from `recall`.
- **Why critical:** silent data loss in user-requested feature. `build_index` has model-change check (`:2294`); memory write path doesn't.
- **Fix:** In `insert_memory`/`upsert_memory_by_title`, query stored `embedding_dims` and compare to `embedding.len()` before BLOB write; `RecoverableError` with hint to run `index_project --force`.

### C2 — Library-scope `index_project` not gated by `IndexingState::Running`
- **Location:** `src/tools/semantic.rs:371-468` (lib branch).
- **Issue:** Concurrent `index_project(scope="lib:foo")` calls (or one lib + one project) race on:
  - same `lib_foo.sqlite` (WAL + 5s busy timeout serializes but slow),
  - in-memory `LibraryRegistry` (fine — `inner.write().await` serializes mutations),
  - `libraries.json` rewrite (last-writer-wins; one indexing's `entry.indexed = true` may be lost).
- **Fix:** Share `IndexingState::Running` guard across both branches OR per-library `Arc<Mutex<()>>` keyed by lib name. Apply consistent `started`/`already_running` shape for LLM feedback.

---

## Important

### I1 — `cross_embed_memory` / `create_semantic_anchors` failures = `tracing::debug` only
- **Location:** `src/tools/memory.rs:521-555`.
- **Evidence:** Three `tracing::debug!("...failed (non-fatal): {e}")` in `memory(action="write")`. Combined with C1, user has no signal that semantic indexing failed. `tracing::debug` below default level → never seen.
- **Fix:** Bump to `tracing::warn!`. Return `warnings: [...]` array in `memory write` response when something cross-cutting failed (legitimate exception to `json!("ok")` rule per CLAUDE.md — caller genuinely needs to know).

### I2 — `find_python_source` / `find_node_source` don't reject symlinks escaping project root
- **Location:** `src/library/auto_register.rs:212-219` (Node), `:353-369` (Python).
- **Evidence:** `project_root.join(dep_name)` + `is_dir()` check only. Malicious `node_modules/express` symlink → `/etc` accepted. Auto-runs on every `activate_project`. Combined with S2 → poisoned `node_modules` escalates to indexing arbitrary host paths.
- **Fix:** `canonicalize()` candidate path; check still under `project_root` (or known cache root). Skip + `tracing::warn` otherwise.

### I3 — `LibraryEntry.path` raw `PathBuf`; non-atomic `save()`
- **Location:** `src/library/registry.rs:13-28`, `:67`.
- **Evidence:** `PathBuf` with non-UTF-8 bytes serializes lossy. `std::fs::write` not atomic — disk-full mid-write could corrupt `libraries.json`.
- **Fix:** Wrap `save()` in `crate::util::fs::atomic_write` (already used at `src/memory/mod.rs:65`).

### I4 — `delete_memory` doesn't delete `.anchors.toml` sidecar
- **Location:** `src/tools/memory.rs:177-216`; sidecar at `src/memory/anchors.rs:246-249` (`anchor_path_for_topic`).
- **Evidence:** Only `store.delete(topic)`. Sidecar `<topic>.anchors.toml` stays on disk and continues being enumerated by `check_all_memories`. Stale anchors accumulate.
- **Fix:** Sidecar-cleanup branch in delete handlers, mirroring `update_anchors_on_write`'s sidecar-aware logic.

---

## Minor (grouped)

- **Retry kind discrimination** (`crates/codescout-embed/src/remote.rs:184-208`): transport errors retried 3× regardless of kind — DNS NXDOMAIN doesn't benefit from retry.
- **`open_db` migration probe outside savepoint** (`src/embed/index.rs:451-460`): swallows transient errors → repeated `ALTER TABLE` attempts on corrupted DB.
- **`extract_paths` regex unanchored** (`src/memory/anchors.rs:43-51`): matches any `src/...` substring; code-fenced JSON `"src/foo.rs"` registers an anchor. Worth a comment.
- **`safe_truncate` for memory titles** (`src/tools/memory.rs:230`): handled correctly — defensive pattern done right.
- **`from_url` no upfront URL validation** — `Url::parse(url)?` would fail faster with better message than `reqwest` rejection downstream.
- **Local model from `~/.cache/huggingface/hub/`** (`crates/codescout-embed/src/local.rs:5,30`): no integrity check beyond fastembed/HF. Out-of-scope (HF trust); CONTRIBUTING note.
- **`auto_register_deps` writes `libraries.json` under agent write lock**, sync `save()` blocks executor briefly. Not critical.

---

## Open questions

1. **Q1:** On Windows, does `Path::new("..\\..\\foo").components()` parse `\\` as separator and return `[ParentDir, ParentDir, Normal("foo")]`? If yes, `sanitize_topic` correct on Windows. If no, S3 to fix. Cannot verify from Linux.
2. **Q2:** Is `RemoteEmbedder::custom()` dead code now (deprecation at lib.rs:174-184 redirects callers)? If dead, remove — keeping it makes reviewers think HTTPS gate is in place when `from_url` (live path) lacks it.
3. **Q3:** `EmbeddingsConfig.api_key` in `.codescout/project.toml` — file NOT auto-added to `.gitignore` (unlike `private-memories/`). Intentional? `project.toml` plausibly committed for team config → `api_key = "sk-…"` leaks. Docs warn loudly + runtime check on tracked files.
4. **Q4:** `IndexingState::Running` returns `"already_running"` JSON instead of `RecoverableError`. CLAUDE.md says "RecoverableError for expected input-driven failures." Inconsistency intentional?
