// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate, RefKind, RefPosition};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub fn parse_refs(
    text: &str,
    md_path: &Path,
    syntax: PathSyntax,
) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    // Forward-slash normalize so md_file keys are consistent across platforms.
    let md_file = crate::util::fs::RepoPath::from(md_path).into_string();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut candidates = Vec::new();
    let warnings = fence_warnings(text, &md_file);

    let mut in_code_block = false;
    // Set by an `<!-- audit-doc-refs:ignore -->` comment, cleared at the next
    // heading of any level. See `Suppression` for why the scope is a section, and
    // for the `ignore-refs` form that names individual tokens instead.
    let mut suppression = Suppression::None;
    let parser = Parser::new_ext(text, opts).into_offset_iter();
    for (event, span) in parser {
        let line = byte_offset_to_line(text, span.start);
        match event {
            Event::Html(html) | Event::InlineHtml(html)
                if parse_ignore_marker(html.as_ref()).is_some() =>
            {
                // `is_some()` was checked in the guard, so this cannot re-enter the
                // `None` arm and clear an active suppression.
                suppression = parse_ignore_marker(html.as_ref()).unwrap_or(Suppression::None);
            }
            Event::Start(Tag::Heading { .. }) => suppression = Suppression::None,
            // A span that renders a code span literally is showing what a
            // reference looks like, not making one. See `is_markup_display`.
            Event::Code(content)
                if !suppression.blocks_everything() && !is_markup_display(content.as_ref()) =>
            {
                for raw in tokenize_code_span(content.as_ref()) {
                    if suppression.blocks(raw) {
                        continue;
                    }
                    if let Some(kind) = classify(raw, true, syntax) {
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line,
                            raw_ref: raw.to_string(),
                            ref_kind: kind,
                            position: RefPosition::InlineSpan,
                        });
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            Event::Text(content) if in_code_block && !suppression.blocks_everything() => {
                for raw in tokenize_code_span(content.as_ref()) {
                    if suppression.blocks(raw) {
                        continue;
                    }
                    if let Some(kind) = classify(raw, true, syntax) {
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line,
                            raw_ref: raw.to_string(),
                            ref_kind: kind,
                            position: RefPosition::FencedBlock,
                        });
                    }
                }
            }
            // A link whose target is a naming-convention placeholder
            // (`[YYYY-MM-DD-slug.md](./YYYY-MM-DD-slug.md)`) is an example of markup
            // to copy, not a citation. Everything else is kept: an explicit link IS
            // author intent to point somewhere real, which is why link targets are
            // otherwise unfiltered here.
            Event::Start(Tag::Link { dest_url, .. })
                if !suppression.blocks(dest_url.as_ref()) && !is_placeholder(dest_url.as_ref()) =>
            {
                candidates.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: line,
                    raw_ref: dest_url.into_string(),
                    ref_kind: RefKind::Link,
                    position: RefPosition::LinkTarget,
                });
            }
            _ => {}
        }
    }
    (candidates, warnings)
}
/// How the surrounding language spells a qualified name — the one thing the ref
/// classifier needs from a language to judge a dotted token like `a.b.c`.
///
/// `is_module_path` accepts all-lowercase dotted tokens, which is simultaneously
/// the shape of a Python module (`os.path`), a Go qualified name (`pkg.symbol`),
/// and a Rust field or SQL column (`commits.git_root`, `report.remap`). The token
/// alone cannot separate them; only the language can.
///
/// Measured 2026-08-16, and the reason this exists: across
/// `src/librarian/catalog/**` (41 refs) **56% came back `unknown`**, essentially
/// all of them dotted identifiers in Rust doc comments naming SQL columns and
/// struct fields. The same scan over every non-Rust source file in the repo
/// (51 files, 39 refs) reported **5% unknown and 79% resolved**. The noise was
/// never a property of source comments in general — it was Rust doc comments
/// discussing schemas, and a language-blind rule tuned on it would have deleted
/// real module references from Python, Go, Java and Kotlin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSyntax {
    /// Dotted qualified names are real, so keep classifying them: Python, Go,
    /// Java, Kotlin, TypeScript, JavaScript. Also the default for markdown,
    /// which has no language — markdown behaviour is deliberately unchanged.
    DottedModules,
    /// Qualified names use `::`, so a dotted token is field access and never a
    /// module path: Rust.
    ColonColonModules,
    /// No module concept at all: shell, CSS, HTML.
    NoModules,
}

impl PathSyntax {
    /// Map a `crate::ast::detect_language` key onto its qualified-name syntax.
    ///
    /// `None` — markdown, which reaches the classifier with no language — maps to
    /// `DottedModules` because that is exactly what the classifier did before this
    /// distinction existed. The markdown surface has already been swept once
    /// (SD-1); changing its verdicts here would be an unrequested behaviour change
    /// to the corpus with the most citations.
    ///
    /// An unrecognised key also maps to `DottedModules`: a new grammar should
    /// arrive with today's behaviour and be tightened deliberately, not silently
    /// lose refs the moment it is vendored.
    pub fn for_language(language: Option<&str>) -> Self {
        match language {
            Some(l) if l.eq_ignore_ascii_case("rust") => Self::ColonColonModules,
            // Keys exactly as `crate::ast::detect_language` emits them — it is the
            // sole producer, so an alias it never returns (`sh`, `shell`) would be
            // a dead arm that reads as coverage.
            Some(l)
                if matches!(
                    l.to_ascii_lowercase().as_str(),
                    "bash" | "css" | "scss" | "less" | "html"
                ) =>
            {
                Self::NoModules
            }
            _ => Self::DottedModules,
        }
    }

    /// Whether a dotted token may be classified as [`RefKind::ModulePath`].
    fn admits_dotted_modules(self) -> bool {
        matches!(self, Self::DottedModules)
    }
}

fn classify(s: &str, in_code_context: bool, syntax: PathSyntax) -> Option<RefKind> {
    // Try Rust-style `path::symbol` first so the trailing colon doesn't leak
    // into the path part. Fall back to single `:` for python-style and line
    // refs (file.py:cmd, file.rs:42, file.rs:42-99).
    if let Some((path_part, suffix)) = s.rsplit_once("::") {
        if looks_like_path(path_part) && is_symbol_suffix(suffix) {
            return Some(RefKind::FileSymbol);
        }
    }
    if let Some((path_part, suffix)) = s.rsplit_once(':') {
        if looks_like_path(path_part) {
            if is_line_or_range(suffix) {
                return Some(RefKind::FileLine);
            }
            if is_symbol_suffix(suffix) {
                return Some(RefKind::FileSymbol);
            }
        }
    }
    if looks_like_path(s) {
        return Some(RefKind::FilePath);
    }
    if in_code_context && syntax.admits_dotted_modules() && is_module_path(s) {
        return Some(RefKind::ModulePath);
    }
    None
}
fn is_symbol_suffix(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '/' || c == '.')
        && s.chars()
            .next()
            .map(|c| !c.is_ascii_digit())
            .unwrap_or(false)
}

fn is_line_or_range(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // path:N-M line range — both ends must be non-empty digit-only.
    if let Some((start, end)) = s.split_once('-') {
        return !start.is_empty()
            && !end.is_empty()
            && start.chars().all(|c| c.is_ascii_digit())
            && end.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Extract refs from **plain prose**, for callers whose text is a source comment.
///
/// `parse_refs` deliberately scans only three places — inline code spans,
/// fenced blocks, and link targets — because in a *document* a path mentioned
/// in a sentence is as often an example as a citation, and admitting prose
/// would drown the report. That trade-off inverts in a **code comment**:
/// `// see docs/issues/foo.md` is a pointer, and whether the author reached
/// for backticks is a style habit, not a statement of intent.
///
/// Measured on this repo the day the code path shipped: 699 `docs/…md`
/// citations live in `.rs` files and only **140** are backticked. Scanning
/// code spans alone therefore saw 20% of what it was built to see.
///
/// Markdown must never call this. The caller separation is the whole safety
/// argument, and `mod.rs` enforces it by calling this only from
/// `scan_code_comments`.
pub fn parse_prose_refs(text: &str, md_path: &Path, syntax: PathSyntax) -> Vec<RefCandidate> {
    let md_file = crate::util::fs::RepoPath::from(md_path).into_string();
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        // Strip the comment marker before tokenizing. Without this the marker
        // itself is a candidate: `//` and `///` contain a slash, and in prose
        // there is no backtick to say "this is not a path".
        let line = line.trim_start();
        let line = line
            .strip_prefix("///")
            .or_else(|| line.strip_prefix("//!"))
            .or_else(|| line.strip_prefix("//"))
            .or_else(|| line.strip_prefix("#"))
            .or_else(|| line.strip_prefix("*"))
            .unwrap_or(line);
        for raw in tokenize_code_span(line) {
            // Prose puts sentence punctuation against the path — `see
            // docs/a.md).` — which a citation never includes. Trailing-only:
            // a LEADING '(' has already been split off by the tokenizer, and
            // stripping from the front would eat a leading './'.
            let raw =
                raw.trim_end_matches(['.', ',', ';', ':', ')', ']', '}', '"', '\'', '!', '?']);
            if raw.is_empty() {
                continue;
            }
            // Prose needs its own admission rule, and this is the whole
            // false-positive defence. In a code span the backticks ARE the
            // signal that a token is a path; prose has no such signal, so
            // `classify` alone admits any slash-bearing word — measured before
            // this guard, one file went from 2 refs to 106, reporting
            // `overview/read` and `generated/vendored` as paths.
            //
            // Requiring a file extension is deliberately strict, and it has a
            // known cost: a citation written without one
            // (`docs/issues/2026-06-11-symbols-search-include-docs-and-focus`)
            // is missed. That is a malformed citation and better fixed in the
            // comment than accommodated by a fuzzier matcher here — a rule a
            // reader can predict beats a rule that catches slightly more.
            if !has_file_extension(raw) {
                continue;
            }
            if let Some(kind) = classify(raw, false, syntax) {
                out.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: (idx + 1) as u32,
                    raw_ref: raw.to_string(),
                    ref_kind: kind,
                    position: RefPosition::Prose,
                });
            }
        }
    }
    out
}

/// Does this token's last segment end in something that looks like a file
/// extension — `.md`, `.rs`, `.toml`?
///
/// Only [`parse_prose_refs`] uses this. A dot alone is not enough: prose is
/// full of sentence-final dots and version numbers, so the extension must be
/// short and alphanumeric, and must not be the whole segment (`.gitignore`
/// is a filename, not an extension, and carries no path to resolve).
fn has_file_extension(token: &str) -> bool {
    let last = token.rsplit('/').next().unwrap_or(token);
    let Some((stem, ext)) = last.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && !ext.is_empty()
        && ext.len() <= 8
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod prose_tests {
    use super::*;

    fn refs(text: &str) -> Vec<(u32, String)> {
        parse_prose_refs(text, Path::new("src/x.rs"), PathSyntax::ColonColonModules)
            .into_iter()
            .map(|c| (c.md_line, c.raw_ref))
            .collect()
    }

    #[test]
    fn a_bare_citation_in_a_comment_is_found() {
        // The whole point: only 140 of this repo's 699 in-source citations are
        // backticked, so a code-span-only scan saw a fifth of them.
        assert_eq!(
            refs("// see docs/issues/2026-01-01-a.md for why\n"),
            vec![(1, "docs/issues/2026-01-01-a.md".to_string())]
        );
    }

    #[test]
    fn sentence_punctuation_does_not_become_part_of_the_path() {
        // Prose writes `(see docs/a.md).` — a citation never contains the
        // trailing `).`, and leaving it attached makes every such ref
        // unresolvable for a reason that has nothing to do with drift.
        for line in [
            "// eviction cycle, see docs/a.md).",
            "// see docs/a.md.",
            "// see docs/a.md;",
            "/// see docs/a.md!",
        ] {
            assert_eq!(
                refs(line),
                vec![(1, "docs/a.md".to_string())],
                "failed for: {line}"
            );
        }
    }

    #[test]
    fn comment_markers_are_not_reported_as_paths() {
        // `//` and `///` contain a slash, and prose has no backticks to say
        // "not a path". Measured before the marker strip: one file reported
        // 106 refs, dozens of them the marker itself.
        assert!(refs("// nothing here\n").is_empty());
        assert!(refs("/// nothing here\n").is_empty());
        assert!(refs("//! nothing here\n").is_empty());
        assert!(refs("# nothing here\n").is_empty());
    }

    #[test]
    fn slash_bearing_prose_without_an_extension_is_not_a_path() {
        // The discriminator for `has_file_extension`. These are real strings
        // from this repo's comments that the unguarded version reported as
        // broken paths.
        for line in [
            "// the overview/read distinction",
            "// generated/vendored trees are skipped",
            "// references/symbol_at/call_graph/edit_code all do this",
        ] {
            assert!(refs(line).is_empty(), "false positive on: {line}");
        }
    }

    #[test]
    fn the_extension_rule_admits_real_extensions_and_rejects_prose_dots() {
        for good in ["docs/a.md", "src/main.rs", "a/b/c.toml", "x.py"] {
            assert!(has_file_extension(good), "{good} should pass");
        }
        for bad in [
            "docs/issues/2026-06-11-symbols-search-include-docs",
            "overview/read",
            "//",
            ".gitignore", // a filename, not an extension — no path to resolve
            "nodothere",
        ] {
            assert!(!has_file_extension(bad), "{bad} should NOT pass");
        }
    }

    #[test]
    fn the_extension_pre_filter_is_not_the_whole_decision() {
        // `has_file_extension` is deliberately cheap and permissive: it admits
        // `e.g` (stem `e`, ext `g`) and `1.2.3`, because tightening it enough
        // to reject those would also reject the genuine one-character
        // extensions `.c` and `.h`. `classify` is the second gate, and THIS is
        // the assertion that matters — that prose dots do not become findings.
        assert!(has_file_extension("e.g"), "the pre-filter admits it …");
        assert!(
            refs("// e.g. this one, see below").is_empty(),
            "… and classify is what rejects it"
        );
        assert!(refs("// bumped to version 1.2.3 today").is_empty());
        assert!(refs("// costs ~0.5ms per call").is_empty());
    }

    #[test]
    fn line_numbers_are_one_based_within_the_text_given() {
        // scan_code_comments rebases these onto the file; getting the origin
        // wrong here shifts every finding in a multi-line comment.
        assert_eq!(
            refs("// nothing\n// docs/b.md\n"),
            vec![(2, "docs/b.md".to_string())]
        );
    }
}

fn tokenize_code_span(s: &str) -> impl Iterator<Item = &str> + '_ {
    // Split on whitespace AND on punctuation that wraps path-like tokens in
    // realistic code shapes — function-call parens, quotes, commas, backticks.
    // Without this, a fenced-block line like
    //   read_markdown("docs/trackers/foo.md",
    // would be a single whitespace-separated token with the function-call
    // prefix attached, producing a missing-FilePath false positive on the
    // wrong string. Splitting on `(`, `)`, `"`, `,`, etc. lets the real path
    // surface as its own token.
    s.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ',' | ';' | '`'))
        .map(trim_token_edges)
        .filter(|t| !t.is_empty())
}
/// Whether a code span is *displaying markup* rather than making a reference.
///
/// A single-backtick span cannot contain a backtick, so any backtick inside the
/// span's content proves the author reached for multi-backtick delimiters —
/// `` `src/foo.py` `` — whose only purpose is to render a code span literally.
/// That is an illustration of what a reference looks like, not a reference. The
/// audit's own manual page is the motivating case: its "Reference kinds" table
/// shows one example ref per `ref_kind`, and every one of them was reported as
/// drift against this repo.
///
/// Skipped outright rather than severity-capped, for the same reason
/// `is_placeholder` skips a placeholder link target: it is not a citation at
/// all, so there is nothing to report at any band.
fn is_markup_display(content: &str) -> bool {
    content.contains('`')
}
/// True when an HTML comment carries the suppression marker.
///
/// Deliberately a `contains`, not an exact match: the marker is written inside a
/// comment that usually explains WHY, and requiring an exact string would force the
/// reason to live somewhere the next reader will not find it.
///
/// **Scope is the enclosing section, not the file** — suppression clears at the next
/// heading of any level, so a marker cannot leak past the passage it was reasoned
/// about. Use it for text that is reference-SHAPED but is not a reference: a removal
/// notice naming the path it removed, an example of what a citation looks like, a
/// quoted truncation. Do not use it to silence a reference you have not checked — the
/// whole value of the gate is that an unchecked stale path is loud.
fn is_ignore_marker(html: &str) -> bool {
    html.contains("audit-doc-refs:ignore")
}

/// Which refs a marker suppresses.
///
/// The bare `audit-doc-refs:ignore` form silences an entire section, which is the
/// only granularity that existed until 2026-09-02 and is too coarse for the case that
/// motivated this: `docs/PROBES.md` names two truncated paths as *examples of
/// truncation*, inside a section carrying **27** real refs. Silencing the section to
/// clear two false positives would leave 25 genuine citations unguarded in the one
/// document whose job is telling a reader which instrument to trust — so the coarse
/// form is not merely inconvenient there, it is the wrong trade.
///
/// The scoped form `audit-doc-refs:ignore-refs` names its targets in **backticks**:
///
/// ```text
/// <!-- audit-doc-refs:ignore-refs `src/serve` `src/lsp/m` — truncation examples -->
/// ```
///
/// Backticks are the delimiter rather than whitespace or commas because the marker
/// body also carries prose, and a whitespace split would read the explanation as
/// targets. That is this very parser's own defect class one level up: a grammar over
/// a namespace owes a way to say which token it means, and free prose beside a
/// token list makes the two indistinguishable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Suppression {
    /// No marker in scope.
    None,
    /// Bare `audit-doc-refs:ignore` — the whole section.
    All,
    /// `audit-doc-refs:ignore-refs` with backticked targets — only these raw refs.
    /// An empty list is unreachable: `parse_ignore_marker` degrades to `All` rather
    /// than silently suppressing nothing, because a marker someone wrote and that
    /// matches no ref is a typo, and failing open there would hide the finding they
    /// were trying to annotate.
    Only(Vec<String>),
}

impl Suppression {
    /// Whether this suppression hides `raw`.
    fn blocks(&self, raw: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Only(targets) => targets.iter().any(|t| t == raw),
        }
    }

    /// Whether the whole event can be skipped without inspecting its tokens.
    ///
    /// Kept separate from [`Self::blocks`] so the `Only` case still walks its tokens:
    /// collapsing the two would make a scoped marker behave like a bare one.
    fn blocks_everything(&self) -> bool {
        matches!(self, Self::All)
    }
}

/// Read a marker comment into the suppression it declares.
///
/// Returns `None` for a comment that is not a marker at all, so the caller can leave
/// the current suppression untouched rather than clearing it.
fn parse_ignore_marker(html: &str) -> Option<Suppression> {
    if !is_ignore_marker(html) {
        return None;
    }
    if !html.contains("audit-doc-refs:ignore-refs") {
        return Some(Suppression::All);
    }
    let targets: Vec<String> = backtick_re()
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .collect();
    // A scoped marker naming nothing is a typo, not an instruction to suppress
    // nothing — degrade to the coarse form so the author sees the effect they asked
    // for rather than a silently inert comment.
    if targets.is_empty() {
        return Some(Suppression::All);
    }
    Some(Suppression::Only(targets))
}

fn backtick_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`]+)`").unwrap())
}

/// Trim trailing sentence punctuation (period, brackets, braces) that often
/// sticks to a path-like token in prose: `See foo.md.` → `foo.md`.
/// Does NOT trim `:` (significant for FileLine refs like `file.rs:42`) or `/`.
fn trim_token_edges(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '[' | ']' | '{' | '}'))
        .trim_end_matches('.')
}

/// U-46: `is_module_path`'s character class — lowercase, digits, dots,
/// underscores — is also the shape of a version number (`1.16.8`) and of
/// common Latin abbreviations (`e.g`, `i.e`). Both were reaching prose as
/// `ModulePath` false positives. Rejected here rather than as a later
/// severity cap, because neither is a plausible qualified name at all — a
/// real one has at least one alphabetic segment, and neither abbreviation
/// names anything.
fn is_module_path(s: &str) -> bool {
    const KNOWN_ABBREVIATIONS: &[&str] = &["e.g", "i.e"];

    s.contains('.')
        && !s.contains('/')
        && !s.contains(char::is_whitespace)
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
        && s.split('.').all(|part| !part.is_empty())
        && s.split('.')
            .any(|part| part.chars().any(|c| c.is_alphabetic()))
        && !KNOWN_ABBREVIATIONS.contains(&s)
}

#[cfg(test)]
mod path_syntax_tests {
    use super::{classify, PathSyntax, RefKind};

    #[test]
    fn a_dotted_token_is_a_module_path_only_where_the_language_spells_them_that_way() {
        // The discriminating triple: one token, three languages, three verdicts.
        // `commits.git_root` is a SQL column named in a Rust doc comment; `os.path`
        // is a real Python module. They are the same string.
        let tok = "commits.git_root";
        assert_eq!(
            classify(tok, true, PathSyntax::DottedModules),
            Some(RefKind::ModulePath),
            "python/go/java/kotlin/ts spell qualified names with dots"
        );
        assert_eq!(
            classify(tok, true, PathSyntax::ColonColonModules),
            None,
            "rust spells them `a::b`, so a dotted token is field access"
        );
        assert_eq!(
            classify(tok, true, PathSyntax::NoModules),
            None,
            "shell/css/html have no module concept"
        );
    }

    #[test]
    fn narrowing_touches_only_the_module_branch() {
        // The guard that matters: `PathSyntax` must not cost us a single FILE
        // reference. Every kind below is classified before the module branch is
        // reached, so all three syntaxes must agree on them.
        for syntax in [
            PathSyntax::DottedModules,
            PathSyntax::ColonColonModules,
            PathSyntax::NoModules,
        ] {
            assert_eq!(
                classify("src/librarian/tools/scope.rs", true, syntax),
                Some(RefKind::FilePath),
                "{syntax:?} must still classify a plain path"
            );
            assert_eq!(
                classify("src/retrieval/config.rs:61", true, syntax),
                Some(RefKind::FileLine),
                "{syntax:?} must still classify a file:line"
            );
            assert_eq!(
                classify("src/ast/mod.rs::detect_language", true, syntax),
                Some(RefKind::FileSymbol),
                "{syntax:?} must still classify a file::symbol"
            );
            assert_eq!(
                classify("docs/FEATURES.md", false, syntax),
                Some(RefKind::FilePath),
                "{syntax:?} must still classify a prose path"
            );
        }
    }

    #[test]
    fn every_language_detect_language_emits_maps_deliberately() {
        // Keys copied from `crate::ast::detect_language`, its sole producer. A new
        // grammar arriving without a decision here lands on DottedModules — today's
        // behaviour — rather than silently losing refs, and this table is where that
        // decision gets made explicit.
        let cases: &[(&str, PathSyntax)] = &[
            ("rust", PathSyntax::ColonColonModules),
            ("python", PathSyntax::DottedModules),
            ("go", PathSyntax::DottedModules),
            ("java", PathSyntax::DottedModules),
            ("kotlin", PathSyntax::DottedModules),
            ("typescript", PathSyntax::DottedModules),
            ("tsx", PathSyntax::DottedModules),
            ("javascript", PathSyntax::DottedModules),
            ("jsx", PathSyntax::DottedModules),
            ("bash", PathSyntax::NoModules),
            ("css", PathSyntax::NoModules),
            ("scss", PathSyntax::NoModules),
            ("less", PathSyntax::NoModules),
            ("html", PathSyntax::NoModules),
        ];
        for (lang, want) in cases {
            assert_eq!(
                PathSyntax::for_language(Some(lang)),
                *want,
                "language `{lang}`"
            );
        }
    }

    #[test]
    fn markdown_and_unknown_languages_keep_todays_behaviour() {
        // Markdown reaches the classifier with no language and has already been
        // swept once (SD-1); changing its verdicts here would be an unrequested
        // behaviour change to the corpus carrying the most citations.
        assert_eq!(
            PathSyntax::for_language(None),
            PathSyntax::DottedModules,
            "markdown must be unchanged"
        );
        assert_eq!(
            PathSyntax::for_language(Some("some-future-grammar")),
            PathSyntax::DottedModules,
            "an unvendored language must arrive with today's behaviour, not a silent loss"
        );
    }

    #[test]
    fn a_bare_version_number_is_not_a_module_path() {
        // U-46: `1.16.8` (a plugin version in prose) satisfies `is_module_path`'s
        // character class — digits and dots only — and was classified ModulePath.
        // A real qualified name always has at least one alphabetic segment; a
        // token where every dot-separated segment is pure digits is a version
        // string, never a name.
        assert_eq!(
            classify("1.16.8", true, PathSyntax::DottedModules),
            None,
            "an all-numeric dotted token is a version number, not a qualified name"
        );
        assert_eq!(
            classify("1.0", true, PathSyntax::DottedModules),
            None,
            "two-segment version numbers must be rejected too"
        );
    }

    #[test]
    fn common_latin_abbreviations_are_not_module_paths() {
        // U-46: `e.g` and `i.e` satisfy the same character class as `os.path` —
        // dotted, lowercase, no slash — and were classified ModulePath in prose
        // that was never talking about a module at all.
        assert_eq!(
            classify("e.g", true, PathSyntax::DottedModules),
            None,
            "e.g is prose punctuation, not a qualified name"
        );
        assert_eq!(
            classify("i.e", true, PathSyntax::DottedModules),
            None,
            "i.e is prose punctuation, not a qualified name"
        );
    }

    #[test]
    fn the_new_filters_do_not_touch_real_module_paths() {
        // Positive control: a short real module path must still classify. Without
        // this, a rule broad enough to reject "e.g" could just as easily reject
        // "os.path" — both are two single-word-ish segments — and the two filters
        // above would be passing for the wrong reason.
        assert_eq!(
            classify("os.path", true, PathSyntax::DottedModules),
            Some(RefKind::ModulePath),
            "a real short module path must still classify"
        );
        assert_eq!(
            classify("commits.git_root", true, PathSyntax::DottedModules),
            Some(RefKind::ModulePath),
            "a real field/column reference with a numeric-free segment must still classify"
        );
    }
}

/// A documentation placeholder rather than a concrete target.
///
/// Two spellings, both of which this repo uses in naming-convention docs:
/// angle-bracket templates (`<date>-<slug>.md`, `<topic>-session-log.md`) and the
/// un-bracketed date template (`YYYY-MM-DD-slug.md`) used for issue, research, and
/// ADR filenames. No real filename carries a literal `YYYY`.
///
/// Applied to link targets as well as code-span tokens — a markdown link whose
/// target is a placeholder is an example of markup to copy, not a citation.
fn is_placeholder(s: &str) -> bool {
    s.contains('<') || s.contains('>') || s.contains("YYYY") || s.contains("yyyy")
}

/// One segment of an unanchored path candidate, spelled the way filesystem
/// paths actually are: lowercase letters, digits, and separator punctuation.
///
/// Capitalization is the discriminator. Real directory names are lowercase or
/// kebab/snake (`docs`, `crates`, `codescout-embed`); an uppercase segment in
/// an unanchored slash-joined token almost always means the token is an
/// identifier idiom rather than a path — `Type/method`, `LspClient/hover`,
/// `Kotlin/kotlin-lsp`, `rocks/v492/LOCK`, `mcpServers/codescout/env`.
/// Uppercase *file* names reach `looks_like_path` with an extension
/// (`README.md`) and are admitted before this rule runs.
fn is_path_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

fn looks_like_path(s: &str) -> bool {
    if s.contains(char::is_whitespace) {
        return false;
    }
    // Reject URI schemes (doc://, http://, file://, etc.) — they're handled
    // as links, not as filesystem paths.
    if has_uri_scheme(s) {
        return false;
    }
    // Reject obvious non-paths embedded in path-shaped strings — these are
    // common in documentation and produce noisy false positives when treated
    // as filesystem refs.
    if s.starts_with('~') {
        // Home-relative paths (~/.cargo/bin/foo, ~/.claude/config.json)
        // cannot be resolved against the project root.
        return false;
    }
    if s.starts_with("origin/") || s.starts_with("upstream/") {
        // Git refs (origin/master, upstream/main). Common inside `git`
        // command examples in markdown — not filesystem paths.
        return false;
    }
    if s.starts_with("path/to/") {
        // Documentation placeholder ("clone to `path/to/foo`, then ...").
        // Common in setup / agent-onboarding docs.
        return false;
    }
    if s.contains('*') {
        // Glob patterns (docs/**/*.md, *.rs, foo/*.txt) describe a shape, not
        // a concrete path.
        return false;
    }
    if is_placeholder(s) {
        // Template placeholders (<date>-<slug>.md, <topic>-session-log.md,
        // YYYY-MM-DD-slug.md) are documentation, not real paths.
        return false;
    }
    if s.contains('$') {
        // Shell expressions ($(pwd), ${VAR}, $HOME/foo).
        return false;
    }
    if s.contains('/') {
        // An explicit anchor is unambiguous author intent: they meant a
        // location on a filesystem, so classify and let the resolver judge.
        if s.starts_with('/') || s.starts_with("./") || s.starts_with("../") {
            // `/foo` with no further structure (no second segment, no extension)
            // is almost always a slash-command or shell shorthand in prose, not a
            // file path. Require either a second path segment or a known extension.
            //
            // "Segment" means a NON-EMPTY one. Testing for a second *slash*
            // instead admitted bare Rust comment markers — `//`, `///`, `//!` —
            // whose second segment is empty, and an empty segment is evidence of
            // nothing. Each then surfaced as an `unknown` finding carrying
            // "path outside active project; scope=umbrella required": advice that
            // reads as actionable about a token that is not a path in any scope.
            //
            // A trailing slash still counts as a second segment, because it is
            // the directory marker (`/docs/`), and a UNC-style `//server/share`
            // still has two non-empty segments — so neither of those regresses.
            let segments = s.split('/').filter(|seg| !seg.is_empty()).count();
            let has_second_segment = segments >= 2 || (segments == 1 && s.ends_with('/'));
            let single_root_segment = s.starts_with('/') && !has_second_segment;
            if single_root_segment {
                return has_known_ext(s);
            }
            return true;
        }
        // Unanchored `a/b` — require POSITIVE evidence of pathness instead of
        // accepting by default. Accept-by-default is what grew the rejection
        // list above one documentation idiom at a time, and it still let three
        // whole classes through: `Type/method` (codescout's own name_path
        // syntax), GitHub `org/repo` slugs, and JSON config pointers like
        // `mcpServers/codescout/env`. Each of those fails on capitalization,
        // which real directory names essentially never carry without also
        // carrying an extension (README.md, CHANGELOG.md — admitted above by
        // has_known_ext before this rule can see them).
        return has_known_ext(s) || s.ends_with('/') || s.split('/').all(is_path_segment);
    }
    has_known_ext(s)
}

fn has_uri_scheme(s: &str) -> bool {
    if let Some(colon) = s.find(':') {
        let scheme = &s[..colon];
        !scheme.is_empty()
            && scheme.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
            && s[colon..].starts_with("://")
    } else {
        false
    }
}

fn has_known_ext(s: &str) -> bool {
    let Some((prefix, ext)) = s.rsplit_once('.') else {
        return false;
    };
    if prefix.is_empty() {
        // Bare extension like ".rs" or ".py" — a documentation token
        // ("touch a `.rs` file"), not a filesystem path. Reject.
        return false;
    }
    matches!(
        ext,
        "rs" | "py" | "ts" | "js" | "kt" | "java" | "go" | "md" | "toml" | "yaml" | "yml" | "json"
    )
}

fn byte_offset_to_line(text: &str, offset: usize) -> u32 {
    1 + text[..offset.min(text.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count() as u32
}
fn fence_warnings(text: &str, md_file: &str) -> Vec<ParseWarning> {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    let re = FENCE_RE.get_or_init(|| Regex::new(r"(?m)^```").unwrap());
    let opens: Vec<_> = re.find_iter(text).collect();
    if opens.len() % 2 == 1 {
        let last = opens.last().unwrap();
        let line = 1 + text[..last.start()].bytes().filter(|&b| b == b'\n').count() as u32;
        vec![ParseWarning {
            md_file: md_file.to_string(),
            line,
            reason: "unterminated code fence".to_string(),
        }]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::tools::audit_doc_refs::{RefKind, RefPosition};
    use std::path::PathBuf;

    fn parse(text: &str) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
        parse_refs(text, &PathBuf::from("test.md"), PathSyntax::DottedModules)
    }

    #[test]
    fn parser_resolves_simple_file_path() {
        let (cands, _) = parse("See `src/foo.py` for the entry point.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].raw_ref, "src/foo.py");
        assert_eq!(cands[0].ref_kind, RefKind::FilePath);
        assert_eq!(cands[0].position, RefPosition::InlineSpan);
    }
    /// The audit's own manual page documents each `ref_kind` with one example ref,
    /// written with double-backtick delimiters so the inner backticks render. Every
    /// one of those examples was reported as drift against this repo — the tool
    /// flagging its own documentation. A span whose content carries a backtick is
    /// markup being displayed, not a reference being made.
    #[test]
    fn parser_skips_a_code_span_that_is_displaying_markup() {
        let (cands, _) =
            parse("| `file_path` | extension-bearing path | `` `src/mrv/chat_app.py` `` |");
        assert!(
            !cands.iter().any(|c| c.raw_ref == "src/mrv/chat_app.py"),
            "a markup-display span should yield no candidate; got {:?}",
            cands.iter().map(|c| &c.raw_ref).collect::<Vec<_>>()
        );

        // Over-match guard: an ordinary single-backtick span naming the same path is
        // a citation and must still be extracted. Without this the skip could
        // swallow every code span and still look correct.
        let (cands, _) = parse("The reader is `src/mrv/chat_app.py` itself.");
        assert!(cands.iter().any(|c| c.raw_ref == "src/mrv/chat_app.py"));
    }
    /// The marker's scope is a section, so the test that matters is the one showing
    /// where suppression *stops*. A version that never cleared the flag would pass
    /// any single-section assertion and silence the rest of the file.
    #[test]
    fn parser_ignore_marker_suppresses_to_the_next_heading_only() {
        let md = "\
## Examples

<!-- audit-doc-refs:ignore -->

Walk through `src/services/auth.rs`, then run:

```bash
grep -r foo src/services/
```

See [the sample](src/foo.py) too.

## Real references

The extractor lives in `src/librarian/tools/audit_doc_refs/parser.rs`.
";
        let (cands, _) = parse(md);
        let refs: Vec<&str> = cands.iter().map(|c| c.raw_ref.as_str()).collect();

        for silenced in ["src/services/auth.rs", "src/services/", "src/foo.py"] {
            assert!(
                !refs.contains(&silenced),
                "{silenced} is inside the marked section; got {refs:?}"
            );
        }

        // The heading ended it. Without this the marker would silence everything
        // after it, which is the failure mode a per-section scope exists to avoid.
        assert!(
            refs.contains(&"src/librarian/tools/audit_doc_refs/parser.rs"),
            "suppression must stop at the next heading; got {refs:?}"
        );
    }
    /// The scoped form silences the tokens it names and NOTHING else.
    ///
    /// This is the case the bare marker could not serve: `docs/PROBES.md` names two
    /// truncated paths as examples of truncation, in a section carrying 27 real
    /// refs. The fixture mirrors that shape — two targets to silence, one genuine
    /// citation beside them that must survive.
    ///
    /// **The surviving ref is the load-bearing half of this test.** Assert only that
    /// the examples are gone and the test passes just as well against
    /// `Suppression::All`, which is the behaviour this form exists to avoid.
    #[test]
    fn ignore_refs_silences_only_the_named_tokens() {
        let md = "\
## Examples

<!-- audit-doc-refs:ignore-refs `src/serve` `src/lsp/m` — truncation examples, not citations -->

A 200-char cut leaves a truncated path (`src/serve`, `src/lsp/m`); the real probe is
`scripts/peer-sessions.sh`.
";
        let (refs, _) = parse_refs(md, Path::new("d.md"), PathSyntax::DottedModules);
        let got: Vec<&str> = refs.iter().map(|r| r.raw_ref.as_str()).collect();
        assert!(
            !got.contains(&"src/serve") && !got.contains(&"src/lsp/m"),
            "named targets must be suppressed, got {got:?}"
        );
        assert!(
            got.contains(&"scripts/peer-sessions.sh"),
            "a ref the marker did NOT name must still be audited — without this the \
             test cannot tell `ignore-refs` from a bare `ignore`, got {got:?}"
        );
    }

    /// A scoped marker still clears at the next heading, like the bare form.
    ///
    /// Guards the seam between the two forms: `Suppression::None` is assigned on
    /// `Tag::Heading` regardless of which variant was active, and nothing else
    /// asserts that the scoped variant is included in that reset.
    #[test]
    fn ignore_refs_scope_ends_at_the_next_heading() {
        let md = "\
## First

<!-- audit-doc-refs:ignore-refs `src/serve` -->

Here `src/serve` is an example.

## Second

Here `src/serve` is a citation again.
";
        let (refs, _) = parse_refs(md, Path::new("d.md"), PathSyntax::DottedModules);
        let hits: Vec<u32> = refs
            .iter()
            .filter(|r| r.raw_ref == "src/serve")
            .map(|r| r.md_line)
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "exactly the occurrence AFTER the next heading survives; got lines {hits:?}"
        );
    }

    /// A scoped marker naming nothing degrades to the coarse form, not to inert.
    ///
    /// The failing-open alternative is the dangerous one: a typo'd marker that
    /// suppresses nothing looks identical to no marker at all, so the author sees
    /// the finding they were annotating and assumes the marker is wrong about the
    /// ref rather than about its own syntax.
    #[test]
    fn ignore_refs_with_no_backticked_target_falls_back_to_suppressing_all() {
        let md = "\
## Examples

<!-- audit-doc-refs:ignore-refs but I forgot the backticks -->

A ref: `src/serve`, and another: `scripts/peer-sessions.sh`.
";
        let (refs, _) = parse_refs(md, Path::new("d.md"), PathSyntax::DottedModules);
        assert!(
            refs.is_empty(),
            "an empty target list must not silently suppress nothing, got {refs:?}"
        );
    }

    /// Over-match guard for the marker: an unmarked section behaves exactly as
    /// before. Asserted separately so a regression cannot hide behind the
    /// suppression assertions above.
    #[test]
    fn parser_extracts_normally_without_an_ignore_marker() {
        let md = "\
## Examples

Walk through `src/services/auth.rs`, then see [the sample](src/foo.py).
";
        let (cands, _) = parse(md);
        let refs: Vec<&str> = cands.iter().map(|c| c.raw_ref.as_str()).collect();
        assert!(refs.contains(&"src/services/auth.rs"), "got {refs:?}");
        assert!(refs.contains(&"src/foo.py"), "got {refs:?}");
    }

    #[test]
    fn parser_ignores_prose_outside_code_spans() {
        let (cands, _) = parse("We use Pydantic for validation.");
        assert_eq!(cands.len(), 0);
    }

    #[test]
    fn parser_rejects_root_single_segment_without_extension() {
        // `/claude-traces`, `/mcp`, `/tmp` etc. are slash-commands or shell
        // shorthand in prose — not file paths. Reject them in code spans.
        let (cands, _) =
            parse("Run `/claude-traces` then `/mcp`; also `/tmp` is not a project file.");
        let kinds: Vec<_> = cands
            .iter()
            .map(|c| (c.raw_ref.as_str(), c.ref_kind))
            .collect();
        assert!(
            kinds.is_empty(),
            "expected no path candidates, got {kinds:?}",
        );
    }

    #[test]
    fn parser_accepts_root_single_segment_with_extension() {
        // `/foo.rs` is plausibly an absolute path — keep accepting it so
        // genuine absolute file refs still resolve.
        let (cands, _) = parse("See `/foo.rs` for the reference impl.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].raw_ref, "/foo.rs");
        assert_eq!(cands[0].ref_kind, RefKind::FilePath);
    }

    #[test]
    fn parser_accepts_multi_segment_absolute_path() {
        let (cands, _) = parse("Check `/usr/local/bin/codescout`.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].raw_ref, "/usr/local/bin/codescout");
    }

    #[test]
    fn parser_rejects_bare_comment_markers() {
        // `//`, `///` and `//!` appear in almost every Rust snippet. Each used to
        // classify as a file_path and surface as an `unknown` finding carrying
        // "path outside active project; scope=umbrella required" — advice that
        // reads as actionable about a token that is not a path in any scope.
        // docs/issues/archive/2026-08-15-audit-doc-refs-classifies-comment-markers-as-paths.md
        let (cands, _) = parse("Doc comments use `///`, inner ones `//!`, plain ones `//`.");
        let kinds: Vec<_> = cands
            .iter()
            .map(|c| (c.raw_ref.as_str(), c.ref_kind))
            .collect();
        assert!(
            kinds.is_empty(),
            "expected no path candidates, got {kinds:?}"
        );
    }

    #[test]
    fn parser_keeps_anchored_paths_the_marker_fix_could_have_broken() {
        // The discriminating set. Each is accepted for a DIFFERENT reason, so a
        // regression in any one of them is legible from which assert fails:
        // two real segments, a trailing-slash directory marker, and a Windows UNC
        // share — the last being why the broader "reject slash-only strings" fix
        // was rejected in favour of counting non-empty segments.
        let (cands, _) = parse("See `/etc/hosts`, the `/docs/` tree, and `//server/share`.");
        let refs: Vec<&str> = cands.iter().map(|c| c.raw_ref.as_str()).collect();
        assert!(
            refs.contains(&"/etc/hosts"),
            "two non-empty segments must still be a path; got {refs:?}"
        );
        assert!(
            refs.contains(&"/docs/"),
            "a trailing slash is the directory marker; got {refs:?}"
        );
        assert!(
            refs.contains(&"//server/share"),
            "UNC still has two non-empty segments; got {refs:?}"
        );
    }

    #[test]
    fn parser_classifies_file_line_over_file_path() {
        let (cands, _) = parse("at `scripts/eval_chunking.py:807` we see...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileLine);
        assert_eq!(cands[0].raw_ref, "scripts/eval_chunking.py:807");
    }

    #[test]
    fn parser_classifies_file_symbol_over_file_line() {
        let (cands, _) = parse("see `src/mrv/cli.py:cmd_generate` for...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);

        let (cands, _) = parse("see `src/foo.rs:Bar/baz` for...");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);
    }

    #[test]
    fn parser_module_path_requires_code_context() {
        // Prose — must NOT classify
        let (cands, _) = parse("We import from mrv.chat_app in the runner.");
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::ModulePath),
            "prose dotted-ident must not emit ModulePath"
        );

        // Code span — must classify
        let (cands, _) = parse("Use `mrv.chat_app` here.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::ModulePath);
    }

    #[test]
    fn parser_extracts_link_targets() {
        let (cands, _) = parse("[label](src/foo.py)");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::Link);
        assert_eq!(cands[0].position, RefPosition::LinkTarget);
    }

    #[test]
    fn parser_walks_fenced_code_blocks() {
        let text = "```\nimport mrv.chat_app\n```\n";
        let (cands, _) = parse(text);
        // expect at least one module_path candidate from the fenced block
        assert!(cands.iter().any(|c| c.ref_kind == RefKind::ModulePath));
    }
    #[test]
    fn parser_recovers_from_unterminated_fence() {
        let text = "intro\n```\nsome code without close\n";
        let (_cands, warns) = parse(text);
        assert!(
            !warns.is_empty(),
            "expected at least one parse_warning for unterminated fence"
        );
        assert!(warns[0].reason.contains("fence") || warns[0].reason.contains("unterminated"));
    }

    #[test]
    fn parser_rejects_home_relative_paths() {
        // ~/.cargo/bin/foo cannot be resolved against the project root —
        // treat as informational text, not a missing ref.
        let (cands, _) = parse("See `~/.cargo/bin/codescout` for the binary.");
        assert!(
            cands.is_empty(),
            "home-relative path must not classify as FilePath, got {cands:?}"
        );
    }

    #[test]
    fn parser_rejects_glob_patterns() {
        // `docs/**/*.md`, `docs/issues/*.md`, `**/*.rs` etc. describe a shape,
        // not a real path. Common in documentation; do not flag as missing.
        let cases = [
            "Default scope: `docs/**/*.md`.",
            "Run audit over `docs/trackers/*.md` once a week.",
            "All `**/*.rs` files in the workspace.",
        ];
        for case in cases {
            let (cands, _) = parse(case);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "expected no FilePath candidate for {case:?}, got {cands:?}"
            );
        }
    }

    #[test]
    fn parser_rejects_template_placeholders() {
        // `<date>`, `<slug>`, `YYYY-MM-DD` are documentation placeholders —
        // even if the surrounding shape looks like a real path, the value
        // is symbolic.
        let cases = [
            "Open `docs/issues/<date>-<slug>.md`.",
            "Template at `docs/issues/YYYY-MM-DD-<slug>.md`.",
            "Append to `docs/trackers/<topic>-session-log.md`.",
        ];
        for case in cases {
            let (cands, _) = parse(case);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "expected no FilePath candidate for {case:?}, got {cands:?}"
            );
        }
    }

    #[test]
    fn parser_rejects_shell_expressions() {
        // $(pwd), ${VAR}, $HOME/x are shell-eval shapes, not paths to verify.
        let (cands, _) = parse("Run `ln -sf \"$(pwd)/target/release/codescout\" foo`.");
        assert!(
            cands
                .iter()
                .all(|c| !c.raw_ref.contains('$') || c.ref_kind != RefKind::FilePath),
            "shell expression must not classify as FilePath, got {cands:?}"
        );
    }

    #[test]
    fn parser_strips_wrapping_punctuation_from_code_block_tokens() {
        // Code fences often have call-site shapes like
        //   read_markdown("docs/foo.md")
        // The whitespace tokenizer used to keep the trailing `,` / quotes
        // attached, producing a missing FilePath finding on the wrong string.
        // After the trim, the bare path inside resolves correctly.
        let text = "```\nread_markdown(\"docs/trackers/skill-frictions.md\",\n  action=\"insert_after\")\n```\n";
        let (cands, _) = parse(text);
        assert!(
            cands
                .iter()
                .any(|c| c.raw_ref == "docs/trackers/skill-frictions.md"
                    && c.ref_kind == RefKind::FilePath),
            "expected the bare path to be extracted from the code-block call shape, got {cands:?}"
        );
        // And nothing should retain the wrapping `,` or `"`.
        assert!(
            cands.iter().all(|c| !c.raw_ref.ends_with(',')
                && !c.raw_ref.starts_with('"')
                && !c.raw_ref.ends_with('"')),
            "tokens must be trimmed of wrapping punctuation, got {cands:?}"
        );
    }

    #[test]
    fn parser_rejects_git_refs() {
        // origin/master, upstream/main are git refs (common in `git` command
        // examples) — not filesystem paths.
        let (cands, _) =
            parse("Run `git rev-parse master experiments origin/master origin/experiments`.");
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
            "expected no FilePath candidate for git refs, got {cands:?}"
        );
        let (cands, _) = parse("Push to `upstream/main` not `origin/main`.");
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
            "expected no FilePath candidate for git refs, got {cands:?}"
        );
    }

    #[test]
    fn parser_handles_rust_double_colon_symbol_separator() {
        // src/foo.rs::symbol should produce path="src/foo.rs", suffix="symbol".
        // Pre-fix used rsplit_once(':') which left a trailing colon on the
        // path part, causing resolver to look for a nonexistent file.
        let (cands, _) = parse("see `src/prompts/source.rs::extract_surface` for the parser.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);
        // raw_ref retains the original form; resolver re-parses it
        assert_eq!(cands[0].raw_ref, "src/prompts/source.rs::extract_surface");
    }

    #[test]
    fn parser_rejects_path_to_placeholder() {
        // "path/to/X" is a documentation placeholder, not a filesystem path.
        // Common shape in agent-onboarding docs: "clone to `path/to/foo`".
        let (cands, _) = parse("Replace `path/to/copilot-codescout` with your clone location.");
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
            "expected no FilePath candidate for placeholder, got {cands:?}"
        );
        let (cands, _) = parse("Run `cp path/to/codescout/Skills/* .github/skills/`.");
        assert!(
            cands
                .iter()
                .all(|c| c.raw_ref != "path/to/codescout/Skills"),
            "expected no FilePath candidate for placeholder prefix, got {cands:?}"
        );
    }

    #[test]
    fn parser_rejects_bare_extension_as_path() {
        // Inline code spans containing only a file extension (`.rs`, `.py`)
        // are documentation tokens ("touch a `.rs` file"), not file paths.
        for ext in [
            ".rs", ".py", ".ts", ".js", ".md", ".toml", ".yaml", ".yml", ".json",
        ] {
            let text = format!("Edit a `{ext}` file.");
            let (cands, _) = parse(&text);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "bare ext '{ext}' must not classify as FilePath, got: {cands:?}"
            );
        }
    }
    /// `Type/method` is codescout's own `name_path` symbol syntax, accepted by
    /// `symbols(name_path=…)` and `edit_code(symbol=…)`. It is not a path.
    #[test]
    fn parser_rejects_name_path_symbol_syntax() {
        for case in [
            "See `Type/method` for the shape.",
            "The `SensitiveString/fmt` impl redacts.",
            "`LspClient/hover` returns None when offline.",
        ] {
            let (cands, _) = parse(case);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "name_path must not classify as FilePath, got {cands:?} for {case}"
            );
        }
    }

    /// GitHub `org/repo` and Homebrew tap slugs live next to upstream URLs in
    /// prose about other projects.
    #[test]
    fn parser_rejects_org_repo_slugs() {
        for case in [
            "Upstream `Kotlin/kotlin-lsp` state as of today.",
            "Run `brew install JetBrains/utils/kotlin-lsp` to get it.",
        ] {
            let (cands, _) = parse(case);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "org/repo slug must not classify as FilePath, got {cands:?} for {case}"
            );
        }
    }

    /// JSON/config pointers (`mcpServers/codescout/env`) and elided external
    /// paths (`…/rocks/v492/LOCK`) are the two remaining accept-by-default
    /// classes from the slash branch.
    #[test]
    fn parser_rejects_config_pointers_and_elided_paths() {
        for case in [
            "Set `mcpServers/codescout/env` in the client config.",
            "The JVM holds `rocks/v492/LOCK` until it exits.",
            "The lock lives at `…/rocks/v492/LOCK` under the analyzer home.",
        ] {
            let (cands, _) = parse(case);
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::FilePath),
                "config pointer / elided path must not classify as FilePath, got {cands:?} for {case}"
            );
        }
    }

    /// The regression guard for the positive-evidence rule: tightening the
    /// slash branch must not cost us extension-less *directory* refs, which are
    /// a large share of what the audit legitimately checks.
    #[test]
    fn parser_still_accepts_extensionless_directory_refs() {
        for case in [
            "docs/issues",
            "src/lsp/mux",
            "crates/codescout-embed",
            ".github/workflows",
        ] {
            let (cands, _) = parse(&format!("See `{case}` for details."));
            assert!(
                cands
                    .iter()
                    .any(|c| c.raw_ref == case && c.ref_kind == RefKind::FilePath),
                "directory ref {case} must still classify as FilePath, got {cands:?}"
            );
        }
    }
    /// Naming-convention docs show the shape of a filename, sometimes inside a
    /// markdown link. Link targets are otherwise unfiltered (an explicit link is
    /// author intent), so the placeholder check has to run there too.
    #[test]
    fn parser_rejects_date_template_placeholder_links() {
        let (cands, _) = parse(
            "> **Superseded** by [YYYY-MM-DD-slug.md](./YYYY-MM-DD-slug.md) — one-line reason.\n",
        );
        assert!(
            cands.is_empty(),
            "a date-template placeholder link is an example, not a citation; got {cands:?}"
        );
    }

    /// The complement: a real link target must still be extracted, or the
    /// placeholder filter would have disarmed link checking entirely.
    #[test]
    fn parser_still_extracts_concrete_link_targets() {
        let (cands, _) = parse("See [the guide](docs/manual/src/architecture.md) for detail.\n");
        assert!(
            cands
                .iter()
                .any(|c| c.raw_ref == "docs/manual/src/architecture.md"
                    && c.ref_kind == RefKind::Link),
            "concrete link targets must still be checked; got {cands:?}"
        );
    }

    #[test]
    fn parser_classifies_file_line_range() {
        // `path:N-M` should be FileLine, not FilePath. Before the range parser
        // landed, classify() rsplit_once(':') saw a non-digit suffix and fell
        // through to FilePath, which then resolved as Missing because no file
        // literally named `path:N-M` exists.
        let (cands, _) = parse("See `src/tools/core/types.rs:238-246` for the impl.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::FileLine);
        assert_eq!(cands[0].raw_ref, "src/tools/core/types.rs:238-246");
    }
}
