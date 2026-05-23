use super::severity;
use super::{RefCandidate, RefKind, Resolution, Severity, Verdict};
use std::path::Path;

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    pub lsp: Option<&'a dyn crate::lsp::ops::LspProvider>,
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
    verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
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

fn resolve_file_line(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, line_str) = c.raw_ref.rsplit_once(':').expect("file_line invariant");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs);
    }
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
    // fs-scheme link → same as file_path
    let path = ctx.repo_root.join(&c.raw_ref);
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
    let Some(lsp) = ctx.lsp else {
        ctx.degraded_languages.borrow_mut().push(lang.to_string());
        return Resolution {
            verdict: Verdict::Unknown,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    };
    // Call the async LSP on a fresh runtime — the resolver is single-threaded per scan.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let lang_id = lang.to_string();
    let result = rt.block_on(async {
        let client = lsp
            .get_or_start(&lang_id, ctx.repo_root, None)
            .await
            .map_err(|e| e.to_string())?;
        let syms = client
            .document_symbols(&path, &lang_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(syms)
    });
    match result {
        Ok(syms) => {
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
        Err(_) => {
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
        let r = resolve_ref(
            &cand("gone.py", "docs/spec.md", RefKind::FilePath),
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
                lsp: Some(lsp.as_ref()),
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
                lsp: Some(lsp.as_ref()),
                degraded_languages: Default::default(),
                basename_index: std::collections::HashMap::new(),
            },
        );
        // File exists on disk but LSP returned no symbols → SymbolMissing, NOT Unknown.
        // This encodes the "prefer disk truth" rule: the LSP responded (not offline),
        // so an empty symbol list means the symbol genuinely isn't there.
        assert_eq!(r.verdict, Verdict::SymbolMissing);
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
        assert_eq!(r.severity, Severity::High);
    }

    #[test]
    fn resolver_skips_basename_fallback_when_ref_contains_slash() {
        // Path-prefixed ref (`src/foo/bar.py`) that doesn't exist on disk
        // must remain Missing — even if `bar.py` is in the basename index,
        // we don't second-guess an explicit path.
        let tmp = TempDir::new().unwrap();
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
}
