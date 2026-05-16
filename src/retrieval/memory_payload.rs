//! Payload and identity for the Qdrant `memories` collection.
//!
//! Semantic memories live in a single shared collection filtered by
//! `project_id`. Each point carries the memory itself plus an embedded
//! array of file anchors. The point ID is UUIDv5 over
//! `(project_id, bucket, title)` so upsert is idempotent without a
//! lookup round-trip.

use anyhow::{anyhow, Context, Result};
use qdrant_client::qdrant::Value;
use qdrant_client::Payload;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Namespace for UUIDv5 derivation of memory point IDs. Stable across
/// versions — never change.
const MEMORY_NS: Uuid = Uuid::from_bytes([
    0x6c, 0x6f, 0x2d, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x2d, 0x76, 0x35, 0x6e, 0x73, 0x00, 0x01,
]);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryAnchor {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemory {
    pub project_id: String,
    pub bucket: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub anchors: Vec<MemoryAnchor>,
    pub created_at: String,
    pub updated_at: String,
}

impl SemanticMemory {
    /// Deterministic point ID — UUIDv5 over `(project_id, bucket, title)`.
    /// Same inputs → same id, so re-titling content moves it to a new point.
    pub fn point_id(&self) -> Uuid {
        point_id_for(&self.project_id, &self.bucket, &self.title)
    }
}
/// Compute the deterministic point ID for a memory's `(project_id, bucket,
/// title)` tuple without constructing a full `SemanticMemory`. Used by
/// callers that need to derive an id for delete-by-name (the cross-embed
/// delete sidecar in `memory(action="delete")`).
pub fn point_id_for(project_id: &str, bucket: &str, title: &str) -> Uuid {
    let key = format!("{project_id}\x1f{bucket}\x1f{title}");
    Uuid::new_v5(&MEMORY_NS, key.as_bytes())
}

/// Serialize a memory into the Qdrant payload map. The `anchors` array
/// becomes a list of nested string maps so payload field indexes on
/// `anchors[].path` work as expected.
pub fn memory_to_payload(m: &SemanticMemory) -> HashMap<String, Value> {
    let mut map = HashMap::new();
    map.insert("project_id".into(), Value::from(m.project_id.clone()));
    map.insert("bucket".into(), Value::from(m.bucket.clone()));
    map.insert("title".into(), Value::from(m.title.clone()));
    map.insert("content".into(), Value::from(m.content.clone()));
    map.insert("created_at".into(), Value::from(m.created_at.clone()));
    map.insert("updated_at".into(), Value::from(m.updated_at.clone()));

    let anchors: Vec<Value> = m
        .anchors
        .iter()
        .map(|a| {
            let mut inner: HashMap<String, Value> = HashMap::new();
            inner.insert("path".into(), Value::from(a.path.clone()));
            Value::from(Payload::from(inner))
        })
        .collect();
    map.insert("anchors".into(), Value::from(anchors));
    map
}

/// Parse a Qdrant payload map back into a `SemanticMemory`.
pub fn payload_to_memory(m: &HashMap<String, Value>) -> Result<SemanticMemory> {
    Ok(SemanticMemory {
        project_id: get_str(m, "project_id")?,
        bucket: get_str(m, "bucket")?,
        title: get_str(m, "title")?,
        content: get_str(m, "content")?,
        anchors: get_anchors(m)?,
        created_at: get_str(m, "created_at")?,
        updated_at: get_str(m, "updated_at")?,
    })
}

fn get_str(m: &HashMap<String, Value>, key: &str) -> Result<String> {
    m.get(key)
        .ok_or_else(|| anyhow!("missing field: {key}"))?
        .as_str()
        .map(|s| s.as_str().to_owned())
        .ok_or_else(|| anyhow!("field {key} is not a string"))
}

fn get_anchors(m: &HashMap<String, Value>) -> Result<Vec<MemoryAnchor>> {
    let Some(v) = m.get("anchors") else {
        return Ok(vec![]);
    };
    let list = v
        .as_list()
        .ok_or_else(|| anyhow!("field anchors is not a list"))?;
    list.iter()
        .map(|item| {
            let inner = item
                .as_struct()
                .ok_or_else(|| anyhow!("anchor item is not a struct"))?
                .fields
                .clone();
            Ok(MemoryAnchor {
                path: get_str(&inner, "path").context("anchor.path")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SemanticMemory {
        SemanticMemory {
            project_id: "codescout".into(),
            bucket: "system".into(),
            title: "BUG-021 parallel writes".into(),
            content: "Never dispatch parallel write tool calls...".into(),
            anchors: vec![MemoryAnchor {
                path: "src/tools/memory/mod.rs".into(),
            }],
            created_at: "2026-03-03T12:00:00Z".into(),
            updated_at: "2026-05-13T05:00:00Z".into(),
        }
    }

    #[test]
    fn point_id_is_stable_for_same_inputs() {
        let m1 = sample();
        let m2 = sample();
        assert_eq!(m1.point_id(), m2.point_id());
    }

    #[test]
    fn point_id_differs_across_projects() {
        let m1 = sample();
        let mut m2 = sample();
        m2.project_id = "other-project".into();
        assert_ne!(m1.point_id(), m2.point_id());
    }

    #[test]
    fn point_id_differs_across_buckets() {
        let m1 = sample();
        let mut m2 = sample();
        m2.bucket = "preferences".into();
        assert_ne!(m1.point_id(), m2.point_id());
    }

    #[test]
    fn payload_roundtrip() {
        let m = sample();
        let payload = memory_to_payload(&m);
        let back = payload_to_memory(&payload).expect("roundtrip");
        assert_eq!(back.project_id, m.project_id);
        assert_eq!(back.bucket, m.bucket);
        assert_eq!(back.title, m.title);
        assert_eq!(back.content, m.content);
        assert_eq!(back.anchors, m.anchors);
        assert_eq!(back.created_at, m.created_at);
        assert_eq!(back.updated_at, m.updated_at);
    }

    #[test]
    fn payload_roundtrip_no_anchors() {
        let mut m = sample();
        m.anchors.clear();
        let payload = memory_to_payload(&m);
        let back = payload_to_memory(&payload).expect("roundtrip");
        assert!(back.anchors.is_empty());
    }
}
