---
id: '736c88123dd2b852'
kind: bug
status: open
title: 'BUG: fmt-mine.sh reformats a live peer''s uncommitted hunk after a hunk-split commit of the same file'
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-26
owner: marius
related:
- d567a429109f6fd8
severity: high
---

# BUG: `fmt-mine.sh` reformats a live peer's uncommitted hunk after a hunk-split commit of the same file

## Summary

`scripts/fmt-mine.sh` formats the files `scripts/file-provenance.py` attributes to the current session and refuses the rest (CLAUDE.md § *Development Commands*, step 1). The provenance window still starts at the path's **last commit time**, which is the defect in `docs/issues/archive/2026-09-13-file-provenance-reads-a-commit-time-as-proof-the-writes-are-in-head.md` (`d567a429109f6fd8`). That fix shipped item 2, a printed "N write(s) also exist but predate the window" caveat, and deferred items 1 and 3 *"for a follow-up bug if the visibility fix here turns out not to be enough in practice."*

**It was not enough in practice.** A hunk-split commit of a shared file moves that file's last-commit time past a live peer's still-uncommitted writes. The peer's writes then fall outside the window, the verdict reads `MINE`, and `fmt-mine.sh`, which acts on the verdict and cannot read the caveat, reformatted the peer's uncommitted Rust. This is the incident `fmt-mine.sh` exists to prevent (`docs/issues/archive/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md`).

## Symptom (Effect)

2026-09-26, in this checkout:
- Peer session `b4de6398-fed1-4c1d-a359-2b9a42554e10` held an uncommitted, unformatted edit in `src/server.rs`: 8 hunks at about 3044–3147, inside `tool_descriptions_name_every_action_they_claim_to_enumerate`.
- Session `3c5b02df-b6ce-45f5-9d03-1194e38465c0` committed only its own disjoint hunks of that file through a temp-index split (`18e15470`, the procedure in `docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*).
- On the next gate run, by that session's Task 2 implementer subagent, `fmt-mine.sh` reformatted the peer's hunk.
- The implementer noticed, restored the peer's pre-gate bytes by hand, and reported it.

The controller verified the restoration: the peer hunk's 167 diff lines are byte-identical (`cmp`) to a snapshot taken before `18e15470`.

**Nothing refused and nothing warned in a form a script reads.** Only a reader who noticed a formatting diff in a region they did not write caught it.

## Reproduction

1. Session P edits file F and leaves it uncommitted.
2. Session S edits a disjoint region of F and commits only its own hunks (index split). F's last-commit time is now later than every one of P's writes.
3. S writes F again (or any session runs the gate), then runs `./scripts/fmt-mine.sh`.
4. `file-provenance.py F` scans only writes after F's last commit. It sees S's writes only and returns `MINE`, with the item-2 caveat printed. `fmt-mine.sh` formats F whole, including P's hunk.

The observed intermediate state: right after step 2 and before step 3, `file-provenance.py src/server.rs` returned `UNKNOWN … the worktree is DIRTY for this path and every write on record predates the window … (657 write(s) also exist but predate the window)`. After step 3's writes, the in-window writes were all this session's, so the verdict became `MINE`.

## Environment

codescout `experiments` at `2884ae0d` / `705bca92`; `scripts/file-provenance.py` and `scripts/fmt-mine.sh` as of those commits; several live sessions sharing the checkout.

## Root cause

`d567a429109f6fd8`'s item 1: the window is derived from a commit time, on the premise that "writes older than that are baked into HEAD". A pathspec or hunk-split commit of F by S does not contain P's writes to F, so the premise is false for exactly the case this checkout's commit discipline produces. Item 2 made the exclusion visible to a human reading the verdict. `fmt-mine.sh` consumes the verdict, not the caveat, so for the script the exclusion is still invisible.

## Evidence

- The peer hunk's integrity after restoration: 167-line `cmp` against the pre-`18e15470` snapshot, clean.
- The provenance output quoted in *Reproduction*, from before step 3's writes.
- The Task 2 implementer's report, an SDD workspace file that is git-ignored and deleted at plan end. Its account is restated here: gate run 1's `fmt-mine.sh` read the file as MINE, formatted it, the implementer restored the region, and gate run 2 returned `FMT=0`.

## Hypotheses tried

N/A. The mechanism is the one `d567a429109f6fd8` already diagnosed, reached by a new trigger: a hunk-split commit.

## Fix

Not fixed. Candidates, from `d567a429109f6fd8`'s deferred items:
- **Item 3, the narrowest:** downgrade `MINE` to `SHARED` whenever the path is dirty and any other session has a write outside the window. `fmt-mine.sh` then refuses, which is its existing and correct behaviour for `SHARED`.
- **Item 1, the honest default:** an unbounded window by default, keeping `--since` for callers who want one.

Either closes this trigger. Item 3 is enough for `fmt-mine.sh`'s safety.

## Tests added

None yet. The regression to write: in `tests/file-provenance.sh`, a peer write before a later commit of the same dirty path gives `SHARED`, not `MINE`. Then a `fmt-mine.sh` case showing it refuses that path.

## Workarounds

After committing part of a shared file, do not run `./scripts/gate.sh` or `fmt-mine.sh` while that file is dirty with a peer's hunk. Or run `file-provenance.py --all <path>` first and treat any peer write it lists as `SHARED`.

## Resume

Implement item 3 in `scripts/file-provenance.py`, then add the two tests above.

## References

- `docs/issues/archive/2026-09-13-file-provenance-reads-a-commit-time-as-proof-the-writes-are-in-head.md`: the deferred items.
- `docs/conventions/shared-checkout-commit-sequence.md` § *The entangled single file*: the procedure that triggers it.
- `docs/issues/archive/2026-09-09-the-documented-gates-first-command-rewrites-every-peers-uncommitted-rust.md`: the incident `fmt-mine.sh` exists to prevent.
