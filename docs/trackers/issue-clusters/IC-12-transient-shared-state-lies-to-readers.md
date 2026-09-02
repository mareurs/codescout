---
kind: tracker
status: active
title: 'transient shared state lies to every reader, and the standard diagnostic confirms the lie'
owners:
- marius
tags:
- defect-classes
- clusters
- transient-shared-state-lies-to-readers
topic: issue clusters and rule promotion
---

## IC-12 — transient shared state lies to every reader, and the standard diagnostic confirms the lie

**Slug:** `cluster/transient-shared-state-lies-to-readers`
**Claim:** One session's tooling mutates shared state for the duration of an operation. Every other session's read is wrong for that window, and the standard diagnostic reports the lie as truth rather than as an outage — so the symptoms are indistinguishable from permanent loss.
**Members:** `filter={"tags": {"contains": "cluster/transient-shared-state-lies-to-readers"}}` — `n=2`, 2026-09-01, by query. The instance below — measured 2026-08-31 — finally has a file of its own: `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md`, carrying a deterministic isolated reproduction and the mechanism at `pre_commit/staged_files_only.py:108`. This line read *"`n=0` tagged … nothing to tag yet"* until then, which was accurate and is exactly why the class read as empty: the finding had been filed as a paragraph inside another bug file, so no query could reach it. **The second member is `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md`**, and it is this class holding about a gate whose own module header argues *against* reading the working tree: the bound is enforced on the file LIST (`git ls-files`) and not on the file CONTENT (`fs::read_to_string`), so a peer's half-written bug file reds another session's build for the duration of the write. It is also the first member caught **by its own transience** — the red named a cluster the change under test never touched, a hand re-derivation over the identical population returned the ledger's own figure, and a re-run passed 18/18 with nothing altered in between. Its sibling `scripts/pre-commit-ledger-counts.py` reads the index and says so in its header, which makes this a divergence between two implementations of one rule rather than an open problem.
**Blind party:** the *reading* session, and note the inversion — every other class here blinds a writer. Here the writer is fine and the reader is deceived, by an operation it did not initiate and cannot see.
**Promotes to:** `not yet` — `n=2`, and the remedy so far is knowledge rather than mechanism. What changed on 2026-09-01 is legibility, not the count's meaning: the first instance became queryable rather than newly discovered, and the second was filed the same day. **Both members are shared-checkout reads and neither is a race in the usual sense** — the pre-commit stash removes a peer's unstaged work for the duration of someone else's hooks, and the cluster-count gate reads a peer's half-written file as corpus. Spread is therefore 2 across one subsystem (shared-checkout tooling), so this clears neither bar; a third instance **outside** that tooling is what would move it.
**Mechanism status:** none yet. Documented at the point of use (`scripts/pre-commit-unreviewed-content.sh` header, `0b763983`), which is a knowledge fix and by this ledger's own rule a worklist item rather than a rule.
**Valid:** dated 2026-09-01

Measured 2026-08-31, within a minute of git hooks being enabled on this shared checkout. The pre-commit framework stashes unstaged changes while hooks run, and that stash covers **every** session's in-flight work, not only the committing one's. For the sub-second duration of a peer's commit, a session observed its own edited file revert to HEAD content, `git status` report it clean, and a `grep` for text it had just written return nothing.

**The detail that makes it a class rather than a footnote: `git stash list` is EMPTY throughout.** pre-commit writes a patch under `~/.cache/pre-commit` instead of using `git stash`, so the obvious way to detect a stash reports that there is not one. The reader is not merely misinformed — the instrument they would reach for to check confirms the false reading. There is no opt-out; `pre-commit run --help` exposes no stash flag and the stash is unconditional when unstaged changes exist.

**The danger is not the window, it is reacting inside it.** Rewriting a section from memory races the restore and can genuinely lose or duplicate work while "recovering" from a problem that has already fixed itself. So the remedy is an oracle, not a fix: for a librarian artifact, `artifact_event(action="list")`'s `field_patch` byte counts, which no git operation touches; for anything else, `wc -c <path>` against `git show HEAD:<path> | wc -c`. Never `git status`.

**Kept apart from `IC-1` on the observer, not the substrate.** `IC-1` is a write reaching further than the set of peers you can see; here no write collides at all and the shared state is correct at both ends of the window. It generalises past `pre-commit` to anything that transiently mutates shared state — a formatter run, a build that moves files, a script that checks out.

**Falsified by** an instance where the standard diagnostic correctly reported the transient state as unavailable rather than as settled truth. That is an outage, which is a different and much safer thing.
