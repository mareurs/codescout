# Roadmap

The v1 implementation plan this used to link to no longer exists under that name
anywhere in history — v1 shipped and the plan was deleted rather than archived. The
surviving design records live under `docs/superpowers/plans/`.

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
- **Semantic search** — Qdrant + TEI hybrid retrieval (dense + BM25 sparse, RRF fusion); see [benchmark](research/2026-05-06-retrieval-stack-benchmark.md). Memory storage and recall still run on the in-process sqlite-vec path; see [legacy-removal tracker](trackers/2026-05-07-legacy-retrieval-removal.md) ([concepts](manual/src/concepts/semantic-search.md), [backends](manual/src/configuration/embedding-backends.md))
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
- **Companion Claude Code plugin** — `codescout-companion` for tool routing guidance (live at [mareurs/claude-plugins](https://github.com/mareurs/claude-plugins))
- **Usage monitor** — per-tool call stats in `usage.db`, surfaced via the dashboard
- **Semantic memories** — `remember`/`recall`/`forget` actions with sqlite-vec vector search, auto-classification into buckets (code/system/preferences/unstructured), cross-embedding of markdown memories, preferences auto-injection during onboarding
- **Git blame** via git2; persistent memory store (markdown topics + semantic memories)
- **MCP over stdio and HTTP/SSE** (rmcp); 1142 tests passing
- **Debug logging** — `--debug` flag enables structured file logging with rotation (`tracing-appender`)
- **Multi-project workspaces** — `workspace.toml` registration, per-project memory/LSP/indexing, cross-project search guidance, workspace-aware onboarding
- **Library version tracking** — per-library embedding DBs (`.codescout/embeddings/lib/`), lockfile version comparison, staleness hints in `semantic_search`
- **LSP idle TTL eviction** — per-language configurable timeouts (Kotlin 2h, others 30min), transparent shutdown and restart

## What's Next

<!-- audit-doc-refs:ignore — a roadmap names modules, files and tests that do not
     exist yet; that is what makes it a roadmap. Anything here that has since shipped
     should be moved out of this section rather than silenced. -->

**Guidance / prompt-surface work stream (2026-07-03, eval-backed).** Findings synthesis:
[`docs/research/2026-07-03-mcp-guidance-findings.md`](research/2026-07-03-mcp-guidance-findings.md);
memory `research/loadbearing-mcp-guidance`.

- **Server-computed provenance envelope keys** (`refreshed_at_commit`, `commits_behind_head`)
  on `doc(get)` — **SHIPPED 0de733aa (2026-07-04), live-verified.** Emits a
  server-computed `provenance` block and activates the freshness engine's stale-by-commit-
  distance path (`commit_refresh` records HEAD; distance drives `commits_behind_head` +
  `freshness`); the co-located G5 bug (augmentation fields omitted from `doc(get)`) is
  fixed in the same projection. (KEY-PRIORITY 6/6 across two models; CALIBRATE 9-10/10 at
  n=10 pinned Sonnet.) Optional follow-ups: extend provenance to the `context.rs` `[LIVE]`
  bundle + `state_at` time-travel surfaces (deferred by design).
- **Multi-turn eval harness** (prompt-tdd `input.history`) — the standing blocker for the
  whole guidance research line: every time-dependent question (instruction decay,
  re-derivation of returned facts, guidance persistence, adherence at distance) escapes
  single-turn measurement. Unblocks 4 parked findings at once. **SHIPPED (prompt-engineering `3fa2ab0`, 2026-07-04)** — `input.history` replays a persisted session via `claude -p --resume`; assertions target the final turn. Real `--resume` smoke verified live 2026-07-04 (3-turn session: no error, final-turn response returned, and a `CLAUDE.md` one-word rule survived to the last turn).
- **`get_guide` / Iron-Law adherence — clarity audit + just-in-time coverage** *(unblocked;
  reframed by A-10/A-11, 2026-07-05).* The multi-turn harness landed AND answered the decay
  half: **no decay through ~20 turns / ~24k tokens / mid-context position** — once fetched,
  on-demand guidance is as authoritative as `CLAUDE.md`, so the lever is DISCOVERABILITY
  (the auto-inject trigger firing at the right moment) plus per-rule **clarity + merit**
  (A-1 precedent: 30%→90%→100%; A-11 precedent: an "unverifiable" verdict section moved
  report calibration 1–2/5→5/5). Remaining work: Hamsa-audit each Iron-Law/guide rule as a
  stranger would + coverage-check the auto-inject triggers. Audit ledger:
  `docs/trackers/prompt-hamsa-audit-log.md` (now entry-filterable; open items:
  `entry_filter={"outcome":{"eq":""}}` — an open audit's `outcome` is **empty**, the
  convention the tracker's own H1 states: *"`Outcome` starts empty and is filled when
  evidence later arrives."* This line read `{"contains":"pending"}` until 2026-08-28;
  measured that day, it returns **zero rows** while the `eq ""` form returns A-15,
  A-17, A-24. Nothing was wrong with the tracker — the query never matched its
  convention, and no one re-ran it after writing it down.).
- *Shipped this stream:* reader-first tracker prompts (`tracker_design`), the
  `get_guide("untrusted-content")` data-vs-directive rule, and the prompt-tdd
  model-pinning + ambient-credential fixes (`../prompt-engineering` `aecb76f`/`8790c80`).

**Standing backlog:**

**A local gate that covers the non-default feature configs (2026-08-06).** CLAUDE.md's
pre-commit line (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`) covers
**one of nine** CI test cells — the `default` config on the host OS. CI runs a 3x3
matrix (ubuntu/macos/windows x default/local-embed/no-features), and feature-gate rot
is invisible to a default-features build **by construction**: a test file that uses
`codescout::librarian` without `#[cfg(feature = "librarian")]` compiles and passes
locally while failing six matrix cells. That happened twice in one cohort — once at
compile time (`tests/link_scan.rs`, `src/server.rs`'s `make_server`, fixed `7938d68b`)
and once at behaviour time (three `server::tests` asserting the `artifact` tool is
registered, fixed `be75e705`) — and kept CI red for roughly three weeks because the
local gate reports green.

Cheapest guard is a single alias running the five steps that actually gate a merge:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --no-default-features
cargo test --features local-embed --no-default-features
```

A `cargo xtask gate` (or a `[alias]` entry in `.cargo/config.toml`) plus a one-line
CLAUDE.md change pointing at it. The two non-default `cargo test` runs are the whole
value — they are what the host-OS default build cannot see. Note this does NOT cover
the Windows-only failures (`docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`),
which need a real Windows runner. The gate definition lives in
[`CLAUDE.md`](../CLAUDE.md) § *Development Commands* and nowhere else;
[`docs/RELEASE.md`](RELEASE.md) § *Large-Cohort Promotion* holds the ancestry check and
now references the gate rather than restating it. Friction write-up is F-3
in `docs/trackers/release-promotion-session-log.md`.

**Update 2026-08-30 — partly discharged, and the pointer above was aimed at the wrong
file.** The analysis above is kept as the dated record that argued for the change; do not
read its present tense as current. Two corrections. (1) It describes CLAUDE.md's line as
`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` — that is what it said on
2026-08-06 and is no longer true. The lean lane landed: the gate is four commands and
includes `cargo test --workspace --no-default-features`, so it now covers **two** of the
nine CI cells on the host OS (`default` and `no-features`) rather than one. The remaining
config, `--features local-embed --no-default-features`, has a *clippy* lane in the gate but
no *test* lane, and the Windows cells still need a real runner. (2) This entry used to
point at RELEASE.md § *Large-Cohort Promotion* as the "full gate definition". That block
was the least complete of the five transcriptions in the repo — it omitted the lean lane
and ran `cargo clippy --all-targets` with neither `--workspace` nor `--features
local-embed`, which reaches neither `crates/codescout-embed` nor the `local-embed`-gated
module. A reader following this pointer did everything right and landed on the weakest
list. That is the argument for one definition and references everywhere else, and it is
why the pointer now names CLAUDE.md.

**Tracker cross-linking stream (2026-07-05, research-validated, SHIPPED W1–W3 + seed).**
Plan: `~/.claude-sdd/plans/eager-pondering-brook.md`. `link_scan` derives scanner-owned
`rel="cites"` edges from prose citations (dc35c70e); cross-linking convention +
TAXONOMY/guide fixes (439a9c7a); audit-log retrofitted as filterable augmented artifact
(1d10c072). Live: 755 artifacts → 430 edges from zero, idempotent fixpoint verified.
**Residual (2) FIXED (2026-07-05, experiments-only):** `context(anchor_id)` large-anchor
neighbor starvation — root cause was the packing loop's "always include the first item"
guard (deliberate for topic-search, pinned by `tests/max_tokens_caps_inclusion`) admitting
an oversized anchor unconditionally and exhausting the budget before any neighbor was
considered. Fix reserves half of `char_cap` for neighbors and truncates the anchor's own
section when it exceeds that reserve, scoped to anchor-mode only (topic-search behavior
untouched). New test `anchor_neighbors_are_not_starved_by_oversized_anchor`; full gate
green (3033 tests, clippy clean). Bug file closed:
`docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md`.
**Residual (2) DONE (2026-07-05, experiments-only):** dangling/ambiguous triage pass —
while triaging, found and fixed a second real bug:
`src/librarian/tools/link_scan/extract.rs` enabled pulldown_cmark's
`ENABLE_YAML_STYLE_METADATA_BLOCKS`, which pairs up ANY two bare `---` lines anywhere in a
doc as a YAML metadata block, not just leading frontmatter — session-log trackers separate
every entry with a bare `---`, so roughly every other entry's heading was silently swallowed
and never registered as a link-scan `Definition`. Fixed by skipping frontmatter via an
explicit byte-offset guard (reusing `frontmatter::parse`'s delimiter logic) instead of
pulldown_cmark's own heuristic. New test
`bare_dash_separator_between_entries_is_not_mistaken_for_yaml_metadata`; bug file
`docs/issues/archive/2026-07-05-link-scan-yaml-metadata-block-swallows-headings.md` (fixed). Live
re-scan post-fix: dangling 366→332, ambiguous 232→213, +16 previously-invisible edges added,
8 stale edges pruned, idempotent fixpoint reconfirmed (438 edges). Triage of what remains:
**ambiguous** (213) is overwhelmingly the expected, pre-validated consequence of F-N/W-N
being locally-scoped per-tracker (no fix — system correctly declines to guess); **dangling**
(332) breaks into (a) `T-N` tokens that exist only as augmented-artifact `params` rows,
invisible to link_scan's heading-based detector by design (both now documented in
`.codescout/memories/gotchas.md`), (b) a real but already-tracked stale-id drift
(`42dfdfc8b1522192` in `windows-platform-support.md`, F-2 in
`perf-windows-session-log.md`, fix owned by `docs/superpowers/plans/2026-07-02-perf-vdi-closure.md`
Track 3 — not duplicated here), (c) legacy `BUG-NNN`/forward R-N references, low-value
cleanup. Full gate green (3034 tests, clippy clean, MCP live-verified with both fixes).
**Remaining residuals:** (1) cherry-pick the experiments run to master (ship sequence —
gate is green: 3034 tests, clippy clean, MCP live-verified — now carries both the
anchor-starvation and YAML-metadata-block fixes); (2) W4 `review_by:` TTL frontmatter — cut
from v1, revisit only if freshness-by-review-event proves insufficient; (3) three
`audit_doc_refs` bug files from scouting (lsp stub / scope ignored / fail_on mismatch).

- Additional tree-sitter grammars (currently: Rust, Python, TypeScript, Go, Java, Kotlin)
- Additional LSP server configurations
- Configurable LSP idle TTL via `project.toml`
- GitHub tools: `github_issue`, `github_pr` method parity with `github_repo`
- Extend `OutputForm::Text` (ripgrep-style wire format) to remaining bulk-result
  tools — `semantic_search` first (`file:Lstart-Lend (score)\n  preview` shape;
  format_compact already exists at `src/tools/semantic/semantic_search.rs:218`),
  then `memory(action="list"|"recall")` and `IndexStatus`. Pattern established by
  `OutputForm` commits `71c0e90f` / `9c12c404` / `a00c94cb` on `experiments`:
  for each tool, audit `format_compact` for field-loss vs JSON, upgrade the
  formatter if needed (precedent: `format_glob` path-listing patch), then add
  `output_form() = Text` override + regression test. Marginal cost ≈ 30 LOC per
  tool once `format_compact` is complete.
## Future Improvements

Implemented features have been moved to [`FEATURES.md`](FEATURES.md).

### MCP Elicitation Integration ✅ (Partial)

Leverage the MCP elicitation spec (Claude Code 2.1.76, March 2026) for interactive user input:
stdin prompts and PostCompact hook integration.

> Reference: the docs/TODO-mcp-elicitation.md note this pointed at was deleted, not
> moved; nothing in history carries that name today. (Left un-code-spanned on purpose —
> naming a nonexistent path inside backticks is itself a finding.)

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

3. **`codescout-companion` plugin / extension** — a thin adapter per agent platform:
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

<!-- audit-doc-refs:ignore — unbuilt feature: the module and store paths it names
     are the proposed layout, not the current one. -->

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

### Bidirectional Code ↔ Tracker Linking

<!-- audit-doc-refs:ignore — unbuilt feature; src/foo.rs here is the stand-in file an
     agent would be exploring, not a file in this repo. -->

Surface relevant trackers/bugs/plans when an agent explores code, and surface referenced
code locations when an agent reads a tracker — in both directions, automatically, without
a separate manual query.

**Motivation:** `link_scan` (shipped, see "Tracker cross-linking stream" above) already
derives `rel="cites"` edges between *artifacts* from prose citations of IDs. `audit_doc_refs`
(`src/librarian/tools/audit_doc_refs/`) already *extracts* code references from tracker/doc
prose — file paths, symbol names, line numbers, module paths — and checks them against the
current filesystem + LSP symbol index, but only to report staleness in a one-off audit run;
the extracted refs aren't persisted as queryable graph edges. Right now an agent exploring
`src/foo.rs` has no way to discover "there's an open bug about this file" short of grepping
`docs/issues/` by hand, and reading a bug file gives no structured pointer back to the code
it's about beyond whatever's typed in prose.

**Core mechanism:**
1. **Persist code refs as edges.** Reuse `audit_doc_refs`'s parser/resolver
   (`src/librarian/tools/audit_doc_refs/parser.rs`, `resolver.rs`) to extract
   `(artifact_id, file_path, symbol?, line?)` tuples from artifact prose, but instead of
   only using them for one-off staleness reporting, persist them in a new
   `catalog/code_refs.rs` table (parallel to `catalog/links.rs`'s `LinkRow`, except the
   "destination" is a code location, not another artifact) — same idempotent-rebuild
   pattern `link_scan` already established (`link_scan(write=true)` materializes/prunes).
2. **Code → tracker surfacing.** Extend `symbols`, `references`, and/or `read_file`'s
   response envelope with a cheap, indexed lookup: "N tracker(s) reference this file/symbol"
   — a hint line, not a forced fetch, matching the existing `_guide_hint`/progressive-disclosure
   convention. Backed by an index on `code_refs.file_path` (+ optional symbol column).
3. **Tracker → code surfacing.** Extend `doc(get)` to render a "Referenced code"
   block from the same table, and reuse `audit_doc_refs`'s existing filesystem/LSP-symbol
   staleness check to flag drift inline (e.g. "`src/foo.rs:42` — symbol `Bar::baz` not
   found, likely moved/renamed") instead of requiring a separate `audit_doc_refs` run.
4. **Keep it manual-cadence like its siblings.** `link_scan` and `audit_doc_refs` are both
   deliberately manual-trigger, not automatic-on-every-write (cost/staleness tradeoff); this
   should follow the same convention — run periodically (a `link_scan`-adjacent action), not
   re-derived on every edit.

**Implementation sketch:**
- New `src/librarian/catalog/code_refs.rs` + schema addition to `schema.sql`
- Extend `src/librarian/tools/link_scan/` (or add a sibling module) to call
  `audit_doc_refs`'s extraction layer and write `code_refs` rows alongside/instead of
  `cites` edges
- Hook code→tracker surfacing into the existing hint-injection path (`src/tools/output.rs` /
  progressive-disclosure envelope), gated so it doesn't fire when zero refs exist — no hint
  spam on files nobody's ever filed a bug against
- Tracker→code surfacing: new rendered section in `doc(get)`'s body, or a queryable
  field, reusing `audit_doc_refs`'s resolver for live staleness

**Related:** `link_scan` (`src/librarian/tools/link_scan/`) — the shipped artifact↔artifact
sibling this generalizes from. `audit_doc_refs` (`src/librarian/tools/audit_doc_refs/`) —
already does the hard part (extraction + staleness classification) for a one-off report;
this makes that extraction durable and queryable instead of ephemeral.

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

**Design doc:** [`docs/superpowers/plans/2026-03-01-interactive-sessions-design.md`](superpowers/plans/2026-03-01-interactive-sessions-design.md)
**Implementation plan:** [`docs/superpowers/plans/2026-03-01-interactive-sessions-plan.md`](superpowers/plans/2026-03-01-interactive-sessions-plan.md)

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

**Context:** the Kotlin concurrent-instance bug file this cited was pruned as a duplicate
in `c6184884`, after the fix landed in `dc44ac3d` (per-instance system-path, circuit
breaker, diagnostics). The canonical account is now codescout memory `gotchas` § LSP; the
last Kotlin bug in that family closed on
2026-08-15 and is archived at
`docs/issues/archive/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`.

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
`src/tools/run_command/`; companion plugin's `pre-tool-guard.sh` already
funnels every Bash call through `run_command`, so the audit hook covers all
agent shell activity.

### `audit_doc_refs`: `scope=repo`/`umbrella` widening

**Priority:** Low | **Effort:** Medium

`librarian(action="audit_doc_refs")` declares the generic `scope` param in
its schema, but as of `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md`
it only ever scans the active project — any other `scope` value is now
rejected with a `RecoverableError` ("audit_doc_refs is project-scoped in
v1") rather than silently ignored.

The reason it isn't simply implemented like `context`/`reindex`/`link_scan`'s
`scope` handling: those tools query the SQL catalog, so widening scope is a
`WHERE` clause change (`apply_scope` in `src/librarian/tools/scope.rs`).
`audit_doc_refs` walks the filesystem directly
(`collect_markdown_files(&repo_root, ...)` in `audit_doc_refs/mod.rs`) —
widening to `repo` or `umbrella` means scanning multiple project roots and
aggregating findings/parse-warnings/tracker state across repos with
potentially different `docs/trackers/doc-ref-audit.md` targets. That's real
feature work, not a bug-fix-sized change.

**Sketch (not yet implemented):**
- For `scope="repo"`: walk from `ctx.current_project.git_root` instead of
  `.abs_path` — likely the easy half, single root, just a wider glob base.
- For `scope="umbrella"`: iterate `ws.umbrellas[].members`, run the existing
  scan per member root, and decide how findings/exit_code/tracker emission
  aggregate — one tracker per member (current per-project behavior,
  unchanged) vs. one umbrella-level rollup tracker (new concept). Needs a
  design decision, not just plumbing.
- `scope="all"` presumably means "every registered workspace root" —
  same aggregation question as umbrella, at a larger scale.
### Distill Fable's Tracker Fluency into a Cross-Model Pattern

Mine session traces to understand *why* `claude-fable-5` reaches for trackers (librarian
artifacts, `append_entry`, session logs) far more fluently than other models — sometimes to
excess — and turn that mechanism into transferable knowledge/skill for any agent.

**Motivation:** Observation from real traces (2026-07): Fable uses codescout's tracker
surfaces both *more often* and *more appropriately* than Opus/Sonnet, though occasionally
to a fault (redundant or over-eager appends). If we can name the underlying mechanism —
prompt-surface sensitivity, a disposition to externalize working state, reward coupling with
structured note-taking — we can (a) codify the good behavior as model-agnostic guidance other
agents can follow, and (b) trim Fable's over-use without losing the instinct. This is the
mirror image of the `fable-tuning` work stream: there Fable is the *patient* (recover lost
quality); here Fable is the *positive exemplar* to learn from.

**Approach sketch:**

1. **Corpus** — pull comparable tracker-eligible tasks across models (Fable, Opus, Sonnet)
   from Langfuse + local JSONL — the same sources the `fable-tuning` stream already mines.
2. **Compare** — quantify tracker-tool usage per model: call frequency, *appropriateness*
   (right entry on the right surface), and over-use rate (redundant/excessive appends).
3. **Mechanism** — form and test hypotheses for the delta; use the fable-tuning
   subtract-and-measure method (T-11) to isolate which prompt/tool signal drives the behavior.
4. **Distill** — write the validated pattern as a memory or a skill, phrased model-agnostically
   so Opus/Sonnet can adopt it; feed the excess-suppression findings back into Fable tuning.

**Related:** `fable-tuning` work stream — trackers `ca8c26fecbbc4f37` (index),
`35de33286cd34f87` (findings), `ab2170158c7d264e` (research / trace evidence). Distilled
patterns feed Tool Usage Patterns (`f2ecdd76a6189efb`).

---
### Practice Rules — Injecting codescout's Own Working Rules Into Skills We Don't Own

**Proposal: `CAP-10`** in [`docs/trackers/capability-proposals.md`](trackers/capability-proposals.md).

codescout's hard-won working rules — how to trust a number, when a green result proves
nothing, what makes a claim checkable — are model-agnostic and live in three places that
never reach the moment the rule applies: `CLAUDE.md` (Claude Code only, loaded once),
`get_guide` (tool contracts, not practice), and the session-log ledgers (durable, never
surfaced unprompted).

The surfaces where they *would* fire are third-party skills — `superpowers:writing-plans`,
`subagent-driven-development` — and **we cannot edit those**: they sit in a plugin cache
that updates overwrite, and an edit would not travel to another machine, profile, or agent.

**Measured 2026-08-20**, executing the Statement-validity plan through
`subagent-driven-development`: **six of six task briefs contained code defects**, all from
one cause — the plan's Rust was written from `symbols(path=…)` overviews rather than
function bodies. Wrong capture indices, a crate that is not a dependency, a constructor's
return type, a field's type, non-existent test helpers, and *two* independent hand-rolled
reimplementations of a date function that already existed. One rule prevents all six:
**a plan that names a function must have opened it** — and it belongs in `writing-plans`.

Most of the mechanism exists already. `get_guide` does just-in-time injection with a
per-conversation ledger that survives `/mcp` reconnects; the rules exist as prose in
`reconnaissance-patterns.md` and the Iron Rules; and the in-flight
[Statement-validity spec](superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md)
supplies the promotion metadata, since **a practice rule is exactly a promoted Statement**.
What is missing is curation and a trigger.

The open question is activity detection — explicit call, inference from the tool sequence,
or a hook (rejected: Claude-Code-only defeats the point). See CAP-10's *Open decisions*.

## Contributor Skills

Three Claude Code skills living in `.claude/skills/` within this repo. Contributors who open codescout in Claude Code get them automatically — no build step required. The design doc this referenced (2026-02-26-contributor-skills-design.md) is not in the
tree under that name; the skills themselves are the surviving record.

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
