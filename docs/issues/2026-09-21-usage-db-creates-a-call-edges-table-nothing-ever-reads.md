---
id: c4e0c5cc182997ba
kind: bug
status: open
title: usage.db creates a call_edges table on every open that no code path has ever read or written
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-21
owner: marius
related: []
severity: low
---

# BUG: `usage.db` creates a `call_edges` table on every open that no code path has ever read or written

## Summary

`src/usage/db.rs:5` `open_db` executes `CREATE TABLE IF NOT EXISTS call_edges (…)` plus three
indexes (`src/usage/db.rs:34-47`) inside `.codescout/usage.db` on **every open**. Nothing reads or
writes that table. The live call-edge cache is a different file — `.codescout/call_edges.db`, opened
by `src/tools/symbol/call_edges/cache.rs:38` — and every SQL statement against a `call_edges` table
runs through `EdgeCache`, whose production connections all come from that second `open_db`.

The defect is a vestigial schema object, not a correctness bug. The one observable consequence is
that `.claude/skills/analyze-usage/SKILL.md:358` runs `DELETE FROM call_edges` against `usage.db` as
part of its clear step — and `:385` lists forgetting it as a "Common Mistake" — advice that reads as
necessary, succeeds, and does nothing.

**Severity low, and this is filed as incidental.** It costs one `CREATE TABLE IF NOT EXISTS` and
three `CREATE INDEX IF NOT EXISTS` per open against an empty table. It may well not be worth fixing;
§ *Fix* states the argument both ways rather than prescribing one.

## Symptom (Effect)

Measured 2026-09-21 at HEAD `46c3aa57770421592a8e36573c4d57a530e3d74f`, branch `experiments`:

```
$ sqlite3 .codescout/usage.db "SELECT count(*) FROM call_edges; SELECT count(*) FROM tool_calls;"
0
70148
$ sqlite3 .codescout/call_edges.db "SELECT count(*) FROM call_edges;"
9794
$ sqlite3 crates/librarian-mcp/.codescout/usage.db "SELECT count(*) FROM call_edges;"
0
```

`usage.db` is live — 70,148 `tool_calls` rows — and its `call_edges` table is empty. The 9,794 real
edges are in the sibling file. A second project's `usage.db` in this same checkout reports `0` too.

Nothing errors anywhere. `DELETE FROM call_edges` against `usage.db` exits `0` having deleted
nothing, which is indistinguishable from a successful clear of a populated table.

## Reproduction

At any commit containing `src/usage/db.rs:34`:

```
sqlite3 .codescout/usage.db "SELECT count(*) FROM call_edges;"      # 0, always
sqlite3 .codescout/call_edges.db "SELECT count(*) FROM call_edges;" # the real cache
```

Run a `call_graph` query first if `.codescout/call_edges.db` does not yet exist; the `usage.db`
figure does not move.

## Environment

Linux, branch `experiments`, HEAD `46c3aa57`. Applies to every project codescout activates — the DDL
is unconditional in `open_db`, not feature-gated.

## Root cause

**Two declarations of one table; one of them has no reader.**

- `src/usage/db.rs:34-47` — `CREATE TABLE IF NOT EXISTS call_edges` + `call_edges_caller`,
  `call_edges_callee`, `call_edges_file`, inside the `execute_batch` that also creates `tool_calls`
  and `lsp_events`. These four lines are the *only* occurrences of `call_edges` in `src/usage/`.
- `src/tools/symbol/call_edges/cache.rs:9` (`apply_schema`) declares the identical DDL, and `:38`
  `open_db` applies it to `.codescout/call_edges.db` (`:46`).
- Every statement against the table lives in `cache.rs`: `:70` `lookup_callers`, `:81`
  `lookup_callees`, `:103` `upsert`, `:122` `invalidate_file` — all methods of `EdgeCache`.
- Every production `EdgeCache::new` takes a connection from `call_edges::cache::open_db`:
  `src/agent/mod.rs:1638` (conn from `:1634`), `src/agent/mod.rs:1672` (conn from `:1668`),
  `src/tools/symbol/call_graph/mod.rs:228` and `:270` (conn from `:456`). The other seven
  `EdgeCache::new` call sites are in-memory test connections inside `cache.rs`.

Measured 2026-09-21: `grep(pattern="call_edges", path="src", glob="*.rs")` → **61 matches in 11
files**; `references(symbol="impl EdgeCache<'a>/new", path=…)` → **12 references in 3 files**, of
which 4 are production and 8 are in `cache.rs` tests. No call site anywhere pairs a
`usage::db::open_db` connection with the `call_edges` table.

**History — and it falsifies the obvious reading of it.** The natural hypothesis is *"the L-01 split
created the new file and never dropped the old table"*. That is not what happened, and the
difference is worth recording because the "old" store was never `usage.db`.

- `9053f2ea` (2026-05-01 18:12) added the DDL to `src/usage/db.rs`, with the commit message *"Adds
  call_edges table to the project DB (usage.db)"*. At that commit `cache.rs` had **no `open_db` at
  all** — only `apply_schema`, `EdgeCache`, and tests on `Connection::open_in_memory()`. Nothing in
  production opened it.
- `db4ec198` (2026-05-01 18:22 — **ten minutes later**) wired the production cache into the **embed**
  DB instead: `src/embed/index.rs::open_db` gained an `apply_schema()` call, and the new
  `Agent::invalidate_call_edges` opened `crate::embed::index::open_db`, i.e.
  `.codescout/embeddings/project.db`.
- `6ccab48f` (2026-05-13, "L-01 step 7.5") moved the cache out of the embed DB into
  `.codescout/call_edges.db`. Its message names `embeddings/project.db` as the thing being left
  behind, and so does the surviving doc comment at `cache.rs:34-37`. Neither mentions `usage.db`.

So the `usage.db` table was dead within ten minutes of being created and has never held a row written
by production code. `git log -S 'CREATE TABLE IF NOT EXISTS call_edges' -- src/usage/db.rs` returns
exactly one commit: the DDL has not been touched since it was added.

**And that history is why the documentation half is not `cluster/doc-contradicted-by-code`.** That
class requires the statement to have been *true when written*. `7105b839` (2026-05-03) added the
`DELETE FROM call_edges` line to the skill **two days after** the production path had already gone to
the embed DB. It was composed from the schema, not from the data path — which is this file's class,
not `IC-11`'s.

## Evidence

### The DDL, `src/usage/db.rs:34-47`

```
        CREATE TABLE IF NOT EXISTS call_edges (
            project_id   TEXT NOT NULL,
            caller_sym   TEXT NOT NULL,
            callee_sym   TEXT NOT NULL,
            file         TEXT NOT NULL,
            line         INTEGER NOT NULL,
            col          INTEGER NOT NULL,
            source       TEXT NOT NULL,
            computed_at  INTEGER NOT NULL,
            PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
        );
        CREATE INDEX IF NOT EXISTS call_edges_caller ON call_edges(project_id, caller_sym);
        CREATE INDEX IF NOT EXISTS call_edges_callee ON call_edges(project_id, callee_sym);
        CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);
```

### The live store, `src/tools/symbol/call_edges/cache.rs:34-46`

```
/// This used to share `.codescout/embeddings/project.db` with the legacy
/// `embed::index` storage, but the two concerns were structurally
/// unrelated — call_edges is an LSP cache, not a semantic index — so the
/// L-01 retrieval-stack migration split them into separate files.
pub fn open_db(project_root: &std::path::Path) -> rusqlite::Result<Connection> {
    ...
    let db_path = dir.join("call_edges.db");
```

### The one consumer that reads the dead declaration as live

`.claude/skills/analyze-usage/SKILL.md:358`, in § *3. Clear each DB*, where `<db>` is the
`*/.codescout/usage.db` path the skill's own § *Common Mistakes* insists on:

```bash
sqlite3 <db> "DELETE FROM tool_calls; DELETE FROM lsp_events; DELETE FROM call_edges; VACUUM;"
```

and `:385`:

```
- **Forgetting `call_edges`** — three tables need clearing: `tool_calls`, `lsp_events`, `call_edges`.
```

Two of the three are real. The third is always empty, and the real edge cache —
`.codescout/call_edges.db`, 9,794 rows here — is never cleared by this step at all. So the advice is
inert in one direction and silently incomplete in the other.

## Hypotheses tried

1. **Hypothesis:** the L-01 split (`6ccab48f`) moved the cache out of `usage.db` and forgot to drop
   the table. **Test:** `git log -S 'call_edges.db'`, then read `cache.rs` and `agent/mod.rs` at
   `9053f2ea` and `db4ec198`. **Verdict:** **rejected.** The production store went from *nothing* to
   `embeddings/project.db` (ten minutes after the DDL landed) to `call_edges.db`. `usage.db` was never
   in that chain. Evidence: § *Root cause*, history bullets.
2. **Hypothesis:** the SKILL.md line was correct when written and decayed. **Test:**
   `git log -S 'DELETE FROM call_edges' -- .claude/skills/analyze-usage/SKILL.md` → `7105b839`,
   2026-05-03, against `db4ec198` on 2026-05-01. **Verdict:** **rejected** — never true.
3. **Hypothesis:** some other consumer (dashboard, CLI, a test fixture) reads `call_edges` out of
   `usage.db`. **Test:** `grep` over `src/**/*.rs` (61 matches / 11 files) and
   `references` on `EdgeCache::new` (12 refs / 3 files). **Verdict:** **rejected** — the four
   production constructions all take a `call_edges::cache::open_db` connection.

## Fix

**Not attempted — filing only.**

The smallest coherent change is two edits that must land together:

- delete `src/usage/db.rs:34-47`;
- delete the `DELETE FROM call_edges;` clause at `.claude/skills/analyze-usage/SKILL.md:358` and the
  Common-Mistakes bullet at `:385`.

They are coupled in one direction only, and it is the direction that bites: dropping the DDL while
the skill still runs `DELETE FROM call_edges` turns a silent no-op into `Error: no such table` on
every clear. A `DROP TABLE` migration for existing `usage.db` files is optional and probably not
worth it — it is a write to every project's database to reclaim an empty table.

**The argument for leaving it:** the table is empty, `IF NOT EXISTS` makes the DDL a no-op after the
first open, and no query plan touches it. Doing nothing costs nothing measurable. **The argument for
removing it:** the declaration has already been read as a shipped feature once, by a documentation
surface that then told its readers that forgetting it was a mistake. That is the class this file
records, and the cost is one reader's confidence, not any runtime cycle.

Either outcome is acceptable. `wontfix` with this file as the record would be a defensible close.

## Tests added

N/A — no code change in this commit. If the DDL is removed, the regression guard worth having is not
a test of `usage.db` (asserting the *absence* of a table is monotone under removal of the whole
`open_db`, per `CLAUDE.md` § *Testing Discipline*) but an assertion that the analyze-usage clear step
names exactly the tables `src/usage/db.rs` creates — a per-member check over a set that can grow,
rather than an `is_empty()` over it.

## Workarounds

None needed. The `DELETE FROM call_edges` step is harmless; it is merely inert.

## References

- `src/usage/db.rs:5`, `:34-47`
- `src/tools/symbol/call_edges/cache.rs:9`, `:34-46`, `:70`, `:81`, `:103`, `:122`
- `src/agent/mod.rs:1614-1642`, `:1649-1674`
- `src/tools/symbol/call_graph/mod.rs:228`, `:270`, `:456`
- `.claude/skills/analyze-usage/SKILL.md:358`, `:385`
- Commits `9053f2ea`, `db4ec198`, `6ccab48f`, `7105b839`
- Class: `docs/trackers/issue-clusters/IC-3-declared-not-wired.md`
