# Project Activation Bootstrap

You just activated a project. Orient before you explore or edit — the cheapest
bug is the one you never re-investigate because the project already documented it.

## Phase 0 — load what the project already knows (do FIRST)

- `memory(action="list")`, then read the topics matching your task.
  `architecture`, `gotchas`, and `conventions` usually pay off.
- Bug or regression work:
  `artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating", "zombie"]}})` —
  `status="open"` alone hides `investigating` (actively being worked) and
  `zombie` (recurring-but-unconfirmed — a "has this come back?" check, not a
  task to pick up) —
  the known-bug ledger. Don't re-file a filed bug as new; mark a rediscovery
  KNOWN and cite the ledger path.
- If a `get_guide` topic matches your area (`error-handling`,
  `progressive-disclosure`, `workspace-state`, `librarian`,
  `tracker-conventions`), read it — it states the contract whose violations you
  hunt.

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
- Other harnesses: follow `docs/templates/session-log.md` (any agent that reads
  markdown can use the template — no plugin required).

It forces the doc-vs-code reconciliation and logs frictions (F-N) and wins (W-N)
so the next session inherits them.

## When you dispatch subagents — brief them

Pass what you already loaded: memories read, open bugs, prior results. For
guides, pass the *instruction to fetch*, not a "these are loaded" note — the
guide-hint ledger is shared parent↔subagent, so a topic you triggered will not
auto-inject for them, and your context is not theirs. Name the topics and tell
them to call `get_guide` on the ones their task needs; an explicit fetch always
returns the full body. A subagent re-discovering what you already knew is a
dispatch defect (Iron Law 6), not the subagent's fault — but a subagent fetching
its own guide is doing exactly what it should.
