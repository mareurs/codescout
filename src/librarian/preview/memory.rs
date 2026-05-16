//! `memory` artifact preview: observation feed + summary.

use crate::librarian::catalog::artifact::ArtifactRow;
use crate::librarian::catalog::observations;
use crate::librarian::preview::summary;
use crate::librarian::tools::ToolContext;
use serde_json::{json, Value};

const LATEST_OBSERVATIONS: usize = 3;
const OBSERVATION_TEXT_MAX: usize = 200;

pub fn extract(row: &ArtifactRow, body: &str, ctx: &ToolContext) -> Value {
    let cat = ctx.catalog.lock();
    let mut obs = observations::list_for_artifact(&cat, &row.id).unwrap_or_default();
    let observation_count = obs.len();
    obs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    obs.truncate(LATEST_OBSERVATIONS);

    let latest: Vec<Value> = obs
        .into_iter()
        .map(|o| {
            json!({
                "text": truncate_text(&o.text),
                "created_at": o.created_at,
            })
        })
        .collect();

    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };

    json!({
        "shape": "memory",
        "observation_count": observation_count,
        "latest_observations": latest,
        "summary": summary::extract(body),
        "line_count": line_count,
    })
}

fn truncate_text(s: &str) -> String {
    if s.chars().count() <= OBSERVATION_TEXT_MAX {
        return s.to_string();
    }
    let mut out: String = s.chars().take(OBSERVATION_TEXT_MAX).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact;
    use crate::librarian::catalog::observations::ObservationRow;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_row(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
            kind: "memory".into(),
            status: "active".into(),
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

    fn mk_ctx(cat: Catalog) -> ToolContext {
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
    fn latest_observations_ordered_desc_by_created_at() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        for (i, ts) in [10i64, 30, 20, 40, 50].iter().enumerate() {
            observations::insert(
                &cat,
                &ObservationRow {
                    id: None,
                    artifact_id: "m".into(),
                    text: format!("obs{i}-{ts}"),
                    source: None,
                    created_at: *ts,
                },
            )
            .unwrap();
        }
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["shape"], "memory");
        assert_eq!(v["observation_count"], 5);
        let latest = v["latest_observations"].as_array().unwrap();
        assert_eq!(latest.len(), 3);
        // Ordered by created_at DESC: 50, 40, 30
        assert_eq!(latest[0]["created_at"], 50);
        assert_eq!(latest[1]["created_at"], 40);
        assert_eq!(latest[2]["created_at"], 30);
    }

    #[test]
    fn no_observations_returns_zero_count_and_empty_list() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["observation_count"], 0);
        assert_eq!(v["latest_observations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn summary_falls_back_to_empty_when_body_empty() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["summary"], "");
    }

    #[test]
    fn summary_uses_body_when_present() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "# Title\n\nMemory prose here.\n", &ctx);
        assert_eq!(v["summary"], "Memory prose here.");
    }

    #[test]
    fn observation_text_truncated_to_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let long = "y".repeat(300);
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "m".into(),
                text: long,
                source: None,
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        let text = v["latest_observations"][0]["text"].as_str().unwrap();
        assert!(text.ends_with('…'));
        assert!(text.chars().count() <= OBSERVATION_TEXT_MAX + 1);
    }
}
