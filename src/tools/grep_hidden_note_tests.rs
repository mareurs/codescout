//! A `grep` result WITH matches skipped the dot-prefixed entries at the search root, and said
//! nothing: the reader got a partial list that read as complete. Measured 2026-10-31 on this repo:
//! `grep("gate-slot")` gave 23 files and no note; `include_hidden=true` gave 27, among them the CI
//! workflow that is the only runner of the suite. The zero-match result already said so (its
//! `completeness_warning`); these tests hold the non-zero half, `hidden_skipped`.
//!
//! docs/issues/archive/2026-10-31-a-grep-result-with-matches-is-silent-about-the-hidden-path-skip.md

use super::*;
use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::tools::ToolContext;

/// The same context as `tests::rooted_ctx` in `grep.rs`, which is private to that module.
async fn rooted_ctx(root: &std::path::Path) -> ToolContext {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    ToolContext {
        agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
            crate::tools::guide_ledger::GuideLedger::mid_session(),
        )),
        workspace_override: None,
    }
}

/// A visible `src/a.rs` and a pruned `.github/ci.yml`, both holding the pattern.
fn tree_with_a_hidden_match() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github")).unwrap();
    std::fs::write(dir.path().join(".github/ci.yml"), "TARGET_NAME\n").unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/a.rs"), "// TARGET_NAME\n").unwrap();
    dir
}

#[tokio::test]
async fn a_result_with_matches_names_the_hidden_root_entries_it_skipped() {
    let dir = tree_with_a_hidden_match();
    let ctx = rooted_ctx(dir.path()).await;
    let root = dir.path().to_str().unwrap();

    // Both producers: the files-mode early return and the main tail.
    for mode in ["content", "files"] {
        let res = Grep
            .call(
                json!({ "pattern": "TARGET_NAME", "path": root, "mode": mode }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(
            res["total"].as_u64().unwrap(),
            1,
            "precondition ({mode}): {res}"
        );
        let note = res["hidden_skipped"].as_str().unwrap_or_else(|| {
            panic!(
                "a result with matches must say hidden root entries were skipped ({mode}): {res}"
            )
        });
        assert!(
            note.contains(".github/"),
            "must name the skipped entry ({mode}): {note}"
        );
        assert!(
            note.contains("include_hidden"),
            "must name the remedy ({mode}): {note}"
        );
        assert!(
            res.get("completeness_warning").is_none(),
            "the zero-match warning says 'this zero', so it must not ride a non-zero ({mode}): {res}"
        );
        let rendered = format_grep(&res);
        assert!(
            rendered.contains(note),
            "the rendered result must carry the note ({mode}): {rendered}"
        );
    }

    // Positive twin: with hidden paths searched, the hidden match is found and the note goes.
    let widened = Grep
        .call(
            json!({ "pattern": "TARGET_NAME", "path": root, "include_hidden": true }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(widened["total"].as_u64().unwrap(), 2, "{widened}");
    assert!(widened.get("hidden_skipped").is_none(), "{widened}");
}

/// The note must not ride every result: with no informative hidden entry at the root, or with a
/// glob that cannot reach one, nothing was hidden from this query.
#[tokio::test]
async fn a_result_with_matches_stays_bare_when_nothing_hidden_was_reachable() {
    let clean = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(clean.path().join(".git")).unwrap();
    // Hidden FILES at the root: nearly every repository has a `.gitignore`, so a note that
    // counted them would ride almost every search. Only hidden directories are named.
    std::fs::write(clean.path().join(".gitignore"), "target/\n").unwrap();
    std::fs::write(clean.path().join(".env"), "X=1\n").unwrap();
    std::fs::write(clean.path().join("a.rs"), "// TARGET_NAME\n").unwrap();
    let ctx = rooted_ctx(clean.path()).await;
    let res = Grep
        .call(
            json!({ "pattern": "TARGET_NAME", "path": clean.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(res["total"].as_u64().unwrap(), 1, "precondition: {res}");
    assert!(
        res.get("hidden_skipped").is_none(),
        "only `.git`, `.codescout` and hidden files are at the root, and none is named: {res}"
    );

    let dir = tree_with_a_hidden_match();
    let ctx = rooted_ctx(dir.path()).await;
    let res = Grep
        .call(
            json!({
                "pattern": "TARGET_NAME",
                "path": dir.path().to_str().unwrap(),
                "glob": "src/*.rs",
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(res["total"].as_u64().unwrap(), 1, "precondition: {res}");
    assert!(
        res.get("hidden_skipped").is_none(),
        "a glob anchored under `src/` cannot reach `.github/`: {res}"
    );
}
