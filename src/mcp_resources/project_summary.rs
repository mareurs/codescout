use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use async_trait::async_trait;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummarySnapshot {
    pub active_project: Option<String>,
    pub index_status: String,
    pub language: Option<String>,
    pub lsp_ready: bool,
}

#[async_trait]
pub trait SummarySource: Send + Sync {
    async fn snapshot(&self) -> SummarySnapshot;
}

pub struct ProjectSummaryProvider<S: SummarySource> {
    source: S,
}

impl<S: SummarySource> ProjectSummaryProvider<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }
}

const URI: &str = "project://summary";

#[async_trait]
impl<S: SummarySource + 'static> ResourceProvider for ProjectSummaryProvider<S> {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![ResourceDescriptor {
            uri: URI.into(),
            name: "project-summary".into(),
            description: Some("Active project, index freshness, language, LSP readiness.".into()),
            mime_type: "application/json".into(),
        }]
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        if uri != URI {
            return Err(ResourceError::NotFound(uri.into()));
        }
        let snap = self.source.snapshot().await;
        let text = serde_json::to_string_pretty(&snap)
            .map_err(|e| ResourceError::Other(anyhow::Error::from(e)))?;
        Ok(ResourceBytes::Text(text))
    }
}

/// Detect the primary language of a project using manifest-file heuristics.
///
/// Priority:
/// 1. A canonical manifest at the project root (Cargo.toml → rust,
///    package.json → typescript/javascript, pyproject.toml/setup.py → python,
///    go.mod → go, pom.xml → java, build.gradle.kts/build.gradle → kotlin).
///    The manifest's language is accepted only when it also appears in the
///    `configured` list — that keeps us honest to the user's own configuration.
/// 2. Fall back to `configured.first()` if no manifest matches.
///
/// For `package.json`: returns `"typescript"` when a `tsconfig.json` also exists,
/// otherwise returns `"javascript"`.
fn detect_primary_language(
    project_root: &std::path::Path,
    configured: &[String],
) -> Option<String> {
    // Prefer the dominant language by file count (skips markdown); the build
    // manifest and configured list are only fallbacks when the directory has no
    // source files. (2026-06-03-project-languages-from-manifest-not-files)
    if let Some(dom) = crate::workspace::dominant_language(project_root) {
        return Some(dom);
    }

    let configured_contains = |lang: &str| configured.iter().any(|l| l.eq_ignore_ascii_case(lang));

    // package.json: typescript when tsconfig.json is also present, else javascript.
    if project_root.join("package.json").exists() {
        let lang = if project_root.join("tsconfig.json").exists() {
            "typescript"
        } else {
            "javascript"
        };
        if configured_contains(lang) {
            return Some(lang.to_string());
        }
    }

    const MANIFESTS: &[(&str, &str)] = &[
        ("Cargo.toml", "rust"),
        ("pyproject.toml", "python"),
        ("setup.py", "python"),
        ("go.mod", "go"),
        ("pom.xml", "java"),
        ("build.gradle.kts", "kotlin"),
        ("build.gradle", "kotlin"),
    ];

    for (manifest, lang) in MANIFESTS {
        if project_root.join(manifest).exists() && configured_contains(lang) {
            return Some((*lang).to_string());
        }
    }

    configured.first().cloned().filter(|s| !s.is_empty())
}

/// Adapter that fills [`SummarySnapshot`] from live [`crate::agent::Agent`] state.
///
/// All probes are best-effort — any field that can't be determined falls back to
/// `None` / `"unknown"` / `false` rather than propagating an error.
pub struct AgentSummarySource {
    agent: crate::agent::Agent,
    lsp: std::sync::Arc<dyn crate::lsp::ops::LspProvider>,
}

impl AgentSummarySource {
    pub fn new(
        agent: crate::agent::Agent,
        lsp: std::sync::Arc<dyn crate::lsp::ops::LspProvider>,
    ) -> Self {
        Self { agent, lsp }
    }
}

#[async_trait]
impl SummarySource for AgentSummarySource {
    async fn snapshot(&self) -> SummarySnapshot {
        let root = self.agent.project_root().await;

        let active_project = root.as_ref().map(|p| p.display().to_string());

        let configured: Vec<String> = self
            .agent
            .with_project(|p| Ok(p.config.project.languages.clone()))
            .await
            .unwrap_or_default();

        let language = root
            .as_deref()
            .and_then(|r| detect_primary_language(r, &configured))
            .or_else(|| configured.first().cloned().filter(|s| !s.is_empty()));

        let index_status = self.agent.index_status_label();

        // Probe configured languages; fall back to the detected language when
        // configured list is empty (e.g. no project.toml yet).
        let probe_langs: Vec<&str> = if configured.is_empty() {
            language.as_deref().into_iter().collect()
        } else {
            configured.iter().map(String::as_str).collect()
        };

        let lsp_ready = if let Some(ref r) = root {
            let mut any = false;
            for lang in &probe_langs {
                if self.lsp.is_ready(lang, r).await {
                    any = true;
                    break;
                }
            }
            any
        } else {
            false
        };

        SummarySnapshot {
            active_project,
            index_status,
            language,
            lsp_ready,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct StubSource;
    #[async_trait]
    impl SummarySource for StubSource {
        async fn snapshot(&self) -> SummarySnapshot {
            SummarySnapshot {
                active_project: Some("/tmp/proj".into()),
                index_status: "fresh".into(),
                language: Some("rust".into()),
                lsp_ready: true,
            }
        }
    }

    #[tokio::test]
    async fn summary_returns_json_with_required_keys() {
        let p = ProjectSummaryProvider::new(StubSource);
        let bytes = p.read("project://summary").await.unwrap();
        let json: serde_json::Value = match bytes {
            ResourceBytes::Text(s) => serde_json::from_str(&s).unwrap(),
            _ => panic!("expected text"),
        };
        for k in ["active_project", "index_status", "language", "lsp_ready"] {
            assert!(json.get(k).is_some(), "missing {}", k);
        }
    }

    #[tokio::test]
    async fn summary_rejects_wrong_uri() {
        let p = ProjectSummaryProvider::new(StubSource);
        let err = p.read("project://other").await.unwrap_err();
        assert!(matches!(err, ResourceError::NotFound(_)));
    }

    // ── detect_primary_language unit tests ────────────────────────────────────

    #[test]
    fn detect_language_cargo_in_configured() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let lang = detect_primary_language(dir.path(), &["rust".into(), "bash".into()]);
        assert_eq!(lang.as_deref(), Some("rust"));
    }

    #[test]
    fn detect_language_cargo_not_in_configured_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        // "rust" not configured → falls back to first configured language
        let lang = detect_primary_language(dir.path(), &["bash".into()]);
        assert_eq!(lang.as_deref(), Some("bash"));
    }

    #[test]
    fn detect_language_no_manifest_falls_back_to_configured_first() {
        let dir = tempfile::tempdir().unwrap();
        let lang = detect_primary_language(dir.path(), &["bash".into()]);
        assert_eq!(lang.as_deref(), Some("bash"));
    }

    #[test]
    fn detect_language_empty_configured_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let lang = detect_primary_language(dir.path(), &[]);
        assert!(lang.is_none());
    }

    // ── lsp_ready probe tests using stub LspProvider ─────────────────────────

    /// A stub LspProvider that reports `is_ready = true` for any language.
    struct ReadyLspProvider;

    #[async_trait]
    impl crate::lsp::ops::LspProvider for ReadyLspProvider {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &std::path::Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn crate::lsp::ops::LspClientOps>> {
            anyhow::bail!("not used in tests")
        }

        async fn notify_file_changed(&self, _path: &std::path::Path) {}

        async fn shutdown_all(&self) {}

        async fn is_ready(&self, _language: &str, _workspace_root: &std::path::Path) -> bool {
            true
        }
    }

    /// A stub LspProvider that always reports `is_ready = false` (uses the default impl).
    struct NotReadyLspProvider;

    #[async_trait]
    impl crate::lsp::ops::LspProvider for NotReadyLspProvider {
        async fn get_or_start(
            &self,
            _language: &str,
            _workspace_root: &std::path::Path,
            _mux_override: Option<bool>,
        ) -> anyhow::Result<Arc<dyn crate::lsp::ops::LspClientOps>> {
            anyhow::bail!("not used in tests")
        }

        async fn notify_file_changed(&self, _path: &std::path::Path) {}

        async fn shutdown_all(&self) {}
    }

    /// Create a tempdir with `.codescout/project.toml` declaring `languages = ["rust"]`.
    fn make_rust_project_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"t\"").unwrap();
        dir
    }

    #[tokio::test]
    async fn agent_summary_source_lsp_ready_true_when_provider_reports_ready() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(ReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert!(
            snap.lsp_ready,
            "expected lsp_ready=true when provider reports ready"
        );
    }

    #[tokio::test]
    async fn agent_summary_source_lsp_ready_false_when_provider_not_ready() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(NotReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert!(
            !snap.lsp_ready,
            "expected lsp_ready=false when provider not ready"
        );
    }

    #[tokio::test]
    async fn agent_summary_source_index_status_idle() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        // default IndexingState is Idle
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(NotReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert_eq!(snap.index_status, "idle");
    }

    #[tokio::test]
    async fn agent_summary_source_index_status_running() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        *agent.indexing.lock().unwrap() = crate::agent::IndexingState::Running {
            done: 5,
            total: 10,
            eta_secs: None,
        };
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(NotReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert_eq!(snap.index_status, "indexing");
    }

    #[tokio::test]
    async fn agent_summary_source_index_status_done() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        *agent.indexing.lock().unwrap() = crate::agent::IndexingState::Done {
            files_indexed: 42,
            files_deleted: 0,
            detail: "ok".into(),
            total_files: 42,
            total_chunks: 100,
        };
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(NotReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert_eq!(snap.index_status, "indexed");
    }

    #[tokio::test]
    async fn agent_summary_source_index_status_failed() {
        let dir = make_rust_project_dir();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        *agent.indexing.lock().unwrap() = crate::agent::IndexingState::Failed("embed error".into());
        let lsp: Arc<dyn crate::lsp::ops::LspProvider> = Arc::new(NotReadyLspProvider);
        let source = AgentSummarySource::new(agent, lsp);
        let snap = source.snapshot().await;
        assert_eq!(snap.index_status, "failed");
    }
}
