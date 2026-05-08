use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::classify::Rule;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub roots: Vec<Root>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub umbrellas: Vec<Umbrella>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub name: String,
    pub path: PathBuf,
}

/// User-declared grouping of sub-projects that share enough context to be
/// queried together. Members are absolute filesystem paths; the umbrella
/// matches any current project whose `abs_path` is a descendant of any member.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Umbrella {
    pub name: String,
    #[serde(default)]
    pub members: Vec<PathBuf>,
}

pub fn default_config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("no config dir")?;
    Ok(base.join("librarian").join("workspace.toml"))
}

pub fn load(path: &Path) -> Result<WorkspaceConfig> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: WorkspaceConfig = toml::from_str(&s).context("parsing workspace.toml")?;
    Ok(cfg)
}

/// Compile a list of glob patterns into a [`globset::GlobSet`] for ignore matching.
/// Returns an empty set if `patterns` is empty (matches nothing).
pub fn compile_ignore(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut b = globset::GlobSetBuilder::new();
    for p in patterns {
        b.add(globset::Glob::new(p).with_context(|| format!("invalid ignore glob: {p}"))?);
    }
    b.build().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn loads_minimal_config() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
[[roots]]
name = "backend-kotlin"
path = "/home/x/work/backend-kotlin"

[[roots]]
name = "eduplanner-ui"
path = "/home/x/work/eduplanner-ui"

[[rule]]
glob = "**/docs/specs/*.md"
kind = "spec"
"#
        )
        .unwrap();
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.roots.len(), 2);
        assert_eq!(cfg.rules.len(), 1);
    }

    #[test]
    fn rejects_typo_field() {
        let mut f = NamedTempFile::new().unwrap();
        // "rooots" typo - should be "roots"
        writeln!(
            f,
            r#"
[[rooots]]
name = "x"
path = "/tmp"
"#
        )
        .unwrap();
        assert!(load(f.path()).is_err());
    }
}
