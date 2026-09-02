use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Artifact;

#[async_trait]
impl Tool for Artifact {
    fn name(&self) -> &'static str {
        "doc"
    }

    fn description(&self) -> &'static str {
        "Document CRUD and query. \
         Defaults: scope=project (active project only), archived/superseded hidden when \
         filter does not constrain status. Shortcut params kind/status expand to eq-filters \
         and combine with filter via AND. \
         Trackers are artifacts with kind=tracker — augmented documents that auto-refresh their \
         body via a persistent prompt; call librarian(tracker_design) before creating one. \
         append_entry atomically assigns the next id for any monotonic-ID ledger and, WITH entry_collection, appends the row; WITHOUT it the ledger is prose (`## PREFIX-N` body sections) and the call reserves the id, writing nothing — \
         use it instead of a manual read-then-write for any monotonic-ID tracker (F-N, W-N, T-N, ...). \
         update_entry patches ONE existing entry in place; use it to change a row (e.g. flip a status) \
         instead of patch={params:...}, whose RFC 7396 array semantics replace the whole collection."
    }
    fn description_cap(&self) -> usize {
        1_800
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["find", "get", "create", "update", "move", "delete", "graft", "link", "graph", "state_at", "append_entry", "update_entry"],
                    "description": "Operation to perform"
                },
                "filter": {
                    "type": "object",
                    "description": "find: filter AST. Compose with {\"and\":[...]}, {\"or\":[...]}, {\"not\":{...}}. Leaf format: {\"field_name\": {\"op\": value}}, e.g. {\"rel_path\": {\"contains\": \"docs/trackers\"}}, {\"kind\": {\"eq\": \"spec\"}}, {\"tags\": {\"in\": [\"foo\",\"bar\"]}}. Ops: eq ne in nin gt lt gte lte contains prefix. contains on strings = LIKE '%v%' (works on title, rel_path, etc.); prefix = LIKE 'v%'. contains on tags/owners = array membership."
                },
                "kind": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on kind. create: artifact kind (spec/plan/adr/tracker/...)"
                },
                "status": {
                    "type": "string",
                    "description": "find: shortcut eq-filter on status (disables archived-hide). create/update: set status."
                },
                "topic": {
                    "type": "string",
                    "description": "create/update: semantic topic used by librarian(action=\"context\") grouping. NOT filterable via find."
                },
                "time_scope": {
                    "type": "string",
                    "description": "create/update: temporal scope tag written to frontmatter + catalog (e.g. '2026-W25', a date, or 'dated_snapshot'). Filterable via find."
                },
                "extra": {
                    "type": "object",
                    "description": "create/update: custom frontmatter keys (e.g. {\"origin_session_id\":\"abc\",\"branch\":\"x\"}). Written verbatim to YAML and round-trip-safe across updates; surfaced by get as `extra`. NOT catalog-indexed — NOT filterable via find. On update, each key is upserted; a null value deletes it; omitted keys are preserved."
                },
                "semantic": {
                    "type": "string",
                    "description": "find: natural-language query for semantic search (requires embedder). Hits are CHUNK-grain: each item carries `matched` (line range, enclosing entry token, bounded snippet), so a hit names the entry that matched, not the file's opening lines. One chunk per artifact; `hints.cap_suppressed` counts the rest."
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "find: scope for listing. Defaults to active project."
                },
                "augmented": {
                    "type": "boolean",
                    "description": "find: filter to augmented (true) or non-augmented (false) artifacts"
                },
                "include_archived": {
                    "type": "boolean",
                    "default": false,
                    "description": "find: include archived and superseded rows, which the default scope hides."
                },
                "limit": {
                    "type": "integer",
                    "default": 50,
                    "maximum": 500,
                    "description": "find: max rows (default 50, max 500)."
                },
                "offset": {
                    "type": "integer",
                    "default": 0,
                    "maximum": 100000,
                    "description": "find: rows to skip for paging (default 0)."
                },
                "id": {
                    "type": "string",
                    "description": "get/update/move/delete/graph/state_at/append_entry/update_entry: document id (16-hex). find and create take none."
                },
                "include_links": { "type": "boolean", "default": false, "description": "get: include link edges" },
                "links_direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "description": "get: filter links by direction (default: both)"
                },
                "links_rel": { "type": "string", "description": "get: filter links to this rel type" },
                "include_observations": {
                    "type": "boolean",
                    "default": false,
                    "description": "get: include observation rows recorded against the document (default false)."
                },
                "full": { "type": "boolean", "default": false, "description": "get: include full body" },
                "heading": { "type": "string", "description": "get: fetch one section by heading" },
                "occurrence": { "type": "integer", "minimum": 1, "description": "get: 1-indexed selector when `heading` matches several sections. Omitted, an ambiguous heading returns body_meta.heading_ambiguous naming each match's line." },
                "headings": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "get: fetch multiple sections by heading"
                },
                "entry_filter": {
                    "type": "object",
                    "description": "get: filter AST (same shape as find's `filter`) applied to the rows of the tracker's declared entry_collection; returns matching rows as `entries` + `entry_total`. Requires the artifact to be augmented with an entry_collection naming the params array to filter. e.g. {\"and\":[{\"status\":{\"eq\":\"open\"}}]}"
                },
                "start_line": { "type": "integer", "description": "get: 1-indexed start of line slice" },
                "end_line": { "type": "integer", "description": "get: 1-indexed inclusive end of line slice" },
                "new_rel_path": { "type": "string", "description": "move: destination path relative to repo root (e.g. 'docs/archive/foo.md'). Parent directories are created automatically. Fails if destination already exists. NOTE: a move MINTS A NEW ID (id = sha256(abs_path)); the artifact's events, links, observations and augmentation are grafted onto it and the old row is dropped. The response carries `id` (new), `previous_id`, `id_changed` and `history_grafted` — read the new id from there, re-point prose citing the old one, and never reuse a cached id across a move." },
                "rel_path": { "type": "string", "description": "create: relative path for new file, e.g. 'docs/plans/my-plan.md' — relative to repo root, and NOT including the repo name (use the `repo` field for that). Also accepted on find as a shorthand, where it is lifted to filter={\"rel_path\": {\"contains\": <value>}} and the lift is reported under `corrections`." },
                "repo": { "type": "string", "description": "create: workspace root name (git repo basename). Omit to infer from active project — rel_path is then treated as project-relative and the subdir prefix is prepended automatically." },
                "title": { "type": "string", "description": "create: artifact title. append_entry: the new entry's title — with `body` + `anchor_heading`, the server writes the section itself." },
                "body": { "type": "string", "description": "create: markdown body. append_entry: the new entry's section body — see `anchor_heading`." },
                "owners": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: owner list"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "create/update: tag list"
                },
                "augment": {
                    "type": "object",
                    "description": "create: attach the augmentation atomically, so a tracker needs no follow-up call. Fields below are artifact_augment's — see that tool for what each means. Unknown keys are REJECTED, not ignored — a typo here fails loudly rather than silently dropping the field.",
                    "properties": {
                        "prompt": { "type": "string" },
                        "params": { "type": "object" },
                        "render_template": { "type": "string" },
                        "params_schema": { "type": "object" },
                        "entry_collection": { "type": "string" },
                        "append_mode": { "type": "boolean" },
                        "history_cap": { "type": "integer" }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                },
                "patch": {
                    "type": "object",
                    "description": "update: the fields to change. Accepted keys: status, title, owners, tags, topic, time_scope, extra, body, body_edits, params (any other key returns RecoverableError). Top-level status/title/owners/tags/topic/time_scope/extra are lifted into patch automatically and reported under `corrections`; an update that changes nothing is refused. Body editing — three modes: (1) `body_edits: [{heading, action, content?|old_string+new_string?, at?, occurrence?, replace_all?, include_subsections?}]` for surgical per-section edits — edit_markdown's batch shape exactly, including its action semantics and occurrence rule; applied atomically, RECOMMENDED for tracker maintenance; (2) `body` for total overwrite, gated by the 50% shrink guard unless `force=true` is passed at top level; (3) frontmatter-only changes via status/title/owners/tags/topic/time_scope. `body` and `body_edits` are mutually exclusive. `params` is RFC 7396 merge-patched into the augmentation params — arrays are REPLACED whole, so use update_entry to change one row. Body mutations emit `field_patch` events (kind=field_patch, payload.field=body)."
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "update: bypass the body-shrink guard. Required when a body write would cut the file by >50% in bytes or lines. Use only when shrinkage is intentional (full rewrite, archiving stale sections). Default false. See get_guide(\"librarian\") § Body Editing Surfaces."
                },
                "commit_refresh": {
                    "type": "boolean",
                    "description": "update: atomically record a completed refresh cycle"
                },
                "src_id": { "type": "string", "description": "link: source artifact id" },
                "dst_id": { "type": "string", "description": "link: destination artifact id" },
                "from_id": { "type": "string", "description": "graft: id of the row whose history is folded in. This row is DELETED by the call — its events, links, observations and augmentation move to `into_id` first." },
                "into_id": { "type": "string", "description": "graft: id of the surviving row that absorbs `from_id`'s history. Both ids are REQUIRED; graft is refused if either is unknown or the two are equal." },
                "rel": { "type": "string", "description": "link: relation type (supersedes, implements, ...)" },
                "depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "description": "graph: BFS depth (1–3)"
                },
                "rels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "graph: filter edges to these rel types"
                },
                "include_events": {
                    "type": "boolean",
                    "default": false,
                    "description": "graph: also walk event and source nodes via event_edges"
                },
                "commit": { "type": "string", "description": "state_at: git commit hash as time-travel cutoff" },
                "timestamp": {
                    "type": "integer",
                    "format": "int64",
                    "description": "state_at: unix epoch ms as time-travel cutoff"
                },
                "entry_collection": {
                    "type": "string",
                    "description": "append_entry/update_entry: the augmentation's entry_collection array to write into (must match the artifact's declared entry_collection). OMIT it on append_entry for a PROSE ledger — one whose entries live as `## PREFIX-N` body sections rather than params rows: the call reserves the next id under the same transaction and returns it without writing anything, and you add the section, whose heading must be `## PREFIX-N — <title>` or the entry defines no citable token. Required for update_entry."
                },
                "entry_id": {
                    "type": "string",
                    "description": "update_entry: the id of the entry to patch (e.g. 'T-7'). Unknown ids are refused with the list of ids that do exist — never a silent no-op."
                },
                "fields": {
                    "type": "object",
                    "description": "update_entry: fields to set on that one entry, merged shallowly; a null value deletes the key. Every other entry, and every field this patch does not name, is left untouched. NOTE the asymmetry with append_entry, which takes `entry` (a whole new row) — this action takes `fields` (the subset to change); passing `entry` here is refused rather than silently ignored. An empty patch is refused too. `id` is rejected — entry ids key entry_cite rows, so re-keying one would strand its citations."
                },
                "id_prefix": {
                    "type": "string",
                    "description": "append_entry: id prefix — the assigned id is `<id_prefix>-<next integer>`, computed from the live max across both existing params entries and ids the markdown body already claims (headings / index rows), so a body that ran ahead of params cannot be reissued. Response carries a `warning` when params lags the body."
                },
                "entry": {
                    "type": "object",
                    "description": "append_entry: the new entry's fields, excluding `id` — the server assigns and overwrites `id`"
                },
                "cites": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "append_entry: optional write-time citations. Each ref is a 16-hex artifact id, a `<slug>:<local>` entry id, or a unique rel_path. Creates entry_cite edges from the new entry atomically; an unresolvable/ambiguous ref aborts the whole call. Not supported from a worktree checkout."
                },
                "anchor_heading": {
                    "type": "string",
                    "description": "append_entry: prose ledgers — pass with `title` + `body` (all three or none; a partial set is refused naming what is missing) and the server writes `## <ID> — <title>` itself, before this heading, in the same write that records the high-water mark. Must name a heading that exists verbatim; a bad anchor writes nothing at all. Why prefer it over reserving an id: get_guide(\"tracker-conventions\") § Entry ids."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
            RecoverableError::new(
                "action required — one of: find, get, create, update, move, graft, link, graph, state_at, append_entry, update_entry",
            )
        })?;
        // Best-effort: identity enrichment must never fail a tool call; a failed
        // stamp degrades the row to verb=NULL, which audit_log surfaces honestly.
        if let Err(e) = ctx.catalog.lock().set_audit_verb(&format!("doc.{action}")) {
            tracing::warn!("audit verb stamp failed: {e}");
        }
        match action {
            "find"     => super::find::call(ctx, args).await,
            "get"      => super::get::call(ctx, args).await,
            "create"   => super::create::call(ctx, args).await,
            "update"   => super::update::call(ctx, args).await,
            "move"     => super::mv::call(ctx, args).await,
            "delete"   => super::delete::call(ctx, args).await,
            "graft"    => super::graft::call(ctx, args).await,
            "link"     => super::link::call(ctx, args).await,
            "graph"    => super::graph::call(ctx, args).await,
            "state_at" => super::state_at::call(ctx, args).await,
            "append_entry" => super::append_entry::call(ctx, args).await,
            "update_entry" => super::update_entry::call(ctx, args).await,
            other => Err(RecoverableError::new(format!(
                "unknown action '{other}' — expected one of: find, get, create, update, move, delete, graft, link, graph, state_at, append_entry, update_entry"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = Artifact
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
    }

    #[tokio::test]
    async fn dispatch_stamps_the_audit_verb() {
        let ctx = mk_ctx();
        // find is read-only; the stamp happens at dispatch regardless of verb kind
        let _ = Artifact
            .call(&ctx, serde_json::json!({"action": "find"}))
            .await;
        let verb: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row("SELECT verb FROM audit_ctx", [], |r| r.get(0))
            .unwrap();
        assert_eq!(verb.as_deref(), Some("doc.find"));
    }

    #[tokio::test]
    async fn missing_action_returns_recoverable_error() {
        let err = Artifact
            .call(&mk_ctx(), serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "expected RecoverableError, got: {err}"
        );
    }

    #[tokio::test]
    async fn update_action_passes_through_dispatcher_without_unknown_field_error() {
        // Regression: deny_unknown_fields on update::Args used to reject the
        // outer dispatcher's `action` field, breaking every doc(update)
        // call through the Tool surface. Unit tests of update::call directly
        // missed this because they passed args without `action`. Going through
        // Artifact.call exercises the dispatcher pass-through.
        // See docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md.
        let err = Artifact
            .call(
                &mk_ctx(),
                serde_json::json!({
                    "action": "update",
                    "id": "nonexistent",
                    "patch": {"title": "X"},
                }),
            )
            .await
            .expect_err("update on nonexistent id should error");
        let msg = err.to_string();
        assert!(
            !msg.contains("unknown field `action`"),
            "outer dispatcher's `action` must pass through to update::call; got: {msg}"
        );
        assert!(
            msg.contains("unknown id") || msg.contains("nonexistent"),
            "expected unknown-id error after dispatcher passes; got: {msg}"
        );
    }

    #[tokio::test]
    async fn find_action_routes_correctly() {
        let v = Artifact
            .call(&mk_ctx(), serde_json::json!({"action": "find"}))
            .await
            .unwrap();
        assert!(v["count"].is_number());
    }

    #[test]
    fn input_schema_has_no_phantom_update_fields() {
        // owner/activeForm/addBlocks/addBlockedBy were copy-pasted from the
        // harness's unrelated TaskUpdate tool schema and never had any backing
        // implementation in update.rs — see
        // docs/issues/archive/2026-07-13-artifact-update-phantom-schema-fields.md.
        let schema = Artifact.input_schema();
        let props = &schema["properties"];
        for phantom in ["owner", "activeForm", "addBlocks", "addBlockedBy"] {
            assert!(
                props.get(phantom).is_none(),
                "schema documents `{phantom}` but update.rs has no field backing it"
            );
        }
    }

    /// Site 1 of 4. The rationale that used to live here — why two calls are compared rather
    /// than one asserted to fail, why `deny_unknown_fields` is unavailable (measured: adding
    /// it once broke every `doc(update)` call), and what the `accepts_any_json` escape
    /// admits — now lives on `crate::tools::param_probe`, shared with `librarian`,
    /// `artifact_event` and `artifact_refresh`.
    ///
    /// Required params are type-valid dummies chosen to **fail resolution**: a nonexistent
    /// id, an escaping `rel_path`. That failure is the point — it is reached *after*
    /// deserialisation, so a deserialisation error is visibly different from it. `create` in
    /// particular must not succeed, or its own second call would hit "already exists" and
    /// differ for the wrong reason.
    #[tokio::test]
    async fn every_action_labelled_schema_key_is_honored_by_that_action() {
        use crate::tools::param_probe::assert_all_honored;

        // 37 labelled keys across the 12 actions as of 2026-08-17. The floor leaves room for
        // the schema to shrink without a false alarm while still catching a break in the
        // `<action>:` label convention.
        assert_all_honored(
            "doc",
            &Artifact.input_schema(),
            &probe_spec(),
            30,
            |args| async move { Artifact.call(&mk_ctx(), args).await },
        )
        .await;
    }

    const PROBE_NO_SUCH_ID: &str = "0000000000000000";

    const PROBE_ACTIONS: [&str; 12] = [
        "find",
        "get",
        "create",
        "update",
        "move",
        "delete",
        "graft",
        "link",
        "graph",
        "state_at",
        "append_entry",
        "update_entry",
    ];

    /// The minimum type-valid args each action needs to get *past* deserialisation.
    ///
    /// **Every value here is chosen to fail resolution, and that is the load-bearing
    /// detail**: a nonexistent id, an escaping `rel_path`. The failure must be reached
    /// *after* deserialisation so `sweep` can tell it apart from a deser error. `create`
    /// in particular must not succeed, or its second call would hit "already exists" and
    /// differ for the wrong reason. Swap any of these for a value that resolves and the
    /// probe keeps passing while comparing the wrong two outcomes.
    ///
    /// This table is read by two tests pulling in opposite directions —
    /// `every_action_labelled_schema_key_is_honored_by_that_action` (schema→action) and
    /// `every_required_param_is_advertised` (action→schema). It is deliberately the
    /// single copy: it was previously inlined in the forward test, where it recorded
    /// `graft`'s `from_id`/`into_id` while the schema advertised neither, and supplying
    /// them out-of-band is exactly what let that defect pass.
    fn probe_required(action: &str) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        match action {
            "get" | "graph" | "delete" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
            }
            "update" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("patch".into(), json!({}));
            }
            "move" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("new_rel_path".into(), json!("docs/nope.md"));
            }
            "graft" => {
                m.insert("from_id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("into_id".into(), json!("1111111111111111"));
            }
            "append_entry" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("id_prefix".into(), json!("ZZ"));
            }
            "update_entry" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("entry_collection".into(), json!("nope"));
                m.insert("entry_id".into(), json!("ZZ-1"));
                m.insert("fields".into(), json!({}));
            }
            "link" => {
                m.insert("src_id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("dst_id".into(), json!("1111111111111111"));
                m.insert("rel".into(), json!("cites"));
            }
            "state_at" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
            }
            "create" => {
                m.insert("kind".into(), json!("bug"));
                m.insert("title".into(), json!("probe"));
                // Escaping path: refused before anything is written, so the baseline is
                // stable and no file is created.
                m.insert("rel_path".into(), json!("../probe-must-not-exist.md"));
            }
            _ => {}
        }
        m
    }

    fn probe_spec() -> crate::tools::param_probe::Spec<'static> {
        crate::tools::param_probe::Spec {
            actions: &PROBE_ACTIONS,
            accepts_any_json: &[],
            required: probe_required,
        }
    }

    /// Site 1 of 4, reverse direction. See
    /// `crate::tools::param_probe::assert_required_are_advertised`.
    ///
    /// Written before the fix it demanded, and red on first run: `graft` required
    /// `from_id` and `into_id`, and `artifact`'s schema advertised neither, so the action
    /// could not be called as advertised. That red is the deliberate break — earned from
    /// a real defect rather than staged by mutating a passing test.
    #[tokio::test]
    async fn every_required_param_is_advertised() {
        use crate::tools::param_probe::assert_required_are_advertised;

        assert_required_are_advertised("doc", &Artifact.input_schema(), &probe_spec());
    }

    /// The doc half of
    /// `docs/issues/archive/2026-08-17-find-silently-drops-top-level-rel-path.md`, and the part
    /// that actually caused the wrong call.
    ///
    /// `rel_path`'s description opened `create: relative path for new file` and then
    /// spent two more sentences instructing `find` callers — "In find results…", "When
    /// filtering by path use contains/prefix…" — with an example in the *inverted* leaf
    /// shape that `repair_node` exists to correct. An agent looking for how to find by
    /// path found `rel_path`: top-level, and described in find terms. `find::Args` had
    /// no such field, so serde discarded it.
    ///
    /// The invariant asserted is not "never mention another action" — mentioning `find`
    /// is now correct, because `find` honors the key. It is that the mention and the
    /// support must agree. Red before the fix on both halves: the description named
    /// `find` while `find` dropped the param, and it taught the inverted shape.
    #[tokio::test]
    async fn rel_path_description_and_find_support_agree() {
        let schema = Artifact.input_schema();
        let desc = schema["properties"]["rel_path"]["description"]
            .as_str()
            .expect("rel_path is documented");

        // The discriminator is the `"field"` KEY, not the op name. An earlier version
        // looked for `{"contains"` and matched the canonical
        // `{"rel_path": {"contains": …}}` too — the inverted shape is the one that names
        // its field inside the op object, so that key is what identifies it.
        assert!(
            !desc.contains("\"field\""),
            "rel_path's description carries an inverted filter-leaf example \
             ({{op: {{field, value}}}}), teaching the shape repair_node exists to \
             correct: {desc}"
        );

        if desc.contains("find") {
            // Same serde probe as `schema_keys_labelled_find_are_honored_by_find`: a
            // real field type-checks and rejects `[]`; a missing one is discarded and
            // the call succeeds.
            let ctx = mk_ctx();
            let probe = Artifact
                .call(&ctx, json!({"action": "find", "rel_path": []}))
                .await;
            assert!(
                probe.is_err(),
                "rel_path's description tells find callers about it, but find::Args has \
                 no such field — so the param is silently discarded and the query runs \
                 at defaults. Either honor it on find or stop documenting it there."
            );
        }
    }
}
