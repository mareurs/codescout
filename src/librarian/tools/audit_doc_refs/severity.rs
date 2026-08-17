use super::{Severity, SeverityReason, Verdict};
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
/// See `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`.
pub fn cap_inferred_path(
    verdict: Verdict,
    evidence: PathEvidence,
    sev: Severity,
    reason: SeverityReason,
) -> (Severity, SeverityReason) {
    // `FileMissing` carries the same inference as `Missing`: a `file_symbol` ref
    // whose path part is a bare name earned its path classification from an
    // extension alone, so a miss there is equally a guess.
    if matches!(verdict, Verdict::Missing | Verdict::FileMissing)
        && evidence == PathEvidence::Inferred
        && sev == Severity::High
    {
        return (Severity::Med, SeverityReason::InferredPath);
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
    reason: SeverityReason,
) -> (Severity, SeverityReason) {
    if matches!(
        verdict,
        Verdict::Missing | Verdict::FileMissing | Verdict::SymbolMissing
    ) && position == super::RefPosition::FencedBlock
        && sev == Severity::High
    {
        return (Severity::Med, SeverityReason::CodeBlock);
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
    reason: SeverityReason,
) -> (Severity, SeverityReason) {
    if matches!(verdict, Verdict::Missing | Verdict::FileMissing)
        && is_gitignored
        && sev == Severity::High
    {
        return (Severity::Med, SeverityReason::GitignoredPath);
    }
    (sev, reason)
}
/// The first line belonging to a *released* section of a changelog, if this file
/// is one. `None` for every other file.
///
/// One boundary is enough because a Keep-a-Changelog file is reverse-chronological
/// and append-at-top: `## [Unreleased]` leads, and every heading below it describes
/// a version that already shipped. So the first non-`[Unreleased]` version heading
/// divides live claims from history, and no per-section bookkeeping is needed.
pub fn released_history_boundary(text: &str, md_path: &Path) -> Option<u32> {
    let name = md_path.file_name()?.to_str()?;
    if !name.eq_ignore_ascii_case("CHANGELOG.md") {
        return None;
    }
    text.lines().enumerate().find_map(|(i, line)| {
        // `## [1.2.3] - 2026-01-01`, but not `## [Unreleased]`.
        let rest = line.strip_prefix("## ")?;
        let inner = rest.trim_start().strip_prefix('[')?;
        let (tag, _) = inner.split_once(']')?;
        (!tag.eq_ignore_ascii_case("unreleased")).then_some(i as u32 + 1)
    })
}

/// Cap a citation found in a **source comment** at `Med`.
///
/// Sibling of [`cap_released_history`]: both exist because a verdict that is
/// correct in the abstract can still be the wrong thing to gate a build on.
///
/// A citation rotting inside a comment is real drift and belongs in the
/// report. It must not fail CI, because the person who breaks it is rarely the
/// person editing that comment — archiving a bug file orphans citations
/// repo-wide (measured 2026-08-15: one archive convention left 95 of 111 cited
/// bug files pointing at a path that had moved). Gating on that would make an
/// unrelated contributor's build fail for a comment they never touched.
///
/// Promote to full severity when the surface has earned it on evidence, not on
/// its first day. See SD-1 in docs/trackers/structural-debt-refactor.md.
pub fn cap_code_comment(severity: Severity, reason: SeverityReason) -> (Severity, SeverityReason) {
    if severity == Severity::High {
        (Severity::Med, SeverityReason::CodeCommentCapped)
    } else {
        (severity, reason)
    }
}

/// Cap a `missing` severity when the ref is cited from a released changelog section.
///
/// A shipped release section is a true statement about the past. When it says a
/// change touched `src/embed/index.rs`, that was where the code lived *at that
/// version* — the module has since been reorganised, and rewriting the entry to
/// name today's path would falsify what that release actually shipped. So the
/// reference is unfixable by design, and unfixable findings must not gate.
///
/// This is the changelog analogue of `archive_drop`, and it drops rather than
/// suppresses for the same reason: a historical record citing a moved path carries
/// no *live* drift signal, but it is still worth reporting below the gate.
///
/// `[Unreleased]` is deliberately excluded and keeps gating at full severity. That
/// is the half that describes the current tree, and it earns its keep: the change
/// that added this cap also repaired a genuinely broken `docs/issues/…` ref sitting
/// in `[Unreleased]`, whose file had been moved to `docs/issues/archive/`.
pub fn cap_released_history(
    verdict: Verdict,
    md_line: u32,
    boundary: Option<u32>,
    sev: Severity,
    reason: SeverityReason,
) -> (Severity, SeverityReason) {
    let Some(from) = boundary else {
        return (sev, reason);
    };
    if md_line >= from
        && sev == Severity::High
        && matches!(
            verdict,
            Verdict::Missing | Verdict::FileMissing | Verdict::SymbolMissing
        )
    {
        return (drop_one(sev), SeverityReason::ReleasedHistory);
    }
    (sev, reason)
}

/// Apply path-based drop rules. Returns `(severity, reason)`.
pub fn apply_drops(
    md_file: &Path,
    base: Severity,
    memory_globs: &[globset::Glob],
) -> (Severity, SeverityReason) {
    if matches_archive(md_file) {
        return (drop_one(base), SeverityReason::ArchiveDrop);
    }
    if matches_memory(md_file, memory_globs) {
        return (drop_two(base), SeverityReason::MemoryDrop);
    }
    if matches_issues(md_file) {
        return (drop_one(base), SeverityReason::IssuesDrop);
    }
    if matches_historical(md_file) {
        return (drop_one(base), SeverityReason::HistoricalDrop);
    }
    (base, SeverityReason::PolicyDefault)
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
