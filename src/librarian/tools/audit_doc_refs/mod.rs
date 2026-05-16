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
    Resolved,
    Missing,
    FileMissing,
    SymbolMissing,
    LineOob,
    AnchorMissing,
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

use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct AuditArgs {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub emit_tracker: bool,
    #[serde(default)]
    pub tracker_id: Option<String>,
    #[serde(default)]
    pub severity_overrides: Option<Value>,
    #[serde(default = "default_fail_on")]
    pub fail_on: String,
}

fn default_true() -> bool {
    true
}
fn default_fail_on() -> String {
    "never".to_string()
}

pub const DEFAULT_AUDIT_GLOBS: &[&str] = &[
    "docs/**/*.md",
    "CLAUDE.md",
    "**/CLAUDE.md",
    "**/README.md",
];

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

    let globs: Vec<String> = args
        .paths
        .clone()
        .unwrap_or_else(|| DEFAULT_AUDIT_GLOBS.iter().map(|s| s.to_string()).collect());

    let files = collect_markdown_files(&repo_root, &globs)?;

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

    let resolve_ctx = resolver::ResolveCtx {
        repo_root: &repo_root,
        memory_globs: &memory_globs,
        lsp: None, // v1: LSP not plumbed through ToolContext yet
        degraded_languages: Default::default(),
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

    let response = build_response(
        &all_findings,
        &all_warnings,
        &offline,
        files.len(),
        None, // tracker_id — Task 15 wires
        None, // tracker_path — Task 15 wires
        &args.fail_on,
    );
    Ok(response)
}

fn collect_markdown_files(
    root: &std::path::Path,
    globs: &[String],
) -> Result<Vec<std::path::PathBuf>> {
    use ignore::WalkBuilder;
    let mut set_builder = globset::GlobSetBuilder::new();
    for g in globs {
        set_builder.add(globset::Glob::new(g).map_err(|e| {
            RecoverableError::with_hint(format!("bad glob {g}: {e}"), "fix glob syntax")
        })?);
    }
    let set = set_builder.build()?;
    let mut out = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        let entry = entry?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if set.is_match(rel) {
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
            *by_file
                .entry(f.candidate.md_file.clone())
                .or_insert(0) += 1;
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
                    && !matches!(
                        f.resolution.verdict,
                        Verdict::Resolved | Verdict::External
                    )
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
        "md_file": f.candidate.md_file,
        "md_line": f.candidate.md_line,
        "raw_ref": f.candidate.raw_ref,
        "ref_kind": f.candidate.ref_kind,
        "verdict": f.resolution.verdict,
        "severity": f.resolution.severity,
        "severity_reason": f.resolution.severity_reason,
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
}
