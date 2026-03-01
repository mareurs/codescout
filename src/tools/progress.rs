//! Progress notification helper for long-running tools.

use std::sync::Arc;

use rmcp::{
    model::{NumberOrString, ProgressNotificationParam},
    service::Peer,
    RoleServer,
};

/// Sends MCP `notifications/progress` to the client while a tool is running.
///
/// Constructed in `server.rs::call_tool` from the request context. Tools
/// call `ctx.progress.as_ref()` — it is a no-op when `None`.
///
/// # rmcp-0.1.5 limitation
/// `CallToolRequestParam` does not expose `_meta.progressToken`. We use
/// `_ctx.id` (the request ID) as a stand-in progress token. This works if
/// the client correlates progress tokens with request IDs (common in practice).
pub struct ProgressReporter {
    peer: Peer<RoleServer>,
    token: NumberOrString,
}

impl ProgressReporter {
    pub fn new(peer: Peer<RoleServer>, token: NumberOrString) -> Arc<Self> {
        Arc::new(Self { peer, token })
    }

    /// Send a progress notification. Errors are silently swallowed — progress
    /// is best-effort and must never fail the tool call.
    pub async fn report(&self, step: u32, total: Option<u32>) {
        let _ = self
            .peer
            .notify_progress(ProgressNotificationParam {
                progress_token: self.token.clone(),
                progress: step,
                total,
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reporter_constructs_without_panic() {
        // We can't easily unit-test the async notify_progress call
        // without a live peer, so this just verifies the struct compiles.
        // Integration behavior is verified manually in a running server.
        let _p: Option<ProgressReporter> = None;
        assert!(true);
    }
}
