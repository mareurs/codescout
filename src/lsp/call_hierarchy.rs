use lsp_types::ServerCapabilities;

pub fn supports_call_hierarchy(caps: &ServerCapabilities) -> bool {
    caps.call_hierarchy_provider.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_when_provider_present() {
        let caps = ServerCapabilities {
            call_hierarchy_provider: Some(lsp_types::CallHierarchyServerCapability::Simple(true)),
            ..Default::default()
        };
        assert!(supports_call_hierarchy(&caps));
    }

    #[test]
    fn unsupported_when_none() {
        let caps = ServerCapabilities::default();
        assert!(!supports_call_hierarchy(&caps));
    }
}
