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

> **Closed wontfix 2026-08-19, same day, by the operator.** The triggering incident — a
> promotion reaching one Claude Code profile of three — is **fixed**: all three `CLAUDE.md`
> files are byte-identical (157 lines, `md5 08f0ef6cb534`) and carry both the Mutation-apply
> discipline and the promoted measurement rule.
>
> The generalisation from that one incident to a checking mechanism was the agent's, not a
> response to repeated cost. At **n=1**, building it is speculative: the design work below
> measured a candidate population of 101 entries for the naive check, which is noise, and the
> precise version needs a schema change plus a retroactive back-fill. Not worth it yet.
>
> **What did ship, and stands on its own** — the zero-code half, already committed:
> `docs/templates/session-log.md` now carries the `Promotion status` audit section ported
> from `eduplanner-ui`, and a `promotion-due` win status. Those reach all nine session logs
> and every future one without any check existing.
>
> Re-open if a second promotion lands incompletely, or if an unharvested `Promote-when` is
> found to have cost something. The measurements below are kept because they are the
> expensive part and would otherwise be re-derived.

> **RE-OPENED 2026-08-20.** The closure above pre-registered its own exit condition —
> *"re-open if a second promotion lands incompletely"* — and it fired.
> `prompt-surface-compaction-session-log:F-9`: three rules promoted into the reconnaissance
> `SKILL.md` (`claude-plugins:23a11c3`) reached **zero of three** profiles, because the
> commit did not bump the version the plugin cache is keyed on. Different mechanism from the
> 2026-08-18 instance, same class. At n=2 the design below is revised — and the fix that was
> sequenced first turns out to be the weaker one.

## The corrected design (2026-08-20, n=2)

**Fix idea 3 — the profile-divergence md5 — would not have caught the second instance.**
Measured 2026-08-20:

    08f0ef6cb5345a3df50a3f4b3b989a96  ~/.claude/CLAUDE.md
    08f0ef6cb5345a3df50a3f4b3b989a96  ~/.claude-sdd/CLAUDE.md
    08f0ef6cb5345a3df50a3f4b3b989a96  ~/.claude-kat/CLAUDE.md

Green — and correctly so. The two failures have **different signatures**:

| | 2026-08-18 (`CLAUDE.md`) | 2026-08-20 (`SKILL.md`, F-9) |
|---|---|---|
| the copies vs **each other** | diverge → md5 fires | byte-identical → md5 green |
| each copy vs **what the entry claims is in it** | absent in 2 of 3 | absent in 3 of 3 |

Comparing the copies to each other catches one case. Comparing each copy to the **claim**
catches both. So the thing to build is claim-verification; fix idea 3 is demoted to a
strictly weaker test of the same property.

**And the anchor must not be a verbatim quote.** Measured the same day: rewriting R-89's
promoted bullet forced a matching edit to the quote stored in `reconnaissance-patterns.md`.
Had that edit been missed, a quote-based check would have reported red on a *correctly*
promoted entry — a false positive produced by the promotion working exactly as intended.

The durable form is **bidirectional**: the entry names the target path, and the promoted text
back-cites the entry id. `R-1` and `R-3` have done this since 2026-05 —
*"(R-1 + R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so they are
verifiable by `grep -c 'R-1' <target>`, invariant under every rewording of the rule itself.
Measured 2026-08-20: the reconnaissance `SKILL.md` back-cites 20 distinct entry ids, and
**none of the three promoted that day cite themselves**.

**Population, measured 2026-08-20** — live trackers, excluding `archive/`, counted per entry
rather than per line: **149** entries carry a `**Promote-when:**`; **13** claim a promoted
status; `promotion-due` is applied to **0**, existing only in `docs/templates/session-log.md`
and in this file.

**Sequenced remedy, revised:**

1. **Back-fill the 13 promotion claims** to carry target + durable anchor. Zero code.
   In progress 2026-08-20.
2. **A `tracker-hygiene` detector that verifies claims every sweep.** The skill already does
   this — inside **D10 step 1**, which fires only at ≥21 days idle. That is archive time,
   which is after the lesson was needed: fix idea 1 restated, now as a located defect rather
   than a general worry.
3. **Only then a `doctor` check**, if 1–2 prove insufficient. It can never be a CI gate: the
   targets (`~/.claude*/CLAUDE.md`, plugin caches) are machine-local and outside every repo,
   so its verdict is not reproducible on another host.
## Summary

Session-log entries carry a `**Promote-when:**` line naming the condition under which the
lesson graduates to a permanent surface (`CLAUDE.md`, an ADR, a skill). When that condition
fires, **codescout** notices nothing: the entry keeps `Status: validated`, which reads as
healthy, and the lesson stays where only its own work stream can see it.

> **Corrected 2026-08-19, hours after filing.** This section first read *"nothing notices"*,
> unqualified. That was false, and asserted without checking the sibling repos — a claim of
> absence from an unchecked scope, which is exactly `reconnaissance-patterns` R-3 (*a search
> that finds nothing is evidence about the search, not about the world*). Something does
> notice, it is described below, and it works.

**The remedy already exists as a practice, in another repo.**
`eduplanner-ui`'s `docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`
carries a **`Promotion status`** section: at archive time each `W-N` is checked *against the
target surface itself*, and recorded as *already promoted* (with the promoted text quoted
verbatim), *UNFIRED, carried forward*, or *FIRED but not yet applied*. Its own `W-4` is
logged as:

> **W-4 — FIRED but not yet applied, carried forward as an action item.** Already at 6
> datapoints … Target is **CLAUDE.md / global review guidance** … *"A review that reports
> findings without observed mutation pass/fail counts has not verified anything."* Not found
> in either the project or the user's global CLAUDE.md as of 2026-08-18.

That audit did its job — it caught the fired state and named the exact text. The lesson was
then promoted, and is the Mutation-apply discipline now in `CLAUDE.md`.

So this bug is narrower and more embarrassing than filed: **the audit convention itself was
never promoted.** Measured 2026-08-19 — `promotion status` appears in **0 of 9** codescout
session logs and **0 times** in `docs/templates/session-log.md`, the generator they are all
copied from, which does mention `Promote-when` twice. Nine trackers accumulate promotion
criteria with no audit step because the template never had one.

That is `prompt-surface-compaction-session-log:W-5` exactly — *when several records make the
same mistake, fix the generator; the records were obeying it*.
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

**0. Port the `Promotion status` convention into `docs/templates/session-log.md`.** Zero
code, and it is the generator fix that reaches all nine logs and every future one. **Done
2026-08-19** — plus a `promotion-due` win status, so the fired state is representable at all.
The three below are what remains after it.

1. **The audit is manual and archive-time-only.** eduplanner-ui's ran when the work stream
   wrapped, which is after the point where the lesson was needed — its `W-4` fired at 6
   datapoints well before archive. The template now says to run it whenever a criterion
   fires mid-stream, but nothing enforces that.
2. **A `doctor` check: `promotion_due_unharvested`.** Report entries with
   `Status: validated` whose datapoint count has reached the number their `Promote-when`
   names. Parsing *"at 3"* out of prose is fragile — prefer a structured `promote_at: 3`
   beside the prose and compare against a counted field. Report-only: judging a lesson ready
   is judgement, not repair. Cheaper now that `promotion-due` exists, since the check can
   simply report entries that *should* carry it.
3. **A profile-divergence check — the cheapest half, and it needs no `doctor` work.** Three
   files that should be byte-identical have an md5. The failure this bug is named for was
   not the audit missing a fired criterion; the audit caught it. It was the promotion
   landing in **one profile of three**, because the audit wrote *"the user's global
   CLAUDE.md"* — singular — and nothing re-checked. A hook comparing `~/.claude*/CLAUDE.md`
   would have caught it the day it was written.

Sequenced 3 first among the remainder: smallest, no schema change, and it addresses the
failure mode that actually bit.
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
