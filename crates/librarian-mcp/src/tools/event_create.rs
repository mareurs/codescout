use anyhow::Result;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::catalog::{event_edges, events, sources};

fn any_value_schema(_g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({})
}

fn source_schema(_g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": ["object", "null"],
        "properties": {
            "uri": {"type": "string"},
            "kind": {"type": "string"},
            "payload": {}
        },
        "required": ["uri", "kind"]
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Args {
    pub artifact_id: String,
    pub kind: String,
    #[schemars(schema_with = "any_value_schema")]
    pub payload: Value,
    #[serde(default)]
    pub anchor_commit: Option<String>,
    #[serde(default)]
    pub head_commit: Option<String>,
    #[serde(default)]
    pub parent_event_id: Option<String>,
    #[serde(default)]
    pub also_mutates: Option<Vec<String>>,
    #[serde(default)]
    pub resolves_intent_event_id: Option<String>,
    #[serde(default)]
    #[schemars(schema_with = "source_schema")]
    pub source: Option<SourceArg>,
    #[serde(default)]
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SourceArg {
    pub uri: String,
    pub kind: String,
    #[serde(default)]
    #[schemars(schema_with = "any_value_schema")]
    pub payload: Option<Value>,
}

const ALLOWED_KINDS: &[&str] = &[
    "note",
    "reviewed",
    "status_change",
    "field_patch",
    "superseded_by",
    "external_signal",
    "intent",
    "verdict",
];

/// Per-artifact write lock registry.
///
/// `event_create::call` acquires a per-artifact-id lock that spans the
/// frontmatter mutation, parent-event lookup, and the row + edges
/// transaction. Two concurrent calls on the same artifact serialise
/// (preventing interleaved frontmatter writes and ensuring the
/// parent_event_id chain forms a line, not a fan). Calls on different
/// artifacts do not contend.
///
/// Memory grows linearly with the number of distinct artifact ids ever
/// seen in this process. Acceptable at v1 catalog sizes; revisit if the
/// catalog grows past tens of thousands of artifacts in long-lived
/// servers.
#[derive(Default)]
struct WriteLockRegistry {
    inner: parking_lot::Mutex<
        std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
    >,
}

impl WriteLockRegistry {
    fn lock_for(&self, artifact_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        let mut g = self.inner.lock();
        g.entry(artifact_id.to_string())
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}

static WRITE_LOCKS: std::sync::OnceLock<WriteLockRegistry> = std::sync::OnceLock::new();

fn write_locks() -> &'static WriteLockRegistry {
    WRITE_LOCKS.get_or_init(WriteLockRegistry::default)
}

fn validate_payload(kind: &str, p: &Value) -> Result<()> {
    let obj = p
        .as_object()
        .ok_or_else(|| RecoverableError::new("payload must be object"))?;
    match kind {
        "note" => {
            obj.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("note.text required"))?;
        }
        "reviewed" => { /* both fields optional */ }
        "status_change" => {
            obj.get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("status_change.to required"))?;
        }
        "field_patch" => {
            obj.get("field")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("field_patch.field required"))?;
            obj.get("to")
                .ok_or_else(|| RecoverableError::new("field_patch.to required"))?;
        }
        "superseded_by" => {
            obj.get("target_artifact_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    RecoverableError::new("superseded_by.target_artifact_id required")
                })?;
        }
        "external_signal" => {
            obj.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("external_signal.source_id required"))?;
            obj.get("summary")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("external_signal.summary required"))?;
        }
        "intent" => {
            obj.get("hypothesis")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("intent.hypothesis required"))?;
        }
        "verdict" => {
            let outcome = obj
                .get("outcome")
                .and_then(|v| v.as_str())
                .ok_or_else(|| RecoverableError::new("verdict.outcome required"))?;
            if !matches!(outcome, "confirmed" | "refuted" | "partial" | "abandoned") {
                return Err(RecoverableError::new(
                    "verdict.outcome must be confirmed|refuted|partial|abandoned",
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn apply_payload_to_frontmatter(
    ctx: &ToolContext,
    artifact_id: &str,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    match kind {
        "status_change" => {
            let to = payload.get("to").and_then(|v| v.as_str()).unwrap(); // already validated above
            crate::tools::update::write_field_to_frontmatter(
                ctx,
                artifact_id,
                "status",
                &Value::String(to.into()),
            )?;
        }
        "field_patch" => {
            let field = payload.get("field").and_then(|v| v.as_str()).unwrap(); // already validated above
            let to = payload.get("to").unwrap(); // already validated above
            crate::tools::update::write_field_to_frontmatter(ctx, artifact_id, field, to)?;
        }
        _ => {}
    }
    Ok(())
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;

    if !ALLOWED_KINDS.contains(&a.kind.as_str()) {
        return Err(RecoverableError::with_hint(
            format!("unknown event kind: {}", a.kind),
            format!("allowed: {}", ALLOWED_KINDS.join(", ")),
        ));
    }
    validate_payload(&a.kind, &a.payload)?;

    let _write_guard = write_locks().lock_for(&a.artifact_id).lock_owned().await;

    if let Some(ref target) = a.resolves_intent_event_id {
        if a.kind != "verdict" {
            return Err(RecoverableError::new(
                "resolves_intent_event_id only valid on verdict events",
            ));
        }
        let cat = ctx.catalog.lock();
        let target_kind: Option<String> = cat
            .conn
            .query_row(
                "SELECT kind FROM events WHERE id=?1",
                rusqlite::params![target],
                |r| r.get(0),
            )
            .ok();
        match target_kind.as_deref() {
            Some("intent") => {}
            Some(k) => {
                return Err(RecoverableError::new(format!(
                    "target event {target} is kind={k}, not intent"
                )))
            }
            None => {
                return Err(RecoverableError::new(format!(
                    "target event {target} not found"
                )))
            }
        }
        if !event_edges::incoming_by_rel(&cat, target, "resolves")?.is_empty() {
            return Err(RecoverableError::new(format!(
                "intent {target} already resolved"
            )));
        }
    }

    let now = chrono::Utc::now().timestamp_millis();
    let id = ulid::Ulid::new().to_string();

    let parent_id = match &a.parent_event_id {
        Some(p) => Some(p.clone()),
        None => {
            let cat = ctx.catalog.lock();
            events::latest_for_artifact(&cat, &a.artifact_id)?.map(|e| e.id)
        }
    };

    if a.kind == "status_change" || a.kind == "field_patch" {
        apply_payload_to_frontmatter(ctx, &a.artifact_id, &a.kind, &a.payload)?;
    }

    let payload_str = serde_json::to_string(&a.payload)?;
    let event_row = events::EventRow {
        id: id.clone(),
        artifact_id: a.artifact_id.clone(),
        kind: a.kind.clone(),
        payload: payload_str,
        anchor_commit: a.anchor_commit.clone(),
        head_commit: a.head_commit.clone(),
        author: a.author.clone(),
        created_at: now,
    };

    let supersedes_link = if a.kind == "superseded_by" {
        let target_id = a
            .payload
            .get("target_artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RecoverableError::new("superseded_by.target_artifact_id required"))?;
        Some(crate::catalog::links::LinkRow {
            src_id: a.artifact_id.clone(),
            dst_id: target_id.into(),
            rel: "supersedes".into(),
            created_at: now,
        })
    } else {
        None
    };

    let mut edges: Vec<event_edges::EdgeRow> = Vec::new();

    if let Some(p) = parent_id.clone() {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: Some(p),
            dst_artifact_id: None,
            dst_source_id: None,
            rel: "parent".into(),
        });
    }

    let source_row = a.source.as_ref().map(|s| {
        let src_id = format!("{}:{}", s.kind, s.uri);
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: None,
            dst_artifact_id: None,
            dst_source_id: Some(src_id.clone()),
            rel: "triggered_by".into(),
        });
        sources::SourceRow {
            id: src_id,
            uri: s.uri.clone(),
            kind: s.kind.clone(),
            payload: s.payload.as_ref().map(|p| p.to_string()),
            ingested_at: now,
        }
    });

    for art in a.also_mutates.unwrap_or_default() {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: None,
            dst_artifact_id: Some(art),
            dst_source_id: None,
            rel: "mutates".into(),
        });
    }

    if let Some(target) = a.resolves_intent_event_id {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: Some(target),
            dst_artifact_id: None,
            dst_source_id: None,
            rel: "resolves".into(),
        });
    }

    {
        let cat = ctx.catalog.lock();
        let tx = cat.conn.unchecked_transaction()?;
        if let Some(s) = &source_row {
            sources::upsert_with(&tx, s)?;
        }
        if let Some(l) = &supersedes_link {
            crate::catalog::links::insert_with(&tx, l)?;
        }
        events::insert_with(&tx, &event_row)?;
        #[cfg(test)]
        if tests::INJECT_FAIL_AFTER_EVENT_INSERT.with(|c| {
            let v = c.get();
            c.set(false);
            v
        }) {
            anyhow::bail!("test injection: forced failure after event insert");
        }
        event_edges::insert_many_in_tx(&tx, &edges)?;
        tx.commit()?;

        if a.kind == "note" {
            if let Some(text) = a.payload.get("text").and_then(|v| v.as_str()) {
                let obs = crate::catalog::observations::ObservationRow {
                    id: None,
                    artifact_id: a.artifact_id.clone(),
                    text: text.to_string(),
                    source: a.author.clone(),
                    created_at: now,
                };
                let _ = crate::catalog::observations::insert(&cat, &obs);
            }
        }
    }

    Ok(json!({
        "event_id": id,
        "parent_event_id": parent_id,
        "anchor_commit": a.anchor_commit,
        "head_commit": a.head_commit,
    }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_insert, ArtifactRow};
    use crate::tools::ToolContext;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;
    use tempfile::TempDir;

    pub(crate) fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        use crate::workspace::Root;
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(
                crate::catalog::Catalog::open_in_memory().unwrap(),
            )),
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

    thread_local! {
        /// When set true, `call()` aborts with an error after inserting the
        /// event row but before inserting edges. Used by the transaction-rollback
        /// test below to verify atomicity. Thread-local so parallel tests
        /// running on different threads do not race on a shared flag.
        pub(super) static INJECT_FAIL_AFTER_EVENT_INSERT: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
    }

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[tokio::test]
    async fn note_event_round_trip() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a1")).unwrap();
        }
        let result = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "note",
                "payload": {"text": "hi"}
            }),
        )
        .await
        .unwrap();
        let event_id = result["event_id"].as_str().unwrap();
        assert!(!event_id.is_empty());
    }

    #[tokio::test]
    async fn note_event_also_writes_observation_row() {
        use crate::catalog::observations;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("obs-art")).unwrap();
        }

        call(
            &ctx,
            json!({
                "artifact_id": "obs-art",
                "kind": "note",
                "payload": {"text": "hello observation"}
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let obs = observations::list_for_artifact(&cat, "obs-art").unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].text, "hello observation");
    }

    #[tokio::test]
    async fn rejects_unknown_kind() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "bogus",
                "payload": {}
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("unknown event kind"));
    }

    #[tokio::test]
    async fn verdict_resolves_intent_emits_edge() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a2")).unwrap();
        }

        // Write intent event
        let intent_result = call(
            &ctx,
            json!({
                "artifact_id": "a2",
                "kind": "intent",
                "payload": {"hypothesis": "X causes Y"}
            }),
        )
        .await
        .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // Write verdict resolving the intent
        let verdict_result = call(
            &ctx,
            json!({
                "artifact_id": "a2",
                "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap();
        let verdict_id = verdict_result["event_id"].as_str().unwrap().to_string();

        let cat = ctx.catalog.lock();
        let edges = event_edges::outgoing(&cat, &verdict_id).unwrap();
        let resolves_edge = edges.iter().find(|e| e.rel == "resolves");
        assert!(resolves_edge.is_some());
        assert_eq!(
            resolves_edge.unwrap().dst_event_id.as_deref(),
            Some(intent_id.as_str())
        );
    }

    #[tokio::test]
    async fn cannot_resolve_intent_twice() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a3")).unwrap();
        }

        // Write intent event
        let intent_result = call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "intent",
                "payload": {"hypothesis": "P implies Q"}
            }),
        )
        .await
        .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // First verdict — should succeed
        call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "verdict",
                "payload": {"outcome": "refuted"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap();

        // Second verdict against the same intent — should fail
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("already resolved"));
    }

    #[tokio::test]
    async fn event_create_superseded_by_creates_link() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("src-art")).unwrap();
            art_insert(&cat, &art("dst-art")).unwrap();
        }

        call(
            &ctx,
            serde_json::json!({
                "artifact_id": "src-art",
                "kind": "superseded_by",
                "payload": {"target_artifact_id": "dst-art"}
            }),
        )
        .await
        .unwrap();

        // Expect an artifact_link with rel=supersedes from src-art → dst-art.
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_link WHERE src_id='src-art' AND dst_id='dst-art' AND rel='supersedes'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "superseded_by event must create an artifact_link rel=supersedes"
        );
    }

    #[tokio::test]
    async fn unknown_kind_is_recoverable_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "bogus",
                "payload": {}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err:#}"
        );
    }

    #[tokio::test]
    async fn rollback_on_failure_after_event_insert_leaves_no_orphan_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a-tx")).unwrap();
        }
        INJECT_FAIL_AFTER_EVENT_INSERT.with(|c| c.set(true));
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a-tx",
                "kind": "note",
                "payload": {"text": "should roll back"}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("test injection"),
            "expected injected failure, got: {err:#}"
        );
        // Event row must be rolled back — count must be zero for this artifact.
        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='a-tx'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "expected event-row rollback after failure between event insert and edges insert"
        );
        // No edges either.
        let edge_count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM event_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edge_count, 0);
    }

    #[tokio::test]
    async fn intent_inputs_payload_passthrough() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a-inp")).unwrap();
        }
        let inputs = json!([
            {"kind": "doc", "ref": "spec/foo.md"},
            {"kind": "issue", "ref": "linear://abc-123"}
        ]);
        let result = call(
            &ctx,
            json!({
                "artifact_id": "a-inp",
                "kind": "intent",
                "payload": {"hypothesis": "X works", "inputs": inputs.clone()}
            }),
        )
        .await
        .unwrap();
        let event_id = result["event_id"].as_str().unwrap().to_string();

        let cat = ctx.catalog.lock();
        let payload_str: String = cat
            .conn
            .query_row(
                "SELECT payload FROM events WHERE id=?1",
                rusqlite::params![event_id],
                |r| r.get(0),
            )
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
        assert_eq!(
            payload["inputs"], inputs,
            "intent.inputs must round-trip unchanged through events.payload"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_calls_on_same_artifact_chain_not_fan() {
        let tmp = TempDir::new().unwrap();
        let ctx = std::sync::Arc::new(mk_ctx(tmp.path().to_path_buf()));
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("conc")).unwrap();
        }
        let c1 = ctx.clone();
        let c2 = ctx.clone();
        let h1 = tokio::spawn(async move {
            call(
                &c1,
                json!({
                    "artifact_id": "conc",
                    "kind": "note",
                    "payload": {"text": "first"}
                }),
            )
            .await
            .unwrap()
        });
        let h2 = tokio::spawn(async move {
            call(
                &c2,
                json!({
                    "artifact_id": "conc",
                    "kind": "note",
                    "payload": {"text": "second"}
                }),
            )
            .await
            .unwrap()
        });
        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();

        let id1 = r1["event_id"].as_str().unwrap().to_string();
        let id2 = r2["event_id"].as_str().unwrap().to_string();
        let p1 = r1["parent_event_id"].as_str().map(|s| s.to_string());
        let p2 = r2["parent_event_id"].as_str().map(|s| s.to_string());

        // Exactly one event must have no parent (the lock-winner) and the
        // other must point to it. Without the lock, both would race
        // `latest_for_artifact` and both would return parent_event_id=None.
        match (p1.as_deref(), p2.as_deref()) {
            (None, Some(p)) => assert_eq!(p, id1, "second event must chain off first"),
            (Some(p), None) => assert_eq!(p, id2, "second event must chain off first"),
            other => panic!(
                "expected one None and one Some(other_id); got {other:?} (id1={id1}, id2={id2})"
            ),
        }

        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='conc'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn field_patch_unwritable_field_errors_and_writes_no_event() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        // Write a real artifact file on disk so frontmatter ops have a target.
        let rel_path = "a-fp.md";
        std::fs::write(
            tmp.path().join(rel_path),
            "---\nid: a-fp\nrepo: r\nrel_path: a-fp.md\nkind: spec\nstatus: active\n---\n# body\n",
        )
        .unwrap();
        {
            let cat = ctx.catalog.lock();
            art_insert(
                &cat,
                &ArtifactRow {
                    rel_path: rel_path.into(),
                    ..art("a-fp")
                },
            )
            .unwrap();
        }
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a-fp",
                "kind": "field_patch",
                "payload": {"field": "owners", "to": ["alice"]}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err:#}"
        );
        assert!(
            err.to_string().contains("not writable"),
            "expected 'not writable' in error, got: {err:#}"
        );
        // No event row may be written when the disk write would have been a no-op.
        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='a-fp'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "field_patch on unwritable field must not insert an event row"
        );
    }
}
