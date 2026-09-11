use super::super::routes::DashboardState;
use crate::memory::MemoryStore;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

fn internal_error(context: &str, err: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    tracing::warn!(target: "dashboard", "{context}: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "internal" })),
    )
}

pub async fn list_memories(State(state): State<DashboardState>) -> Json<Value> {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(_) => return Json(json!({ "topics": [] })),
    };
    let topics = store.list().unwrap_or_default();
    Json(json!({ "topics": topics }))
}

pub async fn get_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (get)", e),
    };
    match store.read(&topic) {
        Ok(Some(content)) => (
            StatusCode::OK,
            Json(json!({ "topic": topic, "content": content })),
        ),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "Not found" }))),
        Err(e) => internal_error("memory read", e),
    }
}

/// The UI's write payload.
///
/// `force` exists because the shrink guard arrived here in the same change. A guard with
/// no escape would have converted "the UI can silently destroy a memory" into "the UI
/// cannot legitimately replace one", which is a different defect rather than a fix — the
/// MCP path has carried a `force` parameter for exactly this since the guard was written.
#[derive(Deserialize)]
pub struct WriteMemoryBody {
    pub content: String,
    #[serde(default)]
    pub force: bool,
}

/// Write a memory through the guarded path, not through the store.
///
/// This handler called `store.write` directly until 2026-09-11, skipping both the shrink
/// guard and the anchor re-stamp that the MCP write has always applied — so a UI edit
/// could truncate a memory with no refusal and leave it reported `stale` immediately
/// after being brought current. See `crate::memory::guarded` for why the two protections
/// live in one function rather than as two calls added here.
pub async fn write_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
    Json(body): Json<WriteMemoryBody>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (write)", e),
    };
    match crate::memory::guarded::guarded_write(
        &store,
        Some(&state.project_root),
        &topic,
        &body.content,
        body.force,
    ) {
        Ok(warnings) => (
            StatusCode::OK,
            Json(json!({ "ok": true, "warnings": warnings })),
        ),
        // 409, not 400: the request is well-formed and the refusal is about the state on
        // disk, which is what `force` overrides. A 400 would tell the UI to fix the
        // payload, and the payload is fine.
        //
        // `describe()` rather than a format! composed here — `shrink_guard`'s own header
        // records that "three copies is how the gap below survived", and a forked message
        // decays exactly the way a forked predicate does.
        Err(crate::memory::guarded::GuardedWriteError::Shrink(r)) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "shrink guard",
                "detail": format!(
                    "writing `{topic}` {}. Re-send with \"force\": true if that is intended.",
                    r.describe()
                ),
                "old_bytes": r.old_bytes,
                "new_bytes": r.new_bytes,
                "old_lines": r.old_lines,
                "new_lines": r.new_lines,
            })),
        ),
        Err(crate::memory::guarded::GuardedWriteError::Failed(e)) => {
            internal_error("memory write", e)
        }
    }
}

/// Delete a memory and its anchor sidecar.
///
/// This called `store.delete` directly until 2026-09-11, which left the `.anchors.toml`
/// sidecar orphaned — the MCP delete removes it precisely so a deleted topic stops
/// surfacing in staleness scans. Same mechanism as the write gap above: a second caller
/// added beside the guarded one.
pub async fn delete_memory(
    State(state): State<DashboardState>,
    Path(topic): Path<String>,
) -> (StatusCode, Json<Value>) {
    let store = match MemoryStore::open(&state.project_root) {
        Ok(s) => s,
        Err(e) => return internal_error("MemoryStore::open (delete)", e),
    };
    match crate::memory::guarded::guarded_delete(&store, &topic) {
        Ok(()) => (StatusCode::OK, Json(json!("ok"))),
        Err(e) => internal_error("memory delete", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::anchors::anchor_path_for_topic;

    /// These test the HANDLERS, not `memory::guarded`.
    ///
    /// That is the whole point: `guarded_write` being correct is not what was wrong. The
    /// defect was that this route did not call it, and a test of the helper passes
    /// identically whether the route reaches it or not — the same reason a guard nothing
    /// reaches is exactly as informative as no guard.
    fn state(root: &tempfile::TempDir) -> DashboardState {
        DashboardState {
            project_root: root.path().to_path_buf(),
        }
    }

    async fn write(
        st: &DashboardState,
        topic: &str,
        content: &str,
        force: bool,
    ) -> (StatusCode, Value) {
        let (code, Json(v)) = write_memory(
            State(st.clone()),
            Path(topic.to_string()),
            Json(WriteMemoryBody {
                content: content.to_string(),
                force,
            }),
        )
        .await;
        (code, v)
    }

    /// The regression. Before 2026-09-11 this returned 200 and the long content was gone.
    #[tokio::test]
    async fn a_shrinking_ui_edit_is_refused_and_the_old_content_survives() {
        let root = tempfile::tempdir().unwrap();
        let st = state(&root);
        // Long enough to clear SHRINK_GUARD_MIN_BYTES — under the floor any ratio is
        // noise and the guard declines to object, so a short fixture would pass this
        // test against the unfixed handler too.
        let long = "a long-standing memory line\n".repeat(80);
        assert_eq!(write(&st, "keepme", &long, false).await.0, StatusCode::OK);

        let (code, body) = write(&st, "keepme", "gone", false).await;
        assert_eq!(code, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"], "shrink guard", "{body}");

        let store = MemoryStore::open(root.path()).unwrap();
        assert_eq!(
            store.read("keepme").unwrap().as_deref(),
            Some(long.as_str()),
            "the refused write must not have landed"
        );
    }

    /// The escape has to exist, or the fix converts "the UI can destroy a memory" into
    /// "the UI cannot legitimately replace one".
    #[tokio::test]
    async fn force_true_applies_the_same_shrinking_edit() {
        let root = tempfile::tempdir().unwrap();
        let st = state(&root);
        let long = "a long-standing memory line\n".repeat(80);
        write(&st, "keepme", &long, false).await;

        let (code, body) = write(&st, "keepme", "gone", true).await;
        assert_eq!(code, StatusCode::OK, "{body}");

        let store = MemoryStore::open(root.path()).unwrap();
        assert_eq!(store.read("keepme").unwrap().as_deref(), Some("gone"));
    }

    /// A first write cannot shrink anything, so the guard must not block topic creation.
    #[tokio::test]
    async fn a_first_write_is_never_refused() {
        let root = tempfile::tempdir().unwrap();
        let (code, body) = write(&state(&root), "brand-new", "x", false).await;
        assert_eq!(code, StatusCode::OK, "{body}");
    }

    /// The second half of the same defect: the UI wrote content and left the topic
    /// reported `stale`, because nothing re-stamped its anchors.
    #[tokio::test]
    async fn a_ui_write_stamps_the_anchor_sidecar() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        // The anchor only seeds for a path that EXISTS — `seed_anchors` skips anything it
        // cannot stat. Creating this file is what makes the test discriminate; without it
        // the sidecar is legitimately absent and the assertion would be asserting the bug.
        std::fs::write(root.path().join("src/lib.rs"), "fn main() {}\n").unwrap();

        let st = state(&root);
        let (code, body) = write(&st, "arch", "the entry point is `src/lib.rs`\n", false).await;
        assert_eq!(code, StatusCode::OK, "{body}");

        let store = MemoryStore::open(root.path()).unwrap();
        let sidecar = anchor_path_for_topic(store.dir(), "arch");
        assert!(
            sidecar.exists(),
            "a UI write must stamp anchors, or the topic it just brought current keeps \
             reporting stale: {}",
            sidecar.display()
        );
    }

    /// The gap this bug's own § Resume asked about. The answer was yes.
    #[tokio::test]
    async fn a_ui_delete_removes_the_anchor_sidecar() {
        let root = tempfile::tempdir().unwrap();
        let st = state(&root);
        write(&st, "doomed", "content\n", false).await;

        let store = MemoryStore::open(root.path()).unwrap();
        let sidecar = anchor_path_for_topic(store.dir(), "doomed");
        // Placed directly rather than seeded: this asserts about DELETE, and making it
        // depend on the seeding heuristic would couple it to an unrelated mechanism.
        std::fs::write(&sidecar, "anchors = []\n").unwrap();

        let (code, _) = delete_memory(State(st.clone()), Path("doomed".to_string())).await;
        assert_eq!(code, StatusCode::OK);
        assert!(
            !sidecar.exists(),
            "an orphaned sidecar keeps surfacing in staleness scans for a topic that no \
             longer exists, with nothing to bring current"
        );
    }

    /// Most topics never grow a sidecar, so its absence must not fail the delete.
    #[tokio::test]
    async fn a_ui_delete_tolerates_a_missing_sidecar() {
        let root = tempfile::tempdir().unwrap();
        let st = state(&root);
        write(&st, "plain", "content\n", false).await;

        let (code, _) = delete_memory(State(st.clone()), Path("plain".to_string())).await;
        assert_eq!(code, StatusCode::OK);
        assert!(MemoryStore::open(root.path())
            .unwrap()
            .read("plain")
            .unwrap()
            .is_none());
    }
}
