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
//! docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md

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
/// The shrink guard refuses an overwrite that cuts a file by more than half. That refusal keeps
/// the input too, so agreeing to the reduction does not mean sending the payload again, and the
/// replay carries `force=true` that the caller never sent.
/// docs/issues/archive/2026-11-08-create-file-shrink-guard-refusal-makes-the-agent-resend-the-whole-write.md
#[tokio::test]
async fn a_shrink_guard_refusal_keeps_the_write_and_an_ack_applies_it_with_force() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("big.txt");
    let original = "a".repeat(400);
    std::fs::write(&target, &original).unwrap();

    let (_e, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "tiny", "overwrite": true }),
    )
    .await;
    assert!(
        text.contains("shrink guard"),
        "precondition: the shrink guard must refuse the cut: {text}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), original);
    assert!(
        text.contains("create_file(ack=") && text.contains("@ack_"),
        "the refusal must name a handle: {text}"
    );
    let handle = handle_in(&text);

    // The replay carries NOTHING but the handle: no path, no content, no overwrite, no force.
    let (replay_err, replay) = call(&server, "create_file", json!({ "ack": handle })).await;
    assert!(!replay_err, "the replay must succeed: {replay}");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "tiny",
        "the replay must write the input the refusal kept, past the shrink guard"
    );
}

/// An `ack` replay of the existing-path refusal over a file that the content would cut by more
/// than half meets the shrink guard. That second refusal mints its own handle, so the caller never
/// has to send the payload again, however many refusals stand between it and the write.
#[tokio::test]
async fn an_existing_path_ack_that_meets_the_shrink_guard_mints_a_second_handle() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("big.txt");
    let original = "a".repeat(400);
    std::fs::write(&target, &original).unwrap();

    let (_e, first) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "tiny" }),
    )
    .await;
    assert!(
        first.contains("file already exists"),
        "precondition: the first refusal is the existing path's: {first}"
    );
    let (_e, second) = call(&server, "create_file", json!({ "ack": handle_in(&first) })).await;
    assert!(
        second.contains("shrink guard") && second.contains("@ack_"),
        "the replay meets the shrink guard, and that refusal must offer a handle: {second}"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), original);

    let (err, done) = call(&server, "create_file", json!({ "ack": handle_in(&second) })).await;
    assert!(!err, "the second replay must succeed: {done}");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "tiny");
}

/// A shrink-guard handle is bound to the file it saw, like the existing-path one.
#[tokio::test]
async fn a_shrink_guard_ack_does_not_apply_to_a_file_that_changed_since_the_refusal() {
    let (dir, server) = make_server().await;
    let target = dir.path().join("big.txt");
    std::fs::write(&target, "a".repeat(400)).unwrap();

    let (_e, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "tiny", "overwrite": true }),
    )
    .await;
    assert!(text.contains("shrink guard"), "precondition: {text}");
    let handle = handle_in(&text);

    let changed = "b".repeat(500);
    std::fs::write(&target, &changed).unwrap();

    let (_e, replay) = call(&server, "create_file", json!({ "ack": handle })).await;
    assert!(
        replay.contains("changed since"),
        "the replay must be refused: {replay}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        changed,
        "the file written by someone else must be left alone"
    );
}
