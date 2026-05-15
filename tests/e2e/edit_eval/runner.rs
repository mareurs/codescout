use crate::e2e::edit_eval::matchers::{grade, MatchResult, Observation, ReturnObservation};
use crate::e2e::edit_eval::types::EditCase;
use crate::e2e::eval_common::{cargo_check, git_restore, read_fixture_file, Verdict};
use codescout::agent::Agent;
use codescout::lsp::manager::LspManager;
use codescout::tools::symbol::EditCode;
use codescout::tools::{
    output_buffer::OutputBuffer, section_coverage::SectionCoverage, Tool, ToolContext,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE_TIMEOUT: Duration = Duration::from_secs(45);

#[allow(dead_code)]
pub struct EditEvalCtx {
    pub tool_ctx: Arc<ToolContext>,
    pub fixture_root: PathBuf,
    pub fixture_src: PathBuf,
}

#[allow(dead_code)]
pub async fn edit_eval_context() -> EditEvalCtx {
    let fixture_root: PathBuf = std::env::current_dir()
        .expect("cwd")
        .join("tests/fixtures/edit-eval-rust");
    assert!(
        fixture_root.exists(),
        "edit-eval fixture missing: {}",
        fixture_root.display()
    );
    let fixture_src = fixture_root.join("src");

    // Warm the cargo target dir so per-case checks are fast.
    cargo_check(&fixture_root).expect("pre-flight cargo check on edit-eval fixture");

    let agent = Agent::new(Some(fixture_root.clone()))
        .await
        .expect("Agent::new for edit-eval");
    let lsp = LspManager::new_arc();

    let tool_ctx = Arc::new(ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
    });

    EditEvalCtx {
        tool_ctx,
        fixture_root,
        fixture_src,
    }
}

#[allow(dead_code)]
pub async fn run_one(ctx: &EditEvalCtx, case: &EditCase) -> MatchResult {
    let mut last = MatchResult {
        verdict: Verdict::SilentWrong,
        evidence: String::from("no attempts ran"),
    };
    for attempt in 0..6u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
            let _ = git_restore(&ctx.fixture_src);
        }
        let fut = invoke(&ctx.tool_ctx, case);
        let result = match tokio::time::timeout(CASE_TIMEOUT, fut).await {
            Err(_) => {
                return MatchResult {
                    verdict: Verdict::Hung,
                    evidence: format!("exceeded {}s", CASE_TIMEOUT.as_secs()),
                }
            }
            Ok(r) => r,
        };

        let disk = read_fixture_file(&ctx.fixture_src, case.target_file);
        let compiler_ok = cargo_check(&ctx.fixture_root).is_ok();

        let candidate = match &result {
            Ok(v) => {
                let obs = Observation {
                    return_: ReturnObservation::Ok(v),
                    disk: disk.as_deref(),
                    compiler_ok,
                };
                grade(case, obs)
            }
            Err(e) => {
                let msg = format!("{e}");
                let return_obs = if e
                    .downcast_ref::<codescout::tools::RecoverableError>()
                    .is_some()
                {
                    ReturnObservation::Recoverable(msg.as_str())
                } else if msg.contains("content modified") || msg.contains("-32801") {
                    ReturnObservation::TransientLsp(msg.as_str())
                } else {
                    ReturnObservation::Fatal(msg.as_str())
                };
                let obs = Observation {
                    return_: return_obs,
                    disk: disk.as_deref(),
                    compiler_ok,
                };
                grade(case, obs)
            }
        };
        match candidate.verdict {
            Verdict::Correct | Verdict::Partial | Verdict::CleanError | Verdict::Panic => {
                return candidate;
            }
            _ => last = candidate,
        }
    }
    last
}

async fn invoke(ctx: &ToolContext, case: &EditCase) -> anyhow::Result<serde_json::Value> {
    EditCode.call(case.input.clone(), ctx).await
}
