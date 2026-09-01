---
id: '73158c500ff6b293'
kind: bug
status: zombie
title: SDD ledger directory and this work-stream's catalog rows both vanished between sessions
tags:
- cluster/shared-resource-carries-no-owner
- librarian
- catalog
- sdd
- data-loss
closed: null
last_observed: 2026-08-25
opened: 2026-08-25
owner: marius
related:
- docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
- docs/issues/archive/2026-05-17-reindex-cascade-delete-data-loss.md
reopen_trigger: Catalog rows disappear for files that are STILL ON DISK — i.e. a find(scope=umbrella, include_archived=true) reports count 0 for a path that exists, and the response's own unindexed_files hint is non-zero, with no artifact(move) or archive in the window.
severity: high
unverified: 'Root cause still undetermined, and now believed UNDETERMINABLE after the fact: the catalog keeps no write audit trail, so "which process deleted these rows" has no answer once the window closes. Hypotheses 4 and 6 are acquitted (2026-08-30); 8 is unfalsifiable rather than untested. Re-open trigger is in the Resume section.'
---

# BUG: SDD ledger directory and this work-stream's catalog rows both vanished between sessions

## Summary

Between 2026-08-24 17:13 (commit `85bd01e1`) and 2026-08-25 08:00, two things
belonging to the hidden-information-eval work stream disappeared: the git-ignored SDD
ledger directory `.superpowers/sdd/2026-08-23-hidden-information-eval/` was deleted from
disk, and the librarian catalog rows for that stream's plan and spec were dropped even
though both files were still on disk. Nothing in git was lost. The ledger was
reconstructed from the session transcript (see **Workarounds**); the catalog was repaired
by `librarian(action="reindex")`.

## Symptom (Effect)

Catalog side — the plan artifact's id, read from the file's own committed frontmatter,
no longer resolved:

```
artifact(action="get", id="89c2984ca7c074a0")
→ unknown artifact id '89c2984ca7c074a0'. If this id came from an earlier call, an
  artifact(action="move") since then will have re-keyed it (id = sha256(abs_path))…
```

A path query found nothing either, at the widest scope and with archived rows included:

```
artifact(action="find", filter={"rel_path": {"contains": "hidden-information"}},
         include_archived=true, scope="umbrella")
→ {"count": 0, "items": []}
```

The `find` response's own hints named the real shape of the problem:

```
"unindexed_files": 12,
"unindexed_hint": "12 file(s) under this scope are not in the catalog and cannot match
                   any filter; run librarian(action=\"reindex\") to include them"
```

Filesystem side — the ledger named by the plan's `ledger:` frontmatter key was gone:

```
read_markdown(".superpowers/sdd/2026-08-23-hidden-information-eval/progress.md")
→ file not found
```

## Reproduction

Not yet reproducible — the loss was noticed on resuming after a compaction, with no
session running to observe the deletion. Best leads are under **Hypotheses tried**.

`git rev-parse HEAD` at detection: `047dd433`.

## Environment

- codescout `experiments`, main checkout `/home/marius/work/claude/codescout`
  (5 linked worktrees registered, none active).
- Catalog: `/home/marius/.local/share/librarian/catalog.db` (machine-local, not in git).
- Profile `~/.claude-sdd`. Two sessions touched this repo in the window: the one that
  wrote `85bd01e1` (17:13:03) and one that wrote `047dd433` (17:39:45).

## Root cause

Undetermined, and — as of 2026-08-30 — believed **undeterminable after the fact**.

Of the four named candidates, three are now acquitted from code and live measurement
(4, 6, and the newly-found 9; see *Hypotheses tried*). The survivor is hypothesis 8, a
foreign non-codescout writer against the shared machine-local catalog, and it is
unfalsifiable rather than merely untested: nothing records which process wrote or deleted
a catalog row.

That is the durable finding here. The incident is not undiagnosed because the
investigation stopped early; it is undiagnosed because the catalog cannot say who wrote
to it, and a recurrence would be exactly as opaque.
## Evidence

### Reindex re-minted the same ids, so the paths never moved

```
librarian(action="reindex", scope="repo")
→ {"added": 12, "updated": 42, "removed": 0, "unchanged": 1065, "embedded": 54}
```

After it, both rows resolved at their original ids — `89c2984ca7c074a0` (plan, still
`status: active`) and `556cc34167321863` (spec). Since `id = sha256(abs_path)`, identical
ids prove the files never moved; the rows were simply absent.

### The row loss was wider than this work stream, and cost event history

`docs/trackers/prompt-surface-measurement-session-log.md` came back with
`created_at == updated_at == 1787634828271` — the reindex timestamp — meaning its row was
**re-created**, not updated, so its events and links are gone. Its `extra` survived intact
(`entry_high_water_F: 9`, `entry_high_water_W: 7`, `entry_prefix: [F, W]`) because those
live in committed frontmatter. This is the design working as `get_guide("tracker-conventions")`
describes: *"the counter has to travel with the repo too."*

### It was not a blanket clean of ignored files

Six sibling directories under `.superpowers/sdd/` survived with their original mtimes
(2026-08-06 through 2026-08-20), and `.superpowers/sdd/.gitignore` is `*`, so a
`git clean -fdx` would have taken all of them. Only the 2026-08-23 directory went.

### Augmentations were NOT lost this time

`docs/trackers/tool-usage-patterns.md` (`f2ecdd76a6189efb`) still carries its
augmentation, `entry_collection: "observations"`, and all 26 `T-N` rows. This
distinguishes the incident from `docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md`
(F-4) and from the archived `2026-05-17-reindex-cascade-delete-data-loss.md`.

**Update 2026-08-26 — this observation was evidence *against* the 2026-08-23
file's conclusion, not merely a contrast with it.** That bug claimed every
augmentation in the catalog had been destroyed on 08-23. This section records
`f2ecdd76a6189efb` alive with its full `observations` collection two days later,
with no restore performed in between — which is only possible if it was never
destroyed. It has since been refuted outright: the row's `created_at` is
2026-07-05 and its history is unbroken. The contrast that still stands is with
the archived `2026-05-17-reindex-cascade-delete-data-loss.md`, which was a real
loss.

## Hypotheses tried

1. **Hypothesis:** `git clean` removed the ignored ledger.
   **Test:** compare mtimes of all seven `.superpowers/sdd/*` directories.
   **Verdict:** rejected — six siblings survived, all equally ignored.

2. **Hypothesis:** the artifacts were moved, re-keying them.
   **Test:** reindex and compare the new ids to the ids in the files' frontmatter.
   **Verdict:** rejected — ids identical, so `abs_path` never changed.

3. **Hypothesis:** the catalog DB was swapped or repointed (`catalog_db_path`).
   **Test:** read the reindex counts.
   **Verdict:** rejected — `unchanged: 1065` means the same populated DB was in use;
   only 12 files were absent from it.

4. **Hypothesis:** `librarian(doctor, fix=prune_missing)` pruned rows under a root it
   judged dead.
   **Test:** not run. The files exist on disk, and `prune_missing` refuses a root that
   still exists — but the batch mode's dead-root derivation is the one place this could
   still bite. **Verdict:** deferred; this is the strongest remaining lead.

5. **Hypothesis:** the rows were never created — the plan and spec were written with
   `create_file` rather than `artifact(action="create")` and only ever appeared indexed.
   **Test:** the previous session ran `artifact(action="update", id="89c2984ca7c074a0",
   patch={...})` successfully, which requires a row. **Verdict:** rejected.

6. **Hypothesis:** concurrent writers to the shared machine-local catalog dropped the rows.
   **Test:** enumerated live codescout server processes during an unrelated recon pass,
   2026-08-25 11:50 — `ps -o pid=,ppid=,lstart= -C codescout`.
   **Observed: SIX concurrent servers**, every one with a live parent, so none are orphans:

   ```
   PID       PPID     PARENT   RSS    STARTED
   22767     22728    claude   337M   Mon Aug 24 17:37:22
   1082945   1082692  codex    305M   Mon Aug 24 21:55:31
   1930828   1930731  claude   351M   Tue Aug 25 08:08:40
   2136184   2135916  claude   384M   Tue Aug 25 09:02:39
   2719647   2719361  claude   363M   Tue Aug 25 10:48:28
   3031648   1923118  claude   332M   Tue Aug 25 11:49:58
   ```

   All six share **one** catalog at `/home/marius/.local/share/librarian/catalog.db`
   (machine-local, git-ignored). One is a **`codex`** client — a different agent entirely,
   which would not be following codescout's own conventions. The loss window
   (2026-08-24 17:13 → 2026-08-25 08:00) contains at least the `22767` and `1082945`
   sessions, and `1082945` is the codex one.
   **Verdict: deferred — this displaces `prune_missing` as the strongest lead.** It also
   fits a detail hypothesis 4 does not: the 12 lost rows were the *most recently written*
   ones, which is what a lost-update or a rolled-back transaction on a shared DB looks
   like, and not what a dead-root prune looks like (that would take a contiguous subtree
   regardless of age).
   **Next test:** `PRAGMA journal_mode` on the catalog, and whether writes take an
   exclusive lock or last-writer-wins. Check `catalog-sql-hazards` memory first — it may
   already name this.

7. **Hypothesis:** nothing was lost — work continued on the laptop during a few days away,
   and this desktop is simply behind.
   **Test:** full host comparison, 2026-08-25. `git` state on both; then a pruned `find`
   over both work repos on each host, diffed.
   **Verdict: REJECTED, and it is the useful kind of rejection.**
   - Both repos were **already in sync by git**: laptop `codescout` at `047dd433`, laptop
     `prompt-engineering` at `cf97286` — exactly what this desktop held before today. No
     laptop-only commits exist.
   - 4,872 laptop files vs 5,209 desktop; **210 only on the laptop**. Of those, 208 are
     ephemeral machine-local state (96 `.buddy/`, 90 `.codescout/` guide-hint session
     JSONs, diagnostic logs, onboarding temp files, local DBs).
   - The remaining **2** were `docs/trackers/2026-05-07-shine-improvements.md` and
     `docs/trackers/dependency-review-session-log.md` — and they are not losses either.
     Commit `45411044` (2026-08-25 09:12, a **peer session**) archived the first and
     compacted the second into `dependency-review-session-log-2026-08-25.md`. The laptop
     is pre-sweep.
   - The uncommitted `conclude-last` work is byte-for-byte the same size on both hosts
     (134 files, 456K + 156K).

   So the desktop is not behind the laptop on anything. The loss is local to this host and
   to git-ignored surfaces, which is what hypothesis 6 already predicted.

8. **Hypothesis (strengthening 6):** the concurrent writer was a **non-codescout agent**.
   **Evidence:** the process table taken 2026-08-25 11:50 shows `1082945`, started
   **Mon Aug 24 21:55:31**, parented by **`codex`** — inside the loss window
   (2026-08-24 17:39 → 2026-08-25 08:00) and the only non-`claude` client on the machine.
   A codex session is not bound by codescout's own conventions about catalog writes or
   about `.superpowers/` state.
   **Verdict:** deferred. Note the ordering constraint this adds: the row loss was detected
   at ~08:00, **before** any of 2026-08-25's commits, so the 09:12 hygiene sweep cannot be
   the cause — only an earlier writer can be. That leaves `22767` (claude, Aug 24 17:37)
   and `1082945` (codex, Aug 24 21:55) as the two candidates in-window.

**Concurrency in this repo is now documented from the other side, too.** A peer session
filed `bug-fix-session-log:F-60` today: this bug file's own author committed `a468e69d`
with `git add -A`, which swept up that peer's on-disk-but-unstaged `append_entry` writes
to a shared tracker. No content was lost — only provenance — but it is direct evidence that
two sessions share one working tree with no isolation, which is the same substrate
hypothesis 6 indicts.

### 2026-08-30 — hypotheses 4 and 6 both acquitted, and the prescribed test for 6 was itself broken

**Hypothesis 6 — ACQUITTED, by this file's own criterion.** The Resume states *"WAL plus
a non-zero busy timeout would largely acquit concurrency."* Both hold:

| check | result |
|---|---|
| `PRAGMA journal_mode` on the live catalog | **`wal`** (WAL sidecars present on disk) |
| `Catalog::open`, `src/librarian/catalog/mod.rs:422` and `:459` | `foreign_keys = ON; journal_mode = WAL; busy_timeout = 5000` |
| pinned by | `open_sets_busy_timeout_for_cross_process_writers`, asserts `5000` |
| the read-max-write path | `append_entry` runs in one `IMMEDIATE` transaction, doc-commented as safe under cross-process concurrency |
| migrations | `BEGIN IMMEDIATE`, with a comment naming "two codescout instances open a shared catalog" as the case it exists for |

**And the Resume's step 2 would have convicted the wrong suspect.** It prescribes
`sqlite3 <db> 'PRAGMA journal_mode; PRAGMA busy_timeout;'`. `busy_timeout` is a
**per-connection** setting, so a fresh `sqlite3` CLI connection reports its own default
forever, regardless of what codescout sets. Measured: **`0` from the CLI, `5000` in the
code.** Read literally, the recipe returns *"delete-or-WAL with no timeout → largely
convicts concurrency"* — and no other reading was available from that command. Only
`journal_mode` is persistent and therefore answerable from outside the process.

*(That is an instance of
`docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md`: an instrument that
answers a neighbouring question, consulted precisely because someone was being careful.)*

**Hypothesis 4 — REJECTED, from the code rather than deferred.** `derive_dead_roots`
(`src/librarian/tools/doctor.rs:1078`) opens with two guards:

```rust
if path.exists() { continue; }                       // not a missing row
match path.parent() {
    Some(parent) if parent.exists() => continue,     // single file under a live dir
```

This file's own Symptom section records that **the plan and spec were still on disk**.
The first guard therefore disqualifies them before the dead-root climb begins, so batch
`prune_missing` could not have selected them however its root derivation behaves. Case
(b) is pinned by `derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files`.

This also removes the shape argument that made 4 plausible: a dead-root prune takes a
contiguous subtree, and the 12 lost rows were the most recently written ones.

### 9. Hypothesis: the `force` reindex path issued a destructive DELETE — REJECTED, twice

Found while chasing 4. `src/librarian/mod.rs:392-398` (`reindex_cli`) still holds a
`DELETE FROM artifact WHERE abs_path LIKE ?1` that `reindex.rs` removed from the real
path in `d482ca8a` for cascade-wiping augmentations. It cannot be the cause, for two
independent reasons:

- The pattern is `format!("{}/", root)` — **no `%`**. `LIKE` without a wildcard is
  equality, and no `abs_path` equals `<root>/`. Measured against SQLite directly: the
  no-wildcard form matches only a literal row, the `%` form matches the subtree.
- `reindex_cli` is `#[cfg(test)]`, and `codescout --help` lists no `reindex` subcommand.
  Nothing user-facing reaches it.

Filed separately as
`docs/issues/archive/2026-08-30-reindex-cli-carries-a-broken-copy-of-a-deliberately-removed-delete.md`
— harmless today, and one character from being the data-loss path it descends from.
**Fixed and archived 2026-08-30** (`9f743091`, patch-id `92db5adf65b7a748`): the block and
its `force` parameter are gone, a comment records the decision where they stood, and a
mutation-verified regression test now fails if anyone restores the `%`. That does not
disturb this hypothesis's rejection — the statement really was inert, which is exactly what
acquitted it here, and the fix confirmed it at runtime rather than by reading.

### What remains: hypothesis 8, and it is unfalsifiable rather than untested

Acquitting 6 acquits **SQLite concurrency mechanics** — lost updates, `SQLITE_BUSY`,
interleaved writers. It does not acquit hypothesis 8, which is a different claim: that a
**foreign agent** (the `codex` client observed in-window) issued its own writes against
the shared machine-local catalog. No pragma prevents a third party from running a DELETE.

That hypothesis cannot be tested after the fact, and the reason is structural: **the
catalog keeps no write audit trail.** Nothing records which process wrote or deleted a
row, so "who dropped these 12" has no answer once the window closes. This is why the
incident stays undiagnosed — not for want of effort, but because the evidence was never
recorded. Any future recurrence will be equally opaque unless that changes.
## Fix

None yet — the incident is recorded, not diagnosed. Recovery is documented below.

## Tests added

None. A regression test needs a reproduction first, and hypothesis 4 is the only
untested lead.

## Workarounds

**Catalog rows:** `librarian(action="reindex", scope="repo")` restores them at their
original ids, because `id = sha256(abs_path)`. Frontmatter-borne state (`status`,
`extra`, `entry_high_water_*`) survives; catalog-only state (events, links, and — per
F-4 — augmentation) does not.

**A deleted git-ignored working file:** replay it out of the session transcript. Every
`create_file` / `edit_markdown` / `edit_file` call is recorded there with its full
payload, so the file's whole write history is recoverable:

1. Scan `~/.claude*/projects/<project-slug>/*.jsonl` for `tool_use` blocks whose input
   names the path.
2. Pair each with its `tool_result` and **drop the ones that failed** — this ledger had
   two `edit_markdown` calls refused with *"File writes are disabled for this project"*
   that wrote nothing, and replaying them would have inserted content the original never
   held.
3. Apply the survivors in timestamp order.

That recovered 1,264 lines and all 24 `R-N` rulings here. Two caveats found the hard way:
a simulated `insert_before` will not reproduce the tool's exact whitespace, so any later
`edit_file` keyed to that text misses and must be reconciled by hand; and the
reconstruction is content-faithful but not byte-faithful, so it should say so in its own
header. The scripts are in this session's scratchpad (`recover_ledger.py`,
`rebuild_ledger.py`).

## Resume

**Closed as `zombie` 2026-08-30** — the disposition this section itself prescribed:
*"If both are acquitted, close as `zombie` with a re-open trigger rather than leaving it
open."* Both were. Steps 1–3 below are done and their results are in *Hypotheses tried*;
they are kept because step 2 was **wrong in a way worth preserving** — `PRAGMA
busy_timeout` is per-connection, so the command as written could only ever have returned
`0`, and reading it literally convicts concurrency on evidence that carries no
information.

**Re-open trigger** (also in frontmatter, so a query can reach it): catalog rows
disappear for files that are **still on disk** — a `find(scope="umbrella",
include_archived=true)` returning `count: 0` for a path that exists, with a non-zero
`unindexed_files` hint in the same response, and no `artifact(move)` or archive in the
window.

**If it does re-open, do not re-run the hypothesis list.** Three of four are closed and
the fourth cannot be settled by inspection. The only thing that would answer it is
evidence captured *while it happens*: a write audit trail on the catalog, or at minimum
recording the writing process on `artifact` mutations. Propose that first; investigating
again without it repeats a search whose outcome is already known.

Prospective instrument now exists: `librarian(action="audit_log")` records every catalog
mutation incl. foreign writers (T-1, this plan). The historical loss stays
undeterminable; any recurrence is now answerable. Re-open trigger unchanged.
## References

- `docs/superpowers/plans/2026-08-23-hidden-information-eval.md` — the plan whose
  `ledger:` key names the deleted file.
- `docs/trackers/prompt-surface-measurement-session-log.md` — the work stream's session
  log; its row was re-created by the repair reindex.
- `docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md` — F-4,
  augmentation loss; related but distinct (augmentations survived here).
- `docs/issues/archive/2026-05-17-reindex-cascade-delete-data-loss.md` — the earlier
  reindex-driven data loss, fixed.
