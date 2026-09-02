//! Wiring proof for `open-issue-work-queue:BL-48`: `edit_file`'s markdown grammar must hand a
//! frontmatter write to the catalog-sync hook, and must not bother it for a body edit.
//!
//! ## Why this lives in `tests/` and not beside the code
//!
//! The hook lives in a process-wide slot (`util::librarian_sync::HOOK`), last-writer-wins
//! for the reason `librarian_guard::ORACLE` documents. In the unit-test binary that slot
//! is contested: server-building test helpers construct a librarian runtime, which
//! installs the real syncer and would replace a test's recorder mid-run. A test asserting
//! "my hook was called" would then fail depending on scheduling.
//!
//! `librarian_guard` has the identical exposure and resolves it by never testing the
//! global path — its own tests pass an oracle explicitly. That leaves the wiring itself
//! unproven, which is exactly the shape `bug-fix-session-log:W-73` warns about: a
//! guard that is written, tested, and never called reads as fully covered.
//!
//! An integration test is its own binary, so nothing else installs here. That makes the
//! global path testable without a race, and without weakening the assertion to tolerate
//! one.
//!
//! Both directions live in ONE test on purpose. Two `#[tokio::test]`s in this file would
//! each install their own recorder into the same slot and race each other — reintroducing,
//! between two tests I control, precisely the contention this file exists to escape.

use codescout::agent::Agent;
use codescout::lsp::LspManager;
use codescout::tools::edit_file::EditFile;
use codescout::tools::{Tool, ToolContext};
use codescout::util::librarian_sync::{install_catalog_sync, CatalogFrontmatterSync};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Records every path the tool hands it. Reports `false` — "not an artifact" — because
/// this test is about *whether the hook is reached*, not about what a real catalog
/// would do with it. The catalog behaviour is covered in `librarian::adapter`'s tests
/// against a real `Catalog`.
#[derive(Default)]
struct Recorder {
    seen: Mutex<Vec<PathBuf>>,
}

impl Recorder {
    fn names(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect()
    }
}

impl CatalogFrontmatterSync for Recorder {
    fn sync_frontmatter(&self, abs_path: &Path) -> bool {
        self.seen.lock().unwrap().push(abs_path.to_path_buf());
        false
    }
}

async fn ctx_in(dir: &Path) -> ToolContext {
    std::fs::create_dir_all(dir.join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.to_path_buf())).await.unwrap();
    ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: Arc::new(codescout::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(std::sync::Mutex::new(
            codescout::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    }
}

#[tokio::test]
async fn edit_markdown_syncs_the_catalog_on_a_frontmatter_write_and_not_on_a_body_edit() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_in(dir.path()).await;

    let recorder = Arc::new(Recorder::default());
    install_catalog_sync(recorder.clone());

    // ── A frontmatter write MUST reach the hook ──────────────────────────────
    // This is BL-48's whole mechanism: the file gets `status: fixed`, and without
    // this call the catalog row keeps its pre-edit value forever.
    std::fs::write(
        dir.path().join("bug.md"),
        "---\nkind: bug\nstatus: open\n---\n\n## Summary\n\nold\n",
    )
    .unwrap();

    EditFile
        .call(
            json!({ "path": "bug.md", "frontmatter": { "set": { "status": "fixed" } } }),
            &ctx,
        )
        .await
        .expect("frontmatter write should succeed");

    assert_eq!(
        recorder.names(),
        vec!["bug.md".to_string()],
        "a frontmatter write must hand its path to the catalog sync — without this the \
         row silently keeps the pre-edit status, which is BL-48"
    );

    // ── A body-only edit must NOT ────────────────────────────────────────────
    // Not an optimisation detail: a body edit cannot move an indexed column, so a
    // catalog write per body edit would be pure cost on the hottest markdown path.
    std::fs::write(
        dir.path().join("notes.md"),
        "---\nkind: bug\nstatus: open\n---\n\n## Summary\n\nold\n",
    )
    .unwrap();

    EditFile
        .call(
            json!({
                "path": "notes.md",
                "heading": "## Summary",
                "action": "replace",
                "content": "new body\n"
            }),
            &ctx,
        )
        .await
        .expect("body edit should succeed");

    assert_eq!(
        recorder.names(),
        vec!["bug.md".to_string()],
        "a body-only edit must NOT reach the catalog sync; seeing notes.md here means \
         the call is unconditional rather than gated on frontmatter_changed"
    );
}
