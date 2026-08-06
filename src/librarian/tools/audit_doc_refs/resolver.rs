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
    /// Compiled `.gitignore` for the repo, used by `is_gitignored` to cap
    /// findings whose absence is the normal state. `None` disables the cap
    /// (every ref is treated as tracked), which is the behaviour tests get
    /// unless they opt in.
    pub gitignore: Option<ignore::gitignore::Gitignore>,
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
    if points_outside_project(&c.raw_ref) {
        return outside_project();
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
    verdict_with_drops_for_ref(
        Verdict::Missing,
        &c.raw_ref,
        Path::new(&c.md_file),
        c.position,
        ctx,
    )
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
/// Depth-first search for `name` anywhere in a document-symbol tree.
///
/// `document_symbols` returns a TREE — `convert_document_symbols` nests impl
/// methods and class members under their parent — so a flat `iter()` over the top
/// level only ever sees free functions and type declarations. Every `Type::method`
/// reference in the docs was reported `symbol_missing` for that reason:
/// `src/server.rs::from_parts` names a child of `impl CodeScoutServer`, not a
/// top-level symbol.
///
/// Matches the bare `name` or the slash-joined `name_path`, so both
/// `src/server.rs::from_parts` and `src/server.rs::CodeScoutServer/from_parts`
/// resolve — `is_symbol_suffix` already admits `/`, so docs use both shapes.
fn symbol_tree_contains(syms: &[crate::lsp::SymbolInfo], name: &str) -> bool {
    syms.iter()
        .any(|s| s.name == name || s.name_path == name || symbol_tree_contains(&s.children, name))
}

fn resolve_file_line(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, line_str) = c.raw_ref.rsplit_once(':').expect("file_line invariant");
    if points_outside_project(path_str) {
        return outside_project();
    }
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
            verdict_with_drops_for_ref(
                Verdict::Missing,
                path_str,
                Path::new(&c.md_file),
                c.position,
                ctx,
            )
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

    // A cross-file link may carry a fragment: `../concepts/lite-stack.md#the-tradeoff`
    // addresses a heading inside another page. Only the path part names a file, so
    // strip the fragment before touching the filesystem — otherwise the whole
    // string is stat'd and an existing page is reported as a missing file. The
    // fragment itself is deliberately not validated across files: mdBook's slug
    // rules are its own, and a wrong-slug guess would trade these false positives
    // for a new set.
    let path_part = c
        .raw_ref
        .split_once('#')
        .map(|(p, _)| p)
        .unwrap_or(&c.raw_ref);
    if path_part.is_empty() {
        return Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }

    // fs-scheme link → resolve per markdown convention.
    //
    // Refs starting with `./` or `../` are *explicitly* relative to the file
    // containing the link. Joining them to repo_root would resolve
    // `docs/agents/foo.md` → `../manual/X.md` against repo_root and miss the
    // intended `docs/manual/X.md`. Anchor to md_file.parent() so the OS's `..`
    // lookup walks from the right base.
    //
    // A ref with no explicit prefix is ambiguous, because this repo genuinely
    // holds two conventions:
    //   * `docs/manual/src/**` is an mdBook, where an internal link MUST be
    //     page-relative — `[x](configuration/project-toml.md)` from
    //     `src/architecture.md` addresses `src/configuration/project-toml.md`,
    //     and writing it repo-root-relative would break the rendered book.
    //   * elsewhere under `docs/`, the house style writes such refs
    //     repo-root-relative even from a subdirectory.
    // Accept either. Existence under one convention is proof the author meant
    // that one, and reporting a link the reader can follow is the bug worth
    // avoiding. Page-relative is tried first because it is the markdown standard.
    let md_dir = ctx
        .repo_root
        .join(&c.md_file)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ctx.repo_root.to_path_buf());
    let resolved = if path_part.starts_with("./") || path_part.starts_with("../") {
        md_dir.join(path_part).exists()
    } else {
        md_dir.join(path_part).exists() || ctx.repo_root.join(path_part).exists()
    };
    if resolved {
        return Resolution {
            verdict: Verdict::Resolved,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: None,
        };
    }
    if let Some(r) = try_basename_fallback(path_part, ctx) {
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
    if points_outside_project(path_str) {
        return outside_project();
    }
    // Same basename shorthand `resolve_file_path` and `resolve_file_line` accept:
    // a doc citing `refresh.rs::call` means the repo's one `refresh.rs`. Without
    // this the three ref kinds disagree about identical path parts.
    let direct = ctx.repo_root.join(path_str);
    let path = if direct.exists() {
        direct
    } else if let Some(resolved) = unique_basename_path(path_str, ctx) {
        resolved
    } else {
        return verdict_with_drops_for_ref(
            Verdict::FileMissing,
            path_str,
            Path::new(&c.md_file),
            c.position,
            ctx,
        );
    };
    let lang = detect_language(path_str);
    let Some(lsp) = ctx.lsp.clone() else {
        note_degraded(ctx, lang);
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
            if symbol_tree_contains(&syms, name) {
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
            note_degraded(ctx, lang);
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
/// Record that a language server could not answer for `lang`.
///
/// `"unknown"` is deliberately excluded. `detect_language` above maps six
/// extensions; every other path part in a `file_symbol` ref — a `.sh`, `.toml`,
/// `.md` — comes back `"unknown"`, and there is no server for it to begin with.
/// Counting that as degradation made `scan_meta.degraded` **true on every run**,
/// including all fifteen green CI jobs, which left the flag carrying no
/// information: a saturated signal cannot distinguish anything.
///
/// The flag's job is to tell a caller *"my coverage was incomplete because a server
/// I expected was missing"*. A file that has no server by design is not that, and
/// conflating the two is what made a proposed fix — gate on `degraded` — unusable,
/// since it would have failed every run ever.
///
/// See `docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.
fn note_degraded(ctx: &ResolveCtx<'_>, lang: &str) {
    if lang != "unknown" {
        ctx.degraded_languages.borrow_mut().push(lang.to_string());
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

/// `verdict_with_drops`, then cap the band for each reason the ref's absence is
/// weaker evidence than plain drift.
///
/// Three independent caps apply, in order of how much they narrow:
/// 1. the path classification was inferred, not structural (`cap_inferred_path`);
/// 2. the ref sits in a code block, so it is a transcript (`cap_code_block`);
/// 3. the path is gitignored, so absence is its normal state
///    (`cap_gitignored_path`).
///
/// Used only on the two path-classification paths (`file_path`, `file_line`).
/// A markdown link target is deliberately NOT routed through here: `[x](y.md)`
/// is explicit author intent to point at a local file, so a missing link target
/// stays `high` even without a slash.
fn verdict_with_drops_for_ref(
    verdict: Verdict,
    raw_ref: &str,
    md_file: &Path,
    position: super::RefPosition,
    ctx: &ResolveCtx<'_>,
) -> Resolution {
    let base = severity::default_severity(verdict);
    let (sev, reason) = severity::apply_drops(md_file, base, ctx.memory_globs);
    let (sev, reason) =
        severity::cap_inferred_path(verdict, path_evidence(raw_ref, ctx), sev, reason);
    let (sev, reason) = severity::cap_code_block(verdict, position, sev, reason);
    let (sev, reason) =
        severity::cap_gitignored_path(verdict, is_gitignored(raw_ref, ctx), sev, reason);
    Resolution {
        verdict,
        severity: sev,
        severity_reason: reason,
        notes: None,
    }
}

/// Whether a ref's path part points outside the active project.
///
/// `Path::join` resolves an absolute argument by *discarding* the base
/// (`/repo`.join(`/etc/passwd`) == `/etc/passwd`) and never normalises `..`, so
/// an unguarded `repo_root.join(path_str).exists()` stats the real filesystem
/// outside the project. `resolve_file_path` has always rejected these shapes;
/// `file_line` and `file_symbol` split off only the path part and did not, so a
/// doc citing `/etc/passwd:12` came back `Resolved` — range-checked against the
/// host's real file.
fn points_outside_project(path_str: &str) -> bool {
    let p = Path::new(path_str);
    p.is_absolute()
        || p.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
}

/// The shared verdict for a ref whose target lies outside the active project.
/// Not a miss — the audit simply has no authority over it.
fn outside_project() -> Resolution {
    Resolution {
        verdict: Verdict::Unknown,
        severity: Severity::Low,
        severity_reason: "policy_default",
        notes: Some("path outside active project; scope=umbrella required".to_string()),
    }
}

/// Whether `raw_ref` names a location git will never track.
///
/// Checks the ref *and every parent directory*: `.gitignore` here carries
/// `/.worktrees/`, a directory rule, and the docs cite paths *inside* it
/// (`.worktrees/my-feature`). `matched_path_or_any_parents` is the ignore
/// crate's entry point for exactly that question. Both the file and directory
/// forms are tried because the ref does not exist, so there is nothing to stat
/// and no way to know which it would have been.
fn is_gitignored(raw_ref: &str, ctx: &ResolveCtx<'_>) -> bool {
    // The escape guard is load-bearing, not defensive: `matched_path_or_any_parents`
    // *panics* on a path outside its root. Callers now reject those refs earlier,
    // but this function must stay safe on its own — a panic here aborts the whole
    // audit process, which is how it was found.
    if points_outside_project(raw_ref) {
        return false;
    }

    // Docs name a directory with a trailing slash (`.claude/worktrees/`), and so do
    // gitignore's own directory rules — but the matcher reads the final path
    // component, and a trailing separator leaves that component empty, so nothing
    // matches. Every such ref silently escaped this cap. Strip it first: `foo/` and
    // `foo` denote the same path.
    let cleaned = raw_ref.trim_end_matches('/');
    if cleaned.is_empty() {
        return false;
    }

    // `.git/` is not in `.gitignore` — git excludes its own directory structurally,
    // so no pattern mentions it and the matcher cannot know. It is nonetheless the
    // clearest possible case of untracked state whose shape depends on repo
    // history: `.git/worktrees/` exists only once a worktree has been added, which
    // is exactly why a doc describing worktrees resolved locally and not in CI.
    if cleaned == ".git" || cleaned.starts_with(".git/") {
        return true;
    }

    let Some(gi) = ctx.gitignore.as_ref() else {
        return false;
    };
    let abs = ctx.repo_root.join(cleaned);
    gi.matched_path_or_any_parents(&abs, false).is_ignore()
        || gi.matched_path_or_any_parents(&abs, true).is_ignore()
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
            gitignore: None,
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
    /// A ref inside a code block is a transcript — a command the reader runs, a
    /// payload they send — so `high`, the gating band, is the wrong report for it.
    /// The pair is the point: identical ref, identical absent target, only the
    /// position differs.
    #[test]
    fn resolver_caps_a_missing_path_cited_inside_a_code_block() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let at = |position| RefCandidate {
            md_file: "docs/manual/src/concepts/guide.md".to_string(),
            md_line: 1,
            raw_ref: "src/services/auth.rs".to_string(),
            ref_kind: RefKind::FilePath,
            position,
        };

        let in_block = resolve_ref(&at(RefPosition::FencedBlock), &ctx(tmp.path(), &[]));
        assert_eq!(in_block.verdict, Verdict::Missing);
        assert_eq!(in_block.severity, Severity::Med);
        assert_eq!(in_block.severity_reason, "code_block");

        // Over-match guard: the same ref written in prose is a citation, and still
        // gates. Without this the cap could swallow every finding and look correct.
        let in_prose = resolve_ref(&at(RefPosition::InlineSpan), &ctx(tmp.path(), &[]));
        assert_eq!(in_prose.severity, Severity::High);
        assert_eq!(in_prose.severity_reason, "policy_default");
    }

    /// A gitignored path is *expected* absent, so its absence is not drift. Both
    /// rule shapes are covered: a rule naming the file, and a directory rule whose
    /// match lands on the ref's parent rather than the ref itself.
    #[test]
    fn resolver_caps_missing_paths_that_gitignore_declares_generated_or_local() {
        let tmp = TempDir::new().unwrap();
        // The root segments exist — as they do in the real repo — so these refs are
        // `Structural` and reach the gating band before this cap sees them.
        // Otherwise `cap_inferred_path` would fire first and the assertion on
        // `severity_reason` would silently be testing the wrong cap.
        std::fs::create_dir_all(tmp.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".worktrees")).unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join(".gitignore"),
            "/.codescout/embeddings.db\n/.worktrees/\n",
        )
        .unwrap();

        let gi = {
            let mut b = ignore::gitignore::GitignoreBuilder::new(tmp.path());
            assert!(b.add(tmp.path().join(".gitignore")).is_none());
            b.build().unwrap()
        };
        let globs: [globset::Glob; 0] = [];
        let ctx_gi = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &globs,
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
            gitignore: Some(gi),
        };

        for raw in [".codescout/embeddings.db", ".worktrees/my-feature"] {
            let r = resolve_ref(
                &cand(raw, "docs/manual/src/concepts/x.md", RefKind::FilePath),
                &ctx_gi,
            );
            assert_eq!(r.severity, Severity::Med, "{raw} should not gate");
            assert_eq!(r.severity_reason, "gitignored_path", "{raw}");
        }

        // Over-match guard: a tracked path that is genuinely gone still gates, in
        // the same doc, under the same gitignore.
        let tracked = resolve_ref(
            &cand(
                "src/gone.rs",
                "docs/manual/src/concepts/x.md",
                RefKind::FilePath,
            ),
            &ctx_gi,
        );
        assert_eq!(tracked.severity, Severity::High);
        assert_eq!(tracked.severity_reason, "policy_default");
    }
    /// Docs name a *directory* with a trailing slash — `.claude/worktrees/`,
    /// `.codescout/private-memories/` — and gitignore's own directory rules are
    /// written the same way. Both forms must reach the same verdict, or the cap
    /// silently misses exactly the refs it exists for.
    ///
    /// This is the CI-vs-local skew that a green local gate hid: those directories
    /// exist in a working checkout, so the refs resolved and produced no finding at
    /// all locally, while a fresh clone has neither and reported them `high`.
    #[test]
    fn resolver_caps_a_gitignored_directory_ref_written_with_a_trailing_slash() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".codescout")).unwrap();
        // A real checkout always has `.git`, so the root segment exists and these
        // refs reach the gating band as `Structural`. Without it `cap_inferred_path`
        // fires first and the assertion would be reading the wrong cap.
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
        std::fs::write(
            tmp.path().join(".gitignore"),
            "/.claude/*\n!/.claude/skills/\n.codescout/private-memories/\n",
        )
        .unwrap();
        let gi = {
            let mut b = ignore::gitignore::GitignoreBuilder::new(tmp.path());
            assert!(b.add(tmp.path().join(".gitignore")).is_none());
            b.build().unwrap()
        };
        let globs: [globset::Glob; 0] = [];
        let ctx_gi = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &globs,
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
            gitignore: Some(gi),
        };

        for raw in [
            ".claude/worktrees/",
            ".claude/worktrees",
            ".codescout/private-memories/",
            ".codescout/private-memories",
            // `.git/` is never in `.gitignore` — git excludes its own directory
            // structurally — yet `.git/worktrees/` exists only once a worktree has been
            // added, which is why the docs describing worktrees passed locally and
            // failed on a fresh clone.
            ".git/worktrees/",
            ".git/worktrees",
        ] {
            let r = resolve_ref(
                &cand(
                    raw,
                    "docs/manual/src/concepts/worktrees.md",
                    RefKind::FilePath,
                ),
                &ctx_gi,
            );
            assert_eq!(r.severity, Severity::Med, "{raw} should not gate");
            assert_eq!(r.severity_reason, "gitignored_path", "{raw}");
        }
    }

    /// `ignore`'s `matched_path_or_any_parents` panics — not errors — when handed a
    /// path outside its root, and two ref shapes reach it that way: an absolute path
    /// (which `Path::join` resolves by *discarding* the base) and one containing
    /// `..`. `resolve_file_path` rejects both early, but `file_line` and
    /// `file_symbol` split off only the path part and pass it straight through, so
    /// the first real run aborted the whole audit with SIGABRT.
    #[test]
    fn resolver_does_not_abort_on_a_ref_that_escapes_the_repo_root() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "/.worktrees/\n").unwrap();
        let gi = {
            let mut b = ignore::gitignore::GitignoreBuilder::new(tmp.path());
            assert!(b.add(tmp.path().join(".gitignore")).is_none());
            b.build().unwrap()
        };
        let globs: [globset::Glob; 0] = [];
        let ctx_gi = ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &globs,
            lsp: None,
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
            gitignore: Some(gi),
        };

        // Each of these aborted the process before the escape guard. `/etc/passwd:12`
        // additionally came back `Resolved` — range-checked against the host's real
        // file — because `file_line` never had the outside-project guard that
        // `file_path` did.
        for raw in [
            "/etc/passwd:12",
            "../outside/thing.rs:3",
            "/absolute/file.rs",
            "../../climb.rs",
            "docs/../../escape.rs",
        ] {
            let kind = if raw.contains(':') {
                RefKind::FileLine
            } else {
                RefKind::FilePath
            };
            let r = resolve_ref(&cand(raw, "docs/spec.md", kind), &ctx_gi);
            assert_eq!(r.verdict, Verdict::Unknown, "{raw} is out of scope");
            assert_eq!(r.severity, Severity::Low, "{raw}");
        }

        // Over-match guard: an ordinary in-project ref is unaffected by the escape
        // check — it still reaches the real resolution path and reports a miss.
        let inside = resolve_ref(
            &cand("docs/spec.md", "docs/x.md", RefKind::FilePath),
            &ctx_gi,
        );
        assert_eq!(inside.verdict, Verdict::Missing);
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
    /// The project archives in place, so `archive/` appears at several depths.
    /// Keying the drop on the literal `docs/archive/` left retired session logs and
    /// superseded plans gating at full severity — documents nobody is asked to
    /// trust, blocking CI on references they were correct to make.
    #[test]
    fn severity_drops_one_level_in_any_archive_directory() {
        let tmp = TempDir::new().unwrap();
        // A structural ref rooted in an existing directory, so the band reaching
        // `apply_drops` is `high` and these assertions read the archive rule rather
        // than `cap_inferred_path`.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let missing = "src/gone.rs";

        for md in [
            "docs/trackers/archive/old-session-log.md",
            "docs/plans/archive/2026-03-20-superseded.md",
            "docs/superpowers/plans/archive/older.md",
        ] {
            let r = resolve_ref(&cand(missing, md, RefKind::FilePath), &ctx(tmp.path(), &[]));
            assert_eq!(r.severity, Severity::Med, "{md}");
            assert_eq!(r.severity_reason, "archive_drop", "{md}");
        }

        // Over-match guard: a path merely *containing* the word is not an archive
        // directory, and a live reference doc keeps gating. (A live tracker is no
        // longer the example here — trackers now take `historical_drop`; see
        // `severity_drops_one_level_for_dated_record_directories_only`.)
        for md in [
            "docs/manual/src/concepts/archived-artifacts.md",
            "docs/conventions/test-env-isolation.md",
        ] {
            let r = resolve_ref(&cand(missing, md, RefKind::FilePath), &ctx(tmp.path(), &[]));
            assert_eq!(r.severity, Severity::High, "{md}");
            assert_eq!(r.severity_reason, "policy_default", "{md}");
        }
    }
    /// Dated ledgers report below the gate; what a reader acts on now does not.
    /// The two halves are asserted together because the value of this rule is
    /// entirely in where its boundary falls — a version that dropped everything
    /// under `docs/` would pass the first loop and be useless.
    #[test]
    fn severity_drops_one_level_for_dated_record_directories_only() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let missing = "src/gone.rs";

        for md in [
            "docs/plans/2026-04-22-refactoring-plan.md",
            "docs/reviews/2026-04-24/phase-2-tools.md",
            "docs/research/2026-04-03-embedding-model-benchmark.md",
            "docs/usage-reports/2026-07-01-usage-analysis.md",
            "docs/superpowers/plans/2026-02-28-lsp-mock-impl.md",
            "docs/spikes/something.md",
            "docs/trackers/bug-fix-session-log.md",
            // The root-level form of the same thing — a dated review, which the
            // librarian's own classifier patterns already name as `docs/review-*.md`.
            "docs/review-2026-03-05.md",
        ] {
            let r = resolve_ref(&cand(missing, md, RefKind::FilePath), &ctx(tmp.path(), &[]));
            assert_eq!(r.severity, Severity::Med, "{md} should not gate");
            assert_eq!(r.severity_reason, "historical_drop", "{md}");
        }

        // The boundary. Every one of these is read to decide what to do *now*, so a
        // stale reference in it costs the reader something the gate exists to prevent.
        for md in [
            "docs/manual/src/concepts/tool-selection.md",
            "docs/ROADMAP.md",
            "CLAUDE.md",
            "README.md",
            "docs/architecture/companion-plugin.md",
            "docs/conventions/test-env-isolation.md",
            "docs/adrs/2026-07-20-artifact-vec-shared-catalog-boundary.md",
            "docs/evals/reconnaissance-output.md",
            "docs/templates/session-log.md",
            // Not a dated review: the `review-` prefix is what the rule keys on, and
            // a guide that merely mentions reviewing must keep gating.
            "docs/reviewing-guide.md",
        ] {
            let r = resolve_ref(&cand(missing, md, RefKind::FilePath), &ctx(tmp.path(), &[]));
            assert_eq!(r.severity, Severity::High, "{md} must keep gating");
            assert_eq!(r.severity_reason, "policy_default", "{md}");
        }
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
    /// `high` gates CI, so the band must be reserved for verdicts that are
    /// deterministic on an unchanged tree. This pins the split, because nothing did:
    /// `SymbolMissing` sat at `high` untested, and it is the one verdict computed from
    /// an LSP response rather than from the filesystem.
    ///
    /// `resolve_file_symbol` returns `SymbolMissing` when the server answers and the
    /// symbol is absent, and `Unknown` when the server does not answer. With
    /// `SymbolMissing` at `high` those two straddled the gate, so one unanswered
    /// request flipped the exit code — observed twice as a count moving up and back
    /// down on an identical tree.
    #[test]
    fn default_severity_gates_only_on_deterministic_filesystem_verdicts() {
        use crate::librarian::tools::audit_doc_refs::severity::default_severity;

        // Filesystem facts: same answer on every run.
        for v in [Verdict::Missing, Verdict::FileMissing] {
            assert_eq!(default_severity(v), Severity::High, "{v:?} should gate");
        }

        // Everything else reports without gating. `SymbolMissing` is the load-bearing
        // entry: it is a real drift signal, and it is still reported — at `med`.
        for v in [
            Verdict::SymbolMissing,
            Verdict::AnchorMissing,
            Verdict::LineOob,
            Verdict::AmbiguousBasename,
        ] {
            assert_eq!(default_severity(v), Severity::Med, "{v:?} must not gate");
        }

        for v in [
            Verdict::Unknown,
            Verdict::ResolvedBasename,
            Verdict::Resolved,
            Verdict::External,
        ] {
            assert_eq!(default_severity(v), Severity::Low, "{v:?}");
        }
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
                gitignore: None,
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
            gitignore: None,
        };
        let r = resolve_ref(&c, &ctx);
        assert_eq!(r.verdict, Verdict::Unknown);
        assert!(ctx.degraded_languages.borrow().iter().any(|l| l == "rust"));
    }
    /// A path part with no LSP mapping is **out of scope**, not degraded coverage.
    /// Recording it made `scan_meta.degraded` true on every run — including all fifteen
    /// green CI jobs — which left the flag unable to distinguish anything, and made
    /// "gate on `degraded`" an unusable proposal. The sibling test above pins the other
    /// half: a language that *does* have a server still marks the scan degraded.
    #[test]
    fn resolver_does_not_mark_scan_degraded_for_a_language_with_no_server_by_design() {
        let tmp = TempDir::new().unwrap();
        // Extensions outside detect_language's six. There is no server for any of them,
        // so their absence is not a coverage gap.
        for (file, refstr) in [
            ("deploy.sh", "deploy.sh:main"),
            ("Cargo.toml", "Cargo.toml:package"),
            ("notes.md", "notes.md:Summary"),
        ] {
            std::fs::write(tmp.path().join(file), "x\n").unwrap();
            let ctx = ResolveCtx {
                repo_root: tmp.path(),
                memory_globs: &[],
                lsp: None,
                degraded_languages: Default::default(),
                basename_index: std::collections::HashMap::new(),
                gitignore: None,
            };
            let r = resolve_ref(&cand(refstr, "docs/spec.md", RefKind::FileSymbol), &ctx);
            assert_eq!(r.verdict, Verdict::Unknown, "{refstr}");
            assert!(
                ctx.degraded_languages.borrow().is_empty(),
                "{refstr} must not mark the scan degraded; got {:?}",
                ctx.degraded_languages.borrow()
            );
        }
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
                gitignore: None,
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
                gitignore: None,
            },
        );
        assert_eq!(r.verdict, Verdict::Resolved);
    }
    /// An impl method is a CHILD of its `impl` block in the document-symbol
    /// tree, so the old flat `syms.iter()` never saw it. Every
    /// `src/foo.rs::method` reference in the docs was reported
    /// `symbol_missing` against a symbol that plainly exists —
    /// `src/server.rs::from_parts` supplied 5 of 50 high findings on
    /// 2026-08-06. Both the bare name and the slash-joined `name_path` must
    /// resolve, since `is_symbol_suffix` admits `/` and docs use both shapes.
    #[test]
    fn resolver_resolves_a_symbol_nested_under_its_impl_block() {
        use crate::lsp::mock::{MockLspClient, MockLspProvider};
        use crate::lsp::{SymbolInfo, SymbolKind};
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("foo.rs"),
            "impl Thing { pub fn nested() {} }\n",
        )
        .unwrap();
        let leaf = SymbolInfo {
            name: "nested".to_string(),
            name_path: "Thing/nested".to_string(),
            kind: SymbolKind::Method,
            file: tmp.path().join("foo.rs"),
            start_line: 0,
            end_line: 0,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        };
        let parent = SymbolInfo {
            name: "Thing".to_string(),
            name_path: "Thing".to_string(),
            kind: SymbolKind::Struct,
            file: tmp.path().join("foo.rs"),
            start_line: 0,
            end_line: 0,
            range_start_line: None,
            start_col: 0,
            children: vec![leaf],
            detail: None,
        };
        let ctx_for = |lsp| ResolveCtx {
            repo_root: tmp.path(),
            memory_globs: &[],
            lsp: Some(lsp),
            degraded_languages: Default::default(),
            basename_index: std::collections::HashMap::new(),
            gitignore: None,
        };
        let mk_lsp = || {
            MockLspProvider::with_client(
                MockLspClient::new().with_symbols(tmp.path().join("foo.rs"), vec![parent.clone()]),
            )
        };

        // Bare method name, the shape the docs actually use.
        let r = resolve_ref(
            &cand("foo.rs::nested", "docs/spec.md", RefKind::FileSymbol),
            &ctx_for(mk_lsp()),
        );
        assert_eq!(
            r.verdict,
            Verdict::Resolved,
            "bare nested name must resolve"
        );

        // Slash-joined name_path.
        let r = resolve_ref(
            &cand("foo.rs::Thing/nested", "docs/spec.md", RefKind::FileSymbol),
            &ctx_for(mk_lsp()),
        );
        assert_eq!(r.verdict, Verdict::Resolved, "name_path must resolve too");

        // The guard against over-matching: a name absent from the whole tree
        // must still report missing, or this fix would resolve everything.
        let r = resolve_ref(
            &cand("foo.rs::absent", "docs/spec.md", RefKind::FileSymbol),
            &ctx_for(mk_lsp()),
        );
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
            gitignore: None,
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
            gitignore: None,
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
            gitignore: None,
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
            gitignore: None,
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
            gitignore: None,
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
            gitignore: None,
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
            gitignore: None,
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
    /// `docs/manual/src/**` is an mdBook, where an internal link with no `./`
    /// prefix is page-relative — writing it repo-root-relative would break the
    /// rendered book. Both conventions live in this repo, so both must resolve; the
    /// sibling test above pins the repo-root half.
    #[test]
    fn resolver_link_resolves_page_relative_inside_the_mdbook() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("docs/manual/src/configuration");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("project-toml.md"), "# Project toml\n").unwrap();

        let r = resolve_ref(
            &cand(
                "configuration/project-toml.md",
                "docs/manual/src/architecture.md",
                RefKind::Link,
            ),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Resolved);

        // Over-match guard: accepting both conventions must not accept a link that
        // exists under neither.
        let r = resolve_ref(
            &cand(
                "configuration/nope.md",
                "docs/manual/src/architecture.md",
                RefKind::Link,
            ),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
    }

    /// A cross-file link may address a heading inside another page. Only the path
    /// part names a file, so stat'ing the whole string reports an existing page as a
    /// missing file — which is strictly worse than saying nothing about the anchor.
    #[test]
    fn resolver_link_strips_the_fragment_before_resolving_the_path() {
        let tmp = TempDir::new().unwrap();
        let concepts = tmp.path().join("docs/manual/src/concepts");
        std::fs::create_dir_all(&concepts).unwrap();
        std::fs::write(
            concepts.join("lite-stack.md"),
            "# Lite stack\n\n## The tradeoff\n",
        )
        .unwrap();

        for raw in [
            "../concepts/lite-stack.md#the-tradeoff",
            "lite-stack.md#the-tradeoff",
        ] {
            let r = resolve_ref(
                &cand(raw, "docs/manual/src/concepts/security.md", RefKind::Link),
                &ctx(tmp.path(), &[]),
            );
            assert_eq!(r.verdict, Verdict::Resolved, "{raw}");
        }

        // Over-match guard: a fragment is not a licence to invent the file.
        let r = resolve_ref(
            &cand(
                "../concepts/gone.md#whatever",
                "docs/manual/src/concepts/security.md",
                RefKind::Link,
            ),
            &ctx(tmp.path(), &[]),
        );
        assert_eq!(r.verdict, Verdict::Missing);
    }
}
