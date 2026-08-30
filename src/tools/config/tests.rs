use super::*;
use crate::agent::Agent;

use std::sync::Arc;
use tempfile::tempdir;

fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
    crate::lsp::LspManager::new_arc()
}

#[tokio::test]
async fn activate_and_get_config() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // No project initially
    assert!(ProjectStatus.call(json!({}), &ctx).await.is_err());

    // Activate
    let result = ActivateProject
        .call(
            json!({
                "path": dir.path().to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");

    // Now project_status works
    let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    assert!(!status["project_root"].as_str().unwrap().is_empty());
    assert!(status["languages"].is_array());
    assert!(status["embeddings_model"].is_string());
}

#[tokio::test]
async fn activate_surfaces_project_hints_from_cargo_toml() {
    // Agents that never call `onboarding` should still see primary language,
    // manifest, entry points, and build commands in the activate response.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hints = &result["project_hints"];
    assert_eq!(hints["primary_language"], "rust");
    assert_eq!(hints["manifest"], "Cargo.toml");
    assert_eq!(hints["entry_points"], json!(["src/main.rs"]));
    assert!(
        hints["build_commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "cargo test"),
        "hints must include cargo test: {hints:?}"
    );
    assert_eq!(hints["onboarded"], false);
}

#[tokio::test]
async fn activate_hints_empty_for_unrecognised_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // No manifest file.

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hints = &result["project_hints"];
    assert!(hints["primary_language"].is_null());
    assert!(hints["manifest"].is_null());
    assert_eq!(hints["entry_points"], json!([]));
    assert_eq!(hints["build_commands"], json!([]));
}

#[tokio::test]
async fn activate_nonexistent_path_errors() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(
            json!({
                "path": "/nonexistent/path/xyz"
            }),
            &ctx,
        )
        .await;
    assert!(result.is_err());
}

/// Shape B from `docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md`
/// — the mechanism that file called "a hypothesis wearing a conclusion's clothes",
/// isolated from `/clear`, from the companion plugin, and from any second session.
///
/// The asymmetry is six lines in `ActivateProject::call`: with the rendezvous gate
/// OPEN the ledger is preserved (only the project-scoped topic re-arms, and only on a
/// genuine switch); with it SHUT the whole ledger is cleared. `rendezvous_active` is
/// **monotone** — nothing closes it — so a companion hook that dies mid-process leaves
/// the gate latched open. A later `/clear` then mints a conversation that gets neither
/// path: no blunt re-arm here, and no rekey elsewhere, because the rekey was the dead
/// hook's job. That is the one case where Phase C is strictly less forgiving than the
/// blunt predecessor it replaced.
///
/// **Asserted on a tool-contract topic, deliberately.** `project-activation-bootstrap`
/// is the sole member of `PROJECT_SCOPED`, so the gate-OPEN path also drops it whenever
/// the activation registers as a switch — a test keyed on it would be measuring path
/// canonicalization, not the gate. `librarian` can only be removed by `clear()`, which
/// isolates the disputed branch and is the semantically right target: preserving
/// tool-contract guides the model already holds is exactly what the open gate is for.
///
/// This settles the MECHANISM only. It cannot measure how often a hook dies mid-process
/// — nothing in-process can, and the server keeps no state that would answer it.
#[tokio::test]
async fn rendezvous_gate_open_withholds_the_rearm_a_shut_gate_performs() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

    // A dead hook looks exactly like this: gate latched open from an earlier stamp,
    // a topic already delivered, and no rekey — because rekeying was the hook's job.
    let mut led = crate::tools::guide_ledger::GuideLedger::default();
    led.insert("librarian".to_string());
    led.set_rendezvous_active(true);

    let ctx = ToolContext {
        agent: Agent::new(Some(dir.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(led)),
        workspace_override: None,
    };

    let activate = json!({ "path": dir.path().to_str().unwrap() });

    ActivateProject.call(activate.clone(), &ctx).await.unwrap();
    assert!(
        ctx.guide_hints_emitted.lock().contains("librarian"),
        "gate OPEN must leave a tool-contract topic suppressed — this is the starvation \
         path a dead hook produces, and the asymmetry the bug file describes"
    );

    // Same call, same project, gate SHUT: the blunt always-safe clear runs.
    ctx.guide_hints_emitted.lock().set_rendezvous_active(false);
    ActivateProject.call(activate, &ctx).await.unwrap();
    assert!(
        !ctx.guide_hints_emitted.lock().contains("librarian"),
        "gate SHUT must clear the ledger — without this half the assertion above would \
         pass even if activate had stopped touching the ledger entirely"
    );
}

#[tokio::test]
async fn activate_replaces_previous_project() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(dir1.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Activate dir2
    ActivateProject
        .call(
            json!({
                "path": dir2.path().to_str().unwrap()
            }),
            &ctx,
        )
        .await
        .unwrap();

    let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let root = status["project_root"].as_str().unwrap();
    assert!(root.contains(dir2.path().file_name().unwrap().to_str().unwrap()));
}

#[tokio::test]
async fn project_status_returns_all_sections() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let tool = ProjectStatus;
    let result = tool.call(json!({}), &ctx).await.unwrap();
    assert!(result["project_root"].is_string(), "missing project_root");
    assert!(result["languages"].is_array(), "missing languages field");
    assert!(
        result["embeddings_model"].is_string(),
        "missing embeddings_model field"
    );
    assert!(result.get("index").is_some(), "missing index section");
    assert!(
        result.get("libraries").is_some(),
        "missing libraries section"
    );
}

/// `status` must declare the binary that answered, through the real call.
///
/// The comparison this exists for is `status.server.git_sha` against the
/// sidecar's `written_by.git_sha` — "did the process answering me also write my
/// index state, and was either of them running code that no longer exists?" —
/// so the two must come from the same constructor. Asserting against
/// `current_writer()` rather than against a literal is what pins that: a copy
/// that drifted to its own snapshot would still look plausible in the JSON.
///
/// Unconditional by design. A field that appeared only when something was wrong
/// could not answer the question on a healthy day, and the failure it reports is
/// one where the operator's own belief about which build is serving them is the
/// thing in doubt.
#[tokio::test]
async fn project_status_declares_the_binary_that_answered() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    let server = result
        .get("server")
        .unwrap_or_else(|| panic!("status must declare the serving binary: {result:#}"));
    let want = crate::retrieval::index_state::current_writer();

    assert_eq!(
        server["git_sha"], want.git_sha,
        "must be the SAME identity the sidecar writer stamps, so the two are \
         comparable: {result:#}"
    );
    assert_eq!(server["pid"], want.pid, "{result:#}");
    assert_eq!(
        server["git_dirty"], want.git_dirty,
        "a dirty build does not fully identify its code, and a reader comparing \
         shas needs to know that: {result:#}"
    );
    assert!(
        server.get("exe_deleted").is_some(),
        "the key must be present even where the answer is None — absence of the \
         KEY would read as 'not deleted' rather than 'could not tell': {result:#}"
    );
}

#[tokio::test]
async fn project_status_compact_shape() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    // Flat config fields — no blob
    assert!(result["languages"].is_array(), "missing languages");
    assert!(
        result["embeddings_model"].is_string(),
        "missing embeddings_model"
    );
    assert!(
        result.get("config").is_none(),
        "config blob must be removed"
    );

    // Index section has status field, no drift
    assert!(
        result["index"]["status"].is_string(),
        "index.status must be present"
    );
    assert!(
        result["index"].get("drift").is_none(),
        "drift must not appear in project_status"
    );

    // Libraries section still present
    assert!(result["libraries"].is_object(), "libraries section missing");
}

#[tokio::test]
async fn status_reports_the_live_backend_and_what_is_compiled_in() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    assert!(
        result["embeddings_model"].is_string(),
        "the existing flat field must survive"
    );

    // Ground truth, computed independently of `ProjectStatus::call` from the
    // same feature flags and the same env vars the implementation ultimately
    // reads (transitively, via `RetrievalConfig::from_env_and_project`), so a
    // hardcoded value in the implementation cannot coincidentally agree with
    // it. (Mutation check, verified by hand: hardcode `backend` in the
    // implementation to always "remote-http" -- this test then fails in a
    // clean shell with neither env var set, because the fresh tempdir below
    // has no project.toml, so the effective model is the built-in default
    // `local:AllMiniLML6V2Q` with no embedder_url, which never resolves to
    // "remote-http".)
    let mut expected_compiled_in: Vec<&str> = Vec::new();
    if cfg!(feature = "remote-embed") {
        expected_compiled_in.push("remote");
    }
    if cfg!(any(
        feature = "local-embed",
        feature = "local-embed-dynamic"
    )) {
        expected_compiled_in.push("local-onnx");
    }
    assert_eq!(
        result["embedding_compiled_in"],
        json!(expected_compiled_in),
        "must name exactly which backends this binary was compiled with"
    );

    // Two env-var families reach the effective embedder url, at different
    // layers: `CODESCOUT_EMBEDDER_URL` (read directly by `EmbedEnv::from_real_env`)
    // takes precedence over `CODESCOUT_EMBED_URL` (applied inside
    // `ProjectConfig::load_or_default`, see src/retrieval/config.rs:41-51).
    // A dev shell that exports a local embedder daemon's address commonly
    // sets both -- checking only one family here would make this test's
    // ground truth wrong on exactly that shell, independent of any mutation.
    // The fresh tempdir has no project.toml, so no third (file-based) layer
    // is in play.
    let embedder_url_set = std::env::var("CODESCOUT_EMBEDDER_URL")
        .ok()
        .or_else(|| std::env::var("CODESCOUT_EMBED_URL").ok())
        .filter(|s| !s.is_empty())
        .is_some();
    // Effective model, derived independently of the implementation with the
    // same precedence `RetrievalConfig::from_env_and_project` uses:
    // `CODESCOUT_EMBEDDER_MODEL` first, then `CODESCOUT_EMBED_MODEL`, else the
    // built-in default. The fresh tempdir has no project.toml, so no
    // file-based layer is in play. This ladder must NOT call
    // `backend_is_local` itself -- that would make the check a tautology
    // against the implementation it's supposed to independently verify.
    let effective_model = std::env::var("CODESCOUT_EMBEDDER_MODEL")
        .ok()
        .or_else(|| std::env::var("CODESCOUT_EMBED_MODEL").ok())
        .unwrap_or_else(|| "local:AllMiniLML6V2Q".to_string());
    let model_is_local =
        effective_model.starts_with("local:") || effective_model.starts_with("local-dir:");
    let expected_backend = if embedder_url_set {
        "remote-http"
    } else if model_is_local {
        if expected_compiled_in.contains(&"local-onnx") {
            "local-onnx"
        } else {
            "unavailable"
        }
    } else {
        "remote-http"
    };
    assert_eq!(
        result["embedding_backend"],
        json!(expected_backend),
        "must name the backend this config actually resolves to, not a fixed string"
    );
}
/// I1 (final whole-branch review): the old rule computed `backend` purely
/// from "is local-onnx compiled in", never looking at the configured model
/// string. That misreports two real configurations: a lean build (no
/// `local-embed`) with an `ollama:`/`openai:` model and no url reported
/// `"unavailable"` even though the config works fine over the network,
/// and a `local-embed` build with the same model reported `"local-onnx"`
/// even though the live backend is `RemoteEmbedder` over Ollama, never
/// `LocalEmbedder`. `backend_is_local` (`client.rs:37-40`) is documented as
/// the single source of truth for exactly this question — this test varies
/// the *model string*, not just env, so it fails against the old
/// compiled-in-only rule regardless of which optional backends this
/// binary happens to have compiled in.
///
/// **Refined 2026-08-14.** I1's rationale was "the config works fine over the
/// network" — true whenever `remote-embed` is compiled, which it is by default.
/// A `--no-default-features` build turns that default feature off, and then no
/// arm of `create_embedder_with_config` can build an `ollama:` model at all; it
/// bails with "Unknown model". Reporting `remote-http` there claimed a working
/// network config for one that cannot build anything, so the expectation is now
/// feature-aware. I1's original assertion is preserved verbatim for every build
/// that has the feature — which is every default build, including CI's.
/// docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md
#[tokio::test]
async fn status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join(".codescout/project.toml"),
        "[project]\nname = \"test\"\n\n[embeddings]\nmodel = \"ollama:nomic-embed-text\"\n",
    )
    .unwrap();

    // Two ambient env layers can override the model this test just set on
    // disk (see the sibling test above) — if either is present on this
    // machine, the ground truth this test asserts no longer holds, so skip
    // rather than assert a premise that isn't true here.
    if std::env::var("CODESCOUT_EMBEDDER_MODEL").is_ok()
        || std::env::var("CODESCOUT_EMBED_MODEL").is_ok()
    {
        eprintln!(
            "skipping status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends: \
                 CODESCOUT_EMBEDDER_MODEL/CODESCOUT_EMBED_MODEL ambient override present"
        );
        return;
    }
    let embedder_url_set = std::env::var("CODESCOUT_EMBEDDER_URL")
        .ok()
        .or_else(|| std::env::var("CODESCOUT_EMBED_URL").ok())
        .filter(|s| !s.is_empty())
        .is_some();
    if embedder_url_set {
        eprintln!(
            "skipping status_reports_remote_http_for_an_urlless_ollama_model_regardless_of_compiled_backends: \
                 an ambient embedder url is set, which always wins over the model"
        );
        return;
    }

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    // Ground truth: an `ollama:` model with no url is never
    // `backend_is_local` (it doesn't start with `local:`/`local-dir:`), so
    // whether a *local* backend is compiled in cannot change the answer — that
    // is still what this test's name asserts. What does change it is whether
    // the *remote* backend it names is compiled at all.
    let expected = if cfg!(feature = "remote-embed") {
        "remote-http"
    } else {
        "unavailable"
    };
    assert_eq!(
        result["embedding_backend"],
        json!(expected),
        "an ollama: model with no url must report {expected} for this build \
         (remote-embed compiled: {}), got: {:?}",
        cfg!(feature = "remote-embed"),
        result["embedding_backend"]
    );
}

/// The sibling above covers the `ollama:` half of
/// `docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md`.
/// This covers the other half: a **bare** model name with no url. Arm 6 of
/// `create_embedder_with_config` resolves such a name as a local ONNX model, so
/// with `local-embed` compiled in the live backend really is `LocalEmbedder` and
/// the status must say `local-onnx` — it used to say `remote-http`.
///
/// Gated on the feature because the classifier can only answer "local" when a
/// local backend exists to answer for; the `local-embed` CI lane runs this.
#[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
#[tokio::test]
async fn status_reports_local_onnx_for_an_urlless_bare_model_name() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(
        dir.path().join(".codescout/project.toml"),
        "[project]\nname = \"test\"\n\n[embeddings]\nmodel = \"AllMiniLML6V2Q\"\n",
    )
    .unwrap();

    // Same two ambient override layers the sibling test guards against: either
    // one replaces the model this test just wrote, invalidating its premise.
    if std::env::var("CODESCOUT_EMBEDDER_MODEL").is_ok()
        || std::env::var("CODESCOUT_EMBED_MODEL").is_ok()
    {
        eprintln!(
            "skipping status_reports_local_onnx_for_an_urlless_bare_model_name: \
             CODESCOUT_EMBEDDER_MODEL/CODESCOUT_EMBED_MODEL ambient override present"
        );
        return;
    }
    if std::env::var("CODESCOUT_EMBEDDER_URL")
        .ok()
        .or_else(|| std::env::var("CODESCOUT_EMBED_URL").ok())
        .filter(|s| !s.is_empty())
        .is_some()
    {
        eprintln!(
            "skipping status_reports_local_onnx_for_an_urlless_bare_model_name: \
             an ambient embedder url is set, which always wins over the model"
        );
        return;
    }

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

    assert_eq!(
        result["embedding_backend"],
        json!("local-onnx"),
        "a bare model name with no url resolves through arm 6 to a local ONNX \
         embedder, so the status must name it, got: {:?}",
        result["embedding_backend"]
    );
}

#[tokio::test]
async fn project_status_includes_memory_staleness() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Create memories dir and a memory file
    let memories_dir = dir.path().join(".codescout/memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(memories_dir.join("architecture.md"), "# Arch").unwrap();

    // Create anchored file and sidecar
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/server.rs"), "fn main() {}").unwrap();

    let anchors =
        crate::memory::anchors::seed_anchors(dir.path(), "Uses `src/server.rs`.").unwrap();
    crate::memory::anchors::write_anchor_file(
        &memories_dir.join("architecture.anchors.toml"),
        &anchors,
    )
    .unwrap();

    // Before change — should be fresh
    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let staleness = &result["memory_staleness"];
    assert!(staleness["stale"].as_array().unwrap().is_empty());
    assert!(staleness["fresh"]
        .as_array()
        .unwrap()
        .contains(&json!("architecture")));

    // Modify the anchored file
    std::fs::write(dir.path().join("src/server.rs"), "fn changed() {}").unwrap();

    let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
    let staleness = &result["memory_staleness"];
    let stale = staleness["stale"].as_array().unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0]["topic"], "architecture");
    assert!(stale[0]["changed_files"]
        .as_array()
        .unwrap()
        .contains(&json!("src/server.rs")));
}

#[tokio::test]
async fn activate_includes_cwd_hint() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Canonicalize to match Agent::activate's canonicalization
    // (resolves /var → /private/var on macOS, strips \\?\ prefix on Windows).
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let agent = Agent::new(None).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let input = json!({ "path": root.to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.starts_with("CWD: "),
        "hint should start with CWD: but was: {hint}"
    );
    // Response paths are forward-slash normalized (RepoPath convention); the
    // raw canonicalized PathBuf renders with native separators on Windows.
    assert!(hint.contains(&crate::util::fs::to_forward_slash(&root)));
}

#[tokio::test]
async fn activate_hint_shows_switched_when_away_from_home() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
    // Canonicalize to match Agent::new/activate's canonicalization
    // (resolves /var → /private/var on macOS, strips \\?\ prefix on Windows).
    let root1 = std::fs::canonicalize(dir1.path()).unwrap();
    let root2 = std::fs::canonicalize(dir2.path()).unwrap();
    let agent = Agent::new(Some(root1.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let input = json!({ "path": root2.to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();
    // Non-home default is RO: "Browsing … (read-only). CWD: … — remember to workspace(action='activate', …)"
    assert!(
        hint.contains("remember to workspace"),
        "hint should warn to switch back: {hint}"
    );
    // Response paths are forward-slash normalized (RepoPath convention); the
    // raw canonicalized PathBufs render with native separators on Windows.
    assert!(
        hint.contains(&crate::util::fs::to_forward_slash(&root2)),
        "should contain new path: {hint}"
    );
    assert!(
        hint.contains(&crate::util::fs::to_forward_slash(&root1)),
        "should contain home path: {hint}"
    );
}

/// hamsa A-29 — the gate parses, and it parses CLOSED by default.
///
/// Pure, by convention: `docs/conventions/test-env-isolation.md` retired
/// `EnvGuard` + `#[serial]` crate-wide, so the env read stays at the edge and only
/// the decision is tested.
#[test]
fn pin_notice_gate_is_closed_unless_explicitly_opened() {
    for opener in ["1", "true"] {
        assert!(
            super::pin_notice_enabled_from(Some(opener)),
            "{opener:?} must open the gate"
        );
    }
    for closed in [
        None,
        Some(""),
        Some("0"),
        Some("false"),
        Some("yes"),
        Some("TRUE"),
    ] {
        assert!(
            !super::pin_notice_enabled_from(closed),
            "{closed:?} must leave the gate closed — anything but an explicit opener is off, \
             so an unvalidated intervention cannot reach production by typo"
        );
    }
}

/// hamsa A-29 — the notice CONTRASTS, and does not CONTRADICT.
///
/// A-26 measured that naming a tool in a routing line does not displace a strong
/// competing prior — what moved its number was explicitly contrasting the two and naming
/// the wrong one. The prior here is the restore instruction this text replaces.
///
/// The no-contradiction assertion is the one that earned its place. The first version
/// APPENDED to the existing hint, yielding adjacent sentences reading "remember to
/// workspace(action='activate', …) when done" and then "do not activate". That was found
/// by reading the real composed string out of a failing test, not from the source, and it
/// would have shipped into A-29's arms — measuring a muddled instruction instead of the
/// intended one. Both instructions are now CONDITIONED, so each says when it applies.
#[test]
fn pin_notice_contrasts_the_competing_prior_rather_than_naming_the_pin() {
    let text = super::workspace_pin_contrast(
        "Switched project (read-write). CWD: /home/me/work/other-repo",
        "/home/me/work/other-repo",
        "/home/me/work/home-repo",
    );

    assert!(
        text.contains("server-global"),
        "must state the SCOPE — an agent that does not know activation is process-wide \
         cannot infer it just clobbered a peer. Got: {text:?}"
    );
    assert!(
        text.contains("do not activate"),
        "must name the WRONG action, not just the right one (A-26). Got: {text:?}"
    );
    assert!(
        text.contains("workspace=\"/home/me/work/other-repo\""),
        "must give the pin already filled in with the path the agent just used, so the \
         remedy is copyable rather than derivable. Got: {text:?}"
    );

    // Both instructions must be CONDITIONED. Without their conditions the hint tells the
    // agent to activate and not to activate, in adjacent sentences.
    let dont = text.find("do not activate").expect("checked above");
    let restore = text
        .find("remember to workspace")
        .expect("restore clause must survive");
    assert!(
        restore > dont,
        "the restore clause must come AFTER the do-not-activate clause, so the reader \
         meets the general rule before its exception. Got: {text:?}"
    );
    for condition in [
        "If other agents are working concurrently",
        "If you are the only agent here",
    ] {
        assert!(
            text.contains(condition),
            "each instruction needs its condition or the two contradict — missing \
             {condition:?}. Got: {text:?}"
        );
    }
}

/// hamsa A-29 — DEFAULT OFF is the shipped behaviour, asserted on the REAL composed hint.
///
/// The first version of this test checked the gate parser and that the suffix string was
/// non-empty, and called that coverage. It was a green bar: forcing
/// `workspace_pin_notice_enabled()` to return `true` unconditionally left all 62 tests in
/// this module passing, so an unmeasured intervention could have shipped and nothing
/// would have failed. `activate_hint_shows_switched_when_away_from_home` cannot catch it
/// either — it asserts `contains`, which an appended suffix does not disturb.
///
/// So this drives a real activation and asserts on the hint the caller actually receives.
/// Same discipline as the inverted guards on A-25/A-26/A-27: an intervention that has not
/// been measured must not reach production by drift.
#[tokio::test]
async fn switch_away_hint_carries_no_pin_notice_while_the_gate_is_shut() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
    let root1 = std::fs::canonicalize(dir1.path()).unwrap();
    let root2 = std::fs::canonicalize(dir2.path()).unwrap();
    let agent = Agent::new(Some(root1.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let input = json!({ "path": root2.to_str().unwrap() });
    let result = ActivateProject.call(input, &ctx).await.unwrap();
    let hint = result["hint"].as_str().unwrap();

    // Sanity: this really is the switch-away branch, so the assertion below is not
    // vacuously true on some other hint.
    assert!(
        hint.contains("remember to workspace"),
        "expected the switch-away hint, got: {hint}"
    );

    for marker in ["server-global", "do not activate", "workspace=\""] {
        assert!(
            !hint.contains(marker),
            "the A-29 pin notice reached a shipped hint with the gate shut \
             (marker {marker:?}). It is UNMEASURED — A-29 has not run. Enable it only \
             via CODESCOUT_WORKSPACE_PIN_NOTICE. Hint was: {hint}"
        );
    }
}

#[tokio::test]
async fn activate_hint_shows_returned_when_back_home() {
    let dir1 = tempdir().unwrap();
    let dir2 = tempdir().unwrap();
    std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
    // Canonicalize to match Agent::new/activate's canonicalization
    // (resolves /var → /private/var on macOS, strips \\?\ prefix on Windows).
    let root1 = std::fs::canonicalize(dir1.path()).unwrap();
    let root2 = std::fs::canonicalize(dir2.path()).unwrap();
    let agent = Agent::new(Some(root1.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Switch away
    ActivateProject
        .call(json!({ "path": root2.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    // Return home
    let result = ActivateProject
        .call(json!({ "path": root1.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(hint.contains("Returned to home project"), "hint: {hint}");
    // Response paths are forward-slash normalized (RepoPath convention); the
    // raw canonicalized PathBuf renders with native separators on Windows.
    assert!(hint.contains(&crate::util::fs::to_forward_slash(&root1)));
}

#[tokio::test]
async fn project_status_shows_workspace_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create multi-project structure
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    // Create workspace.toml
    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["test"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus
        .call(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let ws = result.get("workspace");
    assert!(
        ws.is_some(),
        "project_status should include workspace section"
    );
    let projects = ws.unwrap().get("projects").unwrap().as_array().unwrap();
    assert_eq!(projects.len(), 2);
}

/// docs/issues/archive/2026-08-26-list-projects-reports-declared-projects-not-workspace-members.md
///
/// `status`/`list_projects` used to re-parse `.codescout/workspace.toml` and report
/// only its declared `[[project]]` array. A sub-project codescout auto-discovers by
/// manifest walk needs no declaration at all — `activate`'s workspace table and
/// `memory(project_id=...)` both already know about it — so the one surface
/// documented as "how to list valid project ids" could omit a valid id. This
/// declares 2 projects (root + `declared-svc`) but a manifest makes a 3rd,
/// `extra-service`, live and undeclared.
#[tokio::test]
async fn project_status_reports_a_live_discovered_project_the_declared_config_omits() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("build.gradle.kts"), "").unwrap();

    let declared_svc = root.join("declared-svc");
    std::fs::create_dir_all(&declared_svc).unwrap();
    std::fs::write(
        declared_svc.join("package.json"),
        r#"{"scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let extra_service = root.join("extra-service");
    std::fs::create_dir_all(&extra_service).unwrap();
    std::fs::write(
        extra_service.join("package.json"),
        r#"{"scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    // Deliberately omits "extra-service" — exactly like the bug's own
    // reproduction on this repo (codescout-embed was live but undeclared).
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "declared-svc"
root = "declared-svc"
languages = ["typescript"]
depends_on = ["test"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus
        .call(serde_json::json!({}), &ctx)
        .await
        .unwrap();
    let projects = result["workspace"]["projects"].as_array().unwrap();
    assert_eq!(
        projects.len(),
        3,
        "declared config names 2 projects, but a manifest makes 3 live — status \
         must report the live count, matching what activate() already shows and \
         what memory(project_id=...) already accepts: {projects:?}"
    );
    assert!(
        projects.iter().any(|p| p["id"] == "extra-service"),
        "the undeclared-but-discovered sub-project must appear by its live id: {projects:?}"
    );
    let declared = projects
        .iter()
        .find(|p| p["id"] == "declared-svc")
        .expect("the declared sub-project must still appear: {projects:?}");
    assert_eq!(
        declared["depends_on"],
        serde_json::json!(["test"]),
        "depends_on is metadata discovery cannot supply and must still be looked \
         up from the declared config by id, same as Agent::workspace_summary: {declared:?}"
    );
}

/// The load-bearing half of the worktree-divergence fix: the formatter tests
/// below would pass even if the block were never emitted. This one activates a
/// real linked-worktree root and asserts the response actually carries it.
///
/// docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md
#[tokio::test]
async fn activating_a_linked_worktree_reports_the_divergence_it_creates() {
    let dir = tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();

    // A linked worktree is identified by `.git` being a FILE whose `gitdir:`
    // pointer contains a `worktrees` component — not by asking git.
    let wt = base.join("main/.worktrees/feat");
    std::fs::create_dir_all(wt.join(".codescout")).unwrap();
    std::fs::write(wt.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    std::fs::write(
        wt.join(".git"),
        // `join`, not string concatenation. `base` is canonicalized, which on Windows is
        // the verbatim `\\?\C:\…` form — and inside a verbatim path Rust does NOT treat
        // `/` as a separator. A hand-built `<verbatim>/main/.git/worktrees/feat` then
        // parses as ONE component, `is_linked_worktree` never sees a `worktrees`
        // component, and the activation silently reports no worktree at all. Git itself
        // never writes that spelling; only a fixture concatenating strings does.
        format!(
            "gitdir: {}\n",
            base.join("main")
                .join(".git")
                .join("worktrees")
                .join("feat")
                .display()
        ),
    )
    .unwrap();

    let agent = Agent::new(Some(wt.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(serde_json::json!({"path": wt.to_string_lossy()}), &ctx)
        .await
        .unwrap();

    let wt_block = result
        .get("worktree")
        .unwrap_or_else(|| panic!("no worktree block on a linked-worktree activation: {result}"));
    assert!(
        wt_block["main_root"]
            .as_str()
            .is_some_and(|m| m.ends_with("main")),
        "the notice must name the main checkout so the caller can compare, got: {wt_block}"
    );
    assert_eq!(
        wt_block["topology"], "inferred",
        "no .codescout/workspace.toml here — it is gitignored and does not travel \
         into a worktree — so the sub-project list is auto-detected, got: {wt_block}"
    );
    assert!(
        wt_block["memories_are_this_checkouts"].is_string(),
        "the memory set is this commit's, and that has to be said: {wt_block}"
    );
}

/// The third topology state, and the one that stops the hint from lying. Since
/// `load_discover_settings` reads through to the main checkout, a worktree whose
/// main HAS a `workspace.toml` no longer diverges — reporting it as "inferred"
/// with "ran with defaults" would assert something measurably false about the walk
/// that just ran.
///
/// The discriminator against the sibling test above is one file: `main/.codescout/
/// workspace.toml`. Nothing else differs.
///
/// docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md
#[tokio::test]
async fn a_worktree_whose_main_has_workspace_toml_reports_inherited_topology() {
    let dir = tempdir().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();

    // The one thing the sibling test does not do.
    std::fs::create_dir_all(base.join("main/.codescout")).unwrap();
    std::fs::write(
        base.join("main/.codescout/workspace.toml"),
        "exclude_projects = [\"fixtures\"]\n[workspace]\nname = \"t\"\ndiscovery_max_depth = 3\n",
    )
    .unwrap();

    let wt = base.join("main/.worktrees/feat");
    std::fs::create_dir_all(wt.join(".codescout")).unwrap();
    std::fs::write(wt.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
    std::fs::write(
        wt.join(".git"),
        // `join`, not string concatenation. `base` is canonicalized, which on Windows is
        // the verbatim `\\?\C:\…` form — and inside a verbatim path Rust does NOT treat
        // `/` as a separator. A hand-built `<verbatim>/main/.git/worktrees/feat` then
        // parses as ONE component, `is_linked_worktree` never sees a `worktrees`
        // component, and the activation silently reports no worktree at all. Git itself
        // never writes that spelling; only a fixture concatenating strings does.
        format!(
            "gitdir: {}\n",
            base.join("main")
                .join(".git")
                .join("worktrees")
                .join("feat")
                .display()
        ),
    )
    .unwrap();

    let agent = Agent::new(Some(wt.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(serde_json::json!({"path": wt.to_string_lossy()}), &ctx)
        .await
        .unwrap();

    let wt_block = result
        .get("worktree")
        .unwrap_or_else(|| panic!("no worktree block on a linked-worktree activation: {result}"));
    assert_eq!(
        wt_block["topology"], "inherited",
        "the main checkout has a workspace.toml, so discovery read it rather than \
         running with defaults, got: {wt_block}"
    );
    let hint = wt_block["topology_hint"].as_str().unwrap_or_default();
    assert!(
        hint.contains("MAIN"),
        "the hint must say where the settings came from, got: {hint}"
    );
    assert!(
        !hint.contains("ran with defaults"),
        "reporting defaults here would be the exact falsehood this state exists to \
         prevent, got: {hint}"
    );
}

/// The other side: a plain checkout must not grow a worktree block, or the
/// notice becomes noise in every ordinary session.
#[tokio::test]
async fn activating_a_plain_checkout_adds_no_worktree_block() {
    let dir = tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();

    let agent = Agent::new(Some(root.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(serde_json::json!({"path": root.to_string_lossy()}), &ctx)
        .await
        .unwrap();
    assert!(
        result.get("worktree").is_none(),
        "a plain checkout has no divergence to report, got: {result}"
    );
}

#[tokio::test]
async fn activate_project_switches_focus_by_id() {
    let dir = tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();

    // Create multi-project structure
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let agent = Agent::new(Some(root.clone())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Initially focused on root project
    let root_path = ctx.agent.require_project_root().await.unwrap();
    assert_eq!(root_path, root);

    // Switch focus to mcp-server by ID
    let result = ActivateProject
        .call(serde_json::json!({"path": "mcp-server"}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");

    // Now focused on mcp-server
    let new_root = ctx.agent.require_project_root().await.unwrap();
    assert_eq!(new_root, root.join("mcp-server"));
}

#[tokio::test]
async fn activate_project_unknown_id_with_no_slash_returns_error() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"test\"\n").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // "unknown-project" has no slash and does not exist as a project ID or a path
    let result = ActivateProject
        .call(serde_json::json!({"path": "unknown-project"}), &ctx)
        .await;
    // Should fail: not a known project ID, and not a valid directory path
    assert!(
        result.is_err() || result.as_ref().unwrap().get("error").is_some(),
        "expected error or error field, got: {:?}",
        result
    );
}

#[tokio::test]
async fn post_compact_flushes_lsp_clients_and_returns_flushed() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // post_compact=true should return flushed:true without the normal status fields
    let result = ProjectStatus
        .call(json!({"post_compact": true}), &ctx)
        .await
        .unwrap();
    assert_eq!(result["flushed"], json!(true), "expected flushed:true");
    assert!(result["hint"].is_string(), "expected hint string");
    // Normal status fields must NOT be present in the compact-flush response
    assert!(
        result.get("project_root").is_none(),
        "post_compact response must not include project_root"
    );

    // post_compact=false (or absent) should return the normal status response
    let result = ProjectStatus
        .call(json!({"post_compact": false}), &ctx)
        .await
        .unwrap();
    assert!(
        result["project_root"].is_string(),
        "normal call must include project_root"
    );
}

/// The `post_compact` hint must price the flush, not merely describe its mechanism.
///
/// `docs/issues/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`
/// (`open-issue-work-queue:BL-49`). The restart is lazy, so the next navigation call
/// pays the language-server start — on a 1697-file Rust crate that once exceeded the
/// 60s tool timeout and returned nothing. The old hint read "Clients restart
/// automatically on the next navigation call (symbol_at, references)": true about the
/// mechanism, silent about who pays.
///
/// Measured 2026-08-30, and it is why this test does not simply demand the word
/// "cold": the cost is real but NOT unconditional. `rust-analyzer` runs under a
/// workspace-keyed `codescout mux` process (`--idle-timeout 180`), not under the
/// codescout server that uses it, so `shutdown_all` drains this server's client map
/// while the language server survives. A flush followed 18 minutes later by
/// `references` returned immediately and spawned no new process — served by a mux a
/// *different session's* server had started. A hint promising an unconditional cold
/// start would be as wrong as one promising none.
///
/// Both assertions fail on a revert to the old text, which names neither the remedy
/// nor the shared-server caveat. Lowercased because a sibling assertion in
/// `src/retrieval/sync.rs` was written case-brittle and had to be repaired.
#[tokio::test]
async fn post_compact_hint_prices_the_flush_rather_than_only_describing_the_mechanism() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ProjectStatus
        .call(json!({"post_compact": true}), &ctx)
        .await
        .unwrap();
    let hint = result["hint"]
        .as_str()
        .expect("post_compact response must carry a hint")
        .to_lowercase();

    assert!(
        hint.contains("re-run"),
        "the hint must name the remedy for a first navigation call that stalls, \
         since the caller otherwise cannot tell a slow start from a dead tool: {hint}"
    );
    assert!(
        hint.contains("another session"),
        "the hint must say the language server is shared across sessions in this \
         workspace, so the cost is conditional rather than certain: {hint}"
    );
}

/// docs/issues/archive/2026-08-15-worktree-memory-set-and-subproject-topology-diverge.md
///
/// A worktree activation serves that commit's memories and, because
/// `workspace.toml` is gitignored and therefore absent, an unpruned sub-project
/// walk. Both used to be silent. The compact line is what most callers read, so
/// the divergence has to reach it — not just the JSON.
#[test]
fn a_worktree_activation_says_the_memory_count_is_this_checkouts() {
    let result = json!({
        "status": "ok",
        "project": "codescout",
        "project_root": "/repo/.worktrees/feat",
        "read_only": false,
        "memories": ["arch", "conventions"],
        "index": {"status": "indexed"},
        "worktree": {
            "main_root": "/repo",
            "topology": "configured",
            "memories_are_this_checkouts": "2 memory topics come from THIS worktree's commit.",
        },
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.contains("linked worktree"),
        "a worktree activation must announce itself, got: {compact}"
    );
    assert!(
        !compact.contains("topology inferred"),
        "workspace.toml was present here, so the topology is declared: {compact}"
    );
}

/// The sharper half: no `workspace.toml` means the sub-project walk ran with no
/// `exclude_projects`, so the list is auto-detected and likely wider than main's.
/// An agent that reads it as declared topology draws a wrong conclusion.
#[test]
fn a_worktree_without_workspace_toml_marks_its_topology_inferred() {
    let result = json!({
        "status": "ok",
        "project": "codescout",
        "project_root": "/repo/.worktrees/feat",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "worktree": {
            "main_root": "/repo",
            "topology": "inferred",
            "topology_hint": "No .codescout/workspace.toml here ...",
        },
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.contains("topology inferred"),
        "an inferred topology must say so, got: {compact}"
    );
}

/// The compact line is what most callers actually read, so the third state has to
/// reach it too. "inherited" must not fall through to the bare `_` arm — that would
/// render identically to a worktree with its own declared workspace.toml, losing the
/// one fact this state exists to convey.
#[test]
fn a_worktree_inheriting_its_topology_says_so_on_the_compact_line() {
    let result = json!({
        "status": "ok",
        "project": "codescout",
        "project_root": "/repo/.worktrees/feat",
        "read_only": false,
        "languages": ["rust"],
        "index": {"status": "indexed"},
        "worktree": {
            "main_root": "/repo",
            "topology": "inherited",
            "topology_hint": "No .codescout/workspace.toml here ... read the MAIN checkout's ...",
            "memories_are_this_checkouts": "2 memory topics come from THIS worktree's commit.",
        },
    });

    let compact = format_activate_project(&result);

    assert!(
        compact.contains("topology inherited from main"),
        "an inherited topology must say so, got: {compact}"
    );
    assert!(
        !compact.contains("topology inferred"),
        "inherited is not inferred — discovery used real settings, got: {compact}"
    );
}

/// The common case pays nothing: a plain checkout gets no worktree block and no
/// extra words on the summary line.
#[test]
fn a_plain_checkout_activation_says_nothing_about_worktrees() {
    let result = json!({
        "status": "ok",
        "project": "codescout",
        "project_root": "/repo",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "hint": "CWD: /repo"
    });
    let compact = format_activate_project(&result);
    assert!(
        !compact.contains("worktree"),
        "no worktree, nothing to say, got: {compact}"
    );
}

#[test]
fn format_activate_project_rw_compact() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch", "conventions", "gotchas"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · my-project (rw) · 3 memories · index: not_indexed"
    );
}

#[test]
fn format_activate_project_prepends_legacy_index_banner() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "legacy_semantic_index": {
            "path": "/home/user/my-project/.codescout/embeddings/project.db",
            "hint": "Run `codescout migrate-memories` to port memories to Qdrant, then delete this file.",
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ LEGACY INDEX: run `codescout migrate-memories`"),
        "expected legacy-index banner prepended, got:\n{compact}"
    );
    assert!(compact.contains("activated · my-project (rw)"));
}

#[test]
fn format_activate_project_no_legacy_banner_when_absent() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(!compact.contains("LEGACY INDEX"));
}

#[test]
fn format_activate_project_stacks_legacy_under_stale_warning() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "legacy_semantic_index": { "path": "/x/y", "hint": "..." },
        "system_prompt_stale": {
            "stored_version": 1,
            "current_version": 5,
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    let lines: Vec<&str> = compact.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 lines, got:\n{compact}");
    assert!(lines[0].contains("SYSTEM PROMPT STALE"));
    assert!(lines[1].contains("LEGACY INDEX"));
    assert!(lines[2].starts_with("activated · "));
}

#[test]
fn format_activate_project_ro_with_workspace() {
    let result = json!({
        "status": "ok",
        "project": "sub-lib",
        "project_root": "/home/user/mono/sub-lib",
        "read_only": true,
        "memories": [],
        "index": {"status": "indexed"},
        "workspace": [
            {"id": "main", "root": ".", "languages": ["rust"]},
            {"id": "sub-lib", "root": "libs/sub-lib", "languages": ["rust"]},
        ],
        "hint": "Browsing sub-lib (read-only)."
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · sub-lib (ro) · 0 memories · index: indexed · 2 workspace projects"
    );
}

#[test]
fn format_activate_project_with_auto_libs() {
    let result = json!({
        "status": "ok",
        "project": "web",
        "project_root": "/home/user/web",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 12, "without_source": 3},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(compact, "activated · web (rw) · 1 memories · index: not_indexed · auto-registered 12 libs (3 without source)");
}

#[test]
fn format_activate_project_auto_libs_all_with_source() {
    let result = json!({
        "status": "ok",
        "project": "app",
        "project_root": "/home/user/app",
        "read_only": false,
        "memories": [],
        "index": {"status": "indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 0},
        "hint": "CWD: ..."
    });
    let compact = format_activate_project(&result);
    assert_eq!(
        compact,
        "activated · app (rw) · 0 memories · index: indexed · auto-registered 5 libs"
    );
}

#[tokio::test]
async fn activate_project_rw_includes_security_fields() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // A `root` profile must be surfaced on activation — it disables every
    // path/command gate, so the agent needs to know the sandbox is off.
    std::fs::write(
        dir.path().join(".codescout/project.toml"),
        "[project]\nname = \"test\"\n\n[security]\nprofile = \"root\"\n",
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(
            json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(
        result["security_profile"], "root",
        "RW + root profile should surface security_profile"
    );
    assert!(
        result["shell_enabled"].is_null(),
        "shell_enabled is no longer reported"
    );
}

#[tokio::test]
async fn activate_project_rw_default_omits_security_profile() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Default (sandboxed) profile is the common case — it carries no signal,
    // so the activation card omits security_profile entirely.
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(
            json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(
        result["security_profile"].is_null(),
        "RW + default profile should omit security_profile"
    );
    assert!(
        result["shell_enabled"].is_null(),
        "shell_enabled is no longer reported"
    );
}

#[tokio::test]
async fn activate_project_ro_excludes_security_fields() {
    let home = tempdir().unwrap();
    let other = tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(other.path().join(".codescout")).unwrap();
    // Start with a home project (always RW)
    let ctx = ToolContext {
        agent: Agent::new(Some(home.path().to_path_buf())).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Now activate another project as RO
    let result = ActivateProject
        .call(
            json!({"path": other.path().to_str().unwrap(), "read_only": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(
        result["security_profile"].is_null(),
        "RO should not include security_profile"
    );
    assert!(
        result["shell_enabled"].is_null(),
        "RO should not include shell_enabled"
    );
}

#[tokio::test]
async fn activate_project_includes_memories_and_index() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(
        result["memories"].is_array(),
        "should include memories array"
    );
    assert!(result["index"].is_object(), "should include index object");
    assert!(
        result["index"]["status"].is_string(),
        "index should have status"
    );
}

#[tokio::test]
async fn activate_project_rw_hint_promotes_project_status() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(
            json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
            &ctx,
        )
        .await
        .unwrap();
    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.contains("workspace(action='status')"),
        "RW hint should promote workspace status, got: {hint}"
    );
}

#[tokio::test]
async fn activate_project_single_project_no_workspace() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    assert!(
        result["workspace"].is_null(),
        "single-project should have null workspace"
    );
}

#[tokio::test]
async fn activate_project_focus_switch_returns_full_response() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a sub-project
    let sub = root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Focus-switch by ID
    let result = ActivateProject
        .call(json!({"path": "api"}), &ctx)
        .await
        .unwrap();

    assert_eq!(result["status"], "ok");
    assert!(result["project"].is_string(), "should have project name");
    assert!(result["languages"].is_array(), "should have languages");
    assert!(result["memories"].is_array(), "should have memories");
    assert!(result["index"].is_object(), "should have index");
    assert!(!result["read_only"].is_null(), "should have read_only");
}

#[tokio::test]
async fn activate_project_workspace_includes_depends_on() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let sub_a = root.join("packages").join("core");
    let sub_b = root.join("packages").join("web");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();
    std::fs::write(
        sub_a.join("package.json"),
        r#"{"name":"core","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();
    std::fs::write(
        sub_b.join("package.json"),
        r#"{"name":"web","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();

    let ctx = ToolContext {
        agent: Agent::new(Some(root)).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    if let Some(ws) = result["workspace"].as_array() {
        for entry in ws {
            assert!(
                entry["depends_on"].is_array(),
                "each workspace entry should have depends_on"
            );
        }
    }
}

#[tokio::test]
async fn activate_project_ro_hint_warns_switch_back() {
    let home = tempdir().unwrap();
    let other = tempdir().unwrap();
    std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
    std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Activate home first
    ActivateProject
        .call(json!({"path": home.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    // Activate other as RO
    let result = ActivateProject
        .call(
            json!({"path": other.path().to_str().unwrap(), "read_only": true}),
            &ctx,
        )
        .await
        .unwrap();

    let hint = result["hint"].as_str().unwrap();
    assert!(
        hint.contains("remember to workspace"),
        "RO hint should warn about switching back, got: {hint}"
    );
    assert!(
        hint.contains("read-only"),
        "RO hint should mention read-only, got: {hint}"
    );
}

#[test]
fn activate_project_auto_libs_is_summary_not_array() {
    let result = json!({
        "status": "ok",
        "project": "test",
        "project_root": "/tmp/test",
        "read_only": false,
        "memories": [],
        "index": {"status": "not_indexed"},
        "auto_registered_libs": {"count": 5, "without_source": 2},
    });
    assert!(result["auto_registered_libs"].is_object());
    assert_eq!(result["auto_registered_libs"]["count"], 5);
    assert_eq!(result["auto_registered_libs"]["without_source"], 2);
}

#[tokio::test]
async fn activate_project_memories_graceful_on_error() {
    // A project with no .codescout dir should still activate with memories: []
    let dir = tempdir().unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
        .await
        .unwrap();
    let memories = result["memories"].as_array().unwrap();
    assert!(
        memories.is_empty(),
        "empty project should have empty memories array"
    );
}

#[tokio::test]
async fn workspace_action_activate_dispatches_to_activate_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = Workspace
        .call(
            json!({
                "action": "activate",
                "path": dir.path().to_str().unwrap(),
                "read_only": false,
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["status"], "ok");
    assert!(result.get("project_hints").is_some());
}

#[tokio::test]
async fn workspace_action_status_dispatches_to_project_status() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let result = Workspace
        .call(json!({ "action": "status" }), &ctx)
        .await
        .unwrap();
    assert!(result["project_root"].is_string());
    assert!(result["languages"].is_array());
    assert!(result["index"].is_object());
}

#[tokio::test]
async fn workspace_action_list_projects_returns_workspace_field() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let result = Workspace
        .call(json!({ "action": "list_projects" }), &ctx)
        .await
        .unwrap();
    // The result must contain the "workspace" key (value may be null when no
    // workspace.toml is present — that's still a successful list_projects call).
    assert!(result.as_object().unwrap().contains_key("workspace"));
    // And no other fields should leak through.
    assert_eq!(result.as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn workspace_action_unknown_errors() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let err = Workspace
        .call(json!({ "action": "wat" }), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("unknown workspace action"),
        "expected unknown action error, got: {err}"
    );
}

#[tokio::test]
async fn workspace_post_compact_without_action_infers_status() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // post_compact=true without action should infer action='status' and flush LSP
    let result = Workspace
        .call(json!({ "post_compact": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["flushed"], json!(true), "expected flushed:true");
    assert!(result["hint"].is_string(), "expected hint string");
    assert!(
        result.get("project_root").is_none(),
        "compact flush must not include status fields"
    );

    // missing action without post_compact must still error
    let err = Workspace.call(json!({}), &ctx).await.unwrap_err();
    assert!(
        err.to_string().contains("workspace requires 'action'"),
        "expected missing-action error, got: {err}"
    );
}

#[tokio::test]
async fn activation_response_includes_stale_warning_when_no_stored_version() {
    // No project.toml → onboarding_version = None → stale
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let stale = &result["system_prompt_stale"];
    assert!(
        stale.is_object(),
        "system_prompt_stale missing; got: {result}"
    );
    assert!(
        stale["stored_version"].is_null(),
        "stored_version should be null for None"
    );
    assert_eq!(
        stale["current_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION as u64
    );
    assert!(
        stale["action"].as_str().unwrap().contains("refresh_prompt"),
        "action should mention refresh_prompt"
    );
}

#[tokio::test]
async fn activation_response_emits_legacy_index_when_db_present() {
    let dir = tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout/embeddings")).unwrap();
    let legacy_db = root.join(".codescout/embeddings/project.db");
    std::fs::write(&legacy_db, b"-- sqlite placeholder").unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({ "path": root.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let legacy = &result["legacy_semantic_index"];
    assert!(
        legacy.is_object(),
        "expected legacy_semantic_index field; got: {result}"
    );
    assert_eq!(
        legacy["path"].as_str().unwrap(),
        crate::util::fs::to_forward_slash(&legacy_db)
    );
    assert!(legacy["hint"]
        .as_str()
        .unwrap()
        .contains("migrate-memories"));
}

#[tokio::test]
async fn activation_response_omits_legacy_index_when_db_absent() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert!(
        result["legacy_semantic_index"].is_null(),
        "expected no legacy_semantic_index when db absent; got: {result}"
    );
}

#[tokio::test]
async fn activation_response_no_stale_warning_when_version_current() {
    let dir = tempdir().unwrap();
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    // Write project.toml with current onboarding version
    std::fs::write(
        cs_dir.join("project.toml"),
        format!(
            "[project]\nname = \"test\"\nlanguages = []\nonboarding_version = {}\n",
            crate::tools::onboarding::ONBOARDING_VERSION
        ),
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    assert!(
        result["system_prompt_stale"].is_null(),
        "system_prompt_stale should be absent; got: {result}"
    );
}

#[tokio::test]
async fn activation_response_includes_stale_warning_when_version_outdated() {
    let dir = tempdir().unwrap();
    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    // Write project.toml with an outdated onboarding version
    std::fs::write(
        cs_dir.join("project.toml"),
        format!(
            "[project]\nname = \"test\"\nlanguages = []\nonboarding_version = {}\n",
            crate::tools::onboarding::ONBOARDING_VERSION.saturating_sub(1)
        ),
    )
    .unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    let result = ActivateProject
        .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let stale = &result["system_prompt_stale"];
    assert!(
        stale.is_object(),
        "system_prompt_stale missing; got: {result}"
    );
    assert_eq!(
        stale["stored_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION.saturating_sub(1) as u64,
        "stored_version should reflect the outdated version"
    );
    assert_eq!(
        stale["current_version"].as_u64().unwrap(),
        crate::tools::onboarding::ONBOARDING_VERSION as u64
    );
}

#[test]
fn format_activate_project_prepends_warning_when_stale() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "system_prompt_stale": {
            "stored_version": 20,
            "current_version": 23,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ SYSTEM PROMPT STALE (v20 → v23):"),
        "compact should start with stale warning but was: {compact}"
    );
    assert!(
        compact.contains("activated · my-project (rw)"),
        "compact should still contain activation summary but was: {compact}"
    );
}

#[test]
fn format_activate_project_no_warning_when_current() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": ["arch"],
        "index": {"status": "not_indexed"},
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        !compact.contains("STALE"),
        "no stale warning expected but was: {compact}"
    );
    assert_eq!(
        compact,
        "activated · my-project (rw) · 1 memories · index: not_indexed"
    );
}

#[test]
fn format_activate_project_prepends_warning_with_none_stored_version() {
    let result = json!({
        "status": "ok",
        "project": "my-project",
        "project_root": "/home/user/my-project",
        "read_only": false,
        "memories": [],
        "index": {"status": "not_indexed"},
        "system_prompt_stale": {
            "stored_version": null,
            "current_version": 23,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        },
        "hint": "CWD: /home/user/my-project"
    });
    let compact = format_activate_project(&result);
    assert!(
        compact.starts_with("⚠ SYSTEM PROMPT STALE (none → v23):"),
        "should show 'none' not 'v0' for null stored_version; got: {compact}"
    );
}

/// The index-status cache state machine, exercised without touching the network.
///
/// This used to drive step 1 through a real probe and assert it completed, on the
/// unstated premise that "the stack is offline in tests => false, fast". That premise
/// is false in the shipped configuration: `cargo rb` compiles `server-stack`, and with
/// a reachable Qdrant the probe does real work. It then failed for a real reason (the
/// probe enumerated the whole corpus — see
/// `docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`),
/// and after that fix it still failed under full-suite load, because "a network round
/// trip finishes inside two seconds" is not a property a unit test controls.
///
/// What this test is actually about is the cache logic, so it drives that directly:
/// `resolve_first_probe` is already a pure function over the probe outcome.
#[tokio::test]
#[serial_test::serial]
async fn index_status_cache_serves_stale_then_refreshes() {
    // Unique key so the process-global cache can't collide across tests.
    let pid = format!("cache-sandwich-{}", std::process::id());
    super::index_status_remove(&pid);

    // 1. A completed probe is cached, definitively.
    assert!(!super::resolve_first_probe(&pid, Some(false)));
    assert_eq!(super::index_status_get(&pid), Some(false));

    // 1b. A timed-out probe reports false but must NOT be cached, so the next
    //     activation re-probes instead of serving a poisoned negative. This is the
    //     branch the old test reached by accident and misread as a failure.
    super::index_status_remove(&pid);
    assert!(!super::resolve_first_probe(&pid, None));
    assert_eq!(
        super::index_status_get(&pid),
        None,
        "a timeout must not cache"
    );

    // 2. Assert-STALE: seed true; the cached value is returned on a cache hit without
    //    an eager re-probe. Regression step — fails if the cache is ever bypassed.
    super::index_status_put(&pid, true);
    assert!(super::check_has_index_cached(&pid, &std::env::temp_dir()).await);

    // 3. Invalidate: the entry is gone, so the next call would re-probe.
    super::index_status_remove(&pid);
    assert_eq!(super::index_status_get(&pid), None);
}
#[test]
fn first_probe_timeout_is_not_cached() {
    // A first-probe TIMEOUT (None) must NOT be cached as a definitive
    // `false` — caching it poisons the session cache and reports
    // not_indexed for a fully queryable index until a later refresh lands.
    // A COMPLETED probe (Some) is still cached.
    let pid = "test-first-probe-timeout-not-cached";
    super::index_status_remove(pid);

    // Timed-out probe: returns false for this call but leaves the cache
    // empty so the next activation re-probes.
    assert!(!super::resolve_first_probe(pid, None));
    assert_eq!(
        super::index_status_get(pid),
        None,
        "a timed-out first probe must NOT be cached"
    );

    // Completed probe: cached.
    assert!(super::resolve_first_probe(pid, Some(true)));
    assert_eq!(super::index_status_get(pid), Some(true));

    super::index_status_remove(pid);
}

#[test]
fn refresh_in_flight_guard_is_exclusive_per_project() {
    let pid = format!("refresh-guard-{}", std::process::id());
    assert!(super::refresh_begin(&pid), "first acquire must succeed");
    assert!(
        !super::refresh_begin(&pid),
        "second acquire while in flight must fail"
    );
    super::refresh_end(&pid);
    assert!(super::refresh_begin(&pid), "acquire after end must succeed");
    super::refresh_end(&pid);
}
#[tokio::test]
async fn workspace_activate_injects_bootstrap_guide_body_v2() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let blocks = Workspace
        .call_content(
            json!({ "action": "activate", "path": dir.path().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(
        blocks.len(),
        2,
        "first workspace(activate) must append the bootstrap guide body block, got {}",
        blocks.len()
    );
    let second = blocks[1].as_text().expect("second block must be text");
    assert!(
        second
            .text
            .contains("<!-- auto-injected get_guide('project-activation-bootstrap')"),
        "second block missing the auto-inject opening marker"
    );
}

/// `workspace(activate)`'s `memories` array and `memory(action="list", project_id=…)`
/// must describe the same directory. For a SUB-project they did not.
///
/// `activate` read `<sub_root>/.codescout/memories` via `MemoryStore::open(p.root)`,
/// while every branch of the live `memory` tool routes through
/// `Workspace::memory_dir_for_project`, which puts a non-root project's memories at
/// `<workspace_root>/.codescout/projects/<id>/memories`. The two paths coincide only
/// when `relative_root == "."`, which is why a single-project repo never reproduced it
/// and why two earlier verify-open passes on the home project could not clear the bug.
///
/// The WRITE path settles which location is canonical, and it was never in doubt:
/// `memory(action="write")` stores through `resolve_memory_dir`, so the per-project
/// tree is where data actually lands — 53 git-tracked files across 9 directories on
/// this repo when this was fixed. The reader was the outlier, so the readers move.
/// No migration.
///
/// The last assertion covers the side effect rather than the symptom: `MemoryStore`
/// creates its directory on open, so the old reader did not merely miss the memories,
/// it materialised an empty `<sub_root>/.codescout/memories` on the way past — leaving
/// a directory that looks like an empty memory set for a project that holds topics.
///
/// docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
#[tokio::test]
async fn activating_a_sub_project_lists_the_memories_the_memory_tool_writes() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    // Exactly where memory(action="write", project_id="svc") puts a topic.
    // Deliberately NOT svc/.codescout/memories — that is the path under test.
    let per_project = codescout.join("projects").join("svc").join("memories");
    std::fs::create_dir_all(&per_project).unwrap();
    std::fs::write(per_project.join("architecture.md"), "# Arch").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let activated = ActivateProject
        .call(json!({ "path": "svc" }), &ctx)
        .await
        .unwrap();
    let listed = crate::tools::memory::Memory
        .call(json!({ "action": "list", "project_id": "svc" }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        activated["memories"],
        json!(["architecture"]),
        "activate must report the sub-project's real memory set: {activated:?}"
    );
    assert_eq!(
        activated["memories"], listed["topics"],
        "the two surfaces must describe one directory — a caller told 0 by one and 1 \
         by the other cannot tell which is lying: activate={:?} memory={:?}",
        activated["memories"], listed["topics"]
    );
    assert!(
        !svc.join(".codescout").join("memories").exists(),
        "reading the wrong path also CREATED it — MemoryStore makes its directory on \
         open, so the miss leaves behind an empty dir that looks like an empty memory set"
    );
}

/// The sibling test above covers the bare-id route; this one covers the PATH route.
/// `ActivateProject` takes the focus-switch branch when `path` names a member of the
/// current workspace — by bare id, or by a path resolving to that project's root.
///
/// Before the fix an absolute path fell through to root resolution and built a
/// STANDALONE workspace at the target, where the sub-project is its own root, so
/// memories resolved to `<sub_root>/.codescout/memories` — the directory nothing
/// writes to for a workspace member. The response carried no `workspace` array at
/// all, which was the tell.
///
/// No reader could repair it downstream: `Agent::activate` calls
/// `inner.workspaces.clear()`, so the parent workspace and its per-project tree are
/// gone before any reader runs. That is why the read-union shipped for the id route
/// (`020ea69a`) does not reach here, and why this needed a dispatch change.
///
/// The costs this fix was deferred on were re-measured before it landed, and one was
/// wrong: `read_only` does NOT move — `Agent::activate` and
/// `activate_within_workspace` derive it with the same `explicit > home > read-only`
/// match. Nor does the "a path is how you browse a foreign repo" argument apply: a
/// foreign repo is by definition not a member of the loaded workspace and cannot
/// match this branch. What does move is residency and the response shape, both
/// pinned by the assertions below.
///
/// docs/issues/archive/2026-08-27-activate-by-path-bypasses-workspace-memory-resolution.md
#[tokio::test]
async fn activating_a_sub_project_by_path_lists_the_same_memories() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let per_project = codescout.join("projects").join("svc").join("memories");
    std::fs::create_dir_all(&per_project).unwrap();
    std::fs::write(per_project.join("architecture.md"), "# Arch").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let activated = ActivateProject
        .call(json!({ "path": svc.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        activated["memories"],
        json!(["architecture"]),
        "by-path activation must resolve the same memory set as by-id: {activated:?}"
    );
    // The two things that DO move, pinned so a later change cannot quietly undo the
    // dispatch while leaving the memory assertion satisfied some other way.
    assert!(
        activated["workspace"].is_array(),
        "the response must now carry the parent topology — its absence was the tell \
         that a standalone workspace had been built: {activated:?}"
    );
    assert_eq!(
        activated["read_only"],
        json!(true),
        "read_only must NOT move: both paths derive it with the same \
         `explicit > home > read-only` match, and the deferral claimed otherwise. \
         Got: {activated:?}"
    );
}

/// Negative control for the test above: a path that is NOT a member of the current
/// workspace must still build a standalone workspace.
///
/// Without this, a dispatch that routed EVERY path into `activate_within_workspace`
/// would satisfy the sibling test while destroying the excursion semantics the
/// read-only hint assumes — "Browsing X (read-only) … remember to activate the parent
/// when done" reads as a trip out, not a focus switch. It is also the assertion behind
/// the claim that this fix leaves foreign-repo browsing untouched.
#[tokio::test]
async fn activating_a_non_member_path_still_builds_a_standalone_workspace() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    // A wholly separate repo — no relation to the workspace above.
    let outside = tempdir().unwrap();
    std::fs::write(
        outside.path().join("Cargo.toml"),
        "[package]\nname = \"x\"\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let activated = ActivateProject
        .call(json!({ "path": outside.path().to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        activated["project_root"],
        json!(crate::util::fs::to_forward_slash(
            &outside.path().canonicalize().unwrap()
        )),
        "a non-member path must activate the target itself: {activated:?}"
    );
    assert!(
        activated["hint"]
            .as_str()
            .unwrap_or_default()
            .contains("remember to workspace"),
        "the excursion hint must survive — this is the browse-a-foreign-repo path: \
         {activated:?}"
    );
}

/// The workspace ROOT addressed by absolute path must keep taking FULL activation,
/// not the new focus-switch branch.
///
/// This pins the one exclusion in that branch's filter, and it exists because the
/// exclusion was otherwise unfalsifiable: removing it left all 4466 lib tests green.
/// A guard nothing can fail is not a guard, so here is the thing it protects.
///
/// "Return home by absolute path" is the commonest activate call there is, and full
/// activation is what re-arms the guide ledger (`config/mod.rs`: with no rendezvous
/// the ledger is cleared outright, because a `/clear` is otherwise invisible to the
/// server). Routing the root through the focus-switch early return would skip that
/// silently, starving a fresh conversation of every guide the previous one had
/// already seen — a failure with no error and no wrong number, just guidance that
/// never arrives.
///
/// There is also nothing to gain: the root project's two memory layouts coincide by
/// construction (`memory_dir_for_project` returns `<ws>/.codescout/memories` for
/// `relative_root == "."`), so the bug this branch fixes cannot occur for it.
///
/// docs/issues/archive/2026-08-27-activate-by-path-bypasses-workspace-memory-resolution.md
#[tokio::test]
async fn activating_the_workspace_root_by_path_still_takes_full_activation() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    // Seed the ledger. Only the full-activation path clears it.
    ctx.guide_hints_emitted
        .lock()
        .insert("librarian".to_string());
    assert!(
        ctx.guide_hints_emitted.lock().contains("librarian"),
        "precondition: the ledger is seeded"
    );

    ActivateProject
        .call(
            json!({ "path": root.canonicalize().unwrap().to_str().unwrap() }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        !ctx.guide_hints_emitted.lock().contains("librarian"),
        "the workspace root by path must take FULL activation, which re-arms the \
         guide ledger. A surviving entry means it took the focus-switch early \
         return, which never touches the ledger — so a fresh conversation would \
         silently never receive guides the previous one had consumed."
    );
}

/// `workspace(activate)`'s `memories` array reports the UNION of a sub-project's
/// two memory layouts, and `memory(action="list")` returns the same set.
///
/// The sibling fix above moved `activate`'s reader onto the write path, which made
/// the two surfaces agree. But for a project whose memories live project-local it
/// made them agree at ZERO — and the pre-fix contradiction had at least been a
/// signal that memories existed on disk. Measured 2026-08-27 across two real
/// workspaces on one machine: one held 53 memories project-local and 0 in the
/// workspace tree, the other 42 in the workspace tree and none project-local.
/// Neither layout is debris, so reads union both and writes are left untouched —
/// no file moves, none is untracked, no write lands anywhere new.
///
/// The last assertion is the one that keeps this honest: agreement is cheap to
/// fake by making both surfaces equally blind, so the test pins the COUNT too.
///
/// docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
#[tokio::test]
async fn activating_a_sub_project_reports_the_union_of_both_memory_layouts() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    // One topic in each layout. Before the union, `activate` and `memory(list)`
    // agreed on the workspace-tree one alone and reported the project-local one
    // as not existing.
    let per_project = codescout.join("projects").join("svc").join("memories");
    std::fs::create_dir_all(&per_project).unwrap();
    std::fs::write(per_project.join("architecture.md"), "# Arch").unwrap();
    let project_local = svc.join(".codescout").join("memories");
    std::fs::create_dir_all(&project_local).unwrap();
    std::fs::write(project_local.join("conventions.md"), "# Conv").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let activated = ActivateProject
        .call(json!({ "path": "svc" }), &ctx)
        .await
        .unwrap();
    let listed = crate::tools::memory::Memory
        .call(json!({ "action": "list", "project_id": "svc" }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        activated["memories"],
        json!(["architecture", "conventions"]),
        "activate must report both layouts, not whichever one its reader happens \
         to resolve: {activated:?}"
    );
    assert_eq!(
        activated["memories"], listed["topics"],
        "the two surfaces must describe the same set: activate={:?} memory={:?}",
        activated["memories"], listed["topics"]
    );
    assert_eq!(
        activated["memories"].as_array().map(Vec::len),
        Some(2),
        "agreement at the wrong count is the failure this replaced — both surfaces \
         blind in the same direction still agree: {activated:?}"
    );
}

/// The `## Project Status` block appended to tool responses must report the same
/// memory set as the activation JSON it rides beside.
///
/// `Agent::project_status` is a FOURTH reader of a project's memories, after
/// `activate`'s array, `memory(action="list")` and `p.memory` itself — and it was
/// the one left behind. Caught live rather than by test: activating
/// `eduplanner-ui` returned a JSON `memories` array of twelve with
/// "**Memories:** None yet — run `onboarding` to create project memories"
/// rendered directly underneath it, in the same message. The unit tests could not
/// see it because they assert on `.call()`'s `Value`, and this block is appended
/// by the server around it.
///
/// docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md
#[tokio::test]
async fn the_project_status_block_reports_the_same_memories_as_activation() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    // Only the project-local layout is populated — the live shape that produced
    // "None yet": `p.memory` is workspace-resolved on the focus-switch path, and
    // the workspace tree is empty.
    let project_local = svc.join(".codescout").join("memories");
    std::fs::create_dir_all(&project_local).unwrap();
    std::fs::write(project_local.join("architecture.md"), "# Arch").unwrap();
    std::fs::write(project_local.join("gotchas.md"), "# Got").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let activated = ActivateProject
        .call(json!({ "path": "svc" }), &ctx)
        .await
        .unwrap();
    let status = ctx.agent.project_status().await.expect("project status");

    assert_eq!(
        status.memories,
        vec!["architecture".to_string(), "gotchas".to_string()],
        "the status block must see the project-local layout too: {:?}",
        status.memories
    );
    assert_eq!(
        activated["memories"],
        json!(status.memories),
        "the status block and the activation JSON ride in the SAME message — a \
         caller reading one and then the other cannot tell which is lying: \
         activate={:?} status={:?}",
        activated["memories"],
        status.memories
    );
}
