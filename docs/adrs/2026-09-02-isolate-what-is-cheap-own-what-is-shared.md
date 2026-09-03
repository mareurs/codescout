---
id: f123cb2fe4697656
kind: adr
status: draft
title: Isolate what is cheap to isolate, own what must be shared, and stop storing shared mutable state
tags:
- architecture
- concurrency
- shared-checkout
- worktree
- ledgers
- multi-agent
topic: multi-agent contention strategy
---

# ADR: isolate what is cheap to isolate, own what must be shared, and stop storing shared mutable state

- **Date:** 2026-09-02
- **Status:** proposed (no code written)
- **Deciders:** Marius (with session `f13f8169-93a1-4392-95d1-8774d296e0c0`)
- **Commits:** none yet. Diagnosis measured on `experiments` at `b8c226df`, during an evening
  with **9 sessions live in this checkout** (8 peers plus me, by socket enumeration) and 2
  linked worktrees.

## Decision

**Multi-agent contention on this repo is a power law with a ledger head, not a tree-level
problem — so isolate at the artifact level, not the checkout level.**

Four layers, ordered by payoff, only the first of which is large:

1. **Remove shared *mutable* state from ledgers.** Entries land as per-session files; a fold
   merges them; **ids are allocated at fold time, not at write time.**
2. **Give every remaining shared resource an owner field**, on the model of
   `.git/session-stage-log`.
3. **Isolate `target/` per feature-matrix lane**, not per session.
4. **Serialize the tail** rather than isolating it.

**And explicitly: do not adopt per-session worktrees as the general remedy.** They cost 29 GB
each to solve a 43 MB problem, leave the actual hot spot shared, and introduce a failure class
that is worse than the one they remove.

## Context / forces

### Measured 2026-09-02 — contention, by distinct `Session-Id` trailers per file, one day

| file | sessions | commits |
|---|---:|---:|
| `docs/trackers/issue-clusters.md` | **16** | 53 |
| `src/server.rs` | 7 | 10 |
| `docs/issues/…peer-idle-timeout…` | 6 | 8 |
| `docs/trackers/reconnaissance-patterns.md` | 5 | 9 |
| `docs/trackers/bug-fix-session-log.md` | 5 | 18 |
| *(tail)* | 1–4 | — |

**Derivation, so it can be re-run rather than re-trusted:** for each path in
`git log --since=midnight --name-only`, count distinct `Session-Id:` trailers among the commits
touching it. Authorship is read from the trailer, never from a commit range — a range is a proxy
for authorship and stops being one the moment anyone else commits
(`docs/conventions/shared-checkout-commit-sequence.md` § 2).

The head is 3× the next entry, and **both of the top two are ledgers** — files whose purpose is
to be appended to by every work stream.

### And two thirds of that traffic is mechanical

**35 of the 53** commits to the hottest file have a diff consisting *only* of `**Members:**` /
`**Promotes to:**` / index-row / `entry_high_water` lines. Method: for each commit, count diff
lines matching `^[+-]` that are **not** one of those four shapes; 35 commits scored zero.

Nobody is contending over prose. They are contending over registry bookkeeping.

> **A caveat that belongs with the number, not in a footnote.** Two companion counts in the same
> run printed `0` because their regex (`^+\*\*Members:\*\*`) was rejected by `ugrep` — `+` after
> `^` is a quantifier with no operand. The command **errored and reported a clean zero**, in a
> measurement about mechanisms that return plausible answers rather than errors (`OB-15`). The
> 35 figure comes from the third pattern, which parsed. Anyone re-deriving should check the
> exit status, not the output.

### Isolation cost is inverted from the intuition

```
.git (shared object store)      43 M
target/  (main checkout)        92 G
target/  (one worktree)         29 G
.codescout (main)              781 M
```

**Git isolation is nearly free. Build isolation is the entire expense of a worktree**, and it is
the one thing a worktree buys that a cheaper mechanism cannot.

### The forces in tension

- Sessions are **not** the durable unit — compaction, `/clear`, resume and profile restarts all
  re-mint a session's *name*, and only the `sessionId` survives (`CLAUDE.md` § *Observer
  Blindness*). A design keyed on session names decays silently.
- The ledgers are **cross-cutting by construction**: every work stream must touch
  `issue-clusters.md` to classify a bug. That is the feature, and it is also the contention.
- Guards can only fire *after* the work they invalidate. The existing hook set already says so
  in its own header: *"It narrows the window, it does not close it — only a per-session worktree
  does that."*

## Alternatives considered

### A. Per-session worktrees — **rejected**

The remedy the hook headers gesture at. Rejected on measurement, not taste:

- **Cost:** 29 GB per session, for a git problem that is 43 MB.
- **It does not reach the head.** Ledgers are still shared, and worktrees make merging them
  *worse*: cross-tree entry-id collision
  (`docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`), shadow
  catalog rows requiring `librarian(action="merge_worktree")`, and
  `repair_frontmatter_id` rewriting inside a live worktree.
- **It trades loud failures for silent ones**, which is the decisive argument. A shared tree
  fails at *write* time — a hook refuses, a build breaks, a test reds. A worktree fails at
  *read* time: *which tree does this tool mean?* is a question that did not previously exist,
  and every tool's default answer is silent. Measured the same evening:
  `symbols(name="duplicate_definitions")` unpinned returned a **populated, plausible** result
  from the wrong tree — 7 occurrences existed in the worktree, 0 at `experiments` HEAD, and the
  answer named no scope. A zero prompts doubt; a near-miss does not.
- Four rows of the shared-resource table do not improve under worktrees at all: the
  `~/.cargo/bin` symlink still points at the main checkout's build, the MCP server and its Agent
  state are still shared, the catalog is still shared, and session identity is still ambiguous.

### B. More guards on shared resources — **rejected as the primary strategy**

This is the current de-facto trajectory, and one evening exhausted it:

- `foreign-index` accepts only a pathspec commit; `ledger-counts` accepts only a commit carrying
  a count with its member — which on an entangled index is only the bare one. **The
  intersection is empty**, and no ordering of correct steps escapes it
  (`docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`).
- `unreviewed-content`'s condition — *no unstaged content at the committed paths* — is
  **satisfied by the `git add` its own remedy prescribes**, which is what erased the difference
  it would have caught.
- Its remedy text (*"read the content; that is the whole point"*) is emitted **only on
  refusal**, so it reaches every commit except the one where it mattered. *Loudness is a
  property of a path.*

Guards remain right for the tail. They are the wrong instrument for the head.

### C. Serialize everything (one writer at a time) — **rejected**

Correct, and it discards the reason for running several sessions. Retained for `target/` only,
where the resource is genuinely singular.

## Mechanism

### Layer 1 — the spool (the only large change)

Entries are written to **per-session files** and folded into the canonical ledger by a
single-writer step:

```
docs/trackers/<ledger>/inbox/<sessionId>-<n>.md     ← written by any session, no contention
docs/trackers/<ledger>.md                           ← written only by the fold
```

Git cannot conflict on distinct files, so 16 sessions never touch one path.

**Ids are allocated at fold time.** That is the load-bearing detail and it retires four separate
open defects at once:

| retired | why |
|---|---|
| ledger entry capture | your inbox file is yours; `git add` cannot sweep a peer's entry from it |
| `append_entry`'s two-call window | no window — the section and its row are written by the fold together |
| cross-host high-water collision | no id exists until the fold, which has one writer |
| `ledger-counts` × `foreign-index` empty intersection | nothing to couple in the same commit |

It also shrinks the upstream-freshness guard's population to the fold alone, which removes the
*per-push, not per-fix* friction that currently lands on commit-early discipline.

### Layer 2 — owner fields

`.git/session-stage-log` is the existence proof: it keys every staging event to a `sessionId`
**and** a blob, and answered in one command a question three sessions had been answering from
memory. Extend the pattern to `target/` (who is building, with which feature set) and to the MCP
server's active project.

The principle is `OB-8`: *a shared resource carries no owner, so seeing the peer does not help.*
Ownership metadata is what makes sharing survivable — not another refusal.

### Layer 3 — `target/` per lane

Isolate on the **feature matrix** (`--no-default-features` vs default), which is what actually
clobbers, rather than per session. This captures the 29 GB win without creating resolution
ambiguity, because a target directory has no notion of "the project."

### Layer 4 — serialize the tail

A build lock costs seconds. A worktree costs 29 GB and a new bug class.

## Consequences

**Good**

- The head disappears without any isolation: a 16-session file becomes 16 one-session files.
- Four open defects retire from one change, none of them by adding a guard.
- Worktrees stay available for what they are actually good at — a feature branch with a
  divergent build — instead of being pressed into service as a concurrency primitive.

**Costs, stated rather than discovered later**

- The fold is a new step with an owner, and an unfolded inbox is a new way to lose work. It must
  be visible: an inbox with entries is a state a query can report, unlike an uncommitted file.
- A reader of a ledger between folds sees a stale ledger plus a spool. The ledger stops being
  the single place to look — which is the honest cost of removing the single place to write.
- Ordering within a fold is the fold's choice, so entry ids stop reflecting write order. Nothing
  currently depends on that; a `**Valid:** dated` stamp does the work an ordinal was doing.

**Explicitly unresolved**

- Whether the fold runs on a hook, on a schedule, or by hand. All three are plausible, and the
  choice is the difference between "unfolded work is invisible for minutes" and "for days."
- Whether `docs/issues/` needs the same treatment. It probably does not — bug files are already
  one-file-per-instance, which is the spool shape arrived at by accident.

## Revisit-when

- The hottest file's session-count drops below ~4 after layer 1 ships; if it does not, the
  diagnosis was wrong and the contention is in prose after all.
- A second resource enters the head — any file crossing ~8 distinct sessions in a day.
- The count of live sessions per checkout falls below ~3, at which point most of this is
  over-engineering and guards alone are proportionate.

## Confidence

**Measured, and re-runnable from the derivations above:** the contention distribution, the 35/53
mechanical share, the disk figures, the session count.

**Inferred, not measured, and the weakest steps:**

- That the spool *would* drop the head. It follows from git's conflict semantics on distinct
  paths, but no version of it has run here.
- That layer 3 captures the clobber cases. Derived from two bug files
  (`shared-target-dir-feature-clobber`, `a-peer-build-unlinks-the-test-binary`), not from an
  enumeration of every way concurrent `cargo` invocations interfere.
- The claim that worktrees leave four resource rows unimproved is a reading of the corpus, not
  an experiment; nobody has run a session in a fully-isolated worktree and counted what still
  broke.

**Falsifiable prediction, so this ADR can be wrong rather than merely unpopular:** after layer 1,
`issue-clusters.md` should fall out of the top five contended files while total ledger *writes*
stay flat or rise. If writes fall instead, the spool has added friction rather than removed it.

## Sites (initial)

- `docs/trackers/issue-clusters.md` — the head; first candidate
- `docs/trackers/bug-fix-session-log.md` — second
- `src/librarian/tools/append_entry.rs` — id allocation moves to the fold
- `scripts/pre-commit-ledger-counts.py`, `scripts/pre-commit-foreign-index.sh`,
  `scripts/pre-commit-unreviewed-content.sh` — populations shrink
- `.git/session-stage-log` — the layer-2 model to copy

## References

- `docs/conventions/shared-checkout-commit-sequence.md` — the six steps and their measurements
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — Instance 7
- `docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`
- `docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`
- `docs/issues/archive/2026-09-02-one-ledger-file-serializes-every-class-edit.md`
- `docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`
- `docs/trackers/observer-blindness.md` — `OB-8`, `OB-15`
- `CLAUDE.md` § *Reaching a Peer Session*, § *Observer Blindness*
