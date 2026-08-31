---
id: '94e449c896beb016'
kind: bug
status: mitigated
title: 'BUG: an archived bug file''s fix SHA orphans when experiments is rebased, and nothing re-reads archive/ to notice'
tags:
- archive
- git
- citations
- provenance
- patch-id
- cluster/record-asserts-an-unchecked-completion
closed: 2026-08-19
opened: 2026-08-19
owner: marius
related: []
severity: high
unverified: '10 of the 63 archived records were ALREADY unrecoverable when this was mitigated — their objects are gone from the object DB — and no patch-id can restore them. The 53 recoverable ones were back-filled, but nothing gates a FUTURE archive that omits the pair at write time; detection rests on `doctor`''s `terminal_status_without_fix_anchor`, which is run manually. `patch-id` also dies under squash, since a union diff hashes differently.'
---

# BUG: an archived fix SHA orphans on rebase, and nothing re-reads archive/ to notice

> **Status: mitigated 2026-08-19.** The 53 recoverable records now carry a rebase-invariant
> patch-id. The 10 already-dead records are **unrecoverable** — their objects are gone from
> the object database. Root cause (a positional identifier in a permanent record) is not
> fixed; see *Remaining*.

## Summary

`CLAUDE.md` requires an archived bug file to carry its fix SHA, "because nothing re-reads
`archive/`". That SHA is a **positional** identifier: it names a commit's location in a
branch's history. `experiments` is rewritable. When it is rebased, every SHA cited from it
is minted anew and the original is orphaned, then garbage-collected.

The archive therefore certifies a pointer into a namespace that is rewritten underneath it,
and the failure is silent — nothing reads these files again, so nothing ever notices.

## Measurements (2026-08-19)

| | |
|---|---|
| Archived bug files carrying a fix-SHA line | 63 |
| SHA dead — object absent from the object DB | **10 (15.9%)** |
| SHA alive and reachable from `master` (permanent) | 16 |
| SHA alive but `experiments`-only (was hostage) | **37** |
| `master` last advanced | **2026-07-05**, 1104 commits behind `experiments` |

Recoverability of the 10: **none by mechanism.** `git cat-file -t` reports the objects
absent, not merely unreferenced, so the reflog cannot help. Subject-keyword probes returned
between 2 and 153 candidate commits — ambiguous, not a lookup. The *fix* survives in the
tree; the bug-to-commit link does not.

### The control that makes this causal

One file followed the cherry-pick discipline and recorded **both** SHAs:

- `c770cd6e` (experiments-side) — **DEAD**
- `a96af3ae` (master-side, same file) — **RESOLVES**

The discipline works. The 10 that died recorded only the perishable half. This is a known
mechanism firing on schedule, not drift.

### The trap in the monthly trend

By month the dead rate reads May 35%, June 20%, July 23%, **August 0/28** — which looks
like the fast-forward discipline having fixed it. It has not. `master` has not advanced
since 2026-07-05, so August's SHAs have simply **not been rebased yet**; all 28 sat in the
at-risk set. A clean trend can be exactly right about the data and wrong about the world.

## Root cause

A permanent record stores a *positional* identifier. This is the same class as three other
identity schemes here, each with its own measured failure:

| Identity | Derived from | Failure |
|---|---|---|
| artifact id | `sha256(abs_path)` | re-keys on move |
| entry id | per-file `PREFIX-N` counter | 423 ambiguous citations |
| citation qualifier | file stem | truncated past 31 chars (fixed) |
| fix pointer | git SHA on a rewritable branch | **this bug** |

## Mitigation applied

Every archived bug file whose fix SHA still resolves now carries a `## Fix provenance`
section recording `git patch-id --stable` — the content hash of the change's diff, which is
invariant under rebase and cherry-pick. 53 files, 901 insertions, no deletions.

Why patch-id, measured rather than assumed:

- **Controlled test:** a commit cherry-picked onto a different parent changed SHA
  (`cbb7a26b` → `0f2a9a18`) while its patch-id was byte-identical.
- **Coverage:** computable for 3594 of 3613 commits; the 20 without are 18 merges plus
  empties. Of the 53 fix commits, **53 have one and none is a merge**.
- **Specificity:** across all history, 104 patch-ids appear more than once and **all 104
  are the same change on two branches** — cherry-pick pairs. **Zero genuine collisions.**
- **Immediate payoff:** the first file inoculated recorded `e4062186` (experiments-only).
  Its patch-id resolves to *two* commits — the second, `69d09851`, is master-reachable and
  permanent. The record had been pointing at the perishable twin of a change that was
  already safe.

The recorded resolution procedure uses **redirects, not pipes**: codescout's Iron Law 3
blocks an unbounded `git log -p` piped to a trimmer, so the pipe form would have been
unrunnable in the environment that reads it. Caught by executing the instruction before
recording it.

## Remaining

1. **The 10 dead are permanent losses.** Do not spend effort; spend it on prevention.
2. **Root cause stands.** Nothing prevents the *next* record from citing only a SHA. A
   `librarian(action="doctor")` check — *"archived fix SHA no longer resolves"*, and
   *"archived bug file has a fix SHA but no patch-id"* — is the natural home, alongside the
   existing drift checks; that placement is what makes relaxing the archive gate safe.
3. **`patch-id` dies under squash** (a union diff hashes differently). Both documented
   promotion paths — cherry-pick and fast-forward — preserve it. Revisit if squash-merge is
   ever adopted.
4. **A fast-forward promotion** would make all 37 permanent outright, since `master` is a
   strict ancestor (`0 1104`). Deferred: that is a release decision, and it would also make
   permanent 1104 commits including another session's in-flight work.

## References

- `CLAUDE.md` § Bug Tracking — the pending-master-SHA rule and the two promotion paths.
- `get_guide("tracker-conventions")` § Bug files — the archive trigger.
- `docs/RELEASE.md` — cherry-pick vs fast-forward.
- [2026-08-19 run_command rewrites pipes inside heredoc content](archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md) — surfaced by this pass; would have written 53 broken resolution commands.
- [2026-08-18 qualified citation silently truncated past 31 chars](archive/2026-08-18-qualified-citation-silently-truncated-when-file-stem-exceeds-31-chars.md) — sibling positional-identity failure, on the citation qualifier rather than the fix pointer.

## Fix provenance

- **SHA:** `c757cbb4`
- **patch-id:** `4e19a177e02ca8e74a44dc700e18102a9862ad25`
