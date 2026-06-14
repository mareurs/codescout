//! Progress notification helper for long-running tools.

use std::sync::Arc;
use std::time::Duration;

use rmcp::{
    model::{
        LoggingLevel, LoggingMessageNotificationParam, NumberOrString, ProgressNotificationParam,
        ProgressToken,
    },
    service::Peer,
    RoleServer,
};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Minimum gap between consecutive numeric progress emissions (2 Hz).
const MIN_GAP: Duration = Duration::from_millis(500);

/// Abstraction over the MCP peer so progress can be tested without a live server.
#[async_trait::async_trait]
pub trait ProgressSink: Send + Sync {
    async fn emit_progress(&self, step: f64, total: Option<f64>, token: &NumberOrString);
    async fn emit_text(&self, text: &str);
}

/// Default sink — forwards notifications to an rmcp `Peer<RoleServer>`.
pub struct PeerSink {
    pub peer: Peer<RoleServer>,
}

#[async_trait::async_trait]
impl ProgressSink for PeerSink {
    async fn emit_progress(&self, step: f64, total: Option<f64>, token: &NumberOrString) {
        let mut params = ProgressNotificationParam::new(ProgressToken(token.clone()), step);
        if let Some(t) = total {
            params = params.with_total(t);
        }
        let _ = self.peer.notify_progress(params).await;
    }

    async fn emit_text(&self, text: &str) {
        let _ = self
            .peer
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                logger: Some("codescout".to_string()),
                data: serde_json::Value::String(text.to_string()),
            })
            .await;
    }
}

/// Sends MCP `notifications/progress` to the client while a tool is running.
///
/// Constructed in `server.rs::call_tool` from the request context. Tools
/// call `ctx.progress.as_ref()` — it is a no-op when `None`.
///
/// # Progress token
/// `CallToolRequestParams._meta.progressToken` is the canonical (and only)
/// source. `server.rs::call_tool` constructs a reporter *only* when the client
/// sent one — otherwise `ctx.progress` is `None` and every `report()` is a
/// no-op. We deliberately do NOT synthesize a token from the request id:
/// emitting progress the client never requested is an unsolicited notification
/// that crashes Claude Code 2.x (BUG-038).
///
/// # Throttling
/// `report()` is throttled to at most one emission per 500 ms (2 Hz).
/// Calls within the window are silently dropped — the next permitted call
/// carries whatever step/total values the caller reports at that moment.
/// `report_text()` is unthrottled; text messages are user-facing state
/// transitions and are expected to be infrequent.
pub struct ProgressReporter {
    sink: Arc<dyn ProgressSink>,
    token: NumberOrString,
    last_emit: Mutex<Option<Instant>>,
}

impl ProgressReporter {
    /// Production constructor — preserves the existing public signature.
    pub fn new(peer: Peer<RoleServer>, token: NumberOrString) -> Arc<Self> {
        Arc::new(Self {
            sink: Arc::new(PeerSink { peer }),
            token,
            last_emit: Mutex::new(None),
        })
    }

    /// Test constructor — accepts any `ProgressSink` implementation.
    #[cfg(test)]
    pub fn with_sink(sink: Arc<dyn ProgressSink>, token: NumberOrString) -> Arc<Self> {
        Arc::new(Self {
            sink,
            token,
            last_emit: Mutex::new(None),
        })
    }

    /// Send a throttled progress notification (at most one per 500 ms).
    ///
    /// Errors are silently swallowed — progress is best-effort and must never
    /// fail the tool call.
    pub async fn report(&self, step: u32, total: Option<u32>) {
        let now = Instant::now();
        {
            let mut g = self.last_emit.lock().await;
            match *g {
                Some(t) if now.duration_since(t) < MIN_GAP => return,
                _ => {
                    *g = Some(now);
                }
            }
        }
        self.sink
            .emit_progress(step as f64, total.map(|t| t as f64), &self.token)
            .await;
    }

    /// Send a free-form text message via `notifications/message` (MCP logging
    /// channel). This is out-of-band from `CallToolResult`, so the LLM never
    /// sees it — only the MCP client (Claude Code terminal) does.
    ///
    /// Used to deliver user-facing output (ANSI previews, status lines) without
    /// polluting the LLM context. Errors are silently swallowed. Unthrottled.
    pub async fn report_text(&self, text: &str) {
        self.sink.emit_text(text).await;
    }
}

/// Test helpers shared across tool test modules.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A `ProgressSink` that counts emitted progress and text notifications.
    /// Shared by tests in `progress`, `semantic`, and `workflow`.
    #[derive(Default)]
    pub struct CountingSink {
        pub progress_calls: AtomicU32,
        pub text_calls: AtomicU32,
    }

    #[async_trait::async_trait]
    impl crate::tools::progress::ProgressSink for CountingSink {
        async fn emit_progress(
            &self,
            _step: f64,
            _total: Option<f64>,
            _token: &rmcp::model::NumberOrString,
        ) {
            self.progress_calls.fetch_add(1, Ordering::Relaxed);
        }

        async fn emit_text(&self, _text: &str) {
            self.text_calls.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::CountingSink;
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn report_throttles_to_one_per_window() {
        let sink = Arc::new(CountingSink::default());
        let r = ProgressReporter::with_sink(sink.clone(), NumberOrString::Number(1));

        // 100 rapid calls advancing 9 ms each → total 891 ms elapsed.
        // First call at t=0 is allowed. Next window opens at t=500 ms.
        // One more call passes at some point after 500 ms → expect 2 total.
        for i in 0..100 {
            r.report(i, Some(100)).await;
            tokio::time::advance(Duration::from_millis(9)).await;
        }
        let n = sink.progress_calls.load(Ordering::SeqCst);
        assert!((1..=2).contains(&n), "expected 1–2 emissions, got {}", n);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn report_allows_after_window_elapsed() {
        let sink = Arc::new(CountingSink::default());
        let r = ProgressReporter::with_sink(sink.clone(), NumberOrString::Number(2));

        r.report(1, None).await; // first always allowed
        tokio::time::advance(Duration::from_millis(600)).await; // > 500 ms
        r.report(2, None).await; // second window — must pass
        assert_eq!(sink.progress_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn report_text_is_not_throttled() {
        let sink = Arc::new(CountingSink::default());
        let r = ProgressReporter::with_sink(sink.clone(), NumberOrString::Number(3));

        for _ in 0..10 {
            r.report_text("hi").await;
        }
        assert_eq!(sink.text_calls.load(Ordering::SeqCst), 10);
    }
}
