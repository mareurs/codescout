use anyhow::Result;
use serde_json::{json, Value};

use super::{OutputForm, Tool, ToolContext};
use crate::util::fs::to_forward_slash;

pub struct ListLibraries;

#[async_trait::async_trait]
impl Tool for ListLibraries {
    fn name(&self) -> &str {
        "list_libraries"
    }

    fn description(&self) -> &str {
        "List registered libraries and their index status. \
         Use scope='lib:<name>' in semantic_search, symbols, or index(action='build') to target a library. \
         Version staleness detection currently supports Cargo.lock (Rust) and package-lock.json (npm/Node); \
         Go, Python, and Yarn lockfiles are not yet tracked."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn call(&self, _input: Value, ctx: &ToolContext) -> Result<Value> {
        let libs: Vec<Value> = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |project| {
                Ok(project
                    .library_registry
                    .all()
                    .iter()
                    .map(|entry| {
                        let stale = entry.indexed
                            && entry.version.is_some()
                            && entry.version_indexed.is_some()
                            && entry.version != entry.version_indexed;
                        json!({
                            "name": entry.name,
                            "version": entry.version,
                            "version_indexed": entry.version_indexed,
                            "stale": stale,
                            "path": to_forward_slash(&entry.path),
                            "language": entry.language,
                            "discovered_via": entry.discovered_via,
                            "indexed": entry.indexed,
                            "source_available": entry.source_available,
                        })
                    })
                    .collect())
            })
            .await
            .map_err(|_| {
                super::RecoverableError::with_hint(
                    "No active project. Use workspace(action='activate') first.",
                    "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                )
            })?;

        Ok(json!({ "libraries": libs }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_list_libraries(result))
    }
}

pub struct RegisterLibrary;

#[async_trait::async_trait]
impl Tool for RegisterLibrary {
    fn name(&self) -> &str {
        "register_library"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Register an external library for searching with scope='lib:<name>'. \
         Auto-detects name and language from manifest files (Cargo.toml, package.json, etc.). \
         Use name/language params to override auto-detection."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the library root directory"
                },
                "name": {
                    "type": "string",
                    "description": "Library name (auto-detected from manifest if omitted)"
                },
                "language": {
                    "type": "string",
                    "description": "Primary language (auto-detected if omitted)"
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        // Not the shared `path` hint: this one is a library root. BL-3 Class B.
        let raw_path = super::require_str_param_or_hint(
            &input,
            "path",
            &[],
            "Pass the library's root directory — the sources to index, e.g. \
             path=\"/usr/lib/python3.12/site-packages/requests\". A directory, not a file, \
             and outside the active project.",
        )?;
        let lib_path = std::path::PathBuf::from(raw_path);

        if !lib_path.exists() {
            return Err(super::RecoverableError::with_hint(
                format!("Path does not exist: {}", lib_path.display()),
                "Provide an absolute path to an existing directory.",
            )
            .into());
        }
        if !lib_path.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("Path is not a directory: {}", lib_path.display()),
                "Provide a path to a directory, not a file.",
            )
            .into());
        }

        // Scope guard (phase-5 S2): reject registering the home directory,
        // its parent, or a system path like `/etc` / `/usr`. Without this,
        // a prompt-injected `library(action="register", path="/etc")` would let a later
        // `index_project(scope="lib:…")` walk and embed the entire directory,
        // leaking secrets back to the LLM via `semantic_search`.
        //
        // Canonicalize first so relative traversals (`../..`) and symlinks
        // cannot bypass the classifier.
        let canon_lib_path = std::fs::canonicalize(&lib_path).unwrap_or_else(|_| lib_path.clone());
        if let Some(reason) = crate::embed::preflight::classify_path(&canon_lib_path) {
            return Err(super::RecoverableError::with_hint(
                format!(
                    "refusing to register library at '{}': {:?}",
                    canon_lib_path.display(),
                    reason,
                ),
                "Register a library root under a specific package directory, \
                 not your home directory or a system path.",
            )
            .into());
        }

        // Auto-detect from manifest, with user overrides.
        // IMPORTANT: discover_library_root expects a *file* path and calls .parent()
        // to start searching. Passing a directory would skip the directory itself.
        // We pass a synthetic file path inside the directory to work around this.
        let discovered = crate::library::discovery::discover_library_root(&lib_path.join("_probe"));
        let name = input["name"]
            .as_str()
            .map(String::from)
            .or_else(|| discovered.as_ref().map(|d| d.name.clone()))
            .unwrap_or_else(|| {
                lib_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
        let language = input["language"]
            .as_str()
            .map(String::from)
            .or_else(|| discovered.as_ref().map(|d| d.language.clone()))
            .unwrap_or_else(|| "unknown".to_string());

        // Register and save — pinned to the resolved workspace (regime-3).
        ctx.agent
            .with_project_at_mut(ctx.workspace_override.as_deref(), |project| {
                project.library_registry.register(
                    name.clone(),
                    lib_path.clone(),
                    language.clone(),
                    crate::library::registry::DiscoveryMethod::Manual,
                    true,
                );
                let registry_path = project.root.join(".codescout").join("libraries.json");
                project.library_registry.save(&registry_path)?;
                Ok(())
            })
            .await?;

        Ok(json!({
            "status": "ok",
            "name": name,
            "language": language,
            "hint": format!(
                "Use scope='lib:{}' in symbols/semantic_search. \
                 Run index(action='build', scope='lib:{}') to enable semantic search.",
                name, name
            ),
        }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format!(
            "Registered library '{}' ({})",
            result["name"].as_str().unwrap_or("?"),
            result["language"].as_str().unwrap_or("?"),
        ))
    }
}

fn format_list_libraries(result: &Value) -> String {
    let libs = match result["libraries"].as_array() {
        Some(l) if !l.is_empty() => l,
        _ => return "0 libraries".to_string(),
    };
    let name_width = libs
        .iter()
        .filter_map(|l| l["name"].as_str())
        .map(|n| n.len())
        .max()
        .unwrap_or(0);
    let mut out = format!("{} libraries", libs.len());
    for lib in libs.iter() {
        let name = lib["name"].as_str().unwrap_or("?");
        let status = if lib["indexed"].as_bool().unwrap_or(false) {
            "indexed"
        } else {
            "not indexed"
        };
        let stale_marker = if lib["stale"].as_bool().unwrap_or(false) {
            " [stale]"
        } else {
            ""
        };
        out.push_str(&format!("\n  {name:<name_width$}  {status}{stale_marker}"));
    }
    out
}

pub struct Library;

#[async_trait::async_trait]
impl Tool for Library {
    fn name(&self) -> &str {
        "library"
    }

    fn is_write(&self, input: &Value) -> bool {
        input.get("action").and_then(Value::as_str) == Some("register")
    }

    fn description(&self) -> &str {
        "Library registry. Actions: \
         `list` (show registered libraries with index/version status), \
         `register` (add a library directory for cross-project search; \
         pass `path` and optional `name`/`language`)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "register"],
                    "description": "Operation to perform."
                },
                "path": {
                    "type": "string",
                    "description": "register: directory path of the library."
                },
                "name": {
                    "type": "string",
                    "description": "register: override the auto-detected library name."
                },
                "language": {
                    "type": "string",
                    "description": "register: override the auto-detected language."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                super::RecoverableError::with_hint(
                    "library requires 'action' parameter",
                    "Pass action='list' or action='register'.",
                )
            })?;
        match action {
            "list" => ListLibraries.call(input, ctx).await,
            "register" => RegisterLibrary.call(input, ctx).await,
            other => Err(super::RecoverableError::with_hint(
                format!("unknown library action: {}", other),
                "Valid actions: 'list', 'register'.",
            )
            .into()),
        }
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("libraries").is_some() {
            ListLibraries.format_compact(result)
        } else {
            RegisterLibrary.format_compact(result)
        }
    }

    /// `Always`, deliberately — this method is the fix for a circular gate.
    ///
    /// It used to delegate to `ListLibraries.availability(caps)`, i.e.
    /// `Availability::RequiresLibraries`, i.e. `caps.has_libraries`, which
    /// `current_capabilities` computes as *"at least one library is already registered for
    /// the active project"* (`src/server.rs`). But `library` is the **only** surface on which
    /// a library can be registered — `register` is one of its actions. So the tool that
    /// establishes the precondition was gated on that precondition: hidden from `tools/list`
    /// in exactly the state where you need it.
    ///
    /// **Measured 2026-09-01**, fresh git repo with no registered libraries: **15** tools
    /// advertised and `library` absent — yet `library(action="register", path=…)` dispatched
    /// normally and returned `Registered library 'codescout' (rust)`. The gate removed the
    /// capability from **discovery** without removing it from **dispatch**, which is the bad
    /// half of each: an agent reading the only discovery surface it has concludes the
    /// capability does not exist, while the capability sits there working.
    ///
    /// `auto_register_deps` (run on every project activation) masks this wherever a
    /// dependency manifest is detectable, which is why it survived — it fails exactly for the
    /// project that has no manifest and therefore has to register by hand.
    ///
    /// Do not re-gate this on `has_libraries`. A future gate must key on something
    /// **upstream** of registration, never on its outcome.
    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::Always
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use std::path::PathBuf;

    async fn project_ctx() -> ToolContext {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let agent = Agent::new(Some(root)).await.unwrap();
        // Leak the tempdir so it stays alive
        std::mem::forget(dir);
        ToolContext {
            agent,
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        }
    }

    fn project_ctx_with_agent(agent: Agent) -> ToolContext {
        ToolContext {
            agent,
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        }
    }

    #[tokio::test]
    async fn list_libraries_empty() {
        let ctx = project_ctx().await;
        let result = Library
            .call(json!({ "action": "list" }), &ctx)
            .await
            .unwrap();
        let libs = result["libraries"].as_array().unwrap();
        assert!(libs.is_empty());
    }

    #[tokio::test]
    async fn list_libraries_shows_registered() {
        let ctx = project_ctx().await;
        {
            let mut inner = ctx.agent.inner.write().await;
            let project = inner.active_project_mut().unwrap();
            project.library_registry.register(
                "serde".into(),
                PathBuf::from("/tmp/serde"),
                "rust".into(),
                crate::library::registry::DiscoveryMethod::Manual,
                true,
            );
        }
        let result = Library
            .call(json!({ "action": "list" }), &ctx)
            .await
            .unwrap();
        let libs = result["libraries"].as_array().unwrap();
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0]["name"], "serde");
        assert_eq!(libs[0]["indexed"], false);
    }

    #[tokio::test]
    async fn list_libraries_errors_without_project() {
        let agent = Agent::new(None).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };
        let result = Library.call(json!({ "action": "list" }), &ctx).await;
        assert!(result.is_err());
    }

    // --- format_list_libraries tests ---

    #[test]
    fn format_list_libraries_shows_names_and_status() {
        let result = serde_json::json!({
            "libraries": [
                {"name": "serde", "indexed": true},
                {"name": "tokio", "indexed": false}
            ]
        });
        let out = format_list_libraries(&result);
        assert!(
            out.contains("serde"),
            "should show library name, got: {out}"
        );
        assert!(
            out.contains("tokio"),
            "should show library name, got: {out}"
        );
        assert!(
            out.contains("indexed"),
            "should show index status, got: {out}"
        );
    }

    #[test]
    fn library_declares_output_form_text() {
        // Pinned wire contract: small `library` results (lib list with status)
        // render via the compact text form, not pretty JSON. format_list_libraries
        // is lossless (every lib + indexed/stale flags, no cap), so the small path
        // is safe to flip.
        use crate::tools::{OutputForm, Tool};
        assert_eq!(Library.output_form(), OutputForm::Text);
    }

    #[tokio::test]
    async fn index_project_scope_lib_errors_for_unknown() {
        let ctx = project_ctx().await;
        // Register nothing — querying an unknown lib name should return RecoverableError
        let tool = crate::tools::semantic::IndexProject;
        let result = tool.call(json!({ "scope": "lib:nonexistent" }), &ctx).await;
        assert!(result.is_err(), "expected error for unknown library");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("nonexistent") || msg.contains("not found"),
            "error should mention the library name: {msg}"
        );
    }

    // --- RegisterLibrary tests ---

    #[tokio::test]
    async fn register_library_manual() {
        let dir = tempfile::tempdir().unwrap();
        let lib_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            lib_dir.path().join("Cargo.toml"),
            "[package]\nname = \"mylib\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = project_ctx_with_agent(agent.clone());
        let result = Library
            .call(
                json!({
                    "action": "register",
                    "path": lib_dir.path().display().to_string(),
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["name"], "mylib");
        assert_eq!(result["language"], "rust");

        let reg = agent.library_registry().await.unwrap();
        assert_eq!(reg.all().len(), 1);
        assert_eq!(reg.all()[0].name, "mylib");
    }

    #[tokio::test]
    async fn register_library_with_explicit_name_and_language() {
        let dir = tempfile::tempdir().unwrap();
        let lib_dir = tempfile::tempdir().unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = project_ctx_with_agent(agent.clone());
        let result = Library
            .call(
                json!({
                    "action": "register",
                    "path": lib_dir.path().display().to_string(),
                    "name": "custom-name",
                    "language": "python",
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["name"], "custom-name");
        assert_eq!(result["language"], "python");
    }

    #[tokio::test]
    async fn register_library_fails_for_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = project_ctx_with_agent(agent);
        let result = Library
            .call(
                json!({
                    "action": "register",
                    "path": "/nonexistent/path/to/lib",
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn library_action_unknown_errors() {
        let ctx = project_ctx().await;
        let err = Library
            .call(json!({ "action": "wat" }), &ctx)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown library action"),
            "expected unknown action error, got: {err}"
        );
    }

    #[tokio::test]
    async fn library_action_missing_errors() {
        let ctx = project_ctx().await;
        let err = Library.call(json!({}), &ctx).await.unwrap_err();
        assert!(
            err.to_string().contains("library requires 'action'"),
            "expected missing action error, got: {err}"
        );
    }

    #[test]
    fn library_is_write_depends_on_action() {
        assert!(Library.is_write(&json!({ "action": "register" })));
        assert!(!Library.is_write(&json!({ "action": "list" })));
        assert!(!Library.is_write(&json!({})));
    }

    /// Site 5 for the `IC-15` param probe, and the first outside `src/librarian/` — see
    /// `crate::tools::param_probe` for why it compares two calls and what `accepts_any_json`
    /// admits.
    ///
    /// `library` sat on that probe's Owed list as one of three unprobed multi-action tools. Two
    /// things had to change first, both **measured 2026-09-02 rather than inferred from the
    /// list**:
    ///
    /// - Its three labelled keys read `For action='register': …`. `sweep` parses a label as
    ///   `desc.split(':').next()`, which yields `For action='register'` — matching no action, so
    ///   every key was skipped and the probe would have passed **having checked zero keys**.
    ///   Relabelled to `register: …`. `floor` catches that convention breaking wholesale; it
    ///   cannot catch one key losing its label, so this was invisible in the direction that
    ///   matters.
    /// - `name` and `language` are read `input["name"].as_str()` — an untyped accessor, where
    ///   every wrong type reads as *absent*. No ill-typed value exists for them, so the probe
    ///   cannot speak for either and they are declared in `accepts_any_json`: an admission, not
    ///   a pass. `path` goes through `require_str_param_or_hint`, which type-checks, so it is
    ///   genuinely probeable.
    ///
    /// **Hence the floor is 1, not 3, and 1 is the honest number.** A floor of 0 would have
    /// reproduced the zero-keys defect at this new site while the shared helper stayed correct —
    /// the reason `floor` is per-call-site rather than a constant inside `assert_all_honored`.
    #[tokio::test]
    async fn every_action_labelled_schema_key_is_honored_by_that_action() {
        use crate::tools::param_probe::{assert_all_honored, assert_required_are_advertised, Spec};

        fn required(action: &str) -> serde_json::Map<String, Value> {
            let mut m = serde_json::Map::new();
            if action == "register" {
                // Type-valid, and chosen to fail resolution *after* deserialisation: the path
                // does not exist, so the baseline dies in `RegisterLibrary`'s path check rather
                // than in serde, and a deser error is visibly different from it. Reusing the
                // literal from `register_library_fails_for_nonexistent_path`, which establishes
                // that this input does fail.
                m.insert("path".into(), json!("/nonexistent/path/to/lib"));
            }
            m
        }

        let spec = Spec {
            actions: &["list", "register"],
            accepts_any_json: &["name", "language"],
            required,
        };

        assert_required_are_advertised("library", &Library.input_schema(), &spec);
        assert_all_honored(
            "library",
            &Library.input_schema(),
            &spec,
            // Measured, not chosen: raising this to 99 reports "covered 1". `path` is the only
            // type-checked key of the three labelled `register:` — `name` and `language` are
            // untyped accessors and sit in `accepts_any_json`. If this ever reads 0 the relabel
            // has been undone; if it reads 3 the two accessors have been given real types and
            // should leave `accepts_any_json`.
            1,
            |args| async move { Library.call(args, &project_ctx().await).await },
        )
        .await;
    }
}
