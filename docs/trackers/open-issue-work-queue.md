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

## Queue — rendered snapshot (2026-08-18)

> **`params` is the source of truth; this table is a snapshot of it.** Params live in the librarian
> catalog (`~/.local/share/librarian/catalog.db`), which is **not** in the repo — so without this
> section the queue would be invisible to git and to any other checkout. Re-render it when rows
> change. Query the live rows with
> `artifact(get, id="9a892c2a5976e296", entry_filter={"status":{"eq":"open"}})`.

| ID | Ph | Task | Status | Bug |
|----|---:|------|--------|-----|
| BL-1 | 1 | json_path: add a `Segment::Wildcard` arm so the overflow hint's own recovery works | **done** | `875e5d03d980ceac` |
| BL-2 | 1 | grep: stop printing a self-refuting "Showing N of N" when collection hit the cap | **done, archived** | `8e665c2d041ebb04` |
| BL-3 | 1 | Tool schemas: stop advertising conditionally-required params as optional | **done, archived** | `02d2d9d8a7eeec2e` |
| BL-4 | 1 | usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer | **done, archived** | `fc00c33f2403ae8a` |
| BL-5 | 1 | librarian: split `tracker_design` so its guidance arrives inline | **done** | `3f88d49c38ced0c1` |
| BL-6 | 1 | read_file: give the buffered full-read summary an incompleteness signal | **done** | `a9644b964edac789` |
| BL-7 | 1 | Write-scope denial should name `approve_write` | **done** | `0a15c81150c4cce7` |
| BL-8 | 2 | `truncate_compact` cuts from the tail, destroying the overflow signal | **done** | `c320b6564d1cb003` |
| BL-9 | 2 | `server_instructions` arrives truncated mid-word, dropping the guide pointers | **done, archived** | `be057e5e9d7c4c16` |
| BL-37 | 2 | Kotlin warnings / workspace table / Custom Instructions cannot fit the 2048-char instructions channel — ordering fixes the common case (shipped `30f3df81`); the two oversized blocks need a new carrier | done | `e3437bd1ec116dec` |
| BL-10 | 2 | `audit_doc_refs` reads bare comment markers as file paths | **done** | `772fff5739620581` |
| BL-11 | 2 | `context`/`workspace_state_at`/`link_scan` never dedup the worktree overlay | done | `d31233700ca979c2` |
| BL-12 | 2 | worktree divergence guard covers writes but not reads | done | `c611a3dce4f05d45` |
| BL-35 | 2 | `guard_worktree_write` is dead code in production (startup cwd sets the flag it gates on) | **done, archived** | `a742a50ea6723daf` |
| BL-13 | 3 | IL1: run subtract-and-measure on the step-3 wording — ran as hamsa A-25, clause LOST (base 10/10, clause 8/10 vs a ≤ 1/10 ship bar), reverted at `32b34efa` behind an inverted guard | **done, archived** | `b4d48dbfecc205c9` |
| BL-14 | 3 | read_file: `force=true` silently discarded on whole-file reads | done | `1780acde047ffca2` |
| BL-15 | 3 | Read-only metadata commands (wc/ls/stat) blocked on source paths | done | `6902806f459fcf62` |
| BL-16 | 3 | Worktree activation diverges memory set and sub-project topology (topology CLOSED `1869adcb`; memories = option 1, a semantic call) | open | `403e3fad0356f171` |
| BL-17 | 4 | Reconcile a bug sitting in `archive/` while still marked `status: open` | **done** — measured 0; its own bug file is gone | — |
| BL-18 | 1 | `artifact(create)`: `augment` silently discarded five of its seven fields | **done** | `29f1ddf259562b7f` |
| BL-19 | 1 | Overflow envelopes with no compact summary waste a whole call | **done, archived** | `3d733b00b134634c` |
| BL-20 | 1 | params merge-patch wipes entry arrays wholesale — gave entries an update path (`update_entry`) + always-on counts | **done, archived** | `36eda0c2634dbea9` |
| BL-21 | 1 | edit_file's replace_all + batch paths write librarian-managed artifacts with no guard | **done, archived** | `e52abced30ff1dbc` |
| BL-22 | 1 | `move` broke the `id == hash(abs_path)` invariant, so the next reindex cascade-deleted the history | **done, archived** | `18a637f59289192c` |
| BL-23 | 3 | a moved artifact's frontmatter still asserts its pre-move id | **done, archived** | `61e2360408cb206b` |
| BL-24 | 2 | usage.db records a sha that need not describe the built code, and drops the dirty bit | **done, archived** | `0cd1fe818951b232` |
| BL-25 | 1 | the 2200-byte cap evicts rules into `get_guide` topics nothing triggers — 7 of 10 guides (~46 KB) have no trigger at all | **done** | `cfcbee6f7d047a55` |
| BL-26 | 2 | `get_guide("librarian-runtime")` says a move preserves the id; a move mints a new one — 2d8c7f39 repaired 3 of 4 copies | **done, archived** | `5d8584d109d876ea` |
| BL-27 | 3 | `update_entry`'s entry-param guard only fires when `fields` is absent; send both and `entry` is dropped silently | **done, archived** | `d082f963f57bd76b` |
| BL-28 | 3 | a directory named `--help` holding an initialised codescout project sits untracked in the repo root | **done, archived** | `ba6ab341eab97416` |
| BL-29 | 1 | `append_entry` writes catalog-only state, so this very snapshot drifts silently — tool says success, git says clean | partial (`99aaf83f` + `6ff00eee` + `0dbfd0ee`): drift reported at write time and by `doctor`; hamsa reconciled; gate now needs majority coverage; **0 trackers adrift** | `0694a4a9946e10fe` |
| BL-30 | 2 | FRICTION: adding one entry costs four bookkeeping sub-tasks — id, workflow, row format, re-render | done | `c3f08f7cb8b386fe` |
| BL-31 | 2 | grep: `cap_grouped`'s file-diversity round-robin is unreachable, so overflow hints name walk-order files not hot ones | **done, archived** | `2a9fd7654cf82013` |
| BL-32 | 3 | R-N ledger reused nine ids for unrelated lessons — split by suffix in `52fca682`; the hand-allocation cause is BL-30 | done | `a0251c34af7aa012` |
| BL-33 | 1 | the librarian guard keys on YAML quoting, so 15 of 27 trackers (incl. this queue) are unprotected | **done, archived** | `e7353641aafe0098` |
| BL-34 | 2 | repairing a frontmatter id re-serializes the whole block, reformatting hand-authored YAML | **done, archived** | `529a6c05895cc686` |
| BL-36 | 1 | `artifact(update)` re-serializes the whole frontmatter block on a single-field patch — BL-34's mechanism at the mandated archive step | **done, archived** | `82ba248228301486` |
| BL-40 | 1 | every drift check asks whether the body kept up with params — nothing detects params falling behind a body that ran ahead | **done** `87f3b936` — fired twice on its first live run, incl. a second repo nobody had looked at; data repair split to BL-42. **Archived** — id re-keyed `bde782f4cc52ac22` → `0808a5251625e6db` | `0808a5251625e6db` |
| BL-41 | 1 | link_scan's dangling count is prefix-gated, so a namespace with zero definitions reports as healthy | **done** `ff088630` — measured on the wire: dangling 548 → 471, but BL-41's own contribution is **ZERO**; the prediction in the bug file was wrong. Coverage completion → BL-43. **Archived** — id re-keyed `52269554ea4f51a4` → `e891b7c6a5b1dbe7` | `e891b7c6a5b1dbe7` |
| BL-42 | 2 | DATA REPAIR: entry rows exist in a tracker body and in no params row, so `entry_filter` and every params-based query miss them | **done — codescout half**: the 6 WIN rows repaired, `doctor` 2→1. A second defect (7 rows present on both sides with disagreeing content) surfaced while diffing and was also repaired — split out as its own bug, not this one. SI×10 still handed off | `0808a5251625e6db` |
| BL-44 | 3 | NEW, surfaced while closing BL-42: no check detects an existing params row whose CONTENT has gone stale relative to its body (distinct from BL-40/BL-42, which are about row absence) | **dropped** — the 7 known-bad rows on `windows-platform-support.md` are fixed and re-verified; the detection gap itself is a design decision, not a queued task | `2a4d51f2e0521468` |
| BL-43 | 1 | complete BL-41's coverage — a ledger that declares no `entry_prefix` AND defines nothing is still invisible to the dangling gate | **dropped — handed off**, both targets are other repos; codescout's own declarations are already done | `e891b7c6a5b1dbe7` |
| BL-39 | 1 | the two sanctioned entry formats are not equivalent — a params-rendered index defines no citable token, so 117 BL-N citations (incl. this queue's own) resolve to nothing | **done, archived** — all steps shipped (`de4df2cd`, `f19d5296`, `758b37dc`, `d3c1e6ed`, backfills `f04e4c17`/`0d101eb8`/`f5f602e6`/`9703102c`/`c7bdfd22`). `doctor` `ledger_defines_nothing` 10→2, `entry_without_definition` 3→1. The blocking peer file committed and was archived; moved 2026-08-18, id re-keyed `d34dfcd2cc718bd8` → `9dc28c0860b214d9`; 19 citations across 10 live files re-pointed in the same commit | `9dc28c0860b214d9` |
| BL-38 | 1 | the librarian guard is blind to any artifact whose frontmatter omits `id:` — fixed by teaching it the `entry_prefix` ledger declaration; the plan's heading-scoped half was cut as unnecessary | done | `388290ad0f86fe03` |

> **Params and body reconciled again** (2026-08-16, second pass — 31 rows). The
> previous reconciliation held for status but not for **ids**: BL-26 and BL-27 were
> archived, and `artifact(action="move")` re-keys, so params carried the new ids while
> this snapshot still cited `db02045fdbaaf860` / `ea21099f9d39f734` — neither of which
> resolves any more. Three rows had drifted (BL-2, BL-26, BL-27) and BL-31 was missing
> entirely.
>
> A **duplicate BL-31 row** also had to be removed here: two sessions hand-rendered the
> same new params entry into this table within minutes of each other, one of them with
> `—` placeholders for the fields it could not see. `append_entry` prevented the *id*
> collision server-side (the concurrent session's entry took BL-32, mine BL-33, with no
> coordination) — but nothing protects the hand-maintained snapshot from the same race.
> That gap between the two is BL-30's cost stated precisely.
>
> That is **BL-29** demonstrated, not a lapse: `update_entry` and `move` both write
> catalog-only state, so every entry-grain write silently ages this table. The check that
> catches it is `artifact(get, entry_filter=…)` against the live params before trusting
> any row here — an id that returns `count: 0` is archived, not deleted.
>
> Earlier note, still true: BL-1, BL-20 and BL-22 were flipped with
> `artifact(action="update_entry", …)`. The note that used to sit here said the flip was
> unsafe because there was no entry-grain update; that was BL-20, and it is now fixed.
> Its own row was the first thing the fix was used on.

Next actions per row live in each bug's `## Resume`, and in the live params — not duplicated here,
because a snapshot that carries instructions goes stale in the way that matters most.

## Per-entry detail

### BL-44 — a params row can drift out of sync with its body counterpart with no check on either side of that direction

Surfaced while executing BL-42's data repair: diffing `windows-platform-support.md`'s body table against `params` field-by-field (not just by id) found 7 rows present on both sides with different content — `WIN-1`, `WIN-4`, `WIN-5`, `WIN-20`, `WIN-27` had a stale pre-archive `ref`; `WIN-28` and `WIN-29` were worse, `params` held an earlier `open` snapshot with a superseded root-cause summary while the body already carried the resolved `fixed` story. This tracker's own `entry_filter={"status":{"eq":"open"}}` convention would have returned two closed issues as open, with the wrong explanation.

`doctor`'s `params_behind_body` (BL-40) does not cover this: it computes set difference on ids, so a row present on both sides with disagreeing fields passes it silently. `update_entry` already warns one direction (`snapshot_stale`, params-changed/body-didn't) but nothing scans the other direction — a body edit that never touches params leaves no trace.

Repaired the 7 known-bad rows via `update_entry` (safe — every row already existed) and re-verified by diffing all 35 `WIN-N` rows field-by-field: zero mismatches remain. Left open as a design question whether the fix should be a `doctor` check (report-only, matching `params_behind_body`'s shape) or a write-time guard (symmetric to `snapshot_stale`, firing on a body edit instead of a params edit). Status `dropped` here because nothing is queued to execute — see `docs/issues/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` (id `2a4d51f2e0521468`, `mitigated`) for the full evidence and the two candidate shapes.

One section per BL-N, and its only job is to **define the token**. `link_scan` derives a citable
definition from a `## <ID> — <title>` heading and from nothing else, so until 2026-08-18 this
ledger — which defines the queue everything else cites — could not be cited at all: 117 BL-N
references from 37 other files resolved to nothing, and the table above defined none of them.
Deliberately terse; the table holds status, phase and bug id, and `next` (catalog-side) holds the
working notes.

**When you add a BL-N, add its section here too.** Not optional and not the table — confirm with
`librarian(action="doctor")`, which reports `ledger_defines_nothing` / `entry_without_definition`.
See `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`.

### BL-1 — json_path: add a Segment::Wildcard arm so the overflow hint's own recovery works
**done**

### BL-2 — grep: stop printing a self-refuting "Showing N of N" when collection hit the cap
**done**

### BL-3 — Tool schemas: stop advertising conditionally-required params as optional
**done**

### BL-4 — usage.db: derive the backfill gate from the taxonomy, not a hand-maintained integer
**done**

### BL-5 — librarian: split tracker_design so its guidance arrives inline
**done**

### BL-6 — read_file: give the buffered full-read summary an incompleteness signal
**done**

### BL-7 — Write-scope denial should name approve_write
**done**

### BL-8 — truncate_compact cuts from the tail, destroying the overflow signal
**done**

### BL-9 — server_instructions arrives truncated mid-word, dropping the guide pointers
**done**

### BL-10 — audit_doc_refs reads bare comment markers as file paths
**done**

### BL-11 — context and workspace_state_at never dedup the worktree overlay
**done**

### BL-12 — worktree divergence guard covers writes but not reads
**done**

### BL-13 — IL1: run subtract-and-measure on the step-3 wording
**done** — ran as prompt-hamsa A-25 and the clause LOST; reverted behind an inverted guard.

### BL-14 — read_file: force=true silently discarded on whole-file reads
**done**

### BL-15 — read-only metadata commands blocked on source paths
**done**

### BL-16 — worktree activation diverges memory set and sub-project topology

**open** — but for one half rather than two. The **topology** half is closed
(`1869adcb`): `load_discover_settings` reads through to the main checkout's
`workspace.toml` when the worktree has none, which is option 3's goal without its
file-sync cost, and the activation notice gained a third topology state
(`inherited`) so it stops claiming discovery "ran with defaults" about a walk that
inherited main's.

What remains is **option 1 only**, and it is a decision rather than an
implementation: should a worktree serve the MAIN checkout's memories (the
librarian-overlay precedent — overlay onto main, fork on first write) or its own
commit's? Option 3 is spent; do not re-raise it.
### BL-17 — reconcile a bug that sits in archive/ while still marked status: open
**done** — measured 0 instances; the bug file it was filed against is itself gone.

### BL-18 — artifact(create): augment silently discarded five of its seven fields
**done**

### BL-19 — overflow envelopes with no compact summary waste a whole call
**done**

### BL-20 — params merge-patch wipes entry arrays wholesale
**done** — gave entries an update path (`update_entry`) plus always-on counts.

### BL-21 — edit_file's replace_all and batch paths write managed artifacts unguarded
**done**

### BL-22 — reindex re-keys moved artifacts and cascade-deletes their events
**done** — one reindex destroyed 11 events while reporting `removed: 0`.

### BL-23 — a moved artifact's frontmatter still asserts its pre-move id
**done**

### BL-24 — usage.db records a sha that need not describe the built code
**done**

### BL-25 — the byte cap evicts rules into get_guide topics nothing triggers
**done** — 7 of 10 guides (~46 KB) had no trigger at all.

**Valid:** dated 2026-08-18

### BL-26 — get_guide("librarian-runtime") said a move preserves the id
**done** — one fact in four files; an earlier pass repaired three and missed this one.

### BL-27 — update_entry's entry-param guard only fires when fields is absent
**done**

### BL-28 — a directory named `--help` sits untracked in the repo root
**done**

### BL-29 — append_entry writes catalog-only state, so the committed snapshot drifts
**open** — partial: drift is now reported at write time and by `doctor`, and 0 trackers are adrift; the gate still needs majority coverage.

### BL-30 — FRICTION: adding one tracker entry costs four bookkeeping sub-tasks
**open** — the hand-allocation root cause behind BL-32.

### BL-31 — grep: cap_grouped's file-diversity round-robin is unreachable
**done** — overflow hints named walk-order files rather than hot ones.

### BL-32 — reconnaissance-patterns.md reused nine ids for unrelated lessons
**open** — split by suffix already; the hand-allocation cause is BL-30.

### BL-33 — the librarian guard keys on YAML quoting, leaving trackers unprotected
**done** — 15 of 27 trackers, this queue included.

### BL-34 — repairing a frontmatter id re-serializes the whole block
**done**

### BL-35 — guard_worktree_write is dead code in production
**done** — the startup-cwd fallback sets the very flag it gates on.

### BL-36 — artifact(update) re-serializes the whole frontmatter block on a single-field patch
**done** — BL-34's mechanism, firing at the mandated archive step.

### BL-37 — Kotlin warnings, workspace table and Custom Instructions cannot fit the channel
**open** — ordering fixed the common case; the two oversized blocks need a new carrier.

### BL-38 — the librarian guard is blind to any artifact whose frontmatter omits `id:`
**done** — fixed by teaching it the `entry_prefix` ledger declaration. Its "26 of 66 unprotected" framing was later retracted, and the `id:`-stamping remedy it suggested was reverted in `bb9a94d7`.

### BL-39 — the two sanctioned entry formats are not equivalent
**done, archived** — a params-rendered index defines no citable token. All steps shipped, `doctor` verified (`ledger_defines_nothing` 10→2, `entry_without_definition` 3→1). The blocking peer file committed and was archived; the bug itself moved to `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (id re-keyed `d34dfcd2cc718bd8` → `9dc28c0860b214d9`), all live citations re-pointed in the same commit.

### BL-40 — every drift check asks whether the body kept up with params, never the reverse
**done** `87f3b936` — `params_behind_body` in `doctor`, same two sets subtracted the other way. Ids only; the message names `append_entry` and says explicitly not to re-render from params, because that is `snapshot_drift`'s remedy and here it would delete the newer record. Deliberately **not** gated on `body_keeps_snapshot` — that gate is right for the row question and would silence a body id the catalog has never seen, which is the whole finding. Also extracted `params_backed_ledgers`, since this would have been the third hand-rolled copy of a 45-line preamble shared by three checks that must agree on what a ledger is.

Found by nearly publishing from the stale side: `windows-platform-support.md` had 29 params rows against 35 in the body, with two statuses stale, and `append_entry`'s `warning` was the only surface that could see this direction — only during an append, which that ledger had not had since the divergence. On its first live run the check fired **twice**: the WIN case, and `mirela/…/solver-invariants.md` at 10 of 68, a different repo no surface had ever reported. Data repair for both → **BL-42**.
### BL-41 — link_scan's dangling count is prefix-gated, so a whole namespace can read as healthy
**done** `ff088630` — 129 dead `WIN-N` citations moved the project total by zero. The gate is right in intent (it suppresses `CI-2`-shaped prose) and wrong in discriminator: it cannot tell "not a namespace" from "a namespace that is wholly broken". Fixed by gating on the `entry_prefix` **declaration** as well as on observed definitions.

The wiring is the load-bearing choice and it is not the one the bug file proposed: the declaration rides on `DocExtract.declared_prefixes`, populated inside `extract()`, which already holds the whole file text including frontmatter — so **there is no wire for a caller to forget**. Threading it through `DefinitionIndex::build` would have made an omission a silent no-op and touched 11 test call sites. Pinned by an end-to-end test that runs a real row-only body through the real extractor into the real index, the only one of five that fails if either half drops its end.

**Still open on this entry:** the predicted dangling RISE is unmeasured. `link_scan` runs in the MCP server and has no CLI, so it needs `cargo rb` + `/mcp` first. Pre-fix baseline, taken on the old binary right before the rebuild: dangling **548**, ambiguous 410, 3,649 citations, 1,055 artifacts, `edges_added` 0.

### BL-42 — DATA REPAIR: rows live in a body and in no params row, so params-based queries miss them
**open, scoped to codescout 2026-08-18.** Surfaced by BL-40's `params_behind_body` on its first live run, so this is the fix proving itself rather than fresh breakage.

**Ours:** `windows-platform-support.md` WIN-30, WIN-32..WIN-36 (6 of 35). Left stale on purpose at an earlier session close so the new check had something true to find; that duty is discharged.

**Corrected on two counts 2026-08-18, both found by trying to execute the repair rather than by re-reading the filing.**

- **Severity is LOWER than filed.** The heading above and the original text said the ids could be reissued. They cannot, today: `append_entry` allocates `params_next.max(body_max + 1)`, and `body_max` folds in the very rows this check reports — it would mint `WIN-37`. `append_entry`'s own comment says so outright (*"Folding the body's max in makes the reissue impossible instead of silent"*). Reissue becomes possible only after a compaction moves those rows to an archive companion, and this ledger carries no committed `entry_high_water_WIN`, so that is the real risk — conditional, not current. The present harm is query invisibility plus a committed body that disagrees with the catalog.
- **Difficulty is HIGHER than filed.** It is not 6 `append_entry` calls. Neither `append_entry` (which overwrites `entry["id"]` with its own allocation) nor `update_entry` (which patches an existing row and never changes the row count) can create a row at a GIVEN id. The repair needs the wholesale write — `artifact_augment(merge=true, params={issues: […all 35 rows…]})`, or the CLI's `--params @<file>` past the inline budget. **Build the full array programmatically, not by hand:** a params patch replaces the collection, and re-typing 29 existing rows is exactly the transcription risk that took another tracker from 19 entries to 1.

Verify: `doctor` `params_behind_body` 2 → 1.

**Handed off:** `mirela/backend-kotlin/docs/trackers/solver-invariants.md` SI-59..SI-68 (10 of 68) — never reported by any surface before BL-40. Same two corrections apply to it. Recorded here only so the finding is not lost with this queue's scope change; it will be solved in that repo.

**Codescout half DONE 2026-08-18.** Built the full 35-row array programmatically — the existing 29 read back from `params`, the 6 missing rows parsed mechanically from the body's own `## Issue index` table (never hand-retyped) — and wrote it via `artifact_augment(merge=true, params_path=...)`. Verified `doctor` `params_behind_body` 2 → 1; the remaining hit is mirela's, per the handoff above.

**A second, distinct defect surfaced while building that diff.** Comparing the body table against params row-for-row (not just by id, but field-by-field) found **7 rows present on both sides with different content**: `WIN-1`, `WIN-4`, `WIN-5`, `WIN-20`, `WIN-27` had a stale pre-archive `ref`; `WIN-28` and `WIN-29` were worse — `params` held an earlier `open` snapshot with a superseded root-cause summary, while the body already carried the resolved `fixed` story. A canonical `entry_filter={"status":{"eq":"open"}}` query — the exact pattern this tracker's own augmentation prompt recommends — would have surfaced two closed issues as open, with the wrong explanation. Repaired via 7 `update_entry` calls (safe: every row already existed, no wholesale rewrite needed) and re-verified by diffing all 35 rows field-by-field — zero mismatches remain. Filed separately as `docs/issues/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md` (id `2a4d51f2e0521468`, `mitigated`): the seven rows are fixed, but nothing detects this class of drift — a row present on both sides with disagreeing fields — so it can recur the next time a body table is edited without a matching `update_entry`.
### BL-43 — complete BL-41's coverage: an undeclared, undefined ledger is still invisible
**dropped from this queue 2026-08-18 — handed off, not withdrawn.** Both remaining targets are other repos (stefanini `CR`×8, researcher `T`×2).

**Codescout's half is already done.** `entry_prefix` is declared on all four ledgers backfilled in `c7bdfd22`, and every one of the nine prefixes declared in this repo now has at least one defining heading. Nothing here is affected.

**The finding stands** and lives in the BL-41 bug file: a ledger that declares no `entry_prefix` AND defines nothing is still invisible to the dangling gate, so BL-41's retrospective coverage is incomplete by construction — measured, not predicted. Its prospective value is unaffected: the next ledger created here with `entry_prefix` and row-only entries dangles loudly.
### BL-43 — complete BL-41's coverage: an undeclared, undefined ledger is still invisible
**open** — measured 2026-08-18, and it is the honest counterpart to BL-41's result. BL-41's marginal effect on the current corpus is **zero**: all nine declared prefixes (`GF`, `CAP`, `U`, `H`, `FND`, `T`, `R`, `SD`, `HY`) now have at least one defining heading, and `SD`/`GF`/`FND`/`T` got their declaration and their headings in the *same* commit (`c7bdfd22`), so the widened gate never had a case to fire on.

The surviving hole is concrete rather than hypothetical: `stefanini/…/june-fixes-review-followups.md` holds 8 `CR` entries, defines none of them, and declares no `entry_prefix` — so a wholly-broken namespace is **still** invisible, which is the exact defect BL-41 reports, surviving its own fix. The bug file predicted this limit ("improves coverage without completing it"); it is now instantiated.

Remedy is one frontmatter line per ledger. Both targets are outside the working dirs of the session that shipped BL-41, so it needs the owner's go-ahead — and it shares its two files with BL-39 step 4's remainder, so doing them together makes one citation sweep instead of two.
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

### 2026-08-18 (later) — BL-40 and BL-41 shipped, step 4 taken from 10 open ledgers to 2

Worked the order the previous entry designated, and it was the right order for the reason it
gave: BL-41 changes what the dangling number means, so measuring the backfills first would
have scored them against a baseline about to move.

**BL-40 — `params_behind_body` (`87f3b936`).** The inverse of `snapshot_drift`: same two sets,
subtracted the other way. Two decisions each pinned by a test rather than a comment — not
gated on `body_keeps_snapshot` (that gate would silence a body id the catalog has never seen,
which is the entire finding), and the message names `append_entry` while explicitly forbidding
a body re-render, because inheriting `snapshot_drift`'s remedy here is data loss rather than
noise. Six tests, five watched fail on their assertions; the sixth passes on a stub **by
design**, since it asserts the check stays silent on a lagging body and only discriminates once
the code can subtract backwards. Refactored the 45-line preamble the three entry-drift scans
share into `params_backed_ledgers` — this would have been the third copy, and three checks that
must agree on what a ledger *is* drifting apart is the failure mode this whole family is about.

**It paid off on first contact with the real catalog**, which is the step distinct from the
gate: fired twice, neither vacuous. The WIN near-miss it was filed for, and
`mirela/…/solver-invariants.md` at 10 of 68 — a different repo, never reported by any surface,
with ten ids that were never allocated. Split to **BL-42**.

**BL-41 — declared prefixes widen the gate (`ff088630`).** The wiring differs from what the bug
file proposed, and deliberately: the declaration rides on `DocExtract.declared_prefixes`,
populated inside `extract()`, which already holds the whole file text. There is therefore **no
wire for a caller to forget**; threading it through `DefinitionIndex::build` would have made an
omission a silent no-op across 11 call sites. Its known cost is inherent — once a prefix is
known, prose that merely looks like a token is reported too, and there is a measured instance
(`bug-fix-session-log.md:467` says "a parallel session's T-13 commit", an old plan's task
numbering). Right trade: a false positive costs one glance, the false negative cost 129
silently-dead citations.

**Step 4 (`c7bdfd22`) — 49 entries made citable across four codescout-local ledgers**, each
shaped to what its ledger needed rather than one template. `SD` had no table and no sections,
so its whole substantive record lived only in the git-ignored catalog. `GF` is a dated snapshot
whose prompt forbids rewriting sections while telling readers to read a section that did not
exist — anchors only, pointing at where its evidence already sits. `FND` had nothing at all for
four of its eighteen claims. Plus mirela's `G`×6 and `OTK-35`, left **uncommitted** — that is
the user's own checkout, with 9 modified files from a concurrent session on
`feat/year-scoped-catalog`.

**Two beliefs from the previous close turned out wrong when checked.**

1. *"Backfilling `fable-tuning-tasks` would make its `T` tokens ambiguous."* False.
   `tool-usage-patterns` spells its first thirteen entries zero-padded (`T-001`..`T-013`) and
   its later ones `T-14`..`T-24`, and the resolver matches **token strings**, so the spaces are
   disjoint and `T-1`..`T-12` were always safe. The wrong version of this belief nearly blocked
   correct work; filed the real hazard as
   `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`
   (fixed and archived 2026-08-21),
   because the disjointness is a formatting accident that nothing records or enforces, and two
   ordinary edits would break it.
2. The closing report's `dangling` 547 and `ledger_defines_nothing` 8 both read one lower when
   re-measured. A peer session shares this machine-local catalog. Recorded as measured, not
   attributed — the point being that a catalog number is a fact about an instant.

**Next, in this order.**

1. **`/mcp`, then `librarian(action="link_scan")`.** BL-41's central claim is still unverified.
   Report the two effects separately: BL-41 pushes dangling **up**, `c7bdfd22`'s 49 headings
   push it **down**, so a flat total would be two real effects cancelling rather than a no-op.
   `edges_added` is the clean read on the backfill half. Do not archive BL-41 before this.
2. **BL-42**, `solver-invariants` first — id reissue is silent corruption, and it needs a
   confirm because that checkout has concurrent work.
3. **researcher `T`×2 and stefanini `CR`×8** — the last two `ledger_defines_nothing`. Both are
   **outside this session's working dirs**, so they need the user's go-ahead, not just a
   decision. Read the prefix-`T` bug file before touching researcher's: giving its `T-1`/`T-2`
   headings is precisely what makes `prompt-hamsa-audit-log`'s bare citations ambiguous, and
   the fix then is to qualify them as `fable-tuning-tasks:T-N` in the same commit.
4. `provenance-subsystem`'s 42 uncited `PV` stay undefined on purpose — only 22 of 64 entries
   are cited, and the ledger's own convention is narrative only where a row is insufficient.
#### Addendum — measured on the wire, and BL-41's prediction was wrong

`cargo rb` + `/mcp`, then verified the **server process** was running the new code rather than
trusting that a build existed: `librarian(action="doctor")` reported `params_behind_body`, which
is BL-40's check and shipped in the same binary. Its counts matched the CLI exactly.

| metric | pre-fix | post-fix | Δ |
|---|---|---|---|
| `dangling` | 548 | **471** | **−77** |
| `ambiguous` | 410 | 411 | +1 |
| `edges_desired` | 860 | 895 | +35 |
| `edges_added` (write=true) | — | **44** | — |
| `citations` | 3,649 | 3,718 | +69 |

The 44 edges are materialized; the graph now holds **895** `cites` edges, which is a measured
absolute rather than a running sum across sessions.

**BL-41's bug file predicted the dangling total would RISE, and called that "the fix working".
It fell by 77, and BL-41's contribution to the number is ZERO.** Not because the fix is broken
— because it keys on the *declaration*, and no prefix in this corpus is any longer declared and
wholly undefined. Verified by inventory, not inferred: nine declared prefixes, every one with
≥1 defining heading. The four ledgers that WERE wholly undefined received their declaration and
their headings in the same commit, so the widened gate never had a case to fire on. The −77 and
the 44 edges belong entirely to the backfill.

Recording the mismatch rather than reframing the prediction to fit the result. Two things follow.
First, `edges_added` really is the clean read on backfill progress — as the file argued, and
**`edges_missing` is its dry-run twin** (`write=false` leaves `edges_added` at 0, which is easy to
misread as "nothing happened"). Second, BL-41's value is prospective: the next ledger created with
`entry_prefix` and row-only entries dangles loudly instead of silently. Its retrospective coverage
is incomplete, concretely so — filed as **BL-43**.

Both bug files are left `fixed` and **NOT archived**, deliberately. BL-43 and BL-39's step-4
remainder target the same two out-of-scope ledgers; archiving now would bury BL-43's rationale in
a file nothing re-reads and force the 11-citation sweep (3 of them in Rust source) twice instead
of once.
#### Scope decision — 2026-08-18: cross-repo trackers leave this queue

Explicit call: **other repos' trackers and issues are solved in those repos, properly.** This
queue is codescout's. What that changed, and what it deliberately did not:

| item | disposition |
|---|---|
| BL-39 step 4 | **complete for codescout** — no codescout ledger reports `ledger_defines_nothing`; researcher `T`×2 + stefanini `CR`×8 handed off |
| BL-42 | **split** — the 6 WIN rows are ours; SI-59..SI-68 handed off |
| BL-43 | **dropped, handed off** — both targets are elsewhere; codescout's own declarations already done |
| mirela G×6 + OTK-35 | left uncommitted in that checkout for its owner; pure additions, 41 + 8 lines |

Two things this does NOT mean. The **findings** are not withdrawn — each is recorded above with
its measurement, because a scope change should not be able to erase evidence. And the
**codescout-owned halves of cross-repo findings stay ours**: the `frontmatter_id_mismatch`
defect lives in `doctor.rs` and `mv.rs` even though every affected file is in another repo, so
it is fixed here against synthetic fixtures.

This also unblocks the archive sweep that was deferred. BL-39, BL-40 and BL-41 were all held
`fixed`-but-unarchived for one reason — BL-43 and step 4's remainder shared their two out-of-scope
files, and archiving early would have buried the rationale and forced the 11-citation sweep twice.
With the remainder handed off, all three can archive in one pass with one sweep.
### 2026-08-18 (session close) — the citability chain, and what a later session should pick up

One long session. Written here rather than in any `next` field because **`next` lives only in the
catalog, which is machine-local and git-ignored** — the defect BL-29 names, and the reason a
session's own record has to be committed prose.

**Closed.** BL-13 (IL1 wording measured as hamsa A-25 and REFUTED — reverted at `32b34efa` behind an
inverted guard test, bug archived). U-44 (companion IL3 warn-hook — fix was already shipped in
1.16.9; verified across all three profiles and on the wire, and the closure found `docs/architecture/companion-plugin.md`
wrong in **every** row, since the 1.14.0 `.sh`→`.mjs` port was never swept there). The
`tracker-conventions` id-stamping bug (`450df27edcfe9c08`), whose real origin turned out to be
`tracker_design`'s **archetype defaults** rather than the guide.

**BL-39, steps 0-3 and 5 shipped.** `de4df2cd` `body_defined_indices` → `f19d5296`
`undefined_in_body` on both entry-writing tools → `758b37dc` the two `doctor` checks → `d3c1e6ed`
guide + `librarian.md` + the archetype defaults. Step 4 (backfill) is **4 of 13 ledgers**: WIN
(`f04e4c17`), BL (`0d101eb8`), PV (`f5f602e6`), A (`9703102c`). Project dangling **621 → 547** and
**67 edges** materialised.

**Filed this session and NOT yet worked: BL-40 and BL-41.** Both were found by doing step 4 rather
than by looking for them, and both are cheap:

- **BL-40** — no check sees params falling behind a body that ran ahead. Nearly published two wrong
  statuses and dropped six entries from `windows-platform-support.md`.
- **BL-41** — `link_scan`'s dangling count is prefix-gated, so a wholly-undefined namespace reads as
  healthy. 129 dead `WIN-N` citations moved the total by zero.

**Read BL-41 before trusting any dangling number**, including the 621 → 547 above. Which metric
moves depends on whether the prefix already had one definition: `edges_added` measures
`ledger_defines_nothing` progress, `dangling` measures `entry_without_definition` progress, and
`doctor` measures both. That was established by prediction across four backfills, not in hindsight.

**Next, in this order.** BL-40 and BL-41 first — both are small, and BL-41 changes what the numbers
mean for the rest of step 4. Then the remaining codescout-local ledgers by external citation count:
SD + GF (~36), `fable-tuning-findings.md` (FND, 19). **`fable-tuning-tasks.md` is a prefix decision,
not a backfill** — its `T` prefix is already defined by `tool-usage-patterns.md` (T-001…T-24), so
adding `## T-N` headings there makes those tokens *ambiguous* rather than resolved. Four ledgers in
`researcher` / `mirela` / `stefanini` need their own workspaces activated and are a separate call.

**Two standing constraints for whoever continues.** `tracker_design`'s SYSTEM_PROMPT has **~102
bytes** of inline headroom, down from ~640 — treat it as zero, and `default_response_fits_inline` is
what will say so. And `body_claimed_indices` also counts a heading inside a fenced block and a
code-first `` `A-3` `` heading, so the *claimed* set is not merely "defined plus rows".

### 2026-08-18 — the shell-gate cluster closed, and the snapshot had drifted on six rows

**Five bugs closed across two sessions**, none of which had a BL row — they were filed
and fixed inside a day, faster than this queue turns over. `find`'s silent `count: 0`
(`9d386566`), `chunk_id` collapsing duplicate chunks (`933af744`), the IL3 heredoc gap
plus `find` dropping `rel_path` (`4fad1aa4`), the newline separator in both gates
(`308014b5`), `artifact(get)` returning bare `null` (`9a71357e`), the source gate
counting relative tokens as in-project (`be2d7781`), and worktree reads answering about
the old checkout (`dd788ce1`). The IL3 advisory hook was deleted outright in
`codescout-companion` 1.16.9 rather than corrected. All recorded in `CHANGELOG.md`
(`fa107a6a`) — which had itself gone 27 commits without an update.

**The snapshot was wrong on six rows.** BL-6, BL-7, BL-8, BL-10 and BL-25 read `open`
here while params said `done`; BL-16 read `blocked` while params said `open`. So the
committed file — the only copy that exists in git — advertised **eight** open items when
four were live. That is BL-29 again, and worth stating plainly: every fix in this queue's
history that flipped a row with `update_entry` aged this table by one row, and the drift
is invisible until someone diffs params against the body. The check is
`artifact(get, id="9a892c2a5976e296", entry_filter={"status":{"ne":"done"}})` — run it
before trusting any "what's open" read of this section.

**BL-38 added** for the ledger-aware librarian guard, which had a 770-line plan
(`8a793791`, at `docs/superpowers/plans/`, not `docs/plans/`) and no row. It is the only remaining open bug whose defect is still actively
producing damage rather than waiting on a decision.

**BL-13 is no longer merely blocked.** The A-25 pre-registration landed (`e2fbefe2`) with
a numeric ship/no-ship rule fixed *before* either arm runs — ship the clause only if arm A
base is >= 3/10 and arm B clause is <= 1/10; arm A <= 1/10 is a ceiling and the 57
characters get reverted. What remains is running the arms in `prompt-engineering`, not
deciding anything here.

**Verify-open pass run on the two rows whose bugs were absent from the open-bug query.**
BL-16 (`403e3fad0356f171`) and BL-29 (`0694a4a9946e10fe`) are both genuinely `mitigated`,
not zombie-open — clean, against this project's measured 75% zombie rate.

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

### 2026-08-16 — BL-1 fixed; BL-19 filed; the queue was not actually in git

**BL-1** — `[*]` now parses and projects, the recovery hint is derived from the payload's shape
instead of the constant `$.field`, and both rejection hints plus
`get_guide("progressive-disclosure")` advertise the grammar. Verified live: the exact call that used
to be rejected, `read_file("@tool_…", json_path="$.augmentation.params.tasks[*].id")`, returned all
18 BL ids from a buffered handle.

**BL-19** — filed from a complaint about the fix's own output. The hint is now correct, but the
*envelope* still costs a whole call to return nothing: `artifact(get)` answers with `output_id`, a
byte count and a hint, and nothing about the artifact. The librarian adapter's `format_compact` has
exactly one case — a body-truncation warning — so every other response falls through to the generic
"Result stored in …". Fixing the hint makes the second call land; it does not make it unnecessary.

**And this file was not what it appeared to be.** The BL rows are `params`, and params live in the
librarian catalog under `~/.local/share/`, **not in the repo**. The markdown carried frontmatter and
prose only — so the queue existed on this machine and nowhere else, which is the opposite of why a
tracker was chosen over Claude Code's per-profile memory in the first place. The rendered snapshot
above fixes that. Worth knowing when creating any augmented tracker: writing a good body does not
make its live state durable, and the file will not look wrong.

**BL-20** — filed from an own goal committed while writing the line above. Flipping BL-1 to `done`
via `patch={params:{tasks:[one row]}}` deleted BL-2..BL-19: merge-patch replaces arrays wholesale,
and the call answered `updated: true`. The rows survived only because the snapshot had been written
minutes earlier, for an unrelated reason.

That near-miss is the argument for the snapshot, independent of git: **params have no shrink guard,
no `force` gate, no report of what a write destroyed, and no version control** — all four of which
the body surface has. The less-protected surface is the one with no backup. Keep a rendered table in
the body of every entry-bearing tracker.


### 2026-08-16 — six fixed and archived; the queue's own tooling was most of the work

**Shipped, all on `experiments`, all fast-forward (so each SHA *is* the master SHA):**

| BL | what | SHA |
|---|---|---|
| BL-1 | `[*]` projection + payload-derived hint (+ depth walk) | `7c91cdf7`, `336d3b04` |
| BL-20 | `update_entry` — entry-grain patching; always-on entry counts | `02a87a83` |
| BL-21 | librarian guard hoisted into `read_edit_target` (all 3 write paths) | `47abcb6d` |
| BL-22 | `move` grafts history onto the new id instead of stranding it | `2d8c7f39` |
| BL-26 | `librarian-runtime` guide corrected + guard test over every guide | `6018b7ad` |
| BL-27 | `entry`-param guard fires whenever `entry` is present | `6018b7ad` |

Plus `61ab520a`: **14 `fixed`-but-unarchived bug files archived**, and three deliberately
left open because their Resumes describe real undone work —
`audit-doc-refs-gate-hides-its-own-cause`, `edit-code-remove-ast-repair-over-deletes`,
`workspace-toml-mis-rooted`. Those three were indistinguishable from the other fourteen by
status, path or age; only reading the Resumes separated them.

**BL-22 is the one to understand before touching the catalog.** `move` used to preserve an
artifact's id while changing its path, which breaks the invariant `doctor.rs` states twice
(`id == artifact_id_from_abs(abs_path)`). The next `reindex` then re-keyed the row and
`upsert`'s abs_path pre-clean cascade-deleted its events. Measured: one reindex took the
catalog from **1845 to 1834 events while reporting `removed: 0`**, and archived bug files
carried 0.02 events/row against 0.65 for live ones. Fixing it is what made the 14-file
sweep above safe to run at all.

**Two guards were written to the reproduction rather than the condition**, hours apart:
`edit_file`'s covered 1 write path of 3, and `update_entry`'s read
`entry.is_some() && fields.is_none()` so sending both dropped `entry` again. Both were
caught by the other session. The test shape that catches this class is a table containing a
row that was **green before the fix** — it proves the table discriminates rather than
refusing everything.

### 2026-08-16 — BL-2 fixed; the filed root cause was understating it

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-2 | grep stops printing a denominator the walk never counted | `4b77dff5` + `358f1ced` |
| BL-31 | filed — found while fixing BL-2 | (open) |

**The filing was wrong about scope, and the reconnaissance is what caught it.** BL-2 was
filed as a corner case: "N of N" appears *when* `budget >= total`. Tracing `max` from its
single binding showed it serves as **both** the collection break threshold and
`cap_grouped`'s display budget, so `visible.len() == total` unconditionally and the second
disjunct of `truncated = hit_cap || total > visible.len()` is **dead code**. The hint had
never printed anything else. Had the fix been written to the filing, it would have
branched on a condition that cannot occur.

**Three surfaces, not one.** `format_overflow` already rendered the honest `… showing
first N` when `shown == total`; grep's own hint then re-asserted `Showing N of N` right
after it, under a header stating the count as fact. The one true clause was the shortest
and sat between the two that overrode it. Fixing only the hint would have left the header
— the line a reader anchors on — still wrong.

**The test shape, third instance this session.** Two rows over one corpus, capped and
complete; the complete row was **green before the fix**. That is the only shape that can
catch a defect whose symptom is that two renderings are byte-identical — asserting on the
capped string alone passes against the bug. Same shape as the `edit_file` guard (1 write
path of 3) and the `update_entry` guard (1 input shape of 2).

**Verified by invoking, not by inspecting** (T-20). `cargo test` proved `Grep::call` →
`format_compact`; only the live MCP call proved the running server serves it:

```
grep(pattern="^use serde_json::json;", glob="src/**/*.rs", limit=5)
5 matches (capped) in 5 files
  … showing first 5 — Collection stopped at limit=5, so the true total is unknown …
```

**BL-31 fell out of the same reading.** Because collection stops at `max` in walk order,
`cap_grouped`'s round-robin never runs from grep. Live proof, same session: a capped
search offered three files holding **one match each** as ways to narrow a result already
capped at five — advice that is not merely unranked but inert. Two capped searches before
it returned `across 1 files`, so the common capped result is one file's worth, which is
also why BL-2's new `(capped)` header marker is invisible in the common case. The two
defects interact; fixing BL-31 restores the marker.

**Shared-tree hazard #2.** `cargo fmt` formats the whole crate, and it was run while the
concurrent session had uncommitted edits under `src/librarian/tools/audit_doc_refs/`. Any
reformatting of their in-progress files landed in *their* diff. Pathspec-scoped commits
protect the index; they do not protect against a whole-crate formatter. Prefer
`cargo fmt -- <file>` on a shared tree.

### 2026-08-16 — BL-33: the convention was hiding the answer

| BL | What shipped | SHA |
|----|--------------|-----|
| BL-33 | guard keys on what is actually managed, not on YAML quoting | `29f0c015` |

**Filed as a quoting bug; fixed as a predicate bug.** `is_librarian_artifact` tested the
frontmatter text for a 16-lowercase-hex `id:`, and a quoted `'…'` is 18 characters, so
protection depended on which serialiser last wrote the file — 12 guarded trackers, 15 not,
the queue among the unguarded. The quoting fix alone closes 86 files repo-wide.

**The user's course-correction is the entry worth keeping.** My first pass concluded the
id-less trackers were *not* a defect: the stamped id looked like an intentional opt-in
marker, and a catalog-backed guard would refuse `edit_markdown` on
`docs/trackers/skill-frictions.md`, which CLAUDE.md documents. That reasoning deferred to a
convention instead of testing it — *"lets correct it properly not follow a rule written 10
years ago"*. Two queries then settled the design:

```
docs/RELEASE.md, CONTRIBUTING.md, PROGRESSIVE_DISCOVERABILITY.md, TAXONOMY.md, ROADMAP.md
  -> ALL catalog rows                       (so membership is the WRONG predicate)
artifact(find, augmented=true, scope="repo")
  -> count: 16, all trackers                (so augmentation is the RIGHT one)
```

Augmentation is exactly the set where state lives *outside* the file. It keeps
`skill-frictions.md` editable — but now because that is correct, not because it is written
down — and it catches `artifact-augmentation-followups.md`, augmented with **no** frontmatter
id, which no amount of string-parsing could ever reach.

**Cost of the naive route, measured before rejecting it:** the core `ToolContext`
(`src/tools/core/types.rs`) has no catalog handle, and adding a field means editing **124
construction sites across 21 files**. A trait object installed once at server construction
costs one line in `server.rs`. `guard_with_oracle` takes the oracle explicitly so no test
installs into the `OnceLock` and none can poison another in the same binary.

**Verified live on four cases**, the two negatives mattering as much as the positives:
augmented-with-no-id refused, augmented-with-quoted-id refused, prose tracker readable,
`docs/RELEASE.md` readable.

**A closing loop.** This queue is now guarded against the direct `read_markdown` that
T-22 caught me making — the observation and the fix landed in the same session, and
`artifact(get, entry_filter=…)` is now the only way in.

### 2026-08-16 — BL-34 fixed, and the corpus proved something the unit tests could not

`858f22ec` — `frontmatter::replace_id_line` splices the `^id:` line instead of round-tripping
the block through `parse` → `write`. `mv::repair_frontmatter_id` now reads through the parser
(authoritative about *whether* a repair is needed) and writes through the splice (authoritative
about *which bytes move*). When they disagree — a folded or flow-mapped `id:` the line scan
cannot see — it warns and leaves the file alone. Declining beats reformatting: a stale id is a
broken citation, a reformat is data loss.

**Verified live against the named regression corpus**, `/home/marius/work/mirela/eduplanner-ui`
(reverted on purpose 2026-08-16 precisely so it could serve as one):

| | before | after |
|---|---|---|
| files | 26 | 26 |
| diff | 402+ / 370− | **26+ / 26−** |
| lines/file | 30 | **1** |

Every hunk in `git diff -U0` is `@@ -2 +2 @@`, and every `−`/`+` line begins with `id:`. Both
ADR/FDR templates are byte-identical outside that line — `created: {YYYY-MM-DD}` intact.

**Three things worth carrying forward.**

1. **The corpus caught a class the unit tests structurally could not.** `title: "{Title}"` and
   `category: a | b | c` are shapes a *round-trip* mangles and a *splice* never sees. The fix
   is not "handle more YAML shapes correctly" — it is "stop reading shapes you have no business
   rewriting." A narrower contract needs fewer cases.

2. **Fourth instance this session of a test that exercises the mechanism but not the call
   site.** `move_rewrites_the_frontmatter_id_it_just_invalidated` was green through this entire
   defect, because it only ever asserted *that the id changed*. When a fix lands in a shared
   helper, drive at least one real call site — and mutation-verify that one, since it is the
   only test that can fail for the right reason.

3. **A read-only precondition check is cheap and buys the apply.** Before writing into a
   foreign repo, `grep -c '^id:'` over the 26 flagged files confirmed every one would take the
   splice path rather than the decline path. The four files the doctor did *not* flag
   (ADR-0023, ADR-0024, two READMEs) have no `id:` at all — the abstention branch, visible for
   free in the same result.

The 26-line repair is left **uncommitted** in `eduplanner-ui`'s working tree. It is the correct
repair now, but committing into another repo is not this session's call.
### 2026-08-17 — BL-35 fixed

Swapped `guard_worktree_write`'s gate from `is_project_explicitly_activated` to
`is_project_chosen_this_session` (option 1 from the bug's own three) — the
maintainer's call, made directly, accepting that this repo (two live
worktrees at the time) starts refusing un-activated writes immediately.
Three regression tests added (`src/tools/core/tests.rs`), reusing the
`seed_linked_worktree` / `rooted_ctx` fixtures the read-side notice fix had
already built — including one pinning the case that matters most for a
re-armed refusal: no worktrees, never activated, still allowed. Bug archived
(`1523556488a95de2` → `a742a50ea6723daf`); citations of the old path in
`src/tools/core/types.rs`, `guards.rs`, `tests.rs` re-pointed in the same pass.
### 2026-08-17 — BL-14, BL-15, BL-37(partial); and `experiments` was red at a clean tree

Compaction handoff for the BL-N run. Everything below is on `experiments`; gate green at
`9ad5818d` — `cargo fmt --check` clean, `clippy --all-targets -D warnings` clean,
`cargo test` **4055 passed / 0 failed / 45 ignored**.

**Closed**

| Item | Fix | Archive |
|---|---|---|
| BL-15 / B-6 — read-only metadata blocked on source paths | `90c5aea1` | `c4211712` |
| BL-14 / B-1 — `force=true` discarded in silence on whole-file reads | `2703410e` | `201628f9` |
| `frontmatter_max` dropped at the MCP boundary | `4cdafd9a` | `8a91e950` |

**BL-37 — partial.** Interim priority-ordered trim shipped (`30f3df81`; note shortened
`9f4807ef`). Open on the **carrier decision alone**, which is the maintainer's.

**Fixed outside the queue**

- `bf485a00` — `append_entry`'s hint asserted `##` for every ledger; now derived from the
  body (mode of the existing entry headings). `docs/PROGRESSIVE_DISCOVERABILITY.md` gained
  **Anti-Pattern 5 — Asserting a Convention the Tool Never Read** as the standing rule.
- `fd9e63d0` — Iron Law 3 describes two gates three paragraphs apart that share vocabulary
  (`wc` appears in three roles on one page); signposted.
- `9ad5818d` — **`experiments` was red at a clean tree.** `90d76d8a` re-armed
  `guard_worktree_write` deliberately and left `tests/integration.rs` asserting the removed
  behaviour. Inverted rather than deleted; the odd-looking
  `assert!(is_project_explicitly_activated)` is now the point, not the premise.

**Filed** — `audit_doc_refs` `include_str!` false positive (fixed by a peer, `da55100a`);
`/mcp` refreshes schemas but not `server_instructions`; U-40 (`old_string` "not found" cannot
distinguish a bad needle from a bad haystack); U-41 (`snapshot_stale` overclaims); R-101, R-102.

#### Standing hazards for the next session — read before writing anything

1. **Writes now require an explicit activate in this repo.** `90d76d8a` re-armed the
   worktree guard on `is_project_chosen_this_session`, and this checkout has two linked
   worktrees, so the first `edit_code` / `edit_file` of a session is **refused** until
   `workspace(action="activate", path="/home/marius/work/claude/codescout")`. Hit live on
   2026-08-17. Working as designed; not a bug.
2. **A peer session shares this tree.** It broke compilation three times mid-turn
   (`audit_doc_refs`, `link_scan`), so `cargo test` is not always available. Commit by
   **pathspec**, never `git add -A`; use `cargo fmt -- <file>` rather than whole-tree fmt.
   A full-suite red is worth attributing before assuming it is yours.
3. **`cargo test --lib` does not build `tests/integration.rs`** — memory
   `cargo-test-lib-skips-integration`. That is exactly how a deliberate behaviour change
   left HEAD red for ~40 minutes.
4. **The instructions surface cannot be verified from the session that renders it.** The
   workaround, and it works: speak MCP to the release binary directly —
   `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' | ./target/release/codescout start --project .`
   That is how `30f3df81` was verified on the wire.
5. **Reindex before any `artifact(find)` you will act on.** Measured twice today: the
   catalog lagged disk by one file, and a lagging catalog and an empty filesystem return
   byte-identical answers.

#### Resume

Nine bugs open. Next by evidence quality: **`artifact(find)` is silent about unindexed
files** — fix shape settled by precedent (make the zero self-describing, as `grep`'s
hidden-path skip was), needs care because `find` is on the hot path. Then the **BL-37
carrier decision**, which is a design call rather than an implementation.

Promotion is **not** blocked by any of the above, but is not gated either: `0 941`
fast-forward available, code gate green, and `docs/RELEASE.md`'s step-4 documentation gate
is unrun — the CHANGELOG / mdBook / README checks, plus the audit re-run against a **fresh
clone**, which the file warns is the one green not to believe from a working tree.
### Resume — state at compaction, 2026-08-16

**15 of 35 rows open.** Phase 1 remaining: **BL-4, BL-19** (BL-29 is phase 1 but the other
session's). Phase 2: BL-9, BL-24, BL-30, BL-31, BL-35. Phase 3: BL-13, BL-14, BL-15, BL-16,
BL-28, BL-32. Phase 4: BL-17.

BL-29/BL-30/BL-35 belong to the concurrent session. **BL-4 and BL-19 are the natural next** —
BL-19 is a progressive-disclosure sibling of the BL-2 fix (incompleteness signals on buffered
output).

**Working practices, carried forward:**

- **Read the filing as a hypothesis, not a spec.** BL-2's filed root cause was narrower
  than the defect; the fix would have been written to a branch that cannot execute.
  Trace the variable from its binding before implementing the fix a bug file prescribes.
- **The catching test contains a row that was green before the fix.** Four for four
  this session on guard/message defects — BL-34 made it a helper-vs-call-site variant.
- **When the fix lands in a shared helper, drive at least one real call site**, and
  mutation-verify that test specifically. The helper's own tests cannot prove the caller
  reaches for it.
- **Verify a fix is live by invoking it**, never by inspecting the binary or
  `codescout_sha` (T-20).
- **Archive with `artifact(action="move")` and re-point ids in the same commit** — read
  `id_changed`. An id that returns `count: 0` from `find` is archived, not deleted.
- **Check the snapshot against live params before trusting a row** (BL-29). Three rows
  had drifted here, all from catalog-only writes.
- **A concurrent session shares this working tree, index, and formatter.** Commit by
  pathspec; prefer `cargo fmt -- <file>`. It also shares the **git index**: a peer's
  `git commit` swept a file this session had `git add`-ed into their commit (observed
  2026-08-16, `2f94ce40`). Stage immediately before committing, not early.
- **Bug-file location vs status — the standing check** (BL-17). The invariant is that no
  *archived* bug carries a non-terminal status:

  ```
  artifact(action="find", kind="bug", filter={"and": [
    {"abs_path": {"contains": "issues/archive/"}},
    {"status":   {"in": ["open", "investigating"]}}]})
  ```

  It must return **0** — measured 0 on 2026-08-16. The *inverse* (terminal status, still
  in `docs/issues/`) is **not** a defect and should not be swept: every such file states
  its residual work in `## Resume` — a fixed *damage* with an unfixed *trigger*, a
  mitigation with a root cause still open, a deliberate `open` on an unreproduced
  flake. Archiving those would hide real work to satisfy a tidiness rule. Check the
  Resume before moving anything.
- **Before writing into a foreign repo, run the read-only precondition check first.** The
  dry-run says *which* files; a `grep` on the shape says *which code path* they will take.
  Both are free, and together they make the apply reviewable in advance (BL-34).

### 2026-08-18 (later) — BL-38 fixed, and two of its three pieces were already shipped

`f4db4e9c` and `9ac00440` (**experiments**) closed BL-38. The interesting part is what
reconnaissance found before any code was written: the plan's three-piece design stood in
three different states.

Pieces 1 and 2 — declare `entry_prefix` in frontmatter, wire `allocate_entry_id` to the
tool surface — were **already shipped**, the latter at
`src/librarian/tools/append_entry.rs:91` and hardened since by three follow-up fixes. The
bug file said "NOT yet called from any MCP tool"; true at `540c29c3`, stale by the time it
was read.

Piece 3's goal shipped: `declared_entry_prefixes` plus a third arm in the guard's union, so
a ledger is guarded by its declaration rather than incidentally. All five ledgers that
exist today were *already* guarded — two by a stamped `id:`, three by augmentation — so the
protection was accidental, not principled. The reachable hole was proved end-to-end rather
than argued: a scratch ledger with `entry_prefix: ZZ` / `entry_high_water_ZZ: 3` / no `id:`
/ not augmented accepted a hand-written `## ZZ-4` heading via `edit_markdown` and left the
mark at 3, which is the input compaction later reads back to reissue `ZZ-4`.

Piece 3's *mechanism* was **cut**. The heading-scoped guard existed to answer the objection
that a whole-file guard makes a typo fix in a 2,800-line tracker a ceremony — and that
premise is false. `artifact(update, patch={body_edits: [{heading, action: "edit",
old_string, new_string}]})` is already a section-scoped swap and works on any catalog row,
verified on `skill-frictions.md`, which has neither `id:` nor augmentation. Building it
would have cost a fourth parameter on a public function threaded through three call sites,
for a class of file with zero current members. What survives is hint text routing both
intents.

One defect the plan itself introduced, caught in review: it held the guard's hand-rolled
`entry_prefix` reader and the librarian's `serde_yml` one in agreement with a doc comment.
That is a co-change contract enforced by prose — the shape that cost this project 48
needlessly-compiled crates (`docs/adrs/2026-07-25-embedding-transport-boundary.md`). Now a
parity test over 11 YAML forms.

Follow-up filed rather than folded in — and **fixed later the same day at `d3c1e6ed`**, archived to
`docs/issues/archive/2026-08-18-tracker-conventions-guide-recommends-reverted-id-stamping.md`.
`get_guide("tracker-conventions")` had gone on prescribing stamping `id:` into frontmatter — the
remedy BL-38's own file retracted and `bb9a94d7` reverted — on a surface that auto-injects at the
first `artifact` call of every session, so the disproved advice sat somewhere louder than its own
retraction. It now says *declare `entry_prefix`* instead, which BL-38's fix is what made true.

Chasing it one layer down found the origin: `librarian(tracker_design)`, the surface you are told
to call *before* creating a tracker, shipped the losing shape as an archetype **default** —
`task_list` with no per-entry section at all, `failure_table` calling entry headings "optional",
`constitution` prescribing `## C-N` without the dash-and-title that makes a heading define
anything. The guide was the second-loudest place the shape was taught, not the cause.
