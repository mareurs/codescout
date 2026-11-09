//! A write refused by a read-only activation must be replayable by handle, not re-sent.
//!
//! The refusal used to tell the caller to pass `workspace=<root>` on the call, which means the
//! caller writes the whole `content` a second time. Measured in `usage.db` (2026-11-08), failed
//! writes whose next call repeated the same input are the cost this removes. The refusal now
//! keeps the input under an `@ack_*` handle, and `ack="@ack_…"` on any write tool replays it
//! with the pin the refusal asked for, so the lift is for that one call only.
//!
//! Every test drives the real dispatch (`call_tool_inner`): the mint is in the refusal branch and
//! the replay is before the pin is read, and a tool's own `call()` meets neither.
//!
//! docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md

use super::config_reload_tests::call;
use super::test_support::{make_server, make_server_with_project_toml};
use serde_json::json;

/// Activate a fresh directory read-only as the session default. Returns its canonical root.
async fn read_only_root(
    server: &super::CodeScoutServer,
    dir: &tempfile::TempDir,
) -> std::path::PathBuf {
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let (is_error, text) = call(
        server,
        "workspace",
        json!({ "action": "activate", "path": root.to_string_lossy(), "read_only": true }),
    )
    .await;
    assert!(
        !is_error,
        "precondition: the activation must succeed: {text}"
    );
    root
}

/// The `@ack_<8 hex>` handle named in a refusal.
fn handle_in(text: &str) -> String {
    let at = text
        .find("@ack_")
        .unwrap_or_else(|| panic!("the refusal names no @ack_ handle: {text}"));
    text[at..at + "@ack_".len() + 8].to_string()
}

#[tokio::test]
async fn a_read_only_refusal_keeps_the_write_and_an_ack_replays_it_without_resending() {
    let (_home, server) = make_server().await;
    let dir = tempfile::tempdir().unwrap();
    let root = read_only_root(&server, &dir).await;
    let target = root.join("kept.txt");

    let (refused, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "the whole payload" }),
    )
    .await;
    assert!(
        refused,
        "precondition: the read-only gate must refuse: {text}"
    );
    assert!(
        text.contains("File writes are disabled"),
        "it must be the gate that refused: {text}"
    );
    assert!(!target.exists(), "a refused write must write nothing");
    assert!(
        text.contains("ack=\"@ack_"),
        "the refusal must say how to replay the kept write: {text}"
    );
    assert!(
        text.contains("create_file"),
        "the refusal must name the tool the handle replays through: {text}"
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

    // The lift was for that one call. The activation is still read-only.
    let other = root.join("second.txt");
    let (again, again_text) = call(
        &server,
        "create_file",
        json!({ "path": other.to_string_lossy(), "content": "x" }),
    )
    .await;
    assert!(
        again && again_text.contains("File writes are disabled"),
        "an unpinned write after the replay must still be refused: {again_text}"
    );
    assert!(!other.exists(), "nothing may have been written");
}

#[tokio::test]
async fn an_ack_cannot_be_replayed_through_a_different_tool() {
    let (_home, server) = make_server().await;
    let dir = tempfile::tempdir().unwrap();
    let root = read_only_root(&server, &dir).await;
    let target = root.join("only-create.txt");

    let (_, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "c" }),
    )
    .await;
    let handle = handle_in(&text);

    let (is_error, reply) = call(&server, "edit_file", json!({ "ack": handle })).await;
    // A RecoverableError is `{"ok": false, …}` with `is_error` unset, by design: sibling calls in
    // the turn must survive it. So the refusal is read from the body, not from the flag.
    assert!(
        is_error || reply.contains("\"ok\": false"),
        "a handle minted by create_file must not run edit_file: {reply}"
    );
    assert!(
        reply.contains("create_file") && reply.contains("edit_file"),
        "the refusal must name both tools: {reply}"
    );
    assert!(!target.exists(), "nothing may have been written");
}

#[tokio::test]
async fn an_unknown_ack_handle_is_refused_by_name() {
    let (_home, server) = make_server().await;
    let (is_error, reply) = call(&server, "create_file", json!({ "ack": "@ack_deadbeef" })).await;
    // A RecoverableError is `{"ok": false, …}` with `is_error` unset, so read the body.
    assert!(
        is_error || reply.contains("\"ok\": false"),
        "an unknown handle must be refused: {reply}"
    );
    assert!(
        reply.contains("expired or unknown"),
        "the error must say the handle is not held: {reply}"
    );
}

/// `file_write_enabled = false` is durable and a pin does not lift it, so a handle for it would
/// replay into the same refusal. No handle must be offered.
#[tokio::test]
async fn a_refusal_that_a_pin_cannot_lift_offers_no_ack() {
    let toml = "[project]\nname = \"t\"\n\n[security]\nfile_write_enabled = false\n";
    let (dir, server) = make_server_with_project_toml(Some(toml)).await;
    let target = dir.path().join("never.txt");
    let (is_error, text) = call(
        &server,
        "create_file",
        json!({ "path": target.to_string_lossy(), "content": "c" }),
    )
    .await;
    assert!(
        is_error,
        "precondition: writes are off in project.toml: {text}"
    );
    assert!(
        !text.contains("@ack_"),
        "a handle that cannot work must not be offered: {text}"
    );
}
/// An action `doc` does not advertise cannot write, so a read-only activation must not refuse it
/// as a write: the caller typed a wrong verb and must be told which verbs exist, not told to lift
/// read-only (and offered a handle that replays into the same unknown-action answer).
///
/// The control is the half that guards: a real write action is still refused by the same gate in
/// the same session, so the fix narrowed the classification and did not open the gate.
/// docs/issues/archive/2026-11-08-unknown-doc-action-is-refused-as-a-read-only-write.md
#[cfg(feature = "librarian")]
#[tokio::test]
async fn an_unknown_doc_action_under_a_read_only_activation_names_the_action_not_the_gate() {
    let (_home, server) = make_server().await;
    let dir = tempfile::tempdir().unwrap();
    read_only_root(&server, &dir).await;

    let (_e, text) = call(&server, "doc", json!({ "action": "read" })).await;
    assert!(
        text.contains("unknown action 'read'") && text.contains("find, get, create"),
        "the caller must be told the action is unknown, with the valid ones: {text}"
    );
    assert!(
        !text.contains("File writes are disabled") && !text.contains("@ack_"),
        "an action that cannot write must not meet the write gate or be offered a replay: {text}"
    );

    let (refused, control) = call(&server, "doc", json!({ "action": "create" })).await;
    assert!(
        refused && control.contains("File writes are disabled"),
        "control: a real write action must still be refused under read-only: {control}"
    );
}
/// The `librarian` twin of the `doc` test above: an unknown `librarian` action cannot write, so a
/// read-only activation names it and offers no handle, and `librarian(reindex)`, which writes, is
/// still refused by the same gate in the same session.
/// docs/issues/2026-11-09-unknown-librarian-action-is-refused-as-a-read-only-write.md
#[cfg(feature = "librarian")]
#[tokio::test]
async fn an_unknown_librarian_action_under_a_read_only_activation_names_the_action_not_the_gate() {
    let (_home, server) = make_server().await;
    let dir = tempfile::tempdir().unwrap();
    read_only_root(&server, &dir).await;

    let (_e, text) = call(&server, "librarian", json!({ "action": "frobnicate" })).await;
    assert!(
        text.contains("unknown action 'frobnicate'") && text.contains("context, reindex, status"),
        "the caller must be told the action is unknown, with the valid ones: {text}"
    );
    assert!(
        !text.contains("File writes are disabled") && !text.contains("@ack_"),
        "an action that cannot write must not meet the write gate or be offered a replay: {text}"
    );

    let (refused, control) = call(&server, "librarian", json!({ "action": "reindex" })).await;
    assert!(
        refused && control.contains("File writes are disabled"),
        "control: a real write action must still be refused under read-only: {control}"
    );
}
