//! Fork-on-first-write for the worktree overlay. See
//! docs/superpowers/specs/2026-07-17-worktree-overlay-design.md §3.
//!
//! Wired into every mutating artifact handler (append_entry/update/
//! event_create/augment/link/delete/mv/create) — see the
//! `resolve_write_target`/`ensure_registration` call sites in
//! `librarian::tools::*`.

use anyhow::Result;
use serde_json::json;

use super::ToolContext;
use crate::librarian::catalog::{artifact, augmentation, events, links, worktree as reg, Catalog};
use crate::librarian::current_project::CurrentProject;
use crate::librarian::ids;
use crate::util::fs::RepoPath;

pub(crate) const FORK_EVENT_KIND: &str = "worktree_fork";
pub(crate) const LINEAGE_REL: &str = "worktree_of";

fn under(path: &str, root: &str) -> bool {
    path == root || path.starts_with(&format!("{root}/"))
}
/// True iff `row_abs_path` belongs to the MAIN checkout as seen from session `cp`:
/// the session is a worktree (`main_root.is_some()`), and the path is under
/// `main_root` but NOT under the worktree's own root. False for non-worktree
/// sessions, worktree-born rows (under the worktree root), and foreign-repo rows.
pub(crate) fn is_main_checkout_artifact(
    cp: &CurrentProject,
    row_abs_path: &std::path::Path,
) -> bool {
    let Some(main_root) = cp.main_root.as_ref() else {
        return false;
    };
    let main_s = RepoPath::from(main_root.as_path()).into_string();
    let wt_s = RepoPath::from(cp.git_root.as_path()).into_string();
    let row = RepoPath::from(row_abs_path).into_string();
    under(&row, &main_s) && !under(&row, &wt_s)
}

/// Best-effort branch name: worktree `.git` file → gitdir → `<gitdir>/HEAD`
/// → `ref: refs/heads/<branch>`. Filesystem-only, like current_project.rs.
fn read_branch(worktree_root: &std::path::Path) -> Option<String> {
    let gitfile = std::fs::read_to_string(worktree_root.join(".git")).ok()?;
    let gitdir = gitfile.strip_prefix("gitdir:")?.trim();
    let head = std::fs::read_to_string(std::path::Path::new(gitdir).join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(String::from)
}

/// Upsert the durable registration for a worktree session. No-op otherwise.
pub(crate) fn ensure_registration(cat: &Catalog, cp: &CurrentProject) -> Result<()> {
    let Some(main_root) = cp.main_root.as_ref() else {
        return Ok(());
    };
    reg::upsert_active(
        cat,
        &RepoPath::from(cp.git_root.as_path()).into_string(),
        &RepoPath::from(main_root.as_path()).into_string(),
        read_branch(&cp.git_root).as_deref(),
        chrono::Utc::now().timestamp_millis(),
    )
}

/// The overlay write gate. A mutating action targeting a MAIN-root artifact
/// from a worktree session forks it: shadow row at the worktree path (seeded
/// from the main row), `worktree_fork` event carrying the base snapshot,
/// `worktree_of` lineage link, durable registration. Returns the id the write
/// must proceed against. Everything else passes through unchanged.
///
/// NOTE (F-2, session log): the shadow's params are a base COPY + the
/// worktree's delta. merge_worktree extracts the delta against the fork
/// event's base — never bare-graft a seeded shadow.
pub(crate) fn resolve_write_target(
    cat: &mut Catalog,
    ctx: &ToolContext,
    id: &str,
) -> Result<String> {
    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok(id.to_string());
    };
    let Some(main_root) = cp.main_root.as_ref() else {
        return Ok(id.to_string());
    };
    let Some(row) = artifact::get(cat, id)? else {
        return Ok(id.to_string()); // unknown id: let the caller produce its own error
    };
    if !is_main_checkout_artifact(cp, &row.abs_path) {
        return Ok(id.to_string()); // already shadow, or foreign repo — no isolation (spec non-goal)
    }

    let main_s = RepoPath::from(main_root.as_path()).into_string();
    let wt_s = RepoPath::from(cp.git_root.as_path()).into_string();
    let row_path = RepoPath::from(row.abs_path.as_path()).into_string();
    let rel = row_path
        .strip_prefix(&format!("{main_s}/"))
        .unwrap_or(&row_path);
    let shadow_path = format!("{wt_s}/{rel}");
    let shadow_id = ids::artifact_id_from_abs(std::path::Path::new(&shadow_path));
    if artifact::get(cat, &shadow_id)?.is_some() {
        return Ok(shadow_id); // already forked
    }

    let now = chrono::Utc::now().timestamp_millis();
    let base_aug = augmentation::get(cat, id)?;
    let tx = cat.conn.unchecked_transaction()?;
    ensure_registration(cat, cp)?;
    // Seed the shadow from the main row. The FILE at the shadow path is git's
    // checkout copy — identical at fork — so file_mtime/file_sha256 carry over.
    let shadow_row = artifact::ArtifactRow {
        id: shadow_id.clone(),
        abs_path: std::path::PathBuf::from(&shadow_path),
        created_at: now,
        updated_at: now,
        ..row.clone()
    };
    artifact::upsert(cat, &shadow_row)?;
    if let Some(aug) = &base_aug {
        let mut shadow_aug = aug.clone();
        shadow_aug.artifact_id = shadow_id.clone();
        augmentation::upsert(cat, &shadow_aug)?;
    }
    events::insert(
        cat,
        &events::EventRow {
            id: ulid::Ulid::new().to_string(),
            artifact_id: shadow_id.clone(),
            kind: FORK_EVENT_KIND.into(),
            payload: json!({
                "main_id": id,
                "branch": read_branch(&cp.git_root),
                "base_params": base_aug.as_ref()
                    .and_then(|a| serde_json::from_str::<serde_json::Value>(&a.params).ok()),
                "base_frontmatter": {
                    "status": row.status, "title": row.title, "tags": row.tags,
                    "topic": row.topic, "time_scope": row.time_scope, "owners": row.owners,
                },
            })
            .to_string(),
            anchor_commit: None,
            head_commit: None,
            author: Some("worktree-overlay".into()),
            created_at: now,
        },
    )?;
    links::insert(
        cat,
        &links::LinkRow {
            src_id: shadow_id.clone(),
            dst_id: id.to_string(),
            rel: LINEAGE_REL.into(),
            created_at: now,
        },
    )?;
    tx.commit()?;
    Ok(shadow_id)
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
    use crate::librarian::catalog::augmentation::{self, AugmentationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::ids;
    use crate::librarian::tools::{TestToolContextBuilder, ToolContext};
    use std::sync::Arc;

    /// A `ToolContext` whose `current_project` simulates a linked-worktree
    /// session (`git_root` under `/repo/.worktrees/feat`, `main_root` =
    /// `/repo`). Shared by later worktree-overlay task tests.
    pub(crate) fn wt_ctx(cat: Catalog) -> ToolContext {
        TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(CurrentProject {
                abs_path: "/repo/.worktrees/feat".into(),
                git_root: "/repo/.worktrees/feat".into(),
                main_root: Some("/repo".into()),
                umbrella: None,
            }))
            .build()
    }

    /// Seeds a main-root tracker artifact (with augmentation params) at
    /// `/repo/docs/trackers/t.md` and returns its id. Shared by later
    /// worktree-overlay task tests.
    pub(crate) fn seed_main_tracker(cat: &Catalog) -> String {
        let id = ids::artifact_id_from_abs(std::path::Path::new("/repo/docs/trackers/t.md"));
        artifact::upsert(
            cat,
            &TestArtifactRowBuilder::new(&id)
                .with_abs_path("/repo/docs/trackers/t.md")
                .with_kind("tracker")
                .build(),
        )
        .unwrap();
        augmentation::upsert(
            cat,
            &AugmentationRow {
                artifact_id: id.clone(),
                prompt: "p".into(),
                params: r#"{"items":[{"id":"F-1","t":"a"}],"note":"base"}"#.into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("items".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        id
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{seed_main_tracker, wt_ctx};
    use super::*;
    use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
    use crate::librarian::tools::TestToolContextBuilder;
    use std::sync::Arc;

    #[test]
    fn fork_creates_shadow_with_lineage_registration_and_base() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let cat = ctx.catalog.lock();
            seed_main_tracker(&cat)
        };
        let shadow_id = {
            let mut cat = ctx.catalog.lock();
            resolve_write_target(&mut cat, &ctx, &main_id).unwrap()
        };
        assert_ne!(shadow_id, main_id);
        let cat = ctx.catalog.lock();
        let shadow = artifact::get(&cat, &shadow_id).unwrap().unwrap();
        assert_eq!(
            shadow.abs_path.to_string_lossy(),
            "/repo/.worktrees/feat/docs/trackers/t.md"
        );
        // params seeded from base
        let aug = augmentation::get(&cat, &shadow_id).unwrap().unwrap();
        assert!(aug.params.contains(r#""F-1""#));
        // lineage link
        let out = links::outgoing(&cat, &shadow_id).unwrap();
        assert!(out
            .iter()
            .any(|l| l.rel == "worktree_of" && l.dst_id == main_id));
        // fork event with base snapshot
        let ev = events::latest_for_artifact(&cat, &shadow_id)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, "worktree_fork");
        assert!(ev.payload.contains(r#""main_id""#));
        // base_params VALUE must be the full base snapshot — not a hash, not a
        // subset, not an empty object. Task 8's merge extracts the worktree
        // delta by diffing against this exact value, so a mutation that
        // shrinks/replaces it must fail here.
        let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
        let base = &payload["base_params"];
        assert_eq!(base["items"][0]["id"], "F-1");
        assert_eq!(base["note"], "base");
        // durable registration
        assert!(
            reg::get(&cat, "/repo/.worktrees/feat")
                .unwrap()
                .unwrap()
                .status
                == "active"
        );
    }

    #[test]
    fn fork_is_idempotent() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let cat = ctx.catalog.lock();
            seed_main_tracker(&cat)
        };
        let a = {
            let mut c = ctx.catalog.lock();
            resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        let b = {
            let mut c = ctx.catalog.lock();
            resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        assert_eq!(a, b);
        let cat = ctx.catalog.lock();
        // exactly one fork event
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE artifact_id=?1 AND kind='worktree_fork'",
                [&a],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn passthrough_for_non_worktree_session_and_foreign_targets() {
        // Non-worktree session: id unchanged.
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: "/repo".into(),
                git_root: "/repo".into(),
                main_root: None,
                umbrella: None,
            }))
            .build();
        let main_id = {
            let cat = ctx.catalog.lock();
            seed_main_tracker(&cat)
        };
        let got = {
            let mut c = ctx.catalog.lock();
            resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        assert_eq!(got, main_id);

        // Worktree session, target OUTSIDE main_root (umbrella peer): unchanged.
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let peer_id = {
            let cat = ctx.catalog.lock();
            let id =
                crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/other/doc.md"));
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new(&id)
                    .with_abs_path("/other/doc.md")
                    .build(),
            )
            .unwrap();
            id
        };
        let got = {
            let mut c = ctx.catalog.lock();
            resolve_write_target(&mut c, &ctx, &peer_id).unwrap()
        };
        assert_eq!(got, peer_id);
    }

    #[test]
    fn worktree_born_target_passes_through() {
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let id = {
            let cat = ctx.catalog.lock();
            let id = crate::librarian::ids::artifact_id_from_abs(std::path::Path::new(
                "/repo/.worktrees/feat/docs/new.md",
            ));
            artifact::upsert(
                &cat,
                &TestArtifactRowBuilder::new(&id)
                    .with_abs_path("/repo/.worktrees/feat/docs/new.md")
                    .build(),
            )
            .unwrap();
            id
        };
        let got = {
            let mut cat = ctx.catalog.lock();
            resolve_write_target(&mut cat, &ctx, &id).unwrap()
        };
        assert_eq!(got, id);
        // pure passthrough: no fork, no new shadow row, no lineage link
        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(events::latest_for_artifact(&cat, &id).unwrap().is_none());
        assert!(links::outgoing(&cat, &id).unwrap().is_empty());
    }
}
