use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};

pub struct Librarian;

#[async_trait]
impl Tool for Librarian {
    fn name(&self) -> &'static str {
        "librarian"
    }

    fn description(&self) -> &'static str {
        "Workspace-level librarian operations. \
             action: context | reindex | tracker_design | workspace_state_at | audit_doc_refs | legibility_scan | link_scan | doctor | merge_worktree. \
             context: pack topic/anchor neighbourhood into a markdown bundle. \
             reindex: re-scan and classify markdown artifacts. \
             tracker_design: return teaching prompt + archetype library (call BEFORE artifact(create) for trackers). \
             workspace_state_at: time-travel snapshot of all artifacts at a commit/timestamp. \
             audit_doc_refs: scan markdown for stale code refs (paths, symbols, \
             line refs, links) against the filesystem + LSP index. Manual — run \
             before a doc-heavy merge. Emits an `audit_issues` tracker. \
             legibility_scan: rank refactor candidates from usage.db friction + the \
             AST symbol index; writes the legibility-backlog tracker, auto-closing \
             refactored ones. write=false for a dry-run JSON. \
             link_scan: derive rel=\"cites\" edges from prose citations (entry \
             tokens, ids, md links); default reports, write=true \
             materializes/prunes cites edges. \
             doctor: catalog drift scanner (read-only by default): abs_path form, \
             ADS colons, '..' segments, missing files; commits.git_root form; \
             worktree-scoped rows; frontmatter id vs catalog id. JSON \
             violation-count report. Opt-in repairs, each detailed under the `fix` \
             param: prune_missing, reseat_worktree, rehome, repair_frontmatter_id, \
             mint_slugs. \
             merge_worktree: fold a worktree's shadow rows onto their main twins \
             (delta-only, never duplicates base entries); reseats worktree-born \
             rows. root=<worktree_root>; dry_run=true previews only; abandon=true \
             drops the shadows."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["context", "reindex", "tracker_design", "workspace_state_at", "audit_doc_refs", "legibility_scan", "link_scan", "doctor", "merge_worktree"],
                    "description": "Operation to perform"
                },
                "topic": { "type": "string", "description": "context: subject for semantic/LIKE search across titles and topics" },
                "anchor_id": { "type": "string", "description": "context: artifact id to anchor the bundle (uses link graph)" },
                "max_tokens": { "type": "integer", "default": 4000, "description": "context: approximate token budget" },
                "include_archived": { "type": "boolean", "default": false },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "context/reindex/workspace_state_at/link_scan: scope. audit_doc_refs: project-scoped only in v1 — any other value is rejected. Defaults to the active project on every action; `reindex` alone widens to `all` when no project is active, since there is then no project to re-scan."
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
                "write": { "type": "boolean", "description": "legibility_scan (default true): reconcile the backlog tracker (false = dry-run JSON only). link_scan (default false): materialize/prune cites edges (false = report only)." },
                "project": { "type": "string", "description": "legibility_scan: project root path; defaults to active project. Scopes the recorder lane." },
                "limit": { "type": "integer", "description": "legibility_scan: cap candidates returned/written. link_scan: cap artifacts scanned (default 10000). doctor: size of the emitted abs_path_outside_managed_roots window (default 10) — raise it to reach the rows the report counts but elides; pair with offset to page." },
                "offset": { "type": "integer", "description": "doctor: skip this many abs_path_outside_managed_roots rows before the window (default 0). Rows are ordered by abs_path, so pages are stable and disjoint across calls." },
                "fix": { "type": "string", "enum": ["prune_missing", "reseat_worktree", "rehome", "repair_frontmatter_id", "mint_slugs"], "description": "doctor: opt-in repair. prune_missing removes every artifact + commits row under a dead/renamed root (requires root=). reseat_worktree auto-reseats no-collision worktree-scoped catalog rows to their main-repo path; collisions are reported for manual artifact(action=\"graft\"). rehome migrates a moved repo's catalog rows from old_root to new_root, preserving ids/history (requires old_root=<old path, gone> [alias: root=] + new_root=<new path, must exist>; dry-run by default, pass confirm=true to apply). repair_frontmatter_id rewrites every frontmatter_id_mismatch file's `id:` to its catalog row's id, for every artifact UNDER ONE ROOT (root= or the active project — the catalog spans every repo on the machine and this writes files). Dry-run by default, confirm=true to apply; a file with NO frontmatter id is left alone rather than stamped. mint_slugs backfills artifact.slug where NULL (dry-run; confirm=true to apply). Omit for a read-only scan." },
                "root": { "type": "string", "description": "doctor fix=prune_missing: absolute path of the dead/renamed repo root to prune (refused if the path still exists on disk). OMIT root to run BATCH mode: dry-run lists every dead root (whole-subtree-gone) with row counts; pass confirm=true to prune them all. fix=rehome: absolute path the repo used to live at (must no longer exist) — old_root is preferred; root is accepted as an alias for back-compat. merge_worktree: the worktree root to merge/abandon (must have an active registration)." },
                "old_root": { "type": "string", "description": "For fix=rehome: absolute path the repo USED TO live at (must no longer exist on disk). Preferred alias of root — use this name, it's the one the doctor hints and error text surface." },
                "new_root": { "type": "string", "description": "For fix=rehome: absolute path the repo now lives at." },
                "confirm": { "type": "boolean", "description": "doctor fix=prune_missing batch mode / fix=rehome: pass true to apply; omitted/false = dry-run." },
                "dry_run": { "type": "boolean", "description": "merge_worktree: compute and return the full merge report without writing anything." },
                "abandon": { "type": "boolean", "description": "merge_worktree: delete all of the worktree's shadow rows and mark its registration abandoned, instead of merging." }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let action = args["action"].as_str().ok_or_else(|| {
                RecoverableError::new(
                    "action required — one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor, merge_worktree",
                )
            })?;
        match action {
                "context"            => super::context::call(ctx, args).await,
                "reindex"            => super::reindex::call(ctx, args).await,
                "tracker_design"     => super::tracker_design::call(ctx, args).await,
                "workspace_state_at" => super::workspace_state_at::call(ctx, args).await,
                "audit_doc_refs"     => super::audit_doc_refs::call(ctx, args).await,
                "legibility_scan"    => super::legibility_scan::call(ctx, args).await,
                "link_scan"          => super::link_scan::call(ctx, args).await,
                "doctor"             => super::doctor::call(ctx, args).await,
                "merge_worktree"     => super::merge_worktree::call(ctx, args).await,
                other => Err(RecoverableError::new(format!(
                    "unknown action '{other}' — expected one of: context, reindex, tracker_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor, merge_worktree"
                ))),
            }
    }
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

    #[tokio::test]
    async fn unknown_action_returns_recoverable_error() {
        let err = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn tracker_design_routes_correctly() {
        let v = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "tracker_design"}))
            .await
            .unwrap();
        assert!(v["archetypes"].is_array());
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
