//! Per-project configuration loaded from `.codescout/project.toml`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    #[serde(default)]
    pub embeddings: EmbeddingsSection,
    #[serde(default)]
    pub ignored_paths: IgnoredPathsSection,
    #[serde(default)]
    pub security: SecuritySection,
    #[serde(default)]
    pub memory: MemorySection,
    #[serde(default)]
    pub libraries: LibrariesSection,
    #[serde(default)]
    pub lsp: LspSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default = "default_encoding")]
    pub encoding: String,
    #[serde(default = "default_timeout")]
    pub tool_timeout_secs: u64,
    /// Deprecated: use `.codescout/system-prompt.md` instead.
    /// This field is still read as a fallback if the file doesn't exist.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// Tracks which ONBOARDING_VERSION was used to generate the system prompt.
    /// `None` means pre-versioning — treated as stale.
    #[serde(default)]
    pub onboarding_version: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsSection {
    /// Model identifier — prefix determines the backend:
    ///   "ollama:<model>"                    → Ollama local daemon (default)
    ///   "openai:<model>"                    → OpenAI API (requires OPENAI_API_KEY)
    ///   "local:<EmbeddingModel variant>"    → fastembed-rs, no daemon needed,
    ///                                         CPU/WSL2-friendly. Downloads model
    ///                                         on first use to ~/.cache/huggingface/
    ///
    /// Recommended local models (rebuild with: cargo build --features local-embed):
    ///   "local:AllMiniLML6V2Q"              → 384d, INT8-quantized, ~22MB, **default**
    ///   "local:BGESmallENV15"               → 384d, full precision
    ///   "local:NomicEmbedTextV15Q"          → 768d, INT8-quantized, ~158MB
    ///   "local:JinaEmbeddingsV2BaseCode"    → 768d, code-specific, ~300MB
    #[serde(default = "default_embed_model")]
    pub model: String,
    /// Base URL for an OpenAI-compatible embedding endpoint.
    ///
    /// When set, the `model` field is sent as the model name in the request body.
    /// The URL should point to the API base (e.g., `http://127.0.0.1:43300/v1`).
    /// Works with llama.cpp, vLLM, TEI, Ollama, OpenAI, and any server implementing
    /// `POST /v1/embeddings`.
    ///
    /// When absent, the `model` field's prefix determines the backend
    /// (`local:`, `ollama:`, `openai:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// API key for the embedding endpoint. Only used when `url` is set.
    /// Can also be provided via the `EMBED_API_KEY` environment variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Override the per-chunk size in characters. Smaller chunks produce
    /// sharper semantic search results and cost less LLM context per hit
    /// (typical search returns 3–5 chunks; large chunks bloat the agent's
    /// remaining context budget across multiple searches in a session).
    ///
    /// When `None` (the default), `embed::chunk_size_for_model` is used —
    /// derived from the model's published context window. When `Some(n)`,
    /// the value is capped at the model's max so users can't accidentally
    /// exceed the embedding API's input limit.
    ///
    /// Recommended starting point for code projects: `chunk_size = 1500`
    /// (≈375–500 tokens — fits comfortably in any embedding context, keeps
    /// retrieval slices small enough that 5 hits use ≈7.5KB of LLM context).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunk_size: Option<usize>,
    /// Ignored — kept for backwards-compatible deserialisation of existing
    /// `project.toml` files that include a `chunk_overlap` key.
    ///
    /// Overlap is meaningless for AST-aware chunking (clean semantic boundaries)
    /// and was removed from the public API. The plain-text fallback paths also
    /// use 0 overlap so each sub-chunk is distinct.
    #[serde(default, skip_serializing, rename = "chunk_overlap")]
    pub _chunk_overlap_ignored: Option<usize>,
    /// Enable semantic drift detection during index builds (default: true).
    ///
    /// When enabled, `index_project` compares old and new chunk embeddings to
    /// score how much each file's *meaning* changed (not just its bytes). Results
    /// are stored in the `drift_report` table and surfaced via the `check_drift` tool.
    ///
    /// Experimental — reads all old embeddings before deletion, adding memory and
    /// DB overhead proportional to the number of changed files.
    ///
    /// Opt out in `.codescout/project.toml`:
    /// ```toml
    /// [embeddings]
    /// drift_detection_enabled = false
    /// ```
    #[serde(default = "default_drift_detection_enabled")]
    pub drift_detection_enabled: bool,
    /// Max concurrent in-flight embedding requests during `index_project`.
    ///
    /// Defaults to 8 (see `DEFAULT_MAX_INFLIGHT`). Bump this when using a
    /// remote GPU embedding backend (llama-server, TEI, vLLM) — the GPU can
    /// handle far more than 8 parallel batches, and higher inflight keeps it
    /// saturated while codescout writes the previous group to SQLite.
    ///
    /// For local CPU backends (`local:`, default), keep at 8 or lower — each
    /// embed call already uses all cores, so more inflight just queues work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_inflight: Option<usize>,
    /// Number of files processed per indexing group (embed → scatter → write).
    ///
    /// Defaults to 50 (see `DEFAULT_FILE_GROUP_SIZE`). Larger groups reduce
    /// the number of DB-write stalls per full reindex but raise peak RAM to
    /// O(group × chunks). On a remote-GPU setup, 200–500 is often better.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_group_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IgnoredPathsSection {
    #[serde(default = "default_ignored_patterns")]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySection {
    /// Security profile: "default" (sandboxed) or "root" (unrestricted).
    #[serde(default)]
    pub profile: crate::util::path_security::SecurityProfile,
    /// Additional directories where writes are allowed (beyond project root).
    #[serde(default)]
    pub extra_write_roots: Vec<String>,
    /// Shell command mode: "unrestricted", "warn" (default), "disabled"
    #[serde(default = "default_shell_mode")]
    pub shell_command_mode: String,
    /// Max bytes for shell command stdout/stderr (default 100KB)
    #[serde(default = "default_shell_output_limit")]
    pub shell_output_limit_bytes: usize,
    /// Enable shell command execution (default: true)
    #[serde(default = "default_true")]
    pub shell_enabled: bool,
    /// Enable file write tools: create_file, edit_file, symbol write tools (default: true)
    #[serde(default = "default_true")]
    pub file_write_enabled: bool,
    /// Enable semantic search and indexing tools (default: true)
    #[serde(default = "default_true")]
    pub indexing_enabled: bool,
    /// Enable additional GitHub tools: github_identity, github_issue, github_pr, github_file.
    /// github_repo is always available. (default: false)
    #[serde(default)]
    pub github_enabled: bool,
    /// Additional regex patterns to flag as dangerous commands.
    #[serde(default)]
    pub shell_dangerous_patterns: Vec<String>,
    /// Seconds to wait for the cross-process write lock before returning a
    /// RecoverableError. Default: 5.
    #[serde(default = "default_write_lock_timeout")]
    pub write_lock_timeout_secs: u64,
    /// Approximate raw source-byte threshold above which `index_project` requires
    /// user confirmation via MCP elicitation. Default: 500 MB.
    #[serde(default = "default_max_index_bytes")]
    pub max_index_bytes: u64,
}

impl Default for SecuritySection {
    fn default() -> Self {
        Self {
            profile: crate::util::path_security::SecurityProfile::Default,
            extra_write_roots: Vec::new(),
            shell_command_mode: default_shell_mode(),
            shell_output_limit_bytes: default_shell_output_limit(),
            shell_enabled: true,
            file_write_enabled: true,
            indexing_enabled: true,
            github_enabled: false,
            shell_dangerous_patterns: Vec::new(),
            write_lock_timeout_secs: 5,
            max_index_bytes: default_max_index_bytes(),
        }
    }
}

fn default_shell_mode() -> String {
    "warn".into()
}

fn default_shell_output_limit() -> usize {
    100 * 1024 // 100KB
}

fn default_true() -> bool {
    true
}

fn default_write_lock_timeout() -> u64 {
    5
}

fn default_max_index_bytes() -> u64 {
    500 * 1024 * 1024
}

impl SecuritySection {
    pub fn to_path_security_config(&self) -> crate::util::path_security::PathSecurityConfig {
        crate::util::path_security::PathSecurityConfig {
            profile: self.profile,
            extra_write_roots: self
                .extra_write_roots
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            shell_command_mode: self.shell_command_mode.clone(),
            shell_output_limit_bytes: self.shell_output_limit_bytes,
            shell_enabled: self.shell_enabled,
            file_write_enabled: self.file_write_enabled,
            indexing_enabled: self.indexing_enabled,
            github_enabled: self.github_enabled,
            library_paths: Vec::new(),
            shell_dangerous_patterns: self.shell_dangerous_patterns.clone(),
            max_index_bytes: self.max_index_bytes,
        }
    }
}

fn default_drift_detection_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySection {
    /// Min drift score to trigger reverse-drift staleness flag (0.0-1.0)
    #[serde(default = "default_staleness_drift_threshold")]
    pub staleness_drift_threshold: f32,
    /// Min similarity for semantic anchor creation (0.0-1.0)
    #[serde(default = "default_semantic_anchor_min_similarity")]
    pub semantic_anchor_min_similarity: f32,
    /// Number of top chunks to consider for semantic anchoring
    #[serde(default = "default_semantic_anchor_top_n")]
    pub semantic_anchor_top_n: usize,
    /// Memory topics protected from blind overwrite during force re-onboarding.
    /// Protected topics go through a staleness-check + merge + user-approval flow.
    #[serde(default = "default_protected_topics")]
    pub protected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrariesSection {
    #[serde(default)]
    pub auto_index: bool,
    #[serde(default)]
    pub auto_fetch_sources: bool,
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,
    #[serde(default)]
    pub version_overrides: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspSection {
    /// Per-language overrides, keyed by language name ("rust", "java", ...).
    #[serde(flatten)]
    pub langs: std::collections::HashMap<String, LspLangOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspLangOverride {
    /// Force `mux: false` (direct-process) or `mux: true` (multiplexer).
    /// `None` means "use the built-in default from servers::default_config".
    #[serde(default)]
    pub mux: Option<bool>,
}

fn default_fetch_timeout() -> u64 {
    300
}

impl Default for LibrariesSection {
    fn default() -> Self {
        Self {
            auto_index: false,
            auto_fetch_sources: false,
            fetch_timeout_secs: default_fetch_timeout(),
            version_overrides: std::collections::HashMap::new(),
        }
    }
}

fn default_staleness_drift_threshold() -> f32 {
    0.3
}
fn default_semantic_anchor_min_similarity() -> f32 {
    0.3
}
fn default_semantic_anchor_top_n() -> usize {
    10
}
fn default_protected_topics() -> Vec<String> {
    vec!["gotchas".to_string()]
}

impl Default for MemorySection {
    fn default() -> Self {
        Self {
            staleness_drift_threshold: default_staleness_drift_threshold(),
            semantic_anchor_min_similarity: default_semantic_anchor_min_similarity(),
            semantic_anchor_top_n: default_semantic_anchor_top_n(),
            protected: default_protected_topics(),
        }
    }
}

impl Default for EmbeddingsSection {
    fn default() -> Self {
        Self {
            model: default_embed_model(),
            url: None,
            api_key: None,
            chunk_size: None,
            _chunk_overlap_ignored: None,
            drift_detection_enabled: default_drift_detection_enabled(),
            max_inflight: None,
            file_group_size: None,
        }
    }
}

impl EmbeddingsSection {
    /// Resolve the chunk size in characters.
    ///
    /// - User-set `chunk_size` → `min(user_value, model_max)` (user opt-in to
    ///   larger or smaller, capped at API input limit).
    /// - Unset/zero → `min(model_max, DEFAULT_CAP)`.
    ///
    /// DEFAULT_CAP of 4096 chars keeps embed throughput reasonable on large-
    /// context models (nomic-embed, jina, bge-m3 would otherwise default to
    /// ~20k chars per chunk, which both slows indexing and dilutes ranking
    /// signal by averaging too many concepts into one vector).
    pub fn effective_chunk_size(&self) -> usize {
        const DEFAULT_CAP: usize = 4096;
        let model_max = codescout_embed::chunk_size_for_model(&self.model);
        match self.chunk_size {
            Some(n) if n > 0 => n.min(model_max),
            _ => model_max.min(DEFAULT_CAP),
        }
    }

    /// Resolve the concurrent in-flight embedding request limit for indexing.
    /// Defaults to 8. See `max_inflight` doc for tuning guidance.
    pub fn effective_max_inflight(&self) -> usize {
        const DEFAULT_MAX_INFLIGHT: usize = 8;
        self.max_inflight
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_INFLIGHT)
    }

    /// Resolve the per-group file count for indexing. Defaults to 50.
    /// See `file_group_size` doc for tuning guidance.
    pub fn effective_file_group_size(&self) -> usize {
        const DEFAULT_FILE_GROUP_SIZE: usize = 50;
        self.file_group_size
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_FILE_GROUP_SIZE)
    }
}

fn default_encoding() -> String {
    "utf-8".into()
}
fn default_timeout() -> u64 {
    60
}
fn default_embed_model() -> String {
    "local:AllMiniLML6V2Q".into()
}

fn default_ignored_patterns() -> Vec<String> {
    vec![
        ".git".into(),
        "node_modules".into(),
        "target".into(),
        "__pycache__".into(),
        ".venv".into(),
        "dist".into(),
        "build".into(),
        ".codescout".into(),
        ".worktrees".into(),
        ".claude".into(),
    ]
}

#[allow(dead_code)]
fn merge_toml(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base_map), toml::Value::Table(overlay_map)) => {
            for (k, v) in overlay_map {
                let merged = if let Some(base_val) = base_map.remove(&k) {
                    merge_toml(base_val, v)
                } else {
                    v
                };
                base_map.insert(k, merged);
            }
            toml::Value::Table(base_map)
        }
        (_, overlay) => overlay,
    }
}

impl ProjectConfig {
    /// Load from `.codescout/project.toml`, or return a sensible default
    /// derived from the directory name.  Global config (~/.config/codescout/config.toml)
    /// is loaded first as the base layer; project.toml is merged on top.
    pub fn load_or_default(root: &Path) -> Result<Self> {
        use crate::config::global::GlobalConfig;

        let global_base: toml::Value = GlobalConfig::load()?
            .map(|g| g.to_toml_value())
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));

        Self::load_with_global_base(root, global_base)
    }

    /// Inner implementation — accepts an already-resolved global base so tests can
    /// inject it directly without touching environment variables.
    fn load_with_global_base(root: &Path, global_base: toml::Value) -> Result<Self> {
        let config_path = root.join(".codescout").join("project.toml");

        let project_overlay: toml::Value = if config_path.exists() {
            let metadata = std::fs::metadata(&config_path)?;
            if metadata.len() > 1024 * 1024 {
                anyhow::bail!(
                    "project.toml exceeds 1 MiB limit ({} bytes)",
                    metadata.len()
                );
            }
            let text = std::fs::read_to_string(&config_path)?;
            toml::from_str(&text)?
        } else {
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string();
            let mut project_map = toml::map::Map::new();
            project_map.insert("name".to_string(), toml::Value::String(name));
            let mut root_map = toml::map::Map::new();
            root_map.insert("project".to_string(), toml::Value::Table(project_map));
            toml::Value::Table(root_map)
        };

        let merged = merge_toml(global_base, project_overlay);
        Ok(toml::Value::try_into(merged)?)
    }

    pub fn default_for(name: String) -> Self {
        Self {
            project: ProjectSection {
                name,
                languages: vec![],
                encoding: default_encoding(),
                tool_timeout_secs: default_timeout(),
                system_prompt: None,
                onboarding_version: None,
            },
            embeddings: EmbeddingsSection::default(),
            ignored_paths: IgnoredPathsSection::default(),
            security: SecuritySection::default(),
            memory: MemorySection::default(),
            libraries: LibrariesSection::default(),
            lsp: LspSection::default(),
        }
    }

    /// Path to the per-project data directory.
    pub fn data_dir(root: &Path) -> PathBuf {
        root.join(".codescout")
    }
}

#[test]
fn security_section_defaults_write_lock_timeout_to_5s() {
    let toml = "";
    let config: SecuritySection = toml::from_str(toml).unwrap();
    assert_eq!(config.write_lock_timeout_secs, 5);
}

#[test]
fn security_section_accepts_custom_write_lock_timeout() {
    let toml = "write_lock_timeout_secs = 10";
    let config: SecuritySection = toml::from_str(toml).unwrap();
    assert_eq!(config.write_lock_timeout_secs, 10);
}

#[test]
fn lsp_section_parses_per_language_opt_out() {
    let toml = r#"
[project]
name = "demo"

[lsp.rust]
mux = false

[lsp.python]
mux = true
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.lsp.langs.get("rust").and_then(|o| o.mux), Some(false));
    assert_eq!(cfg.lsp.langs.get("python").and_then(|o| o.mux), Some(true));
    assert!(!cfg.lsp.langs.contains_key("go"));
}

#[test]
fn lsp_section_absent_parses_to_empty_map() {
    let toml = r#"
[project]
name = "demo"
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert!(cfg.lsp.langs.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_embed_model_is_allminilm() {
        assert_eq!(default_embed_model(), "local:AllMiniLML6V2Q");
    }

    #[test]
    fn default_config_has_expected_embeddings() {
        let cfg = ProjectConfig::default_for("my-project".into());
        assert_eq!(cfg.embeddings.model, "local:AllMiniLML6V2Q");
    }

    #[test]
    fn security_section_default_enables_write_git_indexing() {
        // Previously derived Default gave false for all bool fields,
        // silently disabling write tools for projects without a [security] TOML block.
        let sec = SecuritySection::default();
        assert!(
            sec.file_write_enabled,
            "file_write_enabled should default to true"
        );
        assert!(
            sec.indexing_enabled,
            "indexing_enabled should default to true"
        );
        assert!(sec.shell_enabled, "shell_enabled should default to true");
        assert!(
            !sec.github_enabled,
            "github_enabled should default to false"
        );
    }

    #[test]
    fn project_config_default_for_enables_write_tools() {
        // default_for() is used when no .codescout/project.toml exists.
        let cfg = ProjectConfig::default_for("test-project".into());
        assert!(cfg.security.file_write_enabled);
        assert!(cfg.security.indexing_enabled);
        assert!(cfg.security.shell_enabled);
        assert!(!cfg.security.github_enabled);
    }

    #[test]
    fn toml_without_security_section_enables_write_tools() {
        // When [security] is entirely absent from TOML, serde calls Default::default()
        // for SecuritySection. This must agree with the serde field-level defaults.
        let toml = "[project]\nname = \"test\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(cfg.security.file_write_enabled);
        assert!(cfg.security.indexing_enabled);
        assert!(cfg.security.shell_enabled);
        assert!(!cfg.security.github_enabled);
    }

    #[test]
    fn system_prompt_defaults_to_none() {
        let toml = "[project]\nname = \"test\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(cfg.project.system_prompt.is_none());
    }

    #[test]
    fn system_prompt_parses_from_toml() {
        let toml = "[project]\nname = \"test\"\nsystem_prompt = \"Use pytest for testing.\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.project.system_prompt.as_deref(),
            Some("Use pytest for testing.")
        );
    }

    #[test]
    fn memory_section_defaults() {
        let toml = "[project]\nname = \"test\"";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!((config.memory.staleness_drift_threshold - 0.3).abs() < 0.01);
        assert!((config.memory.semantic_anchor_min_similarity - 0.3).abs() < 0.01);
        assert_eq!(config.memory.semantic_anchor_top_n, 10);
    }

    #[test]
    fn memory_section_override() {
        let toml = "[project]\nname = \"test\"\n[memory]\nstaleness_drift_threshold = 0.5\n";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!((config.memory.staleness_drift_threshold - 0.5).abs() < 0.01);
    }

    #[test]
    fn memory_section_default_includes_gotchas() {
        let section = MemorySection::default();
        assert_eq!(section.protected, vec!["gotchas".to_string()]);
    }

    #[test]
    fn memory_section_serde_roundtrip_with_protected() {
        let toml_str = r#"
staleness_drift_threshold = 0.3
protected = ["gotchas", "conventions"]
"#;
        let section: MemorySection = toml::from_str(toml_str).unwrap();
        assert_eq!(
            section.protected,
            vec!["gotchas".to_string(), "conventions".to_string()]
        );

        // Round-trip
        let serialized = toml::to_string_pretty(&section).unwrap();
        let deserialized: MemorySection = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.protected, section.protected);
    }

    #[test]
    fn memory_section_missing_protected_uses_default() {
        let toml_str = r#"
staleness_drift_threshold = 0.3
"#;
        let section: MemorySection = toml::from_str(toml_str).unwrap();
        assert_eq!(section.protected, vec!["gotchas".to_string()]);
    }

    #[test]
    fn project_config_deserializes_libraries_section() {
        let toml = r#"
[project]
name = "test"

[libraries]
auto_index = true
auto_fetch_sources = true
fetch_timeout_secs = 120
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(config.libraries.auto_index);
        assert!(config.libraries.auto_fetch_sources);
        assert_eq!(config.libraries.fetch_timeout_secs, 120);
    }

    #[test]
    fn project_config_libraries_defaults() {
        let toml = "[project]\nname = \"test\"\n";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(!config.libraries.auto_index);
        assert!(!config.libraries.auto_fetch_sources);
        assert_eq!(config.libraries.fetch_timeout_secs, 300);
    }

    #[test]
    fn security_profile_parses_from_toml() {
        let toml_str = "[project]\nname = \"test\"\n\n[security]\nprofile = \"root\"\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.security.profile,
            crate::util::path_security::SecurityProfile::Root
        );
    }

    #[test]
    fn security_profile_defaults_to_default() {
        let toml_str = "[project]\nname = \"test\"\n\n[security]\nshell_enabled = true\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.security.profile,
            crate::util::path_security::SecurityProfile::Default
        );
    }

    #[test]
    fn project_section_deserializes_onboarding_version() {
        let toml_with = r#"
            name = "test"
            languages = ["rust"]
            onboarding_version = 2
        "#;
        let section: ProjectSection = toml::from_str(toml_with).unwrap();
        assert_eq!(section.onboarding_version, Some(2));
    }

    #[test]
    fn project_section_deserializes_without_onboarding_version() {
        let toml_without = r#"
            name = "test"
            languages = ["rust"]
        "#;
        let section: ProjectSection = toml::from_str(toml_without).unwrap();
        assert_eq!(section.onboarding_version, None);
    }

    #[test]
    fn embeddings_section_parses_url_and_api_key() {
        let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
api_key = "test-key-123"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.embeddings.url.as_deref(),
            Some("http://127.0.0.1:43300/v1")
        );
        assert_eq!(config.embeddings.api_key.as_deref(), Some("test-key-123"));
    }

    #[test]
    fn embeddings_section_url_defaults_to_none() {
        let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "ollama:nomic-embed-text"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert!(config.embeddings.url.is_none());
        assert!(config.embeddings.api_key.is_none());
    }

    #[test]
    fn security_section_default_max_index_bytes_is_500mb() {
        let sec = SecuritySection::default();
        assert_eq!(sec.max_index_bytes, 500 * 1024 * 1024);
    }

    #[test]
    fn project_config_default_propagates_max_index_bytes() {
        let cfg = ProjectConfig::default_for("test-project".into());
        assert_eq!(cfg.security.max_index_bytes, 500 * 1024 * 1024);
    }
    /// Default chunk_size is None → effective uses model max.
    #[test]
    fn effective_chunk_size_none_uses_model_max() {
        let sec = EmbeddingsSection {
            model: "ollama:nomic-embed-text".into(),
            ..Default::default()
        };
        // nomic-embed model max is ~20k chars; default caps at 4096.
        assert_eq!(sec.effective_chunk_size(), 4096);
    }

    /// Explicit chunk_size below model max is honored verbatim — fixes the
    /// silent-ignore behavior where users could not opt into smaller chunks
    /// for tighter LLM context budgets during semantic search.
    #[test]
    fn effective_chunk_size_user_value_below_cap_honored() {
        let sec = EmbeddingsSection {
            model: "ollama:nomic-embed-text".into(),
            chunk_size: Some(1500),
            ..Default::default()
        };
        assert_eq!(sec.effective_chunk_size(), 1500);
    }

    /// Explicit chunk_size above model max is capped — protects against
    /// API-side truncation when a user misconfigures.
    #[test]
    fn effective_chunk_size_user_value_above_cap_clamped() {
        let model_max = codescout_embed::chunk_size_for_model("local:AllMiniLML6V2Q");
        let sec = EmbeddingsSection {
            model: "local:AllMiniLML6V2Q".into(),
            chunk_size: Some(model_max * 10),
            ..Default::default()
        };
        assert_eq!(sec.effective_chunk_size(), model_max);
    }

    /// chunk_size = Some(0) is treated as unset (model max), not as a
    /// degenerate zero chunk size.
    #[test]
    fn effective_chunk_size_zero_falls_back_to_model_max() {
        let sec = EmbeddingsSection {
            model: "ollama:nomic-embed-text".into(),
            chunk_size: Some(0),
            ..Default::default()
        };
        // Some(0) falls back to default path, which caps at 4096.
        assert_eq!(sec.effective_chunk_size(), 4096);
    }

    #[test]
    fn effective_max_inflight_defaults_to_8() {
        let sec = EmbeddingsSection::default();
        assert_eq!(sec.effective_max_inflight(), 8);
    }

    #[test]
    fn effective_max_inflight_user_value_honored() {
        let sec = EmbeddingsSection {
            max_inflight: Some(32),
            ..Default::default()
        };
        assert_eq!(sec.effective_max_inflight(), 32);
    }

    #[test]
    fn effective_max_inflight_zero_falls_back_to_default() {
        let sec = EmbeddingsSection {
            max_inflight: Some(0),
            ..Default::default()
        };
        assert_eq!(sec.effective_max_inflight(), 8);
    }

    #[test]
    fn effective_file_group_size_defaults_to_50() {
        let sec = EmbeddingsSection::default();
        assert_eq!(sec.effective_file_group_size(), 50);
    }

    #[test]
    fn effective_file_group_size_user_value_honored() {
        let sec = EmbeddingsSection {
            file_group_size: Some(200),
            ..Default::default()
        };
        assert_eq!(sec.effective_file_group_size(), 200);
    }

    #[test]
    fn embeddings_section_parses_inflight_and_group_size() {
        let toml = r#"
            [embeddings]
            model = "local:AllMiniLML6V2Q"
            max_inflight = 16
            file_group_size = 100
        "#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            embeddings: EmbeddingsSection,
        }
        let w: Wrap = toml::from_str(toml).unwrap();
        assert_eq!(w.embeddings.max_inflight, Some(16));
        assert_eq!(w.embeddings.file_group_size, Some(100));
    }

    /// Project config TOML round-trip: explicit chunk_size deserializes,
    /// missing key produces None.
    #[test]
    fn project_config_chunk_size_round_trip() {
        let toml_with = r#"
[project]
name = "test"
[embeddings]
model = "local:AllMiniLML6V2Q"
chunk_size = 1500
"#;
        let cfg: ProjectConfig = toml::from_str(toml_with).unwrap();
        assert_eq!(cfg.embeddings.chunk_size, Some(1500));

        let toml_without = r#"
[project]
name = "test"
[embeddings]
model = "local:AllMiniLML6V2Q"
"#;
        let cfg: ProjectConfig = toml::from_str(toml_without).unwrap();
        assert_eq!(cfg.embeddings.chunk_size, None);
    }

    fn load_or_default_applies_global_when_project_absent() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        let global_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join("config.toml"),
            "[embeddings]\nmodel = \"local:BGESmallENV15\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");

        let project_dir = dir.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(cfg.embeddings.model, "local:BGESmallENV15");
        assert_eq!(cfg.project.name, "my-project");
    }

    fn load_or_default_project_wins_over_global() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        let global_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join("config.toml"),
            "[embeddings]\nmodel = \"global-model\"\n",
        )
        .unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");

        let project_dir = dir.path().join("proj");
        let codescout_dir = project_dir.join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
        )
        .unwrap();
        let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(cfg.embeddings.model, "project-model");
    }

    fn load_or_default_global_fills_gap_in_project() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        let global_dir = dir.path().join(".config").join("codescout");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join("config.toml"),
            "[security]\nshell_enabled = false\n",
        )
        .unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");

        let project_dir = dir.path().join("proj");
        let codescout_dir = project_dir.join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
        )
        .unwrap();
        let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert!(!cfg.security.shell_enabled);
        assert_eq!(cfg.embeddings.model, "project-model");
    }

    fn load_or_default_no_global_behaves_as_before() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", dir.path());
        std::env::remove_var("XDG_CONFIG_HOME");

        let project_dir = dir.path().join("proj");
        let codescout_dir = project_dir.join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n",
        )
        .unwrap();
        let cfg = ProjectConfig::load_or_default(&project_dir).unwrap();
        match saved_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match saved_xdg {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        assert_eq!(cfg.embeddings.model, default_embed_model());
        assert!(cfg.security.shell_enabled);
    }

    #[test]
    fn merge_toml_overlay_wins_scalar() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "base-model" });
        let overlay = toml::Value::Table(toml::toml! { [embeddings] model = "project-model" });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(
            merged["embeddings"]["model"].as_str(),
            Some("project-model")
        );
    }

    #[test]
    fn merge_toml_base_fills_missing_key() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "global-model" });
        let overlay =
            toml::Value::Table(toml::toml! { [embeddings] drift_detection_enabled = false });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged["embeddings"]["model"].as_str(), Some("global-model"));
        assert_eq!(
            merged["embeddings"]["drift_detection_enabled"].as_bool(),
            Some(false)
        );
    }

    #[test]
    fn merge_toml_nested_tables_merge_recursively() {
        let base = toml::Value::Table(toml::toml! {
            [security]
            shell_enabled = false
            shell_command_mode = "warn"
        });
        let overlay = toml::Value::Table(toml::toml! {
            [security]
            shell_command_mode = "unrestricted"
        });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged["security"]["shell_enabled"].as_bool(), Some(false));
        assert_eq!(
            merged["security"]["shell_command_mode"].as_str(),
            Some("unrestricted")
        );
    }

    #[test]
    fn merge_toml_non_table_overlay_replaces_base() {
        let base = toml::Value::String("base".into());
        let overlay = toml::Value::String("overlay".into());
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged.as_str(), Some("overlay"));
    }

    #[test]
    fn merge_toml_empty_overlay_returns_base() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "base-model" });
        let overlay = toml::Value::Table(toml::map::Map::new());
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged["embeddings"]["model"].as_str(), Some("base-model"));
    }
}
