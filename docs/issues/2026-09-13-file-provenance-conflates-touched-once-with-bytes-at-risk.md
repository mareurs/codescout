---
id: '1f3b31d9b51371c6'
kind: bug
status: fixed
title: 'BUG: file-provenance conflates "touched this path once" with "has bytes at risk right now"'
tags:
- cluster/addressing-without-an-escape-hatch
- provenance
- shared-checkout
topic: shared-checkout authorship
closed: 2026-10-06
unverified: STANDING — the fix sets a peer aside only inside the commit's own second, so an uncommitted edit that merely moves a block already in HEAD reads as set aside in that second. The original incident could not be replayed; the filed shape was rebuilt in a throwaway repo.
---

# BUG: file-provenance conflates "touched this path once" with "has bytes at risk right now"

## Summary

`scripts/file-provenance.py` reports a path as `SHARED` on evidence that does not mean shared
ownership of *uncommitted* state. Two distinct causes, measured the same day by two sessions:

1. **A REFUSED write counts as a write.** A tool call naming the path as a write target is counted
   whether or not the write landed.
2. **A COMMITTED historical write counts as uncommitted co-ownership.** A session whose contribution
   is already in git is named as a live co-writer.

Both produce the same wrong answer at the point of use, and the point of use is not cosmetic:
`scripts/fmt-mine.sh` consults this tool before refusing to format, and CLAUDE.md's authorship
procedure instructs readers to intersect it with the socket enumeration.

## Symptom (Effect)

**Instance 1 — refused write** (sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb`, 2026-09-13).
`scripts/architecture-boundary-probe.py` reported:

```
SHARED    scripts/architecture-boundary-probe.py
          written by THIS session (9403d62d)
          written by aa272bed-...  [LIVE]
```

This session never wrote that file. Its only write-shaped call was an `edit_file` that was
**refused** — `old_string not found`, because a peer's fix had already landed. Nothing was written,
and the scanner counted it.

**Instance 2 — committed historical write** (sessionId
`aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`, 2026-09-13). `fmt-mine.sh` refused
`src/tools/session_key.rs` as SHARED, naming session `55515bc5` as co-writer — **not live, so
unaskable**. Git settled it in one command: 99 insertions, 0 deletions, all `aa272bed`'s.
`55515bc5`'s contribution is committed at `bc933c01`, dated exactly the window timestamp the scanner
printed. The scanner was reporting committed history as bytes-at-risk.

## Reproduction

Instance 1:

1. Attempt an `edit_file` on a tracked file with an `old_string` that does not match. Observe the
   refusal.
2. `python3 scripts/file-provenance.py <that path>` — your session is named as a writer.

Instance 2: find a file whose history contains a commit by an exited session, with no uncommitted
bytes from that session, and run the tool on it.

## Environment

codescout `experiments` @ `099cab38`. `scripts/file-provenance.py`, consumed by
`scripts/fmt-mine.sh`.

## Root cause

The tool's own docstring states the intended discriminator correctly:

> Only a tool call whose INPUT names the path as a write TARGET counts.

That predicate is satisfied by a call that was refused, and by a call whose result has since been
committed. The question the consumers actually ask is narrower: *does another party have
**uncommitted bytes** in this file **right now**?* The tool answers *has any session ever named this
path as a write target inside the window?*

Instance 1 is `cluster/addressing-without-an-escape-hatch`: a request and an accomplished act are
the same token in a transcript, with no way to mark the difference. Instance 2 is arguably a
distinct class — a window that does not subtract what git already absorbed — and is filed here
rather than separately because the consumers cannot tell them apart and neither can the reader.

## Evidence

The tool is carefully right about the direction it *does* guard, which is what makes this expensive:

> And `UNKNOWN` is never rendered as "not mine". A Bash write this tool's heuristics miss is
> indistinguishable from no write at all, so absence is a statement about coverage, not about
> ownership.

That is a false-negative guarantee, stated prominently. The false **positive** direction is
unguarded and undocumented, and it is the one wired into a refusal: `fmt-mine.sh` has no `--force`
by design, so a false SHARED is a hard stop whose remedy text sends the reader to ask a party who —
in instance 2 — had already committed and, being exited, could not have answered anyway.

## Hypotheses tried

1. **Instance 1 is a peer mis-attributing.** **Refuted** — both parties compared notes; the peer
   confirmed the fix was theirs and that this session wrote nothing.
2. **Instance 2 is a genuine shared edit.** **Refuted positively, not by elimination:**
   `git diff --stat` on the working tree showed 99 insertions / 0 deletions attributable to the
   asker, and the named session's bytes resolve to commit `bc933c01`.

## Fix

Not implemented. Sketch:

- **Refused calls:** a transcript records the tool RESULT next to the call. Counting only calls
  whose result is not an error removes instance 1 without narrowing real coverage.
- **Committed writes:** intersect the candidate set with paths git reports as actually dirty. A
  session's write to a path with no uncommitted delta has nothing at risk by definition. This also
  makes the `SHARED` verdict mean what its consumers assume.

Both narrow the FALSE-POSITIVE direction only, so the documented `UNKNOWN`-is-not-absence guarantee
is untouched.

## Tests added

Superseded 2026-10-06 by the partial fix below (commit `6f6fdc96`; tests listed under `## Partial fix (2026-10-06)`). The original text read "None." The fixtures named next are the ones the commit implements, each negative with a landed-write positive twin. The discriminating fixtures are: (a) a refused write in the transcript, asserting the session
is NOT named; (b) a committed write with a clean worktree at that path, asserting the same. Both are
absence assertions, monotone under the scanner being disabled entirely — so each needs a positive
twin (a real uncommitted peer write that MUST be named) in the same test.

## Workarounds

When `fmt-mine.sh` or a provenance run returns SHARED and the named session is not live, check
`git log -1 --format=%H -- <path>` and `git diff --stat -- <path>` before concluding anything. A
named session whose bytes are committed is not a co-owner of your working tree.

## Partial fix (2026-10-06)

- **SHA:** `6f6fdc96` (`experiments`)
- **patch-id:** `5ff64bb6c5af618b40e2c52843bf4de467783ef5`

What is covered. **Instance 1 (refused write):** `scan()` in `scripts/file-provenance.py` now pairs each `tool_use` with its `tool_result` by `tool_use_id` and drops the write targets of all-or-nothing calls (edits, creates, doc writes) whose result says they were refused. A codescout refusal is `{"ok": false}` text with `is_error` absent (the `RecoverableError` mapping), so `is_error` alone would not have fixed the case this bug was filed on; `is_error: true` (native tools) and `pending_ack` also count as refused. A call with no result on record, and every Bash / `run_command` call whatever its exit status, still counts (a failed shell command may have written). **Instance 2, the CLEAN-path half:** `main()` now prints a new first-column verdict token `CLEAN` where `SHARED` / `PEER` would have printed for a path git reports clean. `MINE`, `UNKNOWN` and untracked paths are unchanged. `scripts/fmt-mine.sh` now formats `CLEAN` rows. Behaviour change: a clean file needing rustfmt that a peer once wrote used to be a hard refusal and is now formatted.

Tests added: `tests/file-provenance.sh` grows from 197 to 223 assertions (every negative paired with a landed-write twin; the section covers a codescout refusal, a native `is_error` refusal, a `pending_ack` park, a failed shell command that must still count, and CLEAN versus MINE / SHARED / PEER), and `tests/fmt-mine.sh` gains case 12 (CLEAN is formatted, not reported as a refusal). The assertion totals are quoted from the commit brief; this bookkeeping pass did not re-run the suites.

What is NOT covered:

- **Instance 2 as filed was not fixed when this was written; it is now (see `## Second fix`).** A DIRTY path where a peer's bytes were already committed still prints `SHARED`, because git dirtiness is per path and cannot separate the asker's uncommitted bytes from a peer's committed ones. `src/tools/session_key.rs` in the filed case had 99 uncommitted insertions from `aa272bed`, so it is dirty and would still print `SHARED` naming `55515bc5`.
- **Suspect, since CONFIRMED as the cause (see `## Second fix`):** `last_commit_time` (`scripts/file-provenance.py:905-910`) reads `git log -1 --format=%cI`, which has second resolution, so a write in the same second as the commit may count as in-window. Read at the bytes; the in-window effect was not reproduced.
- A refused `workspace(activate)` is still treated as moving the active tree (pre-existing).
- A harness `is_error` on a codescout write is treated as nothing-written; a timeout where the write landed would be missed (theoretical, not observed).
- The sibling bug `2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer` was not touched.

## Second fix (2026-10-06)

- **SHA:** `4d49908a` (`experiments`)
- **patch-id:** `84231299b0a78b797a8fece7987660cdf9112ca7`

Instance 2 as filed, fixed. The window floor is the last commit's time, which git reports to the second (`%cI`). A write that the commit took can fall inside that commit's own second, and a dirty path cannot discount it, because the CLEAN verdict does not apply. The filed case had this shape: the peer's `create_file` landed 0.4 s into the second of the commit that took it, and the asker then added 99 lines. The original transcript is gone, so the fork that fixed it rebuilt the shape in a throwaway repo. Before the fix it printed `SHARED` naming the peer. After the fix it prints `MINE` with a note.

`scripts/file-provenance.py` now sets a peer aside only when all of these hold: its write is dated inside `[floor, floor + 1 s)`; its text is known and at least 24 characters; every piece of that text is in `HEAD`; and the worktree holds no more copies of the text than `HEAD` does. Shell and `doc()` writes, short strings, writes outside that second, `--since` and `--all` runs and the asker's own writes are always counted. A set-aside session appears in a note, not as a co-writer, and a path with no other writer on record prints `UNKNOWN` with a cause line. `scan()` keeps its signature. A new `scan_with_evidence()` also returns the written text.

Tests: `tests/file-provenance.sh` grows from 223 to 244 assertions (re-run for this record: `passed=244 failed=0`). The seven new negatives each have a twin that must still name the writer. Without the fix exactly those seven fail (reported by the fork). Five targeted mutations each turned its twin red: no copy-count check, no upper bound on the second, not excluding the asker, no minimum length, and letting `--since` absorb. A sixth, `--all` absorbing, is equivalent because the floor is `None` there (reported by the fork). The full gate ran green on the integrated tree on 2026-10-06.

Accepted residue: an uncommitted edit that only moves a block already in `HEAD` leaves the copy count unchanged, so its author would read as set aside, and only when the move lands in the commit's own second. The original incident could not be replayed.

## Resume

Instance 1, instance 2's CLEAN-path half and instance 2 as filed are all fixed on `experiments` (local, not pushed). Status flipped to `fixed` on 2026-10-06 with a STANDING caveat. The notes below are the earlier plan, kept for the record:

1. RESOLVED 2026-10-06 (`## Second fix`): instance 2 as filed was the commit's own second, not a class of its own, and it is fixed. The text that follows is the original reasoning. The candidate claim is still *"a window over history, used to answer a question about the present, cannot subtract what has since been absorbed"*, which would also cover the mtime-versus-authorial-write confusion seen the same day. Per-path git dirtiness cannot separate those bytes; a fix would need per-hunk attribution (for example `git blame` or a diff against the peer's commit).
2. DONE 2026-10-06: it does, and that was the cause (`## Second fix`).
3. Keep this file's relation to the archived sibling (`2026-09-13-file-provenance-reads-a-commit-time-as-proof-the-writes-are-in-head`) and to the unfixed `2026-09-19-file-provenance-answers-at-session-grain-...` in mind: a change to the window that narrows false positives can widen the false-negative direction.

## References

- **`docs/issues/archive/2026-09-13-file-provenance-reads-a-commit-time-as-proof-the-writes-are-in-head.md`
  — the SAME window, failing in the opposite direction, filed the same day and missed on the first
  pass of this file.** There the scan window defaults to the target path's last commit time and
  discards older writes as *"baked into HEAD"* — false on a shared checkout, because a pathspec
  commit of `P` by session A does not include session B's unstaged edits to `P`. So a live co-owner
  is silently dropped and the tool reports `MINE` to whoever wrote most recently. **That is a false
  NEGATIVE; this file is the false POSITIVE.** The two are complementary rather than duplicate — one
  window, over-reporting at one end and under-reporting at the other — and a fix to either that does
  not read the other risks trading one for the other. Found by running
  `git grep -il 'file-provenance' -- 'docs/issues/archive/*.md'`, which takes one command and was not
  run before this file was opened.
- Instance 2 measured and reported by sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`, who also
  supplied the framing that the two causes share a consequence at the point of use.
- `scripts/fmt-mine.sh` — the consumer that turns a false positive into a refusal
- CLAUDE.md § *Reaching a Peer Session* — the intersect-with-socket-enumeration procedure
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
