use sha2::{Digest, Sha256};

/// Stable artifact id: sha256("{repo}\n{rel_path}") hex, truncated to 16 chars.
pub fn artifact_id(repo: &str, rel_path: &str) -> String {
    let mut h = Sha256::new();
    h.update(repo.as_bytes());
    h.update(b"\n");
    h.update(rel_path.as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex[..16].into()
}
/// Stable artifact id: sha256(abs_path) hex, truncated to 16 chars.
pub fn artifact_id_from_abs(abs_path: &std::path::Path) -> String {
    let mut h = Sha256::new();
    h.update(abs_path.to_string_lossy().as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex[..16].into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        assert_eq!(artifact_id("r", "p.md"), artifact_id("r", "p.md"));
    }

    #[test]
    fn different_inputs_different_ids() {
        assert_ne!(artifact_id("r1", "p.md"), artifact_id("r2", "p.md"));
        assert_ne!(artifact_id("r", "a.md"), artifact_id("r", "b.md"));
    }

    #[test]
    fn sixteen_hex_chars() {
        let id = artifact_id("r", "p.md");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
