// src/librarian/tools/audit_doc_refs/mod.rs
use serde::{Deserialize, Serialize};

pub mod code_comments;
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
    /// Bare prose — a path written without backticks or link markup.
    ///
    /// Produced only by `parser::parse_prose_refs`, which only the source-comment
    /// path calls. Markdown deliberately does not scan prose: in a document, a
    /// path mentioned in a sentence is as often an example as a citation, and
    /// admitting them would drown the report. In a **code comment** the reverse
    /// holds — `// see docs/issues/foo.md` is a pointer, and the backticks are
    /// a style habit rather than a signal of intent.
    Prose,
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

/// Why a finding carries the severity it does.
///
/// This was a bare `&'static str` until 2026-08-17, set from thirteen literals scattered
/// across `severity.rs` and `resolver.rs`. `Verdict` and `Severity` beside it were already
/// serde enums — discoverable, greppable, exhaustive — while the one field a reader most
/// needs in order to interpret a report had no enumerable domain at all. The cost was
/// measured on the way in: a filter written against a guessed `"ok"` printed resolved refs
/// as problems, and a `med` cap was attributed to `issues_drop` when `inferred_path` was
/// doing the work. Both are the same defect, which is that the domain could only be
/// learned by reading every producer.
///
/// Serialization is unchanged: `rename_all = "snake_case"` reproduces the exact strings
/// the literals emitted, so no consumer of the JSON sees a difference.
///
/// [`Self::as_str`] and [`Self::explain`] are both exhaustive matches on purpose — adding
/// a variant without describing it is a compile error, which is the property a const array
/// of names cannot give.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeverityReason {
    /// No cap applied — the verdict's own default band.
    PolicyDefault,
    /// Resolved through the basename index: the ref named a file without its path, and
    /// exactly one file in the repo carried that basename.
    BasenameMatch,
    /// The basename matched more than one file, so the ref is ambiguous rather than broken.
    BasenameAmbiguous,
    /// The LSP answered without the symbol but tree-sitter finds it — the server is
    /// mid-index, so the scan is degraded rather than the reference wrong.
    LspBehindAst,
    /// The ref's claim to be repo-relative was INFERRED, not syntactic: no separator at
    /// all, or a root segment absent from the repo. Keeps `high` for "definitely a local
    /// path, definitely gone".
    InferredPath,
    /// Cited inside a fenced code block, where a path is often illustrative rather than a
    /// reference.
    CodeBlock,
    /// `.gitignore` declares the target generated or machine-local, so its absence from a
    /// clean checkout is expected.
    GitignoredPath,
    /// Found in a source code comment rather than prose.
    CodeCommentCapped,
    /// In a released section of a changelog — history, which must not be rewritten to
    /// satisfy a linter.
    ReleasedHistory,
    /// The citing document lives under an `archive/` directory: a retired document citing
    /// a moved path is expected and must not gate.
    ArchiveDrop,
    /// The citing document is a memory file.
    MemoryDrop,
    /// The citing document is a bug file under `docs/issues/`.
    IssuesDrop,
    /// The citing document lives in a dated-record directory (plans, research, reviews).
    HistoricalDrop,
}

impl SeverityReason {
    /// Every variant. Kept in step with the type by [`Self::as_str`]'s exhaustive match:
    /// a new variant fails to compile there long before anyone notices this array.
    pub const ALL: [SeverityReason; 13] = [
        SeverityReason::PolicyDefault,
        SeverityReason::BasenameMatch,
        SeverityReason::BasenameAmbiguous,
        SeverityReason::LspBehindAst,
        SeverityReason::InferredPath,
        SeverityReason::CodeBlock,
        SeverityReason::GitignoredPath,
        SeverityReason::CodeCommentCapped,
        SeverityReason::ReleasedHistory,
        SeverityReason::ArchiveDrop,
        SeverityReason::MemoryDrop,
        SeverityReason::IssuesDrop,
        SeverityReason::HistoricalDrop,
    ];

    /// The wire value — identical to the string literal this variant replaced.
    pub fn as_str(self) -> &'static str {
        match self {
            SeverityReason::PolicyDefault => "policy_default",
            SeverityReason::BasenameMatch => "basename_match",
            SeverityReason::BasenameAmbiguous => "basename_ambiguous",
            SeverityReason::LspBehindAst => "lsp_behind_ast",
            SeverityReason::InferredPath => "inferred_path",
            SeverityReason::CodeBlock => "code_block",
            SeverityReason::GitignoredPath => "gitignored_path",
            SeverityReason::CodeCommentCapped => "code_comment_capped",
            SeverityReason::ReleasedHistory => "released_history",
            SeverityReason::ArchiveDrop => "archive_drop",
            SeverityReason::MemoryDrop => "memory_drop",
            SeverityReason::IssuesDrop => "issues_drop",
            SeverityReason::HistoricalDrop => "historical_drop",
        }
    }

    /// One sentence a reader can act on, without opening the producer.
    ///
    /// This is the field's real payload. A reason name tells you which branch fired; it
    /// does not tell you whether the finding is your problem — and the difference between
    /// "your severity was capped because the path was inferred" and "capped because the
    /// citing file is archived" changes what you do next.
    pub fn explain(self) -> &'static str {
        match self {
            SeverityReason::PolicyDefault => "no cap applied — the verdict's default band",
            SeverityReason::BasenameMatch => {
                "resolved through the basename index — the ref omitted its path and exactly \
                 one file matched"
            }
            SeverityReason::BasenameAmbiguous => {
                "the basename matches several files — ambiguous, not broken; name the path"
            }
            SeverityReason::LspBehindAst => {
                "the LSP is mid-index and tree-sitter disagrees with it — the scan is \
                 degraded, not the reference"
            }
            SeverityReason::InferredPath => {
                "the ref's claim to be repo-relative was inferred, not syntactic — capped \
                 so `high` still means definitely-local-and-definitely-gone"
            }
            SeverityReason::CodeBlock => {
                "cited inside a fenced code block, where a path is often illustrative"
            }
            SeverityReason::GitignoredPath => {
                "`.gitignore` declares the target generated or machine-local, so its \
                 absence is expected"
            }
            SeverityReason::CodeCommentCapped => "found in a source comment rather than prose",
            SeverityReason::ReleasedHistory => {
                "a released changelog section — history, which must not be rewritten to \
                 satisfy a linter"
            }
            SeverityReason::ArchiveDrop => {
                "the citing document is archived — a retired doc citing a moved path is \
                 expected and must not gate"
            }
            SeverityReason::MemoryDrop => "the citing document is a memory file",
            SeverityReason::IssuesDrop => "the citing document is a bug file under `docs/issues/`",
            SeverityReason::HistoricalDrop => {
                "the citing document is a dated record (plans, research, reviews)"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub verdict: Verdict,
    pub severity: Severity,
    pub severity_reason: SeverityReason,
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
    /// Coverage was incomplete. Accurate as-is, and deliberately unchanged: a mid-index
    /// server really does cost resolutions, which is why
    /// `docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md` made
    /// this true for that case.
    pub degraded: bool,
    /// Languages whose resolutions were incomplete.
    ///
    /// Renamed from `lsp_languages_offline`, which was an honest description of only one
    /// of the three causes and a false claim about the other two — most sharply about a
    /// server that answered. The `alias` keeps trackers written under the old name
    /// loading; nothing needs to migrate, since every scan overwrites the field.
    ///
    /// See `docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`.
    #[serde(default, alias = "lsp_languages_offline")]
    pub lsp_languages_degraded: Vec<String>,
    /// Per-language reasons behind `degraded`, deduplicated and sorted.
    ///
    /// This is the field that makes the flag actionable. `lsp_behind_index` says the
    /// server is up and mid-index, so re-running resolves it; `no_server` says no
    /// re-run will help. The old surface could not express that difference, so a reader
    /// had to guess — and guessed wrong.
    #[serde(default)]
    pub degraded_causes: std::collections::BTreeMap<String, Vec<String>>,
}

/// A scan's degradation state: which languages came back incomplete, and why.
///
/// Bundled rather than threaded as two parallel arguments so the language list and the
/// causes behind it cannot drift apart — the whole defect this replaces came from a
/// surface that carried the languages and dropped the reasons.
#[derive(Debug, Clone, Default)]
pub struct Degradation {
    /// Languages whose resolutions were incomplete, sorted and deduplicated.
    pub languages: Vec<String>,
    /// Per-language causes, sorted and deduplicated.
    /// See [`resolver::DegradedCause`] for what each one means and what it implies.
    pub causes: std::collections::BTreeMap<String, Vec<String>>,
}

impl Degradation {
    /// Whether coverage was incomplete — the `scan_meta.degraded` flag.
    pub fn is_degraded(&self) -> bool {
        !self.languages.is_empty()
    }
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
    pub paths: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub emit_tracker: bool,
    #[serde(default)]
    pub tracker_id: Option<String>,
    #[serde(default = "default_fail_on")]
    pub fail_on: String,
    #[serde(default)]
    pub scope: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_fail_on() -> String {
    "never".to_string()
}

/// The default scan set.
///
/// `CHANGELOG.md` and `CONTRIBUTING.md` are enumerated explicitly because the rest
/// of this list covers `docs/**` plus two conventional filenames — a root-level
/// project doc that is neither `CLAUDE.md` nor `README.md` was never in scope, and
/// there was no exclusion to notice, only an absence. Both went unscanned for the
/// project's whole life; the first default run over them found a real broken
/// `docs/issues/…` ref whose file had moved to `docs/issues/archive/`.
///
/// Adding `CHANGELOG.md` is only safe alongside `cap_released_history` — a shipped
/// release section names paths that were correct at that version, and correcting
/// them would falsify what the release shipped. See that function.
pub const DEFAULT_AUDIT_GLOBS: &[&str] = &[
    "docs/**/*.md",
    "CLAUDE.md",
    "**/CLAUDE.md",
    "**/README.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
];

/// The `fail_on` values `build_response` actually honors — the single source of
/// truth for the CLI's `--fail-on` value set, so the two cannot drift.
///
/// They drifted once: the engine gained `med` and `low` on 2026-07-05
/// (`docs/issues/archive/2026-07-05-audit-doc-refs-fail-on-doc-mismatch.md`),
/// and the CLI went on refusing both — with help text citing the already-fixed
/// bug as the reason, which is the worst form of stale doc because it reads as
/// a live pointer. `fail_on_values_are_exactly_what_build_response_honors`
/// pins both directions.
///
/// `any` is a pre-existing undocumented alias for `low`, kept so no existing
/// caller's behavior changes.
pub const FAIL_ON_VALUES: [&str; 5] = ["high", "med", "low", "any", "never"];

/// Source files whose **documentation nodes** join the default scan.
///
/// Citations live in code — `// see docs/issues/….md` — and were invisible for
/// the project's whole life because the include set was markdown. Measured
/// 2026-08-15: 111 unique bug files cited from 94 `.rs` files, 95 of them
/// pointing at a path archived out from under the comment. See SD-1 in
/// docs/trackers/structural-debt-refactor.md.
///
/// One extension per grammar codescout vendors, and no more: a language with
/// no grammar cannot have its comments found, so listing it would add walk
/// cost for a guaranteed zero. `ignore::WalkBuilder` honours `.gitignore`, so
/// vendored trees (`node_modules`, `target`) never reach this.
///
/// These are matched by the SAME include/exclude machinery as the markdown
/// globs; what makes a source file behave differently is the per-file branch
/// in `call`, which routes anything with a tree-sitter grammar through
/// `scan_code_comments` instead of parsing it whole.
pub const DEFAULT_AUDIT_CODE_GLOBS: &[&str] = &[
    "**/*.rs",
    "**/*.py",
    "**/*.go",
    "**/*.ts",
    "**/*.tsx",
    "**/*.js",
    "**/*.jsx",
    "**/*.java",
    "**/*.kt",
    "**/*.kts",
    "**/*.sh",
    "**/*.bash",
    "**/*.html",
    "**/*.css",
    "**/*.scss",
    "**/*.less",
];

/// Patterns matched against rel paths AFTER the include set. Matching files
/// are dropped from the scan. Used to exclude content that IS path-shaped
/// markdown but represents reader-side references (agent-onboarding docs
/// describe files in the *reader's* repo, not codescout's) which would only
/// produce noise. Applied only when `paths` is left as the default — an
/// explicit `paths` argument is honoured verbatim so callers can opt back
/// into auditing excluded subtrees on demand. See H-6 (C) in
/// docs/trackers/codescout-usage-hookify.md.
///
/// `docs/lessons/**` joins for the same reason one layer out: a lessons
/// retrospective records what using codescout on *another* project taught us, so
/// every path it cites belongs to that project's tree. Resolving them here is a
/// category error, not drift — one such file supplied 28 of 50 high-severity
/// findings on 2026-08-06, every one of them correct prose
/// (`src/pc_kb/corpus.py`, `docs/trackers/mrv-chat-watch/**`, …). See
/// docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md.
///
/// The `src/prompts` payload files are agent-facing instruction text whose
/// example paths (`src/foo.rs`, `docs/plans/foo.md`, `docs/trackers/foo.md`)
/// are teaching placeholders, not citations — 26 broken refs, 9 of them high,
/// measured 2026-08-08. They are already silent today only because no entry in
/// `DEFAULT_AUDIT_GLOBS` happens to reach them; listing them here turns an
/// accident of the include set into a decision, so widening the globs later
/// cannot pull the placeholders into the gate.
///
/// `src/prompts/README.md` is deliberately NOT excluded, which is why the
/// payloads are enumerated rather than matched with `src/prompts/**`. It is the
/// one file in that directory whose refs are real citations to real codescout
/// files (28 refs, 24 resolved, 0 high), and `**/README.md` already scans it —
/// excluding the subtree wholesale would drop live coverage in order to
/// document a silence that never covered this file. `globset` has no negation,
/// so a broad pattern plus a carve-out is not expressible.
pub const DEFAULT_AUDIT_EXCLUDES: &[&str] = &[
    "docs/agents/**",
    "docs/lessons/**",
    "src/prompts/guides/**",
    "src/prompts/source.md",
    "src/prompts/memory-templates.md",
    "src/prompts/workspace_onboarding_prompt.md",
];

pub const MAX_FILES_DEFAULT: usize = 10_000;

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: AuditArgs = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("audit_doc_refs: bad args: {e}"),
            "see librarian(action=\"audit_doc_refs\") input schema",
        )
    })?;

    if let Some(scope) = args.scope.as_deref() {
        if scope != "project" {
            return Err(RecoverableError::with_hint(
                "audit_doc_refs is project-scoped in v1",
                "call from within the target project, or omit `scope`",
            ));
        }
    }

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
            DEFAULT_AUDIT_GLOBS
                .iter()
                .chain(DEFAULT_AUDIT_CODE_GLOBS.iter())
                .map(|s| s.to_string())
                .collect(),
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
    enforce_file_cap(files.len(), max_files)?;

    let memory_globs: Vec<_> = severity::DEFAULT_MEMORY_GLOBS
        .iter()
        .map(|g| globset::Glob::new(g).unwrap())
        .collect();

    let basename_index = build_basename_index(&repo_root);
    // The matcher is only half the answer — see `build_tracked_dirs`. Building the
    // index set only when a `.gitignore` exists keeps the git open off the path of
    // repos that have none.
    let gitignore = build_gitignore(&repo_root).map(|matcher| resolver::TrackedIgnore {
        tracked_dirs: build_tracked_dirs(&repo_root),
        matcher,
    });

    let resolve_ctx = resolver::ResolveCtx {
        repo_root: &repo_root,
        memory_globs: &memory_globs,
        lsp: Some(ctx.lsp.clone()),
        degraded_languages: Default::default(),
        basename_index,
        gitignore,
    };

    let mut all_findings: Vec<Finding> = Vec::new();
    let mut all_warnings: Vec<ParseWarning> = Vec::new();

    for md in &files {
        let text = std::fs::read_to_string(md).unwrap_or_default();
        let rel = md
            .strip_prefix(&repo_root)
            .unwrap_or(md.as_path())
            .to_path_buf();

        // A file codescout has a grammar for is scanned through its
        // DOCUMENTATION NODES ONLY, never whole. `detect_language` alone is not
        // the test — it answers `Some("markdown")` for `.md` — so the
        // discriminator is "has a tree-sitter grammar", which markdown does not.
        //
        // Whole-file parsing of source is not merely noisy, it is meaningless:
        // handing `.rs` to the markdown parser yields 33,105 "refs" because it
        // reads `tokio::sync::Semaphore` as a link. This branch is also what
        // makes an explicit `paths=["src/**/*.rs"]` argument behave sanely
        // rather than reporting that garbage.
        let grammar_lang =
            crate::ast::detect_language(md).filter(|l| crate::ast::get_ts_language(l).is_some());
        if let Some(language) = grammar_lang {
            all_findings.extend(scan_code_comments(&text, language, &rel, &resolve_ctx));
            continue;
        }

        // Computed per file, not per ref: a changelog has one boundary and the
        // scan is line-ordered anyway.
        let history_from = severity::released_history_boundary(&text, &rel);
        // Markdown has no language, and `DottedModules` is exactly what the
        // classifier did before `PathSyntax` existed — this surface's verdicts
        // are deliberately unchanged.
        let (cands, warns) = parser::parse_refs(&text, &rel, parser::PathSyntax::DottedModules);
        for c in cands {
            let mut r = resolver::resolve_ref(&c, &resolve_ctx);
            let (sev, reason) = severity::cap_released_history(
                r.verdict,
                c.md_line,
                history_from,
                r.severity,
                r.severity_reason,
            );
            r.severity = sev;
            r.severity_reason = reason;
            all_findings.push(Finding {
                candidate: c,
                resolution: r,
            });
        }
        all_warnings.extend(warns);
    }

    // Split the (language, cause) pairs into the two shapes the surfaces want: the flat
    // language list the tracker template renders, and the per-language cause map that
    // makes `degraded` actionable — `lsp_behind_index` means re-running helps, `no_server`
    // means it will not.
    let degradation = {
        let pairs = resolve_ctx.degraded_languages.borrow();
        let mut causes: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (lang, cause) in pairs.iter() {
            causes
                .entry(lang.clone())
                .or_default()
                .push(cause.as_str().to_string());
        }
        for v in causes.values_mut() {
            v.sort();
            v.dedup();
        }
        // The map is already keyed by language and BTreeMap-ordered, so the flat list
        // comes out sorted and deduplicated for free.
        Degradation {
            languages: causes.keys().cloned().collect(),
            causes,
        }
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
            degradation.clone(),
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
        &degradation,
        files.len(),
        tracker_id.as_deref(),
        tracker_path.as_deref(),
        &args.fail_on,
    )?;
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
/// Compile the repo-root `.gitignore` for the gitignored-path severity cap.
///
/// Thin alias over [`crate::util::gitignore::build_root_gitignore`], which carries
/// the root-file-only rationale and the failure policy. Shared with
/// `memory::anchors`, whose remedy differs — it refuses to record an ignored path
/// rather than capping a finding about one — but whose question is the same.
fn build_gitignore(repo_root: &std::path::Path) -> Option<ignore::gitignore::Gitignore> {
    crate::util::gitignore::build_root_gitignore(repo_root)
}

/// Forward-slash relative directories holding at least one file in git's index —
/// every ancestor included — plus `""` for the repo root.
///
/// `.gitignore` has no effect on paths already tracked, so the matcher alone cannot
/// answer "would git ever track this"; see `resolver::is_gitignored`, which needs both
/// halves. `/.github/` is ignored in this repo while five files beneath it are tracked.
///
/// `Repository::open` rather than `discover`, deliberately: the set must be keyed to
/// the same root the matcher was built for, and `discover` could walk up to an
/// enclosing repo whose index paths would not line up with `repo_root`. This matches
/// `git_head_commit` above. Any failure yields an empty set, which restores the prior
/// matcher-only behaviour rather than failing the audit — the same trade
/// `build_gitignore` makes.
///
/// Keys are `String`, not `PathBuf`: git index paths are always forward-slash, and so
/// are the doc refs they get compared against, whereas `PathBuf` equality on Windows
/// would not equate the two separator forms.
fn build_tracked_dirs(repo_root: &std::path::Path) -> std::collections::HashSet<String> {
    let mut dirs = std::collections::HashSet::new();
    let Ok(repo) = git2::Repository::open(repo_root) else {
        return dirs;
    };
    let Ok(index) = repo.index() else {
        return dirs;
    };
    for entry in index.iter() {
        let Ok(rel) = std::str::from_utf8(&entry.path) else {
            continue;
        };
        // Anything tracked at all makes the repo root a tracked directory.
        dirs.insert(String::new());
        let mut cur = rel;
        while let Some((parent, _)) = cur.rsplit_once('/') {
            // Already present means its ancestors are too — every chain walks to the
            // root — so there is nothing left to add above this point.
            if !dirs.insert(parent.to_string()) {
                break;
            }
            cur = parent;
        }
    }
    dirs
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
        "tags": ["codescout", "doc-ref-audit"],
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
    if let Err(e) = crate::librarian::tools::augment::call(ctx, augment_args).await {
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
    crate::librarian::tools::augment::call(ctx, augment_args).await?;
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
    degradation: Degradation,
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
    new_params.scan_meta.degraded = degradation.is_degraded();
    new_params.scan_meta.lsp_languages_degraded = degradation.languages;
    new_params.scan_meta.degraded_causes = degradation.causes;
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

/// Refuse a glob that matched more files than the cap allows.
///
/// Pure — takes the count and the cap rather than reading `LIBRARIAN_AUDIT_MAX_FILES`
/// itself, so the "glob explosion is RECOVERABLE, not a hard failure" contract can be
/// tested without `set_var`. See
/// `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
fn enforce_file_cap(file_count: usize, max_files: usize) -> Result<()> {
    if file_count > max_files {
        return Err(RecoverableError::with_hint(
            format!("audit_doc_refs: glob matched {file_count} files (cap {max_files})"),
            "tighten `paths` glob or set LIBRARIAN_AUDIT_MAX_FILES",
        ));
    }
    Ok(())
}

/// Scan one source file's documentation nodes for citations.
///
/// The pipeline below the text source is unchanged — the same `parse_refs`,
/// the same `resolve_ref`. Only two things differ from the markdown path, and
/// both are deliberate:
///
/// 1. **Line offsetting.** `parse_refs` numbers lines from the start of the
///    text it is given, which here is one comment, not the file. The ref's
///    line is rebased onto the file so a finding points where a reader can go.
/// 2. **Severity is forced to `Med`.** A citation rotting inside a comment is
///    real drift and should be visible, but it must not gate CI at
///    `--fail-on high` — a contributor who archives a bug file should not
///    break the build for everyone via a comment they never touched. Promote
///    to full severity later if the surface earns it; that is a policy change
///    worth making on evidence rather than on the first day.
///
/// `released_history_boundary` is deliberately NOT applied: it exists to stop
/// a shipped CHANGELOG section being "corrected" into falsehood, and source
/// comments carry no release history.
fn scan_code_comments(
    text: &str,
    language: &str,
    rel: &std::path::Path,
    resolve_ctx: &resolver::ResolveCtx,
) -> Vec<Finding> {
    let mut out = Vec::new();
    // The one thing classification needs from the language: whether a dotted
    // token like `a.b.c` can be a module path at all. See `PathSyntax`.
    let syntax = parser::PathSyntax::for_language(Some(language));
    for block in code_comments::extract_comments(text, language) {
        // Warnings are dropped rather than collected: they describe markdown
        // fence structure, and a comment is a fragment, so every one would be
        // an artefact of slicing rather than a fact about the file.
        let (mut cands, _warns) = parser::parse_refs(&block.text, rel, syntax);
        // Prose too — see `parse_prose_refs`. Only 140 of this repo's 699
        // in-source citations are backticked, so code spans alone would see a
        // fifth of them. Deduped on (line, raw_ref) because a path that IS
        // backticked is found by both passes and must be reported once.
        let seen: std::collections::HashSet<(u32, String)> = cands
            .iter()
            .map(|c| (c.md_line, c.raw_ref.clone()))
            .collect();
        cands.extend(
            parser::parse_prose_refs(&block.text, rel, syntax)
                .into_iter()
                .filter(|c| !seen.contains(&(c.md_line, c.raw_ref.clone()))),
        );
        for mut c in cands {
            c.md_line = block.line + c.md_line.saturating_sub(1);
            let mut r = resolver::resolve_ref(&c, resolve_ctx);
            let (sev, reason) = severity::cap_code_comment(r.severity, r.severity_reason);
            r.severity = sev;
            r.severity_reason = reason;
            out.push(Finding {
                candidate: c,
                resolution: r,
            });
        }
    }
    out
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

/// Which summary counter a verdict contributes to.
///
/// `bucket`'s match is deliberately **exhaustive** — no `_` arm — so adding a `Verdict`
/// variant is a compile error until it has been bucketed. The three counters used to be
/// three independent filters covering 7 of the 10 variants, which left
/// `ResolvedBasename`, `AmbiguousBasename` and `External` in none of them: `n_refs_found`
/// (itself only `findings.len()`) did not equal their sum, and 2,426 of 47,094 refs on
/// this repo were invisible in the summary. A test can *assert* a partition; an
/// exhaustive match **is** one.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Bucket {
    /// The doc's claim holds — the reference points at something real.
    Resolved,
    /// Actionable: a reader following this lands nowhere useful.
    Broken,
    /// The resolver could not form an opinion. Evidence for neither side.
    Unknown,
}

/// The single definition of "did this reference resolve", used by the summary counters,
/// the display ranking, and the exit code alike. Those were three separate `matches!`
/// expressions and they did not agree.
fn bucket(verdict: Verdict) -> Bucket {
    match verdict {
        // `ResolvedBasename` sits here by decision (2026-08-07). It IS a successful
        // resolution: the audit found exactly one file with that basename and says which
        // one in `notes`, and `docs/RELEASE.md` step 5 already describes it as OK.
        // Counting it as unresolved made `--fail-on low`/`any` exit 1 on a working
        // reference — 3 of 44 findings on `docs/RELEASE.md` alone.
        Verdict::Resolved | Verdict::ResolvedBasename | Verdict::External => Bucket::Resolved,
        // `AmbiguousBasename` is a doc defect the author fixes by adding a path prefix,
        // and `default_severity` already bands it `Med` alongside every other verdict
        // here — so this is where it was implicitly grouped all along.
        Verdict::Missing
        | Verdict::FileMissing
        | Verdict::SymbolMissing
        | Verdict::LineOob
        | Verdict::AnchorMissing
        | Verdict::AmbiguousBasename => Bucket::Broken,
        Verdict::Unknown => Bucket::Unknown,
    }
}

fn build_response(
    findings: &[Finding],
    warnings: &[ParseWarning],
    degradation: &Degradation,
    n_files: usize,
    tracker_id: Option<&str>,
    tracker_path: Option<&str>,
    fail_on: &str,
) -> Result<Value> {
    let cap = 50;
    let total = findings.len();
    // Show the findings that DRIVE the exit code first. Plain `take(50)` in scan
    // order fronted the list with *resolved* refs, so a `--fail-on high` run could
    // exit 1 while every one of the 50 shown findings was `resolved`/`low` — a gate
    // that reports failure and hides the cause. Ranking by (unresolved, severity)
    // makes the output self-explaining; the exit code itself still considers every
    // finding, so this changes presentation only. Stable sort, so scan order is
    // preserved within a rank.
    // See docs/issues/archive/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md
    let mut ranked: Vec<&Finding> = findings.iter().collect();
    ranked.sort_by_key(|f| {
        let resolved = bucket(f.resolution.verdict) == Bucket::Resolved;
        let sev = match f.resolution.severity {
            Severity::High => 0u8,
            Severity::Med => 1,
            Severity::Low => 2,
        };
        (resolved, sev)
    });
    let shown_findings: Vec<_> = ranked
        .iter()
        .take(cap)
        .map(|f| finding_to_json(f))
        .collect();

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
                "shown findings are ordered most-severe-first, so any finding driving \
                 the exit code appears above; narrow with paths=[...] or read full \
                 tracker at {}",
                tracker_path.unwrap_or("<no tracker>")
            ),
        })
    } else {
        Value::Null
    };

    // These three now PARTITION `findings`, so `n_refs_found` == their sum by
    // construction. Asserted by `summary_counters_partition_every_verdict`.
    let count_in = |b: Bucket| {
        findings
            .iter()
            .filter(|f| bucket(f.resolution.verdict) == b)
            .count()
    };
    let n_broken = count_in(Bucket::Broken);
    let n_unknown = count_in(Bucket::Unknown);
    let n_resolved = count_in(Bucket::Resolved);

    // A finding "counts" toward a fail_on threshold unless it RESOLVED, regardless of the
    // severity it carries. Shares `bucket` with the counters and the ranking above, which
    // were three separate `matches!` expressions that disagreed: `ResolvedBasename` was
    // exempt from neither, so `--fail-on low`/`any` exited 1 on a successful basename
    // resolution that `docs/RELEASE.md` step 5 calls OK.
    let counts = |f: &Finding| -> Option<Severity> {
        if bucket(f.resolution.verdict) == Bucket::Resolved {
            None
        } else {
            Some(f.resolution.severity)
        }
    };
    let exit_code: i32 = match fail_on {
        "never" => 0,
        "high" => findings.iter().any(|f| counts(f) == Some(Severity::High)) as i32,
        "med" => findings
            .iter()
            .any(|f| matches!(counts(f), Some(Severity::High) | Some(Severity::Med)))
            as i32,
        // "any" is an undocumented pre-existing alias for "low" — kept so no
        // existing caller's behavior silently changes.
        "low" | "any" => findings.iter().any(|f| counts(f).is_some()) as i32,
        other => {
            return Err(RecoverableError::with_hint(
                format!("audit_doc_refs: unknown fail_on value `{other}`"),
                "valid values: high | med | low | never",
            ))
        }
    };

    // A legend for the reasons THIS run used, not for the whole enum. `severity_reason`
    // names the branch that fired; it does not say whether the finding is the reader's
    // problem, and learning that meant reading thirteen literals across two producers.
    // Derived from the shown findings so the legend stays proportional to the report —
    // dumping `SeverityReason::ALL` would hand back documentation to filter instead of
    // an answer.
    let severity_legend: serde_json::Map<String, Value> = shown_findings
        .iter()
        .filter_map(|f| f.get("severity_reason").and_then(|r| r.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|name| {
            SeverityReason::ALL
                .iter()
                .find(|r| r.as_str() == name)
                .map(|r| (name.to_string(), json!(r.explain())))
        })
        .collect();

    Ok(json!({
        "n_files_scanned": n_files,
        "n_refs_found": findings.len(),
        "n_refs_resolved": n_resolved,
        "n_refs_broken": n_broken,
        "n_refs_unknown": n_unknown,
        "tracker_id": tracker_id,
        "tracker_path": tracker_path,
        "findings": shown_findings,
        "severity_legend": severity_legend,
        "overflow": overflow,
        "parse_warnings": warnings,
        "scan_meta": {
            "degraded": degradation.is_degraded(),
            // Was `lsp_languages_offline`. The old key asserted "offline" about a server
            // the `lsp_behind_index` branch had just measured as ANSWERING — and a reader
            // acted on it, discarding a sound scan. `degraded_causes` carries the
            // distinction the flat list threw away.
            "lsp_languages_degraded": degradation.languages,
            "degraded_causes": degradation.causes,
        },
        "exit_code": exit_code,
    }))
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
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_smoke_ctx(root: std::path::PathBuf) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(CurrentProject {
                abs_path: root.clone(),
                git_root: root,
                main_root: None,
                umbrella: None,
            }))
            .build()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lsp_wiring_resolves_real_symbol_end_to_end() {
        use crate::lsp::{MockLspClient, MockLspProvider, SymbolInfo, SymbolKind};

        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "def bar():\n    pass\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "See `foo.py:bar`.\n").unwrap();

        let mock_client = MockLspClient::new().with_symbols(
            tmp.path().join("foo.py"),
            vec![SymbolInfo {
                name: "bar".to_string(),
                name_path: "bar".to_string(),
                kind: SymbolKind::Function,
                file: tmp.path().join("foo.py"),
                start_line: 0,
                end_line: 1,
                range_start_line: None,
                start_col: 0,
                children: vec![],
                detail: None,
            }],
        );

        let mut ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        ctx.lsp = MockLspProvider::with_client(mock_client);

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
            result["n_refs_broken"], 0,
            "a real symbol served by the LSP should resolve, not be flagged broken: {result}"
        );
        assert_eq!(
            result["n_refs_unknown"], 0,
            "the real LSP should have been consulted, not skipped as absent: {result}"
        );
        assert_eq!(
            result["scan_meta"]["degraded"], false,
            "a responsive LSP must not be reported as degraded: {result}"
        );
    }

    fn mk_finding(verdict: Verdict, severity: Severity) -> Finding {
        Finding {
            candidate: RefCandidate {
                md_file: "docs/spec.md".to_string(),
                md_line: 1,
                raw_ref: "src/gone.py".to_string(),
                ref_kind: RefKind::FilePath,
                position: RefPosition::InlineSpan,
            },
            resolution: Resolution {
                verdict,
                severity,
                severity_reason: SeverityReason::PolicyDefault,
                notes: None,
            },
        }
    }

    /// `n_refs_found` must equal the three buckets' sum.
    ///
    /// It did not. The counters were three independent filters covering 7 of the 10 `Verdict`
    /// variants, so `ResolvedBasename`, `AmbiguousBasename` and `External` were counted in
    /// `found` and in no bucket — 2,426 of 47,094 refs on this repo, invisible in the summary.
    /// `bucket`'s exhaustive match is the structural guard; this pins the arithmetic that guard
    /// is supposed to produce.
    #[test]
    fn summary_counters_partition_every_verdict() {
        let all = [
            Verdict::Resolved,
            Verdict::ResolvedBasename,
            Verdict::External,
            Verdict::Missing,
            Verdict::FileMissing,
            Verdict::SymbolMissing,
            Verdict::LineOob,
            Verdict::AnchorMissing,
            Verdict::AmbiguousBasename,
            Verdict::Unknown,
        ];
        let findings: Vec<Finding> = all.iter().map(|v| mk_finding(*v, Severity::Low)).collect();
        let r = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "never",
        )
        .unwrap();

        let found = r["n_refs_found"].as_u64().unwrap();
        let sum = r["n_refs_resolved"].as_u64().unwrap()
            + r["n_refs_broken"].as_u64().unwrap()
            + r["n_refs_unknown"].as_u64().unwrap();
        assert_eq!(found, all.len() as u64, "one finding per verdict was built");
        assert_eq!(
            sum, found,
            "the three counters must partition n_refs_found; resolved={} broken={} unknown={} \
             sum={sum} found={found}",
            r["n_refs_resolved"], r["n_refs_broken"], r["n_refs_unknown"]
        );
    }

    /// A basename resolution is a SUCCESS, and all three consumers of that judgement must agree.
    /// Before this, `ResolvedBasename` was counted in no bucket, ranked with the *unresolved*
    /// half, and counted toward the fail-on threshold — so `--fail-on low` exited 1 on a working
    /// reference that `docs/RELEASE.md` step 5 calls OK.
    #[test]
    fn resolved_basename_counts_as_resolved_everywhere() {
        let f = [mk_finding(Verdict::ResolvedBasename, Severity::Low)];
        let r = build_response(&f, &[], &Degradation::default(), 1, None, None, "low").unwrap();
        assert_eq!(
            r["exit_code"], 0,
            "a successful basename resolution must not trip --fail-on low"
        );
        assert_eq!(r["n_refs_resolved"], 1, "and it must count as resolved");
        assert_eq!(r["n_refs_broken"], 0);
        assert_eq!(r["n_refs_unknown"], 0);
    }

    /// The other side of that condition, so widening the exemption cannot quietly relax the gate.
    /// `AmbiguousBasename` is NOT a success — the basename matched more than one file, and the
    /// author fixes it by adding a path prefix — so it stays in `broken` and stays gating.
    #[test]
    fn ambiguous_basename_still_counts_against_the_gate() {
        let f = [mk_finding(Verdict::AmbiguousBasename, Severity::Med)];
        let r = build_response(&f, &[], &Degradation::default(), 1, None, None, "med").unwrap();
        assert_eq!(
            r["exit_code"], 1,
            "an ambiguous basename must still trip --fail-on med"
        );
        assert_eq!(r["n_refs_broken"], 1, "and it belongs in the broken bucket");
        assert_eq!(r["n_refs_resolved"], 0);
    }

    /// The CLI's `--fail-on` value set and the set `build_response` honors are
    /// the same list, so they cannot drift. They did drift once, for two months.
    #[test]
    fn fail_on_values_are_exactly_what_build_response_honors() {
        // Every listed value must be accepted. Kills "someone adds a value to
        // FAIL_ON_VALUES that the engine has no arm for".
        for v in FAIL_ON_VALUES {
            let r = build_response(
                &[mk_finding(Verdict::Missing, Severity::High)],
                &[],
                &Degradation::default(),
                1,
                None,
                None,
                v,
            );
            assert!(
                r.is_ok(),
                "FAIL_ON_VALUES lists `{v}` but build_response rejects it"
            );
        }

        // The other direction. Without this the loop above stays green when a
        // value is DELETED from the list — which is exactly how `med` and `low`
        // stayed unreachable from the CLI after the engine learned them.
        for required in ["high", "med", "low", "never"] {
            assert!(
                FAIL_ON_VALUES.contains(&required),
                "`{required}` is documented in the librarian input schema and honored \
                 by build_response, so the CLI must accept it"
            );
        }

        // An unlisted value must be REJECTED, not silently treated as `never` —
        // the original defect was a gate that reported success while gating
        // nothing.
        assert!(
            build_response(&[], &[], &Degradation::default(), 0, None, None, "medium").is_err(),
            "an unknown fail_on value must error rather than behave like `never`"
        );
    }

    /// `build_response` shows only the first 50 findings but computes `exit_code` over
    /// every one of them. Ranking by `(unresolved, severity)` before truncating is what
    /// keeps those two consumers agreeing: a `--fail-on high` run that exits 1 must SHOW
    /// the finding that drove it. Pre-fix, `findings.iter().take(50)` took scan order, so
    /// the gate could report failure with 50 `resolved` refs as its entire output.
    ///
    /// This fixture pins the **unresolved** half of the key. Every finding carries
    /// `Severity::High`, so a ranking that sorted on severity alone would be a stable
    /// no-op and strand the one broken ref at index 50 — outside the window.
    ///
    /// Regression guard for
    /// `docs/issues/archive/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`.
    #[test]
    fn ranking_surfaces_the_unresolved_finding_hidden_behind_a_full_window() {
        let mut findings: Vec<Finding> = (0..50)
            .map(|_| mk_finding(Verdict::Resolved, Severity::High))
            .collect();
        findings.push(mk_finding(Verdict::Missing, Severity::High));

        let r = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "high",
        )
        .expect("`high` is a valid fail_on value");

        assert_eq!(
            r["exit_code"], 1,
            "one Missing/High finding must fail a --fail-on high run"
        );

        let shown = r["findings"].as_array().expect("findings is an array");
        assert_eq!(
            shown.len(),
            50,
            "the cap still applies — ranking is presentation only"
        );
        assert_eq!(
            shown[0]["verdict"], "missing",
            "the finding that DROVE exit_code=1 must lead the window, else the gate \
             reports failure and hides its own cause; first shown verdict was {}",
            shown[0]["verdict"]
        );
    }

    /// The **severity** half of the same key. Every finding here is unresolved, so a
    /// ranking that sorted on the resolved/unresolved dimension alone would be a stable
    /// no-op and strand the single `High` at index 50 — the gate would again exit 1 while
    /// showing nothing but `Low`s.
    ///
    /// Paired with `ranking_surfaces_the_unresolved_finding_hidden_behind_a_full_window`:
    /// each fixture holds one dimension uniform so that dropping the *other* dimension
    /// from the sort key is observable. Neither test alone pins both.
    #[test]
    fn ranking_surfaces_the_high_severity_finding_among_low_severity_breakage() {
        let mut findings: Vec<Finding> = (0..50)
            .map(|_| mk_finding(Verdict::Missing, Severity::Low))
            .collect();
        findings.push(mk_finding(Verdict::Missing, Severity::High));

        let r = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "high",
        )
        .expect("`high` is a valid fail_on value");

        assert_eq!(
            r["exit_code"], 1,
            "one Missing/High finding must fail a --fail-on high run"
        );

        let shown = r["findings"].as_array().expect("findings is an array");
        assert_eq!(
            shown.len(),
            50,
            "the cap still applies — ranking is presentation only"
        );
        assert_eq!(
            shown[0]["severity"], "high",
            "the high-severity finding that drove exit_code=1 must lead the window; \
             first shown severity was {}",
            shown[0]["severity"]
        );
    }

    #[test]
    fn fail_on_never_is_always_zero() {
        let findings = vec![mk_finding(Verdict::Missing, Severity::High)];
        let result = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "never",
        )
        .unwrap();
        assert_eq!(result["exit_code"], 0);
    }

    #[test]
    fn fail_on_high_ignores_med_severity() {
        let findings = vec![mk_finding(Verdict::Missing, Severity::Med)];
        let result = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "high",
        )
        .unwrap();
        assert_eq!(
            result["exit_code"], 0,
            "a Med finding should not trip fail_on=high"
        );
    }

    #[test]
    fn fail_on_high_trips_on_high_severity() {
        let findings = vec![mk_finding(Verdict::Missing, Severity::High)];
        let result = build_response(
            &findings,
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "high",
        )
        .unwrap();
        assert_eq!(result["exit_code"], 1);
    }

    #[test]
    fn fail_on_med_trips_on_med_and_high_but_not_low() {
        let med = build_response(
            &[mk_finding(Verdict::Missing, Severity::Med)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "med",
        )
        .unwrap();
        assert_eq!(med["exit_code"], 1, "med finding should trip fail_on=med");

        let high = build_response(
            &[mk_finding(Verdict::Missing, Severity::High)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "med",
        )
        .unwrap();
        assert_eq!(
            high["exit_code"], 1,
            "high finding should also trip fail_on=med"
        );

        let low = build_response(
            &[mk_finding(Verdict::Missing, Severity::Low)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "med",
        )
        .unwrap();
        assert_eq!(
            low["exit_code"], 0,
            "low finding should not trip fail_on=med"
        );
    }

    #[test]
    fn fail_on_low_trips_on_any_unresolved_severity() {
        let result = build_response(
            &[mk_finding(Verdict::Unknown, Severity::Low)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "low",
        )
        .unwrap();
        assert_eq!(
            result["exit_code"], 1,
            "even a Low-severity finding should trip fail_on=low"
        );
    }

    #[test]
    fn fail_on_low_is_silent_when_all_resolved() {
        let result = build_response(
            &[mk_finding(Verdict::Resolved, Severity::Low)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "low",
        )
        .unwrap();
        assert_eq!(
            result["exit_code"], 0,
            "a Resolved verdict should never trip any fail_on level"
        );
    }

    #[test]
    fn fail_on_any_alias_matches_low_semantics() {
        let result = build_response(
            &[mk_finding(Verdict::Unknown, Severity::Low)],
            &[],
            &Degradation::default(),
            1,
            None,
            None,
            "any",
        )
        .unwrap();
        assert_eq!(
            result["exit_code"], 1,
            "undocumented `any` alias should keep working like `low`"
        );
    }

    #[test]
    fn fail_on_unknown_value_is_rejected() {
        let err = build_response(&[], &[], &Degradation::default(), 1, None, None, "critical")
            .unwrap_err();
        assert!(
            format!("{err}").contains("critical"),
            "error should name the bad value; got: {err}"
        );
    }

    /// The scan may report incomplete coverage; it may not report a live server as down.
    ///
    /// `lsp_behind_index` fires only on a branch where the server ANSWERED, and the old
    /// key name — `lsp_languages_offline` — asserted the opposite. That is not a
    /// cosmetic complaint: on 2026-08-16 a tracker-hygiene sweep read
    /// `lsp_languages_offline: ["rust"]`, concluded rust-analyzer was down, and threw
    /// away a scan whose numbers were fine.
    ///
    /// See `docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`.
    #[test]
    fn scan_meta_reports_degradation_without_calling_a_live_server_offline() {
        let degradation = Degradation {
            languages: vec!["rust".to_string()],
            causes: [("rust".to_string(), vec!["lsp_behind_index".to_string()])]
                .into_iter()
                .collect(),
        };
        let r = build_response(&[], &[], &degradation, 1, None, None, "never").unwrap();
        let meta = &r["scan_meta"];

        // `degraded` stays true — coverage really was incomplete, and the 2026-08-06 fix
        // that made it so is deliberately preserved.
        assert_eq!(meta["degraded"], true, "coverage was incomplete: {meta}");
        assert_eq!(meta["lsp_languages_degraded"][0], "rust");

        // The cause is what makes the flag actionable: a caller reading `lsp_behind_index`
        // knows the server is up and re-running will resolve it.
        assert_eq!(meta["degraded_causes"]["rust"][0], "lsp_behind_index");

        assert!(
            meta.get("lsp_languages_offline").is_none(),
            "the key that made the false claim must be gone, not merely joined: {meta}"
        );
        assert!(
            !meta.to_string().contains("offline"),
            "no part of scan_meta may call a server that answered offline: {meta}"
        );
    }

    /// The rename must not silently blank an existing tracker's recorded state.
    ///
    /// `ScanMeta` is deserialized from a tracker's persisted params, so dropping the
    /// `serde(alias)` would read every pre-rename tracker as "never degraded" — a
    /// second false claim, introduced by the fix for the first. Pinned here because
    /// nothing else would fail if the alias were removed.
    #[test]
    fn scan_meta_still_loads_a_tracker_written_under_the_old_field_name() {
        let old = serde_json::json!({
            "last_scan_at": "2026-08-06T00:00:00Z",
            "last_scan_commit": "deadbeef",
            "n_files_scanned": 276,
            "n_refs_found": 44,
            "degraded": true,
            "lsp_languages_offline": ["rust"],
        });
        let meta: ScanMeta = serde_json::from_value(old).expect("old params must still load");

        assert_eq!(
            meta.lsp_languages_degraded,
            vec!["rust".to_string()],
            "the alias must carry the old value across the rename"
        );
        assert!(meta.degraded, "the flag itself is unchanged by the rename");
        assert!(
            meta.degraded_causes.is_empty(),
            "a tracker written before causes existed has none — absent, not invented"
        );
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
    async fn scope_repo_is_rejected() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "hello\n").unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            serde_json::json!({"emit_tracker": false, "scope": "repo"}),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("project-scoped"),
            "error should explain audit_doc_refs is project-scoped; got: {err}"
        );
    }

    #[tokio::test]
    async fn scope_umbrella_is_rejected() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "hello\n").unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            serde_json::json!({"emit_tracker": false, "scope": "umbrella"}),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{err}").contains("project-scoped"),
            "error should explain audit_doc_refs is project-scoped; got: {err}"
        );
    }

    #[tokio::test]
    async fn scope_project_is_accepted() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "hello\n").unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({"emit_tracker": false, "scope": "project"}),
        )
        .await
        .unwrap();
        assert_eq!(result["exit_code"], 0);
    }

    #[tokio::test]
    async fn scope_absent_is_accepted() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "hello\n").unwrap();
        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({"emit_tracker": false}))
            .await
            .unwrap();
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

    #[test]
    fn glob_explosion_returns_recoverable() {
        // A glob that matches more files than the cap must be a RECOVERABLE error —
        // the caller can tighten the glob and retry — not a hard failure.
        //
        // Tested through the pure `enforce_file_cap` seam. It used to set
        // LIBRARIAN_AUDIT_MAX_FILES=1 with `set_var` (and needed `#[serial]` for it),
        // which is UB in a parallel test binary. See the note in config/global.rs.
        let err = enforce_file_cap(5, 1).unwrap_err();
        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "a glob explosion must be recoverable, not a hard failure; got: {err}"
        );
        let msg = format!("{err}");
        assert!(
            msg.contains("cap") && msg.contains('5'),
            "error should name the cap and the actual count; got: {msg}"
        );
    }

    #[test]
    fn within_the_cap_is_ok() {
        assert!(enforce_file_cap(1, 5).is_ok());
        assert!(enforce_file_cap(5, 5).is_ok(), "the cap itself is allowed");
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

    #[test]
    fn prompt_payload_excludes_survive_a_glob_widening_but_spare_the_readme() {
        // The four src/prompts entries are inert under today's
        // DEFAULT_AUDIT_GLOBS — no include pattern reaches that directory
        // except `**/README.md`, so the payloads are already silent. They
        // exist for the widening that would END that silence, so the widening
        // is what this test simulates: a default-scan assertion would pass
        // just as happily with the four entries deleted.
        let tmp = TempDir::new().unwrap();
        let prompts = tmp.path().join("src/prompts");
        std::fs::create_dir_all(prompts.join("guides")).unwrap();
        for payload in [
            "source.md",
            "memory-templates.md",
            "workspace_onboarding_prompt.md",
        ] {
            std::fs::write(prompts.join(payload), "# Payload\n").unwrap();
        }
        std::fs::write(prompts.join("guides/iron-laws-detail.md"), "# Guide\n").unwrap();
        std::fs::write(prompts.join("README.md"), "# Prompts\n").unwrap();

        let widened = vec!["src/prompts/**/*.md".to_string()];
        let excludes: Vec<String> = DEFAULT_AUDIT_EXCLUDES
            .iter()
            .map(|s| s.to_string())
            .collect();

        let kept: Vec<String> = collect_markdown_files(tmp.path(), &widened, &excludes)
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(tmp.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            kept,
            vec!["src/prompts/README.md"],
            "a widened include set must still drop the four payload patterns, \
             and must still keep the README — its refs are real citations"
        );

        // Discriminator: the same widening without the excludes pulls in all five.
        let unguarded = collect_markdown_files(tmp.path(), &widened, &[]).unwrap();
        assert_eq!(
            unguarded.len(),
            5,
            "sanity: the widened glob really does reach all five files, so the \
             assertion above is measuring the excludes and not the glob"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn default_scan_covers_changelog_and_contributing() {
        // Both are tracked root-level project docs that are neither CLAUDE.md nor
        // README.md, so neither was ever enumerated — an absence, with no exclusion
        // to notice. The first default run over them found a real broken ref.
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CHANGELOG.md"), "# Changelog\n").unwrap();
        std::fs::write(tmp.path().join("CONTRIBUTING.md"), "# Contributing\n").unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({"emit_tracker": false}))
            .await
            .unwrap();
        assert_eq!(
            result["n_files_scanned"].as_u64().unwrap(),
            2,
            "default scan must cover CHANGELOG.md and CONTRIBUTING.md"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn changelog_released_sections_drop_below_the_gate_but_unreleased_does_not() {
        // The whole reason CHANGELOG.md could not simply be added to the globs. A
        // shipped release section naming a since-moved path is accurate about the
        // past and unfixable without falsifying it, so it must not gate. The
        // `[Unreleased]` half describes the current tree and must keep gating —
        // this is the single test that keeps the cap from swallowing both.
        let tmp = TempDir::new().unwrap();
        // A real `src/` root, so both refs are judged as *structural* paths.
        // Without it `cap_inferred_path` caps them to med first and the two arms
        // become indistinguishable — the test would pass for the wrong reason.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("CHANGELOG.md"),
            "# Changelog\n\n\
                 ## [Unreleased]\n\n\
                 ### Fixed\n\n\
                 - live claim about `src/still_gone.rs`\n\n\
                 ## [1.0.0] — 2026-01-01\n\n\
                 ### Changed\n\n\
                 - shipped change to `src/history_gone.rs`\n",
        )
        .unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({"emit_tracker": false}))
            .await
            .unwrap();
        let findings = result["findings"].as_array().unwrap();

        let live = findings
            .iter()
            .find(|f| f["raw_ref"] == "src/still_gone.rs")
            .expect("unreleased ref must still be reported");
        assert_eq!(
            live["severity"], "high",
            "a ref in [Unreleased] must keep gating: {live}"
        );

        let historical = findings
            .iter()
            .find(|f| f["raw_ref"] == "src/history_gone.rs")
            .expect("released ref must still be reported, just below the gate");
        assert_eq!(
            historical["severity"], "med",
            "a ref in a released section must drop below the gate: {historical}"
        );
        assert_eq!(historical["severity_reason"], "released_history");
    }

    /// A report that names its own vocabulary. `severity_reason` tells a reader WHICH
    /// branch capped a finding; it does not tell them whether the finding is their
    /// problem, and the difference between "capped because the path was inferred" and
    /// "capped because the citing file is archived" changes what they do next. Learning
    /// that domain used to mean reading thirteen literals across two producers — which
    /// is how a `med` cap got attributed to `issues_drop` when `inferred_path` was doing
    /// the work, and how a filter written against a guessed `"ok"` printed resolved refs
    /// as problems.
    ///
    /// Derived from the findings actually SHOWN, not from `SeverityReason::ALL`. A legend
    /// listing all thirteen would be documentation the reader has to filter; one listing
    /// the three in front of them is an answer. The absent-reason assertion below is what
    /// pins that distinction.
    #[tokio::test]
    #[serial_test::serial]
    async fn the_report_explains_every_severity_reason_it_actually_used() {
        let tmp = TempDir::new().unwrap();
        // A real `src/` root so both refs read as structural paths — same reason the
        // changelog test above needs it, and without it both collapse to inferred_path
        // and the fixture yields only one reason.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("CHANGELOG.md"),
            "# Changelog\n\n\
                 ## [Unreleased]\n\n\
                 - live claim about `src/still_gone.rs`\n\n\
                 ## [1.0.0] — 2026-01-01\n\n\
                 - shipped change to `src/history_gone.rs`\n",
        )
        .unwrap();

        let ctx = mk_smoke_ctx(tmp.path().to_path_buf());
        let result = call(&ctx, serde_json::json!({"emit_tracker": false}))
            .await
            .unwrap();

        let legend = result["severity_legend"]
            .as_object()
            .expect("the report must carry a legend for its own severity_reason values");

        // Every reason present in the shown findings is explained.
        for f in result["findings"].as_array().unwrap() {
            let reason = f["severity_reason"].as_str().unwrap();
            let explanation = legend
                .get(reason)
                .unwrap_or_else(|| panic!("no legend entry for `{reason}`: {legend:?}"));
            assert!(
                explanation.as_str().is_some_and(|s| !s.is_empty()),
                "legend entry for `{reason}` must be a non-empty sentence"
            );
        }

        // This fixture produces both of these, so the legend must carry both.
        assert!(legend.contains_key("policy_default"), "{legend:?}");
        assert!(legend.contains_key("released_history"), "{legend:?}");

        // Discriminating: a legend that simply dumped SeverityReason::ALL would pass
        // every assertion above. Nothing in this fixture is a memory file, so the
        // reader must not be handed an entry for it.
        assert!(
            !legend.contains_key("memory_drop"),
            "the legend must describe the reasons this run used, not the whole enum: {legend:?}"
        );
    }

    #[test]
    fn released_history_boundary_only_applies_to_changelogs() {
        use std::path::Path;
        let text = "# Changelog\n\n## [Unreleased]\n\n- a\n\n## [1.0.0] — 2026-01-01\n";
        // Line 7, 1-based: the first version heading that is not [Unreleased].
        assert_eq!(
            severity::released_history_boundary(text, Path::new("CHANGELOG.md")),
            Some(7)
        );
        // Same text under any other name is not a changelog and gets no boundary —
        // otherwise every doc with a `## [x]` heading would silently stop gating.
        assert_eq!(
            severity::released_history_boundary(text, Path::new("docs/notes.md")),
            None
        );
        // A changelog with nothing released yet has no history to forgive.
        assert_eq!(
            severity::released_history_boundary(
                "# Changelog\n\n## [Unreleased]\n\n- a\n",
                Path::new("CHANGELOG.md")
            ),
            None
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
