//! A `create_file` refused by an existing path must be replayable by handle, not re-sent.
//!
//! The refusal used to tell the caller to pass `overwrite: true`, so the whole `content` crossed
//! the wire twice. `create_file` now keeps the input under an `@ack_*` handle, and
//! `create_file(ack="@ack_…")` replays it with `overwrite=true`, after checking that the file is
//! still the one the refusal described (size and modification time).
//!
//! Every test drives the real dispatch (`call_tool_inner`): the replay is in the dispatcher,
//! before the tool body, and a tool's own `call()` never reaches it.
//!
//! docs/issues/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md

use super::config_reload_tests::call;
use super::test_support::make_server;
use serde_json::json;

fn handle_in(text: &str) -> String {
    let at = text
        .find("@ack_")
        .unwrap_or_else(|| panic!("the refusal names no @ack_ handle: {text}"));
    text[at..at + "@ack_".len() + 8].to_string()
}

#[tokio::test]
async fn an_existing_path_refusal_keeps_the_write_and_an_ack_replaces_the_file() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("exists.txt");
    std::fs::write(&target, "old").unwrap();

    let (_e, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "the whole payload" }),
    )
    .await;
    assert!(
        text.contains("file already exists"),
        "precondition: create_file must refuse an existing path: {text}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
    assert!(
        text.contains("create_file(ack=") && text.contains("@ack_") && text.contains("3 bytes"),
        "the refusal must name the handle and say what would be replaced: {text}"
    );
    let handle = handle_in(&text);

    // The replay carries NOTHING but the handle: no path, no content, no overwrite.
    let (replay_err, replay) = call(&server, "create_file", json!({ "ack": handle })).await;
    assert!(!replay_err, "the replay must succeed: {replay}");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "the whole payload",
        "the replay must write the input the refusal kept, verbatim"
    );
}

/// The caller agreed to replace the file it was told about, not one that changed since.
#[tokio::test]
async fn an_ack_does_not_replace_a_file_that_changed_since_the_refusal() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("moved-on.txt");
    std::fs::write(&target, "old").unwrap();

    let (_e, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "the whole payload" }),
    )
    .await;
    let handle = handle_in(&text);

    std::fs::write(&target, "someone else wrote this meanwhile").unwrap();

    let (is_error, reply) = call(&server, "create_file", json!({ "ack": handle })).await;
    assert!(
        is_error || reply.contains("\"ok\": false"),
        "the replay over a changed file must be refused: {reply}"
    );
    assert!(
        reply.contains("changed since"),
        "the refusal must say why: {reply}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "someone else wrote this meanwhile",
        "the changed file must be left as it is"
    );
}

/// A handle replays through the tool that minted it, and `overwrite` is not a pin: a handle from
/// an existing-path refusal must not be usable to bypass the tool check.
#[tokio::test]
async fn an_overwrite_ack_cannot_be_replayed_through_another_tool() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("only-create.txt");
    std::fs::write(&target, "old").unwrap();
    let (_e, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "c" }),
    )
    .await;
    let handle = handle_in(&text);

    let (is_error, reply) = call(&server, "edit_file", json!({ "ack": handle })).await;
    assert!(
        is_error || reply.contains("\"ok\": false"),
        "a handle minted by create_file must not run edit_file: {reply}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "old");
}
