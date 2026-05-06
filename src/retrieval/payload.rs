use anyhow::{anyhow, Result};
use std::collections::HashMap;

use qdrant_client::qdrant::Value;

#[derive(Debug, Clone)]
pub struct CodePayload {
    pub project_id:          String,
    pub file_path:           String,
    pub language:            String,
    pub start_line:          i64,
    pub end_line:            i64,
    pub ast_kind:            String,
    pub ast_header:          String,
    pub content:             String,
    pub content_hash:        String,
    pub last_indexed_commit: String,
    pub chunk_id:            String,
}

pub fn payload_to_map(p: &CodePayload) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    m.insert("project_id".into(),          Value::from(p.project_id.clone()));
    m.insert("file_path".into(),           Value::from(p.file_path.clone()));
    m.insert("language".into(),            Value::from(p.language.clone()));
    m.insert("start_line".into(),          Value::from(p.start_line));
    m.insert("end_line".into(),            Value::from(p.end_line));
    m.insert("ast_kind".into(),            Value::from(p.ast_kind.clone()));
    m.insert("ast_header".into(),          Value::from(p.ast_header.clone()));
    m.insert("content".into(),             Value::from(p.content.clone()));
    m.insert("content_hash".into(),        Value::from(p.content_hash.clone()));
    m.insert("last_indexed_commit".into(), Value::from(p.last_indexed_commit.clone()));
    m.insert("chunk_id".into(),            Value::from(p.chunk_id.clone()));
    m
}

fn get_str<'a>(m: &'a HashMap<String, Value>, key: &str) -> Result<&'a str> {
    m.get(key)
        .ok_or_else(|| anyhow!("missing field: {key}"))?
        .as_str()
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow!("field {key} is not a string"))
}

fn get_int(m: &HashMap<String, Value>, key: &str) -> Result<i64> {
    m.get(key)
        .ok_or_else(|| anyhow!("missing field: {key}"))?
        .as_integer()
        .ok_or_else(|| anyhow!("field {key} is not an integer"))
}

pub fn map_to_payload(m: &HashMap<String, Value>) -> Result<CodePayload> {
    Ok(CodePayload {
        project_id:          get_str(m, "project_id")?.to_owned(),
        file_path:           get_str(m, "file_path")?.to_owned(),
        language:            get_str(m, "language")?.to_owned(),
        start_line:          get_int(m, "start_line")?,
        end_line:            get_int(m, "end_line")?,
        ast_kind:            get_str(m, "ast_kind")?.to_owned(),
        ast_header:          get_str(m, "ast_header")?.to_owned(),
        content:             get_str(m, "content")?.to_owned(),
        content_hash:        get_str(m, "content_hash")?.to_owned(),
        last_indexed_commit: get_str(m, "last_indexed_commit")?.to_owned(),
        chunk_id:            get_str(m, "chunk_id")?.to_owned(),
    })
}
