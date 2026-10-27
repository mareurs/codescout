//! LSP client layer: manages language server processes and exposes a
//! unified async interface for symbol operations.

pub mod client;
pub mod client_ops;
pub(crate) mod init_options;
pub mod manager;
pub mod manager_provider;
pub mod mock;
pub(crate) mod progress;
pub mod servers;
pub mod symbols;
pub use mock::{MockLspClient, MockLspProvider};
pub mod call_hierarchy;
pub mod ops;

pub use ops::{LspClientOps, LspProvider};
pub mod mux;
pub mod transport;

pub use client::{LspClient, LspServerConfig};
pub use manager::LspManager;
pub use symbols::{SymbolInfo, SymbolKind};

/// Languages whose LSP servers are pre-warmed on project activation.
/// Hardcoded to JVM languages — Kotlin in particular takes 30–60 s to start.
const PREWARM_LANGUAGES: &[&str] = &["java", "kotlin"];

/// Consecutive failed starts, since a language's last success, after which the pre-warm stops
/// trying. Two can be bad luck (a lock held by a peer); three in a row is a pattern.
// cap-class: NOT_A_CAP — a back-off threshold; skipping the pre-warm drops no answer, because a call that needs the server still starts it
const PREWARM_BACKOFF_AFTER_FAILURES: usize = 3;

/// How long the pre-warm stays off after the newest failed start. Bounds the cost of a
/// deterministic failure to one attempt per window, while a fixed server is still noticed.
// cap-class: NOT_A_CAP — a back-off window; it delays an optional warm-up and never shortens a result
const PREWARM_BACKOFF_WINDOW_SECS: u64 = 6 * 60 * 60;

/// Whether `history` says this language fails every time it starts, recently enough to believe it.
fn prewarm_backs_off(history: &crate::usage::db::LspStartHistory) -> bool {
    history.consecutive_failures >= PREWARM_BACKOFF_AFTER_FAILURES
        && history
            .last_failure_age_secs
            .is_some_and(|age| age < PREWARM_BACKOFF_WINDOW_SECS)
}

/// The failed-start history of `language` in `root`'s `usage.db`, or `None` when it cannot be
/// read. `None` means "pre-warm as before": the history only ever removes a start, so an
/// unreadable one must not.
async fn start_history(
    root: &std::path::Path,
    language: &str,
) -> Option<crate::usage::db::LspStartHistory> {
    let root = root.to_path_buf();
    let language = language.to_string();
    tokio::task::spawn_blocking(move || {
        let conn = crate::usage::db::open_db(&root).ok()?;
        crate::usage::db::lsp_start_history(&conn, &language).ok()
    })
    .await
    .ok()
    .flatten()
}

/// Spawn background `get_or_start` tasks for any JVM languages found in
/// `project_languages`. Safe to call concurrently: `LspManager`'s starting-map
/// serialises parallel starters via a watch channel — no double-start risk.
///
/// A language whose last starts all failed is skipped (see [`prewarm_backs_off`]). The circuit
/// breaker cannot do this: it lives in one process and each activation is a new one, so a start
/// that fails every time was retried on every activation (235 starts of `jdtls`, ~16 s each).
/// Only the optional warm-up is skipped; a tool call that needs the server still starts it.
///
/// Returns the spawned tasks. Production callers drop them; a test awaits them so it can
/// assert on what the pre-warm did without sleeping.
pub fn prewarm_lsp_background(
    lsp: std::sync::Arc<dyn LspProvider>,
    root: std::path::PathBuf,
    project_languages: &[String],
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut tasks = Vec::new();
    for lang in project_languages {
        if PREWARM_LANGUAGES.contains(&lang.as_str()) {
            let lsp = lsp.clone();
            let root = root.clone();
            let lang = lang.clone();
            tasks.push(tokio::spawn(async move {
                if let Some(history) = start_history(&root, &lang).await {
                    if prewarm_backs_off(&history) {
                        tracing::info!(
                            "LSP pre-warm for {lang} skipped: its last {} starts failed, the newest {}s ago",
                            history.consecutive_failures,
                            history.last_failure_age_secs.unwrap_or(0)
                        );
                        return;
                    }
                }
                if let Err(e) = lsp.get_or_start(&lang, &root, None).await {
                    tracing::debug!("LSP pre-warm for {lang} skipped: {e}");
                }
            }));
        }
    }
    tasks
}

/// Budget for a tool call that would otherwise block on an LSP cold start.
/// Callers fall back to tree-sitter output (marked `"lsp": "warming"`) when
/// the budget elapses; the start continues in a DETACHED task so the next
/// call hits the warm fast path.
// cap-class: RESULT_CAP lsp.first_call_budget — probed
pub const LSP_FIRST_CALL_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);

/// Bounded LSP acquisition: immediate when a live client exists; otherwise
/// start it on a detached task and wait at most `budget`. `None` means "not
/// ready yet — serve the AST fallback"; it is never an error.
pub async fn client_within_budget(
    lsp: std::sync::Arc<dyn LspProvider>,
    language: &str,
    root: &std::path::Path,
    mux_override: Option<bool>,
    budget: std::time::Duration,
) -> Option<std::sync::Arc<dyn LspClientOps>> {
    if lsp.is_ready(language, root).await {
        return lsp.get_or_start(language, root, mux_override).await.ok();
    }
    let lang = language.to_string();
    let root_buf = root.to_path_buf();
    let handle =
        tokio::spawn(async move { lsp.get_or_start(&lang, &root_buf, mux_override).await });
    match tokio::time::timeout(budget, handle).await {
        Ok(Ok(Ok(client))) => Some(client),
        _ => None, // start error, join error, or budget elapsed (task keeps warming)
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// LspProvider whose cold start takes `delay`; `is_ready` flips true once
    /// a start has completed (mirrors LspManager's cache-hit fast path).
    struct SlowStart {
        client: Arc<MockLspClient>,
        delay: Duration,
        ready: AtomicBool,
    }

    #[async_trait::async_trait]
    impl LspProvider for SlowStart {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            if !self.ready.load(Ordering::SeqCst) {
                tokio::time::sleep(self.delay).await;
                self.ready.store(true, Ordering::SeqCst);
            }
            Ok(self.client.clone())
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
        async fn is_ready(&self, _language: &str, _workspace_root: &Path) -> bool {
            self.ready.load(Ordering::SeqCst)
        }
    }

    // Virtual time, not the wall clock: every wait here is a tokio timer — SlowStart's
    // `delay`, `client_within_budget`'s `timeout`, and the warm-up wait below — so the
    // whole schedule is deterministic under `start_paused`. Auto-advance jumps to the NEXT
    // deadline, which is the 50 ms budget and not the 200 ms cold start, so `None` is still
    // returned for the right reason. On the real clock this test allowed 100 ms of
    // scheduler slack and flaked under wine on a shared runner (WIN-30).
    #[tokio::test(start_paused = true)]
    async fn cold_start_over_budget_returns_none_but_keeps_warming() {
        let lsp: Arc<dyn LspProvider> = Arc::new(SlowStart {
            client: Arc::new(MockLspClient::new()),
            delay: Duration::from_millis(200),
            ready: AtomicBool::new(false),
        });
        // Budget (50ms) < cold start (200ms): must yield None, not block 200ms.
        // `tokio::time::Instant` IS virtualised, unlike `std::time::Instant`, so the
        // ceiling below states something exact about the schedule rather than tolerating
        // jitter: it fails if the call ever waits the cold start out.
        let t0 = tokio::time::Instant::now();
        let got = client_within_budget(
            lsp.clone(),
            "rust",
            Path::new("/tmp"),
            None,
            Duration::from_millis(50),
        )
        .await;
        assert!(got.is_none());
        assert!(
            t0.elapsed() < Duration::from_millis(150),
            "must not wait out the cold start"
        );

        // The DETACHED warm-up must finish on its own: after the delay elapses,
        // the provider is ready and the next call succeeds immediately.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let got = client_within_budget(
            lsp,
            "rust",
            Path::new("/tmp"),
            None,
            Duration::from_millis(50),
        )
        .await;
        assert!(
            got.is_some(),
            "second call after warm-up must hit the ready fast path"
        );
    }
}

/// The pre-warm runs once per activation in a fresh process, so the in-memory circuit breaker
/// (5 failures in 60 s) never reaches its ceiling for a start that fails every time. These pin
/// that the pre-warm consults the failed-start history in `usage.db` instead.
#[cfg(test)]
mod prewarm_tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Counts `get_or_start` calls and always fails, like `jdtls` under a JDK it cannot read.
    struct FailingStart {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl LspProvider for FailingStart {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn LspClientOps>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("LSP server disconnected"))
        }
        async fn notify_file_changed(&self, _path: &Path) {}
        async fn shutdown_all(&self) {}
        async fn is_ready(&self, _language: &str, _workspace_root: &Path) -> bool {
            false
        }
    }

    /// Seed `n` failed starts of `language` into a fresh project's `usage.db`, run the pre-warm
    /// for `language`, and return how many starts it attempted.
    async fn starts_after(language: &str, seed: impl FnOnce(&rusqlite::Connection)) -> usize {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        {
            let conn = crate::usage::db::open_db(&root).unwrap();
            seed(&conn);
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let lsp: Arc<dyn LspProvider> = Arc::new(FailingStart {
            calls: calls.clone(),
        });
        for task in prewarm_lsp_background(lsp, root, &[language.to_string()]) {
            task.await.unwrap();
        }
        calls.load(Ordering::SeqCst)
    }

    fn fail(conn: &rusqlite::Connection, language: &str, n: usize) {
        for _ in 0..n {
            crate::usage::db::write_lsp_failure(
                conn,
                language,
                "new_session",
                16_000,
                "LSP server disconnected",
            )
            .unwrap();
        }
    }

    #[tokio::test]
    async fn a_language_whose_recent_starts_all_failed_is_not_pre_warmed() {
        assert_eq!(starts_after("java", |c| fail(c, "java", 3)).await, 0);
        // Kotlin is pre-warmed too, and gets the same treatment.
        assert_eq!(starts_after("kotlin", |c| fail(c, "kotlin", 5)).await, 0);
    }

    #[tokio::test]
    async fn the_pre_warm_still_runs_when_the_history_does_not_justify_skipping() {
        // No history: the first activation must still try.
        assert_eq!(starts_after("java", |_| {}).await, 1);
        // Below the threshold: two failures can be bad luck.
        assert_eq!(starts_after("java", |c| fail(c, "java", 2)).await, 1);
        // Another language's failures say nothing about this one.
        assert_eq!(starts_after("java", |c| fail(c, "kotlin", 9)).await, 1);
        // A success ends the run of failures.
        assert_eq!(
            starts_after("java", |c| {
                fail(c, "java", 4);
                crate::usage::db::write_lsp_event(c, "java", "new_session", 2_000).unwrap();
            })
            .await,
            1
        );
        // Old failures: after the window the pre-warm tries again, so a fixed JDK is noticed.
        assert_eq!(
            starts_after("java", |c| {
                fail(c, "java", 4);
                c.execute(
                    "UPDATE lsp_events SET started_at = datetime('now', '-7 hours')",
                    [],
                )
                .unwrap();
            })
            .await,
            1
        );
    }
}
