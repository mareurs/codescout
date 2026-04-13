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

/// Adapter that fills [`SummarySnapshot`] from live [`crate::agent::Agent`] state.
///
/// All probes are best-effort — any field that can't be determined falls back to
/// `None` / `"unknown"` / `false` rather than propagating an error.
pub struct AgentSummarySource {
    agent: crate::agent::Agent,
}

impl AgentSummarySource {
    pub fn new(agent: crate::agent::Agent) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl SummarySource for AgentSummarySource {
    async fn snapshot(&self) -> SummarySnapshot {
        let active_project = self
            .agent
            .project_root()
            .await
            .map(|p| p.display().to_string());

        let language = self
            .agent
            .with_project(|p| {
                Ok(p.config
                    .project
                    .languages
                    .first()
                    .cloned()
                    .unwrap_or_default())
            })
            .await
            .ok()
            .filter(|s: &String| !s.is_empty());

        // No runtime freshness API — leave as "unknown".
        let index_status = "unknown".to_string();

        SummarySnapshot {
            active_project,
            index_status,
            language,
            lsp_ready: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
