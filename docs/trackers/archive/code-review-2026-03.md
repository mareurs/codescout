---
id: null
kind: null
status: archived
title: null
owners: []
tags: []
topic: null
time_scope: null
---
# Code Review Tracker — March 2026

Full codebase audit of codescout (60K lines, 82 files). Issues prioritized by severity.

## Critical

### C1. Deadlock: lock-ordering inversion in LspManager
- **Location:** `src/lsp/manager.rs` — `get_or_start` vs `evict_idle`
- **Problem:** `get_or_start` acquires `clients` then `last_used`. `evict_idle` acquires `last_used` then `clients`. Classic lock-ordering inversion — will deadlock under concurrent load.
- **Fix:** Enforce consistent lock order. Collect eviction candidates under `last_used` alone, drop it, then acquire `clients` for removal.
- **Note:** Reviewer overstated severity — `evict_idle` uses sequential (not nested) locks, so no current deadlock exists. Fixed as defensive hardening.
- **Status:** fixed

### C2. Deadlock risk: Agent::activate() holds inner write lock across cached_embedder lock
- **Location:** `src/agent.rs:264-271`
- **Problem:** Acquires `inner.write()` then `cached_embedder.lock()`. If any code path holds `cached_embedder` while waiting for `inner.read()`, the write lock blocks all readers, creating deadlock.
- **Fix:** Drop the `inner` write guard before clearing the embedder cache.
- **Note:** Latent risk — no current code path triggers the deadlock. Fixed as defensive hardening.
- **Status:** fixed

### C3. Global static VEC0_ACTIVE is unsound across multiple databases
- **Location:** `src/embed/index.rs` — `is_vec0_active`
- **Problem:** Caches result in a process-global `AtomicBool`. Once any DB triggers it to `true`, all subsequent calls for every other connection (including unmigrated library DBs) take the vec0 SQL path. Querying a plain-BLOB table with vec0 SQL will error.
- **Fix:** Removed global cache. The sqlite_master query is O(1) and runs once per search — no perf impact.
- **Status:** fixed

### C4. Non-atomic file writes in edit_file / edit_markdown
- **Location:** `src/tools/file.rs:1885`, `src/tools/markdown.rs:675`
- **Problem:** Use bare `std::fs::write()`. If killed mid-write, file is left corrupt. Meanwhile `write_lines` in `symbol.rs` correctly uses write-to-tmp-then-rename.
- **Fix:** Added `util::fs::atomic_write()`. Used in `edit_file` (3 sites), `edit_markdown`, `write_utf8`, and `write_lines`.
- **Status:** fixed

### C5. Non-atomic two-table writes in insert_chunk / insert_memory
- **Location:** `src/embed/index.rs`
- **Problem:** Inserts into `chunks` then `chunk_embeddings` (or `memories` then `vec_memories`) without a transaction. Safe inside `build_index`'s outer transaction, but these are `pub` functions callable outside that context.
- **Fix:** Both functions now use raw `SAVEPOINT`/`RELEASE` SQL for atomicity. Works with `&Connection` (no signature change) and nests safely inside callers' existing transactions.
- **Status:** fixed

### C6. Weak HTTP auth token generation
- **Location:** `src/server.rs:534-561`
- **Problem:** `/dev/urandom` path uses `std::fs::read` (undefined on device nodes). Fallback uses `pid ^ timestamp_nanos` — trivially predictable.
- **Fix:** Changed to `File::open` + `read_exact(32)`. Fallback uses `DefaultHasher` with pid + thread id + timestamp entropy, hashed 4x to fill 32 bytes.
- **Status:** fixed

### C7. Path traversal in LibraryRegistry::resolve_path
- **Location:** `src/library/registry.rs`
- **Problem:** Bare `entry.path.join(relative)` with no sanitization. `"../../etc/passwd"` escapes the library root.
- **Fix:** Added `Component::ParentDir` rejection — `..` in any path component now returns an error.
- **Status:** fixed

## Important

### I1. TOCTOU race in Agent::activate()
- **Location:** `src/agent.rs:201-262`
- **Problem:** Read lock dropped, I/O done, write lock acquired. `is_home` may be stale by the time write lock is held.
- **Fix:** Moved `is_home` check and `effective_read_only` computation inside the write lock. All I/O (config loading, memory stores, library registry, project discovery) still runs before acquiring the lock — it's independent of `is_home`. Under the write lock we compute `is_home`, build `ActiveProject` with the correct `read_only`, build the workspace, and commit atomically. No TOCTOU window between check and use.
- **Status:** fixed

### I2. Unbounded stderr buffer in LspClient
- **Location:** `src/lsp/client.rs:186-206`
- **Problem:** `stderr_lines` grows unbounded for the entire LSP process lifetime. Never capped or cleared after init.
- **Fix:** Added `MAX_STDERR_LINES: usize = 200` constant. Push site in the stderr reader task now evicts the oldest entry (`remove(0)`) before pushing when the cap is reached, bounding the buffer to the 200 most recent error/exception/fatal lines.
- **Status:** fixed

### I3. TTL discrepancy between LspManager::new() and new_arc()
- **Location:** `src/lsp/manager.rs`
- **Problem:** `new()` = 20min, `new_arc()` = 30min. Confusing; tests use `new()` directly.
- **Fix:** Added `pub const DEFAULT_IDLE_TTL: Duration = Duration::from_secs(30 * 60)` as an associated constant on `LspManager`. Both `new()` and `new_arc()` now reference it.
- **Status:** fixed

### I4. Mux initialize missing hierarchicalDocumentSymbolSupport
- **Location:** `src/lsp/mux/process.rs:127-153`
- **Problem:** Mux doesn't advertise `hierarchicalDocumentSymbolSupport` — degrades to flat symbols through mux path.
- **Fix:** Added `"hierarchicalDocumentSymbolSupport": true` to the `documentSymbol` capability block in the mux's initialize handshake (`src/lsp/mux/process.rs`). Now matches what the direct `LspClient::initialize()` path already advertises via the typed `lsp_types` structs.
- **Status:** fixed

### I5. best_effort_canonicalize passes raw .. components when parent doesn't exist
- **Location:** `src/util/path_security.rs`
- **Problem:** When parent directory doesn't exist, raw path with `..` components passes `starts_with` check.
- **Fix:** Added `Component::ParentDir` rejection in `validate_write_path` immediately after `canonicalize_write_target`. If the resolved path still contains `..` (because an intermediate dir didn't exist and `best_effort_canonicalize` fell back to the raw path), the write is denied. Regression test `write_traversal_via_nonexistent_dir_rejected` verifies the exact failure scenario.
- **Status:** fixed

### I6. validate_write_path CWD root may be overly broad
- **Location:** `src/util/path_security.rs`
- **Problem:** Adds `std::env::current_dir()` as allowed write root. If CWD is `/` or `$HOME`, overly permissive.
- **Fix:** Added `is_broad` guard before pushing CWD onto the allowed-roots list. CWD is skipped when it equals `/` or `$HOME` (checked via `home_dir()`). Uses `is_some_and` per clippy idiom.
- **Status:** fixed

### I7. run_command_inner is a 510-line god function

- **Location:** `src/tools/workflow.rs:2274-2780`
- **Problem:** Handles security checks, background spawning, tee injection, process execution, output buffering, and summarization in one function.
- **Status:** fixed — extracted `TmpfileGuard`, `AbortOnDrop`, `resolve_work_dir`, `spawn_background_command`, `inject_tee`, `handle_successful_output`; `run_command_inner` is now a ~180-line dispatcher
### I8. Onboarding::call is 570 lines
- **Location:** `src/tools/workflow.rs:1105-1675`
- **Problem:** Multiple return paths and deeply nested conditionals.
- **Status:** fixed — extracted `handle_refresh_prompt`, `handle_already_onboarded`, `perform_full_onboarding`; `call` is now a 12-line dispatcher

### I9. ReadFile::call is 565 lines with complex branching
- **Location:** `src/tools/file.rs:45-610`
- **Problem:** Buffer ref handling (4 sub-paths) mixed with real file reading in one method.
- **Fix:** Extracted 8 focused helpers: `strip_buffer_ref_quotes`, `read_from_buffer`, `validate_read_nav_params`, `compute_source_tag`, `read_file_text`, `read_complete_mode`, `read_json_path_nav`, `read_toml_yaml_key`, `read_with_line_range`, `read_full_file`. `call` is now a ~75-line dispatcher. Each helper has a single responsibility and a doc comment.
- **Status:** fixed

### I10. edit_file new_string schema vs code mismatch
- **Location:** `src/tools/file.rs:1685-1710`
- **Problem:** `new_string` in `required` schema but extracted with `.unwrap_or("")` — contradicts declared contract.
- **Fix:** Removed `new_string` from top-level `required` (batch mode genuinely doesn't use it). Added `is_string()` guard in the prepend/append path, matching the existing guard in the single-edit path. Schema descriptions updated to clarify which modes require `new_string`.
- **Status:** fixed

### I11. ~1500 lines of unregistered dead code in github.rs
- **Location:** `src/tools/github.rs`
- **Problem:** 5 full tool implementations unregistered since c808995. Maintenance burden on every `Tool` trait refactor.
- **Fix:** Deleted `src/tools/github.rs` and removed `pub mod github;` from `src/tools/mod.rs`. No other references existed.
- **Status:** fixed

### I12. CORS allows all localhost ports
- **Location:** `src/dashboard/routes.rs`
- **Problem:** Any local web app can hit memory write/delete endpoints.
- **Fix:** `build_router` now takes `port: u16`. CORS allow-list is exact: `http://localhost:{port}` and `http://127.0.0.1:{port}`. Added `cors_rejects_wrong_port` test.
- **Status:** fixed

### I13. open_db schema migrations not transactional
- **Location:** `src/embed/index.rs`
- **Problem:** Multiple `ALTER TABLE` migrations run outside any explicit transaction. Partial migration on crash.
- **Fix:** Three ALTER TABLE column-addition migrations in `open_db` now run inside a `SAVEPOINT schema_migrations`. Error path does `ROLLBACK TO` + `RELEASE` to leave the connection usable.
- **Status:** fixed

### I14. SQL limit interpolated via format! instead of parameterized
- **Location:** `src/embed/index.rs` — `search_scoped_vec0`
- **Problem:** `usize` values are safe but sets a bad precedent for future contributors.
- **Fix:** Added an explicit comment in `search_scoped_vec0` explaining why `{limit}`/`{inner_limit}` must be format-interpolated: sqlite-vec's KNN planner requires a literal LIMIT; binding via `?N` bypasses the KNN-k optimisation. The outer LIMIT values are already parameterized. No injection risk (`usize`).
- **Status:** fixed (documented constraint, not a code defect)

## Minor

### M1. ActiveProject fields all pub — breaks encapsulation
- **Location:** `src/agent.rs:88-101`
- **Status:** fixed — all 8 fields changed to `pub(crate)`

### M2. Cached instructions field stale for stdio transport
- **Location:** `src/server.rs:46-50`
- **Status:** fixed — `instructions: Arc<RwLock<String>>`; `refresh_instructions()` called after each `activate_project`

### M3. strip_project_root_from_result uses naive string replacement
- **Location:** `src/server.rs:634-648`
- **Status:** fixed — boundary-aware `strip_prefix_from_text` helper; 3 regression tests added

### M4. project_status does blocking file I/O under async read lock
- **Location:** `src/agent.rs:461-501`
- **Status:** fixed — `project_status()` clones active project under read lock, drops lock before any I/O

### M5. Stale comment says retry disabled but RETRY_ON_CANCELLED = true
- **Location:** `src/lsp/client.rs:448`
- **Status:** fixed — comment updated to reflect `RETRY_ON_CANCELLED = true`

### M6. i32 version counter — no overflow check
- **Location:** `src/lsp/client.rs:1073`
- **Status:** fixed — `*v = v.wrapping_add(1)`

### M7. Reader task code duplicated between start() and connect()
- **Location:** `src/lsp/client.rs:327`
- **Status:** fixed — extracted `dispatch_lsp_message` associated fn; both reader tasks call it

### M8. Duplicated get_ts_language with different case sensitivity
- **Location:** `src/embed/ast_chunker.rs` + `src/ast/parser.rs`
- **Status:** fixed — canonical `pub(crate) get_ts_language` in `src/ast/mod.rs`; both callers unified

### M9. RemoteEmbedder::dimensions() returns 0 — leaky abstraction
- **Location:** `src/embed/remote.rs`
- **Status:** fixed — `cached_dims: Arc<AtomicUsize>` populated on first successful embed

### M10. 15 languages detected but only 9 have AST support
- **Location:** `src/embed/mod.rs` + `src/ast/mod.rs`
- **Status:** fixed — `detect_language` doc clarified; points to `get_ts_language` for AST support check

### M11. get_path_param(true)?.unwrap() — safe but brittle pattern
- **Location:** `src/tools/symbol.rs` (7 occurrences)
- **Status:** fixed — `require_path_param` helper with `RecoverableError`; all 7 call sites updated

### M12. Unnecessary re-read after write in perform_edit
- **Location:** `src/tools/file.rs:1899`
- **Status:** fixed — uses `&new_content` already in scope; disk re-read removed

### M13. std::mem::forget(temp_path) — unusual pattern
- **Location:** `src/tools/output_buffer.rs:478`
- **Status:** fixed — replaced with `tmp.keep()` (idiomatic `NamedTempFile` API)

### M14. Mode and Context enums appear unused
- **Location:** `src/config/modes.rs`
- **Status:** fixed — `src/config/modes.rs` deleted; `pub mod modes` removed from `src/config/mod.rs`

### M15. try_init().ok() silently swallows subscriber failure
- **Location:** `src/logging.rs:81`
- **Status:** fixed — `if let Err(e)` with `eprintln!` on failure
