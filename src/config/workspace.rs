use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub resources: ResourcesSection,
    #[serde(default)]
    pub exclude_projects: Vec<String>,
    #[serde(default, rename = "project")]
    pub projects: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSection {
    pub name: String,
    #[serde(default = "default_discovery_depth")]
    pub discovery_max_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesSection {
    #[serde(default = "default_max_lsp_clients")]
    pub max_lsp_clients: usize,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

impl Default for ResourcesSection {
    fn default() -> Self {
        Self {
            max_lsp_clients: default_max_lsp_clients(),
            idle_timeout_secs: default_idle_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: String,
    pub root: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_discovery_depth() -> usize {
    3
}
fn default_max_lsp_clients() -> usize {
    5
}
fn default_idle_timeout() -> u64 {
    600
}

/// Return the canonical path to the workspace config file for a given project root.
pub fn workspace_config_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".codescout").join("workspace.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_workspace_config() {
        // Note: exclude_projects must appear before any [section] headers in TOML
        // because after a [section] header, bare keys belong to that section.
        let toml_str = r#"
exclude_projects = ["node_modules", "build"]

[workspace]
name = "backend-kotlin"
discovery_max_depth = 4

[resources]
max_lsp_clients = 3
idle_timeout_secs = 300

[[project]]
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["backend-kotlin"]
"#;
        let config: WorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.name, "backend-kotlin");
        assert_eq!(config.workspace.discovery_max_depth, 4);
        assert_eq!(config.resources.max_lsp_clients, 3);
        assert_eq!(config.resources.idle_timeout_secs, 300);
        assert_eq!(config.exclude_projects, vec!["node_modules", "build"]);
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].id, "mcp-server");
        assert_eq!(config.projects[0].depends_on, vec!["backend-kotlin"]);
    }

    #[test]
    fn defaults_are_sensible() {
        let toml_str = r#"
[workspace]
name = "test"
"#;
        let config: WorkspaceConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.workspace.discovery_max_depth, 3);
        assert_eq!(config.resources.max_lsp_clients, 5);
        assert_eq!(config.resources.idle_timeout_secs, 600);
        assert!(config.exclude_projects.is_empty());
        assert!(config.projects.is_empty());
    }
}
