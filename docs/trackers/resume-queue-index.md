---
id: ea6e212a549f9972
kind: tracker
status: active
title: Resume queue INDEX — every tracker holding remaining work (RQ-N)
owners:
- marius
tags:
- resume-queue
- index
- cross-machine-handoff
- vacation-stop-2026-09-04
topic: cross-machine handoff and session wrap-up
entry_high_water_RQ: 4
entry_prefix: RQ
---

# Resume queue INDEX — every tracker holding remaining work (RQ-N)

**One page naming every artifact that holds unfinished work, across all repos on this
machine.** Built at the 2026-09-04 vacation stop so a laptop session can find the open work
without the catalog, which is machine-local and gitignored and therefore absent — silently — on
any other host.

**Read the roster first, then this.** They answer different questions and neither contains the
other:

| surface | axis | scope |
|---|---|---|
| `docs/trackers/resume-vacation-wrapup-2026-09-04.md` (VW-N) | **session → status** at the stop | all repos, point-in-time |
| `docs/TAXONOMY.md` § resume-queue table | **id prefix → file** | codescout only, permanent |
| **this file** (RQ-N) | **tracker → remaining work** | all repos, the resumption view |

## The index

`entries` is the **total** count of `## PREFIX-N` headings in the file, *not* the count of open
ones — no cheap query separates them, and a reader who takes it as a backlog size will
overstate every row. Treat it as "how much is in there", never "how much is left".

### codescout — `/home/marius/work/claude/codescout`

| tracker | prefix | entries | holds | verified |
|---|---|---|---|---|
| `docs/trackers/resume-artifact-chunk-grain-retrieval.md` | `AC` | 3 | (a)-vs-stale-vector decomposition; `7695ad877b44e96a` reconciliation; AE-11/AE-12 suite repair | ✅ authored here |
| `docs/trackers/resume-cross-machine-catalog-restore.md` | `CM` | 10 | what a cross-machine resume restores, decides against, permanently loses | ✅ read |
| `docs/trackers/resume-embedding-transport-stages-1-3.md` | `ET` | 10 | embedding transport consolidation, stages 1–3 | ✅ read |
| `docs/trackers/resume-get-guide-section-grain-phases-2-3.md` | `GG` | 10 | `get_guide` section grain, phases 2 and 3 | ✅ read |
| `docs/trackers/resume-statement-validity-layers-3-5.md` | `SV` | 6 | statement validity, layers 3c/5b | ✅ read |
| `docs/trackers/resume-tool-surface-structural-mechanisms.md` | `SM` | 4 | tool-surface structural mechanisms | ✅ read — **and missing from `TAXONOMY.md`'s table, see RQ-4** |
| `docs/trackers/resume-workspace-pinning-phase-4b-5.md` | `WP` | 5 | per-request workspace pinning, phase 4b + 5 | ✅ read |
| `docs/trackers/gate-contract-consolidation.md` | *(none)* | 0 | five transcriptions of one command list, and what replaces them | ✅ read — prose, declares no prefix, so nothing in it is citable by token |
| `docs/trackers/resume-vacation-wrapup-2026-09-04.md` | `VW` | — | the roster itself; `status: draft` while replies were still arriving | ✅ read |

**Excluded on purpose:** `docs/trackers/resume-tool-surface-budget.md` (`TB`) is `archived` — the
stream shipped in full 2026-08-18 and the queue was opened on a false-negative grep. It is a
closed record, not remaining work, and listing it would inflate this index by one.

**Not enumerated here, by design:** codescout bug files carrying open `## Resume` sections. Two
sessions reported theirs at the stop (`d2bc134a…` across three committed bug files, `08a2785b…`
at `d73ee203`). They are reachable by query and restating them would make this a second roster:

```
doc(action="find", kind="bug",
    filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

### mirela/backend-kotlin — `/home/marius/work/mirela/backend-kotlin`

| tracker | prefix | entries | holds | verified |
|---|---|---|---|---|
| `docs/trackers/innovaplan-live-contract.md` | `IPC` | 14 | **the authority for ITS/Innovaplan** per its session's own hand-off | ✅ file + count read |
| `docs/trackers/its-integration-index.md` | *(none)* | 0 | the first file a laptop session opens for ITS; corrected at the stop after leading with pre-27-August prose | ✅ file read |

### MRV-poc — `/home/marius/work/stefanini/southpole/MRV-poc`

| tracker | holds | verified |
|---|---|---|
| `docs/trackers/ingest-roadmap.md` § *Resume — Phase 3 gold re-authoring* | session `3d806b09…` reported **resume-tracker** | ✅ file and the named section both exist |
| `docs/handoffs/2026-09-04-readiness-erasure-merged-not-deployed.md` | session `8f9907a3…` reported **finalize** | ✅ file exists |
| — | **four sessions whose state was never collected** | ❌ see RQ-3 |

## RQ-1 — This index points; it does not restate, and that is the design

**Valid:** invariant

**Status:** active

Every row names a file and one line of what it holds. None summarises its contents. A summary
here would be a second copy of a live document, and the copy decays while reading identical to
the original — the failure this repo has already paid for at
`docs/trackers/issue-clusters.md` (the one-line `**Members:**` rule, recorded three times in
three places, none of them where an author was standing).

So: **fix drift by re-pointing a row, never by updating a description.** If a row's one-line
summary is wrong, delete the summary rather than correct it.

## RQ-2 — What is verified here, and what is a self-report

**Valid:** dated 2026-09-04

**Status:** active

✅ means **I opened the file on disk from this machine** and, where a count is given, ran it.
Every codescout, mirela and MRV-poc path above carries ✅ — including the two MRV-poc files,
whose existence and (for `ingest-roadmap.md`) named `## Resume` heading were both checked.

**What is NOT verified, in any row: that the work described is still outstanding.** File
existence and entry counts are cheap and mechanical; "is this still open" is a judgement inside
each document. A row here proves a tracker exists and holds entries, never that its contents are
live. **Two sessions at this same stop reported that their real debt was prose already in git
that had stopped being true** — a state no `git status`, no enumeration and no index can detect.
Open the file.

The per-session status column of the roster is a **self-report** by each session about itself
and is not re-derived here. Where this index and the roster disagree, the roster is the record
of what was *said* and this index is the record of what is *on disk*.

## RQ-3 — Four MRV-poc sessions were never collected, and that is this index's known hole

**Valid:** conditional — those four sessions' state is gathered

**Status:** open

The roster's own final row reads `MRV-poc × 4 — *awaiting*`, with two of them started
2026-08-31 and idle four days at the stop. `*awaiting*` means **the reply had not arrived**, not
that the session was clean.

So the MRV-poc section above is a floor, not a total. Any conclusion of the form "MRV-poc has two
open items" is wrong at the unit: it has two *collected* items and four *unknown* ones. On the
laptop, start there — `git status` in that repo plus a read of its own `docs/trackers/`, rather
than trusting either this index or the roster to have spanned it.

**This is the one row where absence of evidence is being reported as absence of evidence.**
Every other row asserts something checked.


**Update, same day, verified rather than relayed:** at least one of the four is now **permanently** unrecoverable, not merely uncollected. `codescout-69` reports PID 4020515 exited without ever replying; I verified `/proc/4020515` is gone and that **three** MRV-poc sessions remain live (1689215, 3737053, 3939042) against the four the roster was waiting on. So one session's state left with the process.

That changes what this row means. "Uncollected" implies a pending action — ask them. "Exited unasked" has no action attached: the only remaining evidence is whatever that session committed, plus MRV-poc's own `git status`. **A hole that can still be closed and a hole that cannot are different facts, and only the second one is final.** Nothing in the roster distinguishes them, because a roster row records the reply that did not arrive and not the process that stopped being able to send one.
## RQ-4 — `TAXONOMY.md`'s resume-queue table is missing `SM-N`

**Valid:** conditional — the paragraph and table are reconciled

**Status:** open — **two defects, not one**, and a third layer neither author caused.

`docs/TAXONOMY.md:90-92` contradicts itself **inside one paragraph**, before its table is reached:

> **Resume queues (opened 2026-08-28).** **Six** declared ledgers, one per partially-implemented
> work stream… Filename class marker is `docs/trackers/resume-*.md`; **all five** are prose ledgers

Six, then five, in consecutive clauses. The table below carries **six** rows (`SV GG WP ET CM TB`),
so *five* was already wrong on the day it was written — this is not decay, it is an error that
shipped. Found by `codescout-69` (sessionId `f2ab55f8-e3ac-4e0a-9eef-3b92d77bac20`) after I filed
the narrower version below; I verified both clauses at the bytes.

**The second defect is the one I filed:** `SM-N`
(`docs/trackers/resume-tool-surface-structural-mechanisms.md`, 4 entries, `status: active`) is in
neither the prose count nor the table.

**The third layer is decay proper, and nobody caused it.** Counted on disk 2026-09-04 09:2x —
**9** files matching `docs/trackers/resume-*.md` declare an `entry_prefix`, **8** of them
non-archived. Three are absent from the table: `SM` (pre-existing), plus `AC` and `RQ`, both
created within the hour by this same wrap-up. So the table was one short before today and is three
short now.

**Still not fixed here, and the reason is stronger than before.** Neither `codescout-69` nor I
knows whether *six* or *five* was the intended figure, so any repair picks a side by guessing —
and a guess written into a canonical index is worse than a contradiction, because a contradiction
announces itself to the next reader while a confident wrong number does not. Recorded, not
actioned. Detector class: `D1` index-drift in `/codescout-companion:tracker-hygiene` terms, though
the self-contradicting clause is not drift at all — it never matched its own table.

**Note for whoever does reconcile it:** the roster `docs/trackers/resume-vacation-wrapup-2026-09-04.md`
titles itself `VW-N` but declares **no** `entry_prefix` in frontmatter, so its entries are not
citable by token and it is not a declared ledger. Whether it belongs in that table is part of the
same decision.

## Template for new entries

```
## RQ-N — <claim-shaped title>

**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Status:** open | active | done

<the claim, and what would falsify it>
```

## History

### 2026-09-04 — opened at the vacation stop

Created because neither existing surface answers *"which trackers hold remaining work?"*: the
roster is per-session and point-in-time, `TAXONOMY.md` is per-prefix and codescout-only. Rows
derived by reading each file from this machine at 09:1x; the four uncollected MRV-poc sessions
(RQ-3) were the hole at creation and are recorded as such rather than omitted.
