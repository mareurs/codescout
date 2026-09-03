# Progressive Disclosure

How codescout handles results too big to inline, and how the model
should respond to them.

## Output budgets

Tool output is capped to keep the model's context window healthy.
Results exceeding the inline budget are stored in a server-side
`@tool_*` buffer and a compact summary is returned in their place.

| Constant | Value | Notes |
|---|---|---|
| `MAX_INLINE_TOKENS` | 2,500 tokens (~10 KB) | base budget (~4 bytes/token) |
| `TOOL_OUTPUT_BUFFER_THRESHOLD` | 10,000 bytes | derived: `MAX_INLINE_TOKENS * 4` |
| `INLINE_BYTE_BUDGET` | 9,000 bytes | derived: 90% of threshold |
| `COMPACT_SUMMARY_MAX_BYTES` | 2,000 bytes | summary soft cap |
| `COMPACT_SUMMARY_HARD_MAX_BYTES` | 3,000 bytes | summary hard cap |
| `LINE_SOFT_CAP` | 150 lines | markdown read nudge |
| `HEADINGS_HARD_CAP` | 40 headings | markdown map-shape escalation |

Token estimate is `bytes / 4`. Above `MAX_INLINE_TOKENS`, the full
result is buffered and the response shrinks to `{output_id, summary,
hint, …}`.

## The @ref buffer

When a tool returns an overflow envelope (typical fields: `output_id`,
`summary`, `hint`, `complete`, `next`):

- `output_id` is a handle like `@cmd_abc` or `@tool_xyz` pointing to
  the server-side buffer holding the full result.
- `summary` is the compact form — capped at ~2 KB.
- `hint` shows the most useful follow-up call for that tool.

Query the buffer instead of re-running the tool:

```
grep PATTERN @cmd_abc                             # search the buffer
read_file("@tool_xyz", json_path="$.foo")         # one field
read_file("@tool_xyz", json_path="$.rows[*].id")  # that field from EVERY row
read_file("@tool_xyz", start_line=N, end_line=M)  # slice lines
```

**Buffered results are usually arrays of records, so `[*]` is the common form** — it projects the
rest of the path over every element and returns an array. Nesting is preserved, not flattened:
`$.groups[*].rows[*].v` gives `[[1,2],[3]]`. Every element must satisfy the path after `[*]`; a
missing key errors naming the element rather than silently dropping the row.

The addressing grammar is a deliberate subset: `.key`, `["key"]` (for keys containing `.`), `[N]`,
`[-N]`, `[-N:]` (last N), and `[*]`. Forward slices (`[1:3]`) and filters (`[?(...)]`) are **not**
supported — project with `[*]` and filter the returned array yourself.

The envelope's `hint` names a concrete path derived from the payload's actual shape; prefer it over
guessing a key.

`@cmd_*` buffers come from `run_command`. `@tool_*` buffers come from
other tools. Both are addressable by any tool that accepts a path.
`@file_*` and `@ack_*` are sibling handle kinds — same mechanics. `@ack_*`
covers both dangerous commands and out-of-scope writes: re-invoke the tool with
the handle to acknowledge and proceed.



## Path-relative annotation

Paths in tool responses are project-relative. The response to each
`activate_project` call carries a trailing `[codescout] paths are relative
to <root>` note naming the root they resolve against; every later response
in that activation window omits it, because the same fact lives in the
`Active project` line of `server_instructions`. The gate also fires on the
first eligible response after **server start**, not only after an explicit
`activate_project` call — launching codescout with `--project <path>`
(`src/main.rs:23-25`) activates a project before any tool call, so that
first response carries the banner too.

Relativization is **field-aware and allowlist-driven**: only keys in a
fixed list of path-valued JSON fields (`PATH_KEYS`) are relativized — a
key outside that allowlist keeps its absolute path by design (verbose,
never corrupt; do not assume every path-looking field in a response is
relative). It never touches file content, shell output, prose, or error
text — all byte-faithful. Root-valued fields (`ROOT_KEYS`: `cwd`,
`git_root`, `new_root`, `old_root`, `project_root`, `repo_root`, `root`)
stay absolute — they are the anchor the rest resolve against. The catalog
itself always stores absolute paths; this is a display-time transform,
not a change to what's on disk.

To check a path against catalog state, read the value straight from the
response — an allowlisted path field is relative, a root field absolute,
and content is verbatim. `run_command` output is raw shell bytes and is
never rewritten.

## Anti-patterns

- **Re-running a tool because the result was "too long".** Query the
  buffer instead. The full result is sitting on the server; pulling
  one slice costs no extra tool latency and no extra LLM tokens.
- **Asking the user to paste content from a buffered result.** The
  buffer is server-side — you can read it directly with the `@ref`.
- **Treating `output_id` as a filename.** It's an opaque handle;
  `read_file("@tool_xyz")` works, filesystem paths derived from it
  do not.
- **Passing the handle to a non-codescout reader** (the harness's own
  file-read tool, `cat`, `bash Read`). `@cmd_*`/`@tool_*`/`@file_*`/`@ack_*`
  buffers live only in codescout's server process — they resolve exclusively
  through codescout's own `read_file`, `run_command`, or `grep`. Any other
  tool reports the handle as a missing file, because it is not one.
- **Piping unbounded `run_command` output to log-trimmers** (`cargo
  test 2>&1 | grep FAILED`). Server-side enforcement blocks this.
  Run bare, then `grep FAILED @cmd_id` against the buffer.
- **Treating the summary as authoritative.** It's a preview, not the
  whole result. Pull from the buffer before drawing conclusions.
- **Trying to round-trip oversized data back into a tool argument.** A
  result ≳9 KB (e.g. a big `doc(action="augment")` params array) can't be read
  back inline to re-emit as an argument — every read buffers. Write it to
  a file server-side and use a file-reading param instead: `doc(action="augment")`'s
  `params_path`, or the `codescout` CLI's `--params @<file>` / `--params -`.

## Related

- Error routing for inputs that overflow: `get_guide("error-handling")`
