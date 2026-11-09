//! Native MCP identity for this invocation's spawned shells.
//!
//! The observed Codex client sends a shared root `sessionId` and an executing
//! `threadId` in request metadata. These are distinct from guide-ledger keys.
//! Scope the owned snapshot to the request future; never mutate process env.

use super::{ClientIdentity, RecoverableError};
use std::future::Future;

#[derive(Debug, Clone, Default)]
pub(crate) struct CodexFields {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub call_id: Option<String>,
    pub turn_id: Option<String>,
    pub version: Option<String>,
}

/// One native validation policy for command context and stock observations.
/// A nested turn root never substitutes for a missing top-level sessionId.
pub(crate) fn codex_fields(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
    client: Option<&ClientIdentity>,
) -> Result<Option<CodexFields>, &'static str> {
    let Some(client) = client.filter(|c| c.name == "codex-mcp-client") else {
        return Ok(None);
    };
    let empty = serde_json::Map::new();
    let meta = meta.unwrap_or(&empty);
    fn id(
        map: &serde_json::Map<String, serde_json::Value>,
        key: &'static str,
    ) -> Result<Option<String>, &'static str> {
        match map.get(key) {
            None => Ok(None),
            Some(serde_json::Value::String(s))
                if !s.trim().is_empty() && s.len() <= 1024 && !s.chars().any(|c| c < ' ') =>
            {
                Ok(Some(s.clone()))
            }
            _ => Err(key),
        }
    }
    let turn = match meta.get("x-codex-turn-metadata") {
        None => &empty,
        Some(serde_json::Value::Object(map)) => map,
        _ => return Err("x-codex-turn-metadata"),
    };
    let session_id = id(meta, "sessionId")?;
    let thread_id = id(meta, "threadId")?;
    let call_id = id(meta, "callId")?;
    let turn_id = id(meta, "turnId")?;
    let nested_root = id(turn, "session_id")?;
    let nested_thread = id(turn, "thread_id")?;
    let nested_call = id(turn, "call_id")?;
    let nested_turn = id(turn, "turn_id")?;
    for (a, b) in [
        (&session_id, &nested_root),
        (&thread_id, &nested_thread),
        (&call_id, &nested_call),
        (&turn_id, &nested_turn),
    ] {
        if a.as_ref().zip(b.as_ref()).is_some_and(|(a, b)| a != b) {
            return Err("context-conflict");
        }
    }
    let version = client.version.clone();
    if version
        .as_ref()
        .is_some_and(|v| v.trim().is_empty() || v.len() > 1024 || v.chars().any(|c| c < ' '))
    {
        return Err("version");
    }
    Ok(Some(CodexFields {
        session_id,
        thread_id: thread_id.or(nested_thread),
        call_id: call_id.or(nested_call),
        turn_id: turn_id.or(nested_turn),
        version,
    }))
}

#[derive(Debug)]
pub(crate) enum NativeInvocation {
    Absent,
    Codex {
        session_id: String,
        thread_id: Option<String>,
        call_id: Option<String>,
        turn_id: Option<String>,
        version: Option<String>,
    },
    Invalid(&'static str),
}

impl NativeInvocation {
    pub(crate) fn from_meta(
        meta: Option<&serde_json::Map<String, serde_json::Value>>,
        client: Option<&ClientIdentity>,
    ) -> Self {
        match codex_fields(meta, client) {
            Ok(Some(fields)) => match fields.session_id {
                Some(session_id) => Self::Codex {
                    session_id,
                    thread_id: fields.thread_id,
                    call_id: fields.call_id,
                    turn_id: fields.turn_id,
                    version: fields.version,
                },
                None => Self::Absent,
            },
            Ok(None) => Self::Absent,
            Err(field) => Self::Invalid(field),
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
                ..
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
        .unwrap_or(Ok(()))?;
    super::provenance_fence_context::apply_to_shell(command)
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
                call_id: None,
                turn_id: None,
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

    #[test]
    fn native_context_rejects_duplicated_identity_conflicts_and_bounds() {
        let client = ClientIdentity {
            name: "codex-mcp-client".into(),
            version: Some("0.167.0".into()),
        };
        for meta in [
            serde_json::json!({"sessionId":"root","x-codex-turn-metadata":{"session_id":"different"}}),
            serde_json::json!({"sessionId":"root","threadId":"child-a","x-codex-turn-metadata":{"thread_id":"child-b"}}),
            serde_json::json!({"sessionId":"root","callId":7}),
            serde_json::json!({"sessionId":"root","turnId":"a","x-codex-turn-metadata":{"turn_id":"b"}}),
            serde_json::json!({"sessionId":"x".repeat(1025)}),
        ] {
            assert!(
                matches!(
                    NativeInvocation::from_meta(meta.as_object(), Some(&client)),
                    NativeInvocation::Invalid(_)
                ),
                "{meta}"
            );
        }
    }
}
