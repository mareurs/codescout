# Worktree merge → catalog reconciliation

> **UPDATE 2026-07-17 — a first-class overlay flow is built on `experiments`
> (not yet on `master`).** The post-hoc reconciliation below is what's true on
> `master` today and remains the LEGACY fallback for UNREGISTERED worktree rows.
> On `experiments`, a worktree-overlay feature makes reconciliation write-time
> instead of merge-time: worktree sessions read the main catalog via an overlay
> and fork-on-first-write into shadow rows (recorded `worktree_of` link +
> `worktree_fork` base-snapshot event + durable `worktree_registration` row that
> survives `git worktree remove`); merge is `librarian(action="merge_worktree",
> root=…, [dry_run]/[abandon])` — it folds ONLY the shadow's DELTA onto main
> (never bare-grafts a seeded shadow), three-ways scalars (main wins on
> conflict), renumbers colliding entry ids, and closes the registration. `doctor`
> now flags registered rows as `pending_merge` (skipped by `reseat_worktree`) and
> `prune_missing` refuses a root with an active registration. When this ships to
> `master`, the overlay flow becomes primary and the steps below apply only to
> pre-feature (unregistered) rows. Branch commits: 4450f20f..c2104e90. Design:
> `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md`; plan:
> `docs/superpowers/plans/2026-07-17-worktree-overlay.md`; session log:
> `docs/trackers/archive/worktree-overlay-session-log.md` (F-1..F-4, W-1).

When merging a git worktree branch that created/updated **librarian trackers**, the
catalog needs explicit reconciliation — `git merge` moves file *content* but is blind
to the catalog (`artifact id = sha256(abs_path)`; `append_entry` writes only
`artifact_augmentation.params` in the DB, never the `.md`). Rows created at the
worktree path orphan on merge.

**Do this (once the MCP binary has the tools — `cargo rb` + `/mcp` if `doctor`'s
schema lacks `worktree_scoped_row` / the `fix` enum lacks `reseat_worktree`):**
1. Merge the branch (rebase-before-merge keeps the deterministic renumber path valid).
2. `librarian(action="doctor")` → find `worktree_scoped_row` violations; read each
   `classification` (`no_collision` | `collision`).
3. **no_collision** → `librarian(action="doctor", fix="reseat_worktree")` (seeds the
   main-path id + grafts children across — durable, survives reindex).
4. **collision** → settle file content via R-3 first, then
   `doc(action="graft", from_id=<worktree-id>, into_id=<main-id>)`; check the
   returned `remap`/`suspicious`, rewrite live-tree citations + breadcrumb.
5. Verify (`doctor` clean, entries+events under the surviving id), THEN
   `git worktree remove`.

**Ordering is load-bearing: reconcile BEFORE `git worktree remove`.** Detection uses
`is_linked_worktree` (reads the worktree `.git` pointer); once removed, the row is
unrecognizable and becomes a missing-file orphan that `prune_missing` deletes.
(NOTE: the overlay flow above removes this hazard — its `worktree_registration` row
survives removal, so `merge_worktree` works from DB state alone.)

**Do NOT improvise `reindex` + manual re-augment + `prune_missing`.** A RED-baseline
agent that couldn't discover `graft`/`reseat` did exactly this — it silently DROPS the
event log (reindex makes a fresh row with no events; prune deletes the old) and relies
on hand-copying params. `reseat`/`graft` re-point events/observations/links/event_edges
+ migrate augmentation atomically; the improvised path does not.

Tools shipped on `experiments` (commits 49214372..bc1119c9 + 89fd089b); a companion
orchestration skill was investigated and REFUTED by baseline (tools + schema
discoverability suffice). Design: `docs/superpowers/specs/2026-07-10-worktree-merge-tracker-safety-design.md`.

## Cross-machine (two clones, not a worktree) — schema and params are mutually gating

Measured 2026-08-31 reconciling this desktop's catalog against the laptop's. The
worktree flow above does not cover it: a worktree has a `worktree_fork` creation
event to three-way against, and a second clone has none.

**The trap.** When two hosts have both migrated an augmentation's `params_schema`
AND its `params` to a new field shape, you cannot restore one field-group at a time
in either order. `doc(action="augment")` validates merged params against the schema, so:

- old stored schema **rejects** new params, and
- new schema **rejects** the old stored params.

Measured on `research/README` (`5086e3c7c0b9d83c`): stored schema had
`required: [file, …]` **plus `additionalProperties: false`**, while the incoming rows
carried `path` — so they failed on **two** counts at once (missing `file`, unexpected
`path`). Sanity checks confirmed old/old and new/new both accept, so the
incompatibility is exactly the swapped field. `fable-tuning-findings`
(`35de33286cd34f87`) showed the same mutual rejection on `claim` vs `title`.

**The escape is one atomic call, and it is a designed affordance rather than a
bypass.** `validate_merged_against_schema` (`src/librarian/tools/augment.rs:36-52`,
called at `:370`, comment-tagged `F-5`) validates merged params against *the schema
this call supplies*. So a single `doc(action="augment", merge=true, …)` carrying params and
`params_schema` together is fully validated against the target schema — not slipped
past a guard. The three-call alternative (permissive schema → params → real schema)
writes shape fields twice for no gain — and did, until 2026-08-31, additionally risk the
write-through clobber described below.

**Two hazards that bite here specifically.**

- **`doc(action="augment")` used to republish the WHOLE augmentation row**, so a call changing
  one field published its stale siblings over a correct sidecar. **Fixed 2026-08-31 at
  `eab7fca3`** — the write-through now refuses to republish a field the calling merge never
  authored; archived at
  `docs/issues/archive/2026-08-31-artifact-augment-write-through-republishes-the-whole-row.md`.
  The fix's own framing is the part worth keeping: this mechanism sits directly in front of
  `sidecar_shape_drift`, whose design position is that when row and committed file disagree
  **the direction is undecidable without a human** — and the write-through was silently
  deciding it. Prefer atomic calls anyway, for the schema reason above rather than this one.
- **`append_entry`'s high-water mark collides across hosts.** It already refuses id
  allocation from a *worktree* (`src/librarian/tools/append_entry.rs:97`) on exactly
  these grounds, but `is_main_checkout_artifact` cannot see a second clone. Measured:
  desktop `entry_high_water_R: 146`, laptop `147` unpushed, and both desktop allocator
  inputs resolved to 147. Open bug:
  `docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`.

**Establish sync direction empirically, never by heuristic.** "Longer field = newer"
was wrong on 5 of 9 `render_template`s — the other host had *condensed* them. What
settled it was byte-comparing one host's catalog against the committed sidecars.

**Prefer a field-level union to a wholesale copy.** Two hosts' row counts matching
(30 vs 32) says nothing about per-field content: 10 rows carried a `verdict` on one
host and `<none>` on the other, so copying either side's params would have erased ten
fields while every count looked healthy.

**`length()` on TEXT is CHARACTERS, not bytes** — `length(CAST(x AS BLOB))` is bytes.
Six wrong "N bytes" claims in one session came from this.

Full design and the rejected alternatives:
`docs/superpowers/specs/2026-08-31-cross-machine-catalog-integration-design.md`
(§ 1.3a is this deadlock). Recovery plan and its 47 steps:
`docs/superpowers/plans/2026-08-31-cross-machine-catalog-recovery.md`.

**A catalog-only task leaves no git anchor unless its event carries one.** When a task's
entire output is catalog-side — params restored, shape migrated, nothing written to a file —
there is no commit, so `doc(action="event_create")`'s `anchor_commit` / `head_commit` are the only thing
that can place the work in repo history. Measured 2026-08-31: two events recording a
cross-host schema migration were written with both fields empty, leaving a wall-clock
timestamp as the sole locator. Pass `head_commit` explicitly on any event whose task will not
produce a commit.
