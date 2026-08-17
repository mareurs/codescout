use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Artifact;

#[async_trait]
impl Tool for Artifact {
    fn name(&self) -> &'static str {
        "artifact"
    }

    fn description(&self) -> &'static str {
        "Artifact CRUD and query. action: find | get | create | update | move | delete | graft | link | graph | state_at | append_entry | update_entry. \
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
                    "description": "find: natural-language query for semantic search (requires embedder)"
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
                "include_archived": { "type": "boolean", "default": false },
                "limit": { "type": "integer", "default": 50, "maximum": 500 },
                "offset": { "type": "integer", "default": 0, "maximum": 100000 },
                "id": {
                    "type": "string",
                    "description": "get/update/graph/append_entry: artifact id"
                },
                "include_links": { "type": "boolean", "default": false, "description": "get: include link edges" },
                "links_direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "description": "get: filter links by direction (default: both)"
                },
                "links_rel": { "type": "string", "description": "get: filter links to this rel type" },
                "include_observations": { "type": "boolean", "default": false },
                "full": { "type": "boolean", "default": false, "description": "get: include full body" },
                "heading": { "type": "string", "description": "get: fetch one section by heading" },
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
                "title": { "type": "string", "description": "create: artifact title" },
                "body": { "type": "string", "description": "create: markdown body" },
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
                    "description": "create: attach the augmentation atomically. Accepts every caller-controlled augmentation field, so a tracker needs no follow-up artifact_augment call. Unknown keys are REJECTED, not ignored — a typo here fails loudly rather than silently dropping the field.",
                    "properties": {
                        "prompt": { "type": "string", "description": "Required. Persistent instruction: what to maintain and how to format it." },
                        "params": { "type": "object", "description": "Initial params payload." },
                        "render_template": { "type": "string", "description": "MiniJinja template projecting params into the librarian(context) [LIVE] block. Omit and the tracker contributes no live state there." },
                        "params_schema": { "type": "object", "description": "JSON Schema validating params on every merge." },
                        "entry_collection": { "type": "string", "description": "Names the params array holding filterable entry rows; enables artifact(get, entry_filter=...)." },
                        "append_mode": { "type": "boolean", "description": "artifact_update prepends a new dated section instead of replacing the body." },
                        "history_cap": { "type": "integer", "description": "Max dated sections retained; oldest dropped beyond the cap." }
                    },
                    "required": ["prompt"],
                    "additionalProperties": false
                },
                "patch": {
                    "type": "object",
                    "description": "REQUIRED for action='update' — an update with no `patch` fails with the bare serde message `missing field 'patch'`, which names the field but not the action that wanted it. Fields to change. Accepted keys: status, title, owners, tags, topic, time_scope, extra, body, body_edits, params (any other key returns RecoverableError). Body editing — three modes: (1) `body_edits: [{heading, action, content?|old_string+new_string?, at?, replace_all?, include_subsections?}]` for surgical per-section edits (mirrors edit_markdown's batch shape, applied atomically, RECOMMENDED for tracker maintenance) — action is one of replace|insert_before|insert_after|remove|edit: use action='edit' for a scoped text swap (heading + old_string + new_string), action='replace' to overwrite an entire section body (heading + content); (2) `body` for total overwrite, gated by the 50% shrink guard unless `force=true` is passed at top level; (3) frontmatter-only changes via status/title/owners/tags/topic/time_scope. `body` and `body_edits` are mutually exclusive. `params` is RFC 7396 merge-patched into the augmentation params — use null values to delete keys. Body mutations emit `field_patch` events (kind=field_patch, payload.field=body)."
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "update: bypass the body-shrink guard. Required when a body write would reduce the file by more than 50%. Use only when shrinkage is intentional (full rewrite, archiving stale sections). Default false. See get_guide(\"librarian\") § Body Editing Surfaces."
                },
                "commit_refresh": {
                    "type": "boolean",
                    "description": "update: atomically record a completed refresh cycle"
                },
                "src_id": { "type": "string", "description": "link: source artifact id" },
                "dst_id": { "type": "string", "description": "link: destination artifact id" },
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
                "artifact_id": { "type": "string", "description": "state_at: artifact id" },
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
        // outer dispatcher's `action` field, breaking every artifact(update)
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

    /// Class-level guard for the family
    /// `docs/issues/2026-08-17-find-silently-drops-top-level-rel-path.md` belongs to.
    ///
    /// **This one passes today.** It is a tripwire for the next variant, not a
    /// reproduction of this one — `rel_path` is labelled `create:`, so a label-driven
    /// sweep cannot reach it by construction, and the doc half is pinned separately by
    /// `rel_path_description_does_not_instruct_find_callers`. Mutation-verified: change
    /// any `find:`-labelled key's name in `input_schema()` without adding the field to
    /// `find::Args` and this goes red.
    ///
    /// `input_schema_has_no_phantom_update_fields` asserts a key is backed by *some*
    /// action. That is the gap this closes: a key can be real, backed by a sibling
    /// action, and silently discarded by the one whose label it carries — because
    /// `Args` cannot carry `deny_unknown_fields` (the dispatcher passes `action` down,
    /// and adding it once broke every `artifact(update)` call).
    ///
    /// The probe exploits serde's asymmetry: a key that IS a field gets type-checked,
    /// so an ill-typed value errors; a key that is NOT a field is silently discarded,
    /// so the same value succeeds. `[]` is invalid for every type in `find::Args`
    /// (`Option<String>`, `Option<bool>`, `usize`, `Option<Scope>`, `Option<FilterNode>`)
    /// — check that before reusing the probe on an action whose `Args` holds a `Vec`,
    /// where `[]` would be accepted and the probe would read as a pass.
    #[tokio::test]
    async fn schema_keys_labelled_find_are_honored_by_find() {
        let schema = Artifact.input_schema();
        let props = schema["properties"]
            .as_object()
            .expect("schema has properties");

        let find_keys: Vec<String> = props
            .iter()
            .filter(|(name, spec)| {
                // `action` is the dispatcher's own key, not any sub-tool's.
                *name != "action"
                    && spec["description"]
                        .as_str()
                        .is_some_and(|d| d.starts_with("find:"))
            })
            .map(|(name, _)| name.clone())
            .collect();

        assert!(
            !find_keys.is_empty(),
            "expected some find-labelled schema keys; the label convention may have changed"
        );

        let ctx = mk_ctx();
        for key in &find_keys {
            let result = Artifact
                .call(&ctx, json!({"action": "find", key.as_str(): []}))
                .await;
            assert!(
                result.is_err(),
                "schema labels `{key}` as a find param, but find::Args has no such \
                 field — serde discards it and the query runs at defaults, returning \
                 an unfiltered first page whose count reads as a match total"
            );
        }
    }

    /// The doc half of
    /// `docs/issues/2026-08-17-find-silently-drops-top-level-rel-path.md`, and the part
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
