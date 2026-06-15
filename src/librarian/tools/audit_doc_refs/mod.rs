// src/librarian/tools/audit_doc_refs/mod.rs
use serde::{Deserialize, Serialize};

pub mod parser;
pub mod resolver;
pub mod severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    FilePath,
    FileLine,
    FileSymbol,
    ModulePath,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPosition {
    InlineSpan,
    FencedBlock,
    LinkTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefCandidate {
    pub md_file: String,
    pub md_line: u32,
    pub raw_ref: String,
    pub ref_kind: RefKind,
    pub position: RefPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub md_file: String,
    pub line: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// File / symbol resolved by literal path match.
    Resolved,
    /// File resolved via basename fallback (raw_ref had no `/`; exactly one
    /// file in the repo matched the basename). Lower confidence than `Resolved`
    /// — the doc didn't specify the path, we inferred it.
    ResolvedBasename,
    /// File / symbol could not be located at all.
    Missing,
    FileMissing,
    SymbolMissing,
    LineOob,
    AnchorMissing,
    /// Basename fallback matched more than one file. The reference is not
    /// broken per se, but it's ambiguous — the doc should specify the path.
    AmbiguousBasename,
    Unknown,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Med,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub verdict: Verdict,
    pub severity: Severity,
    pub severity_reason: &'static str,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub candidate: RefCandidate,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMeta {
    pub last_scan_at: Option<String>,
    pub last_scan_commit: Option<String>,
    pub n_files_scanned: u32,
    pub n_refs_found: u32,
    pub degraded: bool,
    pub lsp_languages_offline: Vec<String>,
}

pub mod merger;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub n: u32,
    pub title: String,
    pub severity: Severity,
    pub severity_reason: String,
    pub status: String, // "open" | "in-progress" | "fixed" | "wontfix"
    pub owner: String,
    pub ref_kind: RefKind,
    pub md_file: String,
    pub md_line: u32,
    pub raw_ref: String,
    pub first_seen_commit: String,
    pub first_seen_at: String,
    pub last_verified_at: String,
    pub notes: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackerParams {
    pub issues: Vec<Issue>,
    pub scan_meta: ScanMeta,
    pub parse_warnings: Vec<ParseWarning>,
}

use crate::librarian::tools::{RecoverableError, Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct AuditArgs {
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub emit_tracker: bool,
    #[serde(default)]
    pub tracker_id: Option<String>,
    #[serde(default = "default_fail_on")]
    pub fail_on: String,
}

fn default_true() -> bool {
    true
}
fn default_fail_on() -> String {
    "never".to_string()
}

pub const DEFAULT_AUDIT_GLOBS: &[&str] =
    &["docs/**/*.md", "CLAUDE.md", "**/CLAUDE.md", "**/README.md"];

/// Patterns matched against rel paths AFTER the include set. Matching files
/// are dropped from the scan. Used to exclude content that IS path-shaped
/// markdown but represents reader-side references (agent-onboarding docs
/// describe files in the *reader's* repo, not codescout's) which would only
/// produce noise. Applied only when `paths` is left as the default — an
/// explicit `paths` argument is honoured verbatim so callers can opt back
/// into auditing excluded subtrees on demand. See H-6 (C) in
/// docs/trackers/codescout-usage-hookify.md.
pub const DEFAULT_AUDIT_EXCLUDES: &[&str] = &["docs/agents/**"];

pub const MAX_FILES_DEFAULT: usize = 10_000;

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: AuditArgs = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("audit_doc_refs: bad args: {e}"),
            "see librarian(action=\"audit_doc_refs\") input schema",
        )
    })?;

    let repo_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| {
            RecoverableError::new("audit_doc_refs: no active project; activate one first")
        })?
        .abs_path
        .clone();

    let (globs, excludes): (Vec<String>, Vec<String>) = match args.paths.clone() {
        Some(p) => (p, Vec::new()),
        None => (
            DEFAULT_AUDIT_GLOBS.iter().map(|s| s.to_string()).collect(),
            DEFAULT_AUDIT_EXCLUDES
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
    };

    let files = collect_markdown_files(&repo_root, &globs, &excludes)?;

    let max_files = std::env::var("LIBRARIAN_AUDIT_MAX_FILES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(MAX_FILES_DEFAULT);
    if files.len() > max_files {
        return Err(RecoverableError::with_hint(
            format!(
                "audit_doc_refs: glob matched {} files (cap {})",
                files.len(),
                max_files
            ),
            "tighten `paths` glob or set LIBRARIAN_AUDIT_MAX_FILES",
        ));
    }

    let memory_globs: Vec<_> = severity::DEFAULT_MEMORY_GLOBS
        .iter()
        .map(|g| globset::Glob::new(g).unwrap())
        .collect();

    let basename_index = build_basename_index(&repo_root);

    let resolve_ctx = resolver::ResolveCtx {
        repo_root: &repo_root,
        memory_globs: &memory_globs,
        lsp: None, // v1: LSP not plumbed through ToolContext yet
        degraded_languages: Default::default(),
        basename_index,
    };

    let mut all_findings: Vec<Finding> = Vec::new();
    let mut all_warnings: Vec<ParseWarning> = Vec::new();

    for md in &files {
        let text = std::fs::read_to_string(md).unwrap_or_default();
        let rel = md
            .strip_prefix(&repo_root)
            .unwrap_or(md.as_path())
            .to_path_buf();
        let (cands, warns) = parser::parse_refs(&text, &rel);
        for c in cands {
            let r = resolver::resolve_ref(&c, &resolve_ctx);
            all_findings.push(Finding {
                candidate: c,
                resolution: r,
            });
        }
        all_warnings.extend(warns);
    }

    let offline: Vec<String> = {
        let mut v = resolve_ctx.degraded_languages.borrow().clone();
        v.sort();
        v.dedup();
        v
    };

    let (tracker_id, tracker_path) = if args.emit_tracker {
        let now = chrono::Utc::now();
        let commit = git_head_commit(&repo_root).unwrap_or_else(|| "unknown".to_string());
        match upsert_tracker(
            ctx,
            &args,
            all_findings.clone(),
            all_warnings.clone(),
            files.len(),
            now,
            &commit,
            offline.clone(),
        )
        .await
        {
            Ok((id, path)) => (Some(id), Some(path)),
            Err(e) => {
                tracing::warn!("audit_doc_refs: tracker upsert failed: {e:#}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    let response = build_response(
        &all_findings,
        &all_warnings,
        &offline,
        files.len(),
        tracker_id.as_deref(),
        tracker_path.as_deref(),
        &args.fail_on,
    );
    Ok(response)
}
fn git_head_commit(repo_root: &std::path::Path) -> Option<String> {
    let repo = git2::Repository::open(repo_root).ok()?;
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    let full = commit.id().to_string();
    Some(full[..8.min(full.len())].to_string())
}

/// Walk `repo_root` and build a basename → relative-paths index for the
/// audit resolver's basename-fallback path.
///
/// `ignore::WalkBuilder` honours `.gitignore` so generated artefacts
/// (`target/`, `node_modules/`, `.venv/`) are skipped naturally. A hard cap
/// keeps a runaway monorepo from blowing up the audit's runtime; once the
/// cap is hit we stop indexing (the audit still functions — basename misses
/// just fall through to `Verdict::Missing` for the un-indexed tail).
fn build_basename_index(
    repo_root: &std::path::Path,
) -> std::collections::HashMap<String, Vec<std::path::PathBuf>> {
    /// Soft cap — typical projects (1k–10k files) fit comfortably; monorepos
    /// stop indexing past this and degrade gracefully.
    const MAX_INDEXED_FILES: usize = 50_000;

    let mut index: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();
    let walker = ignore::WalkBuilder::new(repo_root)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut count = 0usize;
    for entry in walker.flatten() {
        if count >= MAX_INDEXED_FILES {
            break;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let rel = path.strip_prefix(repo_root).unwrap_or(path).to_path_buf();
        index.entry(name.to_string()).or_default().push(rel);
        count += 1;
    }
    index
}

async fn ensure_default_tracker(ctx: &ToolContext) -> Result<(String, String)> {
    let tracker_rel_path = "docs/trackers/doc-ref-audit.md";

    // Check if tracker already exists in catalog — search by path suffix
    let find_args = json!({
        "action": "find",
        "filter": { "rel_path": { "contains": tracker_rel_path } },
        "include_archived": true
    });
    if let Ok(v) = crate::librarian::tools::find::call(ctx, find_args).await {
        if let Some(items) = v.get("items").and_then(|x| x.as_array()) {
            if let Some(first) = items.first() {
                if let (Some(id), Some(abs)) = (
                    first.get("id").and_then(|x| x.as_str()),
                    first.get("abs_path").and_then(|x| x.as_str()),
                ) {
                    // Derive rel_path from abs_path relative to project root
                    let project_root = ctx
                        .current_project
                        .as_ref()
                        .map(|p| p.abs_path.clone())
                        .unwrap_or_default();
                    let rel = std::path::Path::new(abs)
                        .strip_prefix(&project_root)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| tracker_rel_path.to_string());
                    return Ok((id.to_string(), rel));
                }
            }
        }
    }

    // Create the tracker file on disk first (create::call needs the parent dir)
    let project_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| {
            crate::librarian::tools::RecoverableError::new("audit_doc_refs: no active project")
        })?
        .abs_path
        .clone();
    let trackers_dir = project_root.join("docs/trackers");
    std::fs::create_dir_all(&trackers_dir)?;

    // create::call writes the file itself — we just need the parent dir to exist.
    let create_args = json!({
        "action": "create",
        "kind": "tracker",
        "title": "Doc Ref Audit",
        "rel_path": tracker_rel_path,
        "tags": ["doc-ref-audit"],
        "body": "Auto-managed by `librarian(audit_doc_refs)`.\n",
        "augment": {
            "prompt": include_str!("./render_prompt.md"),
            "params": { "issues": [], "scan_meta": {}, "parse_warnings": [] }
        }
    });
    let created = crate::librarian::tools::create::call(ctx, create_args).await?;
    let id = created
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("artifact create did not return id — got: {created}"))?
        .to_string();

    // Attach render_template separately (AugmentSpec in create doesn't carry it)
    let augment_args = json!({
        "id": id,
        "prompt": include_str!("./render_prompt.md"),
        "params": { "issues": [], "scan_meta": {}, "parse_warnings": [] },
        "render_template": include_str!("./render_template.j2")
    });
    // Ignore error — render_template is cosmetic; tracker is usable without it
    if let Err(e) = crate::librarian::tools::augment::ArtifactAugment
        .call(ctx, augment_args)
        .await
    {
        tracing::warn!("audit_doc_refs: failed to attach render_template: {e:#}");
    }

    Ok((id, tracker_rel_path.to_string()))
}

async fn load_tracker_params(ctx: &ToolContext, tracker_id: &str) -> Option<TrackerParams> {
    let get_args = json!({
        "action": "get",
        "id": tracker_id,
    });
    let v = crate::librarian::tools::get::call(ctx, get_args)
        .await
        .ok()?;
    let params_value = v.get("augmentation").and_then(|a| a.get("params"))?;
    serde_json::from_value::<TrackerParams>(params_value.clone()).ok()
}

async fn write_tracker_params(
    ctx: &ToolContext,
    tracker_id: &str,
    params: &TrackerParams,
) -> Result<()> {
    let params_value = serde_json::to_value(params)?;
    let augment_args = json!({
        "id": tracker_id,
        "merge": true,
        "params": params_value,
    });
    crate::librarian::tools::augment::ArtifactAugment
        .call(ctx, augment_args)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_tracker(
    ctx: &ToolContext,
    args: &AuditArgs,
    findings: Vec<Finding>,
    warnings: Vec<ParseWarning>,
    n_files: usize,
    now: chrono::DateTime<chrono::Utc>,
    commit: &str,
    offline: Vec<String>,
) -> Result<(String, String)> {
    let (tracker_id, tracker_path) = if let Some(id) = &args.tracker_id {
        let path = find_tracker_path(ctx, id)
            .await
            .unwrap_or_else(|| "docs/trackers/doc-ref-audit.md".to_string());
        (id.clone(), path)
    } else {
        ensure_default_tracker(ctx).await?
    };

    let prior = load_tracker_params(ctx, &tracker_id)
        .await
        .unwrap_or_default();
    let n_refs_found = findings.len() as u32;
    let mut new_params = merger::merge_into_tracker(findings, &prior, now, commit);
    new_params.scan_meta.last_scan_at = Some(now.to_rfc3339());
    new_params.scan_meta.last_scan_commit = Some(commit.to_string());
    new_params.scan_meta.n_files_scanned = n_files as u32;
    new_params.scan_meta.n_refs_found = n_refs_found;
    new_params.scan_meta.degraded = !offline.is_empty();
    new_params.scan_meta.lsp_languages_offline = offline;
    new_params.parse_warnings = warnings;

    write_tracker_params(ctx, &tracker_id, &new_params).await?;
    Ok((tracker_id, tracker_path))
}

async fn find_tracker_path(ctx: &ToolContext, id: &str) -> Option<String> {
    let v = crate::librarian::tools::get::call(
        ctx,
        json!({
            "id": id,
        }),
    )
    .await
    .ok()?;
    // get returns rel_path in the top-level object
    v.get("rel_path")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            // Fallback: derive from abs_path
            let abs = v.get("abs_path").and_then(|p| p.as_str())?;
            let project_root = ctx.current_project.as_ref().map(|p| p.abs_path.clone())?;
            std::path::Path::new(abs)
                .strip_prefix(&project_root)
                .map(|p| p.to_string_lossy().into_owned())
                .ok()
        })
}

fn collect_markdown_files(
    root: &std::path::Path,
    globs: &[String],
    excludes: &[String],
) -> Result<Vec<std::path::PathBuf>> {
    use ignore::WalkBuilder;
    let mut include_builder = globset::GlobSetBuilder::new();
    for g in globs {
        include_builder.add(globset::Glob::new(g).map_err(|e| {
            RecoverableError::with_hint(format!("bad glob {g}: {e}"), "fix glob syntax")
        })?);
    }
    let include_set = include_builder.build()?;

    let mut exclude_builder = globset::GlobSetBuilder::new();
    for g in excludes {
        exclude_builder.add(globset::Glob::new(g).map_err(|e| {
            RecoverableError::with_hint(format!("bad exclude glob {g}: {e}"), "fix glob syntax")
        })?);
    }
    let exclude_set = exclude_builder.build()?;

    let mut out = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        let entry = entry?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if include_set.is_match(rel) && !exclude_set.is_match(rel) {
            out.push(entry.path().to_path_buf());
        }
    }
    Ok(out)
}

fn build_response(
    findings: &[Finding],
    warnings: &[ParseWarning],
    offline: &[String],
    n_files: usize,
    tracker_id: Option<&str>,
    tracker_path: Option<&str>,
    fail_on: &str,
) -> Value {
    let cap = 50;
    let total = findings.len();
    let shown_findings: Vec<_> = findings.iter().take(cap).map(finding_to_json).collect();

    let overflow = if total > cap {
        let mut by_file: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for f in findings {
            *by_file.entry(f.candidate.md_file.clone()).or_insert(0) += 1;
        }
        json!({
            "shown": cap,
            "total": total,
            "by_file": by_file,
            "hint": format!(
                "narrow with paths=[...] or read full tracker at {}",
                tracker_path.unwrap_or("<no tracker>")
            ),
        })
    } else {
        Value::Null
    };

    let n_broken = findings
        .iter()
        .filter(|f| {
            matches!(
                f.resolution.verdict,
                Verdict::Missing
                    | Verdict::FileMissing
                    | Verdict::SymbolMissing
                    | Verdict::LineOob
                    | Verdict::AnchorMissing
            )
        })
        .count();
    let n_unknown = findings
        .iter()
        .filter(|f| f.resolution.verdict == Verdict::Unknown)
        .count();
    let n_resolved = findings
        .iter()
        .filter(|f| f.resolution.verdict == Verdict::Resolved)
        .count();

    let exit_code: i32 = match fail_on {
        "high"
            if findings.iter().any(|f| {
                f.resolution.severity == Severity::High
                    && !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External)
            }) =>
        {
            1
        }
        "any" if n_broken + n_unknown > 0 => 1,
        _ => 0,
    };

    json!({
        "n_files_scanned": n_files,
        "n_refs_found": findings.len(),
        "n_refs_resolved": n_resolved,
        "n_refs_broken": n_broken,
        "n_refs_unknown": n_unknown,
        "tracker_id": tracker_id,
        "tracker_path": tracker_path,
        "findings": shown_findings,
        "overflow": overflow,
        "parse_warnings": warnings,
        "scan_meta": {
            "degraded": !offline.is_empty(),
            "lsp_languages_offline": offline,
        },
        "exit_code": exit_code,
    })
}

fn finding_to_json(f: &Finding) -> Value {
    json!({
        "n": Value::Null,
        "md_file": f.candidate.md_file,
        "md_line": f.candidate.md_line,
        "raw_ref": f.candidate.raw_ref,
        "ref_kind": f.candidate.ref_kind,
        "verdict": f.resolution.verdict,
        "severity": f.resolution.severity,
        "severity_reason": f.resolution.severity_reason,
        "status": "open",
        "notes": f.resolution.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_smoke_ctx(root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: root.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            artifact_store: None,
            current_project: Some(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                umbrella: None,
            })),
        }
    }

    #[tokio::test]
    async fn smoke_scan_yields_zero_on_clean_repo() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "x = 1\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "See `foo.py`.\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "emit_tracker": false,
                "paths": ["docs/**/*.md"],
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["n_refs_broken"], 0);
        assert_eq!(result["exit_code"], 0);
    }

    #[tokio::test]
    async fn smoke_creates_tracker_when_absent() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "x = 1\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "See `foo.py`.\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "emit_tracker": true,
                "paths": ["docs/**/*.md"],
            }),
        )
        .await
        .unwrap();

        assert!(
            result["tracker_id"].as_str().is_some(),
            "tracker_id should be set when emit_tracker=true, got: {result}"
        );
        assert!(
            tmp.path().join("docs/trackers/doc-ref-audit.md").exists(),
            "tracker md file should be created on disk"
        );
    }

    #[tokio::test]
    async fn smoke_tracker_idempotent_on_second_run() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "See `foo.py`.\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let args = serde_json::json!({
            "emit_tracker": true,
            "paths": ["docs/**/*.md"],
        });

        let r1 = call(&ctx, args.clone()).await.unwrap();
        let r2 = call(&ctx, args).await.unwrap();

        let id1 = r1["tracker_id"].as_str().unwrap();
        let id2 = r2["tracker_id"].as_str().unwrap();
        assert_eq!(id1, id2, "second run should reuse same tracker id");
    }

    #[tokio::test]
    async fn outputguard_caps_findings_inline() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        let mut body = String::new();
        for i in 0..51 {
            body.push_str(&format!("`src/gone{i}.py`\n"));
        }
        std::fs::write(tmp.path().join("docs/spec.md"), body).unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "emit_tracker": false,
                "paths": ["docs/**/*.md"],
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            result["findings"].as_array().unwrap().len(),
            50,
            "findings should be capped at 50"
        );
        assert_eq!(
            result["overflow"]["total"], 51,
            "overflow.total should reflect all 51 findings"
        );
        // by_file is a BTreeMap serialized as a JSON object: {<path>: <count>}
        assert!(
            result["overflow"]["by_file"]["docs/spec.md"]
                .as_u64()
                .is_some(),
            "by_file should map docs/spec.md to a count; got overflow: {}",
            result["overflow"]
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn glob_explosion_returns_recoverable() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            std::fs::write(tmp.path().join(format!("doc{i}.md")), "x").unwrap();
        }
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        std::env::set_var("LIBRARIAN_AUDIT_MAX_FILES", "1");
        let err = call(&ctx, serde_json::json!({"paths": ["*.md"]}))
            .await
            .unwrap_err();
        std::env::remove_var("LIBRARIAN_AUDIT_MAX_FILES");
        assert!(
            format!("{err}").contains("cap") || format!("{err}").contains("files"),
            "error should mention the file cap; got: {err}"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn default_scan_excludes_docs_agents() {
        // H-6 (C): docs/agents/** lives in DEFAULT_AUDIT_EXCLUDES because the
        // files there describe reader-side paths (in the reader's repo) and
        // produce only FPs against the audited project. The default scan
        // must skip them.
        let tmp = TempDir::new().unwrap();
        let agents = tmp.path().join("docs/agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("copilot.md"), "# Copilot\n").unwrap();
        let other = tmp.path().join("docs/other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("guide.md"), "# Guide\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({"emit_tracker": false}))
            .await
            .unwrap();
        let n_scanned = result["n_files_scanned"].as_u64().unwrap();
        assert_eq!(
                n_scanned, 1,
                "default scan should exclude docs/agents/** — only docs/other/guide.md should be scanned"
            );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn explicit_paths_override_default_exclude() {
        // An explicit `paths` argument bypasses DEFAULT_AUDIT_EXCLUDES so
        // callers can opt back into auditing the excluded subtree on demand
        // (e.g. when a doc-author wants to verify their agent-onboarding
        // docs).
        let tmp = TempDir::new().unwrap();
        let agents = tmp.path().join("docs/agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(agents.join("copilot.md"), "# Copilot\n").unwrap();
        let other = tmp.path().join("docs/other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("guide.md"), "# Guide\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "paths": ["docs/**/*.md"],
                "emit_tracker": false
            }),
        )
        .await
        .unwrap();
        let n_scanned = result["n_files_scanned"].as_u64().unwrap();
        assert_eq!(
            n_scanned, 2,
            "explicit paths should override the default exclude — both files scanned"
        );
    }
}
