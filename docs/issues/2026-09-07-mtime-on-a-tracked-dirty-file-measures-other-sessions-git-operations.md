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


### Re-verified 2026-10-08 — identical git rewrites cause a false buffer change notice

Live server: `workspace(status).server.git_sha=a2871af0`, `git_dirty=true`, `exe_deleted=false`. Code inspected at HEAD `21237e7f2734d8757c6b9666e2f79a9cfd6a318c`.

In a disposable git repo under `/tmp`, committed a baseline, left `tracked.txt` dirty (600 lines, 27,000 bytes), set its mtime to 1700000000000000000 ns, and read it through the live MCP to obtain one file handle. `git stash push` followed by `git stash pop` advanced mtime to 1791434122835862084 ns. SHA-256 before and after was identical: `4c1d8cc098e6c361a9aef49a20ca431860c5ceaddad14cf9466f7d958780734f`. The first `run_command(wc -l <handle>)` then printed `refreshed from disk (file changed since last read)`; the second printed no notice. Both returned 600 lines.

This now affects code: `OutputBuffer::resolve` reports `needs_refresh` whenever mtime advances, without comparing the re-read bytes. The older statement that there is nothing to repair in code applies to authorship triage, not this new change notice. Keep mtime as a signal to re-read; compare bytes before claiming a content change.

Separately, `store_file_inner` rejects a late read only when its mtime predates the entry stamp and the disk no longer has that read's mtime. An identical rewrite can make an old store ineligible, but this probe did not establish corruption or loss of a newer version. The ordered stale-read regression is present; equal-millisecond changes remain a separate limit of this instrument. Keep the authorship/doctor proposal open. The buffer consequence is fixed below.

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
### Buffer mitigation verified 2026-10-08

The refresh-notice consequence is fixed in experiments `4487a34c2e7a4a919b2d0bd9e92be7f3856f9d34` (patch-id `7e1a3a2858f3c27a5b777db567352e50b31b6d44`). Mtime still triggers a re-read and advances the read stamp; only changed bytes enqueue a content-change notice. Silent reads retain that pending notice for the next reporting read. An identical git rewrite no longer produces a spurious content-change notice.

The full gate passed. Buffer tests cover unchanged rewrites, silent refreshes, existing pending notices, future mtimes and one-shot reporting. The updated plain/Markdown read-race regression passed and killed independently applied late-stat mutations on both paths. A fresh stdio MCP probe observed unchanged-byte silence and a changed-byte notice exactly once. The installed MCP binary was not replaced.

This fixes the buffer consequence, not git's filesystem property or the authored-field/doctor proposal below. The stale-write guard is unchanged; equal-millisecond changes remain outside this probe's conclusion.
### Re-verified 2026-09-24 — the most frequent trigger is gone; the property is not

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`), verifier evidence. **The per-commit trigger is removed:** `074b749e` (patch-id `4c3958557408b19cdf60354a5f8288167e4342e4`) retired pre-commit's stash-and-restore cycle. `scripts/pre-commit-run.sh` mentions stash only in comments, the installed `.git/hooks/pre-commit` has 0 `stash` occurrences, and `rebase.autoStash` is unset. **The property this file describes still holds:** reproduced in a temp repo, an explicit `git stash; git stash pop` moved a dirty file's mtime from 2026-09-01 00:00 to 2026-09-24 14:46 with no content change, and `rebase --autostash` behaves the same way. So mtime on a tracked, dirty file is still not authorship evidence, just much less often wrong. The remedy this file proposes, a triage rule plus a `doctor` `last_observed` check, was never built (0 hits in `src/librarian`). This is a property of git rather than a code defect in this repo.

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
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` and
  `b5139fc0` — the stash source, closed; this class survived it
- `CLAUDE.md` § *Testing Discipline* — "where a system already names its own failure state, assert
  on the name, not on a proxy for it"
- Found jointly 2026-09-07 by sessionIds `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30` (who noticed the
  second perturbation, built the three-population control, and corrected the author's cluster
  count) and `89d91024-cd66-4361-9300-c55b87b179ea` (who caused both perturbations). Cited by
  sessionId rather than name: a name is registry-minted and re-minted by compaction or a restart,
  a sessionId is not.
