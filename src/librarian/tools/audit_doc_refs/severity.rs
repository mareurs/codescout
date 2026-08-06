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
/// Whether a ref's classification as a *local filesystem path* is structural
/// or inferred.
///
/// `Structural` — the token carries path syntax that cannot mean much else: a
/// `/` separator, or an explicit `./` `../` `/` anchor.
///
/// `Inferred` — the token is a bare name that earned `FilePath` from nothing
/// but a known extension. `RELEASES.md`, cited in an ADR about an upstream
/// repository, is this shape; so is every doc that names a file belonging to
/// someone else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathEvidence {
    Structural,
    Inferred,
}

impl PathEvidence {
    /// Purely syntactic half of the judgement: a token with no separator at all
    /// earned `FilePath` from an extension alone. The other half — whether the
    /// root segment exists in the repo — needs the filesystem and so lives in
    /// `resolver::path_evidence`.
    pub fn of_syntax(raw_ref: &str) -> Self {
        if raw_ref.contains('/') {
            Self::Structural
        } else {
            Self::Inferred
        }
    }
}

/// Cap a `missing` severity when the path classification itself was a guess.
///
/// `high` is what gates CI (`--fail-on high`), so it has to mean "definitely a
/// local path, and definitely gone". A bare name that resolves nowhere may
/// simply live in another repository — report it, but below the gate, and say
/// so in `severity_reason` rather than leaving `policy_default` to imply the
/// classification was certain.
///
/// See `docs/issues/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`.
pub fn cap_inferred_path(
    verdict: Verdict,
    evidence: PathEvidence,
    sev: Severity,
    reason: &'static str,
) -> (Severity, &'static str) {
    // `FileMissing` carries the same inference as `Missing`: a `file_symbol` ref
    // whose path part is a bare name earned its path classification from an
    // extension alone, so a miss there is equally a guess.
    if matches!(verdict, Verdict::Missing | Verdict::FileMissing)
        && evidence == PathEvidence::Inferred
        && sev == Severity::High
    {
        return (Severity::Med, "inferred_path");
    }
    (sev, reason)
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
    let s = crate::util::fs::RepoPath::from(p);
    s.as_str().contains("docs/archive/") || s.as_str().ends_with(".archive.md")
}

fn matches_issues(p: &Path) -> bool {
    crate::util::fs::RepoPath::from(p)
        .as_str()
        .contains("docs/issues/")
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
