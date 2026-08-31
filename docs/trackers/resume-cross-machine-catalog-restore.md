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
entry_high_water_CM: 10
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

**Correction 2026-08-31 — the premise below is false.** All five have live augmentations in the desktop's `catalog.db`: `structural-debt-refactor` (11 `items`, 25,455 characters / 25,521 bytes of params), `test-escape-hardening` (7 `interventions`), `retrieval-benchmark`, `code-dupes-backlog`, `2026-08-15-tool-usage-investigation`. Restoration is therefore possible and would **not** be invention. This does not overturn the entry's decision — the argument that restoring an augmentation purely to clear a check converts a precise signal into a false all-clear still stands, and the `[LIVE]` block is still read by every agent meeting the tracker cold. Only the impossibility claim is withdrawn. Original text follows.

Five of the 13 have **no provenance anywhere**: `2026-08-15-tool-usage-investigation`,
`code-dupes-backlog`, `retrieval-benchmark`, `structural-debt-refactor`,
`test-escape-hardening`. For those, restoration is not possible, only invention.

**Next:** nothing, until someone writes a query against one. Then restore that one
per § 7, and only that one.

## CM-2 — provenance-subsystem is missing 38 rows, permanently — RETRACTED 2026-08-31

**Status:** fixed 2026-08-31 — recovered in full from the desktop catalog
**Valid:** dated 2026-08-31

**Observed.** `e12cd7e0060ed9b8` recovered **30 of 68**. The 38 lost are
enumerated by id in its augmentation prompt (PV-1, PV-3, PV-6, PV-10, PV-12..PV-24,
PV-28, PV-32..PV-37, PV-39, PV-41..PV-43, PV-45, PV-47..PV-52, PV-54, PV-57, PV-59).
Several are cited from the tracker's own STATE block, so those references resolve to
no row.

The original count of 68 is stated **nowhere on disk except two commit messages**
(`f5f602e6`, `a20b492c`). Without them the restore would have reported success at
44% completeness.

**Done 2026-08-31.** All 38 recovered, ids matching this entry's enumeration exactly, 38 of 38. Projected verbatim from `artifact_augmentation.params` into `docs/trackers/archive/provenance-subsystem-recovered-entries.md`; both hosts' catalogs now hold all 68 rows and agree id for id. The `Next:` line below named this exact path and conditioned it on the desktop's `catalog.db` still existing — it did. Retained for the record:

**Superseded Next:** do not fabricate them from the prose that cites them — that prose is a
citation, not a record. If the desktop's `catalog.db` still exists, a targeted
export of `artifact_augmentation.params` for this one id is the only real recovery
path. Otherwise this is closed as permanent loss.

## CM-3 — four PV rows have no defining heading

**Status:** fixed 2026-08-28 — body edit only, no code. Verified live: `doctor`'s
`entry_without_definition` went from 1 to **absent from `by_check` entirely**, and
`provenance-subsystem` now returns zero findings.
**Valid:** dated 2026-08-31

**Update 2026-08-31.** The AUTHORED titles this entry describes were replaceable after all — PV-9's and PV-11's canonical titles were in the desktop catalog. Both were replaced with the canonical text byte-for-byte: PV-9's authored text asserted a materially different claim, and PV-11's authored text merely *approximated* the canonical (missing the `RESOLVED — ` prefix, `NOT` lowercased) — a substance match discovered in fix round 1 not to license a `RECOVERED-VERBATIM` label, so it too was replaced rather than kept and re-labelled. The VERBATIM/AUTHORED/DERIVED distinction this entry introduced is what made both divergences findable, and it stays.

**Observed.** `doctor` reports `entry_without_definition: 1` against
`e12cd7e0060ed9b8`: PV-9, PV-11, PV-40, PV-46 exist as params rows and as a table's
first cell, but no `## PV-N — <title>` heading defines them, so their citations
resolve to nothing.

Not a regression — those citations were already dangling; adding the rows let
`doctor` attribute them. The ledger ran at 42-of-68 undefined by design.

**Done.** Four `#### PV-N — <title>` sections added under § *Defining sections for
cited entries*, in the block's existing ascending order (… 8, **9**, **11**, 25 … 38,
**40**, 44, **46**, 53 …). Shape matches its neighbours: heading, then
`` `type` · **status** `` and any qualifier.

**The provenance distinction is carried into the body, deliberately.** Each row's
`note` field records how its fields were recovered, and they are not equal:
PV-40 and PV-46 have titles **VERBATIM** from § *The four transferable rules*, while
PV-9 and PV-11 have titles **AUTHORED** on 2026-08-28 from the round-2 table's
question/answer cells because no canonical title survived the catalog loss. Every
`type`/`status` on all four is **DERIVED** by parallel with a sibling in the same
table. A reader who takes an authored title as recovered would be citing an
invention, so each heading's metadata line says which it is and points at the row's
`note` for the full argument.

Also 2026-08-31: this entry's **Status** line above is no longer current in one respect. `entry_without_definition` reads **1** again today, naming `e12cd7e0060ed9b8` — Task 2's restore of the 38 `items` rows brought them back into the check's count, expected and not a regression. The check's accompanying claim that the 31 cited ids "resolve to nothing right now" is false as of Task 1 and is filed as `docs/issues/2026-08-31-entry-without-definition-claims-broken-refs-that-resolve.md`: `scan_undefined_entries` reads only the ledger's own body, and the 38 are now defined in `docs/trackers/archive/provenance-subsystem-recovered-entries.md`. So the count is 1, the count is expected, and the count is misleading — three separate claims, none of which the 2026-08-28 Status line anticipated.

Note this closes a citation break, not a cosmetic gap: those four ids were cited
from outside and resolved to **nothing**.

## CM-4 — body_keeps_snapshot: one predicate, both errors

**Status:** fixed 2026-08-28 — `experiments` `16b5b243`, patch-id
`2293ef75e6a6525efc99c14d3c80b1eec0e25081`.
**Valid:** invariant

**Observed.** Filed as
`docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`
(archived 2026-08-28; the move re-keyed it `ec40b63996d15b62` → `ac7b2b741844aa87`). The
predicate gates on majority id coverage, but a heading satisfies coverage, so:
`tool-usage-patterns` (30 headings, **0** table rows) gets a false positive telling
a maintainer to fix a table that does not exist, while `prompt-hamsa-audit-log`
(30 rows, 34 headings, table genuinely 4 behind) gets **silence**.

Not a threshold bug: the known false positive (21%, scattered) and this one (100%,
headings) sit on opposite sides of every coverage threshold.

**Done.** Added `body_snapshot_row_indices` (the `|`-anchored subset) and routed
the three snapshot call sites through it — `snapshot_stale_note`, `append_entry`'s
`snapshot_missing`, and `doctor`'s `scan_snapshot_drift`. Eight tests; three red
before the fix, two guarding against over-correction.

**The title of this entry is slightly wrong, and it is worth knowing why.** The
predicate was innocent: `body_claimed_indices` folds headings and rows into one
`BTreeSet<u64>` *before* `body_keeps_snapshot` is called, so it never saw the
distinction it is accused of ignoring. The fix is upstream of it and its body is
unchanged.

Two call sites deliberately keep the wide reading, and narrowing either would be a
silent regression: **id allocation** (a heading claiming `F-33` must still block
reissuing `F-33`, or citations re-point) and **`scan_params_behind_body`** (which
subtracts the other way round, where a heading params never saw is a real finding).

One existing fixture changed — `snapshot_drift_does_not_accept_a_prose_mention_as_a_snapshot_row`
anchored its ids as headings and now uses rows. The shape it vacated is covered by
a new test asserting `scan_undefined_entries` reports it instead.

## CM-5 — hamsa Index table is 4 rows behind its params

**Status:** fixed 2026-08-28 — `experiments` `bc6eee3a`, patch-id
`3171b3c5c03b5ccac343909b980ac3d15bc49928`. Verified: `git diff` against
`dde7491b` is the four rows plus a trailing newline, zero deletions; 34 rows
and 34 `## A-N` headings; `doctor`'s `snapshot_drift` now 0 repo-wide.
**Valid:** dated 2026-08-28

**Observed.** `prompt-hamsa-audit-log.md` had 30 `| A-N |` index rows against 34
`## A-N` headings; A-31..A-34 had no row. Its own body calls that table "its
**git-durable snapshot**", so four entries lived only in a machine-local catalog.
A blank line orphaning A-30's row was repaired on 2026-08-28; the missing rows were
not.

**The "author's voice" concern was misplaced, and the section's own prose
retires it.** The Index says it is *rendered from* `params.audits` and is that
collection's git-durable snapshot — a full projection, not a summary. Nothing
needed reconstructing, because the text already exists in the catalog; the
"Gap (1-line)" column label is stale and is what made the table read as a
digest. So the work was mechanical, and the right tool was a generator rather
than transcription: `jq` over `params.audits`, spliced server-side.

**Verify the generator before trusting it.** The template was checked by
projecting **A-30** — a row already on disk — and diffing the two: byte-identical
at 3320 bytes. That positive control is what licenses the other four; without it,
"full projection" is an inference from prose about a convention, and the prose
was already wrong about one column. Also checked the four payloads carry no `|`
or newline before splicing, either of which would have broken the table.

**Cost a data-loss incident on the way, now filed.** The first attempt built the
new body from `jq -r '.body'` of `artifact get --full --json`. That body is
**capped at 500 lines** and the file's is 1553, so the write deleted 1047 lines
and reported `updated: true`; the byte-only shrink guard did not fire, because
the loss was 29% of bytes against 68% of lines. Restored from
`git show dde7491b:<path>` and the bad commit was amended out before it left the
machine. The response *did* warn — `body_meta.line_count` vs `source_line_count`,
plus a whole `overflow` object — and `jq -r '.body'` reads past all of it.
Filed as
`docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md`
and **fixed the next day** — `experiments` `45a88531`, patch-id
`5bcb69c2d5f06b9126ea78c7e8cf2d640c097463`. The guard now trips on lines as
well as bytes, and the predicate moved into one `crate::util::shrink_guard`
shared by all three surfaces that overwrite documents wholesale; there turned
out to be **three** private copies, not the two the bug file named, and
`edit_markdown`'s had no test at all.

**The lesson generalises past this entry:** never build a write payload from a
`get` response — rebuild it from the file or from git, which is exactly what
CM-8 did and why CM-8 was safe.
## CM-6 — `memory(write)` has no shrink guard

**Status:** fixed 2026-08-28 — `experiments` `5b7b82cc`, patch-id
`4477be7feb16fad3ff16b9dfabaa1e884a3ca53e`. Half of it; see *Next*.
**Valid:** invariant

**Observed.** Filed as `docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md`
(archived 2026-08-28; the move re-keyed it `d8a7d136a92ee5a2` → `efb6cea2d5c0cf7e`).
`memory(action="write")` replaces a topic wholesale; writing two sections to the
`gotchas` memory deleted the other 15 (391 → 66 lines, 83%) and returned
`{"status": "ok"}`. The identical artifact operation is refused by a 50% shrink
guard without `force=true`. Restored from git; both `.md` and `.anchors.toml` must
be restored together.

**Done — the guard.** `MemoryStore::shrink_check` (pure, non-mutating) plus the
refusal at the tool, in both the private and project branches. Deliberately NOT in
`MemoryStore::write`, even though that is the single chokepoint: `tools/onboarding.rs`
rewrites two memories wholesale by design, `overwrite_replaces_content` pins
replace-wholesale as the specified primitive semantics, and the artifact precedent
puts its guard in a *tool* with `force` as a caller argument. Ten tests; the
file-unchanged assertion is mutation-verified — warn-but-write leaves `expect_err`
and the error-text check both passing and fails only there.

**Next — two things this did NOT do:**

1. **Unconditional delta reporting** (`prev_bytes`/`new_bytes`/`sections_before`/
   `sections_after`). Still open, still right: a field that appears only on failure
   cannot confirm success.
2. **The `TOOL_SURFACE_CHAR_BUDGET` sweep.** `force` cost ~280 chars against ~27 of
   headroom, so the budget was raised 56_266 → 56_519 at the owner's direction and
   against the gate's own advice. That is **debt**. The sweep that repays it must
   *lower* the constant.

The **workaround is retired for agents on a rebuilt binary** — a destructive
`memory(write)` now refuses — but note the guard ships in code, not in the running
MCP server until `cargo rb` + `/mcp`. Until this host's binary is rebuilt, keep using
`edit_markdown` and assert the section count.

## CM-7 — `@tool_*` buffer grep nesting is unreproduced

**Status:** reproduced 2026-08-29; root cause measured; fix not yet written.
Bug file flipped to `investigating`.
**Valid:** dated 2026-08-29

**Observed.** Filed as
`docs/issues/archive/2026-08-28-tool-buffer-grep-returns-envelope-not-stdout.md`. Six reads
across four handle kinds never reached the payload; escaped by redirecting to a
file. Three reduction attempts had failed to reproduce.

**The prescribed probe worked on the first try.** Running the one untested
difference — the compound `run_command` against a large `@tool_*` — reproduced
every step, with all four numbers within 3% of the original transcript. Kept
here because it is the cheapest known reproducer:

```
librarian(doctor) -> @tool_X (132 KB)
run_command("grep -c 'work/claude/codescout' @tool_X; echo ---;
             grep -o '\"path\": \"[^\"]*\"' @tool_X")   -> @tool_Y, 10011 B
run_command("grep -v 'work/claude/codescout' @tool_Y")  -> @tool_Z, 11158 B
read_file("@tool_Z")  -> the envelope, not the stdout
```

**Root cause, measured, not inferred.** The buffer holds the JSON envelope with
**all of stdout on one line** — `awk` reports 4 lines, line 3 = 9998 B. `read_file`
addresses buffers by *line*, and that line exceeds `INLINE_BYTE_BUDGET` (9000 B),
so every read touching it overflows into a new buffer of the same shape. **The
smallest addressable unit is larger than the largest returnable one**, so the
regress cannot terminate. Reading two lines of a six-line buffer returned a
13389-byte handle — bigger than the thing being read.

**Two corrections the reproduction earned.** The bug file's leading hypothesis —
handle *prefix* decides the payload, `@cmd_*` raw vs `@tool_*` envelope — is
**refuted**: both held structurally identical envelopes. The real axis is which
response *field* the handle came from (`unfiltered_output` = raw text, 400 lines;
`output_id` = envelope). And the data was never lost: `jq -e .` parses and all 117
entries are present, so "the stdout is unreachable" holds only for line-oriented
readers — `jq`, `grep`, `awk`, `head -c` reach it immediately. The bug file's
title is now overstated and says so.

**Second pass, same day — traced to one function, and the severity drops.** The
oversized line is made by `to_string_pretty`: it expands JSON *fields* onto
separate lines but leaves a *string value's* embedded newlines escaped, so a
`run_command` envelope's `stdout` stays on one line. Both pretty-print sites
(`output_buffer.rs:618-776`, `read_file.rs:238-243`) say in their own comments
that they exist to make the content navigable; both are defeated for the one
field that carries the payload. The read then wraps because
`extract_lines_with_cost` (`src/util/text.rs:366-403`) has a **documented safety
valve** — always emit at least one line even if it exceeds the budget, to avoid
stalling — and `extract_lines_to_json_budget`'s doc comment already names the
resulting wrap, citing
`docs/issues/archive/2026-08-25-run-command-nested-buffer-recursion.md`. Stall or
wrap: for a single over-budget line one of the two always happens, which is why
varying the command never reproduced it.

**And it was never unreachable.** `read_file(id, json_path="$.stdout")` returns
the payload as 119 real lines, in one call. Neither the original session nor the
first pass tried it.

**Next:** decide on the fix, which touches a shared primitive and so is not a
drive-by. Recommended is byte-truncating the oversized line inside the safety
valve (~10 lines in `extract_lines_with_cost`, needs a char-boundary-safe cut and
a visible marker), plus a hint naming `$.<field>` when `read_file` line-slices a
`@tool_*` with an over-budget string field. A regression test must use a fixture
whose SINGLE LINE exceeds the budget — many small lines never reach the valve and
would pass with the fix reverted. Options and blast radius are written up in the
bug file's § Fix.
## CM-8 — duplicate frontmatter in the hamsa log

**Status:** fixed 2026-08-28 — body edit only, no code. Verified: `git diff` is
exactly 14 deletions plus a trailing newline; 44 `A-N` headings and 30 index rows
both unchanged; `id`/`kind`/`status` unchanged; augmentation survived.
**Valid:** dated 2026-08-28

**Observed.** Filed as
`docs/issues/archive/2026-08-28-duplicate-frontmatter-block-in-hamsa-log.md`
(archived 2026-08-28; the move re-keyed it `a843bfdefaf6347b` → `5c4f87e68cdad584`). Two
frontmatter blocks since `fec17cd8` (2026-06-14); only the first parses. One file
of 54 — a `kind:`-counting sweep is the correct detector; counting bare `---`
reports 22 and all are false.

**Done, and the caution paid off.** The orphan is *body* to the catalog —
`artifact(get)`'s `$.body` starts with it — so it came out via a body write, not a
frontmatter edit. A heading-scoped `body_edits` could not have reached it: it sits
*before* the first heading, in no section. The new body was a pure line-drop taken
**from the file itself** (`awk 'NR>=27'`) and passed server-side via
`codescout artifact update --body @file`, so nothing in 1,573 lines was re-typed.

**The mechanism question is now tested rather than inferred — the fix was the
test.** Removing the block required a write, and the canonical block **did not come
back**. A writer that re-serialises without detecting an existing block would have
re-emitted it in that same call. Scope it honestly: that exercises full-body replace
only, not `append_entry` / `update_entry`, which are the paths this tracker actually
takes in use. Re-open trigger is a recurrence after one of those.

One claim in the bug file was wrong and is corrected there: hamsa is **not** among
the `augmentation_declared_but_absent` set. A live `doctor` lists 13 and it is not
one of them — its augmentation is present and working. The two issues were never
coupled.

## CM-9 — CM is the ninth declared ledger with no TAXONOMY row — left for the session that owns the file

**Valid:** dated 2026-08-28

**Status:** closed 2026-08-28 — landed by the *other* session about five minutes after this
entry was written. **The prescription below is wrong about which table**; it is kept verbatim
rather than corrected in place, because the error is the instructive part. See § *Resolution*.

**Observed 2026-08-28, after compaction.** This ledger declares `entry_prefix: CM` and
`entry_high_water_CM: 8` in committed frontmatter, and `docs/TAXONOMY.md` carries **zero**
rows naming it. That is the same defect class `HY-22` § *Class 1* documents — declared
ledgers absent from the prefix registry, because the registry is a prose table and nothing
binds it to the frontmatter it describes.

`HY-22` counted eight. **`CM` is the ninth**, and it is ours: this tracker was created at
`25f15f80`, after that sweep's scan, so its absence is not an omission the sweep made.

**Why the row was not added here.** `docs/TAXONOMY.md` was under a *concurrent* Claude
session's hand — PID 2215517, cwd this repo, the file written 60 seconds before the check,
mid-sweep and uncommitted. Editing a file another session is holding in memory risks a lost
update in the direction that leaves no trace: their write lands last and this row vanishes
with no conflict, no error, and nothing to notice. The row is cheap; re-finding it after it
silently disappears is not.

**What to do — WRONG, see § *Resolution*.** Add one row to the § *Other declared ledgers
scoped to one work stream* table the peer sweep introduced:

| Prefix | Ledger | Captures | Entries | Append |
|---|---|---|---:|---|
| **CM-N** | `resume-cross-machine-catalog-restore.md` (`f4923e5e894de62f`) | Work left after a cross-machine catalog resume — what was restored, what was decided against, what is permanently lost | 9 | prose |

Prose ledger: no `entry_collection`; append with `anchor_heading` + `title` + `body`.

**Two adjacent findings checked and dismissed — do not re-chase them.**

1. **No `HY` prefix collision.** `grep -rl '^entry_prefix: HY' docs/` returns two files,
   but the second — `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`
   — has **no `entry_prefix` key in its frontmatter at all** and defines zero `HY-N`
   headings. The match is body text, in a bug file that is *about* ledger prefixes. The
   grep was a proxy and it was wrong; the frontmatter read is what settles it. Same failure
   mode as `T-32`.
2. **`link_scan`'s `prefix_conflicts: [F, W]` is structural, not drift.** Eleven
   `*-session-log.md` files define `F-N`/`W-N` in per-stream namespaces; the five files
   with an empty `entry_prefix:` are those logs, which own two prefixes where the field
   holds one name. Pre-existing, by design, and documented — cite qualified
   (`bug-fix-session-log:F-33`), never bare.

### Resolution — 2026-08-28, by the other session

**Where it actually went.** `docs/TAXONOMY.md` § *Resume queues*, **not** the *Other
declared ledgers* table prescribed above. That is the correct home and this entry had it
wrong: `resume-cross-machine-catalog-restore.md` matches the `docs/trackers/resume-*.md`
filename class that section is defined by, so it is the **sixth resume queue**, not a ninth
miscellaneous ledger. "Five declared ledgers" in that paragraph became six.

**And they caught the half this entry missed.** `CM` was simultaneously sitting in the
drift table's *"one-off spec- or session-local namespaces — and none of them is a defect"*
row. A row there and a row in the queue table are contradictory claims. They removed it,
re-derived *"The twelve without a row"* → *"The eleven"*, and left the note: *"`CM` was in
this list for five minutes."*

**The one-off classification was falsifiable, and false.** Of the eight prefixes in that
row — `AB B CM DF I L TMR TU` — exactly one declares `entry_prefix` in committed
frontmatter:

```
AB->0  B->0  CM->1  DF->0  I->0  L->0  TMR->0  TU->0   (declaring files)
```

The column the row sorts on is *files defining the prefix*, and that is **1 each** for all
eight — so the table's own evidence cannot separate a declared ledger from an undeclared
spec-local namespace. The declaration can. That distinction is what `HY-22` § *Class 1* is
about, which is why the miss landed inside the sweep that was hunting the class.

**What the deferral bought — the reason to keep this entry.** Two sessions reached the same
defect from opposite ends inside five minutes, and *not* editing their open file was still
the right call. The row landed once, in the right table, written by the session that held
the whole registry in context — which is precisely why it also caught the drift-table half.
Had this session raced it in, the outcome would have been a row in the **wrong** table
needing later removal, plus a live lost-update window. Deferring cost nothing and bought
correctness. The generalisation: when a peer holds the file, hand them the finding, not the
edit — they have context you do not.

**Rests on:** the registry being prose with no binding to frontmatter — the condition
`HY-22` names. If a `doctor` check ever derives the table from declarations, this entry and
its eight siblings close together.

## CM-10 — The operator-rules cross-machine revisit trigger fired — two hosts now run the profiles, nothing checks either

**Status:** open
**Valid:** dated 2026-08-28
**Rests on:** `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md` § *Not in scope* → *Cross-machine*

**Observed.** The operator-rules spec deferred cross-machine sync behind a named
trigger: *"a second machine would need a sync design that does not exist and is not
needed to validate the core. Revisit when a second machine runs these profiles."*

That trigger fired 2026-08-28. This host's three profiles were compiled for the first
time (`codescout operator-rules compile` → wrote all three), so two machines now run
them.

Before that compile, all three profiles here were byte-identical to the baseline the
spec's § *Problem* records as *"Measured 2026-08-27, this machine"* — 4639/`b583ffaa`,
4640/`d52fc86c` twice. That identity is the proof this host had never been onboarded:
it still sat at the pre-engine baseline. They are now uniform at 3845/`9b554ef6`, the
retired OP-5 prose is gone from all three, and `operator-rules check` reports all three
current.

**The gap this exposes is asymmetric and silent.** The ledger
(`docs/trackers/operator-rules.md`) is git-tracked, so `OP-N` edits travel between
hosts. The compiled block lives in `~/.claude*/CLAUDE.md`, which are untracked and
machine-local, so it does not. And **nothing runs `operator-rules check` on either
host** — verified across the codescout repo (`*.yml`, `*.yaml`, `*.sh`, `*.toml`,
`*.json`, `*.mjs`, `*.js`, `*.py`, `include_hidden=true`) and the whole
`claude-plugins` tree, each zero paired with a positive control that hit
(`cargo clippy` → `.github/workflows/ci.yml`; `cs-hint` → `buddy/scripts/hook_helpers.py`).

Consequence: the next `OP-N` change pushed from one host leaves the other's profiles
stale, and the engine cannot see across hosts the drift it was built to detect. Same
shape as the catalog problem this queue exists for — nothing fails, you quietly get
less.

**Next:** take the smaller half first. A per-host trigger that runs `operator-rules
check` (SessionStart hook, or a CI job that checks the *ledger* parses and leaves
profiles to the hook) is independently useful and needs no sync design. Full
cross-machine profile sync is spec-sized and belongs in its own plan — do not fold the
two together.

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
