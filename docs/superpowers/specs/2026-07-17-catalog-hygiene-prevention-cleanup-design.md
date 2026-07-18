# Design — Catalog hygiene: temp-write prevention + batch dead-root cleanup

- **Date:** 2026-07-17
- **Status:** approved (design)
- **Branch:** experiments
- **Fixes / relates to:**
  - `docs/issues/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md` (bug #3)
  - `docs/issues/2026-07-17-catalog-dead-rows-no-gc.md` (bug #2 — cleanup half only; lifecycle deferred)

## Problem

The shared global librarian catalog (`~/.local/share/librarian/catalog.db`) has accreted
dead rows with no lifecycle:

- **179 `missing_file` rows** — catalog rows whose `abs_path` no longer exists on disk
  (deleted/renamed repos: `stefanini/AI-enablement` 77, `stefanini/IATA` 52,
  `stefanini/southpole` 11, `mirela/deployment` 5, other `/home` 6), plus
- **28 `/tmp` rows** — probe/test runs that wrote artifacts into the real global catalog
  (3 still `kind=tracker, status=active` **with augmentation rows**), from `mktemp`-style
  dirs that are long gone.

`doctor` detects all of this but nothing runs it automatically, and `find`/`search` return
these ghosts. Two root problems:

1. **No prevention** — a run whose workspace lives under the OS temp dir writes straight
   into whatever catalog is configured, including the real persistent one.
2. **No usable cleanup** — `fix=prune_missing` takes one `root=` per call and refuses roots
   that still exist, so removing 6+ dead clusters is 6+ hand-typed invocations, and there is
   no batch path.

## Scope

**In scope**

1. **Prevention** — refuse artifact writes that would put a temp-dir-rooted artifact into a
   persistent catalog (bug #3, code-only).
2. **Batch cleanup capability** — a dry-run-first batch prune over all dead roots `doctor`
   finds, reusing the existing safe per-root primitive (enables removing the 179 + 28 rows).

**Out of scope (deferred to a follow-up spec)**

The ongoing GC *lifecycle*: `missing_since` timestamping, hide-from-find after N days,
time-based auto-prune after M days, and surfacing the `doctor` summary in
`workspace(action="status")` / the SessionStart banner. The predecessor auto-GC was removed
for over-aggression; a lifecycle policy deserves its own design once prevention has shown
whether the catalog stays clean on its own.

## Component 1 — Prevention guard

**Placement.** A shared helper invoked at the two write entry points that introduce *new*
catalog rows:

- `src/librarian/tools/create.rs::call` — guard the resolved `base_dir` (the workspace root
  the artifact is written under; it always exists, so it canonicalizes cleanly).
- `src/librarian/tools/reindex.rs::call` — guard each `target` scan root before the walk.

`update` / `augment` / `append_entry` / `event_create` act on an existing artifact `id`; if
`create` and `reindex` cannot introduce a temp-rooted row, there is nothing for them to
mutate. **Documented assumption:** guarding the two introduction points is sufficient; a
future path that introduces rows another way must call the same helper.

**Predicate — refuse the write when BOTH hold:**

1. The workspace root being written (create's `base_dir`, or a reindex `target`) is under
   `std::env::temp_dir()` (canonicalized comparison); AND
2. The catalog is the **real / shared** one — its backing DB file exists **and lives outside**
   the temp dir. Detected via `PRAGMA database_list`: the `main` database's `file` column is
   non-empty (file-backed, not in-memory) **and** that file is not itself under
   `std::env::temp_dir()`.

**Why condition 2 is "outside temp", not merely "file-backed".** Server / integration tests
are the load-bearing case: they run against a *file-backed* catalog whose DB lives **under the
test's own `TempDir`** (`src/server.rs`: `db: Some(dir.path().join("librarian.db"))`), paired
with `TempDir` workspaces. A bare "file-backed" predicate would refuse those legitimate creates
and break the suite. Requiring the catalog file to be *outside* temp lets an isolated /
ephemeral catalog (test or scratch, under temp) accept temp-workspace writes, while the guard
still fires for the real pollution vector: a temp workspace writing into the shared
`~/.local/share/librarian/catalog.db`, which is outside temp. Unit-level librarian tests use
`Catalog::open_in_memory()` (no file) and are likewise unaffected.

**Escape hatch.** `CODESCOUT_ALLOW_TEMP_WORKSPACE=1` bypasses the guard, for the rare
legitimate case of a real session working a scratch project under the temp dir.

**Error.** `RecoverableError` (non-fatal, correctable) whose hint names both the opt-in env
var and the isolated-catalog alternative for test harnesses.

**Sketch.**

```rust
/// The catalog's backing DB file, or None for an in-memory connection.
/// PRAGMA database_list yields (seq, name, file); `main.file` is "" in-memory.
fn catalog_db_path(conn: &rusqlite::Connection) -> Option<std::path::PathBuf> { /* ... */ }

fn is_under_temp(path: &Path) -> bool {
    let temp = std::fs::canonicalize(std::env::temp_dir())
        .unwrap_or_else(|_| std::env::temp_dir());
    std::fs::canonicalize(path)
        .map(|p| p.starts_with(&temp))
        .unwrap_or_else(|_| path.starts_with(&temp))
}

/// Refuse iff the workspace root is under temp AND the catalog is the real one
/// (file-backed, outside temp) AND the caller did not opt in.
fn guard_temp_workspace_write(root: &Path, conn: &rusqlite::Connection) -> anyhow::Result<()> {
    if std::env::var_os("CODESCOUT_ALLOW_TEMP_WORKSPACE").is_some() {
        return Ok(());
    }
    let catalog_is_real = catalog_db_path(conn).is_some_and(|p| !is_under_temp(&p));
    if is_under_temp(root) && catalog_is_real {
        return Err(RecoverableError::with_hint(
            "refusing to write an artifact rooted under the system temp dir into the shared \
             persistent catalog — this is how probe/test runs pollute the shared catalog",
            "Use an isolated catalog (under the temp dir, or in-memory) for tests, or set \
             CODESCOUT_ALLOW_TEMP_WORKSPACE=1 if this scratch workspace is intentional.",
        )
        .into());
    }
    Ok(())
}
```
## Component 2 — Batch dead-root cleanup

**Shape.** Extend `librarian(action="doctor")`'s existing `fix="prune_missing"` — **no new
tool, no new deletion semantics**. When called **without `root=`**, it runs in batch mode.

**Dead-root derivation rule (the safety core).** From the `missing_file` scan, include a
missing artifact only if its **parent directory is also missing** — i.e. a whole subtree is
gone, not a single file under a live directory. For each included artifact, the **dead root**
is the *highest nonexistent ancestor whose parent still exists*. Missing artifacts whose
parent directory *does* exist (a single deleted/renamed file under a live repo) are
**excluded** and left to `reindex`'s per-file walk — reproducing, automatically, the
philosophy of today's per-root existence gate.

Worked against the current catalog:

- `/tmp/tmp.XYZ/probe.md` → parent `/tmp/tmp.XYZ` gone, `/tmp` exists → dead root
  `/tmp/tmp.XYZ`. ✅ pruned.
- `…/stefanini/AI-enablement/docs/x.md` → whole repo gone, `…/stefanini` exists → dead root
  `…/stefanini/AI-enablement`. ✅ pruned.
- `mirela/deployment/…` "file deletions" under a *live* repo → parent exists → **excluded**,
  left for reindex. ✅ correctly not touched.

**Modes.**

- **Dry-run (default).** Return the derived dead roots with per-root artifact/commit row
  counts and totals. No mutation.
- **Apply (`confirm=true`).** Iterate the dead roots, pruning each through the existing
  `prune_dead_root`, which retains its own guards (root must not exist; refuse a root an
  ACTIVE `worktree_registration` still covers). Return pruned counts per root + totals.

**Idempotent** — a second apply finds fewer/no dead roots. Reuses `prune_dead_root`, so no
new SQL deletion path is introduced.

## Data flow

```
create / reindex
  └─ resolve workspace root ─→ guard_temp_workspace_write(root, conn) ─→ refuse | proceed

doctor(fix="prune_missing")            # no root=
  └─ scan missing_file
      └─ derive dead roots (parent-also-gone rule)
          ├─ dry-run (default):  report roots + counts        # no mutation
          └─ confirm=true:       prune_dead_root per root      # existing guarded primitive
```

## Error handling

- **Prevention:** `RecoverableError` — non-fatal, does not abort sibling parallel calls;
  correctable via env var or an isolated catalog.
- **Cleanup dry-run:** never mutates.
- **Cleanup apply:** per-root guards preserved; a root that reappeared or carries an active
  worktree registration is skipped/reported, not force-pruned.

## Testing

**Prevention**

- `catalog_db_path`: in-memory → `None`; file-backed (tempfile) → `Some(path)`.
- temp-dir root + **real** catalog (DB file **outside** temp) → write **refused**.
- temp-dir root + catalog whose DB file is **under** temp (isolated) → **allowed** — this is
  the case that keeps server / integration tests green.
- temp-dir root + **in-memory** catalog → **allowed**.
- non-temp root + real catalog → allowed.
- opt-in decision (`should_refuse(under_temp, real_catalog, opted_in=true) == false`) covers
  the `CODESCOUT_ALLOW_TEMP_WORKSPACE` bypass without mutating global env in tests.

**Cleanup**

- Seed rows under (a) a gone root (parent also gone), (b) a single missing file under a live
  directory, (c) a live root. Dry-run lists only (a)'s dead root; apply prunes (a) and leaves
  (b) and (c). A worktree-registered dead root is skipped by the inherited guard.

## Execution / rollout (destructive step is gated)

- Implementation ships **capability + dry-run only**. Prevention is pure code (no data
  mutation).
- Pruning the **real** global catalog is a **separate, explicitly-approved step**: run the
  dry-run, present the dead roots + counts, and apply only on the user's confirm.

## Bug bookkeeping

- Bug #3 **prevention** → fixed by Component 1.
- Bug #3 **cleanup** (28 `/tmp`) + bug #2 **cleanup** (179 dead) → *enabled* by Component 2,
  *executed* in the gated rollout step.
- Bug #2 **ongoing-GC-lifecycle** remainder → stays open, marked deferred to a follow-up spec.
