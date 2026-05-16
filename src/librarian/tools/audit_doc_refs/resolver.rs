use super::severity;
use super::{RefCandidate, RefKind, Resolution, Severity, Verdict};
use std::path::Path;

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    pub lsp: Option<&'a dyn crate::lsp::ops::LspProvider>,
    pub degraded_languages: std::cell::RefCell<Vec<String>>,
}

pub fn resolve_ref(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    match c.ref_kind {
        RefKind::FilePath => resolve_file_path(c, ctx),
        RefKind::FileLine => resolve_file_line(c, ctx),
        RefKind::FileSymbol => resolve_file_symbol(c, ctx),
        RefKind::ModulePath => stub_unknown(c, ctx),
        RefKind::Link => resolve_link(c, ctx),
    }
}

fn resolve_file_path(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let path = ctx.repo_root.join(&c.raw_ref);
    if path.exists() {
        Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        }
    } else {
        verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
    }
}

fn resolve_file_line(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, line_str) = c.raw_ref.rsplit_once(':').expect("file_line invariant");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs);
    }
    let line: u32 = line_str.parse().unwrap_or(0);
    let total = std::fs::read_to_string(&path)
        .map(|s| s.lines().count() as u32)
        .unwrap_or(0);
    if line == 0 || line > total {
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
    if c.raw_ref.starts_with('#') {
        // anchor — wired in Task 8b; stub as resolved for now
        return Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }
    // fs-scheme link → same as file_path
    let path = ctx.repo_root.join(&c.raw_ref);
    if path.exists() {
        Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        }
    } else {
        verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
    }
}

fn stub_unknown(_c: &RefCandidate, _ctx: &ResolveCtx<'_>) -> Resolution {
    Resolution {
        verdict: Verdict::Unknown,
        severity: Severity::Low,
        severity_reason: "policy_default",
        notes: None,
    }
}
fn resolve_file_symbol(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, name) = c
        .raw_ref
        .rsplit_once(':')
        .expect("file_symbol invariant: raw_ref must be 'path:symbol'");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(Verdict::FileMissing, Path::new(&c.md_file), ctx.memory_globs);
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
                verdict_with_drops(Verdict::SymbolMissing, Path::new(&c.md_file), ctx.memory_globs)
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
    let (sev, reason) = severity::apply_drops(md_file, verdict, base, memory_globs);
    Resolution {
        verdict,
        severity: sev,
        severity_reason: reason,
        notes: None,
    }
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
        let r = resolve_ref(&cand("gone.py", "docs/spec.md", RefKind::FilePath), &ctx(tmp.path(), &[]));
        assert!(!r.severity_reason.is_empty(), "FilePath severity_reason empty");
        // FileLine: must have a colon-separated line number to satisfy the resolver invariant
        let r = resolve_ref(&cand("gone.py:1", "docs/spec.md", RefKind::FileLine), &ctx(tmp.path(), &[]));
        assert!(!r.severity_reason.is_empty(), "FileLine severity_reason empty");
        // Link: external URL
        let r = resolve_ref(&cand("https://example.com/gone", "docs/spec.md", RefKind::Link), &ctx(tmp.path(), &[]));
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
            },
        );
        // File exists on disk but LSP returned no symbols → SymbolMissing, NOT Unknown.
        // This encodes the "prefer disk truth" rule: the LSP responded (not offline),
        // so an empty symbol list means the symbol genuinely isn't there.
        assert_eq!(r.verdict, Verdict::SymbolMissing);
    }
}
