//! Configuration and project management tools.

use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
use crate::tools::onboarding::{onboarding_version_stale, ONBOARDING_VERSION};
use crate::util::fs::to_forward_slash;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct Workspace;

#[async_trait::async_trait]
impl Tool for Workspace {
    fn name(&self) -> &str {
        "workspace"
    }

    /// Mixed: `status`/`list_projects` read, `activate` writes — it calls
    /// `auto_register_deps`, which persists `.codescout/libraries.json`
    /// (`src/library/auto_register.rs:64-65`). Re-activating the same root re-registers
    /// nothing, so the write is additive and idempotent.
    ///
    /// **This deliberately disagrees with `is_write`**, which returns the trait default
    /// `false` for every `workspace` call. That is the narrower claim — `is_write` gates
    /// the cross-process write lock, and the lock has never covered this path. Annotating
    /// truthfully here does not change that; see
    /// `docs/issues/2026-09-03-workspace-activate-writes-libraries-json-outside-the-write-lock.md`.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::additive_closed()
    }

    fn description(&self) -> &str {
        "Project workspace operations. Actions: \
         `activate` (switch active project; pass `path` and optional `read_only`), \
         `status` (current project + index + memories), \
         `list_projects` (workspace members)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["activate", "status", "list_projects"],
                    "description": "Operation to perform. Required unless post_compact=true, which implies status."
                },
                "path": {
                    "type": "string",
                    "description": "For action='activate': project path or workspace project id."
                },
                "read_only": {
                    "type": "boolean",
                    "description": "For action='activate': read-only mode (default: false at home, true elsewhere; explicit wins)."
                },
                "post_compact": {
                    "type": "boolean",
                    "description": "Flush all LSP clients after context compaction. Implies action='status' when action is omitted."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let post_compact = input
            .get("post_compact")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None if post_compact => "status",
            None => {
                return Err(super::RecoverableError::with_hint(
                    "workspace requires 'action' parameter",
                    "Pass action='activate' | 'status' | 'list_projects'.",
                )
                .into());
            }
        };
        match action {
            "activate" => ActivateProject.call(input, ctx).await,
            "status" => ProjectStatus.call(input, ctx).await,
            "list_projects" => {
                let full = ProjectStatus.call(json!({}), ctx).await?;
                Ok(json!({ "workspace": full.get("workspace") }))
            }
            other => Err(super::RecoverableError::with_hint(
                format!("unknown workspace action: {}", other),
                "Valid actions: 'activate', 'status', 'list_projects'.",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // `activate` responses carry `auto_libs` or `project_root` at the top level;
        // `status` responses carry `index`/`memory_staleness`; `list_projects` carries
        // only `workspace`. Use shape detection.
        if result.get("project_hints").is_some() {
            Some(format_activate_project(result))
        } else {
            Some(format_project_status(result))
        }
    }
    fn relevant_guide_topic(&self, _result: &Value) -> Option<&str> {
        // `workspace-state` — the guide about home/foreign activation and the
        // restore-before-you-finish rule — is the one this tool's caller is about to
        // need. Until 2026-08-16 nothing fired it at all: `a926fdf5` moved the subagent
        // workspace-pinning rule out of the always-loaded `server_instructions` slice
        // into that file to fit the 2200-byte cap, and the file had no trigger, so the
        // rule stopped reaching anyone. It is the measured instance of BL-25.
        //
        // This arm used to return SESSION_OPENING_GUIDE. That was already redundant:
        // `ActivateProject::call` governs the bootstrap topic itself — re-arming
        // it via `PROJECT_SCOPED` on a genuine project switch, or wiping it along
        // with everything else via the blunt clear when no rendezvous is active
        // (same-project re-activation touches neither). The opener's trigger (the
        // `!emitted.contains(SESSION_OPENING_GUIDE)` check in `Tool::call_content`,
        // `src/tools/core/types.rs`) is already downstream of that logic regardless
        // of what this arm returns, so returning it here would only be a second,
        // redundant path to the same effect. `post_compact_rearms_guide_hints`
        // covers the separate `ProjectStatus` clear.
        //
        // See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
        Some("workspace-state")
    }
}

pub struct ActivateProject;
impl ActivateProject {
    pub const NAME: &'static str = "activate_project";
}

/// Topics forgotten on a genuine project switch. Deliberately just the one:
/// the tool-contract guides the model already holds stay valid across a switch,
/// and re-sending them is the waste this phase exists to remove.
///
/// Tied to `SESSION_OPENING_GUIDE` by construction rather than a second copy
/// of the literal: that's the exact topic the opener trigger in
/// `Tool::call_content` keys on, so the two must never drift apart.
pub(crate) const PROJECT_SCOPED: &[&str] = &[crate::prompts::SESSION_OPENING_GUIDE];

pub struct ProjectStatus;

#[async_trait::async_trait]
impl Tool for ActivateProject {
    fn name(&self) -> &str {
        Self::NAME
    }
    fn description(&self) -> &str {
        "Switch the active project to the given path. All subsequent tool calls \
         operate relative to this project root. Response includes `project_hints` \
         (primary language, manifest, entry points, build commands) so agents have \
         context even without running onboarding."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the project root" },
                "read_only": { "type": "boolean", "description": "Activate in read-only mode (default: true for non-home projects, false for home)" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        // Not the shared `path` hint: this one is a project root, not a file. BL-3 Class B.
        let path = super::require_str_param_or_hint(
            &input,
            "path",
            &[],
            "Pass the project directory, e.g. path=\"/home/me/work/myrepo\" — a workspace \
             project id also works. This is a directory, not a file; \
             workspace(action=\"list_projects\") shows valid project ids, including \
             auto-discovered sub-projects.",
        )?;
        let read_only = optional_bool_param(&input, "read_only");

        // Focus-switch path: a member of the CURRENT workspace, named either by bare
        // project id or by a path that resolves to that project's root. Returns
        // early — never reaches root resolution below, so the guide ledger stays
        // untouched.
        //
        // The path form exists because without it the two spellings of one project
        // resolved to different memory stores. A path built a STANDALONE workspace
        // rooted at the target, where the sub-project is its own root, so
        // `memory_dir_for_project` returned `<sub>/.codescout/memories` — the
        // directory nothing writes to for a project that is a workspace member. No
        // reader could repair that downstream: `Agent::activate` calls
        // `inner.workspaces.clear()`, so the parent workspace and its per-project
        // tree are gone before any reader runs. The fix has to be here, in which
        // workspace gets built.
        //
        // Scoped deliberately, and each exclusion is load-bearing:
        //   * The ROOT project is excluded from the PATH form. Its two layouts
        //     already coincide, so there is no defect to fix — and routing it here
        //     would change what "return home by path" does to the guide ledger,
        //     which is a far commoner call than this bug. The bare-id form still
        //     accepts it, exactly as before.
        //   * A FOREIGN repo cannot match, being no member of the loaded workspace,
        //     so the browse-an-excursion semantics the read-only hint assumes are
        //     untouched.
        //   * `read_only` does not move: `Agent::activate` and
        //     `activate_within_workspace` both derive it from the one rule in
        //     `AgentInner::resolve_read_only` — an explicit request wins at
        //     either root, otherwise home is rw and a foreign root is ro.
        //
        // docs/issues/archive/2026-08-27-activate-by-path-bypasses-workspace-memory-resolution.md
        let focus_target: Option<String> = {
            let inner = ctx.agent.inner.read().await;
            inner.default_workspace().and_then(|ws| {
                let by_id = !path.contains('/') && !path.contains('\\');
                let target = if by_id {
                    None
                } else {
                    Some(std::fs::canonicalize(path).ok()?)
                };
                ws.projects
                    .iter()
                    .filter(|p| by_id || p.discovered.relative_root != std::path::Path::new("."))
                    .find(|p| match &target {
                        None => p.discovered.id == path,
                        Some(target) => {
                            std::fs::canonicalize(ws.root.join(&p.discovered.relative_root))
                                .map(|abs| abs == *target)
                                .unwrap_or(false)
                        }
                    })
                    .map(|p| p.discovered.id.clone())
            })
        };
        if let Some(project_id) = focus_target {
            ctx.agent
                .activate_within_workspace(&project_id, read_only)
                .await?;
            let scenario = if ctx.agent.is_home().await {
                HintScenario::ReturnToHome
            } else {
                HintScenario::SwitchAway
            };
            let project_root = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await?;
            let prewarm_langs = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok(p.config.project.languages.clone())
                })
                .await
                .unwrap_or_default();
            crate::lsp::prewarm_lsp_background(
                ctx.lsp.clone(),
                project_root.clone(),
                &prewarm_langs,
            );
            let auto_registered =
                crate::library::auto_register::auto_register_deps(&project_root, ctx).await;
            return build_activation_response(ctx, scenario, &auto_registered).await;
        }

        // Full-activation path
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("path '{}' is not a directory", path),
                "Provide an absolute path to an existing directory.",
            )
            .into());
        }
        let root = root.canonicalize().unwrap_or(root);

        // Re-arm the guide ledger BEFORE `Agent::activate` below mutates
        // `default_workspace_root`, or this comparison always reads "same
        // project" and the whole feature is inert. Both sides are already
        // canonical (see `Agent::activate`'s doc comment) — no redundant
        // `canonicalize()` here.
        let switched = {
            let inner = ctx.agent.inner.read().await;
            inner.default_workspace_root.as_deref() != Some(root.as_path())
        };
        {
            let mut led = ctx.guide_hints_emitted.lock();
            if led.rendezvous_active() {
                // A companion hook is reporting in, so a `/clear` is visible to us
                // via the rendezvous poll elsewhere — safe to re-arm only the
                // project-scoped topic on a genuine switch, leaving the
                // tool-contract guides the model already holds untouched.
                if switched {
                    led.re_arm(PROJECT_SCOPED);
                }
            } else {
                // No rendezvous ⇒ a `/clear` is invisible to this server ⇒ surgical
                // re-arming could starve a new conversation that kept the old
                // session key. Keep the blunt, always-safe behaviour.
                led.clear();
            }
        }

        let had_home = ctx.agent.home_root().await.is_some();
        let mut timer = crate::perf::PhaseTimer::start("activate_project");

        ctx.agent.activate(root.clone(), read_only).await?;
        timer.lap("agent_activate");

        let prewarm_langs = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok(p.config.project.languages.clone())
            })
            .await
            .unwrap_or_default();
        crate::lsp::prewarm_lsp_background(ctx.lsp.clone(), root.clone(), &prewarm_langs);

        let scenario = if !had_home {
            HintScenario::FirstActivation
        } else if ctx.agent.is_home().await {
            HintScenario::ReturnToHome
        } else {
            HintScenario::SwitchAway
        };

        let concurrent_warning = ctx.agent.note_activation(&root).await;
        timer.lap("note_activation");
        let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
        timer.lap("auto_register_deps");
        let mut resp = build_activation_response(ctx, scenario, &auto_registered).await?;
        timer.lap("build_response");
        if let Some(w) = concurrent_warning {
            if let Some(obj) = resp.as_object_mut() {
                obj.insert("concurrent_activation_warning".to_string(), json!(w));
            }
        }
        timer.finish();
        Ok(resp)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_activate_project(result))
    }
}

#[async_trait::async_trait]
impl Tool for ProjectStatus {
    fn name(&self) -> &str {
        "project_status"
    }

    fn description(&self) -> &str {
        "Active project state: languages, embedding model, index health summary, and memory staleness. \
         Pass post_compact=true after context compaction to flush stale LSP position caches — \
         clients restart lazily on the next LSP call. \
         Call index_status() for detailed index info and live progress."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "post_compact": {
                    "type": "boolean",
                    "description": "Set true after context compaction to flush stale LSP position caches. \
                                    LSP clients restart lazily on the next navigation call."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::agent::IndexingState;

        // --- PostCompact cache flush ---
        if parse_bool_param(&input["post_compact"]) {
            ctx.lsp.shutdown_all().await;
            // Re-arm guide hints: compaction summarized the guide bodies out
            // of context, so allow them to re-inject. A bare /mcp restart
            // re-arms the session-opening guide only, on any non-empty
            // reloaded ledger (see `CodeScoutServer::from_parts_with_env`,
            // src/server.rs) — every other topic survives a restart.
            // Compaction clears everything. See
            // docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md.
            ctx.guide_hints_emitted.lock().clear();
            tracing::info!("PostCompact: flushed all LSP clients; they will restart lazily.");
            return Ok(json!({
                "flushed": true,
                "hint": "LSP position caches cleared. Clients restart on the next navigation call \
                         (symbol_at, references), which pays the language-server start unless another \
                         session in this workspace is already holding it warm — the server is shared \
                         per workspace, not per session. If that first call stalls or times out, re-run it."
            }));
        }

        // --- Essential config + library section ---
        let (root, languages, embeddings_model, lib_count, lib_indexed) = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                let lib_count = p.library_registry.all().len();
                let lib_indexed = p
                    .library_registry
                    .all()
                    .iter()
                    .filter(|e| e.indexed)
                    .count();
                Ok((
                    p.root.clone(),
                    p.config.project.languages.clone(),
                    p.config.embeddings.model.clone(),
                    lib_count,
                    lib_indexed,
                ))
            })
            .await?;

        let mut result = json!({
            "project_root": to_forward_slash(&root),
            "languages": languages,
            "embeddings_model": embeddings_model,
            "libraries": { "count": lib_count, "indexed": lib_indexed },
        });

        // --- Serving-binary identity ---
        // Direction 3 of the zombie-server bug, at the only grain that is not
        // noise. The direction wanted a response to declare the build it came
        // from, and stalled on two design questions: what triggers it, and
        // whether "differs from the on-disk binary" is even the right predicate
        // — because a peer's rebuild makes every OTHER server stale while its
        // answers stay correct, so a per-response warning would cry wolf
        // continuously in exactly the multi-session case it exists to serve.
        // Measured on this host 2026-08-28: six of eight servers were running
        // unlinked binaries and answering fine.
        //
        // Reporting it on `status` dodges both questions rather than answering
        // them. There is no trigger to choose — the caller explicitly asked what
        // state it is in — and nothing cries wolf, because this is reported
        // state, not a warning. Unconditional, for the same reason: a field that
        // appears only when something is wrong cannot be used to confirm that
        // things are right, and "which build answered me?" is a question worth
        // being able to ask on a healthy day.
        //
        // Same four facts `write_index_state_with_dirty` stamps into the
        // sidecar, from the same constructor, so what ANSWERED and what WROTE
        // are directly comparable without a /proc walk on either side.
        // docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md
        {
            let w = crate::retrieval::index_state::current_writer();
            result["server"] = json!({
                "git_sha": w.git_sha,
                "git_dirty": w.git_dirty,
                "pid": w.pid,
                "exe_deleted": w.exe_deleted,
            });
        }

        // --- Embedding backend section ---
        // Names the backend the *effective* config would resolve to right
        // now, plus which backends this binary was actually compiled with —
        // so a lean build stops silently disagreeing with a config that
        // names a backend it cannot load. `RetrievalConfig::from_env_and_project`
        // only reads env + project.toml (no network), same as the existing
        // Index section's `RetrievalClient::from_env` a few lines below.
        let retrieval_config =
            crate::retrieval::config::RetrievalConfig::from_env_and_project(Some(&root))?;

        let mut compiled_in = Vec::new();
        if cfg!(feature = "remote-embed") {
            compiled_in.push("remote");
        }
        if cfg!(any(
            feature = "local-embed",
            feature = "local-embed-dynamic"
        )) {
            compiled_in.push("local-onnx");
        }
        // Route through `backend_is_local` — the single source of truth
        // `dense_only` and `guard_sparse` also read (`client.rs`) —
        // rather than re-deriving "is this local" from which backends happen
        // to be compiled in. The old rule ("url set? remote-http :
        // (local-onnx compiled in? local-onnx : unavailable)") never looked at
        // the model string at all, so an `ollama:`/`openai:` model with no url
        // was misreported as "local-onnx" whenever `local-embed` happened to
        // be compiled in (it is never local, regardless of that), and as
        // "unavailable" when it wasn't (it works fine over the network
        // either way).
        let names_remote =
            crate::retrieval::client::model_names_remote_backend(&retrieval_config.model);
        let backend = if retrieval_config.embedder_url.is_some() {
            "remote-http"
        } else if crate::retrieval::client::RetrievalClient::backend_is_local(&retrieval_config) {
            if compiled_in.contains(&"local-onnx") {
                "local-onnx"
            } else {
                // The configured model genuinely names a local backend this
                // binary cannot load. Saying so here is the whole point of
                // this block.
                "unavailable"
            }
        } else if names_remote && !compiled_in.contains(&"remote") {
            // Mirror of the local arm above, for the other direction. The model
            // names `ollama:`/`openai:`, but the arms that build those are
            // `remote-embed`-gated (a *default* feature, so this only happens on
            // a `--no-default-features` build) and this binary lacks them:
            // `create_embedder_with_config` has no compiled path and bails with
            // "Unknown model". Reporting "remote-http" claimed a working network
            // config for one that cannot build anything.
            // docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md
            "unavailable"
        } else {
            "remote-http"
        };

        result["embedding_backend"] = json!(backend);
        result["embedding_compiled_in"] = json!(compiled_in);
        if backend == "unavailable" {
            // Two ways to be unavailable, and the actionable advice differs —
            // telling someone with an `ollama:` model to rebuild with
            // --features local-embed would send them to the wrong backend.
            result["embedding_hint"] = json!(if names_remote {
                "This binary has no remote embedding backend compiled in, but the \
                 configured model names one. Rebuild without --no-default-features \
                 (or with --features remote-embed), or point [embeddings].model at \
                 local:<model> and rebuild with --features local-embed."
            } else {
                "This binary has no local embedding backend compiled in, but the \
                 configured model names one. Rebuild with --features local-embed, \
                 or set [embeddings].url to an OpenAI-compatible endpoint."
            });
        }

        // --- Index section ---
        // Running state takes priority over DB stats.
        let indexing_state = ctx
            .agent
            .indexing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if let IndexingState::Running {
            done,
            total,
            eta_secs,
        } = indexing_state
        {
            result["index"] = json!({
                "status": "running",
                "done": done,
                "total": total,
                "eta_secs": eta_secs,
                "hint": "Call index_status() for detailed breakdown.",
            });
        } else {
            // Resolve project_id + ask Qdrant for stats. When the retrieval
            // stack is offline or the project has no chunks indexed, fall
            // through to the same "not_indexed" envelope the legacy sqlite
            // path returned.
            let project_id = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok(p.project_id().to_string())
                })
                .await?;
            let qdrant_stats =
                match crate::retrieval::client::RetrievalClient::from_env(Some(&root)).await {
                    Ok(client) => {
                        let coll = client.config.collection("code_chunks");
                        client.project_index_stats(&coll, &project_id).await.ok()
                    }
                    Err(_) => None,
                };
            match qdrant_stats {
                Some((chunks, files)) if chunks > 0 => {
                    // `chunks > 0` answers "is there an index", never "is it
                    // current". Let the git-sync state name the state instead of
                    // asserting the strong one from a non-emptiness check.
                    result["index"] = index_envelope(
                        chunks,
                        files,
                        crate::retrieval::index_state::git_sync_status(&root),
                    );
                }
                _ => {
                    result["index"] = json!({
                        "status": "not_indexed",
                        "hint": "Run index(action='build') to build the index.",
                    });
                }
            }
        }

        // --- Memory staleness section ---
        let staleness_result = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                let memories_dir = p.root.join(".codescout").join("memories");
                crate::memory::anchors::check_all_memories(&p.root, &memories_dir)
            })
            .await;
        match staleness_result {
            Ok(staleness) => {
                result["memory_staleness"] = staleness;
            }
            Err(e) => {
                tracing::debug!("memory staleness check failed: {e}");
            }
        }

        // --- Workspace section ---
        // `projects` reports the LIVE, discovered workspace members — same source
        // `activate`'s workspace table already uses (`Agent::workspace_summary`) —
        // not just the declared `[[project]]` entries. A sub-project codescout
        // finds by manifest walk needs no declaration, so the declared array alone
        // used to under-report: a valid id `memory`/`symbols`/`activate` all accept
        // could be absent from the one surface documented as how to list them.
        // `depends_on` is the one field discovery cannot supply (nothing on disk
        // states a dependency edge), so it is still looked up from the declared
        // config by id, same as `Agent::workspace_summary`.
        // docs/issues/archive/2026-08-26-list-projects-reports-declared-projects-not-workspace-members.md
        let live_projects = ctx.agent.discovered_projects().await;
        let workspace_toml_path = crate::config::workspace::workspace_config_path(&root);
        let workspace_info = if workspace_toml_path.exists() {
            std::fs::read_to_string(&workspace_toml_path)
                .ok()
                .and_then(|s| toml::from_str::<crate::config::workspace::WorkspaceConfig>(&s).ok())
                .map(|ws| {
                    json!({
                        "name": ws.workspace.name,
                        "projects": live_projects.iter().map(|p| {
                            let depends_on = ws.projects.iter()
                                .find(|e| e.id == p.id)
                                .map(|e| e.depends_on.clone())
                                .unwrap_or_default();
                            json!({
                                "id": p.id,
                                "root": to_forward_slash(&p.relative_root),
                                "languages": p.languages,
                                "depends_on": depends_on,
                            })
                        }).collect::<Vec<_>>(),
                        "resources": {
                            "max_lsp_clients": ws.resources.max_lsp_clients,
                            "idle_timeout_secs": ws.resources.idle_timeout_secs,
                        },
                    })
                })
        } else {
            None
        };
        result["workspace"] = json!(workspace_info);

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_project_status(result))
    }
}

/// Determines the hint text shown after activation.
enum HintScenario {
    /// First-ever activation (home project, session start)
    FirstActivation,
    /// Returning to the home project after visiting another
    ReturnToHome,
    /// Switching to a non-home project
    SwitchAway,
}

/// Opt-in: append the per-call `workspace=` pin to the switch-away hint.
///
/// Gated because it is the intervention under measurement in hamsa A-29, and the arms
/// have to differ in server behaviour without differing in binary — an env switch keeps
/// the comparison controlled where a rebuild between arms would not.
/// Default OFF preserves today's behaviour, which is arms A/B.
fn workspace_pin_notice_enabled() -> bool {
    pin_notice_enabled_from(
        std::env::var("CODESCOUT_WORKSPACE_PIN_NOTICE")
            .ok()
            .as_deref(),
    )
}

/// The pure half of the gate, so tests never touch real process env.
///
/// `docs/conventions/test-env-isolation.md` retired `EnvGuard` + `#[serial]` crate-wide:
/// it cannot coordinate with non-serial tests elsewhere that read the same var. Reading
/// env at the edge and testing the parse in isolation is the pattern that replaced it.
fn pin_notice_enabled_from(raw: Option<&str>) -> bool {
    matches!(raw, Some("1") | Some("true"))
}

/// The switch-away hint, rebuilt to teach the per-call pin. hamsa A-29.
///
/// **Conditions both instructions instead of appending one after the other.** The first
/// version appended the pin advice to the existing hint, producing adjacent sentences
/// that read "remember to `workspace(action='activate', …)` when done" and then "do not
/// activate" — a flat contradiction, caught by reading the real composed string out of a
/// failing test rather than by reading the source. Since A-26's finding is that competing
/// instructions are exactly what decides whether guidance lands, shipping that into an
/// arm would have measured a muddled instruction rather than the intended one. Each
/// clause now carries the condition under which it applies.
///
/// **Contrastive by design, not descriptive.** A-26 measured that naming a tool in a
/// routing line does not displace a strong competing prior; what moved its number was
/// explicitly contrasting the two and naming the wrong one. The prior here is the restore
/// instruction this text replaces, which normalises activate-then-restore as *the*
/// pattern.
///
/// The scope claim is the load-bearing half: `Agent::activate` mutates a single shared
/// project (`activate_replaces_previous_project`), so an agent that does not know
/// activation is server-global has no way to infer that it just clobbered a peer.
fn workspace_pin_contrast(prefix: &str, project_root: &str, home: &str) -> String {
    format!(
        "{prefix} — activating is server-global: it replaced the active project for \
         every session on this server, not just yours. If other agents are working \
         concurrently, do not activate — pass workspace=\"{project_root}\" on each call \
         instead, which scopes only your own calls. If you are the only agent here, \
         remember to workspace(action='activate', path=\"{home}\") when done."
    )
}

/// Best-effort Qdrant probe: does this project have any chunks indexed?
///
/// Returns `false` when the retrieval stack is offline or the probe fails.
/// Used by `build_activation_response` to populate the `index.status` field —
/// callers treat `false` as "not indexed" and surface a build hint.
///
/// Asks `project_has_chunks`, NOT `project_index_stats`. The latter enumerates
/// every chunk in the project to count distinct files, which cannot finish inside
/// `FIRST_PROBE_TIMEOUT` on a real corpus — so this reported every large project as
/// unindexed and, because a timeout is deliberately not cached, repeated the whole
/// scan on every activation. See
/// `docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`.
async fn check_has_index(project_id: &str, project_root: &std::path::Path) -> bool {
    match crate::retrieval::client::RetrievalClient::from_env(Some(project_root)).await {
        Ok(client) => {
            let coll = client.config.collection("code_chunks");
            client
                .project_has_chunks(&coll, project_id)
                .await
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}

/// Session-scoped last-known index status per project id. Avoids a vector-store
/// round-trip on every activation: `index.status` is a hint field where
/// one-activation staleness is acceptable, a per-activation network probe is not.
static INDEX_STATUS_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, bool>>,
> = std::sync::OnceLock::new();

/// A slow or hung vector store must not stall the first activation. Sized for a
/// cold retrieval stack: the prior 500ms bound frequently timed out on the first
/// activation after a restart/sync. A timeout is NOT cached (see
/// `resolve_first_probe`), so a rare slow probe self-corrects on the next
/// activation instead of poisoning the session cache with a false `not_indexed`.
const FIRST_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

fn index_status_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    INDEX_STATUS_CACHE.get_or_init(Default::default)
}

fn index_status_get(project_id: &str) -> Option<bool> {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(project_id)
        .copied()
}

fn index_status_put(project_id: &str, has_index: bool) {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(project_id.to_string(), has_index);
}

#[cfg(test)]
fn index_status_remove(project_id: &str) {
    index_status_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(project_id);
}
/// Decide the cached index-status value from a first-probe outcome.
/// `Some(v)` = the probe completed with result `v`; `None` = it timed out.
fn resolve_first_probe(project_id: &str, probe: Option<bool>) -> bool {
    match probe {
        // Completed probe → cache the definitive result.
        Some(v) => {
            index_status_put(project_id, v);
            v
        }
        // Timed out → report false for this call but do NOT cache it, so the
        // next activation re-probes instead of serving a poisoned negative.
        None => false,
    }
}

/// At most one detached refresh per project id at a time (thundering-herd guard).
static REFRESH_IN_FLIGHT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

/// Generous bound for the detached refresh — accumulation, not latency, is the risk.
const BACKGROUND_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn refresh_begin(project_id: &str) -> bool {
    REFRESH_IN_FLIGHT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(project_id.to_string())
}

fn refresh_end(project_id: &str) {
    REFRESH_IN_FLIGHT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(project_id);
}

/// Cached wrapper: cache hit returns immediately and refreshes in a detached
/// task; first-per-session probe is bounded by `FIRST_PROBE_TIMEOUT`
/// (timeout => `false`, corrected by the background refresh on the next
/// activation).
async fn check_has_index_cached(project_id: &str, project_root: &std::path::Path) -> bool {
    if let Some(cached) = index_status_get(project_id) {
        if refresh_begin(project_id) {
            let pid = project_id.to_string();
            let root = project_root.to_path_buf();
            tokio::spawn(async move {
                if let Ok(fresh) =
                    tokio::time::timeout(BACKGROUND_REFRESH_TIMEOUT, check_has_index(&pid, &root))
                        .await
                {
                    index_status_put(&pid, fresh);
                }
                refresh_end(&pid);
            });
        }
        return cached;
    }
    let probe = tokio::time::timeout(
        FIRST_PROBE_TIMEOUT,
        check_has_index(project_id, project_root),
    )
    .await
    .ok();
    resolve_first_probe(project_id, probe)
}

/// Build the activation response JSON for both full-activation and focus-switch paths.
async fn build_activation_response(
    ctx: &ToolContext,
    scenario: HintScenario,
    auto_registered: &[crate::library::auto_register::RegisteredDep],
) -> anyhow::Result<Value> {
    let mut timer = crate::perf::PhaseTimer::start("activation_response");
    let (
        project_name,
        project_root_str,
        project_root_path,
        languages,
        read_only,
        memories,
        security_profile,
        stored_onboarding_version,
    ) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let memories = p.memory.list().unwrap_or_default();
            // Only carry the security profile out of the project lock; it is
            // surfaced later only when it departs from the sandboxed default.
            let security_profile = if !p.read_only {
                Some(p.config.security.profile)
            } else {
                None
            };
            Ok((
                p.config.project.name.clone(),
                to_forward_slash(&p.root),
                p.root.clone(),
                p.config.project.languages.clone(),
                p.read_only,
                memories,
                security_profile,
                p.config.project.onboarding_version,
            ))
        })
        .await?;

    // `p.memory`'s directory depends on which activation path ran: the bare-id
    // focus-switch resolves it through `Workspace::memory_dir_for_project`
    // (`agent/mod.rs`), while `Agent::new` and `load_project_resources` open it
    // project-local via `MemoryStore::open(root)`. Reporting whichever one that
    // happened to be advertised a count the `memory` tool could not always serve —
    // activate said 12, `memory(action="list")` said 0, same project, same instant.
    //
    // Report the union of both layouts, which is the same set
    // `memory(action="list")` now returns, so the two surfaces agree by
    // construction rather than by both paths happening to pick the same directory.
    // docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
    let memories = crate::tools::memory::union_with_workspace_memories(ctx, memories).await;

    timer.lap("project_snapshot");

    // has_index probe via Qdrant — best-effort. When the retrieval stack is
    // offline (common in tests), report false rather than erroring out the
    // activation response.
    let has_index = check_has_index_cached(&project_name, &project_root_path).await;
    timer.lap("check_has_index");

    let version_stale = onboarding_version_stale(stored_onboarding_version);

    let index = if has_index {
        json!({"status": "indexed"})
    } else {
        json!({"status": "not_indexed", "hint": "Run index(action='build') to enable semantic_search."})
    };

    let workspace = ctx.agent.workspace_summary().await;
    timer.lap("workspace_summary");
    let workspace_json = workspace.as_ref().map(|projects| {
        projects
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "root": p.root,
                    "languages": p.languages,
                    "depends_on": p.depends_on,
                })
            })
            .collect::<Vec<_>>()
    });

    let home_root = ctx.agent.home_root().await;
    let hint = match scenario {
        HintScenario::FirstActivation => format!(
            "CWD: {}. Run workspace(action='status') for health checks and memory staleness.",
            project_root_str
        ),
        HintScenario::ReturnToHome => format!(
            "Returned to home project. CWD: {}. Run workspace(action='status') to check memory staleness.",
            project_root_str
        ),
        HintScenario::SwitchAway if read_only => {
            let home_str = home_root
                .as_ref()
                .map(|p| to_forward_slash(p))
                .unwrap_or_default();
            if workspace_pin_notice_enabled() {
                workspace_pin_contrast(
                    &format!("Browsing {} (read-only). CWD: {}", project_name, project_root_str),
                    &project_root_str,
                    &home_str,
                )
            } else {
                format!(
                    "Browsing {} (read-only). CWD: {} — remember to workspace(action='activate', path=\"{}\") when done.",
                    project_name, project_root_str, home_str,
                )
            }
        }
        HintScenario::SwitchAway => {
            let home_str = home_root
                .as_ref()
                .map(|p| to_forward_slash(p))
                .unwrap_or_default();
            if workspace_pin_notice_enabled() {
                workspace_pin_contrast(
                    &format!("Switched project (read-write). CWD: {}", project_root_str),
                    &project_root_str,
                    &home_str,
                )
            } else {
                format!(
                    "Switched project (read-write). CWD: {} — remember to workspace(action='activate', path=\"{}\") when done.",
                    project_root_str, home_str,
                )
            }
        }
    };

    // Manifest-derived hints so agents that never call `onboarding` still have
    // project context (primary language, entry points, build commands). When
    // an `onboarding` memory is present these are redundant but cheap enough
    // to always include.
    let hints =
        crate::mcp_resources::project_hints::probe_project_hints(&project_root_path, &memories);
    timer.lap("probe_project_hints");

    // Legacy sqlite memory db detection — surfaces a one-line migration hint
    // when `.codescout/embeddings/project.db` exists on disk. This file is
    // produced by the pre-Qdrant codescout (`embed::index`); after a
    // successful `codescout migrate-memories` run + manual delete the field
    // drops out of subsequent activations.
    let legacy_db_path = project_root_path.join(".codescout/embeddings/project.db");
    let legacy_db_present = legacy_db_path.exists();

    let mut result = json!({
        "status": "ok",
        "project": project_name,
        "project_root": project_root_str,
        "read_only": read_only,
        "languages": languages,
        "index": index,
        "memories": memories,
        "project_hints": hints,
        "hint": hint,
    });

    if legacy_db_present {
        result["legacy_semantic_index"] = json!({
            "path": to_forward_slash(&legacy_db_path),
            "hint": "Run `codescout migrate-memories` to port memories to Qdrant, then delete this file.",
        });
    }

    if let Some(ws) = workspace_json {
        result["workspace"] = json!(ws);
    }

    // Activating a linked worktree silently changes TWO things versus the main
    // checkout, in opposite directions, and neither used to say so:
    //
    //   * `.codescout/memories/` IS git-tracked, so the worktree serves its own
    //     commit's memory set — a memory written on main afterwards is simply
    //     absent. Not corruption; git working as specified. But an agent reads
    //     the short list and concludes the fact was never recorded.
    //   * `.codescout/workspace.toml` is NOT tracked (.gitignore), so it is
    //     absent here. Note the precise mechanism: discovery is ALWAYS the
    //     manifest walk — there is no second mode to "fall back" to. What the
    //     missing file removes is `exclude_projects` and `discovery_max_depth`,
    //     so the walk stops pruning. In this repo that alone took the
    //     sub-project count from 2 to 9 (every `tests/fixtures/*`), because the
    //     only non-default setting is `exclude_projects = ["fixtures"]`.
    //
    //     `load_discover_settings` now reads through to the MAIN checkout when
    //     the worktree has no file of its own, so this half no longer diverges
    //     by default — but it still can, when neither location has one. The
    //     three topology states below distinguish those cases; do not collapse
    //     them back into present/absent, which is what made the old hint assert
    //     "ran with defaults" about a walk that had in fact inherited main's.
    //
    // Reporting the divergence is deliberately all this does. Whether a worktree
    // SHOULD share main's memories and topology is a separate, still-open
    // semantic question; saying so out loud does not foreclose either answer.
    // docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md
    if crate::util::path_security::is_linked_worktree(&project_root_path) {
        let mut notice = json!({
            "main_root": crate::util::path_security::worktree_main_root(&project_root_path)
                .map(|p| to_forward_slash(&p)),
            "memories_are_this_checkouts": format!(
                "{} memory topics come from THIS worktree's commit. A memory written \
                 on the main checkout after it was created does not exist here.",
                memories.len()
            ),
        });
        let ws_toml = crate::config::workspace::workspace_config_path(&project_root_path);
        let main_has_ws = crate::util::path_security::worktree_main_root(&project_root_path)
            .map(|m| crate::config::workspace::workspace_config_path(&m))
            .is_some_and(|p| p.exists());
        if ws_toml.exists() {
            notice["topology"] = json!("configured");
        } else if main_has_ws {
            notice["topology"] = json!("inherited");
            notice["topology_hint"] = json!(
                "No .codescout/workspace.toml here (it is gitignored, so it does not \
                 travel into a worktree), so sub-project discovery read the MAIN \
                 checkout's settings instead. exclude_projects and \
                 discovery_max_depth match it, and so does the project list."
            );
        } else {
            notice["topology"] = json!("inferred");
            notice["topology_hint"] = json!(
                "Neither this worktree nor its main checkout has a \
                 .codescout/workspace.toml, so sub-project discovery ran with \
                 defaults — no exclude_projects, depth 3. The project list is \
                 auto-detected, not declared."
            );
        }
        result["worktree"] = notice;
    }

    // Surface `security_profile` only when it departs from the sandboxed
    // `default` — `root` disables every path/command gate, which is worth
    // flagging on activation. `shell_enabled` is intentionally omitted: it has
    // defaulted to true for all projects, so reporting it carried no signal.
    if let Some(profile) = security_profile {
        if profile != crate::util::path_security::SecurityProfile::Default {
            result["security_profile"] = json!(profile);
        }
    }

    if !auto_registered.is_empty() {
        let without_source = auto_registered
            .iter()
            .filter(|r| !r.source_available)
            .count();
        result["auto_registered_libs"] = json!({
            "count": auto_registered.len(),
            "without_source": without_source,
        });
    }

    // stored > ONBOARDING_VERSION (downgrade scenario) intentionally treated as current by onboarding_version_stale
    if version_stale {
        result["system_prompt_stale"] = json!({
            "stored_version": stored_onboarding_version,
            "current_version": ONBOARDING_VERSION,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        });
    }

    timer.finish();
    Ok(result)
}

fn format_activate_project(result: &Value) -> String {
    let name = result["project"].as_str().unwrap_or("?");
    let ro = result["read_only"].as_bool().unwrap_or(true);
    let mode = if ro { "ro" } else { "rw" };
    let mem_count = result["memories"].as_array().map(|a| a.len()).unwrap_or(0);
    let index_status = result["index"]["status"].as_str().unwrap_or("unknown");

    let mut parts = vec![format!(
        "activated · {name} ({mode}) · {mem_count} memories · index: {index_status}"
    )];

    if let Some(ws) = result["workspace"].as_array() {
        parts.push(format!("{} workspace projects", ws.len()));
    }

    // A worktree activation is the one case where the memory count and the
    // project count above describe a DIFFERENT tree than the caller may
    // assume. Say so on the summary line, not only in the JSON — the
    // compact form is what most callers actually read.
    if let Some(wt) = result["worktree"].as_object() {
        parts.push(match wt.get("topology").and_then(|v| v.as_str()) {
            Some("inferred") => {
                "linked worktree · memories + topology are this checkout's (topology inferred)"
                    .to_string()
            }
            Some("inherited") => {
                "linked worktree · memories are this checkout's · topology inherited from main"
                    .to_string()
            }
            _ => "linked worktree · memories are this checkout's".to_string(),
        });
    }

    if let Some(libs) = result["auto_registered_libs"].as_object() {
        let count = libs.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let without = libs
            .get("without_source")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if without > 0 {
            parts.push(format!(
                "auto-registered {} libs ({} without source)",
                count, without
            ));
        } else {
            parts.push(format!("auto-registered {} libs", count));
        }
    }

    let body = parts.join(" · ");

    // Prepend severity-ordered banners. System prompt staleness is higher
    // priority than the legacy-index hint — show both when both fire.
    let legacy_banner = if result["legacy_semantic_index"].is_object() {
        Some("⚠ LEGACY INDEX: run `codescout migrate-memories` to port memories to Qdrant.")
    } else {
        None
    };

    if let Some(stale) = result["system_prompt_stale"].as_object() {
        let stored_label = match stale.get("stored_version").and_then(|v| v.as_u64()) {
            Some(v) => format!("v{v}"),
            None => "none".to_string(),
        };
        let current = stale
            .get("current_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut out = format!(
            "⚠ SYSTEM PROMPT STALE ({stored_label} → v{current}): run onboarding(action=\"refresh_prompt\") now."
        );
        if let Some(b) = legacy_banner {
            out.push('\n');
            out.push_str(b);
        }
        out.push('\n');
        out.push_str(&body);
        out
    } else if let Some(b) = legacy_banner {
        format!("{b}\n{body}")
    } else {
        body
    }
}

/// Build the `index` envelope for a project whose collection HOLDS chunks.
///
/// Split out of `ProjectStatus/call` because this mapping *is* the defect it
/// exists to prevent: the shipped code hardcoded `"up_to_date"` on `chunks > 0`
/// alone — a statement about non-emptiness wearing the name of a statement about
/// currency — and, sitting behind a live Qdrant call, no test could reach it. An
/// index 286 commits behind HEAD reported as up to date
/// (`docs/issues/archive/2026-08-31-workspace-status-claims-up-to-date-without-checking-git-sync.md`).
///
/// **Three states, because three exist.** `git_sync_status` returns `None` when
/// freshness is *indeterminate* — a non-git root, no sidecar, an unreadable HEAD
/// — which is not the same as known-current. The honest word there is `indexed`,
/// the one `build_activation_response` already uses for "a usable index exists";
/// defaulting to the strong claim is how the original went wrong. Note the
/// distance is omitted rather than zeroed in that arm: a `behind_commits: 0` we
/// never measured is indistinguishable from one we did.
///
/// The status vocabulary is `index_state.rs`'s verbatim, so the two surfaces
/// cannot drift apart into different words for one state again.
fn index_envelope(chunks: usize, files: usize, git_sync: Option<Value>) -> Value {
    const DETAILS: &str = "Call index(action='status') for full Qdrant collection details.";

    let mut env = json!({ "files": files, "chunks": chunks });

    let Some(sync) = git_sync else {
        env["status"] = json!("indexed");
        env["hint"] = json!(DETAILS);
        return env;
    };

    // Emit git_sync's own word rather than re-deriving one. `indexed` is the
    // fallback for a shape we do not recognise, never `up_to_date`.
    let status = sync["status"].as_str().unwrap_or("indexed");
    env["status"] = json!(status);
    for key in ["behind_commits", "last_indexed_commit", "head_commit"] {
        if let Some(v) = sync.get(key) {
            env[key] = v.clone();
        }
    }

    // A stale index's hint must name the call that FIXES it. The shipped one
    // named `status`, which only re-reports the staleness.
    env["hint"] = if status == "behind" {
        json!("Index is behind HEAD; run index(action='build') to catch up.")
    } else {
        json!(DETAILS)
    };
    env
}

fn format_project_status(result: &Value) -> String {
    let root = result["project_root"].as_str().unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    let status = result["index"]["status"].as_str().unwrap_or("unknown");
    let index_str = match status {
        // `indexed` belongs here, not in the `_` arm: it is a POPULATED state
        // (freshness indeterminate), and falling through would print
        // `index:none` for an index that exists — the same class of false
        // report one layer down in `index_envelope`.
        "up_to_date" | "behind" | "indexed" => {
            let files = result["index"]["files"].as_u64().unwrap_or(0);
            let chunks = result["index"]["chunks"].as_u64().unwrap_or(0);
            format!("index:{files}f/{chunks}c ({status})")
        }
        "running" => {
            let done = result["index"]["done"].as_u64().unwrap_or(0);
            let total = result["index"]["total"].as_u64().unwrap_or(0);
            format!("index:running {done}/{total}")
        }
        _ => "index:none".to_string(),
    };
    format!("status · {name} · {index_str}")
}

#[cfg(test)]
mod tests;
