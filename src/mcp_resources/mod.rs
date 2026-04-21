//! MCP resource providers — handlers for `resources/list` and `resources/read`.
//!
//! Distinct from `config::ResourcesSection`, which governs LSP resource limits.

use std::collections::HashMap;

pub mod doc;
pub mod memory;
pub mod project_hints;
pub mod project_summary;
pub mod tool_guide;
pub mod tool_usage;

#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: String,
}

#[derive(Debug)]
pub enum ResourceBytes {
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("source unavailable for {0}: {1}")]
    SourceUnavailable(String, String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait ResourceProvider: Send + Sync {
    fn descriptors(&self) -> Vec<ResourceDescriptor>;
    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError>;
}

#[derive(Default)]
pub struct ResourceRegistry {
    providers: Vec<Box<dyn ResourceProvider>>,
    index: HashMap<String, usize>,
}

impl ResourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_register(&mut self, p: Box<dyn ResourceProvider>) -> anyhow::Result<()> {
        let descriptors = p.descriptors();
        for d in &descriptors {
            if self.index.contains_key(&d.uri) {
                anyhow::bail!("duplicate resource URI: {}", d.uri);
            }
        }
        let idx = self.providers.len();
        for d in descriptors {
            self.index.insert(d.uri, idx);
        }
        self.providers.push(p);
        Ok(())
    }

    /// Convenience: panic on duplicate — use for static registration.
    pub fn register(&mut self, p: Box<dyn ResourceProvider>) {
        self.try_register(p).expect("resource URI collision");
    }

    pub fn list(&self) -> Vec<ResourceDescriptor> {
        self.providers
            .iter()
            .flat_map(|p| p.descriptors())
            .collect()
    }

    pub async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let idx = self
            .index
            .get(uri)
            .ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        self.providers[*idx].read(uri).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_duplicate_uri() {
        let mut reg = ResourceRegistry::new();
        reg.register(Box::new(StubProvider::new("doc://a")));
        let err = reg
            .try_register(Box::new(StubProvider::new("doc://a")))
            .expect_err("duplicate URI must error");
        assert!(err.to_string().contains("doc://a"));
    }

    #[tokio::test]
    async fn registry_read_unknown_returns_not_found() {
        let reg = ResourceRegistry::new();
        let err = reg
            .read("doc://missing")
            .await
            .expect_err("unknown URI must error");
        assert!(matches!(err, ResourceError::NotFound(_)));
    }

    #[tokio::test]
    async fn registry_read_returns_provider_bytes() {
        let mut reg = ResourceRegistry::new();
        reg.register(Box::new(StubProvider::new("doc://ok")));
        let bytes = reg.read("doc://ok").await.unwrap();
        assert!(matches!(bytes, ResourceBytes::Text(_)));
    }

    struct StubProvider {
        uri: String,
    }
    impl StubProvider {
        fn new(u: &str) -> Self {
            Self { uri: u.into() }
        }
    }
    #[async_trait::async_trait]
    impl ResourceProvider for StubProvider {
        fn descriptors(&self) -> Vec<ResourceDescriptor> {
            vec![ResourceDescriptor {
                uri: self.uri.clone(),
                name: "stub".into(),
                description: None,
                mime_type: "text/plain".into(),
            }]
        }
        async fn read(&self, _uri: &str) -> Result<ResourceBytes, ResourceError> {
            Ok(ResourceBytes::Text("stub".into()))
        }
    }
}
