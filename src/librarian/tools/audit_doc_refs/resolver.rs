use super::severity;
use super::{RefCandidate, RefKind, Resolution, Severity, Verdict};
use std::path::Path;

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    // LSP wired in Task 8
}

pub fn resolve_ref(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    match c.ref_kind {
        RefKind::FilePath => resolve_file_path(c, ctx),
        RefKind::FileLine => resolve_file_line(c, ctx),
        RefKind::FileSymbol => stub_unknown(c, ctx),
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
}
