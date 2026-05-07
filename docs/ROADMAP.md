# Roadmap

See the detailed implementation plan: [`plans/2026-02-25-v1-implementation-plan.md`](plans/2026-02-25-v1-implementation-plan.md)

## Quick Status

| Phase | Description | Sprints | Status |
|-------|-------------|---------|--------|
| 0 | Architecture Foundation (ToolContext) | 0.1 | **Done** |
| 1 | Wire Existing Backends | 1.1–1.4 | **Done** |
| 2 | Complete File Tools | 2.1 | **Done** |
| 3 | LSP Client | 3.1–3.5 | **Done** |
| 4 | Tree-sitter AST Engine | 4.1–4.2 | **Done** |
| 5 | Polish & v1.0 | 5.1–5.3 | **In progress** |

## What's Built

See [`FEATURES.md`](FEATURES.md) for the full feature reference. Summary:

- **29 tools** across 7 categories (file, workflow, symbol, semantic, memory, config/nav, GitHub)
- **LSP client** — transport, lifecycle, document symbols, references, definition, hover, rename + text sweep
- **Tree-sitter AST** — symbol extraction + docstrings for Rust, Python, TypeScript, Go, Java, Kotlin
- **Semantic search** — Qdrant + TEI hybrid retrieval (dense + BM25 sparse, RRF fusion) is the default since Phase 6 ([benchmark](research/2026-05-06-retrieval-stack-benchmark.md)); legacy in-process sqlite-vec `vec0` KNN remains available via `CODESCOUT_RETRIEVAL_BACKEND=legacy` ([concepts](manual/src/concepts/semantic-search.md), [backends](manual/src/configuration/embedding-backends.md))
- **Library search** — navigate third-party deps via LSP-inferred discovery, scoped symbol nav + semantic search
- **OutputBuffer** — `@cmd_*` / `@file_*` handles; large output stored, queried with Unix tools
- **run_command** — cwd, acknowledge_risk, dangerous-cmd speed bump, smart summaries per command type
- **read_file** — smart buffering with per-type summarizers; source files require symbol tools or start/end lines
- **Dual-audience output** — 8 tools emit structured JSON for agents + readable preview for humans
- **Progressive discoverability** — overflow responses include `by_file` breakdown + narrowing hints; `kind` filter
- **edit_file / remove_symbol** — find-and-replace and symbol deletion with security gating
- **Worktree write guard** — advisory `worktree_hint` field prevents silent cross-worktree corruption
- **Symbol signatures** — LSP `detail` field captured; `signature` synthesized for display
- **Project customization** — `.codescout/system-prompt.md` injects project-specific agent guidance
- **Onboarding** — language-specific nav hints, system-prompt draft generation
- **RecoverableError** — non-fatal tool failures don't abort sibling parallel calls
- **Dashboard** — `codescout dashboard` web UI with tool stats and project health ([concept page](manual/src/concepts/dashboard.md))
- **Companion Claude Code plugin** — `code-explorer-routing` for tool routing guidance (live at [mareurs/claude-plugins](https://github.com/mareurs/claude-plugins))
- **Usage monitor** — per-tool call stats in `usage.db`, surfaced via the dashboard
- **Semantic memories** — `remember`/`recall`/`forget` actions with sqlite-vec vector search, auto-classification into buckets (code/system/preferences/unstructured), cross-embedding of markdown memories, preferences auto-injection during onboarding
- **Git blame** via git2; persistent memory store (markdown topics + semantic memories)
- **MCP over stdio and HTTP/SSE** (rmcp); 1142 tests passing
- **Debug logging** — `--debug` flag enables structured file logging with rotation (`tracing-appender`)
- **Multi-project workspaces** — `workspace.toml` registration, per-project memory/LSP/indexing, cross-project search guidance, workspace-aware onboarding
- **Library version tracking** — per-library embedding DBs (`.codescout/embeddings/lib/`), lockfile version comparison, staleness hints in `semantic_search`
- **LSP idle TTL eviction** — per-language configurable timeouts (Kotlin 2h, others 30min), transparent shutdown and restart

## What's Next

- Additional tree-sitter grammars (currently: Rust, Python, TypeScript, Go, Java, Kotlin)
- Additional LSP server configurations
- Configurable LSP idle TTL via `project.toml`
- GitHub tools: `github_issue`, `github_pr` method parity with `github_repo`

## Future Improvements

Implemented features have been moved to [`FEATURES.md`](FEATURES.md).

### MCP Elicitation Integration ✅ (Partial)

Leverage the MCP elicitation spec (Claude Code 2.1.76, March 2026) for interactive user input:
stdin prompts and PostCompact hook integration.

> Reference: [`docs/TODO-mcp-elicitation.md`](../TODO-mcp-elicitation.md)

**Implemented:**

- **E-0: Elicitation plumbing** ✅ — Added `elicitation/requestInput` support to `ToolContext`.
  Helper: `ctx.elicit(message, schema) -> ElicitResult`. Integrated into `ServerHandler` so
  any tool can call it.

- **E-3: Interactive sessions via elicitation** ✅ — `run_command(interactive: true)` drives
  a process with piped stdin/stdout/stderr. Each round: display accumulated output, elicit
  user input, feed to stdin. Settle detection: 150 ms silence window. Max 50 rounds guard.
  Note: Practical for slow-interaction CLIs (setup wizards, REPLs); unsuitable for
  high-frequency TUIs (ncurses, vim) due to MCP round-trip latency (~1–3 s).

- **E-5: `PostCompact` hook integration** ✅ — Register for Claude Code's `PostCompact` hook.
  On fire: invalidate stale LSP position caches (symbol positions shift when files change
  during compaction). Optionally re-inject fresh project status into next request's
  server instructions.

- **E-6c: Auto-register Cargo dependencies** ✅ — During `activate_project`, scan `Cargo.lock`
  and auto-register top N dependencies as libraries. Eliminates manual `register_library`
  calls; `find_symbol(scope="lib:...")` immediately available (fixes BUG-022).

**Removed (by design):**

- **E-1: Tool disambiguation** ❌ — Removed. Elicitation is server→human, not server→AI.
  Disambiguation (e.g., "which symbol match?") should be handled autonomously by the AI agent
  based on context and heuristics. The LLM can reason about the most likely match given
  conversation state.

- **E-2: Dangerous command confirmation** ❌ — Removed. The two-round-trip `pending_ack` /
  `acknowledge_risk` pattern works well for autonomous AI agents. Elicitation disrupts the
  agent's autonomy and should be reserved for interactive human input, not confirmation loops.

- **E-4: Mutation confirmation** ❌ — Removed. Same reasoning as E-1 and E-2: disambiguation
  and confirmation should be handled autonomously by the AI agent, not via server-to-human
  elicitation.

- **E-6a/E-6b: PreToolUse hook proposals** ❌ — Research showed PreToolUse hooks cannot
  trigger elicitation (they only return allow/block). Proposed `suggest_alternative` field
  deferred pending deeper design of agent guidance patterns.

---

### Multi-Agent Support (Generalize Beyond Claude Code)

Make codescout usable by any MCP-capable agent — Copilot, Cursor, Cline, custom agents — with routing knowledge included so agents know *when* to reach for each tool.

**Motivation:** The server already speaks MCP over stdio. The gap is that agents other than Claude Code lack the curated routing guidance (the `server_instructions.md` prompt) that tells Claude *how* to choose between `semantic_search`, `find_symbol`, `list_symbols`, etc. Without this, agents default to over-using a single tool (usually semantic search).

**Work streams:**

1. **HTTP/SSE transport** (already planned) — lets non-CLI agents connect without spawning a subprocess.

2. **Agent-neutral routing prompt** — refactor `server_instructions.md` into a well-structured decision tree that any agent can consume as a system prompt or tool description prefix. Avoid Claude-specific framing.

3. **`code-explorer-routing` plugin / extension** — a thin adapter per agent platform:
   - Claude Code: existing plugin approach
   - VS Code Copilot: Language Model Tools API (`vscode.lm.registerTool`)
   - Cursor: `.cursorrules` + MCP config
   - Generic: OpenAPI spec + routing hints as tool descriptions

4. **Tool description quality** — every tool's `description()` should embed just enough routing guidance to work even without a system prompt (one-sentence "prefer this over X when Y" hint).

5. **Benchmark routing quality** — extend the live benchmark to test tool selection accuracy across agent backends, not just result quality.

---

### Filesystem Watcher (Realtime Index Updates)

Background filesystem watcher for near-realtime index updates. **Depends on** Incremental Index Rebuilding (Layer 2 of that design).

**Motivation:** Layers 0+1 of incremental indexing cover commit-oriented workflows well, but some users want the index to stay current as they edit — especially in long coding sessions where commits are infrequent.

**Implementation sketch:**
- Use the `notify` crate (cross-platform: inotify on Linux, FSEvents on macOS, ReadDirectoryChangesW on Windows)
- Spawn a background `tokio::spawn` task in the MCP server on startup
- Debounce events with a 2s window to batch rapid saves
- Filter events through `.gitignore` + `ignored_paths` config
- Call `diff_and_reindex` with a per-file candidate list (no git diff needed — watcher knows exactly which files changed)
- Opt-in via `project.toml`: `[index] watch = true`

**Platform considerations:**
- Linux: `inotify` has a per-user watch limit (`fs.inotify.max_user_watches`), may need guidance for large repos
- macOS: `FSEvents` is directory-level, efficient for large trees
- Windows: `ReadDirectoryChangesW` works but has buffer overflow edge cases on burst writes
- The `notify` crate abstracts all of this, but platform-specific tuning docs may be needed

---

### Glossary & Documentation Management (Hash-Based Change Tracking)

Maintain project glossaries and documentation that stay in sync with the codebase via content-hash change detection.

**Motivation:** LLM-generated documentation (onboarding summaries, architecture glossaries, API docs) goes stale the moment the underlying code changes. Manual upkeep is unsustainable. By tracking file content hashes, codescout can detect *which* documented files changed, compute targeted diffs, and trigger glossary/documentation updates — keeping project knowledge accurate without full re-indexing.

**Core mechanism:**

1. **Hash tracking** — Store a content hash (e.g. SHA-256) for every file that contributes to a glossary or documentation entry. Persist in `.codescout/doc-hashes.db` (SQLite, same pattern as `embeddings.db`).

2. **Change detection** — On a `check_docs` or `sync_docs` tool call (or automatically during `onboarding`), compare stored hashes against current file content. Files with mismatched hashes are flagged as stale.

3. **Targeted diff** — For each stale file, compute a diff (reusing `git/diff` infra or direct content comparison). Surface only the *meaningful* changes (skip whitespace-only, comment-only changes via configurable filters).

4. **Update trigger** — Present the diffs to the LLM with the current glossary entry, prompting a targeted update rather than a full rewrite. Alternatively, for structured glossaries, apply rule-based updates (renamed symbol → rename in glossary).

**Glossary features:**
- **Term extraction** — Build a glossary from codebase symbols, domain concepts, and abbreviations (combining AST/LSP data with semantic search)
- **Cross-reference** — Link glossary terms to source locations (file:line), kept accurate via hash tracking
- **Scope** — Per-project glossary in `.codescout/glossary.md` or structured `.codescout/glossary.json`

**Documentation management features:**
- **Doc registration** — `register_doc(path, sources: [file globs])` links a documentation file to the source files it describes
- **Staleness report** — `check_docs()` tool returns which docs are stale, what changed, and suggested update scope
- **Auto-update** — `sync_docs(path)` re-generates or patches a specific doc using the diffs as context

**Storage schema (doc-hashes.db):**
```sql
CREATE TABLE doc_sources (
    doc_path    TEXT NOT NULL,     -- the documentation/glossary file
    source_path TEXT NOT NULL,     -- a source file it depends on
    hash        TEXT NOT NULL,     -- SHA-256 of source content at last sync
    synced_at   TEXT NOT NULL,     -- ISO 8601 timestamp
    PRIMARY KEY (doc_path, source_path)
);
```

**Implementation sketch:**
- New `src/tools/docs.rs` module with `register_doc`, `check_docs`, `sync_docs`, `build_glossary` tools
- `src/docs/` module for hash computation, staleness detection, diff generation
- Integration with existing memory store — glossary terms can cross-reference memory topics
- Progressive disclosure: `check_docs` in exploring mode shows only stale counts; focused mode shows full diffs

**Example workflow:**
1. Onboarding creates `glossary.md` with key terms and `architecture.md` summary
2. `register_doc("glossary.md", sources: ["src/**/*.rs"])` tracks all Rust source hashes
3. Developer adds a new tool module — hash changes detected on next `check_docs()`
4. LLM receives: "3 files changed since last sync" + targeted diffs → updates glossary with new tool's terms

---

### Interactive Sessions

Allow the agent to interact with long-running processes — REPLs, debuggers, and confirmation prompts — instead of waiting for them to exit.

**Motivation:** `run_command` currently blocks until the process exits. Commands like `python3 -i`, `pdb`, or `npm install` (with y/n prompts) hang until timeout. There is no way for the agent to send input to a running process.

**Design:** Three tools built on a `SessionStore` (analogous to `OutputBuffer`):

| Tool | Purpose |
|------|---------|
| `run_command(interactive: true)` | Spawns with piped I/O, waits for initial output to settle, returns a `@ses_<hex>` session handle |
| `session_send(session_id, input)` | Writes a line to stdin, waits for settle window of silence, returns the output delta |
| `session_cancel(session_id)` | Kills the process and frees all resources |

**Settle detection:** After each write, poll the output buffer every 10ms. When 150ms passes with no new bytes, the response is considered complete. Configurable via `settle_ms`. No prompt-pattern knowledge needed.

**Scope:** REPLs, debuggers, confirmation flows. Full-screen TUI apps (vim, less) are explicitly out of scope — no PTY allocation.

**Design doc:** [`plans/2026-03-01-interactive-sessions-design.md`](plans/2026-03-01-interactive-sessions-design.md)
**Implementation plan:** [`plans/2026-03-01-interactive-sessions-plan.md`](plans/2026-03-01-interactive-sessions-plan.md)

---

### Auto-Memories with Temporal Decay

Automatically capture and surface contextual knowledge — code gotchas, deployment
pitfalls, debugging insights — with a decay mechanism that lets transitory memories
fade while persistent truths remain.

**Motivation:** Agents frequently rediscover the same gotchas ("this test is flaky
on CI", "don't forget to restart Ollama after config changes", "the LSP crashes if
you open >50 files"). Currently these are lost between sessions. The `remember`
action requires explicit invocation — most insights slip through. Auto-memories
capture them passively, but some gotchas are temporary (a bug gets fixed, a
workaround becomes unnecessary), so blind accumulation would pollute the context
with stale advice.

**Auto-capture triggers:**
- Agent hits an error and recovers → capture the recovery pattern
- Agent deviates from a preference with confirmation → capture the exception
- Agent discovers a non-obvious build/deploy step → capture as system gotcha
- User says "watch out for..." or "this is tricky" → capture as code gotcha

**Decay mechanism — confidence scoring:**

Each auto-memory gets a `confidence` score (0.0–1.0) and a `last_verified` timestamp:

```sql
ALTER TABLE memories ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0;
ALTER TABLE memories ADD COLUMN last_verified TEXT;
ALTER TABLE memories ADD COLUMN auto_captured BOOLEAN DEFAULT 0;
```

Decay rules:
1. **Time-based decay:** Auto-captured memories lose confidence over time
   (e.g., -0.1 per month since `last_verified`). Manually created memories
   (`remember`) don't decay.
2. **Verification prompts:** During onboarding, if low-confidence memories exist
   (< 0.5), the system prompt includes: "These memories may be outdated — verify
   if they still apply: [list]". Agent confirmation resets confidence to 1.0.
3. **Contradiction detection:** If an auto-memory says "X doesn't work" but the
   agent successfully does X, flag for review.
4. **Garbage collection:** Memories below 0.1 confidence are auto-archived
   (moved to a `memories_archive` table, not deleted — recoverable if needed).

**Bucket extensions:**
- `code_gotcha` — tricky code behaviors, non-obvious API contracts, flaky tests
- `deploy_gotcha` — deployment pitfalls, environment-specific issues
- Both are sub-types of the existing buckets, tagged via a `sub_bucket` column

**Integration with preferences:**
- Preferences don't decay (they're intentional)
- Gotchas decay (they may be transitory)
- Both are auto-injected during onboarding, but gotchas show their confidence
  score so agents can judge reliability

**Design doc:** TBD

---


### LSP Temp Dir Cleanup

**Priority:** Low | **Effort:** Small

kotlin-lsp instances use per-process `--system-path=/tmp/codescout-<PID>-kotlin-lsp`
dirs for workspace isolation. These are tiny (~4KB) and PID-scoped, but accumulate
across sessions until OS reboot clears `/tmp`.

**Options:**
- Clean up in `LspManager::evict_idle` or `LspClient::shutdown` — requires plumbing
  the system-path from `LspServerConfig` through to the cleanup site
- Periodic sweep: `glob /tmp/codescout-*-kotlin-lsp`, skip dirs whose PID is alive
- Do nothing — `/tmp` is self-cleaning and dirs are negligible

**Context:** See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`

### Dangerous-Command Audit Log

**Priority:** Medium | **Effort:** Small/Medium

`run_command` already detects dangerous commands (sudo, `rm -rf`, force-push, etc.)
and routes them through the two-round-trip `acknowledge_risk` gate. What it does
**not** do today is record those calls anywhere durable — once the response is
returned, the command and its output are only visible in the live `@cmd_*`
buffer (LRU, 20 entries, session-scoped) and the optional `--diagnostic` log.

For projects where multiple agents share the same shell-enabled instance, this
is too thin: there is no after-the-fact way to answer "what `sudo` calls did the
agent run last week, on which project, with what output?".

**Goal:** persist every dangerous-command invocation (and the user's
acknowledge_risk decision) to a per-project audit log that survives restarts
and is straightforward to inspect.

**Sketch:**
- New table in `.codescout/usage.db` (already SQLite; see `src/usage/db.rs`):
  `dangerous_commands(rowid, ts_unix_ms, project, cwd, command, matched_pattern,
  acknowledged, exit_code, stdout_excerpt, stderr_excerpt, agent_id)`.
- Write site: the existing dangerous-detection branch in `run_command` —
  log on both rejection (first round-trip) and execution (second round-trip
  with `acknowledge_risk: true`), so denials are auditable too.
- Read site: a new tool, e.g. `audit_log(action="list", since="…", project="…")`
  or extend `project_status` with a recent-dangerous-commands tail.
- Excerpts capped (e.g. first/last 32 lines) so the table stays small.
- Optional: opt-out via `[security] audit_disabled = true` in `project.toml`,
  but default ON — auditability beats convenience for this category.

**Open questions:**
- Where do shell calls inside sub-agents get attributed? Tag with the
  spawning session id if available.
- Should we hash the command instead of storing it verbatim when secrets
  are likely (e.g. `--password=`)? Maybe redact known secret-flag patterns
  before insert.
- Surface a UI in the dashboard (`src/dashboard/`) — list view + per-row
  drill-down to the buffered output if the cmd_id is still alive.

**Context:** see `is_dangerous_command` and the `acknowledge_risk` flow in
`src/tools/run_command.rs`; companion plugin's `pre-tool-guard.sh` already
funnels every Bash call through `run_command`, so the audit hook covers all
agent shell activity.
## Contributor Skills

Three Claude Code skills living in `.claude/skills/` within this repo. Contributors who open codescout in Claude Code get them automatically — no build step required. See [`plans/2026-02-26-contributor-skills-design.md`](plans/2026-02-26-contributor-skills-design.md) for the full design.

| Skill | Purpose | Status |
|---|---|---|
| `project-management` | Navigate sprint status, roadmap, open PRs and issues | Planned |
| `debugging` | Systematic debugging workflow for the Rust codebase | Planned |
| `log-stat-analyzer` | Analyze `usage.db` for call pattern drift and latency regressions | Ready |

### `project-management`

Surface current sprint status from the roadmap, map recent commits to sprint items, and guide contributors through opening correctly-structured PRs. Uses `run_command` with `git log`, `run_command` with `git diff`, and the GitHub MCP tools alongside `docs/ROADMAP.md` and `docs/plans/`.

### `debugging`

Systematic workflow from symptom to fix to verification — covering build failures, test failures, LSP timeouts, tree-sitter parse errors, and embedding pipeline issues. Guides contributors through hypothesis formation (`semantic_search`, `find_symbol`), targeted investigation (`run_command("git log/blame")`, `search_pattern`), and the `cargo build` / `cargo test` / `cargo clippy` verification loop.

### `log-stat-analyzer`

Structured workflow for interpreting Tool Usage Monitor data: per-tool call counts, error rates, p50/p99 latency, overflow rates, and time-bucketed drift detection. Produces actionable summaries (e.g. "semantic_search error rate up 3× in last 24h"). Uses the dashboard (`codescout dashboard`).
