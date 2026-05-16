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
            semantic: None,
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
    if path.contains("..") || std::path::Path::new(path).is_absolute() {
        anyhow::bail!("path must be relative and must not contain '..'");
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
}
