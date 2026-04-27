# Code Review Tracker — March 2026

Full codebase audit of codescout (60K lines, 82 files). Issues prioritized by severity.

## Re-verification — 2026-04-27

All 36 items re-checked against current code:

- **Fixed (29):** C1–C7, I1–I14 (I11 retired 2026-04-27), M2, M3, M4, M5, M7, M11, M12, M14
- **Obsolete (3):** M6 (widened to `i64`), M8 (single shared impl), M15 (pattern gone)
- **Open by design (2):** M9 (`RemoteEmbedder` dimensions unknown until first response), M13 (intentional tempdir leak in test fixture)
- **Open — actionable (2):** M1 (encapsulation), M10 (11 detected langs lack AST)

Net: 32 / 36 resolved. 2 minor refactors remain.
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
- **Status:** fixed (verified 2026-04-27 — `is_home`/`effective_read_only` now computed under write lock at `src/agent/mod.rs:296-304`)

### I2. Unbounded stderr buffer in LspClient
- **Location:** `src/lsp/client.rs:186-206`
- **Problem:** `stderr_lines` grows unbounded for the entire LSP process lifetime. Never capped or cleared after init.
- **Status:** fixed (verified 2026-04-27 — `MAX_STDERR_LINES=200` cap + eviction at `src/lsp/client.rs:28,349`)

### I3. TTL discrepancy between LspManager::new() and new_arc()
- **Location:** `src/lsp/manager.rs`
- **Problem:** `new()` = 20min, `new_arc()` = 30min. Confusing; tests use `new()` directly.
- **Status:** fixed (verified 2026-04-27 — both unified on `DEFAULT_IDLE_TTL = 30 * 60s` at `src/lsp/manager.rs:177`)

### I4. Mux initialize missing hierarchicalDocumentSymbolSupport
- **Location:** `src/lsp/mux/process.rs:127-153`
- **Problem:** Mux doesn't advertise `hierarchicalDocumentSymbolSupport` — degrades to flat symbols through mux path.
- **Status:** fixed (verified 2026-04-27 — capability advertised at `src/lsp/mux/process.rs:139`)

### I5. best_effort_canonicalize passes raw .. components when parent doesn't exist
- **Location:** `src/util/path_security.rs`
- **Problem:** When parent directory doesn't exist, raw path with `..` components passes `starts_with` check.
- **Status:** fixed (verified 2026-04-27 — `Component::ParentDir` rejected post-canonicalize at `src/util/path_security.rs:282-287`; regression test at L794-812)

### I6. validate_write_path CWD root may be overly broad
- **Location:** `src/util/path_security.rs`
- **Problem:** Adds `std::env::current_dir()` as allowed write root. If CWD is `/` or `$HOME`, overly permissive.
- **Status:** fixed (verified 2026-04-27 — CWD `/` and `$HOME` skipped at `src/util/path_security.rs:316-323`)

### I7. run_command_inner is a 510-line god function
- **Location:** `src/tools/workflow.rs:2274-2780`
- **Problem:** Handles security checks, background spawning, tee injection, process execution, output buffering, and summarization in one function.
- **Status:** fixed (verified 2026-04-27 — moved to `src/tools/run_command.rs:956-1175`, ~220 lines)

### I8. Onboarding::call is 570 lines
- **Location:** `src/tools/workflow.rs:1105-1675`
- **Problem:** Multiple return paths and deeply nested conditionals.
- **Status:** fixed (verified 2026-04-27 — `call` is 17 lines at `src/tools/onboarding.rs:276-292`; logic split into `handle_refresh_prompt`, `handle_already_onboarded`, `perform_full_onboarding`)

### I9. ReadFile::call is 565 lines with complex branching
- **Location:** `src/tools/file.rs:45-610`
- **Problem:** Buffer ref handling (4 sub-paths) mixed with real file reading in one method.
- **Status:** fixed (verified 2026-04-27 — `call` is 61 lines at `src/tools/read_file.rs:38-98`; buffer handling extracted to `read_from_buffer:126-257`)

### I10. edit_file new_string schema vs code mismatch
- **Location:** `src/tools/file.rs:1685-1710`
- **Problem:** `new_string` in `required` schema but extracted with `.unwrap_or("")` — contradicts declared contract.
- **Status:** fixed (verified 2026-04-27 — schema declares only `path` required at `src/tools/edit_file.rs:110-135`; `new_string` optional with `unwrap_or("")` is consistent)

### I11. ~1500 lines of unregistered dead code in github.rs
- **Location:** `src/tools/github.rs`
- **Problem:** 5 full tool implementations unregistered since c808995. Maintenance burden on every `Tool` trait refactor.
- **Status:** fixed (2026-04-27 — retired: deleted `src/tools/github.rs`, `src/prompts/github_instructions.md`, `docs/manual/src/tools/github.md`. Removed `github_enabled` from `PathSecurityConfig`, `SecuritySection`, `GlobalSecuritySection`, `ProjectStatus`, `activate_project` output, and all gating + tests. Bumped `ONBOARDING_VERSION` to 11)

### I12. CORS allows all localhost ports
- **Location:** `src/dashboard/routes.rs`
- **Problem:** Any local web app can hit memory write/delete endpoints.
- **Status:** fixed (verified 2026-04-27 — exact-port allowlist `http://localhost:{port}` / `127.0.0.1:{port}` at `src/dashboard/routes.rs:25-29`)

### I13. open_db schema migrations not transactional
- **Location:** `src/embed/index.rs`
- **Problem:** Multiple `ALTER TABLE` migrations run outside any explicit transaction. Partial migration on crash.
- **Status:** fixed (verified 2026-04-27 — `SAVEPOINT schema_migrations` / `RELEASE` with rollback at `src/embed/index.rs:459-502`)

### I14. SQL limit interpolated via format! instead of parameterized
- **Location:** `src/embed/index.rs` — `search_scoped_vec0`
- **Problem:** `usize` values are safe but sets a bad precedent for future contributors.
- **Status:** fixed (verified 2026-04-27 — no unsafe `format!`-interpolated SQL limit found in current `search_scoped_vec0` at `src/embed/index.rs:1176`)

## Minor

### M1. ActiveProject fields all pub — breaks encapsulation
- **Location:** `src/agent.rs:88-101`
- **Status:** open (verified 2026-04-27 — all 12 fields still `pub` at `src/agent/mod.rs:93-121`)

### M2. Cached instructions field stale for stdio transport
- **Location:** `src/server.rs:46-50`
- **Status:** obsolete (verified 2026-04-27 — `cached_instructions` no longer exists; instructions generated dynamically)

### M3. strip_project_root_from_result uses naive string replacement
- **Location:** `src/server.rs:634-648`
- **Status:** fixed (re-verified 2026-04-27 — delegates to `strip_prefix_from_text` at `src/server.rs:1181` which checks for value-boundary chars (quotes, spaces, colons, newlines) before stripping; preserves embedded path literals inside code/comments)

### M4. project_status does blocking file I/O under async read lock
- **Location:** `src/agent.rs:461-501`
- **Status:** fixed (2026-04-27 — split into Phase 1 (cheap clones under read lock), Phase 2 (`memory.list()` + FS reads in `tokio::task::spawn_blocking`), Phase 3 (workspace summary). Cloned `MemoryStore` (cheap, just `PathBuf`) instead of holding the lock)

### M5. Stale comment says retry disabled but RETRY_ON_CANCELLED = true
- **Location:** `src/lsp/client.rs:448`
- **Status:** fixed (verified 2026-04-27 — no "retry disabled" comment exists; only a test comment at `src/lsp/client.rs:2035` accurately states `RETRY_ON_CANCELLED=true`)

### M6. i32 version counter — no overflow check
- **Location:** `src/lsp/client.rs:1073`
- **Status:** obsolete (verified 2026-04-27 — `next_id` widened to `AtomicI64` at `src/lsp/client.rs:195,416,510`)

### M7. Reader task code duplicated between start() and connect()
- **Location:** `src/lsp/client.rs:327`
- **Status:** fixed (2026-04-27 — extracted `Self::run_dispatch_loop<R: AsyncRead>` and `Self::drain_pending_disconnect` helpers in `src/lsp/client.rs`; both `start()` and `connect()` now call the shared loop, keeping only their transport-specific cleanup (child wait + warn logging vs. silent drain))

### M8. Duplicated get_ts_language with different case sensitivity
- **Location:** `src/embed/ast_chunker.rs` + `src/ast/parser.rs`
- **Status:** obsolete (verified 2026-04-27 — single shared `crate::ast::get_ts_language` impl, both call-sites delegate to it)

### M9. RemoteEmbedder::dimensions() returns 0 — leaky abstraction
- **Location:** `src/embed/remote.rs`
- **Status:** open — by design (verified 2026-04-27 at `src/embed/remote.rs:151-157`; comment notes dimensions unknown until first response)

### M10. 15 languages detected but only 9 have AST support
- **Location:** `src/embed/mod.rs` + `src/ast/mod.rs`
- **Status:** open (verified 2026-04-27 — `detect_language` at `src/ast/mod.rs:52-81` returns 24 langs; `get_ts_language` at L91-105 supports ~14. Detected without AST: c, cpp, csharp, ruby, php, swift, scala, elixir, haskell, lua, markdown)

### M11. get_path_param(true)?.unwrap() — safe but brittle pattern
- **Location:** `src/tools/symbol.rs` (7 occurrences)
- **Status:** fixed (verified 2026-04-27 — zero `get_path_param(_, true)?.unwrap()` occurrences in `src/tools/`)

### M12. Unnecessary re-read after write in perform_edit
- **Location:** `src/tools/file.rs:1899`
- **Status:** fixed (verified 2026-04-27 — syntax check runs on in-memory `&new_content` at `src/tools/edit_file.rs:283-351`; no re-read after `atomic_write`)

### M13. std::mem::forget(temp_path) — unusual pattern
- **Location:** `src/tools/output_buffer.rs:478`
- **Status:** open — by design (verified 2026-04-27 at `src/tools/library.rs:261`; intentional tempdir leak for test fixture)

### M14. Mode and Context enums appear unused
- **Location:** `src/config/modes.rs`
- **Status:** fixed (2026-04-27 — deleted `src/config/modes.rs` entirely; file was orphaned with no `mod modes` declaration)

### M15. try_init().ok() silently swallows subscriber failure
- **Location:** `src/logging.rs:81`
- **Status:** obsolete (verified 2026-04-27 — `try_init().ok()` pattern not present in current `src/logging.rs`)
