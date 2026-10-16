use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::librarian::classify::Rule;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub roots: Vec<Root>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    #[serde(default, rename = "umbrella")]
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
    config_path_under(dirs::config_dir())
}

/// The default `workspace.toml` under `config_dir`. A platform with no user config
/// directory is an error that names the way out, not a bare "no config dir".
fn config_path_under(config_dir: Option<PathBuf>) -> Result<PathBuf> {
    let base = config_dir.context(
        "this platform reports no user config directory, so the global librarian \
         workspace.toml has no default location; set LIBRARIAN_WORKSPACE to the path of one",
    )?;
    Ok(base.join("librarian").join("workspace.toml"))
}

pub fn load(path: &Path) -> Result<WorkspaceConfig> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let cfg: WorkspaceConfig = toml::from_str(&s).context("parsing workspace.toml")?;
    if !cfg.roots.is_empty() {
        tracing::warn!(
            "[[roots]] is deprecated; safe to remove from {} after the v6 migration completes. \
             Roots are no longer consulted at query time. See the v6 release notes.",
            path.display()
        );
    }
    Ok(cfg)
}
/// Written to the default path on first start. Comments only: an empty file is a
/// valid [`WorkspaceConfig`], and the built-in classifier rules need no copy here.
///
/// It must stay comments-only. Creation is not atomic with the write, so a server that
/// loses the `create_new` race can read the file while it is still a prefix; a prefix of
/// a comments-only text is always a valid empty config, a prefix of real content might not be.
pub const DEFAULT_WORKSPACE_TOML: &str = r##"# Global librarian workspace config (librarian/workspace.toml in the user config dir).
# Created automatically on first start. An empty file is valid.
# This is NOT the per-project .codescout/workspace.toml.
#
# The built-in classifier rules (specs, plans, ADRs, trackers, ...) need no
# entry here. Rules you add win over the built-in ones. Example:
#
# [[rule]]
# glob = "**/docs/specs/*.md"
# kind = "spec"
#
# Exclude paths from indexing (a top-level key, before any [[table]]):
# ignore = ["**/node_modules/**"]
#
# Group sub-projects that should be queried together:
# [[umbrella]]
# name = "my-umbrella"
# members = ["/abs/path/one", "/abs/path/two"]
"##;

/// [`load`], but with `init` and an absent file, first write [`DEFAULT_WORKSPACE_TOML`].
///
/// Never overwrites: creation is `create_new`, and losing that race to another
/// server is success. A creation failure (unwritable location) is not fatal — the
/// librarian starts on the in-memory default and a warning names the path. An
/// existing file that does not parse is still an error, and is left untouched.
pub fn load_or_init(path: &Path, init: bool) -> Result<WorkspaceConfig> {
    // `try_exists`, not `exists`: `exists()` is false on EACCES too, which would send an
    // existing-but-unreadable file down the create path and replace the user's config
    // with an empty one. Only a confirmed absence creates; any other error falls through
    // to `load`, which reports it.
    if init && confirmed_absent(path) {
        if let Err(e) = create_default(path) {
            tracing::warn!(
                "could not create {}: {e}; starting the librarian with an empty workspace config",
                path.display()
            );
            return Ok(WorkspaceConfig::default());
        }
    }
    load(path)
}

/// True when nothing can live at `path`: it is absent, or a parent is not a directory
/// (creating it will then fail into the in-memory fallback). Any other error — notably
/// `PermissionDenied`, where the file may well exist — is NOT absence.
fn confirmed_absent(path: &Path) -> bool {
    match path.try_exists() {
        Ok(exists) => !exists,
        Err(e) => e.kind() == std::io::ErrorKind::NotADirectory,
    }
}

fn create_default(path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => f.write_all(DEFAULT_WORKSPACE_TOML.as_bytes()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
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

    fn assert_empty(cfg: &WorkspaceConfig) {
        assert!(cfg.roots.is_empty() && cfg.ignore.is_empty());
        assert!(cfg.rules.is_empty() && cfg.umbrellas.is_empty());
    }

    #[test]
    fn init_creates_the_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-yet/librarian/workspace.toml");

        let cfg = load_or_init(&path, true).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            DEFAULT_WORKSPACE_TOML
        );
        assert_empty(&cfg);
    }

    #[test]
    fn the_default_text_parses_to_the_default_config() {
        let cfg: WorkspaceConfig = toml::from_str(DEFAULT_WORKSPACE_TOML).unwrap();
        assert_empty(&cfg);
    }

    #[test]
    fn init_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.toml");
        let mine = "[[rule]]\nglob = \"**/docs/x/*.md\"\nkind = \"x\"\n";
        std::fs::write(&path, mine).unwrap();

        let cfg = load_or_init(&path, true).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), mine);
        assert_eq!(cfg.rules.len(), 1);
    }
    #[test]
    fn create_default_leaves_an_existing_file_alone() {
        // The race loser: another server created the file between the absence check and
        // `create_new`. That is success, and the winner's bytes must survive.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.toml");
        let theirs = "# written by the race winner\n";
        std::fs::write(&path, theirs).unwrap();

        create_default(&path).expect("AlreadyExists is success, not an error");

        assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);
    }

    #[test]
    fn a_missing_config_dir_names_the_remedy() {
        let err = config_path_under(None).expect_err("no config dir has no default path");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("LIBRARIAN_WORKSPACE"),
            "the error must name the way out, got: {msg}"
        );
    }

    #[test]
    fn the_default_path_lives_under_the_config_dir() {
        let base = std::path::PathBuf::from("/cfg");
        assert_eq!(
            config_path_under(Some(base)).unwrap(),
            std::path::PathBuf::from("/cfg/librarian/workspace.toml")
        );
    }

    #[test]
    fn without_init_a_missing_file_is_an_error_and_stays_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.toml");

        assert!(load_or_init(&path, false).is_err());
        assert!(!path.exists(), "init=false must create nothing");
    }

    #[test]
    fn an_unwritable_location_falls_back_to_the_in_memory_default() {
        // A regular file where a directory is needed: creation fails whatever the
        // uid, so this holds when the suite runs as root too.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        let path = blocker.join("workspace.toml");

        let cfg = load_or_init(&path, true).unwrap();

        assert_empty(&cfg);
        assert!(!path.exists());
    }

    #[test]
    fn a_malformed_existing_file_is_still_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.toml");
        std::fs::write(&path, "not = [valid").unwrap();

        assert!(load_or_init(&path, true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not = [valid");
    }
    #[cfg(unix)]
    #[test]
    fn an_unreadable_existing_file_is_an_error_not_an_empty_default() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("librarian");
        std::fs::create_dir(&sub).unwrap();
        let path = sub.join("workspace.toml");
        let mine = "[[rule]]\nglob = \"**/README.md\"\nkind = \"spec\"\n";
        std::fs::write(&path, mine).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).unwrap();

        // Root can still stat through a mode-000 directory, so the setup does not hold
        // there; restore and skip rather than assert on a world that is not the one tested.
        let setup_holds = path.try_exists().is_err();
        let result = load_or_init(&path, true);
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !setup_holds {
            return;
        }

        assert!(
            result.is_err(),
            "an existing file that cannot be read must not become an empty config"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), mine);
    }

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

    #[test]
    fn load_warns_on_legacy_roots() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"
[[roots]]
name = "x"
path = "/abs/x"
"#
        )
        .unwrap();
        let cfg = load(f.path()).unwrap();
        assert_eq!(cfg.roots.len(), 1, "still parsed");
    }

    #[test]
    fn parses_umbrella_block() {
        // Docs, error messages, and the companion hint all instruct users to
        // write `[[umbrella]]` (singular) — the same convention as `[[rule]]`.
        // Without a matching serde rename, deny_unknown_fields rejects it and
        // the whole workspace config fails to load, disabling the librarian.
        let cfg: WorkspaceConfig = toml::from_str(
            r#"
    [[umbrella]]
    name = "eco"
    members = ["/a/b", "/c/d"]
    "#,
        )
        .expect("[[umbrella]] must parse");
        assert_eq!(cfg.umbrellas.len(), 1);
        assert_eq!(cfg.umbrellas[0].name, "eco");
        assert_eq!(cfg.umbrellas[0].members.len(), 2);
    }
}
