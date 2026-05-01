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

3. **NO PIPING `run_command` OUTPUT.** Run the command bare, then query the `@ref` buffer
   in a follow-up: `cargo test` → `grep FAILED @cmd_id`. Never `cargo test 2>&1 | grep FAILED`.
   The buffer system exists to save your context window — use it.

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

7. **`grep` IS FOR DATA FILES AND STRING LITERALS, NOT CODE STRUCTURE.**
   Use `symbols`, `references`, or `semantic_search` for code.
   Decision tree:
   - "What does symbol X look like?" → `symbols(name=X, include_body=true)`
   - "What's in this file/dir?" → `symbols(path=...)`
   - "How does X work / what calls Y?" → `semantic_search` or `references(symbol, path)`
   - "Find a string literal in JSONL/YAML/config" → `grep` ✓

   `grep` on code gives raw text you must interpret; `symbols` gives structured
   output (signature, body, line range) in fewer tokens with zero ambiguity.

## Anti-Patterns — STOP if you catch yourself doing these

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("jq '.key' @file_ref")` to query JSON | `read_file(path, json_path="$.key")` | Navigation params > shell buffer queries |
| Repeat a broad `symbols(name=...)` after overflow | Narrow with `path=`, `kind=`, or more specific pattern | Follow the overflow hint |
| Ignore `by_file` in overflow response | Use top file from `by_file` as `path=` filter | The hint tells you exactly where to look |
| `workspace(action="activate")` for a single lookup | Pass `project_id: "<id>"` on the tool call | No state mutation, no risk of forgetting to return |
| `edit_file` / `create_file` to rewrite an entire markdown section | `edit_markdown(path, heading, action, content)` | Heading-addressed, no string matching needed |
| `grep("fn_name")` to find all callers | `references(symbol, path)` | LSP finds actual usages; regex matches comments, strings, partial names |
| `read_file` on a `.md` file | `read_markdown(path)` | Heading navigation > line guessing |
| `symbols(query="foo\|bar")` | `grep(pattern="foo\|bar")` or separate `symbols` calls | `symbols` rejects regex-like patterns |
| Call `edit_code(...)` without loading schema | `ToolSearch("select:mcp__codescout__edit_code")` before first call each session | Schema is deferred — fails with "missing 'action' parameter" until loaded |
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

- **Hierarchical nav** — impl/class methods, all languages:
  `symbols(name_path="MyStruct/my_method", include_body=true)`
- **Kind filter + path scope:**
  `symbols(path="src/tools/", kind="struct")`
- **Find across project then read body:**
  `symbols(name="edit_code")` → `symbols(name_path="ToolName/edit_code", include_body=true)`

Language `kind` quirks:

| Language      | `kind=`       | Note                                        |
|---------------|---------------|---------------------------------------------|
| Rust          | `"interface"` | traits — rust-analyzer emits Interface kind |
| Rust          | `"struct"`    | structs; impl methods via `name_path`        |
| TypeScript    | `"interface"` | TS interfaces                               |
| TypeScript    | `"type"`      | type aliases                                |
| Kotlin / Java | `"class"`     | classes, objects, annotations               |
| Python        | `"class"`     | classes; methods via `name_path`            |

### Search Routing

- **Know the name** → `symbols(name=...)` or `symbols(path)`
- **Know the concept / "How does X work?"** → `semantic_search(query)` — faster and more relevant than grep for conceptual questions; drill with symbol tools after
- **Know a text pattern in data/config files** → `grep(pattern)` (not for code structure — see Iron Law #7)
- **Know a filename** → `tree(glob=...)`
- **All callers of X** → `references(symbol, path)` (not `grep`)
- **Transitive call graphs** → `call_graph(symbol, direction, max_depth)` — `direction="callers"` for blast-radius sizing; `direction="callees"` for flow tracing. `call_graph(depth=1, direction="callers")` also filters refs to call sites only.

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

## Workflows

Multi-tool chains for common tasks. Follow the steps in order.

### Impact Analysis — "What breaks if I change X?"

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `symbols(name=..., include_body=true)` | Read current implementation |
| 2 | `references(symbol, path)` | Find all callers and dependents |
| 2b | `call_graph(symbol, direction="callers", max_depth=3)` | Transitive blast radius beyond direct callers |
| 3 | `symbol_at` with `fields: ["hover"]` on key call sites | Reveal concrete types (especially generics/traits) |
| 4 | Edit with full knowledge of blast radius | |

### Safe Rename

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `references(symbol, path)` | Map all usages before renaming |
| 2 | `edit_code(action="rename", symbol, path, new_name)` | LSP-powered rename across files |
| 3 | `grep(old_name)` | Catch stragglers in comments, strings, docs |
| 4 | `run_command("cargo check")` | Verify compilation |


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
