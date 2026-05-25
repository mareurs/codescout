# Changelog

All notable changes to codescout are documented here.

## [0.14.0] — 2026-05-25

### Added

- **Surgical body editing for `artifact(update)` — `patch={body_edits: [...]}`.**
  Mirrors `edit_markdown`'s batch shape: each entry is `{heading, action,
  content? | old_string+new_string?, at?, replace_all?, include_subsections?}`,
  applied atomically. Mutually exclusive with `patch={body}`. Removes the
  temptation to write a partial body in the first place — the anti-pattern
  that cost a real ~600-line tracker on 2026-05-25 (`artifact(get,
  heading=…)` returns *a section*, but `patch={body: <section>}` *replaces
  the entire body*). See
  [docs/architecture/augmented-artifacts.md § Body editing surfaces](docs/architecture/augmented-artifacts.md)
  and the live MCP guide at `src/prompts/guides/librarian.md § Body Editing
  Surfaces`. Refs `docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md`.

### Changed

- **Body-write safety net: 50% shrink guard + `force=true` escape hatch.**
  Any `patch={body}` (or `edit_markdown`) write that would reduce the file
  by more than 50% is refused with
  `RecoverableError("body-shrink guard: …")`. The error hint names both
  `body_edits[]` and `force=true`. Exemptions: files under 200 bytes (the
  percentage is meaningless for shells), and augmentations with
  `append_mode + history_cap` set (legitimate history trimming).

- **Strict patch deserializer on `UpdatePatch`.** Misspelled keys like
  `body_prepend_section` now return `RecoverableError` listing the valid
  fields instead of silently no-opping. (Strictness applies to the patch
  body only; the outer `Args` still accepts dispatcher-injected fields.)

- **Forensic body-mutation trail.** Every body write emits a `field_patch`
  event with `payload={field: "body", prev_bytes, new_bytes, edits_count,
  mode, forced}`. Query via `artifact_event(action="list", artifact_id=X)`
  — a body write that shouldn't have happened is now reconstructable from
  the event timeline.

### Fixed

- **`call_graph` depth > 1 timeout.** BFS now parallelizes per-level LSP
  one-hop queries via `futures::try_join_all`; previous serial loop
  saturated at ~6s × N siblings, hitting the 60s tool timeout on depth=2
  walks across realistic call sites.

- **`call_graph` spurious self-edges in the tree-sitter fallback.** When
  rust-analyzer cannot model a symbol (e.g. `#[test]` fns), the
  `references()` fallback could return locations inside the symbol's own
  body, which the AST walk-up resolved back to the symbol itself. Filtered
  at the edge construction site.

- **Test isolation regressions.** `retrieval_unit` env tests now use the
  `temp-env` crate (drop `serial_test` markers; concurrent test runs no
  longer leak env into sibling tests). `symbols_no_body_start_line` pins
  the new auto-inline contract.

### Docs

- New section in `docs/architecture/augmented-artifacts.md` covering the
  body / `body_edits` / `force` surfaces, the shrink-guard exemptions, the
  forensic trail, and the anti-pattern story.
- New `## Body Editing Surfaces` section in the live MCP guide
  (`src/prompts/guides/librarian.md`).
- `librarian` guide action table now includes `doctor` and `audit_doc_refs`.
- `progressive-disclosure` guide documents the path-relative annotation
  convention.

## [0.12.1] — 2026-05-16

### Added

- **`audit_doc_refs` librarian action** — scans markdown for stale code refs
  (file paths, line refs, symbols, module paths, link targets) against the
  current filesystem and LSP symbol index. Emits findings as an `audit_issues`
  tracker at `docs/trackers/doc-ref-audit.md` (auto-created). Manual cadence
  in v1; `fail_on` flag available for downstream CI integration. See
  [docs/manual/src/concepts/audit-doc-refs.md](docs/manual/src/concepts/audit-doc-refs.md).

### Changed

- **LSP pool default capacity bumped 5 → 10** to support multi-worktree swarm
  workflows. Each `(language, project_root)` pair gets its own pooled LSP
  client; with 5 slots, a swarm of 3-4 worktrees touching 2 languages would
  thrash via LRU eviction and pay cold-start cost on every switch. 10 covers
  realistic parallel-agent setups while staying well under per-process memory
  ceilings. ([#5](https://github.com/mareurs/codescout/issues/5))

### Docs

- **README compacted and rewired to the manual.** Dropped detail-heavy sections
  (full retrieval-stack tables, Kotlin specifics, embedding model rundown) in
  favour of short summaries that link to the corresponding pages on
  [mareurs.github.io/codescout](https://mareurs.github.io/codescout/). Added a
  prominent **Artifacts** section advertising the embedded librarian — what it
  is, why it matters, and a four-call usage example.
- Manual is now advertised on the repo landing page: 3 badges (docs /
  crates.io / license), a 📖 callout linking to the manual root, `homepage`
  and `documentation` fields populated in `Cargo.toml` (so crates.io shows
  them), and 9 GitHub topic tags (`mcp`, `mcp-server`, `claude-code`, etc.)
  for discovery.
- Plugin documentation consolidated: there is only one plugin
  (`codescout-companion`), not two. Removed the stale "companion-plugin.md"
  duplicate and merged content into the single canonical page.

### Publishing

- **First publish of `codescout-embed` (0.1.0) and `librarian-mcp` (0.1.0)**
  to crates.io. Both were previously workspace-internal path dependencies and
  blocked `codescout` itself from publishing past 0.9.0. Now public.
- License unified to MIT across the workspace (was inconsistently Apache-2.0
  in `[workspace.package]` while the repo `LICENSE` file is MIT).

## [0.12.0] — 2026-05-16

### Breaking changes

- **Retrieval substrate replaced.** The in-process sqlite-vec + Tantivy index
  is gone. Semantic search now talks to a network-attached retrieval stack:
  Qdrant (`:6334`), a dense embedder service (`:48081`), a sparse SPLADE
  service (`:48084`), and a cross-encoder reranker (`:48083`). All four ship
  as a single `docker-compose.yml` with `cpu` and `gpu` profiles. See
  [`docs/manual/src/concepts/retrieval-stack.md`](docs/manual/src/concepts/retrieval-stack.md)
  for setup, Ollama/llama.cpp/OpenAI integration, and the benchmark we used
  to pick defaults.
- **`local-embed` is no longer the default Cargo feature.** Default build
  drops `local-embed` from defaults; `cargo install codescout` produces a
  network-only binary. Use `--features local-embed` to re-enable the
  in-process ONNX path; note that it skips the rerank + sparse fusion
  pipeline (benchmark penalty ~9 points on the 75-query suite).
- **Memory IDs are UUIDs now.** `memory.recall` returns string UUIDs (UUIDv5
  of `(project_id, bucket, title)`); the prior integer rowids no longer
  apply. `memory.forget` accepts UUID strings.

### Added

- **`codescout migrate-memories` subcommand** for moving legacy
  `.codescout/embeddings/project.db` content into Qdrant. `--dry-run` previews;
  the active-project banner shows a `⚠ LEGACY INDEX` hint when it detects an
  old file.
- **`CODESCOUT_RERANKER_PROTOCOL=tei|infinity`** env knob for swapping
  between TEI-protocol (default `bge-reranker-v2-m3`) and Infinity-protocol
  (e.g. `jina-rerank-v2`) rerankers without rebuilding.
- **`CODESCOUT_QUERY_PREFIX`** env for asymmetric retrieval models that
  require a query-side prefix (e.g. Nomic, BGE-large).
- **`semantic_search` mode parameter** — `code` (default) excludes markdown
  chunks for implementation queries; `full` includes all indexed content.
- **`call_edges` cache extracted to `.codescout/call_edges.db`** — call-graph
  data is now in its own SQLite file rather than co-housed with the deleted
  chunk storage.
- **Nav-tool eval harness** (`tests/e2e/nav_eval_harness.rs`,
  `tests/fixtures/nav-eval-rust/`). Library-level adversarial eval grading
  action-correctness of `symbols`, `symbol_at`, `references`, and `call_graph`
  on hand-authored Rust ambiguity traps. Run via
  `cargo test --test e2e_tests -- --ignored run_nav_eval`. Verdicts for
  rounds 1-4 under `docs/superpowers/specs/2026-05-15-nav-eval-round-*.md`.
- **Adversarial library-level eval for `edit_code`** — 14 cases across
  replace/insert/remove/rename, graded via composite (return + on-disk content
  + cargo check exit). Shared `eval_common` module factored out of nav-eval.
- **`call_graph` TS same-file fallback (LIMIT-001 Phase A).**
  `CachedResolver::lookup_pos` now falls back to a tree-sitter scan of the
  seed file when both pre-seeded positions and LSP `workspace_symbols` are
  unavailable, rescuing depth-≥2 BFS in LSP-down scenarios. Closes nav-eval
  C-11 (a→b→c→a cycle, depth=5).

### Changed

- **AMD ROCm profile in `docker-compose.yml`.** New `amd` profile alongside
  `cpu` and `gpu`. Dense embedder and cross-encoder reranker run as
  `rocm/llama.cpp:b6652_rocm7.0.0_ubuntu24.04_server` containers with
  `/dev/kfd` + `/dev/dri` passthrough; sparse SPLADE stays on CPU (no llama.cpp
  MLM path). Ports `48081`/`48083`/`48084`/`6334` are profile-agnostic — only
  the underlying container changes. Codescout ships `.env.amd` setting
  `CODESCOUT_EMBEDDER_PROTOCOL=llama-server` and
  `CODESCOUT_RERANKER_PROTOCOL=llama-server`. See
  [`docs/manual/src/concepts/retrieval-stack.md`](docs/manual/src/concepts/retrieval-stack.md)
  → "AMD ROCm profile".
- **Protocol aliases.** `CODESCOUT_EMBEDDER_PROTOCOL` and
  `CODESCOUT_RERANKER_PROTOCOL` now accept `llama-server` (and `llamacpp`,
  `llama_server`) as aliases for the OpenAI/Cohere-compatible shapes, so users
  configuring against llama-server's `/v1/embeddings` and `--reranking`
  endpoints can use the mental model of the backend rather than its wire
  protocol.
- **Binary size 54 MiB → 30 MiB (–44%).** Dropped `ort_sys`, `tokenizers`,
  `hf-hub`, `image`/`ravif` (~22 MiB) by moving `local-embed` out of defaults.
  Dropped `aws_lc_sys` (~2 MiB) by switching `rustls` to the `ring` crypto
  provider. Trimmed `qdrant-client` to `default-features = false, features =
  ["serde"]` to drop a duplicate `reqwest` dep tree.
- **`fs2` → `fs4`** for cross-process file locking (fs2 unmaintained since
  2018).
- **Tool hint strings** updated to current names (`index_project` →
  `index(action='build')`, `Run index_project()` → `Run index(action='build')`).
- **`symbols` `scope="project"`** now filters out stdlib/dependency matches
  whose path is outside the project root (only when scope is strictly
  `Project`, not `All`). Found via nav-eval round 3.
- **`call_graph` BFS** no longer aborts when a non-seed node's resolver
  returns `RecoverableError` — skips that hop and continues, matching the
  existing `lookup_pos` behavior. Found via nav-eval round 3.

### Removed

- `src/embed/index.rs` (~4900 LOC), `src/embed/drift.rs`, `src/embed/bm25.rs`,
  `src/embed/chunker.rs`, `src/embed/local.rs`, `src/embed/remote.rs`.
- `sqlite-vec` and `tantivy` dependencies (still pinned in `[workspace.dependencies]`
  for librarian-mcp, which retains its own sqlite-vec store).
- `percent-encoding` dep (URL handling is fully covered by the `url` crate).

### Fixed

- `semantic_search` now classifies retrieval-stack errors into actionable
  hints (which service is down, how to start it) rather than opaque
  transport errors.
- `memory.delete` (unified-tool path) correctly removes the anchor sidecar
  file alongside the memory entry.
- `create_semantic_anchors` now uses the cross-encoder reranker for anchor
  selection, raising precision.
- `edit_code(replace)` no longer strips preceding `///` doc comments or
  `#[...]` attributes when the new body omits them. The walk-back extension
  introduced for BUG-031 was unconditional; it now narrows back forward past
  decorator lines when `new_body` does not lead with one. Regression:
  `tests/symbol_lsp.rs::replace_symbol_preserves_doc_when_new_body_has_no_doc_comment`
  and edit-eval R-08 (BUG-055).
- Nav-eval runner grades transient LSP `-32801 / content modified` errors as
  SilentWrong (retryable) instead of Panic; the existing retry-on-warmup loop
  now absorbs LSP reindex races.

---
## [0.11.0] — 2026-05-06

### Breaking changes — tool consolidation

- **`replace_symbol`, `insert_code`, `rename_symbol`, `remove_symbol`
  consolidated into `edit_code`** with `action: replace|insert|remove|rename`.
  The four standalone tools are no longer registered. Migration:
    - `replace_symbol(name_path, path, new_body)` → `edit_code(symbol, path, action="replace", body=...)`
    - `insert_code(name_path, path, code, position)` → `edit_code(symbol, path, action="insert", body=..., position=...)`
    - `rename_symbol(name_path, path, new_name)` → `edit_code(symbol, path, action="rename", new_name=...)`
    - `remove_symbol(name_path, path)` → `edit_code(symbol, path, action="remove")`
- **GitHub tools unregistered** (`github_identity`, `github_issue`,
  `github_pr`, `github_file`, `github_repo`). Code remains in `src/tools/github.rs`
  but tools are not exposed to MCP clients. Use the `gh` CLI via `run_command`
  for GitHub operations.

### Added

- **`edit_code`** unified structural-edit tool (consolidates four prior tools).
- **`call_graph`** — transitive caller/callee traversal with `direction` and
  `max_depth` for impact analysis before refactoring.
- **`approve_write`** — session-scoped grant for writes outside the project
  root (e.g. user's home dotfiles).
- **`read_file` source-range gate** — line-range reads that overlap a named
  symbol redirect to `symbols(include_body=true)`. Bypass with `force=true`.
- **JVM pre-warm on activation** for Java/Kotlin projects — the JDTLS / Kotlin
  LSP process starts in the background during `workspace(action="activate")`
  rather than on first symbol query.
- **9 experimental features graduated** to stable docs (`concepts/` from
  `experimental/`): security profiles, diagnostic logging, memory sections
  filter, call_graph, auto-reindex, hybrid search, librarian guide resource,
  artifact_refresh list_stale, augmentation templates.
- **`symbol_at`** + **`references`** added to the prompt's pre-edit
  navigation strategy.

### Changed

- **Librarian-mcp default-on** — the embedded librarian indexer is compiled
  in by default. Runtime registration remains opt-in via `LIBRARIAN_ENABLED=1`
  or `[librarian] enabled = true` in `project.toml`.
- **Librarian tool collapse (16 → 5)** — `artifact`, `artifact_event`,
  `artifact_augment`, `artifact_refresh`, `librarian` cover what 16 individual
  tools did before.
- **`edit_code` rename/replace caller-check hint** appended to success
  responses so the LLM verifies call sites without a separate `references`
  call.

### Fixed

- `edit_code` propagates the caller hint through `format_compact` for large
  rename results (was previously dropped on overflow).
- Stale `replace_symbol` tip in `read_file` blocking error.
- LSP tool enforcement gaps where `symbols` / `references` could be skipped
  in favour of `Read` / `Grep`.

---

## [0.2.2] — 2026-03-11

### Added

- **Hardware-aware embedding model selection** — `onboarding` now picks the best
  available embedding model based on detected hardware (GPU/CPU/Apple Silicon),
  writing the optimal `embedding_model` into `project.toml` automatically.
- **`index_project` progress reporting** — live progress output during indexing
  (files processed, ETA) so long runs are no longer silent.
- **`project_status` trimmed output** — cleaner, more scannable status view with
  memory staleness section (`stale` / `fresh` / `untracked`).
- **Memory staleness detection** — `memory` tool tracks path anchors and semantic
  anchors; `project_status` reports which memories have drifted from their
  source files since last write.
- **Pre-onboarding semantic index gate** — prevents `semantic_search` from
  returning empty results on a freshly cloned project before indexing completes.
- **`language-patterns` shared memory** — curated per-language anti-patterns and
  correct idioms for 7 languages, consulted automatically before code changes.
- **CWD awareness in Agent** — `home_root` tracking so tools resolve relative
  paths correctly when invoked from a subdirectory.
- **LSP `RequestCancelled` retry** — LSP `-32800` errors are now retried
  automatically instead of surfacing as failures.

### Fixed

- **UTF-8 byte-slice crash in onboarding** (BUG-026) — preference text was
  sliced at a byte offset that could fall inside a multi-byte character, causing
  a panic. Now uses `char_indices` for safe truncation.
- **Stale `@bg_*` refs** — background command refs that have expired now return
  a `RecoverableError` with a descriptive hint instead of an opaque failure.
- **`.env` accidentally tracked** — added `.env` to `.gitignore`.

---

## [0.2.1] — 2026-03-09

### Fixed

- **`github_file` schema** — `files` array parameter now includes a proper `items`
  schema (`{ path, content }` object with required fields). VS Code and other
  spec-compliant MCP clients rejected the tool with *"tool parameters array type must
  have items"* because JSON Schema requires `items` on every `array` type.

---

## [0.2.0] — 2026-03-09


> **TL;DR:** The project was renamed from `code-explorer` to `codescout`. If you're
> migrating, update your MCP config and any scripts that reference the old binary name.
> [Full story and migration guide →](docs/manual/src/history.md)

### Breaking changes

- **Binary renamed:** `code-explorer` → `codescout`
- **MCP server ID renamed:** `code-explorer` → `codescout` — update `.mcp.json` or Claude Code settings accordingly
- **Tool renames** (API consistency):

| Old name | New name |
|---|---|
| `get_symbols_overview` | `list_symbols` |
| `find_referencing_symbols` | `find_references` |
| `replace_symbol_body` | `replace_symbol` |
| `insert_before_symbol` + `insert_after_symbol` | `insert_code` (+ `position` param) |
| `execute_shell_command` | `run_command` |
| `create_text_file` | `create_file` |
| `search_for_pattern` | `search_pattern` |
| `search_code` | `semantic_search` |
| `index_stats` | `index_status` |
| `get_current_config` | `get_config` |
| `check_onboarding_performed` | `onboarding` |

- **Tool consolidations** — `insert_before_symbol` + `insert_after_symbol` merged into
  `insert_code(position)`, `is_onboarded` folded into `onboarding(force)`

### Added

#### New tools
- `goto_definition` — LSP-backed jump to symbol definition; auto-discovers libraries
- `hover` — LSP type info and doc comments at a given position
- `insert_code` — insert code before or after a named symbol (replaces the separate
  `insert_before_symbol` / `insert_after_symbol` tools via a `position` parameter)
- `list_libraries` — list all registered external libraries and their index status
- `memory` semantic actions — `remember`, `recall`, `forget`, `refresh_anchors` added
  to the unified `memory` tool for vector-backed episodic memory
- `github_identity`, `github_issue`, `github_pr`, `github_file`, `github_repo` — five
  new GitHub tools backed by the `gh` CLI

> **Note:** `edit_lines` (line-splice editing) and `index_library` (separate library
> index tool) were drafted during this cycle but not shipped. Library indexing is covered
> by `index_project(scope: "lib:<name>")` instead.

#### Library search
- Symbol tools (`list_symbols`, `find_symbol`, `find_references`, `goto_definition`,
  `hover`) and `semantic_search` now accept a `scope` parameter: `"project"` (default),
  `"libraries"`, `"all"`, or `"lib:<name>"` for a specific library
- `LibraryRegistry` — persistent registry; libraries auto-registered via `goto_definition`
  when definitions resolve outside the project
- Manifest discovery auto-registers `Cargo.toml` / `package.json` / `go.mod` paths as
  named libraries

#### Semantic search improvements
- Incremental index: hash-based change detection (git diff → mtime → SHA-256 fallback);
  only changed files are re-indexed
- Semantic drift detection in `index_status` — surfaces files whose content has drifted
  significantly from their indexed embeddings
- sqlite-vec extension replaces hand-rolled Rust cosine loop for distance computation
- AST-aware chunker splits files by symbol boundaries before embedding
- `local-embed` feature flag: fastembed-rs LocalEmbedder for CPU-only inference,
  no Ollama required
- CPU fallback: automatically switches to local model when Ollama is unreachable
- Concurrent embedding with single-transaction writes for faster indexing
- `.cjs` and `.mjs` files now indexed as JavaScript

#### Progressive disclosure
- `OutputGuard` module enforces two output modes across all list/search tools:
  - **Exploring** (default): compact, capped at 200 items, overflow hint included
  - **Focused**: full detail with `detail_level: "full"` + `offset`/`limit` pagination
- `read_file` capped at 200 lines in exploring mode; explicit `start_line`/`end_line`
  bypasses the cap
- `next_offset` field in overflow JSON for seamless pagination

#### Robustness & DX
- Recoverable errors (`RecoverableError`) return `isError: false` with a `hint` field —
  sibling parallel tool calls are not aborted when one tool returns an expected error
- Dynamic server instructions injected into the MCP `initialize` response so Claude
  sees guidance before the first tool call
- `system_prompt` field in `.codescout/project.toml` for project-specific guidance
- Auto-detect project root from the server's working directory on startup
- Configurable per-language LSP init timeout via `lsp_init_timeout_secs`
- `text_sweep` helper: after `rename_symbol`, scans for residual textual occurrences
  (comments, strings, docs) that LSP rename cannot reach
- JetBrains official `kotlin-lsp` replaces community `kotlin-language-server`
- TSX/JSX LSP support via `typescript-language-server`
- tree-sitter support for Java, Kotlin, and TSX
- E2E test fixture projects with TOML-driven data harness
- Windows support: path separators, home directory in security checks, cmd.exe shell

#### Security
- Path sandboxing: all reads/writes validated against project root
- Tool category access controls (read, write, git, index, shell) configurable per-project
- Platform-specific deny-list (SSH keys, `/etc/passwd`, Windows credential stores)

### Changed

- `read_file` now rejects source code files (`.rs`, `.py`, `.ts`, etc.) — forces use of
  symbol tools; pass `start_line`/`end_line` only on non-source files
- `onboarding` redesigned: produces richer project context and memory-creation guidance
- Tool count: 33 → 30 (consolidation of insert tools, removal of git_log/git_diff)
- Default embedding model: `ollama:mxbai-embed-large`

### Removed
- `git_log` tool — use `run_command` with `git log` for file history
- `git_diff` tool — use `run_command` with `git diff` for diffs
- `replace_content` tool — superseded by `replace_symbol` and `edit_lines`

### Fixed
- Ghost blank lines in `replace_symbol` and `insert_code` when replacement body contains
  a trailing newline (`.push(body)` → `.extend(body.lines())`)
- `write_lines` empty-output guard: no longer writes `"\n"` when result is empty
- 1-indexed line numbers in all symbol/AST tool outputs (`start_line`, `end_line`)
- Concurrent `semantic_search` deadlock when multiple calls hit a cold LSP simultaneously
- LSP thundering-herd race condition on cold start (watch-channel barrier)
- LSP deadlock in waiter-retry path and excessive lock hold during shutdown
- Graceful LSP shutdown prevents orphaned language server processes
- `search_pattern` returns `RecoverableError` for invalid regex (not a hard error)
- Char-safe truncation in drift snippets (prevented panic on multibyte Unicode)
- HTTP timeout wired through to embedding client
- Hidden directories (`.worktrees`, `.claude`) excluded from all file walkers
- `git_blame` reads committed content correctly; better error for dirty files
- `SecuritySection::default()` now enables write/git/indexing tools (was too restrictive)

---

## [0.1.0] — 2026-02-25

### Added

#### Core MCP server
- Rust MCP server (`rmcp` 0.1) with 29 tools across 8 categories
- Stdio and HTTP/SSE dual transport — stdio for Claude Code, HTTP for multi-session use
- Library + binary split (`src/lib.rs`) enabling integration tests and external use
- Release profile: `opt-level 3`, thin LTO, symbol stripping

#### File tools (3)
- `read_file` — read files with optional line range
- `list_dir` — directory listing, recursive mode
- `search_for_pattern` — regex search across project files

#### Workflow tools (3)
- `execute_shell_command` — run shell commands in project root
- `create_text_file` — create or overwrite files
- `find_file` — glob pattern file discovery

#### Symbol tools — LSP-backed (7)
- `get_symbols_overview` — hierarchical symbol tree for a file or directory
- `find_symbol` — workspace-wide symbol search by name pattern
- `find_referencing_symbols` — find all usages of a symbol
- `rename_symbol` — rename across the whole workspace
- `replace_symbol_body` — replace the body of a symbol
- `insert_before_symbol` / `insert_after_symbol` — precise code insertion
- JSON-RPC 2.0 LSP client with async stdio transport, 30s timeout, crash recovery
- Language server configs for 9 languages: Rust, Python, TypeScript/JS, Go, Java,
  Kotlin, C/C++, C#, Ruby

#### AST tools — tree-sitter offline (2)
- `list_functions` — extract all function/method signatures
- `extract_docstrings` — extract doc comments with associated symbol names
- **Rust** (`tree-sitter-rust`): functions, structs, enums, traits, impl methods,
  modules, constants — `///` and `//!` doc comments
- **Python** (`tree-sitter-python`): functions, classes, methods, decorated definitions
  — triple-quoted docstrings
- **Go** (`tree-sitter-go`): functions, methods with receiver type, structs, interfaces
  — `//` and `/* */` comments
- **TypeScript** (`tree-sitter-typescript`): functions, classes, interfaces, enums,
  type aliases, export statements — JSDoc `/** */`
- **TSX** (`tree-sitter-typescript` LANGUAGE_TSX): full JSX grammar — same extraction
  as TypeScript
- **Java** (`tree-sitter-java`): classes, interfaces, enums, records, methods,
  constructors, fields, enum constants — Javadoc `/** */`
- **Kotlin** (`tree-sitter-kotlin-ng`): classes, objects, functions, properties, enums,
  companion objects, type aliases, enum entries — KDoc `/** */`

#### Git tools (3)
- `git_blame` — per-line blame with commit SHA, author, timestamp
- `git_log` — file commit history
- `git_diff` — working tree or commit-range diff

#### Semantic search tools (3)
- `search_code` — vector similarity search over indexed codebase
- `index_project` — build/update embedding index (chunked, content-hashed)
- `index_status` — show index stats and coverage

#### Memory tools (4)
- `write_memory` — store named notes per project
- `read_memory` — retrieve a note by topic
- `list_memories` — list all stored topics
- `delete_memory` — remove a note

#### Config tools (2)
- `activate_project` — switch active project root
- `get_current_config` — show config and project root

#### Onboarding tools (2)
- `onboarding` — project discovery: detect languages, structure, create config
- `check_onboarding_performed` — check if onboarding has run

### Infrastructure
- `.mcp.json` — Claude Code MCP config for using codescout on its own source
- 141 tests: 136 unit + 5 end-to-end integration tests
- Integration tests cover: read→search→replace, AST analysis, memory+config roundtrip,
  git history creation, onboarding+explore


## [0.10.0] — 2026-05-01

### Breaking changes — tool surface compression (L3)

| Old name | New name |
|----------|----------|
| `find_symbol`, `list_symbols` | `symbols` |
| `find_references` | `references` |
| `goto_definition`, `hover` | `symbol_at(fields: ["def", "hover"])` |
| `list_dir`, `find_file` | `tree` |
| `activate_project`, `project_status` | `workspace(action: activate / status / list_projects)` |
| `list_libraries`, `register_library` | `library(action: list / register)` |
| `index_project`, `index_status` | `index(action: build / status)` |

Added: `call_graph` stub (implementation tracked in item A).

---
