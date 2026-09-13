---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: high
---

# BUG: `file-provenance.py` treats a path's last commit time as proof those writes are in HEAD

## Summary

`scripts/file-provenance.py` defaults its scan window to the target path's **last commit
time** (`last_commit_time`, `git log -1 --format=%cI -- <path>`), discarding older writes
on the reasoning quoted in its own docstring: *"Writes older than that are baked into
HEAD."*

On a shared checkout that inference is false. A pathspec commit of path `P` by session A
does **not** include session B's unstaged edits to `P`. B's writes are then silently
dropped as baked-in while they are still live in the working tree — and the tool reports
`MINE` to whoever wrote most recently.

## Symptom (Effect)

`MINE` is the verdict a reader acts on: it means "nobody else's work is here, committing
this path is safe." Produced this way it licenses exactly the capture documented in
`docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

Two things make it expensive rather than untidy:

- The `[cs-hint]` on `edit_file` routes readers to this script **by name** for this
  question — *"To find out whose: ./scripts/file-provenance.py <path>"* — so the
  misleading verdict sits at the end of the documented path.
- `UNKNOWN` warns about the window and counts what it hid
  (*"(N write(s) exist but predate the window; re-run with --all to see them)"*).
  `MINE` prints the window line and **no** count. The caveat is attached to the harmless
  verdict and omitted from the one that authorises a commit. Measured:
  `grep -c "predate the window"` returns 1 on the `UNKNOWN` output and 0 on `MINE`.

## Reproduction

Observed 2026-09-13 on `src/librarian/catalog/augmentation.rs`, carrying an uncommitted
32-insert/32-delete `RecoverableError` → `LibrarianRecoverableError` rename with zero
lines of mine in it:

```
$ ./scripts/file-provenance.py src/librarian/catalog/augmentation.rs
MINE      src/librarian/catalog/augmentation.rs
          window: writes at or after 2026-09-12T17:00:35+00:00
          written by THIS session (8bd791df)
```

The window is the path's last commit, `408709ea` at `2026-09-12T20:00:35+03:00`. The
decisive check is what that commit contains:

```
$ git show --format='' 408709ea -- src/librarian/catalog/augmentation.rs \
    | grep -c LibrarianRecoverableError
0
```

**Zero.** The cutoff commit does not contain the rename, so the writes it is being used to
excuse were never baked into HEAD. Any write to that path made before 20:00:35 — by
anyone, committed or not — is invisible to the default invocation.

Control, same tool and run, on a path with no in-window writes:

```
$ ./scripts/file-provenance.py src/librarian/server.rs
UNKNOWN   src/librarian/server.rs
          no record of any session writing this path in the window …
          (2 write(s) exist but predate the window; re-run with --all to see them)
```

## Environment

Tree at `34a0beb1`+ on `experiments`; `scripts/file-provenance.py`, default (no `--all`);
34 files under `src/` carrying the uncommitted rename, `LibrarianRecoverableError` present
nowhere at HEAD.

## Root cause

`last_commit_time` (`scripts/file-provenance.py:512-517`) returns the path's last commit
timestamp and `main` uses it as the default `since`. The docstring states the assumption
outright — *"Writes older than that are baked into HEAD"* — which holds for a solo
checkout and fails for a shared one, because a commit's **time** says nothing about whose
working-tree state it **captured**.

This is the proxy `docs/conventions/shared-checkout-commit-sequence.md` § 2 already warns
against, applied inside the tool built to answer that question: *"Identify your own work
POSITIVELY … never by a commit range. A range is a proxy for authorship and stops being
one the moment anyone else commits."* The tool derives its window from a commit.

## Evidence

The four commands above, re-derivable in about a minute. The `grep -c` on the cutoff
commit is the load-bearing one — it converts "the window might be wrong" into "the window's
stated justification is absent from the commit it rests on".

## Hypotheses tried

- **"MCP-tool writes are a second blind spot"** — **rejected, and recording it because it
  is the plausible wrong fix.** Raised by a peer who observed that `fmt-mine.sh`'s
  provenance check recognised none of their 33 rename-touched files, all written via
  codescout's `edit_code`. But `CS_WRITE_TOOLS` (`scripts/file-provenance.py:72-80`)
  contains `mcp__codescout__edit_code`, `edit_file`, `edit_markdown` and four legacy
  aliases. Tool-name recognition is not the gap; the commit-time cutoff explains their
  observation without it. Chasing the MCP hypothesis would have added recognition for
  tools already recognised and left the window untouched.
- **"`MINE` is correct within its stated contract"** — *partly accepted, does not dispose
  of this.* The window line IS printed. But `UNKNOWN` shows the author already judged a
  bare windowed verdict misleading and supplied the remedy — for the branch where acting
  on it is harmless.

**Attribution NOT established, deliberately.** An earlier draft of this file named a
specific session as the rename's author. That was inferred from "the only LIVE writer in
the `--all` list", which is elimination over a population this very tool is unreliable
about — the error `CLAUDE.md` § *Observer Blindness* names ("never close an authorship
question by elimination"). The author is whoever says so: the session that claimed it
verified its own 33-file diff independently. This record does not need the name and does
not assert one.

## Fix

Not implemented. Three parts, in order of value:

1. **Stop deriving the window from a commit.** Either default to unbounded, or keep the
   cutoff only when the commit demonstrably contains the write (which requires content,
   not time — likely not worth it). Unbounded-by-default is the honest option; `--since`
   remains for callers who want a window.
2. **Print the hidden-write count on every verdict**, most loudly on `MINE` and `SHARED`.
3. **Downgrade `MINE` to `SHARED`** whenever any other session has a discarded write on
   that path.

## Tests added

None. The discriminating test seeds a write by another session before the path's last
commit, with that commit not containing the write, and asserts the default invocation does
not return `MINE`. It reds today.

## Workarounds

**Run `--all` before any pathspec commit on a file you did not create.** The default form
answers "who wrote this since the last commit", which is not the question a commit poses.
`fmt-mine.sh` consumes the same provenance and inherits the same blind spot.

## Resume

Found while starting BL-29 step 1. `edit_file`'s hook warned that `augmentation.rs` held
foreign uncommitted changes; the tool the hook names then said `MINE`; only `--all`
revealed a live peer mid-refactor across 34 files. **The hook was right and the tool it
routes to contradicted it, on the same file in the same minute** — which is the pairing
worth keeping, because a reader who trusts the named tool over the vaguer hook gets the
wrong answer by following instructions.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` — the
  capture this verdict enables.
- `docs/conventions/shared-checkout-commit-sequence.md` § 2 — "never by a commit range".
- `CLAUDE.md` § *Observer Blindness* — a windowed instrument's zero is scoped to its
  window.
