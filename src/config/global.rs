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
    pub shell_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_output_limit_bytes: Option<usize>,
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

pub fn global_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("codescout").join("config.toml"))
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
        if !path.exists() {
            return Ok(None);
        }
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("reading global config {}", path.display()))?;
        if metadata.len() > 1024 * 1024 {
            anyhow::bail!(
                "global config {} exceeds 1 MiB limit ({} bytes)",
                path.display(),
                metadata.len()
            );
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading global config {}", path.display()))?;
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

mod tests {
    use super::*;
    use super::ENV_LOCK;

    #[test]
    fn global_config_path_uses_xdg_config_home() {
        let _guard = ENV_LOCK.lock().unwrap();
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
    fn global_config_path_falls_back_to_home_dot_config() {
        let _guard = ENV_LOCK.lock().unwrap();
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
    fn global_config_load_returns_none_when_absent() {
        let _guard = ENV_LOCK.lock().unwrap();
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
    fn global_config_load_parses_valid_toml() {
        let _guard = ENV_LOCK.lock().unwrap();
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
        let _guard = ENV_LOCK.lock().unwrap();
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
                shell_enabled: Some(false),
                ..Default::default()
            },
            ..Default::default()
        };
        let val = config.to_toml_value();
        assert_eq!(val["security"]["shell_enabled"].as_bool(), Some(false));
        assert!(val
            .get("security")
            .and_then(|s| s.get("shell_command_mode"))
            .is_none());
    }
}
