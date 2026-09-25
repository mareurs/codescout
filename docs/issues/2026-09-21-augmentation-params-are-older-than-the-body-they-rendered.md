---
id: '787345ce75805909'
kind: bug
status: open
title: the legibility-backlog's catalog params are older than the committed body that was machine-rendered from them
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: null
opened: 2026-09-21
owner: marius
related: []
severity: medium
---

# BUG: the legibility-backlog's catalog `params` are OLDER than the committed body that was machine-rendered from them

## Summary

`docs/trackers/legibility-backlog.md` is an augmented tracker whose body is written by machine
from its augmentation `params`. The committed body says the backlog was scanned 2026-08-28 and
holds 47 open candidates; this machine's catalog holds a `params` snapshot from 2026-06-15 with
17 open candidates. Both numbers are rendered from the same field by the same template, so the
catalog now holds a state *older* than the render it produced. Nothing reports the divergence —
`librarian(action="doctor")` is silent on this artifact — so a reader of the file and a consumer
of `params` get two different answers with nothing marking either stale.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live; the mechanism cannot be
settled from this host.** Both sides reproduce: the committed body says "Scanned 2026-08-28 · **47 open**" with 47
rows, while `doc get` returns `scan_meta.last_scan_at` 2026-06-15 and `n_candidates` 17 (42 candidates: 17 open,
25 closed). `doctor` reports 80 violations, none naming `cd886c414f6751b4`: no check renders params through
`render_template` and diffs the result against the managed region. The audit trail's only augmentation write is
an insert on host `ripper-1848b7` (2026-09-14); this host has none — hypotheses 2 and 3 need that host's catalog.

## Symptom (Effect)

Committed body, `docs/trackers/legibility-backlog.md` line 15 (working tree clean, identical to
HEAD `937161bd`):

```
Ranked by the legibility engine — **Tier 1** = biting-now (structural defect + observed `usage.db` friction); **Tier 2** = latent (structural only). Scanned 2026-08-28 · **47 open**. Re-run `librarian(action="legibility_scan")` to reconcile — …
```

`doc(action="get", id="cd886c414f6751b4")` → `augmentation.params.scan_meta` on this host
(`ripper-65e654`), read 2026-09-21:

```json
{
  "last_scan_at": "2026-06-15",
  "last_scan_commit": "4d601b5de8e11551825d719769b654be7f498584",
  "n_candidates": 17,
  "project_root": "/home/marius/work/claude/codescout"
}
```

Side by side, both halves of the rendered region:

| quantity | committed body | catalog `params` |
|---|--:|--:|
| scan date | 2026-08-28 | 2026-06-15 |
| open candidates | 47 | 17 |
| closed candidates | 0 | 25 |

No error is raised on either side. Both are internally coherent: the body's open table holds
exactly 47 rows and its `### Closed` table holds 0; `params.candidates` holds 42 rows of which
exactly 17 are `status: "open"` and 25 `"closed"`, so `n_candidates: 17` is the correct count of
its own array. The candidate *sets* differ too, not only the counts.

## Reproduction

```
git rev-parse HEAD                    # de873278 at time of filing, branch experiments
sed -n '15p' docs/trackers/legibility-backlog.md
grep -c '^| `' docs/trackers/legibility-backlog.md          # -> 47
```

then, through the MCP server:

```
doc(action="get", id="cd886c414f6751b4")
read_file("@tool_<id>", json_path="$.augmentation.params.scan_meta")
read_file("@tool_<id>", json_path="$.augmentation.params.candidates[*].status")
```

Machine-dependent by construction: the catalog is machine-local and gitignored
(`docs/conventions/cross-machine-catalog-resume.md`), so the `params` half of this reproduction
is expected to differ per host. The committed half does not.

## Environment

- Host `ripper-65e654`, branch `experiments`, HEAD `de873278`, 2026-09-21.
- Catalog: `~/.local/share/librarian/catalog.db` (machine-local, gitignored).
- Artifact `cd886c414f6751b4`, `docs/trackers/legibility-backlog.md`, `entry_collection: candidates`,
  `expects_augmentation: docs/augmentations/docs-trackers-legibility-backlog.yaml`.

## Root cause

**Unknown — see Hypotheses tried.** What is established is the shape of the divergence, not what
produced it.

Established (each read at the bytes on 2026-09-21):

- **Both figures in the preamble are machine-rendered, not prose.**
  `src/librarian/tools/legibility_scan/render_template.j2` line 3 is
  `Scanned {{ scan_meta.last_scan_at }} · **{{ scan_meta.n_candidates }} open**`, and the same
  template renders both tables from `candidates`.
- **The body is written from `params` by the scan itself.**
  `write_backlog` (`src/librarian/tools/legibility_scan/mod.rs:385-396`) merges `params` through
  `doc(action="augment")` and then calls `render_managed_body` (`:402-432`), which renders
  `include_str!("./render_template.j2")` against those params and writes the result into the body
  via `doc(action="update", patch={body}, force=true)`, preserving everything from the verdicts
  heading down. So the committed table region is a projection of some `params` state — it cannot
  have been hand-typed.
- **The last commit to touch the file did not touch the managed region.** `937161bd` (2026-09-09)
  edited only two lines of Dzo verdict prose below the verdicts heading; the preamble and both
  tables are untouched since the render.
- **The state the body was rendered from once existed in a catalog.** `6b9765ad` (2026-08-28
  12:41 +0300) re-attached this augmentation from its compiled-in source and then ran
  `legibility_scan(write=true)`; its own message records the outcome as *"47 open candidates, 0
  closed"*, which is exactly what the committed body holds.
- **The `params` now in this catalog predate that.** `last_scan_commit` `4d601b5d` is dated
  2026-06-15, matching `last_scan_at`.
- **Nothing reports it.** `librarian(action="doctor", scope="project")` on 2026-09-21 returned 191
  violations and **not one** names `cd886c414f6751b4`. The nearest check by name is
  `params_behind_body`, and it is structurally out of scope rather than failing: it computes an
  id-set difference over `PREFIX-N` tokens anchored in the body
  (`scan_params_behind_body`, `src/librarian/tools/doctor.rs:5361`), and its own doc comment states
  *"Ids only, never statuses"* because a table-cell comparison *"needs a text comparison against a
  column whose format is each tracker's own choice — fragile, and a separate decision"*. This
  tracker's entries are keyed by source path, not by a `PREFIX-N` namespace, so the check has no
  ids to difference and is silent by design, not by defect. Recorded so nobody credits it with
  coverage here.

## Evidence

### The sidecar does NOT carry `params` — the briefing's suspicion is refuted, in the code

`CLAUDE.md` claims the committed augmentation sidecar exports *shape, never params*. Verified true
on three independent surfaces:

- `AugmentationSidecar` (`src/librarian/augmentation_sidecar.rs:49-64`) declares seven fields —
  `schema_version`, `prompt`, `entry_collection`, `params_schema`, `render_template`,
  `append_mode`, `history_cap`. There is no `params` field, so the type cannot carry one.
- `AugmentationSidecar::from_row`'s doc comment: *"Project a catalog row onto its committable
  shape, dropping `params` and every bookkeeping column"*. `AugmentationSidecar::to_row` writes
  `params: "{}".to_string()` with *"`params` starts empty by construction — the data half never
  travels, so a restore gives you a working tracker with no rows, not a tracker with someone
  else's rows."*
- Two tests pin it: `a_written_sidecar_carries_no_params_key`
  (`src/librarian/augmentation_sidecar.rs:412`) and
  `every_committed_sidecar_parses_and_carries_no_params` (`:527`), the second walking the whole
  committed corpus through the restore path's own deserializer.
- The corpus agrees. Across the 24 files in `docs/augmentations/`, the top-level key census is
  `schema_version` 24, `prompt` 24, `render_template` 16, `entry_collection` 15, `params_schema`
  12, and `params` **0**. `docs/augmentations/docs-trackers-legibility-backlog.yaml` itself holds
  exactly `schema_version`, `prompt`, `entry_collection`, `render_template`.

**This is not merely reassurance — it falsifies one of the two candidate mechanisms named in the
briefing.** A `reindex` re-attach from the sidecar produces `params == {}`. What is in this
catalog is a complete, internally consistent 42-row June snapshot. A sidecar re-attach alone
therefore cannot have produced the observed state. (`reindex` also attaches only when a row is
*absent*, so it cannot overwrite a live row at all.)

### The audit trail, and the window it cannot see

`librarian(action="audit_log", row_id="cd886c414f6751b4")` returns 8 rows. Seven are `artifact`
updates on this host; exactly one touches `artifact_augmentation`, and it is an **insert** on a
*different* host — `ripper-1848b7`, seq 11212, 2026-09-14 12:39:15Z — read out of the committed
replica `.codescout/audit/ripper-1848b7-202609.jsonl`, not out of this machine's trail.

**That zero is scoped to a window that opens after the event it would need to see, so read it as
nothing more than it is.** `librarian(action="audit_log", until=1787950000000)` (≈ 2026-08-28
20:46 UTC) returns **0** rows on this host, and the audit-shard work landed 2026-09-01
(`605f0ece`, `f8df0ed3`, `51d01d56`, `f19cf031`). So the local trail begins *after* the 2026-08-28
scan. What the trail does establish: **no in-band `artifact_augmentation` write has touched this
row on this host since ~2026-08-28 21:00 UTC.** It establishes nothing about anything earlier, and
nothing about a write that bypassed the catalog's audited path entirely.

### Blast radius

`doc(action="find", augmented=true, scope="project")` returns **31** augmented artifacts in this
repo, derived 2026-09-21. (The commonly cited figure is 22 — that is `d6d66e4c`'s 2026-08-28
subject line, and the population has grown since; derive it rather than quoting either number.)
Every one of them stores machine-readable state in the same gitignored catalog while its rendered
body travels in git. An augmentation silently reverting means a tracker's machine-readable state
can regress while its committed body still shows the newer values, and **any consumer reading
`params` gets the older answer with no error** — `entry_filter`, `append_entry`'s id allocator,
`render_template` inside `librarian(action="context")`, and every scan that reconciles against a
prior snapshot.

### Two observable consequences, today

1. **Two answers, neither marked stale.** A reader comparing the rendered preamble against a
   `params` query gets 2026-08-28/47 from one and 2026-06-15/17 from the other, with nothing on
   either surface indicating which is current. Both are internally coherent, so every check aimed
   at either one confirms it.
2. **A 30-row discrepancy in a backlog someone is meant to work from**, and the candidate sets
   differ, not only the counts: the committed body's `### Closed` table is empty while `params`
   holds 25 closed rows. A `legibility_scan` run from this catalog would reconcile against the
   June snapshot — `reconcile` (`src/librarian/tools/legibility_scan/mod.rs:242-312`) diffs the
   current scan against `prior.candidates` and auto-closes every prior `open` row absent from it —
   so the stale prior is an input to the next write, not merely a stale read.

## Hypotheses tried

1. **Hypothesis:** a `reindex` re-attached the augmentation from its committed sidecar, and the
   sidecar carried an older `params`.
   **Test:** read `AugmentationSidecar`, `from_row`, `to_row`, the two pinning tests, and the
   committed corpus (above).
   **Verdict:** **rejected.** The sidecar type has no `params` field, `to_row` attaches with
   `params: "{}"`, and no committed sidecar carries a top-level `params` key. A re-attach would
   have produced empty params, not a coherent June snapshot.

2. **Hypothesis:** cross-machine divergence — the 2026-08-28 augment-and-scan ran against a
   catalog this host does not share; this host's row was never advanced past its June state, and
   the newer body arrived by `git pull`.
   **Test:** none run.
   **Verdict:** **untested.** Consistent with what is known: the catalog is machine-local and
   gitignored (`docs/conventions/cross-machine-catalog-resume.md`), `c2039a16`'s message speaks of
   *"one of the two machines that still hold these rows"*, and the merged audit trail names three
   hosts (`ripper-65e654`, `ripper-1848b7`, `archlinux-d9b5c3`). Note the counter-signal before
   anyone treats this as settled: `scan_meta.project_root` is this machine's path — though that
   field records the path the scan ran against, and the path could coincide across hosts.

3. **Hypothesis:** the catalog was restored or rolled back from a backup predating 2026-08-28.
   **Test:** none run.
   **Verdict:** **untested.** A pre-integration catalog backup is known to have existed and to
   have been used for a recovery on 2026-08-31 (recorded in `write_through`'s doc comment,
   `src/librarian/augmentation_sidecar.rs`). Whether this row's state came from one is unchecked.
   A file-level DB restore would also bypass the audit trigger, which is why the trail's silence
   does not discriminate here.

4. **Hypothesis:** an in-band wholesale `params` replace (a `merge=false` augment, or a
   hand-built array) reverted the row.
   **Test:** `librarian(action="audit_log", row_id="cd886c414f6751b4")`.
   **Verdict:** **untested, and bounded.** No such write appears on this host since ~2026-08-28
   21:00 UTC, which is the whole of the trail's coverage. Anything earlier is outside the window.

## Fix

None. Filed for the record; not fixed in this session.

The repair for *this artifact* is a `legibility_scan(write=true)` re-run, which would overwrite
both halves from a fresh measurement — but that discards the question rather than answering it,
and would destroy the evidence above. Do not run it before the mechanism is established.

What a real fix needs is a detector: a check that compares the committed rendered region against
what the current `params` would render, for every artifact declaring a `render_template`. That is
strictly wider than `params_behind_body`, which is id-keyed and correctly declines this case, and
it is the direction `sidecar_shape_drift` already takes for the *shape* half of the same row.

## Tests added

None — no fix was attempted. A regression test is owed with the fix and cannot be written before
the mechanism is known: the assertion would have to name the write path that reverts the row.

## Workarounds

Treat the committed body as the record of what a scan last measured, and never read `params` as
current without checking `scan_meta.last_scan_at` against the preamble. They are two copies of one
state and only one of them travels.

## Resume

- The mechanism is open. The two live candidates are hypotheses 2 and 3; hypothesis 1 is refuted
  in code and should not be re-opened.
- Cheapest next step: ask the other hosts. `ripper-1848b7` inserted an `artifact_augmentation`
  row for this id on 2026-09-14 — read *that* catalog's `scan_meta` and see which of the three
  states it holds.
- Do not run `legibility_scan(write=true)` on this artifact first; it overwrites both sides.

## References

- Artifact: `cd886c414f6751b4`, `docs/trackers/legibility-backlog.md`
- `docs/conventions/cross-machine-catalog-resume.md` — the catalog is machine-local and gitignored
- `docs/augmentations/docs-trackers-legibility-backlog.yaml` — the committed shape, params-free
- Commits: `6b9765ad` (re-attach + scan, 2026-08-28), `c2039a16` (sidecars committed, 2026-08-30),
  `937161bd` (last touch, verdict prose only, 2026-09-09)
- Nearby and deliberately distinct — both `fixed`, and neither covers this:
  `docs/issues/archive/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md`
  (per-row *content* staleness where both sides hold the same id) and
  `docs/issues/archive/2026-08-18-no-check-detects-a-body-that-has-run-ahead-of-params.md`
  (the body ahead of params, which is this defect's inverse and the origin of `params_behind_body`).
  Here there is no shared id namespace at all, and it is the catalog that ran *behind*.
- Same tracker, different claim, filed the same day:
  `docs/issues/2026-09-21-legibility-auto-close-cannot-tell-repair-from-detector-removal.md`

## Cluster classification — nearest fit, with the mismatch recorded

Tagged `cluster/value-correct-in-a-frame-its-name-does-not-state` (`IC-24`). The frame here is the
**catalog instance and the instant**: `47 open` is exactly right in the frame *"the catalog that
rendered this body on 2026-08-28"*, `17 open` is exactly right in the frame *"this host's catalog
as of 2026-06-15"*, and both are published under a name — the tracker's live Backlog section, and
`scan_meta.n_candidates` — that states neither. `IC-24`'s blind-party clause fits exactly: on the
host that ran the scan the two frames coincide in every check its author would think to run, and
every instrument aimed at either value confirms it, because neither value is what is broken.

**Where it strains:** `IC-24`'s claim says the value is *"exactly right and exactly recoverable"*,
and here neither number recovers the other or the truth — the current open count is not derivable
from either side without re-running the scan. The class already carries two members recorded with
this same strain (`called-at-records-completion-at-second-resolution`,
`predicate-probe-overstates-retrieval-and-redundancy`), which is the precedent this entry follows
rather than an excuse.

**`IC-8` (`record-asserts-an-unchecked-completion`) was considered and rejected**, on the class's
own recorded test: the committed preamble asserts a scan that genuinely completed, and `6b9765ad`
records its real outcome. `IC-8` withdrew a member for exactly this reason — *"They were all true.
… The record asserted nothing false, so this class's claim is not satisfied"*. Nothing here is a
closure note for a step that did not happen; what failed is that one of two copies of the state
went backwards.
