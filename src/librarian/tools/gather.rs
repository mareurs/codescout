use crate::librarian::catalog::{
    find::{find, FindOpts},
    observations,
};
use crate::librarian::filter::FilterNode;
use crate::librarian::tools::ToolContext;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum GatherSource {
    GitLog {
        limit: Option<usize>,
        /// "last_refresh" or ISO-8601 timestamp
        since: Option<String>,
        branch: Option<String>,
        grep: Option<String>,
    },
    Artifacts {
        filter: Option<Value>,
        limit: Option<usize>,
    },
    Observations {
        artifact_id: Option<String>,
        limit: Option<usize>,
        /// "last_refresh" or ISO-8601 timestamp
        since: Option<String>,
    },
    File {
        path: String,
    },
    Grep {
        pattern: String,
        path: Option<String>,
        limit: Option<usize>,
    },
    ConfigValue {
        path: String,
        key: String,
    },

    #[serde(other)]
    Unknown,
}

pub struct GatherResult {
    pub source_key: String,
    pub data: Value,
}

fn resolve_since(since: &str, last_refreshed_at: Option<&str>) -> Option<i64> {
    if since == "last_refresh" {
        last_refreshed_at.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
        })
    } else {
        chrono::DateTime::parse_from_rfc3339(since)
            .ok()
            .map(|dt| dt.timestamp_millis())
    }
}

pub async fn gather_all(
    sources: &[GatherSource],
    ctx: &ToolContext,
    last_refreshed_at: Option<&str>,
) -> Result<(Vec<GatherResult>, Vec<String>)> {
    let mut results: Vec<GatherResult> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for source in sources {
        match source {
            GatherSource::ConfigValue { path, key } => match gather_config_value(ctx, path, key) {
                Ok(data) => results.push(GatherResult {
                    source_key: "config_value".to_string(),
                    data,
                }),
                Err(e) => warnings.push(format!("config_value gather failed for '{path}': {e}")),
            },
            GatherSource::Unknown => {
                warnings.push("unknown gather source skipped".to_string());
            }
            GatherSource::GitLog {
                limit,
                since,
                branch,
                grep,
            } => {
                match gather_git_log(
                    ctx,
                    *limit,
                    since.as_deref(),
                    branch.as_deref(),
                    grep.as_deref(),
                    last_refreshed_at,
                ) {
                    Ok(data) => results.push(GatherResult {
                        source_key: "git_log".to_string(),
                        data,
                    }),
                    Err(e) => warnings.push(format!("git_log gather failed: {e}")),
                }
            }
            GatherSource::Artifacts { filter, limit } => {
                match gather_artifacts(ctx, filter.as_ref(), *limit) {
                    Ok(data) => results.push(GatherResult {
                        source_key: "artifacts".to_string(),
                        data,
                    }),
                    Err(e) => warnings.push(format!("artifacts gather failed: {e}")),
                }
            }
            GatherSource::Observations {
                artifact_id,
                limit,
                since,
            } => {
                let since_ms = since
                    .as_deref()
                    .and_then(|s| resolve_since(s, last_refreshed_at));
                match gather_observations(
                    ctx,
                    artifact_id.as_deref(),
                    since_ms,
                    limit.unwrap_or(20),
                ) {
                    Ok(data) => results.push(GatherResult {
                        source_key: "observations".to_string(),
                        data,
                    }),
                    Err(e) => warnings.push(format!("observations gather failed: {e}")),
                }
            }
            GatherSource::File { path } => match gather_file(ctx, path) {
                Ok(data) => results.push(GatherResult {
                    source_key: "file".to_string(),
                    data,
                }),
                Err(e) => warnings.push(format!("file gather failed for '{path}': {e}")),
            },
            GatherSource::Grep {
                pattern,
                path,
                limit,
            } => match gather_grep(ctx, pattern, path.as_deref(), limit.unwrap_or(50)) {
                Ok(data) => results.push(GatherResult {
                    source_key: "grep".to_string(),
                    data,
                }),
                Err(e) => warnings.push(format!("grep gather failed: {e}")),
            },
        }
    }

    Ok((results, warnings))
}

/// For each child reference, fetch the child's augmentation params from the
/// catalog and run `goal_aggregation::child_status_in_context(archetype, …)`.
///
/// Returns a JSON array of `{child_id, artifact_id, archetype, status, basis}`
/// records. `basis` is one of:
/// - `"deterministic"` — status derived from `child_status_in_context`
///   (pure for 6 archetypes, signal-driven for `metric_baseline` when at
///   least one `metric_threshold` signal cites the child)
/// - `"needs parent context"` — archetype is `metric_baseline` but no
///   parent signal cites it; LLM falls back to rule 1b
/// - `"unknown archetype"` — archetype not in the known set (H-5)
/// - `"no augmentation"` — child has no augmentation row
/// - `"child unreachable"` — child artifact not found in catalog (Orphan)
///
/// Yak's variant (b) gather-time integration: the LLM reads ground truth from
/// `context.deterministic_child_statuses` rather than re-deriving rule 1
/// per-archetype reconciliation logic from the augmentation prompt.
pub fn gather_goal_children(
    ctx: &ToolContext,
    children: &[(String, String, String)],
    parent_signals: &[crate::librarian::tools::goal_aggregation::AcceptanceSignal],
) -> Result<Value> {
    use crate::librarian::catalog::augmentation;
    use crate::librarian::tools::goal_aggregation::{child_status_in_context, ChildStatus};

    let cat = ctx.catalog.lock();

    // Pre-fetch every child's params once so the metric_baseline lookup
    // (which evaluate_signal needs) sees the same snapshot as the
    // per-child status derivation.
    let mut child_params_lookup: Vec<(String, String, Value)> = Vec::with_capacity(children.len());
    for (child_id, artifact_id, archetype) in children {
        let params = augmentation::get(&cat, artifact_id)?
            .and_then(|a| serde_json::from_str(&a.params).ok())
            .unwrap_or(Value::Null);
        child_params_lookup.push((child_id.clone(), archetype.clone(), params));
    }

    let mut out = Vec::with_capacity(children.len());

    for (child_id, artifact_id, archetype) in children {
        // Distinguish "no augmentation" (artifact exists, no augmentation row)
        // from "unreachable" (no artifact row at all).
        let artifact_exists =
            crate::librarian::catalog::artifact::get(&cat, artifact_id)?.is_some();

        let (status, basis) = if !artifact_exists {
            (ChildStatus::Orphan, "child unreachable")
        } else {
            let aug_row = augmentation::get(&cat, artifact_id)?;
            match aug_row {
                None => (ChildStatus::Unknown, "no augmentation"),
                Some(aug) => {
                    let params: Value = serde_json::from_str(&aug.params).unwrap_or(Value::Null);
                    let s = child_status_in_context(
                        archetype,
                        child_id,
                        &params,
                        parent_signals,
                        &child_params_lookup,
                    );
                    let b = match s {
                        ChildStatus::Active if archetype == "metric_baseline" => {
                            // child_status_in_context returns Active when
                            // metric_baseline has no citing signal OR all
                            // signals unmet. Distinguish for clarity.
                            let any_citing = parent_signals.iter().any(|sig| {
                                matches!(
                                    &sig.spec,
                                    crate::librarian::tools::goal_aggregation::AcceptanceSignalSpec::MetricThreshold { evidence_child_id, .. }
                                        if evidence_child_id == child_id
                                )
                            });
                            if any_citing {
                                "deterministic"
                            } else {
                                "needs parent context"
                            }
                        }
                        ChildStatus::Unknown if archetype.is_empty() => "no augmentation",
                        ChildStatus::Unknown => "unknown archetype",
                        _ => "deterministic",
                    };
                    (s, b)
                }
            }
        };

        out.push(json!({
            "child_id": child_id,
            "artifact_id": artifact_id,
            "archetype": archetype,
            "status": status.as_str(),
            "basis": basis,
        }));
    }

    Ok(Value::Array(out))
}

fn project_root(ctx: &ToolContext) -> Option<std::path::PathBuf> {
    // CurrentProject.path is already the resolved absolute path to the project root
    ctx.current_project
        .as_ref()
        .map(|cp| cp.abs_path.clone())
        .or_else(|| ctx.workspace.roots.first().map(|r| r.path.clone()))
}

fn gather_git_log(
    ctx: &ToolContext,
    limit: Option<usize>,
    since: Option<&str>,
    branch: Option<&str>,
    grep: Option<&str>,
    last_refreshed_at: Option<&str>,
) -> Result<Value> {
    let root = project_root(ctx).ok_or_else(|| anyhow::anyhow!("no project root"))?;
    let repo = git2::Repository::discover(&root)
        .map_err(|e| anyhow::anyhow!("git repo not found: {e}"))?;

    let since_secs: Option<i64> =
        since.and_then(|s| resolve_since(s, last_refreshed_at).map(|ms| ms / 1000));

    let mut revwalk = repo.revwalk()?;
    if let Some(branch_name) = branch {
        let branch_ref = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .or_else(|_| repo.find_branch(branch_name, git2::BranchType::Remote))
            .map_err(|_| anyhow::anyhow!("branch '{branch_name}' not found"))?;
        revwalk.push(branch_ref.get().peel_to_commit()?.id())?;
    } else {
        revwalk.push_head()?;
    }
    revwalk.set_sorting(git2::Sort::TIME)?;

    let limit = limit.unwrap_or(20);
    let grep_re = grep.map(regex::Regex::new).transpose()?;

    let commits: Vec<Value> = revwalk
        .filter_map(|oid| oid.ok())
        .filter_map(|oid| repo.find_commit(oid).ok())
        .filter(|c| since_secs.is_none_or(|ts| c.time().seconds() > ts))
        .filter(|c| {
            grep_re
                .as_ref()
                .is_none_or(|re| c.summary().is_some_and(|s| re.is_match(s)))
        })
        .take(limit)
        .map(|c| {
            json!({
                "hash": &c.id().to_string()[..8],
                "time": c.time().seconds(),
                "subject": c.summary().unwrap_or(""),
                "author": c.author().name().unwrap_or(""),
            })
        })
        .collect();

    Ok(json!(commits))
}

fn gather_artifacts(
    ctx: &ToolContext,
    filter: Option<&Value>,
    limit: Option<usize>,
) -> Result<Value> {
    let filter_node: Option<FilterNode> = filter
        .map(|f| serde_json::from_value(f.clone()))
        .transpose()?;
    let cat = ctx.catalog.lock();
    let rows = find(
        &cat,
        &FindOpts {
            filter: filter_node,
            limit: limit.unwrap_or(20),
            offset: 0,
        },
    )?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.id,
                "kind": r.kind,
                "status": r.status,
                "title": r.title,
                "topic": r.topic,
                "abs_path": r.abs_path.display().to_string(),
            })
        })
        .collect();
    Ok(json!(items))
}

fn gather_observations(
    ctx: &ToolContext,
    artifact_id: Option<&str>,
    since_ms: Option<i64>,
    limit: usize,
) -> Result<Value> {
    let cat = ctx.catalog.lock();
    let obs = observations::list_recent(&cat, artifact_id, since_ms, limit)?;
    let items: Vec<Value> = obs
        .iter()
        .map(|o| {
            json!({
                "artifact_id": o.artifact_id,
                "text": o.text,
                "source": o.source,
                "created_at": o.created_at,
            })
        })
        .collect();
    Ok(json!(items))
}

fn guard_relative_path(path: &str) -> Result<()> {
    // Reject absolute paths in BOTH POSIX (leading /) and Windows (leading \ or
    // drive letter) forms — Path::is_absolute() only checks the current
    // platform's notion of absolute, but the gather tool must reject any
    // shape that could escape the project root on any platform.
    //
    // Also reject any embedded colon: on Windows the colon is reserved for
    // drive prefixes (`C:foo`) AND for NTFS alternate data streams
    // (`legit.txt:hidden`). In legitimate relative paths on either platform
    // there is no reason to allow colons, so a blanket reject closes the
    // ADS-read path the drive-letter check alone misses (Ibex S-2).
    let starts_with_drive =
        path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':';
    if path.contains("..")
        || std::path::Path::new(path).is_absolute()
        || path.starts_with('/')
        || path.starts_with('\\')
        || starts_with_drive
        || path.contains(':')
    {
        anyhow::bail!("path must be relative and must not contain '..' or ':'");
    }
    Ok(())
}

fn gather_file(ctx: &ToolContext, path: &str) -> Result<Value> {
    guard_relative_path(path)?;
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let full = base.join(path);
    let content = std::fs::read_to_string(&full)
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {e}", full.display()))?;
    Ok(json!(content))
}

fn gather_grep(
    ctx: &ToolContext,
    pattern: &str,
    path: Option<&str>,
    limit: usize,
) -> Result<Value> {
    if let Some(p) = path {
        guard_relative_path(p)?;
    }
    use walkdir::WalkDir;
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let search_root = path.map(|p| base.join(p)).unwrap_or(base);
    let re = regex::Regex::new(pattern)?;

    let mut matches: Vec<Value> = Vec::new();
    'outer: for entry in WalkDir::new(&search_root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path_str = entry.path().to_string_lossy().to_string();
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for (lineno, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(json!({
                        "file": path_str,
                        "line": lineno + 1,
                        "text": line,
                    }));
                    if matches.len() >= limit {
                        break 'outer;
                    }
                }
            }
        }
    }

    Ok(json!(matches))
}

fn last_changed(project_root: &std::path::Path, rel_path: &str) -> Option<(String, String)> {
    let repo = git2::Repository::open(project_root).ok()?;
    let blame = repo.blame_file(std::path::Path::new(rel_path), None).ok()?;
    let hunk = blame
        .iter()
        .max_by_key(|h| h.final_signature().when().seconds())?;
    let commit_id = hunk.final_commit_id().to_string();
    let seconds = hunk.final_signature().when().seconds();
    use chrono::TimeZone as _;
    let dt = chrono::Utc.timestamp_opt(seconds, 0).single()?;
    Some((commit_id, dt.to_rfc3339()))
}

fn gather_config_value(
    ctx: &ToolContext,
    path: &str,
    key: &str,
) -> anyhow::Result<serde_json::Value> {
    guard_relative_path(path)?;
    let base = project_root(ctx).unwrap_or_else(|| std::path::PathBuf::from("."));
    let full = base.join(path);
    let content = std::fs::read_to_string(&full)
        .map_err(|e| anyhow::anyhow!("cannot read '{}': {e}", full.display()))?;

    let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mut val: serde_json::Value = match ext {
        "toml" => {
            let parsed: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("TOML parse error in '{path}': {e}"))?;
            serde_json::to_value(parsed)?
        }
        "yaml" | "yml" => {
            let parsed: serde_yml::Value = serde_yml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("YAML parse error in '{path}': {e}"))?;
            serde_json::to_value(parsed)?
        }
        "json" => serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("JSON parse error in '{path}': {e}"))?,
        other => anyhow::bail!("unsupported config extension '.{other}' for '{path}'"),
    };

    for segment in key.split('.') {
        val = match val {
            serde_json::Value::Object(map) => map
                .get(segment)
                .ok_or_else(|| anyhow::anyhow!("key '{segment}' not found in '{path}'"))?
                .clone(),
            serde_json::Value::Array(arr) => {
                let idx: usize = segment.parse().map_err(|_| {
                    anyhow::anyhow!("array index '{segment}' is not a number in '{path}'")
                })?;
                arr.get(idx)
                    .ok_or_else(|| anyhow::anyhow!("array index {idx} out of bounds in '{path}'"))?
                    .clone()
            }
            _ => anyhow::bail!("cannot traverse into scalar at segment '{segment}' in '{path}'"),
        };
    }

    let (commit, at) = last_changed(&base, path)
        .map(|(c, a)| (json!(c), json!(a)))
        .unwrap_or((json!(null), json!(null)));

    Ok(json!({
        "path": path,
        "key": key,
        "value": val,
        "last_changed_commit": commit,
        "last_changed_at": at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use parking_lot::Mutex;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp: &TempDir) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        ToolContext {
            catalog: Arc::new(Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "repo".into(),
                    path: tmp.path().to_path_buf(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            artifact_store: None,
            current_project: None,
        }
    }

    #[test]
    fn guard_relative_path_rejects_dotdot() {
        assert!(guard_relative_path("../etc/passwd").is_err());
        assert!(guard_relative_path("foo/../bar").is_err());
    }

    #[test]
    fn guard_relative_path_rejects_absolute() {
        assert!(guard_relative_path("/etc/passwd").is_err());
    }

    #[test]
    fn guard_relative_path_accepts_normal() {
        assert!(guard_relative_path("src/main.rs").is_ok());
        assert!(guard_relative_path("a/b/c.txt").is_ok());
    }

    #[test]
    fn guard_relative_path_rejects_ads_colon() {
        // NTFS alternate data stream selector: foo.txt:hidden reads a hidden
        // stream attached to foo.txt on Windows. Ibex S-2 in the rounds 3-8
        // review found the drive-letter guard alone did not catch this.
        assert!(guard_relative_path("legit.txt:hidden").is_err());
        assert!(guard_relative_path("docs/foo.md:stream").is_err());
        // Drive-letter relative form must remain rejected.
        assert!(guard_relative_path("C:foo.txt").is_err());
    }

    #[test]
    fn gather_file_rejects_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_file(&ctx, "../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn gather_file_reads_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("test.txt"), "hello content").unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_file(&ctx, "test.txt").unwrap();
        assert_eq!(result, serde_json::json!("hello content"));
    }

    #[test]
    fn gather_grep_rejects_dotdot_path() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_grep(&ctx, "pattern", Some("../etc"), 10);
        assert!(result.is_err());
    }

    #[test]
    fn gather_grep_limits_results() {
        let tmp = tempfile::tempdir().unwrap();
        // Write a file with 10 matching lines
        let content = (0..10)
            .map(|i| format!("match line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(tmp.path().join("test.txt"), content).unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_grep(&ctx, "match line", None, 5).unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.len() <= 5);
    }

    #[tokio::test]
    async fn unknown_source_produces_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        let sources = vec![GatherSource::Unknown];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(results.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unknown"));
    }

    #[test]
    fn gather_source_config_value_deserializes() {
        let src: GatherSource = serde_json::from_str(
            r#"{"source":"config_value","path":"Cargo.toml","key":"package.version"}"#,
        )
        .unwrap();
        assert!(matches!(src, GatherSource::ConfigValue { .. }));
    }

    #[test]
    fn gather_config_value_toml_key_found() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[package]\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.toml", "package.version").unwrap();
        assert_eq!(result["value"], serde_json::json!("1.2.3"));
        assert_eq!(result["path"], "config.toml");
        assert_eq!(result["key"], "package.version");
    }

    #[test]
    fn gather_config_value_yaml_key_found() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.yaml"),
            "database:\n  host: localhost\n",
        )
        .unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.yaml", "database.host").unwrap();
        assert_eq!(result["value"], serde_json::json!("localhost"));
    }

    #[test]
    fn gather_config_value_json_key_found() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"feature":{"enabled":true}}"#,
        )
        .unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.json", "feature.enabled").unwrap();
        assert_eq!(result["value"], serde_json::json!(true));
    }

    #[test]
    fn gather_config_value_array_index() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.json"),
            r#"{"servers":[{"host":"a"},{"host":"b"}]}"#,
        )
        .unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.json", "servers.1.host").unwrap();
        assert_eq!(result["value"], serde_json::json!("b"));
    }

    #[test]
    fn gather_config_value_key_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "[package]\nname = \"x\"\n").unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.toml", "package.missing_key");
        assert!(result.is_err());
    }

    #[test]
    fn gather_config_value_unknown_extension() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("config.conf"), "key=value\n").unwrap();
        let ctx = mk_ctx(&tmp);
        let result = gather_config_value(&ctx, "config.conf", "key");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn gather_all_config_value_source() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("app.toml"), "[server]\nport = 8080\n").unwrap();
        let ctx = mk_ctx(&tmp);
        let sources = vec![GatherSource::ConfigValue {
            path: "app.toml".to_string(),
            key: "server.port".to_string(),
        }];
        let (results, warnings) = gather_all(&sources, &ctx, None).await.unwrap();
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_key, "config_value");
        assert_eq!(results[0].data["value"], serde_json::json!(8080));
    }

    // --- gather_goal_children (Yak variant (b) integration) ---

    fn sample_artifact(id: &str) -> crate::librarian::catalog::artifact::ArtifactRow {
        crate::librarian::catalog::artifact::ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: Some("T".to_string()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "abc".to_string(),
            confidence: 1.0,
        }
    }

    fn sample_augmentation(
        artifact_id: &str,
        params_json: &str,
    ) -> crate::librarian::catalog::augmentation::AugmentationRow {
        crate::librarian::catalog::augmentation::AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: "test".to_string(),
            params: params_json.to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: None,
        }
    }

    #[test]
    fn gather_goal_children_returns_deterministic_status_per_child() {
        use crate::librarian::catalog::{artifact, augmentation};

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        {
            let cat = ctx.catalog.lock();
            artifact::upsert(&cat, &sample_artifact("child-a")).unwrap();
            augmentation::upsert(
                &cat,
                &sample_augmentation("child-a", r#"{"failures":[{"id":"F-1","status":"pass"}]}"#),
            )
            .unwrap();
            artifact::upsert(&cat, &sample_artifact("child-b")).unwrap();
            augmentation::upsert(&cat, &sample_augmentation("child-b", r#"{"tasks":[]}"#)).unwrap();
        }

        let children = vec![
            (
                "C-1".to_string(),
                "child-a".to_string(),
                "failure_table".to_string(),
            ),
            (
                "C-2".to_string(),
                "child-b".to_string(),
                "task_list".to_string(),
            ),
        ];
        let result = gather_goal_children(&ctx, &children, &[]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["child_id"], "C-1");
        assert_eq!(arr[0]["archetype"], "failure_table");
        assert_eq!(arr[0]["status"], "done");
        assert_eq!(arr[0]["basis"], "deterministic");
        assert_eq!(arr[1]["child_id"], "C-2");
        assert_eq!(arr[1]["status"], "pending");
        assert_eq!(arr[1]["basis"], "deterministic");
    }

    #[test]
    fn gather_goal_children_marks_missing_artifact_as_orphan() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        // No artifact upserted — catalog is empty.
        let children = vec![(
            "C-1".to_string(),
            "ghost".to_string(),
            "failure_table".to_string(),
        )];
        let result = gather_goal_children(&ctx, &children, &[]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["status"], "orphan");
        assert_eq!(arr[0]["basis"], "child unreachable");
    }

    #[test]
    fn gather_goal_children_marks_metric_baseline_needs_context() {
        use crate::librarian::catalog::{artifact, augmentation};

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        {
            let cat = ctx.catalog.lock();
            artifact::upsert(&cat, &sample_artifact("metric")).unwrap();
            augmentation::upsert(
                &cat,
                &sample_augmentation("metric", r#"{"baseline":{"P@5":0.18}}"#),
            )
            .unwrap();
        }

        let children = vec![(
            "C-1".to_string(),
            "metric".to_string(),
            "metric_baseline".to_string(),
        )];
        // No parent_signals → metric_baseline child is Active (the child
        // exists, but no acceptance_signal[kind=metric_threshold] cites it).
        // basis is "needs parent context" to signal the LLM should fall back
        // to prompt rule 1b for this child.
        let result = gather_goal_children(&ctx, &children, &[]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0]["status"], "active");
        assert_eq!(arr[0]["basis"], "needs parent context");
    }

    #[test]
    fn gather_goal_children_handles_missing_augmentation() {
        use crate::librarian::catalog::artifact;

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        {
            let cat = ctx.catalog.lock();
            artifact::upsert(&cat, &sample_artifact("bare")).unwrap();
            // No augmentation upserted.
        }

        let children = vec![(
            "C-1".to_string(),
            "bare".to_string(),
            "failure_table".to_string(),
        )];
        let result = gather_goal_children(&ctx, &children, &[]).unwrap();
        assert_eq!(result[0]["status"], "unknown");
        assert_eq!(result[0]["basis"], "no augmentation");
    }

    #[test]
    fn gather_goal_children_metric_baseline_done_via_signal_context() {
        // D8 integration: parent goal has a metric_threshold signal citing
        // C-M; child's current.P@5 satisfies the threshold; child must
        // resolve to "done" with basis="deterministic".
        use crate::librarian::catalog::{artifact, augmentation};
        use crate::librarian::tools::goal_aggregation::{
            AcceptanceSignal, AcceptanceSignalSpec, ThresholdOp,
        };

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        {
            let cat = ctx.catalog.lock();
            artifact::upsert(&cat, &sample_artifact("metric")).unwrap();
            augmentation::upsert(
                &cat,
                &sample_augmentation(
                    "metric",
                    r#"{"baseline":{"P@5":0.18},"current":{"P@5":0.21}}"#,
                ),
            )
            .unwrap();
        }
        let children = vec![(
            "C-M".to_string(),
            "metric".to_string(),
            "metric_baseline".to_string(),
        )];
        let parent_signals = vec![AcceptanceSignal {
            description: "P@5 ≥ 0.20".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "P@5".into(),
                op: ThresholdOp::Gte,
                threshold: 0.20,
            },
        }];
        let result = gather_goal_children(&ctx, &children, &parent_signals).unwrap();
        assert_eq!(result[0]["status"], "done");
        assert_eq!(result[0]["basis"], "deterministic");
    }

    #[test]
    fn gather_goal_children_metric_baseline_in_progress_when_threshold_unmet() {
        use crate::librarian::catalog::{artifact, augmentation};
        use crate::librarian::tools::goal_aggregation::{
            AcceptanceSignal, AcceptanceSignalSpec, ThresholdOp,
        };

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(&tmp);
        {
            let cat = ctx.catalog.lock();
            artifact::upsert(&cat, &sample_artifact("metric")).unwrap();
            augmentation::upsert(
                &cat,
                &sample_augmentation("metric", r#"{"current":{"P@5":0.15}}"#),
            )
            .unwrap();
        }
        let children = vec![(
            "C-M".to_string(),
            "metric".to_string(),
            "metric_baseline".to_string(),
        )];
        let parent_signals = vec![AcceptanceSignal {
            description: "P@5 ≥ 0.20".into(),
            met: false,
            evidence: String::new(),
            spec: AcceptanceSignalSpec::MetricThreshold {
                evidence_child_id: "C-M".into(),
                metric_key: "P@5".into(),
                op: ThresholdOp::Gte,
                threshold: 0.20,
            },
        }];
        let result = gather_goal_children(&ctx, &children, &parent_signals).unwrap();
        // Single citing signal, unmet → active (per child_status_in_context fold).
        assert_eq!(result[0]["status"], "active");
        assert_eq!(result[0]["basis"], "deterministic");
    }
}
