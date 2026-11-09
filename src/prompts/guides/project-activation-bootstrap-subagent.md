# Project Activation Bootstrap (subagent)

Your parent's brief comes first. Load only what your task needs:

- **Memory:** `memory(action="read", topic=...)`; `gotchas`, `conventions`, `architecture` usually pay off; `memory(action="list")` if unsure.
- **Bug or regression work:** `doc(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})`. `status="open"` alone hides `taken` (held by a live session).
- **Contract guides:** `get_guide(topic)` when the task touches one: `error-handling`, `progressive-disclosure`, `workspace-state`, `librarian`, `tracker-conventions`, `untrusted-content`.
- **Lookups:** name → `symbols(name=X)`; concept → `semantic_search(query)`; exact string → `grep(pattern)`; callers → `references(symbol, path)`, never grep.

**Do not hypothesise but ALWAYS VERIFY:** a finding needs lines you read; a claim about a tool needs the call run once.

Full orientation: `get_guide("project-activation-bootstrap")`.
