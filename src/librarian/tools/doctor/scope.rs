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
pub(super) struct DoctorScope {
    // Read only from `#[cfg(test)]` code today (a plain-lib build never
    // branches on it), so it trips `dead_code` under `-D warnings` without
    // this annotation. Kept per the Task 2 interface spec — Task 7 (and any
    // later umbrella-specific hint) is the intended production reader.
    #[allow(dead_code)]
    pub scope: Scope,
    /// Managed roots for the lexical half. Empty means `Scope::All` —
    /// everything is in scope and [`Self::contains`] short-circuits to `true`.
    roots: Vec<PathBuf>,
    /// Rows refused by [`Self::admit`], tallied by [`super::outside_roots_group`]
    /// so Ruling 17's aggregate can still account for them even though they
    /// never became a `Violation`.
    scoped_out: BTreeMap<String, usize>,
}

impl DoctorScope {
    pub(super) fn new(scope: Scope, ctx: &ToolContext) -> Result<Self> {
        // `apply_scope` owns the umbrella lookup and the worktree-overlay OR
        // clause; re-deriving either here is how the two definitions of
        // "project" diverged in the first place. Its `FilterNode` output is
        // no longer consumed here (see the module doc) — only its umbrella
        // validation (an unconfigured umbrella must still error) is wanted,
        // which `apply_scope` performs regardless of what the caller does
        // with the returned filter.
        let (_filter, _applied) = apply_scope(
            None,
            scope,
            &ctx.workspace,
            ctx.current_project.as_deref(),
            &[],
        )?;

        // The lexical twin of that clause. `Scope::Project` over a linked
        // worktree spans BOTH the worktree and its main checkout, matching
        // `apply_scope`'s deliberate over-selection.
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

    /// Lexical predicate for a path already in hand. `Scope::All` (empty
    /// `roots`) admits everything.
    pub(super) fn contains(&self, abs_path: &Path) -> bool {
        if self.roots.is_empty() {
            return true;
        }
        containing_root(&self.roots, abs_path).is_some()
    }

    /// `contains`, and on `false` tallies the row under its project group so
    /// Ruling 17's global metric still sees it.
    ///
    /// `id` is accepted but not yet consumed here — Task 7's
    /// cited-from-here exemption reads it to decide whether a row scoped out
    /// of the worklist is nonetheless cited by something in scope. Taking it
    /// from the start means every one of the ~13 call sites Tasks 3–5 add
    /// already carries the id `admit` will need, rather than risking a
    /// signature change later that silently misses one of them.
    pub(super) fn admit(&mut self, id: &str, abs_path: &str) -> bool {
        let _ = id;
        if self.contains(Path::new(abs_path)) {
            return true;
        }
        *self
            .scoped_out
            .entry(super::outside_roots_group(abs_path))
            .or_insert(0) += 1;
        false
    }

    pub(super) fn scoped_out(&self) -> &BTreeMap<String, usize> {
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
/// here is unreachable, not silently tolerated.
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
    fn scope_all_admits_every_path_and_binds_no_sql() {
        let ctx = unscoped_ctx();
        let mut s = DoctorScope::new(Scope::All, &ctx).unwrap();
        assert!(s.admit("a1", "/anywhere/at/all/docs/x.md"));
        assert!(s.scoped_out().is_empty());
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
        assert!(s.admit("mine", &root.join("docs/mine.md").to_string_lossy()));
        assert!(!s.admit("theirs", "/home/other/repo/docs/theirs.md"));
        assert_eq!(s.scoped_out().values().sum::<usize>(), 1);
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
        assert!(!s.admit("sib", &sibling));
    }
}
