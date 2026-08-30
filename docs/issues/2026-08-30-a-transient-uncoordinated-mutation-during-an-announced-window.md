---
id: '28d61d3b0dca0932'
kind: bug
status: open
title: 'BUG: the working tree briefly held a mutation nobody applied, during the window announcing that exact mutation'
tags:
- shared-checkout
- cross-session
- unexplained
- protocol
- mutation-testing
opened: 2026-08-30
owner: marius
severity: medium
unverified: Cause NOT established. Three peers deny with evidence, no worktree holds the content, and the read is not reproducible. Filed for the facts and the protocol finding, not for a diagnosis. Do not cite this as a peer action.
---

# BUG: the working tree briefly held a mutation nobody applied, during the window announcing that exact mutation

> **Cause not established.** This file records timestamped observations, the
> hypotheses ruled out, and one protocol finding that stands regardless of cause.
> It deliberately does **not** name an author. Today already produced four
> misattributions in this checkout, three of them from reasoning about peers over
> a set `ListAgents` reports incompletely.

## Summary

While fixing `dense_and_sparse_legs_run_concurrently`, I broadcast a heads-up to
three peer sessions saying I was about to replace `tokio::try_join!` in
`embed_one_batch` with two sequential `.await`s, and to ignore the resulting red.

Roughly 60 seconds later — and **before I had made any edit to that region** —
`src/retrieval/embedder.rs` on disk contained exactly that mutation. It reverted
within ~15 seconds. No session claims it.

## Timeline (all 2026-08-30, local)

| time | event |
|---|---|
| ~16:54–16:55 | Window announced to `codescout-ae`, `codescout-36`, `git-travel-augmentation-shape`. Announcement names the edit verbatim. |
| ~16:56:0x | `read_file(src/retrieval/embedder.rs, 686–700, force=true)` returns a **sequential-await** body: `}` / `.await?;` / `let sparse_nonempty = async {`. |
| same batch | `grep('^\s*\)\?;$', same file)` returns **0 matches** — independently consistent with the `try_join!` being absent. |
| **16:56:15** | File mtime. Something wrote. |
| 16:56:21 | `git status` / `git diff --stat`: one hunk, inside `mod tests` — my own test rewrite. `embed_one_batch` **unmodified vs HEAD**. |
| 16:57:11 | sha256 captured; file stable thereafter. |
| ~17:0x | I apply my own mutation, verify red, restore byte-exact (sha256 matches). |

My only edits before 16:56 were two `edit_code` calls, both targeting
`tests/dense_and_sparse_legs_run_concurrently` and the helper above it — lines
1909+. `embed_one_batch` is at 658–815. The `git diff` at 16:56:21 confirms it.

## What was ruled out

**Worktrees — refuted.** All three linked worktrees checked directly:

| worktree | `tokio::try_join` | `let sparse_nonempty = async` |
|---|---|---|
| `.claude/worktrees/peer-delegation` | 2 | **0** |
| `.worktrees/vdi-windows` | 2 | **0** |
| `.claude/worktrees/operator-rules-phase-2` | 4 | **0** |

No worktree has ever held the sequential form, so a path-resolution slip into
another tree cannot explain it.

**Peer action — denied by all three, with evidence rather than recall.**

- `codescout-ae`: all ten of today's commits return 0 for `grep -c
  'retrieval/embedder'` on their `--stat`; the single source commit touches only
  `src/librarian/tools/doctor.rs`. Their own mutation window that day was in
  `doctor.rs`.
- `codescout-36`: no edits, no shell calls, no subagents this session.
- `git-travel-augmentation-shape`: one write this session, into
  `src/librarian/augmentation_sidecar.rs`, and it landed *after* my window-closed
  message. No build, no script, no subagent.

Denials do not close it — `ListAgents` under-reports (see
`docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md`,
which measures the population at **six or more** against views reporting two or
three). A session outside every view could neither be asked nor know to answer.

**Not reproducible.** The identical call — same path, same range, same
`force=true` — returned the correct content minutes later and has ever since.

## The two live hypotheses

**H1 — the disk really was different.** Supported by two *independent* tools
agreeing in the same call batch: `read_file` rendered the sequential body and
`grep` independently found no `)?;`. Also by the 16:56:15 mtime, which is a real
write. Under H1 the revert was surgical: my `mod tests` hunk survived intact, so
whatever restored `embed_one_batch` touched only that region — the signature of a
reverse edit, not of `git checkout -- <file>`, which would have destroyed my work.

**H2 — a tool-layer cache served phantom content**, and the 16:56:15 write has a
mundane unrelated cause. Weakened by the two-tool agreement *unless* both share a
caching layer — which is the open question below, and the one worth answering,
because it decides whether `read_file` can silently serve bytes that are not on
disk. That would be the more serious of the two findings by a distance.

`git-travel-augmentation-shape` raised a third framing worth keeping: a
contain-then-revert with a terminal write is also the signature of a formatter or
editor round-trip. No editor is attached to this checkout and no formatter ran in
that window, but the shape is right and it is the correct thing to disconfirm
first — a peer is the more interesting hypothesis and therefore the one to attack
hardest.

## The protocol finding — independent of cause

Raised by `codescout-ae`, and it stands whether or not a peer acted here:

> A broadcast saying *"I am about to replace `try_join!` with two sequential
> awaits in `embed_one_batch`"* contains, verbatim, an executable instruction. It
> is a heads-up in the sender's intent and an imperative in its grammar, and the
> two are indistinguishable to a reader arriving without context.

This **inverts** `reconnaissance-patterns:R-129`'s first clause. That clause
exists to reduce harm from deliberate breaks; if announcing one can cause a peer
to apply it, the clause acquires a failure mode *proportional to how well it is
followed* — and the more precise the announcement, which is exactly what makes it
useful, the more directly actionable it becomes.

A session that acted on such an announcement would have had no way to tell the
announcer and no reason to think it needed to. The ~15-second lifetime fits
someone realising and reverting.

## Fix

**Put the mitigation in the announcement's form, not in a rule nobody can
enforce.** Say *what* you are about to do, never *how*:

- ✗ "replacing `try_join!` with two sequential `.await`s in `embed_one_batch`"
- ✓ "breaking the concurrency guard in `embed_one_batch` for ~2 min"

Same warning value, nothing to execute. Cheap, and it removes the imperative
entirely. Proposed for promotion into `R-129` as a second clause.

The open technical question — **can `read_file` and `grep` share a cache that
serves content absent from disk?** — needs a separate investigation. Until it is
answered, H2 is not dismissible, and neither is H1.

## What this does NOT affect

The concurrency fix itself (`614b1271`, patch-id
`bf12a8cc52e518da7edad0887e7e96f41bf3f38f`). Its mutation run and its restore are
both sha256-anchored, and the restore was verified byte-identical against a hash
captured before the mutation.

## References

- `docs/issues/archive/2026-08-30-concurrency-timing-test-flakes-as-its-own-regression-signature.md` — the fix whose window this happened in.
- `docs/issues/2026-08-30-listagents-omits-cross-profile-sessions-in-the-same-checkout.md` — why the denials cannot close it.
- `docs/trackers/reconnaissance-patterns.md` — `R-129`, the clause this inverts.

