use super::{Severity, Verdict};
use std::path::Path;

pub fn default_severity(verdict: Verdict) -> Severity {
    use Severity::*;
    use Verdict::*;
    match verdict {
        Missing | FileMissing | SymbolMissing => High,
        AnchorMissing | LineOob | AmbiguousBasename => Med,
        Unknown | ResolvedBasename => Low,
        Resolved | External => Low,
    }
}

/// Apply path-based drop rules. Returns `(severity, reason)`.
pub fn apply_drops(
    md_file: &Path,
    base: Severity,
    memory_globs: &[globset::Glob],
) -> (Severity, &'static str) {
    if matches_archive(md_file) {
        return (drop_one(base), "archive_drop");
    }
    if matches_memory(md_file, memory_globs) {
        return (drop_two(base), "memory_drop");
    }
    if matches_issues(md_file) {
        return (drop_one(base), "issues_drop");
    }
    (base, "policy_default")
}

fn drop_one(s: Severity) -> Severity {
    match s {
        Severity::High => Severity::Med,
        Severity::Med | Severity::Low => Severity::Low,
    }
}

fn drop_two(s: Severity) -> Severity {
    drop_one(drop_one(s))
}

fn matches_archive(p: &Path) -> bool {
    let s = crate::util::fs::to_forward_slash(p);
    s.contains("docs/archive/") || s.ends_with(".archive.md")
}

fn matches_issues(p: &Path) -> bool {
    crate::util::fs::to_forward_slash(p).contains("docs/issues/")
}

fn matches_memory(p: &Path, globs: &[globset::Glob]) -> bool {
    let mut builder = globset::GlobSetBuilder::new();
    for g in globs {
        builder.add(g.clone());
    }
    builder.build().map(|set| set.is_match(p)).unwrap_or(false)
}

pub const DEFAULT_MEMORY_GLOBS: &[&str] = &[
    ".buddy/memory/**",
    "**/.buddy/memory/**",
    "**/buddy/memory/**",
    "**/projects/**/memory/**",
];
