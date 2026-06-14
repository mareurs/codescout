codescout MCP — semantic code intelligence.
Subagents inherit these rules. Pass them along.

## Iron Laws (never X, do Y)

1. NEVER full-read source → symbols(path) overview,
   symbols(name=..., include_body=true) bodies. Line-range
   read_file is fine for imports/glue.
2. NEVER edit_file structural code → edit_code (LSP-aware).
3. NEVER pipe unbounded run_command → run bare, query @cmd_* buffer
   (grep "ERROR" @cmd_abc). Bounded LHS (ls, cat, awk, sed,
   find -maxdepth N) is OK. Shell on source files is blocked.
4. NEVER read_file markdown → read_markdown (heading-addressed).
5. NEVER edit_file markdown → edit_markdown (heading-addressed).
6. Subagents see only what you brief them with. Pass: which
   get_guide(topic) to call (or the content itself), prior tool results,
   file paths, symbol names, topics already triggered this session.
   Applies at every spawn boundary. A subagent re-discovering what you
   knew is a dispatch defect — yours, not theirs.
## Search/Edit decision quickref

- Know name → symbols(name=X) | symbol_at(path, line, col)
- Know concept → semantic_search(query)
- Exact string/regex → grep(pattern, path=optional)
- Who calls X → references(symbol, path) — NOT grep
- Structural code edit → edit_code | Text/import edit → edit_file

## Workspace gate

After workspace(activate, path=foreign), call workspace(activate, path=home)
before finishing the turn. Foreign-project state otherwise leaks.

Parallel subagents on DIFFERENT workspaces: pin each call with
workspace=<abs path>, don't activate. Full rules: get_guide("workspace-state").

## Deeper guidance

Call get_guide(topic) where topic in:
- "librarian"               — artifact model, filters, trackers, body editing
- "tracker-conventions"     — frontmatter, archive flow, status
- "progressive-disclosure"  — output budgets, @ref buffer details
- "error-handling"          — RecoverableError vs anyhow::bail
- "workspace-state"         — activate_project, home/foreign, ledger reset
- "iron-laws-detail"        — per-law gate text, exceptions, edge cases
- "symbol-navigation"       — per-language symbol/ref nav tips
