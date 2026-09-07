---
id: c4a3d1eb1be7654b
kind: bug
status: fixed
title: 'BUG: append_entry''s two-call protocol guarantees an interval where a ledger entry exists on disk without its index row'
tags:
- cluster/shared-resource-carries-no-owner
- librarian
- trackers
- shared-checkout
- multi-session
closed: 2026-09-07
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
severity: medium
unverified: 'The window is closed for callers who USE the new parameters; it is not closed for callers who do not. `index_row` + `index_after_line` are opt-in, so any ledger whose appends omit them keeps the original two-call window unchanged. Nothing migrates the 21 table-keeping ledgers'' callers, and no gate requires the parameters — a recipe in docs/TAXONOMY.md or get_guide("tracker-conventions") that still prescribes the second call will keep producing the window. The original file''s other unverified: also still stands — the window is now observed on a second ledger (bug-fix-session-log:F-118, this session) but "every table-keeping ledger has it" remains reasoned from the protocol rather than measured per ledger.'
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

**FIXED at `8857b0b2`, patch-id `dec1af44d91b701b1ff7863e9f44b7946df470ce`, on `experiments`
— direction (3).** Chosen on the measurement in § *Resume* rather than on the ordering of this
list: 21 of 49 guarded ledgers keep a row table, so direction (1) is a loss for 43% of them
rather than free.

`doc(action="append_entry")` accepts `index_row` + `index_after_line`. `PendingSection` gains an
`index_row`, spliced into the **same string** as the section and the high-water mark, so one
`fs::write` carries all three.

```
doc(action="append_entry", id=…, id_prefix="F",
    anchor_heading="## Template for new entries", title=…, body=…,
    index_row="| {id} | 2026-09-07 | med | recon | open | **title** — text |",
    index_after_line="|----|------|---------:|----------|--------|-------|")
```

### The three original directions, resolved

1. **Stop instructing the row** — not adopted. Free for 28 ledgers, a lost reading surface for
   21. A per-ledger judgement, not a project-wide fix.
2. **`append_entry` derives the row** — not adopted, and still the only option if a row should
   ever be *derived*. Needs a declared column shape per ledger; nothing here argues for it.
3. **A transactional pair** — **shipped**. No schema inference, and the row's prose stays with
   the caller.

### Design decisions, because the alternatives are the tempting ones

- **`{id}` is a template, not a literal.** The caller cannot know the id before the call — the
  same reason the heading is formatted server-side rather than by the caller.
- **`after_line` is explicit and matches the FIRST such line.** A separator is not unique across
  a ledger with several tables, and filling every one silently is worse than refusing. Same law
  as `anchor_heading` (`docs/adrs/2026-07-10-repair-and-continue-input-handling.md`).
- **One struct, not two `Option`s**, so "a row with no anchor" is unrepresentable rather than
  refused at runtime.
- **A missing anchor writes nothing and allocates nothing.**

### Reachability was half the change

`Args` has no `deny_unknown_fields`, so before the schema learned these fields a caller passing
`index_row` got `Ok` with **no row and no error**. The capability would have existed in
`allocate_entry_id` and been unusable — `IC-3` exactly, and `CLAUDE.md` § *Testing Discipline*
names two tools that shipped in that state for months.
## Tests added

Four, all in the **default** lane only — `augmentation.rs` and `append_entry.rs` are librarian
code, so `--no-default-features` compiles neither. Verified by exact name: **0** of the four run
in lean, **4** in default; controls held (`librarian::` 0 in lean, `prompts::` 101).

| test | site |
|---|---|
| `the_section_and_its_index_row_land_in_one_write` | substitution + placement |
| `a_missing_index_row_anchor_writes_nothing_and_allocates_nothing` | the refusal |
| `the_tool_writes_the_index_row_in_the_same_call` | **reachability** — the only one that fails if the wiring is dropped |
| `an_index_row_without_its_anchor_is_refused_and_names_the_missing_half` | both-or-neither, and that the refusal NAMES the missing half |

**Three mutations on the production path, one per site, each observed RED and each killing a
different test:** missing anchor silently no-ops → the refusal test; `{id}` left unsubstituted →
the one-write test; row inserted *before* the anchor → the one-write test. Restored green.

**The refusal test's discriminator is the RETRY**, not the error: it asserts the next call still
gets `F-2`, so an implementation that errors but lets the transaction commit — issuing `F-3` —
fails. Asserting only *"it errored"* is monotone under exactly that bug.

**One assertion is guaranteed by construction and is annotated as such rather than credited:**
*the file must be byte-identical* after a refused row. No simple mutation breaks it, because the
section and the row are spliced into one string before a single `fs::write`. It is not inert — it
is the regression guard for someone later splitting the row into a second write, which is the
exact defect this closes.

**A loose `grep index_row` over the lean log returns 4**, all of them `tests/issue_clusters.rs`
integration tests sharing the substring. That is the selector-wider-than-its-population shape
filed at `3e040c51`, met again here and caught only by checking exact names.
## Workarounds

- **Write the index row immediately after the append**, in the very next call, and stage both together. This narrows the window; per `codescout-dd`'s measurement the gap can be smaller than a single tool call, so it does not close it.
- On a busy shared checkout, prefer appending when no peer is mid-commit — check `git status --porcelain` for populated column 1 first. Necessary, not sufficient, for the same reason.

## Resume

**The input this section asked for was MEASURED on 2026-09-07. It does not support direction (1),
and it points at (3).**

Population: files under `docs/trackers/` (one subdirectory level included) declaring
`entry_prefix` — **49 guarded ledgers**.

| selector | n | what it actually measures |
|---|---|---|
| `^## (…)Index$` heading | 18 | ledgers with a conventionally-named index section |
| any `h2`–`h4` mentioning "index", case-insensitive | 25 | …plus prose sections *about* indexes |
| **a row matching `^\| *[A-Z]{1,3}-[0-9]+ *\|`** | **21** | **ledgers that actually maintain entry ROWS** |

**Take 21/49 — 43%.** The other two answer a question about *headings*; this bug is about a
second **write**, and the write is a row. Recording all three because the spread (18–25) is wide
enough that any one of them cited bare would have carried a different conclusion, which is
§ *Testing Discipline*'s rule holding on this file's own evidence.

**What that decides.** Direction (1) — *stop instructing the row* — was costed here as plausibly
free because "the population may be small". It is not small: it is 43%, and those 21 ledgers keep
the table as a reading surface. So (1) is free for 28 ledgers and a real loss for 21, which makes
it a per-ledger judgement rather than a project-wide fix.

**Recommendation: direction (3).** Let `append_entry` accept the row text and write the section
and the row in ONE file write. It closes the window for the 21 without costing the 28 anything,
needs no schema inference (which is (2)'s expense — column shapes differ per ledger), and leaves
the row's prose with the caller. (2) remains the only option if the row should ever be *derived*
rather than supplied, and nothing here argues for that.

**First-hand instance, this session:** `bug-fix-session-log:F-118` was written with `append_entry`
(which wrote the section) and then a separate `doc(action="update")` for the index row. The
interval was about ninety seconds on a checkout with five live sessions.
## References

- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` § *Instance 7* — the capture this was observed in
- `docs/trackers/bug-fix-session-log.md` `W-99` — the entry that was captured
- `get_guide("tracker-conventions")` § *Entry ids*, § *One entry format, never two*
