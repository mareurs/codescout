//! Doctor — catalog drift scanner.
//!
//! Read-only invariant checks against the librarian catalog. Surfaces drift
//! the moment it lands instead of when it cascades into downstream query
//! failures (e.g. rounds 5–9 of the forward-slash saga, where the symptom
//! was `LIKE` patterns returning empty sets rather than the catalog
//! flagging its own corruption).
//!
//! Checks (MVP):
//!
//! 1. `abs_path_must_be_absolute` — the schema declares
//!    `abs_path TEXT NOT NULL UNIQUE` but does not enforce absoluteness.
//!    Pre-#66 code paths stored relative strings in some rows; the doctor
//!    surfaces them so they can be migrated (or evicted via `reindex`)
//!    rather than masquerading as `missing_file` false positives.
//!    Discovered in the live-catalog smoke test after the #69 commit.
//! 2. `backslash_in_abs_path` — `artifact.abs_path` must contain only `/`
//!    separators. After the [`crate::util::fs::RepoPath`] newtype migration,
//!    every write goes through `to_forward_slash` — any backslash row is
//!    pre-migration drift.
//! 3. `ads_colon_in_abs_path` — no colon outside the optional Windows
//!    drive-letter prefix (`[a-zA-Z]:/`). Defends against the NTFS alternate
//!    data stream `foo.txt:hidden` shape (Ibex S-2 in rounds 3–8 review).
//! 4. `dotdot_segment_in_abs_path` — no segment is exactly `..`. Catches
//!    path-escape strings even though the gather tool's
//!    [`guard_relative_path`] already rejects them on input.
//! 5. `missing_file` — every `artifact.abs_path` must exist on disk
//!    (`Path::exists()`). Catches rows orphaned by `git rm` /
//!    out-of-band file moves that bypassed `reindex`.
//! 6. `backslash_in_git_root` — `commits.git_root` carries paths too;
//!    the same forward-slash invariant applies (commits.rs writes via
//!    `RepoPath::from_path(...).into_string()` post-#66).
//! 7. `worktree_scoped_row` — flags catalog rows whose abs_path is under a
//!    linked git worktree; classifies no_collision vs collision, and flags
//!    `registered` (an ACTIVE `worktree_registration` covers the row's
//!    worktree root — pending `librarian(action="merge_worktree")`, not a
//!    reseat). Unregistered rows still feed `fix=reseat_worktree`, which is
//!    now the LEGACY fallback for catalog drift the overlay never saw.
//! 8. `abs_path_outside_managed_roots` — every `artifact.abs_path` must
//!    resolve under some managed root, i.e. the precondition
//!    `artifact(move)` / `artifact(delete)` enforce via `containing_root`.
//!    Added after WIN-30, where that resolution silently failed for every
//!    row on Windows and no existing check could see it: the catalog stores
//!    `//?/C:/...` while `current_project` holds `\\?\C:\...`, and
//!    `check_backslash` was enforcing the very forward-slash form that broke
//!    the comparison. A firing row may simply belong to another workspace,
//!    so the detail names the roots tried. Skipped when no roots are
//!    configured, and for rows already flagged by
//!    `abs_path_must_be_absolute`.
//! 9. `declared_root_missing` — every `[[project]]` in `.codescout/workspace.toml`
//!    must declare a `root` that resolves to a directory under the root owning
//!    that file. The only check here that reads config rather than catalog, and
//!    the only defect class that is unreviewable by construction: the file is
//!    gitignored, so a wrong root reaches no diff. Added after a mis-rooted
//!    config declared eight sibling repos as sub-projects for a month while
//!    `activate` kept reporting the real tree. Skipped, loudly, in a linked
//!    worktree — see `scan_declared_project_roots`.
//! 10. `terminal_status_with_caveat` — a bug file whose `status` is terminal and
//!     whose `unverified:` field is non-empty. The reader half of the `unverified:`
//!     convention: authors write the caveat, the canonical triage query filters on
//!     `status`, and every caveated record has a terminal one by construction — so
//!     without this the field only moved the problem from prose into frontmatter.
//!     Archived files are included on purpose; see `scan_terminal_status_with_caveat`.
//! 11. `archived_fix_sha_unresolvable` — a bug file whose DECLARED fix SHA no
//!     longer names an object in this repo. Scans only the structured
//!     `## Fix provenance` triple (54 of 350 archived files); freeform prose is
//!     skipped rather than swept, because sampled prose names reproduction
//!     commits and an explicitly exonerated suspect. Coverage is reported so a
//!     clean result cannot be read as "all archived fixes resolve".
//!
//! Deferred to a follow-up: NFC unicode normalization, orphan
//! `artifact_augmentation` rows (the FK already cascades on artifact
//! deletion, but a defensive check would catch FK-constraint disabled
//! corruption).
//!
//! The default scan is read-only. An opt-in `fix=prune_missing` mode (with a
//! required `root=` argument) prunes every `artifact` + `commits` row anchored
//! under a dead/renamed repo root — cascade-safe through codescout's own
//! (vec0-linked) connection, which a bare `sqlite3` CLI cannot do (7ca71bf7).
//! Output is a JSON report with `violations` + `summary` (per-check counts); a
//! fix run returns `pruned` counts instead.
//!
//! A second opt-in fix, `fix=reseat_worktree`, consumes `scan_worktree_scoped`
//! violations: `no_collision` rows are durably re-seeded at the main-repo
//! path — a fresh row is written at `id_m = artifact_id_from_abs(main_path)`
//! and [`crate::librarian::catalog::graft::graft_rows`] folds the worktree
//! row's entire history (events, links, event_edges, and the git-invisible
//! `append_entry` augmentation) onto it before deleting the worktree row. The
//! id CHANGES (`id_w` -> `id_m`): catalog identity is
//! `id == artifact_id_from_abs(abs_path)`, so a bare `abs_path` UPDATE that
//! kept `id_w` would leave the row mismatched, and the next MAIN-repo
//! reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact WHERE
//! abs_path=? AND id != ?`) would delete it — cascading away exactly the
//! history this exists to preserve. `collision` rows are left untouched and
//! reported for a manual `graft`. `registered` rows (an ACTIVE
//! `worktree_registration` covers them) are SKIPPED entirely and reported
//! under `skipped` — they belong to `librarian(action="merge_worktree")`.
//!
//! `fix=prune_missing` carries the same registration guard in the other
//! direction: it refuses to prune a dead root an ACTIVE registration still
//! covers, so a `git worktree remove` before merge can't silently delete the
//! catalog's only remaining record of that worktree's unmerged history.

use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::librarian::catalog::artifact::{self, ArtifactRow};
use crate::librarian::catalog::graft;
use crate::librarian::catalog::worktree;
use crate::librarian::{current_project, ids};

use super::{RecoverableError, ToolContext};

/// Declares every check `doctor` can emit, generating the enum and its `ALL`
/// list from ONE list so the two cannot drift apart.
///
/// **The wire string is pinned beside its variant on purpose, never derived
/// from the variant name.** These strings are this tool's public vocabulary:
/// `docs/issues/`, the guides and `get_guide("tracker-conventions")` name
/// checks like `augmentation_declared_but_absent` in prose, and readers match
/// on them in `doctor`'s JSON. Deriving the string from the identifier would
/// make a variant rename a silent breaking change to that vocabulary —
/// compiler-clean, because nothing outside this crate is typed against it.
/// Pinned, a rename is visible in the diff as a changed literal.
macro_rules! declare_checks {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// Every check name `doctor` can emit.
        ///
        /// This exists because a hand-maintained enumeration was tried here
        /// before, in `Violation`'s own doc comment, and went stale by three of
        /// eight with nobody noticing — see that comment. The lesson is not
        /// "a list is wrong" but "a *hand-maintained* list is wrong": the
        /// adjective is the whole finding, and this macro drops it by
        /// generating `ALL` from the same arms as the variants.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Check {
            $(
                #[allow(missing_docs)]
                $variant
            ),+
        }

        impl Check {
            /// Every variant, generated from the same macro arms as the enum
            /// itself — adding a check adds it here with no second edit.
            pub const ALL: &'static [Check] = &[$(Check::$variant),+];

            /// The wire name, as it appears in `doctor`'s JSON.
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Check::$variant => $name),+
                }
            }

            /// Resolve a wire name back to its variant. `None` means the name
            /// is not a declared check — which `Violation::new` treats as a
            /// programming error rather than data.
            pub fn from_wire(name: &str) -> Option<Check> {
                match name {
                    $($name => Some(Check::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

declare_checks! {
    AbsPathMustBeAbsolute => "abs_path_must_be_absolute",
    AbsPathOutsideManagedRoots => "abs_path_outside_managed_roots",
    AdsColonInAbsPath => "ads_colon_in_abs_path",
    ArchivedFixShaUnresolvable => "archived_fix_sha_unresolvable",
    AugmentationDeclarationUnparseable => "augmentation_declaration_unparseable",
    AugmentationDeclaredButAbsent => "augmentation_declared_but_absent",
    BackslashInAbsPath => "backslash_in_abs_path",
    BackslashInGitRoot => "backslash_in_git_root",
    CitedPrefixWithNoDefiner => "cited_prefix_with_no_definer",
    DeclaredRootMissing => "declared_root_missing",
    DotdotSegmentInAbsPath => "dotdot_segment_in_abs_path",
    EntryCitedFromOutsideButUndeclared => "entry_cited_from_outside_but_undeclared",
    EntryConditionalPastDue => "entry_conditional_past_due",
    EntryDatedStale => "entry_dated_stale",
    EntryWithoutDefinition => "entry_without_definition",
    FrontmatterIdIsNotACatalogId => "frontmatter_id_is_not_a_catalog_id",
    FrontmatterIdMismatch => "frontmatter_id_mismatch",
    LedgerDefinesNothing => "ledger_defines_nothing",
    MissingFile => "missing_file",
    ParamsBehindBody => "params_behind_body",
    ParamsStatusDrift => "params_status_drift",
    PrematureArchiveCitation => "premature_archive_citation",
    SidecarShapeDrift => "sidecar_shape_drift",
    SidecarUnparseable => "sidecar_unparseable",
    SnapshotDrift => "snapshot_drift",
    TerminalStatusWithCaveat => "terminal_status_with_caveat",
    TerminalStatusWithoutFixAnchor => "terminal_status_without_fix_anchor",
    UnterminatedFence => "unterminated_fence",
    ValidityUnparseable => "validity_unparseable",
    WorktreeScopedRow => "worktree_scoped_row",
}

/// One violation of a doctor invariant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    /// Which check fired.
    ///
    /// Deliberately not enumerated here: this list named eight checks and had gone
    /// stale by three (`snapshot_drift`, `ledger_defines_nothing`,
    /// `entry_without_definition`) before anyone noticed, because nothing gates a doc
    /// comment against the `scan_*` functions that emit these strings. The authoritative
    /// set is the `Violation::new` call sites — every `scan_*` function in this module
    /// documents its own check names on itself.
    pub check: String,
    /// The artifact id that owns the violating row, when applicable.
    /// `None` for table-wide checks (e.g. `commits.git_root` has no
    /// artifact_id).
    pub artifact_id: Option<String>,
    /// The path string that triggered the violation.
    pub path: String,
    /// Human-readable detail (position of the offending byte, segment,
    /// etc.). Empty string when the check name alone is sufficient.
    pub detail: String,
}

impl Violation {
    fn new(
        check: &str,
        artifact_id: Option<String>,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        // A name no `Check` variant declares is a programming error, not data:
        // it would be counted in `by_check` but absent from the seeded set, so
        // the report would silently carry a check the registry does not know.
        // Debug-only, so it fires across the test suite — every `scan_*` with a
        // test that makes it fire is gated — and costs nothing in release.
        debug_assert!(
            Check::from_wire(check).is_some(),
            "undeclared doctor check name {check:?} — add it to declare_checks! \
             or by_check will not seed it"
        );
        Self {
            check: check.into(),
            artifact_id,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

/// MCP entry point. Runs every invariant check and returns a structured
/// report. Reads-only; safe to invoke against a live catalog.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Opt-in mutation: prune catalog rows under a dead/renamed repo root.
    // Default (no `fix`) stays read-only.
    if let Some(fix) = args.get("fix").and_then(Value::as_str) {
        let confirm = args
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // `old_root` is the self-documenting name the move-candidate hint
        // and validate_rehome_request's own error text both surface;
        // `root` is accepted as a fallback for back-compat and remains
        // the only name prune_missing/reseat_worktree callers use.
        let old_root_arg = args
            .get("old_root")
            .and_then(Value::as_str)
            .or_else(|| args.get("root").and_then(Value::as_str));
        return run_fix(
            ctx,
            fix,
            old_root_arg,
            args.get("new_root").and_then(Value::as_str),
            confirm,
        )
        .await;
    }

    // Managed roots are derived before taking the catalog lock — `managed_roots`
    // only reads `ctx`, and holding the lock across it would widen the critical
    // section for nothing.
    let roots = super::managed_roots(ctx);

    // Config state rather than catalog state: this reads `.codescout/workspace.toml` off
    // disk and touches no connection, so it runs outside the lock alongside
    // `managed_roots` rather than widening the critical section.
    let (declared_root_violations, declared_roots_health) = scan_declared_project_roots(ctx);

    let cat = ctx.catalog.lock();
    let mut all_violations: Vec<Violation> = Vec::new();

    all_violations.extend(declared_root_violations);
    // Roots that belong to a workspace this machine knows about but is not
    // managing this session — the discriminator that lets the outside-roots check
    // separate "another workspace's row" from "orphan". Needs the connection, so
    // it runs inside the lock.
    let known_elsewhere = known_workspace_roots(ctx, &cat.conn);
    let (artifact_path_violations, outside_scoped_by_project) =
        scan_artifact_paths(&cat.conn, &roots, &known_elsewhere)?;
    all_violations.extend(artifact_path_violations);
    all_violations.extend(scan_commits_git_root(&cat.conn)?);
    all_violations.extend(scan_worktree_scoped(&cat.conn)?);
    all_violations.extend(scan_snapshot_drift(&cat.conn)?);
    // Runs beside snapshot_drift rather than inside it: the two ask different
    // questions of the same body (does it carry the row / can anything cite the
    // entry) and are allowed to disagree. See `scan_undefined_entries`.
    all_violations.extend(scan_undefined_entries(&cat.conn)?);
    // Starts from the citation graph rather than a known ledger's claimed entries — the
    // gap neither scan_undefined_entries nor link_scan's own report reaches. See
    // `scan_cited_prefix_with_no_definer`. Scoped like the entry-validity family (Ruling
    // 17): the corpus decides whether a prefix is unowned, the active project decides
    // whether this reader is handed it.
    let (cited_prefix_violations, cited_prefix_scoped) =
        scan_cited_prefix_with_no_definer(ctx, &cat.conn)?;
    all_violations.extend(cited_prefix_violations);
    // The one check here that needs no threshold: a citation naming an archive path that
    // holds nothing, for a bug still sitting un-archived, is wrong in every world. See
    // `scan_premature_archive_citation`.
    all_violations.extend(scan_premature_archive_citation(&cat.conn)?);
    // And beside both, the inverse of snapshot_drift: `params` behind a body that ran
    // ahead. Same two sets, subtracted the other way; opposite remedy.
    all_violations.extend(scan_params_behind_body(&cat.conn)?);
    // And the content half of that same drift: the id is on both sides, but the two
    // representations disagree about its STATUS. Its three siblings are id-set
    // comparisons and are silent by construction when every id matches — which is how
    // `BL-60` came to read `open` in `params` while the committed body said
    // `done-archived`. See `scan_params_status_drift` for the measured sensitivity and
    // the three cases it deliberately does not report.
    all_violations.extend(scan_params_status_drift(&cat.conn)?);
    // Reads bug-file frontmatter rather than catalog columns: `unverified:` lands in
    // `extra`, which is not indexed. The SQL narrows first, so only terminal bug rows
    // are ever opened.
    all_violations.extend(scan_terminal_status_with_caveat(&cat.conn)?);
    // The reader half of the one artifact state with no on-disk form. Reads frontmatter
    // off disk for the same reason as the check above — `expects_augmentation` lands in
    // `extra`, which is not catalog-indexed — but narrows via LEFT JOIN first, so only
    // artifacts that could violate are ever opened.
    all_violations.extend(scan_augmentation_declared_but_absent(&cat.conn)?);
    // The inverse population of the check above: artifacts that DO have a row, whose
    // committed sidecar may nonetheless disagree with it. Reports only — drift has a
    // direction this cannot determine, and guessing it would overwrite a pulled shape.
    all_violations.extend(scan_sidecar_shape_drift(&cat.conn)?);
    // Per-entry cross-file citation exposure, computed once and shared by every check
    // in the validity-decay family (Tasks 5-7) so each prices its worklist against the
    // same population rather than recomputing it. Stays GLOBAL/unscoped — Ruling 17 —
    // even though the three checks below now scope their REPORTED population to the
    // active project: narrowing the metric itself would understate real cross-repo
    // exposure and manufacture false negatives.
    let indegree = entry_indegree(&cat.conn)?;
    let (conditional_violations, conditional_scoped) =
        scan_conditional_past_due(ctx, &cat.conn, &indegree)?;
    all_violations.extend(conditional_violations);
    // Same shared `indegree`; today's date is computed once here rather than inside
    // `scan_dated_stale` itself, so the horizon comparison stays deterministic under test.
    let today_epoch_days = {
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        (chrono::Utc::now().date_naive() - epoch).num_days()
    };
    let (dated_violations, dated_scoped) =
        scan_dated_stale(ctx, &cat.conn, &indegree, today_epoch_days)?;
    all_violations.extend(dated_violations);
    // Same shared `indegree`; the inverse question of the two checks above — a
    // load-bearing entry that declares no class at all, rather than one whose
    // declared class needs revisiting.
    let (cited_violations, cited_scoped) = scan_cited_but_undeclared(ctx, &cat.conn, &indegree)?;
    all_violations.extend(cited_violations);
    // The fourth partition of the family: a declaration that FAILED to parse at all
    // (shape-invalid, calendar-invalid, or an unknown class). Unlike the three above,
    // this one takes no `indegree` — it is deliberately ungated on exposure; see
    // `scan_validity_unparseable`'s own doc comment for the reasoning and the measured
    // population size that justifies it today.
    let (unparseable_violations, unparseable_scoped) = scan_validity_unparseable(ctx, &cat.conn)?;
    all_violations.extend(unparseable_violations);
    // Entry-validity rows scoped OUT of the report because they belong to a project
    // root other than the active one (Fix 2 for MF-1). A scoped-out row never becomes
    // a `Violation`, so `summary.total` cannot count it the way the
    // `abs_path_outside_managed_roots` sampler's elided rows are counted — this map is
    // how the drop stays visible instead of silent. Combined across all four checks
    // rather than kept per-check: the reader-facing question is "how much of my
    // worklist is actually mine", not which check it came from.
    let mut entry_validity_scoped_by_project: std::collections::BTreeMap<String, usize> =
        Default::default();
    for (group, n) in conditional_scoped
        .into_iter()
        .chain(dated_scoped)
        .chain(cited_scoped)
        .chain(unparseable_scoped)
    {
        *entry_validity_scoped_by_project.entry(group).or_insert(0) += n;
    }
    // Needs both `ctx` (for the repo to resolve against) and the connection, so unlike
    // `scan_declared_project_roots` it runs inside the lock. Its health block is carried
    // out to `catalog_health` below, because a clean result over 54 of 350 archived files
    // must not read as "every archived fix resolves".
    let (fix_sha_violations, archived_fix_shas) =
        scan_archived_fix_sha_unresolvable(ctx, &cat.conn)?;
    all_violations.extend(fix_sha_violations);
    // The complement of the check above: it validates a declared anchor, this one reports
    // a record that declares none. Scoped to LIVE terminal bug files — 297 of 355 archived
    // files predate the rule, and the guide calls those stale instructions rather than debt.
    all_violations.extend(scan_terminal_status_without_fix_anchor(ctx, &cat.conn)?);
    // Both checks above answer "is a fix pointer declared?". This one answers why that
    // answer can be confidently wrong: an unterminated fence mutes every line-anchored
    // scan below it, so a "nothing declared" finding may be about the parse and not the
    // file. Ordered here so the two readings sit together in the report.
    all_violations.extend(scan_unterminated_fence(ctx, &cat.conn)?);

    // Ruling 17 for the row-grain checks that carry no per-row state, applied as one
    // filter over the finished list rather than five scoping blocks.
    //
    // The two in-scan precedents scope inside their own loop because they need
    // something that loop already computed — `scan_artifact_paths` the known-elsewhere
    // set, the entry-validity family an indegree key. These five need only the row's
    // own path, so a single filter is both less code and more auditable: the scoped set
    // is a list you can read in one place instead of five blocks you have to find.
    //
    // **`worktree_scoped_row` is deliberately absent, and the omission is the
    // interesting half.** The discriminator is not "is the finding foreign" but *does
    // this check's repair write files*:
    //
    // - `repair_frontmatter_id` writes to disk. It therefore refuses to run without a
    //   scope and filters `scan_frontmatter_id_mismatches` to one root (`run_fix`), so
    //   its REPORT was the outlier — it named rows its own repair declines to touch.
    // - `fix=reseat_worktree` only re-keys catalog rows. It takes no root and filters by
    //   none, reseating every unregistered worktree-scoped row in the catalog. Scoping
    //   its report while the repair stays machine-wide would understate what
    //   `confirm=true` is about to do — a worse defect than the two rows of noise it
    //   would remove.
    //
    // The remaining row-grain checks (`snapshot_drift`, `params_behind_body`,
    // `augmentation_declared_but_absent`) report zero findings here today, so they are
    // left out rather than swept in on an unmeasured assumption — see the bug file's
    // Resume. `entry_without_definition` IS listed despite reporting zero foreign rows
    // today: it shares a scan with `ledger_defines_nothing`, and one scan whose two
    // outputs scope differently is a trap for the next reader.
    const SCOPED_ROW_CHECKS: &[&str] = &[
        "frontmatter_id_mismatch",
        "frontmatter_id_is_not_a_catalog_id",
        "ledger_defines_nothing",
        "entry_without_definition",
        "terminal_status_with_caveat",
    ];
    let mut row_checks_scoped_by_project: std::collections::BTreeMap<String, usize> =
        Default::default();
    // No active project means no scoping — the same degradation the scans themselves
    // use, so a config-only caller is never handed a silently empty worklist.
    if let Some(cp) = ctx.current_project.as_deref() {
        let git_root = std::slice::from_ref(&cp.git_root);
        all_violations.retain(|v| {
            if !SCOPED_ROW_CHECKS.contains(&v.check.as_str()) {
                return true;
            }
            if super::containing_root(git_root, Path::new(&v.path)).is_some() {
                return true;
            }
            *row_checks_scoped_by_project
                .entry(outside_roots_group(&v.path))
                .or_insert(0) += 1;
            false
        });
    }

    // Catalog health: hidden-row count from the GC lifecycle (Tasks 1-5).
    // Reads happen while the lock is still held — kept minimal, then dropped
    // before computing the summary below.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let cutoff = crate::librarian::catalog::gc::visibility_cutoff_ms(&cat.conn, now_ms)?;
    let hidden_rows = crate::librarian::catalog::gc::hidden_count(&cat.conn, cutoff)?;
    let grace = crate::librarian::catalog::gc::grace_days(&cat.conn)?;
    let (slugs_with, slugs_without) =
        crate::librarian::catalog::artifact::slug_coverage(&cat.conn)?;

    // Move-candidate detection (Task 9): needs the ACTIVE repo's git root.
    // The librarian ToolContext exposes it as `current_project.git_root`,
    // populated per-call by `LibrarianAdapter::derive_ctx` from the host's
    // currently-active project (see current_project.rs). No active project
    // (e.g. a config-only caller, or an unresolvable path) means no
    // candidates — same as Task 6's placeholder. Done while the lock is
    // still held, since `detect_move_candidates` needs `&cat.conn`.
    let candidates = match ctx.current_project.as_deref() {
        Some(cp) => {
            let active_git_root = crate::util::fs::RepoPath::from_path(&cp.git_root).into_string();
            crate::librarian::catalog::gc::detect_move_candidates(&cat.conn, &active_git_root)?
        }
        None => Vec::new(),
    };

    // Computed here, not beside the other catalog_health inserts below: the
    // lock is dropped immediately after (next line) to keep lock scope
    // minimal, and audit::health needs &cat.conn.
    let mut audit_health = crate::librarian::catalog::audit::health(&cat.conn)?;
    // Task 6: scoped to the active project's repo root (main checkout for a
    // linked worktree) — same destination `export` itself now uses. No
    // active project means no scope to report against, same degradation as
    // the move-candidate detection just above.
    let pending = match ctx.current_project.as_deref() {
        Some(cp) => {
            let audit_repo_root = cp.main_root.as_deref().unwrap_or(&cp.git_root);
            crate::librarian::catalog::audit::shard::unexported_count(&cat.conn, audit_repo_root)?
        }
        None => 0,
    };
    audit_health["host"] = json!(crate::librarian::catalog::audit::host::resolve_host_id(
        &cat.conn
    )?);
    audit_health["unexported_rows"] = json!(pending);
    if pending > 0 {
        audit_health["hint"] = json!(format!(
            "{pending} audit rows are not in a committed shard — run librarian(action=\"audit_log\", export=true) and commit .codescout/audit/. A shard is a replica: it is only as fresh as its last export."
        ));
    }

    // Drop the lock before computing the summary — keeps lock scope minimal.
    drop(cat);

    // BOTH summary numbers are taken here, above the truncation below, so they
    // describe the same set and `total == by_check.values().sum()` holds by
    // construction — asserted by `summary_total_partitions_by_check`.
    //
    // Reading `all_violations.len()` *after* the `retain` is what broke that:
    // `total` counted the 10 shown rows while `by_check` counted all of them,
    // in the same `summary` object, with neither labelled as authoritative.
    // It is the same defect the audit-doc-refs counters shipped in
    // `docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.
    // Seeded with every declared check at 0 BEFORE counting, so a check that
    // ran and found nothing reports `0` instead of being absent. Absence was
    // the only signal left for "this check did not run", and it collided with
    // the happy case in the one direction that reads as a clean bill of
    // health: a clean catalog produced `by_check: {}`, indistinguishable from
    // a doctor with no checks at all.
    //
    // `total == by_check.values().sum()` still holds — zeros add nothing —
    // which `summary_total_partitions_by_check` asserts.
    let mut by_check: std::collections::BTreeMap<String, usize> = Check::ALL
        .iter()
        .map(|c| (c.as_str().to_string(), 0usize))
        .collect();
    for v in &all_violations {
        *by_check.entry(v.check.clone()).or_insert(0) += 1;
    }
    let total_violations = all_violations.len();

    // Cap the emitted `abs_path_outside_managed_roots` rows. On a catalog
    // spanning several projects this check legitimately fires for every row
    // belonging to another workspace (417 of 1314 on the authoring machine),
    // and an unbounded list buries every other finding in the report.
    //
    // `by_check` and `total` are both computed ABOVE this truncation, so the
    // summary keeps the true count, and the elision is announced in the hint
    // rather than applied silently — a report that quietly drops findings is
    // the same failure mode this check was added to close. `shown` carries the
    // post-truncation length so the two can never be confused for each other.
    // The window is caller-controlled, because a sample nobody can look past is
    // a report that names findings it cannot produce. `limit` widens or narrows
    // it; `offset` pages through the rest. Both rely on the `ORDER BY abs_path`
    // in `scan_artifact_paths` for stability — without it `offset` would page
    // through a set that reshuffles between calls.
    const OUTSIDE_ROOTS_SAMPLE_DEFAULT: usize = 10;
    let sample_limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(OUTSIDE_ROOTS_SAMPLE_DEFAULT);
    let sample_offset = args
        .get("offset")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(0);

    // Grouped BEFORE the truncation, so every row is accounted for even when
    // most are dropped. This is what keeps an alphabetically-ordered window
    // honest: the first `limit` rows all belong to whichever project sorts
    // first, and the aggregate is how a reader still sees the whole shape and
    // knows which project to page into.
    let mut outside_by_project: std::collections::BTreeMap<String, usize> = Default::default();
    for v in all_violations
        .iter()
        .filter(|v| v.check == "abs_path_outside_managed_roots")
    {
        *outside_by_project
            .entry(outside_roots_group(&v.path))
            .or_insert(0) += 1;
    }
    // Ruling 17: fold the SCOPED-OUT rows back into the metric. They never became
    // `Violation`s, so the loop above cannot see them — and a metric that shrank
    // when the worklist did would understate real cross-repo exposure, which is
    // precisely the false negative this aggregate exists to prevent.
    for (group, n) in &outside_scoped_by_project {
        *outside_by_project.entry(group.clone()).or_insert(0) += n;
    }

    let mut seen_outside = 0usize;
    let mut shown_outside = 0usize;
    let mut elided_outside = 0usize;
    all_violations.retain(|v| {
        if v.check != "abs_path_outside_managed_roots" {
            return true;
        }
        let idx = seen_outside;
        seen_outside += 1;
        if idx >= sample_offset && shown_outside < sample_limit {
            shown_outside += 1;
            true
        } else {
            elided_outside += 1;
            false
        }
    });

    let mut hint_parts: Vec<String> = Vec::new();
    if elided_outside > 0 {
        let total_outside = shown_outside + elided_outside;
        let next_offset = sample_offset + shown_outside;
        // Naming a parameter WITH a real value is the
        // docs/PROGRESSIVE_DISCOVERABILITY.md § Pattern 1 contract. The previous
        // wording told the reader to inspect the elided rows and named nothing
        // that could reach them, because nothing could.
        hint_parts.push(format!(
            "abs_path_outside_managed_roots fired {total_outside} time(s); showing {shown_outside} from offset {sample_offset}, {elided_outside} elided. \
             Rows are ordered by abs_path, so the window is stable across calls: \
             librarian(action=\"doctor\", limit={total_outside}) returns all of them, \
             or limit={sample_limit}, offset={next_offset} for the next page. \
             catalog_health.outside_roots_by_project counts every row, elided AND scoped-out ones included. \
             A row here is under NO managed root, NO umbrella sibling, and NO repo the catalog has commits for — \
             so nothing on this machine claims it and artifact(move)/artifact(delete) have nothing to resolve it against."
        ));
    }
    if !outside_scoped_by_project.is_empty() {
        let total: usize = outside_scoped_by_project.values().sum();
        let n_projects = outside_scoped_by_project.len();
        hint_parts.push(format!(
            "{total} outside-managed-roots row(s) across {n_projects} project root(s) were scoped OUT of this \
             report because they belong to a workspace this machine knows about — an umbrella sibling of the \
             active project, or a repo the catalog holds commits for. See \
             catalog_health.outside_roots_scoped_by_project. The metric is unscoped: \
             outside_roots_by_project still counts them, so cross-repo exposure is not understated — only the \
             worklist is limited to rows nothing on this machine claims."
        ));
    }
    if !entry_validity_scoped_by_project.is_empty() {
        let total_scoped: usize = entry_validity_scoped_by_project.values().sum();
        let n_projects = entry_validity_scoped_by_project.len();
        hint_parts.push(format!(
            "{total_scoped} entry-validity row(s) (entry_conditional_past_due / entry_dated_stale / \
             entry_cited_from_outside_but_undeclared / validity_unparseable) scoped out of this report because they belong \
             to {n_projects} other project root(s) — see catalog_health.entry_validity_scoped_by_project. \
             Exposure itself stays cross-repo (entry_indegree is not scoped); only the reported \
             worklist is limited to the active project, so a developer here is not handed other \
             repos' work."
        ));
    }
    if !cited_prefix_scoped.is_empty() {
        let total_scoped: usize = cited_prefix_scoped.values().sum();
        let n_projects = cited_prefix_scoped.len();
        hint_parts.push(format!(
            "{total_scoped} cited_prefix_with_no_definer finding(s) across {n_projects} other project \
             root(s) were scoped OUT of this report — the prefix is genuinely unowned, but its citations \
             are not in the active project. See catalog_health.cited_prefix_scoped_by_project. \
             Definers stay corpus-wide: narrowing them would make every prefix defined only in a sibling \
             repo fire here as unowned, so the metric is not scoped — only the worklist is."
        ));
    }
    if !row_checks_scoped_by_project.is_empty() {
        let total_scoped: usize = row_checks_scoped_by_project.values().sum();
        let n_projects = row_checks_scoped_by_project.len();
        hint_parts.push(format!(
            "{total_scoped} row-grain finding(s) (frontmatter_id_mismatch / \
             frontmatter_id_is_not_a_catalog_id / ledger_defines_nothing / \
             entry_without_definition / terminal_status_with_caveat) across {n_projects} other \
             project root(s) were scoped OUT of this report — see \
             catalog_health.row_checks_scoped_by_project. worktree_scoped_row is deliberately NOT \
             scoped: fix=reseat_worktree takes no root and reseats every unregistered row in the \
             catalog, so narrowing its report would understate what confirm=true is about to do."
        ));
    }
    if hidden_rows > 0 {
        hint_parts.push(format!(
            "{hidden_rows} row(s) hidden as missing (>{grace}d). Run librarian(action=\"doctor\", fix=\"prune_missing\") to remove, or doctor(fix=\"rehome\", old_root, new_root) to migrate a moved repo."
        ));
    }
    if let Some(c) = candidates.first() {
        hint_parts.push(format!(
            "possible repo move detected ({} -> {}). Run doctor(fix=\"rehome\", old_root=\"{}\", new_root=\"{}\") to migrate.",
            c.old_root, c.new_root, c.old_root, c.new_root
        ));
    }
    if let Some(n) = declared_roots_health
        .get("missing")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
    {
        hint_parts.push(format!(
            "declared_root_missing fired {n} time(s): .codescout/workspace.toml declares \
             sub-project root(s) that are not directories. That file is gitignored and \
             per-machine, so no diff or review will surface it — repair it there. \
             Cross-repo grouping belongs in an [[umbrella]], not a [[project]]."
        ));
    }
    let health_hint = hint_parts.join(" ");

    let mut catalog_health = serde_json::Map::new();
    catalog_health.insert("hidden_rows".to_string(), json!(hidden_rows));
    // Slug backfill progress. Reported unconditionally so the gap is visible before
    // anyone needs it: `entry_cite.src_slug` FKs `artifact(slug)`, so every row without
    // one is a source that cannot carry an entry-grain citation.
    catalog_health.insert(
        "slug_coverage".to_string(),
        json!({ "with_slug": slugs_with, "without_slug": slugs_without }),
    );
    catalog_health.insert("move_candidates".to_string(), json!(candidates.len()));
    if !candidates.is_empty() {
        let detail: Vec<Value> = candidates
            .iter()
            .map(|c| {
                json!({
                    "old_root": c.old_root,
                    "new_root": c.new_root,
                    "shared_commits": c.shared_commits,
                    "artifact_rows": c.artifact_rows,
                })
            })
            .collect();
        catalog_health.insert("move_candidates_detail".to_string(), json!(detail));
    }
    if !outside_by_project.is_empty() {
        catalog_health.insert(
            "outside_roots_by_project".to_string(),
            json!(outside_by_project),
        );
    }
    if !outside_scoped_by_project.is_empty() {
        catalog_health.insert(
            "outside_roots_scoped_by_project".to_string(),
            json!(outside_scoped_by_project),
        );
    }
    if !entry_validity_scoped_by_project.is_empty() {
        catalog_health.insert(
            "entry_validity_scoped_by_project".to_string(),
            json!(entry_validity_scoped_by_project),
        );
    }
    if !cited_prefix_scoped.is_empty() {
        catalog_health.insert(
            "cited_prefix_scoped_by_project".to_string(),
            json!(cited_prefix_scoped),
        );
    }
    if !row_checks_scoped_by_project.is_empty() {
        catalog_health.insert(
            "row_checks_scoped_by_project".to_string(),
            json!(row_checks_scoped_by_project),
        );
    }
    // Always present, even when nothing fired: its `note` is how a SKIP (linked worktree,
    // absent/unreadable/unparseable config) stays distinguishable from a pass.
    catalog_health.insert("declared_roots".to_string(), declared_roots_health);
    catalog_health.insert("archived_fix_shas".to_string(), archived_fix_shas);
    catalog_health.insert("audit".to_string(), audit_health);
    catalog_health.insert("hint".to_string(), json!(health_hint));

    Ok(json!({
        "violations": all_violations,
        "summary": {
            "total": total_violations,
            "shown": all_violations.len(),
            "by_check": by_check,
        },
        "catalog_health": catalog_health,
    }))
}

/// Validate a `prune_missing` request without touching the catalog beyond a
/// read-only registration check. Returns the validated dead-root path, or a
/// `RecoverableError` for an unsupported fix, a missing/relative root, a root
/// that still exists (a live root's rows are not orphans — per-file
/// deletions belong to reindex's walk, not a bulk prune), or a root an
/// ACTIVE `worktree_registration` still covers (pruning it would delete the
/// catalog's only remaining record of an unmerged worktree's history — that
/// belongs to `librarian(action="merge_worktree")`, not a bulk prune).
fn validate_prune_request<'a>(
    fix: &str,
    root: Option<&'a str>,
    conn: &rusqlite::Connection,
) -> Result<&'a std::path::Path> {
    if fix != "prune_missing" {
        return Err(RecoverableError::new(format!(
            "unknown fix '{fix}' — supported: prune_missing (requires root=<absolute path of the dead/renamed repo root>)"
        )));
    }
    let root = root.ok_or_else(|| {
        RecoverableError::new(
            "fix=prune_missing requires root=<absolute path of the dead/renamed repo root to prune>",
        )
    })?;
    let root_path = std::path::Path::new(root);
    if !root_path.is_absolute() {
        return Err(RecoverableError::new(format!(
            "root must be an absolute path, got '{root}'"
        )));
    }
    if root_path.exists() {
        return Err(RecoverableError::new(format!(
            "root '{root}' still exists on disk — prune_missing only removes rows under a dead/renamed root; nothing pruned"
        )));
    }
    let root_str = crate::util::fs::RepoPath::from_path(root_path).to_string();
    if worktree::covering_conn(conn, &root_str)?.is_some() {
        return Err(RecoverableError::with_hint(
            format!(
                "root '{root}' is covered by an ACTIVE worktree registration — pruning would delete the catalog's only record of an unmerged worktree's history"
            ),
            format!(
                "merge it first via librarian(action=\"merge_worktree\", root=\"{root}\"), or if the worktree is being discarded, librarian(action=\"merge_worktree\", root=\"{root}\", abandon=true) — then retry prune_missing"
            ),
        ));
    }
    Ok(root_path)
}

/// Validate a `rehome` request without touching the catalog. Returns the
/// validated `(old_root, new_root)` path pair, or a `RecoverableError` for a
/// missing/relative `old_root`/`new_root`, an `old_root` that still exists
/// on disk (rehome only migrates rows from a path that is gone — a live
/// root's rows are not orphans), a `new_root` that does not exist (rehoming
/// onto a missing directory would leave the catalog pointing at nothing), or
/// an `old_root` an ACTIVE `worktree_registration` still covers (mirrors
/// `validate_prune_request`'s guard — that history belongs to
/// `librarian(action="merge_worktree")`, not a bulk rehome).
fn validate_rehome_request<'a>(
    old_root: Option<&'a str>,
    new_root: Option<&'a str>,
    conn: &rusqlite::Connection,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let old = old_root.ok_or_else(|| {
        RecoverableError::new(
            "fix=rehome requires old_root=<absolute path the repo used to live at>",
        )
    })?;
    let new = new_root.ok_or_else(|| {
        RecoverableError::new("fix=rehome requires new_root=<absolute path the repo now lives at>")
    })?;
    let (op, np) = (std::path::Path::new(old), std::path::Path::new(new));
    if !op.is_absolute() || !np.is_absolute() {
        return Err(RecoverableError::new(
            "old_root and new_root must both be absolute paths",
        ));
    }
    if op.exists() {
        return Err(RecoverableError::new(format!(
            "old_root '{old}' still exists — rehome only migrates rows from a path that is gone"
        )));
    }
    if !np.exists() {
        return Err(RecoverableError::new(format!(
            "new_root '{new}' does not exist — cannot rehome onto a missing directory"
        )));
    }
    let old_str = crate::util::fs::RepoPath::from_path(op).to_string();
    if worktree::covering_conn(conn, &old_str)?.is_some() {
        return Err(RecoverableError::with_hint(
            format!(
                "old_root '{old}' is covered by an ACTIVE worktree registration — rehoming would orphan the catalog's only record of an unmerged worktree's history"
            ),
            format!(
                "merge it first via librarian(action=\"merge_worktree\", root=\"{old}\"), or if the worktree is being discarded, librarian(action=\"merge_worktree\", root=\"{old}\", abandon=true) — then retry rehome"
            ),
        ));
    }
    Ok((op.to_path_buf(), np.to_path_buf()))
}

/// Write a committed sidecar for every augmented artifact under `scope_root` that has none,
/// and stamp the artifact's `expects_augmentation:` to name it.
///
/// This is the export half of making shape travel, and it is the one that is
/// **time-sensitive on a machine that still holds rows others have lost**: the catalog is
/// gitignored, machine-local, and has no backup, so an augmentation absent from every clone
/// exists in exactly one place until this runs.
///
/// Skips an artifact whose sidecar already exists AND is already declared — so it is
/// idempotent, and a second run reports 0 exported rather than rewriting files.
///
/// **The skip is REPORTED, not merely `continue`d.** Idempotence is correct; delivering it
/// through silence was not. A decision to decline leaves no trace in a count of what was
/// written, so `exported: 0` is a truthful answer to "how many did you create?" and the
/// wrong answer to "did you fix my drift?" — which is what callers sent here actually
/// asked. `sidecar_shape_drift` prescribed this fix for a case whose sidecar exists by that
/// check's own precondition, making every such run a guaranteed no-op that returned no
/// error and named no artifact. The remedy text is fixed; this reports the no-op whether or
/// not anyone suspected one.
fn export_augmentation_sidecars(
    cat: &crate::librarian::catalog::Catalog,
    scope_root: &std::path::Path,
    confirm: bool,
) -> Result<(Vec<Value>, Vec<Value>, Vec<Value>)> {
    use crate::librarian::augmentation_sidecar as sidecar;

    let mut stmt = cat.conn.prepare(
        "SELECT a.id, a.abs_path FROM artifact a \
         JOIN artifact_augmentation g ON g.artifact_id = a.id \
         WHERE a.missing_since IS NULL \
         ORDER BY a.abs_path",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut exported: Vec<Value> = Vec::new();
    let mut failed: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();

    for (id, abs_path) in rows {
        let art = std::path::Path::new(&abs_path);
        // Component-boundary containment, the same predicate the other writing fixes use:
        // a prefix match would let `/repo-backup` pass as `/repo`, and this WRITES.
        // Out of scope is not a decision about this artifact, so it stays unreported.
        if !art.starts_with(scope_root) {
            continue;
        }
        let sidecar_abs = sidecar::path_for(scope_root, art);
        let sidecar_rel = sidecar::rel_path_for(scope_root, art);

        let Ok(content) = std::fs::read_to_string(art) else {
            failed.push(json!({ "path": abs_path, "error": "unreadable" }));
            continue;
        };
        let declared_already = crate::librarian::frontmatter::parse(&content)
            .ok()
            .and_then(|(fm, _)| fm)
            .and_then(|fm| fm.extra.get("expects_augmentation").cloned())
            .is_some_and(|v| {
                matches!(parse_declaration(&v), Declaration::Declared { sidecar: Some(s) } if s == sidecar_rel)
            });
        if declared_already && sidecar_abs.is_file() {
            skipped.push(json!({
                "path": abs_path,
                "sidecar": sidecar_rel,
                "reason": "already exported and declared — this fix CREATES sidecars and \
                           never refreshes them. To republish a shape this catalog owns, \
                           delete the sidecar and re-run.",
            }));
            continue;
        }

        let Ok(Some(row)) = crate::librarian::catalog::augmentation::get(cat, &id) else {
            failed
                .push(json!({ "path": abs_path, "error": "augmentation row vanished mid-sweep" }));
            continue;
        };

        if !confirm {
            exported.push(json!({ "path": abs_path, "sidecar": sidecar_rel }));
            continue;
        }

        let doc = sidecar::AugmentationSidecar::from_row(&row);
        if let Err(e) = sidecar::write(&sidecar_abs, &doc) {
            // One unwritable file must not abandon the rest of the sweep — the same
            // reasoning as `repair_frontmatter_id`, and it matters more here because the
            // rows this is rescuing may exist nowhere else.
            failed.push(json!({ "path": abs_path, "error": format!("{e:#}") }));
            continue;
        }
        match stamp_augmentation_declaration(art, &sidecar_rel) {
            Ok(()) => exported.push(json!({ "path": abs_path, "sidecar": sidecar_rel })),
            Err(e) => failed.push(json!({
                "path": abs_path,
                "error": format!("sidecar written but declaration not stamped: {e:#}")
            })),
        }
    }
    Ok((exported, failed, skipped))
}

/// Point an artifact's `expects_augmentation:` at its sidecar, preserving everything else.
///
/// Goes through `frontmatter::parse` + `frontmatter::write` rather than a line edit, because
/// that pair already owns the reserved-key and scalar-quoting rules a hand-written
/// `expects_augmentation: <path>` line would have to re-learn.
fn stamp_augmentation_declaration(artifact: &std::path::Path, sidecar_rel: &str) -> Result<()> {
    let content = std::fs::read_to_string(artifact)?;
    let (fm, body) = crate::librarian::frontmatter::parse(&content)?;
    let mut fm = fm.ok_or_else(|| anyhow::anyhow!("no frontmatter block"))?;
    fm.extra.insert(
        "expects_augmentation".to_string(),
        Value::String(sidecar_rel.to_string()),
    );
    std::fs::write(artifact, crate::librarian::frontmatter::write(&fm, body))?;
    Ok(())
}

/// Opt-in catalog repair. Two fixes: `prune_missing` — remove every row
/// anchored under a dead/renamed repo `root`; `reseat_worktree` — re-point
/// `no_collision` worktree-scoped rows (from `scan_worktree_scoped`) onto
/// their main-repo path, leaving `collision` rows untouched for a manual
/// `graft`.
async fn run_fix(
    ctx: &ToolContext,
    fix: &str,
    root: Option<&str>,
    new_root: Option<&str>,
    confirm: bool,
) -> Result<Value> {
    match fix {
        "prune_missing" => {
            let cat = ctx.catalog.lock();
            match root {
                Some(_) => {
                    // Single-root path (unchanged behaviour).
                    let root_path = validate_prune_request(fix, root, &cat.conn)?;
                    let (artifact_rows, commit_rows) = prune_dead_root(&cat.conn, root_path)?;
                    let out = json!({
                        "fix": "prune_missing",
                        "root": root_path.to_string_lossy(),
                        "pruned": { "artifact_rows": artifact_rows, "commit_rows": commit_rows },
                    });
                    drop(cat);
                    Ok(out)
                }
                None => {
                    // Batch mode over all doctor-identified dead roots.
                    let dead_roots = derive_dead_roots(&cat.conn)?;
                    if !confirm {
                        let mut rows = Vec::new();
                        let (mut ta, mut tc) = (0usize, 0usize);
                        for r in &dead_roots {
                            let (a, c) = count_dead_root(&cat.conn, r)?;
                            let root_str = crate::util::fs::RepoPath::from_path(r).to_string();
                            let covered = worktree::covering_conn(&cat.conn, &root_str)?.is_some();
                            if covered {
                                // Mirror the apply-time skip so the dry-run
                                // preview's totals never promise more than
                                // `confirm=true` would actually delete.
                                rows.push(json!({
                                    "root": r.to_string_lossy(),
                                    "artifact_rows": a, "commit_rows": c,
                                    "would_skip": "active worktree registration",
                                }));
                            } else {
                                ta += a;
                                tc += c;
                                rows.push(json!({
                                    "root": r.to_string_lossy(),
                                    "artifact_rows": a, "commit_rows": c,
                                }));
                            }
                        }
                        return Ok(json!({
                            "fix": "prune_missing", "mode": "dry_run",
                            "dead_roots": rows,
                            "totals": { "roots": dead_roots.len(), "artifact_rows": ta, "commit_rows": tc },
                            "hint": "re-run with confirm=true to prune these rows",
                        }));
                    }
                    let mut results = Vec::new();
                    let (mut ta, mut tc) = (0usize, 0usize);
                    for r in &dead_roots {
                        let root_str = crate::util::fs::RepoPath::from_path(r).to_string();
                        if worktree::covering_conn(&cat.conn, &root_str)?.is_some() {
                            results.push(json!({
                                "root": r.to_string_lossy(),
                                "skipped": "active worktree registration — merge_worktree first",
                            }));
                            continue;
                        }
                        let (a, c) = prune_dead_root(&cat.conn, r)?;
                        ta += a;
                        tc += c;
                        results.push(json!({
                            "root": r.to_string_lossy(),
                            "artifact_rows": a, "commit_rows": c,
                        }));
                    }
                    Ok(json!({
                        "fix": "prune_missing", "mode": "applied",
                        "pruned": results,
                        "totals": { "artifact_rows": ta, "commit_rows": tc },
                    }))
                }
            }
        }
        "reseat_worktree" => reseat_worktree(ctx),
        // Sweep-all WITHIN ONE ROOT, dry-run by default. Reuses `mv`'s repair so the
        // invariant has exactly one implementation: a move writes the new id going
        // forward, this rewrites the ones written before that shipped. BL-23.
        //
        // The scope is mandatory because this fix WRITES and the catalog is
        // machine-global. Measured on a live dry-run before this guard existed: 207
        // files across FIVE unrelated repositories (backend-kotlin, eduplanner-ui,
        // two southpole projects, claude-plugins), only ~90 of them the active
        // project's. `prune_missing` and `rehome` are root-scoped for the same
        // reason. "Sweep-all" means don't make me name each artifact — not cross
        // into another repository.
        "repair_frontmatter_id" => {
            let scope_root = match root {
                Some(r) => std::path::PathBuf::from(r),
                None => match ctx.current_project.as_deref() {
                    Some(cp) => cp.git_root.clone(),
                    None => {
                        return Err(RecoverableError::new(
                            "fix=repair_frontmatter_id needs a scope — activate a project, \
                             or pass root=<repo root>. The catalog spans every repo indexed \
                             on this machine, and this fix rewrites files.",
                        ))
                    }
                },
            };
            let roots = [scope_root.clone()];
            let stale: Vec<Violation> = {
                let cat = ctx.catalog.lock();
                scan_frontmatter_id_mismatches(&cat.conn)?
            }
            .into_iter()
            // Component-boundary containment, the same predicate
            // `abs_path_outside_managed_roots` uses — a prefix match would let
            // `/repo-backup` pass as `/repo`.
            .filter(|v| super::containing_root(&roots, std::path::Path::new(&v.path)).is_some())
            .collect();
            if !confirm {
                return Ok(json!({
                    "fix": "repair_frontmatter_id", "mode": "dry_run",
                    "root": scope_root.to_string_lossy(),
                    "files": stale.iter()
                        .map(|v| json!({ "path": v.path, "detail": v.detail }))
                        .collect::<Vec<_>>(),
                    "totals": { "files": stale.len() },
                    "hint": "re-run with confirm=true to rewrite these frontmatter ids. \
                             Only files under `root` are touched; pass root=<other repo> to sweep another.",
                }));
            }
            let mut repaired = Vec::new();
            let mut failed = Vec::new();
            for v in &stale {
                // `artifact_id` is always Some for this check — it is built from the
                // catalog row — but a default beats an unwrap in a repair loop.
                let id = v.artifact_id.clone().unwrap_or_default();
                match super::mv::repair_frontmatter_id(std::path::Path::new(&v.path), &id) {
                    Ok(_) => repaired.push(json!({ "path": v.path, "id": id })),
                    // One unwritable file must not abandon the rest of the sweep.
                    Err(err) => failed.push(json!({ "path": v.path, "error": format!("{err:#}") })),
                }
            }
            Ok(json!({
                "fix": "repair_frontmatter_id", "mode": "applied",
                "root": scope_root.to_string_lossy(),
                "repaired": repaired,
                "failed": failed,
                "totals": { "files": repaired.len(), "failed": failed.len() },
            }))
        }
        "rehome" => {
            let cat = ctx.catalog.lock();
            let (old, new) = validate_rehome_request(root, new_root, &cat.conn)?;
            let plan = crate::librarian::catalog::gc::plan_rehome(&cat.conn, &old, &new)?;
            if plan.rows.is_empty() && plan.collisions.is_empty() {
                return Err(RecoverableError::new(format!(
                    "no catalog rows found under old_root '{}'",
                    old.display()
                )));
            }
            if !confirm {
                return Ok(json!({
                    "fix": "rehome", "mode": "dry_run",
                    "old_root": old.to_string_lossy(), "new_root": new.to_string_lossy(),
                    "artifact_rows": plan.rows.len(),
                    "commit_rows": plan.commit_rows,
                    "collisions": plan.collisions,
                    "hint": "re-run with confirm=true to migrate these rows (ids + history preserved)",
                }));
            }
            let stats = crate::librarian::catalog::gc::apply_rehome(&cat.conn, &plan)?;
            let commit_rows = crate::librarian::catalog::gc::rehome_commits(&cat.conn, &old, &new)?;
            Ok(json!({
                "fix": "rehome", "mode": "applied",
                "old_root": old.to_string_lossy(), "new_root": new.to_string_lossy(),
                "migrated": { "artifact_rows": stats.artifact_rows, "commit_rows": commit_rows,
                              "skipped_collisions": stats.skipped_collisions },
            }))
        }
        "mint_slugs" => {
            // Layer 3a of the Statement spec: `entry_cite.src_slug` FKs
            // `artifact(slug)`, so an entry-grain edge can only exist for a source
            // that has a slug — and slugs are minted lazily on first
            // `append_entry(cites=…)`. Measured 2026-08-20: 2 of 4106 rows had one.
            //
            // NOT scoped to a root, unlike `repair_frontmatter_id`: that one writes
            // FILES in whatever repo it sweeps, while this writes only machine-local
            // catalog rows. One deterministic corpus-wide pass also makes assignment a
            // pure function of (corpus, already-minted slugs) rather than of which
            // repo happened to run it first — and a slug is immutable once set, so
            // that ordering is permanent.
            let cat = ctx.catalog.lock();
            let minted =
                crate::librarian::catalog::artifact::mint_missing_slugs(&cat.conn, confirm)?;
            let (with, without) = crate::librarian::catalog::artifact::slug_coverage(&cat.conn)?;
            drop(cat);

            // Sampled like `abs_path_outside_managed_roots`: the full list is ~4000
            // rows and would bury the response. `totals` counts every one, and the
            // elision is announced rather than applied silently.
            const SAMPLE: usize = 10;
            let sample: Vec<Value> = minted
                .iter()
                .take(SAMPLE)
                .map(|m| json!({ "path": m.abs_path, "slug": m.slug }))
                .collect();
            let elided = minted.len().saturating_sub(sample.len());
            Ok(json!({
                "fix": "mint_slugs",
                "mode": if confirm { "applied" } else { "dry_run" },
                "minted": minted.len(),
                "sample": sample,
                "elided": elided,
                "slug_coverage": { "with_slug": with, "without_slug": without },
                "hint": if confirm {
                    "slugs are immutable once minted — a re-run mints only rows still missing one."
                } else {
                    "dry run: the mint ran and was rolled back, so these are the exact slugs \
                     confirm=true would assign. Re-run with confirm=true to keep them."
                },
            }))
        }
        // Scope is mandatory for the same reason `repair_frontmatter_id`'s is: this WRITES
        // and the catalog is machine-global. "Sweep-all" means don't make me name each
        // artifact — not cross into another repository.
        "export_augmentations" => {
            let scope_root = match root {
                Some(r) => std::path::PathBuf::from(r),
                None => match ctx.current_project.as_deref() {
                    Some(cp) => cp.git_root.clone(),
                    None => {
                        return Err(RecoverableError::new(
                            "fix=export_augmentations needs a scope — activate a project, \
                             or pass root=<repo root>. The catalog spans every repo indexed \
                             on this machine, and this fix writes files.",
                        ))
                    }
                },
            };
            let (exported, failed, skipped) = {
                let cat = ctx.catalog.lock();
                export_augmentation_sidecars(&cat, &scope_root, confirm)?
            };
            Ok(json!({
                "fix": "export_augmentations",
                "mode": if confirm { "applied" } else { "dry_run" },
                "root": scope_root.to_string_lossy(),
                "exported": exported,
                "failed": failed,
                "skipped": skipped,
                "totals": {
                    "exported": exported.len(),
                    "failed": failed.len(),
                    "skipped": skipped.len(),
                },
                // The all-skipped arm comes FIRST because it is the one a misled reader
                // actually lands on: `sidecar_shape_drift` used to send them here, and
                // every one of its findings has a sidecar on disk by that check's own
                // precondition. Explaining the no-op where it happens beats explaining it
                // in a finding they may never re-read.
                "hint": if exported.is_empty() && !skipped.is_empty() {
                    "Nothing written: every augmented artifact in scope already has a \
                     declared sidecar, and this fix CREATES sidecars rather than refreshing \
                     them. If you arrived here from sidecar_shape_drift, note that all of \
                     its findings have a sidecar on disk — so this call could not have \
                     repaired one. Establish which side is correct first; if it is the \
                     catalog, delete the sidecar and re-run to republish its shape."
                } else if confirm {
                    "Commit docs/augmentations/ and the stamped frontmatter together — the \
                     sidecar is only a recovery path once it is in git."
                } else {
                    "dry run: nothing written. Re-run with confirm=true. Run this on a \
                     machine whose catalog still HOLDS the augmentations — it can only \
                     export rows this catalog has."
                },
            }))
        }
        other => Err(RecoverableError::new(format!(
            "unknown fix '{other}' — expected 'prune_missing', 'reseat_worktree', \
             'rehome', 'repair_frontmatter_id', 'mint_slugs', or 'export_augmentations'"
        ))),
    }
}

/// `fix=reseat_worktree`: consume `scan_worktree_scoped` violations. Rows
/// where an ACTIVE `worktree_registration` covers the worktree root
/// (`detail.registered == true`) are SKIPPED — they are pending
/// `librarian(action="merge_worktree")`, which folds registered shadows onto
/// their main-repo counterparts via the overlay, not this legacy reseat
/// path; reseating them here would sever the row from the registration
/// bookkeeping `merge_worktree` depends on. Skipped rows are reported under
/// `skipped`, not silently dropped.
///
/// For each remaining (unregistered) `no_collision` row (`id_w`, at the
/// worktree path), durably re-seed a row at the main-repo path instead of a
/// bare `abs_path` UPDATE. Catalog identity is `id ==
/// artifact_id_from_abs(abs_path)`; keeping `id_w` while pointing `abs_path`
/// at the main path would leave that invariant broken, and the next
/// MAIN-repo reindex's [`artifact::upsert`] pre-clean (`DELETE FROM
/// artifact WHERE abs_path=? AND id != ?`) would delete the row — cascading
/// away its events / links / event_edges / augmentation (the git-invisible
/// `append_entry` history this feature exists to preserve).
///
/// Instead: seed a fresh row at `id_m = artifact_id_from_abs(main_path)`
/// (`no_collision` means nothing lives there yet, so the pre-clean deletes
/// nothing), then [`graft::graft_rows`] folds `id_w`'s entire history —
/// including the augmentation — onto `id_m` and deletes `id_w`. A subsequent
/// reindex now hits `ON CONFLICT(id)` (id already matches path) instead of
/// the pre-clean `DELETE`, so nothing is lost. `collision` rows are left
/// untouched and reported for a manual `graft`.
fn reseat_worktree(ctx: &ToolContext) -> Result<Value> {
    let mut cat = ctx.catalog.lock();
    // Owned Vec: the immutable borrow of `cat.conn` ends here, before the
    // mutable `graft_rows` calls below.
    let violations = scan_worktree_scoped(&cat.conn)?;
    let mut reseated = Vec::new();
    let mut collisions = Vec::new();
    let mut skipped = Vec::new();
    for v in &violations {
        let Some(id_w) = v.artifact_id.as_deref() else {
            continue;
        };
        let detail: Value = serde_json::from_str(&v.detail).unwrap_or_default();
        if detail["registered"].as_bool() == Some(true) {
            skipped.push(json!({
                "id": id_w,
                "main_path": detail["main_path"].clone(),
                "reason": "registered — pending librarian(action=\"merge_worktree\"), not reseat_worktree",
            }));
            continue;
        }
        let main_path = detail["main_path"].as_str().unwrap_or_default();
        match detail["classification"].as_str() {
            Some("no_collision") => {
                let Some(row_w) = artifact::get(&cat, id_w)? else {
                    continue; // race: row vanished since the scan; nothing to reseat
                };
                let id_m = ids::artifact_id_from_abs(Path::new(main_path));
                let row_m = ArtifactRow {
                    id: id_m.clone(),
                    abs_path: PathBuf::from(main_path),
                    ..row_w
                };
                // Two separate transactions (`upsert` autocommits; `graft_rows`
                // runs its own IMMEDIATE tx) — acceptable for a manual
                // diagnostic. A crash between them is recoverable, not data
                // loss: either an orphan `id_m` row with no history yet, or an
                // un-grafted `id_w` that the next run's scan reports as a
                // `collision` against `id_m` for a manual `graft`.
                artifact::upsert(&cat, &row_m)?;
                graft::graft_rows(&mut cat, id_w, &id_m)?;
                reseated.push(json!({
                    "old_id": id_w,
                    "new_id": id_m,
                    "new_path": main_path,
                }));
            }
            _ => collisions.push(json!({
                "id": id_w,
                "main_path": main_path,
                "into_id": detail["collision_with"].clone(),
            })),
        }
    }
    drop(cat);
    Ok(json!({
        "fix": "reseat_worktree",
        "reseated": reseated,
        "collisions": collisions,
        "skipped": skipped,
    }))
}

/// Delete every catalog row anchored under a dead repo `root`: `artifact` rows
/// whose `abs_path` is `root` or under `root/`, and `commits` rows whose
/// `git_root` is `root` or under `root/`. Runs through codescout's own
/// (vec0-linked, trusted-schema) connection, so the `artifact_vec` cascade
/// trigger and the FK `ON DELETE CASCADE`s (augmentation / links / events) all
/// fire — a bare `sqlite3` CLI cannot (7ca71bf7). Returns (artifact_rows,
/// commit_rows) removed.
fn prune_dead_root(conn: &rusqlite::Connection, root: &std::path::Path) -> Result<(usize, usize)> {
    let root_fwd = format!("{}", crate::util::fs::RepoPath::from_path(root));
    let under = format!("{root_fwd}/%");
    let artifact_rows = conn.execute(
        "DELETE FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2",
        rusqlite::params![root_fwd, under],
    )?;
    let commit_rows = conn.execute(
        "DELETE FROM commits WHERE git_root = ?1 OR git_root LIKE ?2",
        rusqlite::params![root_fwd, under],
    )?;
    Ok((artifact_rows, commit_rows))
}

/// Distinct DEAD ROOTS to prune, derived from the catalog's missing rows. A
/// missing artifact is included ONLY if its parent directory is ALSO missing (a
/// whole subtree is gone, not a single file under a live dir — single-file
/// deletions under a live repo are reindex's job). The dead root is the highest
/// nonexistent ancestor whose parent still exists. Returns a sorted, de-duped list.
fn derive_dead_roots(conn: &rusqlite::Connection) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut stmt = conn.prepare("SELECT abs_path FROM artifact")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut roots = std::collections::BTreeSet::new();
    for p in &paths {
        let path = std::path::Path::new(p);
        // Malformed (non-absolute) abs_path rows are abs_path_must_be_absolute's
        // eviction job, not a dead-root: skip them so the climb never bottoms out
        // at an empty PathBuf (whose prune WHERE would match every absolute row).
        if !path.is_absolute() {
            continue;
        }
        if path.exists() {
            continue; // not a missing row
        }
        match path.parent() {
            Some(parent) if parent.exists() => continue, // single file under a live dir
            None => continue,
            _ => {}
        }
        // Walk up to the highest nonexistent ancestor whose parent exists.
        let mut dead = path.to_path_buf();
        while let Some(parent) = dead.parent() {
            if parent.exists() {
                break;
            }
            dead = parent.to_path_buf();
        }
        roots.insert(dead);
    }
    Ok(roots.into_iter().collect())
}

/// Read-only count of `(artifact_rows, commit_rows)` under `root`, mirroring the
/// WHERE clauses `prune_dead_root` deletes with.
fn count_dead_root(
    conn: &rusqlite::Connection,
    root: &std::path::Path,
) -> anyhow::Result<(usize, usize)> {
    let root_fwd = format!("{}", crate::util::fs::RepoPath::from_path(root));
    let under = format!("{root_fwd}/%");
    let arts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE abs_path = ?1 OR abs_path LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    let commits: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commits WHERE git_root = ?1 OR git_root LIKE ?2",
        rusqlite::params![root_fwd, under],
        |r| r.get(0),
    )?;
    Ok((arts.max(0) as usize, commits.max(0) as usize))
}

/// Group key for the `outside_roots_by_project` aggregate: the project root a
/// row's path implies.
///
/// Artifact paths are `<project>/docs/...` by convention, so the prefix before
/// the first `docs` component names the project. A path with no `docs`
/// component is a stray file rather than part of a docs tree; its parent
/// directory is the honest answer there, since nothing in the path identifies a
/// project to attribute it to.
///
/// This is deliberately a *reporting* key, not a managed-root lookup. It exists
/// so a truncated sample can still account for every row it dropped, and it
/// never decides whether a row is a violation.
fn outside_roots_group(path: &str) -> String {
    // Split the string, not a `PathBuf`. `input` is a catalog `abs_path`, forward-slash
    // by invariant (check #1) — but `PathBuf::push` re-joins with the NATIVE separator,
    // so the round-trip emitted `\home\u\work\proj` on Windows and this function was
    // producing the very spelling `check_backslash` exists to forbid.
    let mut end = None;
    let mut cursor = 0usize;
    for part in path.split('/') {
        if part == "docs" {
            end = Some(cursor);
            break;
        }
        cursor += part.len() + 1; // component plus the '/' that followed it
    }
    if let Some(end) = end {
        // `end` indexes the '/' before `docs`; slicing there drops the trailing separator.
        return path[..end.saturating_sub(1)].to_string();
    }
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => path.to_string(),
    }
}

/// Pulls every `(id, abs_path)` row once and runs six per-row checks
/// (abs_path_must_be_absolute / backslash / ads_colon / dotdot /
/// missing_file / outside_managed_roots). Single SQL fetch + in-memory passes
/// is cheaper than six separate queries. `abs_path_must_be_absolute` runs
/// first because it is the gating shape check — a relative-path row should be
/// evicted, not further analyzed, so the managed-root check is skipped for it
/// (a relative path resolves under nothing, and reporting both would be one
/// defect wearing two names).
///
/// `roots` is the managed-root list from [`super::managed_roots`]. Empty means
/// the caller had no active project and no `[[roots]]`, in which case the
/// managed-root check is skipped entirely rather than flagging every row —
/// "no roots configured" is not evidence that any row is misplaced.
///
/// `known_elsewhere` is [`known_workspace_roots`]. An outside-roots row under
/// one of those is SCOPED OUT of the returned violations and counted into the
/// returned map instead — Ruling 17: the metric stays global, the worklist is
/// the active developer's. The caller owes folding that map into
/// `catalog_health.outside_roots_by_project` so the global count is unchanged.
/// Pass an empty slice to get the pre-2026-08-27 behaviour of reporting every
/// firing row.
fn scan_artifact_paths(
    conn: &rusqlite::Connection,
    roots: &[PathBuf],
    known_elsewhere: &[PathBuf],
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    // `ORDER BY abs_path` is load-bearing, not tidiness. The outside-roots
    // findings are sampled downstream, and without an ORDER BY the kept rows
    // are whatever prefix the planner happened to return — a set that can
    // change when an index is added or the DB is VACUUMed, with no content
    // change at all. A stable order is also what makes the `offset` parameter
    // mean anything. See
    // docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    let mut scoped: std::collections::BTreeMap<String, usize> = Default::default();
    for (id, abs_path) in &rows {
        let not_absolute = check_abs_path_must_be_absolute(id, abs_path);
        let is_absolute = not_absolute.is_none();
        if let Some(v) = not_absolute {
            violations.push(v);
        }
        if let Some(v) = check_backslash(id, abs_path, "backslash_in_abs_path") {
            violations.push(v);
        }
        if let Some(v) = check_ads_colon(id, abs_path) {
            violations.push(v);
        }
        if let Some(v) = check_dotdot_segment(id, abs_path) {
            violations.push(v);
        }
        if let Some(v) = check_missing_file(id, abs_path) {
            violations.push(v);
        }
        // Reads the file, so it runs after the cheap string checks. A missing or
        // unreadable file yields None here — `check_missing_file` above already
        // owns that finding.
        if let Some(v) = check_frontmatter_id_matches_catalog(id, abs_path) {
            violations.push(v);
        }
        if is_absolute && !roots.is_empty() {
            if let Some(v) = check_outside_managed_roots(id, abs_path, roots) {
                // Belongs to a workspace this machine knows about (umbrella
                // sibling, or a repo the catalog has commits for)? Then it is
                // real, it is someone's, and it is not this developer's work.
                // Counted, not reported.
                if super::containing_root(known_elsewhere, Path::new(abs_path)).is_some() {
                    *scoped.entry(outside_roots_group(abs_path)).or_insert(0) += 1;
                } else {
                    violations.push(v);
                }
            }
        }
    }
    Ok((violations, scoped))
}

/// Roots that belong to a workspace this machine KNOWS about, but which are not
/// managed roots of the active session.
///
/// This is the discriminator [`check_outside_managed_roots`]'s doc comment has
/// always described in prose — *"expected if it belongs to another workspace; a
/// defect if it should be under one of ..."* — and never computed. Without it the
/// check cannot separate the two, so it reported both and told the reader to sort
/// it out; measured 2026-08-27 on this machine, that was **401 of 516** findings,
/// 78% of the report.
///
/// Two sources, unioned, both already on hand:
///
/// - **Umbrella members.** If the active project declares an umbrella, its sibling
///   members are workspaces the user has explicitly grouped with this one. They are
///   the single largest bucket — 359 of 402 firing rows.
/// - **`commits.git_root`.** Every repo the librarian has ever indexed left rows
///   here (29 distinct roots at measurement). A path under one of them belongs to a
///   real, known checkout — 33 further rows.
///
/// What is left over is the finding worth a reader's attention: 10 rows under no
/// umbrella member and no repo the catalog has ever seen. Those are the ones
/// `artifact(move)` / `artifact(delete)` will refuse with nothing to point at.
///
/// Deliberately does NOT consult the filesystem. A row is scoped out for belonging
/// somewhere known, not for existing — `check_missing_file` already owns
/// disappearance, and conflating them would make one defect wear two names, which
/// is the same rule [`scan_artifact_paths`] applies to the relative-path case.
fn known_workspace_roots(ctx: &ToolContext, conn: &rusqlite::Connection) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    if let Some(name) = ctx
        .current_project
        .as_deref()
        .and_then(|c| c.umbrella.as_deref())
    {
        if let Some(u) = ctx.workspace.umbrellas.iter().find(|u| u.name == name) {
            for m in &u.members {
                let p = PathBuf::from(m);
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }

    // Best-effort: a catalog without a `commits` table (or an unreadable one) just
    // contributes no roots. This check is advisory, and failing the whole doctor
    // run over a missing aggregate would be worse than reporting a few extra rows.
    if let Ok(mut stmt) = conn.prepare(
        "SELECT DISTINCT git_root FROM commits WHERE git_root IS NOT NULL AND git_root <> ''",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for gr in rows.flatten() {
                let p = PathBuf::from(gr);
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }

    out
}

/// Every `artifact.abs_path` must resolve under some managed root.
///
/// This is exactly the precondition [`super::containing_root`] enforces for
/// `artifact(move)` and `artifact(delete)`, restated as an invariant so it is
/// observable *before* someone tries to move a file. It exists because it was
/// missing: `containing_root` could not match any catalog path on Windows
/// (WIN-30) — the catalog stores `//?/C:/...` while `current_project` holds
/// `\\?\C:\...` — and every other doctor check stayed green throughout,
/// because `check_backslash` was busy enforcing the very forward-slash form
/// that broke the comparison.
///
/// A firing row is not necessarily corrupt: a catalog spanning several
/// workspaces legitimately holds rows outside the active project's roots. The
/// detail therefore names the roots that were tried, so "belongs to another
/// workspace" is distinguishable from "drifted spelling" at a glance rather
/// than by re-deriving it.
fn check_outside_managed_roots(id: &str, abs_path: &str, roots: &[PathBuf]) -> Option<Violation> {
    if super::containing_root(roots, Path::new(abs_path)).is_some() {
        return None;
    }

    // Cap the root list: a large workspace registry would otherwise dominate
    // the report, and the first few are the ones a reader actually checks.
    const MAX_LISTED: usize = 5;
    let listed: Vec<String> = roots
        .iter()
        .take(MAX_LISTED)
        .map(|r| r.display().to_string())
        .collect();
    let elided = roots.len().saturating_sub(listed.len());
    let suffix = if elided > 0 {
        format!(" (+{elided} more)")
    } else {
        String::new()
    };

    Some(Violation::new(
        "abs_path_outside_managed_roots",
        Some(id.to_string()),
        abs_path,
        format!(
            "resolves under none of the {} managed root(s), so artifact(move) and \
             artifact(delete) will refuse this row. Expected if it belongs to another \
             workspace; a defect if it should be under one of: {}{}",
            roots.len(),
            listed.join(", "),
            suffix
        ),
    ))
}

/// `declared_root_missing`: a `[[project]]` in `.codescout/workspace.toml` declares a
/// `root` that does not resolve to a directory under the root that owns the file.
///
/// Sited beside [`check_outside_managed_roots`] because it is the same class of defect —
/// config state that no compile step, no test and no review ever sees. It is worse in one
/// respect: the file is gitignored, so it is per-machine and never appears in a diff.
///
/// **The failure it exists to catch, measured.**
/// `docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md`:
/// a config authored for a `$HOME`-rooted session was persisted into `<repo>/.codescout/`,
/// so eight *sibling repos* stood declared as sub-projects of this workspace with roots
/// relative to the wrong base. All eight had `relative_root != "."`, so
/// [`crate::workspace::Workspace::memory_dir_for_project`] routed their memory writes into
/// `<repo>/.codescout/projects/<id>/memories` — answering the wrong question correctly, for
/// a month. `workspace(action="activate")` reported repo-rooted discovery the whole time, so
/// the two views of "what projects exist" disagreed and nothing said so out loud.
///
/// **Why the base and the config are read from one variable.** Declared `root` values are
/// relative to whichever directory owns the file. Candidate roots are probed in
/// [`super::managed_roots`] order (`git_root`, then `abs_path`) and the FIRST one actually
/// holding a `.codescout/workspace.toml` becomes the base every declaration is joined onto.
/// Deriving a base independently of the file is how a mis-rooted config gets validated
/// against the root it was mis-rooted *from*, and passes.
///
/// **A linked worktree is skipped on purpose.** The file is gitignored, so it does not
/// travel into `git worktree add`; discovery there falls back to the main checkout's copy —
/// the state `project_status` already labels `topology: "inherited"`. Reporting main's
/// declarations from each worktree would multiply one defect by the number of checkouts, and
/// the operator cannot repair it from here in any case. The skip is *stated* in
/// `catalog_health.declared_roots.note` rather than applied silently: a check that goes quiet
/// without saying why is indistinguishable from a check that passed, which is the
/// misleading-green this module exists to prevent. Unreadable and unparseable configs are
/// reported the same way, and for the same reason.
///
/// Reports only; there is no `fix=`. The right value for a wrong `root` is a fact about the
/// operator's disk layout, not something a repair can derive.
fn scan_declared_project_roots(ctx: &ToolContext) -> (Vec<Violation>, Value) {
    let Some(cp) = ctx.current_project.as_deref() else {
        return (
            Vec::new(),
            json!({
                "config": Value::Null,
                "note": "no active project resolved, so no workspace config was located — \
                         declared project roots were NOT checked",
            }),
        );
    };

    let mut candidates: Vec<&Path> = vec![cp.git_root.as_path()];
    if cp.abs_path != cp.git_root {
        candidates.push(cp.abs_path.as_path());
    }
    let located = candidates.into_iter().find_map(|root| {
        let path = crate::config::workspace::workspace_config_path(root);
        path.is_file().then_some((root, path))
    });

    let Some((base, config_path)) = located else {
        let note = if cp.main_root.is_some() {
            "no .codescout/workspace.toml in this linked worktree, so sub-project discovery \
             inherits the MAIN checkout's copy (the file is gitignored and does not travel). \
             Declared roots are main's to check — run doctor from the main checkout. NOT \
             checked here."
        } else {
            "no .codescout/workspace.toml under the active project, so nothing declares \
             sub-project roots — nothing to check"
        };
        return (Vec::new(), json!({"config": Value::Null, "note": note}));
    };

    let config_display = crate::util::fs::RepoPath::from_path(&config_path).into_string();
    let text = match std::fs::read_to_string(&config_path) {
        Ok(t) => t,
        Err(e) => {
            return (
                Vec::new(),
                json!({
                    "config": config_display,
                    "note": format!(
                        "unreadable ({e}) — declared roots were NOT checked. This is not a pass."
                    ),
                }),
            )
        }
    };
    let cfg: crate::config::workspace::WorkspaceConfig = match toml::from_str(&text) {
        Ok(c) => c,
        Err(e) => {
            return (
                Vec::new(),
                json!({
                    "config": config_display,
                    "note": format!(
                        "unparseable ({e}) — declared roots were NOT checked. This is not a pass."
                    ),
                }),
            )
        }
    };

    let base_display = crate::util::fs::RepoPath::from_path(base).into_string();
    let mut out = Vec::new();
    for entry in &cfg.projects {
        let declared = Path::new(&entry.root);

        // Absolute roots are checked first and separately, because `Path::join` DISCARDS the
        // base when the joined path is absolute — so an absolute declaration would be
        // validated against itself and pass, which is precisely the cross-repo
        // mis-declaration the field cannot express. Cross-repo grouping is an `[[umbrella]]`.
        if declared.is_absolute() {
            out.push(Violation::new(
                "declared_root_missing",
                None,
                config_display.clone(),
                format!(
                    "[[project]] id = \"{}\" declares an ABSOLUTE root = \"{}\". Sub-project \
                     roots are relative to the directory owning this file ({base_display}); an \
                     absolute value escapes the workspace, and a sibling repo declared this way \
                     routes every per-project memory write for `{}` into \
                     `<workspace>/.codescout/projects/{}/memories`. Cross-repo grouping belongs \
                     in an [[umbrella]], not a [[project]].",
                    entry.id, entry.root, entry.id, entry.id
                ),
            ));
            continue;
        }

        let resolved = base.join(declared);
        if resolved.is_dir() {
            continue;
        }
        let what = if resolved.exists() {
            "exists but is not a directory"
        } else {
            "does not exist"
        };
        out.push(Violation::new(
            "declared_root_missing",
            None,
            config_display.clone(),
            format!(
                "[[project]] id = \"{}\" declares root = \"{}\", which resolves to `{}` — that \
                 path {what}. Per-project memory for `{}` still routes to \
                 `<workspace>/.codescout/projects/{}/memories`, so writes land under a project \
                 root that is not there, while discovery reports the real tree — the two views \
                 of \"what projects exist\" disagree and nothing else says so. Either the entry \
                 is stale, or this config was authored for a different workspace root and \
                 persisted here; see \
                 docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md. \
                 The file is gitignored, so no diff or review will ever show it to anyone.",
                entry.id,
                entry.root,
                crate::util::fs::RepoPath::from_path(&resolved).into_string(),
                entry.id,
                entry.id
            ),
        ));
    }

    let health = json!({
        "config": config_display,
        "declared": cfg.projects.len(),
        "missing": out.len(),
    });
    (out, health)
}

fn scan_commits_git_root(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    // `commits.git_root` carries normalized paths (since #66). A backslash
    // here is pre-migration drift, same shape as the artifact-side check
    // but without an artifact_id anchor.
    let mut stmt = conn.prepare("SELECT DISTINCT git_root FROM commits ORDER BY git_root")?;
    let roots: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    for root in &roots {
        if let Some(pos) = root.find('\\') {
            violations.push(Violation::new(
                "backslash_in_git_root",
                None,
                root.clone(),
                format!("backslash at byte position {pos}"),
            ));
        }
    }
    Ok(violations)
}

/// Computes the would-be path of a worktree-scoped row in the MAIN repo:
/// `abs_path` re-rooted from `worktree_root` onto `main_root`. `None` if
/// `abs_path` is not actually under `worktree_root` — defensive; the only
/// caller invokes this after confirming ancestry via `is_linked_worktree`.
fn main_path_for(abs_path: &Path, worktree_root: &Path, main_root: &Path) -> Option<PathBuf> {
    let rel = abs_path.strip_prefix(worktree_root).ok()?;
    Some(main_root.join(rel))
}

/// The id a linked worktree's shadow file legitimately declares in its frontmatter: its
/// MAIN twin's. `None` when `abs_path` is not inside a linked worktree, or the main root
/// cannot be resolved.
///
/// Filesystem-only, exactly like [`scan_worktree_scoped`]'s own resolution — an ancestor
/// walk for a `.git` *file*, then a re-root. That is what lets
/// [`check_frontmatter_id_matches_catalog`] use it without a connection, which it does not
/// have.
fn worktree_twin_id(abs_path: &Path) -> Option<String> {
    let worktree_root = abs_path
        .ancestors()
        .find(|a| current_project::is_linked_worktree(a))?;
    let main_root = current_project::worktree_main_root(worktree_root)?;
    let main_path = main_path_for(abs_path, worktree_root, &main_root)?;
    Some(ids::artifact_id_from_abs(&main_path))
}

/// Reads `artifact_augmentation.entry_collection` + `params` for `artifact_id`.
/// `None` if the row is unaugmented.
fn augmentation_entry_collection(
    conn: &rusqlite::Connection,
    artifact_id: &str,
) -> Result<Option<(Option<String>, String)>> {
    let row = conn
        .query_row(
            "SELECT entry_collection, params FROM artifact_augmentation WHERE artifact_id = ?1",
            rusqlite::params![artifact_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    Ok(row)
}

/// Extracts the `id` field of every object in `params_json[collection]`
/// (skipping entries with no string `id`). Best-effort: malformed JSON or a
/// missing/non-array collection yields an empty list rather than an error —
/// this feeds diagnostic detail, not the collision classification itself.
fn entry_ids(params_json: &str, collection: &str) -> Vec<String> {
    let Ok(parsed) = serde_json::from_str::<Value>(params_json) else {
        return Vec::new();
    };
    parsed
        .get(collection)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("id").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Given both sides of a `worktree_scoped_row` collision, returns the
/// overlapping entry ids IF both rows are augmented with the SAME non-null
/// `entry_collection` name. `None` means that precondition isn't met (either
/// row unaugmented, or the collection names differ/are absent) — distinct
/// from `Some(vec![])`, which means the precondition held but nothing
/// actually overlapped.
fn shared_entry_overlap(
    conn: &rusqlite::Connection,
    row_id: &str,
    other_id: &str,
) -> Result<Option<Vec<String>>> {
    let Some((row_collection, row_params)) = augmentation_entry_collection(conn, row_id)? else {
        return Ok(None);
    };
    let Some((other_collection, other_params)) = augmentation_entry_collection(conn, other_id)?
    else {
        return Ok(None);
    };
    let (Some(rc), Some(oc)) = (row_collection, other_collection) else {
        return Ok(None);
    };
    if rc != oc {
        return Ok(None);
    }
    let row_ids = entry_ids(&row_params, &rc);
    let other_ids = entry_ids(&other_params, &oc);
    Ok(Some(
        row_ids
            .into_iter()
            .filter(|i| other_ids.contains(i))
            .collect(),
    ))
}

/// Flags artifact rows whose `abs_path` lives inside a linked git worktree.
/// For each such row, computes the row's would-be path in the MAIN repo and
/// classifies whether a catalog row already exists there:
///
/// - `no_collision` — no row at the main-repo path; the worktree-scoped row
///   is merely absent from the main catalog view, not conflicting with it.
/// - `collision` — a row already exists at the main-repo path (same
///   [`ids::artifact_id_from_abs`]). If both rows are augmented with the
///   SAME `entry_collection`, the overlapping entry ids are surfaced too —
///   `fix=reseat_worktree` (a follow-up change) will need this to merge
///   safely instead of clobbering.
///
/// Every row also carries `registered`: whether an ACTIVE
/// `worktree_registration` covers the row's worktree root. Registered rows
/// are pending a `librarian(action="merge_worktree")`, not a `reseat` —
/// `fix=reseat_worktree` skips them (see [`reseat_worktree`]) and the detail
/// carries a `hint` pointing at `merge_worktree` instead.
///
/// Filesystem-only: walks each `abs_path`'s ancestor directories looking for
/// one [`current_project::is_linked_worktree`] recognizes (a `.git` *file*
/// containing a `gitdir: .../worktrees/<name>` pointer) — no `git` subprocess.
fn scan_worktree_scoped(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    // Ordered for the same reason as `scan_artifact_paths` — a report whose row
    // order shifts after a VACUUM is one nobody can diff against a prior run.
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut violations = Vec::new();
    for (id, abs_path) in &rows {
        let path = Path::new(abs_path);
        let Some(worktree_root) = path
            .ancestors()
            .find(|a| current_project::is_linked_worktree(a))
        else {
            continue;
        };
        let Some(main_root) = current_project::worktree_main_root(worktree_root) else {
            continue;
        };
        let Some(main_path) = main_path_for(path, worktree_root, &main_root) else {
            continue;
        };
        let main_id = ids::artifact_id_from_abs(&main_path);
        let main_path_str = crate::util::fs::RepoPath::from_path(&main_path).to_string();

        let exists_at_main: bool = conn
            .query_row(
                "SELECT 1 FROM artifact WHERE id = ?1",
                rusqlite::params![main_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();

        let worktree_root_str = crate::util::fs::RepoPath::from_path(worktree_root).to_string();
        let registered = worktree::covering_conn(conn, &worktree_root_str)?.is_some();

        let mut detail = json!({
            "main_path": main_path_str,
            "classification": if exists_at_main { "collision" } else { "no_collision" },
            "registered": registered,
        });

        if exists_at_main {
            detail["collision_with"] = json!(main_id);
            if let Some(overlap) = shared_entry_overlap(conn, id, &main_id)? {
                detail["id_overlap"] = json!(overlap);
            }
        }

        if registered {
            detail["hint"] = json!("pending merge — use librarian(action=\"merge_worktree\")");
        }

        violations.push(Violation::new(
            "worktree_scoped_row",
            Some(id.clone()),
            abs_path.clone(),
            detail.to_string(),
        ));
    }
    Ok(violations)
}

fn check_backslash(id: &str, abs_path: &str, check_name: &str) -> Option<Violation> {
    abs_path.find('\\').map(|pos| {
        Violation::new(
            check_name,
            Some(id.to_string()),
            abs_path,
            format!("backslash at byte position {pos}"),
        )
    })
}

fn check_ads_colon(id: &str, abs_path: &str) -> Option<Violation> {
    // Exempt the Windows drive-letter slot (`C:`) from being flagged as an
    // NTFS alternate-data-stream selector. `drive_letter_prefix_len` already
    // accounts for the `//?/` verbatim marker `fs::canonicalize` prepends on
    // Windows, so the drive-letter colon isn't mistaken for a real ADS colon
    // (false positive on every Windows-indexed row otherwise).
    let prefix_len = crate::util::fs::drive_letter_prefix_len(abs_path).unwrap_or(0);
    let tail = &abs_path[prefix_len..];
    tail.find(':').map(|pos_in_tail| {
        let absolute_pos = pos_in_tail + prefix_len;
        Violation::new(
            "ads_colon_in_abs_path",
            Some(id.to_string()),
            abs_path,
            format!("colon at byte position {absolute_pos} (outside drive prefix)"),
        )
    })
}

fn check_dotdot_segment(id: &str, abs_path: &str) -> Option<Violation> {
    if abs_path.split('/').any(|seg| seg == "..") {
        Some(Violation::new(
            "dotdot_segment_in_abs_path",
            Some(id.to_string()),
            abs_path,
            "path contains a '..' segment",
        ))
    } else {
        None
    }
}

fn check_missing_file(id: &str, abs_path: &str) -> Option<Violation> {
    if std::path::Path::new(abs_path).exists() {
        None
    } else {
        Some(Violation::new(
            "missing_file",
            Some(id.to_string()),
            abs_path,
            "file does not exist on disk",
        ))
    }
}

/// Every artifact file's frontmatter `id:` must name the row that owns it.
///
/// Catalog identity is `id == artifact_id_from_abs(abs_path)`, so a move re-keys
/// the row — and before `ec9e63d0` the file kept asserting the id it was moved
/// away from, which resolves to nothing. Measured 2026-08-16: **all 78** unique
/// `^id:` values in `docs/issues/archive/` were stale, and none could be repaired
/// through any write tool (each carries a 16-hex id, so `edit_markdown` and
/// `edit_file` refuse it, and `artifact(update)`'s `extra` cannot write `id`).
/// That is why the repair lives here.
///
/// **Two findings, not one, because the causes differ and so do the remedies.**
/// A declared value that is not a 16-hex catalog id was never minted by the catalog,
/// so no move produced it — `frontmatter_id_is_not_a_catalog_id`. Reporting those as
/// `frontmatter_id_mismatch` told the reader to look for a move that never happened,
/// and measured 2026-08-18 that was **3 of 6** live instances: two ADR/FDR template
/// placeholders and a `meetings-reranker` slug. The split is also what keeps the
/// repair safe, since `scan_frontmatter_id_mismatches` feeds `fix=repair_frontmatter_id`
/// and now sees only the stale-move rows.
///
/// Shape is tested with [`crate::util::librarian_guard::is_librarian_id`] rather than a
/// local regex, deliberately: it strips matching quotes, because a quoted id is 18
/// characters and once failed a raw length test — leaving 15 files in `docs/trackers/`
/// unguarded (BL-33). Re-deriving the predicate here would reintroduce that defect in a
/// second place.
///
/// Only a **present and differing** id is a violation. The four abstentions are
/// deliberate:
/// - **No `id:` at all** is not a false assertion, and stamping one would newly
///   subject the file to the librarian guard — `docs/trackers/skill-frictions.md`
///   would stop accepting the `edit_markdown` workflow CLAUDE.md documents.
/// - **A missing file** is [`check_missing_file`]'s finding. Reporting it here too
///   would inflate the count on precisely the rows a repair cannot help.
/// - **Unparseable frontmatter** is left alone rather than guessed at.
/// - **A linked worktree's shadow declaring its MAIN twin's id** is correct, not drift —
///   see the block comment at the abstention itself.
fn check_frontmatter_id_matches_catalog(id: &str, abs_path: &str) -> Option<Violation> {
    let content = std::fs::read_to_string(abs_path).ok()?;
    let (fm, _) = crate::librarian::frontmatter::parse(&content).ok()?;
    let declared = fm?.id?;
    if declared == id {
        return None;
    }
    if !crate::util::librarian_guard::is_librarian_id(&declared) {
        return Some(Violation::new(
            "frontmatter_id_is_not_a_catalog_id",
            Some(id.to_string()),
            abs_path,
            format!(
                "frontmatter declares id '{declared}', which is not a 16-hex catalog id, so it \
                 was never minted by the catalog and no move produced it — the row's id is \
                 '{id}'. Common causes: a template placeholder, or a hand-written slug. \
                 `fix=repair_frontmatter_id` deliberately SKIPS this row: overwriting the value \
                 would destroy a placeholder, and stamping a 16-hex id newly subjects the file \
                 to the librarian guard. Decide by hand whether the file belongs in the catalog \
                 at all."
            ),
        ));
    }
    // A linked worktree's shadow legitimately declares its MAIN twin's id. The overlay's
    // fork-on-first-write seeds a row at the worktree path while the file on disk stays a
    // checkout of the same commit, so its frontmatter still carries the main file's id.
    // Nothing moved. Reporting it as `frontmatter_id_mismatch` asserted a move that never
    // happened, and the detail text said so in as many words.
    //
    // The write half is why this is an abstention rather than a reworded finding. This
    // function feeds `scan_frontmatter_id_mismatches`, which feeds
    // `fix=repair_frontmatter_id`, whose only filter is path containment — and a worktree
    // sits UNDER its main checkout's root. `confirm=true` would therefore rewrite a tracked
    // file inside another session's live working tree; if that session then commits (the
    // entire point of a worktree), the worktree-path-derived id ships and becomes a
    // GENUINE mismatch once merged back. The repair would manufacture the defect it exists
    // to remove.
    //
    // Abstaining HERE is the whole guard, deliberately. A row that never becomes a
    // violation cannot reach the writer, so a second check inside `run_fix` would be
    // unreachable code — and an unreachable guard is one no planted violation can
    // exercise, which is the failure recorded as `prompt-surface-compaction-session-log:F-5`.
    // The two sibling fixes each carry their own registration guard because each has a
    // reachable path that needs one; this one does not.
    //
    // Only the twin id is excused. A worktree file declaring some OTHER id is ordinary
    // post-move drift and still fires. And the row is not silently dropped either way:
    // `scan_worktree_scoped` already reports it, with `collision_with` naming this very id.
    // docs/issues/archive/2026-08-19-repair-frontmatter-id-rewrites-files-in-registered-worktrees.md
    if worktree_twin_id(Path::new(abs_path)).as_deref() == Some(declared.as_str()) {
        return None;
    }

    Some(Violation::new(
        "frontmatter_id_mismatch",
        Some(id.to_string()),
        abs_path,
        format!(
            "frontmatter declares id '{declared}' but the catalog row is '{id}' — \
             a move re-keys the row and this file kept the id it was moved away from"
        ),
    ))
}

/// One params-backed ledger, with everything the four entry-drift scans need in order
/// to compare its `params` against its body.
struct ParamsBackedLedger {
    id: String,
    abs_path: String,
    collection: String,
    /// The single prefix every id in the collection shares.
    prefix: String,
    /// The entry indices `params` claim exist.
    claimed: std::collections::BTreeSet<u64>,
    /// The body as read from disk. Each scan derives its own set from it —
    /// `body_claimed_indices` for the row question, `body_defined_indices` for the
    /// citability question — so this deliberately does not pick one for them.
    body: String,
    /// Each entry's `(id, status)` exactly as `params` records it, in collection order.
    ///
    /// Read only by [`scan_params_status_drift`]; the three id-set scans ignore it. An
    /// entry with no `status` field is **absent rather than defaulted** — a ledger that
    /// does not track status must not be reported as disagreeing about one.
    statuses: Vec<(String, String)>,
    /// The `status` enum this ledger's `params_schema` declares, if any.
    ///
    /// A closed vocabulary is the whole reason a body-vs-params status comparison is
    /// possible without parsing a column whose format is each tracker's own choice —
    /// which is the objection [`scan_params_behind_body`] records when it declines this
    /// comparison. `None` disables [`scan_params_status_drift`] for this ledger, which is
    /// why that scan reaches 4 of this repo's 9 params-backed ledgers rather than all 9.
    status_enum: Option<Vec<String>>,
}

/// Every ledger whose entry rows live in `params`, in a stable order.
///
/// [`scan_snapshot_drift`], [`scan_undefined_entries`], [`scan_params_behind_body`] and
/// [`scan_params_status_drift`] ask four different questions of the same two surfaces, and
/// each needs the same preamble first: the augmentation row, the collection's ids, the
/// single prefix owning them, and the body off disk. Extracted at the point a third
/// hand-rolled copy would have existed — four checks that must agree on what a ledger *is*
/// drifting apart is the same failure mode this whole family of bugs is about.
///
/// Ordered by `abs_path` for the reason `scan_artifact_paths` is: a report whose row
/// order shifts after a VACUUM cannot be diffed against a prior run.
///
/// Skips, all silent and all deliberate: unparseable `params` (a different defect); an
/// empty collection (nothing to compare); a **mixed or malformed** prefix set, since one
/// prefix per collection is the tracker convention and guessing which one "owns" the body
/// would manufacture findings; and a file that is gone, which is `missing_file`'s finding.
/// A ledger with no `status` field on its entries, or no `status` enum in its
/// `params_schema`, is **not** skipped here — it is carried with those fields empty, so
/// the three id-set scans still see it and only the status scan opts out.
fn params_backed_ledgers(conn: &rusqlite::Connection) -> Result<Vec<ParamsBackedLedger>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.abs_path, g.entry_collection, g.params, g.params_schema \
         FROM artifact_augmentation g JOIN artifact a ON a.id = g.artifact_id \
         WHERE g.entry_collection IS NOT NULL ORDER BY a.abs_path",
    )?;
    let rows: Vec<(String, String, String, String, Option<String>)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path, collection, params_text, schema_text) in rows {
        let Ok(params) = serde_json::from_str::<serde_json::Value>(&params_text) else {
            continue;
        };
        let ids: Vec<&str> = params
            .get(&collection)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|e| e.get("id").and_then(|v| v.as_str()))
            .collect();
        if ids.is_empty() {
            continue;
        }
        let mut split = ids.iter().filter_map(|i| i.rsplit_once('-'));
        let Some((prefix, _)) = split.next() else {
            continue;
        };
        // Over ALL ids, not just the ones that split: an id carrying no `-` yields
        // `None != Some(prefix)` and bails too, which is what a malformed collection
        // should do.
        if ids
            .iter()
            .any(|i| i.rsplit_once('-').map(|(p, _)| p) != Some(prefix))
        {
            continue;
        }
        let claimed: std::collections::BTreeSet<u64> = ids
            .iter()
            .filter_map(|i| i.rsplit_once('-'))
            .filter_map(|(_, n)| n.parse::<u64>().ok())
            .collect();
        // Entries missing `status` are dropped rather than defaulted — see the field's
        // doc comment. This is why `statuses` may be shorter than `claimed`.
        let statuses: Vec<(String, String)> = params
            .get(&collection)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let eid = e.get("id")?.as_str()?;
                let st = e.get("status")?.as_str()?;
                Some((eid.to_string(), st.to_string()))
            })
            .collect();
        let status_enum = schema_text
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|s| declared_status_enum(&s));
        let Ok(body) = std::fs::read_to_string(&abs_path) else {
            continue;
        };
        out.push(ParamsBackedLedger {
            id,
            abs_path,
            collection,
            prefix: prefix.to_string(),
            claimed,
            body,
            statuses,
            status_enum,
        });
    }
    Ok(out)
}

/// The `enum` declared for a `status` property anywhere in a JSON Schema, or `None`.
///
/// Walks rather than indexing a fixed path because the entry objects sit at different
/// depths per tracker — `properties.tasks.items.properties.status` in one,
/// `properties.issues.items.properties.status` in another — and the collection key is
/// the tracker's own choice. The first `properties.status.enum` found wins; a schema
/// declaring two would be a malformed schema, not a case worth resolving here.
///
/// Deliberately keyed on the name `status` and nothing else. `tool-usage-patterns`
/// declares an enum on `verdict`, which is its disposition field but not a status, and
/// treating any enum-valued string property as a status would put every ledger's
/// severity, phase and category columns into a comparison they were never meant for.
fn declared_status_enum(schema: &serde_json::Value) -> Option<Vec<String>> {
    match schema {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(props)) = map.get("properties") {
                if let Some(serde_json::Value::Object(status)) = props.get("status") {
                    if let Some(serde_json::Value::Array(vals)) = status.get("enum") {
                        let out: Vec<String> = vals
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect();
                        if !out.is_empty() {
                            return Some(out);
                        }
                    }
                }
            }
            map.values().find_map(declared_status_enum)
        }
        serde_json::Value::Array(items) => items.iter().find_map(declared_status_enum),
        _ => None,
    }
}

/// What one sweep of the corpus knows about entry tokens.
#[derive(Default)]
struct CorpusTokens {
    /// Tokens cited anywhere in the catalog.
    cited: std::collections::BTreeSet<String>,
    /// Token → the artifacts carrying a `## <ID> — <title>` heading for it, in `abs_path`
    /// order. A token absent from this map is defined **nowhere**, which is the only state
    /// that makes a citation of it genuinely dangle.
    definers: std::collections::BTreeMap<String, Vec<String>>,
}

/// Every entry token cited anywhere in the catalog, and every artifact that DEFINES one,
/// computed fresh from the files in a single pass.
///
/// **Deliberately not `entry_cite`.** That table has two writers with different freshness
/// guarantees. `append_entry(cites=…)` writes an `origin="write"` row permanently at
/// entry-creation time — the primary key excludes `origin`, so a later scan can never
/// overwrite it. `link_scan(write=true)` materializes `origin="scan"` rows instead, pruned
/// and re-derived only for the slugs that pass actually scanned. A check reading the table
/// would therefore report against a mix of always-current rows and whatever the last scan
/// happened to leave behind for whichever artifacts it covered — a stale-substrate diagnostic
/// of exactly the kind `doctor` exists to catch. `link_scan::extract` is a pure function over
/// a body, so the citations are recomputed here with nothing to go stale. It is the same door
/// [`crate::librarian::catalog::augmentation::body_defined_indices`] uses for the
/// *definition* half, which is what keeps the two halves of this check agreeing about what
/// a citation and a definition are.
///
/// **The definitions half is collected here rather than per-ledger, and that closes a
/// measured false claim.** `body_defined_indices` answers "does THIS body define the token",
/// which is right for the ledger-scoped *count* and wrong for "does this citation resolve":
/// `link_scan` binds a token to its defining heading wherever that heading lives, and
/// `tracker-conventions` § *Compaction and archival* prescribes moving definitions into an
/// archive companion — so a definer outside the ledger is a supported end state, not an
/// anomaly. Measured 2026-08-31: `doctor` named 31 `PV-N` tokens as resolving to nothing
/// while the companion defining them carried live incoming entry links
/// (`docs/issues/archive/2026-08-31-entry-without-definition-claims-broken-refs-that-resolve.md`).
/// The sweep already reads every file, so this half costs no extra I/O — only the half of a
/// `DocExtract` the loop was already building and discarding.
///
/// **Both citation shapes count.** A bare `PV-12` extracts as `EntryToken`; a qualified
/// `provenance-subsystem:PV-12` extracts as `CrossRepoToken`, because a file-stem qualifier
/// and a repo qualifier are one syntactic shape that extraction cannot tell apart and does
/// not try to. Taking the token half of the qualified form is what stops a qualified
/// citation reading as no citation at all.
///
/// **Self-citations count too**, and that is a decision rather than an oversight.
/// `link_scan` reports them separately because it creates no self-edges; here the question
/// is whether a reader following the token lands anywhere, and a reference that resolves to
/// nothing is broken whether or not it came from the same file. Measured 2026-08-19 on
/// `provenance-subsystem.md`: `PV-12` is cited 8 times in the ledger's own prose, once
/// inside a section heading, and defines nothing. Every real break found there was a
/// self-citation, so excluding them would have reported the ledger as clean.
///
/// **Known hazard, measured and accepted for now.** `extract()` cannot distinguish a real
/// citation from a documentation example of citation syntax — the same root cause
/// `link_scan`'s own report was fixed for in
/// `docs/issues/archive/2026-08-19-doc-examples-of-citation-syntax-counted-as-real-citations.md`.
/// That fix added a per-source breakdown to `link_scan`'s report only; this function feeds
/// `entry_without_definition` from the same extractor with no equivalent mitigation —
/// `docs/issues/archive/2026-08-21-doctor-cited-uncited-partition-inherits-doc-example-defect.md`.
/// Measured 2026-08-25 across the full machine-wide catalog: zero live findings are actually
/// affected — the one current `entry_without_definition` violation
/// (`provenance-subsystem.md`, 33 cited / 9 uncited) was independently hand-verified as
/// accurate, not example-inflated. Revisit if a future violation's cited count looks
/// suspicious against its source's actual prose, rather than pre-emptively widening this
/// function's return type for a problem with no measured victim yet.
///
/// Extraction is deliberately dumb — `UTF-8` and `SHA-256` arrive as entry tokens — which
/// costs nothing here, because the result is only ever intersected against ids a ledger
/// actually claims.
///
/// Unreadable files are skipped, as `missing_file` is the finding for those.
fn corpus_cited_tokens(conn: &rusqlite::Connection) -> Result<CorpusTokens> {
    use crate::librarian::tools::link_scan::extract::{extract, CitationKind};

    let mut stmt = conn.prepare("SELECT abs_path FROM artifact ORDER BY abs_path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = CorpusTokens::default();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let doc = extract(&text);
        for c in doc.citations {
            match c.kind {
                CitationKind::EntryToken => {
                    out.cited.insert(c.raw);
                }
                CitationKind::CrossRepoToken => {
                    if let Some((_, token)) = c.raw.rsplit_once(':') {
                        out.cited.insert(token.to_string());
                    }
                }
                _ => {}
            }
        }
        for d in doc.definitions {
            out.definers.entry(d.token).or_default().push(path.clone());
        }
    }
    Ok(out)
}

/// Whether this row counts as "archived" for exposure purposes.
///
/// **Not status alone.** Measured 2026-08-20: of 465 citer rows a status-only filter
/// missed, statuses were `fixed 352 · closed 59 · draft 17 · unknown 12 · wontfix 11 ·
/// mitigated 10 · active 3 · done 1` — NONE said `archived`. This repo archives a file
/// by MOVING it into `docs/issues/archive/` or `docs/trackers/archive/` (per
/// `get_guide("tracker-conventions")`) while `status:` keeps meaning the bug/tracker
/// OUTCOME, not archival state. So archived-ness is encoded by location for roughly
/// 70% of this corpus's archived rows, and a status-only filter reads all of them as
/// live.
///
/// **The status half is deliberately kept in parity with `link_scan/resolve.rs:61`'s
/// exact vocabulary (`"archived" | "superseded"`), not `HIDDEN_STATUSES`** (which adds
/// `retired` — zero live rows carry it today, but that is not a guarantee). Do not
/// "fix" this into `HIDDEN_STATUSES`: it would silently change the population this
/// function and `resolve()` agree on.
fn row_is_archived(abs_path: &str, status: &str) -> bool {
    matches!(status, "archived" | "superseded") || abs_path.contains("/archive/")
}

/// `(defining file, token)` → how many OTHER files cite that entry.
///
/// **Keyed by the DEFINER, not by the bare token, because per-work-stream namespaces
/// reuse low numbers.** `F-1` is defined in every live session log. Folding all of their
/// citations under one `F-1` key lets one ledger's traffic arm another's entries — and
/// the previous defence against that, dropping any multi-definer token outright, traded
/// a false-positive for a silent exemption of the entire `F`/`W` population. Measured
/// 2026-08-21, before this change: all 32 rows of `entry_cited_from_outside_but_undeclared`
/// named some other prefix and **not one** named an `F` or `W` entry, while `entry_cite`
/// held 442 resolved entry-grain edges into 252 distinct `F`/`W` Statements. The fix is the
/// one this function's own comment named and declined — resolve the qualified form against
/// its specific definer — and what unblocked it was Layer 3b building the `by_stem`
/// vocabulary this now mirrors.
///
/// **Resolution mirrors [`link_scan::resolve::resolve`](crate::librarian::tools::link_scan::resolve::resolve)
/// arm for arm**, and must keep doing so: one materializes the edge, this one prices it,
/// and a disagreement means an entry is gated on exposure it does not have (or vice
/// versa).
///
/// | citation | counts toward |
/// |---|---|
/// | a token this very file defines | nothing — SelfCite; the local definition wins |
/// | bare, exactly one definer | that definer, **active or archived** |
/// | bare, several definers, exactly one active | the active one |
/// | bare, several definers, zero or many active | nothing — Ambiguous |
/// | `<stem>:TOKEN`, stem names one file that defines it | that file — resolved precisely |
/// | `<qualifier>:TOKEN`, anything else | falls back to the bare rows above, on TOKEN |
///
/// A qualified citation names its target explicitly, so the active-vs-archived tie-break
/// never applies to it — exactly as `resolve`'s `[only]` arm does not consult `active`.
///
/// **The fallback row is where this function deliberately DIVERGES from `resolve`, and it
/// is not an oversight.** `resolve` answers "can this become an edge", so an unknown
/// qualifier is `CrossRepo` and stops there — edges cannot span workspaces. This function
/// answers "how exposed is this entry", and cross-repo exposure is real: [`call`]'s own
/// comment states that narrowing the metric to the active project "would understate real
/// cross-repo exposure and manufacture false negatives", which is why only the reported
/// worklist is scoped while the metric stays global. So an unknown qualifier, a colliding
/// stem, and a qualifier naming a file that lacks the entry all fall back to resolving the
/// token half — precisely the behaviour that predates this rewrite. The ONLY thing that
/// changed is that a qualifier uniquely naming a local definer now attributes there
/// instead of pooling into the bare token.
///
/// **This is a file-level count, not a citation-occurrence count, and that follows
/// from `extract()`'s own dedup rather than from a choice made here.**
/// `push_citation` keys on `(CitationKind, raw)` per document, so a file that mentions the
/// same bare token five times still contributes exactly one `Citation` to
/// `extract(text).citations`. Measured live while building the previous version: a fixture
/// citing `R-1` twice in prose produced ONE citation, not two — the brief it was built from
/// assumed occurrence-counting and its own test was wrong about the number for that reason.
/// What this computes is "how many other files reach this entry at least once", which is
/// the better exposure signal anyway: it stops one chatty file inflating an entry's reach.
/// Note the corollary of keying on `raw`: a file citing both `R-3` and
/// `reconnaissance-patterns:R-3` contributes two, because those are two distinct raws.
/// Unchanged by this rewrite, and called out so it is not mistaken for a regression.
///
/// **Exposure is a target-side count.** Which entry a citation came FROM requires
/// attribution and slugs; which entry it points AT does not — that is what let this
/// check, and the ones built on it, ship before the entry graph existed.
///
/// **Same-file citations are excluded, and that is load-bearing.** Measured
/// 2026-08-20: 407 of 1427 ledger citations (28.5%) sit above the first definition —
/// hand-maintained `## Index` rows. Counting them would let an entry's own index row
/// inflate its own exposure, which is a self-reference wearing exposure's clothes.
/// `link_scan` already classifies these `SelfCite`.
///
/// **Citations from archived artifacts do not count toward exposure** — see
/// [`row_is_archived`]. The DEFINING side is untouched by that filter: an archived file
/// still defines its token, and a unique archived definer still receives exposure.
/// Ruling 2026-08-20.
///
/// **Computed fresh from the files**, deliberately, and this is why the entry graph is
/// NOT read here even though it now holds the same resolutions: a check that reads a
/// materialized table reports on whatever the last scan left behind. `entry_cite` is the
/// right source for a *query*; it is the wrong source for a *gate*.
fn entry_indegree(
    conn: &rusqlite::Connection,
) -> Result<std::collections::BTreeMap<(String, String), usize>> {
    use crate::librarian::tools::link_scan::extract::{entry_sections, extract, CitationKind};

    let mut stmt = conn.prepare("SELECT abs_path, status FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Which paths DEFINE which tokens, gathered up front so a same-file citation can
    // be excluded before it is counted rather than filtered back out after the fact.
    // Built from EVERY readable row regardless of archived-ness — that filter touches only
    // the CITING side; an archived definer still defines.
    let mut definer: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    // Paths that are NOT archived, for the bare-token active-vs-archived tie-break.
    let mut active_paths: std::collections::BTreeSet<String> = Default::default();
    // Qualifier vocabulary for `<stem>:<TOKEN>`, mirroring `Corpus.by_stem`'s build in
    // `link_scan::mod` — keyed on file stem, PUSHED rather than inserted, because stems
    // collide across directories and a collision must be reported rather than resolved.
    //
    // Built from every row INCLUDING ones that fail to read, unlike `definer` above. A
    // row present in the catalog is a file a qualifier could have meant; omitting it
    // would make a colliding stem look unique and resolve a citation to the wrong file.
    // Suppressing is the safe direction, so unreadable rows still occupy their stem.
    let mut by_stem: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    // (path, text, citable) — `citable` is false for archived rows, so a citation FROM
    // one of them is gathered here but never counted below.
    let mut bodies: Vec<(String, String, bool)> = Vec::new();
    for (path, status) in rows {
        if let Some(stem) = Path::new(&path).file_stem().and_then(|s| s.to_str()) {
            by_stem
                .entry(stem.to_string())
                .or_default()
                .push(path.clone());
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let citable = !row_is_archived(&path, &status);
        if citable {
            active_paths.insert(path.clone());
        }
        for s in entry_sections(&text) {
            definer.entry(s.id).or_default().insert(path.clone());
        }
        bodies.push((path, text, citable));
    }

    // Bare-token resolution — `resolve`'s `EntryToken` arm, and the fallback every
    // unresolvable qualified form lands on. `None` means "contributes no exposure":
    // undefined, self-cited, or ambiguous.
    let resolve_bare = |token: &str, citer: &String| -> Option<String> {
        // Zero definers is `scan_undefined_entries`' question, not this one's — and
        // there would be no definer to key on regardless.
        let defs = definer.get(token)?;
        if defs.contains(citer) {
            return None; // the local definition wins — SelfCite, never exposure
        }
        let mut it = defs.iter();
        match (it.next(), it.next()) {
            // Exactly one definer resolves even when archived — archived is not
            // nonexistent (`get_guide("tracker-conventions")`).
            (Some(only), None) => Some(only.clone()),
            (Some(_), Some(_)) => {
                let mut act = defs.iter().filter(|p| active_paths.contains(*p));
                match (act.next(), act.next()) {
                    (Some(one), None) => Some(one.clone()),
                    // Zero or many active definers: Ambiguous. `link_scan` reports it;
                    // nothing here guesses.
                    _ => None,
                }
            }
            _ => None,
        }
    };

    let mut deg: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for (path, text, citable) in &bodies {
        if !citable {
            continue; // archived citers do not add exposure
        }
        for c in extract(text).citations {
            let (dst, token) = match c.kind {
                CitationKind::EntryToken => match resolve_bare(&c.raw, path) {
                    Some(dst) => (dst, c.raw.clone()),
                    None => continue,
                },
                CitationKind::CrossRepoToken => {
                    let Some((qualifier, token)) = c.raw.split_once(':') else {
                        continue;
                    };
                    // A qualifier naming exactly one local file that DEFINES the token is
                    // a within-repo qualified citation, and resolves precisely. This one
                    // row is the whole change; every other shape falls through.
                    let precise = match by_stem.get(qualifier).map(Vec::as_slice) {
                        Some([only]) if definer.get(token).is_some_and(|d| d.contains(only)) => {
                            Some(only.clone())
                        }
                        _ => None,
                    };
                    match precise {
                        // Qualified citation of your own file — SelfCite.
                        Some(only) if only == *path => continue,
                        Some(only) => (only, token.to_string()),
                        // Unknown qualifier (a genuine cross-repo reference), colliding
                        // stem, or a named file that lacks the entry. All fall back to the
                        // token half, which is what this function did before the rewrite —
                        // see the doc comment on why dropping them would manufacture false
                        // negatives the call site explicitly forbids.
                        None => match resolve_bare(token, path) {
                            Some(dst) => (dst, token.to_string()),
                            None => continue,
                        },
                    }
                }
                _ => continue,
            };
            *deg.entry((dst, token)).or_insert(0) += 1;
        }
    }

    Ok(deg)
}

/// Cross-file citations below which a Statement is not worth anyone's attention.
///
/// Shared, on purpose, by every check in this family (Tasks 5-7): two checks producing
/// work independently is how a backlog becomes the steady state — as of June 2025 more
/// than 604,000 English Wikipedia pages carried at least one `{{citation needed}}`.
/// Also a guess; re-tune from the first month's output.
const EXPOSURE_THRESHOLD: usize = 5;

/// A declared `conditional` whose named event may already have fired.
///
/// **Reports a worklist, never a verdict.** Selection is syntactic and cheap — a
/// section's own `**Valid:**` line, above the exposure gate; whether the condition
/// actually fired is the reader's judgement, and always will be. The `detail` carries
/// the condition text so it can be adjudicated without reopening the file.
///
/// **Gated on `EXPOSURE_THRESHOLD`, not run over every conditional entry.** A
/// conditional nobody is citing is not worth anyone's attention. `indegree` is computed
/// once per `doctor` run by [`entry_indegree`] and shared with the checks that follow,
/// so the population is priced consistently rather than recomputed per check.
///
/// **A malformed `**Valid:**` is swallowed here, not reported here.** That is
/// [`scan_validity_unparseable`]'s business; reporting it here too would duplicate the
/// finding, and staying silent here is what keeps the two checks from ever disagreeing
/// about the same defect.
///
/// **Truncates each section with [`declared_section_text`](crate::librarian::statements::declared_section_text)** before parsing, so a
/// parent entry with no declaration of its own never inherits a nested child's.
///
/// Read-only; there is no `fix=`. Discharging a conditional means judging whether the
/// named event happened, which only a reader can do.
fn scan_conditional_past_due(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<(String, String), usize>,
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    use crate::librarian::statements::{declared_section_text, parse_validity, Validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path, status FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Scoped to the active project's git root, same precedent as
    // `scan_archived_fix_sha_unresolvable` / `scan_terminal_status_without_fix_anchor`.
    // No active project means no scoping (report everything) — a config-only caller
    // must not silently get an empty worklist, the same degradation shape `call()`
    // already uses for `detect_move_candidates`.
    let cp = ctx.current_project.as_deref();
    let mut out = Vec::new();
    let mut scoped_out: std::collections::BTreeMap<String, usize> = Default::default();
    for (aid, path, status) in rows {
        // An archived row is excluded from the REPORTED population, mirroring what
        // `entry_indegree` already does on the citing side (Ruling 15/16) — otherwise
        // an archived definer is reported here carrying the exposure its active twin
        // earned, which is the double-attribution Ruling 14 exists to prevent,
        // surviving one layer down. MF-2, 2026-08-20.
        if row_is_archived(&path, &status) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let sections = entry_sections(&text);
        for s in &sections {
            // Keyed by (defining file, token): `F-1` is defined in every session log,
            // so a bare-token lookup would price this entry on other ledgers' traffic.
            let exposure = indegree
                .get(&(path.clone(), s.id.clone()))
                .copied()
                .unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            let declared = declared_section_text(s, &sections);
            // A malformed declaration is `validity_unparseable`'s business, not this
            // check's — swallowing it here would hide it, reporting it here would
            // duplicate it.
            let Ok(Some(Validity::Conditional { condition })) = parse_validity(&declared) else {
                continue;
            };
            // Exposure stays global — `entry_indegree` is not scoped, and must not
            // be, per Ruling 17 (MF-1's fix on the reported population, not the
            // metric). Only the emitted worklist is limited to the active project,
            // so a developer standing in one repo is not handed another repo's
            // finding to act on. A row this drops never becomes a Violation, so
            // `summary.total` cannot count it — announced instead via
            // `catalog_health.entry_validity_scoped_by_project`.
            if let Some(cp) = cp {
                if super::containing_root(std::slice::from_ref(&cp.git_root), Path::new(&path))
                    .is_none()
                {
                    *scoped_out.entry(outside_roots_group(&path)).or_insert(0) += 1;
                    continue;
                }
            }
            out.push(Violation::new(
                "entry_conditional_past_due",
                Some(aid.clone()),
                path.clone(),
                format!(
                    "{} (exposure {exposure}) is conditional on: {condition} — check \
                     whether that has happened; this is a worklist, not a verdict",
                    s.id
                ),
            ));
        }
    }
    Ok((out, scoped_out))
}

/// Horizon, in days, past a `dated` Statement's declared date before it counts as stale.
///
/// **A guess, not a measurement — 30 days, roughly a work-month.** There is no dataset
/// yet of how long a `dated` Statement actually stays true; re-tune this once
/// `entry_dated_stale` has produced a month of worklist resolutions to look at (how many
/// flagged entries turned out to actually be stale vs still true at the horizon).
///
/// **Deliberately not the deleted `FRESHNESS_HORIZON_DEFAULT`.** That constant measured
/// commit distance (`topo_distance_from_head`) — a different axis from calendar days —
/// and its own doc comment recorded that every call site passed `None`, so it was never
/// exercised before being removed for having no consumer. Reviving it here under a new
/// name would just recreate an unexercised default; this is a fresh guess on a fresh
/// axis, and it says so.
const VALIDITY_HORIZON_DAYS: i64 = 30;

/// Days since the Unix epoch (1970-01-01) for a `YYYY-MM-DD` string. `None` if `iso` is
/// not a valid calendar date in that exact form (covers both a malformed shape and a
/// shape-valid-but-impossible date like `2020-13-45` — `iso_re()` in `statements.rs`
/// only checks digit shape, not calendar validity).
///
/// Uses `chrono`, which is already a workspace dependency (`chrono::Utc::now()` appears
/// earlier in this same file's `call()`, and at `tools/update.rs:405`) — not a
/// hand-rolled civil-date algorithm. A third hand-rolled date implementation next to a
/// working `chrono` dependency is not worth the maintenance surface.
fn iso_to_epoch_days(iso: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d").ok()?;
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    Some((d - epoch).num_days())
}

/// Declared `dated` Statements past [`VALIDITY_HORIZON_DAYS`], **ranked by exposure
/// descending**.
///
/// **The ranking is load-bearing, not a nicety.** A decayed fact nothing cites costs
/// nothing; one cited from a promoted skill costs a lot. An unranked list of every dated
/// entry past the horizon is thousands of rows and will be ignored — the same outcome as
/// not shipping the check, at higher cost.
///
/// **The sort key is TOTAL: `(Reverse(exposure), path, id)`.** No two rows can compare
/// equal (an entry id is unique within its own path), so the output is deterministic by
/// construction rather than by leaning on an implicit stable-sort guarantee. See the
/// comment at the sort call for the measured reasoning: several smaller/less-adversarial
/// tie shapes failed to expose a stable-vs-unstable difference before a 33-entry
/// alternating-exposure fixture did.
///
/// **Declared `dated` only — parsed with `parse_validity`, never `resolve_validity`.**
/// `resolve_validity`'s default-is-decay behavior treats an UNDECLARED entry as `dated
/// <fallback>`, which is exactly the guessed age this check must not produce. An entry
/// with no declaration is a different, not-yet-shipped check's business (Task 7, which
/// reports it as undeclared rather than guessing its age).
///
/// **Gated on `EXPOSURE_THRESHOLD`, not run over every dated entry.** Same `indegree`
/// map computed once per `doctor` run by [`entry_indegree`] and shared with
/// [`scan_conditional_past_due`], so the population is priced consistently.
///
/// **A malformed `**Valid:**` is swallowed here, not reported here** — same split as
/// [`scan_conditional_past_due`]: that is [`scan_validity_unparseable`]'s business;
/// reporting it here too would duplicate the finding.
///
/// **Truncates each section with [`declared_section_text`](crate::librarian::statements::declared_section_text)** before parsing, so a parent
/// entry with no declaration of its own never inherits a nested child's.
///
/// Takes `today_epoch_days` rather than computing `chrono::Utc::now()` itself, so the
/// horizon comparison and the ranking are deterministic under test.
///
/// Read-only; there is no `fix=`. Reports a worklist, never a verdict — re-running the
/// underlying measurement and judging whether the date is still true is the reader's.
fn scan_dated_stale(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<(String, String), usize>,
    today_epoch_days: i64,
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    use crate::librarian::statements::{declared_section_text, parse_validity, Validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path, status FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Named to satisfy `clippy::type_complexity` — the tuple itself is the point: a
    // TOTAL sort key so no two rows can ever compare equal.
    type DatedStaleSortKey = (std::cmp::Reverse<usize>, String, String);
    // Scoped to the active project's git root — see `scan_conditional_past_due` for
    // the full reasoning (same precedent, same "no active project means no
    // scoping" degradation).
    let cp = ctx.current_project.as_deref();
    let mut scored: Vec<(DatedStaleSortKey, Violation)> = Vec::new();
    let mut scoped_out: std::collections::BTreeMap<String, usize> = Default::default();
    for (aid, path, status) in rows {
        // An archived row is excluded from the REPORTED population — same guard as
        // `scan_conditional_past_due` and `scan_cited_but_undeclared`; MF-2, 2026-08-20.
        if row_is_archived(&path, &status) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let sections = entry_sections(&text);
        for s in &sections {
            // Keyed by (defining file, token): `F-1` is defined in every session log,
            // so a bare-token lookup would price this entry on other ledgers' traffic.
            let exposure = indegree
                .get(&(path.clone(), s.id.clone()))
                .copied()
                .unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            let declared = declared_section_text(s, &sections);
            // A malformed declaration, or a declared class other than `dated`
            // (`invariant`/`conditional`), is not this check's finding — swallow it.
            let Ok(Some(Validity::Dated(iso))) = parse_validity(&declared) else {
                continue;
            };
            let Some(days) = iso_to_epoch_days(&iso) else {
                continue;
            };
            let age = today_epoch_days - days;
            if age < VALIDITY_HORIZON_DAYS {
                continue;
            }
            // Exposure stays global; only the emitted worklist is scoped. See
            // `scan_conditional_past_due` for the full reasoning.
            if let Some(cp) = cp {
                if super::containing_root(std::slice::from_ref(&cp.git_root), Path::new(&path))
                    .is_none()
                {
                    *scoped_out.entry(outside_roots_group(&path)).or_insert(0) += 1;
                    continue;
                }
            }
            scored.push((
                (std::cmp::Reverse(exposure), path.clone(), s.id.clone()),
                Violation::new(
                    "entry_dated_stale",
                    Some(aid.clone()),
                    path.clone(),
                    format!(
                        "{} dated {iso} ({age}d old, exposure {exposure}) — re-run the \
                             measurement and record the new figure; this is a worklist, not \
                             a verdict",
                        s.id
                    ),
                ),
            ));
        }
    }
    // The sort key is TOTAL — (Reverse(exposure), path, id) — so no two rows can ever
    // compare equal (an entry id is unique within its own path). Determinism is then a
    // property of the key, not of the sort algorithm's stability: swapping `sort_by`
    // for `sort_unstable_by` here cannot change the output, because there is nothing
    // left for either one to disagree about a tie on. Measured 2026-08-20: a 2-element
    // tie, a 40/300-entry homogeneous-key block, and a 62-entry permutation with one
    // embedded tied pair all failed to expose `sort_unstable_by_key` reordering ties; a
    // 33-entry ledger alternating two exposure values did. Rather than keep hunting for
    // the right fixture shape, the dependency on stability is removed instead.
    scored.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((scored.into_iter().map(|(_, v)| v).collect(), scoped_out))
}

/// A Statement other files depend on that declares no decay class at all.
///
/// The inverse of the checks above: they read a declaration, this one reports its
/// absence where absence costs something. These are the de-facto promotions — a
/// Statement genuinely promoted and declared nowhere reads identically here to one
/// nobody got around to declaring; this check cannot and does not distinguish them.
///
/// **It reports "load-bearing and undeclared", never "promoted".** Measured
/// 2026-08-20: a promotion, an eval-fixture list, and a kin reference are
/// syntactically identical — `grep -c '<id>'` counts any mention, and using it as a
/// promotion predicate mislabelled three of five entries in commit `9a982ed5`. That
/// direction stays human; the `detail` string must not contain the word "promoted".
///
/// **Truncates each section with [`declared_section_text`](crate::librarian::statements::declared_section_text)**, never `s.text`, before
/// parsing — same rule as [`scan_conditional_past_due`] and [`scan_dated_stale`]: a
/// parent with no declaration of its own must not inherit a nested child's. For this
/// check specifically, skipping the truncation would fail in the UNSAFE direction: a
/// parent that declares nothing would read the child's declaration as its own and
/// silently stop being reported, even though the parent itself is still undeclared.
///
/// **A malformed `**Valid:**` is swallowed here, not reported here.** Only `Ok(None)`
/// (declares no class at all) is this check's business — a malformed declaration is
/// [`scan_validity_unparseable`]'s finding, and a well-formed declaration of any class
/// means one of the checks above already covers this entry.
///
/// **Gated on `EXPOSURE_THRESHOLD`, using the same shared `indegree`** as
/// [`scan_conditional_past_due`] and [`scan_dated_stale`] — one exposure computation,
/// three consumers, so the population is priced consistently.
///
/// Read-only; there is no `fix=`. Reports a worklist, never a verdict.
fn scan_cited_but_undeclared(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<(String, String), usize>,
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    use crate::librarian::statements::{declared_section_text, parse_validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path, status FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Scoped to the active project's git root — see `scan_conditional_past_due` for
    // the full reasoning. This is the check MF-1 measured burying every other
    // finding (168 of 215 shown rows, 74% of them about repos other than the
    // active one); the scoping lands here in the same shared shape as the other
    // two checks rather than as a special case.
    let cp = ctx.current_project.as_deref();
    let mut out = Vec::new();
    let mut scoped_out: std::collections::BTreeMap<String, usize> = Default::default();
    for (aid, path, status) in rows {
        // An archived row is excluded from the REPORTED population — same guard as
        // `scan_conditional_past_due` and `scan_dated_stale`; MF-2, 2026-08-20.
        if row_is_archived(&path, &status) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let sections = entry_sections(&text);
        for s in &sections {
            // Keyed by (defining file, token): `F-1` is defined in every session log,
            // so a bare-token lookup would price this entry on other ledgers' traffic.
            let exposure = indegree
                .get(&(path.clone(), s.id.clone()))
                .copied()
                .unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            let declared = declared_section_text(s, &sections);
            // Only a section that declares NOTHING is this check's business. A
            // malformed declaration (`Err(_)`) is swallowed here too — not this
            // check's finding — and any well-formed declared class means one of the
            // checks above already has this entry covered.
            if !matches!(parse_validity(&declared), Ok(None)) {
                continue;
            }
            // Exposure stays global; only the emitted worklist is scoped. See
            // `scan_conditional_past_due` for the full reasoning.
            if let Some(cp) = cp {
                if super::containing_root(std::slice::from_ref(&cp.git_root), Path::new(&path))
                    .is_none()
                {
                    *scoped_out.entry(outside_roots_group(&path)).or_insert(0) += 1;
                    continue;
                }
            }
            out.push(Violation::new(
                "entry_cited_from_outside_but_undeclared",
                Some(aid.clone()),
                path.clone(),
                format!(
                    "{} is cited {exposure}× from other files and declares no \
                         **Valid:** class — add one of: {}; this is a worklist, not a verdict",
                    s.id,
                    crate::librarian::statements::FORMS
                ),
            ));
        }
    }
    Ok((out, scoped_out))
}

/// A `**Valid:**` line that fails to parse — shape-invalid, calendar-invalid
/// (`dated 2026-02-30`), or an unknown class.
///
/// The fourth partition of the validity-decay family, and the one that closes it.
/// [`scan_conditional_past_due`], [`scan_dated_stale`], and [`scan_cited_but_undeclared`]
/// each deliberately swallow `parse_validity`'s `Err` and defer to this check by name in
/// their own doc comments. Before this check shipped, a malformed declaration was
/// invisible to the whole family: the author tried to declare and failed, and their
/// Statement read as healthy to every check that partitions on class. This closes
/// `docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md`,
/// whose filed instance (`dated 2026-02-30`) is one shape of this — the other is any
/// value `parse_validity` refuses outright, which the filed bug did not cover.
///
/// **Ungated on exposure, unlike its three siblings.** The exposure gate exists to
/// prioritise DECAY work — is a Statement's claim still true — which presupposes the
/// declaration parsed in the first place. An unparseable declaration is a different
/// failure, a malformed record rather than a stale one, and it costs the author
/// something the moment it is written, regardless of who cites it yet. The population
/// is bounded by how many sections declare `**Valid:**` at all, not by the full corpus —
/// measured 2026-08-20, 1 of 2869 entry sections declares a `**Valid:**` line, so
/// ungated is safe today. If that population grows large enough that this worklist
/// starts burying others the way `entry_cited_from_outside_but_undeclared` buried
/// everything else pre-MF-1, gate it on `EXPOSURE_THRESHOLD` like its siblings.
///
/// **Truncates each section with [`declared_section_text`](crate::librarian::statements::declared_section_text)**, never `s.text` — same
/// rule as the other three: a parent with no declaration of its own must not inherit a
/// nested child's malformed one.
///
/// Read-only; there is no `fix=`. Reports a worklist, never a verdict — the fix is an
/// author correcting the line, not this check guessing what was meant.
fn scan_validity_unparseable(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    use crate::librarian::statements::{declared_section_text, parse_validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path, status FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    // Scoped to the active project's git root — same precedent and same "no active
    // project means no scoping" degradation as `scan_conditional_past_due`.
    let cp = ctx.current_project.as_deref();
    let mut out = Vec::new();
    let mut scoped_out: std::collections::BTreeMap<String, usize> = Default::default();
    for (aid, path, status) in rows {
        // An archived row is excluded from the REPORTED population — same guard as
        // the other three validity checks; MF-2, 2026-08-20.
        if row_is_archived(&path, &status) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let sections = entry_sections(&text);
        for s in &sections {
            let declared = declared_section_text(s, &sections);
            // This check's whole business is the Err arm the other three swallow.
            let Err(err) = parse_validity(&declared) else {
                continue;
            };
            if let Some(cp) = cp {
                if super::containing_root(std::slice::from_ref(&cp.git_root), Path::new(&path))
                    .is_none()
                {
                    *scoped_out.entry(outside_roots_group(&path)).or_insert(0) += 1;
                    continue;
                }
            }
            out.push(Violation::new(
                "validity_unparseable",
                Some(aid.clone()),
                path.clone(),
                format!(
                    "{} declares a malformed **Valid:** line: {}.{} — this is a worklist, \
                         not a verdict",
                    s.id,
                    err.message,
                    err.hint
                        .as_deref()
                        .map(|h| format!(" {h}"))
                        .unwrap_or_default()
                ),
            ));
        }
    }
    Ok((out, scoped_out))
}

/// `snapshot_drift`: an augmented tracker's `params` hold entry ids that its
/// markdown body line-anchors nowhere.
///
/// Entry rows live in `params`, and params live in the catalog — which is
/// machine-local and git-ignored. A tracker that also keeps a rendered snapshot
/// in its body is the only way those rows reach git, and nothing re-renders it:
/// `append_entry`/`update_entry` write params and return success while the
/// committed file stays byte-identical, so `git status` is clean and the row
/// exists on exactly one machine.
/// docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
///
/// **The gate is the body, not the augmentation config.** A tracker is treated
/// as snapshot-keeping iff its body already line-anchors at least one
/// `PREFIX-N`. That is self-configuring and precise: a prose-only tracker
/// (rows deliberately only in params, `render_template` projecting them into
/// `librarian(context)` instead) anchors none and is silent here. Gating on
/// `render_template != NULL` would be the wrong test in both directions — 26 of
/// 28 augmented trackers declare one, and its documented purpose is precisely
/// that the body does NOT carry the rows.
///
/// For the opposite direction — a body that has run AHEAD of `params` — see
/// [`scan_params_behind_body`]. The two consume the same [`ParamsBackedLedger`] and
/// differ only in which way round they subtract, so keep their remedies straight:
/// this one's is to re-render the body, and that one's is emphatically not.
///
/// Reports only; there is no `fix=`. Re-rendering a body is a content decision
/// (which section, what column order, how much of the row to include) that
/// belongs to whoever maintains the tracker.
fn scan_snapshot_drift(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut out = Vec::new();
    for ledger in params_backed_ledgers(conn)? {
        // ROW anchors only, in both the gate and the subtraction below. A
        // heading is not a snapshot row, and counting it as one broke this check
        // in both directions at once: a headings-only body was told its
        // non-existent table lagged, and a body whose headings covered every id
        // masked a table that genuinely lagged — `claimed.difference(in_body)`
        // came out empty and the check `continue`d on a real finding.
        // See `body_snapshot_row_indices`.
        let in_body = crate::librarian::catalog::augmentation::body_snapshot_row_indices(
            &ledger.body,
            &ledger.prefix,
        );
        // Not merely "anchors something" — anchors a MAJORITY. A tracker whose
        // rows are canonical in params can still line-anchor a few ids in
        // unrelated tables and narrative headings; reporting those as a lagging
        // snapshot nags a deliberate design decision. See `body_keeps_snapshot`.
        if !crate::librarian::catalog::augmentation::body_keeps_snapshot(&ledger.claimed, &in_body)
        {
            continue;
        }
        let missing: Vec<String> = ledger
            .claimed
            .difference(&in_body)
            .map(|n| format!("{}-{}", ledger.prefix, n))
            .collect();
        if missing.is_empty() {
            continue;
        }
        // Name a bounded sample; the count carries the magnitude. An unbounded
        // list of 54 ids buries every other finding in the report, which is the
        // failure `abs_path_outside_managed_roots` already had to cap for.
        const SAMPLE: usize = 8;
        let shown = missing.iter().take(SAMPLE).cloned().collect::<Vec<_>>();
        let suffix = if missing.len() > SAMPLE {
            format!(" … (+{} more)", missing.len() - SAMPLE)
        } else {
            String::new()
        };
        out.push(Violation::new(
            "snapshot_drift",
            Some(ledger.id),
            ledger.abs_path,
            format!(
                "{} of {} `{}` rows exist only in the catalog — the body line-anchors none of \
                 them, so they are absent from git: {}{}. The catalog is machine-local and \
                 git-ignored; re-render the snapshot section from params.",
                missing.len(),
                ledger.claimed.len(),
                ledger.collection,
                shown.join(", "),
                suffix
            ),
        ));
    }
    Ok(out)
}

/// `ledger_defines_nothing` / `entry_without_definition`: an augmented ledger's
/// `params` hold entry ids that its body never defines as citable tokens.
///
/// The twin of [`scan_snapshot_drift`] and deliberately not a variant of it. That one
/// asks whether the body *carries* the row, which an index row satisfies. This asks
/// whether anything can *cite* the entry, which an index row does not satisfy at all:
/// `link_scan` binds a token to a `## <ID> — <title>` heading, so a row-only entry is
/// unreachable however visible it is in the rendered table.
/// docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
///
/// **Why this scan has to exist at all, rather than just the write-path advisory.**
/// `append_entry`/`update_entry` now report `undefined_in_body`, but only for the entry
/// being written. The damage is historical: measured 2026-08-18, ten row-only `A-N`
/// entries with 25 dead cross-file citations, and a ledger with zero `BL-N`
/// definitions against 117. None of those will ever be written again, so no per-write
/// signal reaches them. A sweep is the only thing that does.
///
/// **Two checks, not one, because the remedies differ.** `entry_without_definition`
/// means the ledger writes definitions and these entries missed — one heading each,
/// and the author is the right person to write it. `ledger_defines_nothing` means no
/// entry of the prefix is defined anywhere, so nothing done to one row helps and the
/// ledger's entry format is the subject. Emitting the latter per-entry would be N
/// findings that all say the same thing.
///
/// **The finding is partitioned on whether anything actually cites the entry**, because
/// without that split it reads a ledger that defines on demand as one that forgot.
/// `corpus_cited_tokens` supplies the cited set; ids that are cited AND undefined are named
/// first, since they are the half a reader can act on.
///
/// Three hand-measurements of one ledger gave three answers — "42 omissions", "zero cited",
/// "roughly five cited" — and the check gave the fourth by reading the whole population
/// rather than a sample of it: **33 cited, 9 uncited** on `provenance-subsystem.md`. The
/// uncited 9 are the define-on-citation convention its body documents, working as intended.
/// The other 33 are real dangling references. That spread is why this check measures instead
/// of inferring, and why the message says which of the two it is reporting. History:
/// docs/issues/archive/2026-08-19-entry-without-definition-asserts-omission-without-checking-citations.md
///
/// **NOT gated on `body_keeps_snapshot`, and that is the design decision here.** That
/// gate is right for the row question — a params-canonical tracker mentions a few ids
/// in passing without maintaining a snapshot, and nagging it is noise. Reusing it here
/// would silence precisely the population already broken, since a ledger that defines
/// nothing also tends to anchor little. An advisory that goes quiet where citations
/// break is the defect being fixed, not a pattern to copy.
///
/// **Scoped to params-backed ledgers** (`entry_collection IS NOT NULL`), because those
/// are the only ledgers where the catalog asserts an entry exists that the body may not
/// define. For a prose ledger the body *is* the record: an id with no heading is not an
/// entry that lost its definition, it is an entry that was never written, and reporting
/// it would require guessing intent from a high-water mark.
///
/// Reports only; there is no `fix=`. Writing an entry's heading means writing its
/// title and body, which is content, not repair.
fn scan_undefined_entries(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let ledgers = params_backed_ledgers(conn)?;
    // The citation sweep reads every artifact file, so it runs only when there is something
    // to classify. A catalog whose ledgers all define their entries pays nothing for this.
    let any_undefined = ledgers.iter().any(|l| {
        let defined =
            crate::librarian::catalog::augmentation::body_defined_indices(&l.body, &l.prefix);
        l.claimed.iter().any(|n| !defined.contains(n))
    });
    let corpus = if any_undefined {
        corpus_cited_tokens(conn)?
    } else {
        CorpusTokens::default()
    };

    let mut out = Vec::new();
    for ledger in ledgers {
        let prefix = &ledger.prefix;
        let defined =
            crate::librarian::catalog::augmentation::body_defined_indices(&ledger.body, prefix);
        let undefined: Vec<String> = ledger
            .claimed
            .difference(&defined)
            .map(|n| format!("{prefix}-{n}"))
            .collect();
        if undefined.is_empty() {
            continue;
        }

        if defined.is_empty() {
            out.push(Violation::new(
                "ledger_defines_nothing",
                Some(ledger.id),
                ledger.abs_path,
                format!(
                    "No `{prefix}-N` heading exists anywhere in this body, so all {} entries in \
                     `{}` are uncitable — `link_scan` defines a token only from a \
                     `## {prefix}-N — <title>` heading, and index rows define nothing. Every \
                     citation of every entry here resolves to nothing. This is the ledger's entry \
                     format, not one row's omission: fixing it means giving each entry a heading, \
                     per get_guide(\"tracker-conventions\") § Entry headings.",
                    ledger.claimed.len(),
                    ledger.collection
                ),
            ));
        } else {
            let (cited_undefined, uncited): (Vec<String>, Vec<String>) = undefined
                .iter()
                .cloned()
                .partition(|t| corpus.cited.contains(t));

            // A cited entry with no heading HERE is not automatically a broken reference:
            // `link_scan` binds a token to its defining heading wherever that heading
            // lives, and the compaction ladder deliberately ends with definitions in an
            // archive companion. Only a token defined NOWHERE actually dangles.
            //
            // Self is excluded explicitly rather than assumed away. The local `defined`
            // set comes from the catalog's stored `body` and this map from a fresh read of
            // the file; those are two reads and a disagreement between them must not turn
            // into a claim about the graph.
            let foreign_definer = |t: &String| -> Option<&String> {
                corpus
                    .definers
                    .get(t)?
                    .iter()
                    .find(|p| *p != &ledger.abs_path)
            };
            let (resolved_elsewhere, broken): (Vec<String>, Vec<String>) = cited_undefined
                .iter()
                .cloned()
                .partition(|t| foreign_definer(t).is_some());
            // Named, not merely counted: without the definer a reader cannot tell this
            // case from the broken one, and the obvious repair — add the heading here —
            // creates a SECOND definer, which is an ambiguous token that resolves to
            // nothing. The advice would manufacture the break it claimed to find.
            let elsewhere_example = resolved_elsewhere
                .first()
                .and_then(|t| foreign_definer(t).map(|p| (t.clone(), p.clone())));

            // Bounded sample; the count carries the magnitude. An unbounded list of 50
            // ids buries every other finding — the failure
            // `abs_path_outside_managed_roots` already had to cap for. The sample is drawn
            // from the CITED half whenever there is one, because those are the ids a reader
            // can act on today.
            const SAMPLE: usize = 8;
            // Sample the most actionable population present: genuinely broken first,
            // then the resolves-elsewhere set a reader may still want to see, then the
            // uncited remainder.
            let focus = if !broken.is_empty() {
                &broken
            } else if !resolved_elsewhere.is_empty() {
                &resolved_elsewhere
            } else {
                &uncited
            };
            let shown = focus
                .iter()
                .take(SAMPLE)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let suffix = if focus.len() > SAMPLE {
                format!(" … (+{} more)", focus.len() - SAMPLE)
            } else {
                String::new()
            };

            let detail = if !broken.is_empty() {
                let elsewhere_clause = match &elsewhere_example {
                    Some((token, path)) => format!(
                        "Cited but defined in ANOTHER artifact, so not broken: {} (e.g. \
                             {token} in `{path}`). ",
                        resolved_elsewhere.len()
                    ),
                    None => String::new(),
                };
                format!(
                    "{} of {} `{}` entries have no `## <ID> — <title>` heading. Cited despite \
                         that: {} — {}{}, whose references resolve to nothing right now. Fix those \
                         first; each needs a `## <ID> — <title>` heading, the only shape `link_scan` \
                         binds a token to. {}Uncited: {} — nothing is broken there yet, and that is \
                         consistent with a ledger whose convention is to define an entry when \
                         something first cites it. This check reads the citation graph, so the split \
                         is measured rather than assumed. \
                         See get_guide(\"tracker-conventions\") § Entry headings.",
                    undefined.len(),
                    ledger.claimed.len(),
                    ledger.collection,
                    broken.len(),
                    shown,
                    suffix,
                    elsewhere_clause,
                    uncited.len()
                )
            } else if let Some((token, path)) = &elsewhere_example {
                format!(
                    "{} of {} `{}` entries have no `## <ID> — <title>` heading IN THIS FILE: \
                         {}{}. All {} of the cited ones are defined in a sibling artifact (e.g. \
                         {token} in `{path}`), so `link_scan` resolves them and no reference is \
                         broken. That is the supported end state of the compaction ladder in \
                         get_guide(\"tracker-conventions\") § Compaction and archival — \"live body → \
                         archived section (heading kept)\" — not an omission. Do NOT add a heading \
                         here to close this: a second definer makes the token ambiguous, and an \
                         ambiguous token resolves to nothing, which would manufacture the break this \
                         finding used to claim. Uncited: {}.",
                    undefined.len(),
                    ledger.claimed.len(),
                    ledger.collection,
                    shown,
                    suffix,
                    resolved_elsewhere.len(),
                    uncited.len()
                )
            } else {
                format!(
                    "{} of {} `{}` entries have no `## <ID> — <title>` heading: {}{}. Nothing in \
                     the catalog cites any of them, so no reference is broken today. That is what \
                     a ledger defining an entry only once something cites it looks like, and it is \
                     equally what a ledger that simply never wrote the headings looks like — this \
                     check reads the citation graph, not intent, and will not guess between them. \
                     See get_guide(\"tracker-conventions\") § Entry headings.",
                    undefined.len(),
                    ledger.claimed.len(),
                    ledger.collection,
                    shown,
                    suffix
                )
            };

            out.push(Violation::new(
                "entry_without_definition",
                Some(ledger.id),
                ledger.abs_path,
                detail,
            ));
        }
    }
    Ok(out)
}

/// `cited_prefix_with_no_definer`: a prefix appears in ≥1 citation but has zero definers
/// — no `## <ID> — <title>` heading anywhere in the corpus, and no artifact declares it via
/// `entry_prefix` either. Neither `link_scan` nor `scan_undefined_entries` reaches this state:
/// both start from an entry a ledger already claims (a heading, a `params` row, a declared
/// namespace), and this prefix owns none of those — it is not a *broken* citation in the
/// resolver's terms, it is not a citation candidate at all, so `link_scan`'s own resolver
/// treats it as prose noise (`resolve.rs`'s `prefix_is_known` gate) and reports nothing.
/// docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md
///
/// **Threshold, not a bare "any unknown prefix."** Extraction is deliberately dumb — `UTF-8`
/// and `SHA-256` arrive as entry tokens exactly like `T-1` does, and a design doc that quotes
/// `## R-4` in prose is not a namespace either. Firing on every never-defined prefix would
/// make this check indistinguishable from noise; it fires only when the citation VOLUME looks
/// like a real, abandoned-or-never-created namespace rather than an incidental mention.
///
/// **One combined corpus pass**, not three: citations, definitions, and declared prefixes are
/// all read off the same `extract()` call per artifact — the same door
/// [`corpus_cited_tokens`] and [`crate::librarian::catalog::augmentation::body_defined_indices`]
/// use for the citation and definition halves respectively, so this check cannot disagree with
/// either about what counts as one.
///
/// **Ruling 17 applies to both halves, in opposite directions.**
///
/// - **Definers stay corpus-wide, and that is load-bearing rather than ceremonial.** Narrowing
///   `known_prefixes` to the active project would make every prefix defined only in a sibling
///   repo fire here as "no definer anywhere" — including every cross-repo `<repo>:<TOKEN>`
///   citation, which the resolver already, deliberately, declines to turn into an edge. That
///   manufactures false positives out of correct prose: the mirror of the false negatives
///   Ruling 17 names on the exposure side.
/// - **The firing decision is global too; only the REPORT is filtered.** A prefix is a real,
///   unowned namespace or it is not, and that is a property of the corpus rather than of where
///   the reader happens to be standing. What scoping changes is whether they are handed it. A
///   prefix whose in-project citers clear the same two thresholds is reported — listing those
///   citers, and naming the remainder rather than hiding it. One that does not is counted into
///   `scoped_out` and surfaced via `catalog_health.cited_prefix_scoped_by_project`.
///
/// Measured 2026-08-27, before this change: **33 of the check's 47 findings were other repos'
/// rows** — the single largest contributor to a `doctor` report that was 52% foreign after
/// `442d8b7c` had closed the same class for `abs_path_outside_managed_roots`. See
/// `docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md`.
///
/// A scoped-out prefix is keyed by the group of its first citer **outside** the active project,
/// not by `files[0]`. The alphabetically-first citer overall may well be the in-project one
/// that fell below threshold, and filing the drop under the reader's own project root would
/// read as though their own repo had been excluded from their own report.
fn scan_cited_prefix_with_no_definer(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
) -> Result<(Vec<Violation>, std::collections::BTreeMap<String, usize>)> {
    use crate::librarian::tools::link_scan::extract::{extract, CitationKind};

    // Below this, a prefix reads as incidental prose rather than an abandoned namespace —
    // see the bug's own measurement note: not yet swept against the full catalog, so treat
    // as a starting point, not a tuned constant.
    const MIN_CITATIONS: usize = 3;
    const MIN_FILES: usize = 2;
    // DISPERSION ceiling, as a ratio of files to citations, held in integer form to keep the
    // comparison exact: `files * 5 >= cites * 4` is `files / cites >= 0.8`.
    //
    // `MIN_FILES` is a FLOOR on spread — one file is a one-off. This is the CEILING, and the
    // two are not redundant: a prefix cited once per file across many files is an incidental
    // technical term (`UTF-8`, `SHA-256`, `RFC-7396`), where a real ledger namespace clusters,
    // being cited repeatedly by the few documents that actually depend on it.
    //
    // Measured 2026-09-01 over all 14 live findings on this repo: suppresses 6 of 8 noise
    // prefixes and loses ZERO real ones. Two limits, both deliberate and neither a defect:
    //
    //   * the cut is FITTED to that n=14, on one corpus. What the measurement supports is the
    //     DIRECTION of the signal, not this number; a second corpus is what would establish it.
    //   * 6/8 is a structural CEILING rather than a starting point. `GPT-N` (noise) and
    //     `CC-N`/`O-N` (real) sit at exactly 0.50, so no cut on this signal alone separates
    //     them — lowering it to catch `GPT-N` necessarily loses two real prefixes.
    //
    // docs/issues/archive/2026-09-01-citation-volume-gate-selects-for-the-prose-it-excludes.md
    const DISPERSION_NUM: usize = 5;
    const DISPERSION_DEN: usize = 4;

    let mut stmt = conn.prepare("SELECT abs_path FROM artifact ORDER BY abs_path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    // No active project means no scoping — a config-only caller must not silently receive an
    // empty worklist. Same degradation shape the entry-validity family already uses.
    let cp = ctx.current_project.as_deref();

    let mut known_prefixes: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // prefix -> (citing path -> count in that file). Corpus-wide: this is the metric.
    let mut citations: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, usize>,
    > = std::collections::BTreeMap::new();
    // The citers under the active project's git root — the reported population.
    let mut in_project: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for path in &paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let ex = extract(&text);

        for d in &ex.definitions {
            if let Some((prefix, _)) = d.token.rsplit_once('-') {
                known_prefixes.insert(prefix.to_string());
            }
        }
        known_prefixes.extend(ex.declared_prefixes.iter().cloned());

        let path_in_project = match cp {
            None => true,
            Some(cp) => super::containing_root(std::slice::from_ref(&cp.git_root), Path::new(path))
                .is_some(),
        };
        if path_in_project {
            in_project.insert(path.clone());
        }

        for c in &ex.citations {
            let token = match c.kind {
                CitationKind::EntryToken => Some(c.raw.as_str()),
                CitationKind::CrossRepoToken => c.raw.rsplit_once(':').map(|(_, t)| t),
                _ => None,
            };
            let Some(prefix) = token.and_then(|t| t.rsplit_once('-')).map(|(p, _)| p) else {
                continue;
            };
            *citations
                .entry(prefix.to_string())
                .or_default()
                .entry(path.clone())
                .or_insert(0) += 1;
        }
    }

    let mut out = Vec::new();
    let mut scoped_out: std::collections::BTreeMap<String, usize> = Default::default();
    for (prefix, by_file) in citations {
        if known_prefixes.contains(&prefix) {
            continue;
        }
        // The METRIC: is this a real, unowned namespace anywhere this catalog can see?
        let total: usize = by_file.values().sum();
        if total < MIN_CITATIONS || by_file.len() < MIN_FILES {
            continue;
        }
        // Scattered roughly one-per-file: an incidental term, not a namespace. Applied to the
        // GLOBAL counts only, because whether a prefix is a real namespace is a property of the
        // corpus rather than of where the reader is standing — the same reasoning the doc
        // comment gives for keeping the firing decision global and filtering only the report.
        if by_file.len() * DISPERSION_NUM >= total * DISPERSION_DEN {
            continue;
        }

        // The WORKLIST: the same two thresholds, re-applied to this project's own citers.
        let mut files: Vec<&String> = by_file
            .keys()
            .filter(|f| in_project.contains(f.as_str()))
            .collect();
        files.sort();
        let scoped_total: usize = by_file
            .iter()
            .filter(|(f, _)| in_project.contains(f.as_str()))
            .map(|(_, n)| *n)
            .sum();

        if scoped_total < MIN_CITATIONS || files.len() < MIN_FILES {
            // At least one citer is necessarily outside the project here: were they all
            // inside, the scoped counts would equal the global ones and the metric gate
            // above would have let this through.
            let mut outside: Vec<&String> = by_file
                .keys()
                .filter(|f| !in_project.contains(f.as_str()))
                .collect();
            outside.sort();
            if let Some(first) = outside.first() {
                *scoped_out
                    .entry(outside_roots_group(first.as_str()))
                    .or_insert(0) += 1;
            }
            continue;
        }

        // Carry the corpus-wide figures into the message rather than dropping them. The
        // reader is being handed a subset and should be able to see that it is one —
        // otherwise scoping trades one silent distortion for another.
        let elsewhere_files = by_file.len() - files.len();
        let elsewhere = if elsewhere_files > 0 {
            format!(
                " A further {} citation(s) across {elsewhere_files} file(s) outside this project \
                 are not listed — the namespace is unowned machine-wide, not only here.",
                total - scoped_total
            )
        } else {
            String::new()
        };

        out.push(Violation::new(
            "cited_prefix_with_no_definer",
            None,
            files[0].clone(),
            format!(
                "`{prefix}-N` is cited {scoped_total} times across {} files, but no `## {prefix}-N — \
                 <title>` heading exists anywhere in the corpus and no artifact declares \
                 `entry_prefix: {prefix}`. These citations are neither resolved nor reported \
                 dangling — link_scan's resolver treats a wholly-unknown prefix as prose noise \
                 (the same gate that keeps `UTF-8`/`SHA-256` silent), so this state is reported \
                 nowhere else. Citing files: {}.{elsewhere} Either define the namespace (a heading per \
                 entry) or declare it empty via `entry_prefix` if entries are coming later.",
                files.len(),
                files
                    .iter()
                    .map(|f| f.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    Ok((out, scoped_out))
}

/// Every `docs/issues/archive/<name>.md` a document names, ignoring shapes that cannot be a
/// real citation: a placeholder (`<slug>.md`), a glob (`archive/**`), or a run that swallowed
/// a slash or whitespace because the nearest `.md` was somewhere else entirely.
///
/// Deliberately NOT `link_scan::extract` — that reports a `RelPathLink` only for a markdown
/// link (`Event::Start(Tag::Link)`); inline code goes to `scan_tokens`, which reads entry
/// tokens and artifact ids and no paths at all. In this repo the citation form that actually
/// occurs is a backticked path in prose, so extracting via `extract` would have seen none of
/// the four sites that motivated this check.
fn cited_archive_basenames(text: &str) -> std::collections::BTreeSet<String> {
    const NEEDLE: &str = "docs/issues/archive/";
    let mut out = std::collections::BTreeSet::new();
    let mut rest = text;
    while let Some(pos) = rest.find(NEEDLE) {
        let after = &rest[pos + NEEDLE.len()..];
        if let Some(end) = after.find(".md") {
            let name = &after[..end + ".md".len()];
            if !name.contains('/')
                && !name.contains('<')
                && !name.contains('*')
                && !name.contains(char::is_whitespace)
            {
                out.insert(name.to_string());
            }
        }
        rest = after;
    }
    out
}

/// `premature_archive_citation`: a document cites `docs/issues/archive/<slug>.md`, that path
/// holds no artifact, and `docs/issues/<slug>.md` does — the bug is still open and the
/// citation was written at the path the archive flow *would* create.
///
/// **Always wrong by construction, which is the point.** Every other check here needs a
/// threshold or a judgement call to stay quiet; this one needs neither, because there is no
/// world in which the state is correct. The cited path does not exist, and the file it
/// unambiguously means is sitting un-archived one directory up. Both remedies are legitimate
/// — repoint the citation, or complete the archive — and the check prefers neither.
///
/// **Why nothing else reports it.** `audit_doc_refs` does find these as missing paths, but
/// `cap_code_comment` forces High→Med for a source comment, on purpose, so `--fail-on high`
/// stays silent (`severity.rs`). `ArchiveDrop` does not apply either: it keys on whether the
/// *citing* document is archived, not the target. And the archive sweep in
/// `get_guide("tracker-conventions")` is triggered BY an archive move, so a citation written
/// before one schedules no repair at all — no event fires, no sweep runs, no procedure owns
/// the fix. See `bug-fix-session-log:F-69` and `claim-decay:DC-2`.
///
/// **Scope is catalogued artifacts only**, so this covers markdown and not `.rs` doc
/// comments. That is the honest half of the boundary: of the four citations that motivated
/// it, this reaches the two in markdown — exactly the two that carried full severity anyway
/// — and the two in this file's own doc comments stay `audit_doc_refs` territory, capped.
///
/// A citer under any `archive/` directory is exempt, matching `apply_drops`' `archive_drop`:
/// a retired document citing a path that was correct when written is a record, not drift,
/// and rewriting it would falsify the record to satisfy a linter.
fn scan_premature_archive_citation(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut stmt = conn.prepare("SELECT abs_path FROM artifact ORDER BY abs_path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    // Partition catalogued bug files by basename. Archive is tested FIRST because
    // `/docs/issues/archive/x.md` also contains `/docs/issues/`.
    let mut archived: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut live: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for p in &paths {
        if let Some((_, tail)) = p.split_once("/docs/issues/archive/") {
            if !tail.contains('/') {
                archived.insert(tail);
            }
        } else if let Some((_, tail)) = p.split_once("/docs/issues/") {
            if !tail.contains('/') {
                live.insert(tail);
            }
        }
    }

    let mut out = Vec::new();
    for path in &paths {
        if path.contains("/archive/") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for name in cited_archive_basenames(&text) {
            // The archive path is real — an ordinary, correct citation.
            if archived.contains(name.as_str()) {
                continue;
            }
            // Neither path exists: a plain dead link, and `audit_doc_refs`' to report. Firing
            // here would duplicate it and assert a cause this check cannot know.
            if !live.contains(name.as_str()) {
                continue;
            }
            out.push(Violation::new(
                "premature_archive_citation",
                None,
                path.clone(),
                format!(
                    "cites `docs/issues/archive/{name}`, which holds no artifact, while \
                     `docs/issues/{name}` does — the bug is still open and the citation names \
                     the path the archive flow *would* create. The archive sweep is triggered \
                     BY an archive move, so a citation written before one schedules no repair: \
                     no event fires and no procedure owns the fix. Either repoint the citation \
                     to `docs/issues/{name}`, or complete the archive via \
                     `artifact(action=\"move\")` and re-point every citation in the same commit."
                ),
            ));
        }
    }
    Ok(out)
}

/// `params_behind_body`: an augmented ledger's markdown body anchors entry ids that its
/// `params` hold no row for — the inverse of [`scan_snapshot_drift`].
///
/// Every drift surface codescout had asked one direction of one question: *has the
/// **body** kept up with `params`?* `update_entry`'s `snapshot_stale`, `append_entry`'s
/// `snapshot_missing` and `scan_snapshot_drift` all compute
/// `claimed.difference(&in_body)`. Nothing computed the reverse, so a body that had run
/// **ahead** — rows written into the file by hand, or written before their params row was
/// ever appended — read as perfectly healthy on every surface.
/// docs/issues/archive/2026-08-18-no-check-detects-a-body-that-has-run-ahead-of-params.md
///
/// **The remedy is the opposite of `snapshot_drift`'s, which is why this is a separate
/// check rather than more samples in that one.** There the body is stale and re-rendering
/// it from `params` is repair. Here `params` is stale, and re-rendering would DELETE the
/// newer record. The measured near-miss: generating BL-39's defining headings from the WIN
/// ledger's `params` would have published `WIN-28`/`WIN-29` as `open` when both were
/// `fixed`, and emitted no section at all for six further entries — in the pass whose
/// whole purpose was making entries citable.
///
/// **Ids only, never statuses.** A status mismatch between a params row and a rendered
/// table cell needs a text comparison against a column whose format is each tracker's own
/// choice — fragile, and a separate decision. The id-set difference is exact, and it is
/// what caught the WIN case.
///
/// **Not gated on `body_keeps_snapshot`**, for the reason spelled out on
/// [`scan_undefined_entries`]: that gate answers the *row* question, where a
/// params-canonical tracker anchoring a few ids in passing must not be nagged. Reusing it
/// here would silence a body id the catalog has never seen, which is the entire finding.
/// Pinned by `params_behind_body_is_not_gated_on_body_keeps_snapshot`.
///
/// **Neither `append_entry` nor `update_entry` can perform this repair, and the message
/// must not name them.** It did, on first ship, and the claim was false twice over.
/// `append_entry` ends with `obj.insert("id", new_id)` — it overwrites whatever id the
/// caller passed — and allocates `params_next.max(body_max + 1)`, folding in the very body
/// ids this check is reporting; on the WIN ledger it would mint `WIN-37`, not the missing
/// `WIN-30`. `update_entry` patches a row that already exists and is pinned never to change
/// the row count. The only surface that can create a row at a GIVEN id is the wholesale
/// params write, which is why the message names that and warns that a partial array
/// replaces rather than merges.
///
/// The same first-ship message also claimed an unallocated id "can be reissued". Also
/// false, for the same reason: the `body_max` fold makes reissue impossible while the body
/// still claims the id. It becomes possible only after a compaction moves those rows to an
/// archive companion, and then only for a ledger with no committed
/// `entry_high_water_<PREFIX>` — which is a narrower and conditional claim than the
/// message asserted.
///
/// Reports only; there is no `fix=`. The missing rows carry a status and dates that no
/// scan can infer from an id.
fn scan_params_behind_body(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut out = Vec::new();
    for ledger in params_backed_ledgers(conn)? {
        let in_body = crate::librarian::catalog::augmentation::body_claimed_indices(
            &ledger.body,
            &ledger.prefix,
        );
        let unrowed: Vec<String> = in_body
            .difference(&ledger.claimed)
            .map(|n| format!("{}-{}", ledger.prefix, n))
            .collect();
        if unrowed.is_empty() {
            continue;
        }
        // Bounded sample; the count carries the magnitude. Same cap and reasoning as
        // its two siblings — an unbounded list buries every other finding.
        const SAMPLE: usize = 8;
        let shown = unrowed.iter().take(SAMPLE).cloned().collect::<Vec<_>>();
        let suffix = if unrowed.len() > SAMPLE {
            format!(" … (+{} more)", unrowed.len() - SAMPLE)
        } else {
            String::new()
        };
        out.push(Violation::new(
            "params_behind_body",
            Some(ledger.id),
            ledger.abs_path,
            format!(
                "{n} of {total} `{coll}` ids are anchored in the body but have no row in \
                     `params`: {shown}{suffix}. The body ran ahead of the catalog, so these \
                     entries are absent from `entry_filter` and every params-based query, and \
                     the committed body is their only record. Neither `append_entry` nor \
                     `update_entry` can repair it: the first always allocates the next free id \
                     (folding the body's max in), so it mints a NEW row rather than the missing \
                     ones, and the second only patches a row that already exists. Write the \
                     whole collection instead — `artifact_augment(id=…, merge=true, \
                     params={{\"{coll}\": [ …every row… ]}})`, or the CLI's `--params @<file>` \
                     past the inline budget — and note a params patch REPLACES the array, so a \
                     partial one drops the rest. Do NOT re-render the body from `params`: that \
                     is `snapshot_drift`'s remedy and here it would delete the newer record.",
                n = unrowed.len(),
                total = in_body.len(),
                coll = ledger.collection,
                shown = shown.join(", "),
                suffix = suffix
            ),
        ));
    }
    Ok(out)
}

/// Whether `haystack` states `token` as a status word, tolerating the renderings a
/// hand-written body actually uses.
///
/// Not a substring test, and the difference is load-bearing in both directions.
///
/// **Separators flex.** A body writes `**done, archived**` where `params` holds
/// `done-archived`; measured 2026-08-30, that single comma-vs-hyphen difference accounted
/// for **20 of 26** findings on a naive comparison — the entire "each tracker's own
/// format" objection turned out to be mostly this one convention. Each run of
/// non-alphanumerics in the token therefore matches any run of non-alphanumerics.
///
/// **Word boundaries do not.** Normalising both sides by deleting punctuation — the
/// obvious fix for the above — makes `done` a substring of `aban`**`done`**`d` and
/// `open` of `re-`**`open`**`ed`, so the anchors are kept rather than stripped.
fn status_token_present(haystack: &str, token: &str) -> bool {
    let parts: Vec<String> = token
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(regex::escape)
        .collect();
    if parts.is_empty() {
        return false;
    }
    let pattern = format!(r"(?i)\b{}\b", parts.join(r"[^A-Za-z0-9]*"));
    regex::Regex::new(&pattern)
        .map(|re| re.is_match(haystack))
        .unwrap_or(false)
}

/// The slice of `lines` that states one entry's status, plus the locator that found it.
///
/// Two renderings, because measuring this repo's nine params-backed ledgers on 2026-08-30
/// found both in use and covering disjoint sets: **table rows** (`| BL-60 | … |`) in
/// `open-issue-work-queue` and `windows-platform-support` — 101 entries — and **a heading
/// with a `**Status:**` line** in `fable-tuning-tasks` and `fable-tuning-findings` — 30
/// entries. Supporting only the first, which an earlier draft of this scan did, silently
/// skipped 30 of 131 exposed entries while reporting nothing amiss about them.
///
/// The heading form returns **only the `Status:` line**, never the whole section. A
/// section's prose routinely narrates status history — *"was dropped as a design
/// decision"*, *"local export DONE, one item left"* — and matching against it would report
/// the narration as the status.
///
/// **The table form additionally requires the table to HAVE a status column**, added
/// 2026-09-01. A line anchor alone is not enough: it stops a mention inside another row's
/// prose from masquerading as this entry's row, but not a row that legitimately *starts*
/// with the id in a table about something else. `system-retrospective-improvements` records
/// closures in `## History` and carries an unrelated `| task | headline number | reality |`
/// analysis table, so every entry resolved to a "status region" stating no status — turning
/// the skip this function documents below into four reported findings.
/// docs/issues/archive/2026-09-01-status-locator-reads-any-table-row-as-a-status-row.md
///
/// `None` means *this entry states no status in the body*, which is not a finding: a
/// params-only entry has no second representation and therefore cannot drift from one.
fn entry_status_region(lines: &[&str], eid: &str) -> Option<(String, &'static str)> {
    /// Walk up from a data row to its own table's header and ask whether that table is
    /// about status. Anchored on the top of the row's contiguous `|` block rather than on
    /// "the first table in the file": `windows-platform-support` opens with a *legend*
    /// table explaining its status vocabulary, and `system-retrospective-improvements` has
    /// three tables, so the header must be the one belonging to THIS row.
    ///
    /// Deliberately does NOT require a `|---|` separator. Requiring one looked tidier and
    /// was wrong in the dangerous direction: a real status table written without a
    /// separator would silently stop being compared, which is a false negative in exactly
    /// the direction this check exists to catch. Caught by
    /// `status_drift_fires_when_params_and_the_body_row_disagree`, whose fixture omits it.
    fn table_is_about_status(lines: &[&str], row: usize) -> bool {
        let mut top = row;
        while top > 0 && lines[top - 1].trim_start().starts_with('|') {
            top -= 1;
        }
        if top == row {
            return false; // no header above this row at all
        }
        // Matched case-insensitively because both live ledgers spell it differently —
        // `| ID | Ph | Task | Status | Bug |` and `| id | area | status | … |`.
        lines[top].split('|').any(|c| {
            c.trim()
                .trim_matches('*')
                .trim_matches('`')
                .eq_ignore_ascii_case("status")
        })
    }

    let esc = regex::escape(eid);
    // 1. Table row. Anchored at line start so a mention inside another row's prose
    //    cannot masquerade as this entry's row — and gated on the table having a status
    //    column, so a row in an unrelated table cannot either.
    if let Ok(row_re) = regex::Regex::new(&format!(r"^\|\s*`?{esc}`?\s*\|")) {
        if let Some(i) = lines.iter().position(|l| row_re.is_match(l)) {
            if table_is_about_status(lines, i) {
                return Some((lines[i].to_string(), "table row"));
            }
            // Otherwise fall through to the heading form rather than returning a region
            // that states no status — which would report an entry that cannot drift.
        }
    }
    // 2. Heading section, then its `Status:` line. The dash is required: it is what
    //    `link_scan` treats as defining the entry, and a heading without one is a
    //    section *about* the entry rather than the entry itself.
    let head_re = regex::Regex::new(&format!(r"^(#{{2,6}})\s+`?{esc}`?\s*[—–-]")).ok()?;
    let start = lines.iter().position(|l| head_re.is_match(l))?;
    let level = lines[start].chars().take_while(|c| *c == '#').count();
    for l in &lines[start + 1..] {
        let depth = l.chars().take_while(|c| *c == '#').count();
        if depth > 0 && depth <= level {
            break; // next sibling-or-shallower heading ends the section
        }
        let trimmed = l.trim_start().trim_start_matches('*').trim_start();
        if trimmed.starts_with("Status:") || trimmed.starts_with("Status**:") {
            return Some(((*l).to_string(), "Status: line"));
        }
    }
    None
}

/// `params_status_drift`: an entry's `params` status does not appear in the body region
/// that states its status — the **content** half of the drift whose id half is
/// [`scan_params_behind_body`].
///
/// That scan's own doc comment declines this comparison — *"Ids only, never statuses … a
/// text comparison against a column whose format is each tracker's own choice — fragile,
/// and a separate decision"* — and it was right to, on the evidence then available. What
/// changed is that the fragility was measured rather than estimated
/// (`docs/issues/archive/2026-08-18-no-check-detects-a-params-row-stale-relative-to-its-body.md`,
/// queue row `BL-44`). It is dominated by one convention, `done, archived` for
/// `done-archived`, which [`status_token_present`] absorbs; and a closed `status` enum
/// declared in `params_schema` removes the need to identify a column at all, since any
/// enum token anywhere in the region is evidence about the status.
///
/// **Why it is worth having.** Measured the same day on this repo, `open-issue-work-queue`
/// held four entries whose `params` row contradicted its committed body — `BL-60` read
/// `open` / *"not yet scouted"* for a fix that was shipped, tested, archived and
/// patch-id'd. Every existing surface was silent, because all three are id-set
/// comparisons and every id was present on both sides. `entry_filter` — the query
/// `CLAUDE.md` prescribes for reading that tracker — returns the `params` side, so the
/// documented read path served the wrong answer while the markdown a human opens was
/// correct.
///
/// **Sensitivity, measured, not asserted.** Substituting every other enum value for each
/// entry's true status simulates 536 drifts across this repo's qualifying ledgers; **490
/// (91.4%) are flagged and 46 (8.6%) are silent**, the silent ones being where the
/// substituted word happens to occur in that entry's own region already. So a clean run
/// is evidence, not proof.
///
/// **Three blind spots, all deliberate, none of which the message hides:**
/// - A ledger whose `params_schema` declares no `status` enum is skipped entirely — 5 of
///   this repo's 9. Without a closed vocabulary there is nothing to compare against
///   except free text.
/// - An entry with no body region stating a status is skipped, not reported. It has one
///   representation and cannot drift.
/// - Prose inside a status region that happens to use an enum word is reported. The
///   message is phrased for that: it says the region does not *state* the params status,
///   never that the entry is wrong.
///
/// Reports only; there is no `fix=`. Which side is stale is a judgement — `params` behind
/// a corrected body and a body behind a corrected `params` produce the identical finding,
/// and their remedies are opposites.
fn scan_params_status_drift(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut out = Vec::new();
    for ledger in params_backed_ledgers(conn)? {
        let Some(enum_vals) = ledger.status_enum.as_ref() else {
            continue;
        };
        let lines: Vec<&str> = ledger.body.lines().collect();
        let mut findings: Vec<String> = Vec::new();
        let mut compared = 0usize;
        for (eid, status) in &ledger.statuses {
            let Some((region, locator)) = entry_status_region(&lines, eid) else {
                continue;
            };
            compared += 1;
            if status_token_present(&region, status) {
                continue;
            }
            let seen: Vec<&str> = enum_vals
                .iter()
                .filter(|t| status_token_present(&region, t))
                .map(String::as_str)
                .collect();
            let seen = if seen.is_empty() {
                "no enum value".to_string()
            } else {
                format!("`{}`", seen.join("`, `"))
            };
            findings.push(format!(
                "{eid} (params `{status}`, {locator} states {seen})"
            ));
        }
        if findings.is_empty() {
            continue;
        }
        // Bounded sample; the count carries the magnitude. Same cap and reasoning as its
        // three siblings — an unbounded list buries every other finding.
        const SAMPLE: usize = 8;
        let shown = findings
            .iter()
            .take(SAMPLE)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        let suffix = if findings.len() > SAMPLE {
            format!(" … (+{} more)", findings.len() - SAMPLE)
        } else {
            String::new()
        };
        out.push(Violation::new(
            "params_status_drift",
            Some(ledger.id),
            ledger.abs_path,
            format!(
                "{n} of {compared} `{coll}` entries have a `params` status their body \
                 region does not state: {shown}{suffix}. The two representations of these \
                 entries disagree, so `entry_filter` and the committed markdown answer \
                 the same question differently — and only the body is in git. Read the \
                 body and decide which side is right, then repair the other: \
                 `artifact(action=\"update_entry\", …)` for the `params` side, a \
                 `body_edits` patch for the body. This check is a HEURISTIC and both \
                 directions of error are possible: it is silent on ~8.6% of real \
                 disagreements (measured), and a status region whose prose merely \
                 mentions another enum word is reported here despite being correct — it \
                 reports that the region does not STATE the params status, never that \
                 the entry is wrong.",
                n = findings.len(),
                compared = compared,
                coll = ledger.collection,
                shown = shown,
                suffix = suffix
            ),
        ));
    }
    Ok(out)
}

/// The `frontmatter_id_mismatch` rows on their own, for `fix=repair_frontmatter_id`.
/// Ordered by `abs_path` for the same reason [`scan_artifact_paths`] is — a stable
/// order is what makes a reported sweep reproducible.
///
/// **The check-name filter is the write guard, not tidiness.** The scan's own name has
/// always promised only the mismatch rows, but the check emitted exactly one kind, so
/// nothing enforced it. Now that a declared non-catalog id yields
/// `frontmatter_id_is_not_a_catalog_id`, this filter is what keeps `confirm=true` from
/// splicing a template's `id: ADR-{NUMBER}` to a 16-hex id. Pinned by
/// `repair_frontmatter_id_never_rewrites_a_value_that_was_never_a_catalog_id`, which
/// reproduced that rewrite before the guard existed.
///
/// The second write guard lives one level up, in
/// [`check_frontmatter_id_matches_catalog`]'s worktree-twin abstention: a shadow file whose
/// frontmatter names its main twin never becomes a violation, so it never reaches this
/// function or the sweep it feeds. Pinned by
/// `repair_frontmatter_id_leaves_a_worktree_shadow_alone_and_still_sweeps_the_stale_row`.
fn scan_frontmatter_id_mismatches(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows
        .iter()
        .filter_map(|(id, abs_path)| check_frontmatter_id_matches_catalog(id, abs_path))
        .filter(|v| v.check == "frontmatter_id_mismatch")
        .collect())
}

/// `terminal_status_with_caveat`: a bug file whose `status` is terminal *and* whose
/// `unverified:` field is non-empty.
///
/// **This is the reader half of a feature that shipped without one.** The `unverified:`
/// convention landed 2026-08-19 across `CLAUDE.md`, `docs/TAXONOMY.md`,
/// `docs/issues/_TEMPLATE.md` and `src/prompts/guides/tracker-conventions.md`, and authors
/// began using it the same day. Nothing surfaced it: the canonical triage query filters on
/// `status`, and every record carrying a caveat has by definition a terminal one. A caveat
/// nobody can query is the exact defect the field was introduced to fix, so until this check
/// existed the convention only moved the problem from prose into frontmatter.
///
/// Measured 2026-08-19, the population is small and actionable: 8 files carry the field,
/// 7 live and 1 archived.
///
/// **Archived files are included, deliberately.** `docs/issues/archive/` is where terminal
/// records go and nothing re-reads it — so an archived caveat is not less hidden than a live
/// one, it is more. One of the two archived today records that its own fix *widened* another
/// open bug; that is precisely the kind of consequence which must stay reachable after the
/// file stops being read.
///
/// **Reports every caveat, with no severity split** — CAP-7's open decision 2 asked whether
/// to distinguish blocking from informational caveats via a leading marker. Not done, and
/// not deferred silently: no such marker convention exists, so introducing one now would
/// leave every caveat written before today unmarked and force them all into whichever bucket
/// the default picks. That is a worse report than an undifferentiated one. The check is
/// report-only (open decision 1), so the cost of an over-broad list is a line in a scan
/// nobody is gated on.
///
/// Reports only; there is no `fix=`. Discharging a caveat means establishing the thing it
/// says was never established, which is work, not repair.
fn scan_terminal_status_with_caveat(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    // SQL narrows to the rows worth opening; the field itself lives in `extra`, which is
    // NOT catalog-indexed, so the file has to be parsed. Ordered by abs_path for the same
    // reason as every other scan here — a report nobody can diff against a prior run is
    // half a report.
    let mut stmt = conn.prepare(
        "SELECT id, abs_path, status FROM artifact \
         WHERE kind = 'bug' AND status IN ('fixed', 'mitigated', 'wontfix') \
         ORDER BY abs_path",
    )?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path, status) in &rows {
        // Best-effort per file: an unreadable or unparseable bug file is `missing_file`'s
        // finding or nobody's, never a silent caveat.
        // `content` is bound to a local first because `parse` borrows it — the same shape
        // `check_frontmatter_id_matches_catalog` uses, for the same reason.
        let Ok(content) = std::fs::read_to_string(abs_path) else {
            continue;
        };
        let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) else {
            continue;
        };
        let Some(raw) = fm.extra.get("unverified") else {
            continue;
        };
        // An empty value is the shape the convention explicitly forbids ("never add it
        // empty, because presence is what a query filters on"). Treat it as absent rather
        // than reporting a caveat with no content.
        let caveat = match raw {
            Value::Null => continue,
            Value::String(s) => s.trim().to_string(),
            other => other.to_string(),
        };
        if caveat.is_empty() {
            continue;
        }

        // Char-wise, not byte-wise: these are prose and routinely contain non-ASCII.
        const CAVEAT_MAX: usize = 240;
        let shown = if caveat.chars().count() > CAVEAT_MAX {
            format!("{}…", caveat.chars().take(CAVEAT_MAX).collect::<String>())
        } else {
            caveat
        };

        out.push(Violation::new(
            "terminal_status_with_caveat",
            Some(id.clone()),
            abs_path.clone(),
            format!(
                "status is `{status}` (terminal) but `unverified:` is set, so the canonical \
                 triage query — kind=\"bug\" with status in open/investigating — cannot reach \
                 this record. The caveat says: \"{shown}\". Either discharge it and clear the \
                 field, or leave both: the record stays honest AND findable, which is the \
                 whole point of the field. See get_guide(\"tracker-conventions\") § Bug files."
            ),
        ));
    }
    Ok(out)
}

/// What an artifact's `expects_augmentation:` frontmatter value actually says.
///
/// Three states, not two, because the shape now travels: an artifact can declare that it
/// expects an augmentation *and* name the committed sidecar holding it.
pub(crate) enum Declaration {
    /// No key, or an explicit false.
    Absent,
    /// Declared. `sidecar` is the repo-relative path recorded by the author, when there is
    /// one — `true` is still valid and still means "declared, shape not recorded".
    Declared { sidecar: Option<String> },
    /// Present and uninterpretable. **Reported, never treated as absent** — see below.
    Unparseable,
}

/// Parse `expects_augmentation`: YAML's several spellings of true, plus a sidecar path.
///
/// `expects_augmentation: "true"` must not read as absent. A declaration that silently does
/// not count is worse than no declaration at all, because its author believes they are
/// covered — the same reasoning that makes `validity_unparseable` a reported finding rather
/// than a skipped line.
///
/// That argument is exactly why this replaced a `-> bool` predicate. The bool form returned
/// `false` for every string outside the true-set, so introducing the sidecar form would have
/// made `expects_augmentation: docs/augmentations/foo.yaml` read as **not declared** and
/// switch the check off on the artifacts most carefully configured — the failure the old
/// function's own doc comment warned about, delivered by the change that quoted it. Anything
/// uninterpretable is now a finding.
pub(crate) fn parse_declaration(v: &Value) -> Declaration {
    let declared = || Declaration::Declared { sidecar: None };
    match v {
        Value::Bool(true) => declared(),
        Value::Bool(false) => Declaration::Absent,
        Value::Number(n) => match n.as_i64() {
            Some(1) => declared(),
            Some(0) => Declaration::Absent,
            _ => Declaration::Unparseable,
        },
        Value::String(s) => {
            let t = s.trim();
            match t.to_ascii_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => declared(),
                "false" | "no" | "off" | "0" => Declaration::Absent,
                // A path is a declaration *with* its shape. Extension-gated rather than
                // "any other string": a typo'd `expects_augmentation: ture` must stay a
                // reported finding, not become a sidecar path that will never resolve.
                _ if t.ends_with(".yaml") || t.ends_with(".yml") => Declaration::Declared {
                    sidecar: Some(t.to_string()),
                },
                _ => Declaration::Unparseable,
            }
        }
        // Includes `expects_augmentation:` with no value at all, which parses as null. The
        // author wrote a key and it does nothing; that is the case this reports.
        _ => Declaration::Unparseable,
    }
}

/// `augmentation_declared_but_absent`: frontmatter declares `expects_augmentation: true`
/// and this catalog holds no augmentation row for the artifact.
///
/// **Augmentation is the one artifact state with no on-disk form.** Rows, frontmatter and
/// body all rebuild from disk on `reindex`; `prompt`, `params`, `params_schema`,
/// `render_template` and `entry_collection` live only in the catalog, which is
/// machine-local and git-ignored. So the absence is invisible by construction: `reindex`
/// preserves augmentation keyed by id rather than regenerating it, and therefore reports
/// healthy after a loss and repairs nothing; `artifact(get)` returns `augmentation: null`
/// without comment; and the documented `append_entry` / `update_entry` / `entry_filter`
/// calls fail only at use, one caller at a time.
///
/// **Why a declaration rather than a heuristic.** The obvious detector is to read the body
/// for a `[LIVE]` claim. Measured 2026-08-26 across 4,195 catalogued artifacts: 29 mention
/// `[LIVE]` outside fenced code, and 23 of those carry no augmentation — essentially all of
/// them guides, specs, plans and bug files *describing the mechanism*, including this
/// check's own bug file. A body-sniffing gate is 23 false positives and one true one. The
/// intent simply is not recoverable from prose, so it has to be stated.
///
/// This is the same remedy `entry_prefix` uses, for the same reason and in the same place:
/// *"Frontmatter, because the catalog is machine-local and git-ignored. A declaration
/// stored in the augmentation is absent in a fresh clone."* A ledger is declared, never
/// inferred; so is an augmentation. Firing in a fresh clone is correct behaviour, not a
/// false positive — that clone genuinely has no augmentation, and this check is how its
/// owner finds out before an `append_entry` does.
///
/// Reports only; there is no `fix=`. Re-attaching means supplying a prompt, and often a
/// schema and a template — authored content, not repair. The check names what is missing;
/// it cannot know what it said.
/// docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
fn scan_augmentation_declared_but_absent(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    // The LEFT JOIN is the cost control: only artifacts that could violate are opened, so
    // an already-augmented artifact is never read from disk. Ordered by abs_path like every
    // other scan here — a report nobody can diff against a prior run is half a report.
    let mut stmt = conn.prepare(
        "SELECT a.id, a.abs_path FROM artifact a \
         LEFT JOIN artifact_augmentation g ON g.artifact_id = a.id \
         WHERE g.artifact_id IS NULL AND a.missing_since IS NULL \
         ORDER BY a.abs_path",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path) in &rows {
        // Best-effort per file, as in `scan_terminal_status_with_caveat`: an unreadable or
        // unparseable artifact is `missing_file`'s finding or nobody's, never a silent
        // declaration. `content` is bound first because `parse` borrows it.
        let Ok(content) = std::fs::read_to_string(abs_path) else {
            continue;
        };
        let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) else {
            continue;
        };
        let Some(raw) = fm.extra.get("expects_augmentation") else {
            continue;
        };

        let sidecar_rel = match parse_declaration(raw) {
            Declaration::Absent => continue,
            Declaration::Unparseable => {
                out.push(Violation::new(
                    "augmentation_declaration_unparseable",
                    Some(id.clone()),
                    abs_path.clone(),
                    format!(
                        "`expects_augmentation: {raw}` is neither a boolean nor a sidecar \
                         path ending .yaml/.yml, so it declares nothing. Its author almost \
                         certainly believes this artifact is covered and it is not — the \
                         same failure `validity_unparseable` exists to catch. Write \
                         `true`, or the repo-relative path of the sidecar holding its shape."
                    ),
                ));
                continue;
            }
            Declaration::Declared { sidecar } => sidecar,
        };

        // A declared sidecar that EXISTS on disk is a different finding from one that does
        // not: the first is one command from repaired, the second needs a machine that
        // still holds the row. Reporting both under one detail would hide that.
        let recoverable = sidecar_rel.as_deref().is_some_and(|rel| {
            crate::librarian::current_project::lookup_git_root(std::path::Path::new(abs_path))
                .map(|root| root.join(rel).is_file())
                .unwrap_or(false)
        });

        let detail = if recoverable {
            let rel = sidecar_rel.unwrap_or_default();
            format!(
                "frontmatter declares `expects_augmentation: {rel}` and that sidecar is \
                 present, but this catalog holds no augmentation row for it. The shape \
                 travelled; only the attach is missing. Run librarian(action=\"reindex\") — \
                 it re-attaches a declared sidecar whenever the row is absent, and never \
                 overwrites a live one. `params` are deliberately not carried, so the \
                 restored tracker starts with no rows."
            )
        } else {
            let named = match sidecar_rel.as_deref() {
                Some(rel) => format!(
                    "declares `expects_augmentation: {rel}`, but no such sidecar exists on \
                     disk"
                ),
                None => "declares `expects_augmentation: true`, which records that a shape \
                         is expected but not what it was"
                    .to_string(),
            };
            format!(
                "frontmatter {named}, and this catalog holds no augmentation for it. Its \
                 prompt, params, params_schema, render_template and entry_collection are \
                 all absent: every documented `append_entry` / `update_entry` / \
                 `entry_filter` call against this id fails at use, and it contributes no \
                 [LIVE] block to librarian(action=\"context\"). With no sidecar there is \
                 nothing on disk to rebuild from, so `reindex` cannot help and reports \
                 healthy regardless. The shape survives only in the catalog of a machine \
                 that has not lost it — run librarian(action=\"doctor\", \
                 fix=\"export_augmentations\") THERE and commit the result. Failing that, \
                 re-author with artifact_augment(id=…, prompt=…, …)."
            )
        };

        out.push(Violation::new(
            "augmentation_declared_but_absent",
            Some(id.clone()),
            abs_path.clone(),
            detail,
        ));
    }
    Ok(out)
}

/// `sidecar_shape_drift`: the artifact HAS an augmentation row, declares a sidecar, that
/// sidecar exists on disk — and the two disagree about the shape.
///
/// **This is the safety net for `artifact_augment`'s write-through, and it is why that
/// write-through is trustworthy.** Write-through can only ever cover the call sites someone
/// remembered to sweep; this check covers the invariant regardless of how the disagreement
/// arose — a hand-edit, a graft, a worktree merge, a future writer nobody swept. The failure
/// it guards is one this mechanism has already produced once: the export skips an artifact
/// whose sidecar exists, `reindex` attaches only when a row is absent, so before the
/// write-through landed nothing could update a committed sidecar, and a fresh clone restored
/// the superseded shape reporting `augmentations_restored: 1` — success. A stale sidecar is
/// worse than an absent one, because absence is loud and staleness restores clean.
///
/// **Reports only, and deliberately has no `fix=` — because drift has a DIRECTION this check
/// cannot determine.** If the catalog is newer (a local shape change not yet exported),
/// re-exporting is right. If the *sidecar* is newer — someone pulled a teammate's shape change
/// and has not reindexed, so the local row is the stale one — re-exporting destroys the
/// committed shape and replaces it with the stale local row, which is the very data loss this
/// family of checks exists to prevent. mtime cannot discriminate: a git checkout stamps the
/// file with checkout time regardless of semantic age, so the newer-looking file is routinely
/// the older shape. The remedy therefore requires a human judgement about which side is right,
/// exactly as `augmentation_declared_but_absent` concluded for its own case.
///
/// **The first live run replaced that argument with a stronger one, and this is the argument
/// the design now rests on.** The very first finding — `docs/trackers/open-issue-work-queue.md`
/// — drifted in BOTH DIRECTIONS AT ONCE: the sidecar held a `prompt` edit the catalog never
/// received (a hand-edit of the committed YAML never touches the catalog), while the catalog
/// held a newer `params_schema`. So direction is not a property of an ARTIFACT at all; it is a
/// property of each FIELD. A per-artifact `fix=` is therefore not merely risky, it is
/// **incoherent** — there is no single direction for it to point, and either choice destroys
/// real content: re-exporting would have deleted the widened-vocabulary documentation, and
/// applying the sidecar would have deleted the refined schema description. Getting this from
/// the FIRST live finding means the field-level case is not rare, so a repair shipped on the
/// caution argument would have destroyed one field or the other on its first use.
///
/// Do not "fix" this by adding a `fix=`. If a future reader wants one, the smallest sound
/// version is per-field and requires the operator to choose a side per field, which is the
/// human judgement above with extra machinery, not a replacement for it.
///
/// A sidecar that exists but does not parse is reported separately as `sidecar_unparseable`,
/// because **nothing else can see it**: `reindex` skips the artifact entirely once a row is
/// present, so a corrupt committed shape would otherwise sit unread until the machine that
/// holds the row loses it — which is the one moment it is needed and the one moment it fails.
fn scan_sidecar_shape_drift(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    use crate::librarian::augmentation_sidecar as sidecar;

    // The inverse JOIN of the sibling check: only AUGMENTED artifacts can drift, so an
    // unaugmented one is never opened. Ordered by abs_path so two runs can be diffed.
    let mut stmt = conn.prepare(
        "SELECT a.id, a.abs_path FROM artifact a \
         JOIN artifact_augmentation g ON g.artifact_id = a.id \
         WHERE a.missing_since IS NULL \
         ORDER BY a.abs_path",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path) in &rows {
        let Ok(content) = std::fs::read_to_string(abs_path) else {
            continue;
        };
        let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) else {
            continue;
        };
        let Some(raw) = fm.extra.get("expects_augmentation") else {
            continue;
        };
        // `true` declares no sidecar and an unparseable value is the sibling check's finding.
        let Declaration::Declared { sidecar: Some(rel) } = parse_declaration(raw) else {
            continue;
        };
        let Some(root) =
            crate::librarian::current_project::lookup_git_root(std::path::Path::new(abs_path))
        else {
            continue;
        };
        let path = root.join(&rel);
        // Declared-but-absent with a row present is not this check's case and is not silent:
        // the artifact simply has no committed shape yet, which the export exists to fix.
        if !path.is_file() {
            continue;
        }

        let committed = match sidecar::read(&path) {
            Ok(s) => s,
            Err(e) => {
                out.push(Violation::new(
                    "sidecar_unparseable",
                    Some(id.clone()),
                    abs_path.clone(),
                    format!(
                        "`{rel}` is declared and present but does not parse: {e:#}. Nothing else \
                         reports this — `reindex` skips an artifact that already has a row, so a \
                         corrupt committed shape stays unread until the machine holding the row \
                         loses it, which is the one moment it is needed. Repair the YAML by hand, \
                         or delete it and re-run librarian(action=\"doctor\", \
                         fix=\"export_augmentations\") on a machine whose row is correct."
                    ),
                ));
                continue;
            }
        };

        let Ok(Some(row)) = crate::librarian::catalog::augmentation::get_by_conn(conn, id) else {
            continue;
        };
        let fields = sidecar::drifting_fields(&row, &committed);
        if fields.is_empty() {
            continue;
        }

        out.push(Violation::new(
            "sidecar_shape_drift",
            Some(id.clone()),
            abs_path.clone(),
            format!(
                "the committed sidecar `{rel}` and this catalog's augmentation disagree on: {}. \
                 One of them is stale and THIS CHECK CANNOT TELL WHICH — mtime does not \
                 discriminate, because a git checkout stamps the file with checkout time \
                 whatever its shape's age. Read the difference before acting. If your catalog is \
                 right (you changed the shape here and it has not been published), DELETE the \
                 sidecar and then re-run librarian(action=\"doctor\", \
                 fix=\"export_augmentations\") — that fix CREATES sidecars and never refreshes \
                 them, so it skips any artifact whose sidecar already exists, which is every \
                 finding this check can emit. Without the delete it reports exported: 0, exits \
                 successfully, and repairs nothing. If the SIDECAR is \
                 right (you pulled someone else's shape change), do NOT export — that would \
                 overwrite their shape with your stale row; apply the committed values with \
                 artifact_augment instead, which also rewrites the file and so leaves the two \
                 agreeing. Until this is resolved, a fresh clone restores whatever the sidecar \
                 says and reports success.",
                fields.join(", ")
            ),
        ));
    }
    Ok(out)
}

/// Every structured fix pointer a bug file's `## Fix provenance` section carries.
///
/// Returns one `(sha, patch_id)` per `- **SHA:**` line, each paired with the
/// `- **patch-id:**` line that follows it before the next SHA. Empty when the file declares
/// none. Deliberately NOT a general hex sweep — see [`scan_archived_fix_sha_unresolvable`]
/// for why that would be worse than useless.
///
/// **Plural, because a fix is not always one commit, and the singular shape hid one.**
/// `docs/issues/archive/2026-08-19-entry-without-definition-…` is fixed by two commits in
/// two files, neither superseding the other. Written against a parser that could hold one
/// pair, its author reached for a table instead — which read well to a human and was
/// invisible here, so both anchors it recorded were verified by nothing. A shape that makes
/// the honest case unrepresentable gets worked around, and the workaround is the defect.
fn structured_fix_pointers(content: &str) -> Vec<(String, Option<String>)> {
    fn backticked(s: &str) -> Option<String> {
        let start = s.find('`')? + 1;
        let rest = s.get(start..)?;
        let end = rest.find('`')?;
        Some(rest[..end].to_string())
    }
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    // Fenced lines are quotations, not declarations. A bug file that shows a provenance
    // block — to teach the shape, or to reproduce a defect in this very parser — had its
    // example read as its own claim, and the SHA inside it verified as though declared.
    // Same treatment `tracker-conventions` gives `**Valid:**` / `**Rests on:**`, and the
    // same hazard already documented on `corpus_cited_tokens`. Measured before shipping:
    // of 75 declared pointer lines in docs/issues/, 74 sit outside fences and the only
    // fenced one is a worked example, so this loses no real declaration.
    // docs/issues/archive/2026-08-26-structured-fix-pointers-reads-a-fenced-example-as-a-declaration.md
    let mut fence = crate::util::markdown_fence::FenceState::new();
    for line in content.lines() {
        let t = line.trim_start();
        if fence.feed(t) {
            continue;
        }
        if fence.in_fence() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("- **SHA:**") {
            if let Some(sha) = backticked(rest) {
                out.push((sha, None));
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("- **patch-id:**") {
            // Attach to the most recent SHA still missing one. A patch-id line appearing
            // before any SHA line anchors nothing this check can verify, so it is dropped
            // rather than guessed onto a later commit it may not describe.
            if let Some(last) = out.last_mut() {
                if last.1.is_none() {
                    last.1 = backticked(rest);
                }
            }
        }
    }
    out
}

/// `archived_fix_sha_unresolvable`: an archived bug file whose declared fix SHA no longer
/// names an object in this repo.
///
/// **Why this exists, measured 2026-08-19:** archived bug files carry the only record of
/// which commit fixed what, and nothing re-reads `archive/`. A SHA is positional — it dies
/// when `experiments` is rebased, which happens after every ship — so the pointer rots
/// silently. Subject-keyword recovery of a dead one returned 2–153 ambiguous candidates.
/// That is why the `patch-id` now rides alongside, and why this check reports it: the
/// remedy travels with the finding instead of being looked up afterwards.
///
/// **Scanned population: the STRUCTURED form only, and the report says so.** Of 350
/// archived files, 54 carry the `## Fix provenance` triple; the other 296 mention commits
/// in freeform prose. Sweeping those for hex would be worse than not checking them:
/// sampled prose includes `` `a45f1bd7` `` naming a *reproduction* commit, and `12707fe`
/// appearing as "suspected the recent refactor" and then "**Refactor 12707fe is
/// INNOCENT**". Reporting either as a dead fix SHA would be a confident wrong answer about
/// a commit the file itself exonerates — the exact failure mode
/// [`check_outside_managed_roots`] takes pains to avoid. Coverage is reported in
/// `catalog_health.archived_fix_shas` so a clean result cannot be read as "all 350 archived
/// fixes resolve".
///
/// **Resolvability, not reachability.** `revparse_single` failing means the object is gone
/// from the database. A commit that resolves but is unreachable from any ref is a weaker,
/// different question — orphaned-but-not-yet-collected is the normal state of a
/// cherry-picked fix's original, and flagging it would fire on healthy repos. CAP-7's
/// measurement used the same predicate: "objects absent from the object DB, not merely
/// unreferenced".
///
/// Scoped to the active project's git root, like `fix=repair_frontmatter_id`: the catalog
/// spans every repo on the machine, and a SHA only means anything inside the repo that
/// minted it. Resolving a codescout SHA against a sibling repo would report every one of
/// them dead.
///
/// **Scans every bug row, not only archived ones** — `archived` in the check name is the
/// motivating case, not the filter. A fixed-but-unarchived file's pointer rots by the same
/// mechanism, and gating on path would make the check miss it during exactly the window
/// when someone might still remember the commit. Measured on this repo: 54 scanned, 324
/// skipped, of which 296 are archived and 28 are live.
///
/// Reports only; there is no `fix=`. Recovering a dead pointer means finding the commit by
/// patch-id, which is a search, not a repair.
fn scan_archived_fix_sha_unresolvable(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
) -> Result<(Vec<Violation>, Value)> {
    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok((
            Vec::new(),
            json!({
                "note": "no active project, so there is no repo to resolve SHAs against — \
                         declared fix SHAs were NOT checked"
            }),
        ));
    };
    let Ok(repo) = git2::Repository::open(&cp.git_root) else {
        return Ok((
            Vec::new(),
            json!({
                "note": format!(
                    "{} is not a git repository, so declared fix SHAs were NOT checked",
                    crate::util::fs::RepoPath::from_path(&cp.git_root).into_string()
                )
            }),
        ));
    };

    let mut stmt =
        conn.prepare("SELECT id, abs_path FROM artifact WHERE kind = 'bug' ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let (mut scanned, mut skipped, mut cross_repo, mut out) = (0usize, 0usize, 0usize, Vec::new());
    for (id, abs_path) in &rows {
        let path = Path::new(abs_path);
        if super::containing_root(std::slice::from_ref(&cp.git_root), path).is_none() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let pointers = structured_fix_pointers(&content);
        if pointers.is_empty() {
            skipped += 1;
            continue;
        }
        // `scanned` counts FILES; `unresolvable` counts POINTERS. A two-commit fix can
        // contribute two findings from one file, which is the point — verifying only the
        // first anchor would leave the second rotting exactly as if it were never recorded.
        scanned += 1;
        for (sha, patch_id) in &pointers {
            // `<repo>:<sha>` cites a commit in a SIBLING repo (CLAUDE.md § Git Workflow).
            // This check is deliberately scoped to one repo, holds no map from prefix to
            // checkout path, and the sibling may not be on this machine at all. Handing the
            // whole token to revparse asks git for "the object at path <sha> inside a
            // tree-ish named <repo>" — which fails, so a perfectly good pointer was reported
            // dead and offered a patch-id remedy that cannot address another repo's commit.
            // Counted as not-checked rather than resolved elsewhere: guessing a checkout path
            // would trade one false positive for an unbounded one.
            // docs/issues/archive/2026-08-26-doctor-reports-a-cross-repo-fix-sha-as-dead.md
            if sha.contains(':') {
                cross_repo += 1;
                continue;
            }
            if repo.revparse_single(sha).is_ok() {
                continue;
            }
            let remedy = match patch_id {
                Some(p) => format!(
                    "Recover it by patch-id `{p}` — a content hash of the diff, invariant under \
                 rebase and cherry-pick. Use redirects, not pipes (Iron Law 3 blocks an \
                 unbounded `git log -p` piped to a trimmer): `git log --all -p > /tmp/all.patch` \
                 then `git patch-id --stable < /tmp/all.patch > /tmp/ids.txt` and grep it."
                ),
                None => "No patch-id was recorded next to it, so there is no content-addressed \
                     way back: recovery means subject-keyword search, which returned 2-153 \
                     ambiguous candidates when measured. Record the pair on future fixes."
                    .to_string(),
            };
            out.push(Violation::new(
                "archived_fix_sha_unresolvable",
                Some(id.clone()),
                abs_path.clone(),
                format!(
                    "declared fix SHA `{sha}` no longer names an object in this repo. A SHA is \
                 positional and dies when `experiments` is rebased, which happens after every \
                 ship; nothing re-reads `archive/`, so this rots unobserved. {remedy}"
                ),
            ));
        }
    }

    let health = json!({
        "scanned": scanned,
        "skipped_no_structured_pointer": skipped,
        "skipped_cross_repo_pointer": cross_repo,
        "unresolvable": out.len(),
        "note": "Only files carrying the `## Fix provenance` triple are checked. Files that \
                 mention commits in freeform prose are SKIPPED, not passed: sweeping prose for \
                 hex would flag reproduction and exonerated-suspect commits as dead fixes. \
                 `skipped_cross_repo_pointer` counts `<repo>:<sha>` citations, which name a \
                 commit in a sibling repo and cannot be resolved from here — they were NOT \
                 verified, so a zero `unresolvable` does not mean every declared fix resolves.",
    });
    Ok((out, health))
}
/// Backticked tokens that look like an abbreviated commit hash: 7–12 hex characters, at
/// least one of them a letter.
///
/// Narrow on purpose, in three ways. **16** hex is a catalog artifact id and **40** is a
/// patch-id, so both lengths are excluded — including them would report a bug file's own id
/// as a stray commit reference. Requiring a letter drops line numbers and version digits,
/// which are all-hex by accident. And the result is used only to say *hashes are present*,
/// never to claim one of them is the fix: [`scan_archived_fix_sha_unresolvable`] documents
/// at length why sweeping prose for hex and calling the result a fix SHA produces confident
/// wrong answers about reproduction commits and exonerated suspects.
///
/// **Fences are skipped and backticks pair PER LINE, because global parity is not robust.**
/// The first cut split the whole file on backticks and took alternate spans, which assumes
/// every backtick pairs. One that does not inverts every span after it. Measured on
/// `2026-08-18-three-ledgers-own-prefix-t-…`: a lone `` ` `` inside a fenced `grep` pattern
/// (`'^#\{1,6\}[[:space:]]\+`\?T-[0-9]\+'`) left 393 backticks in the file, 71 of them before
/// line 60 — so `c7bdfd22` read as prose and the file reported zero hashes while carrying
/// three occurrences of one. Per-line pairing confines a stray backtick to its own line, and
/// inline code does not span lines, so nothing legitimate is lost.
fn commit_like_hashes(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in content.lines() {
        // A fence toggles on any ``` line — including ```rust. Content inside is quoted
        // material, not this document's own references, and it is where stray backticks live.
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for span in line.split('`').skip(1).step_by(2) {
            let plausible = (7..=12).contains(&span.len())
                && span
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
                && span.chars().any(|c| c.is_ascii_alphabetic());
            if plausible && !out.iter().any(|s| s == span) {
                out.push(span.to_string());
            }
        }
    }
    out
}

/// `terminal_status_without_fix_anchor`: a live bug file that is `fixed` or `mitigated` but
/// declares no `## Fix provenance` pointer, so nothing records which commit closed it.
///
/// **The complement of [`scan_archived_fix_sha_unresolvable`], and the larger population.**
/// That check verifies a declared SHA still resolves; it cannot fire on a record that
/// declares none — and absence is the common case, since the SHA-plus-patch-id rule landed
/// 2026-08-19 and everything written before it is unanchored by default.
///
/// **Live files only, and that is a measurement rather than a preference.** Of 355 archived
/// bug files, 58 carry the structured pointer and 297 do not; reporting those would be 297
/// findings against records `get_guide("tracker-conventions")` explicitly calls *"stale
/// instructions, not open debt"*. Of 14 live terminal files, 4 are anchored and 10 are not.
/// The check fires where the remedy is still cheap and still owed: the guide requires the
/// anchor **at archive time**, so a live terminal record is exactly the one about to need it.
///
/// **`wontfix` is excluded** — nothing was fixed, so no commit exists to point at. `fixed`
/// and `mitigated` both imply a change landed.
///
/// **A record can declare that it owes nothing, via `no_fix_commit:` in frontmatter.** A
/// mitigation that was a doc note or a workaround has no commit, and with no way to say so
/// the check would nag those records forever until someone silenced it wholesale. Same shape
/// as `unverified:`: absence means an anchor is owed, presence discharges it, and an empty
/// value counts as absent because presence is what a query reads.
///
/// **Names the misleading case separately, because it is the one a reader gets wrong — and
/// it is the majority, not a sub-case.** A file with no declared anchor but with commit-like
/// hashes in its prose READS as anchored. Measured 2026-08-19 on this repo, **8 of the 9**
/// findings carry such hashes and exactly one is plainly hash-free; the hashes are typically
/// the commit the bug was OBSERVED at, sitting in an `Environment` line. (Two earlier figures
/// for this were wrong and are worth knowing: a hand-inspection said *three*, because its
/// probe counted the word "SHA" rather than hashes and so never opened the three heaviest
/// carriers; and the check's own first run said *seven*, because [`commit_like_hashes`] paired
/// backticks across the whole file. Each instrument was measuring a property adjacent to the
/// one being reasoned about.) Stating
/// *"hashes are present, none declared as the fix"* is safe; naming one as the fix is the
/// confident wrong answer this module refuses to give.
///
/// Reports only; there is no `fix=`. Recovering a fix SHA is research, and a wrong anchor is
/// worse than an absent one.
fn scan_terminal_status_without_fix_anchor(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
) -> Result<Vec<Violation>> {
    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT id, abs_path, status FROM artifact \
         WHERE kind = 'bug' AND status IN ('fixed', 'mitigated') \
         ORDER BY abs_path",
    )?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path, status) in &rows {
        let path = Path::new(abs_path);
        if super::containing_root(std::slice::from_ref(&cp.git_root), path).is_none() {
            continue;
        }
        // Archived records are out of scope. Match a path COMPONENT rather than a substring,
        // so a repo that happens to live under a directory named `archive` does not silence
        // its entire issue tree.
        if path
            .components()
            .any(|c| c.as_os_str() == std::ffi::OsStr::new("archive"))
        {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if !structured_fix_pointers(&content).is_empty() {
            continue;
        }
        if let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) {
            if let Some(raw) = fm.extra.get("no_fix_commit") {
                let declared = match raw {
                    Value::Null => String::new(),
                    Value::String(s) => s.trim().to_string(),
                    other => other.to_string(),
                };
                if !declared.is_empty() {
                    continue;
                }
            }
        }

        let decoys = commit_like_hashes(&content);
        const DECOYS_SHOWN: usize = 4;
        let misleading = if decoys.is_empty() {
            String::new()
        } else {
            let shown: Vec<String> = decoys
                .iter()
                .take(DECOYS_SHOWN)
                .map(|h| format!("`{h}`"))
                .collect();
            let more = decoys.len().saturating_sub(DECOYS_SHOWN);
            let tail = if more > 0 {
                format!(" (+{more} more)")
            } else {
                String::new()
            };
            format!(
                " It does not merely lack an anchor, it READS as anchored: {} commit-like \
                 hash(es) sit in its prose — {}{} — none declared as the fix. Where this was \
                 measured, the hash was the commit the bug was OBSERVED at, so a reader \
                 scanning for provenance finds one and stops looking.",
                decoys.len(),
                shown.join(", "),
                tail
            )
        };
        out.push(Violation::new(
            "terminal_status_without_fix_anchor",
            Some(id.clone()),
            abs_path.clone(),
            format!(
                "status is `{status}` but no `## Fix provenance` pointer is declared, so \
                 nothing records which commit closed this.{misleading} Record both lines — \
                 the SHA, and the patch-id from `git show <sha> | git patch-id --stable` — \
                 which the guide requires AT archive time; once the SHA orphans on a rebase, \
                 recovery measured 2-153 ambiguous candidates. If the mitigation had no \
                 commit, say that in `no_fix_commit:` rather than leaving it ambiguous."
            ),
        ));
    }
    Ok(out)
}

/// `unterminated_fence`: a catalogued markdown file that reaches EOF with a fence still
/// open, so every line below the opener is read as code by any line-anchored scan.
///
/// **The failure this catches is silence, which is why nothing else catches it.** A
/// line-anchored field inside a fence is skipped — deliberately, so a worked example
/// teaching `**Valid:**` or `- **SHA:**` syntax is never read back as a declaration. An
/// unterminated fence turns that correct rule into a mute button for the rest of the file,
/// and the consumer cannot tell the difference: "declared nothing" and "declared, but
/// unreadable from line N" produce the identical empty result. Measured 2026-09-01 —
/// `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` had a
/// doubled delimiter at lines 53–54 and `doctor` reported *"no `## Fix provenance` pointer
/// is declared"* for a file that visibly contained one. True of the parse, false of the
/// file, and the reader is the one who pays.
///
/// **Delegates to [`FenceState`](crate::util::markdown_fence::FenceState) rather than
/// counting delimiters, and the distinction is not academic.** A parity count calls any
/// odd number of ``` lines unbalanced; a file using ```` to quote triple-fence syntax has
/// an odd count and is perfectly well formed. Counting would report it, wrongly, and that
/// file shape exists here *because* the sibling checks teach people to write worked
/// examples. `unterminated_fence_is_silent_on_a_quadruple_fenced_example` pins it.
///
/// **Reports the opener's FILE line**, not a body offset — a reader opens the file, and in
/// a ledger thousands of lines long "somewhere below here" is the whole remedy.
///
/// Archived files are included: a stray delimiter misparses wherever it sits, and
/// `scan_archived_fix_sha_unresolvable` already reads that population.
///
/// Reports only; there is no `fix=`. The repair is deleting or closing one delimiter, and
/// which of the two is correct is a content judgement about the surrounding prose.
fn scan_unterminated_fence(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
) -> Result<Vec<Violation>> {
    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (id, abs_path) in &rows {
        let path = Path::new(abs_path);
        if super::containing_root(std::slice::from_ref(&cp.git_root), path).is_none() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        // An unreadable file is `missing_file`'s finding or nobody's, never a silent
        // fence report — same best-effort shape as the sibling scans.
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };

        // Track the line the CURRENTLY open fence was opened at. `FenceState` answers
        // whether we are inside one; it does not carry where that began, and where is
        // the entire value of the finding.
        let mut fence = crate::util::markdown_fence::FenceState::new();
        let mut opened_at: Option<usize> = None;
        for (idx, line) in content.lines().enumerate() {
            if fence.feed(line.trim_start()) {
                opened_at = if fence.in_fence() {
                    Some(idx + 1)
                } else {
                    None
                };
            }
        }
        if !fence.in_fence() {
            continue;
        }
        let line_note = match opened_at {
            Some(n) => format!("line {n}"),
            None => "an unlocatable line".to_string(),
        };

        out.push(Violation::new(
            "unterminated_fence",
            Some(id.clone()),
            abs_path.clone(),
            format!(
                "a fence opened at {line_note} is never closed, so every line below it is \
                 read as code by line-anchored scans — `- **SHA:**`, `**Valid:**`, \
                 `**Rests on:**`. Those scans then report NOTHING DECLARED, which is true \
                 of the parse and false of the file, and no error is raised anywhere. \
                 Close or delete the delimiter; which one is right depends on the prose \
                 around it."
            ),
        ));
    }
    Ok(out)
}

fn check_abs_path_must_be_absolute(id: &str, abs_path: &str) -> Option<Violation> {
    // Schema declares `abs_path TEXT NOT NULL UNIQUE` but does not enforce
    // absoluteness. Pre-#66 code paths stored relative strings here in some
    // cases; the doctor catches the wrong-shape rows so they can be migrated
    // (or evicted via reindex) rather than masquerading as `missing_file`
    // false positives (Path::exists resolves them against the caller's cwd).
    //
    // Absolute on the platforms we care about:
    //   - POSIX: leading `/` (also covers the Windows verbatim-prefix form
    //     `//?/C:/...`, which starts with `/`).
    //   - Windows: leading `<drive>:` (`C:`, `D:`, …), bare or verbatim-prefixed
    //     — see `drive_letter_prefix_len`.
    //   - Windows UNC `\\server\share` is allowed in theory but extremely
    //     unusual in our content corpus; if it ever appears the
    //     `backslash_in_abs_path` check catches it first.
    let starts_with_posix_root = abs_path.as_bytes().first() == Some(&b'/');
    let starts_with_drive = crate::util::fs::drive_letter_prefix_len(abs_path).is_some();
    if starts_with_posix_root || starts_with_drive {
        return None;
    }
    Some(Violation::new(
        "abs_path_must_be_absolute",
        Some(id.to_string()),
        abs_path,
        "abs_path is relative — schema requires absolute form (leading '/' or '<drive>:')",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, upsert as art_upsert, TestArtifactRowBuilder};
    use crate::librarian::catalog::augmentation;
    use crate::librarian::catalog::events::{self, TestEventRowBuilder};
    use crate::librarian::catalog::worktree as reg;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use rusqlite::params;

    fn seed_artifact(cat: &Catalog, id: &str, abs_path: &str) {
        cat.conn
            .execute(
                "INSERT INTO artifact \
                 (id, abs_path, kind, status, created_at, updated_at, file_mtime, file_sha256) \
                 VALUES (?1, ?2, 'spec', 'active', 0, 0, 0, '')",
                params![id, abs_path],
            )
            .unwrap();
    }

    #[tokio::test]
    async fn the_audit_block_names_the_host_and_the_unexported_delta() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "1111111111111111", "/tmp/whatever/a.md");
        // Task 6: `unexported_count` is now scoped to the current project's
        // repo root — without one, the seeded row's own insert audit row has
        // nothing to be counted against (no scope means no pending count,
        // same degradation as `move_candidates` just above it). Root the
        // fixture at the seeded artifact's own parent directory so it
        // resolves as this repo's row, same as `ctx_rooted_at` elsewhere in
        // this module.
        let ctx = ctx_rooted_at(cat, std::path::Path::new("/tmp/whatever"));
        let v = call(&ctx, json!({})).await.unwrap();
        let audit = &v["catalog_health"]["audit"];
        assert!(audit["host"].is_string(), "{audit}");
        assert!(audit["unexported_rows"].is_number(), "{audit}");
        assert!(
            audit["hint"].as_str().unwrap().contains("export"),
            "an unexported delta a reader cannot act on is decoration: {audit}"
        );
    }

    /// An absolute, forward-slash-normalised root that does not exist, on any
    /// platform.
    ///
    /// A literal like `"/nonexistent-root/repo"` is **not absolute on Windows** —
    /// `Path::is_absolute` there wants a drive or UNC prefix — so a POSIX-shaped
    /// fixture silently changes what the test exercises:
    ///
    /// - `validate_rehome_request` rejects it at the absolute-path gate before the
    ///   test's actual subject runs, and
    /// - `derive_dead_roots` skips such rows outright, by design and on purpose
    ///   (see `derive_dead_roots_skips_non_absolute_paths` — the guard stops the
    ///   ancestor climb bottoming out at an empty `PathBuf` whose prune `WHERE`
    ///   would match every absolute row).
    ///
    /// Both are correct product behaviour, so the fixture is what has to change.
    /// `temp_dir()` supplies a real absolute prefix everywhere; the unique suffix
    /// keeps the path missing. Its PARENT exists, which is what makes the derived
    /// dead root land on the suffix directory rather than climbing further.
    /// Normalised with `RepoPath` so it concatenates cleanly with the forward-slash
    /// `abs_path` shape the catalog stores.
    fn dead_root(tag: &str) -> String {
        let p = std::env::temp_dir().join(format!("codescout-nonexistent-{tag}"));
        assert!(
            !p.exists(),
            "fixture root must not exist on disk: {}",
            p.display()
        );
        crate::util::fs::RepoPath::from_path(&p).into_string()
    }

    fn seed_commit(cat: &Catalog, hash: &str, git_root: &str) {
        cat.conn
            .execute(
                "INSERT INTO commits (hash, git_root) VALUES (?1, ?2)",
                params![hash, git_root],
            )
            .unwrap();
    }

    #[test]
    fn check_backslash_finds_byte_position() {
        let v = check_backslash("a1", "C:/foo\\bar.md", "backslash_in_abs_path").unwrap();
        assert_eq!(v.check, "backslash_in_abs_path");
        assert_eq!(v.artifact_id.as_deref(), Some("a1"));
        assert_eq!(v.path, "C:/foo\\bar.md");
        assert!(v.detail.contains("position 6"));
    }

    #[test]
    fn check_backslash_skips_clean_path() {
        assert!(check_backslash("a1", "/home/x/foo.md", "backslash_in_abs_path").is_none());
        assert!(check_backslash("a1", "C:/users/x/foo.md", "backslash_in_abs_path").is_none());
    }

    #[test]
    fn check_ads_colon_exempts_drive_prefix() {
        assert!(check_ads_colon("a1", "C:/Users/marius/foo.md").is_none());
        assert!(check_ads_colon("a1", "/home/marius/foo.md").is_none());
    }

    #[test]
    fn check_ads_colon_exempts_verbatim_prefix_drive_colon() {
        // Windows extended-length ("\\?\") path marker, forward-slash form.
        // std::fs::canonicalize on Windows returns paths in this shape; the
        // drive-letter colon here is legitimate, not an ADS selector, even
        // though it sits at byte 5 rather than byte 1.
        assert!(check_ads_colon("a1", "//?/C:/Users/marius/foo.md").is_none());
    }

    #[test]
    fn check_ads_colon_flags_ads_colon_after_verbatim_prefix() {
        let v = check_ads_colon("a1", "//?/C:/Users/marius/foo.txt:stream").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
        assert!(v.detail.contains("position"));
    }

    #[test]
    fn check_ads_colon_flags_post_drive_colon() {
        let v = check_ads_colon("a1", "C:/foo.txt:stream").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
        assert!(v.detail.contains("position"));
    }

    #[test]
    fn check_ads_colon_flags_colon_without_drive_prefix() {
        // POSIX path with a literal colon would be exotic but legal; treat
        // as suspicious because on a cross-platform catalog it almost
        // always means corruption.
        let v = check_ads_colon("a1", "/home/foo:bar").unwrap();
        assert_eq!(v.check, "ads_colon_in_abs_path");
    }

    #[test]
    fn check_dotdot_segment_flags_only_segment_dotdot() {
        assert!(check_dotdot_segment("a1", "/home/x/../etc").is_some());
        assert!(check_dotdot_segment("a1", "/home/x/..").is_some());
        assert!(check_dotdot_segment("a1", "..").is_some());
        // Filename with two dots is NOT a path escape — must not flag.
        assert!(check_dotdot_segment("a1", "/home/x/foo..bar.md").is_none());
        assert!(check_dotdot_segment("a1", "/home/x/.hidden").is_none());
    }

    #[test]
    fn check_missing_file_for_obviously_absent_path() {
        let v = check_missing_file("a1", "/nonexistent/path/that/will/never/exist.md").unwrap();
        assert_eq!(v.check, "missing_file");
    }

    #[test]
    fn check_abs_path_must_be_absolute_accepts_posix_and_drive() {
        assert!(check_abs_path_must_be_absolute("a1", "/home/x/foo.md").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "/").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "C:/Users/x/foo.md").is_none());
        assert!(check_abs_path_must_be_absolute("a1", "z:/").is_none());
    }

    #[test]
    fn check_abs_path_must_be_absolute_flags_relative() {
        let v = check_abs_path_must_be_absolute("a1", "docs/foo.md").unwrap();
        assert_eq!(v.check, "abs_path_must_be_absolute");
        assert_eq!(v.path, "docs/foo.md");
        assert!(v.detail.contains("relative"));

        // Relative with drive-shape but missing colon at pos 1 — still wrong
        assert!(check_abs_path_must_be_absolute("a1", "Cusers/foo.md").is_some());
        // Empty string is not absolute (no leading slash)
        assert!(check_abs_path_must_be_absolute("a1", "").is_some());
    }

    /// BL-23 / `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`.
    ///
    /// `ec9e63d0` stopped `artifact(move)` creating this drift. The population it
    /// left behind was total: all 78 unique `^id:` values in `docs/issues/archive/`
    /// resolved to nothing.
    ///
    /// The three `None` rows are each load-bearing, not padding — they are the ways
    /// a naive implementation over-reports:
    /// a file with **no** `id:` asserts nothing false (and stamping one would newly
    /// subject a prose tracker to the librarian guard); a file with no frontmatter
    /// at all is not an artifact's business; and a **missing** file already has its
    /// own finding in `check_missing_file`, so reporting it twice would inflate the
    /// count on exactly the rows a repair cannot help.
    #[test]
    fn check_frontmatter_id_flags_only_an_id_that_is_present_and_wrong() {
        let tmp = tempfile::tempdir().unwrap();
        let write = |name: &str, body: &str| -> String {
            let p = tmp.path().join(name);
            std::fs::write(&p, body).unwrap();
            crate::util::fs::RepoPath::from_path(&p).into_string()
        };

        let stale = write(
            "stale.md",
            "---\nid: aaaaaaaaaaaaaaaa\nkind: bug\n---\n# x\n",
        );
        let v = check_frontmatter_id_matches_catalog("bbbbbbbbbbbbbbbb", &stale)
            .expect("a present-but-wrong id must be flagged");
        assert_eq!(v.check, "frontmatter_id_mismatch");
        assert!(
            v.detail.contains("aaaaaaaaaaaaaaaa") && v.detail.contains("bbbbbbbbbbbbbbbb"),
            "the detail must name both ids so the finding is actionable, got: {}",
            v.detail
        );

        for (label, path) in [
            (
                "id already correct",
                write("ok.md", "---\nid: bbbbbbbbbbbbbbbb\nkind: bug\n---\n# x\n"),
            ),
            (
                "no id — asserts nothing false",
                write("noid.md", "---\nkind: tracker\n---\n# x\n"),
            ),
            ("no frontmatter at all", write("plain.md", "# just a doc\n")),
            (
                "missing file — check_missing_file owns that finding",
                crate::util::fs::RepoPath::from_path(&tmp.path().join("gone.md")).into_string(),
            ),
        ] {
            assert!(
                check_frontmatter_id_matches_catalog("bbbbbbbbbbbbbbbb", &path).is_none(),
                "{label}: must not be flagged"
            );
        }
    }

    /// The discriminating pair the check could not tell apart. Both files declare an
    /// `id:` differing from the catalog row, so both were filed as
    /// `frontmatter_id_mismatch` and both readers were told the same story — *"a move
    /// re-keys the row and this file kept the id it was moved away from"*. True of the
    /// first. **False of the second:** `ADR-{NUMBER}` was never a catalog id, so no move
    /// ever produced it, and a reader following that detail goes hunting for a commit
    /// that does not exist.
    ///
    /// Measured 2026-08-18 on the live catalog: **3 of 6** instances were the second
    /// kind — two ADR/FDR template placeholders and a `meetings-reranker` slug.
    ///
    /// The **quoted** fixture is deliberate, not decoration. `is_librarian_id` strips
    /// matching quotes because a quoted id is 18 characters and once failed a raw length
    /// test, leaving 15 of `docs/trackers/` unguarded (BL-33). Re-deriving the shape test
    /// here with a hand-rolled 16-hex regex would have reintroduced exactly that defect,
    /// so this pins that a quoted-but-stale id still reads as a catalog id.
    #[test]
    fn a_declared_value_that_was_never_a_catalog_id_is_a_different_finding() {
        let tmp = tempfile::tempdir().unwrap();
        let write = |name: &str, body: &str| -> String {
            let p = tmp.path().join(name);
            std::fs::write(&p, body).unwrap();
            crate::util::fs::RepoPath::from_path(&p).into_string()
        };

        let quoted_stale = write(
            "quoted.md",
            "---\nid: 'aaaaaaaaaaaaaaaa'\nkind: bug\n---\n# x\n",
        );
        let v = check_frontmatter_id_matches_catalog("bbbbbbbbbbbbbbbb", &quoted_stale)
            .expect("a quoted-but-stale catalog id is still a stale move");
        assert_eq!(
            v.check, "frontmatter_id_mismatch",
            "quoting is a property of the last writer, never of the artifact"
        );

        let placeholder = write(
            "adr-template.md",
            "---\nid: ADR-{NUMBER}\nkind: adr\n---\n# x\n",
        );
        let v = check_frontmatter_id_matches_catalog("bbbbbbbbbbbbbbbb", &placeholder)
            .expect("a non-id value still differs from the row and is worth reporting");
        assert_eq!(
            v.check, "frontmatter_id_is_not_a_catalog_id",
            "a template placeholder is not a stale move and must not be filed as one"
        );
        assert!(
            !v.detail.contains("moved away from"),
            "the detail must not assert a move that never happened: {}",
            v.detail
        );
    }

    /// The sweep, end to end through `call`: default scan reports, dry-run previews
    /// without touching disk, `confirm=true` repairs every stale file in one pass.
    ///
    /// The `c.md` and `d.md` rows are the ones that make this test able to fail for
    /// the right reason — a sweep that rewrote every file would repair the two stale
    /// ones and pass a weaker assertion while silently stamping an id onto `d.md`.
    #[tokio::test]
    async fn repair_frontmatter_id_sweeps_the_stale_and_leaves_everything_else() {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::util::fs::RepoPath::from_path(tmp.path()).into_string();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();

        let seed_file = |name: &str, fm_id: Option<&str>| -> String {
            let body = match fm_id {
                Some(f) => format!("---\nid: {f}\nkind: bug\nstatus: fixed\n---\n\n# {name}\n"),
                None => format!("---\nkind: tracker\nstatus: active\n---\n\n# {name}\n"),
            };
            std::fs::write(tmp.path().join("docs").join(name), body).unwrap();
            format!("{root}/docs/{name}")
        };
        let read =
            |name: &str| std::fs::read_to_string(tmp.path().join("docs").join(name)).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(
            &cat,
            "1111111111111111",
            &seed_file("a.md", Some("dead111111111111")),
        );
        seed_artifact(
            &cat,
            "2222222222222222",
            &seed_file("b.md", Some("dead222222222222")),
        );
        seed_artifact(
            &cat,
            "3333333333333333",
            &seed_file("c.md", Some("3333333333333333")),
        );
        seed_artifact(&cat, "4444444444444444", &seed_file("d.md", None));
        let ctx = TestToolContextBuilder::new(cat).build();

        // Default scan: reports, mutates nothing.
        let scan = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            scan["summary"]["by_check"]["frontmatter_id_mismatch"], 2,
            "the check must run in the DEFAULT scan, not only under fix=. Got: {}",
            scan["summary"]["by_check"]
        );

        // Dry run: previews, still mutates nothing. `root` scopes the sweep — the
        // catalog is machine-global, so an unscoped write would reach other repos.
        let scope = tmp.path().to_str().unwrap();
        let dry = call(
            &ctx,
            json!({ "fix": "repair_frontmatter_id", "root": scope }),
        )
        .await
        .unwrap();
        assert_eq!(dry["mode"], "dry_run");
        assert_eq!(dry["totals"]["files"], 2);
        assert!(
            read("a.md").contains("dead111111111111"),
            "a dry run must not write"
        );

        // Confirm: repairs both, in one sweep.
        let applied = call(
            &ctx,
            json!({ "fix": "repair_frontmatter_id", "root": scope, "confirm": true }),
        )
        .await
        .unwrap();
        assert_eq!(applied["mode"], "applied");
        assert_eq!(applied["totals"]["files"], 2);

        // Assert through the parser, not on a substring: `frontmatter::write`
        // conservatively quotes any scalar YAML 1.1 could reinterpret, and these
        // all-digit fixture ids come back as `id: '1111111111111111'`. A substring
        // assertion would fail on correct output — as it did on the first run.
        let declared = |name: &str| -> Option<String> {
            let text = read(name);
            crate::librarian::frontmatter::parse(&text)
                .unwrap()
                .0
                .and_then(|fm| fm.id)
        };
        assert_eq!(declared("a.md").as_deref(), Some("1111111111111111"));
        assert_eq!(declared("b.md").as_deref(), Some("2222222222222222"));
        assert!(
            read("a.md").contains("# a.md"),
            "the body must survive the repair"
        );
        assert_eq!(
            declared("c.md").as_deref(),
            Some("3333333333333333"),
            "an already-correct file must be left alone"
        );

        let d = read("d.md");
        let (fm, _) = crate::librarian::frontmatter::parse(&d).unwrap();
        assert!(
            fm.expect("frontmatter preserved").id.is_none(),
            "a file with no id must not gain one — stamping it would newly subject a \
             prose tracker to the librarian guard. Got: {d:?}"
        );

        // Idempotent: a second sweep finds nothing left to do.
        let again = call(
            &ctx,
            json!({ "fix": "repair_frontmatter_id", "root": scope, "confirm": true }),
        )
        .await
        .unwrap();
        assert_eq!(again["totals"]["files"], 0);
    }

    /// The sweep WRITES, and the catalog is machine-global.
    ///
    /// Caught by running the dry-run for real before confirming it: on this machine
    /// it listed **207 files across five unrelated repositories** — backend-kotlin,
    /// eduplanner-ui, two southpole projects, claude-plugins — of which only ~90
    /// were codescout's. An unscoped `confirm=true` would have rewritten tracked
    /// files in repos the caller does not have open, in one call, silently.
    ///
    /// `prune_missing` and `rehome` are both root-scoped for exactly this reason.
    /// "Sweep-all" means *don't make me name each artifact*, not *cross into other
    /// repositories*.
    #[tokio::test]
    async fn repair_frontmatter_id_never_writes_outside_the_scoped_root() {
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let mk = |dir: &std::path::Path, name: &str| -> String {
            let p = dir.join(name);
            std::fs::write(&p, "---\nid: deaddeaddeaddead\nkind: bug\n---\n\n# x\n").unwrap();
            crate::util::fs::RepoPath::from_path(&p).into_string()
        };
        let in_path = mk(inside.path(), "in.md");
        let out_path = mk(outside.path(), "out.md");

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "1111111111111111", &in_path);
        seed_artifact(&cat, "2222222222222222", &out_path);
        let ctx = TestToolContextBuilder::new(cat).build();

        let applied = call(
            &ctx,
            json!({
                "fix": "repair_frontmatter_id",
                "root": inside.path().to_str().unwrap(),
                "confirm": true
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            applied["totals"]["files"], 1,
            "only the in-scope file may be rewritten, got: {applied}"
        );

        let declared = |p: std::path::PathBuf| -> Option<String> {
            let t = std::fs::read_to_string(p).unwrap();
            crate::librarian::frontmatter::parse(&t)
                .unwrap()
                .0
                .and_then(|fm| fm.id)
        };
        assert_eq!(
            declared(inside.path().join("in.md")).as_deref(),
            Some("1111111111111111")
        );
        assert_eq!(
            declared(outside.path().join("out.md")).as_deref(),
            Some("deaddeaddeaddead"),
            "a file in another repository must be left exactly as it was"
        );
    }

    /// The destructive half, and the one with teeth.
    ///
    /// `fix=repair_frontmatter_id` filtered its rows by PATH containment and nothing
    /// else, so `confirm=true` would splice a template's `id: ADR-{NUMBER}` to a 16-hex
    /// id — destroying the placeholder that makes it a template, and (because a 16-hex
    /// `id:` is one of the three things `librarian_guard` keys on) making every copy of
    /// it guard-refused for `edit_markdown`.
    ///
    /// That is the **same harm** the existing no-`id:` abstention prevents, and its
    /// stated reason covers this case verbatim: stamping an id newly subjects the file to
    /// the guard. The old code simply tested the wrong predicate — "is there an `id:`
    /// value?" rather than "is there a CATALOG id?". Note a placeholder-bearing template
    /// is **unguarded today**, since `is_librarian_id("ADR-{NUMBER}")` is false, so this
    /// write would have created the very condition the abstention exists to avoid.
    ///
    /// Isolated from `repair_frontmatter_id_sweeps_the_stale_and_leaves_everything_else`
    /// on purpose: this rejects one class, so its fixture must be the only reason
    /// anything is left alone. And the stale row is a **positive control** — without it
    /// a repair that silently did nothing would pass.
    #[tokio::test]
    async fn repair_frontmatter_id_never_rewrites_a_value_that_was_never_a_catalog_id() {
        let tmp = tempfile::tempdir().unwrap();
        let root = crate::util::fs::RepoPath::from_path(tmp.path()).into_string();
        let seed = |name: &str, fm_id: &str| -> String {
            std::fs::write(
                tmp.path().join(name),
                format!("---\nid: {fm_id}\nkind: adr\n---\n\n# {name}\n"),
            )
            .unwrap();
            format!("{root}/{name}")
        };
        let read = |name: &str| std::fs::read_to_string(tmp.path().join(name)).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(
            &cat,
            "1111111111111111",
            &seed("stale.md", "dead111111111111"),
        );
        seed_artifact(
            &cat,
            "2222222222222222",
            &seed("adr-template.md", "ADR-{NUMBER}"),
        );
        let ctx = TestToolContextBuilder::new(cat).build();

        let applied = call(
            &ctx,
            json!({ "fix": "repair_frontmatter_id", "root": root, "confirm": true }),
        )
        .await
        .unwrap();

        assert_eq!(
            applied["totals"]["files"], 1,
            "only the genuinely stale catalog id may be rewritten, got: {applied}"
        );
        // Read through the parser, never by string match: it is authoritative about
        // what YAML actually sees, including whatever quoting the splice emitted.
        // Asserting a spelling the writer does not produce is its own defect.
        let declared = |name: &str| -> Option<String> {
            crate::librarian::frontmatter::parse(&read(name))
                .unwrap()
                .0
                .and_then(|fm| fm.id)
        };
        assert_eq!(
            declared("stale.md").as_deref(),
            Some("1111111111111111"),
            "positive control: the stale one MUST still be repaired, or this test passes \
                 by the sweep doing nothing at all. File: {}",
            read("stale.md")
        );
        assert_eq!(
            declared("adr-template.md").as_deref(),
            Some("ADR-{NUMBER}"),
            "the placeholder must survive untouched. File: {}",
            read("adr-template.md")
        );
    }

    /// A worktree shadow declares its MAIN twin's id, and that is the overlay working.
    ///
    /// The fixture asserts up front that the two ids actually differ — otherwise the
    /// plain `declared == id` early return would abstain and this test would prove
    /// nothing about the worktree branch at all.
    #[test]
    fn frontmatter_id_mismatch_abstains_for_a_worktree_shadow_declaring_its_main_twin() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        std::fs::create_dir_all(worktree_root.join("docs")).unwrap();
        let shadow = worktree_root.join("docs/plan.md");
        let twin_id = crate::librarian::ids::artifact_id_from_abs(&main_root.join("docs/plan.md"));
        std::fs::write(
            &shadow,
            format!("---\nid: {twin_id}\nkind: plan\n---\n\n# plan\n"),
        )
        .unwrap();

        let shadow_str = crate::util::fs::RepoPath::from_path(&shadow).into_string();
        let row_id = crate::librarian::ids::artifact_id_from_abs(&shadow);
        assert_ne!(
            row_id, twin_id,
            "fixture guard: if these matched, the plain equality return would abstain and \
             the worktree branch would never execute"
        );

        assert!(
            check_frontmatter_id_matches_catalog(&row_id, &shadow_str).is_none(),
            "a shadow naming its main twin is fork-on-first-write, not a move"
        );
    }

    /// The discriminator. Without it, "abstain for anything inside a worktree" passes the
    /// test above — and would silence genuine post-move drift in every worktree.
    #[test]
    fn frontmatter_id_mismatch_still_fires_for_a_worktree_file_declaring_an_unrelated_id() {
        let (_tmp, _main_root, worktree_root) = make_worktree_fixture();
        std::fs::create_dir_all(worktree_root.join("docs")).unwrap();
        let f = worktree_root.join("docs/plan.md");
        std::fs::write(&f, "---\nid: dead111111111111\nkind: plan\n---\n\n# plan\n").unwrap();
        let f_str = crate::util::fs::RepoPath::from_path(&f).into_string();
        let row_id = crate::librarian::ids::artifact_id_from_abs(&f);

        let v = check_frontmatter_id_matches_catalog(&row_id, &f_str)
            .expect("being inside a worktree does not excuse an unrelated stale id");
        assert_eq!(v.check, "frontmatter_id_mismatch");
    }

    /// The destructive half, end to end.
    ///
    /// `make_worktree_fixture` puts the worktree UNDER the main root, which is the real
    /// layout and the whole reason the old code reached it: `containing_root` is a path
    /// test, and a worktree passes it. So the scope filter cannot be what keeps the shadow
    /// out here — only the abstention can.
    ///
    /// The stale row in the main checkout is a positive control. Without it a sweep that
    /// silently repaired nothing would pass.
    #[tokio::test]
    async fn repair_frontmatter_id_leaves_a_worktree_shadow_alone_and_still_sweeps_the_stale_row() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        std::fs::create_dir_all(main_root.join("docs")).unwrap();
        std::fs::create_dir_all(worktree_root.join("docs")).unwrap();

        let stale = main_root.join("docs/stale.md");
        std::fs::write(
            &stale,
            "---\nid: dead111111111111\nkind: adr\n---\n\n# stale\n",
        )
        .unwrap();

        let shadow = worktree_root.join("docs/plan.md");
        let twin_id = crate::librarian::ids::artifact_id_from_abs(&main_root.join("docs/plan.md"));
        std::fs::write(
            &shadow,
            format!("---\nid: {twin_id}\nkind: plan\n---\n\n# plan\n"),
        )
        .unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(
            &cat,
            "1111111111111111",
            &crate::util::fs::RepoPath::from_path(&stale).into_string(),
        );
        seed_artifact(
            &cat,
            &crate::librarian::ids::artifact_id_from_abs(&shadow),
            &crate::util::fs::RepoPath::from_path(&shadow).into_string(),
        );
        let ctx = TestToolContextBuilder::new(cat).build();

        let applied = call(
            &ctx,
            json!({
                "fix": "repair_frontmatter_id",
                "root": crate::util::fs::RepoPath::from_path(&main_root).into_string(),
                "confirm": true,
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            applied["totals"]["files"], 1,
            "the shadow is inside the scope root by path, so only the abstention excludes \
             it: {applied}"
        );

        let declared = |p: &std::path::Path| -> Option<String> {
            crate::librarian::frontmatter::parse(&std::fs::read_to_string(p).unwrap())
                .unwrap()
                .0
                .and_then(|fm| fm.id)
        };
        assert_eq!(
            declared(&stale).as_deref(),
            Some("1111111111111111"),
            "positive control: without this the test passes on a sweep that did nothing"
        );
        assert_eq!(
            declared(&shadow).as_deref(),
            Some(twin_id.as_str()),
            "the shadow must be untouched — a write here lands in another session's live \
             working tree, and ships a worktree-derived id if that session commits"
        );
    }

    // ---- terminal_status_with_caveat -----------------------------------------------

    /// A bug file on disk plus its catalog row, since the check reads both: SQL narrows
    /// on `kind`/`status`, the caveat itself lives in frontmatter `extra`.
    fn seed_bug(
        cat: &Catalog,
        dir: &std::path::Path,
        name: &str,
        status: &str,
        unverified: Option<&str>,
    ) {
        let caveat = match unverified {
            Some(v) => format!("unverified: \"{v}\"\n"),
            None => String::new(),
        };
        let path = dir.join(format!("{name}.md"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("---\nkind: bug\nstatus: {status}\n{caveat}---\n\n# BUG: {name}\n"),
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new(name)
            .with_abs_path(&path)
            .with_kind("bug")
            .with_status(status)
            .build();
        art_upsert(cat, &row).unwrap();
    }

    /// Seed an artifact whose frontmatter carries `expects_augmentation: <decl>` (omitted
    /// when `None`), optionally with an augmentation row attached.
    fn seed_declared(
        cat: &Catalog,
        dir: &std::path::Path,
        name: &str,
        decl: Option<&str>,
        augmented: bool,
    ) {
        let line = match decl {
            Some(v) => format!("expects_augmentation: {v}\n"),
            None => String::new(),
        };
        let path = dir.join(format!("{name}.md"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("---\nkind: tracker\nstatus: active\n{line}---\n\n# {name}\n"),
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new(name)
            .with_abs_path(&path)
            .with_kind("tracker")
            .build();
        art_upsert(cat, &row).unwrap();
        if augmented {
            crate::librarian::catalog::augmentation::upsert(
                cat,
                &crate::librarian::catalog::augmentation::AugmentationRow {
                    artifact_id: name.to_string(),
                    prompt: "p".to_string(),
                    params: "{}".to_string(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: "2026-01-01T00:00:00.000Z".to_string(),
                    updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                    render_template: None,
                    params_schema: None,
                    append_mode: false,
                    history_cap: None,
                    entry_collection: None,
                    refreshed_at_commit: None,
                },
            )
            .unwrap();
        }
    }

    /// The decoys are the test. An artifact that declares AND has one is the healthy
    /// state; an artifact that declares nothing is the ordinary case and vastly the
    /// commonest — without both, "report every unaugmented artifact" passes, and that is
    /// 4,000-plus findings.
    #[test]
    fn augmentation_declared_but_absent_reports_only_the_declared_and_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_declared(&cat, tmp.path(), "lost", Some("true"), false);
        seed_declared(&cat, tmp.path(), "healthy", Some("true"), true);
        seed_declared(&cat, tmp.path(), "ordinary", None, false);

        let v = scan_augmentation_declared_but_absent(&cat.conn).unwrap();

        assert_eq!(
            v.len(),
            1,
            "only the declared-and-missing artifact may fire: {v:#?}"
        );
        assert_eq!(v[0].artifact_id.as_deref(), Some("lost"));
        assert_eq!(v[0].check, "augmentation_declared_but_absent");
        assert!(
            v[0].detail.contains("reindex"),
            "the detail must say reindex cannot rebuild it — otherwise the reader's first \
             move is the one thing that will not work, and it will report success: {}",
            v[0].detail
        );
    }

    /// A quoting accident must not silently disarm the declaration, `false` must not arm
    /// it, and a value that means nothing must be REPORTED rather than skipped. Three
    /// states, because a truthiness helper that is wrong in any of them produces a gate
    /// whose author believes they are covered.
    ///
    /// Filtered by check name on purpose: the scan emits two, and collecting every
    /// violation regardless would let a finding under one name silently satisfy an
    /// assertion about the other. That is how this test read before the third state
    /// existed, and it is why it failed on a change it should have passed.
    #[test]
    fn augmentation_declaration_reads_yaml_truthiness_three_ways() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_declared(&cat, tmp.path(), "bare", Some("true"), false);
        seed_declared(&cat, tmp.path(), "quoted", Some("\"true\""), false);
        seed_declared(&cat, tmp.path(), "yes", Some("yes"), false);
        seed_declared(&cat, tmp.path(), "off", Some("false"), false);
        seed_declared(&cat, tmp.path(), "empty", Some("\"\""), false);

        let all = scan_augmentation_declared_but_absent(&cat.conn).unwrap();
        let by_check = |name: &str| -> std::collections::BTreeSet<String> {
            all.iter()
                .filter(|v| v.check == name)
                .filter_map(|v| v.artifact_id.clone())
                .collect()
        };
        let set = |ids: &[&str]| -> std::collections::BTreeSet<String> {
            ids.iter().map(|s| s.to_string()).collect()
        };

        assert_eq!(
            by_check("augmentation_declared_but_absent"),
            set(&["bare", "quoted", "yes"]),
            "true/\"true\"/yes must arm the declaration; false and \"\" must not"
        );
        assert_eq!(
            by_check("augmentation_declaration_unparseable"),
            set(&["empty"]),
            "an empty value declares nothing while looking like a declaration — report it, \
             the same way validity_unparseable does, rather than skipping the line"
        );
        // `false` is a considered no. It must fire NOTHING — not the absent check, not the
        // unparseable one — or the reader learns to ignore this family of findings.
        assert!(
            !all.iter().any(|v| v.artifact_id.as_deref() == Some("off")),
            "an explicit `false` must produce no finding at all: {:?}",
            all.iter()
                .map(|v| (&v.check, &v.artifact_id))
                .collect::<Vec<_>>()
        );
    }

    /// A declared sidecar that EXISTS is one command from repaired; one that does not is a
    /// different problem needing a different machine. The check must not collapse them into
    /// one detail, because the remedy is the discriminator and a reader acts on the remedy.
    ///
    /// Both artifacts here are `augmentation_declared_but_absent` — same check, same
    /// severity, opposite advice. That is the whole point of the split, so the assertions
    /// are on the DETAIL rather than on which check fired.
    #[test]
    fn a_present_sidecar_and_a_missing_one_get_opposite_advice() {
        let tmp = tempfile::tempdir().unwrap();
        // `lookup_git_root` walks up for `.git`, and the sidecar path is repo-relative, so
        // without this marker the recoverable branch cannot resolve and both artifacts
        // would take the same arm — the exact collapse this test exists to forbid.
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        seed_declared(
            &cat,
            tmp.path(),
            "has_sidecar",
            Some("docs/augmentations/has_sidecar.yaml"),
            false,
        );
        crate::librarian::augmentation_sidecar::write(
            &tmp.path().join("docs/augmentations/has_sidecar.yaml"),
            &crate::librarian::augmentation_sidecar::AugmentationSidecar {
                schema_version: 1,
                prompt: "recorded".into(),
                entry_collection: Some("rows".into()),
                params_schema: None,
                render_template: None,
                append_mode: false,
                history_cap: None,
            },
        )
        .unwrap();

        seed_declared(
            &cat,
            tmp.path(),
            "no_sidecar",
            Some("docs/augmentations/absent.yaml"),
            false,
        );

        let all = scan_augmentation_declared_but_absent(&cat.conn).unwrap();
        let detail = |id: &str| -> String {
            all.iter()
                .find(|v| v.artifact_id.as_deref() == Some(id))
                .unwrap_or_else(|| panic!("{id} must produce a finding"))
                .detail
                .clone()
        };

        let present = detail("has_sidecar");
        assert!(
            present.contains("reindex"),
            "a present sidecar's remedy is one reindex; the detail must name it: {present}"
        );
        assert!(
            !present.contains("export_augmentations"),
            "a present sidecar needs no export — pointing at another machine is wrong \
             advice here: {present}"
        );

        let missing = detail("no_sidecar");
        assert!(
            missing.contains("export_augmentations"),
            "with no sidecar the shape lives only on another machine; the detail must say \
             so: {missing}"
        );
        assert!(
            missing.contains("cannot help"),
            "and it must say reindex will NOT fix it, or the reader's first move is the \
             one thing that does nothing and reports success: {missing}"
        );
    }

    /// The drift remedy must name the step without which the export it prescribes does
    /// nothing.
    ///
    /// `sidecar_shape_drift` fires ONLY when the sidecar is present — `is_file()` is its
    /// own precondition — and `export_augmentation_sidecars` skips exactly that
    /// population (`declared_already && sidecar_abs.is_file()`). The intersection is
    /// empty, so "re-export it" with no delete step is advice that cannot act on a single
    /// finding this check can emit: it returns `exported: 0` and exits successfully.
    /// `sidecar_unparseable`, emitted by the same scanner for another present-sidecar
    /// case, already says "delete it and re-run".
    ///
    /// **The assertion is on the DELETE instruction, never on the token
    /// `export_augmentations`.** The shipped wrong string contains that token too, so a
    /// presence check on it is monotone under this exact defect and would have passed
    /// throughout — the trap this test exists to avoid, not merely to document.
    #[test]
    fn the_drift_remedy_names_the_delete_step_without_which_the_export_is_a_no_op() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // `scan_sidecar_shape_drift` resolves the sidecar through `lookup_git_root`, so
        // without this marker the artifact takes the `continue` and reports nothing.
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let rel = "docs/augmentations/drifted.yaml";
        seed_declared(&cat, root, "drifted", Some(rel), true);
        // `seed_declared` gives the ROW prompt "p". Committing a different one is the
        // load-bearing detail: it is what makes `drifting_fields` non-empty, and so what
        // makes the check fire at all. Match the two and this test reports nothing.
        let sidecar = root.join(rel);
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        crate::librarian::augmentation_sidecar::write(
            &sidecar,
            &crate::librarian::augmentation_sidecar::AugmentationSidecar {
                schema_version: 1,
                prompt: "a superseded shape".into(),
                entry_collection: None,
                params_schema: None,
                render_template: None,
                append_mode: false,
                history_cap: None,
            },
        )
        .unwrap();

        let found = scan_sidecar_shape_drift(&cat.conn).unwrap();
        let v = found
            .iter()
            .find(|v| v.artifact_id.as_deref() == Some("drifted"))
            .expect("a committed sidecar disagreeing with its row must be reported");
        assert_eq!(v.check, "sidecar_shape_drift", "{:?}", v.detail);

        assert!(
            v.detail.contains("delete"),
            "the catalog-is-right branch must name the delete step. Without it the \
             prescribed export skips this artifact by its own precondition, reports \
             exported: 0, and exits 0 — a clean no-op the reader reads as \"nothing \
             needed doing\": {}",
            v.detail
        );
    }

    /// The measured reason this check is keyed on a declaration rather than on the body.
    ///
    /// Across 4,195 catalogued artifacts on 2026-08-26, 29 mention `[LIVE]` outside fenced
    /// code and 23 of those carry no augmentation — guides, specs, plans and bug files
    /// describing the mechanism, including this check's own bug file. A body-sniffing gate
    /// is 23 false positives and one true one. This pins the property that matters: prose
    /// about the mechanism, however emphatic, is not a declaration.
    #[test]
    fn a_body_that_merely_describes_live_blocks_is_not_a_declaration() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let path = tmp.path().join("guide.md");
        std::fs::write(
            &path,
            "---\nkind: tracker\nstatus: active\n---\n\n# Guide\n\nAugmented artifacts \
             surface as a `[LIVE]` blockquote in librarian(action=\"context\"); the \
             `[LIVE]` header carries the refresh count. Do not edit the [LIVE] table by \
             hand.\n",
        )
        .unwrap();
        art_upsert(
            &cat,
            &TestArtifactRowBuilder::new("guide")
                .with_abs_path(&path)
                .with_kind("tracker")
                .build(),
        )
        .unwrap();

        assert!(
            scan_augmentation_declared_but_absent(&cat.conn)
                .unwrap()
                .is_empty(),
            "a doc describing [LIVE] blocks must stay silent — 23 of the 29 real-corpus \
             mentions are exactly this shape"
        );
    }

    /// The population the canonical triage query hides by construction.
    ///
    /// The two decoys are the test: an `open` bug carrying a caveat must NOT fire (the
    /// triage query already reaches it, so reporting it is noise), and a `fixed` bug with
    /// no caveat must NOT fire (that is the ordinary, healthy terminal state). Without
    /// them, "report every bug file" passes.
    #[tokio::test]
    async fn terminal_status_with_caveat_reports_only_terminal_records_that_carry_one() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_bug(
            &cat,
            tmp.path(),
            "caveated",
            "fixed",
            Some("no regression test"),
        );
        seed_bug(
            &cat,
            tmp.path(),
            "still-open",
            "open",
            Some("also caveated"),
        );
        seed_bug(&cat, tmp.path(), "clean", "fixed", None);

        let v = scan_terminal_status_with_caveat(&cat.conn).unwrap();

        assert_eq!(
            v.len(),
            1,
            "only the terminal-AND-caveated record may fire: {v:#?}"
        );
        assert_eq!(v[0].artifact_id.as_deref(), Some("caveated"));
        assert!(
            v[0].detail.contains("no regression test"),
            "the caveat must be quoted, or the reader has to open every file: {}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("fixed"),
            "and the status, so the reader knows which query missed it: {}",
            v[0].detail
        );
    }

    /// `mitigated` and `wontfix` are terminal too. Naming only `fixed` in the SQL would
    /// silently exempt two thirds of the vocabulary — and `mitigated` is the status of the
    /// record that motivated the whole `unverified:` convention.
    #[tokio::test]
    async fn terminal_status_with_caveat_covers_every_terminal_status() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_bug(&cat, tmp.path(), "a", "fixed", Some("x"));
        seed_bug(&cat, tmp.path(), "b", "mitigated", Some("x"));
        seed_bug(&cat, tmp.path(), "c", "wontfix", Some("x"));

        let v = scan_terminal_status_with_caveat(&cat.conn).unwrap();
        assert_eq!(v.len(), 3, "all three terminal statuses count: {v:#?}");
    }

    /// The convention says never add the field empty, "because presence is what a query
    /// filters on". A record that does so anyway must read as absent, not as a caveat with
    /// no content — otherwise the report grows entries that say nothing.
    #[tokio::test]
    async fn terminal_status_with_caveat_treats_an_empty_field_as_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_bug(&cat, tmp.path(), "empty", "fixed", Some(""));
        seed_bug(&cat, tmp.path(), "blank", "fixed", Some("   "));

        assert!(
            scan_terminal_status_with_caveat(&cat.conn)
                .unwrap()
                .is_empty(),
            "an empty or whitespace-only caveat is not a caveat"
        );
    }

    /// Archived files are included deliberately: `docs/issues/archive/` is where terminal
    /// records go and nothing re-reads it, so an archived caveat is MORE hidden than a live
    /// one, not less.
    #[tokio::test]
    async fn terminal_status_with_caveat_reaches_an_archived_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_bug(
            &cat,
            &tmp.path().join("archive"),
            "archived-one",
            "fixed",
            Some("widened another open bug"),
        );

        let v = scan_terminal_status_with_caveat(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "archiving must not silence a caveat: {v:#?}");
        assert!(v[0].path.contains("archive"));
    }

    /// The caveat is prose and routinely non-ASCII (em dashes, arrows). Truncating it by
    /// bytes would panic on a split character; this fixture puts multi-byte characters
    /// exactly where a byte-wise cut would land.
    #[tokio::test]
    async fn a_long_caveat_is_truncated_without_splitting_a_character() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let long: String = "—é→ ".repeat(120);
        seed_bug(&cat, tmp.path(), "verbose", "fixed", Some(&long));

        let v = scan_terminal_status_with_caveat(&cat.conn).unwrap();
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains('…'),
            "an over-long caveat is elided rather than dumped whole: {}",
            v[0].detail
        );
    }

    // ---- archived_fix_sha_unresolvable ----------------------------------------------

    /// A real git repo with one real commit, so resolution is genuine rather than mocked.
    fn git_fixture_with_commit() -> (tempfile::TempDir, std::path::PathBuf, String) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let sha = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap()
            .to_string();
        (tmp, root, sha)
    }

    fn seed_archived_bug(cat: &Catalog, root: &std::path::Path, name: &str, body: &str) {
        let dir = root.join("docs").join("issues").join("archive");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.md"));
        std::fs::write(
            &path,
            format!("---\nkind: bug\nstatus: fixed\n---\n\n# BUG: {name}\n\n{body}\n"),
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new(name)
            .with_abs_path(&path)
            .with_kind("bug")
            .with_status("fixed")
            .build();
        art_upsert(cat, &row).unwrap();
    }

    fn ctx_rooted_at(cat: Catalog, root: &std::path::Path) -> ToolContext {
        let cp = std::sync::Arc::new(crate::librarian::current_project::CurrentProject {
            abs_path: root.to_path_buf(),
            git_root: root.to_path_buf(),
            main_root: None,
            umbrella: None,
        });
        TestToolContextBuilder::new(cat)
            .with_current_project(cp)
            .build()
    }

    /// The live commit is the control: without it, a check that flagged every declared SHA
    /// would pass.
    #[tokio::test]
    async fn archived_fix_sha_reports_a_dead_pointer_and_carries_the_patch_id_remedy() {
        let (_tmp, root, live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &root,
            "alive",
            &format!(
                "## Fix provenance\n\n- **SHA:** `{}` (experiments-only)\n",
                &live[..8]
            ),
        );
        seed_archived_bug(
            &cat,
            &root,
            "dead",
            "## Fix provenance\n\n- **SHA:** `deadbee` (experiments-only)\n\
             - **patch-id:** `c5128d873990b049ce956c695a3899750d7b3f08`\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, health) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };

        assert_eq!(v.len(), 1, "only the dead pointer fires: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("dead"));
        assert!(v[0].detail.contains("deadbee"), "names the dead SHA");
        assert!(
            v[0].detail.contains("c5128d873990b049"),
            "and carries the patch-id, so the remedy travels WITH the finding rather than \
             being looked up afterwards: {}",
            v[0].detail
        );
        assert_eq!(health["scanned"], 2);
        assert_eq!(health["unresolvable"], 1);
    }

    /// A dead pointer with nothing to recover it by is a strictly worse finding, and must
    /// not silently reuse the patch-id wording.
    #[tokio::test]
    async fn a_dead_pointer_with_no_patch_id_says_there_is_no_way_back() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &root,
            "orphan",
            "## Fix provenance\n\n- **SHA:** `deadbee` (experiments-only)\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, _) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1);
        assert!(
            v[0].detail.contains("No patch-id"),
            "the absence must be stated, not glossed: {}",
            v[0].detail
        );
    }

    /// The design decision, pinned. Freeform prose naming commits is SKIPPED, not swept.
    ///
    /// Both decoys are real shapes from `docs/issues/archive/`: a reproduction commit, and
    /// a suspect the file explicitly exonerates. Sweeping hex would report each as a dead
    /// fix SHA — a confident wrong answer about a commit that was never the fix.
    #[tokio::test]
    async fn archived_fix_sha_skips_freeform_prose_instead_of_sweeping_it_for_hex() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &root,
            "prose",
            "Added `src/bug032_repro.rs` (commit `a45f1bd7`).\n\n\
             Suspected the recent refactor `12707fe`.\n\n\
             1. **Refactor 12707fe is INNOCENT.** Diff confirms it.\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, health) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };

        assert!(
            v.is_empty(),
            "a reproduction commit and an exonerated suspect are not dead fix SHAs: {v:#?}"
        );
        assert_eq!(health["scanned"], 0);
        assert_eq!(
            health["skipped_no_structured_pointer"], 1,
            "and the skip must be COUNTED — a clean result over a fraction of the corpus \
             read as full coverage is the failure this module keeps guarding against"
        );
    }

    /// A SHA is only meaningful inside the repo that minted it. Resolving one against a
    /// sibling repo would report every declared pointer dead at once.
    #[tokio::test]
    async fn archived_fix_sha_ignores_rows_outside_the_active_repo() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let (_other_tmp, other_root, _) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &other_root,
            "elsewhere",
            "## Fix provenance\n\n- **SHA:** `deadbee` (experiments-only)\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, health) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };
        assert!(
            v.is_empty(),
            "another repo's SHA is not ours to resolve: {v:#?}"
        );
        assert_eq!(health["scanned"], 0);
    }

    /// A `<repo>:<sha>` pointer names a commit in a SIBLING repo, and this check is
    /// deliberately scoped to one. Resolving it locally asks git for *the object at path
    /// `<sha>` inside a tree-ish named `<repo>`*, which fails — so a perfectly good pointer
    /// was reported dead, together with a patch-id remedy that cannot work across repos.
    ///
    /// The genuinely dead LOCAL pointer is the control: without it, "skip everything"
    /// passes and the check is silently disabled.
    /// docs/issues/archive/2026-08-26-doctor-reports-a-cross-repo-fix-sha-as-dead.md
    #[tokio::test]
    async fn a_cross_repo_prefixed_pointer_is_skipped_rather_than_reported_dead() {
        let (_tmp, root, live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &root,
            "cross-repo",
            "## Fix provenance\n\n- **SHA:** `codescout-companion:b8ffa8b`\n\
             - **patch-id:** not computable from this repo (fix lives in codescout-companion)\n",
        );
        seed_archived_bug(
            &cat,
            &root,
            "local-alive",
            &format!("## Fix provenance\n\n- **SHA:** `{}`\n", &live[..8]),
        );
        seed_archived_bug(
            &cat,
            &root,
            "local-dead",
            "## Fix provenance\n\n- **SHA:** `deadbee`\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, health) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };

        let fired: Vec<&str> = v.iter().filter_map(|x| x.artifact_id.as_deref()).collect();
        assert_eq!(
            fired,
            vec!["local-dead"],
            "only the dead LOCAL pointer may fire — a cross-repo one is unresolvable here by \
             construction, not rotted: {v:#?}"
        );
        assert_eq!(
            health["skipped_cross_repo_pointer"], 1,
            "the skip must be counted, or a clean result reads as 'every declared fix \
             resolves' when one of them was never checked: {health}"
        );
    }

    // ---- terminal_status_without_fix_anchor -----------------------------------------

    /// A live (non-archived) bug file plus its catalog row. Distinct from
    /// [`seed_archived_bug`] precisely because path is what this check partitions on.
    fn seed_live_bug(
        cat: &Catalog,
        root: &std::path::Path,
        name: &str,
        status: &str,
        extra_fm: &str,
        body: &str,
    ) {
        let dir = root.join("docs").join("issues");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.md"));
        std::fs::write(
            &path,
            format!("---\nkind: bug\nstatus: {status}\n{extra_fm}---\n\n# BUG: {name}\n\n{body}\n"),
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new(name)
            .with_abs_path(&path)
            .with_kind("bug")
            .with_status(status)
            .build();
        art_upsert(cat, &row).unwrap();
    }

    /// The anchored file is the control: without it a check that fired on every terminal
    /// record would pass this test.
    #[tokio::test]
    async fn terminal_status_without_fix_anchor_fires_only_where_no_pointer_is_declared() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "bare",
            "fixed",
            "",
            "Nothing here names a commit.",
        );
        seed_live_bug(
            &cat,
            &root,
            "anchored",
            "fixed",
            "",
            "## Fix provenance\n\n- **SHA:** `abc1234`\n- **patch-id:** `deadbeefcafe`\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_terminal_status_without_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "only the unanchored record fires: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("bare"));
    }

    /// The misleading case, and the reason this check says more than "no SHA found".
    ///
    /// The fixture is the real shape measured 2026-08-19: the only hash in the file is the
    /// commit the bug was OBSERVED at, sitting in an Environment line. A reader scanning for
    /// provenance finds it and stops.
    #[tokio::test]
    async fn terminal_status_without_fix_anchor_names_the_hash_that_makes_it_read_as_anchored() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "decoy",
            "fixed",
            "",
            "## Environment\n\ncodescout at `6ce49487` (experiments), rust-analyzer pinned.\n",
        );
        seed_live_bug(&cat, &root, "silent", "fixed", "", "No hashes at all.\n");
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_terminal_status_without_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 2);
        let decoy = v.iter().find(|x| x.path.contains("decoy")).unwrap();
        let silent = v.iter().find(|x| x.path.contains("silent")).unwrap();
        assert!(
            decoy.detail.contains("6ce49487") && decoy.detail.contains("READS as anchored"),
            "the decoy hash must be named, and named as a decoy: {}",
            decoy.detail
        );
        assert!(
            !silent.detail.contains("READS as anchored"),
            "a file with no hashes is plainly unanchored and must NOT borrow the stronger \
             wording — the two findings differ in what a reader should do: {}",
            silent.detail
        );
    }

    /// 297 of 355 archived files declare no pointer. Reporting them would bury the 9 live
    /// records where the remedy is still owed, against records the guide calls stale
    /// instructions rather than debt.
    #[tokio::test]
    async fn terminal_status_without_fix_anchor_leaves_archived_records_alone() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(&cat, &root, "old", "No provenance section anywhere.");
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_terminal_status_without_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert!(v.is_empty(), "archived records are out of scope: {v:#?}");
    }

    /// Without a way to say "this mitigation had no commit", the check nags those records
    /// forever and gets silenced wholesale. An EMPTY declaration is absent, matching
    /// `unverified:` — presence is what a query reads.
    #[tokio::test]
    async fn terminal_status_without_fix_anchor_is_discharged_by_a_non_empty_declaration() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "declared",
            "mitigated",
            "no_fix_commit: \"mitigation was a doc note; nothing was committed\"\n",
            "body",
        );
        seed_live_bug(
            &cat,
            &root,
            "hollow",
            "mitigated",
            "no_fix_commit: \"\"\n",
            "body",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_terminal_status_without_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "only the empty declaration still owes: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("hollow"));
    }

    /// `wontfix` means nothing was fixed, so there is no commit to point at. Including it
    /// would make the check demand an anchor that cannot exist.
    #[tokio::test]
    async fn terminal_status_without_fix_anchor_ignores_wontfix() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "declined", "wontfix", "", "Not doing this.");
        seed_live_bug(&cat, &root, "done", "mitigated", "", "Mitigated somehow.");
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_terminal_status_without_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "wontfix owes no anchor: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("done"));
    }

    /// The discrimination that keeps the decoy heuristic honest. A 16-hex catalog id and a
    /// 40-hex patch-id are both all-hex, and both appear routinely in bug files — reporting
    /// either as a stray commit reference would make the finding untrustworthy.
    #[test]
    fn commit_like_hashes_excludes_catalog_ids_patch_ids_and_bare_numbers() {
        let text = "id `10d7e46375cc3053`, patch-id `e9f8df63b9113a5b4073deebc5501a2cb623287a`, \
                    line `1234567`, commit `6ce49487`, short `abc12`";
        assert_eq!(
            commit_like_hashes(text),
            vec!["6ce49487".to_string()],
            "only the 7-12 hex token containing a letter counts"
        );
    }
    /// The parity bug, pinned with the real shape that produced it.
    ///
    /// Found 2026-08-19 while checking why the shipped check reported zero hashes for
    /// `2026-08-18-three-ledgers-own-prefix-t-…`, which cites `c7bdfd22` three times. A lone
    /// backtick inside a fenced `grep` pattern left the file with an odd backtick count, and
    /// the first implementation split the WHOLE file on backticks and took alternate spans —
    /// so every inline span after that character was inverted and the hash read as prose.
    ///
    /// Two behaviours in one fixture, because they are one decision: fenced content is
    /// quoted material rather than this document's own references, and pairing per line stops
    /// a stray character corrupting anything past its own line.
    #[test]
    fn commit_like_hashes_survives_a_lone_backtick_inside_a_fence() {
        let text = "Intro `T-1` inline.\n\
                    ```\n\
                    git show `abc1234f` | git patch-id --stable\n\
                    grep -n '^#\\{1,6\\}[[:space:]]\\+`\\?T-[0-9]\\+' docs/x.md\n\
                    ```\n\
                    codescout `experiments` @ `c7bdfd22`. Measured against the catalog.\n";
        assert_eq!(
            commit_like_hashes(text),
            vec!["c7bdfd22".to_string()],
            "the fenced hash is quoted material and must not count, and the stray backtick \
             inside that fence must not invert the spans after it"
        );
    }
    /// The other half of the same decision, and the half the fence fixture does NOT cover.
    ///
    /// Written because a mutation exposed the gap: with fenced content skipped, the fence
    /// fixture's remaining backticks are even, so global pairing still passes it. Only a
    /// stray backtick in PROSE separates the two designs. Per-line pairing confines the
    /// damage to one line; global pairing loses every hash in the rest of the document.
    #[test]
    fn commit_like_hashes_confines_a_stray_prose_backtick_to_its_own_line() {
        let text = "A sentence with one unmatched ` backtick, outside any fence.\n\
                    codescout `experiments` @ `c7bdfd22`.\n";
        assert_eq!(
            commit_like_hashes(text),
            vec!["c7bdfd22".to_string()],
            "a stray backtick must corrupt at most its own line"
        );
    }

    /// A fix is not always one commit. The singular parser this replaced is what pushed an
    /// author into a table, which no check could read.
    #[test]
    fn structured_fix_pointers_returns_every_declared_pair_in_order() {
        let text = "## Fix provenance\n\n\
                    - **SHA:** `5a72304c` (`experiments`)\n\
                    - **patch-id:** `e9f8df63b911`\n\
                    - **SHA:** `4ffd2803` (`experiments`)\n\
                    - **patch-id:** `c6beb5f60c30`\n";
        assert_eq!(
            structured_fix_pointers(text),
            vec![
                ("5a72304c".to_string(), Some("e9f8df63b911".to_string())),
                ("4ffd2803".to_string(), Some("c6beb5f60c30".to_string())),
            ],
            "each patch-id must bind to the SHA above it, not to the first SHA in the file"
        );
    }

    /// A fenced block is a quotation, not a claim.
    ///
    /// The real declaration outside the fence is the control: without it, "return nothing"
    /// passes and the parser is silently disabled. Found by this parser reading a bug file's
    /// own worked example as a second declaration.
    /// docs/issues/archive/2026-08-26-structured-fix-pointers-reads-a-fenced-example-as-a-declaration.md
    #[test]
    fn structured_fix_pointers_ignores_a_fenced_worked_example() {
        let text = "## Fix provenance\n\n\
                    - **SHA:** `5a72304c` (`experiments`)\n\
                    - **patch-id:** `e9f8df63b911`\n\n\
                    Do not write it like the file this bug is about:\n\n\
                    ```\n\
                    - **SHA:** `deadbee0`\n\
                    - **patch-id:** `ffffffffffff`\n\
                    ```\n";
        assert_eq!(
            structured_fix_pointers(text),
            vec![("5a72304c".to_string(), Some("e9f8df63b911".to_string()))],
            "only the unfenced declaration counts — a quoted example must not be verified \
             as though the file claimed it"
        );
    }

    /// A backtick run SHORTER than the open fence is content, not a delimiter — the
    /// distinction a bare `starts_with` toggle cannot make. It flips on the inner run, is
    /// left inverted for the rest of the file, and silently skips every declaration below.
    ///
    /// The fixture is not exotic: quadruple fences exist in this corpus precisely because a
    /// worked example has to quote triple-fence syntax, which is what the sibling test
    /// `structured_fix_pointers_ignores_a_fenced_worked_example` teaches people to write.
    ///
    /// `src/librarian/statements.rs` already states the rule this pins — delegating to
    /// `FenceState` "rather than a hand-rolled toggle is required, not incidental".
    #[test]
    fn structured_fix_pointers_reads_a_declaration_after_a_quadruple_fenced_example() {
        let text = "````\n```\n````\n\n- **SHA:** `5a72304c` (`experiments`)\n\
                    - **patch-id:** `e9f8df63b911`\n";
        assert_eq!(
            structured_fix_pointers(text),
            vec![("5a72304c".to_string(), Some("e9f8df63b911".to_string()))],
            "a ``` inside a ```` block is content; a bare toggle flips on it and swallows \
             every declaration that follows"
        );
    }

    /// The balanced file is the control. Without it, a check that fired on every
    /// markdown file in the catalog would pass this test.
    ///
    /// Measured 2026-09-01 before this check existed: 2 of 1259 markdown files under
    /// `docs/` left a fence open, so the finding is rare enough to be worth reading.
    #[tokio::test]
    async fn unterminated_fence_fires_only_where_a_fence_is_left_open() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "balanced",
            "open",
            "",
            "```\ncode\n```\n\nprose\n",
        );
        seed_live_bug(&cat, &root, "left-open", "open", "", "```\ncode\n\nprose\n");
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_unterminated_fence(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "only the unterminated file fires: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("left-open"));
    }

    /// A shorter run inside a longer fence is content, so a file using quadruple fences
    /// to quote triple-fence syntax is BALANCED and must stay silent. This is the same
    /// discrimination `structured_fix_pointers` needed, and the reason both delegate to
    /// `FenceState` rather than counting delimiters: a parity count of that file is odd.
    #[tokio::test]
    async fn unterminated_fence_is_silent_on_a_quadruple_fenced_example() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "quoted",
            "open",
            "",
            "````\n```\n````\n\nprose\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_unterminated_fence(&ctx, &cat.conn).unwrap()
        };
        assert!(
            v.is_empty(),
            "three delimiters, odd parity, but the inner ``` is content — a parity \
             counter would report this file and be wrong: {v:#?}"
        );
    }

    /// The line is the whole remedy: it names where the invisible region begins, which is
    /// what a reader needs to find the stray delimiter in a file thousands of lines long.
    #[tokio::test]
    async fn unterminated_fence_names_the_line_the_silenced_region_starts_at() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        // `seed_live_bug` prefixes 5 lines of frontmatter + title before the body, so the
        // opener below is file line 8. Asserting the FILE line, not a body offset, is the
        // point — a reader opens the file, not the body.
        seed_live_bug(&cat, &root, "deep", "open", "", "intro\n\n```\ncode\n");
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_unterminated_fence(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].detail.contains("line 10"),
            "detail must name the opener's FILE line (10), got: {}",
            v[0].detail
        );
    }

    /// A second declared anchor must be verified too — checking only the first would leave
    /// it rotting exactly as if it had never been recorded.
    #[tokio::test]
    async fn archived_fix_sha_checks_every_declared_pointer_not_only_the_first() {
        let (_tmp, root, live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_archived_bug(
            &cat,
            &root,
            "two",
            &format!(
                "## Fix provenance\n\n\
                 - **SHA:** `{}` (`experiments`)\n\
                 - **patch-id:** `aaaabbbbcccc`\n\
                 - **SHA:** `deadbee` (`experiments`)\n\
                 - **patch-id:** `ddddeeeeffff`\n",
                &live[..8]
            ),
        );
        let ctx = ctx_rooted_at(cat, &root);

        let (v, health) = {
            let cat = ctx.catalog.lock();
            scan_archived_fix_sha_unresolvable(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(
            v.len(),
            1,
            "the live pointer passes, the dead one fires: {v:#?}"
        );
        assert!(v[0].detail.contains("deadbee"));
        assert!(
            v[0].detail.contains("ddddeeeeffff"),
            "and carries the SECOND pair's patch-id, not the first's: {}",
            v[0].detail
        );
        assert_eq!(health["scanned"], 1, "scanned counts files, not pointers");
    }

    /// With no `root=` and no active project there is no scope to infer, and the
    /// catalog spans every indexed repo on the machine. Refusing beats guessing:
    /// the wrong guess here edits files in someone else's repository.
    #[tokio::test]
    async fn repair_frontmatter_id_refuses_when_it_cannot_tell_what_to_sweep() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();

        let err = call(&ctx, json!({ "fix": "repair_frontmatter_id" }))
            .await
            .expect_err("an unscoped write sweep must be refused");
        let msg = err.to_string();
        assert!(
            msg.contains("root="),
            "the refusal must name the parameter that supplies a scope, got: {msg}"
        );
    }

    /// The check exists because `containing_root` silently failed for every
    /// catalog row on Windows (WIN-30) while every other doctor check stayed
    /// green. So this asserts the exact spellings involved, not a synthetic pair:
    /// catalog rows are stored forward-slash + `//?/`-prefixed, while
    /// `current_project` supplies the native `\\?\` form.
    ///
    /// Before the `containing_root` fix this test fails — which is the point of
    /// having it. A version that only asserted the genuinely-outside case would
    /// have passed against the broken build.
    #[cfg(windows)]
    #[test]
    fn outside_managed_roots_accepts_catalog_spelling_of_a_contained_path() {
        let roots = vec![PathBuf::from(r"\\?\C:\Users\dev\work\codescout")];
        let contained = "//?/C:/Users/dev/work/codescout/docs/issues/a.md";

        assert!(
            check_outside_managed_roots("a1", contained, &roots).is_none(),
            "a row inside the active project must not be flagged merely because \
         the catalog and current_project spell the path differently"
        );
    }

    #[test]
    fn outside_managed_roots_flags_a_row_under_no_root() {
        #[cfg(windows)]
        let (roots, outside) = (
            vec![PathBuf::from(r"\\?\C:\Users\dev\work\codescout")],
            "//?/C:/Users/dev/work/other-repo/docs/a.md",
        );
        #[cfg(not(windows))]
        let (roots, outside) = (
            vec![PathBuf::from("/home/dev/work/codescout")],
            "/home/dev/work/other-repo/docs/a.md",
        );

        let v = check_outside_managed_roots("a1", outside, &roots).unwrap();
        assert_eq!(v.check, "abs_path_outside_managed_roots");
        assert_eq!(v.artifact_id.as_deref(), Some("a1"));
        // The detail must name the roots tried: a firing row is often just a
        // foreign workspace, and the reader needs to tell that from real drift
        // without re-deriving the root list.
        assert!(v.detail.contains("codescout"), "got: {}", v.detail);
        assert!(v.detail.contains("artifact(move)"), "got: {}", v.detail);
    }

    /// A sibling sharing a name prefix is outside the root. Same boundary
    /// `containing_root` guards for `delete`; asserted here too so a regression
    /// in either place is caught where a reader would look for it.
    #[test]
    fn outside_managed_roots_respects_the_component_boundary() {
        #[cfg(windows)]
        let (roots, sibling) = (
            vec![PathBuf::from(r"\\?\C:\work\sub")],
            "//?/C:/work/subterfuge/docs/a.md",
        );
        #[cfg(not(windows))]
        let (roots, sibling) = (
            vec![PathBuf::from("/work/sub")],
            "/work/subterfuge/docs/a.md",
        );

        assert!(check_outside_managed_roots("a1", sibling, &roots).is_some());
    }

    /// No configured roots means no conclusion is available. Flagging every row
    /// would turn an unconfigured caller into a catalog-wide false alarm.
    #[test]
    fn outside_managed_roots_is_skipped_when_no_roots_are_configured() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "anywhere", "/somewhere/entirely/else/a.md");

        let (v, _) = scan_artifact_paths(&cat.conn, &[], &[]).unwrap();
        assert!(
            !v.iter()
                .any(|x| x.check == "abs_path_outside_managed_roots"),
            "empty root list must skip the check, not flag every row"
        );
    }

    /// A relative `abs_path` is already reported by `abs_path_must_be_absolute`.
    /// It resolves under nothing, so without the gate it would also trip this
    /// check — one defect wearing two names, and a misleading violation count.
    #[test]
    fn outside_managed_roots_defers_to_the_absoluteness_check() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "bad-relative", "docs/issues/foo.md");

        #[cfg(windows)]
        let roots = vec![PathBuf::from(r"\\?\C:\Users\dev\work\codescout")];
        #[cfg(not(windows))]
        let roots = vec![PathBuf::from("/home/dev/work/codescout")];

        let (v, _) = scan_artifact_paths(&cat.conn, &roots, &[]).unwrap();
        assert!(
            v.iter().any(|x| x.check == "abs_path_must_be_absolute"),
            "the gating check must still fire"
        );
        assert!(
            !v.iter()
                .any(|x| x.check == "abs_path_outside_managed_roots"),
            "a relative row must be reported once, by the absoluteness check only"
        );
    }

    /// Ruling 17, at the outside-roots check: the WORKLIST is the active
    /// developer's, the METRIC stays global.
    ///
    /// The discriminating pair — two rows, both outside every managed root, and
    /// only one of them is anyone's work. Before this split, `doctor` reported
    /// both: measured 2026-08-27 on a real machine, 401 of 516 findings were rows
    /// belonging to other workspaces, and the check's own hint called them
    /// EXPECTED while still emitting them.
    ///
    /// Paths are built from `temp_dir()` so they are platform-native on both
    /// Windows and unix — the point under test is the partition, not the path-form
    /// normalisation that `containing_root` owns (WIN-30).
    #[test]
    fn a_row_under_a_known_workspace_root_is_scoped_out_but_still_counted() {
        let cat = Catalog::open_in_memory().unwrap();
        let base = std::env::temp_dir();
        let active = base.join("cs-active");
        let sibling = base.join("cs-sibling");
        let orphan = base.join("cs-orphan");

        let p = |root: &std::path::Path, name: &str| {
            root.join("docs").join(name).to_string_lossy().into_owned()
        };
        seed_artifact(&cat, "mine", &p(&active, "a.md"));
        seed_artifact(&cat, "sib", &p(&sibling, "b.md"));
        seed_artifact(&cat, "orph", &p(&orphan, "c.md"));

        let roots = vec![active.clone()];
        let known = vec![sibling.clone()];
        let (violations, scoped) = scan_artifact_paths(&cat.conn, &roots, &known).unwrap();

        let outside: Vec<&str> = violations
            .iter()
            .filter(|v| v.check == "abs_path_outside_managed_roots")
            .map(|v| v.path.as_str())
            .collect();

        // The row under a KNOWN workspace root leaves the worklist...
        assert_eq!(
            outside.len(),
            1,
            "only the orphan is anyone's work here; got {outside:?}"
        );
        assert!(
            outside[0].contains("cs-orphan"),
            "the surviving finding must be the orphan, not the sibling; got {outside:?}"
        );

        // ...and is NOT silently dropped. A metric that shrank with the worklist
        // would understate real cross-repo exposure, which is the false negative
        // the aggregate exists to prevent.
        assert_eq!(
            scoped.values().sum::<usize>(),
            1,
            "the scoped-out row must still be counted; got {scoped:?}"
        );
        assert!(
            scoped.keys().any(|k| k.contains("cs-sibling")),
            "counted under its own project root; got {scoped:?}"
        );
    }

    /// The escape hatch stays open: an empty `known_elsewhere` reproduces the
    /// pre-split behaviour exactly. Without this, a caller that cannot compute the
    /// known-roots set (no umbrella, no commits rows) would silently lose the
    /// check rather than fall back to reporting everything.
    #[test]
    fn empty_known_elsewhere_reports_every_outside_row_as_before() {
        let cat = Catalog::open_in_memory().unwrap();
        let base = std::env::temp_dir();
        let active = base.join("cs-active2");
        let sibling = base.join("cs-sibling2");
        seed_artifact(
            &cat,
            "sib",
            &sibling.join("docs").join("b.md").to_string_lossy(),
        );

        let (violations, scoped) = scan_artifact_paths(&cat.conn, &[active], &[]).unwrap();
        assert_eq!(
            violations
                .iter()
                .filter(|v| v.check == "abs_path_outside_managed_roots")
                .count(),
            1,
            "with no known-elsewhere roots the row is reported, not scoped"
        );
        assert!(scoped.is_empty(), "nothing to scope out; got {scoped:?}");
    }

    /// The emitted violation list is capped for this check, but `summary.by_check`
    /// must still report the true total and the hint must say what was dropped.
    /// A report that silently truncates findings reads as "only 10 rows affected",
    /// which is the same class of misleading-green this check exists to prevent.
    #[tokio::test]
    async fn outside_managed_roots_caps_the_list_but_not_the_count() {
        let cat = Catalog::open_in_memory().unwrap();
        // 25 rows under no managed root — comfortably past the 10-row sample.
        for i in 0..25 {
            seed_artifact(
                &cat,
                &format!("far-{i}"),
                &format!("/elsewhere/repo/doc-{i}.md"),
            );
        }

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "managed".to_string(),
                path: PathBuf::from("/managed/root"),
            })
            .build();

        let report = call(&ctx, json!({})).await.unwrap();

        let emitted = report["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["check"] == "abs_path_outside_managed_roots")
            .count();
        assert_eq!(
            emitted, 10,
            "emitted list must be capped at the sample size"
        );

        assert_eq!(
            report["summary"]["by_check"]["abs_path_outside_managed_roots"],
            json!(25),
            "summary must report every violation, not just the emitted sample"
        );

        let hint = report["catalog_health"]["hint"]
            .as_str()
            .unwrap_or_default();
        assert!(
            hint.contains("15 elided"),
            "the hint must name what was dropped; got: {hint}"
        );
    }

    /// Collect the emitted outside-roots paths from a report, in report order.
    fn outside_paths(report: &Value) -> Vec<String> {
        report["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["check"] == "abs_path_outside_managed_roots")
            .map(|v| v["path"].as_str().unwrap().to_string())
            .collect()
    }

    /// Seed `n` outside-roots rows spread over two projects and return the ctx.
    fn ctx_with_outside_rows(n: usize) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        for i in 0..n {
            let project = if i % 2 == 0 { "alpha" } else { "beta" };
            seed_artifact(
                &cat,
                &format!("far-{i}"),
                // Zero-padded so lexical order is also numeric order, which keeps
                // the paging assertions readable.
                &format!("/elsewhere/{project}/docs/doc-{i:03}.md"),
            );
        }
        TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "managed".to_string(),
                path: PathBuf::from("/managed/root"),
            })
            .build()
    }

    /// The elided rows must be reachable. Before this, `doctor` announced N
    /// elided violations and exposed no parameter that could return any of them,
    /// while the hint instructed the reader to inspect exactly those rows.
    /// docs/issues/archive/2026-08-08-doctor-outside-roots-sample-is-unranked-and-unreachable.md
    #[tokio::test]
    async fn outside_managed_roots_limit_reaches_every_elided_row() {
        let ctx = ctx_with_outside_rows(25);

        let capped = call(&ctx, json!({})).await.unwrap();
        assert_eq!(outside_paths(&capped).len(), 10, "default sample");

        let full = call(&ctx, json!({"limit": 25})).await.unwrap();
        assert_eq!(
            outside_paths(&full).len(),
            25,
            "limit must reach every row the summary counts"
        );
        assert_eq!(
            full["catalog_health"]["hint"].as_str().unwrap_or_default(),
            "",
            "nothing elided means no elision hint"
        );
    }

    /// `offset` pages through the remainder, and the pages partition the set:
    /// no row appears twice, none is skipped. This is only meaningful because
    /// `scan_artifact_paths` orders by `abs_path` — paging an unordered set
    /// would silently repeat and drop rows.
    #[tokio::test]
    async fn outside_managed_roots_offset_pages_without_gaps_or_repeats() {
        let ctx = ctx_with_outside_rows(25);

        let mut paged: Vec<String> = Vec::new();
        for page in 0..3 {
            let report = call(&ctx, json!({"limit": 10, "offset": page * 10}))
                .await
                .unwrap();
            paged.extend(outside_paths(&report));
        }

        let all = outside_paths(&call(&ctx, json!({"limit": 25})).await.unwrap());
        assert_eq!(
            paged, all,
            "three pages must reconstruct the full set in order"
        );

        let unique: std::collections::BTreeSet<&String> = paged.iter().collect();
        assert_eq!(unique.len(), 25, "no row may appear on two pages");
    }

    /// The window must be the same window on every call. An unordered SELECT
    /// returns a planner-dependent prefix, so which rows appeared could change
    /// after a VACUUM or a new index with no content change at all.
    #[tokio::test]
    async fn outside_managed_roots_sample_is_ordered_and_repeatable() {
        let ctx = ctx_with_outside_rows(25);

        let first = outside_paths(&call(&ctx, json!({})).await.unwrap());
        let second = outside_paths(&call(&ctx, json!({})).await.unwrap());
        assert_eq!(first, second, "repeated calls must return the same window");

        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(first, sorted, "rows must be emitted in abs_path order");
    }

    /// Every row is accounted for by project even when most are elided, so an
    /// alphabetically-ordered window (which clusters in whichever project sorts
    /// first) still tells the reader the whole shape.
    #[tokio::test]
    async fn outside_roots_by_project_counts_elided_rows_too() {
        let ctx = ctx_with_outside_rows(25);
        let report = call(&ctx, json!({})).await.unwrap();

        let by_project = &report["catalog_health"]["outside_roots_by_project"];
        assert_eq!(by_project["/elsewhere/alpha"], json!(13), "even indices");
        assert_eq!(by_project["/elsewhere/beta"], json!(12), "odd indices");

        let counted: u64 = by_project
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum();
        assert_eq!(
            counted, 25,
            "the aggregate must partition the true total, not the shown sample"
        );

        // The emitted sample really is clustered — which is exactly why the
        // aggregate has to exist.
        let shown = outside_paths(&report);
        assert!(
            shown.iter().all(|p| p.starts_with("/elsewhere/alpha")),
            "alphabetical order clusters the window: {shown:?}"
        );
    }

    /// docs/PROGRESSIVE_DISCOVERABILITY.md § Pattern 1: an overflow hint must
    /// name at least one parameter WITH a usable value. The old hint named none,
    /// because none existed.
    #[tokio::test]
    async fn outside_managed_roots_hint_names_a_parameter_that_reaches_the_rest() {
        let ctx = ctx_with_outside_rows(25);
        let report = call(&ctx, json!({})).await.unwrap();
        let hint = report["catalog_health"]["hint"]
            .as_str()
            .unwrap_or_default();

        assert!(
            hint.contains("limit=25"),
            "hint must name limit with the value that returns everything; got: {hint}"
        );
        assert!(
            hint.contains("offset=10"),
            "hint must name the next page's offset; got: {hint}"
        );
        assert!(
            hint.contains("outside_roots_by_project"),
            "hint must point at the field that accounts for the elided rows; got: {hint}"
        );
        assert!(
            hint.contains("15 elided"),
            "the elision must still be announced; got: {hint}"
        );
    }

    #[test]
    fn outside_roots_group_uses_the_project_prefix_before_docs() {
        assert_eq!(
            outside_roots_group("/home/u/work/proj/docs/trackers/x.md"),
            "/home/u/work/proj"
        );
        // Only the FIRST docs component splits, so a nested docs/ dir does not
        // re-split and fragment one project into many groups.
        assert_eq!(
            outside_roots_group("/home/u/work/proj/docs/manual/docs/y.md"),
            "/home/u/work/proj"
        );
    }

    #[test]
    fn outside_roots_group_falls_back_to_the_parent_without_a_docs_component() {
        assert_eq!(
            outside_roots_group("/home/u/agents/system/crash/root_cause.md"),
            "/home/u/agents/system/crash"
        );
    }

    /// `summary.total` must partition `summary.by_check`. Both sit in the same
    /// object and `total` is the headline number a reader takes first.
    ///
    /// This is the INVARIANT half of the pair;
    /// `outside_managed_roots_caps_the_list_but_not_the_count` above is the
    /// member half, and neither substitutes for the other. A member assertion
    /// on `by_check` stays green while `total` alone is read from the truncated
    /// vector — which is exactly how that regression shipped here, and before
    /// it in the audit-doc-refs counters
    /// (`docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`).
    #[tokio::test]
    async fn summary_total_partitions_by_check() {
        let cat = Catalog::open_in_memory().unwrap();
        // Past the 10-row sample, so the truncation branch actually runs.
        for i in 0..25 {
            seed_artifact(
                &cat,
                &format!("far-{i}"),
                &format!("/elsewhere/repo/doc-{i}.md"),
            );
        }

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "managed".to_string(),
                path: PathBuf::from("/managed/root"),
            })
            .build();

        let report = call(&ctx, json!({})).await.unwrap();

        let total = report["summary"]["total"].as_u64().unwrap();
        let shown = report["summary"]["shown"].as_u64().unwrap();
        let summed: u64 = report["summary"]["by_check"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum();

        assert_eq!(
            total, summed,
            "summary.total must equal the sum of summary.by_check; \
             total={total}, sum(by_check)={summed}"
        );
        assert_eq!(
            shown,
            report["violations"].as_array().unwrap().len() as u64,
            "summary.shown must equal the emitted violation count"
        );

        // Guard the guard: if the fixture ever stops tripping the cap, the
        // assertions above hold trivially and prove nothing about truncation.
        assert!(
            total > shown,
            "fixture must exceed the emitted cap or this test cannot fail; \
             total={total}, shown={shown}"
        );
    }

    /// A check that RAN and found nothing must appear in `by_check` with `0`,
    /// never be absent.
    ///
    /// `by_check` is built by iterating the violations that exist, so a check
    /// contributes a key only if it fired. That makes absence carry three
    /// different meanings at once — ran clean, never wired in, or held back by
    /// its own threshold gate — and it resolves them in the one direction that
    /// reads as a clean bill of health. The report is most reassuring exactly
    /// when it is least informative.
    ///
    /// The fixture seeds a row whose file EXISTS, so `missing_file` genuinely
    /// executes and genuinely finds nothing; an empty catalog would leave the
    /// per-artifact loop unentered and prove only that a check nobody ran is
    /// absent, which is the thing already true today.
    ///
    /// Mutation that must kill this: drop the zero-seeding of `by_check` and
    /// the key disappears — the current behaviour.
    #[tokio::test]
    async fn by_check_names_a_clean_check_with_zero_rather_than_omitting_it() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("doc.md");
        std::fs::write(&file, "---\nkind: tracker\nstatus: active\n---\n# doc\n").unwrap();
        let path = crate::util::fs::RepoPath::from_path(&file).into_string();

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "1111111111111111", &path);
        let ctx = TestToolContextBuilder::new(cat).build();

        let report = call(&ctx, json!({})).await.unwrap();
        let by_check = report["summary"]["by_check"].as_object().unwrap();

        assert_eq!(
            by_check.get("missing_file").and_then(|v| v.as_u64()),
            Some(0),
            "a check that ran and found nothing must report 0, not be omitted — \
             absence is the only signal left for 'did not run'. by_check={by_check:?}"
        );
    }

    /// Seed an augmented tracker: a real file with `body`, plus an augmentation
    /// row whose `entry_collection` holds `ids`.
    fn seed_tracker(cat: &Catalog, id: &str, dir: &std::path::Path, body: &str, ids: &[&str]) {
        let path = dir.join(format!("{id}.md"));
        std::fs::write(&path, body).unwrap();
        let abs = crate::util::fs::RepoPath::from(path.as_path()).into_string();
        seed_artifact(cat, id, &abs);
        let rows: Vec<serde_json::Value> = ids
            .iter()
            .map(|i| serde_json::json!({"id": i, "status": "open"}))
            .collect();
        augmentation::upsert(
            cat,
            &augmentation::AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "p".into(),
                params: serde_json::json!({ "tasks": rows }).to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                // Deliberately Some for EVERY fixture here, including the ones
                // that must NOT be flagged. `render_template` was the gate the
                // bug report proposed, and 26 of 28 real trackers declare one —
                // if it were the gate, the prose-only and in-sync cases below
                // would fire too and these tests would be vacuous.
                render_template: Some("{{ tasks }}".into()),
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("tasks".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    /// docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
    ///
    /// The whole-ledger case, and the larger half of that bug: no `BL-N` heading
    /// exists anywhere, so every entry is uncitable and nothing done to one row
    /// helps. Measured on the real queue: zero definitions against 117 cross-file
    /// citations.
    ///
    /// ONE violation for the ledger, not one per entry. Six findings that all say
    /// "change the ledger's format" is five findings of noise.
    #[test]
    fn undefined_entries_reports_a_row_only_ledger_once_for_the_whole_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "queue",
            tmp.path(),
            "# Queue\n\n| ID | task |\n| BL-1 | a |\n| BL-2 | b |\n| BL-3 | c |\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "one finding per LEDGER, got {v:?}");
        assert_eq!(v[0].check, "ledger_defines_nothing");
        assert!(
            v[0].detail.contains("BL-N") && v[0].detail.contains('3'),
            "must name the prefix (the remedy is per-ledger) and the entry count \
             (the magnitude): {}",
            v[0].detail
        );
    }

    /// The per-entry case: this ledger demonstrably writes definitions and BL-3
    /// missed one, so the remedy is a single heading and the message must blame the
    /// entry. A separate `check` name from the whole-ledger finding, so a report can
    /// be filtered and counted by which remedy it needs.
    #[test]
    fn undefined_entries_names_only_the_undefined_rows_in_a_defining_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            "# L\n\n## BL-1 — first\n\n## BL-2 — second\n\n| ID | task |\n| BL-3 | c |\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "entry_without_definition");
        assert!(v[0].detail.contains("BL-3"), "{}", v[0].detail);
        assert!(
            !v[0].detail.contains("BL-1"),
            "BL-1 has its heading — naming it makes the finding untrustworthy: {}",
            v[0].detail
        );
        // BL-3 lives in an index row. Rows define nothing but ARE scanned for citations, so
        // this fixture is the CITED half of the partition: a reference that resolves to
        // nothing today, and the half a reader can act on. Its twin below is the uncited
        // half, and neither fixture alone can tell the two apart.
        assert!(
            v[0].detail.contains("Cited despite that: 1"),
            "a cited-but-undefined entry is the actionable half and must be counted: {}",
            v[0].detail
        );
        // An earlier wording asserted "these are omissions — add a heading for each" on no
        // evidence at all. Measured 2026-08-19 on `provenance-subsystem.md`, that would have
        // had a reader add 42 headings against a define-on-citation convention stated in the
        // ledger's own body — while saying nothing about which of them were actually broken.
        // The shipped check answers that: 33 cited, 9 uncited. An earlier revision of this
        // very comment guessed "~5" from an eight-token sample; the population disagreed.
        assert!(
            v[0].detail.contains("citation graph"),
            "the finding must say the split is measured rather than assumed: {}",
            v[0].detail
        );
    }

    /// The uncited half of the partition, and the twin the test above needs to mean
    /// anything.
    ///
    /// Same defect shape — an id in `params` with no defining heading — but the token
    /// appears nowhere in the corpus, so no reference is broken and the finding must say so
    /// differently. The difference between the two fixtures is exactly one index row.
    ///
    /// A single fixture cannot distinguish "the ledger forgot" from "the ledger defines on
    /// citation", which is why the check used to assert the former on no evidence.
    #[test]
    fn undefined_entries_separates_an_uncited_entry_from_a_cited_one() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            // No index row this time, so nothing mentions BL-3 anywhere.
            "# L\n\n## BL-1 — first\n\n## BL-2 — second\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "entry_without_definition");
        assert!(v[0].detail.contains("BL-3"), "{}", v[0].detail);
        assert!(
            v[0].detail.contains("Nothing in the catalog cites"),
            "an uncited undefined entry breaks no reference and must be reported as such: {}",
            v[0].detail
        );
        assert!(
            !v[0].detail.contains("Cited despite that"),
            "nothing cites BL-3, so the actionable half must be absent entirely: {}",
            v[0].detail
        );
    }

    /// A cited entry whose heading lives in a SIBLING artifact is not a broken reference.
    ///
    /// The count is ledger-scoped and correct — the heading really is absent from this body.
    /// The consequence clause was not: `link_scan` binds a token to its defining heading
    /// wherever that heading lives, and `tracker-conventions` § *Compaction and archival*
    /// prescribes exactly this shape — "live body → archived section (heading kept)" — and
    /// states a unique definer resolves even when archived. So the check was asserting a
    /// graph property it never consulted, about the supported end state of its own ladder.
    ///
    /// Measured 2026-08-31: `doctor` named 31 `PV-N` tokens as resolving to nothing while
    /// the archive companion defining them held live incoming entry links for 38 distinct
    /// `PV-` tokens, three of them in the eight `doctor` listed
    /// (`docs/issues/archive/2026-08-31-entry-without-definition-claims-broken-refs-that-resolve.md`).
    ///
    /// **A reader following the old text would have made things worse**, not merely wasted
    /// time: adding the heading to the live body creates a second definer, and two definers
    /// is an *ambiguous* token that resolves to nothing — manufacturing the break the
    /// finding claimed to have found.
    #[test]
    fn a_cited_entry_defined_in_a_sibling_artifact_is_not_called_broken() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            "# L\n\n## BL-1 — first\n\n## BL-2 — second\n\n| ID | task |\n| BL-3 | c |\n",
            &["BL-1", "BL-2", "BL-3"],
        );
        // The load-bearing fixture detail: BL-3's heading is in a SEPARATE artifact. Move it
        // into `log.md` and BL-3 leaves `undefined` altogether, so the test would pass while
        // exercising none of this. The index row above is what makes BL-3 *cited* — rows
        // define nothing but are scanned for citations.
        seed_ledger(
            &cat,
            "archive",
            &tmp.path().join("archive.md"),
            "# A\n\n## BL-3 — third, archived\n",
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        let detail = &v[0].detail;
        assert!(
            !detail.contains("resolve to nothing"),
            "a sibling defines BL-3, so the reference resolves and the finding must not \
             assert otherwise: {detail}"
        );
        assert!(
            detail.contains("archive.md"),
            "and it must NAME the definer — without it a reader cannot tell this from the \
             genuinely-broken case, and the obvious repair manufactures an ambiguous \
             token: {detail}"
        );
    }

    /// Non-vacuity twin: with no sibling definer, the reference really is broken and the
    /// finding must still say so.
    ///
    /// The fixture differs from the one above by exactly one artifact. Without this, the
    /// test above is satisfied by never claiming a break at all — which would silence the
    /// check's entire actionable half while passing.
    #[test]
    fn a_cited_entry_defined_nowhere_is_still_reported_as_broken() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            "# L\n\n## BL-1 — first\n\n## BL-2 — second\n\n| ID | task |\n| BL-3 | c |\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        let detail = &v[0].detail;
        assert!(
            detail.contains("resolve to nothing"),
            "nothing anywhere defines BL-3, so this citation IS broken and the finding \
             must keep saying so: {detail}"
        );
    }

    /// A citation qualified by file stem (`log:BL-3`) counts as a citation.
    ///
    /// It has to, and the reason is a quirk of extraction rather than a preference: a
    /// file-stem qualifier and a cross-repo qualifier are **one syntactic shape**, so
    /// `link_scan` emits both as `CrossRepoToken` and leaves resolution to decide. Matching
    /// only bare `EntryToken`s would therefore read a qualified citation as no citation at
    /// all — and `get_guide("tracker-conventions")` tells authors to qualify precisely when
    /// several ledgers share a prefix, which is when ambiguity makes it most necessary.
    #[test]
    fn undefined_entries_counts_a_stem_qualified_citation() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            "# L\n\n## BL-1 — first\n\n## BL-2 — second\n\nSee log:BL-3 for the rest.\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(
            v[0].detail.contains("Cited despite that: 1"),
            "a stem-qualified citation is still a citation, and this one dangles: {}",
            v[0].detail
        );
    }

    /// The negative control. Without it, a scan that reported unconditionally would
    /// satisfy both tests above.
    #[test]
    fn undefined_entries_is_silent_when_every_entry_has_its_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "log",
            tmp.path(),
            "# L\n\n## BL-1 — first\n\nbody\n\n## BL-2 — second\n\nbody\n",
            &["BL-1", "BL-2"],
        );
        assert!(
            scan_undefined_entries(&cat.conn).unwrap().is_empty(),
            "every entry is defined — there is nothing to report"
        );
    }

    /// The test that pins the design decision, by asserting BOTH scans on ONE
    /// fixture. The body anchors a small scattered minority (1 of 6), so
    /// `body_keeps_snapshot` is false and `snapshot_drift` stays silent — correctly,
    /// because those rows are deliberately params-canonical.
    ///
    /// This check must fire anyway. Reusing that gate here would silence exactly the
    /// population already broken, which is the bug: an advisory that goes quiet where
    /// citations break. The two scans ask different questions of the same body and are
    /// allowed to disagree — that disagreement is the fix, not a bug in it.
    #[test]
    fn undefined_entries_fires_where_snapshot_drift_is_deliberately_silent() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "prose",
            tmp.path(),
            "# Notes\n\nSee also BL-1 in passing.\n\n| ID |\n| BL-1 |\n",
            &["BL-1", "BL-2", "BL-3", "BL-4", "BL-5", "BL-6"],
        );

        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "a minority anchor is a params-canonical tracker; nagging it is noise"
        );
        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "but not one of its six entries can be cited, and that is worth saying: {v:?}"
        );
        assert_eq!(v[0].check, "ledger_defines_nothing");
    }

    // ---- scan_cited_prefix_with_no_definer -----------------------------------------------

    /// Regression for docs/issues/archive/2026-08-26-cited-prefix-with-no-definer-is-invisible.md:
    /// a prefix with zero definers repo-wide is not a *broken* citation in the resolver's
    /// terms (it is not a citation candidate at all), so it landed in neither `link_scan`'s
    /// dangling/ambiguous buckets nor either existing `doctor` check. Both of those key off
    /// entries a ledger already claims; this one has to start from the citation graph itself.
    #[test]
    fn cited_prefix_with_no_definer_fires_above_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "a",
            &tmp.path().join("a.md"),
            "See T-1 and T-2 for the plan.\n",
        );
        seed_ledger(
            &cat,
            "b",
            &tmp.path().join("b.md"),
            "T-1 is still open; so is T-3.\n",
        );

        let (v, _) = scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "cited_prefix_with_no_definer");
        assert!(
            v[0].detail.contains('T') && v[0].detail.contains('4'),
            "must name the prefix and the citation count (4: T-1 twice, T-2, T-3): {}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("a.md") && v[0].detail.contains("b.md"),
            "must name the citing files: {}",
            v[0].detail
        );
    }

    /// Below the citation-count threshold, stay silent — the guard against the false
    /// positive the guide already warns about for ledger inference (a design doc quoting
    /// `## R-4` in prose is not a namespace).
    #[test]
    fn cited_prefix_with_no_definer_is_silent_below_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "a",
            &tmp.path().join("a.md"),
            "One passing mention of T-1.\n",
        );

        assert!(
            scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn)
                .unwrap()
                .0
                .is_empty(),
            "a single incidental citation must not fire"
        );
    }

    /// DISPERSION, not volume: a prefix whose citations are spread roughly one per file is an
    /// incidental technical term (`UTF-8`, `SHA-256`, `RFC-7396`), and a real ledger namespace
    /// clusters instead. Measured 2026-09-01 across all 14 live findings on this repo: gating on
    /// `files / cites >= 0.8` suppressed 6 of 8 noise prefixes and lost **zero** real ones.
    /// docs/issues/archive/2026-09-01-citation-volume-gate-selects-for-the-prose-it-excludes.md
    ///
    /// **Both directions are in ONE fixture on purpose.** The suppression half alone is monotone
    /// under "the check does nothing" and would pass against a stub that returns `vec![]`; the
    /// clustered prefix is what makes this a discriminator rather than a control. Both clear
    /// `MIN_CITATIONS` and `MIN_FILES`, so neither is decided by a pre-existing gate — mutate
    /// either count below its floor and this test stops testing dispersion at all.
    #[test]
    fn cited_prefix_with_no_definer_suppresses_a_scattered_prefix_but_keeps_a_clustered_one() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        // Scattered: 4 citations across 4 files == 1.00 files/cites. Incidental-term shape.
        // Clustered: 6 citations across 2 files == 0.33. Ledger-namespace shape.
        for (i, name) in ["s1", "s2", "s3", "s4"].iter().enumerate() {
            let body = if i < 2 {
                // The two clustered files carry BOTH prefixes, so the scattered one cannot be
                // suppressed merely by living in different files from the clustered one.
                "Encoded as UT-8 here. See CL-1, CL-2 and CL-3.\n".to_string()
            } else {
                "A passing mention of UT-8.\n".to_string()
            };
            seed_ledger(&cat, name, &tmp.path().join(format!("{name}.md")), &body);
        }

        let (v, _) = scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn).unwrap();
        let prefixes: Vec<&str> = v
            .iter()
            .map(|x| {
                if x.detail.contains("`CL-") {
                    "CL"
                } else {
                    "UT"
                }
            })
            .collect();

        assert!(
            prefixes.contains(&"CL"),
            "a clustered prefix (6 cites / 2 files) must still be reported: {v:?}"
        );
        assert!(
            !prefixes.contains(&"UT"),
            "a prefix cited once per file across 4 files is an incidental term, not a namespace: {v:?}"
        );
        assert_eq!(v.len(), 1, "exactly one finding expected: {v:?}");
    }

    /// PINS the dispersion constant from both sides. The test above proves the gate EXISTS —
    /// it is the sole killer of "gate removed" — but it does not pin the number the gate
    /// introduces, and those are different claims.
    ///
    /// Measured by `codescout-09` on 2026-09-01 via mutation, and confirmed by arithmetic over
    /// this module's fixture ratios (`CL` 0.33, `T-N` 0.50, `ZZ-N` 0.667, `UT-N` 1.00): before
    /// this test, the shipped 0.8 could be moved anywhere in **(0.667, 1.0]** with a fully green
    /// suite. The only thing holding the lower end was `cited_prefix_reports_only_the_active_
    /// projects_citers`, whose purpose is project SCOPING and whose 3-cites/2-files ratio is
    /// incidental to it — so a tidy-up giving `ZZ-N` another citation (making it read more like
    /// a real ledger) would have kept that test green and silently removed the last guard.
    ///
    /// Two cases bracket the constant tightly, and each dies in the opposite direction:
    ///   * `LO-N` — 4 citations across 3 files == 0.75, just UNDER. Must still be reported;
    ///     this fails if the threshold is lowered (0.7 suppresses it).
    ///   * `HI-N` — 5 citations across 4 files == 0.80, exactly AT. Must be suppressed, which
    ///     also pins the comparison as `>=` rather than `>`; this fails if the threshold is
    ///     raised (0.9 keeps it).
    ///
    /// Together they admit only (0.75, 0.80]. Both clear `MIN_CITATIONS` and `MIN_FILES`, so
    /// neither outcome is decided by a pre-existing gate.
    #[test]
    fn cited_prefix_dispersion_threshold_is_pinned_from_both_sides() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        // LO: f1..f3 -> 4 cites / 3 files = 0.75. HI: f1..f4 -> 5 cites / 4 files = 0.80.
        for (name, body) in [
            ("f1", "LO-1 and LO-2. HI-1 and HI-2.\n"),
            ("f2", "LO-3. HI-3.\n"),
            ("f3", "LO-4. HI-4.\n"),
            ("f4", "HI-5.\n"),
        ] {
            seed_ledger(&cat, name, &tmp.path().join(format!("{name}.md")), body);
        }

        let (v, _) = scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn).unwrap();
        let reported = |p: &str| v.iter().any(|x| x.detail.contains(&format!("`{p}-N`")));

        assert!(
            reported("LO"),
            "0.75 is under the 0.80 threshold and must still be reported — this assertion is \
             what fails if DISPERSION_DEN/NUM is lowered: {v:?}"
        );
        assert!(
            !reported("HI"),
            "0.80 is exactly at the threshold and must be suppressed, which also pins the \
             comparison as `>=` rather than `>` — this assertion is what fails if the \
             threshold is raised: {v:?}"
        );
    }

    /// A prefix defined anywhere in the corpus is `link_scan`'s territory (resolved or
    /// dangling), not this check's — even when the citing file itself defines nothing.
    #[test]
    fn cited_prefix_with_no_definer_is_silent_when_prefix_is_defined_elsewhere() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "ledger",
            &tmp.path().join("ledger.md"),
            "## T-1 — the only one\n",
        );
        seed_ledger(
            &cat,
            "a",
            &tmp.path().join("a.md"),
            "T-1 and T-99 both come up here, alongside T-100.\n",
        );

        assert!(
            scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn)
                .unwrap()
                .0
                .is_empty(),
            "T is a known prefix (T-1 is defined), so T-99/T-100 dangle -- link_scan's job, \
             not this check's"
        );
    }

    /// A ledger that DECLARES `entry_prefix` but defines nothing is `ledger_defines_nothing`'s
    /// finding, not this one's -- double-reporting the same underlying namespace under two
    /// check names would be noise, not signal.
    #[test]
    fn cited_prefix_with_no_definer_is_silent_when_prefix_is_declared() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "ledger",
            &tmp.path().join("ledger.md"),
            "---\nentry_prefix: T\n---\n\n# Queue\n\nNo headings yet.\n",
        );
        seed_ledger(
            &cat,
            "a",
            &tmp.path().join("a.md"),
            "T-1, T-2, and T-3 are all still pending.\n",
        );

        assert!(
            scan_cited_prefix_with_no_definer(&unscoped_ctx(), &cat.conn)
                .unwrap()
                .0
                .is_empty(),
            "T is declared, so it's a known-but-empty namespace -- ledger_defines_nothing's \
             territory"
        );
    }

    /// The scoping guard, pinned in BOTH directions in one test: an unowned prefix cited
    /// inside the active project is reported, and one cited only under a sibling root is
    /// COUNTED as scoped out rather than silently dropped. Two separate prefixes, so a
    /// regression that scopes nothing and one that scopes everything fail differently.
    ///
    /// Regression for
    /// docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md:
    /// this check was 33 of its own 47 findings foreign, the largest single contributor to a
    /// `doctor` report that was 52% other repos' rows.
    #[test]
    fn cited_prefix_reports_only_the_active_projects_citers() {
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(
            &cat,
            "in-a",
            &active_root.join("a.md"),
            "ZZ-1 and ZZ-2 here.\n",
        );
        seed_ledger(&cat, "in-b", &active_root.join("b.md"), "ZZ-3 as well.\n");
        seed_ledger(
            &cat,
            "out-c",
            &sibling_root.join("c.md"),
            "QQ-1 and QQ-2 over here.\n",
        );
        seed_ledger(&cat, "out-d", &sibling_root.join("d.md"), "QQ-3 too.\n");

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_prefix_with_no_definer(&ctx, &cat.conn).unwrap()
        };

        assert_eq!(
            v.len(),
            1,
            "only the prefix cited under the ACTIVE project may be reported: {v:#?}"
        );
        assert!(
            // `QQ-N` in BACKTICKS, not a bare `QQ`. The detail embeds the absolute paths of
            // the citing files (`Citing files: {}` below), and those live under a
            // `tempfile::tempdir()` whose name carries ~6 random alphanumerics — so a bare
            // substring search can match the fixture's own scratch path instead of a finding.
            // Observed in CI 2026-08-31 (run 33404896131, server-stack lane): this test failed
            // on a detail whose prefix claim was correctly `ZZ-N`, and passed in the runs
            // either side. Flaky at roughly 1-in-800, i.e. often enough to red a branch and
            // rarely enough to look like something else. A backticked token cannot occur in a
            // path, so the assertion now depends only on what the check reports.
            v[0].detail.contains("`ZZ-N`") && !v[0].detail.contains("`QQ-N`"),
            "the reported finding must be ZZ, the in-project one: {}",
            v[0].detail
        );
        assert_eq!(
            scoped_out.values().sum::<usize>(),
            1,
            "QQ must be COUNTED as scoped out, not silently dropped: {scoped_out:?}"
        );
        let key = scoped_out.keys().next().unwrap();
        assert!(
            key.contains("sibling-project"),
            "the drop is attributed to the project that actually cites it: {key}"
        );
    }

    /// The half that must NOT scope, and the reason the fix is not simply "filter by root".
    ///
    /// A prefix defined only in a SIBLING repo is still defined. Narrowing `known_prefixes`
    /// to the active project would make every such prefix — and every cross-repo
    /// `<repo>:<TOKEN>` citation, which the resolver deliberately declines to turn into an
    /// edge — fire as "no definer anywhere in the corpus", manufacturing false positives out
    /// of correct prose.
    ///
    /// The citations here clear both thresholds (3 across 2 files), so a regression fails
    /// loudly rather than passing for the unrelated reason of sitting under the noise floor.
    #[test]
    fn cited_prefix_definers_stay_corpus_wide_across_project_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        let cat = Catalog::open_in_memory().unwrap();

        // The ONLY definer of HY lives outside the active project.
        seed_ledger(
            &cat,
            "sibling-ledger",
            &sibling_root.join("ledger.md"),
            "## HY-1 — the only definer, and it is in another repo\n",
        );
        seed_ledger(
            &cat,
            "in-a",
            &active_root.join("a.md"),
            "HY-1 and HY-2 are referenced here.\n",
        );
        seed_ledger(&cat, "in-b", &active_root.join("b.md"), "So is HY-3.\n");

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_prefix_with_no_definer(&ctx, &cat.conn).unwrap()
        };

        assert!(
            v.is_empty(),
            "HY IS defined in the corpus — scoping the definer pass to the active project \
             would turn a legitimate cross-repo citation into a false 'unowned namespace': {v:#?}"
        );
        assert!(
            scoped_out.is_empty(),
            "a known prefix is not a scoped-out finding either — it is simply not a finding: \
             {scoped_out:?}"
        );
    }

    /// A prefix real enough to fire corpus-wide, whose in-project citations fall below the
    /// same thresholds, is scoped out — and keyed by its first citer OUTSIDE the project.
    ///
    /// The paths are chosen so the alphabetically-first citer overall is the IN-project one
    /// (`active-project/` sorts before `sibling-project/`). Keying the drop by `files[0]`,
    /// the obvious shortcut, would file it under the reader's own project root and read as
    /// though their own repo had been excluded from their own report.
    #[test]
    fn a_mostly_foreign_prefix_is_scoped_out_and_keyed_outside_the_project() {
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        let cat = Catalog::open_in_memory().unwrap();

        // One in-project citation: below MIN_CITATIONS and MIN_FILES on its own.
        seed_ledger(
            &cat,
            "in-a",
            &active_root.join("a.md"),
            "A single passing mention of MM-1.\n",
        );
        // Three more elsewhere, which is what carries the prefix over the corpus-wide bar.
        seed_ledger(
            &cat,
            "out-b",
            &sibling_root.join("b.md"),
            "MM-2 and MM-3 are tracked here.\n",
        );
        seed_ledger(&cat, "out-c", &sibling_root.join("c.md"), "As is MM-4.\n");

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_prefix_with_no_definer(&ctx, &cat.conn).unwrap()
        };

        assert!(
            v.is_empty(),
            "one in-project citation is below the same floor that keeps UTF-8/SHA-256 quiet: \
             {v:#?}"
        );
        assert_eq!(
            scoped_out.values().sum::<usize>(),
            1,
            "the prefix IS unowned corpus-wide — the metric must still count it: {scoped_out:?}"
        );
        let key = scoped_out.keys().next().unwrap();
        assert!(
            key.contains("sibling-project") && !key.contains("active-project"),
            "keyed by the first citer OUTSIDE the project, not by files[0] — which here is \
             the in-project file: {key}"
        );
    }

    // ---- scan_premature_archive_citation ---------------------------------------------------

    /// Regression for `bug-fix-session-log:F-69` / `claim-decay:DC-2`: a citation written at
    /// fix time naming the path the archive flow *would* create, for a bug that then stayed
    /// open. Nothing else reports it — `audit_doc_refs` caps a source comment to Med by
    /// design, and the guide's archive sweep is triggered BY a move that never happened.
    #[test]
    fn premature_archive_citation_fires_when_the_bug_is_still_live() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "bug", "/repo/docs/issues/2026-08-26-still-open.md");
        seed_ledger(
            &cat,
            "citer",
            &tmp.path().join("guide.md"),
            "Root cause in `docs/issues/archive/2026-08-26-still-open.md`.\n",
        );

        let v = scan_premature_archive_citation(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "premature_archive_citation");
        assert!(
            v[0].detail.contains("2026-08-26-still-open.md"),
            "must name the slug so the fix is actionable: {}",
            v[0].detail
        );
    }

    /// The ordinary, correct case: the bug really is archived. Firing here would flag every
    /// well-formed citation in the repo, which is the failure mode that would get the check
    /// switched off rather than fixed.
    #[test]
    fn premature_archive_citation_is_silent_when_the_archive_path_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "bug", "/repo/docs/issues/archive/2026-08-26-done.md");
        seed_ledger(
            &cat,
            "citer",
            &tmp.path().join("guide.md"),
            "Fixed — see `docs/issues/archive/2026-08-26-done.md`.\n",
        );

        assert!(
            scan_premature_archive_citation(&cat.conn)
                .unwrap()
                .is_empty(),
            "a citation to a path that exists is not this check's business"
        );
    }

    /// Neither path exists: a plain dead link, which `audit_doc_refs` already reports. This
    /// check claims a specific CAUSE — written-before-the-move — and it cannot know that
    /// about a slug the corpus has never held, so it must not guess.
    #[test]
    fn premature_archive_citation_is_silent_when_neither_path_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "citer",
            &tmp.path().join("guide.md"),
            "See `docs/issues/archive/2026-01-01-never-existed.md`.\n",
        );

        assert!(
            scan_premature_archive_citation(&cat.conn)
                .unwrap()
                .is_empty(),
            "a dead link with no live twin is audit_doc_refs' finding, not a premature citation"
        );
    }

    /// A retired document citing a path that was correct when written is a record, not drift.
    /// Same exemption `apply_drops`' `archive_drop` makes, for the same reason: rewriting it
    /// would falsify the record to satisfy a linter.
    #[test]
    fn premature_archive_citation_is_silent_for_a_citer_under_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "bug", "/repo/docs/issues/2026-08-26-still-open.md");
        seed_ledger(
            &cat,
            "retired",
            &tmp.path().join("archive").join("old-log.md"),
            "Back then: `docs/issues/archive/2026-08-26-still-open.md`.\n",
        );

        assert!(
            scan_premature_archive_citation(&cat.conn)
                .unwrap()
                .is_empty(),
            "an archived citer is a historical snapshot and is exempt"
        );
    }

    /// The extractor must not read documentation ABOUT the convention as a citation. Both
    /// shapes occur in `tracker-conventions.md` itself, which is also a catalogued artifact —
    /// so a naive scan would make the guide that teaches the rule the check's loudest
    /// violator.
    #[test]
    fn cited_archive_basenames_ignores_placeholders_and_globs() {
        let names = cited_archive_basenames(
            "write docs/issues/archive/<slug>.md, leave docs/issues/archive/** alone, \
             and cite docs/issues/archive/real-one.md\n",
        );
        assert_eq!(names.len(), 1, "{names:?}");
        assert!(names.contains("real-one.md"), "{names:?}");
    }

    /// The core positive. The body's index table carries ids that `params` has no
    /// row for — the shape measured on the WIN ledger in
    /// `docs/issues/archive/2026-08-18-no-check-detects-a-body-that-has-run-ahead-of-params.md`,
    /// where six rows lived in git and in no query.
    #[test]
    fn params_behind_body_reports_a_body_id_with_no_params_row() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "ahead",
            tmp.path(),
            "| ID |\n| BL-1 |\n| BL-2 |\n| BL-3 |\n",
            &["BL-1", "BL-2"],
        );

        let v = scan_params_behind_body(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "params_behind_body");
        assert!(v[0].detail.contains("BL-3"), "{}", v[0].detail);
    }

    /// The direction pin: on a body that LAGS params, the old check fires and the new
    /// one must stay silent. Both read the same two sets; only the subtraction order
    /// differs, so an implementation that got it backwards would pass every
    /// single-check test and fail here.
    #[test]
    fn a_lagging_body_is_snapshot_drift_and_never_params_behind_body() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "lagging",
            tmp.path(),
            "| ID |\n| BL-1 |\n| BL-2 |\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        assert_eq!(
            scan_snapshot_drift(&cat.conn).unwrap().len(),
            1,
            "BL-3 is in params and not in the body — that is the original check"
        );
        assert!(
            scan_params_behind_body(&cat.conn).unwrap().is_empty(),
            "nothing ran ahead here; reporting it would be the same finding twice"
        );
    }

    /// Seed a tracker whose entries carry explicit statuses and whose `params_schema`
    /// declares a `status` enum — the two preconditions `scan_params_status_drift` needs.
    ///
    /// Deliberately separate from `seed_tracker` rather than a parameter on it. That
    /// helper pins `params_schema: None` and a uniform `"open"` status, and those two
    /// facts are what keep its fifteen fixtures invisible to the status scan. Widening it
    /// would silently enroll every one of them in a comparison they were not written for,
    /// and several would fire.
    fn seed_status_tracker(
        cat: &Catalog,
        id: &str,
        dir: &std::path::Path,
        body: &str,
        entries: &[(&str, &str)],
        status_enum: &[&str],
    ) {
        let path = dir.join(format!("{id}.md"));
        std::fs::write(&path, body).unwrap();
        let abs = crate::util::fs::RepoPath::from(path.as_path()).into_string();
        seed_artifact(cat, id, &abs);
        let rows: Vec<serde_json::Value> = entries
            .iter()
            .map(|(i, s)| serde_json::json!({"id": i, "status": s}))
            .collect();
        let schema = serde_json::json!({
            "properties": {
                "tasks": { "items": { "properties": { "status": { "enum": status_enum } } } }
            }
        });
        augmentation::upsert(
            cat,
            &augmentation::AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "p".into(),
                params: serde_json::json!({ "tasks": rows }).to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                render_template: Some("{{ tasks }}".into()),
                params_schema: Some(schema.to_string()),
                append_mode: false,
                history_cap: None,
                entry_collection: Some("tasks".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    /// The shape this check exists for, taken from the incident rather than invented:
    /// `BL-60`'s `params` row read `open` while its committed table row read
    /// `done-archived`, and all three id-set scans were silent because the id was
    /// present on both sides.
    #[test]
    fn status_drift_fires_when_params_and_the_body_row_disagree() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "disagree",
            tmp.path(),
            "| ID | Task | Status |\n| BL-1 | the CLI drops two fields | **done-archived** |\n",
            &[("BL-1", "open")],
            &["open", "done-archived"],
        );

        assert!(
            scan_params_behind_body(&cat.conn).unwrap().is_empty(),
            "the id is on both sides, so the id-set scans must stay silent — that \
             silence is the gap this check fills"
        );
        let v = scan_params_status_drift(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].check, "params_status_drift");
        assert!(v[0].detail.contains("BL-1"), "{}", v[0].detail);
        assert!(
            v[0].detail.contains("`open`"),
            "the finding must name the params side, since the reader has to decide \
             which of the two is stale: {}",
            v[0].detail
        );
    }

    /// A table row is only this entry's STATUS region if its table has a status column.
    ///
    /// `entry_status_region` tried the table form first and returned the first line matching
    /// `^| <ID> |` with no check that the table concerned status at all — so a ledger whose
    /// entries carry no `**Status:**` line, but which happens to contain an unrelated analysis
    /// table mentioning those ids, had every entry's "status region" resolve to a row stating
    /// no status. That is the case the scan's own doc comment excludes: *"An entry with no body
    /// region stating a status is skipped, not reported."* Branch 1 shadowed branch 2, which
    /// would have found the heading, found no `Status:` line, and correctly returned `None`.
    /// docs/issues/archive/2026-09-01-status-locator-reads-any-table-row-as-a-status-row.md
    ///
    /// **Both directions in one fixture**, because the silence half alone is monotone under
    /// "the check does nothing" and would pass against a stub returning `vec![]`. `T-1` lives
    /// only in the analysis table and must be SILENT; `T-2` has a real status table whose row
    /// contradicts its `params`, and must still FIRE. Mutate the fix to skip table rows
    /// entirely and `T-2` dies; mutate it to accept any row and `T-1` dies.
    #[test]
    fn status_drift_ignores_a_table_that_has_no_status_column() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "no-status-col",
            tmp.path(),
            // An analysis table — three columns, none of them status — sits ABOVE the real
            // one. Order is load-bearing: the locator takes the FIRST matching row, so a fix
            // that only checked the last table would pass this while still being wrong.
            "## T-1 — measured, no status line\n\n\
             **Why:** recorded in History rather than per-entry.\n\n\
             | task | headline number | reality |\n\
             |---|---|---|\n\
             | T-1 | `missing field 'patch'` x11 | all 11 predate the fix |\n\n\
             ## T-2 — tracked in a real status table\n\n\
             | ID | Task | Status |\n\
             |---|---|---|\n\
             | T-2 | the CLI drops two fields | **open** |\n",
            &[("T-1", "done"), ("T-2", "done")],
            &["open", "done", "dropped"],
        );

        let v = scan_params_status_drift(&cat.conn).unwrap();
        let detail = v.first().map(|x| x.detail.clone()).unwrap_or_default();

        assert!(
            !detail.contains("T-1"),
            "T-1 states no status anywhere — its only table row is an analysis row, so it has \
             one representation and cannot drift. Reporting it is the defect: {v:?}"
        );
        assert!(
            detail.contains("T-2"),
            "T-2's real status row says `open` while params says `done` — that is a genuine \
             disagreement and must still be reported, or the fix has simply disabled the \
             table form: {v:?}"
        );
    }

    /// The measured false-positive class, pinned. On 2026-08-30 a naive comparison over
    /// this repo produced 26 findings and **20 were this**: `params` holds
    /// `done-archived` while the body writes `**done, archived**`. Comma versus hyphen.
    ///
    /// Deleting the separator-flexing in `status_token_present` turns this red, which is
    /// the point — without it the check is 77% noise and nobody would keep it on.
    #[test]
    fn status_drift_is_silent_when_the_body_punctuates_the_status_differently() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "punct",
            tmp.path(),
            "| ID | Status |\n| BL-1 | **done, archived** |\n",
            &[("BL-1", "done-archived")],
            &["open", "done-archived"],
        );

        assert!(
            scan_params_status_drift(&cat.conn).unwrap().is_empty(),
            "`done, archived` and `done-archived` are the same status rendered two ways"
        );
    }

    /// The negative half of the predicate, deliberately alone in its own test.
    ///
    /// It began life sharing a test with the positive assertions above it and had to be
    /// split, because the mutation matrix showed why: under a `contains` substitution the
    /// combined test died on its **first** assertion — the positive separator one — and
    /// never reached these. It reported as a kill for a guard it had not exercised. A
    /// test that dies for the wrong reason is not evidence about the reason in its name,
    /// and the two here fail under different mutations, so they cannot share one.
    ///
    /// This is the reason the fix is a boundary-anchored regex rather than "strip the
    /// punctuation from both sides". Stripping is the obvious way to absorb
    /// `**done, archived**`, and it makes `done` a substring of `aban`**`done`**`d` and
    /// `open` of `re-`**`open`**`ed` — both of which occur in this repo's real status
    /// prose. That substitution keeps the positive test green and kills only this one.
    #[test]
    fn status_token_present_does_not_match_a_status_word_inside_a_longer_word() {
        assert!(
            !status_token_present("| the task was abandoned |", "done"),
            "`done` is a substring of `abandoned`; a stripped-punctuation compare \
             would report this row as stating `done`"
        );
        assert!(
            !status_token_present("| **RE-OPENED 2026-08-30** |", "open"),
            "`open` is a substring of `reopened`; same failure, and this exact row \
             text exists in the queue tracker"
        );
    }

    /// The positive half: the separator flexing that absorbs the measured
    /// false-positive class (`**done, archived**` for `done-archived`, 20 of 26 findings
    /// on 2026-08-30). Split from the negative assertions above — see that test's note.
    #[test]
    fn status_token_present_matches_a_status_the_body_punctuates_differently() {
        assert!(status_token_present(
            "| **done, archived** |",
            "done-archived"
        ));
        assert!(status_token_present("| **done** |", "done"));
    }

    /// The second locator, covering the 30 entries a table-row-only implementation
    /// silently skipped while reporting nothing amiss about them: `fable-tuning-tasks`
    /// and `-findings` render entries as a heading plus a `**Status:**` line.
    #[test]
    fn status_drift_finds_the_heading_form_that_renders_no_table_row() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "headings",
            tmp.path(),
            "# T\n\n## FT-1 — do the thing\n\n**Status:** done\n\nSome prose.\n",
            &[("FT-1", "open")],
            &["open", "done"],
        );

        let v = scan_params_status_drift(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].detail.contains("FT-1"), "{}", v[0].detail);
    }

    /// The region is the `Status:` line, never the whole section — and this is the
    /// design decision that keeps the check usable rather than a style preference.
    ///
    /// Entry prose routinely narrates status history: this fixture's wording is taken
    /// from a real queue row (*"was dropped as a design decision"*) whose status is
    /// nonetheless `open`. Widening the region to the section makes that row fire, and
    /// every other row whose author explained how it got where it is.
    #[test]
    fn status_drift_reads_the_status_line_and_not_the_sections_prose() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "prose",
            tmp.path(),
            "# T\n\n## FT-1 — re-opened after a bad call\n\n**Status:** open\n\n\
             Was dropped as a design decision, then done, then archived — all wrong.\n",
            &[("FT-1", "open")],
            &["open", "done", "dropped"],
        );

        assert!(
            scan_params_status_drift(&cat.conn).unwrap().is_empty(),
            "the Status line agrees with params; the narration below it is not a status"
        );
    }

    /// The *precision* half of the heading locator — which, until this test, nothing measured.
    ///
    /// Its sibling above asserts **silence** when the `Status:` line agrees with `params`, and
    /// silence is also what a widened region produces: if the locator returned the whole
    /// section, that fixture's `open` would still be found on its own `Status:` line, agreement
    /// would still hold, and the assertion would still pass. So the test named for *not* reading
    /// the prose cannot witness the locator reading the prose.
    ///
    /// Measured 2026-08-30 by mutating the locator to perform the forbidden act — return the
    /// entire section as the region — rather than by removing it: **all six** of this scan's
    /// `status_drift` tests passed, including both heading-form tests. Removing a mechanism
    /// makes an absence test pass for an uninformative reason; only making the mechanism do the
    /// forbidden thing can show whether the absence test is able to fail at all.
    ///
    /// The direction matters and is why no other test stumbles into it. A widened region makes
    /// the `params` status **more** likely to be found, so the failure is a false *negative* —
    /// the scan silently discharges the very disagreement it exists to report. Here `params`
    /// says `open`, the `Status:` line says `done`, and the prose below happens to contain the
    /// word `open`: correct code reports the drift, a section-swallowing region calls it
    /// agreement and says nothing.
    #[test]
    fn status_drift_does_not_let_prose_discharge_a_real_disagreement() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "widen",
            tmp.path(),
            // LOAD-BEARING, and invisible in the assertion below: the word `open` in
            // the prose line is what makes this fixture discriminating. It must be a
            // status the enum declares, it must differ from the `Status:` line, and it
            // must equal the `params` value — only then does a section-swallowing
            // region find agreement where there is none. Tidy the prose to "held for a
            // week" and this test keeps passing while detecting nothing, because that
            // edit is monotone under the assertion and no assertion can catch it.
            "# T\n\n## FT-2 — closed after the review\n\n**Status:** done\n\n\
             Left open for a week while the review ran.\n",
            &[("FT-2", "open")],
            &["open", "done"],
        );

        assert_eq!(
            scan_params_status_drift(&cat.conn).unwrap().len(),
            1,
            "`params` says open, the Status line says done — the prose saying `open` is \
             narration, and must not discharge the disagreement"
        );
    }

    /// The gate that keeps this scan off the fifteen fixtures written for its three
    /// siblings — and off the five real ledgers here that declare no `status` enum.
    ///
    /// Without a closed vocabulary there is nothing to compare against but free text,
    /// which is the objection `scan_params_behind_body` records when it declines this
    /// comparison. The body here contradicts `params` outright and must still be silent.
    #[test]
    fn status_drift_is_silent_for_a_ledger_declaring_no_status_enum() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // seed_tracker pins params_schema: None and status "open" for every entry.
        seed_tracker(
            &cat,
            "noenum",
            tmp.path(),
            "| ID | Status |\n| BL-1 | **done-archived** |\n",
            &["BL-1"],
        );

        assert!(
            scan_params_status_drift(&cat.conn).unwrap().is_empty(),
            "params says `open` and the body says `done-archived`, but no enum is \
             declared — reporting it would enroll every prose tracker in a comparison \
             it never opted into"
        );
    }

    /// An entry `params` tracks but the body states no status for cannot drift: it has
    /// one representation. Silence here is not a miss, and pinning it stops a later
    /// "why does this skip rows?" from being closed by reporting them.
    #[test]
    fn status_drift_skips_an_entry_whose_body_states_no_status() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_status_tracker(
            &cat,
            "nostatus",
            tmp.path(),
            "# T\n\nProse mentioning BL-1 in passing, with no row and no heading.\n",
            &[("BL-1", "open")],
            &["open", "done"],
        );

        assert!(scan_params_status_drift(&cat.conn).unwrap().is_empty());
    }

    /// Fires where `snapshot_drift` is silent because nothing is missing FROM the
    /// body. A body that is a superset of params leaves `claimed.difference(&in_body)`
    /// empty, so the only surface codescout had reports a clean bill of health.
    #[test]
    fn params_behind_body_fires_where_snapshot_drift_sees_a_complete_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "superset",
            tmp.path(),
            "| ID |\n| BL-1 |\n| BL-2 |\n| BL-3 |\n| BL-4 |\n| BL-5 |\n| BL-6 |\n| BL-7 |\n",
            &["BL-1", "BL-2", "BL-3", "BL-4", "BL-5", "BL-6"],
        );

        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "every params row IS in the body, so the snapshot looks perfectly in sync"
        );
        let v = scan_params_behind_body(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "but BL-7 exists in git and in no query: {v:?}");
        assert!(v[0].detail.contains("BL-7"), "{}", v[0].detail);
    }

    /// **Not gated on `body_keeps_snapshot`, and this is the test that pins it.**
    /// Here the body anchors a single scattered id, so that gate is false and
    /// `snapshot_drift` stays silent by design — correctly, because those rows are
    /// params-canonical. Reusing the gate would silence BL-7 too, and BL-7 is a row
    /// the catalog has never seen. Same argument as
    /// `undefined_entries_fires_where_snapshot_drift_is_deliberately_silent`.
    #[test]
    fn params_behind_body_is_not_gated_on_body_keeps_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "prose_ahead",
            tmp.path(),
            "# Notes\n\n| ID |\n| BL-7 |\n",
            &["BL-1", "BL-2", "BL-3", "BL-4", "BL-5", "BL-6"],
        );

        assert!(
            !crate::librarian::catalog::augmentation::body_keeps_snapshot(
                &[1u64, 2, 3, 4, 5, 6].into_iter().collect(),
                &[7u64].into_iter().collect()
            ),
            "fixture precondition: this body does NOT keep a snapshot"
        );
        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "the gate silences the row question here, as it should"
        );
        let v = scan_params_behind_body(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "but BL-7 has no row at all: {v:?}");
        assert!(v[0].detail.contains("BL-7"), "{}", v[0].detail);
    }

    /// A body with entry HEADINGS and no index table keeps no snapshot, so
    /// there is nothing to be behind. Measured on `tool-usage-patterns`
    /// 2026-08-28: 32 defining headings, **0** table rows — and the hint
    /// fired anyway, telling a maintainer to fix a table that does not exist.
    ///
    /// `body_claimed_indices` reads headings and rows into one set, so heading
    /// coverage alone satisfied the majority gate. See
    /// `docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`.
    #[test]
    fn snapshot_drift_is_silent_when_the_body_has_only_headings() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "headings_only",
            tmp.path(),
            // BL-3 is in params but has no heading and no row. With the bug,
            // heading coverage (2 of 3) still clears the majority gate and
            // BL-3 is reported as a missing snapshot ROW — in a document that
            // has no table at all.
            //
            // Deliberately NOT the all-covered shape: that one passes today
            // for an incidental reason (`missing` comes out empty, so the
            // check `continue`s before the table question is ever asked) and
            // would keep passing with the bug present. It asserts nothing.
            "# Notes\n\n## BL-1 — a\n\nbody\n\n## BL-2 — b\n\nbody\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "a body with no table cannot have a table that lags"
        );
    }

    /// The other half of the same defect, and the more dangerous one: headings
    /// **mask** a genuinely lagging table. `claimed.difference(in_body)` comes
    /// out empty because the headings fill the holes the rows left, so the
    /// check hits `missing.is_empty()` and goes silent — after its gate passed.
    ///
    /// Shape measured on `prompt-hamsa-audit-log` 2026-08-28: every params id
    /// has a heading, the table has fewer rows than that, and `doctor` said
    /// nothing. Here BL-3 has a heading but no row.
    #[test]
    fn snapshot_drift_fires_when_headings_mask_a_lagging_table() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "headings_mask_table",
            tmp.path(),
            "# Notes\n\n| ID | x |\n|---|---|\n| BL-1 | a |\n| BL-2 | b |\n\n\
                 ## BL-1 — a\n\n## BL-2 — b\n\n## BL-3 — c\n",
            &["BL-1", "BL-2", "BL-3"],
        );

        let v = scan_snapshot_drift(&cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "the table is one row behind and must be reported: {v:?}"
        );
        assert!(
            v[0].detail.contains("BL-3"),
            "must name the row the table is missing; got: {}",
            v[0].detail
        );
    }

    /// Guard against over-correcting: a body whose snapshot really is a table,
    /// and really is complete, must stay silent. Without this, "gate on rows"
    /// could be implemented as "always fire when a table exists".
    #[test]
    fn snapshot_drift_is_silent_when_the_table_is_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "table_complete",
            tmp.path(),
            "# Notes\n\n| ID | x |\n|---|---|\n| BL-1 | a |\n| BL-2 | b |\n",
            &["BL-1", "BL-2"],
        );

        assert!(scan_snapshot_drift(&cat.conn).unwrap().is_empty());
    }

    /// The named remedy must be one that can actually perform the repair, and the two
    /// obvious candidates cannot.
    ///
    /// `append_entry` unconditionally overwrites `entry["id"]` with the id it allocates,
    /// and allocates `params_next.max(body_max + 1)` — so on this fixture it would mint
    /// `BL-3`, and on the real WIN ledger `WIN-37`, rather than the missing rows.
    /// `update_entry` patches a row that already exists and is pinned never to change the
    /// row count. Naming either is worse than naming nothing: it sends the reader to a
    /// tool that reports success and repairs nothing.
    ///
    /// **This replaces a test that asserted `detail.contains("append_entry")`.** That
    /// assertion could not tell "append_entry is the fix" from "append_entry cannot fix
    /// this" — it passes under both readings, and the wrong one shipped. The
    /// discriminating assertion is the name of the tool that CAN do it.
    #[test]
    fn params_behind_body_names_a_remedy_that_can_actually_repair_it() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "remedy",
            tmp.path(),
            "| ID |\n| BL-1 |\n| BL-2 |\n",
            &["BL-1"],
        );

        let v = scan_params_behind_body(&cat.conn).unwrap();
        let detail = &v[0].detail;
        assert!(
            detail.contains("artifact_augment"),
            "must name the wholesale params write — the only surface that can create a row \
                 at a GIVEN id: {detail}"
        );
        assert!(
            !detail.contains("re-render the snapshot section from params"),
            "that is `snapshot_drift`'s remedy and it destroys the newer record: {detail}"
        );
    }

    /// The sample cap needs a fixture that EXCEEDS it — a minimal one proves only the
    /// under-cap case and leaves `if more > 0` unexecuted. Memory
    /// `test-design-discipline`: that exact hole shipped in `grep`'s
    /// `completeness_warning` with seven green tests.
    #[test]
    fn params_behind_body_caps_its_sample_and_counts_the_remainder() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let body: String = (1..=13).map(|n| format!("| BL-{n} |\n")).collect();
        seed_tracker(&cat, "capped", tmp.path(), &body, &["BL-1"]);

        let v = scan_params_behind_body(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "{v:?}");
        let detail = &v[0].detail;
        assert!(
            detail.contains("(+4 more)"),
            "12 unrowed, 8 shown: {detail}"
        );
        assert!(
            !detail.contains("BL-13"),
            "the 13th must be behind the cap: {detail}"
        );
    }

    /// docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
    ///
    /// Rows live in `params`, params live in the catalog, and the catalog is
    /// machine-local and git-ignored. A tracker that keeps a rendered snapshot
    /// in its body is the only way they reach git, and nothing re-renders it.
    ///
    /// The fixture carries a MAJORITY of its rows (4 of 6) because that is what
    /// a maintained snapshot lagging at the tail looks like — see
    /// `body_keeps_snapshot`. A 50/50 fixture would sit exactly on the gate and
    /// assert nothing about which side of it this case belongs to.
    #[test]
    fn snapshot_drift_reports_rows_that_reached_params_but_never_the_body() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "queue",
            tmp.path(),
            "# Queue\n\n| ID | task |\n| BL-1 | a |\n| BL-2 | b |\n| BL-3 | c |\n| BL-4 | d |\n",
            &["BL-1", "BL-2", "BL-3", "BL-4", "BL-5", "BL-6"],
        );

        let v = scan_snapshot_drift(&cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "expected exactly one drifted tracker, got {v:?}"
        );
        assert_eq!(v[0].check, "snapshot_drift");
        assert!(
            v[0].detail.contains("BL-5") && v[0].detail.contains("BL-6"),
            "the detail must name the rows that are absent from git, got: {}",
            v[0].detail
        );
        assert!(
            !v[0].detail.contains("BL-1"),
            "BL-1 IS in the body — naming it would make the finding untrustworthy: {}",
            v[0].detail
        );
    }

    /// The gate that keeps this check from being noise. A prose-only tracker
    /// keeps its rows in `params` on purpose and anchors none in its body;
    /// reporting every row as missing would be wrong for 5 of the 28 augmented
    /// trackers on the authoring machine.
    #[test]
    fn snapshot_drift_stays_silent_for_a_tracker_that_keeps_no_body_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "prose",
            tmp.path(),
            "# Notes\n\nAll prose. The rows live in params by design.\n",
            &["BL-1", "BL-2", "BL-3"],
        );
        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "a tracker anchoring no ids keeps no snapshot — nothing can be behind"
        );
    }

    /// The false positive this check shipped with, found on the SECOND tracker
    /// it was pointed at (`docs/trackers/provenance-subsystem.md`, 2026-08-16).
    ///
    /// That tracker is **params-canonical by design** — its own § "PV-N entries"
    /// says *"The canonical PV-N rows live in the augmentation params, not in
    /// this file"*, and narrative goes in the body only for entries needing more
    /// than a row. It still line-anchors 14 of 68 ids incidentally: first cells
    /// of UNRELATED tables (`| PV-25 | rule … |`) and four `### PV-N` write-ups.
    /// The original "anchors ≥1 id" gate read that as a snapshot 79% behind.
    ///
    /// Majority coverage separates them, and the two populations were measured
    /// to be bimodal with no overlap — see `body_keeps_snapshot`.
    #[test]
    fn snapshot_drift_stays_silent_for_a_params_canonical_tracker_that_merely_mentions_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // 3 of 12 anchored (25%), scattered — mentions, not an index.
        seed_tracker(
            &cat,
            "canonical",
            tmp.path(),
            "# Programme\n\nThe canonical rows live in params, not in this file.\n\n\
             | rule | when |\n| BL-2 | design time |\n| BL-7 | design time |\n\n\
             ### BL-11 — needed more than a row\n\nprose.\n",
            &[
                "BL-1", "BL-2", "BL-3", "BL-4", "BL-5", "BL-6", "BL-7", "BL-8", "BL-9", "BL-10",
                "BL-11", "BL-12",
            ],
        );
        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "a body anchoring a small scattered minority is mentioning ids, not \
             maintaining a snapshot — reporting it nags a design decision"
        );
    }

    /// The other side of the same threshold: a body carrying a MAJORITY is a
    /// snapshot that fell behind, and must still be reported. Modelled on
    /// `prompt-hamsa-audit-log.md` as it actually was — 14 of 23 anchored (61%),
    /// a contiguous prefix missing only the tail.
    #[test]
    fn snapshot_drift_still_reports_a_majority_snapshot_that_fell_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let body = "# Q\n\n| ID |\n".to_string()
            + &(1..=8).map(|n| format!("| BL-{n} |\n")).collect::<String>();
        let ids: Vec<String> = (1..=12).map(|n| format!("BL-{n}")).collect();
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        seed_tracker(&cat, "lagging", tmp.path(), &body, &refs);

        let v = scan_snapshot_drift(&cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "8 of 12 anchored is a maintained snapshot lagging at the tail: {v:?}"
        );
        assert!(v[0].detail.contains("BL-9"), "{}", v[0].detail);
    }

    #[test]
    fn snapshot_drift_stays_silent_when_the_body_carries_every_row() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "synced",
            tmp.path(),
            "# Queue\n\n## BL-1 — a\n\n## BL-2 — b\n",
            &["BL-1", "BL-2"],
        );
        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "params and body agree — there is nothing to report"
        );
    }

    /// Prose mentions must not count as the body carrying a row, or the check
    /// silently under-reports exactly the drift it exists to find.
    ///
    /// The three anchored rows keep this above the majority gate, so the test
    /// isolates the prose-vs-anchored distinction rather than re-testing
    /// `body_keeps_snapshot`.
    ///
    /// **Fixture changed 2026-08-28: the three anchors were `## BL-N —`
    /// headings and are now index rows.** The name, the assertion and the
    /// intent are untouched — prose must not count as anchored — but this scan
    /// now reads ROW anchors only, so a headings-only fixture no longer
    /// exercises it. The headings shape is not thereby unguarded; it is
    /// asserted one test down, where it belongs.
    #[test]
    fn snapshot_drift_does_not_accept_a_prose_mention_as_a_snapshot_row() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "prosey",
            tmp.path(),
            "# Queue\n\n| ID | T |\n|----|---|\n| BL-1 | a |\n| BL-2 | b |\n| BL-3 | c |\n\n\
                 We should also look at BL-4 sometime.\n",
            &["BL-1", "BL-2", "BL-3", "BL-4"],
        );
        let v = scan_snapshot_drift(&cat.conn).unwrap();
        assert_eq!(v.len(), 1, "BL-4 is mentioned, not rendered: {v:?}");
        assert!(v[0].detail.contains("BL-4"), "{}", v[0].detail);
    }

    /// The coverage question the fixture change above raises, asserted rather
    /// than assumed: when `snapshot_drift` goes quiet on a headings-only body,
    /// does anything still report the entry that body is missing?
    ///
    /// It does — and it is the check that was always right for this shape.
    /// `snapshot_drift`'s remedy is "re-render the table", and there is no
    /// table here. `undefined_entries`' remedy is "add the `## BL-4 — title`
    /// heading", which is the action a maintainer of a headings-only tracker
    /// actually needs to take. Silencing the first loses nothing.
    ///
    /// Same argument and same shape as
    /// `undefined_entries_fires_where_snapshot_drift_is_deliberately_silent`:
    /// the two scans ask different questions of one body and are allowed to
    /// disagree. This is one more place where the disagreement is the design.
    #[test]
    fn undefined_entries_covers_the_headings_only_body_snapshot_drift_now_ignores() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_tracker(
            &cat,
            "headings_only_coverage",
            tmp.path(),
            "# Queue\n\n## BL-1 — a\n\n## BL-2 — b\n\n## BL-3 — c\n\n\
                 We should also look at BL-4 sometime.\n",
            &["BL-1", "BL-2", "BL-3", "BL-4"],
        );

        assert!(
            scan_snapshot_drift(&cat.conn).unwrap().is_empty(),
            "no table exists here, so no table can be behind"
        );

        let v = scan_undefined_entries(&cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "BL-4 has no heading, so nothing can cite it — that must still be said: {v:?}"
        );
        assert!(
            v[0].detail.contains("BL-4"),
            "must name the entry: {}",
            v[0].detail
        );
    }

    #[tokio::test]
    async fn doctor_call_surfaces_seeded_drift() {
        let cat = Catalog::open_in_memory().unwrap();
        // 6 seeded artifact rows + 1 commit row. Each fault triggers ALL
        // applicable checks (e.g. a backslash path also fails `missing_file`
        // because no host file lives at the bogus path) — so we assert
        // by per-check counts, not by total.
        seed_artifact(&cat, "bad-backslash", "C:/users\\marius\\foo.md");
        seed_artifact(&cat, "bad-ads", "C:/users/foo.txt:stream");
        seed_artifact(&cat, "bad-dotdot", "/home/marius/../etc/passwd");
        seed_artifact(&cat, "bad-missing", "/definitely/not/a/real/path.md");
        // Wrong-shape row — relative string stored where abs is required.
        // Found in the wild during the post-#69 live-catalog smoke test.
        seed_artifact(&cat, "bad-relative", "docs/issues/foo.md");
        // Clean path: absolute, exists, forward-slash form, no backslash / ADS
        // colon / `..` — so it trips none of the checks (notably not missing_file).
        // Must exist on the host running the suite, so it is platform-specific:
        // `/tmp` on unix, `C:/Windows` on Windows (the drive colon is not an ADS
        // colon — same reason the `C:/` seeds above fire only their other checks).
        #[cfg(unix)]
        let clean_path = "/tmp";
        #[cfg(windows)]
        let clean_path = "C:/Windows";
        seed_artifact(&cat, "clean", clean_path);
        seed_commit(&cat, "abc123", "C:/users\\marius");

        // No managed roots: the outside-roots check is skipped entirely, so
        // this existing assertion set is unchanged by its addition.
        let (v, _) = scan_artifact_paths(&cat.conn, &[], &[]).unwrap();
        let mut by_check: std::collections::BTreeMap<&str, usize> = Default::default();
        for x in &v {
            *by_check.entry(x.check.as_str()).or_insert(0) += 1;
        }
        assert_eq!(by_check.get("backslash_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("ads_colon_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("dotdot_segment_in_abs_path").copied(), Some(1));
        assert_eq!(by_check.get("abs_path_must_be_absolute").copied(), Some(1));
        // 5 missing-file hits: bad-backslash, bad-ads, bad-dotdot, bad-missing,
        // and bad-relative (Path::exists on "docs/issues/foo.md" resolves
        // against the test runner's cwd and finds nothing). clean_path exists, so
        // it does not fire.
        assert_eq!(by_check.get("missing_file").copied(), Some(5));

        let r = scan_commits_git_root(&cat.conn).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].check, "backslash_in_git_root");
    }

    #[tokio::test]
    async fn doctor_reports_catalog_health_hidden_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "hidden", "/nonexistent/hidden/path.md");
        cat.conn
            .execute(
                "UPDATE artifact SET missing_since = 1 WHERE id = 'hidden'",
                [],
            )
            .unwrap();
        #[cfg(unix)]
        let live_path = "/tmp";
        #[cfg(windows)]
        let live_path = "C:/Windows";
        seed_artifact(&cat, "live", live_path);

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = call(&ctx, json!({})).await.unwrap();
        assert!(out["catalog_health"]["hidden_rows"].as_u64().unwrap() >= 1);
        assert_eq!(
            out["catalog_health"]["move_candidates"].as_u64().unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn doctor_surfaces_move_candidate_via_commit_hash_overlap() {
        // The active project IS a real git repo: detect_move_candidates
        // opens it via git2 to walk its own reachable commit set (see
        // gc.rs's MECHANISM NOTE on detect_move_candidates for why a pure
        // commits-table query can't answer this — `hash` is that table's
        // PRIMARY KEY, so a naive self-join can never find an overlap).
        let dir = tempfile::tempdir().unwrap();
        let new_root_path = dir.path().join("newrepo");
        std::fs::create_dir_all(&new_root_path).unwrap();
        let repo = git2::Repository::init(&new_root_path).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        std::fs::write(new_root_path.join("a.txt"), "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        let h1 = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap()
            .to_string();
        let new_root = crate::util::fs::RepoPath::from_path(&new_root_path).into_string();

        let cat = Catalog::open_in_memory().unwrap();
        // same hash, attributed to a now-dead root — the move signal.
        seed_commit(&cat, &h1, "/gone/oldrepo");
        let current_project =
            std::sync::Arc::new(crate::librarian::current_project::CurrentProject {
                abs_path: new_root_path.clone(),
                git_root: new_root_path.clone(),
                main_root: None,
                umbrella: None,
            });
        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(current_project)
            .build();

        let out = call(&ctx, json!({})).await.unwrap();
        assert!(out["catalog_health"]["move_candidates"].as_u64().unwrap() >= 1);
        let detail = out["catalog_health"]["move_candidates_detail"]
            .as_array()
            .unwrap();
        assert_eq!(detail[0]["old_root"], "/gone/oldrepo");
        assert_eq!(detail[0]["new_root"], new_root);
        assert!(
            out["catalog_health"]["hint"]
                .as_str()
                .unwrap()
                .contains("rehome"),
            "hint must mention doctor(fix=\"rehome\", ...) when a candidate is found"
        );
    }

    #[test]
    fn validate_prune_request_gates() {
        let cat = Catalog::open_in_memory().unwrap();

        // unknown fix → refused (rejected before any path check)
        assert!(validate_prune_request("zap", Some("/gone"), &cat.conn).is_err());
        // missing root → refused
        assert!(validate_prune_request("prune_missing", None, &cat.conn).is_err());
        // relative root → refused (relative on every platform)
        assert!(validate_prune_request("prune_missing", Some("relative/path"), &cat.conn).is_err());

        // live root refused — an existing absolute dir is not an orphan. Derive it from
        // the OS temp dir so the path is absolute AND present on every platform (Unix
        // /tmp, Windows C:\…\Temp); a hard-coded "/tmp" is not absolute on Windows and
        // broke this test under wine / windows (BUG 36d475f3).
        let live = std::env::temp_dir();
        let live = live.to_str().expect("temp_dir path is valid UTF-8");
        assert!(validate_prune_request("prune_missing", Some(live), &cat.conn).is_err());

        // dead absolute root → accepted. Build a temp-dir-rooted path that does not
        // exist, so it is absolute on every platform.
        let dead = std::env::temp_dir().join("codescout-nonexistent-root-6f3a1c9e");
        assert!(!dead.exists(), "test fixture path must not exist");
        let dead = dead.to_str().expect("temp path is valid UTF-8");
        assert!(validate_prune_request("prune_missing", Some(dead), &cat.conn).is_ok());
    }

    #[test]
    fn validate_rehome_gates() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        let live = dir.path().join("live");
        std::fs::create_dir_all(&live).unwrap();
        // Absolute-but-missing on every platform; see `dead_root`.
        let gone_old = dead_root("rehome-gates-old");
        let gone_new = dead_root("rehome-gates-new");
        // new_root must exist
        assert!(validate_rehome_request(Some(&gone_old), Some(&gone_new), &cat.conn).is_err());
        // old_root must NOT exist
        assert!(validate_rehome_request(
            Some(live.to_str().unwrap()),
            Some(live.to_str().unwrap()),
            &cat.conn
        )
        .is_err());
        // both required + absolute
        assert!(validate_rehome_request(None, Some(live.to_str().unwrap()), &cat.conn).is_err());
        assert!(validate_rehome_request(
            Some("relative/old"),
            Some(live.to_str().unwrap()),
            &cat.conn
        )
        .is_err());
        // new_root is required
        assert!(validate_rehome_request(Some(&gone_old), None, &cat.conn).is_err());
        // new_root must be absolute
        assert!(validate_rehome_request(Some(&gone_old), Some("relative/new"), &cat.conn).is_err());
        // happy path: old gone, new exists
        assert!(
            validate_rehome_request(Some(&gone_old), Some(live.to_str().unwrap()), &cat.conn)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn run_fix_rehome_dry_run_then_confirm_migrates_rows() {
        let new_dir = tempfile::tempdir().unwrap();
        let new_root = new_dir.path().to_str().unwrap();
        let new_root_str = crate::util::fs::RepoPath::from_path(new_dir.path()).into_string();
        let old_root_buf = dead_root("rehome-repo");
        let old_root = old_root_buf.as_str();
        let expected_new_abs = format!("{new_root_str}/docs/x.md");

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "a1", &format!("{old_root}/docs/x.md"));
        seed_commit(&cat, "c1", old_root);
        let ctx = TestToolContextBuilder::new(cat).build();

        // Dry run: reports the plan, must NOT mutate anything.
        let dry = call(
            &ctx,
            json!({ "fix": "rehome", "root": old_root, "new_root": new_root }),
        )
        .await
        .unwrap();
        assert_eq!(dry["mode"], "dry_run");
        assert_eq!(dry["artifact_rows"], 1);
        assert_eq!(dry["commit_rows"], 1);
        assert_eq!(dry["collisions"].as_array().unwrap().len(), 0);

        assert!(
            artifact::get(&ctx.catalog.lock(), "a1").unwrap().is_some(),
            "dry-run must not mutate the artifact row"
        );
        {
            let cat = ctx.catalog.lock();
            let commit_root: String = cat
                .conn
                .query_row("SELECT git_root FROM commits WHERE hash = 'c1'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(
                commit_root, old_root,
                "dry-run must not mutate commits.git_root"
            );
        }

        // Confirm: actually migrates.
        let applied = call(
            &ctx,
            json!({ "fix": "rehome", "root": old_root, "new_root": new_root, "confirm": true }),
        )
        .await
        .unwrap();
        assert_eq!(applied["mode"], "applied");
        assert_eq!(applied["migrated"]["artifact_rows"], 1);
        assert_eq!(applied["migrated"]["commit_rows"], 1);
        assert_eq!(applied["migrated"]["skipped_collisions"], 0);

        assert!(
            artifact::get(&ctx.catalog.lock(), "a1").unwrap().is_none(),
            "old id must no longer exist after rehome"
        );
        let cat = ctx.catalog.lock();
        let new_abs: String = cat
            .conn
            .query_row(
                "SELECT abs_path FROM artifact WHERE abs_path = ?1",
                params![expected_new_abs],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_abs, expected_new_abs);
        let commit_root: String = cat
            .conn
            .query_row("SELECT git_root FROM commits WHERE hash = 'c1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(commit_root, new_root_str);
    }

    #[tokio::test]
    async fn run_fix_rehome_via_surfaced_old_root_arg_dry_runs() {
        // Regression test: the move-candidate hint (call()) and
        // validate_rehome_request's own error text both instruct
        // `doctor(fix="rehome", old_root=..., new_root=...)`. Invoke with
        // that EXACT surfaced wording and assert a dry-run result — not a
        // RecoverableError re-asserting a missing `old_root` param.
        let new_dir = tempfile::tempdir().unwrap();
        let new_root = new_dir.path().to_str().unwrap();
        let old_root_buf = dead_root("rehome-old-root-arg-repo");
        let old_root = old_root_buf.as_str();

        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "a1", &format!("{old_root}/docs/x.md"));
        seed_commit(&cat, "c1", old_root);
        let ctx = TestToolContextBuilder::new(cat).build();

        let dry = call(
            &ctx,
            json!({
                "fix": "rehome",
                "old_root": old_root,
                "new_root": new_root,
                "confirm": false,
            }),
        )
        .await
        .unwrap();
        assert_eq!(dry["mode"], "dry_run");
        assert_eq!(dry["artifact_rows"], 1);
        assert_eq!(dry["commit_rows"], 1);
    }

    #[tokio::test]
    async fn run_fix_rehome_errors_when_no_rows_under_old_root() {
        let new_dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        // Must be absolute-but-missing on every platform, or the call fails at
        // the absolute-path gate and this asserts on the wrong error string.
        let empty_root = dead_root("rehome-empty");
        let err = call(
            &ctx,
            json!({
                "fix": "rehome",
                "root": empty_root,
                "new_root": new_dir.path().to_str().unwrap(),
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("no catalog rows found"),
            "got: {err}"
        );
    }

    /// `rehome` must refuse an `old_root` that an ACTIVE registration still
    /// covers, mirroring `prune_missing_refuses_root_with_active_registration`
    /// — rehoming out from under an unmerged worktree's only catalog record
    /// would silently orphan its history the same way pruning would.
    #[test]
    fn validate_rehome_request_refuses_old_root_with_active_registration() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        let new_root = dir.path().join("new");
        std::fs::create_dir_all(&new_root).unwrap();
        let dead_root =
            std::env::temp_dir().join("codescout-nonexistent-rehome-registered-root-3f8a5d2c");
        assert!(!dead_root.exists(), "test fixture path must not exist");
        let dead_root_str = dead_root.to_str().expect("temp path is valid UTF-8");
        let normalized = crate::util::fs::RepoPath::from_path(&dead_root).to_string();
        reg::upsert_active(&cat, &normalized, &normalized, None, 1000).unwrap();

        let err = validate_rehome_request(
            Some(dead_root_str),
            Some(new_root.to_str().unwrap()),
            &cat.conn,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("merge_worktree"),
            "hint names merge_worktree: {msg}"
        );
        assert!(msg.contains("abandon"), "hint names abandon=true: {msg}");
    }

    #[test]
    fn prune_dead_root_removes_rows_under_root_only() {
        let cat = Catalog::open_in_memory().unwrap();
        // Rows under a dead root /gone/repo (exact + nested).
        seed_artifact(&cat, "g1", "/gone/repo");
        seed_artifact(&cat, "g2", "/gone/repo/a.md");
        seed_artifact(&cat, "g3", "/gone/repo/docs/b.md");
        // A sibling row that merely shares a path PREFIX string but is a
        // different repo — must NOT be pruned (no false LIKE match).
        seed_artifact(&cat, "sib", "/gone/repo-other/c.md");
        // An unrelated live row.
        seed_artifact(&cat, "keep", "/tmp/keep.md");
        seed_commit(&cat, "deadc0de", "/gone/repo");
        seed_commit(&cat, "livecdef", "/tmp");

        let (arts, commits) =
            prune_dead_root(&cat.conn, std::path::Path::new("/gone/repo")).unwrap();
        assert_eq!(arts, 3, "the 3 rows at/under /gone/repo are removed");
        assert_eq!(commits, 1, "the /gone/repo commit is removed");

        // Survivors: the prefix-sibling and the unrelated row remain.
        let exists = |id: &str| -> i64 {
            cat.conn
                .query_row("SELECT COUNT(*) FROM artifact WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert_eq!(
            exists("sib"),
            1,
            "/gone/repo-other not matched by the prefix"
        );
        assert_eq!(exists("keep"), 1);
        let n_com: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM commits", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n_com, 1, "only the /tmp commit remains");
    }

    #[test]
    fn derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files() {
        let cat = Catalog::open_in_memory().unwrap();
        let live = tempfile::tempdir().unwrap(); // exists on disk

        // Boundary that "still exists" (e.g. a real ~/work/ parent dir); the
        // deleted repo lives one level below it and is never created.
        let dead_parent = tempfile::tempdir().unwrap();
        let dead_root = dead_parent.path().join("repo");

        // (a) whole subtree gone: parent dir does not exist -> included.
        seed_artifact(&cat, "a1", &dead_root.join("docs/x.md").to_string_lossy());
        seed_artifact(&cat, "a2", &dead_root.join("docs/y.md").to_string_lossy());
        // (b) single missing file under a LIVE dir -> excluded (reindex's job).
        let missing_under_live = live.path().join("gone.md");
        seed_artifact(&cat, "b1", &missing_under_live.to_string_lossy());
        // (c) a live file -> not missing, excluded.
        let live_file = live.path().join("here.md");
        std::fs::write(&live_file, "x").unwrap();
        seed_artifact(&cat, "c1", &live_file.to_string_lossy());

        let roots = derive_dead_roots(&cat.conn).unwrap();
        assert_eq!(
            roots,
            vec![dead_root],
            "only the gone subtree's highest-nonexistent-ancestor is a dead root"
        );
    }

    #[test]
    fn derive_dead_roots_skips_non_absolute_paths() {
        // A malformed relative abs_path row must NOT yield a dead root — otherwise
        // the climb bottoms out at an empty PathBuf whose prune matches everything.
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "rel", "relative/does/not/exist.md");
        let roots = derive_dead_roots(&cat.conn).unwrap();
        assert!(
            roots.is_empty(),
            "non-absolute row must not yield a dead root, got: {roots:?}"
        );
    }

    #[test]
    fn count_dead_root_counts_rows_under_root() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(&cat, "a1", "/nonexistent-root/repo/docs/x.md");
        seed_artifact(&cat, "a2", "/nonexistent-root/repo/y.md");
        seed_artifact(&cat, "z1", "/nonexistent-root/other/z.md");
        // Prefix sibling: /nonexistent-root/repo-other must NOT match the
        // LIKE '/nonexistent-root/repo/%' clause scoped to .../repo.
        seed_artifact(&cat, "sibling", "/nonexistent-root/repo-other/z.md");
        seed_commit(&cat, "deadbeef", "/nonexistent-root/repo");
        seed_commit(&cat, "cafef00d", "/nonexistent-root/repo-other");
        let (arts, commits) =
            count_dead_root(&cat.conn, std::path::Path::new("/nonexistent-root/repo")).unwrap();
        assert_eq!(
            arts, 2,
            "prefix-sibling repo-other rows must not be counted under repo"
        );
        assert_eq!(
            commits, 1,
            "prefix-sibling commit git_root must not be counted under repo"
        );
    }

    #[tokio::test]
    async fn prune_missing_batch_dry_run_lists_dead_roots_without_deleting() {
        let cat = Catalog::open_in_memory().unwrap();
        seed_artifact(
            &cat,
            "a1",
            &format!("{}/docs/x.md", dead_root("prune-dry-run")),
        );
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing" })).await.unwrap(); // no root, no confirm
        assert_eq!(v["mode"], "dry_run");
        assert_eq!(v["totals"]["artifact_rows"].as_u64().unwrap(), 1);
        // Nothing deleted.
        assert!(artifact::get(&ctx.catalog.lock(), "a1").unwrap().is_some());
    }

    /// A dead root an ACTIVE worktree registration covers must be excluded
    /// from the dry-run `totals` (marked `would_skip` instead) so the preview
    /// promises exactly what `confirm=true` will actually delete — before this
    /// fix the dry-run counted every dead root's rows even though the
    /// apply-time skip would leave the covered root untouched.
    #[tokio::test]
    async fn prune_missing_batch_dry_run_excludes_worktree_covered_root_from_totals() {
        let cat = Catalog::open_in_memory().unwrap();
        let live_parent = tempfile::tempdir().unwrap();
        let covered_root = live_parent.path().join("repo");
        // Seed the RepoPath-normalised (forward-slash) shape that production
        // writers store. `to_string_lossy()` on a joined PathBuf yields
        // BACKSLASHES on Windows, while `count_dead_root` builds its LIKE
        // prefix through `RepoPath::from_path` — so a backslash seed matched
        // nothing there and the per-root count came back 0.
        seed_artifact(
            &cat,
            "wt",
            &crate::util::fs::RepoPath::from_path(&covered_root.join("x.md")).into_string(),
        );
        let covered_root_str = crate::util::fs::RepoPath::from_path(&covered_root).to_string();
        reg::upsert_active(&cat, &covered_root_str, &covered_root_str, None, 1000).unwrap();
        // A second, uncovered dead root so the totals assertion isn't
        // vacuously true either way.
        seed_artifact(
            &cat,
            "uncovered",
            &format!("{}/repo/y.md", dead_root("prune-uncovered")),
        );
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing" })).await.unwrap();
        assert_eq!(v["mode"], "dry_run");

        let rows = v["dead_roots"].as_array().unwrap();
        let covered_row = rows
            .iter()
            .find(|r| {
                // Normalise both sides. The dry-run row's `root` is a PathBuf
                // built out of the (forward-slash) seeded abs_path, while
                // `covered_root` is a tempdir join that renders with BACKSLASHES
                // on Windows — comparing the raw strings compares two spellings
                // of the same path and never matches there.
                crate::util::fs::RepoPath::from_path(std::path::Path::new(
                    r["root"].as_str().unwrap(),
                ))
                .to_string()
                    == covered_root_str
            })
            .expect("covered root present in dry-run preview");
        assert!(
            covered_row["would_skip"]
                .as_str()
                .expect("covered root marked would_skip")
                .contains("active worktree registration"),
            "would_skip names the guard: {covered_row}"
        );
        assert_eq!(
            covered_row["artifact_rows"].as_u64().unwrap(),
            1,
            "per-root counts are still shown even for a covered root"
        );

        assert_eq!(
            v["totals"]["artifact_rows"].as_u64().unwrap(),
            1,
            "only the uncovered root's row counts toward the aggregate total"
        );
    }

    #[tokio::test]
    async fn prune_missing_batch_confirm_prunes_dead_roots_only() {
        let cat = Catalog::open_in_memory().unwrap();
        let live = tempfile::tempdir().unwrap();
        let live_file = live.path().join("here.md");
        std::fs::write(&live_file, "x").unwrap();
        seed_artifact(
            &cat,
            "dead",
            &format!("{}/x.md", dead_root("prune-confirm")),
        ); // gone subtree
        seed_artifact(&cat, "live", &live_file.to_string_lossy()); // live file
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing", "confirm": true }))
            .await
            .unwrap();
        assert_eq!(v["mode"], "applied");
        assert_eq!(v["totals"]["artifact_rows"].as_u64().unwrap(), 1);
        assert!(
            artifact::get(&ctx.catalog.lock(), "dead")
                .unwrap()
                .is_none(),
            "dead row pruned"
        );
        assert!(
            artifact::get(&ctx.catalog.lock(), "live")
                .unwrap()
                .is_some(),
            "live row kept"
        );
    }

    /// The worktree-registration skip is the ONLY delete-safety guard in the
    /// batch path: a dead root an ACTIVE `worktree_registration` still covers
    /// must be reported `skipped`, not pruned, or the catalog's only record of
    /// an unmerged worktree's history is deleted out from under it. If the
    /// `.is_some()` check in `run_fix` were inverted (or removed), this test
    /// fails: the row would be gone and `totals.artifact_rows` would be 1.
    #[tokio::test]
    async fn prune_missing_batch_skips_dead_root_covered_by_active_registration() {
        let cat = Catalog::open_in_memory().unwrap();
        // A real tempdir boundary (exists on disk) pins the derived dead root
        // to exactly `dead_root`, mirroring
        // `derive_dead_roots_groups_gone_subtrees_and_skips_live_dir_files`.
        let live_parent = tempfile::tempdir().unwrap();
        let dead_root = live_parent.path().join("repo");
        seed_artifact(&cat, "wt", &dead_root.join("x.md").to_string_lossy());
        let dead_root_str = crate::util::fs::RepoPath::from_path(&dead_root).to_string();
        reg::upsert_active(&cat, &dead_root_str, &dead_root_str, None, 1000).unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({ "fix": "prune_missing", "confirm": true }))
            .await
            .unwrap();

        assert_eq!(v["mode"], "applied");
        let pruned = v["pruned"].as_array().unwrap();
        assert_eq!(pruned.len(), 1, "the one dead root must be reported");
        let msg = pruned[0]["skipped"]
            .as_str()
            .expect("covered root reports 'skipped', not 'artifact_rows'");
        assert!(
            msg.contains("active worktree registration"),
            "skip reason names the guard: {msg}"
        );
        assert_eq!(
            v["totals"]["artifact_rows"].as_u64().unwrap(),
            0,
            "nothing pruned while the registration is active"
        );
        assert!(
            artifact::get(&ctx.catalog.lock(), "wt").unwrap().is_some(),
            "seeded row must survive — the registration guard protects it"
        );
    }

    #[test]
    fn scan_worktree_scoped_empty_when_no_worktree_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        // Plain rows with no linked-worktree ancestor anywhere on disk —
        // the scan must not flag anything (safe default).
        seed_artifact(&cat, "plain", "/tmp/plain/doc.md");
        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert!(violations.is_empty());
    }

    /// Builds a real `<tmp>/main` + linked-worktree-under-main layout on disk
    /// (a `.git` FILE at the worktree root pointing `gitdir:` back at the
    /// main repo's `.git/worktrees/<name>`), matching exactly what
    /// `is_linked_worktree` / `worktree_main_root` read. Returns
    /// `(tmp, main_root, worktree_root)`; `tmp` must stay alive for the
    /// duration of the test (dropping it deletes the directory).
    fn make_worktree_fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let main_root = tmp.path().join("main");
        std::fs::create_dir_all(main_root.join(".git")).unwrap();
        let worktree_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(&worktree_root).unwrap();
        std::fs::write(
            worktree_root.join(".git"),
            format!(
                "gitdir: {}/main/.git/worktrees/feat\n",
                tmp.path().display()
            ),
        )
        .unwrap();
        (tmp, main_root, worktree_root)
    }

    #[test]
    fn scan_worktree_scoped_classifies_no_collision() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        // Proves is_linked_worktree/worktree_main_root actually fired: with
        // no real .git-file layout on disk this would be empty (see the
        // no-worktree-rows test above). Here it must find exactly the one
        // seeded row.
        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(violations.len(), 1, "the worktree-scoped row is flagged");
        let v = &violations[0];
        assert_eq!(v.check, "worktree_scoped_row");
        assert_eq!(v.artifact_id.as_deref(), Some("wt-row"));

        let detail: serde_json::Value = serde_json::from_str(&v.detail).unwrap();
        assert_eq!(detail["classification"], "no_collision");
        assert!(detail.get("collision_with").is_none());
        let main_doc = main_root.join("docs/x.md");
        assert_eq!(
            detail["main_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );
    }

    #[test]
    fn scan_worktree_scoped_classifies_collision_and_overlap() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        // The collision classifier computes the main-path id via
        // artifact_id_from_abs and checks whether a row with that id
        // exists — so the seeded main-side row's id MUST be exactly that.
        let main_id = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        let main_row = TestArtifactRowBuilder::new(&main_id)
            .with_abs_path(main_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &main_row).unwrap();

        // Augment both sides with the SAME entry_collection name but
        // partially-overlapping id sets — "b" is the only shared id.
        augmentation::upsert(&cat, &aug_row("wt-row", "items", &["a", "b"])).unwrap();
        augmentation::upsert(&cat, &aug_row(&main_id, "items", &["b", "c"])).unwrap();

        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(
            violations.len(),
            1,
            "only the worktree-side row is scanned; the main-side row has no \
             linked-worktree ancestor and is skipped"
        );
        let v = &violations[0];
        assert_eq!(v.artifact_id.as_deref(), Some("wt-row"));

        let detail: serde_json::Value = serde_json::from_str(&v.detail).unwrap();
        assert_eq!(detail["classification"], "collision");
        assert_eq!(detail["collision_with"], main_id);
        assert_eq!(
            detail["main_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );
        let overlap: Vec<String> = detail["id_overlap"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(overlap, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn reseat_worktree_repoints_no_collision_row_without_rename() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let id_m = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, None, false)
            .await
            .unwrap();
        assert_eq!(out["fix"], "reseat_worktree");
        assert_eq!(out["reseated"].as_array().unwrap().len(), 1);
        assert!(out["collisions"].as_array().unwrap().is_empty());
        assert_eq!(out["reseated"][0]["old_id"], "wt-row");
        assert_eq!(out["reseated"][0]["new_id"], id_m);

        // The row is durably re-seeded at id_m (== hash(main_path)) rather than
        // merely re-pointed under the stale worktree-derived id: no filesystem
        // rename (the merged file already lives there; only the catalog
        // moved), but the catalog id DOES change so identity
        // (id == hash(abs_path)) holds and the worktree-id row is gone.
        let expected_main = crate::util::fs::RepoPath::from_path(&main_doc).to_string();
        let cat = ctx.catalog.lock();
        let abs_path: String = cat
            .conn
            .query_row(
                "SELECT abs_path FROM artifact WHERE id = ?1",
                params![id_m],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(abs_path, expected_main);
        assert!(artifact::get(&cat, "wt-row").unwrap().is_none());
    }

    #[tokio::test]
    async fn reseat_worktree_leaves_collisions_for_graft() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let main_id = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        let main_row = TestArtifactRowBuilder::new(&main_id)
            .with_abs_path(main_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &main_row).unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, None, false)
            .await
            .unwrap();
        assert!(out["reseated"].as_array().unwrap().is_empty());
        assert_eq!(out["collisions"].as_array().unwrap().len(), 1);

        // Collision rows are reported, never mutated: the worktree row's
        // abs_path must still point at the worktree, not the main path.
        let expected_wt = crate::util::fs::RepoPath::from_path(&wt_doc).to_string();
        let cat = ctx.catalog.lock();
        let abs_path: String = cat
            .conn
            .query_row(
                "SELECT abs_path FROM artifact WHERE id = ?1",
                params!["wt-row"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(abs_path, expected_wt);
    }

    /// The durability proof: a `no_collision` reseat must survive the NEXT
    /// main-repo reindex without losing history. Seeds a worktree-scoped row
    /// WITH an augmentation (`entry_collection` + params) and an event, runs
    /// `reseat_worktree`, then simulates the next reindex's `artifact::upsert`
    /// at the main path — before this fix, that upsert's abs_path-collision
    /// pre-clean (`DELETE FROM artifact WHERE abs_path=? AND id != ?`) would
    /// fire against the stale worktree-derived id and cascade-drop the
    /// augmentation; this test would have failed on `main` prior to the fix.
    #[tokio::test]
    async fn reseat_worktree_durably_reseeds_and_survives_reindex() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");
        let main_doc = main_root.join("docs/x.md");
        let id_m = crate::librarian::ids::artifact_id_from_abs(&main_doc);

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();
        augmentation::upsert(&cat, &aug_row("wt-row", "items", &["a", "b"])).unwrap();
        events::insert(
            &cat,
            &TestEventRowBuilder::new("wt-row", "note")
                .with_id("ev-1")
                .build(),
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat).build();

        let out = run_fix(&ctx, "reseat_worktree", None, None, false)
            .await
            .unwrap();
        assert_eq!(out["reseated"].as_array().unwrap().len(), 1);
        assert!(out["collisions"].as_array().unwrap().is_empty());
        assert_eq!(out["reseated"][0]["old_id"], "wt-row");
        assert_eq!(out["reseated"][0]["new_id"], id_m);
        assert_eq!(
            out["reseated"][0]["new_path"],
            crate::util::fs::RepoPath::from_path(&main_doc).to_string()
        );

        {
            let cat = ctx.catalog.lock();

            // Catalog identity restored: a row lives at id_m, the stale
            // worktree-id row is gone.
            assert!(artifact::get(&cat, &id_m).unwrap().is_some());
            assert!(artifact::get(&cat, "wt-row").unwrap().is_none());

            // The augmentation (git-invisible append_entry history) migrated.
            let aug = augmentation::get(&cat, &id_m).unwrap().unwrap();
            let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
            let ids: Vec<&str> = params["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["id"].as_str().unwrap())
                .collect();
            assert_eq!(ids, vec!["a", "b"]);

            // The event followed too.
            let ev_artifact_id: String = cat
                .conn
                .query_row(
                    "SELECT artifact_id FROM events WHERE id = ?1",
                    params!["ev-1"],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ev_artifact_id, id_m);
        }

        // Simulate the NEXT main-repo reindex walk: a fresh ArtifactRow at the
        // same id a real walk would compute (hash(main_path)) upserts onto the
        // row this fix just reseeded.
        {
            let cat = ctx.catalog.lock();
            let reindexed_row = TestArtifactRowBuilder::new(&id_m)
                .with_abs_path(main_doc.clone())
                .with_kind("tracker")
                .build();
            art_upsert(&cat, &reindexed_row).unwrap();

            // The durability guarantee: the augmentation is STILL there. Before
            // the fix, id_m would have still been id_w at this point, so the
            // upsert's abs_path pre-clean would have deleted the row (and its
            // cascaded augmentation) out from under this reindex.
            let aug = augmentation::get(&cat, &id_m).unwrap().unwrap();
            let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
            assert_eq!(
                params["items"].as_array().unwrap().len(),
                2,
                "augmentation survives the next reindex"
            );
        }
    }

    /// A worktree-scoped row covered by an ACTIVE `worktree_registration` is
    /// pending merge, not a legacy orphan — `scan_worktree_scoped` must flag
    /// it as `registered` (with a hint pointing at `merge_worktree`), and
    /// `reseat_worktree` must SKIP it rather than reseating it: reseating
    /// would sever the row from the registration's overlay bookkeeping that
    /// `merge_worktree` depends on.
    #[tokio::test]
    async fn worktree_scoped_row_marks_registered_rows_pending_merge() {
        let (_tmp, main_root, worktree_root) = make_worktree_fixture();
        let wt_doc = worktree_root.join("docs/x.md");

        let cat = Catalog::open_in_memory().unwrap();
        let wt_row = TestArtifactRowBuilder::new("wt-row")
            .with_abs_path(wt_doc.clone())
            .with_kind("tracker")
            .build();
        art_upsert(&cat, &wt_row).unwrap();

        let worktree_root_str = crate::util::fs::RepoPath::from_path(&worktree_root).to_string();
        let main_root_str = crate::util::fs::RepoPath::from_path(&main_root).to_string();
        reg::upsert_active(&cat, &worktree_root_str, &main_root_str, None, 1000).unwrap();

        let violations = scan_worktree_scoped(&cat.conn).unwrap();
        assert_eq!(violations.len(), 1);
        let detail: serde_json::Value = serde_json::from_str(&violations[0].detail).unwrap();
        assert_eq!(
            detail["registered"], true,
            "an ACTIVE registration covers this worktree root"
        );
        assert!(
            detail["hint"]
                .as_str()
                .unwrap_or_default()
                .contains("merge_worktree"),
            "registered rows point at merge_worktree, not reseat: {detail}"
        );

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = run_fix(&ctx, "reseat_worktree", None, None, false)
            .await
            .unwrap();
        assert!(
            out["reseated"].as_array().unwrap().is_empty(),
            "a registered row must not be reseated — it belongs to merge_worktree: {out}"
        );
        assert_eq!(
            out["skipped"].as_array().unwrap().len(),
            1,
            "the registered row is reported as skipped: {out}"
        );
    }

    /// Ruling 17 for the row-grain checks, and the deliberate exception beside it.
    ///
    /// Three rows, one call, because the exception is the part a future reader will try
    /// to "finish": `worktree_scoped_row` looks exactly like the four scoped checks —
    /// row-grain, 100% foreign in the live report — and adding it would be wrong.
    /// `fix=reseat_worktree` takes no root and filters by none, so a narrowed report
    /// would understate what `confirm=true` is about to reseat. Its sibling
    /// `repair_frontmatter_id` DOES write files, refuses to run without a scope, and
    /// already filters to one root — which is why `frontmatter_id_mismatch`'s report was
    /// the outlier rather than its repair.
    ///
    /// Regression for
    /// docs/issues/archive/2026-08-27-doctor-still-reports-52pct-foreign-rows-via-six-other-checks.md.
    #[tokio::test]
    async fn row_grain_checks_scope_to_the_project_but_worktree_scoped_row_does_not() {
        let (_wt_tmp, _main_root, worktree_root) = make_worktree_fixture();
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        let cat = Catalog::open_in_memory().unwrap();

        // A stale frontmatter id (declared 16-hex, differs from the catalog row) is the
        // cheapest of the four scoped checks to seed, and the one whose repair is
        // root-scoped — so it is the check the scoping is supposed to bring into line.
        let stale = "---\nid: aaaaaaaaaaaaaaaa\nkind: bug\n---\n\n# x\n";
        seed_ledger(
            &cat,
            "bbbbbbbbbbbbbbbb",
            &active_root.join("docs/in.md"),
            stale,
        );
        seed_ledger(
            &cat,
            "cccccccccccccccc",
            &sibling_root.join("docs/out.md"),
            stale,
        );
        // Outside the active project, and must survive anyway.
        seed_ledger(
            &cat,
            "wt-row",
            &worktree_root.join("docs/w.md"),
            "# plain\n",
        );

        let ctx = ctx_rooted_at(cat, &active_root);
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["frontmatter_id_mismatch"],
            json!(1),
            "the sibling-root row must be scoped out and the in-project one kept: {out:#?}"
        );
        let kept: Vec<&serde_json::Value> = out["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["check"] == "frontmatter_id_mismatch")
            .collect();
        assert!(
            kept[0]["path"].as_str().unwrap().contains("active-project"),
            "the survivor is the ACTIVE project's row, not whichever sorted first: {:#?}",
            kept[0]
        );

        let scoped = &out["catalog_health"]["row_checks_scoped_by_project"];
        let total: u64 = scoped
            .as_object()
            .expect("the drop must be announced, not silent")
            .values()
            .map(|n| n.as_u64().unwrap())
            .sum();
        assert_eq!(total, 1, "the dropped row must be COUNTED: {scoped:#?}");
        assert!(
            scoped
                .as_object()
                .unwrap()
                .keys()
                .any(|k| k.contains("sibling-project")),
            "attributed to the project that owns it: {scoped:#?}"
        );

        assert_eq!(
            out["summary"]["by_check"]["worktree_scoped_row"],
            json!(1),
            "worktree_scoped_row must NOT scope — fix=reseat_worktree reseats every \
             unregistered row in the catalog regardless of root, so a narrowed report \
             would understate what confirm=true is about to do: {out:#?}"
        );
    }

    /// `prune_missing` must refuse a dead root that an ACTIVE registration
    /// still covers — the worktree was `git worktree remove`d before its
    /// shadow rows were merged, and pruning would delete the catalog's only
    /// remaining record of that unmerged history.
    #[test]
    fn prune_missing_refuses_root_with_active_registration() {
        let cat = Catalog::open_in_memory().unwrap();
        let dead_root = std::env::temp_dir().join("codescout-nonexistent-registered-root-9c2e7b1a");
        assert!(!dead_root.exists(), "test fixture path must not exist");
        let dead_root_str = dead_root.to_str().expect("temp path is valid UTF-8");
        let normalized = crate::util::fs::RepoPath::from_path(&dead_root).to_string();
        reg::upsert_active(&cat, &normalized, &normalized, None, 1000).unwrap();

        let err =
            validate_prune_request("prune_missing", Some(dead_root_str), &cat.conn).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("merge_worktree"),
            "hint names merge_worktree: {msg}"
        );
        assert!(msg.contains("abandon"), "hint names abandon=true: {msg}");
    }

    fn aug_row(
        artifact_id: &str,
        entry_collection: &str,
        ids: &[&str],
    ) -> crate::librarian::catalog::augmentation::AugmentationRow {
        let items: Vec<serde_json::Value> =
            ids.iter().map(|i| serde_json::json!({ "id": i })).collect();
        crate::librarian::catalog::augmentation::AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: "test prompt".to_string(),
            params: serde_json::json!({ entry_collection: items }).to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some(entry_collection.to_string()),
            refreshed_at_commit: None,
        }
    }

    // ---- declared_root_missing ----------------------------------------------------

    fn write_workspace_toml(root: &std::path::Path, body: &str) {
        let dir = root.join(".codescout");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("workspace.toml"), body).unwrap();
    }

    fn declared_ctx(abs_path: std::path::PathBuf, git_root: std::path::PathBuf) -> ToolContext {
        let cp = std::sync::Arc::new(crate::librarian::current_project::CurrentProject {
            abs_path,
            git_root,
            main_root: None,
            umbrella: None,
        });
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(cp)
            .build()
    }

    async fn doctor_at(root: std::path::PathBuf) -> Value {
        let ctx = declared_ctx(root.clone(), root);
        call(&ctx, json!({})).await.unwrap()
    }

    fn violations_named<'a>(out: &'a Value, check: &str) -> Vec<&'a Value> {
        out["violations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["check"] == check)
            .collect()
    }

    /// Two declarations that resolve and one that does not.
    ///
    /// The two good entries are not decoration: a check that fired on every declaration
    /// would pass a test that only planted a bad one, and `root = "."` is the entry every
    /// real config carries.
    #[tokio::test]
    async fn declared_root_missing_fires_only_on_the_root_that_is_not_a_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join("crates").join("real-subproject")).unwrap();
        write_workspace_toml(
            &root,
            r#"
[workspace]
name = "fixture"

[[project]]
id = "root"
root = "."

[[project]]
id = "real"
root = "crates/real-subproject"

[[project]]
id = "ghost"
root = "work/elsewhere/ghost"
"#,
        );

        let out = doctor_at(root).await;
        let fired = violations_named(&out, "declared_root_missing");
        assert_eq!(
            fired.len(),
            1,
            "only the missing root may fire; `.` and an existing subdir must not: {fired:#?}"
        );
        let detail = fired[0]["detail"].as_str().unwrap();
        assert!(detail.contains("\"ghost\""), "names the id: {detail}");
        assert!(
            detail.contains("work/elsewhere/ghost"),
            "names the declared root: {detail}"
        );
        let health = &out["catalog_health"]["declared_roots"];
        assert_eq!(
            health["declared"], 3,
            "counts every declaration, not just the failures"
        );
        assert_eq!(health["missing"], 1);
        assert!(
            out["catalog_health"]["hint"]
                .as_str()
                .unwrap()
                .contains("declared_root_missing"),
            "the hint must surface it — a finding nobody is pointed at is not a report"
        );
    }

    /// The `Path::join` trap. Joining an ABSOLUTE declared root onto the base DISCARDS the
    /// base, so an absolute declaration would be validated against itself and pass. The
    /// decoy here is a directory that really exists outside the workspace — the exact
    /// shape of the bug this check was written for, where sibling repos stood declared as
    /// sub-projects — so the only thing that can make this fail is the `is_absolute`
    /// branch actually running.
    #[tokio::test]
    async fn declared_root_missing_fires_on_an_absolute_root_even_though_it_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let sibling = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        assert!(sibling.path().is_dir(), "the decoy must really exist");
        let elsewhere = sibling.path().to_str().unwrap().replace('\\', "/");
        write_workspace_toml(
            &root,
            &format!(
                "[workspace]\nname = \"fixture\"\n\n\
                 [[project]]\nid = \"sibling-repo\"\nroot = \"{elsewhere}\"\n"
            ),
        );

        let out = doctor_at(root).await;
        let fired = violations_named(&out, "declared_root_missing");
        assert_eq!(
            fired.len(),
            1,
            "an absolute root must never pass by being joined onto itself: {fired:#?}"
        );
        let detail = fired[0]["detail"].as_str().unwrap();
        assert!(detail.contains("ABSOLUTE"), "says what is wrong: {detail}");
        assert!(
            detail.contains("[[umbrella]]"),
            "names where a sibling repo belongs: {detail}"
        );
    }

    /// A declared root that resolves to a FILE. The spec is "exists and is a directory",
    /// and only the second half is load-bearing here — a `root` pointing at a file routes
    /// every per-project write under a path that can never hold a directory.
    ///
    /// Added because the mutation `is_dir()` -> `exists()` survived the first four tests:
    /// every fixture used a root that was absent entirely, so the distinction the check
    /// claims to make was never once executed. Observed, not reasoned — 5/5 stayed green
    /// under the mutation before this test existed.
    #[tokio::test]
    async fn declared_root_missing_fires_when_the_root_exists_but_is_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::write(root.join("not-a-dir"), "i am a file").unwrap();
        write_workspace_toml(
            &root,
            "[workspace]\nname = \"fixture\"\n\n\
             [[project]]\nid = \"impostor\"\nroot = \"not-a-dir\"\n",
        );

        let out = doctor_at(root).await;
        let fired = violations_named(&out, "declared_root_missing");
        assert_eq!(
            fired.len(),
            1,
            "a file is not a project root, however present it is: {fired:#?}"
        );
        let detail = fired[0]["detail"].as_str().unwrap();
        assert!(
            detail.contains("exists but is not a directory"),
            "distinguishes this from an absent path, since the two need different fixes: \
             {detail}"
        );
    }

    /// A skip must not read as a pass. A linked worktree carries no config of its own —
    /// the file is gitignored and does not travel — so discovery inherits main's and this
    /// check reports nothing. The `note` is the only thing separating that from a clean
    /// bill of health.
    #[tokio::test]
    async fn declared_roots_states_the_worktree_skip_instead_of_reporting_a_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        // Deliberately no .codescout/workspace.toml.
        let cp = std::sync::Arc::new(crate::librarian::current_project::CurrentProject {
            abs_path: root.clone(),
            git_root: root,
            main_root: Some(std::path::PathBuf::from("/main/checkout")),
            umbrella: None,
        });
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(cp)
            .build();

        let out = call(&ctx, json!({})).await.unwrap();
        assert!(violations_named(&out, "declared_root_missing").is_empty());
        let health = &out["catalog_health"]["declared_roots"];
        assert!(health["config"].is_null());
        let note = health["note"].as_str().unwrap();
        assert!(note.contains("worktree"), "names the reason: {note}");
        assert!(
            note.contains("NOT checked"),
            "says it did not check, not that it passed: {note}"
        );
    }

    /// The misleading-green case: zero violations, and no reason to believe zero. A config
    /// that cannot be parsed is reported as unchecked rather than left silent.
    #[tokio::test]
    async fn declared_roots_marks_an_unparseable_config_as_unchecked_not_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        write_workspace_toml(&root, "this is not = = toml [[[\n");

        let out = doctor_at(root).await;
        assert!(violations_named(&out, "declared_root_missing").is_empty());
        let note = out["catalog_health"]["declared_roots"]["note"]
            .as_str()
            .unwrap();
        assert!(note.contains("unparseable"), "{note}");
        assert!(note.contains("NOT checked"), "{note}");
        assert!(note.contains("not a pass"), "{note}");
    }

    /// The base and the config come from ONE variable. Here the config lives at the
    /// project root while `git_root` is an ancestor carrying none, and a same-named
    /// `decoy/` directory exists under the git root only — so resolving against the wrong
    /// base would find something and pass.
    ///
    /// That is not hypothetical symmetry: validating a mis-rooted config against the root
    /// it was mis-rooted *from* is precisely how the original defect went unnoticed for a
    /// month.
    #[tokio::test]
    async fn declared_roots_resolve_against_the_directory_owning_the_config() {
        let tmp = tempfile::tempdir().unwrap();
        let git_root = tmp.path().to_path_buf();
        let project = git_root.join("sub").join("project");
        std::fs::create_dir_all(project.join("inner")).unwrap();
        std::fs::create_dir_all(git_root.join("decoy")).unwrap();
        write_workspace_toml(
            &project,
            "[workspace]\nname = \"fixture\"\n\n\
             [[project]]\nid = \"inner\"\nroot = \"inner\"\n\n\
             [[project]]\nid = \"decoy\"\nroot = \"decoy\"\n",
        );

        let ctx = declared_ctx(project, git_root);
        let out = call(&ctx, json!({})).await.unwrap();

        let fired = violations_named(&out, "declared_root_missing");
        assert_eq!(
            fired.len(),
            1,
            "`inner` resolves under the config's own directory; `decoy` exists only under \
             the git root and must fire: {fired:#?}"
        );
        assert!(fired[0]["detail"].as_str().unwrap().contains("\"decoy\""));
        assert!(
            out["catalog_health"]["declared_roots"]["config"]
                .as_str()
                .unwrap()
                .contains("sub/project"),
            "reports the config it actually read, so the base is auditable"
        );
    }

    // ---- entry_indegree / scan_conditional_past_due ------------------------------------

    /// A minimal entry ledger: write it to disk AND seed its catalog row, since both
    /// checks read the file (for citations/declarations) and the catalog (for the
    /// abs_path to read it from).
    fn seed_ledger(cat: &Catalog, id: &str, path: &std::path::Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        let row = TestArtifactRowBuilder::new(id).with_abs_path(path).build();
        art_upsert(cat, &row).unwrap();
    }

    /// A ctx with no active project, for the entry-validity scan family's tests that
    /// exercise exposure/declaration/archived-row logic and don't care about project
    /// scoping (Fix 2, MF-1). `current_project: None` is exactly the "no scoping —
    /// report everything" case the scans themselves implement, so these tests see the
    /// same unscoped behaviour they pinned before ctx was threaded through. Backed by
    /// its own throwaway in-memory catalog — the scans only read `ctx.current_project`,
    /// never `ctx.catalog`, so it need not be the same catalog as the test's `cat`.
    fn unscoped_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    /// The exposure-map key: `entry_indegree` counts against the file that DEFINES the
    /// entry, so a test faking exposure must name the same definer or the scan looks up a
    /// key that is not there. Taking the path explicitly is the point — `F-1` is defined
    /// in every session log, and a bare token no longer identifies a Statement.
    fn deg_key(path: &std::path::Path, token: impl Into<String>) -> (String, String) {
        // `RepoPath`, not `to_string_lossy`: the scans key this map on the CATALOG's
        // `abs_path`, which is forward-slash by invariant (check #1). `to_string_lossy`
        // yields the NATIVE spelling, so on Windows every lookup missed, exposure read as
        // zero, and 30 of this module's tests asserted 1 and got 0 — green on Linux,
        // red on all three Windows lanes since 2026-08-20.
        (
            crate::util::fs::RepoPath::from_path(path).into_string(),
            token.into(),
        )
    }

    #[test]
    fn indegree_counts_cross_file_citations_and_excludes_the_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("b.md");
        let c = tmp.path().join("c.md");
        let cat = Catalog::open_in_memory().unwrap();
        // a.md DEFINES R-1 and cites it twice in its own index table and prose — neither
        // may count, because a same-file citation is a SelfCite, not exposure.
        seed_ledger(
            &cat,
            "a",
            &a,
            "## Index\n| R-1 | x |\n\n## R-1 — the law\n\nas R-1 says, ...\n",
        );
        // Two DISTINCT files cite R-1 from outside. `extract()` dedups citations of the
        // same (kind, token) WITHIN one document (see `entry_indegree`'s doc comment), so
        // this is deliberately two files rather than one file citing twice — that would
        // measure something else.
        seed_ledger(&cat, "b", &b, "see R-1\n");
        seed_ledger(&cat, "c", &c, "and again R-1\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&a, "R-1")).copied(),
            Some(2),
            "b.md and c.md's citations both count — a.md defines R-1, so its own index \
         row and prose must not inflate its exposure"
        );
    }

    #[test]
    fn indegree_takes_the_token_half_of_a_cross_repo_citation() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("b.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, "a", &a, "## R-9 — the law\n\nprose only\n");
        seed_ledger(&cat, "b", &b, "see other:R-9 from over here\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&a, "R-9")).copied(),
            Some(1),
            "a qualified `other:R-9` citation must be attributed to the token half, R-9 — \
         the same idiom corpus_cited_tokens uses"
        );
    }

    #[test]
    fn conditional_past_due_fires_only_above_the_exposure_gate() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-1 — exposed\n\n**Valid:** conditional — until the plan edit lands\n\n\
         ## R-2 — ignored\n\n**Valid:** conditional — until something else\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-1"), 9usize);
        deg.insert(deg_key(&p, "R-2"), 1usize);

        let (v, _) = scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "only the exposed conditional is worth anyone's time: {v:#?}"
        );
        assert!(v[0].detail.contains("R-1"));
        assert!(
            v[0].detail.contains("until the plan edit lands"),
            "the worklist must carry the condition to adjudicate, not just an id"
        );
        assert_eq!(v[0].check, "entry_conditional_past_due");
    }

    #[test]
    fn conditional_past_due_fires_exactly_at_the_exposure_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-3 — at the line\n\n**Valid:** conditional — the boundary case\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-3"), EXPOSURE_THRESHOLD);
        assert_eq!(
            scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .len(),
            1,
            "exposure == EXPOSURE_THRESHOLD must fire — the gate is `<`, not `<=`"
        );

        deg.insert(deg_key(&p, "R-3"), EXPOSURE_THRESHOLD - 1);
        assert!(
            scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "one below the threshold must not fire"
        );
    }

    #[test]
    fn conditional_past_due_does_not_read_a_nested_childs_declaration_as_the_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        // PV-2 (the parent, level 3) declares nothing itself; its nested child PV-8
        // (level 4) sits WHOLLY inside PV-2's section text and declares conditional.
        // Without truncation, PV-2 would read PV-8's declaration as its own under
        // `parse_validity`'s first-wins rule.
        seed_ledger(
            &cat,
            "led",
            &p,
            "### PV-2 — parent, no declaration of its own\n\n\
         prose about the parent\n\n\
         #### PV-8 — nested child\n\n\
         **Valid:** conditional — the child's own event\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "PV-2"), EXPOSURE_THRESHOLD);
        deg.insert(deg_key(&p, "PV-8"), EXPOSURE_THRESHOLD);

        let (v, _) = scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "PV-2 declares nothing of its own and must not inherit PV-8's declaration: {v:#?}"
        );
        assert!(v[0].detail.contains("PV-8"));
    }

    #[test]
    fn conditional_past_due_swallows_a_malformed_valid_line_rather_than_reporting_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-7 — malformed\n\n**Valid:** not-a-real-class\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-7"), EXPOSURE_THRESHOLD);

        assert!(
            scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a malformed declaration is `validity_unparseable`'s finding, not this check's"
        );
    }

    #[test]
    fn conditional_past_due_treats_a_token_absent_from_indegree_as_zero_exposure() {
        // Same defect class as `scan_dated_stale`'s identical `unwrap_or(0)` line
        // (`entry_indegree` omits uncited/ambiguous tokens rather than inserting a
        // zero for them, so the missing-key branch is the majority case in the live
        // corpus). This adds a test to `scan_conditional_past_due` without touching its
        // logic or any pre-existing test — pinning the defect CLASS, not a one-off.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-52 — never mentioned in indegree\n\n**Valid:** conditional — until \
             something happens\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-98"), EXPOSURE_THRESHOLD + 50);

        assert!(
            scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a token with no entry in `indegree` at all must be treated as zero \
             exposure, not skip the gate"
        );
    }

    #[test]
    fn conditional_past_due_skips_an_archived_definer_located_under_archive_path() {
        let tmp = tempfile::tempdir().unwrap();
        let archive_dir = tmp.path().join("archive");
        let p = archive_dir.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-60 — archived by path\n\n**Valid:** conditional — until X\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-60"), EXPOSURE_THRESHOLD + 3);

        assert!(
            scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a definer located under an /archive/ path segment must not be reported \
             even though its exposure clears the gate — mirrors entry_indegree's \
             Ruling 15/16 on the citing side"
        );
    }

    #[test]
    fn conditional_past_due_reports_only_the_active_twin_when_a_token_has_an_archived_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let active = tmp.path().join("active.md");
        let archived = tmp.path().join("archived.md");
        seed_ledger(
            &cat,
            "active",
            &active,
            "## R-61 — active twin\n\n**Valid:** conditional — until Y\n",
        );
        std::fs::write(
            &archived,
            "## R-61 — archived twin\n\n**Valid:** conditional — until Y\n",
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new("archived")
            .with_abs_path(&archived)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row).unwrap();

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&active, "R-61"), EXPOSURE_THRESHOLD + 3);

        let (v, _) = scan_conditional_past_due(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "R-61 is defined in both an active and an archived file; only the active \
             one may be reported — the archived twin must not double the finding: {v:#?}"
        );
        assert_eq!(v[0].artifact_id, Some("active".to_string()));
    }

    #[test]
    fn indegree_omits_a_token_with_more_than_one_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("b.md");
        let c = tmp.path().join("c.md");
        // F-1 is defined in BOTH a.md and b.md — two unrelated per-work-stream session
        // logs reusing the same low number, exactly the F-N/W-N collision Ruling 14
        // exists for. c.md cites it from outside either.
        seed_ledger(
            &cat,
            "a",
            &a,
            "## F-1 — stream A's first finding\n\nprose\n",
        );
        seed_ledger(
            &cat,
            "b",
            &b,
            "## F-1 — stream B's first finding\n\nprose\n",
        );
        seed_ledger(&cat, "c", &c, "cites F-1 from a third file\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            (deg.get(&deg_key(&a, "F-1")), deg.get(&deg_key(&b, "F-1")),),
            (None, None),
            "a token with 2 definers must contribute NO exposure — its key must be absent \
         entirely, not merely zero: {deg:?}"
        );
    }
    /// The defect the definer-keying exists to remove. Before it, `F-1`'s two definers
    /// made the bare token ambiguous and it contributed NO exposure at all — measured
    /// 2026-08-21, that exempted 252 `F`/`W` Statements carrying 442 entry-grain edges
    /// from all three exposure-gated checks, and all 32 live
    /// `entry_cited_from_outside_but_undeclared` rows named some other prefix. A
    /// stem-qualified citation names ONE definer, so it resolves.
    ///
    /// **The exemption's size and the worklist delta are different numbers, in different
    /// units.** This function counts distinct citing FILES and the gate is 5 of them,
    /// where those 442 are entry-to-entry EDGES — many from one file. Across all 252 F/W
    /// destinations `entry_cite`'s maximum distinct-citer count is 4, so the immediate
    /// effect of this fix on the reported worklist was a single row (`F-9`, at 5 citing
    /// files, which clears only because non-ledger citers — specs, bug files, READMEs —
    /// produce exposure here while producing no `entry_cite` row at all). The mechanism is
    /// what changed; the population it will price grows as qualified citation spreads.
    #[test]
    fn indegree_attributes_a_stem_qualified_citation_to_its_specific_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let a = tmp.path().join("stream-a-log.md");
        let b = tmp.path().join("stream-b-log.md");
        let c = tmp.path().join("citer.md");
        seed_ledger(
            &cat,
            "a",
            &a,
            "## F-1 — stream A's first finding\n\nprose\n",
        );
        seed_ledger(
            &cat,
            "b",
            &b,
            "## F-1 — stream B's first finding\n\nprose\n",
        );
        // Qualified by file STEM — the form `get_guide("tracker-conventions")`
        // prescribes whenever several ledgers share a prefix.
        seed_ledger(&cat, "c", &c, "see stream-a-log:F-1 for why\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&a, "F-1")).copied(),
            Some(1),
            "the qualifier names stream-a-log, so the exposure is A's: {deg:?}"
        );
        assert_eq!(
            deg.get(&deg_key(&b, "F-1")),
            None,
            "and B must not receive exposure it did not earn — pooling both ledgers under \
             one `F-1` key is the whole defect this keying removes: {deg:?}"
        );
    }

    /// A qualified citation of your OWN file is a SelfCite, decided exactly as
    /// `resolve`'s `[only]` arm decides it. A hand-maintained `## Index` row that spells
    /// its own ledger's stem must not inflate the entry it points at — 28.5% of ledger
    /// citations sit in such rows.
    #[test]
    fn indegree_treats_a_qualified_citation_of_your_own_file_as_a_self_cite() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let own = tmp.path().join("own-log.md");
        seed_ledger(
            &cat,
            "own",
            &own,
            "## Index\n| own-log:F-1 | x |\n\n## F-1 — the finding\n\nprose\n",
        );

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&own, "F-1")),
            None,
            "a ledger citing its own entry by its own stem is a SelfCite, not exposure — \
             otherwise an index row arms the gate on its own entry: {deg:?}"
        );
    }

    /// Where this function deliberately DIVERGES from `resolve`, pinned so the
    /// divergence cannot be "simplified" away into parity. `resolve` answers "can this be
    /// an edge" and stops at `CrossRepo`; this one answers "how exposed is this entry",
    /// and `call`'s own comment states that narrowing the metric to the active project
    /// "would understate real cross-repo exposure and manufacture false negatives".
    #[test]
    fn indegree_falls_back_to_the_token_half_when_a_qualifier_does_not_resolve() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let def = tmp.path().join("the-law.md");
        let unrelated = tmp.path().join("unrelated.md");
        let c1 = tmp.path().join("c1.md");
        let c2 = tmp.path().join("c2.md");
        seed_ledger(&cat, "def", &def, "## R-9 — the law\n\nprose\n");
        seed_ledger(&cat, "unrelated", &unrelated, "## Z-1 — other\n\nprose\n");
        // A genuine CROSS-REPO reference: `codescout` names no file in this corpus.
        seed_ledger(&cat, "c1", &c1, "see codescout:R-9 over there\n");
        // A qualifier naming a real local file that does NOT define R-9 — Dangling to
        // `resolve`, still exposure here.
        seed_ledger(&cat, "c2", &c2, "see unrelated:R-9\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&def, "R-9")).copied(),
            Some(2),
            "both unresolvable qualifiers fall back to the token half, which has exactly \
             one definer — dropping them would manufacture the false negatives call() \
             forbids: {deg:?}"
        );
    }
    /// A colliding stem is reported, never guessed — the fourth mechanism copied from
    /// `resolve`'s `CrossRepoToken` arm, and the one a `[only]` → `[only, ..]` slip drops
    /// silently. Confirmed by mutation 2026-08-21: relaxing that slice pattern to take the
    /// first candidate left all 156 doctor tests green until this test existed. `W-3`'s
    /// shape again — the copy inherited the sibling's discipline but not its tests,
    /// because those tests are named for `resolve`.
    #[test]
    fn indegree_does_not_guess_when_two_files_share_the_qualifiers_stem() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let dir_a = tmp.path().join("a");
        let dir_b = tmp.path().join("b");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        // Same STEM, different directories — `Corpus.by_stem` PUSHES rather than inserts
        // for exactly this case, and so does the map built here.
        let log_a = dir_a.join("log.md");
        let log_b = dir_b.join("log.md");
        let citer = tmp.path().join("citer.md");
        seed_ledger(&cat, "log-a", &log_a, "## F-1 — A's finding\n\nprose\n");
        seed_ledger(&cat, "log-b", &log_b, "## F-1 — B's finding\n\nprose\n");
        seed_ledger(&cat, "citer", &citer, "see log:F-1\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            (
                deg.get(&deg_key(&log_a, "F-1")),
                deg.get(&deg_key(&log_b, "F-1")),
            ),
            (None, None),
            "`log` names two files, so the qualifier resolves to neither; the fallback then \
             finds two active definers for the bare token and stays ambiguous. Guessing the \
             first candidate would credit whichever path happens to sort first: {deg:?}"
        );
    }

    #[test]
    fn indegree_excludes_archived_citers_but_still_allows_an_archived_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("b.md");
        let c = tmp.path().join("c.md");

        // The definer itself is archived. Ruling 15 leaves the defining side alone: an
        // archived file still defines its token.
        std::fs::write(&a, "## R-1 — the law\n\nprose\n").unwrap();
        let row_a = TestArtifactRowBuilder::new("a")
            .with_abs_path(&a)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row_a).unwrap();

        // An active citer — must count.
        seed_ledger(&cat, "b", &b, "see R-1\n");

        // An archived citer — must NOT count.
        std::fs::write(&c, "also see R-1\n").unwrap();
        let row_c = TestArtifactRowBuilder::new("c")
            .with_abs_path(&c)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row_c).unwrap();

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&a, "R-1")).copied(),
            Some(1),
            "only b.md's active citation may count — a.md is still R-1's definer despite \
         being archived, and c.md's archived citation must not add exposure: {deg:?}"
        );

        // The assertion above is INSENSITIVE to whether a.md is genuinely registered as
        // R-1's definer: with only one archived co-definer, "correctly registered" and
        // "silently dropped from `definer`" both leave R-1 with the same one active
        // definer overall, so a mutation that also strips archived rows from the DEFINING
        // side (not just the citing side) leaves the assertion above green — the exact gap
        // this test's own name was found not to cover. Pin it with a second token that has
        // ONLY archived definers: two archived files defining R-2 gives 0 ACTIVE definers
        // among 2 raw ones, which is genuinely ambiguous (neither wins per Ruling 16), so
        // R-2 must stay ABSENT from the map. If archived rows were ever silently dropped
        // from `definer` entirely, R-2 would read as a 0-raw-definer (merely undefined)
        // token instead — which this filter correctly does NOT treat as ambiguous — and
        // would wrongly let it through.
        let a2 = tmp.path().join("a2.md");
        let a3 = tmp.path().join("a3.md");
        let d = tmp.path().join("d.md");
        for (id, p) in [("a2", &a2), ("a3", &a3)] {
            std::fs::write(p, "## R-2 — also the law\n\nprose\n").unwrap();
            let row = TestArtifactRowBuilder::new(id)
                .with_abs_path(p)
                .with_status("archived")
                .build();
            art_upsert(&cat, &row).unwrap();
        }
        seed_ledger(&cat, "d", &d, "see R-2\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            (deg.get(&deg_key(&a2, "R-2")), deg.get(&deg_key(&a3, "R-2")),),
            (None, None),
            "R-2 has two definers and NEITHER is active — genuinely ambiguous per Ruling 16, \
         and must stay dropped even though both would-be definers are archived (not \
         undefined): {deg:?}"
        );
    }

    #[test]
    fn indegree_treats_a_token_as_unique_when_exactly_one_definer_is_active() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let archived_def = tmp.path().join("old.md");
        let active_def = tmp.path().join("new.md");
        let citer = tmp.path().join("citer.md");

        // Two RAW definers for the same token: one archived, one active. Per
        // `get_guide("tracker-conventions")` and `link_scan::resolve::resolve`'s own
        // tie-break (`resolve.rs:306-315`), the sole ACTIVE definer wins and the token is
        // NOT ambiguous — an archived co-definer never creates ambiguity. This is
        // mutation MR-5's target: the OLD rule ("ambiguous whenever raw definer count > 1")
        // would drop this token; the current rule must not.
        std::fs::write(&archived_def, "## R-5 — the old law\n\nprose\n").unwrap();
        let row = TestArtifactRowBuilder::new("old")
            .with_abs_path(&archived_def)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row).unwrap();

        seed_ledger(
            &cat,
            "new",
            &active_def,
            "## R-5 — the current law\n\nprose\n",
        );
        seed_ledger(&cat, "citer", &citer, "see R-5\n");

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&active_def, "R-5")).copied(),
            Some(1),
            "one archived co-definer must not make an otherwise-unique active definer read \
         as ambiguous: {deg:?}"
        );
    }

    #[test]
    fn indegree_excludes_a_citer_archived_by_location_not_status() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let def = tmp.path().join("led.md");
        let active_citer = tmp.path().join("b.md");
        let archive_dir = tmp.path().join("docs").join("issues").join("archive");
        let located_citer = archive_dir.join("old-bug.md");

        seed_ledger(&cat, "led", &def, "## R-1 — the law\n\nprose\n");
        seed_ledger(&cat, "b", &active_citer, "see R-1\n");

        // This repo archives files by MOVING them under an `archive/` directory while
        // leaving `status:` at the bug's OUTCOME (e.g. `fixed`), per
        // `get_guide("tracker-conventions")`. A status-only archived filter reads this row
        // as live; it must not count toward exposure.
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(&located_citer, "also see R-1\n").unwrap();
        let row = TestArtifactRowBuilder::new("old-bug")
            .with_abs_path(&located_citer)
            .with_status("fixed")
            .build();
        art_upsert(&cat, &row).unwrap();

        let deg = entry_indegree(&cat.conn).unwrap();
        assert_eq!(
            deg.get(&deg_key(&def, "R-1")).copied(),
            Some(1),
            "the archive/-located citer must not add exposure even though its status is \
         `fixed`, not `archived`: {deg:?}"
        );
    }

    #[tokio::test]
    async fn call_wires_a_live_entry_indegree_into_scan_conditional_past_due() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let led = tmp.path().join("led.md");
        seed_ledger(
            &cat,
            "led",
            &led,
            "## R-1 — exposed\n\n**Valid:** conditional — until the plan edit lands\n",
        );
        // EXPOSURE_THRESHOLD distinct citing files, each a unique definer-free citer, so
        // entry_indegree's real, computed-from-disk value clears the gate.
        for i in 0..EXPOSURE_THRESHOLD {
            let p = tmp.path().join(format!("citer-{i}.md"));
            seed_ledger(&cat, &format!("citer-{i}"), &p, "see R-1\n");
        }

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["entry_conditional_past_due"],
            json!(1),
            "call() must wire a REAL entry_indegree into scan_conditional_past_due, not an \
         empty/placeholder map: {out:#?}"
        );
    }

    #[test]
    fn conditional_past_due_does_not_report_a_row_under_a_sibling_root() {
        // Fix Round 2, M1: the scoping guard survived deletion because every scoping
        // test for this check only exercised the `None` (no active project) arm.
        // Mirrors `cited_but_undeclared_does_not_report_a_row_under_a_sibling_root`,
        // plus an in-scope entry so the guard's INCLUDE direction is pinned too.
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_root).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let in_scope = active_root.join("led.md");
        seed_ledger(
            &cat,
            "in-scope-led",
            &in_scope,
            "## R-80 — in scope\n\n**Valid:** conditional — until X\n",
        );
        let out_of_scope = sibling_root.join("led.md");
        seed_ledger(
            &cat,
            "out-of-scope-led",
            &out_of_scope,
            "## R-81 — out of scope\n\n**Valid:** conditional — until Y\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&in_scope, "R-80"), EXPOSURE_THRESHOLD + 3);
        deg.insert(deg_key(&out_of_scope, "R-81"), EXPOSURE_THRESHOLD + 3);

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_conditional_past_due(&ctx, &cat.conn, &deg).unwrap()
        };
        assert_eq!(
            v.len(),
            1,
            "only the entry under the ACTIVE project's git_root may be reported: {v:#?}"
        );
        assert_eq!(v[0].artifact_id.as_deref(), Some("in-scope-led"), "{v:#?}");
        assert_eq!(
            scoped_out.values().sum::<usize>(),
            1,
            "the sibling-root row must be COUNTED as scoped out, not silently dropped: \
             {scoped_out:?}"
        );
    }

    // ---- scan_dated_stale -----------------------------------------------------------

    #[test]
    fn dated_stale_ranks_by_exposure_descending() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-1 — low\n\n**Valid:** dated 2020-01-01\n\n\
             ## R-2 — high\n\n**Valid:** dated 2020-01-01\n\n\
             ## R-3 — fresh\n\n**Valid:** dated 2999-01-01\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-1"), 6usize);
        deg.insert(deg_key(&p, "R-2"), 40usize);
        deg.insert(deg_key(&p, "R-3"), 99usize);

        // 2026-08-20 as days since epoch.
        let (v, _) = scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685).unwrap();
        let ids: Vec<String> = v
            .iter()
            .map(|x| x.detail.split_whitespace().next().unwrap().to_string())
            .collect();
        assert_eq!(
            ids,
            vec!["R-2", "R-1"],
            "R-3 is inside the horizon; the rest are ordered by exposure, because an \
             unranked list of every dated entry will be ignored: {v:#?}"
        );

        // The whole point of `detail` is that a reader can adjudicate without
        // reopening the file — pin its content, not just membership/order. 2020-01-01
        // to 2026-08-20 (epoch day 20_685) is 2423 days, verified independently via
        // `python3 -c "(date(2026,8,20)-date(2020,1,1)).days"`.
        assert!(
            v[0].detail.contains("R-2 dated 2020-01-01"),
            "detail must name the entry and its declared date: {:?}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("2423d old"),
            "detail must carry the computed age in days: {:?}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("exposure 40"),
            "detail must carry the exposure count: {:?}",
            v[0].detail
        );
        assert!(
            v[1].detail.contains("R-1 dated 2020-01-01"),
            "{:?}",
            v[1].detail
        );
        assert!(v[1].detail.contains("2423d old"), "{:?}", v[1].detail);
        assert!(v[1].detail.contains("exposure 6"), "{:?}", v[1].detail);
    }

    #[test]
    fn dated_stale_breaks_exposure_ties_deterministically_not_by_sort_stability() {
        // 33 entries alternating EXPOSURE_THRESHOLD / EXPOSURE_THRESHOLD+1 — the
        // reviewer-confirmed shape that actually manifests `sort_unstable_by_key`
        // reordering equal keys (N=2, a 40/300-entry homogeneous block, and a
        // 62-entry permutation with one embedded pair all failed to reproduce it).
        // IDs are two-digit (R-10..R-42) so lexicographic string order equals numeric
        // order, keeping the expected assertion hand-verifiable.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();

        let mut text = String::new();
        let mut deg = std::collections::BTreeMap::new();
        let mut expected_evens = Vec::new();
        let mut expected_odds = Vec::new();
        for id in 10..=42 {
            text.push_str(&format!("## R-{id} — e\n\n**Valid:** dated 2020-01-01\n\n"));
            if id % 2 == 0 {
                deg.insert(deg_key(&p, format!("R-{id}")), EXPOSURE_THRESHOLD + 1);
                expected_evens.push(format!("R-{id}"));
            } else {
                deg.insert(deg_key(&p, format!("R-{id}")), EXPOSURE_THRESHOLD);
                expected_odds.push(format!("R-{id}"));
            }
        }
        seed_ledger(&cat, "led", &p, &text);

        let (v, _) = scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685).unwrap();
        assert_eq!(v.len(), 33);
        let got: Vec<String> = v
            .iter()
            .map(|x| x.detail.split_whitespace().next().unwrap().to_string())
            .collect();
        let mut expected = expected_evens;
        expected.extend(expected_odds);
        assert_eq!(
            got, expected,
            "the higher-exposure tier (evens) must come first, each tier ascending by \
             id — a total (Reverse(exposure), path, id) sort key, not encounter order \
             or a stable-sort accident: {v:#?}"
        );
    }

    #[test]
    fn dated_stale_horizon_gate_fires_exactly_at_the_line_not_one_day_short() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        // 2026-08-20 (epoch day 20_685) minus 30 days = 2026-07-21 (epoch day 20_655).
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-9 — at the line\n\n**Valid:** dated 2026-07-21\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-9"), EXPOSURE_THRESHOLD);

        assert_eq!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .len(),
            1,
            "age == VALIDITY_HORIZON_DAYS must fire — the gate is `<`, not `<=`"
        );
        // One day younger (age 29, "today" = 2026-08-19) must NOT fire.
        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_684)
                .unwrap()
                .0
                .is_empty(),
            "one day inside the horizon must not fire"
        );
    }

    #[test]
    fn dated_stale_fires_exactly_at_the_exposure_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-4 — at the line\n\n**Valid:** dated 2020-01-01\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-4"), EXPOSURE_THRESHOLD);
        assert_eq!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .len(),
            1,
            "exposure == EXPOSURE_THRESHOLD must fire — the gate is `<`, not `<=`"
        );

        deg.insert(deg_key(&p, "R-4"), EXPOSURE_THRESHOLD - 1);
        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "one below the threshold must not fire"
        );
    }

    #[test]
    fn dated_stale_treats_a_token_absent_from_indegree_as_zero_exposure() {
        // `indegree.get(&s.id).copied().unwrap_or(0)` is the MAJORITY branch: only 168
        // of 2869 live sections clear the exposure gate, so most tokens are entirely
        // ABSENT from the map (not present-with-zero) — `entry_indegree` omits uncited
        // and ambiguous tokens rather than inserting a zero for them. `deg` below never
        // mentions "R-51" at all, pinning that the fallback for a missing key is 0
        // (below the gate), not `usize::MAX` or anything else that would put every
        // uncited entry above the gate.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-51 — never mentioned in indegree\n\n**Valid:** dated 2020-01-01\n",
        );

        // A populated but unrelated map — the absence of the KEY "R-51" is what's under
        // test, not an empty map that could pass for an unrelated reason.
        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-99"), EXPOSURE_THRESHOLD + 50);

        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "a token with no entry in `indegree` at all must be treated as zero \
             exposure, not skip the gate"
        );
    }

    #[test]
    fn dated_stale_does_not_read_a_nested_childs_declaration_as_the_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        // PV-3 (the parent, level 3) declares nothing itself; its nested child PV-9
        // (level 4) sits WHOLLY inside PV-3's section text and declares `dated`.
        seed_ledger(
            &cat,
            "led",
            &p,
            "### PV-3 — parent, no declaration of its own\n\n\
             prose about the parent\n\n\
             #### PV-9 — nested child\n\n\
             **Valid:** dated 2020-01-01\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "PV-3"), EXPOSURE_THRESHOLD);
        deg.insert(deg_key(&p, "PV-9"), EXPOSURE_THRESHOLD);

        let (v, _) = scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685).unwrap();
        assert_eq!(
            v.len(),
            1,
            "PV-3 declares nothing of its own and must not inherit PV-9's declaration: {v:#?}"
        );
        assert!(v[0].detail.contains("PV-9"));
    }

    #[test]
    fn dated_stale_ignores_invariant_and_conditional_declarations() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-5 — invariant\n\n**Valid:** invariant\n\n\
             ## R-6 — conditional\n\n**Valid:** conditional — until something happens\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-5"), EXPOSURE_THRESHOLD);
        deg.insert(deg_key(&p, "R-6"), EXPOSURE_THRESHOLD);

        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "an invariant or conditional declaration is not this check's business — only \
             `dated` is"
        );
    }

    #[test]
    fn dated_stale_swallows_a_malformed_valid_line_rather_than_reporting_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-7 — malformed\n\n**Valid:** not-a-real-class\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-7"), EXPOSURE_THRESHOLD);

        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "a malformed declaration is `validity_unparseable`'s finding, not this check's"
        );
    }

    #[test]
    fn dated_stale_skips_a_shape_valid_but_calendar_invalid_date() {
        // `parse_validity` now rejects a calendar-impossible date at declaration time
        // (Fix Round 3, X-2 Half A) — these two never reach `iso_to_epoch_days` at all;
        // `parse_validity(&declared)` itself returns `Err`, so `scan_dated_stale`'s
        // `let Ok(Some(Validity::Dated(iso))) = ... else { continue }` guard swallows
        // them the same way it swallows any other malformed declaration. This test only
        // pins the negative half — that `scan_dated_stale` stays silent on them, not
        // reported or panicked on. The positive half — that they ARE reported, by
        // `scan_validity_unparseable` — is pinned separately by
        // `validity_unparseable_reports_the_calendar_invalid_dates_dated_stale_skips`,
        // because a test that only asserts absence here cannot distinguish "correctly
        // routed elsewhere" from "still invisible", which is exactly the bug this round
        // fixes
        // (docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md).
        //
        // R-8's `2020-13-45` is a COMPOSITE violation — month 13 is out of 1..12 AND day
        // 45 is out of 1..31 — so a merely range-checked parser (reject month > 12, day
        // > 31) would also reject it; it cannot distinguish a real calendar check from a
        // range check. R-9's `2026-02-30` is the discriminating case: day 30 is inside
        // the generic 1..31 range, but no February has a 30th regardless of leap year —
        // only a real calendar (`chrono::NaiveDate`) knows that.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-8 — impossible date\n\n**Valid:** dated 2020-13-45\n\n\
                     ## R-9 — range-valid but no such day\n\n**Valid:** dated 2026-02-30\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-8"), EXPOSURE_THRESHOLD);
        deg.insert(deg_key(&p, "R-9"), EXPOSURE_THRESHOLD);

        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "both a composite-invalid and a range-valid-but-calendar-invalid date must \
                     be skipped, not reported or panicked on"
        );
    }

    #[test]
    fn validity_unparseable_reports_the_calendar_invalid_dates_dated_stale_skips() {
        // The positive half of the test above. `scan_dated_stale` staying silent on
        // R-8/R-9 is consistent with EITHER "correctly routed to
        // `scan_validity_unparseable`" or "still invisible to everything" — the exact
        // ambiguity that let
        // docs/issues/archive/2026-08-20-impossible-date-hides-a-statement-from-every-check.md
        // ship undetected. This asserts the first reading is the true one: both R-8 and
        // R-9 ARE reported, by name, here. `scan_validity_unparseable` is ungated on
        // exposure, so no `indegree` map is seeded or needed.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-8 — impossible date\n\n**Valid:** dated 2020-13-45\n\n\
                     ## R-9 — range-valid but no such day\n\n**Valid:** dated 2026-02-30\n",
        );

        let (violations, _) = scan_validity_unparseable(&unscoped_ctx(), &cat.conn).unwrap();
        let ids: Vec<&str> = violations.iter().map(|v| v.path.as_str()).collect();
        assert_eq!(
            violations.len(),
            2,
            "both R-8 and R-9 must be reported, not just one: {violations:?}"
        );
        assert!(
            violations.iter().all(|v| v.check == "validity_unparseable"),
            "wrong check name: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.detail.contains("R-8") && v.detail.contains("not an ISO date")),
            "R-8 must be reported with the parser's own error text: {violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.detail.contains("R-9") && v.detail.contains("not an ISO date")),
            "R-9 must be reported with the parser's own error text: {violations:?}"
        );
        assert!(
            ids.iter().all(|p| p.contains("led.md")),
            "both rows must carry the seeded path: {violations:?}"
        );
    }

    /// The remediation text `parse_validity` already builds must reach the report.
    ///
    /// `RecoverableError` carries BOTH a `message` (what is wrong) and a `hint` (how to
    /// fix it), and every arm of `parse_validity` populates the hint with [`FORMS`].
    /// Until 2026-08-20 this check interpolated `err.message` alone, so an agent was told
    /// its `**Valid:**` line was malformed and never told what a well-formed one looks
    /// like — with the fix text sitting one field away, already written, in the same
    /// struct. That is the failure this pins: not that the check fires, which
    /// `validity_unparseable_reports_the_calendar_invalid_dates_dated_stale_skips`
    /// already covers, but that its detail is SELF-TEACHING.
    ///
    /// Asserted on the three class names literally, not on `FORMS` itself: comparing the
    /// detail against the same const that built it passes even if that const is gutted to
    /// an empty string. The literals are what an agent actually needs to read.
    #[test]
    fn validity_unparseable_detail_carries_the_parsers_remediation_hint() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-1 — calendar-invalid date\n\n**Valid:** dated 2026-02-30\n\n\
             ## R-2 — condition nobody named\n\n**Valid:** conditional\n\n\
             ## R-3 — unknown class\n\n**Valid:** conditionally speaking\n",
        );

        let (violations, _) = scan_validity_unparseable(&unscoped_ctx(), &cat.conn).unwrap();
        assert_eq!(
            violations.len(),
            3,
            "all three malformed shapes must be reported: {violations:#?}"
        );

        for v in &violations {
            for form in ["invariant", "dated YYYY-MM-DD", "conditional"] {
                assert!(
                    v.detail.contains(form),
                    "every malformed-declaration row must name the three valid forms so the \
                     reader can fix it without leaving the report; {form:?} missing from: {:?}",
                    v.detail
                );
            }
        }

        // The per-arm hint, not just the shared FORMS tail — R-1's hint is the only one
        // that names the ISO layout, so a mutation collapsing all three arms onto one
        // generic hint would still pass the loop above.
        let r1 = violations
            .iter()
            .find(|v| v.detail.contains("R-1"))
            .expect("R-1 must be reported");
        assert!(
            r1.detail.contains("Use `dated YYYY-MM-DD`"),
            "the date arm's own remediation hint must survive into the detail: {:?}",
            r1.detail
        );
        let r2 = violations
            .iter()
            .find(|v| v.detail.contains("R-2"))
            .expect("R-2 must be reported");
        assert!(
            r2.detail.contains("Name the event that ends validity"),
            "the bare-conditional arm's own remediation hint must survive into the \
             detail: {:?}",
            r2.detail
        );

        // The message half must not have been displaced by the hint.
        assert!(
            r1.detail.contains("not an ISO date"),
            "the parser's error message must still be present alongside the hint: {:?}",
            r1.detail
        );
    }

    #[test]
    fn validity_unparseable_skips_an_archived_definer_located_under_archive_path() {
        // Mirrors `conditional_past_due_skips_an_archived_definer_located_under_archive_path`
        // — same guard (`row_is_archived`), same MF-2 precedent. Fix Round 3 mutation M1:
        // deleting this skip is a confirmed-surviving mutation without this test.
        let tmp = tempfile::tempdir().unwrap();
        let archive_dir = tmp.path().join("archive");
        let p = archive_dir.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-61 — archived by path\n\n**Valid:** dated 2026-02-30\n",
        );

        assert!(
            scan_validity_unparseable(&unscoped_ctx(), &cat.conn)
                .unwrap()
                .0
                .is_empty(),
            "a definer located under an /archive/ path segment must not be reported, \
             even though its declaration is malformed"
        );
    }

    #[test]
    fn validity_unparseable_does_not_read_a_nested_childs_declaration_as_the_parents() {
        // Mirrors `conditional_past_due_does_not_read_a_nested_childs_declaration_as_the_parents`.
        // PV-2 (parent, level 3) declares nothing itself; its nested child PV-8 (level 4)
        // sits WHOLLY inside PV-2's section text and carries a MALFORMED declaration.
        // Without truncation (`declared_section_text`), PV-2's untruncated text would also
        // contain PV-8's malformed line, and `parse_validity`'s first-line rule would read
        // it as PV-2's OWN declaration too — reporting PV-2 a second time for a defect
        // that is actually PV-8's alone. Fix Round 3 mutation M3: swapping
        // `declared_section_text` for `s.text` is a confirmed-surviving mutation without
        // this test.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "### PV-2 — parent, no declaration of its own\n\n\
             prose about the parent\n\n\
             #### PV-8 — nested child\n\n\
             **Valid:** dated 2026-02-30\n",
        );

        let (v, _) = scan_validity_unparseable(&unscoped_ctx(), &cat.conn).unwrap();
        assert_eq!(
            v.len(),
            1,
            "PV-2 declares nothing of its own and must not inherit PV-8's malformed \
             declaration: {v:#?}"
        );
        assert!(v[0].detail.contains("PV-8"), "{v:#?}");
    }

    #[tokio::test]
    async fn call_wires_in_validity_unparseable_and_scopes_it_like_its_siblings() {
        // Fix Round 3 mutation M5: dropping the `scan_validity_unparseable` call from
        // `call()` is a confirmed-surviving mutation without an end-to-end test —
        // `validity_unparseable_reports_the_calendar_invalid_dates_dated_stale_skips`
        // calls the scan function directly and cannot see whether `call()` wires it in.
        // This also exercises the scope guard end-to-end (Fix Round 3 mutation M2): a
        // malformed declaration OUTSIDE the active project must be scoped out, contributing
        // to `entry_validity_scoped_by_project` rather than the report, same as its three
        // siblings.
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_root).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        // Malformed, INSIDE the active project — must be reported.
        let inside = active_root.join("led.md");
        seed_ledger(
            &cat,
            "led-inside",
            &inside,
            "## R-70 — inside, malformed\n\n**Valid:** dated 2026-02-30\n",
        );
        // Malformed, OUTSIDE the active project — must be scoped out, not reported.
        let outside = sibling_root.join("led.md");
        seed_ledger(
            &cat,
            "led-outside",
            &outside,
            "## R-71 — outside, malformed\n\n**Valid:** dated 2026-99-99\n",
        );

        let ctx = ctx_rooted_at(cat, &active_root);
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["validity_unparseable"],
            json!(1),
            "the in-project malformed declaration must be reported by name: {out:#?}"
        );
        let violations = out["violations"].as_array().unwrap();
        assert!(
            violations
                .iter()
                .filter(|v| v["check"] == "validity_unparseable")
                .all(|v| v["detail"].as_str().unwrap().contains("R-70")),
            "only R-70 (in-project) may appear, never R-71 (out-of-project): {out:#?}"
        );
        assert!(
            out["catalog_health"]["entry_validity_scoped_by_project"]
                .as_object()
                .is_some_and(|m| !m.is_empty()),
            "R-71 must be ANNOUNCED as scoped out, not silently dropped: {out:#?}"
        );
    }

    #[test]
    fn dated_stale_skips_an_archived_definer_located_under_archive_path() {
        let tmp = tempfile::tempdir().unwrap();
        let archive_dir = tmp.path().join("archive");
        let p = archive_dir.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-62 — archived by path\n\n**Valid:** dated 2020-01-01\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-62"), EXPOSURE_THRESHOLD + 3);

        assert!(
            scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685)
                .unwrap()
                .0
                .is_empty(),
            "a definer located under an /archive/ path segment must not be reported \
             even though its exposure and age both clear the gate"
        );
    }

    #[test]
    fn dated_stale_reports_only_the_active_twin_when_a_token_has_an_archived_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let active = tmp.path().join("active.md");
        let archived = tmp.path().join("archived.md");
        seed_ledger(
            &cat,
            "active",
            &active,
            "## R-63 — active twin\n\n**Valid:** dated 2020-01-01\n",
        );
        std::fs::write(
            &archived,
            "## R-63 — archived twin\n\n**Valid:** dated 2020-01-01\n",
        )
        .unwrap();
        let row = TestArtifactRowBuilder::new("archived")
            .with_abs_path(&archived)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row).unwrap();

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&active, "R-63"), EXPOSURE_THRESHOLD + 3);

        let (v, _) = scan_dated_stale(&unscoped_ctx(), &cat.conn, &deg, 20_685).unwrap();
        assert_eq!(
            v.len(),
            1,
            "R-63 is defined in both an active and an archived file; only the active \
             one may be reported: {v:#?}"
        );
        assert_eq!(v[0].artifact_id, Some("active".to_string()));
    }

    #[tokio::test]
    async fn call_wires_a_live_entry_indegree_into_scan_dated_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let led = tmp.path().join("led.md");
        seed_ledger(
            &cat,
            "led",
            &led,
            "## R-1 — exposed\n\n**Valid:** dated 2000-01-01\n",
        );
        // EXPOSURE_THRESHOLD distinct citing files, each a unique definer-free citer, so
        // entry_indegree's real, computed-from-disk value clears the gate.
        for i in 0..EXPOSURE_THRESHOLD {
            let p = tmp.path().join(format!("citer-{i}.md"));
            seed_ledger(&cat, &format!("citer-{i}"), &p, "see R-1\n");
        }

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["entry_dated_stale"],
            json!(1),
            "call() must wire a REAL entry_indegree, and a real `today`, into \
             scan_dated_stale, not an empty/placeholder map: {out:#?}"
        );
    }

    #[test]
    fn dated_stale_does_not_report_a_row_under_a_sibling_root() {
        // Fix Round 2, M2: same gap as M1 — every scoping test for this check only
        // exercised the `None` (no active project) arm. Mirrors
        // `conditional_past_due_does_not_report_a_row_under_a_sibling_root`.
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_root).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let in_scope = active_root.join("led.md");
        seed_ledger(
            &cat,
            "in-scope-led",
            &in_scope,
            "## R-82 — in scope\n\n**Valid:** dated 2020-01-01\n",
        );
        let out_of_scope = sibling_root.join("led.md");
        seed_ledger(
            &cat,
            "out-of-scope-led",
            &out_of_scope,
            "## R-83 — out of scope\n\n**Valid:** dated 2020-01-01\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&in_scope, "R-82"), EXPOSURE_THRESHOLD + 3);
        deg.insert(deg_key(&out_of_scope, "R-83"), EXPOSURE_THRESHOLD + 3);

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            // 2026-08-20 as days since epoch — matches the other `scan_dated_stale`
            // tests in this file, well past VALIDITY_HORIZON_DAYS for a 2020 date.
            scan_dated_stale(&ctx, &cat.conn, &deg, 20_685).unwrap()
        };
        assert_eq!(
            v.len(),
            1,
            "only the entry under the ACTIVE project's git_root may be reported: {v:#?}"
        );
        assert_eq!(v[0].artifact_id.as_deref(), Some("in-scope-led"), "{v:#?}");
        assert_eq!(
            scoped_out.values().sum::<usize>(),
            1,
            "the sibling-root row must be COUNTED as scoped out, not silently dropped: \
             {scoped_out:?}"
        );
    }

    // ---- scan_cited_but_undeclared ---------------------------------------------------

    #[test]
    fn cited_but_undeclared_reports_load_bearing_entries_with_no_class() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-1 — declared\n\n**Valid:** invariant\n\n\
             ## R-2 — undeclared but load-bearing\n\nprose with no class\n\n\
             ## R-3 — undeclared and unread\n\nalso nothing\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-1"), 20usize);
        deg.insert(deg_key(&p, "R-2"), 20usize);
        deg.insert(deg_key(&p, "R-3"), 1usize);

        let (v, _) = scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "R-1 declares a class; R-3 is below the gate: {v:#?}"
        );
        assert!(v[0].detail.contains("R-2"));
        assert_eq!(v[0].check, "entry_cited_from_outside_but_undeclared");
        assert!(
            !v[0].detail.contains("promoted"),
            "this check must never claim to know WHY an entry is cited — a promotion, \
             an eval-fixture list and a kin reference are syntactically identical"
        );
    }

    /// "Add one" is not actionable unless the row says what "one" is.
    ///
    /// This check has no `RecoverableError` to draw a hint from — the entry declares
    /// nothing, so nothing failed to parse — which is exactly why the forms have to be
    /// named explicitly here. Until 2026-08-20 the detail ended `— add one`, and the two
    /// most natural guesses at what to add (a bare `conditional`, or free text) are
    /// precisely the shapes `parse_validity` refuses, so following this row's advice
    /// converted an `entry_cited_from_outside_but_undeclared` row into a
    /// `validity_unparseable` one.
    ///
    /// Asserted on the literals rather than on [`FORMS`] for the same reason as
    /// `validity_unparseable_detail_carries_the_parsers_remediation_hint`: a const
    /// compared against itself pins nothing.
    #[test]
    fn cited_but_undeclared_detail_names_the_three_forms() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-2 — undeclared but load-bearing\n\nprose with no class\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-2"), 20usize);

        let (v, _) = scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(v.len(), 1, "R-2 is load-bearing and undeclared: {v:#?}");
        for form in ["invariant", "dated YYYY-MM-DD", "conditional"] {
            assert!(
                v[0].detail.contains(form),
                "a row telling an author to \"add one\" must name the three forms it will \
                 accept, or the likeliest guesses are the ones the parser refuses; \
                 {form:?} missing from: {:?}",
                v[0].detail
            );
        }
        assert!(
            !v[0].detail.contains("promoted"),
            "naming the forms must not have reintroduced a claim about WHY the entry is \
             cited"
        );
    }

    #[test]
    fn cited_but_undeclared_fires_exactly_at_the_exposure_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-4 — at the line\n\nno class declared\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-4"), EXPOSURE_THRESHOLD);
        assert_eq!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .len(),
            1,
            "exposure == EXPOSURE_THRESHOLD must fire — the gate is `<`, not `<=`"
        );

        deg.insert(deg_key(&p, "R-4"), EXPOSURE_THRESHOLD - 1);
        assert!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "one below the threshold must not fire"
        );
    }

    #[test]
    fn cited_but_undeclared_does_not_read_a_nested_childs_declaration_as_the_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        // PV-2 (the parent) declares nothing itself; its nested child PV-8 sits WHOLLY
        // inside PV-2's section text and DOES declare a class. Without truncating with
        // `declared_section_text`, `parse_validity(s.text)` would read PV-8's declaration
        // as PV-2's own and PV-2 would wrongly stop being reported as undeclared — the
        // unsafe direction for THIS check (under-reporting a load-bearing gap), the
        // mirror image of the unsafe direction for `scan_conditional_past_due` /
        // `scan_dated_stale` (over-claiming a class nobody declared).
        seed_ledger(
            &cat,
            "led",
            &p,
            "### PV-2 — parent, no declaration of its own\n\n\
             prose about the parent\n\n\
             #### PV-8 — nested child\n\n\
             **Valid:** invariant\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "PV-2"), EXPOSURE_THRESHOLD);
        deg.insert(deg_key(&p, "PV-8"), EXPOSURE_THRESHOLD);

        let (v, _) = scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "PV-2 declares nothing of its own and must still be reported despite PV-8's \
             nested declaration; PV-8 itself declares invariant and must not appear: {v:#?}"
        );
        assert!(v[0].detail.contains("PV-2"));
    }

    #[test]
    fn cited_but_undeclared_swallows_a_malformed_valid_line_rather_than_reporting_it() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-7 — malformed\n\n**Valid:** not-a-real-class\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-7"), EXPOSURE_THRESHOLD);

        assert!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a malformed declaration is `validity_unparseable`'s finding, not this \
             check's — it is not the same thing as declaring nothing"
        );
    }

    #[test]
    fn cited_but_undeclared_treats_a_token_absent_from_indegree_as_zero_exposure() {
        // Same defect class pinned for `scan_conditional_past_due` and
        // `scan_dated_stale`: `entry_indegree` omits uncited/ambiguous tokens rather
        // than inserting a zero for them, so the missing-key branch of `unwrap_or(0)`
        // is the majority case in the live corpus, not an edge case.
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-52 — never mentioned in indegree\n\nno class declared\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-98"), EXPOSURE_THRESHOLD + 50);

        assert!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a token with no entry in `indegree` at all must be treated as zero \
             exposure, not skip the gate"
        );
    }

    #[test]
    fn cited_but_undeclared_ignores_entries_that_declare_any_class() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-10 — invariant\n\n**Valid:** invariant\n\n\
             ## R-11 — dated\n\n**Valid:** dated 2020-01-01\n\n\
             ## R-12 — conditional\n\n**Valid:** conditional — until X\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        for id in ["R-10", "R-11", "R-12"] {
            deg.insert(deg_key(&p, id.to_string()), EXPOSURE_THRESHOLD + 10);
        }

        assert!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "any declared class at all — invariant, dated, or conditional — takes an \
             entry out of this check's business"
        );
    }

    #[test]
    fn cited_but_undeclared_skips_an_archived_definer_located_under_archive_path() {
        let tmp = tempfile::tempdir().unwrap();
        let archive_dir = tmp.path().join("archive");
        let p = archive_dir.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(
            &cat,
            "led",
            &p,
            "## R-64 — archived by path, no class\n\nprose only\n",
        );

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-64"), EXPOSURE_THRESHOLD + 3);

        assert!(
            scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg)
                .unwrap()
                .0
                .is_empty(),
            "a definer located under an /archive/ path segment must not be reported \
             even though its exposure clears the gate"
        );
    }

    #[test]
    fn cited_but_undeclared_reports_only_the_active_twin_when_a_token_has_an_archived_definer() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let active = tmp.path().join("active.md");
        let archived = tmp.path().join("archived.md");
        seed_ledger(
            &cat,
            "active",
            &active,
            "## R-65 — active twin\n\nno class\n",
        );
        std::fs::write(&archived, "## R-65 — archived twin\n\nno class\n").unwrap();
        let row = TestArtifactRowBuilder::new("archived")
            .with_abs_path(&archived)
            .with_status("archived")
            .build();
        art_upsert(&cat, &row).unwrap();

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&active, "R-65"), EXPOSURE_THRESHOLD + 3);

        let (v, _) = scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "R-65 is defined in both an active and an archived file; only the active \
             one may be reported: {v:#?}"
        );
        assert_eq!(v[0].artifact_id, Some("active".to_string()));
    }

    // ---- scan_cited_but_undeclared: project scoping (Fix 2, MF-1) ---------------------

    #[test]
    fn cited_but_undeclared_reports_a_row_under_the_active_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let p = root.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, "led", &p, "## R-70 — in scope\n\nno class\n");

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-70"), EXPOSURE_THRESHOLD + 3);

        let ctx = ctx_rooted_at(cat, &root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_but_undeclared(&ctx, &cat.conn, &deg).unwrap()
        };
        assert_eq!(
            v.len(),
            1,
            "a row under the active project's git_root must be reported: {v:#?}"
        );
        assert!(
            scoped_out.is_empty(),
            "nothing was scoped out: {scoped_out:?}"
        );
    }

    #[test]
    fn cited_but_undeclared_does_not_report_a_row_under_a_sibling_root() {
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_root).unwrap();
        let p = sibling_root.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, "led", &p, "## R-71 — out of scope\n\nno class\n");

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-71"), EXPOSURE_THRESHOLD + 3);

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_but_undeclared(&ctx, &cat.conn, &deg).unwrap()
        };
        assert!(
            v.is_empty(),
            "a row under a SIBLING root must not be reported when an active project is \
             set — component-boundary matching, not a prefix match: {v:#?}"
        );
        assert_eq!(
            scoped_out.values().sum::<usize>(),
            1,
            "the scoped-out row must be COUNTED, not silently dropped — a filtered row \
             never becomes a Violation, so summary.total cannot count it, but this map \
             must: {scoped_out:?}"
        );
    }

    #[test]
    fn cited_but_undeclared_does_not_treat_a_prefix_matching_sibling_as_contained() {
        // `containing_root`'s own doc comment: "/proj/sub must not be treated as
        // contained by /proj/subterfuge". A `String::starts_with`-based scope check
        // would wrongly treat `active-project-2` as inside `active-project`, because
        // the STRING `.../active-project-2/...` starts with the string
        // `.../active-project` even though the directory is a sibling, not a child.
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let lookalike_root = tmp.path().join("active-project-2");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&lookalike_root).unwrap();
        let p = lookalike_root.join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, "led", &p, "## R-74 — prefix lookalike\n\nno class\n");

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-74"), EXPOSURE_THRESHOLD + 3);

        let ctx = ctx_rooted_at(cat, &active_root);
        let (v, scoped_out) = {
            let cat = ctx.catalog.lock();
            scan_cited_but_undeclared(&ctx, &cat.conn, &deg).unwrap()
        };
        assert!(
            v.is_empty(),
            "a string-prefix-matching sibling must not be treated as contained — \
             component-boundary matching, not String::starts_with: {v:#?}"
        );
        assert_eq!(scoped_out.values().sum::<usize>(), 1, "{scoped_out:?}");
    }

    #[test]
    fn cited_but_undeclared_reports_everything_when_there_is_no_active_project() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("led.md");
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, "led", &p, "## R-72 — no active project\n\nno class\n");

        let mut deg = std::collections::BTreeMap::new();
        deg.insert(deg_key(&p, "R-72"), EXPOSURE_THRESHOLD + 3);

        let (v, scoped_out) = scan_cited_but_undeclared(&unscoped_ctx(), &cat.conn, &deg).unwrap();
        assert_eq!(
            v.len(),
            1,
            "no active project must mean NO scoping — report everything, not an empty \
             worklist, matching how call() degrades detect_move_candidates: {v:#?}"
        );
        assert!(scoped_out.is_empty(), "{scoped_out:?}");
    }

    #[tokio::test]
    async fn call_wires_a_live_entry_indegree_into_scan_cited_but_undeclared() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let led = tmp.path().join("led.md");
        seed_ledger(&cat, "led", &led, "## R-1 — undeclared\n\nno class here\n");
        // EXPOSURE_THRESHOLD distinct citing files, each a unique definer-free citer, so
        // entry_indegree's real, computed-from-disk value clears the gate.
        for i in 0..EXPOSURE_THRESHOLD {
            let p = tmp.path().join(format!("citer-{i}.md"));
            seed_ledger(&cat, &format!("citer-{i}"), &p, "see R-1\n");
        }

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["entry_cited_from_outside_but_undeclared"],
            json!(1),
            "call() must wire a REAL entry_indegree into scan_cited_but_undeclared, not \
             an empty/placeholder map: {out:#?}"
        );
    }

    #[tokio::test]
    async fn call_reports_slug_coverage_in_catalog_health() {
        // Reported unconditionally, not gated on a threshold: every row without a slug
        // is a source that cannot carry an entry-grain citation, and the number is how
        // anyone knows the Layer 3a backfill has not run.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let led = tmp.path().join("led.md");
        seed_ledger(&cat, "led", &led, "## R-1 — a\n\nbody\n");

        let ctx = TestToolContextBuilder::new(cat).build();
        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["catalog_health"]["slug_coverage"]["without_slug"],
            json!(1),
            "the seeded artifact has no slug and must be counted: {out:#?}"
        );
        assert_eq!(
            out["catalog_health"]["slug_coverage"]["with_slug"],
            json!(0)
        );
    }

    #[tokio::test]
    async fn mint_slugs_dry_run_reports_without_writing_then_confirm_applies() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        for name in ["alpha", "beta"] {
            let p = tmp.path().join(format!("{name}.md"));
            seed_ledger(&cat, name, &p, "## R-1 — a\n\nbody\n");
        }
        let ctx = TestToolContextBuilder::new(cat).build();

        let dry = call(&ctx, json!({ "fix": "mint_slugs" })).await.unwrap();
        assert_eq!(dry["mode"], json!("dry_run"));
        assert_eq!(dry["minted"], json!(2));
        assert_eq!(
            dry["slug_coverage"]["without_slug"],
            json!(2),
            "a dry run must leave every slug NULL: {dry:#?}"
        );

        let applied = call(&ctx, json!({ "fix": "mint_slugs", "confirm": true }))
            .await
            .unwrap();
        assert_eq!(applied["mode"], json!("applied"));
        assert_eq!(applied["minted"], json!(2));
        assert_eq!(applied["slug_coverage"]["without_slug"], json!(0));

        // Idempotent through the tool surface, not just the catalog function.
        let again = call(&ctx, json!({ "fix": "mint_slugs", "confirm": true }))
            .await
            .unwrap();
        assert_eq!(
            again["minted"],
            json!(0),
            "slugs are immutable; a second apply must mint nothing: {again:#?}"
        );
    }

    #[tokio::test]
    async fn export_augmentations_writes_a_sidecar_stamps_the_declaration_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // Augmented, and declaring nothing yet — the state every augmented artifact was in
        // before this fix existed.
        seed_declared(&cat, root, "t", None, true);
        let ctx = TestToolContextBuilder::new(cat).build();

        let sidecar = root.join("docs/augmentations/t.yaml");
        let args = || json!({ "fix": "export_augmentations", "root": root.to_string_lossy() });

        let dry = call(&ctx, args()).await.unwrap();
        assert_eq!(dry["mode"], json!("dry_run"));
        assert_eq!(dry["totals"]["exported"], json!(1), "{dry:#?}");
        assert!(
            !sidecar.exists(),
            "a dry run must write nothing — this fix's whole value is that it runs on a \
             machine holding the only copy of these rows"
        );

        let mut applied = args();
        applied["confirm"] = json!(true);
        let out = call(&ctx, applied.clone()).await.unwrap();
        assert_eq!(out["mode"], json!("applied"));
        assert_eq!(out["totals"]["exported"], json!(1), "{out:#?}");
        assert_eq!(out["totals"]["failed"], json!(0), "{out:#?}");

        let doc = crate::librarian::augmentation_sidecar::read(&sidecar)
            .expect("the sidecar must be written and parseable");
        assert_eq!(doc.prompt, "p");

        // The sidecar alone is inert: nothing finds it unless the artifact names it.
        let content = std::fs::read_to_string(root.join("t.md")).unwrap();
        assert!(
            content.contains("expects_augmentation: docs/augmentations/t.yaml"),
            "the declaration must be stamped, or reindex has no way to reach the file it \
             just wrote: {content}"
        );

        // Idempotent: a second confirmed run must not rewrite files it already exported.
        let again = call(&ctx, applied).await.unwrap();
        assert_eq!(
            again["totals"]["exported"],
            json!(0),
            "already-exported artifacts must be skipped, not rewritten: {again:#?}"
        );
    }

    /// An artifact the export DECLINES to touch must be reported, never silently omitted.
    ///
    /// The idempotence asserted by the test above is correct and worth keeping — but it
    /// is delivered by a bare `continue`, so the only trace of a decision not to act is
    /// a `0` in a count of what was written. `exported: 0` is a truthful answer to "how
    /// many did you create?" and the wrong answer to "did you fix my drift?", which is
    /// what the caller actually asked. There is no error, no refusal, and no mention of
    /// the artifact.
    ///
    /// That silence is what turns a wrong remedy into an invisible one:
    /// `sidecar_shape_drift` prescribes this very fix for a case whose sidecar exists by
    /// the check's own precondition, and the reader gets a clean exit and no repair.
    /// Making the skip legible is the mechanism half of that bug — it reports the
    /// no-op whether or not anyone suspected one.
    #[tokio::test]
    async fn an_already_exported_artifact_is_reported_as_skipped_not_silently_omitted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_declared(&cat, root, "t", None, true);
        let ctx = TestToolContextBuilder::new(cat).build();

        let args = json!({
            "fix": "export_augmentations",
            "root": root.to_string_lossy(),
            "confirm": true,
        });

        let first = call(&ctx, args.clone()).await.unwrap();
        assert_eq!(first["totals"]["exported"], json!(1), "{first:#?}");
        assert_eq!(
            first["totals"]["skipped"],
            json!(0),
            "nothing is already-exported on the first run: {first:#?}"
        );

        let second = call(&ctx, args).await.unwrap();
        assert_eq!(second["totals"]["exported"], json!(0), "{second:#?}");
        assert_eq!(
            second["totals"]["skipped"],
            json!(1),
            "a run that wrote nothing BECAUSE everything was already exported must say \
                 so; `exported: 0` alone is indistinguishable from `nothing matched`: \
                 {second:#?}"
        );
        assert_eq!(
            second["skipped"][0]["path"]
                .as_str()
                .map(|p| p.ends_with("t.md")),
            Some(true),
            "and it must name WHICH artifact it declined to touch, or the reader \
                 cannot tell whether their artifact was the one skipped: {second:#?}"
        );
    }

    /// This fix WRITES and the catalog is machine-global — 207 files across five unrelated
    /// repos, when the same guard was missing from `repair_frontmatter_id`.
    #[tokio::test]
    async fn export_augmentations_refuses_without_a_scope() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        let err = call(&ctx, json!({ "fix": "export_augmentations" }))
            .await
            .unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("root="),
            "the refusal must name the way out: {text}"
        );
    }

    /// Seeds an augmented artifact that declares its sidecar, plus the sidecar itself holding
    /// `prompt`. Returns the sidecar's path. The catalog row's prompt is `seed_declared`'s
    /// `"p"`, so passing anything else here creates drift on exactly one field.
    ///
    /// FIXTURE NOTE — `.git` is load-bearing and no assertion can say so.
    /// `scan_sidecar_shape_drift` resolves the sidecar through the artifact's git root; with
    /// no `.git` the scan skips every artifact and returns empty, so the silence tests below
    /// would pass for the wrong reason and the drift test would fail confusingly. If this line
    /// is ever removed as tidy-up, two of the four tests stop discriminating while still
    /// passing.
    fn seed_with_sidecar(
        cat: &Catalog,
        root: &std::path::Path,
        name: &str,
        prompt: &str,
    ) -> std::path::PathBuf {
        use crate::librarian::augmentation_sidecar as sc;
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let art = root.join(format!("{name}.md"));
        let rel = sc::rel_path_for(root, &art);
        seed_declared(cat, root, name, Some(&rel), true);
        let path = root.join(&rel);
        sc::write(
            &path,
            &sc::AugmentationSidecar {
                schema_version: sc::SCHEMA_VERSION,
                prompt: prompt.to_string(),
                entry_collection: None,
                params_schema: None,
                render_template: None,
                append_mode: false,
                history_cap: None,
            },
        )
        .unwrap();
        path
    }

    /// The safety net for `artifact_augment`'s write-through, which can only ever cover the
    /// call sites someone remembered to sweep.
    ///
    /// PAIRING NOTE — this test and `..._is_silent_when_the_two_agree` are monotone in
    /// OPPOSITE directions, and that is the whole reason the pair is coverage rather than
    /// decoration. This one asserts a violation IS produced, which is monotone under
    /// over-reporting: a check that flagged every augmented artifact would pass it. Its
    /// sibling asserts silence when they agree, which is monotone under the check being dead.
    /// Either alone leaves a direction unguarded. See reconnaissance-patterns R-132.
    #[test]
    fn sidecar_shape_drift_reports_the_fields_that_disagree() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed_with_sidecar(&cat, tmp.path(), "t", "the superseded prompt");

        let v = scan_sidecar_shape_drift(&cat.conn).unwrap();

        assert_eq!(v.len(), 1, "exactly the drifted artifact must fire: {v:#?}");
        assert_eq!(v[0].check, "sidecar_shape_drift");
        assert!(
            v[0].detail.contains("prompt"),
            "the detail must NAME the drifting field — a reader cannot act on \"they differ\": {}",
            v[0].detail
        );
        assert!(
            v[0].detail.contains("CANNOT TELL WHICH"),
            "the detail must refuse to guess the direction; prescribing a re-export blindly \
             would overwrite a pulled shape with a stale local row: {}",
            v[0].detail
        );
    }

    /// The other direction of the pair above.
    #[test]
    fn sidecar_shape_drift_is_silent_when_the_two_agree() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // "p" is exactly what `seed_declared` puts in the catalog row.
        seed_with_sidecar(&cat, tmp.path(), "t", "p");

        let v = scan_sidecar_shape_drift(&cat.conn).unwrap();

        assert!(v.is_empty(), "an in-sync pair must not fire: {v:#?}");
    }

    /// Comparison is STRUCTURAL, never byte-wise. A real committed sidecar in this repo was
    /// hand-edited (`2a8decc5`) because nothing could refresh it, so byte comparison would
    /// have fired on a file that says exactly the right thing — and a check that cries wolf on
    /// correct files trains its readers to skip it.
    #[test]
    fn a_hand_formatted_sidecar_saying_the_same_thing_is_not_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let path = seed_with_sidecar(&cat, tmp.path(), "t", "p");

        // A YAML comment no serializer emits: byte-different, semantically identical.
        let hand = std::fs::read_to_string(&path).unwrap() + "# hand-edited, and still correct\n";
        std::fs::write(&path, &hand).unwrap();

        let v = scan_sidecar_shape_drift(&cat.conn).unwrap();

        assert!(
            v.is_empty(),
            "a byte-different but semantically identical sidecar must not fire: {v:#?}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            hand,
            "and a read-only scan must not rewrite the file it inspected"
        );
    }

    /// Reported separately because NOTHING else can see it: `reindex` skips an artifact that
    /// already has a row, so a corrupt committed shape sits unread until the machine holding
    /// the row loses it — the one moment it is needed. R-133: an alarm on a path nobody walks.
    #[test]
    fn a_corrupt_sidecar_is_reported_rather_than_read_as_agreement() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let path = seed_with_sidecar(&cat, tmp.path(), "t", "p");
        std::fs::write(&path, "prompt: [unterminated\n\t\tnonsense: {{{\n").unwrap();

        let v = scan_sidecar_shape_drift(&cat.conn).unwrap();

        assert_eq!(v.len(), 1, "the corrupt sidecar must fire: {v:#?}");
        assert_eq!(
            v[0].check, "sidecar_unparseable",
            "a corrupt sidecar is a different finding from drift, and needs different advice"
        );
    }

    #[tokio::test]
    async fn unknown_fix_names_mint_slugs_among_the_valid_ones() {
        // The error text is the only place a caller learns the fix vocabulary, so a
        // new fix that is not listed there is undiscoverable.
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();
        let err = call(&ctx, json!({ "fix": "nonsense" })).await.unwrap_err();
        let text = format!("{err:#}");
        assert!(
            text.contains("mint_slugs"),
            "unknown-fix error must name every valid fix: {text}"
        );
    }

    #[tokio::test]
    async fn call_reports_entry_validity_scoped_out_rows_in_catalog_health() {
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_root = tmp.path().join("sibling-project");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_root).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        // The undeclared, above-threshold entry lives OUTSIDE the active project.
        let led = sibling_root.join("led.md");
        seed_ledger(&cat, "led", &led, "## R-73 — outside\n\nno class here\n");
        for i in 0..EXPOSURE_THRESHOLD {
            let p = sibling_root.join(format!("citer-{i}.md"));
            seed_ledger(&cat, &format!("citer-{i}"), &p, "see R-73\n");
        }

        let ctx = ctx_rooted_at(cat, &active_root);
        let out = call(&ctx, json!({})).await.unwrap();

        // Asserted as `0`, not as an absent key. This test used to read
        // absence as "the check found nothing here" — which was the OB-5
        // ambiguity being load-bearing inside the suite itself, since
        // absence equally meant "the check never ran". `by_check` is now
        // seeded with every declared check at 0, so the intent is stated
        // directly and the assertion is strictly sharper: it distinguishes
        // ran-and-found-nothing from a check that is not there at all,
        // which `is_none()` could not.
        assert_eq!(
            out["summary"]["by_check"]["entry_cited_from_outside_but_undeclared"].as_u64(),
            Some(0),
            "the scoped-out row must not be COUNTED for the active project: {out:#?}"
        );
        assert!(
            out["catalog_health"]["entry_validity_scoped_by_project"]
                .as_object()
                .is_some_and(|m| !m.is_empty()),
            "the drop must be ANNOUNCED, not silent — same discipline as \
             outside_roots_by_project: {out:#?}"
        );
        assert!(
            out["catalog_health"]["hint"]
                .as_str()
                .unwrap()
                .contains("scoped out"),
            "{out:#?}"
        );
    }

    #[tokio::test]
    async fn call_accumulates_scoped_out_counts_per_root_across_checks_and_keeps_roots_distinct() {
        // Fix Round 2, M7 + M10, composed per the brief: M7 is the call()-level fold
        // `+= n` vs `= n` merging conditional_scoped/dated_scoped/cited_scoped — that
        // only diverges when TWO DIFFERENT checks contribute to the SAME group key,
        // since a single check's own scoped_out map already pre-accumulates same-key
        // rows before this fold ever sees them. M10 is the group key itself collapsing
        // to a constant. Root A gets a conditional AND a dated contribution (pins M7's
        // accumulation); root B gets an unrelated cited-but-undeclared contribution
        // under a DIFFERENT root (pins M10's per-root distinctness).
        let tmp = tempfile::tempdir().unwrap();
        let active_root = tmp.path().join("active-project");
        let sibling_a = tmp.path().join("sibling-a");
        let sibling_b = tmp.path().join("sibling-b");
        std::fs::create_dir_all(&active_root).unwrap();
        std::fs::create_dir_all(&sibling_a).unwrap();
        std::fs::create_dir_all(&sibling_b).unwrap();
        let cat = Catalog::open_in_memory().unwrap();

        let cond_a = sibling_a.join("cond.md");
        seed_ledger(
            &cat,
            "cond-a",
            &cond_a,
            "## RA-1 — conditional\n\n**Valid:** conditional — until X\n",
        );
        let dated_a = sibling_a.join("dated.md");
        seed_ledger(
            &cat,
            "dated-a",
            &dated_a,
            "## RA-2 — dated\n\n**Valid:** dated 2020-01-01\n",
        );
        let cited_b = sibling_b.join("cited.md");
        seed_ledger(
            &cat,
            "cited-b",
            &cited_b,
            "## RB-1 — undeclared\n\nno class\n",
        );

        // EXPOSURE_THRESHOLD real citers per entry — call() computes indegree from the
        // real corpus, unlike the lower-level scan_* tests above which hand-build it.
        for (idx, id) in ["RA-1", "RA-2", "RB-1"].into_iter().enumerate() {
            for i in 0..EXPOSURE_THRESHOLD {
                let p = tmp.path().join(format!("citer-{idx}-{i}.md"));
                seed_ledger(
                    &cat,
                    &format!("citer-{idx}-{i}"),
                    &p,
                    &format!("see {id}\n"),
                );
            }
        }

        let ctx = ctx_rooted_at(cat, &active_root);
        let out = call(&ctx, json!({})).await.unwrap();

        let scoped = out["catalog_health"]["entry_validity_scoped_by_project"]
            .as_object()
            .unwrap_or_else(|| panic!("scoped-out rows must be announced: {out:#?}"));
        // Forward-slash, like `deg_key` and for the same reason: `outside_roots_group`
        // keys this map on the catalog's `abs_path` spelling, so a native lookup key
        // misses on Windows.
        let root_a_key = crate::util::fs::RepoPath::from_path(&sibling_a).into_string();
        let root_b_key = crate::util::fs::RepoPath::from_path(&sibling_b).into_string();
        assert_eq!(
            scoped.len(),
            2,
            "exactly two distinct project roots, not one collapsed bucket: {scoped:#?}"
        );
        assert_eq!(
            scoped.get(&root_a_key),
            Some(&json!(2)),
            "root A gets contributions from TWO DIFFERENT checks (conditional + dated) — \
             the call()-level fold must ACCUMULATE them, not overwrite: {scoped:#?}"
        );
        assert_eq!(
            scoped.get(&root_b_key),
            Some(&json!(1)),
            "root B must be its OWN key, not merged into root A's: {scoped:#?}"
        );
    }
}
