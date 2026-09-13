---
id: '1f3b31d9b51371c6'
kind: bug
status: open
title: 'BUG: file-provenance conflates "touched this path once" with "has bytes at risk right now"'
tags:
- cluster/addressing-without-an-escape-hatch
- provenance
- shared-checkout
topic: shared-checkout authorship
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

None. The discriminating fixtures are: (a) a refused write in the transcript, asserting the session
is NOT named; (b) a committed write with a clean worktree at that path, asserting the same. Both are
absence assertions, monotone under the scanner being disabled entirely — so each needs a positive
twin (a real uncommitted peer write that MUST be named) in the same test.

## Workarounds

When `fmt-mine.sh` or a provenance run returns SHARED and the named session is not live, check
`git log -1 --format=%H -- <path>` and `git diff --stat -- <path>` before concluding anything. A
named session whose bytes are committed is not a co-owner of your working tree.

## Resume

Decide whether instance 2 is the same class or its own. If its own, the candidate claim is *"a
window over history, used to answer a question about the present, cannot subtract what has since
been absorbed"* — which would also cover the mtime-vs-authorial-write confusion seen the same day.

## References

- Instance 2 measured and reported by sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`, who also
  supplied the framing that the two causes share a consequence at the point of use.
- `scripts/fmt-mine.sh` — the consumer that turns a false positive into a refusal
- CLAUDE.md § *Reaching a Peer Session* — the intersect-with-socket-enumeration procedure
- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
