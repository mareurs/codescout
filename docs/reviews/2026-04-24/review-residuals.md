# Review Residuals — 2026-04-24

Findings deferred from the 2026-04-24 code review. Each entry tracks why it was
deferred and what's needed before tackling it.

**Source phase files:** `docs/reviews/2026-04-24/phase-N-*.md` (1 through 9).
**Landed fixes:** see the Fix Status table in [README.md](README.md).
**Branch:** `review/2026-04-24` (three commits so far — phases 1, 2, 3).

## How to use this file

- Each entry is a deferred finding with a clear **Why deferred** and
  **Unblock checklist**. Work through checklists in order.
- When a finding ships, delete its entry here and update the Fix Status
  table in README.md.
- New phase-N deferrals go under `# Phase N — <area>` at the bottom
  (phases 1, 2, 3 already present; phases 4–9 to come).

---

---

# Phase 2 — Tools
## F4 — `validate_read_path` does not enforce project-root containment

- **Source:** `docs/reviews/2026-04-24/phase-2-tools.md` § Security F4
- **Location:** `src/util/path_security.rs:189-231`
- **Summary:** Read paths are checked against a deny-list only; there is no
  project/library/extra-read-root containment. A tool call like
  `read_file("/home/user/.aws/credentials")` is permitted unless the path is
  deny-listed. `glob(path="/", pattern="**/*.pem")` walks the whole FS.
- **Why deferred:** Prior experience — agents were confused when codescout
  stripped `cwd` from paths. Any containment change here risks regressions in
  cross-project navigation and library reads. We do not have enough test
  coverage to make the change safely.
- **Unblock checklist:**
  1. Enumerate current read call sites and their expected roots
     (project, library_paths, extra_read_roots, memory dirs, workspace root).
  2. Add integration tests covering each legitimate cross-root read.
  3. Add integration tests covering each leak scenario we want to block
     (`~/.aws`, `~/.ssh`, `~/.config/gh`, `~/.netrc`, `/etc/shadow`, `/`).
  4. Then decide: contain reads, or keep permissive and expand deny-list.
- **Status:** open, low priority.

---

## C5 — `edit_file.prepend`/`append` skip def-keyword guard

- **Source:** `docs/reviews/2026-04-24/phase-2-tools.md` § Critical C5
- **Location:** `src/tools/edit_file.rs` (insert mode arm)
- **Summary:** The def-keyword guard blocks multi-line structural rewrites in
  single-edit and batch modes, but is *not* applied when `insert: "prepend"` or
  `insert: "append"` is used. A caller could append a whole function body via
  `append` and bypass the "use replace_symbol" discipline.
- **Why deferred:** Telemetry (~44 usage DBs, 1,680 `edit_file` calls) shows
  ~81% of `append` calls add a new symbol (test function, new `mod`/`impl`,
  etc.) — these are *legitimate* adds, not rewrites. A hard guard here would
  produce a high false-positive rate and push users to work around it, which
  is worse than the current state. A softer hint guiding toward `insert_code`
  would be better but needs its own design pass.
- **Unblock checklist:**
  1. Design a hint-level (not block-level) signal for append/prepend.
  2. Decide whether to route legitimate "append a new symbol" through
     `insert_code` with `position: "after"` on the last top-level symbol.
  3. Consider whether batch `new_string` deserves its own guard (21% flagged
     in telemetry — also mostly legitimate adds).
- **Status:** open, low priority.

---

## I5 — `inject_tee` pipe detection by string-parse is fragile

- **Source:** `docs/reviews/2026-04-24/phase-2-tools.md` § Important I5
- **Location:** `src/tools/run_command.rs:670` (`detect_terminal_filter`)
- **Summary:** Pipe detection walks the command string looking for `|` to
  decide where to inject `| tee <file>`. A `|` inside a quoted string (e.g.
  `echo "a | b" | head`) is treated as a pipe operator, so `tee` ends up
  injected mid-quote, producing a broken command.
- **Why deferred:** Needs shellwords-aware tokenization. Options: add the
  `shell-words` crate, or port a minimal tokenizer. Either way it's a
  non-trivial refactor that touches a hot path. No bug report from users yet.
- **Unblock checklist:**
  1. Reproduce the broken-injection case with a unit test.
  2. Pick a tokenizer (ideally `shell-words`, already widely-used).
  3. Replace the string-scan with tokenizer output that distinguishes
     operator `|` from literal.
- **Status:** open, low priority.

---

## Other phase-2 minors (not yet addressed)

- **`ProjectStatus` swallows `WorkspaceConfig` parse failures**
  (`src/tools/config.rs:240-260`) — should at least `tracing::warn!` rather
  than silently returning defaults.
- **`memory(action="write")` boundary validation** — relies on deeper
  `sanitize_topic`; defense-in-depth could reject newline/control chars at
  the tool boundary.
- **`perform_edit` double scan** — `matches` + `match_indices` in sequence
  could fuse into a single pass (tiny win).

---

# Phase 3 — LSP

## S1 — LSP server spawn via `$PATH` lookup

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § S1
- **Location:** `src/lsp/servers/mod.rs:14-135` (`default_config`);
  `src/lsp/client.rs:223` (`Command::new(&config.command)`)
- **Summary:** All configured LSP commands are bare binary names
  (`rust-analyzer`, `pyright-langserver`, `kotlin-lsp`). `Command::new` does a
  `$PATH` lookup; a polluted `$PATH` (project `node_modules/.bin`, `.envrc`
  prepend, `~/.local/bin`) could silently swap in an attacker binary.
- **Why deferred:** Cross-platform `$PATH` semantics differ (macOS `/opt/homebrew`
  vs Linux `/usr/local/bin` vs Windows `%PATH%` with `.exe`/`.cmd` extensions);
  `which::which()` is the right primitive but needs testing on all three. No
  active exploit.
- **Unblock checklist:**
  1. Resolve via `which::which()` once per server; stash canonical path.
  2. On Unix, refuse if the resolved binary is writable by the current user
     (reject `~/.local/bin/*` without an explicit override).
  3. Log the resolved absolute path on first spawn (defence-in-depth).
  4. Test on macOS (Homebrew), Linux (apt/dnf), and Windows (winget/scoop).
- **Status:** open, low priority.

---

## S2 — `path_to_uri` no workspace containment check

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § S2
- **Location:** `src/lsp/client.rs:49-60` + every `did_open`/`did_change`
  caller
- **Summary:** `path_to_uri` falls back to `current_dir()` for relative paths
  and does not assert that the resolved absolute path is under
  `self.workspace_root`. Combined with F4 (no read-path containment at tool
  level), the LSP child can be told to `didOpen` arbitrary files.
- **Why deferred:** Blocked on F4 — any decision here must align with the
  tool-level read-path policy. Changing this in isolation would produce
  inconsistent error behaviour across tools.
- **Unblock checklist:**
  1. Resolve F4 first (decide containment vs deny-list).
  2. Mirror that policy in `path_to_uri` + `did_open`/`did_change`.
  3. Add regression tests covering cross-workspace attempts.
- **Status:** blocked on F4.

---

## C2 — `$/cancelRequest` on drop + pending-entry leak

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § C2
- **Location:** `src/lsp/client.rs:458-503` (`request` retry loop);
  `src/lsp/client.rs:506-568` (`request_with_timeout`)
- **Summary:** When a caller's future is dropped mid-await, three things
  leak: (1) the `pending.insert(id, tx)` entry lingers until the LSP
  eventually responds, (2) no `$/cancelRequest` is sent so the server
  continues computing, (3) the LSP child isn't informed. C1 covered the
  timeout path; drop-cancel is still open.
- **Why deferred:** Correct fix needs a `scopeguard`/`RemoveOnDrop` helper
  that sends `$/cancelRequest` asynchronously from a `Drop` impl — which
  is awkward because `Drop` is sync. Probably needs a `tokio::spawn`
  inside Drop, or a separate cancellation sender task. Non-trivial.
- **Unblock checklist:**
  1. Introduce `PendingGuard` struct owning the pending-entry id.
  2. Drop impl spawns a detached task that sends `$/cancelRequest` and
     removes the pending entry.
  3. Regression test: spawn request, drop future, assert pending map is
     empty and server received cancel.
- **Status:** open, medium priority.

---

## I3 — SIGTERM only, no SIGKILL escalation

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § I3
- **Location:** `src/lsp/client.rs:1144-1168` (Drop);
  `src/platform/unix.rs:63-70`
- **Summary:** `terminate_process` sends SIGTERM and moves on. A misbehaving
  LSP child that ignores SIGTERM stays alive as a zombie. Matches the
  existing `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` report.
- **Why deferred:** Proper fix pairs with C2 — hold `Child` on `LspClient`
  directly so `kill_on_drop` handles it, which requires a reader-task
  ownership refactor. Separately, a detached cleanup task that SIGKILLs
  after 5s of SIGTERM is a band-aid but doesn't fix the root cause.
- **Unblock checklist:**
  1. Refactor reader task to share the `Child` handle via `Arc<Mutex<Option<Child>>>`.
  2. Drop impl of `LspClient` takes the handle, sends SIGTERM, spawns a
     detached task that waits 5s and escalates to SIGKILL.
  3. Regression test: spawn client with a stubborn child (trap SIGTERM),
     drop client, assert process is gone within 6s.
- **Status:** open, medium priority.

---

## I1 — Three-query sandwich missing for mux coherence test

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § I1
- **Location:** `src/lsp/mux/coherence_rust.rs::two_agents_coherent_after_edit`
- **Summary:** Test is two-query (only post-invalidation fresh state). Per
  CLAUDE.md the pattern should be: (1) query baseline, (2) mutate, (3)
  query stale, (4) trigger invalidation, (5) query fresh.
- **Why deferred:** Test-only; no functional impact. ~10 LOC refactor.
- **Status:** open, low priority.

---

## I4 — `idle_eviction_loop` interval vs per-language TTL

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § I4
- **Location:** `src/lsp/manager.rs:921-930`
- **Summary:** Loop ticks at `global_ttl/4 = 7.5min`, but kotlin TTL is 2h.
  The eligibility check fires correctly but the eviction event lags by up
  to 7.5min. Doc gap, not a bug.
- **Why deferred:** Doc-only clarification OR minor `min(ttls)/4` refactor.
  Low impact.
- **Status:** open, trivial.

---

## I6 — `MockLspClient` lacks fault injection

- **Source:** `docs/reviews/2026-04-24/phase-3-lsp.md` § I6
- **Location:** `src/lsp/mock.rs`
- **Summary:** No timeouts, no crashes mid-request, no malformed responses,
  no `-32800`, no cancellation simulation. Required to write proper
  regression tests for Phase 1 I4 and Phase 3 C1/C2/C3.
- **Why deferred:** API surface design needs care — should cover: slow
  response delay, failure after N requests, recoverable error injection,
  mid-request process death. Pairs naturally with writing the C2/I3
  regression tests.
- **Unblock checklist:**
  1. Design: `with_slow_response(Duration)`, `with_failure_after(n)`,
     `with_recoverable_error(method)`, `with_request_cancelled(n_times)`.
  2. Port existing tests to the richer mock.
  3. Add the C2/I3 drop-cancel tests.
- **Status:** open, medium priority (blocks proper C2/I3 testing).

---

## Phase 3 Minors — M1, M2, M3, M5, M6, M7, M9

- **M1** `client.rs:215` `as_i64()` brittle if mux ever returns string ids
- **M2** `client.rs:1062` `did_close` canonicalize fallback leaves stale
  `open_files` entry for deleted files
- **M3** `manager.rs:447` `current_exe()` binary-replaced-mid-session risk
- **M5** socket bind TOCTOU (covered by S4 fix, but worth an integration
  test)
- **M6** `manager.rs:425` `try_lock` + drop window before mux child
  re-acquires
- **M7** `client.rs:299` reader task `child.wait()` after read loop — wedged
  child never reaps exit status (cosmetic)
- **M9** `mux/protocol.rs:42-46` `untag_response_id` for ids > `i64::MAX`
  (theoretical)


# Phase 4 — AST + symbols

## S2 — Info disclosure via rollback messages listing sibling name_paths

- **Location:** `src/tools/symbol/replace_symbol.rs:114-119, 144-153`.
- **Evidence:** `dropped.join(", ")` returns sibling symbol names in error
  text. No trust boundary in single-tenant local CLI.
- **Unblock:** only relevant if codescout ever runs server-side multi-tenant.
  Gate disclosure on a `security_profile` flag or redact names when that
  profile is enabled.

## M4 — `extract_python_symbols` doesn't preserve which decorators applied

- Documentation-only; future grammar work. Unblock: decide whether decorators
  belong on `SymbolInfo` or in a side-channel.

## M5 — `extract_ts_symbols` misses `export default function …`

- **Location:** `src/ast/parser.rs` (TypeScript extractor).
- **Evidence:** Recursion handles `export_statement` but not
  `export_default_declaration`.
- **Unblock:** add a branch for `export_default_declaration` and reparse a
  handful of TS fixtures.

## M6 — `extract_kotlin_symbols` coverage gaps

- Missing: `secondary_constructor`, companion-object nested members, top-level
  `val`/`var`. Future grammar work.
- **Unblock:** decide which of these matter for navigation in practice and
  add the tree-sitter node kinds.

## M9 — `utf16_to_byte_offset` O(n) per edit

- Non-issue at scale today (bottom-to-top edit loops iterate once per line).
- **Unblock:** if a profile ever shows this hot, cache per-line UTF-8 byte
  indices.

## Open questions (Q1–Q5) — need user decision

- **Q1** Tree-sitter DOS: is hostile-input parsing in scope? `Parser::parse`
  has no timeout; deeply nested Kotlin string templates can hang a worker.
- **Q2** `text_sweep` uses `regex::escape(old_name)` with `\b…\b`. `\b` is
  ASCII-only — intentional for non-ASCII identifiers (Kotlin/Scala)?
- **Q3** No-rollback intent on `RenameSymbol`: historically "best-effort,
  user runs cargo check" or oversight? (Now partially addressed by I1 fix.)
- **Q4** `find_ast_end_line_in` ±1 tolerance: why `abs_diff <= 1`? Any
  disagreement ≥1 is already a smell; should we log both and surface to the
  caller?
- **Q5** Reparse cost: `replace_symbol` reparses 4–5 times per call. Pool
  parsers + content-hash cache?


# Phase 5 — Embed / memory / library

## S3 — Memory `topic` sanitization gap on Windows-style `\\` separators

- **Location:** `src/memory/mod.rs:131-146`.
- **Evidence:** On Linux `Path::new("..\\..\\etc\\passwd").components()`
  returns a single `Normal(...)` (safe). Windows behavior unverified.
- **Unblock:** verify on a Windows host whether `\\` is a path separator in
  that call; fix or drop accordingly.

## Phase-5 minors — documentation / polish

- Retry kind discrimination in `RemoteEmbedder` (DNS NXDOMAIN no-retry).
- `open_db` migration probe outside savepoint — swallows transient errors.
- `extract_paths` regex unanchored — matches any `src/...` substring; add a
  comment.
- `safe_truncate` memory titles — already correct, no change needed.
- `from_url` upfront URL validation via `Url::parse` for better error.
- Local HF model integrity trust — CONTRIBUTING note.
- `auto_register_deps` sync `libraries.json` write under agent write lock —
  not critical.

## Phase-5 open questions (Q1–Q4) — need user decision

- **Q1** Windows `..\\..\\foo` component parsing — see S3 above.
- **Q2** Is `RemoteEmbedder::custom()` dead code? Deprecation at
  `lib.rs:174-184` redirects callers; removing it would tighten the HTTPS
  surface story.
- **Q3** `EmbeddingsConfig.api_key` in `.codescout/project.toml` — file is
  NOT auto-added to `.gitignore`. Intentional, or should it be?
- **Q4** `IndexingState::Running` returns a `"already_running"` JSON shape
  instead of `RecoverableError`. CLAUDE.md says use `RecoverableError` for
  expected input-driven failures — intentional inconsistency?


# Phase 6 — Git

## Q2 — `Repository::discover` walks upward with no ceiling

- **Location:** `src/git/mod.rs:open_repo`
- **Evidence:** libgit2 default behavior. Activating `/tmp/foo` when there's a
  stray `.git` somewhere above silently binds wrong repo. Related to known
  `detect_project_root_finds_cargo_toml` flake.
- **Unblock:** pass `ceiling_dirs` to `discover` (probably `path` itself, or its
  parent — decide by caller intent) and assert `repo.workdir() == Some(path)` or
  a known ancestor before returning. Changes behavior for mono-repo submodule
  layouts — verify none of the 4 call sites rely on ancestor discovery.
- **Owner:** TBD.

## I5 (expanded from Phase 1) — `Repository::open` uncached per call

- **Locations:** `src/embed/index.rs:1536, 1868, 2416`, `src/dashboard/api/project.rs:45`.
- **Evidence:** Each call does `discover` + parent stat-walking. Hot on large
  monorepos during indexing and per dashboard request.
- **Unblock:** cache a single `Arc<git2::Repository>` on `ActiveProject` and
  hand it out. Profile first — may be noise next to actual index I/O. `git2::Repository`
  is `!Send` in older versions; confirm current crate version allows `Arc`-ing
  across the async boundary, otherwise wrap in `parking_lot::Mutex` or keep
  per-thread.
- **Owner:** TBD.

## Phase-6 minors — landed

M1 (no-op rebind), M2 (rename-detection comment), M3 (dropped-variants comment),
and Q1 doc-comment on `diff_tree_to_tree` revspec validation all landed in the
phase-6 fix commit. No residuals from minors.


# Phase 7 — Dashboard

## S1-DASH — Dashboard has no auth; safety hinges on `--host 127.0.0.1`

- **Location:** `src/dashboard/routes.rs::build_router`; `--host` in `src/main.rs`.
- **Evidence:** No auth middleware on any endpoint. `--host 0.0.0.0` exposes
  read+write+delete memory APIs plus `/api/project`, `/api/config`, `/api/libraries`
  to the LAN. CORS doesn't block non-browser clients (curl).
- **Open question:** decide deployment model. Either (a) hard-bind 127.0.0.1 and
  refuse non-loopback unless `--token` is set + enforced via a bearer-auth
  middleware layer; or (b) generate a random token on startup, print URL with
  `?token=…`, enforce in middleware; or (c) drop the `--host` knob entirely and
  require an SSH tunnel for remote access. Reuse Phase 1 S1 bearer machinery.
- **Owner:** TBD — needs product decision first.

## I1-DASH — `/api/libraries` and `/api/project` leak absolute paths

- **Location:** `src/dashboard/api/libraries.rs:17`, `src/dashboard/api/project.rs:21`.
- **Evidence:** `e.path.display()`, `root.display()` — full home-directory layout
  surfaces to any caller. Only matters once S1-DASH is closed or if operator
  runs `--host 0.0.0.0`.
- **Unblock:** strip to basename/relative for non-loopback context, or omit
  entirely. Gated on S1-DASH decision.

## I2-DASH — Chart.js loaded from CDN with no SRI hash

- **Location:** `src/dashboard/static/index.html:8`.
- **Evidence:** `<script src="https://cdn.jsdelivr.net/npm/chart.js@4">` — no
  `integrity=`, no `crossorigin=`. CDN compromise → arbitrary JS in dashboard
  origin with full access to unauthenticated APIs.
- **Unblock:** pin to a specific version (e.g. `chart.js@4.4.6`), compute SRI
  hash from `sha384sum` on the CDN bundle, add `integrity="sha384-…"
  crossorigin="anonymous"`. Better still: bundle Chart.js into `static/` and
  serve via `include_str!`-backed route (eliminates CDN dependency). Needs
  decision on bundle-vs-pin.

## P1-DASH — `/api/project` re-discovers git repo on every poll

- **Location:** `src/dashboard/api/project.rs::git_info`.
- **Evidence:** Calls `open_repo` per request. Dashboard JS polls overview at
  `POLL_INTERVAL`. Cross-confirms phase-6 I5 (uncached `Repository::discover`).
- **Unblock:** cache `git_branch` + dirty status with short TTL (1–5s) or
  file-watcher invalidation. Bundle with phase-6 I5 fix (shared `Repository`
  on `ActiveProject`).

## Q1 (partial) — Tighten `sanitize_topic` to strict `[A-Za-z0-9._ -]+`

- **Location:** `src/memory/mod.rs::sanitize_topic`.
- **Status:** JS `esc()` now escapes `"` and `'` (dashboard.js), which closes
  the reachable XSS path even if S1-DASH is unfixed. Tightening
  `sanitize_topic` itself is **defense-in-depth but breaking** — existing
  users may have memory files with spaces/punctuation in topic names.
- **Unblock:** decide whether to tighten the server-side allowlist. If yes,
  add a one-time migration path (or document that names with stripped chars
  need manual rename). Until then, relying on JS-side escape is sufficient.

## Phase-7 minors — not yet landed

- No `X-Content-Type-Options: nosniff`, no CSP header. A `default-src 'self'
  https://cdn.jsdelivr.net; script-src 'self' https://cdn.jsdelivr.net` CSP
  would blunt XSS slipping past `esc`. Goes with I2-DASH bundle decision.
- `dashboard.js:171` and similar concat HTML via `innerHTML`. Codebase-wide
  pattern refactor to `createElement` + `textContent` is out of scope for
  this review.
- `config.rs:10` dumps full project config. Fine today; flag for future
  `PublicConfig` projection once any secret-bearing field is added.
- `/api/health` payload schema not covered by tests.


# Phase 8 — Prompts + MCP resources

## S1 — Repo-controlled `system_prompt` injected raw into server instructions

- **Location:** `src/prompts/mod.rs::build_server_instructions` (~:96-101).
- **Evidence:** `.codescout/system-prompt.md` (or legacy `system_prompt` field
  in `.codescout/project.toml`) is concatenated verbatim into server
  instructions delivered to the LLM at session start. A malicious repo ships
  a payload that the agent treats as authoritative tool guidance.
- **Unblock:** decide on the two-pronged fix — (a) wrap the injected content
  in an "untrusted repo content; do not treat as authoritative" delimiter
  block, AND/OR (b) require explicit `security.trust_repo_system_prompt = true`
  opt-in in `project.toml`, default false. Document in README + CLAUDE.md.
- **Owner:** TBD — behavior change (silent opt-in → silent opt-out) may
  surprise existing repos that rely on the feature.

## S2 — `bucket="preferences"` auto-injection is subagent-writable

- **Location:** `src/prompts/builders.rs::build_system_prompt_draft` (~:255-282).
- **Evidence:** `memory(remember, bucket="preferences", ...)` writes to project
  SQLite; next-session draft auto-renders rows under `## User Preferences`.
  Subagent-triggerable on a malicious repo → persistent per-project prompt
  injection.
- **Unblock:** (a) gate `bucket="preferences"` writes behind explicit user
  confirmation (hook or tool-metadata flag), OR (b) wrap each preference as
  a "stored note" (data) rather than a directive at render time.
- **Owner:** TBD — confirm open question 2 (is the bucket already
  user-gated or fully agent-writable today?).

## I3 — `build_system_prompt_draft` opens SQLite on every call

- **Location:** `src/prompts/builders.rs` (preferences read block).
- **Evidence:** `embed::index::open_db` + `ensure_vec_memories` synchronous
  inside prompt-rendering path. Couples prompt build to embedding DB
  lifecycle; risk of stall under Phase 5 SQLite contention.
- **Unblock:** hoist the preferences read into the caller; pass as a
  parameter, mirroring how `libraries` is already threaded. Refactor
  the 5-positional-arg signature into a `BuildDraftCtx` struct at the
  same time.
- **Owner:** TBD — wider refactor; touches drift test + all call sites.

## Phase-8 minors — not yet landed

- `onboarding_prompt.md` is 567 lines / 25 sections. Consider pulling
  `### Memories to Create` subsections into a tool-guide resource.
- Three identical "MCP Resources" sections across `server_instructions.md`,
  `builders.rs`, `onboarding_prompt.md`. Consolidate to one canonical
  pointer (README rule #2 on triple-layer repetition).
- Move `KOTLIN_KNOWN_ISSUES` from `prompts/mod.rs` into
  `prompts/builders.rs::language_navigation_hints` for consistency.
- `ResourceRegistry::register` panics on duplicate URI; doc-comment
  `try_register` as the safe variant for dynamic (user-registered) providers.
- `AgentSummarySource::snapshot` probes LSP readiness sequentially; make
  concurrent if `project://summary` reads ever become latency-sensitive.
- `build_system_prompt_draft` 5-positional-arg signature → `BuildDraftCtx`
  struct (pairs with I3).
- Verify `probe_project_hints` is still consumed somewhere outside this
  directory; if not, remove.

## Phase-8 open questions

1. Confirm which code path loads `.codescout/system-prompt.md` into
   `ProjectStatus.system_prompt`. Any sanitization on the way in bounds S1's
   exploitability.
2. Can `bucket="preferences"` be written autonomously by a subagent today,
   or is it gated on direct user action? Answer determines S2 severity.
3. `WORKSPACE_ONBOARDING_PROMPT` uses `<HARD-GATE>` at lines 25 and 73. Is
   this pattern worth codifying in `src/prompts/README.md`'s 7-rule style
   guide? (Emerging convention.)


# Phase 9 — Cross-cutting

## S9-3 — Windows `taskkill` / `tasklist` PATH hijack

- **Location:** `src/platform/windows.rs::terminate_process` / `process_alive`.
- **Evidence:** `Command::new("taskkill")` / `Command::new("tasklist")` resolve
  via PATH. Attacker-dropped `taskkill.exe` in CWD can run with server
  privileges.
- **Unblock:** use absolute `%SYSTEMROOT%\System32\taskkill.exe`, OR call
  `TerminateProcess` via `windows-sys` / `winapi` directly (matches
  `platform/mod.rs` doc). No Linux reviewer can test this directly — defer
  to a Windows-dev pass.

## C9-2 — `Default` security profile is "Root minus deny-list"

- **Location:** `src/util/path_security.rs::validate_read_path`.
- **Evidence:** Only difference between `Default` and `Root` for absolute
  paths is deny-list intersection. Given how many secret-bearing locations
  the audit enumerated, the `Default` bound is weaker than "containment by
  default." Phase 9 expanded the deny-list (S9-1) but didn't flip the
  model.
- **Unblock:** invert the check — require absolute reads to fall under
  `project_root`, an explicit `extra_read_root`, or a registered
  `library_path`. Deny-list stays as belt-and-suspenders. Big behavior
  change; risk of breaking legitimate cross-project navigation users rely
  on today (overlaps with Phase 2 F4 residual).

## I9-1 — Windows `terminate_process` uses `/F`, Unix uses SIGTERM

- **Location:** `src/platform/windows.rs` vs `src/platform/unix.rs`.
- **Evidence:** Asymmetric cross-platform contract — Windows children die
  without grace window → stale lock files (kotlin-LSP already hit this).
- **Unblock:** send `Ctrl+Break` via `GenerateConsoleCtrlEvent`, grace
  window, fall back to `TerminateProcess`. Or use `windows-sys` directly
  and update doc on `platform/mod.rs`.

## I9-2 (partial) — `probe_ram` signature is `u64`; 0 conflates errors

- **Status:** `#[cfg]`-gating of the Linux vs macOS branches **landed**
  (no more spurious `sysctl` spawns on Linux). Still returns `0` on
  failure.
- **Unblock:** change signature to `Option<u64>` so telemetry can
  distinguish "no probe available" from "0 GiB system." Also add a
  Windows branch via `GlobalMemoryStatusEx`. Touches callers; not landed.

## S9-2 — already addressed

The deprecated `generate_auth_token` (timestamp+PID) now exists only as a
back-compat stub with `#[deprecated]`. Live callers use
`os_random_auth_token`. No residual.

## C9-1 — already addressed

`LibraryRegistry::save` already routes through `crate::util::fs::atomic_write`
as of a prior fix. No residual.

## Phase-9 minors — not yet landed

- `shell_tokenize` Windows-vs-Unix parity: Unix handles single quotes +
  backslash escapes; Windows only double quotes, hardcoded `' '`
  separator. Use `shlex` / `winsplit-rs` or document the gap.
- `probe_nvidia` / `probe_amd` shell out without absolute paths
  (same PATH-hijack class as S9-3, read-only, lower risk). `which::which`
  once at startup.
- `SizeRotatingFile::rotate` — mid-rename failure leaves inconsistent
  rotation chain; log + continue on error.
- `open_db` migration (`usage/db.rs`) has no version table; refactor
  before v0.10.
- `window_to_modifier` silently maps unknowns to "30 days"; surface typos.
- `expand_home` returns `Some` for any non-`~` input; name suggests
  `~`-only success.
- `path_security.rs` has grown to 1500+ lines mixing path validation
  with shell-command inspection. Split into `path_security` (paths) +
  `shell_security` (commands) before another 500 lines accrue.

## Phase-9 open questions

1. `usage.db` with `debug=true` stores raw tool input/output JSON —
   secret-bearing. Should the docs call this out, or should we gate
   debug mode behind a confirmation? (Q9-1 in phase file.)
