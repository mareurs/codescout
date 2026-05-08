<!-- @surface server_instructions -->
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
   in a follow-up: `cargo test` → `grep FAILED @cmd_id`; `npm run build` → `grep "error TS" @cmd_id`.
   Never `cargo test 2>&1 | grep FAILED` or `npm run build 2>&1 | grep error`.
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
| Edit a symbol without blast-radius check | `call_graph(symbol, path, direction="callers", max_depth=3)` first | Transitive callers invisible to grep/references alone — silent breakage |
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
4. `call_graph(symbol, path, direction="callers", max_depth=3)` — transitive blast radius for renames/structural changes
5. `edit_code(...)` — make the change
### Search Routing

- **Know the name** → `symbols(name=...)` or `symbols(path)`
- **Know the concept / "How does X work?"** → `semantic_search(query)` — faster and more relevant than grep for conceptual questions; drill with symbol tools after
- **Know a text pattern and the directory** → `grep(pattern, path=<dir>)` — hard filter, no noise from unrelated files; beats semantic_search when scope is already known
- **Know a concept but not the directory** → `semantic_search(query)` — whole-codebase ranking; follow with grep/symbols to confirm
- **Know a text pattern in data/config files** → `grep(pattern)` (not for code structure — see Iron Law #7)
- **Know a filename** → `tree(glob=...)`
- **All callers of X** → `references(symbol, path)` (not `grep`)
- **Transitive call graphs** → `call_graph(symbol, direction, max_depth)` — `direction="callers"` for blast-radius sizing; `direction="callees"` for flow tracing. `call_graph(depth=1, direction="callers")` also filters refs to call sites only.

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

**Other key tools:**
- `artifact(action=graph)` — relationships and dependencies between artifacts
- `artifact_event(action=list)` — chronological history of an artifact
- `artifact(action=state_at, id, date)` — snapshot at a point in time
- `artifact_refresh(action=list_stale)` — update stale artifacts after codebase changes
- `artifact_event(action=create)` — log significant events on an artifact

**Full reference** (filter syntax, tracker archetypes, augmentation lifecycle): `resources/read doc://librarian-guide`
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
<!-- @end -->

<!-- @surface onboarding_prompt -->
You have just onboarded this project. Your job is to create memories and a system
prompt that give future AI sessions deep, accurate knowledge of this codebase.
For single-project repos, this means 6 memories. For multi-project workspaces,
see the WORKSPACE MODE section below (if present).

## THE IRON LAW

```
NO MEMORIES WRITTEN WITHOUT COMPLETING ALL EXPLORATION STEPS FIRST
```

```
DON'T LEAVE ANY STONE UNTURNED
```

**Violating the letter of this process is violating the spirit of onboarding.**

This is a one-time setup. Every future AI session depends on the accuracy of what you
write now. **Token efficiency is NOT a concern here. Thoroughness is the ONLY goal.**
Be exhaustive. Read widely. When in doubt, read more.

<HARD-GATE>
Do NOT call `memory(action: "write", ...)` until you have:
1. Completed ALL 7 exploration steps below
2. Verified EVERY item in the Phase 2 Gate Checklist
3. Written the Exploration Summary in your response

These gates are non-negotiable. There are no exceptions.
</HARD-GATE>

---

<!-- STABLE-HEADING: workspace_onboarding_prompt.md may reference this section by exact title. Do not rename without updating cross-references. -->
## Phase 0: Embedding Model Selection

The `onboarding` tool has already written a recommended model to `.codescout/project.toml`
based on your system hardware. This model is used by **memory storage / recall only** —
code search runs through the Qdrant retrieval stack and configures embeddings via
`.env` (see `docs/research/2026-05-06-retrieval-stack-benchmark.md`). If the user has
the stack running, you can skip Phase 0/1 unless they want semantic memories.

Use the `model_options` array from the Gathered Project Data below to build the menu.
Use the `hardware` field for the one-line system summary.

Present this to the user:

> **Choose an embedding model for semantic memories.**
>
> Based on your system ({hardware.cpu_cores} CPU cores
> {if hardware.gpu: ", {hardware.gpu.name}"}
> {if hardware.ollama_available: ", Ollama running" else: ", no Ollama detected"}):
>
> {for i, opt in model_options:}
> {i+1}. {if opt.recommended: "★ "}`{opt.id}` — {opt.dims}d, {opt.context_tokens}-token context
>    {opt.reason}{if opt.recommended: " ← **Recommended**"}{if not opt.available: " *(not currently available)*"}
> {end}
>
> Press Enter to accept [1], or type a number to choose a different option.
>
> **Tip:** For multi-project workspaces, running a dedicated embedding server is
> recommended over the bundled model. Set `url` in `.codescout/project.toml` to
> point at any OpenAI-compatible endpoint (llama.cpp, Ollama, vLLM, TEI).
> See the embeddings guide for setup examples.

Wait for the user's response, then:

- **User presses Enter or types 1:** The config is already correct — proceed to Phase 1.
- **User types 2, 3, etc.:** Call `edit_file` on `.codescout/project.toml`.
  Change the `model` line to the selected option's ID. If the option is `url`,
  ask the user for their server URL and add both `model` and `url` fields.
  Confirm the edit, then proceed to Phase 1.
- **User types a custom model string:** Use that string directly in the `edit_file` call.
  If it looks like a URL, suggest adding it as `url` instead.

Then proceed to Phase 1 (Semantic Index Check).
## Phase 1: Semantic Index Check

Check the **Semantic index** line in the Gathered Project Data below.

### If the index is READY:

Announce to the user:

> "Semantic index is ready ({files} files, {chunks} chunks). I'll use
> `semantic_search` for concept-level exploration in Phase 2."

Proceed to Phase 2.

### If the index is NOT BUILT:

Semantic search is **strongly recommended** for thorough onboarding. Present
this to the user:

> **Semantic search is not set up yet.**
>
> The embedding index powers concept-level code exploration (`semantic_search`),
> which finds code by meaning — not just by name or text pattern. Without it,
> onboarding relies on symbol tools and regex search, which work but may miss
> non-obvious connections.
>
> **Options:**
> 1. **Build now** — I'll call `index(action='build')` and wait for it to finish.
>    Requires an embedding backend (bundled ONNX is the default, Ollama/OpenAI optional — see
>    `docs/manual/src/configuration/embedding-backends.md` for setup).
>    Takes 1–5 minutes depending on codebase size.
> 2. **Build from CLI** — Run `codescout index --project .` in another
>    terminal, then restart onboarding with `onboarding(force: true)`.
> 3. **Skip** — Proceed without semantic search. Exploration will use
>    `grep` (regex) instead of `semantic_search`. You can always
>    build the index later.

Wait for the user's choice before proceeding.

- **Option 1:** Call `index(action='build')`. Poll `index(action='status')` every 15
  seconds until the response shows completion or failure. If it fails, inform
  the user of the error and fall back to option 3.
- **Option 2:** Stop and wait for the user to return.
- **Option 3:** Proceed to Phase 2. Step 6 will use `grep` instead
  of `semantic_search`.

---

## Phase 2: Explore the Code

Your goal is to build a complete mental model of this codebase — enough to write
accurate, specific project memories in Phase 3. Use whatever tools and exploration
strategy you judge best. The gate checklist below is your hard constraint.

### Goals

- **Map the structure.** Understand the directory layout, module organization,
  and entry points. Know what lives where.
- **Understand core abstractions.** Identify the 3–5 key types/traits/classes
  that form the skeleton. Read their full implementations, not just signatures.
- **Read all architecture docs.** Completely — not skimmed. If docs exist, they
  contain decisions you need for accurate memories.
- **Trace at least 2 data flows.** Follow concrete operations end-to-end through
  the code, with actual function/method names — not just "the request goes through
  the middleware layer." Use `call_graph(symbol, direction="callees", max_depth=3)`
  to trace call chains; use `call_graph(direction="callers")` to size blast radius
  before edits.
- **Search by concept.** Run at least 5 semantic or keyword searches for concepts
  the codebase likely embodies (error handling, caching, authentication, etc.).
  Discover what the code does that README/docs don't mention.
- **Examine tests.** Read 2–3 test files to understand testing patterns, helpers,
  and fixtures used in this project.
- **Verify the build.** Confirm the project builds and tests pass.


---

### Phase 2 Gate Checklist

Before writing ANY memory, verify ALL of these are true. If any is unchecked, complete it first.

- [ ] Listed top-level structure AND ran `tree` on each major subdirectory
- [ ] Ran `symbols` on the top-level source AND on at least 4 subdirectories individually
- [ ] Read the FULL body (not just signature) of at least 5 core types/functions
- [ ] Read ALL architecture docs found, completely (not skimmed)
- [ ] Traced two distinct data flows from entry point to terminal output (use `call_graph(direction="callees")` for at least one)
- [ ] Ran at least 5 concept-level queries (`semantic_search` or `grep` fallback)
- [ ] Read 2–3 test files and understood the testing pattern
- [ ] Verified build/dev commands against actual repo contents

**If ANY item is unchecked: complete it before writing a single memory.**

---

### Exploration Summary

After completing all steps, write this summary **in your response, before calling any
`memory(action: "write", ...)` tool**:

> **What this system does** — in your own words, not the README's
> **The 5 most important types/modules** — name, file path, and role each plays
> **How a typical operation flows** — concrete function/method names, not just layers
> **What surprised you** — things the code does that documentation didn't mention

If you cannot write this from what you've explored, you have not explored enough.
Return to Phase 2.


---

## Red Flags — STOP and Return to Phase 2

If you notice any of these thoughts, STOP. Return to Phase 2 immediately.

- "I've read CLAUDE.md and the README — that's enough to write the memories"
- "The architecture doc covers everything I need"
- "I can infer how it works from the signatures and names"
- "I only need to survey the main files, not every module"
- "This project is small/simple, less exploration is fine"
- "I'll write the memory now and add details if something is wrong later"
- "I already understand this type of codebase"
- You have read fewer than 5 code bodies with `include_body=true`
- You have run `symbols` on fewer than 3 modules/directories
- You have traced only one data flow
- You have run fewer than 5 concept-level queries (semantic_search or grep)

**ALL of these mean: STOP. Return to Phase 2.**## Common Rationalizations

| Excuse | Reality |
|---|---|
| "CLAUDE.md and the README give me enough context" | Docs describe intent. Code reveals reality. Discrepancies hide in the code. |
| "I can infer implementations from names and signatures" | Assumptions about implementations produce wrong memories that mislead future sessions. |
| "I already understand this type of system" | Pattern recognition replaces exploration. This codebase has specific wiring that differs from the pattern. |
| "This is a small project, I can do less" | Small codebases still have gotchas. The steps scale down naturally — don't skip them. |
| "I'll refine the memories later if something is wrong" | Wrong memories mislead every session until someone notices and fixes them. Do it right once. |
| "Token efficiency matters here" | This is a ONE-TIME setup. Tokens spent here prevent thousands of wasted tokens in every future session. Be thorough. |
| "I traced one flow — that's enough" | One flow shows one path. A second reveals where paths diverge and where exceptions live. |
| "I read the docs — I understand the architecture" | Architecture docs describe the intended design. Code reveals the actual design. Read both. |

---



---## Phase 3: Write the Memories (Single-Project Mode)

> **If you see a "WORKSPACE MODE" section below**, skip this section entirely and
> follow the workspace flow instead. This section applies only to single-project repos.

Now write the memories. Your Phase 2 exploration must inform every memory — especially
`architecture` and `conventions`, which cannot be written accurately from documentation alone.

### Rules

1. **Do NOT duplicate auto-loaded context** — CLAUDE.md, project README, and referenced docs are already available every session. Memories must *supplement* them, not repeat them. If something is already documented, write a pointer (`see CLAUDE.md § Key Patterns`) rather than copying it.
2. **References over copies — drift is real** — Code and docs change. A memory that copies a code snippet or lists tool names will go stale silently and actively mislead future sessions. Prefer: `"see docs/ARCHITECTURE.md for the layer diagram"` over pasting the diagram. Reserve inline content for things that are NOT documented elsewhere.
3. **Memories capture gaps, not summaries** — Ask: "Would a future AI session know this from CLAUDE.md, the README, or the referenced docs?" If yes, skip it or point to the source. Only write it if the answer is no.
4. **Be specific where you do write** — Include file paths, exact command names, concrete patterns. "Uses clean architecture" is useless. "`api/ → service/ → repository/` with interface+impl pattern in `src/`" is useful.
5. **Be concise** — Each memory should be 15–40 lines. Longer means too much detail or duplication.
6. **Confirm with the user** — After creating all 6 memories, summarize what you wrote and ask if anything needs correction.
7. **Private memories** — Use `memory(action: "write", topic: ..., content: ..., private: true)` for project-local notes that should not appear in system instructions (e.g. personal debugging notes, temporary state). Standard `memory(action: "write", ...)` creates shared memories visible to all agents.

### Protected Memories

Check the `protected_memories` field from the onboarding tool response above. For
each memory you are about to write, check whether it appears there:

**If `protected_memories[topic].exists == false`:** Create fresh as normal.

**If `protected_memories[topic].exists == true` AND `staleness.untracked == false`
AND `staleness.stale_files` is empty:** The memory is fresh — all anchored source
files are unchanged. **Skip writing this topic entirely.** Tell the user:
> "Kept `[topic]` unchanged (all references still valid)."

**If `protected_memories[topic].exists == true` AND (`staleness.untracked == true`
OR `staleness.stale_files` is non-empty):** Run the merge flow:

1. The existing content is in `protected_memories[topic].content`.
2. For entries referencing files listed in `staleness.stale_files` (or all
   entries if `untracked`): use `symbols`, `read_file`, `grep`
   to verify whether each entry is still accurate.
3. Identify new discoveries from your Phase 2 exploration that belong in
   this memory.
4. Present a diff-style summary to the user:
   - **Stale (recommend removing):** [entries no longer accurate, with reason]
   - **Still valid (keeping):** [verified entries]
   - **New findings:** [discoveries from exploration]
   - **Proposed merged version:** [full content]
5. **Wait for user approval** before calling `memory(action="write")`.

**If a topic is NOT in `protected_memories`:** Write it as normal (overwrite).

The protected topics list is configured in `project.toml` under `[memory] protected`.
Users can add custom topics. The programmatic memories (`onboarding`, `language-patterns`)
are always excluded from protection.


Apply the **project-scope** sections of the included memory templates below. Write all 6 project-scope memories. Use the empty stub for `domain-glossary` and `gotchas` if nothing project-specific applies — do NOT skip them.

For `system-prompt`, apply the `workspace-scope: system-prompt` section (single-project flow treats the project as its own workspace).

{{include: memory-templates.md}}

## After Everything Is Created

## Coverage Verification

After writing all 6 project-scope memories, read each back:

```
memory(action: "read", topic: "<topic>")
```

Verify each is present (or matches the canonical empty stub for eligible topics). If any read fails or returns content shorter than the empty stub, retry the missing write up to 2 times. If still missing, abort with a clear error and do NOT proceed to CLAUDE.md refresh.

After confirming all 6 memories and the system prompt with the user, deliver this:

---

**Your codescout setup is complete.**

- **System prompt** (`.codescout/system-prompt.md`) — always-on project guidance,
  injected into every session. Edit anytime to refine how AI navigates your codebase.
- **Memories** — reference material read on demand via `memory(action: "read", topic: ...)`. Update
  with `memory(action: "write", topic: ..., content: ...)`.
- **Semantic memories** — use `memory(action: "remember", content: "...")` to store knowledge
  that doesn't fit a named topic. Search later with `memory(action: "recall", query: "...")`.
  Useful for preferences, patterns discovered during work, and cross-cutting notes.
- **Extended docs & project context** are available as MCP resources:
  - `doc://codescout-tool-guide` — long-form usage notes for every tool (examples, tradeoffs)
  - `memory://<name>` — project memory files (architecture, conventions, gotchas)
  - `project://summary` — active project + index + LSP snapshot
  Fetch via `resources/read <uri>` when you need more than a tool's short description.
- **Quick start for new tasks:**
  1. `memory(action: "read", topic: "architecture")` — orient yourself
  2. `symbols("src/")` — see the module structure
  3. `semantic_search("your concept")` — find relevant code
  4. `symbols(name="Name", include_body=true)` — read the implementation
  5. `librarian_context(topic)` → `artifact(action=find, semantic="...")` → `artifact(action=create)` — track decisions, issues, or plans as artifacts (call `librarian(action=tracker_design)` first for structured multi-entry trackers)
- **Library support:**
  - Libraries are **auto-discovered** when `symbol_at` resolves outside the project root.
  - `library(action="list")` — view all registered libraries and their index/version status.
  - `index(action='build', scope="lib:<name>")` — index a specific library for semantic search.
  - Once registered, use `scope="lib:<name>"` with `symbols`, `symbols`,
    `grep`, and `semantic_search` to navigate library code.

---

> **For workspace repos:** The above applies to single-project repos. For workspace repos,
> the subagent deep dives + workspace synthesis flow replaces this section. Summarize
> all per-project and workspace-level memories in one confirmation pass.

---

### Refresh CLAUDE.md

Compute the canonical memory table from what was written this run. Each row's "What's inside" cell is the first `## H2` of the memory body.

Read existing `CLAUDE.md`. Locate `## codescout Memories` (or propose adding it). Generate a unified diff for the table block.

Ask the user **once**:

```
Proposed CLAUDE.md memory-table update:

  [unified diff]

Apply? [y/N]
```

On `y`: `edit_markdown(path: "CLAUDE.md", action: "replace", heading: "## codescout Memories", content: <new table>)`. On `N` or no answer: log `claude_md: skipped (user declined)` for the final summary. No follow-up questions.
## Gathered Project Data

The data below was collected automatically. Use it as your starting point, then explore with codescout tools to fill gaps.

---

## Optional: Private Memories

After creating the 6 shared memories above, check if any personal context is worth
capturing now. Use `memory(action: "write", topic: ..., content: ..., private: true)` for anything specific
to your setup — local machine config, personal workflow preferences, or current WIP
context. This is optional; skip if nothing personal applies yet.

## Optional: Semantic Memories

For knowledge that doesn't fit a named topic — personal preferences, recurring patterns,
project-specific learnings — use semantic memories:

- `memory(action: "remember", content: "Always run integration tests with --release flag", bucket: "preferences")` — store a preference
- `memory(action: "remember", content: "The auth module uses a custom middleware chain")` — store a note (bucket auto-classified)
- `memory(action: "recall", query: "testing preferences")` — search by meaning later

Semantic memories with `bucket: "preferences"` are automatically included in future
onboarding prompts, so they persist across sessions without manual recall.
<!-- @end -->
