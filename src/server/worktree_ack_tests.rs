//! A write refused by the worktree guard must be replayable by handle, not re-sent.
//!
//! The guard (`guard_worktree_write`) refuses inside the tool and names a `workspace=<root>` pin
//! as the remedy, so the caller used to write the whole payload a second time. The dispatcher now
//! keeps the input under an `@ack_*` handle, and `ack="@ack_…"` replays it with that pin.
//!
//! Every test drives the real dispatch (`call_tool_inner`): the mint is in the dispatcher's error
//! arm, and a tool's own `call()` never reaches it.
//!
//! docs/issues/archive/2026-11-08-worktree-write-guard-refusal-makes-the-agent-resend-the-whole-write.md

use super::config_reload_tests::call;
use super::test_support::make_server_in_root;
use serde_json::json;

/// A server whose project root `<tmp>/main` has one linked worktree, and whose session never
/// called `activate`: the exact state in which the guard refuses an unpinned write.
async fn server_with_a_linked_worktree() -> (tempfile::TempDir, super::CodeScoutServer) {
    let (dir, server) = make_server_in_root(None, Some("main")).await;
    let root = dir.path().join("main");
    let wt_root = dir.path().join("wt-feat");
    std::fs::create_dir_all(&wt_root).unwrap();
    let entry = root.join(".git").join("worktrees").join("feat");
    std::fs::create_dir_all(&entry).unwrap();
    std::fs::write(
        entry.join("gitdir"),
        format!("{}/.git\n", wt_root.display()),
    )
    .unwrap();
    (dir, server)
}

fn handle_in(text: &str) -> String {
    let at = text
        .find("@ack_")
        .unwrap_or_else(|| panic!("the refusal names no @ack_ handle: {text}"));
    text[at..at + "@ack_".len() + 8].to_string()
}

#[tokio::test]
async fn a_worktree_refusal_keeps_the_write_and_an_ack_replays_it_without_resending() {
    let (dir, server) = server_with_a_linked_worktree().await;
    let target = dir.path().join("main").join("kept.txt");

    let (_err, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "the whole payload" }),
    )
    .await;
    assert!(
        text.contains("git worktrees detected"),
        "precondition: the worktree guard must refuse: {text}"
    );
    assert!(!target.exists(), "a refused write must write nothing");
    assert!(
        text.contains("ack=\"@ack_") && text.contains("create_file"),
        "the refusal must name the handle and the tool it replays through: {text}"
    );
    let handle = handle_in(&text);

    // The replay carries NOTHING but the handle: no path, no content.
    let (replay_err, replay) = call(&server, "create_file", json!({ "ack": handle })).await;
    assert!(!replay_err, "the replay must succeed: {replay}");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "the whole payload",
        "the replay must write the input the refusal kept, verbatim"
    );

    // The pin was for that one call. The session still has not chosen a project.
    let other = dir.path().join("main").join("second.txt");
    let (_e, again) = call(
        &server,
        "create_file",
        json!({ "path": other.to_string_lossy(), "content": "x" }),
    )
    .await;
    assert!(
        again.contains("git worktrees detected"),
        "an unpinned write after the replay must still be refused: {again}"
    );
    assert!(!other.exists(), "nothing may have been written");
}

/// A pinned call is never refused by the guard, so it must not be handed a handle either.
#[tokio::test]
async fn a_pinned_write_is_not_refused_and_offers_no_ack() {
    let (dir, server) = server_with_a_linked_worktree().await;
    let root = std::fs::canonicalize(dir.path().join("main")).unwrap();
    let target = root.join("pinned.txt");
    let (is_error, text) = call(
        &server,
        "create_file",
        json!({
            "path": target.to_string_lossy(),
            "content": "c",
            "workspace": root.to_string_lossy(),
        }),
    )
    .await;
    assert!(!is_error, "a pinned write must succeed: {text}");
    assert!(!text.contains("@ack_"), "no handle for a success: {text}");
    assert!(target.exists());
}
