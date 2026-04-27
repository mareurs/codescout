//! Kind-aware preview extractors for artifact_get.
//!
//! Each artifact kind (plan, spec, memory, ...) has its own preview shape.
//! Unknown kinds fall back to the `default` extractor. See
//! `docs/superpowers/specs/2026-04-20-artifact-get-preview-design.md`.

pub mod default;
pub mod headings;
pub mod memory;
pub mod plan;
pub mod spec;
pub mod summary;

use crate::catalog::artifact::ArtifactRow;
use crate::tools::ToolContext;
use serde_json::Value;

/// Compute a kind-specific preview for an artifact.
///
/// `body` is the markdown body with frontmatter already stripped.
/// Returns a tagged JSON object with at least a `"shape"` discriminator.
pub fn extract(kind: &str, row: &ArtifactRow, body: &str, ctx: &ToolContext) -> Value {
    match kind {
        "plan" => plan::extract(row, body),
        "spec" => spec::extract(row, body),
        "memory" => memory::extract(row, body, ctx),
        _ => default::extract(row, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::ArtifactRow;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_row(kind: &str) -> ArtifactRow {
        ArtifactRow {
            id: "x".into(),
            repo: "r".into(),
            rel_path: "x.md".into(),
            kind: kind.into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    #[test]
    fn routes_plan_to_plan_extractor() {
        let v = extract("plan", &mk_row("plan"), "- [ ] a\n", &mk_ctx());
        assert_eq!(v["shape"], "plan");
    }

    #[test]
    fn routes_spec_to_spec_extractor() {
        let v = extract("spec", &mk_row("spec"), "# T\n\nx.\n", &mk_ctx());
        assert_eq!(v["shape"], "spec");
    }

    #[test]
    fn routes_memory_to_memory_extractor() {
        let v = extract("memory", &mk_row("memory"), "", &mk_ctx());
        assert_eq!(v["shape"], "memory");
    }

    #[test]
    fn unknown_kind_falls_back_to_default() {
        let v = extract("adr", &mk_row("adr"), "text\n", &mk_ctx());
        assert_eq!(v["shape"], "default");
    }
}
