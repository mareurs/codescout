# Progressive Disclosure

How codescout handles results too big to inline, and how the model
should respond to them.

## Output budgets

Tool output is capped to keep the model's context window healthy.
Results exceeding the inline budget are stored in a server-side
`@tool_*` buffer and a compact summary is returned in their place.

| Constant | Value | Source |
|---|---|---|
| `MAX_INLINE_TOKENS` | 2,500 tokens (~10 KB) | `src/tools/core/types.rs` |
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
grep PATTERN @cmd_abc                       # search the buffer
read_file("@tool_xyz", json_path="$.foo")   # extract a JSON field
read_file("@tool_xyz", start_line=N, end_line=M)  # slice lines
```

`@cmd_*` buffers come from `run_command`. `@tool_*` buffers come from
other tools. Both are addressable by any tool that accepts a path.
`@file_*` and `@ack_*` are sibling handle kinds — same mechanics.



## Path-relative annotation

Every non-`run_command` tool response that contains paths under the
active project root carries a trailing `[codescout] paths are relative
to <root>` note. Paths in the response body are project-relative;
prepend `<root>` mentally for absolute resolution. `run_command` output
is exempt (raw shell bytes; stripping would corrupt path literals) and
never carries the annotation. The catalog stores absolute paths;
the strip layer is a display-time transform — when verifying tool
output against catalog state, prefer reading the buffer directly
(`read_file(@tool_xxx, json_path=...)`) on a known-absolute field.

## Anti-patterns

- **Re-running a tool because the result was "too long".** Query the
  buffer instead. The full result is sitting on the server; pulling
  one slice costs no extra tool latency and no extra LLM tokens.
- **Asking the user to paste content from a buffered result.** The
  buffer is server-side — you can read it directly with the `@ref`.
- **Treating `output_id` as a filename.** It's an opaque handle;
  `read_file("@tool_xyz")` works, filesystem paths derived from it
  do not.
- **Piping unbounded `run_command` output to log-trimmers** (`cargo
  test 2>&1 | grep FAILED`). Server-side enforcement blocks this.
  Run bare, then `grep FAILED @cmd_id` against the buffer.
- **Treating the summary as authoritative.** It's a preview, not the
  whole result. Pull from the buffer before drawing conclusions.
- **Trying to round-trip oversized data back into a tool argument.** A
  result ≳9 KB (e.g. a big `artifact_augment` params array) can't be read
  back inline to re-emit as an argument — every read buffers. Write it to
  a file server-side and use a file-reading param instead: `artifact_augment`
  `params_path`, or the `codescout` CLI's `--params @<file>` / `--params -`.

## Related

- Tool authoring patterns: `docs/PROGRESSIVE_DISCOVERABILITY.md`
- Error routing for inputs that overflow: `get_guide("error-handling")`
