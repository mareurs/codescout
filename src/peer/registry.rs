//! Peer registry: id → target workspace + description + default access. The
//! serve socket is *derived* from the target via socket_discovery, never stored.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::socket_discovery::peer_socket_path_for_workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    #[serde(rename = "ro")]
    ReadOnly,
    #[serde(rename = "rw")]
    ReadWrite,
}

impl Access {
    pub fn is_read_only(self) -> bool {
        matches!(self, Access::ReadOnly)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerEntry {
    pub id: String,
    pub target: PathBuf,
    /// Agentic selection surface — a manager picks a peer by reading this.
    pub description: String,
    pub default_access: Access,
}

impl PeerEntry {
    pub fn socket_path(&self) -> PathBuf {
        peer_socket_path_for_workspace(&self.target)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Registry {
    #[serde(default, rename = "peer")]
    peers: Vec<PeerEntry>,
}

impl Registry {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("failed to parse peer registry TOML")
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Registry::default());
        }
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read peer registry: {}", path.display()))?;
        Self::from_toml_str(&s)
    }

    pub fn get(&self, id: &str) -> Option<&PeerEntry> {
        self.peers.iter().find(|p| p.id == id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.peers.iter().map(|p| p.id.as_str())
    }
    pub fn entries(&self) -> &[PeerEntry] {
        &self.peers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registry_and_resolves_socket() {
        let toml = r#"
            [[peer]]
            id = "backend"
            target = "/home/u/projB"
            description = "The payments backend — Rust, axum, sqlx"
            default_access = "ro"

            [[peer]]
            id = "frontend"
            target = "/home/u/projC"
            description = "Web client — TypeScript, React"
            default_access = "rw"
        "#;
        let reg = Registry::from_toml_str(toml).unwrap();
        let backend = reg.get("backend").unwrap();
        assert_eq!(backend.default_access, Access::ReadOnly);
        assert!(backend.description.contains("payments"));
        let sock = backend.socket_path();
        assert!(sock.to_str().unwrap().contains("codescout-peer-"));
        assert_eq!(
            reg.get("frontend").unwrap().default_access,
            Access::ReadWrite
        );
        assert!(reg.get("missing").is_none());
        assert_eq!(reg.ids().count(), 2);
    }

    #[test]
    fn missing_file_is_empty_registry() {
        let reg = Registry::load(std::path::Path::new("/nonexistent/peers.toml")).unwrap();
        assert_eq!(reg.ids().count(), 0);
    }
}
