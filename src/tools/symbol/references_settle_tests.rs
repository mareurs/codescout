//! `references` on a cold language server: the first call WAITS for the server to settle instead of
//! returning a partial answer plus a "re-run" warning
//! (`docs/issues/archive/2026-11-07-first-references-call-after-start-returns-a-partial-result-and-a-retry-returns-the-rest.md`).
//!
//! Every test drives `References::call_with_settle`, the seam that takes the deadline and poll
//! interval, with millisecond values. The mock answers from a script, so the attempt count is read
//! off the mock (`references_calls`), not inferred from timing.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::tempdir;

use crate::agent::Agent;
use crate::lsp::mock::{MockLspClient, MockLspProvider};
use crate::lsp::ops::LspClientOps;
use crate::lsp::{SymbolInfo, SymbolKind};
use crate::tools::symbol::references::References;
use crate::tools::ToolContext;

const POLL: Duration = Duration::from_millis(10);
/// Room for a loaded CI box on top of a deadline; far below the 45 s production wait.
const SLACK: Duration = Duration::from_millis(3_000);

struct Fixture {
    dir: tempfile::TempDir,
    ctx: ToolContext,
    client: Arc<MockLspClient>,
}

fn location(file: &std::path::Path, line: u32) -> lsp_types::Location {
    lsp_types::Location {
        uri: url::Url::from_file_path(file)
            .unwrap()
            .as_str()
            .parse::<lsp_types::Uri>()
            .unwrap(),
        range: lsp_types::Range {
            start: lsp_types::Position { line, character: 3 },
            end: lsp_types::Position { line, character: 9 },
        },
    }
}

/// A project whose `helper` is declared in `src/lib.rs`. `other_uses_helper` decides whether the
/// text scan can corroborate a use in `src/other.rs`. The mock starts as a FRESH client
/// (`with_references_warmup_pending`), and `script` is its scripted `references` answers.
async fn fixture(
    other_uses_helper: bool,
    script: impl FnOnce(&lsp_types::Location, &lsp_types::Location) -> Vec<Vec<lsp_types::Location>>,
) -> Fixture {
    fixture_scripted(other_uses_helper, |d, e| {
        script(d, e).into_iter().map(Ok).collect()
    })
    .await
}

/// [`fixture`] where a scripted answer may be an error (`Err(message)`).
async fn fixture_scripted(
    other_uses_helper: bool,
    script: impl FnOnce(
        &lsp_types::Location,
        &lsp_types::Location,
    ) -> Vec<Result<Vec<lsp_types::Location>, String>>,
) -> Fixture {
    // The mock starts as a freshly started client whose window is open and that reports no
    // progress: the pyright case. A test about a client long past start-up closes the window with
    // `fixture_with`.
    fixture_with(other_uses_helper, script, |m| m).await
}

/// The project behind every fixture: `helper` declared in `src/lib.rs`, and `src/other.rs`, which
/// uses it when `other_uses_helper`. Returns the (canonical) `lib.rs` and `other.rs` paths and the
/// symbol the mock serves for `lib.rs`.
fn project(
    other_uses_helper: bool,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    SymbolInfo,
) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let src = std::fs::canonicalize(dir.path()).unwrap().join("src");
    std::fs::create_dir_all(&src).unwrap();
    let lib = src.join("lib.rs");
    std::fs::write(&lib, "fn helper() {}\n").unwrap();
    let other = src.join("other.rs");
    std::fs::write(
        &other,
        if other_uses_helper {
            "fn caller() { helper(); }\n"
        } else {
            "fn caller() {}\n"
        },
    )
    .unwrap();
    let helper = SymbolInfo {
        name: "helper".to_string(),
        name_path: "helper".to_string(),
        kind: SymbolKind::Function,
        file: lib.clone(),
        start_line: 0,
        end_line: 0,
        start_col: 3,
        children: vec![],
        range_start_line: None,
        detail: None,
    };
    (dir, lib, other, helper)
}

/// A tool context over `dir` whose provider hands out `clients` in order (the last repeats).
async fn context_over(dir: &std::path::Path, clients: Vec<Arc<MockLspClient>>) -> ToolContext {
    ToolContext {
        agent: Agent::new(Some(dir.to_path_buf())).await.unwrap(),
        lsp: MockLspProvider::with_shared_clients(clients),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    }
}

/// [`fixture_scripted`] with a hook to adjust the mock (window, delays) before it is shared.
async fn fixture_with(
    other_uses_helper: bool,
    script: impl FnOnce(
        &lsp_types::Location,
        &lsp_types::Location,
    ) -> Vec<Result<Vec<lsp_types::Location>, String>>,
    tune: impl FnOnce(MockLspClient) -> MockLspClient,
) -> Fixture {
    fixture_configured(other_uses_helper, script, tune, None).await
}

/// [`fixture_with`] with the project's `tool_timeout_secs` set in `.codescout/project.toml` before
/// the agent loads it (`None` keeps the default of 60 s).
async fn fixture_configured(
    other_uses_helper: bool,
    script: impl FnOnce(
        &lsp_types::Location,
        &lsp_types::Location,
    ) -> Vec<Result<Vec<lsp_types::Location>, String>>,
    tune: impl FnOnce(MockLspClient) -> MockLspClient,
    tool_timeout_secs: Option<u64>,
) -> Fixture {
    let (dir, lib, other, helper) = project(other_uses_helper);
    if let Some(secs) = tool_timeout_secs {
        std::fs::write(
            dir.path().join(".codescout").join("project.toml"),
            format!("[project]\nname = \"settle-test\"\ntool_timeout_secs = {secs}\n"),
        )
        .unwrap();
    }
    let answers = script(&location(&lib, 0), &location(&other, 0));
    let client = Arc::new(tune(
        MockLspClient::new()
            .with_symbols(&lib, vec![helper])
            .with_references_script(answers)
            .with_references_warmup_pending(),
    ));
    let ctx = context_over(dir.path(), vec![Arc::clone(&client)]).await;
    Fixture { dir, ctx, client }
}

async fn call(fx: &Fixture, deadline: Duration) -> (Value, Duration) {
    let started = Instant::now();
    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            deadline,
            POLL,
        )
        .await
        .unwrap();
    (result, started.elapsed())
}

fn warning(result: &Value) -> String {
    result["completeness_warning"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn attempts(fx: &Fixture) -> usize {
    fx.client.references_calls.load(Ordering::SeqCst)
}

/// (a) A cold server answers nothing, then only the declaration, then the full set. ONE tool call
/// returns the full set with no warning, after exactly three attempts.
#[tokio::test]
async fn first_call_waits_through_both_unsettled_signals_and_returns_the_full_set() {
    let fx = fixture(true, |d, e| {
        vec![vec![], vec![d.clone()], vec![d.clone(), e.clone()]]
    })
    .await;

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(
        result.get("completeness_warning").is_none(),
        "a settled result carries no warning: {result}"
    );
    assert_eq!(
        attempts(&fx),
        3,
        "K=2 unsettled answers, then the settled one"
    );
    assert!(
        elapsed < SLACK,
        "settling must not cost the deadline: {elapsed:?}"
    );
    assert_eq!(
        fx.client.prepare_call_hierarchy_calls.lock().unwrap().len(),
        1,
        "a settled call still runs its cross-check"
    );
    assert!(fx.client.references_warmup_done());
}

/// (b) A server that never settles: the call returns at about the deadline with the partial
/// result, the old warning, and a note that the tool already waited. Attempts stay bounded.
#[tokio::test]
async fn first_call_on_a_server_that_never_settles_returns_at_the_deadline_with_a_waited_note() {
    let fx = fixture(true, |d, _| vec![vec![d.clone()]; 10_000]).await;
    let deadline = Duration::from_millis(300);

    let (result, elapsed) = call(&fx, deadline).await;

    assert!(
        elapsed >= deadline - Duration::from_millis(50),
        "{elapsed:?}"
    );
    assert!(elapsed < deadline + SLACK, "{elapsed:?}");
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(
        w.contains("waited"),
        "the caller must learn a retry will not help yet: {w}"
    );
    assert!(
        !w.contains("Re-run"),
        "the message must not both say the call waited and tell the caller to re-run: {w}"
    );
    let n = attempts(&fx);
    assert!((2..=40).contains(&n), "bounded attempts, got {n}");
    assert!(
        fx.client.references_warmup_done(),
        "the deadline case marks the wait spent"
    );
}

/// (c) A second call on the same client after (b) does not wait: one more attempt, the plain
/// warning, no "waited" note.
#[tokio::test]
async fn second_call_after_a_deadline_hit_does_not_wait_again() {
    let fx = fixture(true, |d, _| vec![vec![d.clone()]; 10_000]).await;
    let _ = call(&fx, Duration::from_millis(200)).await;
    let before = attempts(&fx);

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert!(elapsed < SLACK, "{elapsed:?}");
    assert_eq!(attempts(&fx), before + 1);
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(!w.contains("waited"), "{w}");
}

/// (d) A genuinely unused symbol: the declaration alone, and no other file names it. Nothing to
/// wait for, so one attempt, no warning, and the wait is marked spent.
#[tokio::test]
async fn first_call_on_a_genuinely_unused_symbol_does_not_wait() {
    let fx = fixture(false, |d, _| vec![vec![d.clone()]; 10_000]).await;

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert!(elapsed < SLACK, "{elapsed:?}");
    assert_eq!(attempts(&fx), 1);
    assert!(result.get("completeness_warning").is_none(), "{result}");
    assert!(fx.client.references_warmup_done());
}

/// (e) The wait is for the FIRST call only: after a settled first call, a result that looks
/// unsettled is returned at once, with the plain warning.
#[tokio::test]
async fn a_settled_first_call_means_a_later_unsettled_looking_result_does_not_wait() {
    let fx = fixture(true, |d, e| {
        let mut script = vec![vec![d.clone(), e.clone()]];
        script.extend(vec![vec![d.clone()]; 10_000]);
        script
    })
    .await;
    let (first, _) = call(&fx, Duration::from_secs(30)).await;
    assert_eq!(first["total"].as_u64(), Some(2), "{first}");
    assert!(fx.client.references_warmup_done());
    let before = attempts(&fx);

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert!(elapsed < SLACK, "{elapsed:?}");
    assert_eq!(attempts(&fx), before + 1);
    assert!(
        warning(&result).contains("outside the definition file"),
        "{result}"
    );
}

/// (f) The text scan being incomplete is not a warming signal: an unreadable candidate file is
/// reported at once, without spending the wait.
#[tokio::test]
async fn an_incomplete_corroboration_scan_does_not_trigger_the_wait() {
    let fx = fixture(false, |d, _| vec![vec![d.clone()]; 10_000]).await;
    std::fs::write(
        fx.dir.path().join("src").join("latin1.rs"),
        b"// caf\xe9\nfn caller() { helper(); }\n".as_slice(),
    )
    .unwrap();

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert!(elapsed < SLACK, "{elapsed:?}");
    assert_eq!(attempts(&fx), 1);
    assert!(
        warning(&result).contains("could not read or parse"),
        "{result}"
    );
}

/// (g) The PRODUCTION path: `Tool::call` with the real constants, under paused time, so a 45 s wait
/// takes no wall time. The mock never settles, so the call must wait the whole
/// `FIRST_CALL_SETTLE_WAIT`, ask once per `SETTLE_POLL_INTERVAL` (500 ms: 1 + 89 attempts, the
/// mock answers instantly), say it waited "45.0 s", and spend the client's wait. A swapped or
/// zeroed constant in `Tool::call` changes the attempt count or the note.
///
/// Paused time is feasible here because nothing inside the waiting loop touches the blocking
/// pool: the mock answers inline and the corroboration scan is synchronous `std::fs`, so
/// auto-advance only ever jumps over the loop's own `sleep`.
#[tokio::test(start_paused = true)]
async fn tool_call_on_a_server_that_never_settles_waits_the_production_45_seconds() {
    use crate::tools::Tool;
    let fx = fixture(true, |d, _| vec![vec![d.clone()]; 10_000]).await;

    let result = References
        .call(json!({ "symbol": "helper", "path": "src/lib.rs" }), &fx.ctx)
        .await
        .unwrap();

    let w = warning(&result);
    assert!(w.contains("already waited 45.0 s"), "{w}");
    assert!(!w.contains("Re-run"), "{w}");
    let n = attempts(&fx);
    assert!(
        (85..=95).contains(&n),
        "45 s / 500 ms is about 90 attempts, got {n}"
    );
    assert!(fx.client.references_warmup_done());
}

/// (h) The wait is paid INSIDE the server's tool timeout (`tool_timeout_secs`, 60 s by default),
/// together with the bounded call-hierarchy cross-check. They must fit under it with the margin
/// the tool keeps for answering.
#[test]
fn the_first_call_wait_and_the_cross_check_fit_under_the_default_tool_timeout() {
    use crate::tools::symbol::references::{
        CALL_HIERARCHY_BOUND, FIRST_CALL_SETTLE_WAIT, TOOL_TIMEOUT_MARGIN,
    };
    let tool_timeout = Duration::from_secs(
        crate::config::project::ProjectConfig::default_for("x".to_string())
            .project
            .tool_timeout_secs,
    );
    assert_eq!(
        tool_timeout,
        Duration::from_secs(crate::config::project::DEFAULT_TOOL_TIMEOUT_SECS)
    );

    assert!(
        FIRST_CALL_SETTLE_WAIT + CALL_HIERARCHY_BOUND + TOOL_TIMEOUT_MARGIN <= tool_timeout,
        "{FIRST_CALL_SETTLE_WAIT:?} + {CALL_HIERARCHY_BOUND:?} + {TOOL_TIMEOUT_MARGIN:?} must fit \
         in {tool_timeout:?}"
    );
}

/// (k) A client PAST its cold-start window that answers the definition file alone is believed at
/// once, even though another file names the symbol: a used file-local `helper` on a warm server
/// must not block a whole wait. The warning is today's, with no "waited" note, and the wait is
/// spent.
#[tokio::test]
async fn a_client_past_its_cold_start_window_is_believed_without_a_wait() {
    let fx = fixture_with(
        true,
        |d, _| vec![Ok(vec![d.clone()]); 10_000],
        |m| m.with_cold_start_window(false),
    )
    .await;

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(attempts(&fx), 1);
    assert!(elapsed < SLACK, "{elapsed:?}");
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(!w.contains("waited"), "{w}");
    assert!(fx.client.references_warmup_done());
}

/// (l) No location at all contradicts the request itself, so it waits whether or not the client
/// is inside its cold-start window.
#[tokio::test]
async fn no_locations_at_all_waits_even_past_the_cold_start_window() {
    let fx = fixture_with(
        true,
        |d, e| vec![Ok(vec![]), Ok(vec![d.clone(), e.clone()])],
        |m| m.with_cold_start_window(false),
    )
    .await;

    let (result, _) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(result.get("completeness_warning").is_none(), "{result}");
    assert_eq!(attempts(&fx), 2);
}

/// (s) The REPORTED case (pyright): a server that never reports progress, answering the definition
/// file alone while the project text names the symbol elsewhere, on a client inside its cold-start
/// window. The window is the only gate, so the call waits, and the next attempt's full set is
/// returned in ONE call with no warning.
#[tokio::test]
async fn a_server_that_reports_no_progress_still_waits_while_the_window_is_open() {
    let fx = fixture_with(
        true,
        |d, e| vec![Ok(vec![d.clone()]), Ok(vec![d.clone(), e.clone()])],
        |m| m.with_cold_start_window(true),
    )
    .await;

    let (result, _) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(result.get("completeness_warning").is_none(), "{result}");
    assert_eq!(attempts(&fx), 2);
}

/// (m) After a mux disconnect the loop's next attempts start on the REPLACEMENT client, not on
/// the dead original, and the wait is spent on the client that answered. The original's first
/// answer is unsettled, its second call fails with a disconnect (the provider then hands out the
/// replacement), and every later call on it would fail again.
#[tokio::test]
async fn attempts_after_a_mux_disconnect_start_on_the_replacement_client() {
    let (dir, lib, other, helper) = project(true);
    let (decl, ext) = (location(&lib, 0), location(&other, 0));
    let dead = Arc::new(
        MockLspClient::new()
            .with_symbols(&lib, vec![helper.clone()])
            .with_references_script(vec![
                Ok(vec![decl.clone()]),
                Err("Mux connection lost".into()),
            ])
            .with_references_failing_after_script("Mux connection lost")
            .with_references_warmup_pending(),
    );
    let replacement = Arc::new(
        MockLspClient::new()
            .with_symbols(&lib, vec![helper])
            .with_references_script(vec![
                Ok(vec![decl.clone()]),
                Ok(vec![decl.clone(), ext.clone()]),
            ])
            .with_references_warmup_pending(),
    );
    let ctx = context_over(
        dir.path(),
        vec![Arc::clone(&dead), Arc::clone(&replacement)],
    )
    .await;

    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &ctx,
            Duration::from_secs(30),
            POLL,
        )
        .await
        .unwrap();

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert_eq!(
        dead.references_calls.load(Ordering::SeqCst),
        2,
        "a third attempt must not start on the dead client again"
    );
    assert_eq!(replacement.references_calls.load(Ordering::SeqCst), 2);
    assert!(
        replacement.references_warmup_done(),
        "the answering client spent the wait"
    );
    assert!(!dead.references_warmup_done());
}

/// (n) PAUSED time: an attempt that itself takes 30 s on a server that never settles does not
/// carry the call past the deadline: the second attempt (30 s) ends at 30.5 s, the third would
/// end at 61 s and is cut at 45 s.
#[tokio::test(start_paused = true)]
async fn a_slow_attempt_cannot_carry_the_call_past_the_deadline() {
    let s = Duration::from_secs;
    let fx = fixture_with(
        true,
        |d, _| vec![Ok(vec![d.clone()]); 10],
        |m| m.with_references_delays(vec![Duration::ZERO, s(30), s(30), s(30)]),
    )
    .await;
    let started = tokio::time::Instant::now();

    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            s(45),
            Duration::from_millis(500),
        )
        .await
        .unwrap();

    assert!(
        started.elapsed() <= Duration::from_millis(45_500),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(attempts(&fx), 3);
    assert!(warning(&result).contains("already waited"), "{result}");
}

/// (o) PAUSED time: the wait settles at 43.5 s, then a call hierarchy that would take 60 s is cut
/// at CALL_HIERARCHY_BOUND (5 s): the whole call stays within 45 + 5 s.
#[tokio::test(start_paused = true)]
async fn a_hanging_cross_check_after_a_wait_is_cut_at_its_bound() {
    let s = Duration::from_secs;
    let fx = fixture_with(
        true,
        |d, e| vec![Ok(vec![d.clone()]), Ok(vec![d.clone(), e.clone()])],
        |m| {
            m.with_references_delays(vec![Duration::ZERO, s(43)])
                .with_call_hierarchy_delay(s(60))
        },
    )
    .await;
    let started = tokio::time::Instant::now();

    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            s(45),
            Duration::from_millis(500),
        )
        .await
        .unwrap();

    let elapsed = started.elapsed();
    assert!(elapsed <= s(50), "{elapsed:?}");
    assert!(elapsed >= Duration::from_millis(43_500), "{elapsed:?}");
    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert_eq!(
        fx.client.prepare_call_hierarchy_calls.lock().unwrap().len(),
        1,
        "a settled wait still runs its (bounded) cross-check"
    );
}

/// (p) A wait that ran out skips the call-hierarchy cross-check altogether.
#[tokio::test]
async fn a_wait_that_ran_out_makes_no_call_hierarchy_calls() {
    let fx = fixture(true, |d, _| vec![vec![d.clone()]; 10_000]).await;

    let _ = call(&fx, Duration::from_millis(200)).await;

    assert_eq!(
        fx.client.prepare_call_hierarchy_calls.lock().unwrap().len(),
        0
    );
    assert_eq!(fx.client.incoming_calls_count.load(Ordering::SeqCst), 0);
}

/// (q) PAUSED time: a call that did not wait (the server answered fully at once) still bounds its
/// cross-check, by what is left of the tool timeout less the margin (55 s of 60 s).
#[tokio::test(start_paused = true)]
async fn a_hanging_cross_check_without_a_wait_is_cut_by_the_tool_budget() {
    let s = Duration::from_secs;
    let fx = fixture_with(
        true,
        |d, e| vec![Ok(vec![d.clone(), e.clone()])],
        |m| m.with_call_hierarchy_delay(s(3_600)),
    )
    .await;
    let started = tokio::time::Instant::now();

    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            s(45),
            Duration::from_millis(500),
        )
        .await
        .unwrap();

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(
        started.elapsed() <= Duration::from_millis(55_500),
        "{:?}",
        started.elapsed()
    );
}

/// (r) PAUSED time: a first attempt that alone uses the whole deadline re-asks nothing, so the
/// warning does not claim a wait that this call did not make: it is today's plain warning.
#[tokio::test(start_paused = true)]
async fn a_first_attempt_that_used_the_deadline_alone_is_not_reported_as_a_wait() {
    let s = Duration::from_secs;
    let fx = fixture_with(
        true,
        |d, _| vec![Ok(vec![d.clone()]); 10],
        |m| m.with_references_delays(vec![s(60)]),
    )
    .await;

    let result = References
        .call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            s(45),
            Duration::from_millis(500),
        )
        .await
        .unwrap();

    assert_eq!(attempts(&fx), 1, "nothing was re-asked");
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(!w.contains("already waited"), "{w}");
    assert!(fx.client.references_warmup_done());
}

/// (t) PAUSED time, through `Tool::call` with the real constants: a project configured with a
/// 20 s tool timeout cuts the wait to what fits (20 - 5 margin - 5 cross-check = 10 s), so the call
/// ends inside the timeout and the wait is spent, instead of waiting 45 s and being cut.
#[tokio::test(start_paused = true)]
async fn a_short_configured_tool_timeout_shortens_the_wait() {
    use crate::tools::Tool;
    let fx = fixture_configured(
        true,
        |d, _| vec![Ok(vec![d.clone()]); 10_000],
        |m| m,
        Some(20),
    )
    .await;
    let started = tokio::time::Instant::now();

    let result = References
        .call(json!({ "symbol": "helper", "path": "src/lib.rs" }), &fx.ctx)
        .await
        .unwrap();

    let elapsed = started.elapsed();
    assert!(elapsed <= Duration::from_secs(20), "{elapsed:?}");
    assert!(
        elapsed >= Duration::from_secs(9),
        "it did wait: {elapsed:?}"
    );
    assert!(
        warning(&result).contains("already waited 10.0 s"),
        "{result}"
    );
    assert!(fx.client.references_warmup_done());
}

/// (u) PAUSED time: a configured timeout that leaves no room for a wait (10 s: 10 - 5 - 5 = 0)
/// returns the first answer at once with today's plain warning, sleeps nothing, and spends the wait.
#[tokio::test(start_paused = true)]
async fn a_tool_timeout_with_no_room_for_a_wait_returns_the_first_answer() {
    use crate::tools::Tool;
    let fx = fixture_configured(
        true,
        |d, _| vec![Ok(vec![d.clone()]); 10_000],
        |m| m,
        Some(10),
    )
    .await;
    let started = tokio::time::Instant::now();

    let result = References
        .call(json!({ "symbol": "helper", "path": "src/lib.rs" }), &fx.ctx)
        .await
        .unwrap();

    assert!(
        started.elapsed() <= Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(attempts(&fx), 1);
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(!w.contains("already waited"), "{w}");
    assert!(fx.client.references_warmup_done());
}

/// (v) An empty answer after a non-empty one is a server mid-reload: the declaration already in
/// hand is kept and the warning stays the definition-file one, not "no locations at all".
#[tokio::test]
async fn a_later_empty_answer_does_not_replace_the_partial_one() {
    let fx = fixture(true, |d, _| {
        let mut script = vec![vec![d.clone()]];
        script.extend(vec![vec![]; 10_000]);
        script
    })
    .await;

    let (result, _) = call(&fx, Duration::from_millis(300)).await;

    assert_eq!(result["total"].as_u64(), Some(1), "{result}");
    let w = warning(&result);
    assert!(w.contains("outside the definition file"), "{result}");
    assert!(!w.contains("no locations at all"), "{w}");
    assert!(attempts(&fx) >= 2, "it did ask again");
}

/// (w) A location inside a build directory is not a use: an answer of the declaration plus only
/// `target/gen.rs` still counts as "nothing outside the definition file" and is re-asked.
#[tokio::test]
async fn a_build_directory_location_is_not_an_outside_use() {
    let fx = fixture_with(
        true,
        |d, e| {
            let in_target = location(
                &crate::fs::uri_to_path(d.uri.as_str())
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("target")
                    .join("gen.rs"),
                0,
            );
            vec![
                Ok(vec![d.clone(), in_target]),
                Ok(vec![d.clone(), e.clone()]),
            ]
        },
        |m| m,
    )
    .await;

    let (result, _) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(attempts(&fx), 2, "the first answer was provisional");
    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(result.get("completeness_warning").is_none(), "{result}");
}

/// (x) PAUSED time: a call dropped mid-wait (a server timeout, a client cancel) still spends the
/// client's wait, so the next call does not wait and get cut again.
#[tokio::test(start_paused = true)]
async fn a_call_dropped_mid_wait_still_spends_the_clients_wait() {
    let fx = fixture(true, |d, _| vec![vec![d.clone()]; 10_000]).await;

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        References.call_with_settle(
            json!({ "symbol": "helper", "path": "src/lib.rs" }),
            &fx.ctx,
            Duration::from_secs(45),
            Duration::from_millis(500),
        ),
    )
    .await;

    assert!(outcome.is_err(), "the call was cut mid-wait");
    assert!(fx.client.references_warmup_done());
}

/// (i) A later attempt that fails the way a warming server fails (`-32800`) is retried within the
/// deadline rather than ending the wait with the partial answer.
#[tokio::test]
async fn a_transient_error_on_a_later_attempt_is_retried_within_the_deadline() {
    let fx = fixture_scripted(true, |d, e| {
        vec![
            Ok(vec![d.clone()]),
            Err("request failed with code -32800 (cancelled)".to_string()),
            Err("request failed with code -32800 (cancelled)".to_string()),
            Ok(vec![d.clone(), e.clone()]),
        ]
    })
    .await;

    let (result, _) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(result["total"].as_u64(), Some(2), "{result}");
    assert!(result.get("completeness_warning").is_none(), "{result}");
    assert_eq!(attempts(&fx), 4);
}

/// (j) A deterministic failure on a later attempt keeps the partial answer the call already
/// holds, instead of turning a result into an error or retrying for the whole deadline.
#[tokio::test]
async fn a_deterministic_error_on_a_later_attempt_keeps_the_partial_answer() {
    let fx = fixture_scripted(true, |d, _| {
        vec![Ok(vec![d.clone()]), Err("symbol exploded".to_string())]
    })
    .await;

    let (result, elapsed) = call(&fx, Duration::from_secs(30)).await;

    assert_eq!(result["total"].as_u64(), Some(1), "{result}");
    assert!(
        warning(&result).contains("outside the definition file"),
        "{result}"
    );
    assert_eq!(attempts(&fx), 2);
    assert!(elapsed < SLACK, "{elapsed:?}");
    assert!(fx.client.references_warmup_done());
}
