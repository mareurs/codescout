codescout MCP server: high-performance semantic code intelligence.
Provides file operations, symbol navigation (LSP), AST analysis (tree-sitter),
semantic search (embeddings), and project memory.

**Subagents and spawned agents SHOULD use codescout too.** If you spawn a subagent
or delegate to another agent, instruct it to use codescout tools for all code
navigation and editing — do not fall back to native Read/Grep/Glob/Edit/Write on
source files. For structural code changes, use `edit_code` — never the host's native Edit tool.

## Iron Laws

These are non-negotiable. Violating the letter IS violating the spirit.

1. **NO `read_file` ON SOURCE CODE.** Use `symbols(path)` + `symbols(name=..., include_body=true)`.
   `read_file` on source returns a summary, not raw content. Symbol tools give you
   structured, token-efficient navigation. `read_file` is for config, markdown, and data files.

2. **NO `edit_file` FOR STRUCTURAL CODE CHANGES.** Use `edit_code`. `edit_file` is for imports, literals, comments, config.
   Multi-line edits containing definition keywords (`fn`, `class`, `struct`, etc.) on
   LSP-supported languages return a hard error — the tool tells you which symbol tool to use.

3. **NO PIPING LIVE `run_command` OUTPUT TO LOG-TRIMMERS.** Run the command bare, then query
   the `@ref` buffer in a follow-up: `cargo test` → `grep FAILED @cmd_id`; `npm run build` →
   `grep "error TS" @cmd_id`. Never `cargo test 2>&1 | grep FAILED` or `npm run build 2>&1 | grep error`.
   The buffer system exists to save your context window — use it.

   For long-running commands (builds, indexers, dev servers, watchers) use
   `run_in_background=true` instead of bumping `timeout_secs`. Returns
   `{output_id: "@bg_*", hint, stdout: <tail-50 if any>}` immediately; the
   process keeps writing to the buffer. Inspect with `tail -50 @bg_*` or
   `grep PATTERN @bg_*`.

   **Buffer-op pipes are allowed.** Once a command's output is in a buffer (`@cmd_*`, `@bg_*`,
   `@file_*`, `@tool_*`), pipes that *start from* the buffer are fine —
   `grep PATTERN @cmd_xxx | sort -u`, `cat @bg_yyy | head -50`. The capture has already happened;
   chaining buffer-ops is free.

4. **ALWAYS RESTORE THE ACTIVE PROJECT.** After `workspace(action="activate", path=...)` to
   a different project, you MUST call `workspace(action="activate", path=...)` back to the
   original before finishing your task. The MCP server is shared state — forgetting to return
   silently breaks all subsequent tool calls. Subagents share the server with their parent —
   they MUST restore too.

5. **ACTIVATE THE HOME PROJECT WITH WRITE ACCESS AT SESSION START.** At the start of every
   session, call `workspace(action="activate", path=".", read_only=false)`. This ensures write tools work on
   the current working directory — the server may have been left in an unknown state by a
   previous session or subagent.

6. **REUSE `@file_*` BUFFER REFS.** After a tool emits `file_id: "@file_*"`, subsequent
   reads of that content MUST use the buffer ref, not the original path.
   Re-reading the original path duplicates disk work and destroys the
   progressive-disclosure contract. Applies to `read_file`, `read_markdown`,
   and any tool that consumes `@file_*`.

7. **`grep` IS FOR SCOPED SCANS AND LITERALS, NOT CODEBASE-WIDE STRUCTURE.**
   Decision tree:
   - "What does symbol X look like?" → `symbols(name=X, include_body=true)`
   - "I have a path + line number from tool output" → `symbol_at(path, line)` — type sig + hover docs, no re-search needed
   - "What's in this file/dir?" → `symbols(path=...)`
   - "How does X work / what calls Y?" → `semantic_search` or `references(symbol, path)`
   - "Which files in **this directory** reference these property names?" → `grep(pattern, path=<dir>)` ✓ — hard path filter beats embedding similarity
   - "Which files **anywhere** deal with this concept?" → `semantic_search(query)` — not grep
   - "How is symbol X accessed at call sites?" → `symbols(name=X)` to find defining file → `references(symbol, path)` — not a second grep
   - "Find an enum value or string constant?" → `grep` ✓ — constant values aren't navigable symbols
   - "Find a string literal in JSONL/YAML/config" → `grep` ✓

   `grep` on code gives raw text you must interpret; `symbols` gives structured
   output (signature, body, line range) in fewer tokens with zero ambiguity.

8. **CALL GRAPH BEFORE STRUCTURAL EDITS.** Before
   `edit_code(action="rename"|"replace")` of a function, method, or
   public type: `call_graph(symbol, path, direction="callers",
   max_depth=3)` first, then `references` for edit targets. Transitive
   callers are invisible to `references` alone.

## Anti-Patterns — STOP if you catch yourself doing these

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("jq '.key' @file_ref")` to query JSON | `read_file(path, json_path="$.key")` | Navigation params > shell buffer queries |
| Edit a symbol without blast-radius check | `call_graph(...)` — see Impact Analysis | Transitive callers invisible to grep/references alone — silent breakage |
| Repeat a broad `symbols(name=...)` after overflow | Narrow with `path=`, `kind=`, or more specific pattern | Follow the overflow hint |
| Ignore `by_file` in overflow response | Use top file from `by_file` as `path=` filter | The hint tells you exactly where to look |
| `workspace(action="activate")` for a single lookup | Pass `project_id: "<id>"` on the tool call | No state mutation, no risk of forgetting to return |
| `edit_file` / `create_file` to rewrite an entire markdown section | `edit_markdown(path, heading, action, content)` | Heading-addressed, no string matching needed |
| `grep("fn_name")` to find all callers | `references(symbol, path)` | LSP finds actual usages; regex matches comments, strings, partial names |
| `read_file` on a `.md` file | `read_markdown(path)` | Heading navigation > line guessing |
| `read_markdown("docs/trackers/foo.md")` directly | `artifact(action="find", semantic="foo")` then `artifact(action="get", id=...)` | Raw file lacks catalog metadata: link graph, augmentation state, event history |
| `git mv docs/trackers/foo.md docs/archive/foo.md` | `artifact(action="move", id, new_rel_path="docs/archive/foo.md")` | Moving the backing file orphans the catalog record; use artifact(move) instead |
| `artifact(action="find", filter={"in":{"field":"title","value":[...]}})` | `filter={"title":{"in":[...]}}` | Filter leaf is `{field:{op:value}}` — not `{op:{field,value}}` |
| `cat <file> \| head -N` to inspect source | `symbols(path=...)` | Iron Law #1 + #3 double violation — shell gives raw text; symbols give structure |
| `grep("x", path=<dir>)` again after finding files, to trace access patterns | `symbols(name=X)` → `references(symbol, path)` | grep matched files → now you know the symbol name; switch to LSP for precise call sites |
| `semantic_search("concept")` when you already know the directory | `grep(pattern, path=<dir>)` | Embeddings rank by whole-codebase similarity; grep with `path=` is a hard filter — no noise from tests/docs/config |
| `edit_file` to add a callback, handler, or field inside a function body | `edit_code` | Adding anything inside a function body is a structural edit regardless of how small it looks |
| `symbols(query="foo\|bar")` | `grep(pattern="foo\|bar")` or separate `symbols` calls | `symbols` rejects regex-like patterns |
| Call `edit_code(...)` without loading schema | `ToolSearch("select:mcp__codescout__edit_code")` before first call each session | Schema is deferred — fails with "missing 'action' parameter" until loaded |
| `semantic_search("X")` when you already have path+line for X | `symbol_at(path, line)` | Re-searching wastes tokens; you already have the location |
## Tool Routing & Gotchas

Tool descriptions and parameters are in the MCP tool schemas — this section
covers only cross-tool routing and non-obvious behaviors.

### Source Code: Symbol Tools, Not File Tools

- **Reading source:** `symbols(path)` → `symbols(name=..., include_body=true)`.
  `read_file` on source returns a summary, not raw content.
- **Editing code:** `edit_code` for structural changes (rename, remove, replace, insert).
  `edit_file` is for imports, literals, comments, config only.
- **Markdown files:** `read_markdown` / `edit_markdown`, not `read_file` / `edit_file`.
  `edit_file` on `.md` files is gated to `edit_markdown` (except `insert="prepend"|"append"`).


### Symbol Navigation Patterns

{{symbol_navigation_block}}
### LSP Workflow — Standard Sequence

For any symbol change, in order:
1. `symbols(name=X)` — locate the symbol, get its defining file + line
2. `symbol_at(path, line)` — inspect type signature + docs (when you need to understand what it IS)
3. `references(symbol, path)` — enumerate all call sites before touching anything
4. For impact analysis, see Impact Analysis.
5. `edit_code(...)` — make the change
### Search Routing

- **Know the name** → `symbols(name=...)` or `symbols(path)`
- **Know the concept / "How does X work?"** → `semantic_search(query)` — faster and more relevant than grep for conceptual questions; drill with symbol tools after
- **Know a text pattern and the directory** → `grep(pattern, path=<dir>)` — hard filter, no noise from unrelated files; beats semantic_search when scope is already known
- **Know a concept but not the directory** → `semantic_search(query)` — whole-codebase ranking; follow with grep/symbols to confirm
- **Know a text pattern in data/config files** → `grep(pattern)` (not for code structure — see Iron Law #7)
- **Know a filename** → `tree(glob=...)`
- **All callers of X** → `references(symbol, path)` (not `grep`)
- **Transitive call graphs** → `call_graph(symbol, path, direction, max_depth)` — see Impact Analysis for the worked example.

**Retrieval stack required.** `semantic_search` runs through the Qdrant + TEI hybrid stack. If a call returns `retrieval stack offline`, the user must run `./scripts/retrieval-stack.sh up` once per machine. There is no in-process fallback — the legacy sqlite-vec code-search path was removed in Phase 7.

**Modes.** `semantic_search(mode="code")` (default) excludes markdown chunks so implementations rank ahead of plans/specs/trackers. Pass `mode="full"` only when you actually want docs in the results (e.g. searching for a tracker entry by concept).
### Gotchas

- **MUST FOLLOW:** `edit_code(action="rename")` may corrupt string literals containing the
  old name. Always verify compilation (`cargo check` / `tsc --noEmit` / etc.)
  after use, especially if the symbol name is a common word.
- `run_command` output > 50 lines is buffered as `@cmd_*` ref. Query with
  `grep pattern @cmd_id` or `read_file("@cmd_id", start_line=N)`.
- `read_markdown` returns adaptive content: heading map + stats for large files,
  full content + hint for medium files, full content for small files. Pass
  `heading=` or `headings=` for specific sections, or `start_line`/`end_line`
  for line slices (also works on `@file_*` buffer refs).
- `edit_file` `edits=[...]` batch mode is atomic (one write). Prefer over
  sequential single edits on the same file.
- `symbols` directory responses vary by tree size: Small tree (≤30 files) or
  `force_mode: "symbols"` returns `{ "directory": ..., "files": [...] }` (existing shape).
  Medium tree (31–80 files) returns `{ "mode": "class_overview", "subdirectories": [...], ... }`.
  Large tree (>80 files) returns `{ "mode": "directory_map", "subdirectories": [...], ... }`.
  Check `result["mode"]` to detect shape. Use `force_mode: "symbols"` to always get
  the `files` array regardless of tree size.

### Library Routing

Pass `scope="lib:<name>"` on `symbols`, `references`,
`semantic_search`, or `index(action='build')` to target a registered library.
Libraries are auto-discovered when `symbol_at` resolves outside
the project root. All read-only tools work on libraries; write tools are project-only.

**Lifecycle:** `library(action="register", path)` adds a library to the registry
(one-time, per project). Then `index(action='build', scope="lib:<name>")` builds the
symbol+embedding index. `library(action="list")` enumerates registered libraries.
You rarely need `library(action="register")` manually — symbol_at registers
external dependencies on the fly.

**Cancelling a reindex:** `index(action='cancel')` aborts an in-flight
`index(action='build')`. A force-reindex on a large project can run for tens of
minutes (sparse embedder is often the bottleneck); use cancel rather than killing
the server. Returns `{"status": "cancelled"}` or `{"status": "no_active_sync"}`.
### Artifact & Tracker Routing

**When to use artifact tools** — tracking decisions, issues, plans, experiments, or anything with evolving state. Prefer artifacts over plain markdown for anything you'd want to query by meaning, link to other artifacts, or time-travel through.

**Entry point:** `librarian(action="context", topic="...")` — packs a semantic bundle of relevant artifacts and context. Call this first before any artifact task to orient and avoid duplicates.

**Artifact model:** Artifacts can carry **augmentation** — a persistent prompt that auto-refreshes their body as the codebase evolves. **Trackers** (`kind=tracker`) are the canonical augmented artifact: living documents for issue lists, ADR logs, experiment records, and similar multi-entry state.

**Create workflow:**

1. `artifact(action="find", semantic="...")` — semantic search first; never create without checking
   - Filter by field: `artifact(action="find", filter={"kind": {"eq": "tracker"}})` — leaf format: `{"field": {"op": value}}`
   - Combine: `{"and": [{"kind": {"eq": "tracker"}}, {"status": {"eq": "active"}}]}`
2. If **tracker** (multi-entry: issue list, ADR log, experiment record): call `librarian(action=tracker_design)` → pick an archetype → then `artifact(action=create)`
3. If **regular artifact**: `artifact(action=create)` directly (fails if path exists — `artifact(action=find)` guards this)
4. `artifact(action=link, source, target)` to connect related artifacts

**Archive workflow** — when a tracker reaches terminal state:

A tracker (a markdown file in `docs/trackers/` with F-N / T-N entries or `[x]` phase checkboxes) is **terminal** when every entry is closed (`fixed-verified`, `mitigated`, `wontfix`, or `promoted to <other>`) or every checkbox is `[x]`. Then:

1. `artifact(action="find", filter={"rel_path": {"contains": "<name>"}})` — check librarian-indexed status. If indexed → `artifact(action="move", new_rel_path="docs/trackers/archive/<name>.md")` preserves the artifact id across the rename. If unindexed → `git mv` is enough.
2. Grep inbound references; rewrite `docs/trackers/<name>.md` → `docs/trackers/archive/<name>.md`. One atomic commit covers the rename plus all rewires.
3. Self-references inside the archived file stay — narrative prose, not active links.
4. **Before commit:** `git diff --stat` must show 100% rename detection AND symmetric N insertions / N deletions across sibling files. Asymmetry means scope creep — **stop and ask the user; do not commit.**

Eval baseline: `docs/evals/archive-tracker-rule.md` — v0.1 = 4/5 PASS.

**Other key tools:**
- `artifact(action=graph)` — relationships and dependencies between artifacts
- `artifact_event(action=list)` — chronological history of an artifact
- `artifact(action=state_at, id, date)` — snapshot at a point in time
- `artifact_refresh(action=list_stale)` — update stale artifacts after codebase changes
- `artifact_event(action=create)` — log significant events on an artifact

**Full reference** (filter syntax, tracker archetypes, augmentation lifecycle): `resources/read doc://librarian-guide`

### Artifact CLI

For shell scripts and hooks that need to read or mutate the catalog without speaking MCP, the codescout binary exposes the artifact surface as subcommands: `codescout artifact find/get/graph/state-at/create/update/move/link`, `codescout artifact-event create/list`, `codescout artifact-refresh gather/list-stale`, `codescout artifact-augment <id>`. Each subcommand defaults to pretty output and adds `--json` for machine consumers. Names mirror MCP tool names 1:1, so any MCP example translates trivially.

### Goal-trackers

A **goal-tracker** is a tracker artifact (`kind=tracker`, `tags: ["goal"]`) that names a completion criterion and aggregates the state of typed child trackers. Each project should have at most one goal with `status=active` at a time — if multiple goals are simultaneously active, the Stop hook fails open (deferring) and the librarian context surfaces them in created_at order.

**Find the active goal for the current project:**

```
artifact(action="find", kind="tracker",
         filter={"tags":{"in":["goal"]}, "status":{"eq":"active"}})
```

**Get richer context including active goals plus other project signal:**

```
librarian(action="context")   # no anchor — auto-includes active goals
```

When starting work toward a stated objective, create a goal-tracker via `librarian(action="tracker_design", intent="goal: ...")` then `artifact(action="create", kind="tracker", tags=["goal"], augment=...)`. Children are linked via `artifact(action="link", rel="child")` and use existing archetypes (failure_table, task_list, metric_baseline, audit_issues, reflective, deployment_state, or a nested goal for multi-level objectives). Not for open-ended research (use reflective), bare task lists, or anything without a definable 'done' line. The goal archetype requires 2+ child sub-trackers — for a single criterion checked directly, use the underlying archetype.

## Output System

**File paths in tool output are relative to the project root** (e.g. `src/tools/mod.rs`,
not `/home/user/project/src/tools/mod.rs`). Pass them as-is to other tools.

### Modes

Default: **exploring** — compact, capped at 200 items.
Pass `detail_level: "full"` for focused mode with `offset`/`limit` pagination.
Only switch to focused AFTER identifying targets.

Overflow produces: `{ "overflow": { "shown": N, "total": M, "hint": "...", "by_file": [...] } }`
— **follow the hint.** Narrow with `path=`, `kind=`, or a more specific `pattern`.
`by_file` shows per-file match counts; use the top file as your `path=` filter.

### Output Buffers

Large content is stored in an `OutputBuffer`. When a result is buffered you receive an
`output_id` field (or `file_id` for large file reads) containing a `@ref` handle.
The full content costs nothing to hold — query it on demand.

#### Buffer ref types and access

| Signal | Ref | Content | Access |
|---|---|---|---|
| `"output_id": "@cmd_abc"` from `run_command` | `@cmd_*` | plain text | `grep pattern @cmd_abc` or `read_file("@cmd_abc", start_line=N)` |
| `"file_id": "@file_abc"` from `read_file` or `read_markdown` | `@file_*` | plain text | For code/text: `grep pattern @file_abc` or `read_file("@file_abc", start_line=N)`. For markdown: `read_markdown("@file_abc", heading="## Section")` or `start_line`/`end_line`. |
| `"output_id": "@tool_abc"` from other tools | `@tool_*` | JSON | `read_file("@tool_abc", json_path="$.field")` or `start_line`/`end_line` |
| `"output_id": "@bg_abc"` from `run_in_background` | `@bg_*` | plain text | `tail -50 @bg_abc` or `grep pattern @bg_abc` |

**Response fields for `read_file`:**
- `complete: bool` — true if all requested content was returned inline; false if more is available via `next`
- `next: string` — the exact `read_file(...)` call to get the next chunk (only present when `complete: false`)
- `shown_lines: [start, end]` — the original file line numbers of the content shown (present in auto-chunked responses)

**Key distinction:** `@file_*`, `@cmd_*`, `@bg_*` are plain text — grep/sed work directly.
`@tool_*` is JSON — use `json_path` (e.g. `$.symbols[0].body`) or `start_line`/`end_line`.
**MUST FOLLOW:** Do not grep `@tool_*` for code. Bodies are JSON-escaped
strings, so grep returns escaped matches, not raw text. Use
`read_file("@tool_id", json_path="$.symbols[0].body")` to extract a specific
field first.

**Buffer queries** return ≤ 100 lines inline. Truncation hints show the exact `sed` command
to continue.

## Project Management

### Worktrees

After `EnterWorktree`, call `workspace(action="activate", path=...)` with the worktree path — write tools are
NOT automatically coupled to the shell's working directory. If you forget, writes silently
modify the main repo. To clean up: `git worktree prune` from the main repo root.

### Security Profiles

The project's security profile is set in `.codescout/project.toml`:

- `profile = "default"` (default) — standard sandbox: read deny-list active, writes
  restricted to project root + temp dir, dangerous commands require `acknowledge_risk`.
- `profile = "root"` — unrestricted: no read deny-list, writes allowed anywhere,
  dangerous commands execute without speed bump. For system-administration projects
  that need full filesystem access.


- `approve_write(path)` — grant write access to a directory outside the project root for
  this session. Required before `edit_file`/`create_file`/`edit_code`/`edit_markdown` on
  out-of-project paths. Approval is cleared on server restart or re-activation. The
  deny-list (`~/.ssh`, etc.) is always enforced regardless of approval.
## Workflows

Multi-tool chains for common tasks. Follow the steps in order.

### Impact Analysis — "What breaks if I change X?"

`references` = direct call sites. `call_graph` = transitive reach.
Both required for any rename / signature change / contract change.

1. `symbols(name="Service/handle", include_body=true)` — read it.
2. `call_graph(symbol="Service/handle", path="src/service.rs",
   direction="callers", max_depth=3)` — blast radius.
   Tree depth ≈ change risk: shallow = local; deep+branching = contract.
3. `references(symbol, path)` — file:line edit targets.
4. `symbol_at(path, line, fields=["hover"])` on non-obvious callers
   from step 2 — reveal concrete types behind generics/traits.
5. `edit_code(...)`.

`direction`: `callers` (refactors) | `callees` (flow) | `both` (hubs, rare).
`max_depth`: `1` ≈ references; `3` default; `5` only for deep reach.
Skip call_graph only for body-only edits with identical signature.
### Safe Rename

Run Impact Analysis first.

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `edit_code(action="rename", symbol, path, new_name)` | LSP-powered rename across files |
| 2 | `grep(old_name)` | Catch stragglers in comments, strings, docs |
| 3 | `run_command("cargo check")` | Verify compilation |
### Tracking a Decision or Issue

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `librarian(action="context", topic="...")` | Get relevant artifact bundle and staleness warnings |
| 2 | `artifact(action=find, semantic="...")` | Search by meaning — don't create duplicates |
| 3a | `librarian(action=tracker_design)` → `artifact(action=create)` | For trackers: pick archetype first, then create |
| 3b | `artifact(action=create, ...)` | For plain artifacts: create directly |
| 4 | `artifact(action=link, source, target)` | Connect to related artifacts |
| 5 | `artifact_event(action=create, id, ...)` | Log significant events as they happen |

More workflows (markdown editing, dependency tracing) available via `resources/read doc://codescout-tool-guide`.
## MCP Resources

Extended docs and project context are available as MCP resources — fetch via `resources/read <uri>`:

| URI | Contents |
|-----|----------|
| `doc://codescout-tool-guide` | Long-form usage notes for every tool (examples, tradeoffs, edge cases) |
| `memory://<name>` | Project memory files (architecture, conventions, gotchas, language-patterns) |
| `project://summary` | Active project + index status + LSP snapshot |

Use these when a tool's short description leaves questions, or when you need architecture context before starting a task.
## Rules

1. **Exploring mode first.** Only `detail_level: "full"` after you know what you need.
2. **Follow overflow hints.** Narrow with `path=`, `kind=`, or a more specific pattern — don't repeat broad queries.
3. **`run_command` is already in the project root.** Never prefix with `cd /abs/path &&`. Use `cwd` for subdirectories.
4. **Check `features_md` from `onboarding` before suggesting features.** Don't propose work that's already done.
5. **Semantic search for "how does X work?"** Then drill into results with symbol tools.
6. **Read `language-patterns` memory before writing or editing code.** `memory(action="read", topic="language-patterns", sections=["<your language>"])` returns only the patterns for your language. Consult it before code changes or code review.
7. **Symbol edits over `edit_file` for code.** `edit_code` for structural changes (rename, remove, replace, insert). `edit_file` for imports, literals, comments.
