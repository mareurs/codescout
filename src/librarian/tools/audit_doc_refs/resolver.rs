use super::severity;
use super::{RefCandidate, RefKind, Resolution, Severity, Verdict};
use std::path::Path;

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    pub lsp: Option<std::sync::Arc<dyn crate::lsp::ops::LspProvider>>,
    pub degraded_languages: std::cell::RefCell<Vec<String>>,
    /// Basename → list of relative paths in the workspace. Used by
    /// `resolve_file_path` as a fallback when a bare basename (no `/`) doesn't
    /// literally exist at the repo root. Built once per audit run in
    /// `mod.rs::call`. Empty disables the fallback (treat misses as
    /// `Verdict::Missing` exactly as before).
    pub basename_index: std::collections::HashMap<String, Vec<std::path::PathBuf>>,
}

pub fn resolve_ref(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    match c.ref_kind {
        RefKind::FilePath => resolve_file_path(c, ctx),
        RefKind::FileLine => resolve_file_line(c, ctx),
        RefKind::FileSymbol => resolve_file_symbol(c, ctx),
        RefKind::ModulePath => resolve_module_path_v1(c, ctx),
        RefKind::Link => resolve_link(c, ctx),
    }
}

fn resolve_file_path(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    if c.raw_ref.starts_with("../") || c.raw_ref.starts_with('/') {
        return Resolution {
            verdict: Verdict::Unknown,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: Some("path outside active project; scope=umbrella required".to_string()),
        };
    }
    let path = ctx.repo_root.join(&c.raw_ref);
    if path.exists() {
        return Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }
    // Basename fallback: conversational mentions of files without their full
    // path (e.g. `docling_reader.py` instead of `src/mrv/readers/docling_reader.py`).
    // See `docs/issues/2026-05-17-audit-doc-refs-basename-false-positives.md`.
    if let Some(r) = try_basename_fallback(&c.raw_ref, ctx) {
        return r;
    }
    verdict_with_drops_for_ref(Verdict::Missing, &c.raw_ref, Path::new(&c.md_file), ctx)
}

/// Look up `raw_ref` in the basename index when it has no `/`. Returns
/// `Some(Resolution)` with `ResolvedBasename` (single hit) or
/// `AmbiguousBasename` (multiple hits); `None` means "no match — caller should
/// fall through to the default missing verdict".
fn try_basename_fallback(raw_ref: &str, ctx: &ResolveCtx<'_>) -> Option<Resolution> {
    if raw_ref.contains('/') {
        return None;
    }
    let matches = ctx.basename_index.get(raw_ref)?;
    match matches.len() {
        0 => None,
        1 => Some(Resolution {
            verdict: Verdict::ResolvedBasename,
            severity: Severity::Low,
            severity_reason: "basename_match",
            notes: Some(format!("resolved by basename to {}", matches[0].display())),
        }),
        n => {
            let preview: Vec<String> = matches
                .iter()
                .take(5)
                .map(|p| p.display().to_string())
                .collect();
            let suffix = if n > 5 {
                format!(", and {} more", n - 5)
            } else {
                String::new()
            };
            Some(Resolution {
                verdict: Verdict::AmbiguousBasename,
                severity: Severity::Med,
                severity_reason: "basename_ambiguous",
                notes: Some(format!(
                    "basename matches {} files: {}{}",
                    n,
                    preview.join(", "),
                    suffix
                )),
            })
        }
    }
}
/// The single indexed path for a slash-less basename, if exactly one exists.
///
/// `try_basename_fallback` answers "what verdict?"; this answers "which file?",
/// which is what a line-range check needs before it can range-check anything.
fn unique_basename_path(raw_ref: &str, ctx: &ResolveCtx<'_>) -> Option<std::path::PathBuf> {
    if raw_ref.contains('/') {
        return None;
    }
    match ctx.basename_index.get(raw_ref)?.as_slice() {
        [only] => Some(ctx.repo_root.join(only)),
        _ => None,
    }
}

fn resolve_file_line(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, line_str) = c.raw_ref.rsplit_once(':').expect("file_line invariant");
    let direct = ctx.repo_root.join(path_str);
    // Same basename shorthand `resolve_file_path` already accepts, for the same
    // reason: an ADR citing `process.rs:66-135` means the repo's one
    // `process.rs`, and range-checking it is more useful than calling it
    // missing. Without this the two ref kinds disagree about identical path
    // parts — see
    // `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`.
    let path = if direct.exists() {
        direct
    } else if let Some(resolved) = unique_basename_path(path_str, ctx) {
        resolved
    } else {
        return try_basename_fallback(path_str, ctx).unwrap_or_else(|| {
            verdict_with_drops_for_ref(Verdict::Missing, path_str, Path::new(&c.md_file), ctx)
        });
    };
    // Parse `N` (single line) or `N-M` (line range). Range covers two checks:
    // both endpoints in bounds, and start <= end.
    let (start, end): (u32, u32) = if let Some((a, b)) = line_str.split_once('-') {
        (a.parse().unwrap_or(0), b.parse().unwrap_or(0))
    } else {
        let n: u32 = line_str.parse().unwrap_or(0);
        (n, n)
    };
    let total = std::fs::read_to_string(&path)
        .map(|s| s.lines().count() as u32)
        .unwrap_or(0);
    if start == 0 || end == 0 || start > end || end > total {
        verdict_with_drops(Verdict::LineOob, Path::new(&c.md_file), ctx.memory_globs)
    } else {
        Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        }
    }
}

fn resolve_link(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    if c.raw_ref.starts_with("http://") || c.raw_ref.starts_with("https://") {
        return Resolution {
            verdict: Verdict::External,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }
    if let Some(anchor) = c.raw_ref.strip_prefix('#') {
        let target_md = ctx.repo_root.join(&c.md_file);
        if let Ok(text) = std::fs::read_to_string(&target_md) {
            let slugs: std::collections::HashSet<String> = text
                .lines()
                .filter_map(|l| {
                    let trimmed = l.trim_start();
                    if trimmed.starts_with('#') {
                        Some(slugify(trimmed.trim_start_matches('#').trim()))
                    } else {
                        None
                    }
                })
                .collect();
            if slugs.contains(&slugify(anchor)) {
                return Resolution {
                    verdict: Verdict::Resolved,
                    severity: Severity::Low,
                    severity_reason: "policy_default",
                    notes: None,
                };
            }
        }
        return verdict_with_drops(
            Verdict::AnchorMissing,
            Path::new(&c.md_file),
            ctx.memory_globs,
        );
    }
    // fs-scheme link → resolve per markdown convention.
    //
    // Refs starting with `./` or `../` are *explicitly* relative to the
    // file containing the link. Joining them to repo_root would resolve
    // `docs/agents/foo.md` → `../manual/X.md` against repo_root and miss
    // the intended `docs/manual/X.md`. Anchor to md_file.parent() so the
    // OS's `..` lookup walks from the right base.
    //
    // Refs that don't carry an explicit `./` or `../` prefix stay rooted
    // at repo_root — the project convention in this codebase is to write
    // such refs as repo-root-relative even inside docs that live one
    // subdirectory deep.
    let path = if c.raw_ref.starts_with("./") || c.raw_ref.starts_with("../") {
        let md_dir = ctx
            .repo_root
            .join(&c.md_file)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| ctx.repo_root.to_path_buf());
        md_dir.join(&c.raw_ref)
    } else {
        ctx.repo_root.join(&c.raw_ref)
    };
    if path.exists() {
        return Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }
    if let Some(r) = try_basename_fallback(&c.raw_ref, ctx) {
        return r;
    }
    verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
}

/// v1: module_path candidates are reported as Unknown without consulting LSP.
/// Workspace symbol search for dotted module identifiers is Phase 2.
/// We do NOT push to `degraded_languages` here because no language detection
/// is meaningful for bare dotted identifiers.
fn resolve_module_path_v1(_c: &RefCandidate, _ctx: &ResolveCtx<'_>) -> Resolution {
    Resolution {
        verdict: Verdict::Unknown,
        severity: Severity::Low,
        severity_reason: "policy_default",
        notes: None,
    }
}
fn resolve_file_symbol(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    // Accept both `path::symbol` (Rust-style) and `path:symbol` (Python-style)
    // separators. Try `::` first so a trailing colon doesn't leak into the
    // path part on Rust refs like `src/foo.rs::extract_surface`.
    let (path_str, name) = c
        .raw_ref
        .rsplit_once("::")
        .or_else(|| c.raw_ref.rsplit_once(':'))
        .expect("file_symbol invariant: raw_ref must contain a `::` or `:` separator");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(
            Verdict::FileMissing,
            Path::new(&c.md_file),
            ctx.memory_globs,
        );
    }
    let lang = detect_language(path_str);
    let Some(lsp) = ctx.lsp.clone() else {
        ctx.degraded_languages.borrow_mut().push(lang.to_string());
        return Resolution {
            verdict: Verdict::Unknown,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    };
    let lang_id = lang.to_string();
    let repo_root = ctx.repo_root.to_path_buf();
    // `client_within_budget` bounds cold-start latency instead of an unbounded
    // wait a hung/slow LSP could otherwise stall on (see
    // docs/issues/2026-07-05-audit-doc-refs-lsp-stubbed-off.md).
    let fut = async move {
        let client = crate::lsp::client_within_budget(
            lsp,
            &lang_id,
            &repo_root,
            None,
            crate::lsp::LSP_FIRST_CALL_BUDGET,
        )
        .await?;
        client.document_symbols(&path, &lang_id).await.ok()
    };
    // `resolve_file_symbol` is called both from plain sync unit tests (no
    // ambient runtime — safe to spin a throwaway one) and from `call()`,
    // which already runs inside the server's own tokio runtime. Blocking
    // that runtime's worker thread on a second, nested `Runtime::block_on`
    // panics ("Cannot start a runtime from within a runtime") — this branch
    // was previously unreachable dead code because `ctx.lsp` was always
    // `None` in production. `block_in_place` hands the wait to a
    // Tokio-managed blocking thread instead, which requires a
    // multi-threaded runtime — true for the server's `#[tokio::main]`
    // (default flavor); the one test exercising this branch is marked
    // `#[tokio::test(flavor = "multi_thread")]` for the same reason.
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(fut),
    };
    match result {
        Some(syms) => {
            if syms.iter().any(|s| s.name == name) {
                Resolution {
                    verdict: Verdict::Resolved,
                    severity: Severity::Low,
                    severity_reason: "policy_default",
                    notes: None,
                }
            } else {
                verdict_with_drops(
                    Verdict::SymbolMissing,
                    Path::new(&c.md_file),
                    ctx.memory_globs,
                )
            }
        }
        None => {
            ctx.degraded_languages.borrow_mut().push(lang.to_string());
            Resolution {
                verdict: Verdict::Unknown,
                severity: Severity::Low,
                severity_reason: "policy_default",
                notes: None,
            }
        }
    }
}

fn detect_language(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") => "typescript",
        Some("kt") => "kotlin",
        Some("java") => "java",
        Some("go") => "go",
        _ => "unknown",
    }
}

fn verdict_with_drops(
    verdict: Verdict,
    md_file: &Path,
    memory_globs: &[globset::Glob],
) -> Resolution {
    let base = severity::default_severity(verdict);
    let (sev, reason) = severity::apply_drops(md_file, base, memory_globs);
    Resolution {
        verdict,
        severity: sev,
        severity_reason: reason,
        notes: None,
    }
}
/// How much a ref vouches for being a *repo-relative* path, which is what a
/// `missing` verdict actually claims.
///
/// Two independent ways to be a guess:
/// 1. **No separator at all** — the token earned `FilePath` from a known
///    extension alone. `RELEASES.md` in an ADR about an upstream repository.
/// 2. **A root segment absent from the repo** — the token is almost certainly
///    relative to something else. `librarian/catalog.db` is a suffix of
///    `dirs::data_local_dir()`; there is no `librarian/` at the repo root.
///    Meanwhile `docs/issues/x.md` has a real `docs/`, so it is judged strictly
///    and a miss there is genuine drift.
fn path_evidence(raw_ref: &str, ctx: &ResolveCtx<'_>) -> severity::PathEvidence {
    let Some((root, _)) = raw_ref.split_once('/') else {
        return severity::PathEvidence::Inferred;
    };
    if ctx.repo_root.join(root).exists() {
        severity::PathEvidence::Structural
    } else {
        severity::PathEvidence::Inferred
    }
}

/// `verdict_with_drops`, then cap the band when the ref's classification as a
/// repo-relative path was inferred rather than structural.
///
/// Used only on the two path-classification paths (`file_path`, `file_line`).
/// A markdown link target is deliberately NOT routed through here: `[x](y.md)`
/// is explicit author intent to point at a local file, so a missing link target
/// stays `high` even without a slash.
fn verdict_with_drops_for_ref(
    verdict: Verdict,
    raw_ref: &str,
    md_file: &Path,
    ctx: &ResolveCtx<'_>,
) -> Resolution {
    let base = severity::default_severity(verdict);
    let (sev, reason) = severity::apply_drops(md_file, base, ctx.memory_globs);
    let (sev, reason) =
        severity::cap_inferred_path(verdict, path_evidence(raw_ref, ctx), sev, reason);
    Resolution {
        verdict,
        severity: sev,
        severity_reason: reason,
        notes: None,
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'a'..='z' | '0'..='9' => Some(c),
            ' ' | '-' | '_' => Some('-'),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tools::audit_doc_refs::{RefKind, RefPosition};
    use tempfile::TempDir;

    fn cand(raw: &str, md: &str, kind: RefKind) -> RefCandidate {
        RefCandidate {
            md_file: md.to_string(),
            md_line: 1,
            raw_ref: raw.to_string(),
            ref_kind: kind,
            position: RefPosition::InlineSpan,
        }
    }

    fn ctx<'a>(root: &'a Path, globs: &'a [globset::Glob]) -> ResolveCtx<'a> {
        ResolveCtx {
            repo_root: root,
            memory_globs: globs,
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn resolver_resolved_for_existing_path() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "x = 1\n").unwrap();
        let r = resolve_ref(
            &cand("foo.py", "docs/spec.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    #[test]
    fn resolver_missing_for_absent_path() {
        let tmp = TempDir::new().unwrap();
        // A real `src/` with the file absent: the ref is structural AND
        // rooted in the repo, so this is the gating band. The bare-name and
        // absent-root variants are capped — see
        // `resolver_still_missing_when_basename_not_in_index` and
        // `resolver_ref_with_absent_root_segment_is_capped`.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let r = resolve_ref(
            &cand("src/gone.py", "docs/spec.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::High);
        assert_eq!(r.severity_reason, "policy_default");
    }

    #[test]
    fn resolver_line_oob_for_short_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "a\nb\nc\n").unwrap();
        let r = resolve_ref(
            &cand("foo.py:99", "docs/spec.md", RefKind::FileLine),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::LineOob);
        assert_eq!(r.severity, Severity::Med);
    }

    #[test]
    fn resolver_external_for_https_link() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_ref(
            &cand("https://example.com", "docs/spec.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::External);
    }
    #[test]
    fn severity_drops_one_level_in_archive() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_ref(
            &cand("gone.py", "docs/archive/old.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::Med);
        assert_eq!(r.severity_reason, "archive_drop");
    }

    #[test]
    fn severity_drops_two_levels_in_memory() {
        let tmp = TempDir::new().unwrap();
        let globs: Vec<_> = crate::librarian::tools::audit_doc_refs::severity::DEFAULT_MEMORY_GLOBS
            .iter()
            .map(|g| globset::Glob::new(g).unwrap())
            .collect();
        let r = resolve_ref(
            &cand("gone.py", ".buddy/memory/foo.md", RefKind::FilePath),
            &ctx(tmp.path(), &globs),
        );
        assert_eq!(r.severity, Severity::Low);
        assert_eq!(r.severity_reason, "memory_drop");
    }

    #[test]
    fn severity_reason_populated_for_every_finding() {
        let tmp = TempDir::new().unwrap();
        // FilePath: plain path raw ref
        let r = resolve_ref(
            &cand("gone.py", "docs/spec.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert!(
            !r.severity_reason.is_empty(),
            "FilePath severity_reason empty"
        );
        // FileLine: must have a colon-separated line number to satisfy the resolver invariant
        let r = resolve_ref(
            &cand("gone.py:1", "docs/spec.md", RefKind::FileLine),
            &ctx(tmp.path(), &[]),
        );
        assert!(
            !r.severity_reason.is_empty(),
            "FileLine severity_reason empty"
        );
        // Link: external URL
        let r = resolve_ref(
            &cand("https://example.com/gone", "docs/spec.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert!(!r.severity_reason.is_empty(), "Link severity_reason empty");
    }

    // ── Task 8: LSP-backed FileSymbol tests ──────────────────────────────────

    #[test]
    fn resolver_symbol_missing_for_renamed_symbol() {
        use crate::lsp::mock::{MockLspClient, MockLspProvider};
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
        // MockLspClient::new() returns Ok(vec![]) for any document_symbols call.
        let lsp = MockLspProvider::with_client(MockLspClient::new());
        let c = cand("foo.rs:renamed_baz", "docs/spec.md", RefKind::FileSymbol);
        let r = resolve_ref(
            &c,
            &ResolveCtx {
                repo_root: tmp.path(),
                memory_globs: &[],
                lsp: Some(lsp),
                degraded_languages: Default::default(),
                basename_index: std::collections::HashMap::new(),
            },
        );
        assert_eq!(r.verdict, Verdict::SymbolMissing);
    }

    #[test]
    fn resolver_unknown_when_lsp_offline() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
        let c = cand("foo.rs:bar", "docs/spec.md", RefKind::FileSymbol);
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
        };
        let r = resolve_ref(&c, &ctx);
        assert_eq!(r.verdict, Verdict::Unknown);
        assert!(ctx.degraded_languages.borrow().iter().any(|l| l == "rust"));
    }

    #[test]
    fn resolver_prefers_disk_truth_on_lsp_lag() {
        use crate::lsp::mock::{MockLspClient, MockLspProvider};
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
        // LSP returns no symbols (simulates lag / stale index) but file exists on disk.
        let lsp = MockLspProvider::with_client(MockLspClient::new());
        let c = cand("foo.rs:bar", "docs/spec.md", RefKind::FileSymbol);
        let r = resolve_ref(
            &c,
            &ResolveCtx {
                repo_root: tmp.path(),
                memory_globs: &[],
                lsp: Some(lsp),
                degraded_languages: Default::default(),
                basename_index: std::collections::HashMap::new(),
            },
        );
        // File exists on disk but LSP returned no symbols → SymbolMissing, NOT Unknown.
        // This encodes the "prefer disk truth" rule: the LSP responded (not offline),
        // so an empty symbol list means the symbol genuinely isn't there.
        assert_eq!(r.verdict, Verdict::SymbolMissing);
    }

    #[test]
    fn resolver_resolved_when_lsp_returns_matching_symbol() {
        use crate::lsp::mock::{MockLspClient, MockLspProvider};
        use crate::lsp::{SymbolInfo, SymbolKind};
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
        let client = MockLspClient::new().with_symbols(
            tmp.path().join("foo.rs"),
            vec![SymbolInfo {
                name: "bar".to_string(),
                name_path: "bar".to_string(),
                kind: SymbolKind::Function,
                file: tmp.path().join("foo.rs"),
                start_line: 0,
                end_line: 0,
                range_start_line: None,
                start_col: 0,
                children: vec![],
                detail: None,
            }],
        );
        let lsp = MockLspProvider::with_client(client);
        let c = cand("foo.rs:bar", "docs/spec.md", RefKind::FileSymbol);
        let r = resolve_ref(
            &c,
            &ResolveCtx {
                repo_root: tmp.path(),
                memory_globs: &[],
                lsp: Some(lsp),
                degraded_languages: Default::default(),
                basename_index: std::collections::HashMap::new(),
            },
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    // ── Task 8b: path-outside-project + anchor link resolution ───────────────

    #[test]
    fn resolver_unknown_for_path_outside_project() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_ref(
            &cand(
                "../other-repo/src/foo.py",
                "docs/spec.md",
                RefKind::FilePath,
            ),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Unknown);
        assert!(r
            .notes
            .as_deref()
            .unwrap_or("")
            .contains("outside active project"));
    }

    #[test]
    fn resolver_anchor_resolved_when_heading_present() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("spec.md"), "# Top\n\n## Auth\n\nbody\n").unwrap();
        let r = resolve_ref(
            &cand("#auth", "docs/spec.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    #[test]
    fn resolver_anchor_missing_when_heading_absent() {
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("spec.md"), "# Top\n\n## Auth\n\nbody\n").unwrap();
        let r = resolve_ref(
            &cand("#missing-section", "docs/spec.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::AnchorMissing);
        assert_eq!(r.severity, Severity::Med);
    }

    /// Basename fallback: bare basename (no `/`) that exists as exactly one
    /// file in the workspace resolves with `ResolvedBasename` + severity Low.
    /// Closes the false-positive class called out in
    /// `docs/issues/2026-05-17-audit-doc-refs-basename-false-positives.md`.
    #[test]
    fn resolver_resolves_by_basename_when_unique() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/mrv/readers")).unwrap();
        std::fs::write(
            tmp.path().join("src/mrv/readers/docling_reader.py"),
            "# stub\n",
        )
        .unwrap();

        let mut index = std::collections::HashMap::new();
        index.insert(
            "docling_reader.py".to_string(),
            vec![std::path::PathBuf::from(
                "src/mrv/readers/docling_reader.py",
            )],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };
        let r = resolve_ref(
            &cand("docling_reader.py", "docs/adr/0006.md", RefKind::FilePath),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::ResolvedBasename);
        assert_eq!(r.severity, Severity::Low);
        assert_eq!(r.severity_reason, "basename_match");
        assert!(
            r.notes
                .as_ref()
                .is_some_and(|n| n.contains("src/mrv/readers/docling_reader.py")),
            "notes should cite the resolved path: {:?}",
            r.notes
        );
    }
    /// `file_line` gets the same basename shorthand as `file_path`, and then
    /// range-checks the file it resolved to. Real case: an ADR citing
    /// `process.rs:66-135` for codescout's own spawn path was reported
    /// missing at severity high while `src/lsp/mux/process.rs` sat on disk.
    #[test]
    fn resolver_file_line_resolves_via_unique_basename() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/lsp/mux")).unwrap();
        std::fs::write(tmp.path().join("src/lsp/mux/process.rs"), "x\n".repeat(200)).unwrap();

        let mut index = std::collections::HashMap::new();
        index.insert(
            "process.rs".to_string(),
            vec![std::path::PathBuf::from("src/lsp/mux/process.rs")],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };

        let r = resolve_ref(
            &cand("process.rs:66-135", "docs/adrs/mux.md", RefKind::FileLine),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::Resolved, "notes: {:?}", r.notes);
        assert_eq!(r.severity, Severity::Low);
    }

    /// The range is still checked against the *resolved* file, so a basename
    /// hit does not blanket-approve any line number.
    #[test]
    fn resolver_file_line_range_checked_against_resolved_basename() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/tiny.rs"), "a\nb\nc\n").unwrap();

        let mut index = std::collections::HashMap::new();
        index.insert(
            "tiny.rs".to_string(),
            vec![std::path::PathBuf::from("src/tiny.rs")],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };

        let r = resolve_ref(
            &cand("tiny.rs:900", "docs/spec.md", RefKind::FileLine),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::LineOob);
    }

    /// An ambiguous basename in a `file_line` ref reports as ambiguous
    /// (med) rather than missing (high) — same verdict `file_path` gives.
    #[test]
    fn resolver_file_line_reports_ambiguous_basename() {
        let tmp = TempDir::new().unwrap();
        let mut index = std::collections::HashMap::new();
        index.insert(
            "mod.rs".to_string(),
            vec![
                std::path::PathBuf::from("src/a/mod.rs"),
                std::path::PathBuf::from("src/b/mod.rs"),
            ],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };

        let r = resolve_ref(&cand("mod.rs:12", "docs/spec.md", RefKind::FileLine), &ctx);
        assert_eq!(r.verdict, Verdict::AmbiguousBasename);
        assert_eq!(r.severity, Severity::Med);
    }

    #[test]
    fn resolver_ambiguous_when_basename_matches_multiple_files() {
        let tmp = TempDir::new().unwrap();
        let mut index = std::collections::HashMap::new();
        index.insert(
            "__init__.py".to_string(),
            vec![
                std::path::PathBuf::from("src/foo/__init__.py"),
                std::path::PathBuf::from("src/bar/__init__.py"),
                std::path::PathBuf::from("src/baz/__init__.py"),
            ],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };
        let r = resolve_ref(
            &cand("__init__.py", "docs/spec.md", RefKind::FilePath),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::AmbiguousBasename);
        assert_eq!(r.severity, Severity::Med);
        assert_eq!(r.severity_reason, "basename_ambiguous");
        assert!(
            r.notes.as_ref().is_some_and(|n| n.contains("3 files")),
            "notes should report match count: {:?}",
            r.notes
        );
    }

    /// Still reported, but capped below the CI gate. A bare name earns
    /// `FilePath` from nothing but its extension, so "not found anywhere"
    /// does not prove drift — it may name a file in another repository
    /// (`RELEASES.md` in an ADR about upstream kotlin-lsp).
    #[test]
    fn resolver_still_missing_when_basename_not_in_index() {
        let tmp = TempDir::new().unwrap();
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
        };
        let r = resolve_ref(
            &cand("nonexistent.py", "docs/spec.md", RefKind::FilePath),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::Med, "inferred path must not gate CI");
        assert_eq!(r.severity_reason, "inferred_path");
    }

    /// The other half of the contract: a ref rooted in a directory that
    /// really exists is unambiguously local, so a miss stays in the gating
    /// band. Without this the cap could silently disarm the whole audit.
    #[test]
    fn resolver_structural_missing_ref_stays_high() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs/adrs")).unwrap();
        let r = resolve_ref(
            &cand("docs/adrs/gone.md", "docs/spec.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::High);
    }

    /// A ref whose root segment does not exist in the repo is relative to
    /// something else, not to the repo root. Real case: an ADR documenting
    /// `dirs::data_local_dir().join("librarian/catalog.db")` — the audit read
    /// `librarian/catalog.db` as repo-relative and gated CI on it, while the
    /// repo's own librarian lives at `src/librarian/`.
    #[test]
    fn resolver_ref_with_absent_root_segment_is_capped() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/librarian")).unwrap();
        let r = resolve_ref(
            &cand("librarian/catalog.db", "docs/adrs/x.md", RefKind::FilePath),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::Med);
        assert_eq!(r.severity_reason, "inferred_path");
    }

    #[test]
    fn resolver_skips_basename_fallback_when_ref_contains_slash() {
        // Path-prefixed ref (`src/foo/bar.py`) that doesn't exist on disk
        // must remain Missing — even if `bar.py` is in the basename index,
        // we don't second-guess an explicit path.
        let tmp = TempDir::new().unwrap();
        // A real `src/` so the ref is repo-rooted and stays in the gating
        // band; see `resolver_ref_with_absent_root_segment_is_capped`.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let mut index = std::collections::HashMap::new();
        index.insert(
            "bar.py".to_string(),
            vec![std::path::PathBuf::from("other/place/bar.py")],
        );
        let ctx = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: index,
        };
        let r = resolve_ref(
            &cand("src/foo/bar.py", "docs/spec.md", RefKind::FilePath),
            &ctx,
        );
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::High);
    }

    #[test]
    fn resolver_resolved_for_in_bounds_line_range() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
        let r = resolve_ref(
            &cand("foo.rs:3-7", "docs/spec.md", RefKind::FileLine),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    #[test]
    fn resolver_line_oob_for_range_past_eof() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "1\n2\n3\n").unwrap();
        let r = resolve_ref(
            &cand("foo.rs:50-60", "docs/spec.md", RefKind::FileLine),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::LineOob);
    }

    #[test]
    fn resolver_line_oob_for_inverted_range() {
        // start > end should be LineOob, not silently accepted.
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.rs"), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").unwrap();
        let r = resolve_ref(
            &cand("foo.rs:7-3", "docs/spec.md", RefKind::FileLine),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::LineOob);
    }

    #[test]
    fn resolver_link_with_dot_dot_resolves_relative_to_md_file_parent() {
        // Markdown convention: `[text](../path)` is relative to the file
        // containing the link, not to the project root. Pre-fix the resolver
        // joined repo_root + raw_ref unconditionally, so `../manual/X.md` from
        // `docs/agents/foo.md` looked for `<repo>/../manual/X.md` (outside the
        // repo) instead of `<repo>/docs/manual/X.md` — the file the doc
        // author actually meant.
        let tmp = TempDir::new().unwrap();
        // The md_file's parent dir must exist so the OS can traverse `..`
        // segments during `Path::exists`. In real audits this is guaranteed
        // (we're auditing a file that exists). Mirror that here.
        let agents_dir = tmp.path().join("docs/agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("claude-code.md"), "# agents\n").unwrap();
        let target_dir = tmp.path().join("docs/manual/src/concepts");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join("superpowers.md"), "# Superpowers\n").unwrap();

        let r = resolve_ref(
            &cand(
                "../manual/src/concepts/superpowers.md",
                "docs/agents/claude-code.md",
                RefKind::Link,
            ),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(
            r.verdict,
            Verdict::Resolved,
            "../-relative link should resolve against md_file's parent; got {:?}",
            r
        );
    }

    #[test]
    fn resolver_link_with_dot_slash_resolves_relative_to_md_file_parent() {
        // Same convention applies to explicit `./` links — relative to the
        // md_file's parent directory.
        let tmp = TempDir::new().unwrap();
        let docs = tmp.path().join("docs/agents");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("sibling.md"), "# Sibling\n").unwrap();

        let r = resolve_ref(
            &cand("./sibling.md", "docs/agents/claude-code.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    #[test]
    fn resolver_link_without_explicit_relative_prefix_still_repo_root_rooted() {
        // Regression guard: refs that do NOT start with `./` or `../` continue
        // to resolve against repo_root, matching the existing project
        // convention of writing repo-root-relative paths inside docs that
        // live one subdir deep.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("lib.rs"), "fn main() {}\n").unwrap();

        let r = resolve_ref(
            &cand("src/lib.rs", "docs/agents/claude-code.md", RefKind::Link),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(
            r.verdict,
            Verdict::Resolved,
            "repo-root-relative link should keep resolving against repo_root; got {:?}",
            r
        );
    }
}
