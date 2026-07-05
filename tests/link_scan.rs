//! Tier-2 corpus for `librarian(action="link_scan")` — end-to-end over an
//! in-memory catalog + tempdir artifact files.
//!
//! Pins the risk spots from the design-validation memo:
//! - hand-made rels (`supersedes`, `evidence-for`) survive write mode;
//! - scoped prune never touches edges owned by unscanned artifacts;
//! - a moved citation yields exactly {prune old, add new};
//! - idempotency (second write run is a no-op);
//! - reindex durability: the abs_path pre-clean cascade-drops edges when an
//!   artifact's id churns — a re-scan regenerates them (the feature's core
//!   durability claim).

use std::sync::Arc;

use codescout::librarian::{
    catalog::{artifact, artifact::ArtifactRow, links, Catalog},
    current_project::CurrentProject,
    tools::{link_scan, ToolContext},
    workspace::{Root, WorkspaceConfig},
};
use tempfile::TempDir;

fn mk_ctx(root: std::path::PathBuf) -> ToolContext {
    ToolContext {
        catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "r".into(),
                path: root.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }),
        rules: Arc::new(vec![]),
        embedding: None,
        artifact_store: None,
        current_project: Some(Arc::new(CurrentProject {
            abs_path: root.clone(),
            git_root: root,
            umbrella: None,
        })),
    }
}

fn row(id: &str, abs_path: std::path::PathBuf, status: &str) -> ArtifactRow {
    ArtifactRow {
        id: id.into(),
        abs_path,
        kind: "tracker".into(),
        status: status.into(),
        title: None,
        owners: vec![],
        tags: vec![],
        topic: None,
        time_scope: None,
        source: None,
        created_at: 0,
        updated_at: 0,
        file_mtime: 0,
        file_sha256: "".into(),
        confidence: 1.0,
    }
}

/// Write `content` at `root/rel` and upsert a catalog row for it.
fn add_artifact(ctx: &ToolContext, root: &std::path::Path, rel: &str, id: &str, content: &str) {
    let abs = root.join(rel);
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    std::fs::write(&abs, content).unwrap();
    let cat = ctx.catalog.lock();
    artifact::upsert(&cat, &row(id, abs, "active")).unwrap();
}

fn edge_set(ctx: &ToolContext) -> Vec<(String, String, String)> {
    let cat = ctx.catalog.lock();
    let mut all: Vec<_> = links::by_rel(&cat, "cites")
        .unwrap()
        .into_iter()
        .map(|l| (l.src_id, l.dst_id, l.rel))
        .collect();
    all.sort();
    all
}

const ID_A: &str = "aaaaaaaaaaaaaaa1";
const ID_B: &str = "bbbbbbbbbbbbbbb2";
const ID_C: &str = "ccccccccccccccc3";

#[tokio::test]
async fn derives_edges_end_to_end_and_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let ctx = mk_ctx(dir.path().to_path_buf());
    // A defines Z-1; B cites it three ways (token, 16-hex id, rel-path link)
    // — all collapse to ONE (B → A) edge.
    add_artifact(
        &ctx,
        dir.path(),
        "docs/a.md",
        ID_A,
        "## Z-1 — the defined entry\n\nbody\n",
    );
    add_artifact(
        &ctx,
        dir.path(),
        "docs/b.md",
        ID_B,
        &format!("See Z-1 and `{ID_A}` and [a](docs/a.md).\n"),
    );

    let out = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out["counts"]["edges_added"], 1, "one deduped edge: {out}");
    assert_eq!(
        edge_set(&ctx),
        vec![(ID_B.to_string(), ID_A.to_string(), "cites".to_string())]
    );

    // Idempotency: second write run reports nothing to do.
    let out2 = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out2["counts"]["edges_added"], 0, "{out2}");
    assert_eq!(out2["counts"]["edges_pruned"], 0, "{out2}");
    assert_eq!(out2["counts"]["edges_unchanged"], 1, "{out2}");
}

#[tokio::test]
async fn hand_made_rels_survive_write_mode() {
    let dir = TempDir::new().unwrap();
    let ctx = mk_ctx(dir.path().to_path_buf());
    add_artifact(&ctx, dir.path(), "docs/a.md", ID_A, "no citations here\n");
    add_artifact(&ctx, dir.path(), "docs/b.md", ID_B, "none here either\n");
    {
        let cat = ctx.catalog.lock();
        for rel in ["supersedes", "evidence-for"] {
            links::insert(
                &cat,
                &links::LinkRow {
                    src_id: ID_A.into(),
                    dst_id: ID_B.into(),
                    rel: rel.into(),
                    created_at: 1,
                },
            )
            .unwrap();
        }
    }

    link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();

    let cat = ctx.catalog.lock();
    let out = links::outgoing(&cat, ID_A).unwrap();
    assert_eq!(out.len(), 2, "manual rels must be untouched: {out:?}");
}

#[tokio::test]
async fn scoped_prune_spares_unscanned_srcs() {
    let dir = TempDir::new().unwrap();
    let other = TempDir::new().unwrap(); // outside the scanned project
    let ctx = mk_ctx(dir.path().to_path_buf());
    add_artifact(&ctx, dir.path(), "docs/a.md", ID_A, "## Z-1 — target\n");
    // C lives OUTSIDE the project scope, with a pre-existing cites edge whose
    // prose justification we can't see (its file is never scanned).
    add_artifact(
        &ctx,
        other.path(),
        "docs/c.md",
        ID_C,
        "cites nothing anymore\n",
    );
    {
        let cat = ctx.catalog.lock();
        links::insert(
            &cat,
            &links::LinkRow {
                src_id: ID_C.into(),
                dst_id: ID_A.into(),
                rel: "cites".into(),
                created_at: 1,
            },
        )
        .unwrap();
    }

    let out = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out["counts"]["edges_pruned"], 0, "{out}");
    assert_eq!(
        edge_set(&ctx),
        vec![(ID_C.to_string(), ID_A.to_string(), "cites".to_string())],
        "out-of-scope edge must survive"
    );
}

#[tokio::test]
async fn moved_citation_yields_exact_prune_and_add() {
    let dir = TempDir::new().unwrap();
    let ctx = mk_ctx(dir.path().to_path_buf());
    add_artifact(&ctx, dir.path(), "docs/a.md", ID_A, "## Z-1 — target\n");
    add_artifact(&ctx, dir.path(), "docs/b.md", ID_B, "cites Z-1 today\n");
    link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(
        edge_set(&ctx),
        vec![(ID_B.to_string(), ID_A.to_string(), "cites".to_string())]
    );

    // The citation moves from B to a new doc C.
    add_artifact(&ctx, dir.path(), "docs/b.md", ID_B, "no citation anymore\n");
    add_artifact(&ctx, dir.path(), "docs/c.md", ID_C, "now C cites Z-1\n");

    let out = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out["counts"]["edges_added"], 1, "{out}");
    assert_eq!(out["counts"]["edges_pruned"], 1, "{out}");
    assert_eq!(
        edge_set(&ctx),
        vec![(ID_C.to_string(), ID_A.to_string(), "cites".to_string())]
    );
}

#[tokio::test]
async fn ambiguous_and_dangling_report_without_edges() {
    let dir = TempDir::new().unwrap();
    let ctx = mk_ctx(dir.path().to_path_buf());
    // Two ACTIVE definers of F-1 → ambiguous, no edge.
    add_artifact(
        &ctx,
        dir.path(),
        "docs/log1.md",
        ID_A,
        "## F-1 — in log one\n",
    );
    add_artifact(
        &ctx,
        dir.path(),
        "docs/log2.md",
        ID_B,
        "## F-1 — in log two\n",
    );
    // C cites F-1 (ambiguous), F-99 (dangling, prefix known),
    // a stale 16-hex id (dangling), and UTF-8 (suppressed noise).
    add_artifact(
        &ctx,
        dir.path(),
        "docs/c.md",
        ID_C,
        "See F-1 and F-99 and `dddddddddddddddd` in UTF-8 text.\n",
    );

    let out = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out["counts"]["edges_added"], 0, "{out}");
    assert_eq!(out["counts"]["ambiguous"], 1, "{out}");
    assert_eq!(out["counts"]["dangling"], 2, "F-99 + stale id: {out}");
    assert!(edge_set(&ctx).is_empty());
}

#[tokio::test]
async fn reindex_id_churn_cascade_is_healed_by_rescan() {
    let dir = TempDir::new().unwrap();
    let ctx = mk_ctx(dir.path().to_path_buf());
    add_artifact(&ctx, dir.path(), "docs/a.md", ID_A, "## Z-1 — target\n");
    add_artifact(&ctx, dir.path(), "docs/b.md", ID_B, "cites Z-1\n");
    link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(edge_set(&ctx).len(), 1);

    // Simulate reindex id churn on the DEFINING artifact: upserting the same
    // abs_path under a new id triggers the pre-clean (abs_path wins), which
    // CASCADE-drops the old row and all its edges.
    const ID_A2: &str = "aaaaaaaaaaaaaaa9";
    {
        let cat = ctx.catalog.lock();
        artifact::upsert(&cat, &row(ID_A2, dir.path().join("docs/a.md"), "active")).unwrap();
        assert!(
            links::by_rel(&cat, "cites").unwrap().is_empty(),
            "pre-clean cascade must have dropped the edge (the documented hazard)"
        );
    }

    // A re-scan regenerates the edge against the new id — the durability claim.
    let out = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out["counts"]["edges_added"], 1, "{out}");
    assert_eq!(
        edge_set(&ctx),
        vec![(ID_B.to_string(), ID_A2.to_string(), "cites".to_string())]
    );

    // And the healed state is a fixpoint.
    let out2 = link_scan::call(&ctx, serde_json::json!({"write": true}))
        .await
        .unwrap();
    assert_eq!(out2["counts"]["edges_added"], 0, "{out2}");
    assert_eq!(out2["counts"]["edges_pruned"], 0, "{out2}");
}
