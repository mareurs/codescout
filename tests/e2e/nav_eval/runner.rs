use crate::e2e::nav_eval::matchers::{
    match_call_graph, match_references, match_symbol_at_def, match_symbols, MatchResult,
};
use crate::e2e::nav_eval::types::{Case, Expected, ToolUnderTest, Verdict};
use codescout::agent::Agent;
use codescout::lsp::manager::LspManager;
use codescout::tools::symbol::{CallGraph, References, SymbolAt, Symbols};
use codescout::tools::{Tool, ToolContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a ToolContext rooted at the nav-eval fixture crate.
///
/// Distinct from the language-fixture helper because our fixture does not
/// follow the `<lang>-library` naming convention — it is an adversarial
/// probe, not a language sample.
pub async fn nav_eval_context() -> Arc<ToolContext> {
    let dir: PathBuf = std::env::current_dir()
        .expect("cwd")
        .join("tests/fixtures/nav-eval-rust");
    assert!(dir.exists(), "Nav-eval fixture missing: {}", dir.display());

    // Ensure rust-analyzer has build artifacts to attach to. Stdout/stderr are
    // discarded — `cargo check` failure here will surface as LSP misses, which
    // is the failure mode we want to make visible in the report rather than
    // a panic before the report is written.
    let _ = std::process::Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .status();

    let agent = Agent::new(Some(dir.clone()))
        .await
        .expect("Agent::new for nav-eval");
    let lsp = LspManager::new_arc();

    Arc::new(ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(codescout::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(
            codescout::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    })
}

pub async fn run_one(ctx: &ToolContext, case: &Case) -> MatchResult {
    let mut last = MatchResult {
        verdict: Verdict::SilentWrong,
        evidence: String::from("no attempts ran"),
    };
    for attempt in 0..8u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
        }
        let fut = invoke(ctx, case);
        let candidate = match tokio::time::timeout(CASE_TIMEOUT, fut).await {
            Err(_) => {
                return MatchResult {
                    verdict: Verdict::Hung,
                    evidence: format!("exceeded {}s", CASE_TIMEOUT.as_secs()),
                }
            }
            Ok(result) => grade(case, result),
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

async fn invoke(ctx: &ToolContext, case: &Case) -> anyhow::Result<serde_json::Value> {
    match case.tool {
        ToolUnderTest::Symbols => Symbols.call(case.input.clone(), ctx).await,
        ToolUnderTest::SymbolAt => SymbolAt.call(case.input.clone(), ctx).await,
        ToolUnderTest::References => References.call(case.input.clone(), ctx).await,
        ToolUnderTest::CallGraph => CallGraph.call(case.input.clone(), ctx).await,
    }
}

fn grade(case: &Case, result: anyhow::Result<serde_json::Value>) -> MatchResult {
    match result {
        Err(e) => {
            let is_recoverable = e
                .downcast_ref::<codescout::tools::RecoverableError>()
                .is_some();
            if is_recoverable {
                MatchResult {
                    verdict: Verdict::CleanError,
                    evidence: format!("RecoverableError: {e}"),
                }
            } else {
                let msg = format!("{e}");
                if msg.contains("content modified") || msg.contains("-32801") {
                    MatchResult {
                        verdict: Verdict::SilentWrong,
                        evidence: format!("transient LSP race (retryable): {msg}"),
                    }
                } else {
                    MatchResult {
                        verdict: Verdict::Panic,
                        evidence: format!("fatal: {msg}"),
                    }
                }
            }
        }
        Ok(value) => match &case.expected {
            Expected::Symbols {
                must_include,
                must_not_include,
            } => match_symbols(&value, must_include, must_not_include),
            Expected::SymbolAtDef { file, line } => match_symbol_at_def(&value, file, *line),
            Expected::References {
                must_include,
                must_not_include,
                min_count,
            } => match_references(&value, must_include, must_not_include, *min_count),
            Expected::CallGraph {
                must_include_edges,
                must_not_include_edges,
            } => match_call_graph(&value, must_include_edges, must_not_include_edges),
            Expected::NoResult => MatchResult {
                verdict: Verdict::SilentWrong,
                evidence: format!("expected RecoverableError; got Ok: {value}"),
            },
        },
    }
}
