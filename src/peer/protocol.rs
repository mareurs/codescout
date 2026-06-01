//! Wire envelope for the peer-delegation protocol. Networked-ready: pure JSON,
//! no Unix-socket assumptions. A future TCP/TLS transport swaps only the
//! connect/listen layer, not this type.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bumped on any breaking change to the envelope or method set.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnvelopeKind {
    Request,
    Response,
    Event,
    Error,
}

/// The single message type on the wire. `serde(skip_serializing_if)` keeps
/// frames minimal (null fields omitted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEnvelope {
    pub v: u32,
    pub id: String,
    pub kind: EnvelopeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<PeerError>,
}

impl PeerEnvelope {
    pub fn request(id: impl Into<String>, method: &str, params: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Request,
            method: Some(method.into()),
            params: Some(params),
            result: None,
            error: None,
        }
    }

    pub fn response(id: impl Into<String>, result: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Response,
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: impl Into<String>, error: PeerError) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            kind: EnvelopeKind::Error,
            method: None,
            params: None,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Closed taxonomy. Maps onto codescout's `RecoverableError` on the requester
/// side so a peer's bad-input failure does not abort sibling tool calls.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    VersionMismatch,
    UnknownMethod,
    UnknownTool,
    AccessDenied,
    ToolError,
    UnknownHandle,
    BadParams,
}

/// Advertised by `hello`. Lets the requester avoid sending a write tool to a
/// read-only peer (defence-in-depth on top of the peer-side wall).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub project: String,
    pub root: String,
    pub read_only: bool,
    pub tools: Vec<String>,
    pub executor_available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_envelope_round_trips() {
        let env = PeerEnvelope::request(
            "a:1",
            "tool.call",
            json!({ "tool": "symbols", "args": { "path": "src" } }),
        );
        let wire = serde_json::to_value(&env).unwrap();
        assert_eq!(wire["kind"], "request");
        assert_eq!(wire["method"], "tool.call");
        assert!(wire.get("result").is_none());
        assert!(wire.get("error").is_none());
        let back: PeerEnvelope = serde_json::from_value(wire).unwrap();
        assert_eq!(back.method.as_deref(), Some("tool.call"));
    }

    #[test]
    fn error_envelope_carries_code() {
        let env = PeerEnvelope::error(
            "a:1",
            PeerError {
                code: ErrorCode::AccessDenied,
                message: "peer is read-only".into(),
                data: None,
            },
        );
        let wire = serde_json::to_value(&env).unwrap();
        assert_eq!(wire["kind"], "error");
        assert_eq!(wire["error"]["code"], "access_denied");
    }
}
