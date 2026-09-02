# Iron Laws — gates, exceptions, and edge cases

Per-law expansion of the six Iron Laws in the `server_instructions`
surface. The static slice is intentionally compact (1900-character cap,
sized under the MCP channel's measured 2048-CHARACTER limit);
this guide carries the gate error text, exceptions, and edge cases
that don't fit in the slice itself.

## Iron Law 1: source reads → `symbols`

**Rule:** `symbols` is the default for source — `symbols(path=...)` for a
file overview, `symbols(name=..., include_body=true)` for one body,
`symbols(query="...")` to search across the project. But `symbols` is a
*definition projection*: it does NOT return imports / `use` / `package`,
module re-exports (`mod.rs`, barrel `index.ts`), macro-generated code,
annotations, or constructs the AST-extractor drops. For those, a **line-range `read_file` is the correct
tool, not a fallback** — and they are common, not rare.

**Gate is overlap-based, not absolute.** The gate fires when a `read_file`
range *overlaps a named symbol* and redirects you to that symbol's body. A
range that hits no symbol (e.g. the import block at the top of a file)
returns raw bytes. Error on overlap:

> source range overlaps named symbol(s): '<Symbol>'
> Use symbols(name='<Symbol>', include_body=true) to read the body
> directly. Pass force=true to read the raw line range anyway.

**`force=true`** returns raw bytes for any range, including symbol-overlapping
ones (e.g. macro-generated impls the extractor dropped, or exact
byte/whitespace layout before an `edit_file` match).

**The one anti-pattern:** a full, no-range `read_file` of a large indexed
source file — it just returns the `symbols` outline anyway. Call
`symbols(path)` directly for that.

**Why this matters:** `symbols` returns structured navigation (declaration
lines, doc comments, kind metadata) via LSP + tree-sitter, and caches;
`read_file` returns text. For *definitions*, prefer `symbols`. For *what the
AST does not model* — imports, glue, macro output, exact bytes — a line-range
`read_file` is the only tool that returns the answer.
## Iron Law 2: structural code edits → `edit_code`

**Rule:** never `edit_file` for changes that touch a symbol
definition. Use `edit_code(symbol="...", action="replace|insert|
remove|rename", body="...")`. `edit_file` is for imports, literals,
comments, config-only.

**Gate fires when** `edit_file` is called on a source file AND a
**multi-line** edit adds or removes a line containing a
symbol-definition keyword (`fn `, `struct`, `enum`, `impl `, `trait `,
`class`, `def `, `interface`, `function`, etc.), or the edit overlaps a
known symbol range. **Single-line literal edits are always allowed** —
including changing a modifier or keyword on the declaration line
(e.g. `class X` → `data class X`). Error message
includes:

> edit contains a symbol definition — use symbol tools

**Exceptions:** `edit_file` is allowed for:
- `insert: "prepend"` / `insert: "append"` at file boundaries
- `replace_all: true` for file-wide find/replace (no symbol overlap)
- Imports, literals, comments — content the gate's keyword filter
  doesn't catch
- Multi-line edits where a definition keyword sits only on an
  **unchanged context line** — e.g. inserting a blank line or comment
  before an existing `fn`, or changing a function body without
  touching its signature. The gate is diff-aware: only keywords on
  added / removed / changed lines trip it (renames and new symbols
  still block)

**Rename specifically:** use `edit_code(action="rename",
new_name="...")` — LSP-aware, updates all callers and references
in one operation. Doing a rename via `edit_file` + `replace_all`
will silently miss qualified callers (`module::name`) and aliased
imports.

## Iron Law 3: `run_command` output → buffer, not pipe

**Rule:** never pipe `run_command` to a log-trimmer (`| grep`,
`| head`, `| tail`, `| sort`). Run the command bare; query the
returned `@cmd_*` buffer in a follow-up call.

**Gate fires when** the command's right-hand side contains an
unbounded pipe (`cargo`, `npm`, `pytest`, `git`, `rg`, `fd`,
`grep -r`, bare `find`). Error message includes:

> IL3 violation — piped `<cmd>` to a log-trimmer. BLOCKED.
> Rerun the command bare and query the returned @cmd_* buffer.

**Bounded LHS is allowed.** `ls`, `cat`, `stat`, `du`, `diff`,
`awk`, `sed`, non-recursive `grep` — the output is naturally
bounded, so a downstream pipe is fine. So is single-line `git`
plumbing — `rev-parse`, `patch-id`, `merge-base`, `symbolic-ref`,
`describe`, `hash-object` — which emits O(1) lines by construction
and therefore carries no limiter flag for the `git` heuristic to
find.

**Three things on the RIGHT do not trim.** `wc` and a counting
`grep -c` emit only a summary. `cut` and `tr` are 1:1 on records
and cannot hide one. And a stage that COLLAPSES anywhere in the
chain — `wc`, `grep -c`, `sha256sum`, `git patch-id` — bounds the
whole pipeline, so a trimmer after it has nothing left to trim:
`git show X | git patch-id --stable | cut -d' ' -f1` is allowed.
`sed`, `awk` and `sort` still trim, because `sed -n 1,10p`,
`awk NR<10` and `sort -u` each drop records.

**Two gates, same words — read both before concluding.** Everything
above is the PIPE gate. `cat`, `awk`, `sed` and `grep` are bounded
LHS here and still refused on a *source file* by the read-mode gate
below. `grep` is the sharpest case: a filtering `grep` trims on the
RIGHT of a pipe, a counting `grep -c` does not, it is bounded LHS on
the left, and it is refused on a source path. A sentence about one
gate says nothing about the other.

**Windows note:** prefer codescout-native discovery
(`tree(glob=...)`, `grep(pattern=...)`) over shell `find`. On
Windows `find` is ambiguous — cmd.exe ships its own `find` (a
string filter) that shadows the Unix `find`, and `find "x"` with
no file argument reads stdin and hangs the command.

**Read-mode for source code is blocked — content readers, not every
command.** Two predicates must BOTH hold within one compound segment:
its *first token* is a content-reading command — `cat`, `head`,
`tail`, `sed`, `awk`, `less`, `more`, `grep` — **and** the segment
names a source-file extension. So `cat src/foo.rs` refuses with
`shell access to source files is blocked`, while
`git commit -m "fix tail-50 in output_buffer.rs"` passes: `git` is not
a content reader, even though the message contains both `tail` and
`.rs`. Route through the codescout `symbols` / `read_file` / `grep`
tools, or pass `acknowledge_risk: true` for genuine raw shell access.

**Metadata commands are NOT blocked.** `wc`, `ls`, `stat`, `du` and
`file` return a measurement *of* the content rather than the content
itself, and codescout ships no tool that returns a line count — so
refusing them named an alternative that does not exist. `wc` came off
the blocked list on 2026-08-16 for that reason. The line this list
draws is content vs a measurement of content, **not** read-only vs
mutating: `head` and `tail` are read-only and stay blocked, because
they return the file's bytes.

**Why this matters:** every `@cmd_*` buffer is queryable for the
rest of the session via `grep PATTERN @cmd_xxx`, `tail -N @cmd_xxx`,
`read_file(@cmd_xxx, start_line=N, end_line=M)`. Piping to a trimmer
throws away the full output before it lands in the buffer.

## Iron Law 4: markdown reads → `read_file` (heading-addressed)

**Rule:** `read_file` on a `.md` path is heading-addressed by default — no
separate tool. Call `read_file(path)` for the heading map,
`read_file(path, heading="## Section")` for a single section,
`read_file(path, headings=[...])` for multiple, `read_file(path,
start_line=N, end_line=M)` for a line slice, or `read_file(path,
force=true, start_line=N, end_line=M)` for a raw line range that
bypasses heading routing entirely.

**Refused, not silently ignored:** `heading`/`headings` on a non-markdown
path, and `json_path`/`toml_key` on a markdown path — each is an error
naming the format mismatch, not a param dropped on the floor.

**Why this matters:** markdown files are usually large and
heading-structured. The heading map answers most queries with no body
read needed, built directly into `read_file` rather than requiring a
second tool call to discover which one applies.
## Iron Law 5: markdown section edits → `edit_file`'s heading grammar

**Rule:** for whole-section or heading-scoped markdown edits, pass
`heading`/`action`/`content` to `edit_file` — do not try to fake
section addressing with `old_string`/`new_string` line hunting. Use
`edit_file(path, action="replace|insert_before|insert_after|remove|
edit", heading="...", content="...")`.

**Gate fires when** `heading`, `action`, `frontmatter`, or a
heading-addressed `edits[]` item is passed on a non-`.md`/`.markdown`
path — this grammar is markdown-only, so `edit_file` refuses it
elsewhere rather than silently ignoring it. On a `.md`/`.markdown`
path, passing any of those routes the call into the heading grammar
automatically; there is no separate tool to call.

**Plain text edits on `.md` files need no special handling:**
`old_string`/`new_string`, `insert: "prepend"`/`"append"`, batch
`edits[]` of plain items, and `replace_all: true` all go through
`edit_file`'s ordinary text-edit path on markdown files exactly as on
any other file. A batch call may not mix the two `edits[]` grammars —
heading-addressed items (`heading`+`action`) and plain items
(`old_string`+`new_string`) — in the same call.

**A managed artifact is refused regardless of which grammar or write
path is used** — anything under `docs/trackers/`, or any file whose
frontmatter carries an `id:`. *Every* `edit_file` write path refuses
those: a direct write bypasses the catalog, so no `field_patch` event
is recorded, the body-shrink guard never runs, and `updated_at` goes
stale. Use `doc(action="update", id=…, patch={body_edits: […]})`
— its entries mirror `edit_file`'s heading-addressed batch shape.

**Batch mode:** `edit_file`'s `edits: [...]` array is applied
atomically for either grammar. Use for multi-section edits in one
call.

**Frontmatter:** `edit_file` accepts a top-level `frontmatter: {set,
delete}` for YAML frontmatter mutations, combinable atomically with
body edits (`edits` or `heading`+`action`) in the same call.

## Iron Law 6: subagent dispatch — parent briefs

**Rule:** subagents see only what you brief them with. Pass: which
`get_guide(topic)` to call (or the content itself), prior tool
results, file paths, symbol names, **topics already triggered this
session**. Applies at every spawn boundary. A subagent re-discovering
what you knew is a dispatch defect — yours, not theirs.

**No tool gate enforces this.** Iron Law 6 is behavioral, not
substrate-gated. The discipline is observable post-hoc: a subagent
re-deriving what you already held — the paths you had open, a prior
tool result, a symbol name you knew — indicates the parent
underbriefed. A subagent calling `get_guide(topic)` is **not** that
symptom; it is the prescribed behaviour (see the last two bullets
below), because the auto-inject cannot reach it.

**Substrate fact this compensates for:** the `guide_hints_emitted`
ledger is process-wide and shared across the parent and its subagents,
persisted per session so most topics survive `/mcp` restarts (the
session-opening topic is a deliberate exception — server construction
re-arms it on every reconnect; see `get_guide("workspace-state")`).
Once the parent triggers a topic hint, NO subagent receives that hint
independently — the ledger says "already delivered." Iron Law 6 is the
only channel that delivers parent-known context to subagents.

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
- **State which get_guide topics you've already triggered — and tell the
  subagent to fetch them itself.** The `guide_hints_emitted` ledger is shared
  parent↔subagent, so once you trigger a topic the subagent will NOT receive
  its auto-inject independently. Your context is not its context: it holds
  none of those bodies. The brief is therefore "I've triggered [librarian,
  tracker-conventions]; the auto-inject will not fire for you — call
  `get_guide` on the ones your task needs." **Never tell it the guides are
  already loaded, and never tell it to skip a fetch as redundant** — nothing
  is redundant in a context window that never received it.
- **An explicit `get_guide(topic)` always returns the full body**, ledger
  state notwithstanding — `src/tools/guide.rs` never withholds it. So the
  fetch above costs one call and cannot come back empty. What the ledger
  changes is only the accompanying *note*, which is phrased for a re-fetching
  parent; a subagent reading it should trust the body it just received.

## Related

- `get_guide("workspace-state")` — what shared state subagents
  inherit, including `guide_hints_emitted`
- `get_guide("progressive-disclosure")` — `@tool_*` / `@cmd_*`
  buffer queries (referenced from Iron Law 3)
- `get_guide("error-handling")` — `RecoverableError` vs
  `anyhow::bail` (the routing rule behind gate errors)
