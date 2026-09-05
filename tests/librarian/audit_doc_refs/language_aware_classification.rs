//! End-to-end wiring for `PathSyntax`: the same dotted token, in the same
//! position, in two languages, must classify differently.
//!
//! The rule itself is unit-tested in `parser::path_syntax_tests`. What those
//! tests cannot see is whether `scan_code_comments` hands the classifier the
//! language it was given — a one-line wiring that no unit test crosses. This
//! file drives the real tool through `audit_doc_refs::call` and compares two
//! files whose ONLY difference is their extension.
//!
//! Why it matters, measured 2026-08-16: across `src/librarian/catalog/**` (41
//! refs) 56% of findings came back `unknown`, essentially all of them dotted
//! identifiers in Rust doc comments naming SQL columns. The same scan over every
//! non-Rust source file in the repo reported 5% unknown. A language-blind fix
//! tuned on that Rust noise would have deleted real module references from
//! Python, Go, Java and Kotlin.

use std::path::PathBuf;
use std::sync::Arc;

use codescout::librarian::{
    catalog::Catalog,
    current_project::CurrentProject,
    tools::{audit_doc_refs, ToolContext},
    workspace::{Root, WorkspaceConfig},
};

fn mk_ctx(root: PathBuf) -> ToolContext {
    ToolContext {
        lsp: codescout::lsp::MockLspProvider::with_client(codescout::lsp::MockLspClient::default()),
        catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "fixture".into(),
                path: root.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }),
        rules: Arc::new(vec![]),
        temp_guard: codescout::librarian::tools::TempGuardEnv::from_env(),
        progress: None,
        embedding: None,
        artifact_store: None,
        current_project: Some(Arc::new(CurrentProject {
            abs_path: root.clone(),
            git_root: root,
            main_root: None,
            umbrella: None,
        })),
    }
}

async fn refs_found(root: &std::path::Path, glob: &str) -> u64 {
    let ctx = mk_ctx(root.to_path_buf());
    let result = audit_doc_refs::call(
        &ctx,
        serde_json::json!({ "emit_tracker": false, "paths": [glob] }),
    )
    .await
    .unwrap();
    result["n_refs_found"].as_u64().unwrap()
}

#[tokio::test]
async fn a_dotted_token_is_a_ref_in_python_and_not_in_rust() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();

    // Byte-identical citation text; only the comment marker and the extension
    // differ. `commits.git_root` is a SQL column in the Rust file and reads as a
    // module path in the Python one — the token cannot tell you which.
    std::fs::write(
        root.join("a.rs"),
        "/// Rewrites `commits.git_root` in place.\npub fn f() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("a.py"),
        "# Rewrites `commits.git_root` in place.\ndef f():\n    pass\n",
    )
    .unwrap();

    let rust = refs_found(root, "**/*.rs").await;
    let python = refs_found(root, "**/*.py").await;

    assert_eq!(
        rust, 0,
        "rust spells qualified names `a::b`, so a dotted token is field access \
         and must not be reported as a module path"
    );
    assert_eq!(
        python, 1,
        "python spells them with dots, so the SAME token must still be reported"
    );
}

#[tokio::test]
async fn a_real_file_path_is_found_in_both_languages() {
    // The guard against over-narrowing: `PathSyntax` must cost no FILE
    // reference in any language. If this ever drops to 0 on the Rust side, the
    // narrowing has escaped the module branch.
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/GUIDE.md"), "# guide\n").unwrap();

    std::fs::write(
        root.join("a.rs"),
        "/// See `docs/GUIDE.md` for the rationale.\npub fn f() {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("a.py"),
        "# See `docs/GUIDE.md` for the rationale.\ndef f():\n    pass\n",
    )
    .unwrap();

    assert_eq!(
        refs_found(root, "**/*.rs").await,
        1,
        "a real path must still be found in rust"
    );
    assert_eq!(
        refs_found(root, "**/*.py").await,
        1,
        "a real path must still be found in python"
    );
}
