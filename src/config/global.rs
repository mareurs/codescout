use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub embeddings: GlobalEmbeddingsSection,
    #[serde(default)]
    pub security: GlobalSecuritySection,
    #[serde(default)]
    pub ignored_paths: GlobalIgnoredPathsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalEmbeddingsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSecuritySection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_dangerous_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_write_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_index_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalIgnoredPathsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<String>>,
}

/// Resolve the global config dir from explicit `XDG_CONFIG_HOME` / `HOME` values.
///
/// Pure — takes the values instead of reading them. This is the test seam that lets
/// the config tests stop calling `std::env::set_var`: mutating process env while
/// other test threads call `getenv` is UB (glibc may `realloc` `environ` under a
/// concurrent reader), and *every* test that builds an `Agent` reads `HOME` through
/// this function. See
/// `docs/issues/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
pub(crate) fn global_config_dir_from(
    xdg_config_home: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let base = xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("codescout"))
}

/// `$XDG_CONFIG_HOME/codescout` or `$HOME/.config/codescout`. `None` when neither
/// `XDG_CONFIG_HOME` nor `HOME` is set (e.g. some Windows/CI environments).
fn global_config_dir() -> Option<PathBuf> {
    global_config_dir_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

pub fn global_config_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join("config.toml"))
}

/// Default startup-dotenv path: `<global_config_dir>/.env`.
pub fn global_env_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join(".env"))
}

/// What [`load_startup_env`] should do, decided without touching the environment.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StartupEnvPlan {
    /// Load this dotenv file.
    Load(PathBuf),
    /// `$CODESCOUT_ENV_FILE` names a file that does not exist — warn (the user asked
    /// for it explicitly, so silence would hide a typo).
    WarnMissingExplicit(PathBuf),
    /// Nothing to do: no explicit override, and no default path or it does not exist.
    /// A missing *default* is opt-in, so it is silent.
    Nothing,
}

/// Pure: decide the startup-dotenv plan. Split out of [`load_startup_env`] so the
/// decision is testable without `set_var` — see [`global_config_dir_from`].
pub(crate) fn plan_startup_env(
    explicit: Option<PathBuf>,
    default: Option<PathBuf>,
    exists: impl Fn(&std::path::Path) -> bool,
) -> StartupEnvPlan {
    match explicit {
        Some(p) if exists(&p) => StartupEnvPlan::Load(p),
        Some(p) => StartupEnvPlan::WarnMissingExplicit(p),
        None => match default {
            Some(p) if exists(&p) => StartupEnvPlan::Load(p),
            _ => StartupEnvPlan::Nothing,
        },
    }
}

/// Pure: apply dotenv precedence. A variable already present in the environment WINS
/// over the file, so only currently-unset keys are assigned.
pub(crate) fn startup_env_assignments(
    pairs: impl IntoIterator<Item = (String, String)>,
    is_set: impl Fn(&str) -> bool,
) -> Vec<(String, String)> {
    pairs.into_iter().filter(|(key, _)| !is_set(key)).collect()
}

/// Load a startup dotenv into the process environment before config resolution.
///
/// Path: `$CODESCOUT_ENV_FILE` if set, else [`global_env_path`]. An explicit process
/// env var always wins over the file. A missing default path is a silent no-op
/// (opt-in); a missing *explicit* `$CODESCOUT_ENV_FILE` is a surfaced warning. Never
/// reads the current working directory — a user-scoped server must not absorb an
/// arbitrary repo's `.env`.
///
/// This is the one place that still mutates the environment, and it is sound: it runs
/// once at process startup, from `main`, before any worker threads exist. The decision
/// logic lives in [`plan_startup_env`] / [`startup_env_assignments`] so it can be
/// tested without a `set_var` anywhere near a parallel test runner.
pub fn load_startup_env() {
    let plan = plan_startup_env(
        std::env::var_os("CODESCOUT_ENV_FILE").map(PathBuf::from),
        global_env_path(),
        |p| p.exists(),
    );
    let path = match plan {
        StartupEnvPlan::Load(p) => p,
        StartupEnvPlan::WarnMissingExplicit(p) => {
            tracing::warn!(
                "CODESCOUT_ENV_FILE set to {} but the file was not found",
                p.display()
            );
            return;
        }
        StartupEnvPlan::Nothing => return,
    };

    let pairs = match dotenvy::from_path_iter(&path) {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect::<Vec<_>>(),
        Err(e) => {
            tracing::warn!("failed to load startup env from {}: {e}", path.display());
            return;
        }
    };
    for (key, value) in startup_env_assignments(pairs, |k| std::env::var_os(k).is_some()) {
        std::env::set_var(key, value);
    }
    tracing::debug!("loaded startup env from {}", path.display());
}

impl GlobalConfig {
    /// Load `<config_dir>/config.toml`. `Ok(None)` when the file is absent.
    ///
    /// Takes the directory explicitly so tests can point it at a tempdir instead of
    /// mutating `HOME`/`XDG_CONFIG_HOME` (see [`global_config_dir_from`]).
    pub fn load_from_dir(config_dir: &std::path::Path) -> Result<Option<Self>> {
        let path = config_dir.join("config.toml");
        // Race-tolerant: under parallel test runs another tempdir may delete
        // the path between an exists()-check and a subsequent read. Always
        // attempt the I/O and treat ENOENT as Ok(None) at each step.
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e).with_context(|| format!("reading global config {}", path.display()));
            }
        };
        if metadata.len() > 1024 * 1024 {
            anyhow::bail!(
                "global config {} exceeds 1 MiB limit ({} bytes)",
                path.display(),
                metadata.len()
            );
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e).with_context(|| format!("reading global config {}", path.display()));
            }
        };
        let config: GlobalConfig = toml::from_str(&text)
            .with_context(|| format!("parsing global config {}", path.display()))?;
        Ok(Some(config))
    }

    pub fn load() -> Result<Option<Self>> {
        let dir = match global_config_dir() {
            Some(d) => d,
            None => {
                tracing::debug!("$HOME not set, skipping global config");
                return Ok(None);
            }
        };
        Self::load_from_dir(&dir)
    }

    pub fn to_toml_value(&self) -> toml::Value {
        toml::Value::try_from(self).expect("GlobalConfig is always serializable")
    }
}

// ENV_LOCK / lock_env_for_tests used to live here: a process-wide mutex that
// env-mutating tests took around their set_var/remove_var pairs. Both are GONE.
//
// The lock was never sound. It was disjoint from serial_test's lock, and — more
// fundamentally — it could only be held by tests that knew to take it, while the env
// READERS are every test that builds an Agent (Agent::new -> ProjectConfig::load_or_default
// -> GlobalConfig::load -> global_config_dir -> getenv("HOME")). A writer holding a lock
// no reader takes protects nothing: glibc's setenv can realloc `environ` under a
// concurrent getenv regardless.
//
// The fix was to delete the writes, not coordinate them. Config resolution now takes its
// inputs explicitly (global_config_dir_from, GlobalConfig::load_from_dir,
// ProjectConfig::load_with_global_base, ProjectConfig::apply_embed_overrides), so tests
// inject instead of mutating. See
// docs/issues/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    // NOTE: not a single `set_var` / `remove_var` in this module, and no `#[serial]`.
    // Mutating process env while other test threads call `getenv` is UB — glibc may
    // `realloc` `environ` under a concurrent reader — and EVERY test that builds an
    // `Agent` reads HOME through `global_config_dir`. Serializing a subset cannot fix
    // that (serial_test has no lock that unannotated tests participate in), so the
    // writes are GONE rather than coordinated. See
    // docs/issues/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md

    #[test]
    fn config_dir_prefers_xdg_config_home() {
        let dir =
            global_config_dir_from(Some(OsStr::new("/tmp/xdg-test-codescout")), None).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/xdg-test-codescout/codescout"));
    }

    #[test]
    fn config_dir_falls_back_to_home_dot_config() {
        let dir = global_config_dir_from(None, Some(OsStr::new("/tmp/fake-home"))).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/fake-home/.config/codescout"));
    }

    #[test]
    fn config_dir_xdg_wins_over_home() {
        let dir = global_config_dir_from(
            Some(OsStr::new("/tmp/xdg")),
            Some(OsStr::new("/tmp/fake-home")),
        )
        .unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/xdg/codescout"));
    }

    #[test]
    fn config_dir_none_when_neither_set() {
        assert!(global_config_dir_from(None, None).is_none());
    }

    #[test]
    fn env_path_derives_from_config_dir() {
        let dir =
            global_config_dir_from(Some(OsStr::new("/tmp/xdg-test-codescout")), None).unwrap();
        assert_eq!(
            dir.join(".env"),
            PathBuf::from("/tmp/xdg-test-codescout/codescout/.env")
        );
    }

    #[test]
    fn load_returns_none_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(GlobalConfig::load_from_dir(dir.path()).unwrap().is_none());
    }

    #[test]
    fn load_parses_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "[embeddings]\nmodel = \"local:BGESmallENV15\"\n",
        )
        .unwrap();
        let cfg = GlobalConfig::load_from_dir(dir.path()).unwrap().unwrap();
        assert_eq!(cfg.embeddings.model.as_deref(), Some("local:BGESmallENV15"));
    }

    #[test]
    fn load_errors_on_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "this is not = valid = toml").unwrap();
        let err = GlobalConfig::load_from_dir(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("config.toml"),
            "error should mention file path: {msg}"
        );
    }

    #[test]
    fn to_toml_value_emits_only_some_fields() {
        // Two sections, so the assertion still discriminates after
        // `drift_detection_enabled` was retired and left `[embeddings]` with a
        // single field: a `None` needs to exist somewhere for "only Some fields
        // are emitted" to mean anything.
        let config = GlobalConfig {
            embeddings: GlobalEmbeddingsSection {
                model: Some("local:BGESmallENV15".to_string()),
            },
            security: GlobalSecuritySection {
                shell_command_mode: Some("safe".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert!(val["embeddings"]["model"].as_str().is_some());
        assert!(val["security"]["shell_command_mode"].as_str().is_some());
        // A `None` field must be omitted entirely, not emitted as a null.
        assert!(val
            .get("security")
            .and_then(|s| s.get("file_write_enabled"))
            .is_none());
    }

    #[test]
    fn to_toml_value_security_emits_set_fields() {
        let config = GlobalConfig {
            security: GlobalSecuritySection {
                file_write_enabled: Some(false),
                ..Default::default()
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert_eq!(val["security"]["file_write_enabled"].as_bool(), Some(false));
        assert!(val
            .get("security")
            .and_then(|s| s.get("shell_command_mode"))
            .is_none());
    }

    // --- startup dotenv: the decision logic, with no process env in sight ---

    #[test]
    fn plan_loads_explicit_file_when_it_exists() {
        let plan = plan_startup_env(Some(PathBuf::from("/x/custom.env")), None, |_| true);
        assert_eq!(plan, StartupEnvPlan::Load(PathBuf::from("/x/custom.env")));
    }

    #[test]
    fn plan_warns_when_explicit_file_is_missing() {
        // The user named the file, so silence would hide a typo.
        let plan = plan_startup_env(Some(PathBuf::from("/nope/.env")), None, |_| false);
        assert_eq!(
            plan,
            StartupEnvPlan::WarnMissingExplicit(PathBuf::from("/nope/.env"))
        );
    }

    #[test]
    fn plan_explicit_wins_over_default() {
        let plan = plan_startup_env(
            Some(PathBuf::from("/x/custom.env")),
            Some(PathBuf::from("/x/default.env")),
            |_| true,
        );
        assert_eq!(plan, StartupEnvPlan::Load(PathBuf::from("/x/custom.env")));
    }

    #[test]
    fn plan_is_silent_noop_when_default_is_absent() {
        // A missing DEFAULT dotenv is opt-in, not an error — no warning.
        let plan = plan_startup_env(None, Some(PathBuf::from("/x/default.env")), |_| false);
        assert_eq!(plan, StartupEnvPlan::Nothing);
    }

    #[test]
    fn plan_is_noop_when_there_is_no_path_at_all() {
        // e.g. neither HOME nor XDG_CONFIG_HOME set.
        let plan = plan_startup_env(None, None, |_| true);
        assert_eq!(plan, StartupEnvPlan::Nothing);
    }

    #[test]
    fn real_env_wins_over_the_dotenv_file() {
        // dotenv precedence: an already-set variable is NOT overridden by the file.
        let pairs = vec![
            ("ALREADY_SET".to_string(), "from_file".to_string()),
            ("NOT_SET".to_string(), "from_file".to_string()),
        ];
        let applied = startup_env_assignments(pairs, |k| k == "ALREADY_SET");
        assert_eq!(
            applied,
            vec![("NOT_SET".to_string(), "from_file".to_string())],
            "only the unset key may be assigned"
        );
    }

    #[test]
    fn all_keys_apply_when_none_are_set() {
        let pairs = vec![("A".to_string(), "1".to_string())];
        let applied = startup_env_assignments(pairs, |_| false);
        assert_eq!(applied, vec![("A".to_string(), "1".to_string())]);
    }
}
