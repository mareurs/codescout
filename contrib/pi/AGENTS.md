# codescout-aware harness

codescout's tools are the primary path for reading, searching, and editing code.
Use them instead of bash equivalents.

## Reading code
- `symbols` — file/dir symbol overview; add `include_body` for function bodies.
- `read_file` — non-source files or specific line ranges.
- `read_markdown` — markdown (heading-addressed).
- Do NOT `cat`/`sed`/`head` source files via bash.

## Searching
- `semantic_search` — concept-level / natural-language search.
- `references` — who calls / uses a symbol (NOT bash grep).
- exact-regex search: codescout `grep` via the `mcp` proxy (not a first-class tool — its bare name clashes with Pi's built-in grep).
- Do NOT `rg`/`grep -r`/`find -name` source via bash.

## Editing
- `edit_code` — structural, LSP-aware edits (rename, replace/insert/remove a symbol).
- `edit_file` — text/import edits by exact string match.
- `edit_markdown` — markdown edits by heading.
- `write` — create new files.
- Pi's native `edit` is intentionally disabled in this setup.

## Shell
- `bash` — tests, git, build, and process tasks only.

## Deeper codescout (on demand)
- Trackers/artifacts, project memory, librarian, workspace, indexing, and other
  codescout tools are reachable via the `mcp` proxy tool when needed.
