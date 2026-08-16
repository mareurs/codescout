---
id: '9a892c2a5976e296'
kind: tracker
status: active
title: Open-Issue Work Queue (BL-N)
owners:
- marius
tags:
- backlog
- sequencing
- bugs
- work-queue
topic: work-queue
---

> **Prefix:** `BL-N` — a row in this queue. Work-stream-scoped, defined here, not a project-wide
> namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). Deliberately **not** `T-N`, which
> belongs to `docs/trackers/tool-usage-patterns.md`.

## What this is, and what it is not

A **sequencing layer** over the open bug ledger, snapshotted 2026-08-16 from
`artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})` — 17 rows.

It exists because the ledger answers *what is broken* but not *what to pick up next*. A flat
`status="open"` query cannot express readiness, blockers, or the fact that two entries need the same
decision made once. That is all this file adds.

**It does not own bug status.** Every row points at a bug file, and that file is authoritative. If a
row says `open` and the bug says `fixed`, the bug is right and the row is stale. Never close a bug
from here — and never treat the one-line `next` as the instruction. It is a pointer to that bug's
`## Resume`, which carries the real next action along with the caveats.

## Phase descriptions

Phases encode **readiness, not importance.** A phase-3 item may matter far more than a phase-1 one;
it simply cannot be started by an agent alone.

### Phase 1 — Ready

The mechanism has been read at the bytes and the edit site is named. An agent can open the bug, go to
the cited line, and work. Eight rows.

Worth noting what makes these ready: each names a `path:line`. That is the difference between a bug
someone can pick up and a bug someone must first re-investigate — and it is why the bug template asks
for `path:line` on every root-cause claim.

### Phase 2 — Investigate first

The defect is real but the mechanism is **inferred** rather than measured, or the emission site has
not been located. Acting directly here means acting on an unverified premise, which this repo has
been bitten by: of five bugs worked on 2026-08-07, all five had a false premise or a wrong
prescription (W-13, `docs/trackers/release-promotion-session-log.md`).

BL-11 is the clearest case — its root cause is explicitly marked inferred, and its own Resume asks
for a worktree reproduction before any fix.

### Phase 3 — Blocked

Gated on something an agent should not decide alone:

- **BL-14, BL-15, BL-16** each present mutually-exclusive options. These are cheap to unblock — each
  needs one answer, not a discussion — and BL-15's answer may be `wontfix`, which is a legitimate
  outcome, not a failure.
- **BL-13** is gated on an external eval run (`../prompt-engineering/`), not a preference. Steps 1
  and 2 of that bug are already shipped and verified live; only the prompt wording awaits
  subtract-and-measure, which per `src/prompts/README.md` governs whether *any* prompt-surface change
  ships.

### Phase 4 — Ledger hygiene

BL-17: one bug sits at `docs/issues/archive/…` while its frontmatter still says `status: open`. It
was fixed (`43fac6c8`) and moved, but the status flip was missed — so it appears in every
"what's open?" query while being physically archived. Exactly the drift the archive-through-the-catalog
rule exists to prevent.

## Sequencing notes

Two clusters are worth taking together rather than one at a time:

- **The overflow/handle cluster** — BL-1, BL-2, BL-6, BL-8 all concern a result that was cut and
  whether the caller can tell. They share a root shape: *a truncated payload that reads as complete.*
  The `grep` byte-budget fix (archived 2026-08-16) is the first of this family and its
  `… [truncated: N of M bytes shown]` marker is the pattern the rest should match. Fixing them as a
  set gives one consistent signal rather than four dialects.
- **The worktree cluster** — BL-11, BL-12, BL-16. BL-16 needs a decision that likely constrains
  BL-12's design, so answer BL-16 first even though BL-12 is nominally less blocked.

BL-3 and BL-1 carry the strongest measured evidence: `missing_required_param` is the largest
non-routing error family (38 hits / 20 sessions) and `json_path_key_miss` is 27 hits / 17 sessions,
both from the 2026-08-15 tool-usage investigation. If picking by impact rather than readiness, start
there.

## History

### 2026-08-16 — opened

Snapshotted 17 open bugs into BL-1..BL-17 with per-row next actions taken from each bug's `## Resume`
rather than invented. Phase assignment reflects readiness as of this date.

Context: this queue was created at the end of a session that fixed three bugs
(`grep` byte budget — archived; IL1 steps 1-2 — verified live; plus the IL1 prompt wording) and filed
three new ones. The remaining 17 are what was left standing.

### 2026-08-16 — BL-18 added, found by building this file

Creating this tracker surfaced its own bug. `artifact(create, augment={…})` accepts only `prompt`
and `params`; the `render_template`, `params_schema` and `entry_collection` passed alongside them
were silently discarded, and the call still returned success. Both had to be re-applied with a
follow-up `artifact_augment(merge=true)`.

Filed as `29f1ddf259562b7f` and queued as BL-18. It is a recurrence of a class already fixed once in
the same file (`artifact(create)` dropping `topic`, archived 2026-07-13), and it is compounded by
`tracker_design`'s own Final step listing `params_schema` and `render_template` among the fields to
pass to `create` — guidance followed exactly here, with both fields lost.

Worth noting for whoever works the queue: **BL-18 was found by using the tooling, not by reading
it.** Three of this session's bugs came the same way. A queue built by hand is also a probe.

### 2026-08-16 — BL-5 and BL-18 fixed together

Taken as a pair because both edit `tracker_design`'s `SYSTEM_PROMPT`: BL-5 had to shrink it, BL-18
had to correct its Final step. Doing them in one pass avoided touching the same 100-line constant
twice.

**BL-5** — `tracker_design` went from **~41,000 to 9,358 bytes**, from overflowing on 6 of 6 calls to
arriving inline. The split (menu inline, one archetype per named fetch) was the planned half; the
unplanned half was `existing_trackers`, which at a cap of 30 with six fields per row was ~7 KB —
larger than the entire archetype menu. Capped at 5 rows of `{id, title, kind}`, with Step 7 rewritten
to send the caller to a semantic `artifact(find)` for the collision check a title scan cannot do.

**BL-18** — `AugmentSpec` widened from 2 fields to all 7 and gained `deny_unknown_fields`, so
`create` both accepts the full augmentation shape and rejects typos instead of discarding them. The
advertised schema and `tracker_design`'s Final step now say the same thing the code does.

One lesson worth carrying: **BL-5's first regression test was wrong in a way that would have shipped
the bug.** Written against an empty catalog it read 10,396 bytes; the same code against a full
catalog read 17,456. `existing_trackers` is empty in a bare fixture and populated in production, so
the test would have gone green while every real call still overflowed. A size assertion has to be
made against the shape that ships — the same *wrong population* error TU-5 was corrected for.
