/// TimeMachine end-to-end smoke integration test.
///
/// Exercises the full tool chain in-process: ArtifactCreate → ArtifactEventCreate
/// → ArtifactTimeline → ArtifactStateAt → WorkspaceStateAt → ArtifactGet →
/// ArtifactGraph → ArtifactEventCreate (note) → ArtifactLink.
///
/// Mirrors the `mk_ctx` helper pattern from `src/tools/event_create.rs::tests`
/// (inlined here because `pub(crate)` is not accessible from an external
/// integration test crate).
use std::sync::Arc;

use librarian_mcp::{
    catalog::Catalog,
    tools::{
        create::ArtifactCreate, event_create::ArtifactEventCreate, get::ArtifactGet,
        graph::ArtifactGraph, link::ArtifactLink,
        state_at::ArtifactStateAt, timeline::ArtifactTimeline,
        workspace_state_at::WorkspaceStateAt, Tool, ToolContext,
    },
    workspace::{Root, WorkspaceConfig},
};
use serde_json::json;
use tempfile::TempDir;

fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
    ToolContext {
        catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
        workspace: Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "r".into(),
                path: tmp_root,
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }),
        rules: Arc::new(vec![]),
        embedding: None,
        current_project: None,
    }
}

/// Helper: create an artifact and return its id string.
async fn create_artifact(ctx: &ToolContext, rel_path: &str, kind: &str, title: &str) -> String {
    let v = ArtifactCreate
        .call(
            ctx,
            json!({
                "repo": "r",
                "rel_path": rel_path,
                "kind": kind,
                "title": title,
                "body": "smoke test body"
            }),
        )
        .await
        .expect("artifact_create should succeed");
    v["id"]
        .as_str()
        .expect("artifact_create must return an id")
        .to_string()
}

#[tokio::test]
async fn timemachine_full_chain() {
    let tmp = TempDir::new().unwrap();
    let ctx = mk_ctx(tmp.path().to_path_buf());

    // -----------------------------------------------------------------------
    // SETUP — seed two artifacts
    // -----------------------------------------------------------------------
    let tracker_id = create_artifact(&ctx, "tracker.md", "tracker", "Pivot Tracker").await;
    let spec_id = create_artifact(&ctx, "spec.md", "spec", "Design Spec").await;

    // -----------------------------------------------------------------------
    // 1. note event on tracker → assert event_id returned
    // -----------------------------------------------------------------------
    let note_resp = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "note",
                "payload": {"text": "initial note"}
            }),
        )
        .await
        .expect("note event should succeed");
    let note_id = note_resp["event_id"]
        .as_str()
        .expect("event_create must return event_id")
        .to_string();
    assert!(!note_id.is_empty(), "event_id must be non-empty");

    // -----------------------------------------------------------------------
    // 2. reviewed event on tracker (makes freshness=fresh)
    // -----------------------------------------------------------------------
    let _reviewed_resp = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "reviewed",
                "payload": {}
            }),
        )
        .await
        .expect("reviewed event should succeed");

    // -----------------------------------------------------------------------
    // 3. intent event on tracker with inputs referencing spec + anchor_commit
    // -----------------------------------------------------------------------
    let intent_resp = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "intent",
                "payload": {"hypothesis": "tracker drives spec"},
                "anchor_commit": "abc123",
                "also_mutates": [spec_id]
            }),
        )
        .await
        .expect("intent event should succeed");
    let intent_id = intent_resp["event_id"]
        .as_str()
        .expect("intent event must return event_id")
        .to_string();

    // -----------------------------------------------------------------------
    // 4. verdict event resolving the intent
    // -----------------------------------------------------------------------
    let verdict_resp = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .expect("verdict event should succeed");
    let verdict_id = verdict_resp["event_id"]
        .as_str()
        .expect("verdict event must return event_id")
        .to_string();

    // -----------------------------------------------------------------------
    // 5. failure: resolve same intent twice → error contains "already resolved"
    // -----------------------------------------------------------------------
    let double_resolve = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "verdict",
                "payload": {"outcome": "refuted"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await;
    assert!(
        double_resolve.is_err(),
        "resolving the same intent twice must fail"
    );
    let err_msg = double_resolve.unwrap_err().to_string();
    assert!(
        err_msg.contains("already resolved"),
        "error should mention 'already resolved', got: {err_msg}"
    );

    // -----------------------------------------------------------------------
    // 6. failure: kind=note with empty payload → error contains "note.text required"
    // -----------------------------------------------------------------------
    let bad_note = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "note",
                "payload": {}
            }),
        )
        .await;
    assert!(bad_note.is_err(), "note without text must fail");
    let err_msg = bad_note.unwrap_err().to_string();
    assert!(
        err_msg.contains("note.text required"),
        "error should mention 'note.text required', got: {err_msg}"
    );

    // -----------------------------------------------------------------------
    // 7. failure: unknown kind → error contains "unknown event kind"
    // -----------------------------------------------------------------------
    let unknown_kind = ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "bogus_kind",
                "payload": {}
            }),
        )
        .await;
    assert!(unknown_kind.is_err(), "unknown kind must fail");
    let err_msg = unknown_kind.unwrap_err().to_string();
    assert!(
        err_msg.contains("unknown event kind"),
        "error should mention 'unknown event kind', got: {err_msg}"
    );

    // -----------------------------------------------------------------------
    // 8. timeline returns ≥4 events newest-first;
    //    events[0] (verdict) has resolves_intent_id == intent_id;
    //    find the intent event and assert resolved_by_verdict_id == verdict_id
    // -----------------------------------------------------------------------
    let timeline_resp = ArtifactTimeline
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "limit": 100
            }),
        )
        .await
        .expect("timeline should succeed");
    let events = timeline_resp
        .as_array()
        .expect("timeline must return array");
    assert!(
        events.len() >= 4,
        "expected ≥4 events, got {}",
        events.len()
    );

    // Find the verdict event by its known ID and verify the resolves link.
    let verdict_event = events
        .iter()
        .find(|e| e["id"].as_str().unwrap_or("") == verdict_id)
        .expect("verdict event must appear in timeline");
    assert_eq!(
        verdict_event["resolves_intent_id"].as_str().unwrap_or(""),
        intent_id,
        "verdict event must have resolves_intent_id == intent_id"
    );

    // Find the intent event by its known ID and verify the back-link.
    let intent_event = events
        .iter()
        .find(|e| e["id"].as_str().unwrap_or("") == intent_id)
        .expect("intent event must appear in timeline");
    assert_eq!(
        intent_event["resolved_by_verdict_id"]
            .as_str()
            .unwrap_or(""),
        verdict_id,
        "intent event must carry resolved_by_verdict_id == verdict_id"
    );

    // -----------------------------------------------------------------------
    // 9. timeline kinds=["intent"] limit=10 → exactly 1 event
    // -----------------------------------------------------------------------
    let intent_only = ArtifactTimeline
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kinds": ["intent"],
                "limit": 10
            }),
        )
        .await
        .expect("timeline intent filter should succeed");
    let intent_events = intent_only.as_array().expect("must be array");
    assert_eq!(
        intent_events.len(),
        1,
        "kinds=[intent] must return exactly 1 event"
    );
    assert_eq!(
        intent_events[0]["kind"].as_str().unwrap(),
        "intent",
        "filtered event must be kind=intent"
    );

    // -----------------------------------------------------------------------
    // 10. timeline until=<very old timestamp> → 0 events
    //     (regression guard for timeline until-filter fix)
    // -----------------------------------------------------------------------
    let old_until = ArtifactTimeline
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "until": 1_000_000_i64,   // epoch ms far in the past (1970-01-01 +16 min)
                "limit": 100
            }),
        )
        .await
        .expect("timeline with old until should succeed");
    let old_events = old_until.as_array().expect("must be array");
    assert_eq!(
        old_events.len(),
        0,
        "until=very-old-timestamp must return 0 events (regression guard for #2)"
    );

    // -----------------------------------------------------------------------
    // 11. state_at(timestamp=now+1000) → freshness="fresh"
    //     (reviewed event is in window)
    // -----------------------------------------------------------------------
    let future_ts = chrono::Utc::now().timestamp_millis() + 1_000;
    let state_resp = ArtifactStateAt
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "timestamp": future_ts
            }),
        )
        .await
        .expect("state_at should succeed");
    assert_eq!(
        state_resp["freshness_at_as_of"].as_str().unwrap_or(""),
        "fresh",
        "state_at after reviewed event must return freshness_at_as_of=fresh"
    );
    assert!(
        !state_resp["freshness_now"].is_null(),
        "state_at must surface freshness_now alongside freshness_at_as_of"
    );
    assert!(
        state_resp["freshness_changed"].is_boolean(),
        "state_at must surface freshness_changed bool"
    );
    assert!(
        !state_resp["latest_event_at_as_of"].is_null(),
        "state_at must include latest_event_at_as_of object"
    );

    // -----------------------------------------------------------------------
    // 12. workspace_state_at(timestamp=now+1000) → artifacts[] contains tracker;
    //     freshness_at_as_of and freshness_now fields present.
    //     Tolerant of exact field name drift — just check presence.
    // -----------------------------------------------------------------------
    let ws_resp = WorkspaceStateAt
        .call(
            &ctx,
            json!({
                "timestamp": future_ts
            }),
        )
        .await
        .expect("workspace_state_at should succeed");
    let ws_artifacts = ws_resp["artifacts"]
        .as_array()
        .expect("workspace_state_at must return artifacts array");
    assert!(
        !ws_artifacts.is_empty(),
        "workspace_state_at must return at least one artifact"
    );
    let tracker_entry = ws_artifacts
        .iter()
        .find(|a| a["id"].as_str().unwrap_or("") == tracker_id)
        .expect("tracker artifact must appear in workspace_state_at response");
    // Verify freshness fields exist (tolerant of exact value; shape regression guard).
    assert!(
        tracker_entry.get("freshness_at_as_of").is_some(),
        "workspace_state_at artifact must have freshness_at_as_of field (shape drift: {tracker_entry})"
    );
    assert!(
        tracker_entry.get("freshness_now").is_some(),
        "workspace_state_at artifact must have freshness_now field (shape drift: {tracker_entry})"
    );

    // -----------------------------------------------------------------------
    // 13. artifact_get(id=tracker) → freshness="fresh", latest_event is object
    // -----------------------------------------------------------------------
    let get_resp = ArtifactGet
        .call(&ctx, json!({"id": tracker_id}))
        .await
        .expect("artifact_get should succeed");
    assert_eq!(
        get_resp["freshness"].as_str().unwrap_or(""),
        "fresh",
        "artifact_get must return freshness=fresh after reviewed event"
    );
    assert!(
        get_resp["latest_event"].is_object(),
        "artifact_get must return latest_event as object, got: {:?}",
        get_resp["latest_event"]
    );

    // -----------------------------------------------------------------------
    // 14. artifact_graph(include_events=true, depth=2) →
    //     nodes contains intent_id AND verdict_id;
    //     edges contains rel="resolves"
    // -----------------------------------------------------------------------
    let graph_resp = ArtifactGraph
        .call(
            &ctx,
            json!({
                "id": tracker_id,
                "depth": 2,
                "include_events": true
            }),
        )
        .await
        .expect("artifact_graph should succeed");
    let nodes = graph_resp["nodes"]
        .as_array()
        .expect("graph must have nodes");
    let edges = graph_resp["edges"]
        .as_array()
        .expect("graph must have edges");

    let node_ids: Vec<&str> = nodes.iter().filter_map(|n| n["id"].as_str()).collect();
    assert!(
        node_ids.contains(&intent_id.as_str()),
        "graph nodes must include intent event id={intent_id}"
    );
    assert!(
        node_ids.contains(&verdict_id.as_str()),
        "graph nodes must include verdict event id={verdict_id}"
    );

    let has_resolves_edge = edges
        .iter()
        .any(|e| e["rel"].as_str().unwrap_or("") == "resolves");
    assert!(
        has_resolves_edge,
        "graph edges must contain a rel=resolves edge"
    );

    // -----------------------------------------------------------------------
    // 15. artifact_graph WITHOUT include_events → no node has node_type="event"
    // -----------------------------------------------------------------------
    let graph_no_events = ArtifactGraph
        .call(
            &ctx,
            json!({
                "id": tracker_id,
                "depth": 2,
                "include_events": false
            }),
        )
        .await
        .expect("artifact_graph without events should succeed");
    let nodes_no_ev = graph_no_events["nodes"]
        .as_array()
        .expect("nodes must be array");
    let has_event_node = nodes_no_ev
        .iter()
        .any(|n| n["node_type"].as_str().unwrap_or("") == "event");
    assert!(
        !has_event_node,
        "graph without include_events must not contain event nodes"
    );

    // -----------------------------------------------------------------------
    // 16. artifact_event_create(artifact_id=tracker, kind="note", payload={text:"dual-write"}) →
    //     timeline kinds=["note"] now contains an event with text="dual-write"
    // -----------------------------------------------------------------------
    ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kind": "note",
                "payload": {"text": "dual-write"}
            }),
        )
        .await
        .expect("artifact_event_create note should succeed");

    let note_timeline = ArtifactTimeline
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kinds": ["note"],
                "limit": 50
            }),
        )
        .await
        .expect("note timeline should succeed");
    let note_events = note_timeline.as_array().expect("must be array");
    let has_dual_write_note = note_events
        .iter()
        .any(|e| e["payload"]["text"].as_str().unwrap_or("") == "dual-write");
    assert!(
        has_dual_write_note,
        "artifact_observe dual-write must appear in note timeline"
    );

    // -----------------------------------------------------------------------
    // 17. artifact_link(src=tracker, dst=spec, rel="supersedes") →
    //     also writes a superseded_by event;
    //     timeline kinds=["superseded_by"] non-empty
    // -----------------------------------------------------------------------
    ArtifactLink
        .call(
            &ctx,
            json!({
                "src_id": tracker_id,
                "dst_id": spec_id,
                "rel": "supersedes"
            }),
        )
        .await
        .expect("artifact_link supersedes should succeed");

    let superseded_timeline = ArtifactTimeline
        .call(
            &ctx,
            json!({
                "artifact_id": tracker_id,
                "kinds": ["superseded_by"],
                "limit": 10
            }),
        )
        .await
        .expect("superseded_by timeline should succeed");
    let sup_events = superseded_timeline.as_array().expect("must be array");
    assert!(
        !sup_events.is_empty(),
        "artifact_link rel=supersedes must dual-write a superseded_by event"
    );

    // -----------------------------------------------------------------------
    // 18. event_create(kind="superseded_by", target=spec) on a NEW artifact 'foo'
    //     → ALSO creates artifact_link rel=supersedes from foo→spec.
    //     Verify by querying artifact_links tool.
    // -----------------------------------------------------------------------
    let foo_id = create_artifact(&ctx, "foo.md", "tracker", "Foo Tracker").await;

    ArtifactEventCreate
        .call(
            &ctx,
            json!({
                "artifact_id": foo_id,
                "kind": "superseded_by",
                "payload": {"target_artifact_id": spec_id}
            }),
        )
        .await
        .expect("superseded_by event_create should succeed");

    // Check that the dual-write created a supersedes link from foo → spec.
    // (artifact_links was consolidated into artifact_get with include_links=true)
    let links_resp = ArtifactGet
        .call(
            &ctx,
            json!({
                "id": foo_id,
                "include_links": true,
                "links_direction": "out",
                "links_rel": "supersedes"
            }),
        )
        .await
        .expect("artifact_get with include_links should succeed");
    let outgoing = links_resp["links"]["outgoing"].as_array().expect("must be array");
    assert_eq!(
        outgoing.len(), 1,
        "event_create kind=superseded_by must dual-write a supersedes artifact_link from foo→spec"
    );
    assert_eq!(
        outgoing[0]["dst_id"].as_str().unwrap_or(""),
        spec_id,
        "supersedes link must point to spec_id"
    );
    assert_eq!(
        outgoing[0]["rel"].as_str().unwrap_or(""),
        "supersedes",
        "link rel must be supersedes"
    );
}
