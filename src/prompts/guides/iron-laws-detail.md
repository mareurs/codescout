# Iron Laws — gates, exceptions, and edge cases

Per-law expansion of the six Iron Laws in the `server_instructions`
surface. The static slice is intentionally compact (2200-byte cap);
this guide carries the gate error text, exceptions, and edge cases
that don't fit in the slice itself.

## Iron Law 1: source reads → `symbols`

**Rule:** never `read_file` on source code (`.rs`, `.py`, `.ts`,
`.go`, `.java`, `.kt`, `.cpp`, `.c`, `.h`, etc.). Use
`symbols(path=...)` for a file overview, `symbols(name=...,
include_body=true)` for a single body, `symbols(query="...")` to
search across the project.

**Gate fires when** `read_file` is called on a path matching a
source-language extension. Error message includes:

> source range overlaps named symbol(s): '<Symbol>'
> Use symbols(name='<Symbol>', include_body=true) to read the body
> directly. Pass force=true to read the raw line range anyway.

**Exceptions:** `force=true` on `read_file` bypasses the gate when
you genuinely need the raw text (rare — e.g. reading a build-time
generated `.rs` that has no LSP entries).

**Why this matters:** `symbols` returns structured navigation
(declaration lines, doc comments, kind metadata) using LSP +
tree-sitter. `read_file` returns text; for code, you'd then have to
parse it yourself. `symbols` also caches; `read_file` doesn't.

## Iron Law 2: structural code edits → `edit_code`

**Rule:** never `edit_file` for changes that touch a symbol
definition. Use `edit_code(symbol="...", action="replace|insert|
remove|rename", body="...")`. `edit_file` is for imports, literals,
comments, config-only.

**Gate fires when** `edit_file` is called on a source file AND the
edit content contains a symbol-definition keyword (`fn `, `struct`,
`enum`, `impl `, `trait `, `class`, `def `, `interface`,
`function`, etc.) or overlaps a known symbol range. Error message
includes:

> edit contains a symbol definition — use symbol tools
> edit_file is blocked for structural edits on source code files

**Exceptions:** `edit_file` is allowed for:
- `insert: "prepend"` / `insert: "append"` at file boundaries
- `replace_all: true` for file-wide find/replace (no symbol overlap)
- Imports, literals, comments — content the gate's keyword filter
  doesn't catch

**Rename specifically:** use `edit_code(action="rename",
new_name="...")` — LSP-aware, updates all callers and references
in one operation. Doing a rename via `edit_file` + `replace_all`
will silently miss qualified callers (`module::name`) and aliased
imports.

## Iron Law 3: `run_command` output → buffer, not pipe

**Rule:** never pipe `run_command` to a log-trimmer (`| grep`,
`| head`, `| tail`, `| wc`). Run the command bare; query the
returned `@cmd_*` buffer in a follow-up call.

**Gate fires when** the command's right-hand side contains an
unbounded pipe (`cargo`, `npm`, `pytest`, `git`, `rg`, `fd`,
`grep -r`, bare `find`). Error message includes:

> IL3 violation — piped `<cmd>` to a log-trimmer. BLOCKED.
> Rerun the command bare and query the returned @cmd_* buffer.

**Bounded LHS is allowed.** `ls`, `cat`, `stat`, `du`, `diff`,
`awk`, `sed`, non-recursive `grep`, `find -maxdepth N` — the
output is naturally bounded, so a downstream pipe is fine.

**Read-mode for source code is blocked.** `cat src/foo.rs` is
allowed on bounded files but the broader "shell on source" pattern
is intercepted with a hint to route through `symbols`.

**Why this matters:** every `@cmd_*` buffer is queryable for the
rest of the session via `grep PATTERN @cmd_xxx`, `tail -N @cmd_xxx`,
`read_file(@cmd_xxx, start_line=N, end_line=M)`. Piping to a trimmer
throws away the full output before it lands in the buffer.

## Iron Law 4: markdown reads → `read_markdown`

**Rule:** never `read_file` on `.md`. Use `read_markdown(path)` for
the heading map, `read_markdown(path, heading="## Section")` for a
single section, `read_markdown(path, headings=[...])` for multiple,
`read_markdown(path, start_line=N, end_line=M)` for a line slice.

**Gate fires when** `read_file` is called on a `.md` path. Error
includes:

> Use read_markdown for markdown files
> read_markdown provides heading-based editing for .md files.

**Why this matters:** markdown files are usually large and
heading-structured. `read_markdown` returns a heading map for
overview reads — most queries are answered with the map alone, no
body read needed.

## Iron Law 5: markdown edits → `edit_markdown`

**Rule:** never `edit_file` on `.md` for content edits. Use
`edit_markdown(action="replace|insert_before|insert_after|remove|
edit", heading="...", content="...")`.

**Gate fires when** `edit_file` is called on a `.md` path with a
content edit. Same gate as Iron Law 4.

**Exceptions:** `edit_file` is still allowed on `.md` with:
- `insert: "prepend"` / `insert: "append"` (file boundaries — no
  heading addressing needed)
- `replace_all: true` (file-wide text substitution)

**Batch mode:** `edit_markdown` supports a top-level `edits: [...]`
array applied atomically. Use for multi-section edits in one call.

**Frontmatter:** `edit_markdown` supports a top-level
`frontmatter: {set, delete}` for YAML frontmatter mutations
combined with body edits in the same call.

## Iron Law 6: subagent dispatch — parent briefs

**Rule:** subagents see only what you brief them with. Pass: which
`get_guide(topic)` to call (or the content itself), prior tool
results, file paths, symbol names, **topics already triggered this
session**. Applies at every spawn boundary. A subagent re-discovering
what you knew is a dispatch defect — yours, not theirs.

**No tool gate enforces this.** Iron Law 6 is behavioral, not
substrate-gated. The discipline is observable post-hoc: a subagent
whose first tool call is `get_guide(topic)` for a topic obviously
needed by its task indicates the parent underbriefed.

**Substrate fact this compensates for:** the `guide_hints_emitted`
ledger is process-wide — now persisted per `CLAUDE_CODE_SESSION_ID` so it
survives `/mcp` restarts (`CodeScoutServer.guide_hints_emitted`,
shared via `Arc` clone in every per-request `ToolContext`). Once
the parent triggers a topic hint, NO subagent receives that hint
independently — the ledger says "already delivered." Iron Law 6
is the only channel that delivers parent-known context to
subagents.

**Recursion:** applies at every spawn boundary. Grandparent →
parent → child each pass context downward; intermediate agents do
not relay automatically.

**What "brief" means concretely:**
- Name the relevant `get_guide(topic)` the subagent should call
  before its first task, OR paste the relevant guide content into
  the spawn prompt.
- Cite prior tool results pertinent to the task (file paths, line
  numbers, symbol names — concrete, verifiable nouns).
- State the constraints: read-only? specific output shape?
  time/cost budget?
- Avoid context dumps. "Everything I know" wastes the subagent's
  budget; "what the subagent needs to act on this task" is the bar.
- **State which get_guide topics you've already triggered this
  session** (F-6 in `docs/trackers/prompt-guide-refactor-session-log.md`).
  The `guide_hints_emitted` ledger is shared parent↔subagent — so once
  you trigger a topic, the subagent will NOT receive its V2 auto-inject
  independently. Telling the subagent "I've triggered: [librarian,
  progressive-disclosure]" lets it predict its own injection behavior
  accurately and short-circuit redundant `get_guide` calls.

## Related

- `get_guide("workspace-state")` — what shared state subagents
  inherit, including `guide_hints_emitted`
- `get_guide("progressive-disclosure")` — `@tool_*` / `@cmd_*`
  buffer queries (referenced from Iron Law 3)
- `get_guide("error-handling")` — `RecoverableError` vs
  `anyhow::bail` (the routing rule behind gate errors)
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — tool authoring patterns
  that produce gates and overflow envelopes
