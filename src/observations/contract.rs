//! Minimal stock observations; these records never establish exclusive coverage.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const EVENTS: &[&str] = &[
    "request-observed",
    "dispatch-observed",
    "result-observed",
    "hook-pre",
    "hook-post",
    "lifecycle",
    "observer-diagnostic",
];
pub const OUTCOMES: &[&str] = &[
    "requested",
    "dispatched",
    "returned-success",
    "refused",
    "error-possible-effects",
    "missing-terminal",
    "unknown",
];
pub const DIAGNOSTICS: &[&str] = &[
    "missing-root",
    "invalid-context",
    "context-conflict",
    "unsupported-tool",
    "unsupported-event",
    "incomplete-input",
    "opaque-effects",
    "missing-terminal",
    "unavailable",
    "disabled",
    "quota",
    "torn-record",
    "checksum",
    "sequence-gap",
    "conflicting-duplicate",
    "ambiguous-pair",
    "incompatible-schema",
    "checkout-mismatch",
    "reset",
    "unsupported-fingerprint",
];
pub const TOOLS: &[&str] = &[
    "create_file",
    "edit_file",
    "edit_code",
    "run_command",
    "read_file",
    "symbols",
    "grep",
    "semantic_search",
    "workspace",
    "doc",
    "memory",
    "find_file",
    "references",
    "call_graph",
    "symbol_at",
    "get_guide",
    "onboarding",
    "index",
    "execute",
    "exec_command",
    "shell",
    "code_mode",
];
const FIELDS: &[&str] = &[
    "schema_version",
    "observation_id",
    "producer_instance",
    "sequence",
    "store_epoch",
    "event",
    "producer_kind",
    "workspace",
    "caller",
    "tool",
    "local_invocation",
    "native_call_id",
    "turn_id",
    "fingerprints",
    "targets",
    "outcome",
    "diagnostics",
];
const SAFE: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Caller {
    pub harness: String,
    pub principal: Option<String>,
    pub thread_id: Option<String>,
    pub agent_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fingerprint {
    pub domain: String,
    pub digest: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TargetFact {
    pub path: String,
    pub phase: String,
    pub bytes_len: u64,
    pub fingerprint: Option<Fingerprint>,
}
/// Codes are from a closed allowlist; no rejected payload or free-form errors.
pub type Diagnostic = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservationV1 {
    pub schema_version: u32,
    pub observation_id: String,
    pub producer_instance: String,
    pub sequence: u64,
    pub store_epoch: String,
    pub event: String,
    pub producer_kind: String,
    pub workspace: String,
    pub caller: Caller,
    pub tool: Option<String>,
    pub local_invocation: Option<String>,
    pub native_call_id: Option<String>,
    pub turn_id: Option<String>,
    pub fingerprints: Vec<Fingerprint>,
    pub targets: Vec<TargetFact>,
    pub outcome: String,
    pub diagnostics: Vec<Diagnostic>,
}
fn text(s: &str, max: usize) -> Result<(), &'static str> {
    if s.trim().is_empty() || s.len() > max || s.chars().any(|c| c < ' ') {
        return Err("invalid-string");
    }
    Ok(())
}
fn path(s: &str) -> Result<(), &'static str> {
    text(s, 4096)?;
    if !s.starts_with('/') {
        return Err("invalid-path");
    }
    Ok(())
}
fn optional(s: &Option<String>) -> Result<(), &'static str> {
    if let Some(s) = s {
        text(s, 1024)?;
    }
    Ok(())
}
fn uuid(s: &str) -> Result<(), &'static str> {
    if uuid::Uuid::parse_str(s)
        .map(|v| v.to_string())
        .ok()
        .as_deref()
        != Some(s)
    {
        return Err("invalid-uuid");
    }
    Ok(())
}
fn exact(v: &Value, keys: &[&str]) -> Result<(), &'static str> {
    let obj = v.as_object().ok_or("invalid-object")?;
    let actual: BTreeSet<_> = obj.keys().map(String::as_str).collect();
    let want: BTreeSet<_> = keys.iter().copied().collect();
    if actual != want {
        return Err("invalid-fields");
    }
    Ok(())
}
fn validate_fp(f: &Fingerprint) -> Result<(), &'static str> {
    if !["canonical-json-safe-v1", "bytes-v1"].contains(&f.domain.as_str())
        || f.digest.len() != 64
        || !f
            .digest
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("invalid-fingerprint");
    }
    Ok(())
}
pub fn validate_observation(v: Value) -> Result<ObservationV1, &'static str> {
    exact(&v, FIELDS)?;
    exact(
        &v["caller"],
        &["harness", "principal", "thread_id", "agent_id"],
    )?;
    if let Some(ts) = v["targets"].as_array() {
        for t in ts {
            exact(t, &["path", "phase", "bytes_len", "fingerprint"])?;
        }
    }
    let r: ObservationV1 = serde_json::from_value(v).map_err(|_| "invalid-types")?;
    if r.schema_version != 1 || r.sequence > SAFE {
        return Err("incompatible-schema");
    }
    text(&r.observation_id, 1024)?;
    uuid(&r.producer_instance)?;
    uuid(&r.store_epoch)?;
    path(&r.workspace)?;
    if !EVENTS.contains(&r.event.as_str())
        || !["codex-hook", "codescout-server"].contains(&r.producer_kind.as_str())
        || !OUTCOMES.contains(&r.outcome.as_str())
    {
        return Err("invalid-enum");
    }
    if r.caller.harness != "codex" {
        return Err("invalid-harness");
    }
    optional(&r.caller.principal)?;
    optional(&r.caller.thread_id)?;
    optional(&r.caller.agent_id)?;
    if r.caller
        .principal
        .as_ref()
        .is_some_and(|p| !p.starts_with("codex:") || p.len() == 6)
    {
        return Err("invalid-principal");
    }
    optional(&r.local_invocation)?;
    optional(&r.native_call_id)?;
    optional(&r.turn_id)?;
    if r.tool
        .as_ref()
        .is_some_and(|t| !TOOLS.contains(&t.as_str()))
    {
        return Err("unsupported-tool");
    }
    if r.fingerprints.len() > 4 || r.targets.len() > 64 || r.diagnostics.len() > 32 {
        return Err("record-limit");
    }
    for f in &r.fingerprints {
        validate_fp(f)?;
    }
    for t in &r.targets {
        path(&t.path)?;
        if !["attempted", "committed", "uncertain"].contains(&t.phase.as_str())
            || t.bytes_len > SAFE
        {
            return Err("invalid-target");
        }
        if let Some(f) = &t.fingerprint {
            validate_fp(f)?;
        }
    }
    if r.diagnostics
        .iter()
        .any(|d| !DIAGNOSTICS.contains(&d.as_str()))
    {
        return Err("invalid-diagnostic");
    }
    if serde_json::to_vec(&r).map_err(|_| "invalid-record")?.len() > 65536 {
        return Err("record-limit");
    }
    Ok(r)
}
pub fn canonical_json(v: &Value) -> Result<String, &'static str> {
    fn encode(v: &Value, depth: usize) -> Result<String, &'static str> {
        if depth > 32 {
            return Err("depth-limit");
        }
        match v {
            Value::Null => Ok("null".into()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Number(n) => {
                if let Some(i) = n.as_i64().filter(|i| i.unsigned_abs() <= SAFE) {
                    Ok(i.to_string())
                } else if let Some(i) = n.as_u64().filter(|i| *i <= SAFE) {
                    Ok(i.to_string())
                } else {
                    Err("unsupported-number")
                }
            }
            Value::String(s) => {
                let mut out = String::from("\"");
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
                        _ => out.push(c),
                    }
                }
                out.push('"');
                Ok(out)
            }
            Value::Array(a) => Ok(format!(
                "[{}]",
                a.iter()
                    .map(|x| encode(x, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(",")
            )),
            Value::Object(o) => {
                let mut keys: Vec<_> = o.keys().collect();
                keys.sort();
                Ok(format!(
                    "{{{}}}",
                    keys.into_iter()
                        .map(|k| Ok(format!(
                            "{}:{}",
                            encode(&Value::String(k.clone()), depth + 1)?,
                            encode(&o[k], depth + 1)?
                        )))
                        .collect::<Result<Vec<_>, &str>>()?
                        .join(",")
                ))
            }
        }
    }
    encode(v, 0)
}
pub fn fingerprint(v: &Value) -> Result<Fingerprint, &'static str> {
    Ok(Fingerprint {
        domain: "canonical-json-safe-v1".into(),
        digest: hex::encode(Sha256::digest(canonical_json(v)?.as_bytes())),
    })
}
pub fn bytes_fingerprint(bytes: &[u8]) -> Fingerprint {
    Fingerprint {
        domain: "bytes-v1".into(),
        digest: hex::encode(Sha256::digest(bytes)),
    }
}
