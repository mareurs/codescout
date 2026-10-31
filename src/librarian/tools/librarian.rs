use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{LibrarianRecoverableError, Tool, ToolContext};

pub struct Librarian;

#[async_trait]
impl Tool for Librarian {
    fn name(&self) -> &'static str {
        "librarian"
    }

    fn description(&self) -> &'static str {
        "Workspace-level librarian operations. \
             context: pack topic/anchor neighbourhood into a markdown bundle. \
             reindex: re-scan and classify markdown artifacts. \
             status: is a reindex running, how far along? Live counter; lock-free. \
             tracker_design: return teaching prompt + archetype library (call BEFORE doc(create) for trackers). \
             workspace_state_at: time-travel snapshot of all artifacts at a commit/timestamp. \
             audit_doc_refs: scan markdown for stale code refs (paths, symbols, \
             line refs, links) against the filesystem + LSP index. Manual — run \
             before a doc-heavy merge. Emits an `audit_issues` tracker. \
             legibility_scan: rank refactor candidates from usage.db friction + the \
             AST symbol index; writes the legibility-backlog tracker, auto-closing \
             refactored ones. write=false for a dry-run JSON. \
             link_scan: derive rel=\"cites\" edges from prose citations (entry \
             tokens, ids, md links); default reports, write=true \
             materializes/prunes them. \
             doctor: catalog/statement/bug-record/augmentation checks (read-only \
             by default): abs_path form, ADS colons, '..' segments, missing \
             files; commits.git_root form; worktree-scoped rows; frontmatter id \
             vs catalog id; and `claim_liveness` — a `status: taken` bug whose \
             claiming session is dead or unresolvable here. JSON report; \
             repairs are opt-in via `fix`. \
             merge_worktree: fold a worktree's shadow rows onto their main twins \
             (delta-only, never duplicates base entries); reseats worktree-born \
             rows. root=<worktree_root>; dry_run=true previews only; abandon=true \
             drops the shadows. \
             audit_log: query the catalog audit trail (who/what/when mutated \
             audited tables), newest first; filter by tbl/row_id/actor/op/since/ \
             until. prune_before_ms dry-runs a prune (returns would_delete); \
             confirm=true applies it and leaves a self-describing marker row."
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
                    "enum": ["context", "reindex", "status", "tracker_design", "workspace_state_at", "audit_doc_refs", "legibility_scan", "link_scan", "doctor", "merge_worktree", "audit_log"],
                    "description": "Operation to perform"
                },
                "topic": { "type": "string", "description": "context: subject for semantic/LIKE search across titles and topics" },
                "anchor_id": { "type": "string", "description": "context: artifact id to anchor the bundle (uses link graph)" },
                "max_tokens": { "type": "integer", "default": 4000, "description": "context: approximate token budget" },
                "include_archived": { "type": "boolean", "default": false, "description": "context/workspace_state_at: include archived and superseded artifacts when gathering." },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "context/reindex/workspace_state_at/link_scan/doctor: scope. audit_doc_refs: project-scoped only in v1 — any other value is rejected. Defaults to the active project on every action; `reindex` alone widens to `all` when no project is active, since there is then no project to re-scan."
                },
                "repo": { "type": "string", "description": "reindex: restrict to a specific workspace root" },
                "force": { "type": "boolean", "description": "reindex: ignore cached file hashes and re-walk every file (re-classification; does NOT by itself force re-embedding — see reembed)" },
                "reembed": { "type": "boolean", "description": "reindex: also queue every file for re-embedding even when its content hash is unchanged. Use after enabling embeddings for the first time, or after switching embedding models/backends, on an already-indexed project — otherwise unchanged content is silently never (re-)embedded." },
                "intent": { "type": "string", "description": "tracker_design: free-form intent (optional)" },
                "archetype": {
                    "type": "string",
                    // Derived, never hand-copied: the response's `archetype_detail` and
                    // `next_step` both instruct the caller to pass this, so a schema that
                    // omits it (or lists stale names) makes the tool's own instructions
                    // unfollowable by any client that validates against the schema.
                    "enum": super::tracker_design::archetype_names(),
                    "description": "tracker_design: fetch ONE archetype in full — params_shape_example, params_schema_example, render_template_example, body_skeleton, prompt_template. Omit for the menu (name + when_to_use only); the full examples are ~95% of the payload, so they are fetched one at a time once a choice is made. An unknown name is refused with the list of valid ones."
                },
                "commit": { "type": "string", "description": "workspace_state_at: git commit hash as time-travel cutoff. Exactly one of commit or timestamp required." },
                "timestamp": { "type": "integer", "format": "int64", "description": "workspace_state_at: unix epoch ms as cutoff. Exactly one of commit or timestamp required." },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "workspace_state_at: filter by artifact kinds"
                },
                "freshness_filter": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["fresh", "stale", "unknown", "superseded"] },
                    "description": "workspace_state_at: only return artifacts matching these freshness values"
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "audit_doc_refs: glob patterns to restrict scan (default: docs/**/*.md, CLAUDE.md, **/README.md). Default scan excludes docs/agents/** — pass an explicit list to include those files."
                },
                "emit_tracker": { "type": "boolean", "default": true, "description": "audit_doc_refs: create/update an audit_issues tracker artifact with results" },
                "tracker_id": { "type": "string", "description": "audit_doc_refs: existing tracker id to update (creates new if omitted)" },
                "fail_on": { "type": "string", "default": "never", "description": "audit_doc_refs: exit_code 1 when findings reach this severity (high | med | low | never)" },
                "write": { "type": "boolean", "description": "legibility_scan/link_scan: legibility_scan (default true) reconciles the backlog tracker, link_scan (default false) materializes/prunes cites edges; false = dry-run JSON / report only." },
                "offset": { "type": "integer", "description": "doctor: skip this many abs_path_outside_managed_roots rows before the window (default 0); ordered by abs_path, so pages are stable and disjoint." },
                "findings_offset": { "type": "integer", "description": "link_scan: skip N findings per array (default 0); page with findings_limit until counts.truncated is false." },
                "findings_limit": { "type": "integer", "description": "link_scan: findings per array (default 50)." },
                "fix": { "type": "string", "enum": ["prune_missing", "reseat_worktree", "rehome", "repair_frontmatter_id", "mint_slugs", "export_augmentations"], "description": "doctor: opt-in repair; omit for a read-only scan. Each fix WRITES, is scoped (root= or the active project), and is a DRY RUN until confirm=true; the root/new_root params say what each requires. prune_missing drops artifact+commits rows under a dead/renamed root. reseat_worktree auto-reseats no-collision worktree-scoped catalog rows to their main-repo path; collisions are reported for manual doc(action=\"graft\"). rehome migrates a moved repo's rows from old_root to new_root, preserving ids/history. repair_frontmatter_id rewrites every frontmatter_id_mismatch file's `id:` to its catalog row's id, for every artifact under one root; a file with NO frontmatter id is left alone rather than stamped. mint_slugs backfills artifact.slug where NULL. export_augmentations exports each augmentation's shape (never its params) to a committed sidecar and stamps `expects_augmentation:` to name it, so another machine's reindex re-attaches it; it can only export rows THIS catalog holds." },
                "root": { "type": "string", "description": "doctor/merge_worktree: with fix=prune_missing, absolute path of the dead/renamed repo root to prune (refused if the path still exists on disk). OMIT root to run BATCH mode: dry-run lists every dead root (whole-subtree-gone) with row counts; pass confirm=true to prune them all. fix=rehome: use `old_root` instead; root is accepted as a back-compat alias. merge_worktree: the worktree root to merge/abandon (must have an active registration)." },
                "old_root": { "type": "string", "description": "doctor: fix=rehome, absolute path the repo USED TO live at (must no longer exist on disk). Preferred alias of root — use this name, it's the one the doctor hints and error text surface." },
                "new_root": { "type": "string", "description": "doctor: fix=rehome, absolute path the repo now lives at." },
                "dry_run": { "type": "boolean", "description": "merge_worktree: compute and return the full merge report without writing anything." },
                "abandon": { "type": "boolean", "description": "merge_worktree: delete all of the worktree's shadow rows and mark its registration abandoned, instead of merging." },
                "tbl": { "type": "string", "description": "audit_log: filter to one audited table name (e.g. 'artifact', 'commits')." },
                "row_id": { "type": "string", "description": "audit_log: filter to one row's flattened key (e.g. an artifact id)." },
                "actor": { "type": "string", "description": "audit_log: filter to one actor string ('codescout:<session-id>', 'codescout:anonymous', or 'unknown' for an unidentified foreign writer)." },
                "op": { "type": "string", "enum": ["insert", "update", "delete"], "description": "audit_log: filter to one operation kind." },
                "since": { "type": "integer", "format": "int64", "description": "audit_log: only rows at_ms >= this epoch-ms UTC timestamp." },
                "until": { "type": "integer", "format": "int64", "description": "audit_log: only rows at_ms <= this epoch-ms UTC timestamp." },
                "limit": { "type": "integer", "description": "legibility_scan/link_scan/doctor/audit_log: legibility_scan: cap candidates returned/written. link_scan: cap ARTIFACTS walked (default 10000) — findings use findings_limit. doctor: abs_path_outside_managed_roots window size (default 10); raise it to reach rows the report counts but elides, pair with offset to page. audit_log: max rows returned, newest first (default 50, max 500)." },
                "prune_before_ms": { "type": "integer", "format": "int64", "description": "audit_log: delete audit rows with at_ms strictly less than this epoch-ms UTC cutoff. Dry-run by default (returns would_delete); pass confirm=true to apply — the apply always leaves one self-describing marker row explaining the resulting seq gap." },
                "confirm": { "type": "boolean", "description": "doctor/audit_log: fix=prune_missing batch mode / fix=rehome / prune_before_ms: pass true to apply; omitted/false = dry-run." },
                "export": { "type": "boolean", "description": "audit_log: export unexported rows to this host's shard." }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
                LibrarianRecoverableError::new(
                    "action required — one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor, merge_worktree, audit_log",
                )
            })?;
        // Best-effort: identity enrichment must never fail a tool call; a failed
        // stamp degrades the row to verb=NULL, which audit_log surfaces honestly.
        if let Err(e) = ctx
            .catalog
            .lock()
            .set_audit_verb(&format!("librarian.{action}"))
        {
            tracing::warn!("audit verb stamp failed: {e}");
        }
        // Computed before dispatch consumes `args`. A key the schema does not declare is
        // dropped by every action, so the caller must be told or `path="<one file>"` looks
        // like a file-scoped run. docs/issues/2026-10-30-librarian-reindex-link-scan-and-audit-doc-refs-accept-a-path-argument-and-run-project-wide-without-saying-so.md
        let schema = self.input_schema();
        let ignored = undeclared_keys(&args, &schema);
        let not_read = keys_for_other_actions(&args, &schema, action);
        let action = action.to_owned();
        let mut out = match action.as_str() {
                "context"            => super::context::call(ctx, args).await,
                "reindex"            => super::reindex::call(ctx, args).await,
                "status"             => super::status::call(ctx, args).await,
                "tracker_design"     => super::tracker_design::call(ctx, args).await,
                "workspace_state_at" => super::workspace_state_at::call(ctx, args).await,
                "audit_doc_refs"     => super::audit_doc_refs::call(ctx, args).await,
                "legibility_scan"    => super::legibility_scan::call(ctx, args).await,
                "link_scan"          => super::link_scan::call(ctx, args).await,
                "doctor"             => super::doctor::call(ctx, args).await,
                "merge_worktree"     => super::merge_worktree::call(ctx, args).await,
                "audit_log"          => super::audit_log::call(ctx, args).await,
                other => Err(LibrarianRecoverableError::new(format!(
                    "unknown action '{other}' — expected one of: context, reindex, status, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor, merge_worktree, audit_log"
                ))),
            }?;
        if let (false, Some(obj)) = (
            ignored.is_empty() && not_read.is_empty(),
            out.as_object_mut(),
        ) {
            let mut keys: Vec<&String> = ignored.iter().chain(&not_read).collect();
            keys.sort();
            let hints: Vec<String> = [
                (!ignored.is_empty()).then(|| ignored_params_hint(&action, &ignored)),
                (!not_read.is_empty()).then(|| not_read_hint(&action, &not_read)),
            ]
            .into_iter()
            .flatten()
            .collect();
            obj.insert(
                "ignored_params".into(),
                json!({ "keys": keys, "hint": hints.join(" ") }),
            );
        }
        Ok(out)
    }
}

/// Top-level keys of `args` that `schema` does not declare, sorted.
///
/// `workspace` is exempt: the server injects it into the advertised schema of every
/// pinnable tool after `input_schema()` returns (`CodeScoutServer::inject_workspace_param`),
/// so the raw schema never lists it.
fn undeclared_keys(args: &Value, schema: &Value) -> Vec<String> {
    let (Some(given), Some(declared)) = (args.as_object(), schema["properties"].as_object()) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = given
        .keys()
        .filter(|k| k.as_str() != "workspace" && !declared.contains_key(k.as_str()))
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// True when `desc` names `action`. A key's description is where its actions are written
/// (`"reindex: …"`, `"context/reindex/link_scan: …"`), and prose after the label may name
/// more (`scope` names `audit_doc_refs` after the label). Substring match: a false hit only
/// exempts a key from the report, which is the safe direction.
fn names_action(desc: &str, action: &str) -> bool {
    desc.contains(action)
}

/// Declared keys of `args` that `action` does not read, sorted.
///
/// A key counts only when its description names at least one dispatched action and not this
/// one, so a key labelled for no action stays exempt. `status` takes `_args` and reads
/// nothing; every other action's reading is pinned by
/// `tests::no_action_reads_a_key_its_description_does_not_name`, which fails when a label
/// is incomplete, so this report cannot name a key the action does read.
fn keys_for_other_actions(args: &Value, schema: &Value, action: &str) -> Vec<String> {
    let (Some(given), Some(declared)) = (args.as_object(), schema["properties"].as_object()) else {
        return Vec::new();
    };
    let actions: Vec<&str> = schema["properties"]["action"]["enum"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut keys: Vec<String> = given
        .keys()
        .filter(|k| {
            let Some(desc) = declared
                .get(k.as_str())
                .and_then(|d| d["description"].as_str())
            else {
                return false;
            };
            k.as_str() != "action"
                && actions.iter().any(|a| names_action(desc, a))
                && !names_action(desc, action)
        })
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// Say that `keys` are parameters of librarian that `action` does not read.
fn not_read_hint(action: &str, keys: &[String]) -> String {
    let list = keys
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Parameters of librarian, but not of action={action}: {list}. They were ignored; their descriptions name other actions."
    )
}

/// Say that `keys` had no effect and, for `path`, name what the action offers instead.
fn ignored_params_hint(action: &str, keys: &[String]) -> String {
    let list = keys
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut hint = format!(
        "Not a parameter of librarian: {list}. It was ignored; action={action} ran with its declared parameters only."
    );
    if keys.iter().any(|k| k == "path") {
        hint.push_str(match action {
            "audit_doc_refs" => " To restrict the scan, pass paths=[\"<glob>\"].",
            "reindex" => {
                " reindex has no per-file scope; `repo` restricts it to one workspace root."
            }
            "link_scan" => " link_scan has no per-file scope; `scope` and `limit` bound it.",
            _ => "",
        });
    }
    hint
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use std::sync::Arc;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    /// Minimum type-valid args per action, shared by both probe directions.
    fn required(action: &str) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        match action {
            // Exactly one of commit|timestamp is required; a well-formed hash that
            // resolves to nothing keeps the failure AFTER deserialisation, where the
            // probe can see it.
            "workspace_state_at" => {
                m.insert(
                    "commit".into(),
                    json!("0000000000000000000000000000000000000000"),
                );
            }
            "merge_worktree" => {
                m.insert("root".into(), json!("/nonexistent/worktree-probe"));
            }
            _ => {}
        }
        m
    }

    /// Remove every `host` key. `doctor` reports the host name, and each fresh test context
    /// mints a random one (`ripper-2b775b` against `ripper-ff9c14`), so two otherwise
    /// identical `doctor` calls never compare equal.
    fn drop_host(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("host");
                o.values_mut().for_each(drop_host);
            }
            Value::Array(a) => a.iter_mut().for_each(drop_host),
            _ => {}
        }
    }

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
    }

    /// Probe site for `librarian`, forward direction. Rationale, the `deny_unknown_fields` measurement, and the reason the
    /// probe compares two calls rather than asserting one fails all live on
    /// `crate::tools::param_probe`.
    ///
    /// **What this site contributed to the shared helper:** `librarian` labels some keys for
    /// several actions at once (`"context/reindex/workspace_state_at/link_scan: scope"`). The
    /// original `artifact`-only probe split on `:` and required an exact action match, so a
    /// slash-joined label matched nothing and was skipped *silently*. Splitting on `/` first
    /// is what reaches those keys — and `scope` is one of them, which is exactly the key in
    /// `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md`, an `IC-15`
    /// member that was this key on this tool.
    ///
    /// Born red: it flagged `tracker_design`'s `intent` and `archetype`, which were being
    /// discarded wholesale by an `unwrap_or_default()` on the deserialisation
    /// (`docs/issues/archive/2026-09-01-tracker-design-discards-every-argument-on-one-type-error.md`).
    #[tokio::test]
    async fn every_action_labelled_schema_key_is_honored_by_that_action() {
        use crate::tools::param_probe::{assert_all_honored, assert_required_are_advertised, Spec};

        let spec = Spec {
            actions: &[
                "context",
                "reindex",
                "status",
                "tracker_design",
                "workspace_state_at",
                "audit_doc_refs",
                "legibility_scan",
                "link_scan",
                "doctor",
                "merge_worktree",
            ],
            accepts_any_json: &[],
            required,
        };

        assert_all_honored(
            "librarian",
            &Librarian.input_schema(),
            &spec,
            // Floor is **action/key pairs**, not keys, and measured not chosen: 39 pairs, read
            // 2026-10-10 from the sweep's own coverage line (with the floor raised past it the
            // assertion reports `covered 39`). Was 30 before five prose-labelled keys were
            // re-labelled (`write`, `root`, `old_root`, `new_root`, `confirm`) and `limit` was
            // labelled for all four actions that read it; 28 on 2026-09-09, and 15 before
            // that, a figure derived when `sweep` probed only a shared key's first action — see
            // `crate::tools::param_probe::assert_all_honored` for why the margin between
            // floor and count is the number of labels that can go missing in silence.
            39,
            // Pinned from `sweep`'s own `unlabelled`, read 2026-10-10: EIGHT keys are not probed,
            // all labelled `audit_log:` while `audit_log` is NOT in `spec.actions`, on purpose.
            // This generic sweep decides "honored" by whether a call's outcome changes when the
            // key is ill-typed, and every librarian call writes an audit row, so an `audit_log`
            // response (`table_total`, `entries`) differs between ANY two calls: the comparison
            // is true whether or not the key was read. Adding the action turned the eight keys
            // green with the argument validation removed, which is a pass that cannot fail. The
            // keys are covered instead by
            // `audit_log::tests::a_wrong_typed_argument_is_refused_by_name_and_never_read_as_absent`,
            // which fails when the validation is removed. (`confirm` and `limit` are also
            // labelled `audit_log` and are swept through their other actions.)
            &[
                "actor",
                "export",
                "op",
                "prune_before_ms",
                "row_id",
                "since",
                "tbl",
                "until",
            ],
            |args| async move {
                // `doctor` reports a per-context random host id, so two fresh-context calls
                // differ whether or not the key was read: mask it (ab99663291a364a0).
                Librarian.call(&mk_ctx(), args).await.map(|mut v| {
                    drop_host(&mut v);
                    v
                })
            },
        )
        .await;

        // Reverse direction for `librarian` — see `param_probe::assert_required_are_advertised`.
        // Reuses the same `required` table rather than restating it: the point of the check is
        // that the two representations agree, so a second copy would defeat it.
        assert_required_are_advertised("librarian", &Librarian.input_schema(), &spec);
    }

    #[tokio::test]
    async fn tracker_design_routes_correctly() {
        let v = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "tracker_design"}))
            .await
            .unwrap();
        assert!(v["archetypes"].is_array());
    }

    #[tokio::test]
    async fn dispatch_stamps_the_audit_verb() {
        let ctx = mk_ctx();
        // tracker_design is read-only; the stamp happens at dispatch regardless of verb kind
        let _ = Librarian
            .call(&ctx, serde_json::json!({"action": "tracker_design"}))
            .await;
        let verb: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row("SELECT verb FROM audit_ctx", [], |r| r.get(0))
            .unwrap();
        assert_eq!(verb.as_deref(), Some("librarian.tracker_design"));
    }
    /// BUG 048fc80383fc1a75: `path="<one file>"` is not a `librarian` parameter. The three
    /// actions ran project-wide and the response never said the key was dropped.
    #[tokio::test]
    async fn an_undeclared_key_is_reported_as_ignored_and_a_declared_one_is_not() {
        let ctx = mk_ctx();
        let out = Librarian
            .call(
                &ctx,
                json!({"action": "tracker_design", "path": "docs/x.md", "workspace": "/w"}),
            )
            .await
            .unwrap();
        assert_eq!(
            out["ignored_params"]["keys"],
            json!(["path"]),
            "`path` is undeclared and `workspace` is injected by the server: {out}"
        );
        let clean = Librarian
            .call(&ctx, json!({"action": "tracker_design", "intent": "x"}))
            .await
            .unwrap();
        assert!(
            clean.get("ignored_params").is_none(),
            "a declared key must not be reported: {clean}"
        );
    }

    #[test]
    fn the_hint_for_path_names_what_each_action_offers_instead() {
        let keys = vec!["path".to_string()];
        assert!(ignored_params_hint("audit_doc_refs", &keys).contains("paths=["));
        assert!(ignored_params_hint("reindex", &keys).contains("`repo`"));
        assert!(ignored_params_hint("link_scan", &keys).contains("`scope`"));
    }

    /// BUG 27d66ed4f2d89fa4: a key the schema declares, sent to an action that does not read
    /// it, was accepted without a word. `status` reads nothing (`_args`), so every key
    /// labelled for another action is a case.
    #[tokio::test]
    async fn a_declared_key_sent_to_another_action_is_reported_as_ignored() {
        let out = Librarian
            .call(
                &mk_ctx(),
                json!({"action": "status", "force": true, "write": true}),
            )
            .await
            .unwrap();
        assert_eq!(
            out["ignored_params"]["keys"],
            json!(["force", "write"]),
            "{out}"
        );
        let hint = out["ignored_params"]["hint"].as_str().unwrap();
        assert!(hint.contains("action=status"), "{hint}");
    }

    #[test]
    fn a_key_named_for_the_action_or_for_no_action_is_not_reported() {
        let schema = Librarian.input_schema();
        // `force` is labelled `reindex:`.
        assert!(keys_for_other_actions(&json!({"force": true}), &schema, "reindex").is_empty());
        // `scope` names `audit_doc_refs` after its label, not in it.
        assert!(
            keys_for_other_actions(&json!({"scope": "project"}), &schema, "audit_doc_refs")
                .is_empty()
        );
        // `action` and an undeclared key are not this report's business.
        assert!(keys_for_other_actions(
            &json!({"action": "status", "path": "x"}),
            &schema,
            "status"
        )
        .is_empty());
    }

    /// The guard that makes `keys_for_other_actions` safe to trust: it reports a key as
    /// ignored on the strength of a description, so a description missing an action that
    /// does read the key would turn the report into a false claim. Sends each key to every
    /// action its description does NOT name, ill-typed, and fails when the outcome changes.
    ///
    /// `audit_log` is not swept for the reason its forward probe is not: every librarian
    /// call writes an audit row, so two `audit_log` responses always differ and the
    /// comparison would pass whether or not the key was read. It reads only keys that name it
    /// (`audit_log::call`: `export`, `prune_before_ms`, `confirm`, `limit`, `tbl`, `row_id`,
    /// `actor`, `op`, `since`, `until`).
    #[tokio::test]
    async fn no_action_reads_a_key_its_description_does_not_name() {
        use crate::tools::param_probe::{ill_typed, outcome};

        let schema = Librarian.input_schema();
        let actions: Vec<&str> = schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|a| *a != "audit_log")
            .collect();
        let mut read = Vec::new();
        let mut checked = 0usize;
        for (key, spec) in schema["properties"].as_object().unwrap() {
            let desc = spec["description"].as_str().unwrap_or_default();
            if key == "action" || !actions.iter().any(|a| names_action(desc, a)) {
                continue;
            }
            let declared = spec["type"].as_str().unwrap_or("string");
            for action in actions.iter().filter(|a| !names_action(desc, a)) {
                let mut base = required(action);
                base.insert("action".into(), json!(action));
                let mut probe = base.clone();
                probe.insert(key.clone(), ill_typed(declared));
                let base = Librarian.call(&mk_ctx(), Value::Object(base)).await;
                let probed = Librarian.call(&mk_ctx(), Value::Object(probe)).await;
                checked += 1;
                // `ignored_params` is what this change adds, so it is the one difference
                // that must not count as a read.
                let strip = |r: &anyhow::Result<Value>| match r {
                    Ok(v) => {
                        let mut v = v.clone();
                        if let Some(o) = v.as_object_mut() {
                            o.remove("ignored_params");
                        }
                        drop_host(&mut v);
                        Ok(v)
                    }
                    Err(e) => Err(anyhow::anyhow!("{e}")),
                };
                if outcome(&strip(&base)) != outcome(&strip(&probed)) {
                    read.push(format!("{action}:{key}"));
                }
            }
        }
        assert!(
            read.is_empty(),
            "these actions read a key their description does not name, so the key is wrongly \
             reported as ignored — add the action to the key's label: {read:?}"
        );
        // Floor measured, not chosen: 259 action/key pairs, read 2026-10-30 from this sweep's
        // own count. A key that loses its label drops out of the sweep and the count with it.
        assert!(checked >= 259, "the sweep covered only {checked} pairs");
    }

    /// `tracker_design`'s response tells the caller, twice (`archetype_detail`
    /// and `next_step`), to call back with `archetype="<name>"`. Until
    /// 2026-08-17 the input schema did not declare that parameter at all, so
    /// any client that validates arguments against the schema could not follow
    /// the tool's own instruction — the handler accepted it, the contract
    /// denied it existed.
    ///
    /// The enum must be DERIVED from `archetype_names()`, never hand-copied: a
    /// copied list drifts the moment an archetype is added, and then the schema
    /// advertises a set the handler refuses (or hides one it accepts).
    #[test]
    fn input_schema_declares_archetype_derived_from_the_live_names() {
        let schema = Librarian.input_schema();
        let props = &schema["properties"];
        assert!(
            props["archetype"].is_object(),
            "input_schema must declare `archetype` — tracker_design's next_step \
             instructs callers to pass it. Declared properties: {:?}",
            props.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );

        let declared: Vec<String> = props["archetype"]["enum"]
            .as_array()
            .expect("`archetype` must carry an enum of the valid names")
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        let live = crate::librarian::tools::tracker_design::archetype_names();

        assert!(!live.is_empty(), "archetype_names() returned no names");
        assert_eq!(
            declared, live,
            "the schema's `archetype` enum must equal archetype_names() exactly"
        );
    }

    #[tokio::test]
    async fn audit_doc_refs_action_routes() {
        use crate::librarian::current_project::CurrentProject;
        use crate::librarian::workspace::Root;
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        // Write a minimal markdown file so the scanner has something to scan.
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/readme.md"), "# hello\n").unwrap();
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                main_root: None,
                umbrella: None,
            }))
            .build();
        let result = crate::librarian::tools::audit_doc_refs::call(&ctx, serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 0);
        assert!(result["findings"].is_array());
    }

    #[tokio::test]
    async fn legibility_scan_action_routes() {
        let ctx = mk_ctx();
        let args = serde_json::json!({ "action": "legibility_scan", "write": false });
        // No active project in mk_ctx → RecoverableError, NOT "unknown action".
        let err = Librarian.call(&ctx, args).await.unwrap_err();
        let msg = format!("{err}");
        assert!(!msg.contains("unknown action"), "should route, got: {msg}");
    }
}
