//! Native MCP identity for this invocation's spawned shells.
//!
//! The observed Codex client sends a shared root `sessionId` and an executing
//! `threadId` in request metadata. These are distinct from guide-ledger keys.
//! Scope the owned snapshot to the request future; never mutate process env.

use super::{ClientIdentity, RecoverableError};
use std::future::Future;

#[derive(Debug)]
pub(crate) enum NativeInvocation {
    Absent,
    Codex {
        session_id: String,
        thread_id: Option<String>,
        version: Option<String>,
    },
    Invalid(&'static str),
}

impl NativeInvocation {
    pub(crate) fn from_meta(
        meta: Option<&serde_json::Map<String, serde_json::Value>>,
        client: Option<&ClientIdentity>,
    ) -> Self {
        let Some(client) = client.filter(|c| c.name == "codex-mcp-client") else {
            return Self::Absent;
        };
        let Some(meta) = meta else {
            return Self::Absent;
        };
        let Some(root) = meta.get("sessionId") else {
            // A thread alone does not establish the native logical root.
            return Self::Absent;
        };
        let text = |value: &serde_json::Value| {
            value
                .as_str()
                .filter(|s| !s.trim().is_empty() && !s.contains('\0'))
                .map(str::to_owned)
        };
        let Some(session_id) = text(root) else {
            return Self::Invalid("sessionId");
        };
        let thread_id = match meta.get("threadId") {
            None => None,
            Some(value) => match text(value) {
                Some(id) => Some(id),
                None => return Self::Invalid("threadId"),
            },
        };
        Self::Codex {
            session_id,
            thread_id,
            version: client.version.clone(),
        }
    }

    fn apply(&self, command: &mut tokio::process::Command) -> anyhow::Result<()> {
        match self {
            Self::Absent => {}
            Self::Invalid(field) => {
                return Err(RecoverableError::with_hint(
                    format!("invalid native Codex {field} in MCP request metadata"),
                    "Use a client that supplies a nonempty native root sessionId and valid threadId.",
                ).into());
            }
            Self::Codex {
                session_id,
                thread_id,
                version,
            } => {
                command.env("CODEX_SESSION_ID", session_id);
                // Missing per-call child/version detail must not borrow a stale
                // startup value from the long-lived server process.
                command
                    .env_remove("CODEX_THREAD_ID")
                    .env_remove("CODEX_VERSION");
                if let Some(thread_id) = thread_id {
                    command.env("CODEX_THREAD_ID", thread_id);
                }
                if let Some(version) = version {
                    command.env("CODEX_VERSION", version);
                }
            }
        }
        Ok(())
    }
}

tokio::task_local! {
    static INVOCATION: NativeInvocation;
}

pub(crate) async fn scope<F: Future>(identity: NativeInvocation, future: F) -> F::Output {
    INVOCATION.scope(identity, future).await
}

/// Apply only to this command. Background commands capture their environment
/// before their supervisor task is spawned; the request scope is never shared.
pub(crate) fn apply_to_shell(command: &mut tokio::process::Command) -> anyhow::Result<()> {
    INVOCATION
        .try_with(|identity| identity.apply(command))
        .unwrap_or(Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_context_clears_stale_optional_command_environment() {
        use std::ffi::OsStr;
        let mut command = tokio::process::Command::new("unused");
        command
            .env("CODEX_SESSION_ID", "stale-root")
            .env("CODEX_THREAD_ID", "stale-child")
            .env("CODEX_VERSION", "stale-version");
        scope(
            NativeInvocation::Codex {
                session_id: "actual-root".to_owned(),
                thread_id: None,
                version: None,
            },
            async {
                apply_to_shell(&mut command).unwrap();
            },
        )
        .await;
        let environment: std::collections::BTreeMap<_, _> = command.as_std().get_envs().collect();
        assert_eq!(
            environment[OsStr::new("CODEX_SESSION_ID")],
            Some(OsStr::new("actual-root"))
        );
        assert_eq!(environment[OsStr::new("CODEX_THREAD_ID")], None);
        assert_eq!(environment[OsStr::new("CODEX_VERSION")], None);
    }
}
