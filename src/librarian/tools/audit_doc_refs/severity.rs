use super::{Severity, Verdict};
use std::path::Path;

/// Base severity per verdict, before any path-based drop or cap.
///
/// `high` gates CI, so it is reserved for verdicts that are **deterministic
/// filesystem facts**: the path is not there, or the path a `file_symbol` ref
/// names is not there. Those answers do not change between two runs on the same
/// tree.
///
/// `SymbolMissing` is deliberately **not** among them, despite being a real drift
/// signal. It is computed from an LSP `document_symbols` response, and
/// `resolve_file_symbol` puts the answered and unanswered cases in different
/// bands: answered-and-absent was `SymbolMissing`/high, unanswered is
/// `Unknown`/low. That made gate membership a function of whether a language
/// server replied to one request — observed twice as a finding count moving up and
/// then back down on an unchanged tree, and it is worst at zero findings, where a
/// single flap is the whole verdict. `med` keeps the report and takes the
/// non-determinism out of the exit code. See
/// `docs/issues/archive/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.
pub fn default_severity(verdict: Verdict) -> Severity {
    use Severity::*;
    use Verdict::*;
    match verdict {
        Missing | FileMissing => High,
        SymbolMissing | AnchorMissing | LineOob | AmbiguousBasename => Med,
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
/// Cap a `missing` severity when the reference sits inside a code block.
///
/// A fenced or indented code block is a *transcript* — a command the reader will
/// run, a payload they will send, a sample of output they will see — not a
/// citation the author is making. `git worktree add .worktrees/my-feature` names
/// a path that is *supposed* not to exist yet; `"path": "src/services/auth.rs"`
/// inside a tool-call example names a file in the reader's project, not in this
/// one. Reporting either as drift asks the docs to stop teaching.
///
/// The asymmetry is already established for the sibling position: `parse_refs`
/// keeps every link target because an explicit link *is* author intent to point
/// somewhere real. A code block carries no such intent, so it reports below the
/// gate rather than above it.
pub fn cap_code_block(
    verdict: Verdict,
    position: super::RefPosition,
    sev: Severity,
    reason: &'static str,
) -> (Severity, &'static str) {
    if matches!(
        verdict,
        Verdict::Missing | Verdict::FileMissing | Verdict::SymbolMissing
    ) && position == super::RefPosition::FencedBlock
        && sev == Severity::High
    {
        return (Severity::Med, "code_block");
    }
    (sev, reason)
}

/// Cap a `missing` severity when the referenced path is gitignored.
///
/// A gitignored path is *expected* absent. It is generated at runtime
/// (`.codescout/embeddings.db`), created by the reader to opt into something
/// (`.claude/codescout-companion.json`), or a scratch location the docs instruct
/// them to make (`.worktrees/my-feature`). Absence in a clean checkout is the
/// normal state, so it carries no drift signal — and `high` is reserved for
/// "definitely a local path, and definitely gone".
///
/// Note this is strictly narrower than excluding the citing page: a doc that
/// names both a gitignored runtime file and a real tracked path still gates on
/// the tracked one.
pub fn cap_gitignored_path(
    verdict: Verdict,
    is_gitignored: bool,
    sev: Severity,
    reason: &'static str,
) -> (Severity, &'static str) {
    if matches!(verdict, Verdict::Missing | Verdict::FileMissing)
        && is_gitignored
        && sev == Severity::High
    {
        return (Severity::Med, "gitignored_path");
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
    if matches_historical(md_file) {
        return (drop_one(base), "historical_drop");
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

/// Whether the citing document is archived.
///
/// Any `archive/` path segment counts, not just the literal `docs/archive/` this
/// once tested. The project archives in place — `docs/trackers/archive/`,
/// `docs/plans/archive/`, `docs/issues/archive/` — so keying on one location
/// left the others gating at full severity, which is what a retired session log
/// citing `src/prompts/server_instructions.md` (renamed long ago) was doing.
fn matches_archive(p: &Path) -> bool {
    let s = crate::util::fs::RepoPath::from(p);
    let s = s.as_str();
    s.ends_with(".archive.md") || s.split('/').any(|seg| seg == "archive")
}

fn matches_issues(p: &Path) -> bool {
    crate::util::fs::RepoPath::from(p)
        .as_str()
        .contains("docs/issues/")
}
/// Directories under `docs/` holding **dated, point-in-time records**: written
/// once to describe the tree as it stood, and never revised to follow it.
///
/// A code review in `docs/reviews/2026-04-24/` cites `src/tools/github.rs:680-690`
/// because that is what it reviewed; re-checking those line numbers against HEAD
/// asks a historical document to be a live one. Session logs are the same species —
/// an `F-N` entry opens `**Observed:** <date>` — as are superseded plans and
/// benchmark write-ups.
///
/// This is not a new policy so much as the one `issues_drop` already encodes.
/// Bug files are acted on constantly and still report below the gate, because
/// what they *are* is a dated ledger. Extending the same treatment to the other
/// ledgers is the consistent reading; keeping `docs/issues/**` capped while
/// `docs/trackers/**` gates at full severity was the inconsistent one.
///
/// Deliberately **absent**, and still gating at full severity: `docs/manual/**`,
/// the root `docs/*.md` guides, `CLAUDE.md`, every `README.md`,
/// `docs/architecture/**`, `docs/conventions/**`, `docs/adrs/**`,
/// `docs/evals/**`, `docs/templates/**`. Those are what a reader acts on *now*,
/// which is what the gate is for.
pub const DEFAULT_HISTORICAL_DIRS: &[&str] = &[
    "plans",
    "research",
    "reviews",
    "spikes",
    "superpowers",
    "trackers",
    "usage-reports",
];

/// Whether the citing document is a dated, point-in-time record.
///
/// Two shapes, because the project uses two:
///
/// * a directory under `docs/` from `DEFAULT_HISTORICAL_DIRS` — matched on the
///   segment immediately below `docs/`, so `docs/ROADMAP.md` does not qualify
///   (its second segment is the filename) and neither does `docs/manual/src/…`;
/// * a root-level `docs/review-*.md`. Dated reviews live both in
///   `docs/reviews/` and at the top of `docs/` under this name — the librarian's
///   own built-in classifier patterns list `docs/review-*.md` — and a review is a
///   record of one commit either way. `docs/review-2026-03-05.md` cites seventeen
///   line-pinned ranges into modules since split apart.
fn matches_historical(p: &Path) -> bool {
    let s = crate::util::fs::RepoPath::from(p);
    let s = s.as_str();
    let mut segs = s.split('/');
    if segs.next() != Some("docs") {
        return false;
    }
    let Some(second) = segs.next() else {
        return false;
    };
    let is_leaf = segs.next().is_none();
    if is_leaf {
        return second.starts_with("review-") && second.ends_with(".md");
    }
    DEFAULT_HISTORICAL_DIRS.contains(&second)
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
