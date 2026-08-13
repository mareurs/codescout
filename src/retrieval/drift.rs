use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub chunk_id: String,
    pub content_hash: String,
    /// Forward-slashed project-relative path, as stored in the payload's
    /// `file_path`. Present so the dirty-set derivation can find paths that exist
    /// in an index but not on disk without parsing `chunk_id`.
    pub file_path: String,
}

#[derive(Debug, Default)]
pub struct DriftAction {
    pub to_upsert: Vec<String>,
    pub to_delete: Vec<String>,
}

pub fn diff_chunks(server: &[ChunkRef], local: &[ChunkRef]) -> DriftAction {
    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let local_ids: HashSet<&str> = local.iter().map(|c| c.chunk_id.as_str()).collect();
    let to_upsert = local
        .iter()
        .filter(|c| !server_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let to_delete = server
        .iter()
        .filter(|c| !local_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    DriftAction {
        to_upsert,
        to_delete,
    }
}
