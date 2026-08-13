---
status: investigating
opened: 2026-08-13
closed:
severity: medium
owner: marius
related: []
tags: [worktree, workspace-activation, semantic-search, retrieval, companion-plugin, agent-agnostic]
kind: bug
---

# BUG: Claude Code's `EnterWorktree` desyncs codescout's active project, and activating the worktree strands semantic search

## Summary

Claude Code can switch the session into a linked git worktree (its `EnterWorktree`
tool). codescout does not follow: its active project and cwd stay in the main
checkout, so every subsequent codescout tool call reads and edits the **wrong
tree**. Activating the worktree in codescout fixes the tree but loses semantic
search, because the retrieval index is keyed per project and the worktree has no
index of its own. Both halves are real, and the second is not obviously a defect
— a worktree's contents diverge from main's, so serving main's vectors for a
worktree query would return confidently stale results.

Two coupled problems, and they want different fixes: the first is a missing
signal, the second is a missing design decision.

## Symptom (Effect)

User-reported, 2026-08-13 (not yet reproduced by an agent in-session):

1. Claude Code enters a worktree via `EnterWorktree`. codescout's active project
   is unchanged — it still points at the main checkout. Native tools operate on
   the worktree; codescout's `symbols`/`grep`/`edit_code`/`read_file` operate on
   main. Nothing reports the divergence.
2. Telling codescout to switch (`workspace(action="activate", path=<worktree>)`)
   corrects the tree, and **semantic search stops working** for that project.

Exact tool output for either half is not yet captured — see *Resume*.

## Reproduction

Not yet reproduced in-session. Intended steps:

```
# 1. from the main checkout, note the active project
workspace(action="status")

# 2. Claude Code side: EnterWorktree into a linked worktree
# 3. re-check — expected to still name the main checkout
workspace(action="status")

# 4. confirm the wrong-tree read: edit a file in the worktree natively,
#    then read the same relative path through codescout
# 5. activate the worktree, then query
workspace(action="activate", path="<repo>/.worktrees/<name>")
semantic_search(query="<something the corpus certainly contains>")
```

## Environment

- codescout `experiments` @ `c5f434df`; v0.15.0.
- Claude Code with the `codescout-companion` plugin active — native
  `Read`/`Grep`/`Glob`/`Edit`/`Write` and all native `Bash` are hard-denied on
  source files, so codescout's tools are the *only* path to source. That raises
  the severity of half 1: there is no fallback that happens to be correct.
- Worktrees under `<repo>/.worktrees/<name>` (the convention `doctor` and
  `merge_worktree` already know).

## Root cause

**Unknown — under investigation.** Two independent leads, one of them measured.

### Lead 1 — no signal exists (mechanism not yet read)

`EnterWorktree` is a Claude Code harness tool. codescout is an MCP server in a
separate process; nothing in the MCP protocol pushes a cwd change to a server,
and codescout's active project is set only by `workspace(action="activate")` or
by its own startup resolution. So there is likely **no signal to miss** — the
gap is architectural, not a dropped event. This is *inferred from the protocol
shape and not yet verified against `src/librarian/current_project.rs`*, which is
where activation resolves.

This is squarely an **Agent-Agnostic Design** question (memory `conventions`):
`EnterWorktree` is Claude-Code-specific, so the fix must not be a
Claude-Code-specific hook in the server. The companion plugin is the
harness-specific layer and is the more likely home.

### Lead 2 — retrieval has no worktree concept at all (measured)

*Measured 2026-08-13:* `grep worktree src/**/*.rs` → 737 matches in 46 files,
concentrated in the librarian (`librarian/catalog/worktree.rs`,
`librarian/tools/merge_worktree.rs`, `librarian/current_project.rs`,
`librarian/tools/doctor.rs`). The **entire** retrieval subsystem carries two
matches, both in `src/retrieval/sync.rs` (107, 385), and both say only *"a file
named `.git` is a worktree pointer, skip it"*.

So the librarian grew a full worktree overlay — fork-on-first-write shadow rows,
`worktree_of` lineage, `merge_worktree` — and the code-retrieval side grew none.
A worktree activated as a project is, to retrieval, simply a project with no
index. Whether that manifests as an empty result, a refusal, a hint, or a silent
fallback to main's vectors is **not yet measured**, and the difference matters:
the last of those would be the worst outcome and the easiest to ship by accident.

The user's own reading — that refusing is *correct*, because a worktree's files
diverge from main's — is the strongest constraint on any fix. Serving main's
vectors for a worktree query is wrong in a way that looks right.

## Evidence

### The librarian/retrieval asymmetry

```
$ grep worktree src/**/*.rs   (mode=files, 2026-08-13)
148  src/librarian/tools/doctor.rs
 84  src/librarian/tools/merge_worktree.rs
 49  src/librarian/tools/worktree.rs
 45  src/librarian/catalog/worktree.rs
 39  src/librarian/current_project.rs
 ...
  2  src/retrieval/sync.rs        ← the whole retrieval subsystem
```

```
$ grep worktree src/retrieval/*.rs   (mode=content)
sync.rs:107: /// Directory-only by design: a *file* named `.git` is a worktree pointer and a
sync.rs:385: // A FILE named `.git` is a worktree pointer, not a state tree. Skipping it by
```

### Prior worktree bugs, all archived, all catalog-side

`artifact(find, kind="bug", filter=worktree, scope="umbrella", include_archived=true)`
returns 4, every one `fixed` and every one about the **librarian catalog**:
`2026-06-13-linked-worktree-indexed-as-project-pollutes-catalog.md`,
`2026-07-17-worktree-cites-refusal-materializes-shadow-fork.md`,
`2026-05-28-path-annotation-spam.md`,
`2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md`.
None covers activation desync or retrieval. This is not a rediscovery.

Worth noting that `2026-05-28-path-annotation-spam.md` closed with "activation +
worktree state invisible" in its title — the visibility half of half 1 has been
touched before, which is a lead for whether a surface already exists to extend.

## Hypotheses tried

1. **Hypothesis:** this is already filed and I am about to re-file it.
   **Test:** umbrella-scoped bug query including archived, on `worktree`.
   **Verdict:** rejected — 4 hits, all fixed, all catalog-side.

2. **Hypothesis:** retrieval has partial worktree handling that merely has a gap.
   **Test:** `grep worktree src/retrieval/*.rs`.
   **Verdict:** rejected — two matches, both about skipping the `.git` pointer
   file. There is no partial handling to extend; there is nothing.

3. **Hypothesis:** activating a worktree silently serves the main checkout's
   vectors (worst case — confidently stale results).
   **Verdict:** deferred — needs the call actually run. This is the single most
   important thing to measure, because it decides whether the current behaviour
   is merely unhelpful or actively wrong.

## Fix

**Not yet decided — needs Lead 2 measured first.** The design space, recorded so
the exploration has something to falsify rather than invent:

*Half 1 — the desync.* Candidates: (a) the companion plugin detects the
worktree switch and issues `workspace(activate)` itself, keeping the
harness-specific knowledge in the harness-specific layer; (b) codescout notices
that its resolved project root is not the caller's cwd and says so once, which is
agent-agnostic and degrades to a warning rather than a silent wrong answer;
(c) both. (b) is attractive because it fails loudly in *any* harness, including
ones that have no worktree tool at all.

*Half 2 — semantic search in a worktree.* Candidates: (a) refuse with a
`RecoverableError` naming the main checkout and the cost of indexing the
worktree — honest, and the current behaviour if it already refuses; (b) serve
main's index with an explicit staleness annotation, bounded to files the worktree
has not modified (`git diff --name-only <base>` is the exact discriminator);
(c) index the worktree as its own project, correct but expensive per worktree.
(b) is the interesting one and mirrors the shape the librarian already chose —
overlay reads from main until the worktree writes.

## Tests added

None yet — not fixed. When it is, the guard has to run *in* a worktree, which
`src/librarian/catalog/worktree.rs`'s existing tests already do somehow; read
those for the fixture pattern before inventing one. Note the trap this cohort
just hit (F-32): a test that only fails in an environment nobody runs is not a
guard. A worktree fixture must be created by the test, not assumed present.

## Workarounds

Two, both manual:

- Stay in the main checkout for codescout work; do the worktree's editing there
  and let git move it. Loses the isolation `EnterWorktree` was for.
- Or activate the worktree in codescout and accept no semantic search, using
  `symbols` / `grep` / `references` instead — all of which are computed live from
  the filesystem and therefore correct in a worktree. Only the vector index is
  stale-by-construction.

The second is the better one, and worth knowing: **losing semantic search is not
losing code intelligence.** Symbol and reference navigation still work.

## Resume

Measure Lead 2 before touching any code, because it decides the whole shape:

1. Create a throwaway worktree, `workspace(action="activate")` it, and run
   `semantic_search(query=...)`. Record the **verbatim** response. Refusal,
   empty, hint, or main's results? If it silently returns main's results, that is
   a second, worse bug and gets its own file.
2. Read `src/librarian/current_project.rs` for how activation resolves a worktree
   root, and whether a project_id is derived per-worktree or shared with main.
3. Read `src/tools/config/mod.rs::check_has_index` and the `project_id` used by
   `semantic_search` to confirm the keying.
4. Only then decide between the Fix candidates above.

## References

- `src/retrieval/sync.rs:107,385` — retrieval's only worktree mentions.
- `src/librarian/catalog/worktree.rs`, `src/librarian/tools/merge_worktree.rs` —
  the overlay design retrieval did not get; the precedent for Fix half-2 (b).
- `get_guide("workspace-state")` — activation, home/foreign, reset semantics.
- `docs/architecture/companion-plugin.md` — the harness-specific layer, likely
  home for Fix half-1 (a).
- memory `conventions` § Agent-Agnostic Design — why the server must not learn
  about `EnterWorktree` by name.
- `docs/trackers/release-promotion-session-log.md` F-32 — the ambient-dependency
  test trap this fix's guard must avoid.
