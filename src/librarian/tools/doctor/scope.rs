//! One scoping unit for every `doctor` check.
//!
//! Before this module, Ruling 17 — *the metric stays global, the worklist is
//! the active developer's* — was implemented five times in five shapes: an
//! in-loop known-roots filter, four per-scan `ctx` filters each with its own
//! tally map, a fifth of that shape for `cited_prefix_with_no_definer`, a
//! post-hoc `retain` over a seven-string list of check names, and six inline
//! `containing_root` calls that narrowed correctly and **published no count at
//! all**. Two of them keyed off `cp.git_root` while `scope::Scope::Project`
//! keys off `cp.abs_path`, so "scoped to the project" meant two different
//! things inside one tool.
//!
//! This module carries the lexical half only — a predicate ([`DoctorScope::contains`])
//! and a tallying refusal ([`DoctorScope::admit`]) for scans that already hold a path
//! from a shared row loop. **SQL-layer narrowing was withdrawn by Ruling A
//! (2026-09-09)** — see the struct doc comment below for why a scoped-out tally and a
//! query that narrows its own fetch are mutually exclusive at one call site.
//!
//! **2026-09-09 review, round 2 (Critical 1 / Important 1):** the first cut of
//! `admit` tallied by root only, with no record of WHICH check refused the row.
//! That was silently wrong the moment a second call site existed: this module's
//! only consumer at the time (`abs_path_outside_managed_roots`, inside
//! `scan_artifact_paths`) got its scoped-out rows folded into
//! `row_checks_scoped_by_project` — a map whose hint text names seven entirely
//! different checks — instead of into `outside_roots_by_project` /
//! `outside_roots_scoped_by_project`, the pair that check actually belongs to.
//! `scoped_out` is now keyed by check name first, so each caller folds only its
//! own check's contribution into its own destination map.
//!
//! Two modules share the name `scope` in this tool. `src/librarian/tools/scope.rs`
//! (imported here as `shared_scope`) is the tool-wide `Scope` enum plus
//! `apply_scope`/`resolve_scope`; `src/librarian/tools/doctor/scope.rs` (this
//! file) is `doctor`'s own scoping unit built on top of it. From inside this
//! file `super::scope` names *this* module (`doctor::scope`), one level of
//! `super` short of the shared one — `super::super::scope` is the shared
//! module. Call sites here go through the `shared_scope` alias instead of
//! spelling that out each time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::librarian::tools::scope as shared_scope;
use crate::librarian::tools::{containing_root, ToolContext};

use shared_scope::{apply_scope, Scope};

/// One scoping decision, carried in the lexical form a scan already holding a
/// path can test directly (`contains`/`admit`).
///
/// **SQL-layer narrowing is deliberately not carried here.** An earlier
/// revision of this module also compiled a `WHERE` fragment alongside `roots`
/// so a scan issuing its own query could splice it straight in — Ruling A
/// (2026-09-09) withdrew that half: a scoped-out tally requires *observing*
/// the rows you excluded, and a query that narrows its own fetch never reads
/// the rows it dropped, so it cannot count them. SQL-layer narrowing and a
/// published scoped-out count are mutually exclusive at one call site — you
/// cannot count what you never fetched. The half stays *available* to a
/// future scan with its own dedicated query, but only paired with its own
/// precondition: that scan must issue a separate `COUNT(*) ... WHERE NOT
/// (predicate)` query, grouped by root, to produce its own scoped-out count.
/// Nothing here can stand in for a fetch that never happened.
#[derive(Debug)]
pub(super) struct DoctorScope {
    // Read only from `#[cfg(test)]` code today (a plain-lib build never
    // branches on it), so it trips `dead_code` under `-D warnings` without
    // this annotation. Kept per the Task 2 interface spec — Task 7 (and any
    // later umbrella-specific hint) is the intended production reader.
    #[allow(dead_code)] // Task 7 removes this
    pub scope: Scope,
    /// Managed roots for the lexical half. Empty means everything is in scope and
    /// [`Self::contains`] short-circuits to `true` — and this is `Scope::All`'s case
    /// EXCLUSIVELY, not merely its normal one. `Scope::Project`/`Scope::Repo`/
    /// `Scope::Umbrella` with no active project (`ctx.current_project.is_none()`) do
    /// NOT reach the `if let Some(cp) = ...` guard in [`Self::new`] with `roots` staying
    /// empty — [`Self::new`] calls `apply_scope(..)?` first, and `apply_scope`'s own
    /// `require()` guard errors out on exactly that combination (`"scope=<name> requires
    /// an active project"`) before the roots-building code below it ever runs. So an
    /// empty `roots` here can ONLY mean `Scope::All`; there is no second route to it.
    /// This corrects a 2026-09-09 round-3 review claim (Important 2) that assumed the
    /// second route existed; disproven by `scope_project_without_an_active_project_admits_everything`,
    /// which now asserts the refusal (`expect_err`) rather than the originally-proposed
    /// `unwrap()`.
    roots: Vec<PathBuf>,
    /// Rows refused by [`Self::admit`], tallied first by CHECK NAME then by
    /// [`super::outside_roots_group`] root. Keyed by check because Ruling
    /// 17's per-check aggregates (`outside_roots_by_project`,
    /// `entry_validity_scoped_by_project`, `cited_prefix_scoped_by_project`,
    /// ...) are each one check's own map — a flat root->count total cannot
    /// feed more than one of them without silently attributing one check's
    /// drop to another (2026-09-09 review, Important 1). A caller reads its
    /// own check's sub-map via `scoped_out().get(check)` and folds it into
    /// that check's own destination — never into a shared map built for a
    /// different set of checks.
    scoped_out: BTreeMap<String, BTreeMap<String, usize>>,
}

impl DoctorScope {
    pub(super) fn new(scope: Scope, ctx: &ToolContext) -> Result<Self> {
        // `apply_scope` owns the umbrella lookup and the worktree-overlay OR
        // clause; re-deriving either here is how the two definitions of
        // "project" diverged in the first place. Its `FilterNode` output is
        // no longer consumed here (see the module doc) — only its umbrella
        // validation (an unconfigured umbrella must still error, and an
        // umbrella scope with no active project must still error) is wanted,
        // which `apply_scope` performs regardless of what the caller does
        // with the returned filter.
        let (_filter, _applied) = apply_scope(
            None,
            scope,
            &ctx.workspace,
            ctx.current_project.as_deref(),
            &[],
        )?;

        // What this builds: for `Project`/`Repo`, an exact lexical twin of
        // `apply_scope`'s clause — `Scope::Project` over a linked worktree
        // spans BOTH the worktree and its main checkout, matching
        // `apply_scope`'s deliberate over-selection there.
        //
        // For `Umbrella`, this is deliberately WIDER than `apply_scope`'s
        // clause (2026-09-09 review, Important 3). `apply_scope`'s Umbrella
        // arm is `or_of_prefixes(&umb.members)` alone — nothing else — so it
        // silently assumes the umbrella's own member list already includes
        // the active project. `doctor` does not rely on that assumption:
        // `cp.abs_path` (and `main_root`, if any) are always included as a
        // floor before the umbrella members are unioned in, so a
        // workspace.toml that groups siblings without re-listing the active
        // project still leaves doctor seeing its own rows. A doctor scan
        // going blind on the very project running it is worse than
        // over-reporting a member-list gap. Asserted by
        // `scope_umbrella_floors_on_the_active_project_and_widens_via_declared_members`.
        let mut roots = Vec::new();
        if !matches!(scope, Scope::All) {
            if let Some(cp) = ctx.current_project.as_deref() {
                let primary = match scope {
                    Scope::Repo => &cp.git_root,
                    _ => &cp.abs_path,
                };
                roots.push(primary.clone());
                if let Some(main) = &cp.main_root {
                    roots.push(main.clone());
                }
                if matches!(scope, Scope::Umbrella) {
                    roots.extend(umbrella_member_roots(ctx));
                }
            }
        }

        Ok(Self {
            scope,
            roots,
            scoped_out: BTreeMap::new(),
        })
    }

    /// Lexical predicate for a path already in hand. Empty `roots` admits everything, and
    /// (per the `roots` field doc) that state is `Scope::All` exclusively — not "any scope
    /// with no active project," which is instead refused at [`Self::new`] construction time.
    pub(super) fn contains(&self, abs_path: &Path) -> bool {
        if self.roots.is_empty() {
            return true;
        }
        containing_root(&self.roots, abs_path).is_some()
    }

    /// `contains`, and on `false` tallies the row under `check` and its
    /// project group so Ruling 17's per-check global metric still sees it.
    ///
    /// `check` names which finding this refusal is for (e.g.
    /// `"abs_path_outside_managed_roots"`) — every caller's own destination
    /// map is keyed by check, and a flat count could not tell two checks'
    /// drops apart (2026-09-09 review, Important 1).
    ///
    /// `id` is accepted but not yet consumed here — Task 7's
    /// cited-from-here exemption reads it to decide whether a row scoped out
    /// of the worklist is nonetheless cited by something in scope. Taking it
    /// from the start means every one of the ~13 call sites Tasks 3–5 add
    /// already carries the id `admit` will need, rather than risking a
    /// signature change later that silently misses one of them.
    ///
    /// **Not every call site carries the same KIND of `id`/`abs_path` pair.** Sites 1-4
    /// (the entry-validity family) pass a row's own artifact id and its own path — the
    /// uniform shape this paragraph originally assumed for all ~13 eventual callers.
    /// `cited_prefix_with_no_definer` (Task 3's fifth site) does not: it is a per-PREFIX
    /// check with no single owning row, so it passes the namespace prefix as `id` and one
    /// of the prefix's citers OUTSIDE the active project as `abs_path` — never the
    /// finding's own artifact or path. A future Task 7 reader of `id` must branch on
    /// `check` rather than assume an artifact id is always available (2026-09-09 review,
    /// Important 4 — the reviewer's own planning error, not the implementer's: Ruling C
    /// assumed the uniform shape before this site existed).
    pub(super) fn admit(&mut self, check: &str, id: &str, abs_path: &str) -> bool {
        // 2026-09-09 review, round 2, PROMOTED: `Violation::new` (`src/librarian/tools/
        // doctor.rs`) validates its own `check` argument this same way, on the
        // reasoning that an undeclared name is a programming error, not data — a
        // typo'd check name here would tally silently into a `scoped_out` bucket no
        // fold ever reads, which is the exact false-negative Ruling 17 forbids, just
        // one layer earlier than the fold itself.
        debug_assert!(
            super::Check::from_wire(check).is_some(),
            "undeclared doctor check name {check:?} — add it to declare_checks! or \
             scoped_out() will hold a bucket no fold ever reads"
        );
        let _ = id;
        if self.contains(Path::new(abs_path)) {
            return true;
        }
        *self
            .scoped_out
            .entry(check.to_string())
            .or_default()
            .entry(super::outside_roots_group(abs_path))
            .or_insert(0) += 1;
        false
    }

    /// Rows refused by [`Self::admit`], keyed by check name then by project
    /// root. A caller folds one check's sub-map (`.get(check)`) into that
    /// check's own `catalog_health` aggregate — never the whole map into a
    /// single shared one.
    pub(super) fn scoped_out(&self) -> &BTreeMap<String, BTreeMap<String, usize>> {
        &self.scoped_out
    }
}

/// Mirrors `apply_scope`'s `Scope::Umbrella` arm (`src/librarian/tools/scope.rs`)
/// exactly: same lookup (`ctx.current_project`'s umbrella name against
/// `ctx.workspace.umbrellas`), same member list. Must not approximate this —
/// a lexical roots list that disagrees with `apply_scope`'s own umbrella
/// resolution is exactly the failure this mirroring exists to prevent, and by
/// the time this runs `DoctorScope::new` has already propagated any
/// `apply_scope` error for an unconfigured umbrella — so a missing umbrella
/// here is unreachable, not silently tolerated. (`DoctorScope::new` layers its
/// own `cp.abs_path`/`main_root` floor on TOP of this list's output — see the
/// comment there; that floor is this module's own deliberate addition, not
/// part of the mirror.)
fn umbrella_member_roots(ctx: &ToolContext) -> Vec<PathBuf> {
    let Some(name) = ctx
        .current_project
        .as_deref()
        .and_then(|cp| cp.umbrella.as_deref())
    else {
        return Vec::new();
    };
    ctx.workspace
        .umbrellas
        .iter()
        .find(|u| u.name == name)
        .map(|u| u.members.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Umbrella;
    use std::sync::Arc;

    /// A ctx with no active project. `Scope::All` needs no project, and this
    /// keeps that property visible at the fixture rather than folding it into
    /// a project-shaped helper that happens to also work here.
    fn unscoped_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    /// A ctx whose active project is rooted at `root`, backed by its own
    /// throwaway in-memory catalog (accessible via `ctx.catalog`).
    fn ctx_at(root: &Path) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        let cp = Arc::new(CurrentProject {
            abs_path: root.to_path_buf(),
            git_root: root.to_path_buf(),
            main_root: None,
            umbrella: None,
        });
        TestToolContextBuilder::new(cat)
            .with_current_project(cp)
            .build()
    }

    /// `Scope::All` must narrow NOTHING — the property that keeps `scope="all"` a real
    /// widening rather than a differently-shaped default.
    #[test]
    fn scope_all_admits_every_path_and_tallies_nothing() {
        let ctx = unscoped_ctx();
        let mut s = DoctorScope::new(Scope::All, &ctx).unwrap();
        assert!(s.admit(
            "abs_path_outside_managed_roots",
            "a1",
            "/anywhere/at/all/docs/x.md"
        ));
        assert!(s.scoped_out().is_empty());
    }

    /// `Scope::Project` (or `Repo`/`Umbrella`) with `ctx.current_project == None` is REFUSED
    /// at construction, not degraded to empty roots.
    ///
    /// This corrects a 2026-09-09 round-3 review claim (Important 2), which asserted this
    /// combination reaches `DoctorScope::new`'s roots-building code with `roots` silently
    /// staying empty — the same failure mode as `Scope::All` — and that a prior test claiming
    /// to cover "no active project" via `Scope::All` therefore missed the real arm. Verified
    /// here instead: `DoctorScope::new` calls `apply_scope(..)?` BEFORE building `roots` at
    /// all, and `apply_scope`'s own `require()` guard errors out on `Scope::Project` with no
    /// current project (`"scope=project requires an active project"`) — so the roots-building
    /// `if let Some(cp) = ctx.current_project.as_deref()` branch this review's fix targeted is
    /// unreachable for this exact combination; `?` returns `Err` first. The `Scope::All` route
    /// in `cited_but_undeclared_reports_everything_when_there_is_no_active_project` was already
    /// correct: it is not a weaker stand-in for "no active project", it is the ONLY scope
    /// `DoctorScope` can hold when there is no active project, because `call()`'s own
    /// `resolve_scope` (`src/librarian/tools/scope.rs`) rewrites the `Scope::Project`/
    /// `Scope::Repo`, no-active-project case to `(Scope::All, true)` upstream of ever
    /// constructing a `DoctorScope`. Reproduced by literally applying the review's proposed
    /// fix and observing the panic before writing this corrected version. (Kept under its
    /// original name rather than renamed, so a citation of the review's own fix instruction
    /// still resolves to the test that corrects it.)
    #[test]
    fn scope_project_without_an_active_project_admits_everything() {
        let ctx = unscoped_ctx();
        assert!(
            ctx.current_project.is_none(),
            "this test's whole point is the no-active-project arm"
        );
        let err = DoctorScope::new(Scope::Project, &ctx)
            .expect_err("Scope::Project with no active project must be refused, not degraded");
        assert!(
            err.to_string().contains("requires an active project"),
            "{err}"
        );
    }

    /// The discriminating property, at unit grain: a foreign path is refused AND
    /// counted. Counting is not incidental — Ruling 17 requires the metric to survive
    /// the narrowing, and a dropped row that is not tallied is invisible.
    #[test]
    fn scope_project_refuses_a_foreign_path_and_tallies_it() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let ctx = ctx_at(&root);

        let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        assert_eq!(s.scope, Scope::Project, "the requested scope is retained, for callers that need to branch on it (e.g. an Umbrella-specific hint)");
        assert!(s.admit(
            "abs_path_outside_managed_roots",
            "mine",
            &root.join("docs/mine.md").to_string_lossy()
        ));
        assert!(!s.admit(
            "abs_path_outside_managed_roots",
            "theirs",
            "/home/other/repo/docs/theirs.md"
        ));
        assert_eq!(
            s.scoped_out()
                .values()
                .flat_map(|m| m.values())
                .sum::<usize>(),
            1
        );
        assert!(
            s.contains(&root.join("docs/mine.md")),
            "contains must agree with admit"
        );
    }

    /// The component-boundary guarantee `containing_root` provides, restated here
    /// because `DoctorScope` is the layer callers now use. `/proj/sub` must not be
    /// treated as contained by `/proj/subterfuge`.
    #[test]
    fn a_sibling_directory_with_a_shared_prefix_is_not_in_scope() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let ctx = ctx_at(&root);
        let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        let sibling = format!("{}terfuge/docs/x.md", root.to_string_lossy());
        assert!(!s.admit("abs_path_outside_managed_roots", "sib", &sibling));
    }

    /// `Scope::Repo => &cp.git_root` — no other test in this file constructs
    /// `Scope::Repo`, and `ctx_at` sets `abs_path == git_root`, so it could not
    /// discriminate `Repo` from `Project` even if it did (2026-09-09 review,
    /// Important 3). A monorepo-shaped fixture: `abs_path` is a SUBDIRECTORY of
    /// `git_root`, so a sibling subdirectory's row is outside `Project` scope
    /// but inside `Repo` scope.
    #[test]
    fn scope_repo_admits_a_path_under_git_root_that_project_scope_refuses() {
        let tmp = tempfile::tempdir().unwrap();
        let git_root = tmp.path().join("repo");
        let abs_path = git_root.join("packages/mine");
        std::fs::create_dir_all(&abs_path).unwrap();
        let sibling_pkg_file = git_root.join("packages/theirs/docs/x.md");
        let cat = Catalog::open_in_memory().unwrap();
        let cp = Arc::new(CurrentProject {
            abs_path: abs_path.clone(),
            git_root: git_root.clone(),
            main_root: None,
            umbrella: None,
        });
        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(cp)
            .build();

        let mut project_scope = DoctorScope::new(Scope::Project, &ctx).unwrap();
        assert!(
            !project_scope.admit("abs_path_outside_managed_roots", "sib", &sibling_pkg_file.to_string_lossy()),
            "a sibling package under git_root but outside abs_path must be refused at Project scope"
        );

        let mut repo_scope = DoctorScope::new(Scope::Repo, &ctx).unwrap();
        assert!(
            repo_scope.admit(
                "abs_path_outside_managed_roots",
                "sib",
                &sibling_pkg_file.to_string_lossy()
            ),
            "the same path must be admitted at Repo scope — it is under git_root"
        );
    }

    /// `if let Some(main) = &cp.main_root` — no other test in this file sets
    /// `main_root: Some(..)`, so deleting this branch previously stayed green.
    /// It is the only thing keeping a worktree session's MAIN-checkout rows in
    /// the `Project`-scope worklist (2026-09-09 review, Important 3).
    #[test]
    fn scope_project_includes_the_main_checkout_when_a_main_root_is_set() {
        let tmp = tempfile::tempdir().unwrap();
        let main_root = tmp.path().join("main");
        let worktree_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(&worktree_root).unwrap();
        let main_only_file = main_root.join("docs/mine.md");
        let cat = Catalog::open_in_memory().unwrap();
        let cp = Arc::new(CurrentProject {
            abs_path: worktree_root.clone(),
            git_root: worktree_root.clone(),
            main_root: Some(main_root.clone()),
            umbrella: None,
        });
        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(cp)
            .build();

        let mut s = DoctorScope::new(Scope::Project, &ctx).unwrap();
        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "main-row",
                &main_only_file.to_string_lossy()
            ),
            "a row under the main checkout must be admitted from a worktree session's \
             Project scope, not just rows under the worktree itself"
        );
    }

    /// `Scope::Umbrella => roots.extend(umbrella_member_roots(ctx))` — zero
    /// coverage before this test, and production-reachable: with a configured
    /// umbrella, `resolve_scope` aliases an explicit `scope="all"` to `Umbrella`
    /// (2026-09-09 review, Important 3). Also exercises the deliberate widening
    /// documented on `DoctorScope::new`: the umbrella's OWN member list here does
    /// NOT include the active project's `abs_path` — `doctor` still admits the
    /// active project's own rows as a floor, unlike `apply_scope`'s Umbrella
    /// clause, which would not.
    #[test]
    fn scope_umbrella_floors_on_the_active_project_and_widens_via_declared_members() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = tmp.path().join("mine");
        let sibling = tmp.path().join("sibling");
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let cp = Arc::new(CurrentProject {
            abs_path: mine.clone(),
            git_root: mine.clone(),
            main_root: None,
            umbrella: Some("team".to_string()),
        });
        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(cp)
            .with_umbrellas(vec![Umbrella {
                name: "team".to_string(),
                members: vec![sibling.clone()], // deliberately omits `mine`
            }])
            .build();

        let mut s = DoctorScope::new(Scope::Umbrella, &ctx).unwrap();
        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "mine-row",
                &mine.join("docs/x.md").to_string_lossy()
            ),
            "the active project's own rows must stay in scope even when the umbrella's \
             member list omits it — the floor DoctorScope::new adds on top of \
             umbrella_member_roots"
        );
        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "sib-row",
                &sibling.join("docs/x.md").to_string_lossy()
            ),
            "a declared umbrella member's rows must be admitted too"
        );
        assert!(
            !s.admit(
                "abs_path_outside_managed_roots",
                "far-row",
                "/nowhere/near/either/docs/x.md"
            ),
            "a path outside both the active project and the declared umbrella members \
             stays out of scope"
        );
    }

    /// `apply_scope`'s `require(current, "umbrella")` error, propagated through
    /// `DoctorScope::new` (which calls `apply_scope` first, purely for this
    /// validation). No test before this one constructed `Scope::Umbrella` with
    /// no active project at all.
    #[test]
    fn umbrella_scope_without_an_active_project_is_refused() {
        let ctx = unscoped_ctx();
        let err = DoctorScope::new(Scope::Umbrella, &ctx).unwrap_err();
        assert!(
            err.to_string().contains("active project"),
            "refusal must name the missing project: {err}"
        );
    }

    /// The other `apply_scope` Umbrella error path: an active project with no
    /// umbrella declared at all (`cp.umbrella: None`).
    #[test]
    fn umbrella_scope_without_a_declared_umbrella_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        std::fs::create_dir_all(&root).unwrap();
        let ctx = ctx_at(&root); // umbrella: None
        let err = DoctorScope::new(Scope::Umbrella, &ctx).unwrap_err();
        // 2026-09-09 review, round 2, TAKE 1: `.contains("umbrella")` alone cannot
        // discriminate this error from the SIBLING test's ("scope=umbrella requires an
        // active project", which also contains the literal substring "umbrella" via
        // its `scope={}` interpolation) — this test would pass even if `DoctorScope::
        // new` returned the wrong one of the two umbrella error paths. "no umbrella
        // declared" is unique to this path (`src/librarian/tools/scope.rs`'s
        // `"scope=umbrella but no umbrella declared for {}..."`).
        assert!(
            err.to_string().contains("no umbrella declared"),
            "refusal must name the no-declared-umbrella error, not just mention the word \
             \"umbrella\" (which the sibling test's error also does): {err}"
        );
    }
}
