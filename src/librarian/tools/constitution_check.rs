use crate::librarian::catalog::{augmentation, find, Catalog};
use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MatchedRule {
    pub id: String,
    pub tracker_id: String,
    pub title: String,
    pub rule: String,
}

/// Finds active, path-scoped constitution rules whose `paths` glob-match
/// `path`. Path-less (global) rules are never returned — they're surfaced
/// through a different channel (UserPromptSubmit), not this one.
pub fn find_matching_rules(cat: &Catalog, path: &str) -> Result<Vec<MatchedRule>> {
    let opts = find::FindOpts {
        filter: Some(serde_json::from_value(serde_json::json!({
            "and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "constitution"}}
            ]
        }))?),
        limit: 500,
        offset: 0,
    };
    let trackers = find::find(cat, &opts)?;
    let target = std::path::Path::new(path);

    let mut matches = Vec::new();
    for t in trackers {
        let Some(aug) = augmentation::get(cat, &t.id)? else {
            continue;
        };
        if aug.entry_collection.as_deref() != Some("rules") {
            continue;
        }
        let params: Value = serde_json::from_str(&aug.params).unwrap_or(Value::Null);
        let Some(rules) = params.get("rules").and_then(|v| v.as_array()) else {
            continue;
        };
        for r in rules {
            if r.get("status").and_then(|v| v.as_str()) != Some("active") {
                continue;
            }
            let Some(pattern_strs) = r.get("paths").and_then(|v| v.as_array()) else {
                continue; // global rule — not this channel's concern
            };
            let mut builder = globset::GlobSetBuilder::new();
            for p in pattern_strs {
                if let Some(s) = p.as_str() {
                    if let Ok(g) = globset::Glob::new(s) {
                        builder.add(g);
                    }
                }
            }
            let is_match = builder
                .build()
                .map(|set| set.is_match(target))
                .unwrap_or(false);
            if is_match {
                matches.push(MatchedRule {
                    id: r
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    tracker_id: t.id.clone(),
                    title: r
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    rule: r
                        .get("rule")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
    }
    Ok(matches)
}

/// Finds active, path-less (global) constitution rules — the companion of
/// `find_matching_rules`. A rule with no `paths` field is always relevant
/// regardless of what's being touched (e.g. "never commit secrets"); it is
/// surfaced through a session-level channel (UserPromptSubmit), not a
/// tool-call-targeted one, so this function ignores `paths` entirely and
/// returns every active rule that *lacks* it.
pub fn find_global_rules(cat: &Catalog) -> Result<Vec<MatchedRule>> {
    let opts = find::FindOpts {
        filter: Some(serde_json::from_value(serde_json::json!({
            "and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "constitution"}}
            ]
        }))?),
        limit: 500,
        offset: 0,
    };
    let trackers = find::find(cat, &opts)?;

    let mut matches = Vec::new();
    for t in trackers {
        let Some(aug) = augmentation::get(cat, &t.id)? else {
            continue;
        };
        if aug.entry_collection.as_deref() != Some("rules") {
            continue;
        }
        let params: Value = serde_json::from_str(&aug.params).unwrap_or(Value::Null);
        let Some(rules) = params.get("rules").and_then(|v| v.as_array()) else {
            continue;
        };
        for r in rules {
            if r.get("status").and_then(|v| v.as_str()) != Some("active") {
                continue;
            }
            if r.get("paths").and_then(|v| v.as_array()).is_some() {
                continue; // path-scoped — not this channel's concern
            }
            matches.push(MatchedRule {
                id: r
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                tracker_id: t.id.clone(),
                title: r
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rule: r
                    .get("rule")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{
        upsert as art_upsert, ArtifactRow, TestArtifactRowBuilder,
    };
    use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};

    fn sample_art(id: &str, tags: Vec<String>) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        TestArtifactRowBuilder::new(id)
            .with_abs_path(format!("/test/{id}.md"))
            .with_kind("tracker")
            .with_title("T")
            .with_tags(tags)
            .with_created_at(now)
            .with_updated_at(now)
            .with_file_mtime(now)
            .with_file_sha256("x")
            .build()
    }

    fn aug(id: &str, params: &str) -> AugmentationRow {
        AugmentationRow {
            artifact_id: id.to_string(),
            prompt: "test".to_string(),
            params: params.to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some("rules".to_string()),
            refreshed_at_commit: None,
        }
    }

    #[test]
    fn matches_path_scoped_rule_on_glob_hit() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/PinningEngine.kt").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-1");
        assert_eq!(hits[0].tracker_id, "c1");
    }

    #[test]
    fn skips_non_matching_path() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/ui/Button.kt").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn skips_global_path_less_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-2","title":"Never commit secrets","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "anything.txt").unwrap();
        assert!(
            hits.is_empty(),
            "global rules must never surface via path matching"
        );
    }

    #[test]
    fn matches_only_path_scoped_rule_when_tracker_has_mixed_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[
                    {"id":"C-1","paths":["**/solver/**"],"title":"T1","rule":"R1","status":"active"},
                    {"id":"C-2","title":"Never commit secrets","rule":"R2","status":"active"}
                ]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/PinningEngine.kt").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-1");
    }

    #[test]
    fn malformed_glob_in_one_rule_does_not_panic_or_match() {
        // Defense-in-depth: data written before the write-time glob guard
        // (or any path bypassing it) must still degrade to "no match" here,
        // not panic — and must not suppress sibling rules with valid globs.
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[
                    {"id":"C-1","paths":["[invalid"],"title":"T1","rule":"R1","status":"active"},
                    {"id":"C-2","paths":["src/**/*.kt"],"title":"T2","rule":"R2","status":"active"}
                ]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/PinningEngine.kt").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-2");
    }

    #[test]
    fn skips_superseded_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"superseded"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/x.kt").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn skips_trackers_without_constitution_tag() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["some-other-tag".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/x.kt").unwrap();
        assert!(
            hits.is_empty(),
            "trackers not tagged `constitution` must never match"
        );
    }

    #[test]
    fn find_global_rules_returns_only_path_less_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[
                    {"id":"C-1","paths":["**/solver/**"],"title":"T1","rule":"R1","status":"active"},
                    {"id":"C-2","title":"Never commit secrets","rule":"R2","status":"active"}
                ]}"#,
            ),
        )
        .unwrap();

        let hits = find_global_rules(&cat).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-2");
    }

    #[test]
    fn find_global_rules_skips_superseded_and_untagged() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-2","title":"T","rule":"R","status":"superseded"}]}"#,
            ),
        )
        .unwrap();
        art_upsert(&cat, &sample_art("c2", vec!["some-other-tag".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c2",
                r#"{"rules":[{"id":"C-3","title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_global_rules(&cat).unwrap();
        assert!(hits.is_empty());
    }
}
