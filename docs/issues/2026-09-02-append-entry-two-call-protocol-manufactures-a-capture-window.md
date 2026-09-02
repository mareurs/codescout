---
id: c4a3d1eb1be7654b
kind: bug
status: open
title: 'BUG: append_entry''s two-call protocol guarantees an interval where a ledger entry exists on disk without its index row'
tags:
- cluster/shared-resource-carries-no-owner
- librarian
- trackers
- shared-checkout
- multi-session
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
severity: medium
unverified: Observed on ONE ledger. That every table-keeping ledger has the same window is reasoned from the guide's text, not measured on a second instance — see Resume for the population count that would establish it.
---

## Summary

Appending one entry to a ledger that keeps an index table takes **two** calls: `append_entry` writes the `## PREFIX-N — title` section straight to disk, and the index row must be a **separate, later** call. Between them the file on disk holds a complete, unstaged, index-less entry, and **shortening that interval means violating the id-allocation rule** — so no discipline available to the appending session closes it.

> **CORRECTION, before this file was first committed.** An earlier draft called this interval "a capture window" and presented `5c353d8f` as its consequence. **That causal claim is false, and the capturing session settled it positively.** `f13f8169` reports: `git add` of three explicit paths, then a pathspec commit of the same three — no `-A`, no `-u`, no `--no-verify` — and `docs/trackers/bug-fix-session-log.md` was **a file they were legitimately editing that turn too** (appending a sibling datapoint to `F-102`). `git add <path>` stages the **whole file**, so a peer's edits go with it *whether or not* the peer is mid-protocol. A complete, finished, single-call edit would have been captured identically.
>
> So the two-call gap **widens the exposure and did not cause this capture**, and the remedies do not overlap: closing the gap would not have prevented `5c353d8f`. This file stays open on the narrower claim below, which is worth fixing on its own terms — an entry existing without its index row is an incomplete artifact for as long as the interval lasts, and a capture during it splits one logical entry across two commits and two authors, which is strictly worse than capturing a whole edit.

## Symptom (Effect)

Observed 2026-09-02. Session `ffb95976` called `append_entry` on `docs/trackers/bug-fix-session-log.md`, which wrote `## W-99` plus the frontmatter bump. Before the index-row call landed, peer commit `5c353d8f` (session `f13f8169`) took the entry — *coincident with the window, not caused by it; see the correction in § Summary*:

```
$ git show 5c353d8f --stat
 docs/trackers/bug-fix-session-log.md  | 36 ++++++++++-
```

35 insertions, 1 deletion (`entry_high_water_W: 98` → `99`) — all of it another session's work. That commit added no entry of its own to that file.

The captured session's own view is the misleading part:

```
$ git diff --stat -- docs/trackers/bug-fix-session-log.md
 docs/trackers/bug-fix-session-log.md | 1 +
 1 file changed, 1 insertion(+)
```

~36 lines had just been written; `git diff` reports 1.

## Reproduction

`git rev-parse HEAD` at observation: `2e40fba9`, branch `experiments`.

1. Session A: `artifact(action="append_entry", id=<ledger>, id_prefix="W", anchor_heading=…, title=…, body=…)`. The section is now on disk, unstaged.
2. Session B: stage and commit that path (any route that reads the working tree — `git add` then commit, or a pathspec commit under `--no-verify`).
3. Session A: write the index row.

Result: the entry's section is committed under B's Session-Id and message; the row under A's. The entry is split across two commits and two authors.

The window is not incidental to timing — step 1 and step 3 **must** be separate calls (see Root cause), so any B-commit overlapping the interval reproduces it.

## Environment

Linux, codescout MCP on `experiments`, shared checkout `/home/marius/work/claude/codescout` with 6 concurrent sessions in-tree (21 live across 3 profiles). Ledger: `docs/trackers/bug-fix-session-log.md`, `entry_prefix: [F, W]`, guarded (so `edit_markdown` is refused and `append_entry` is the only write path for the section).

## Root cause

**The protocol mandates the split, and the mandate is load-bearing.**

`get_guide("tracker-conventions")` § *Entry ids*: *"Write the index row after, never before. The allocator counts an id already claimed by an index row, so a row written ahead of its section consumes the number it names — which is why codescout's own `statement-validity-session-log` starts at `F-2`/`W-3`."*

So writing the row first is forbidden for a real, measured reason. `append_entry`'s parameters (`anchor_heading`, `title`, `body`, `entry`, `entry_collection`, `id_prefix`, `cites`) include **no** index-row argument, so writing both in one call is not expressible. The two writes are logically atomic and mechanically cannot be.

*Measured 2026-09-02: the capture above. Inferred-not-measured: that every ledger keeping an index table has the same window — this was observed on one ledger, and the reasoning generalises from the guide's text rather than from a second observation.*

**A tension in the guide is relevant to the fix.** The same guide states *"The headings are the index"* and *"Keeping a rendered row table in addition to headings is fine — just write it knowing it is hand-maintained and that the heading, not the row, is what makes the entry reachable."* By that reading the index table is **optional**, while the append recipe instructs writing the row. Both are current text.

## Evidence

### The capture, verified positively

```
$ git show 5c353d8f -- docs/trackers/bug-fix-session-log.md | grep -c '^+## W-99'
1
```

Attribution by the recorded `Session-Id` trailer, not by adjacency or a self-reported name.

### A second session was captured by the same commit, in the other state

`codescout-dd` (sessionId `c45dd5ef`) had `docs/trackers/issue-clusters.md` **staged** (`git status --porcelain` showed `MM`, confirmed one tool call before the loss) and lost it to the same commit. That pair — one staged, one unstaged, one commit — is what eliminates the bare-index-commit route; see `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 7*.

## Hypotheses tried

1. **Hypothesis:** the appending session was careless — it should have staged immediately.
   **Test:** read the allocation rule in `get_guide("tracker-conventions")` § *Entry ids*.
   **Verdict:** rejected. Staging immediately is possible, but the *row* still cannot be written first, so the entry is still incomplete on disk for the interval. Staging narrows what a working-tree read takes; it does not make the entry whole, and a `git add` by a peer captures staged content too.
2. **Hypothesis:** this is just the peer-capture class and needs no separate record.
   **Test:** compare remedies.
   **Verdict:** rejected. The capture class's remedies are all about *how to stage and commit*; none of them is available here, because the defect is upstream of staging. This one has a code-level fix and the class as a whole does not.
3. **Hypothesis:** the two-call window is what caused `5c353d8f` to capture `W-99`.
   **Test:** asked the capturing session what it actually ran, rather than inferring it from outside.
   **Verdict:** **rejected — and this is the load-bearing correction.** `f13f8169` ran `git add` on three explicit paths then a pathspec commit of the same three, and was itself editing that ledger that turn. `git add <path>` stages the whole file, so the capture required no window at all. The gap remains a real defect; it is not this capture's cause. Recorded because a bug file asserting the wrong mechanism is worse than no bug file — it aims the fix at something that would not prevent the symptom, and passes review precisely because the symptom is real.

## Fix

Not implemented. Three candidate directions, cheapest first:

1. **Documentation only — stop instructing the row.** The guide already says headings are the index and a row table is optional and hand-maintained. Making that the single instruction removes the second call entirely for ledgers willing to drop the table. Cheapest, and resolves the internal tension named in Root cause. Cost: ledgers that use the table as a reading surface lose it, or keep maintaining it and keep the window.
2. **`append_entry` writes the row.** Needs the table's column shape, which differs per ledger (`bug-fix-session-log`'s Wins Index is 6 columns; other ledgers differ), so it would need a declared row template — plausibly an augmentation field. Closes the window properly for table-keeping ledgers.
3. **A transactional pair** — let `append_entry` accept the row text and write both in one file write. Narrower than (2), no schema inference, and the caller keeps control of the row's prose.

**No SHA / patch-id — nothing is fixed yet.** Do not record either field until a fix lands.

## Tests added

None — no fix yet. When one lands, the regression test must observe the **file on disk between the two writes**, not just the end state: an end-state assertion is satisfied by the current broken behaviour, since both writes do eventually land. That is a monotone-assertion trap of exactly the kind `CLAUDE.md` § *Testing Discipline* names.

## Workarounds

- **Write the index row immediately after the append**, in the very next call, and stage both together. This narrows the window; per `codescout-dd`'s measurement the gap can be smaller than a single tool call, so it does not close it.
- On a busy shared checkout, prefer appending when no peer is mid-commit — check `git status --porcelain` for populated column 1 first. Necessary, not sufficient, for the same reason.

## Resume

Decide between fix directions (1), (2) and (3) — this is a design call, not an investigation. The input worth gathering first: count how many guarded ledgers under `docs/trackers/` actually keep an index table, since direction (1) is free for any that do not and the population may be small. `grep -l 'entry_prefix' docs/trackers/*.md` for the ledgers, then check each for an `## Index` / `## Wins Index` heading.

## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 7* — the capture this was observed in
- `docs/trackers/bug-fix-session-log.md` `W-99` — the entry that was captured
- `get_guide("tracker-conventions")` § *Entry ids*, § *One entry format, never two*
