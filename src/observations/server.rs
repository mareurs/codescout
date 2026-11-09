//! Stock caller observations. Never consulted to discharge exclusive coverage.
use super::contract::{bytes_fingerprint, fingerprint, Caller, Fingerprint, TargetFact, TOOLS};
use super::store::{Limits, ObservationStore, SanitizedEvent, StoreAvailability};
use crate::tools::{
    invocation_context::{codex_fields, NativeInvocation},
    ClientIdentity,
};
use parking_lot::Mutex;
use rmcp::model::CallToolResult;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

#[cfg(test)]
use std::sync::atomic::AtomicU64;

#[derive(Debug, Clone)]
pub(crate) struct LocalInvocationId {
    server: String,
    connection: String,
    receive: u64,
}
impl LocalInvocationId {
    pub(crate) fn new(connection: &str, receive: u64) -> Self {
        static SERVER: OnceLock<String> = OnceLock::new();
        Self {
            server: SERVER
                .get_or_init(|| uuid::Uuid::new_v4().to_string())
                .clone(),
            connection: connection.into(),
            receive,
        }
    }
    #[cfg(test)]
    pub(crate) fn fresh(connection: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        Self::new(connection, SEQUENCE.fetch_add(1, Ordering::Relaxed))
    }
}
impl std::fmt::Display for LocalInvocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.server, self.connection, self.receive)
    }
}
#[derive(Clone)]
pub(crate) struct ObservationContext {
    root: Option<PathBuf>,
    caller: Caller,
    call_id: Option<String>,
    turn_id: Option<String>,
    native_cwd: Option<String>,
    diagnostics: Vec<String>,
}
impl ObservationContext {
    pub(crate) fn from_meta(
        root: Option<PathBuf>,
        meta: Option<&serde_json::Map<String, Value>>,
        client: Option<&ClientIdentity>,
    ) -> Self {
        let mut context = Self {
            root,
            caller: Caller {
                harness: "unknown".into(),
                principal: None,
                thread_id: None,
                agent_id: None,
            },
            call_id: None,
            turn_id: None,
            native_cwd: None,
            diagnostics: Vec::new(),
        };
        if client.is_some_and(|c| c.name == "codex-mcp-client") {
            context.caller.harness = "codex".into();
        }
        // The existing resolver owns root validation. The shared fields parser
        // retains a child/call for the rootless API case without inventing a root.
        match NativeInvocation::from_meta(meta, client) {
            NativeInvocation::Codex {
                session_id,
                thread_id,
                call_id,
                turn_id,
                ..
            } => {
                context.caller.principal = Some(format!("codex:{session_id}"));
                context.caller.thread_id = thread_id;
                context.call_id = call_id;
                context.turn_id = turn_id;
            }
            NativeInvocation::Invalid(field) => {
                context.diagnostics.push(
                    if field == "context-conflict" {
                        "context-conflict"
                    } else {
                        "invalid-context"
                    }
                    .into(),
                );
            }
            NativeInvocation::Absent => {
                if let Ok(Some(fields)) = codex_fields(meta, client) {
                    context.caller.thread_id = fields.thread_id;
                    context.call_id = fields.call_id;
                    context.turn_id = fields.turn_id;
                }
                context.diagnostics.push("missing-root".into());
            }
        }
        if context.caller.harness == "codex"
            && !context
                .diagnostics
                .iter()
                .any(|d| d == "invalid-context" || d == "context-conflict")
        {
            if let Some(value) = meta
                .and_then(|m| m.get("x-codex-turn-metadata"))
                .and_then(Value::as_object)
                .and_then(|m| m.get("cwd"))
            {
                match value.as_str() {
                    Some(cwd) if super::contract::path(cwd).is_ok() => {
                        context.native_cwd = Some(cwd.to_owned())
                    }
                    _ if value.is_null() => {}
                    _ => context.diagnostics.push("incomplete-input".into()),
                }
            }
        }
        context
    }
}
fn stores() -> &'static Mutex<HashMap<PathBuf, Arc<ObservationStore>>> {
    static STORES: OnceLock<Mutex<HashMap<PathBuf, Arc<ObservationStore>>>> = OnceLock::new();
    STORES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn store(root: &Path) -> Option<Arc<ObservationStore>> {
    let root = super::store::checkout(root).ok()?;
    let mut stores = stores().lock();
    if let Some(s) = stores.get(&root) {
        return Some(s.clone());
    }
    let StoreAvailability::Enabled(s) = ObservationStore::open_if_enabled(&root, Limits::default())
    else {
        return None;
    };
    // Bound retained descriptors; reopening mints a new producer, never sequence reuse.
    if stores.len() >= 64 {
        stores.clear();
    }
    stores.insert(root, s.clone());
    Some(s)
}
#[derive(Clone, Copy)]
pub(crate) enum WritePhase {
    Attempted,
    Committed,
    Uncertain,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ParentIdentity {
    device: u64,
    inode: u64,
}
fn parent_identity(path: &Path) -> Option<ParentIdentity> {
    use std::os::unix::fs::MetadataExt;
    let m = path.parent()?.metadata().ok()?;
    Some(ParentIdentity {
        device: m.dev(),
        inode: m.ino(),
    })
}
struct Request {
    context: ObservationContext,
    invocation: LocalInvocationId,
    tool: Option<String>,
    fingerprints: Vec<Fingerprint>,
    diagnostics: Vec<String>,
    terminal: AtomicBool,
    attempted_effect: AtomicBool,
    raw_outcome: Mutex<Option<&'static str>>,
    pending: Mutex<HashMap<PathBuf, ParentIdentity>>,
}
impl Request {
    fn emit(
        &self,
        root: &Path,
        event: &str,
        targets: Vec<TargetFact>,
        outcome: &str,
        extra: &[&str],
    ) {
        let Ok(root) = super::store::checkout(root) else {
            return;
        };
        let Some(store) = store(&root) else {
            return;
        };
        let mut diagnostics = self.diagnostics.clone();
        for code in extra {
            if !diagnostics.iter().any(|d| d == code) {
                diagnostics.push((*code).into());
            }
        }
        let event = SanitizedEvent {
            schema_version: 1,
            event: event.into(),
            producer_kind: "codescout-server".into(),
            workspace: root.to_string_lossy().into_owned(),
            native_cwd: self.context.native_cwd.clone(),
            caller: self.context.caller.clone(),
            tool: self.tool.clone(),
            local_invocation: Some(self.invocation.to_string()),
            native_call_id: self.context.call_id.clone(),
            turn_id: self.context.turn_id.clone(),
            fingerprints: self.fingerprints.clone(),
            targets,
            outcome: outcome.into(),
            diagnostics,
        };
        if store.append(event).is_err() {
            // Keep user operation results intact. A later call may reopen a
            // changed/reenabled store; no retry of the user effect is requested.
            stores().lock().remove(&root);
        }
    }
    fn emit_home(&self, event: &str, outcome: &str, extra: &[&str]) {
        if let Some(root) = &self.context.root {
            self.emit(root, event, Vec::new(), outcome, extra);
        }
    }

    fn mark_missing_terminal(&self) {
        if !self.terminal.swap(true, Ordering::Relaxed) {
            self.emit_home(
                "observer-diagnostic",
                "missing-terminal",
                &["missing-terminal"],
            );
        }
    }
}
struct Incomplete(Arc<Request>);
impl Drop for Incomplete {
    fn drop(&mut self) {
        self.0.mark_missing_terminal();
    }
}
tokio::task_local! {static CURRENT:Arc<Request>;}
pub(crate) trait ArgumentFingerprint {
    fn argument_fingerprint(self) -> Result<Fingerprint, &'static str>;
}
impl ArgumentFingerprint for &Value {
    fn argument_fingerprint(self) -> Result<Fingerprint, &'static str> {
        fingerprint(self)
    }
}
impl ArgumentFingerprint for Result<Fingerprint, &'static str> {
    fn argument_fingerprint(self) -> Result<Fingerprint, &'static str> {
        self
    }
}

pub(crate) async fn observe_call<F, A: ArgumentFingerprint>(
    context: ObservationContext,
    invocation: LocalInvocationId,
    tool: &str,
    arguments: A,
    future: F,
) -> Result<CallToolResult, rmcp::ErrorData>
where
    F: Future<Output = Result<CallToolResult, rmcp::ErrorData>>,
{
    let mut diagnostics = context.diagnostics.clone();
    let fingerprints = match arguments.argument_fingerprint() {
        Ok(f) => vec![f],
        Err(_) => {
            diagnostics.push("unsupported-fingerprint".into());
            Vec::new()
        }
    };
    let known = TOOLS.contains(&tool);
    if !known {
        diagnostics.push("unsupported-tool".into());
    }
    if [
        "run_command",
        "exec_command",
        "shell",
        "execute",
        "code_mode",
    ]
    .contains(&tool)
    {
        diagnostics.push("opaque-effects".into());
    }
    let request = Arc::new(Request {
        context,
        invocation,
        tool: known.then(|| tool.into()),
        fingerprints,
        diagnostics,
        terminal: AtomicBool::new(false),
        attempted_effect: AtomicBool::new(false),
        raw_outcome: Mutex::new(None),
        pending: Mutex::new(HashMap::new()),
    });
    request.emit_home("request-observed", "requested", &[]);
    let guard = Incomplete(request.clone());
    let result = CURRENT.scope(request.clone(), future).await;
    let mut outcome = match &result {
        Err(_) => "error-possible-effects",
        Ok(result) if result.is_error == Some(true) => "error-possible-effects",
        Ok(result) => {
            if let Some(raw) = *request.raw_outcome.lock() {
                raw
            } else {
                let refused = result
                    .content
                    .first()
                    .and_then(|c| c.as_text())
                    .and_then(|t| serde_json::from_str::<Value>(&t.text).ok())
                    .is_some_and(|v| semantic_refused(&v));
                if refused {
                    "refused"
                } else {
                    "returned-success"
                }
            }
        }
    };
    // A refusal after an attempted write is not proof of a pre-effect refusal.
    if outcome == "refused" && request.attempted_effect.load(Ordering::Relaxed) {
        outcome = "error-possible-effects";
    }
    request.terminal.store(true, Ordering::Relaxed);
    request.emit_home("result-observed", outcome, &[]);
    drop(guard);
    result
}

/// Existing cancellation paths park their response task; record before that park,
/// rather than waiting for the outer observation scope to be dropped on disconnect.
pub(crate) fn observe_cancelled() {
    let _ = CURRENT.try_with(|request| request.mark_missing_terminal());
}

pub(crate) fn dispatch_at(root: &Path) {
    let _ = CURRENT.try_with(|request| {
        let mismatch = request.context.root.as_ref().is_some_and(|old| {
            super::store::checkout(old).ok() != super::store::checkout(root).ok()
        });
        request.emit(
            root,
            "dispatch-observed",
            Vec::new(),
            "dispatched",
            if mismatch {
                &["checkout-mismatch"]
            } else {
                &[]
            },
        );
    });
}
/// Same refusal sentinels as scripts/provenance_records.py::result_refused.
/// Keep legacy usage outcome policy separate: this observer cannot change a guard's permissions.
fn semantic_refused(value: &Value) -> bool {
    crate::usage::recoverable_message(value).is_some()
        || value.get("ok").and_then(Value::as_bool) == Some(false)
        || value.get("pending_ack").is_some()
}

pub(crate) fn observe_value_result(result: &anyhow::Result<Value>) {
    let _ = CURRENT.try_with(|request| {
        let outcome = match result {
            Err(e) if e.downcast_ref::<crate::tools::RecoverableError>().is_some() => "refused",
            Err(_) => "error-possible-effects",
            Ok(v) if semantic_refused(v) => "refused",
            Ok(_) => "returned-success",
        };
        *request.raw_outcome.lock() = Some(outcome);
    });
}
pub(crate) fn observe_write(path: &Path, supplied_bytes: &[u8], phase: WritePhase) {
    let _ = CURRENT.try_with(|request| {
        request.attempted_effect.store(true, Ordering::Relaxed);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let Ok(cwd) = std::env::current_dir() else {
                return;
            };
            cwd.join(path)
        };
        let Some(parent) = path.parent() else {
            return;
        };
        let Ok(parent) = parent.canonicalize() else {
            request.emit_home("observer-diagnostic", "unknown", &["incomplete-input"]);
            return;
        };
        let Some(name) = path.file_name() else {
            return;
        };
        let resolved = parent.join(name);
        let Some(resolved_text) = resolved.to_str() else {
            request.emit_home("observer-diagnostic", "unknown", &["incomplete-input"]);
            return;
        };
        let Ok(root) = super::store::checkout(&parent) else {
            request.emit_home("observer-diagnostic", "unknown", &["checkout-mismatch"]);
            return;
        };
        if store(&root).is_none() {
            return;
        }
        let identity = parent_identity(&resolved);
        let mut pending = request.pending.lock();
        let phase = match phase {
            WritePhase::Attempted => {
                if pending.len() >= 64 {
                    request.emit_home("observer-diagnostic", "unknown", &["incomplete-input"]);
                    return;
                }
                if let Some(identity) = identity {
                    pending.insert(path.clone(), identity);
                }
                "attempted"
            }
            WritePhase::Committed => {
                if pending
                    .remove(&path)
                    .zip(identity)
                    .is_some_and(|(a, b)| a == b)
                {
                    "committed"
                } else {
                    "uncertain"
                }
            }
            WritePhase::Uncertain => {
                pending.remove(&path);
                "uncertain"
            }
        };
        drop(pending);
        let mismatch = request
            .context
            .root
            .as_ref()
            .is_some_and(|p| super::store::checkout(p).ok().as_ref() != Some(&root));
        request.emit(
            &root,
            "dispatch-observed",
            vec![TargetFact {
                path: resolved_text.to_owned(),
                phase: phase.into(),
                bytes_len: supplied_bytes.len() as u64,
                fingerprint: Some(bytes_fingerprint(supplied_bytes)),
            }],
            "dispatched",
            if mismatch {
                &["checkout-mismatch"]
            } else {
                &[]
            },
        );
    });
}
