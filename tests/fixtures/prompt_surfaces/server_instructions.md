codescout MCP — semantic code intelligence.
Subagents inherit these rules. Pass them along.

## Iron Laws (never X, do Y)

1. NEVER full-read source → symbols(path) overview,
   symbols(name=..., include_body=true) bodies. Line-range
   read_file is right for imports/glue; force=true overrides.
2. NEVER edit_file structural code → edit_code (LSP-aware).
3. NEVER pipe unbounded run_command → run bare, query @cmd_* buffer
   (grep "ERROR" @cmd_abc). Bounded LHS (ls, cat, awk,
   sed) is OK. Shell on source files is blocked.
4. NEVER read_file markdown → read_markdown (heading-addressed).
5. NEVER edit_file markdown → edit_markdown (heading-addressed).
6. Subagents see only what you brief them with. Name the guides they
   must fetch themselves, prior results, paths, symbols — at every
   spawn. Re-discovery is your dispatch defect, not theirs.
## Search/Edit decision quickref

- Know name → symbols(name=X) | symbol_at(path, line, col)
- Know concept → semantic_search(query)
- Exact string/regex → grep(pattern, glob, mode="files")
- Who calls X → references(symbol, path) — NOT grep
- Structural code edit → edit_code | Text/import edit → edit_file
- After workspace(activate, foreign) → activate home before turn end

## Deeper guidance

Call get_guide(topic) FIRST before deeper work:
- "librarian" — artifacts, filters, trackers
- "tracker-conventions" — entry fields (`**Valid:**`, `**Rests on:**`), status, archive
- "progressive-disclosure" — output budgets, @ref buffers
- "error-handling" — RecoverableError vs anyhow::bail
- "workspace-state" — activate, home/foreign, pinning, reset
- "iron-laws-detail" — gate text + exceptions
- "symbol-navigation" — per-language nav tips
