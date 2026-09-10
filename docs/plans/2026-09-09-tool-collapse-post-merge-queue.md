# HANDOFF — tool-surface-collapse, post-merge worklist

**Status: implementation COMPLETE, catalog merge COMPLETE. Three ledger items owed, then removal.**

| | |
|---|---|
| Plan | `docs/superpowers/plans/2026-09-02-tool-surface-collapse.md` (1543 lines, in git) |
| Branch | `tool-collapse` @ `6f032dbd` — **strict ancestor of `experiments`**, 0 ahead |
| Verified against | `experiments` @ `20bb2a47`, 2026-09-09 16:58 +0300 |
| Written by | session `bf6a6925-f207-4a2f-8135-95e7563e859f` (`.claude-sdd`, pid 2841831), from inside the worktree |

**Run everything below from the MAIN checkout** `/home/marius/work/claude/codescout`. Not a
preference — `append_entry` and `doc(move|delete)` are **refused by code** from a worktree
(`src/librarian/tools/append_entry.rs:113`, *"id allocation is not supported from a worktree
checkout"*). That refusal is why these items are a handoff rather than done.

> ⚠ **Before staging ANY file under `docs/trackers/` or `docs/issues/`, run
> `scripts/file-provenance.py`.** Every owed item below is a write to a shared ledger, and the
> main checkout runs several sessions concurrently. **A path-scoped commit is not protection:**
> `git add <path>` stages that path's entire current content, so a commit naming exactly the one
> file you edited still captures every peer edit sitting in it. Measured 2026-09-09 — `d71f0aac`
> (17:06 +0300) named a single file, `docs/trackers/bug-fix-session-log.md`, and swept two peer
> entries in with it.
>
> **And `git status` cannot warn you.** It prints one line per dirty file; your own
> `append_entry` already made the file dirty, so that marker is unconditional and carries no
> information about whose other edits are underneath. A per-file aggregate cannot answer a
> per-author question — dirtiness is not authorship. (Reported by session
> `b80a27d4-9729-40ef-8c28-ad8982df6d13`, from both sides of it.)

---

## DONE — do not redo

### All 13 plan tasks (Tasks 1–13; there is no gap in the numbering)

Verified per-task, not inferred from the branch being merged. **Task 12 is the one that matters
here**: it is a paired change in `../claude-plugins`, a different repo, so "branch is 0 commits
ahead of experiments" carries no information about it. Checked separately and it is done.

| Task | Evidence at `experiments` |
|---|---|
| 1 `Tool::description_cap()` | `fn description_cap` in 5 files incl. `src/tools/core/types.rs` |
| 2 `artifact` → `doc` | tool is `doc` in the live registry |
| 3 schema quality + 4 gates | `dac1068a` |
| 4 fold `artifact_event` | `doc` carries `event_create` / `event_list` |
| 5 fold `artifact_augment` | `doc` carries `augment` |
| 6 fold `artifact_refresh` | `doc` carries `gather` / `list_stale` |
| 7 fold `read_markdown` | `read_file` takes `heading` / `headings` |
| 8 fold `edit_markdown` | `edit_file` takes `heading` + `action` |
| 9 prompt surfaces | `ONBOARDING_VERSION: u32 = 31`; `claude_md_contains_no_deprecated_tool_names` present |
| 10 surface budget ratchet | `TOOL_SURFACE_CHAR_BUDGET: usize = 57_296` — `src/server.rs:3555` |
| 11 CLI `codescout doc` | `src/cli/doc.rs`, `doc_event.rs`, `doc_refresh.rs`; `Commands::Doc { verb }` — `src/main.rs:177`, `:437` |
| 12 companion plugin | live surfaces use `doc(action=…)`; two old-name hits remain and **both are correct** — see *Do not "fix"* below |
| 13 registry pin + reachability | `tests/tool_reachability.rs` present |

### The catalog merge — executed 2026-09-09 ~13:30 +0300

This was the only step with unrecoverable downside (`id = sha256(abs_path)`, catalog not in git).

```
6 × doc(action="graft", force=true)      — each dry-run individually first
librarian(action="merge_worktree", root=".worktrees/tool-collapse")
  → registration: "merged",  merged: 1,  reseated: 3,  conflicts: [],  remap: {}
librarian(action="doctor")
  → worktree_scoped_row: 2 — BOTH /home/marius/work/claude/whatsapp (different repo)
  → ZERO pending rows for tool-collapse
```

Re-verified after a second binary rebuild at 16:49:44 and five `experiments` advances: still
2 rows, both `whatsapp`. **The merge held.**

Dispositions, because they contradict what `doctor` alone suggests: `docs/trackers/issue-clusters.md`
(the augmented `IC-N` tracker) took the **`merged`** branch and folded cleanly — the overlay is
delta-only against the fork-time base, so its main-twin edit and the shadow never contended. The
six `reseat_collision` rows were **empty shadows** — every dry run reported
`augmentation: false, links_out: 0, links_in: 0, observations: 0, has_events: false`, and every
applied graft reported `events_repointed: 0` with no `suspicious` and an empty `remap`. So no
citations needed rewriting and nothing was merged away.

### Bug-file archives — DONE BY A PEER, 2026-09-09

All six of the plan's after-merge item 4 are archived at `20bb2a47`. The three that were still
open at 06:15 (`workspace-schema-requires-an-action-the-code-does-not`,
`artifact-patch-schema-describes-a-failure-that-no-longer-happens`,
`artifact-action-labels-omit-delete-move-and-update-entry`) are now under `docs/issues/archive/`.
**Check before acting on any older copy of this document, which lists them as owed.**

Fix citation, if you need it for anything else — `dac1068a`, patch-id
`371bee7c5081481311866cd797d7591abb2c5bc3`, single parent `c202bf70` (not a merge, so the
patch-id is real). Still an ancestor of `experiments` at `20bb2a47` despite the 10:33 rebase —
it predates the rebase base.

---

## OWED — three items

### 1. Hamsa row — `A-39`

Ledger `docs/trackers/prompt-hamsa-audit-log.md`, artifact `59ebeebb6ed05c89`. Index is complete
`A-1`…`A-38`; **next id is A-39.**

> An earlier copy of this document said "A-37 is absent, check before appending." That was
> measured against the 06:15 tip and is **false at `20bb2a47`** — `| A-37 | 2026-09-02 | CAP-10's
> practice rule, THIRD stimulus…` is present. Plausibly the same index-row gap that hit OB-21/OB-22,
> since repaired. Let `append_entry` allocate; do not hand-pick the id.

```
doc(action="append_entry", id="59ebeebb6ed05c89", id_prefix="A", …)
```

Content per the plan: the **Iron Law 4/5 removal as forced-by-removal**, with the two-week
`usage.db` measurement **pre-registered** — share of `read_file` calls on `.md` carrying
`heading`/`headings`, against `read_markdown`'s share in the 30 days before the ship.

### 2. Sibling repos — 7 load-bearing files, 22 hits

Still naming `artifact(` / `read_markdown` / `edit_markdown`. Counts at 16:58 +0300:

```
1  prompt-engineering/CLAUDE.md                      <- served every session
2  researcher/CLAUDE.md                              <- served every session
2  prompt-engineering/docs/issues/_TEMPLATE.md       <- mints new instances
2  researcher/docs/issues/_TEMPLATE.md               <- mints new instances
5  prompt-engineering/docs/templates/session-log.md
8  prompt-engineering/docs/TAXONOMY.md
2  prompt-engineering/src/prompt_tdd/assertions.py   <- executable
```

Plus `prompt-engineering/scenarios/{surface-budget/check_tracker.py,
augment-merge-restatement/check_merge_true.py, workspace-pin-routing/check_pins_workspace.py}`
and `prompt-engineering/.codescout/memories/gotchas.md`.

**Leave as written:** `docs/superpowers/plans/*`, past `docs/issues/*`, `scenarios/**/RESULTS.md`,
anything under `archive/`. **And do NOT sweep `scenarios/**/fixtures/claude-*.md`** — those are
eval fixtures; the tool name is part of the prompt under test, so editing one silently changes
what the arm measures. That is a call for whoever owns the arm.

### 3. Items 3, 6, 8 — judgment, not sweeps

**Item 3** (tracker instruction text). The plan's grep over `docs/trackers/*.md` returns **93
files**, but its own scope is *"each **active** tracker whose **preamble or template** tells an
agent which call to make"* — archived trackers and past entries are explicitly left as written.
Run the grep as a candidate list, then read each hit's context.

**Item 6** (compaction session log, artifact `03464a8808345846`). High-water still **F-10 / W-17**,
so no post-ship entry exists. Its `tools/list` figure is the *pre-collapse* `27 tools / 58,882
chars — over-counted, see correction below`. The 28.7k-token re-measurement went into
`resume-tool-surface-structural-mechanisms` at `284c82bd`, which is now `status: archived` — so
that number sits in a closed record. **Re-derive, don't cite:**
`python3 scripts/probe_tool_surface.py` and
`cargo test --lib tool_surface_report_lengths -- --nocapture`.

**Item 8**. One line in the first `/analyze-usage` report spanning the ship date: reports must
union old and new tool names. `scripts/probe_tool_surface.py` needs no change — it reads the live
registry.

---

## REMOVAL — last, and not runnable from inside the worktree

Preconditions are met: catalog merged and verified, branch is a strict ancestor of `experiments`
so deletion loses no commits.

```
git worktree remove --force .worktrees/tool-collapse
git branch -d tool-collapse
```

`--force` is needed for two untracked paths, **neither of which holds anything unique**:
`.codescout/write.lock.holder` (0 bytes, a session lock artifact) and `.superpowers/` (520K —
11 of its 13 files are `review-<a>..<b>.diff` whose SHA pairs all resolve in git, so `git diff a..b`
regenerates them; the other two are dispatch briefs for Tasks 8 and 11, whose specs live in the
committed plan at lines 1139 and 1413 — **confirmed not needed**).

`git branch -d` **refuses while the worktree exists** (`cannot delete branch 'tool-collapse' used
by worktree at …`), so the order above is mandatory rather than stylistic.

---

## Do not "fix" these — they are correct as written

- **`claude-plugins/codescout-companion/hooks/worktree-write-guard.test.sh`** keeps `edit_markdown`
  as a deliberate **stale sentinel**, annotated on the line: *"it sits in the stale-sentinel block
  precisely so re-adding it to the matcher — which would read as coverage of a tool that cannot
  exist — flips a visible test."*
- **`claude-plugins/codescout-companion/README.md`**'s two hits are in a changelog describing what
  past versions did.

## Three traps this handoff paid for

1. **`doctor` is the LEGACY view of worktree rows; `merge_worktree --dry-run` owns the question.**
   `src/librarian/tools/doctor.rs:33-38` — `registered` rows are *"pending
   librarian(action=\"merge_worktree\"), not a reseat"*; `:96-99` — `registered` rows are **SKIPPED
   entirely**, and reseat's `collision` rows are *"left untouched and reported for a manual graft."*
   An earlier draft of this document built its central table from `doctor` and got the row count
   right and every disposition wrong. If you need worktree-row state, run `merge_worktree` with
   `dry_run=true`.
2. **A wider `scope` does not mean a bigger result — compare `scope.applied`, never the byte count.**
   `doctor` now returns `scope.applied`; `scope="all"` resolves to this project's umbrella
   (`codescout-ecosystem`), whose excluded roots lie outside it anyway, so the size barely moves
   even when the argument is honoured. The live binary was rebuilt 16:49:44 and post-dates the
   `26b60af8` scope fix.
3. **`git grep` pathspec `docs/issues/**/*.md` silently excludes the top level** — it reached 271
   files, all under `archive/`, missing 111 live ones. Use `docs/issues/`. And on that corpus the
   two predicates differ: **382** files *mention* `cluster/`, **369** *carry* a frontmatter tag;
   distinct tag values are **23** under either predicate (22 promoted `IC-N` classes plus
   `cluster/unclassified`, the escape hatch).

## Findings in flight — owned by `codescout-87`, not by this handoff

Session `b80a27d4-9729-40ef-8c28-ad8982df6d13` holds a copy of the companion draft
`DRAFT-bug-worktree-occupancy-cwd.md` and files five artifacts on its own queue, credited:

1. **Worktree occupancy checked against a process cwd a session does not have** — classified
   `cluster/gate-keyed-on-unobservable-event` (`IC-2`) after adjudication, **not** `IC-18`; the
   draft carries the reproduction and the four-process measurement.
2. `git worktree remove` has no pre-flight against pending catalog rows.
3. `observer-blindness.md` index skips **both** `OB-21` and `OB-22` — a two-row gap; fixing only
   one leaves the index still jumping OB-20 → OB-23 and reads as closed.
4. An `OB-22` addendum: *widening a filter while keeping the predicate leaves the fixed point intact.*
5. `edit_code(action="replace")` destroys a symbol's doc comment when `attributes` is supplied —
   the `attributes` path (`edit_code.rs:959-981`) bypasses the two-phase preservation added by
   `14d0a211` (2026-07-29) and was itself introduced 17 days later by `edbdf4d9` (2026-08-15),
   whose stated purpose was disclosing destructive writes. Its `removed_attributes` field
   deliberately excludes doc comments. The five guards `14d0a211` shipped all test
   `skip_lead_region`, which that path never calls — so the remedy is an **outcome-level** test
   (`edit_code(replace, attributes=[…])` on a symbol with a doc comment, assert it survives),
   which is outside no path by construction.
