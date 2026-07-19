# codescout-aware harness

codescout's tools are the primary path for reading, writing, searching, and editing code.
Native `read`, `write`, `edit`, and `bash` are hard-blocked for the cases codescout covers
(via a `tool_call` hook, not just a suggestion — see codescout-mode.ts).

## Reading code

- `symbols` — file/dir symbol overview; add `include_body` for function bodies.
- `read_file` — non-source files or specific line ranges.
- `read_markdown` — markdown (heading-addressed).
- Native `read` is blocked except for images (jpg/png/gif/webp/bmp) — codescout can't view images.

## Searching

- `semantic_search` — concept-level / natural-language search.
- `references` — who calls / uses a symbol (NOT bash grep).
- exact-regex search: `codescout_grep` — direct tool.
- Native `bash` is blocked for source search/dump (`rg`/`ag`, `grep -r`, `find -name`,
  `cat`/`head`/`tail`/`sed`/`awk` on source files). Append `# codescout-override` to the
  command if raw shell access is genuinely required.

## Editing

- `edit_code` — structural, LSP-aware edits (rename, replace/insert/remove a symbol).
- `edit_file` — text/import edits by exact string match.
- `edit_markdown` — markdown edits by heading.
- `create_file` — create new files (pass `overwrite: true` to replace an existing file).
- Native `edit` and `write` are hard-blocked.
- Paths outside the active project need `approve_write` (or the inline `@ack_*` prompt)
  once per session before `edit_file`/`create_file`/`edit_code`/`edit_markdown` can touch them.

## Shell

- `bash` — tests, git, build, and process tasks. Blocked only for the source read/search
  patterns described above.
- `run_command` — codescout's own shell runner (reachable via the `mcp` proxy); project-root
  scoped, 30s default timeout, dangerous-command confirmation.

## Research
- `researcher_research_run` — direct tool. Use `/research-web` for inline lookups, `/research-subagent` for deep/isolated research.
- Load `researcher-mcp` skill for tool selection matrix and brief template.

## Deeper codescout (on demand)
- Trackers/artifacts, project memory, librarian, workspace, indexing, and other
  codescout tools are reachable via the `mcp` proxy tool when needed.
