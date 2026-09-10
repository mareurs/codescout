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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::librarian::tools::link_scan::diff::CITES_REL;
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
    // Read by `reseat_worktree` (`src/librarian/tools/doctor.rs`), which builds
    // this fix's response `"scope"` field from `scope.scope` via
    // `ScopeApplied::to_json()` — the same shape the report path emits. Task 6
    // (2026-09-09) made this the field's first production reader; before that
    // it was read only from `#[cfg(test)]` code and needed `#[allow(dead_code)]`
    // to pass `-D warnings`. Re-verify with `cargo clippy` before re-adding that
    // annotation if this field's only reader is ever removed again.
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
    /// second route existed; disproven by `scope_project_without_an_active_project_is_refused`,
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
    /// Task 7 (relevance exemption): artifact ids living OUTSIDE `roots` that are
    /// cited, `rel="cites"`, by an artifact INSIDE `roots` — computed once in
    /// [`Self::new`] from BOTH cites tables (`artifact_link`, artifact grain, and
    /// `entry_cite`, entry grain; see `cited_from_here` below for why both). `admit`
    /// treats membership here as an admission ticket for a row-grain check whose `id`
    /// is a real artifact id: a foreign row a local artifact actively cites is not
    /// noise the active developer can ignore, so it should surface even though it
    /// physically lives outside `roots`. Empty whenever `roots` is empty (`Scope::All`
    /// already admits everything, so the exemption has nothing to add) — [`Self::new`]
    /// skips the query entirely in that case rather than computing an exemption set
    /// nothing will ever consult.
    cited_from_here: BTreeSet<String>,
    /// Count of `cites` edges (both tables, summed) whose citing side is inside
    /// `roots` and whose cited side is outside — the denominator this exemption ships
    /// with. Measured 2026-09-09 at 0 on today's catalog (no `entry_cite` or
    /// `artifact_link` row crosses a scope boundary), so the exemption itself is inert
    /// until an operator creates one; publishing this count alongside it is what keeps
    /// that inertness visible instead of decorative. See `catalog_health.
    /// cross_root_cites_edges` and the zero-case hint on `admit`'s doc comment.
    ///
    /// `None`, not `Some(0)`, at `Scope::All` (2026-09-09 review round 1, Important 1)
    /// — `roots.is_empty()` short-circuits [`Self::new`] before `cross_root_cites`
    /// ever runs, so `Scope::All` is not "measured zero crossings," it is "never
    /// asked the question." Collapsing that into a bare `0` fed the zero-case hint's
    /// own remedy sentence to an operator who cannot act on it: `doc(action=
    /// "append_entry", cites=[...])` creates a LOCAL-to-foreign edge, which needs an
    /// active project to be the local side of — re-running with no project active
    /// still has no `roots`, still short-circuits, still publishes the same reading,
    /// forever. `Some(n)` (including `Some(0)`, a real measurement) keeps the
    /// existing remedy; `None` is instead reported as "not computed" and the remedy
    /// is suppressed outright.
    cross_root_cites_edges: Option<usize>,
    /// Declared-umbrella member roots (`umbrella_member_roots(ctx)`), computed
    /// UNCONDITIONALLY here regardless of `scope` — 2026-09-09 review round 1,
    /// Important 5 (a coordinator finding, not the reviewer's). Before this field
    /// existed, `umbrella_member_roots` was called only inside the `Scope::Umbrella`
    /// arm above, to widen `roots` itself; a `Project`/`Repo`/`All` scan had no way
    /// to ask "is this specific foreign row an umbrella member" without re-deriving
    /// the umbrella lookup a second, divergent way. [`Self::known_elsewhere_row_is_relevant`]
    /// is the sole reader — see its doc comment for why umbrella membership,
    /// specifically, is the second half of that predicate rather than
    /// `known_elsewhere` membership in general (`known_elsewhere` also includes every
    /// `commits.git_root` the catalog has ever indexed, which is not what "problems
    /// in connections to other projects in an umbrella" asked for). Empty whenever
    /// the active project declares no umbrella, exactly like `umbrella_member_roots`
    /// itself.
    umbrella_roots: Vec<PathBuf>,
}

impl DoctorScope {
    pub(super) fn new(
        scope: Scope,
        ctx: &ToolContext,
        conn: &rusqlite::Connection,
    ) -> Result<Self> {
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

        // Computed unconditionally, regardless of `scope` — see the
        // `umbrella_roots` field doc (2026-09-09 review round 1, Important 5) for
        // why a `Project`/`Repo`/`All` scan still needs this list even though only
        // the `Scope::Umbrella` arm below folds it into `roots` itself.
        let umbrella_roots = umbrella_member_roots(ctx);

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
                    roots.extend(umbrella_roots.clone());
                }
            }
        }

        // Task 7: the relevance exemption's own denominator, computed once here so
        // `admit` stays a pure lookup with no query of its own. Skipped entirely when
        // `roots` is empty — that state is `Scope::All` exclusively (see the field
        // doc), which already admits everything, so a cited-from-here set would be
        // computed only to be consulted by nothing. `None`, not `Some(0)`, is what
        // that skip publishes (2026-09-09 review round 1, Important 1) — see the
        // field doc for why collapsing "never asked" into "measured zero" fed a
        // dead-end remedy to an operator who could never satisfy it.
        let (cited_from_here, cross_root_cites_edges) = if roots.is_empty() {
            (BTreeSet::new(), None)
        } else {
            let (cited_from_here, edges) = cross_root_cites(conn, &roots)?;
            (cited_from_here, Some(edges))
        };

        Ok(Self {
            scope,
            roots,
            scoped_out: BTreeMap::new(),
            cited_from_here,
            cross_root_cites_edges,
            umbrella_roots,
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
        if self.contains(Path::new(abs_path)) {
            return true;
        }
        // Task 7 (relevance exemption): a row otherwise scoped OUT is admitted anyway
        // — WITHOUT tallying into `scoped_out` — when `id` names an artifact a local
        // (in-scope) artifact actively cites. Gated on `check`, not merely on
        // membership, because `cited_prefix_with_no_definer` (the one call site Tasks
        // 3-5 gave a non-uniform `id`/`abs_path` pair) passes a namespace PREFIX as
        // `id`, never an artifact id — looking that up in `cited_from_here` would be a
        // category error, not merely a guaranteed miss, and `cited_from_here` holding a
        // real 16-hex id that happens to match a prefix string is not a risk worth
        // trusting to string mismatch alone. See that call site's own doc comment
        // (`scan_cited_prefix_with_no_definer`, `doctor.rs`) for why its `admit` return
        // is already dead-by-construction; this branch must not resurrect it.
        //
        // 2026-09-09 review round 1, Minor 8 (promoted): `check`'s gate is an explicit
        // ALLOW-list (`Check::admits_relevance_exemption`), not a
        // `!= CitedPrefixWithNoDefiner` deny-list. The deny form was ALSO true for
        // `None` — an undeclared check name — so an unrecognized check got this
        // exemption by default, and the `debug_assert!` above that would normally
        // catch an undeclared name is compiled out in release builds. The allow-list
        // form means a brand-new check (Tasks 8-9 are about to add several) must be
        // deliberately added to `admits_relevance_exemption`'s `matches!` arms before
        // it can be exempted — silence now means excluded, not included.
        if super::Check::from_wire(check).is_some_and(super::Check::admits_relevance_exemption)
            && self.cited_from_here.contains(id)
        {
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
    /// Task 7 (relevance exemption), 2026-09-09 review round 1, Important 5 (a
    /// coordinator finding, not the reviewer's). `scan_artifact_paths`'s
    /// `known_elsewhere` branch (`src/librarian/tools/doctor.rs`) counts a foreign
    /// row into its own `scoped` map — silently, "counted, not reported" — WITHOUT
    /// ever calling [`Self::admit`], so Task 7's relevance exemption never had a
    /// chance to reach it, no matter how relevant that row is to the active
    /// project. This is the predicate that gives it that chance, gated on BOTH
    /// halves of the user's own requirement — *"at most show problems in
    /// connections to other projects in an umbrella, and only if it affects the
    /// current project"* — never either alone:
    ///
    /// - **"affects the current project"**: `id` is cited from inside `roots`
    ///   (`self.cited_from_here`), exactly the same membership `admit`'s own
    ///   exemption reads.
    /// - **"connections to other projects in an umbrella"**: `abs_path` resolves
    ///   under a declared UMBRELLA member specifically (`self.umbrella_roots`), not
    ///   under `known_elsewhere` in general. `known_elsewhere` is strictly WIDER —
    ///   umbrella members UNION every `commits.git_root` the catalog has ever
    ///   indexed — so naively reordering `scan_artifact_paths`'s two branches (call
    ///   `admit` before the `known_elsewhere` check) would also surface a cited row
    ///   under any unrelated indexed repo, which the requirement does not ask for.
    ///
    /// A row satisfying both should NOT be silently counted — the caller is
    /// expected to skip its own `known_elsewhere` short-circuit for exactly this
    /// row and fall through to `admit` instead, so the row gets the same
    /// tally-or-surface treatment as any other `admit` call (Ruling 17's per-check
    /// global metric still sees it either way).
    pub(super) fn known_elsewhere_row_is_relevant(&self, id: &str, abs_path: &Path) -> bool {
        self.cited_from_here.contains(id)
            && containing_root(&self.umbrella_roots, abs_path).is_some()
    }

    /// Rows refused by [`Self::admit`], keyed by check name then by project
    /// root. A caller folds one check's sub-map (`.get(check)`) into that
    /// check's own `catalog_health` aggregate — never the whole map into a
    /// single shared one.
    pub(super) fn scoped_out(&self) -> &BTreeMap<String, BTreeMap<String, usize>> {
        &self.scoped_out
    }

    /// Task 7 (relevance exemption): how many `cites` edges — summed across
    /// `artifact_link` and `entry_cite` — have a citing side inside `roots` and a
    /// cited side outside it. This is the exemption's own denominator: `admit`
    /// silently widening the worklist on a rule that never fires would be
    /// indistinguishable from no rule at all, so this count ships alongside it.
    /// `catalog_health.cross_root_cites_edges` publishes it unconditionally, at
    /// every scope, including `Some(0)` — the value measured on today's catalog.
    ///
    /// `None` at `Scope::All` (2026-09-09 review round 1, Important 1) — see the
    /// field doc. Do not collapse this to `0` at the call site; that is exactly the
    /// conflation this type exists to prevent.
    pub(super) fn cross_root_cites_edges(&self) -> Option<usize> {
        self.cross_root_cites_edges
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

/// Task 7's own denominator computation: for each `cites` edge (both grains) whose
/// citing side resolves to an artifact inside `roots` and whose cited side resolves
/// to an artifact outside `roots`, collect the cited artifact's id into the returned
/// set and count the edge. Returns `(cited_from_here, cross_root_cites_edges)` —
/// see [`DoctorScope`]'s field docs for what each half is for.
///
/// Queries BOTH cites tables, deliberately — they are different grains
/// (`artifact_link` is artifact-to-artifact; `entry_cite` is entry-to-entry/file) and
/// a scanner can write to either depending on what the citation text names. Querying
/// only one would silently miss whichever grain the operator's citations happen to
/// use.
///
/// `entry_cite.dst_ref` has no foreign key (see `src/librarian/catalog/gc.rs`'s
/// rehome comment) and 13 rows on today's catalog do not resolve to any known
/// artifact or slug. Unresolvable rows are skipped — fail CLOSED: an unresolvable
/// `dst_ref` cannot be proven to name a real foreign artifact a local row cites, so
/// admitting it would be an ungrounded relaxation of scope isolation rather than a
/// grounded exemption.
fn cross_root_cites(
    conn: &rusqlite::Connection,
    roots: &[PathBuf],
) -> Result<(BTreeSet<String>, usize)> {
    // Single pass over `artifact`: id -> abs_path for both tables' direct lookups,
    // and slug -> (id, abs_path) for `entry_cite.dst_ref`'s `<slug>:<local>` form.
    let mut by_id: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut by_slug: BTreeMap<String, (String, PathBuf)> = BTreeMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, abs_path, slug FROM artifact")?;
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let id: String = r.get(0)?;
            let abs_path: String = r.get(1)?;
            let slug: Option<String> = r.get(2)?;
            let path = PathBuf::from(abs_path);
            if let Some(s) = slug {
                by_slug.insert(s, (id.clone(), path.clone()));
            }
            by_id.insert(id, path);
        }
    }

    let mut cited_from_here: BTreeSet<String> = BTreeSet::new();
    let mut cross_root_cites_edges = 0usize;

    // Grain 1: artifact_link (artifact-to-artifact).
    {
        let mut stmt = conn.prepare("SELECT src_id, dst_id FROM artifact_link WHERE rel = ?1")?;
        let mut rows = stmt.query(rusqlite::params![CITES_REL])?;
        while let Some(r) = rows.next()? {
            let src_id: String = r.get(0)?;
            let dst_id: String = r.get(1)?;
            let (Some(src_path), Some(dst_path)) = (by_id.get(&src_id), by_id.get(&dst_id)) else {
                continue;
            };
            if containing_root(roots, src_path).is_some()
                && containing_root(roots, dst_path).is_none()
            {
                cited_from_here.insert(dst_id);
                cross_root_cites_edges += 1;
            }
        }
    }

    // Grain 2: entry_cite (entry-to-entry/file). `src_slug` has an FK and always
    // resolves for a live row; `dst_ref` does not (see the doc comment above).
    {
        let mut stmt = conn.prepare("SELECT src_slug, dst_ref FROM entry_cite WHERE rel = ?1")?;
        let mut rows = stmt.query(rusqlite::params![CITES_REL])?;
        while let Some(r) = rows.next()? {
            let src_slug: String = r.get(0)?;
            let dst_ref: String = r.get(1)?;
            let Some((_, src_path)) = by_slug.get(&src_slug) else {
                continue;
            };
            let resolved: Option<(String, PathBuf)> = match dst_ref.split_once(':') {
                Some((dst_slug, _local)) => by_slug.get(dst_slug).cloned(),
                None => by_id.get(&dst_ref).map(|p| (dst_ref.clone(), p.clone())),
            };
            let Some((dst_id, dst_path)) = resolved else {
                // Unresolvable dst_ref (no FK — see doc comment). Fail closed: skip.
                continue;
            };
            if containing_root(roots, src_path).is_some()
                && containing_root(roots, &dst_path).is_none()
            {
                cited_from_here.insert(dst_id);
                cross_root_cites_edges += 1;
            }
        }
    }

    Ok((cited_from_here, cross_root_cites_edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::catalog::{artifact, entry_cite, links};
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
        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::All, &ctx, &cat.conn).unwrap();
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
    /// fix and observing the panic before writing this corrected version.
    #[test]
    fn scope_project_without_an_active_project_is_refused() {
        let ctx = unscoped_ctx();
        assert!(
            ctx.current_project.is_none(),
            "this test's whole point is the no-active-project arm"
        );
        let cat = ctx.catalog.lock();
        let err = DoctorScope::new(Scope::Project, &ctx, &cat.conn)
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

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
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
        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
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

        let cat = ctx.catalog.lock();
        let mut project_scope = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
        assert!(
            !project_scope.admit("abs_path_outside_managed_roots", "sib", &sibling_pkg_file.to_string_lossy()),
            "a sibling package under git_root but outside abs_path must be refused at Project scope"
        );

        let mut repo_scope = DoctorScope::new(Scope::Repo, &ctx, &cat.conn).unwrap();
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

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
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

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Umbrella, &ctx, &cat.conn).unwrap();
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
        let cat = ctx.catalog.lock();
        let err = DoctorScope::new(Scope::Umbrella, &ctx, &cat.conn).unwrap_err();
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
        let cat = ctx.catalog.lock();
        let err = DoctorScope::new(Scope::Umbrella, &ctx, &cat.conn).unwrap_err();
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

    /// Task 7 (relevance exemption), R4: fires for a foreign id cited from inside
    /// scope via EITHER cites table — `artifact_link` (artifact grain) or `entry_cite`
    /// (entry grain) — in one fixture, so a fix wiring only one grain cannot pass by
    /// construction. Non-monotone per this repo's testing discipline: each exempted id
    /// asserts BOTH that `admit()` returns true AND that `scoped_out()` stayed empty —
    /// an implementation that widened `contains`/`roots` instead of adding a real,
    /// targeted exemption would satisfy the first half and fail the second. A final
    /// control row (cited from nowhere) confirms the exemption is targeted, not a
    /// blanket admit of every foreign row.
    #[test]
    fn admit_exempts_a_foreign_id_cited_from_either_cites_table() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        let foreign_root = tmp.path().join("foreign");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(foreign_root.join("docs")).unwrap();
        let ctx = ctx_at(&root);

        {
            let cat = ctx.catalog.lock();

            // Grain 1: artifact_link. A local citer and a foreign cited artifact, no
            // slugs needed — this grain is artifact-id-to-artifact-id.
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("local-link")
                    .with_abs_path(root.join("docs/local-link.md"))
                    .build(),
            )
            .unwrap();
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("foreign-link")
                    .with_abs_path(foreign_root.join("docs/foreign-link.md"))
                    .build(),
            )
            .unwrap();
            links::insert(
                &cat,
                &links::LinkRow {
                    src_id: "local-link".to_string(),
                    dst_id: "foreign-link".to_string(),
                    rel: CITES_REL.to_string(),
                    created_at: 0,
                },
            )
            .unwrap();

            // Grain 2: entry_cite. `src_slug` FKs `artifact(slug)`, so the citing
            // artifact needs a minted slug; `dst_ref` here is the bare foreign id (no
            // colon), the other of the two forms `dst_ref` can take.
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("local-entry")
                    .with_abs_path(root.join("docs/local-entry.md"))
                    .build(),
            )
            .unwrap();
            let local_slug = artifact::ensure_slug(&cat.conn, "local-entry").unwrap();
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("foreign-entry")
                    .with_abs_path(foreign_root.join("docs/foreign-entry.md"))
                    .build(),
            )
            .unwrap();
            entry_cite::insert_with(
                &cat.conn,
                &entry_cite::EntryCiteRow {
                    src_slug: local_slug,
                    src_local: "E-1".to_string(),
                    dst_ref: "foreign-entry".to_string(),
                    rel: CITES_REL.to_string(),
                    origin: entry_cite::ORIGIN_SCAN.to_string(),
                    created_at: 0,
                },
            )
            .unwrap();

            // Control: a third foreign artifact cited by nothing.
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("not-cited")
                    .with_abs_path(foreign_root.join("docs/not-cited.md"))
                    .build(),
            )
            .unwrap();
        }

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
        assert_eq!(
            s.cross_root_cites_edges(),
            Some(2),
            "one crossing edge per grain, both counted"
        );

        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "foreign-link",
                &foreign_root.join("docs/foreign-link.md").to_string_lossy(),
            ),
            "artifact_link-cited foreign id must be admitted"
        );
        assert!(
            s.scoped_out().is_empty(),
            "an exempted row must not tally into scoped_out: {:?}",
            s.scoped_out()
        );

        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "foreign-entry",
                &foreign_root.join("docs/foreign-entry.md").to_string_lossy(),
            ),
            "entry_cite-cited foreign id must be admitted"
        );
        assert!(
            s.scoped_out().is_empty(),
            "an exempted row must not tally into scoped_out: {:?}",
            s.scoped_out()
        );

        assert!(
            !s.admit(
                "abs_path_outside_managed_roots",
                "not-cited",
                &foreign_root.join("docs/not-cited.md").to_string_lossy(),
            ),
            "a foreign id cited from nowhere must still be refused — the exemption is \
             targeted, not a blanket admit of every foreign row"
        );
        assert_eq!(
            s.scoped_out()
                .values()
                .flat_map(|m| m.values())
                .sum::<usize>(),
            1,
            "the uncited control row must be the only tally"
        );
    }

    /// Task 7 (relevance exemption), 2026-09-09 review round 1, Important 2: the
    /// `<dst_slug>:<local>` branch of `cross_root_cites`'s `dst_ref` resolution
    /// (`Some((dst_slug, _local)) => by_slug.get(dst_slug).cloned()`) had no test of
    /// its own — every existing fixture seeded only the bare-id form. This is not a
    /// theoretical gap: `resolve_cite_ref` (`src/librarian/catalog/augmentation.rs`,
    /// arm 2) stores a `<slug>:<local>` ref VERBATIM, and `append_entry(cites=[...])`
    /// — the exact remedy `catalog_health`'s zero-case hint recommends — is the only
    /// cross-root-capable writer that produces this form. A hint recommending a
    /// remedy this code cannot actually admit would be exactly the "loudness without
    /// a working path" defect CLAUDE.md's Testing Discipline names.
    ///
    /// Confirmed by mutation, not merely by this test passing: changing
    /// `Some((dst_slug, _local)) => by_slug.get(dst_slug).cloned()` to
    /// `Some(_) => None` left the rest of this file's suite green and turned only
    /// this test red (reported alongside this fix, not asserted here — a self-test
    /// of a mutation cannot outlive the mutation it names).
    #[test]
    fn admit_exempts_a_foreign_id_cited_via_the_slug_colon_local_dst_ref_form() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        let foreign_root = tmp.path().join("foreign");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(foreign_root.join("docs")).unwrap();
        let ctx = ctx_at(&root);

        {
            let cat = ctx.catalog.lock();

            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("local-citer")
                    .with_abs_path(root.join("docs/local-citer.md"))
                    .build(),
            )
            .unwrap();
            let local_slug = artifact::ensure_slug(&cat.conn, "local-citer").unwrap();

            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("foreign-slugged")
                    .with_abs_path(foreign_root.join("docs/foreign-slugged.md"))
                    .build(),
            )
            .unwrap();
            let foreign_slug = artifact::ensure_slug(&cat.conn, "foreign-slugged").unwrap();

            // The production shape `resolve_cite_ref` writes verbatim for arm 2
            // (`<slug>:<local>`) — the local half (`E-9`) is never consulted by
            // `cross_root_cites`, which discards it via `_local`, so any token proves
            // the branch.
            entry_cite::insert_with(
                &cat.conn,
                &entry_cite::EntryCiteRow {
                    src_slug: local_slug,
                    src_local: "E-1".to_string(),
                    dst_ref: format!("{foreign_slug}:E-9"),
                    rel: CITES_REL.to_string(),
                    origin: entry_cite::ORIGIN_SCAN.to_string(),
                    created_at: 0,
                },
            )
            .unwrap();

            // Control: a foreign artifact cited by nothing, to confirm this is a
            // targeted exemption rather than a widened one.
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("not-cited-via-slug")
                    .with_abs_path(foreign_root.join("docs/not-cited-via-slug.md"))
                    .build(),
            )
            .unwrap();
        }

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
        assert_eq!(
            s.cross_root_cites_edges(),
            Some(1),
            "the slug:local dst_ref must resolve and count as one crossing edge"
        );

        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "foreign-slugged",
                &foreign_root
                    .join("docs/foreign-slugged.md")
                    .to_string_lossy(),
            ),
            "a foreign id cited via the slug:local dst_ref form must be admitted"
        );
        assert!(
            s.scoped_out().is_empty(),
            "an exempted row must not tally into scoped_out: {:?}",
            s.scoped_out()
        );

        assert!(
            !s.admit(
                "abs_path_outside_managed_roots",
                "not-cited-via-slug",
                &foreign_root
                    .join("docs/not-cited-via-slug.md")
                    .to_string_lossy(),
            ),
            "an uncited foreign id must still be refused"
        );
    }
    /// Task 7 (relevance exemption), 2026-09-09 review round 1, Important 3: the
    /// `Check::admits_relevance_exemption` allow-list gate inside `admit` — the clause
    /// that keeps `cited_prefix_with_no_definer` OUT of the exemption, per that
    /// method's own doc comment (a namespace prefix is not an artifact id, so looking
    /// it up in `cited_from_here` would be a category error even when it happens to
    /// string-match) — had no test of its own gating EFFECT. Every other test either
    /// never calls `admit` with that check name, or never seeds a `cited_from_here` id
    /// that could collide with it, so deleting the gate clause (or widening the
    /// allow-list to include `CitedPrefixWithNoDefiner`) left the whole suite green.
    /// This test seeds exactly one `cited_from_here` id and calls `admit` twice with
    /// that SAME id — once under `cited_prefix_with_no_definer` (must stay refused)
    /// and once under an allow-listed check (must be admitted) — so the refusal is
    /// pinned to the check gate specifically, not to a missing `cited_from_here`
    /// membership the first assertion alone could not rule out.
    #[test]
    fn admit_refuses_cited_prefix_with_no_definer_even_when_its_id_is_cited_from_here() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mine");
        let foreign_root = tmp.path().join("foreign");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(foreign_root.join("docs")).unwrap();
        let ctx = ctx_at(&root);

        {
            let cat = ctx.catalog.lock();
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("local-citer")
                    .with_abs_path(root.join("docs/local-citer.md"))
                    .build(),
            )
            .unwrap();
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new("foreign-cited")
                    .with_abs_path(foreign_root.join("docs/foreign-cited.md"))
                    .build(),
            )
            .unwrap();
            links::insert(
                &cat,
                &links::LinkRow {
                    src_id: "local-citer".to_string(),
                    dst_id: "foreign-cited".to_string(),
                    rel: CITES_REL.to_string(),
                    created_at: 0,
                },
            )
            .unwrap();
        }

        let cat = ctx.catalog.lock();
        let mut s = DoctorScope::new(Scope::Project, &ctx, &cat.conn).unwrap();
        assert_eq!(
            s.cross_root_cites_edges(),
            Some(1),
            "sanity check on the fixture: exactly one crossing edge"
        );

        // Same `id`, used as `cited_prefix_with_no_definer`'s "prefix" argument (a
        // category error per that check's own contract, but exactly the string the
        // gate exists to keep from being read as a real cited-from-here membership)
        // must NOT be exempted.
        assert!(
            !s.admit(
                "cited_prefix_with_no_definer",
                "foreign-cited",
                &foreign_root.join("docs/foreign-cited.md").to_string_lossy(),
            ),
            "cited_prefix_with_no_definer must never receive the relevance exemption, \
             even when its `id` string happens to match a real cited_from_here id"
        );
        assert_eq!(
            s.scoped_out()
                .values()
                .flat_map(|m| m.values())
                .sum::<usize>(),
            1,
            "the refused cited_prefix_with_no_definer call must tally into scoped_out"
        );

        // Same id, an allow-listed check this time: must be admitted — proving the
        // refusal above traces to the check gate, not to a missing cited_from_here
        // entry for "foreign-cited".
        assert!(
            s.admit(
                "abs_path_outside_managed_roots",
                "foreign-cited",
                &foreign_root.join("docs/foreign-cited.md").to_string_lossy(),
            ),
            "the same cited-from-here id must be admitted under an allow-listed check"
        );
    }
}
