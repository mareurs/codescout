use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

/// `event_create` arrives as `{id, event: {kind, payload, …}}` so the event kind never
/// shares a key with the document `kind`. `event_create::Args` is flat and reads
/// `artifact_id`; lift the object and carry the id under that name.
fn flatten_event_args(args: &Value) -> Result<Value> {
    let id = args["id"].as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            "doc(action=\"event_create\") requires 'id'",
            "e.g. doc(action=\"event_create\", id=\"<16-hex>\", event={kind: \"note\", payload: {text: \"…\"}})",
        )
    })?;
    let mut flat = match args.get("event") {
        Some(Value::Object(m)) => m.clone(),
        _ => {
            return Err(RecoverableError::with_hint(
                "doc(action=\"event_create\") requires an `event` object",
                "event={kind: <note|reviewed|status_change|field_patch|superseded_by|external_signal|intent|verdict>, payload: {…}}",
            ))
        }
    };
    flat.insert("artifact_id".into(), json!(id));
    Ok(Value::Object(flat))
}

/// `augment` arrives as `{id, merge?, augment: {prompt, params, …}}` so the
/// augmentation's own fields never collide with the document's top-level
/// fields (e.g. both would otherwise fight over `params`). `augment::Args` is
/// flat and reads `id` + `merge` alongside the augmentation fields; lift the
/// nested object and carry `id`/`merge` into it.
fn flatten_augment_args(args: &Value) -> Result<Value> {
    let id = args["id"].as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            "doc(action=\"augment\") requires 'id'",
            "e.g. doc(action=\"augment\", id=\"<16-hex>\", augment={prompt: \"…\"})",
        )
    })?;
    let mut flat = match args.get("augment") {
        Some(Value::Object(m)) => m.clone(),
        _ => {
            return Err(RecoverableError::with_hint(
                "doc(action=\"augment\") requires an `augment` object",
                "augment={prompt: \"…\", params: {…}, …}",
            ))
        }
    };
    flat.insert("id".into(), json!(id));
    if let Some(merge) = args.get("merge") {
        flat.insert("merge".into(), merge.clone());
    }
    Ok(Value::Object(flat))
}

/// `event_list` says `id`; `timeline::Args` reads `artifact_id`. Copy, don't rename the
/// module's field — internals keep their names.
fn id_as_artifact_id(args: &Value) -> Value {
    let mut a = args.clone();
    if let (Some(id), Some(obj)) = (args.get("id").cloned(), a.as_object_mut()) {
        obj.entry("artifact_id").or_insert(id);
    }
    a
}

pub struct Artifact;

#[async_trait]
impl Tool for Artifact {
    fn name(&self) -> &'static str {
        "doc"
    }

    fn description(&self) -> &'static str {
        "Document catalog: find/get/create/update/move/delete markdown documents (specs, plans, ADRs, trackers, bug files) with YAML frontmatter, plus their events, augmentations and entries. Defaults: scope=project; archived/superseded hidden unless the filter constrains status; kind/status shortcuts AND with filter. Trackers are kind=tracker documents that may carry an augmentation (persistent prompt + params) — call librarian(tracker_design) before creating one. Entries: append_entry assigns the next PREFIX-N id atomically — with entry_collection it appends a params row; without it the ledger is prose and, given anchor_heading+title+body, the server writes the `## PREFIX-N — title` section itself. update_entry patches ONE row in place — use it rather than patch={params:…}, whose RFC 7396 array semantics replace the whole collection. Events: event_create appends an immutable record (kind inside the `event` object); event_list reads them newest-first. augment attaches or replaces the augmentation — merge=false (default) overwrites it wholesale, so fields you omit silently reset, merge=true patches only what you pass; gather collects refresh context without writing (write back with update, commit_refresh=true); list_stale lists augmentations older than threshold_hours. graph walks links; link adds a manual rel; graft folds one row's history into another; state_at shows a document as of a commit or timestamp."
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
                    "enum": ["find", "get", "create", "update", "move", "delete", "graft", "link", "graph", "state_at", "append_entry", "update_entry", "event_create", "event_list", "augment", "gather", "list_stale"],
                    "description": "Operation to perform"
                },
                "filter": {
                    "type": "object",
                    "description": "find: filter AST. Compose with {\"and\":[...]}, {\"or\":[...]}, {\"not\":{...}}. Leaf format: {\"field_name\": {\"op\": value}}, e.g. {\"rel_path\": {\"contains\": \"docs/trackers\"}}, {\"kind\": {\"eq\": \"spec\"}}, {\"tags\": {\"in\": [\"foo\",\"bar\"]}}. Ops: eq ne in nin gt lt gte lte contains prefix. contains = substring (strings) / membership (tags, owners); prefix = starts-with. rel_path is repo-relative (docs/trackers, not /abs/path); its gt/lt/gte/lte are refused."
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
                    "description": "find: natural-language query for semantic search (requires embedder). Hits are CHUNK-grain: each item carries `matched` (line range, enclosing entry token, bounded snippet), so a hit names the entry that matched, not the file's head. Up to 2 chunks per artifact, so ONE DOCUMENT CAN APPEAR TWICE with different `matched` spans; `hints.cap_suppressed` counts the rest."
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "find/list_stale: project (default), repo, umbrella, or all."
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
                    "description": "find: max rows (default 50, max 500). event_list: max events (default 50). list_stale: max documents (default 10, max 50)."
                },
                "offset": {
                    "type": "integer",
                    "default": 0,
                    "maximum": 100000,
                    "description": "find: rows to skip for paging (default 0)."
                },
                "id": {
                    "type": "string",
                    "description": "get/update/move/delete/graph/state_at/append_entry/update_entry/event_create/event_list/gather/augment: document id (16-hex). find and create take none."
                },
                "threshold_hours": {
                    "type": "integer",
                    "default": 24,
                    "description": "list_stale: hours since last refresh to count as stale (default 24)."
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
                    "description": "augment: attach or replace a persistent prompt + params on any artifact (merge=false, default) or patch only the fields you provide (merge=true — see the top-level `merge` property). create: attach the augmentation atomically, so a tracker needs no follow-up call. Fields below are augment's own — see that action's own field descriptions for what each means. Unknown keys are REJECTED, not ignored — a typo here fails loudly rather than silently dropping the field.",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Required when merge=false (create/replace). Not required when merge=true — a merge call may patch only params/render_template/etc. and leave the existing prompt untouched. Persistent instruction: what to maintain and how to format it."
                        },
                        "params": {
                            "type": "object",
                            "description": "The data params payload on the augmentation row. On merge=false (default — create/replace), fully replaces existing params. On merge=true, RFC 7396 merge-patched into existing params. NOT gather config — gather behavior is controlled by gather_from/format/max_tokens fields written into the params payload itself by callers that need them."
                        },
                        "params_path": {
                            "type": "string",
                            "description": "Filesystem path to a JSON file holding the params payload, read server-side (absolute path recommended). Mutually exclusive with params. Use when params are too large to pass inline (≳9 KB) — see get_guide(\"librarian\") § Augmentation Lifecycle."
                        },
                        "render_template": {
                            "type": "string",
                            "description": "Optional MiniJinja template projecting `params` into a markdown snippet rendered into librarian_context output. Decouples live state from prose body."
                        },
                        "params_schema": {
                            "type": "object",
                            "description": "Optional JSON Schema validating params on every merge. Initial params are also validated."
                        },
                        "entry_collection": {
                            "type": "string",
                            "description": "Names the params array whose objects are this tracker's filterable entry rows (e.g. \"failures\"). Enables doc(get, entry_filter=...)."
                        },
                        "append_mode": {
                            "type": "boolean",
                            "default": false,
                            "description": "When true, artifact_update prepends a new dated section instead of replacing the body. Prompt should instruct the LLM to write only the new delta block."
                        },
                        "history_cap": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Max number of dated ## YYYY-MM-DD sections to retain. Oldest sections beyond cap are dropped on each append."
                        }
                    },
                    "additionalProperties": false
                },
                "merge": {
                    "type": "boolean",
                    "description": "augment: when true, patch only the fields you provide onto the existing augmentation: params is RFC 7396 merge-patched, any sibling field you pass is overlaid, omitted fields are preserved. prompt is not required. Requires an existing augmentation."
                },
                "patch": {
                    "type": "object",
                    "description": "update: the fields to change. Accepted keys: status, title, owners, tags, topic, time_scope, extra, body, body_edits, params (any other key returns RecoverableError). Top-level status/title/owners/tags/topic/time_scope/extra are lifted into patch automatically and reported under `corrections`; an update that changes nothing is refused. Body editing — three modes: (1) `body_edits: [{heading, action, content?|old_string+new_string?, at?, occurrence?, replace_all?, include_subsections?}]` for surgical per-section edits — edit_file's heading-addressed batch shape exactly, including its action semantics and occurrence rule; applied atomically, RECOMMENDED for tracker maintenance; (2) `body` for total overwrite, gated by the 50% shrink guard unless `force=true` is passed at top level; (3) frontmatter-only changes via status/title/owners/tags/topic/time_scope. `body` and `body_edits` are mutually exclusive. `params` is RFC 7396 merge-patched into the augmentation params — arrays are REPLACED whole, so use update_entry to change one row. Body mutations emit `field_patch` events (kind=field_patch, payload.field=body)."
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "update/delete/graft: apply rather than preview. update — bypass the body-shrink guard, required when a body write would cut the file by >50% in bytes or lines; see get_guide(\"librarian\") § Body Editing Surfaces. delete and graft are DRY RUNS by default and return what WOULD be destroyed: delete cascades to the augmentation, events, links and observations (catalog-only — the file is git-restorable, these are not), and graft DELETES from_id. Default false."
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
                },
                "event": {
                    "type": "object",
                    "description": "event_create: the event to append — an immutable record anchored to git, distinct from a field patch. `kind` lives inside this object so it never shares a key with the document `kind`.",
                    "required": ["kind", "payload"],
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": super::event_create::ALLOWED_KINDS,
                            "description": "event kind"
                        },
                        "payload": {
                            "type": "object",
                            "description": format!("event payload (a JSON object). {}", super::event_create::payload_requirements_sentence())
                        },
                        "author": { "type": "string", "description": "event author" },
                        "anchor_commit": { "type": "string", "description": "git commit to anchor the event to" },
                        "head_commit": { "type": "string", "description": "HEAD commit at write time — pass it explicitly when the task produces no commit of its own" },
                        "parent_event_id": { "type": "string", "description": "parent event id for threading" },
                        "resolves_intent_event_id": { "type": "string", "description": "intent event id this verdict resolves" },
                        "also_mutates": { "type": "array", "items": { "type": "string" }, "description": "additional document ids mutated by this event" },
                        "source": {
                            "type": "object",
                            "description": "external signal source",
                            "properties": { "uri": { "type": "string" }, "kind": { "type": "string" }, "payload": {} },
                            "required": ["uri", "kind"]
                        }
                    }
                },
                "kinds": { "type": "array", "items": { "type": "string" }, "description": "event_list: filter to these event kinds" },
                "since": { "type": "integer", "format": "int64", "description": "event_list: return events after this ms epoch" },
                "until": { "type": "integer", "format": "int64", "description": "event_list: return events before this ms epoch" }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
                RecoverableError::new(
                    "action required — one of: find, get, create, update, move, graft, link, graph, state_at, append_entry, update_entry, event_create, event_list, augment, gather, list_stale",
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
                "event_create" => super::event_create::call(ctx, flatten_event_args(&args)?).await,
                "event_list"   => super::timeline::call(ctx, id_as_artifact_id(&args)).await,
                "augment"      => super::augment::call(ctx, flatten_augment_args(&args)?).await,
                "gather"       => super::refresh::call(ctx, args).await,
                "list_stale"   => super::refresh_stale::call(ctx, args).await,
                other => Err(RecoverableError::new(format!(
                    "unknown action '{other}' — expected one of: find, get, create, update, move, delete, graft, link, graph, state_at, append_entry, update_entry, event_create, event_list, augment, gather, list_stale"
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
    async fn list_stale_action_routes_correctly() {
        let v = Artifact
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "list_stale", "scope": "all"}),
            )
            .await
            .unwrap();
        assert!(v.is_array() || v["items"].is_array());
    }
    /// `gather` dispatches to `super::refresh::call`. A nonexistent id still proves routing:
    /// `refresh::call` fails past deserialization with "no augmentation for artifact" — a
    /// message distinct from the `"unknown action"` fallback this test exists to rule out.
    #[tokio::test]
    async fn gather_action_routes_correctly() {
        let err = Artifact
            .call(
                &mk_ctx(),
                serde_json::json!({"action": "gather", "id": PROBE_NO_SUCH_ID}),
            )
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("no augmentation for artifact"),
            "expected routing to reach refresh::call's augmentation lookup, got: {msg}"
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
    /// Regression for the bug where the `id` param's routing prefix listed eleven actions
    /// and omitted `augment`, even though `doc(action="augment", id=…)` is the documented call
    /// form in CLAUDE.md and `get_guide("librarian")`. The prose routing prefix is the only
    /// machine- or agent-readable statement of which actions accept a param, so an omission
    /// here silently misleads a reader deciding whether `augment` takes an `id`.
    /// docs/issues/archive/2026-09-03-doc-id-param-routing-omits-the-augment-action.md
    #[test]
    fn id_param_routing_names_augment() {
        let schema = Artifact.input_schema();
        let id_desc = schema["properties"]["id"]["description"]
            .as_str()
            .expect("id param must have a description");
        assert!(
            id_desc.contains("augment"),
            "the `id` param's routing prefix must name `augment` — it requires an id and \
             the prefix is the only place that says so: {id_desc}"
        );

        let augment_desc = schema["properties"]["augment"]["description"]
            .as_str()
            .expect("augment param must have a description");
        assert!(
            augment_desc.starts_with("augment") || augment_desc.starts_with("action=\"augment\""),
            "the `augment` param's own description must open with its own routing token \
             rather than leading with an unrelated action's, or a reader has no reason to \
             expect it needs `id`: {augment_desc}"
        );
    }

    /// Site 1 of 4. The rationale that used to live here — why two calls are compared rather
    /// than one asserted to fail, why `deny_unknown_fields` is unavailable (measured: adding
    /// it once broke every `doc(update)` call), and what the `accepts_any_json` escape
    /// admits — now lives on `crate::tools::param_probe`, shared with `librarian` and
    /// `artifact_event`. Task 6 folded the standalone `artifact_refresh` tool's probe
    /// coverage (`gather`, `list_stale`) into this one.
    ///
    /// Required params are type-valid dummies chosen to **fail resolution**: a nonexistent
    /// id, an escaping `rel_path`. That failure is the point — it is reached *after*
    /// deserialisation, so a deserialisation error is visibly different from it. `create` in
    /// particular must not succeed, or its own second call would hit "already exists" and
    /// differ for the wrong reason.
    #[tokio::test]
    async fn every_action_labelled_schema_key_is_honored_by_that_action() {
        use crate::tools::param_probe::assert_all_honored;

        // 58 labelled keys across the 17 actions as of 2026-09-02 (Task 6 folded in
        // `gather`/`list_stale`). The floor leaves room for the schema to shrink
        // without a false alarm while still catching a break in the `<action>:` label
        // convention.
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

    const PROBE_ACTIONS: [&str; 17] = [
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
        "event_create",
        "event_list",
        "augment",
        "gather",
        "list_stale",
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
            "event_list" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
            }
            "event_create" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert(
                    "event".into(),
                    json!({"kind": "note", "payload": {"text": "probe"}}),
                );
            }
            "augment" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("augment".into(), json!({"prompt": "probe"}));
            }
            "gather" => {
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

    /// F3 (2026-09-02 fix round): folding the standalone `artifact_augment` tool's
    /// schema into `doc`'s shared `augment` object (Task 5, `5da2537d`) dropped the
    /// descriptions on five of its properties — `render_template`, `params_schema`,
    /// `entry_collection`, `append_mode` and `history_cap` — leaving bare `{"type": ...}`
    /// entries. `every_property_has_a_description` (server.rs) is documented as blind to
    /// this: it walks only the schema ROOT's `properties`, never a nested object's own
    /// `properties`, so `doc.augment.properties.*` was never checked by it and still
    /// isn't — this test is the dedicated check for that one nested object.
    #[tokio::test]
    async fn augment_object_properties_all_have_descriptions() {
        let schema = Artifact.input_schema();
        let props = &schema["properties"]["augment"]["properties"];
        for field in [
            "prompt",
            "params",
            "params_path",
            "render_template",
            "params_schema",
            "entry_collection",
            "append_mode",
            "history_cap",
        ] {
            let desc = props[field]["description"].as_str().unwrap_or_default();
            assert!(
                !desc.is_empty(),
                "doc.augment.properties.{field} has no description — it is invisible to \
                 every_property_has_a_description (root-only) and must be checked here instead"
            );
        }
    }

    /// F5 (2026-09-02 fix round): the `augment` object's schema declared
    /// `"required": ["prompt"]`, but `merge=true` calls legitimately omit `prompt` — a
    /// merge call may patch only `params`/`render_template`/etc. and leave an existing
    /// prompt untouched (`augment::create_or_replace_augmentation` only requires it on
    /// the `merge=false` path). CLAUDE.md's own worked example
    /// (`doc(action="augment", id=…, merge=true, augment={params:{...}})`) violates this
    /// over-declared schema. JSON Schema's `required` cannot express "required unless a
    /// sibling top-level field is true", so the fix is to drop it from `required`
    /// entirely and rely on the runtime enforcement in `augment::call`, which already
    /// names the condition in its error.
    #[tokio::test]
    async fn augment_object_does_not_over_declare_prompt_as_required() {
        let schema = Artifact.input_schema();
        let required = &schema["properties"]["augment"]["required"];
        assert!(
            required.is_null() || !required.as_array().unwrap().iter().any(|v| v == "prompt"),
            "doc.augment schema requires 'prompt' unconditionally, but merge=true calls \
             legitimately omit it — the schema over-declares what the runtime actually enforces"
        );
    }

    /// F5, runtime half: the schema no longer claims `prompt` is unconditionally
    /// required (asserted above); this confirms the runtime behavior that claim was
    /// deferring to is actually what runs — a prompt-less `merge=true` succeeds, and a
    /// prompt-less `merge=false` is refused with a message naming the condition.
    #[tokio::test]
    async fn augment_prompt_requirement_is_enforced_at_runtime_not_schema() {
        let ctx = mk_ctx();
        let id = "cccccccccccccccc";
        seed_row(&ctx, id);

        // Establish an augmentation first — merge=true patches an EXISTING one,
        // and the fields under test (prompt-less merge behavior) only make sense
        // once there is a prompt already on the row to leave untouched.
        Artifact
            .call(
                &ctx,
                json!({"action": "augment", "id": id, "augment": {"prompt": "keep me"}}),
            )
            .await
            .expect("initial create/replace with prompt must succeed");

        // merge=false, no prompt: refused, naming the condition.
        let err = Artifact
            .call(
                &ctx,
                json!({"action": "augment", "id": id, "augment": {"params": {"a": 1}}}),
            )
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("prompt") && msg.contains("merge"),
            "expected the refusal to name the merge=true escape hatch: {msg}"
        );

        // merge=true, no prompt: succeeds (patches params only).
        Artifact
            .call(
                &ctx,
                json!({
                    "action": "augment",
                    "id": id,
                    "merge": true,
                    "augment": {"params": {"a": 1}}
                }),
            )
            .await
            .expect("merge=true without prompt must succeed — it patches params only");
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

    /// One catalog row, no file. `TestArtifactRowBuilder` is what `timeline.rs` tests use.
    fn seed_row(ctx: &ToolContext, id: &str) {
        use crate::librarian::catalog::artifact::{upsert, TestArtifactRowBuilder};
        let cat = ctx.catalog.lock();
        upsert(&cat, &TestArtifactRowBuilder::new(id).build()).unwrap();
    }

    #[tokio::test]
    async fn event_create_lifts_the_nested_event_and_event_list_reads_it_back() {
        let ctx = mk_ctx();
        let id = "aaaaaaaaaaaaaaaa";
        seed_row(&ctx, id);
        let created = Artifact
            .call(
                &ctx,
                json!({"action": "event_create", "id": id,
                               "event": {"kind": "note", "payload": {"text": "hello"}}}),
            )
            .await
            .expect("event_create succeeds");
        assert!(created["event_id"].is_string(), "{created}");
        let listed = Artifact
            .call(&ctx, json!({"action": "event_list", "id": id}))
            .await
            .expect("event_list succeeds");
        let events = listed["items"].as_array().expect("items array");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["kind"], "note");
    }
    /// The whole point of nesting the event under `event: {kind: …}` is that the event's kind
    /// can never share a key with the document's own `kind` — `flatten_event_args` only ever
    /// reads `kind` from the nested object. Send a top-level `kind` that DIFFERS from the
    /// nested one and assert the stored event kind is the nested one, never the top-level one.
    /// Mutation-verified: making `flatten_event_args` let top-level keys win over the nested
    /// object's reds this test (2026-09-02).
    #[tokio::test]
    async fn event_create_nested_kind_wins_over_a_colliding_top_level_kind() {
        let ctx = mk_ctx();
        let id = "bbbbbbbbbbbbbbbb";
        seed_row(&ctx, id);
        let created = Artifact
            .call(
                &ctx,
                json!({"action": "event_create", "id": id, "kind": "verdict",
                               "event": {"kind": "note", "payload": {"text": "hello"}}}),
            )
            .await
            .expect("event_create succeeds");
        assert!(created["event_id"].is_string(), "{created}");
        let listed = Artifact
            .call(&ctx, json!({"action": "event_list", "id": id}))
            .await
            .expect("event_list succeeds");
        let events = listed["items"].as_array().expect("items array");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["kind"], "note", "{events:?}");
    }

    #[tokio::test]
    async fn event_create_without_an_event_object_is_refused_with_the_shape() {
        let err = Artifact
            .call(
                &mk_ctx(),
                json!({"action": "event_create", "id": "aaaaaaaaaaaaaaaa", "kind": "note"}),
            )
            .await
            .unwrap_err();
        let re = err.downcast_ref::<RecoverableError>().expect("recoverable");
        assert!(
            re.hint.as_deref().unwrap().contains("event={kind:"),
            "{re:?}"
        );
    }

    #[tokio::test]
    async fn augment_round_trips_through_the_nested_object_and_merge_preserves_prompt() {
        let ctx = mk_ctx();
        let id = "bbbbbbbbbbbbbbbb";
        seed_row(&ctx, id);
        Artifact
            .call(
                &ctx,
                json!({"action": "augment", "id": id, "augment": {"prompt": "keep me"}}),
            )
            .await
            .expect("attach");
        Artifact
            .call(
                &ctx,
                json!({"action": "augment", "id": id, "merge": true,
                               "augment": {"params": {"n": 1}}}),
            )
            .await
            .expect("merge");
        let cat = ctx.catalog.lock();
        let aug = crate::librarian::catalog::augmentation::get(&cat, id)
            .unwrap()
            .unwrap();
        assert_eq!(
            aug.prompt, "keep me",
            "merge=true must not touch the prompt"
        );
        let params: Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["n"], 1);
    }

    /// Inverted guard: pins the ABSENCE of a refuted intervention.
    ///
    /// Until 2026-08-18 five properties each restated the merge=false rule for
    /// themselves — 882 characters, on a surface delivered with every request. hamsa
    /// A-27 measured them and they buy nothing:
    ///
    /// | arm | statements of the rule | preservation cue | passed |
    /// |---|---|---|---|
    /// | base | 7 | yes | 10/10 |
    /// | treatment (this cut) | 2 | yes | 10/10 |
    /// | control-null | 0 | yes | 10/10 |
    /// | control-positive | 0 + mandatory merge=false | yes | 0/10 |
    /// | uncued control-null | 0 | **no** | 10/10 |
    ///
    /// The positive control is what makes that data rather than theatre: the same
    /// fixture channel, stimulus, checker and model move 10/10 to 0/10, so the surface
    /// demonstrably reaches the model and the ties are real. The uncued arm closes the
    /// other hole — with zero statements AND no "change nothing else" cue, the model
    /// still passed `merge=true` ten times out of ten. The behaviour is carried by the
    /// parameter's own semantics, not by the prose.
    ///
    /// Re-adding any of it needs a NEW base arm (P-3), not an intuition that more
    /// warning is safer. Note what is deliberately NOT cut: the tool description and
    /// the `merge` property still state the rule once each, and `params`' RFC 7396
    /// sentence is untouched — array replacement is a DIFFERENT rule, and it is the one
    /// that actually caused data loss (an entry collection went 19 rows to 1 on
    /// 2026-08-16). Cutting those two survivors is a different intervention needing its
    /// own arm.
    ///
    /// The BEHAVIOUR remains pinned by `merge_true_patches_sibling_fields_preserving_rest`
    /// (in `augment.rs`) — this guard is about the prose only.
    ///
    /// Ledger: `docs/trackers/prompt-hamsa-audit-log.md` A-27.
    /// Scenario: `prompt-engineering/scenarios/augment-merge-restatement/`.
    #[test]
    fn augment_schema_does_not_restate_the_merge_rule_per_field() {
        let schema = Artifact.input_schema();
        let props = schema["properties"]["augment"]["properties"]
            .as_object()
            .unwrap();

        for field in [
            "render_template",
            "params_schema",
            "append_mode",
            "history_cap",
            "entry_collection",
        ] {
            let desc = props[field]["description"].as_str().unwrap_or_default();
            assert!(
                !desc.contains("On merge=false this field is overwritten"),
                "`{field}` restates the merge=false rule again. A-27 measured five arms \
                     and the rule text moves nothing (0 statements still scored 10/10 with \
                     no cue); re-adding needs a new base arm, not an intuition."
            );
        }

        // The two surviving statements are load-bearing as DOCUMENTATION even though
        // they proved not load-bearing as ROUTING; an over-zealous later cut that
        // removes them is a different intervention and must not ride on A-27.
        assert!(
            Artifact
                .description()
                .contains("fields you omit silently reset"),
            "the tool description must still state the merge=false rule once"
        );
        assert!(
            schema["properties"]["merge"]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("omitted fields are preserved"),
            "the `merge` property must still state the rule once"
        );
        // Rule B is a different rule and the one with a real incident behind it.
        assert!(
            props["params"]["description"]
                .as_str()
                .unwrap_or_default()
                .contains("RFC 7396"),
            "`params` must keep its RFC 7396 sentence — array replacement is the rule \
                 that actually caused data loss, and A-27 did not test it"
        );
    }

    /// Moved verbatim from `artifact_event.rs` (Task 4 folded that tool into `doc`).
    ///
    /// The first half executes `validate_payload` rather than reading
    /// `REQUIRED_PAYLOAD_FIELDS`, so the table cannot drift from the validator: drop a
    /// check there and this fails instead of the schema confidently describing a rule that
    /// is no longer enforced.
    ///
    /// See `docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`.
    #[test]
    fn every_required_payload_field_is_enforced_and_advertised() {
        use crate::librarian::tools::event_create::REQUIRED_PAYLOAD_FIELDS;

        let desc = Artifact.input_schema()["properties"]["event"]["properties"]["payload"]
            ["description"]
            .as_str()
            .expect("payload must carry a description")
            .to_string();

        for (kind, fields) in REQUIRED_PAYLOAD_FIELDS {
            if fields.is_empty() {
                assert!(
                    desc.contains(kind),
                    "`{kind}` has no required keys and the description must say so: {desc}"
                );
                continue;
            }
            assert!(
                desc.contains(kind),
                "the description must name kind `{kind}`: {desc}"
            );

            for omitted in *fields {
                // Every OTHER required field present, so the failure is unambiguously the
                // omitted one and not simply the first check in the arm.
                let mut payload = serde_json::Map::new();
                for f in *fields {
                    if f != omitted {
                        payload.insert((*f).to_string(), serde_json::json!("x"));
                    }
                }
                let err = crate::librarian::tools::event_create::validate_payload(
                    kind,
                    &serde_json::Value::Object(payload),
                )
                .expect_err("a missing required payload field must be refused");

                assert_eq!(
                    err.to_string(),
                    format!("{kind}.{omitted} required"),
                    "the refusal must name kind and field, which is the shape \
                     usage::db::normalize_err_family classifies on"
                );
                assert!(
                    desc.contains(omitted),
                    "`{kind}.{omitted}` is enforced but never advertised: {desc}"
                );
            }
        }
    }

    /// Regression: `docs/issues/archive/2026-05-21-artifact-event-create-payload-rejected.md`.
    /// With no declared `type` on `payload`, MCP clients transported the value as a
    /// stringified JSON, which the server's `.as_object()` guard then rejected with "payload
    /// must be object". The original guard, `payload_schema_declares_object_type`, lived in
    /// the deleted `src/librarian/tools/artifact_event.rs` and was not moved when Task 4
    /// nested `payload` under `event` — `every_required_payload_field_is_enforced_and_advertised`
    /// reads only the *description* at this path, not the `type`, so it does not cover this.
    #[test]
    fn event_payload_schema_declares_object_type() {
        let schema = Artifact.input_schema();
        assert_eq!(
            schema["properties"]["event"]["properties"]["payload"]["type"], "object",
            "payload must declare type=object so clients send an object, not a JSON string"
        );
    }
}
