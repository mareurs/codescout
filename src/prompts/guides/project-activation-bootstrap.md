# Project Activation Bootstrap

You just activated a project. Orient before you explore or edit — the cheapest
bug is the one you never re-investigate because the project already documented it.

## Phase 0 — load what the project already knows (do FIRST)

- `memory(action="list")`, then read the topics matching your task.
  `architecture`, `gotchas`, and `conventions` usually pay off.
- Bug or regression work:
  `doc(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})` —
  the known-bug ledger. `open` alone hides `taken` (a live session holds it),
  `investigating` (no live owner) and `zombie` (a recurrence check, not a task).
  Don't re-file a filed bug as new; mark a rediscovery KNOWN and cite the ledger path.
- If a `get_guide` topic matches your area (`error-handling`,
  `progressive-disclosure`, `workspace-state`, `librarian`,
  `tracker-conventions`, `untrusted-content`), read it — it states the contract whose violations you
  hunt.
- Long task: use `run_in_background=true`, end your turn, and the companion hook wakes you.
  Do not poll. Subagents get no wake.

## Phase 1 — route each lookup by what you know

- symbol name → `symbols(name=X)`
- concept → `semantic_search(query)`
- exact string → `grep(pattern)`
- who calls X → `references(symbol, path)` — never grep for callers

## Phase 2 — verify at the bytes, not from belief

**Do not hypothesise but ALWAYS VERIFY. Do not state what a doc, a memory, or a
prior belief says the code does — open the artifact or run the command, and ALWAYS
VERIFY.**

- A finding needs lines you actually read (`symbols include_body` / `read_file`),
  not a grep hit alone.
- A claim about how a TOOL behaves needs the call run once and the real output
  read — reading the source alone misses runtime shape.
- A comment, doc, or README the code contradicts is itself a finding
  (doc-vs-code drift).

## Before you plan or touch a contract — run reconnaissance

If you will write a plan, change a struct / function signature / API contract,
or verify claims against `docs/trackers`, invoke the reconnaissance skill FIRST.

- Claude Code: `/codescout-companion:reconnaissance`.
- Other harnesses: follow `docs/templates/session-log.md`.

## When you dispatch subagents — brief them

Pass what you already loaded: memories read, open bugs, prior results. For
guides, pass the *instruction to fetch* (name the topics; the subagent calls
`get_guide`), not a "these are loaded" note: your context is not theirs, and a
topic you triggered will not auto-inject for them unless the companion hook
gives each subagent its own ledger. A subagent that re-discovers what you knew is a dispatch defect (Iron Law 6);
one that fetches its own guide is doing right.
