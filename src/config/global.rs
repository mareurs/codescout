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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drift_detection_enabled: Option<bool>,
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

/// `$XDG_CONFIG_HOME/codescout` or `$HOME/.config/codescout`. `None` when neither
/// `XDG_CONFIG_HOME` nor `HOME` is set (e.g. some Windows/CI environments).
fn global_config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("codescout"))
}

pub fn global_config_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join("config.toml"))
}

/// Default startup-dotenv path: `<global_config_dir>/.env`.
pub fn global_env_path() -> Option<PathBuf> {
    Some(global_config_dir()?.join(".env"))
}

/// Load a startup dotenv into the process environment before config resolution.
///
/// Path: `$CODESCOUT_ENV_FILE` if set, else [`global_env_path`]. Uses
/// `dotenvy::from_path`, which does NOT override already-set vars — so an
/// explicit process env var always wins over the file. A missing default path is
/// a silent no-op (opt-in); a missing *explicit* `$CODESCOUT_ENV_FILE` is a
/// surfaced warning. Never reads the current working directory — a user-scoped
/// server must not absorb an arbitrary repo's `.env`.
pub fn load_startup_env() {
    let explicit = std::env::var_os("CODESCOUT_ENV_FILE").map(PathBuf::from);
    let path = match explicit.clone().or_else(global_env_path) {
        Some(p) => p,
        None => return,
    };
    if !path.exists() {
        if explicit.is_some() {
            tracing::warn!(
                "CODESCOUT_ENV_FILE set to {} but the file was not found",
                path.display()
            );
        }
        return;
    }
    match dotenvy::from_path(&path) {
        Ok(()) => tracing::debug!("loaded startup env from {}", path.display()),
        Err(e) => tracing::warn!("failed to load startup env from {}: {e}", path.display()),
    }
}

impl GlobalConfig {
    pub fn load() -> Result<Option<Self>> {
        let path = match global_config_path() {
            Some(p) => p,
            None => {
                tracing::debug!("$HOME not set, skipping global config");
                return Ok(None);
            }
        };
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

    pub fn to_toml_value(&self) -> toml::Value {
        toml::Value::try_from(self).expect("GlobalConfig is always serializable")
    }
}

// Process-wide lock for tests that read or write HOME / XDG_CONFIG_HOME.
// Declared at module level so preflight and other modules can import it.
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire `ENV_LOCK`, ignoring poison.
///
/// The lock only guards env-var setup/teardown sequencing in tests — a
/// poisoned mutex from a panicking test does not corrupt the env-var
/// state itself, since each test sets the vars it needs at the top.
/// Without this helper, a single test panic cascades into "all tests
/// fail with PoisonError" on subsequent runs in the same process.
#[allow(dead_code)] // used by #[cfg(test)] modules across crates
pub(crate) fn lock_env_for_tests() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::lock_env_for_tests;
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn global_config_path_uses_xdg_config_home() {
        let _guard = lock_env_for_tests();
        let saved = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test-codescout");
        let path = global_config_path().unwrap();
        match saved {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/xdg-test-codescout/codescout/config.toml")
        );
    }

    #[test]
    #[serial]
    fn global_config_path_falls_back_to_home_dot_config() {
        let _guard = lock_env_for_tests();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/tmp/fake-home");
        let path = global_config_path().unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/fake-home/.config/codescout/config.toml")
        );
    }

    #[test]
    #[serial]
    fn global_config_load_returns_none_when_absent() {
        let _guard = lock_env_for_tests();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn global_config_load_parses_valid_toml() {
        let _guard = lock_env_for_tests();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.toml"),
            "[embeddings]\nmodel = \"local:BGESmallENV15\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load().unwrap().unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(
            result.embeddings.model,
            Some("local:BGESmallENV15".to_string())
        );
    }

    #[allow(dead_code)] // stale test — missing #[test] attribute, kept for future re-enable
    fn global_config_load_errors_on_malformed_toml() {
        let _guard = lock_env_for_tests();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.toml"),
            "embeddings = {model = [unclosed",
        )
        .unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");
        let result = GlobalConfig::load();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("config.toml"),
            "error should mention file path: {msg}"
        );
    }

    #[test]
    fn to_toml_value_emits_only_some_fields() {
        let config = GlobalConfig {
            embeddings: GlobalEmbeddingsSection {
                model: Some("local:BGESmallENV15".to_string()),
                drift_detection_enabled: None,
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert!(val["embeddings"]["model"].as_str().is_some());
        assert!(val
            .get("embeddings")
            .and_then(|e| e.get("drift_detection_enabled"))
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

    #[test]
    #[serial]
    fn global_env_path_derives_from_config_dir() {
        let _guard = lock_env_for_tests();
        let saved = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-test-codescout");
        let path = global_env_path().unwrap();
        match saved {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/xdg-test-codescout/codescout/.env")
        );
    }

    #[test]
    #[serial]
    fn load_startup_env_reads_explicit_file() {
        let _guard = lock_env_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("custom.env");
        std::fs::write(&file, "CODESCOUT_TEST_LOADER_VAR=from_file\n").unwrap();

        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::remove_var("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_ENV_FILE", &file);

        load_startup_env();
        let got = std::env::var("CODESCOUT_TEST_LOADER_VAR").ok();

        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert_eq!(got.as_deref(), Some("from_file"));
    }

    #[test]
    #[serial]
    fn load_startup_env_does_not_override_existing() {
        let _guard = lock_env_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("custom.env");
        std::fs::write(&file, "CODESCOUT_TEST_LOADER_VAR=from_file\n").unwrap();

        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_TEST_LOADER_VAR", "from_env");
        std::env::set_var("CODESCOUT_ENV_FILE", &file);

        load_startup_env();
        let got = std::env::var("CODESCOUT_TEST_LOADER_VAR").ok();

        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert_eq!(
            got.as_deref(),
            Some("from_env"),
            "real env must win over the file"
        );
    }

    #[test]
    #[serial]
    fn load_startup_env_noop_when_explicit_file_missing() {
        let _guard = lock_env_for_tests();
        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        std::env::remove_var("CODESCOUT_TEST_LOADER_VAR");
        std::env::set_var("CODESCOUT_ENV_FILE", "/nonexistent/codescout-test-xyz/.env");

        load_startup_env(); // must not panic; must not set anything

        let got = std::env::var_os("CODESCOUT_TEST_LOADER_VAR");
        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_LOADER_VAR", v),
            None => std::env::remove_var("CODESCOUT_TEST_LOADER_VAR"),
        }
        assert!(got.is_none());
    }

    #[test]
    #[serial]
    fn load_startup_env_noop_when_default_path_absent() {
        let _guard = lock_env_for_tests();
        let dir = tempfile::tempdir().unwrap();

        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let saved_home = std::env::var_os("HOME");
        let saved_file = std::env::var_os("CODESCOUT_ENV_FILE");
        let saved_var = std::env::var_os("CODESCOUT_TEST_NOOP_SENTINEL");

        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        std::env::remove_var("CODESCOUT_ENV_FILE");
        std::env::remove_var("CODESCOUT_TEST_NOOP_SENTINEL");

        load_startup_env(); // must not panic; default path does not exist

        let got = std::env::var_os("CODESCOUT_TEST_NOOP_SENTINEL");
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_file {
            Some(v) => std::env::set_var("CODESCOUT_ENV_FILE", v),
            None => std::env::remove_var("CODESCOUT_ENV_FILE"),
        }
        match saved_var {
            Some(v) => std::env::set_var("CODESCOUT_TEST_NOOP_SENTINEL", v),
            None => std::env::remove_var("CODESCOUT_TEST_NOOP_SENTINEL"),
        }
        assert!(got.is_none());
    }
}
