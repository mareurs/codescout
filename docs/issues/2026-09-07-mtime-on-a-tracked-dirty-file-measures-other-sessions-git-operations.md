---
id: fff5758f95e2fda2
kind: bug
status: open
title: 'BUG: on a tracked-and-dirty file, mtime measures other sessions'' git operations, not its author''s edits'
owners:
- marius
tags:
- cluster/shared-resource-carries-no-owner
topic: mtime as a staleness instrument on a shared checkout
closed: ''
opened: 2026-09-07
owner: marius
related: []
severity: low
---

# BUG: on a tracked-and-dirty file, mtime measures other sessions' git operations, not its author's edits

## Summary

A file that is **tracked and dirty** gets its mtime rewritten by any operation that restores the
working tree — a pre-commit stash/restore, `git stash pop`, a rebase `--autostash` pop — performed
by *any* session sharing the checkout. The write carries no content change and no owner. mtime
therefore drifts **forward only**, so a file looks fresher every time someone else commits or
rebases, and a staleness triage gets a newer answer each time it asks.

## Symptom (Effect)

Two files, restored by one `--autostash` pop, to the same nanosecond, neither with any content
change:

```
2026-09-07 13:41:16.929739289  docs/issues/2026-09-01-peer-idle-timeout-...md
2026-09-07 13:41:16.929739289  .codescout/audit/ripper-65e654-202609.jsonl
```

The first had already been perturbed once that day, by an unrelated operation:

```
12:38:45.358  pre-commit framework stash/restore around commit b8189278 (12:38:41)
13:41:16.929  rebase --autostash pop
```

Its content is dated **2026-09-03** and has not changed since. Its frontmatter says
`last_observed: 2026-09-03`. Its mtime says 2026-09-07 13:41.

## Reproduction

```
git rev-parse HEAD          # 4012dcd7 at filing
```

1. Leave a tracked file modified and uncommitted.
2. From any session sharing the checkout, run any operation that stashes and restores the working
   tree — `git rebase --autostash <upstream>`, `git stash && git stash pop`, or a commit under a
   pre-commit framework that stashes unstaged changes.
3. `stat -c %y <file>` — mtime is now the moment of the *restore*, not of any edit.

Observed twice on 2026-09-07 against the same file, by two unrelated operations.

## Environment

Linux (`ripper`), `experiments`, shared checkout with 2 concurrent sessions in it
(4 live on the machine).

## Root cause

A restore is a **write**. `git` reconstructs the file's bytes and the filesystem stamps mtime at
that moment; nothing distinguishes "rewritten identically" from "edited". The attribute is shared
state with no owner field — `IC-17` exactly — so it records *that* a write happened and never
*who* did it or *whether the content moved*.

**The mechanism is `tracked AND dirty`, not `tracked`.** Measured on one tree, one moment,
varying one factor at a time:

| file | tracked | dirty | content changed by the rebase | mtime | honest? |
|---|---|---|---|---|---|
| `docs/TAXONOMY.md` | yes | no | no — 0 incoming commits | 2026-09-04 09:42 | **did not move** |
| `CLAUDE.md` | yes | no | **yes — 3 incoming commits** | 2026-09-07 13:40:18 | moved, **truthfully** |
| the two files above | yes | **yes** | no | 2026-09-07 13:41:16 | moved, **falsely** |

`TAXONOMY.md` shows trackedness alone is not sufficient. `CLAUDE.md` shows a moving mtime is not
itself the defect — its bytes really did change, so its mtime is telling the truth. Only
*tracked AND dirty* produces a write with no content change.

**Untracked is not "protected", only invisible to these operations.** Three untracked siblings in
the same directory tree kept their 2026-09-02 mtimes to the nanosecond through both events — but
`git stash -u` captures untracked files and `git clean -fdx` deletes them, so the operations that
*do* reach them are more destructive, not less. They were lucky in which operations ran.

measured 2026-09-07: the `stat -c '%y %n'` runs quoted above, before and after each event.

## Evidence

### The direction is always forward, which is the harmful one

Every instance moves mtime later. A triage asking "how stale is this?" gets a **fresher** answer
each time anyone else rebases, so a file is deprioritised precisely as it ages. Backward drift
would be noticed; forward drift looks like activity.

### The obvious mitigation is unavailable

"Commit it so it stops drifting" is what a reader reaches for. For the file above, committing is
exactly what no present session may do — its content belongs to sessionId
`4a8fb556-240b-4ee9-9db5-ec3cc3008a17`, who has exited. The file is pinned in the one state where
the instrument lies about it.

### Three instances in one day, one with a live owner

Two are above. The third was created **by writing this class up**: a peer editing
`docs/issues/2026-09-02-a-filename-matching-an-ack-handle-is-unreachable.md` made it
tracked-and-dirty at 13:54:11. It is the only unambiguous one, and only because that session
announced it — nothing in `git status` distinguishes it from the other two. That is the remedy
clause of `IC-17` demonstrated live: the resource gained an owner by announcement, since it has no
field for one.

### The discriminator was in the file, and two parties walked past it

`last_observed: 2026-09-03` is at **line 9** of the affected file. It does not drift: no git
operation writes it, and only an author ever sets it. Both sessions triaging the file reached for
mtime anyway — and one of them (`89d91024`) had *printed that very field* in its own `git diff`
output hours earlier, for a different question, and still reasoned from the proxy through two
subsequent rounds. Having the right field in hand did not prevent reaching for the number.

## Hypotheses tried

1. **Hypothesis:** the file was edited today by a live session.
   **Test:** `git diff` on it; enumerate socket-bound sessions and ask.
   **Verdict:** rejected — the diff is a section dated 2026-09-03, and the sole peer in the
   checkout reported zero writes.
2. **Hypothesis:** trackedness is the mechanism.
   **Test:** the three-population table in § *Root cause*.
   **Verdict:** rejected — `TAXONOMY.md` is tracked, clean and did not move. Dirtiness is the
   discriminator.
3. **Hypothesis:** it is the pre-commit framework specifically, closed by `b5139fc0`.
   **Test:** re-measured after a `rebase --autostash` on a tree with the framework uninstalled.
   **Verdict:** rejected — it moved again at 13:41:16. `b5139fc0` closed one source; the class
   outlives it.

## Fix

There is nothing to repair in code — mtime is doing what a filesystem does. The fix is a
**triage rule**, and it belongs where triage happens:

**Read the authored field, never mtime.** For bug files that is `last_observed:` where the record
defines one. Where a record defines no such field — the bug template
(`docs/issues/_TEMPLATE.md`) carries `status/opened/closed/severity/owner/related/tags/kind` and
no staleness field, deliberately, because most bugs are deterministic — the answer is
**announcement**, not inventing frontmatter: a session that owns an uncommitted file says so.
Do not add a `last_observed:` to a record that does not already warrant one; that is deciding a
convention rather than reading one.

Candidate mechanisation, not yet designed: `librarian(action="doctor")` already reads bug-file
frontmatter for `entry_dated_stale`. A check that flags a `status: open` file whose
`last_observed` is old — rather than whose mtime is old — would put the right field on the read
surface. Deliberately not specified further here; it needs a population count first.

- **SHA (experiments):** N/A — no code change
- **patch-id:** N/A

## Tests added

None, and the justification is the point rather than an excuse. The defect is a filesystem
attribute changing under a git operation; asserting on it means asserting that mtime moved, which
is asserting that git works. What *is* testable is the remedy — a doctor check reading
`last_observed` — and that does not exist yet. Filing this without a test rather than adding one
that pins the operating system's behaviour.

## Workarounds

- Triage by `last_observed:` / the record's own dated content, never by `stat`.
- `git log -1 --format=%cI -- <path>` for the last *committed* touch, which no restore rewrites.
- To recover a file lost to an autostash: `--autostash` writes a raw commit and records its sha in
  `.git/rebase-merge/autostash`, **not** a `git stash list` entry. After a successful rebase the
  stash list reads 0 and the object survives unreferenced. `git checkout <sha> -- <path>`. An empty
  stash list plus a vanished `M` file is a convincing picture of data loss and is wrong.

## Resume

Decide whether the doctor check in § *Fix* is worth building. Before designing it, count the
population: how many `status: open` bug files carry `last_observed:` at all
(`grep -l '^last_observed:' docs/issues/*.md`), since a check over a field almost nothing declares
is a check that cannot fire. If the count is low, the finding stays a triage rule and this file
stays open as a pointer rather than a task.

## References

- `docs/trackers/issue-clusters/IC-17-shared-resource-carries-no-owner.md` — the class; its remedy
  clause ("isolating the resource or adding an owner field, never a better listing") is what
  § *Fix* concludes independently
- `docs/issues/archive/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` and
  `b5139fc0` — the stash source, closed; this class survived it
- `CLAUDE.md` § *Testing Discipline* — "where a system already names its own failure state, assert
  on the name, not on a proxy for it"
- Found jointly 2026-09-07 by sessionIds `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30` (who noticed the
  second perturbation, built the three-population control, and corrected the author's cluster
  count) and `89d91024-cd66-4361-9300-c55b87b179ea` (who caused both perturbations). Cited by
  sessionId rather than name: a name is registry-minted and re-minted by compaction or a restart,
  a sessionId is not.

