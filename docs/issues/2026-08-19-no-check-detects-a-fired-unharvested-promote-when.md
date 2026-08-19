---
id: '4295b95e60bafeb9'
kind: bug
status: open
title: 'BUG: no check detects a fired Promote-when that was never harvested, so a validated lesson never reaches the surface that would change behaviour'
owners:
- marius
tags:
- librarian
- doctor
- trackers
- record-legibility
- process
topic: record-legibility
closed: ''
opened: 2026-08-19
owner: marius
related:
- e9667199520251e4
- '53e35aaefb9f7c71'
severity: high
---

> **Status: open, severity high.** Nothing breaks at runtime. What fails is the step that
> makes every other lesson take effect — and it fails silently, because a criterion that
> fires produces no event, matches no query, and trips no CI signal.

## Summary

Session-log entries carry a `**Promote-when:**` line naming the condition under which the
lesson graduates to a permanent surface (`CLAUDE.md`, an ADR, a skill). When that condition
fires, nothing notices. The entry keeps `Status: validated`, which reads as healthy, and the
lesson stays where only its own work stream can see it.

This is the third surface in this project to leak the same way, and the first two are already
measured:

| surface | leak | measured |
|---|---|---|
| bug files | fixed-then-not-archived | **75%** zombie-open in one tracker (CLAUDE.md, verify-open cadence) |
| tracker entries | `Status:` absent, so a fired criterion is unharvestable | **39 of 57** over three months (`get_guide("tracker-conventions")`) |
| **fired `Promote-when`** | **criterion met, never executed** | **this file** |

## Symptom (Effect)

Measured 2026-08-19 on `docs/trackers/prompt-surface-compaction-session-log.md`:

`W-4` ("calibrate a hand-built instrument…") reads `**Promote-when:** … At 3, promote to
CLAUDE.md`. It stood at 4 datapoints with `Status: validated`. Probe across all three
Claude Code profiles at that moment:

```
.claude/CLAUDE.md      calibrate=0 instrument=0 measurement=0 sample=0
.claude-sdd/CLAUDE.md  calibrate=0 instrument=0 measurement=0 sample=0
.claude-kat/CLAUDE.md  calibrate=0 instrument=0 measurement=0 sample=0
```

It had reached nothing. The failure it describes then recurred **three more times the same
evening**, in the session that had quoted the entry.

## A second failure mode, found in the same pass

Promotion that *does* happen can land on a subset of the targets. The Mutation-apply
discipline had been promoted, and reached one profile of three:

```
.claude/CLAUDE.md      115 lines   Mutation-apply discipline: present
.claude-sdd/CLAUDE.md   97 lines   Mutation-apply discipline: ABSENT
.claude-kat/CLAUDE.md   97 lines   ABSENT   (byte-identical to sdd)
```

sdd and kat being byte-identical shows this was **one un-propagated write**, not gradual
drift. `CLAUDE.md`'s own first rule says *"always check and apply to ALL THREE instances."*

The consequence was live and invisible: the session that found this ran on `.claude-sdd` and
applied mutation discipline only because codescout's project-instructions path happens to
resolve to `/home/marius/.claude/CLAUDE.md`, injecting the third profile's copy as a second
file. On a repo without that coincidence the discipline is absent, and nothing reports it.

## Root cause

`Promote-when` is prose. The allocator, the guard, `link_scan` and `doctor` all read
structure — frontmatter keys, `## <ID> — <title>` headings, `params` rows — and none of them
reads a criterion stated in a sentence, still less evaluates whether it has fired.

`Status:` is the field that *could* carry the disposition, and
`get_guide("tracker-conventions")` already insists on it for exactly this reason. But the
vocabulary has no value distinguishing **"validated, criterion not yet met"** from
**"validated, criterion fired, nobody harvested it"**. Both are `validated`. The state that
needs surfacing is not representable.

## Fix ideas

1. **A disposition value that names the fired state** — `promotion-due` between `validated`
   and `promoted-to-permanent-docs`. Cheap, and it makes the state queryable with the
   `entry_filter` machinery that already exists. It relies on an author noticing the
   criterion fired, which is the weak link, but it is strictly better than no representation.
2. **A `doctor` check: `promotion_due_unharvested`.** Report entries whose `Status:` is
   `validated` and whose confirming-datapoint count has reached the number named in their
   `Promote-when` line. Parsing "at 3" out of prose is fragile — prefer a structured
   `promote_at: 3` alongside the prose, and have the check compare it to a datapoint count.
   Report-only: deciding a lesson is ready is judgement, not repair.
3. **A profile-divergence check, which is the cheaper half and may not need `doctor` at
   all.** Three files that should be byte-identical have an md5. A pre-flight comparison of
   `~/.claude*/CLAUDE.md` would have caught the Mutation-apply gap the day it was written,
   and is a handful of lines in a hook rather than a catalog check.

Sequenced, 3 first: it is the smallest, catches the failure mode with the sharpest live
consequence, and needs no schema change.

## Prior art in this repo

The pattern that worked three times on 2026-08-19: take a fact that lived only in prose and
give it a query surface. `terminal_status_with_caveat` made a *stated* caveat findable,
`archived_fix_sha_unresolvable` a *broken* anchor, `terminal_status_without_fix_anchor` a
*missing* one. A fired-but-unharvested criterion is the same shape — the author already
writes the criterion down; it just is not written where a query can read it.

That framing is itself from `get_guide("tracker-conventions")`:

> The system's scarce resource is not candour; it is **legibility**. Authors already write
> the caveat. Write it where a query can read it.

## References

- `docs/trackers/prompt-surface-compaction-session-log.md` — `F-7` (this defect), `W-4` (the
  entry that sat unharvested)
- `get_guide("tracker-conventions")` § *Required fields* — the 39-of-57 measurement
- `src/librarian/tools/doctor.rs` — the three checks this one would sit beside

