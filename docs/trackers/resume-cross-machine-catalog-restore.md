---
id: f4923e5e894de62f
kind: tracker
status: active
title: Resume queue — Cross-Machine Catalog Restore (CM-N)
owners:
- marius
tags:
- resume-queue
- librarian
- catalog
- augmentation
- cross-machine
topic: cross-machine catalog restore
entry_high_water_CM: 8
entry_prefix: CM
---

# Resume queue — Cross-Machine Catalog Restore (CM-N)

Work left after the 2026-08-28 cross-machine resume, which pulled 437 commits onto
a laptop that had never built codescout and repaired the three catalog layers a
clone never carries.

**Process:** [`docs/conventions/cross-machine-catalog-resume.md`](../conventions/cross-machine-catalog-resume.md) — written during that pass; run it, do not re-derive it.
**Predecessor handoff:** `docs/trackers/bug-ledger-resume-2026-08-28.md`.

## How to use this queue

**To act:** scan the `## CM-N` headings — each carries a `**Status:**` line. There
is no index table to fall out of sync. Re-measure any figure before relying on it:
all counts below are dated 2026-08-28 and the catalog is machine-local, so **none
of them describes any other host**.

**To append:** one call, from the main checkout —

```
artifact(action="append_entry", id="<this artifact's id>", id_prefix="CM",
         anchor_heading="## Template for new entries", title=…, body=…)
```

**Deliberately unaugmented** — see `docs/conventions/cross-machine-catalog-resume.md`
§ 7. Restoring an augmentation nothing queries is authoring, not repair.

## What was DONE (do not redo)

Repaired on this host, verified: catalog reindexed; 23/23 memory vectors
(`migrate-memories --in-place`); 697 `cites` edges added with the fixpoint
confirmed; semantic index rebuilt to HEAD, orphans 23 → 0.

Augmentations restored — **5**, `doctor` `augmentation_declared_but_absent`
**22 → 13**:

| artifact | rows | acceptance query re-verified |
|---|---:|---|
| `legibility-backlog` | 47 | `{"tier":{"eq":1}}` → 10 — **byte-identical to `include_str!` source** |
| `docs/research/README.md` | 15 | `{"topic":{"eq":"retrieval-quality"}}` → 3 |
| `fable-tuning-findings` | 18 | `{"dimension":{"eq":"exploration"}}` → 1 |
| `fable-tuning-tasks` | 12 | `{and:[status:open, priority:high]}` → 0 (correctly empty) |
| `provenance-subsystem` | **30 of 68** | `{"type":{"eq":"decision"}}` → 1 |

Also shipped: `entry_high_water_{GF,FND,SD}` committed; `tracker-hygiene` skill
gained a precondition gate (`claude-plugins` `fedd7bc`, v1.19.7); six bug files.

## Provenance

Opened 2026-08-28 at the end of that pass, before compaction. Every CM-N below is
either a measured residual or a decision deliberately not taken by the session that
found it — the reasoning is recorded so a later session can disagree with it on
evidence rather than re-derive it.

## CM-1 — 13 trackers still unaugmented, by decision, not by omission

**Status:** deferred
**Valid:** conditional — a documented `entry_filter` query is written against one of them

**Observed.** 18 trackers reported `augmentation_declared_but_absent`. Every
documented `entry_filter` prescription against them was extracted and executed;
**4** failed, and those 4 were restored (plus `legibility-backlog`, whose
augmentation is code). The remaining 13 have no documented query at all, so their
absence has no observable cost.

Restoring them would mean authoring 13 standing instructions — the `[LIVE]` block
is read by every agent that meets a tracker cold — purely to clear a check. And
`expects_augmentation` firing is a *precise* signal; filling it with reconstruction
converts it into a false all-clear.

Five of the 13 have **no provenance anywhere**: `2026-08-15-tool-usage-investigation`,
`code-dupes-backlog`, `retrieval-benchmark`, `structural-debt-refactor`,
`test-escape-hardening`. For those, restoration is not possible, only invention.

**Next:** nothing, until someone writes a query against one. Then restore that one
per § 7, and only that one.

## CM-2 — provenance-subsystem is missing 38 rows, permanently

**Status:** open
**Valid:** invariant

**Observed.** `e12cd7e0060ed9b8` recovered **30 of 68**. The 38 lost are
enumerated by id in its augmentation prompt (PV-1, PV-3, PV-6, PV-10, PV-12..PV-24,
PV-28, PV-32..PV-37, PV-39, PV-41..PV-43, PV-45, PV-47..PV-52, PV-54, PV-57, PV-59).
Several are cited from the tracker's own STATE block, so those references resolve to
no row.

The original count of 68 is stated **nowhere on disk except two commit messages**
(`f5f602e6`, `a20b492c`). Without them the restore would have reported success at
44% completeness.

**Next:** do not fabricate them from the prose that cites them — that prose is a
citation, not a record. If the desktop's `catalog.db` still exists, a targeted
export of `artifact_augmentation.params` for this one id is the only real recovery
path. Otherwise this is closed as permanent loss.

## CM-3 — four PV rows have no defining heading

**Status:** open
**Valid:** dated 2026-08-28

**Observed.** `doctor` reports `entry_without_definition: 1` against
`e12cd7e0060ed9b8`: PV-9, PV-11, PV-40, PV-46 exist as params rows and as a table's
first cell, but no `## PV-N — <title>` heading defines them, so their citations
resolve to nothing.

Not a regression — those citations were already dangling; adding the rows let
`doctor` attribute them. The ledger ran at 42-of-68 undefined by design.

**Next:** add four `#### PV-N — <title>` headings under its § *Defining sections for
cited entries*. A body edit, which a restore deliberately does not do.

## CM-4 — body_keeps_snapshot: one predicate, both errors

**Status:** open
**Valid:** invariant

**Observed.** Filed as
`docs/issues/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`. The
predicate gates on majority id coverage, but a heading satisfies coverage, so:
`tool-usage-patterns` (30 headings, **0** table rows) gets a false positive telling
a maintainer to fix a table that does not exist, while `prompt-hamsa-audit-log`
(30 rows, 34 headings, table genuinely 4 behind) gets **silence**.

Not a threshold bug: the known false positive (21%, scattered) and this one (100%,
headings) sit on opposite sides of every coverage threshold.

**Next:** gate on **row** anchors rather than any anchor — `link_scan` already makes
that distinction. Add the fixture the suite lacks: 100% coverage, zero table rows,
asserting `false`.

## CM-5 — hamsa Index table is 4 rows behind its params

**Status:** open
**Valid:** dated 2026-08-28

**Observed.** `prompt-hamsa-audit-log.md` has 30 `| A-N |` index rows against 34
`## A-N` headings; A-31..A-34 have no row. Its own body calls that table "its
**git-durable snapshot**", so four entries live only in a machine-local catalog.
A blank line orphaning A-30's row was repaired on 2026-08-28; the missing rows were
not.

**Next:** add the four rows — Gap / Move / Prediction / Confidence in the author's
voice. Deliberately not reconstructed: filling a "git-durable snapshot" with an
agent's paraphrase would make it durable and untrue.

## CM-6 — `memory(write)` has no shrink guard

**Status:** open
**Valid:** invariant

**Observed.** Filed as `docs/issues/2026-08-28-memory-write-has-no-shrink-guard.md`.
`memory(action="write")` replaces a topic wholesale; writing two sections to the
`gotchas` memory deleted the other 15 (391 → 66 lines, 83%) and returned
`{"status": "ok"}`. The identical artifact operation is refused by a 50% shrink
guard without `force=true`. Restored from git; both `.md` and `.anchors.toml` must
be restored together.

**Next:** port the artifact guard, and report `prev_bytes`/`new_bytes`/
`sections_before`/`sections_after` unconditionally — a field that appears only on
failure cannot confirm success. **Until then: never `memory(write)` to add to an
existing topic.** Use `edit_markdown`, then assert the section count.

## CM-7 — `@tool_*` buffer grep nesting is unreproduced

**Status:** open
**Valid:** dated 2026-08-28

**Observed.** Filed as
`docs/issues/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`. Six reads
across four handle kinds never reached the payload; escaped by redirecting to a
file. **Three reduction attempts failed to reproduce.**

**Next:** run the one untested difference — a compound `run_command`
(`grep -c X @tool_ref; echo ---; grep -o Y @tool_ref`) against a large `@tool_*`
buffer — and check whether the result handle is `@tool_*` or `@cmd_*`, and whether
reading it yields stdout or an envelope. Do not fix before reproducing.

## CM-8 — duplicate frontmatter in the hamsa log

**Status:** open
**Valid:** dated 2026-08-28

**Observed.** Filed as
`docs/issues/2026-08-28-duplicate-frontmatter-block-in-hamsa-log.md`. Two
frontmatter blocks since `fec17cd8` (2026-06-14); only the first parses. One file
of 54 — a `kind:`-counting sweep is the correct detector; counting bare `---`
reports 22 and all are false.

**Next:** reproduce before deleting lines 12–25 — the second block's shape points at
two *already-fixed* writer defects, so the obvious diagnosis may name a mechanism
that no longer exists.

## Template for new entries

```
## CM-N — <one-line title>

**Status:** open | in-progress | done | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened

Opened at the end of the cross-machine restore, before compaction. Eight entries:
two decisions deliberately taken (CM-1, and the not-reconstructing in CM-2/CM-5),
four filed bugs carried forward as work (CM-4, CM-6, CM-7, CM-8), and two measured
residuals (CM-2, CM-3).

