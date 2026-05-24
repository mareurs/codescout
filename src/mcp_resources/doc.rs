use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};

/// A documentation resource whose body is embedded in the binary at compile
/// time via `include_str!`. Decouples resource availability from the active
/// project root — doc URIs resolve identically regardless of which project
/// the agent is currently focused on.
#[derive(Debug, Clone)]
pub struct DocSource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub content: &'static str,
}

pub struct DocProvider {
    sources: Vec<DocSource>,
}

impl DocProvider {
    pub fn new(sources: Vec<DocSource>) -> Self {
        Self { sources }
    }
}

#[async_trait::async_trait]
impl ResourceProvider for DocProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        self.sources
            .iter()
            .map(|s| ResourceDescriptor {
                uri: s.uri.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                mime_type: "text/markdown".into(),
            })
            .collect()
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let src = self
            .sources
            .iter()
            .find(|s| s.uri == uri)
            .ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        Ok(ResourceBytes::Text(src.content.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn doc_provider_returns_embedded_content() {
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://guide".into(),
            name: "guide".into(),
            description: None,
            content: "# hello",
        }]);
        let bytes = p.read("doc://guide").await.unwrap();
        match bytes {
            ResourceBytes::Text(s) => assert_eq!(s, "# hello"),
            _ => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn doc_provider_reports_unknown_uri() {
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://known".into(),
            name: "known".into(),
            description: None,
            content: "",
        }]);
        let err = p.read("doc://unknown").await.unwrap_err();
        assert!(matches!(err, ResourceError::NotFound(_)));
    }
}
