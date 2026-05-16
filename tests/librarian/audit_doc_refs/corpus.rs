//! Tier-2 fixture corpus for audit_doc_refs.
//!
//! v1 scope: clean_repo + drift_repo only. Future fixtures (regression_repo,
//! wontfix_repo, archive_drop_repo, parse_recovery_repo) are listed below as
//! #[ignore] stubs; they will fill in after Task 15 (tracker auto-create)
//! and Task 16 (OutputGuard) land — both touch the test fixture shape.
//!
//! Context-construction strategy: `mk_ctx` is inlined here (same shape as
//! `tests/librarian/timemachine_smoke.rs::mk_ctx`) because the internal
//! `mk_smoke_ctx` helper in `src/librarian/tools/audit_doc_refs/mod.rs` lives
//! inside `#[cfg(test)]` and is therefore inaccessible from external crates.

use std::sync::Arc;

use codescout::librarian::{
    catalog::Catalog,
    current_project::CurrentProject,
    tools::{audit_doc_refs, ToolContext},
    workspace::{Root, WorkspaceConfig},
};
use tempfile::TempDir;

/// Returns the absolute path to a named fixture directory.
fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/librarian/audit_doc_refs/fixtures")
        .join(name)
}

/// Build a ToolContext rooted at `root` with an in-memory catalog and no LSP.
/// `current_project` is set so the scan pipeline can locate `repo_root`.
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
        current_project: Some(Arc::new(CurrentProject {
            abs_path: root.clone(),
            git_root: root,
            umbrella: None,
        })),
    }
}

// ---------------------------------------------------------------------------
// T2-MUST: clean_repo — all refs resolve, zero findings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clean_repo_yields_zero_findings() {
    let root = fixture_path("clean_repo");
    let ctx = mk_ctx(root);
    let result = audit_doc_refs::call(
        &ctx,
        serde_json::json!({
            "emit_tracker": false,
            "paths": ["docs/**/*.md"],
        }),
    )
    .await
    .unwrap();

    assert_eq!(
        result["n_refs_broken"], 0,
        "clean_repo: expected zero broken refs, got: {result}"
    );
    assert_eq!(
        result["exit_code"], 0,
        "clean_repo: expected exit_code 0, got: {result}"
    );
    assert_eq!(
        result["n_files_scanned"], 1,
        "clean_repo: expected 1 file scanned, got: {result}"
    );
}

// ---------------------------------------------------------------------------
// T2-MUST: drift_repo — known distribution of broken + unknown refs
//
// docs/spec.md contains:
//   src/gone1.py     → FilePath → Missing   (file absent)
//   src/gone2.rs     → FilePath → Missing   (file absent)
//   src/keeper.py:999 → FileLine → LineOob  (file exists, line > EOF)
//   unknown.module.one → ModulePath → Unknown (no LSP in v1)
//   unknown.module.two → ModulePath → Unknown (no LSP in v1)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn drift_repo_yields_expected_distribution() {
    let root = fixture_path("drift_repo");
    let ctx = mk_ctx(root);
    let result = audit_doc_refs::call(
        &ctx,
        serde_json::json!({
            "emit_tracker": false,
            "paths": ["docs/**/*.md"],
        }),
    )
    .await
    .unwrap();

    let findings = result["findings"].as_array().unwrap();

    let count = |verdict: &str| {
        findings
            .iter()
            .filter(|f| f["verdict"].as_str() == Some(verdict))
            .count()
    };

    assert_eq!(
        count("missing"),
        2,
        "drift_repo: expected 2 'missing' refs (gone1.py + gone2.rs), got: {}",
        result["findings"]
    );
    assert_eq!(
        count("line_oob"),
        1,
        "drift_repo: expected 1 'line_oob' ref (keeper.py:999), got: {}",
        result["findings"]
    );
    assert!(
        count("unknown") >= 2,
        "drift_repo: expected >=2 'unknown' refs (module paths with no LSP), got: {}",
        result["findings"]
    );

    // Totals must be consistent with individual counts.
    assert_eq!(
        result["n_refs_broken"], 3,
        "drift_repo: n_refs_broken should be 3 (2 missing + 1 line_oob), got: {result}"
    );
    assert!(
        result["n_refs_unknown"].as_u64().unwrap_or(0) >= 2,
        "drift_repo: n_refs_unknown should be >=2, got: {result}"
    );
}

// ---------------------------------------------------------------------------
// T2-STUB: future fixtures (Task 15 + Task 16 territory)
// ---------------------------------------------------------------------------

/// regression_repo: pre-seeded tracker with a `fixed` issue — re-scan must
/// not re-open it. Requires tracker write path (Task 15).
#[tokio::test]
#[ignore = "TODO Task 15: tracker auto-create needed to pre-seed fixed issue"]
async fn regression_repo_fixed_issue_not_reopened() {
    let _tmp = TempDir::new().unwrap();
    todo!("wire regression_repo fixture after Task 15 lands");
}

/// wontfix_repo: pre-seeded tracker with a `wontfix` issue — re-scan must
/// never flip it back to `open`. Requires tracker write path (Task 15).
#[tokio::test]
#[ignore = "TODO Task 15: tracker auto-create needed to pre-seed wontfix issue"]
async fn wontfix_repo_issue_stays_wontfix() {
    let _tmp = TempDir::new().unwrap();
    todo!("wire wontfix_repo fixture after Task 15 lands");
}

/// archive_drop_repo: same drift in docs/ and docs/archive/ — severity must
/// be lower for the archive path. Requires OutputGuard pagination (Task 16).
#[tokio::test]
#[ignore = "TODO Task 16: severity-drop in archive path; requires fixture expansion"]
async fn archive_drop_repo_severity_lower_in_archive() {
    let _tmp = TempDir::new().unwrap();
    todo!("wire archive_drop_repo fixture after Task 16 lands");
}

/// parse_recovery_repo: one file with a malformed code fence — parser must
/// emit a ParseWarning and continue scanning remaining files cleanly.
#[tokio::test]
#[ignore = "TODO Task 16: parse-recovery fixture; low priority, parser already tested in unit tests"]
async fn parse_recovery_repo_emits_warning_and_continues() {
    let _tmp = TempDir::new().unwrap();
    todo!("wire parse_recovery_repo fixture after Task 16 lands");
}
