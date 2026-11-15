# Collection hot view: what was built, the rulings, what is parked (2026-11-15)

Built by the lifecycle line: a fork (`4bbde027-1431-49b4-b1f0-30a54064c53c`, account `.claude-stef`, later also copied into `.claude-kat`) of the lifecycle session `codescout-51` (`31731474-…`), on the operator's word of 2026-11-15 ("lets do the work then, since it cannot find codescout-51"). The operator's five decisions and the design note live OUTSIDE this repository: `/home/marius/work/claude/plugins/notes/2026-11-15-memory-system-for-ledgers-design-note.md` (not committed; the plan copies what an executor needs).

## Where it is
- On `integration/identity-lifecycle-20261114` at `5d655bfef` (fast-forwarded 2026-11-15 13:23Z from `fddfe01f2`, 13 commits). NOT on `experiments`; `experiments` gets its one merge from integration when the operator says all work is done.
- Plan: `docs/superpowers/plans/2026-11-15-collection-hot-view.md` (status done; § Decisions R1-R5, § Outcome, § Contract for the hook). No separate spec (a call: the plan carries its decisions).
- Gate on exactly `5d655bfef`: FMT/CLIPPY/LEAN/DEFAULT all 0 (lean 10322, default 10417, python 1836). Not covered: heavy Python suites, server-stack, local ONNX (CI).
- Branch `feat/hot-view-plan-20261115` and worktree `.worktrees/hot-view-plan-20261115` still exist (remove with `scripts/worktree-remove.sh` on the operator's word; the git-ignored SDD workspace `.superpowers/sdd/2026-11-15-collection-hot-view/` inside it holds the ledger, briefs, reports and review packages and dies with the worktree: this memory is the copy of what matters).
- The hook in `../claude-plugins/codescout-companion/hooks/session-start.mjs` is NOT built (separate repository, separate ship).

## What it adds
`EntryRow.status_since` (and `"status_since"` in `TABLE_COLUMNS`, so `doc(get, entry_filter)` rows gain the key); `entry::revisit_due(class, revisit_by, today)` (one definition of "due", used by `check_entry` and the view); readme keys `hot_cap` (whole number 1..=u32::MAX) and `stale_after` (`<n>d`), `SCHEMA_KEYS` is 11 keys, `CollectionSchema.{hot_cap, stale_after_days}`; `src/librarian/lifecycle/hot_view.rs` (`hot_view(&LoadedCollection, today) -> Result<HotView, ArchivedCollection>`, `render_markdown`, `pub const SCOPE`); `codescout hot-view <key> [--project P] [--json]` (`src/cli/hot_view.rs`, `tests/cli_hot_view.rs`); a test that a list field is filterable through `entry_filter`.

## Rulings (what the operator can undo)
- R1: an untouched `open` entry gets a `stale` FLAG on the hot view, not a ninth check (the eight pinned names are unchanged). Threshold is `stale_after`; without it nothing is flagged. Stale is `idle_days > n` with `idle_days = (today - status_since).num_days()` (no date addition: no overflow; a future `status_since` is never stale). Premise "re-confirming an entry clears the flag" is read from `write.rs` `apply_status` (it always sets `status_since` to today), NOT traced through `doc(update)` (does an unchanged status reach `apply_status`, or is it refused as an empty update?).
- R2: hot = live entry of class `open`, or `blocked`/`deferred` with `revisit_by` strictly before today. Entries the view cannot judge (undeclared status; blocked/deferred with no `revisit_by`) are counted in `unclassified`.
- R3: rank = revisit due, then stale open, then other open; inside a group `status_since` descending, then key ascending. `hot_cap` keeps a prefix, so a cap never hides a flagged entry while an unflagged one shows; the view reports `hidden` and `hidden_flagged`. (An Opus plan review found that ordering by `status_since` alone made the cap hide exactly the stale and long-parked entries.)
- R4: the view reads one collection folder (`load_collection`, never `load_docs`), runs no doctor check, and says so (`SCOPE`).
- R5: no `pinned` field.
- Whole-branch fix wave ruling: the process allows no second fix wave, but the re-review found a real Windows-CI breakage in the wave's own diff (a test creating a directory named `two\nlines`); ruled: one attribute (`#[cfg(unix)]`), verified by reading the one-hunk diff, then the gate on the final head. Cost if wrong: that flattening is untested on Windows (covered on Linux).

## Contract for the hook (summary; the full text is in the plan)
Exit 0 = the view is on stdout and stderr is empty. Exit 2 with `NOT SHOWN — <reason>` (or `{"shown": false, "not_shown": ...}` under `--json`) = not shown. Exit 2 with EMPTY stdout = a clap argument error (a binary without this command, version skew); 101 = panic. Treat every non-zero exit as not shown; degrade open. Without `hot_cap` the view lists every hot entry: show only collections that declare it, or cut. `--json` rows nest `fields` (unlike `doc(get)`'s flat rows).

## Parked (deferred on purpose)
- Move `problems` into `HotView` so an in-process consumer (the dashboard) cannot print a healthy-looking zero for a folder whose entries all failed to parse (the CLI adds the problems line itself today). Do it when the dashboard is designed.
- A shared `cli::resolve_root` with `key_gate` (identical root resolution, `one_line`, `run`); `key_gate::evaluate`'s doc has the same "against cwd" flaw the hot-view doc had.
- `HotReason::RevisitDue` doc still names blocked/deferred instead of pointing at `StatusClass::needs_revisit`; the module doc says exit 0 means the view was printed, but `--help` also exits 0; `(1 problems found` grammar.
- Test gaps: the within-group order of the revisit-due group is weakly pinned (the two due entries in the capped fixtures share `created`); `a_title_with_a_pipe_and_a_newline...` has no backslash; the column-order test sorts both lists (pre-existing); no doctor-level "terminal is never due" test (pre-existing); a bad `hot_cap` and a bad `stale_after` are not reported together in one test.
- Rows are selected by key in `hot_view`: the one-node property rests on the tree invariant documented on `LoadedCollection.tree`; revisit when the dashboard builds a `LoadedCollection` from a whole-repo tree.
- Which collections a hook shows, and a command that lists them: the hook change's decision.

## Process lessons (add to tracker-organization-session-log)
- Per-task reviews run no suite: the gate failed once on `tests/result_caps.rs every_cap_constant_is_classified` (a constant named `KEY_HOT_CAP` needs a `// cap-class: NOT_A_CAP — <reason>` line). Second occurrence after F-4 (`read_door`). Budget for it: a task that adds a constant, a file read or an `OpenOptions` should run `scripts/with-slot.sh cargo test --test result_caps` and the read-door test.
- An independent Opus review of the PLAN before dispatch found 6 Important issues (cap hiding the stale entries, fixtures that cannot fail because every shared entry helper writes `status_since` equal to `created`, ambiguous `hot_view` input, a date-overflow panic, an untested exit-code contract, an unpinned JSON shape). A fixture whose "wrong" and "right" orders coincide proves nothing: set the two dates independently.
- Whole-branch review earned its cost again: it found the false exit-2 contract (verified by running an older release binary against the new subcommand name).
- `./scripts/fmt-mine.sh` refuses lifecycle files (attribution message); `rustfmt --edition 2021 <file>` works.
- Never run a Unix-only fixture (a path with a control character) on the Windows lanes: gate it with `#[cfg(unix)]`.
