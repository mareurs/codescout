#[cfg(feature = "server-stack")]
use anyhow::{anyhow, Result};
#[cfg(feature = "server-stack")]
use std::collections::HashMap;

#[cfg(feature = "server-stack")]
use qdrant_client::qdrant::Value;

#[derive(Debug, Clone)]
pub struct CodePayload {
    pub project_id: String,
    pub file_path: String,
    pub language: String,
    pub start_line: i64,
    pub end_line: i64,
    /// The chunk's identity line — `src/foo.rs :: impl Bar :: fn baz(&self)`.
    ///
    /// Produced by `embed::ast_chunker::build_metadata_header` and prepended to
    /// the embedding input by [`embed_text`]. Empty for non-AST chunks (markdown,
    /// plain text), which embed as bare content.
    pub ast_header: String,
    pub content: String,
    pub content_hash: String,
    pub last_indexed_commit: String,
    pub chunk_id: String,
}

/// The text handed to the embedder for one chunk — the single home for that decision.
///
/// It had none before. `flush_pending` read `payload.content` inline, so what got
/// embedded was decided by whichever fields `stream_index` happened to fill in a
/// struct literal a hundred lines away. That is how the AST header — computed for
/// every chunk, with nine tests pinning its shape — silently stopped reaching the
/// embedder: the legacy `embed::index` path that prepended it was deleted in
/// `66db4c70`, and the test that pinned the `{header}\n{content}` contract
/// (`embed_text_format_includes_metadata_prefix`) went with the module it lived in.
/// See `docs/issues/2026-08-08-metadata-header-computed-but-never-embedded-or-stored.md`.
///
/// Changing what a chunk looks like to the embedder means changing this function and
/// nothing else.
pub fn embed_text(p: &CodePayload) -> String {
    if p.ast_header.is_empty() {
        p.content.clone()
    } else {
        format!("{}\n{}", p.ast_header, p.content)
    }
}

#[cfg(feature = "server-stack")]
pub fn payload_to_map(p: &CodePayload) -> HashMap<String, Value> {
    let mut m = HashMap::new();
    m.insert("project_id".into(), Value::from(p.project_id.clone()));
    m.insert("file_path".into(), Value::from(p.file_path.clone()));
    m.insert("language".into(), Value::from(p.language.clone()));
    m.insert("start_line".into(), Value::from(p.start_line));
    m.insert("end_line".into(), Value::from(p.end_line));
    m.insert("ast_header".into(), Value::from(p.ast_header.clone()));
    m.insert("content".into(), Value::from(p.content.clone()));
    m.insert("content_hash".into(), Value::from(p.content_hash.clone()));
    m.insert(
        "last_indexed_commit".into(),
        Value::from(p.last_indexed_commit.clone()),
    );
    m.insert("chunk_id".into(), Value::from(p.chunk_id.clone()));
    m
}

#[cfg(feature = "server-stack")]
fn get_str<'a>(m: &'a HashMap<String, Value>, key: &str) -> Result<&'a str> {
    m.get(key)
        .ok_or_else(|| anyhow!("missing field: {key}"))?
        .as_str()
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow!("field {key} is not a string"))
}

#[cfg(feature = "server-stack")]
fn get_int(m: &HashMap<String, Value>, key: &str) -> Result<i64> {
    m.get(key)
        .ok_or_else(|| anyhow!("missing field: {key}"))?
        .as_integer()
        .ok_or_else(|| anyhow!("field {key} is not an integer"))
}

#[cfg(feature = "server-stack")]
pub fn map_to_payload(m: &HashMap<String, Value>) -> Result<CodePayload> {
    // Points written before 2026-08-08 also carry an `ast_kind` key, always "".
    // It had no producer and no reader, so the field was dropped rather than
    // populated; the stale key on old points is simply not read.
    Ok(CodePayload {
        project_id: get_str(m, "project_id")?.to_owned(),
        file_path: get_str(m, "file_path")?.to_owned(),
        language: get_str(m, "language")?.to_owned(),
        start_line: get_int(m, "start_line")?,
        end_line: get_int(m, "end_line")?,
        ast_header: get_str(m, "ast_header")?.to_owned(),
        content: get_str(m, "content")?.to_owned(),
        content_hash: get_str(m, "content_hash")?.to_owned(),
        last_indexed_commit: get_str(m, "last_indexed_commit")?.to_owned(),
        chunk_id: get_str(m, "chunk_id")?.to_owned(),
    })
}
