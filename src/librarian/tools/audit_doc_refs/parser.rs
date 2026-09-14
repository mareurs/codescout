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
                for (row, raw) in tokenize_code_span(content.as_ref()) {
                    if suppression.blocks(raw) {
                        continue;
                    }
                    if let Some(kind) = classify(raw, true, syntax) {
                        // A token the author already labelled a git patch-id is not an
                        // artifact citation, and nothing about the token can say so —
                        // both namespaces are 16 lowercase hex. Applied here, at the
                        // INLINE arm, because that is the position whose band gates;
                        // a fenced one takes `cap_code_block` and never reds.
                        if kind == RefKind::ArtifactId && labelled_as_patch_id(text, span.start) {
                            continue;
                        }
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line + row,
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
                // `line` is where the EVENT's span starts, which for a fenced block is
                // its first content line and not the line each ref sits on. `row`
                // recovers the rest — see `tokenize_code_span`.
                for (row, raw) in tokenize_code_span(content.as_ref()) {
                    if suppression.blocks(raw) {
                        continue;
                    }
                    if let Some(kind) = classify(raw, true, syntax) {
                        candidates.push(RefCandidate {
                            md_file: md_file.clone(),
                            md_line: line + row,
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
    // Checked first because the shape is unambiguous and cheap: exactly 16 lowercase
    // hex digits, nothing else. No sibling classifier can claim it — `looks_like_path`
    // ends at `has_known_ext` for a token with no `/` and no `.`, so a bare id
    // classified as `None` before this arm existed.
    if is_artifact_id(s) {
        return Some(RefKind::ArtifactId);
    }
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

/// Whether the inline span starting at `span_start` is LABELLED a git patch-id.
///
/// `git patch-id --stable` emits 40 hex, but this corpus records it truncated to 16 — which
/// is byte-identical to an artifact id, and `CLAUDE.md` § *Bug Tracking* MANDATES recording
/// one beside every fix SHA. So the two namespaces collide by construction and no property
/// of the token itself can separate them: the discriminator is the label the author already
/// wrote, and it was sitting in the text unused.
///
/// Measured 2026-09-14 over `docs/**.md`: 26 such labelled tokens, and the real audit
/// reported 10 of the work queue's 28 `ArtifactMissing` findings against records written
/// exactly as the project requires. Tightening the band without this would red CI on
/// compliance.
///
/// **The gap between label and token must contain no letters**, which is the whole
/// precision of the rule and not a detail. The corpus writes `, patch-id `, `(patch-id `
/// and `, patch-id **` — punctuation and markup — but it also writes *"The patch-id belongs
/// to `d5af3d3ceff1d08c`"*, where the token is a genuine artifact id and the label is the
/// SUBJECT of the sentence rather than a tag on the value. A proximity-only rule suppresses
/// that one, which is the false negative this class invites: a mention about patch-ids
/// swallowing a real citation.
fn labelled_as_patch_id(text: &str, span_start: usize) -> bool {
    // The window deliberately CROSSES line breaks, and that is not a loosening.
    //
    // A same-line scan misses the wrapped form, which this corpus writes freely:
    //
    //     **DONE 2026-08-30** — `c2039a16`, patch-id
    //        `63a943ba8e2a1a9b`. 9 sidecars under ...
    //
    // and `docs/issues/archive/2026-09-02-declared-patch-ids-per-line-scan-misses-a-wrapped-value.md`
    // already recorded that exact blind spot in a NEIGHBOURING detector. The first cut of
    // this function reproduced it anyway.
    //
    // Crossing the break is safe because the discriminator is the no-letters gap, not the
    // line: a newline and its indentation are whitespace, so the rule still refuses to
    // reach back through any prose. 240 bytes bounds the walk; the widest real gap in the
    // corpus is under 30.
    const WINDOW: usize = 240;
    let lo = text[..span_start]
        .char_indices()
        .rev()
        .take_while(|(i, _)| span_start - i <= WINDOW)
        .last()
        .map_or(0, |(i, _)| i);
    let before = &text[lo..span_start];
    let Some(at) = before.to_ascii_lowercase().rfind("patch-id") else {
        return false;
    };
    let gap = &before[at + "patch-id".len()..];
    !gap.chars().any(|c| c.is_ascii_alphabetic())
}

/// Exactly 16 lowercase hex digits — a librarian artifact id.
///
/// The length is the whole discriminator, and it is why a **40**-hex git SHA does not
/// match: this is called on a token the tokenizer already split on word boundaries, so
/// a SHA arrives whole and fails the length test rather than contributing a 16-char
/// prefix. `link_scan` gets the same property from `\b[0-9a-f]{16}\b`'s anchors
/// (`link_scan::extract::id_re`); stated differently here because there is no regex,
/// and re-derived rather than assumed — verified against a 40-hex literal on the way in.
///
/// Uppercase is rejected on purpose. Ids are minted lowercase by
/// `crate::librarian::ids::artifact_id_from_abs`, so an uppercase 16-hex run is
/// something else — a truncated hash in a fixture, a colour table, a test vector.
fn is_artifact_id(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
        // The offset is always 0 here: this loop feeds ONE line at a time, and the
        // enclosing `text.lines()` enumeration already owns the line number.
        for (_, raw) in tokenize_code_span(line) {
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

/// The token separators for `tokenize_code_span`. A free `fn` rather than a closure so
/// it can be handed to `str::find` in both polarities without borrow gymnastics.
fn is_code_span_sep(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | ')' | '"' | '\'' | ',' | ';' | '`')
}

/// Split a code span or fenced-block body into path-like tokens, yielding each token
/// with the number of newlines that precede it **within `s`**.
///
/// Splits on whitespace AND on punctuation that wraps path-like tokens in realistic
/// code shapes — function-call parens, quotes, commas, backticks. Without this, a
/// fenced-block line like
///   read_markdown("docs/trackers/foo.md",
/// would be a single whitespace-separated token with the function-call prefix
/// attached, producing a missing-FilePath false positive on the wrong string.
/// Splitting on `(`, `)`, `"`, `,`, etc. lets the real path surface as its own token.
///
/// **The line offset is load-bearing, and dropping it is a silent defect rather than a
/// compile error — which is why it is returned rather than left to the caller.**
/// pulldown-cmark hands a fenced block's entire body to `parse_refs` as ONE
/// `Event::Text` whose span starts at the block's *first content line*. A caller that
/// stamps every token with the event's start line therefore misattributes every ref
/// below the block's first line, by a distance that grows with the block's height —
/// unbounded, not off-by-one. `parse_prose_refs` feeds one line at a time and so always
/// receives 0; that is the exception, not the contract.
///
/// A token can never *contain* a newline, because `\n` is whitespace and therefore a
/// separator — which is why a token has a line and not a line range. `trim_token_edges`
/// only strips `[]{}` and a trailing `.`, so trimming cannot move a token across a line
/// either. Both facts are what make this offset exact rather than approximate.
fn tokenize_code_span(s: &str) -> impl Iterator<Item = (u32, &str)> + '_ {
    let mut cursor = 0usize;
    let mut newlines = 0u32;
    std::iter::from_fn(move || {
        while cursor < s.len() {
            let rest = &s[cursor..];
            // Separator runs are the only place a '\n' can sit, so counting them here
            // counts every newline in `s` exactly once.
            let Some(skip) = rest.find(|c: char| !is_code_span_sep(c)) else {
                cursor = s.len();
                return None;
            };
            newlines += rest[..skip].bytes().filter(|&b| b == b'\n').count() as u32;
            let start = cursor + skip;
            let tail = &s[start..];
            let end = start + tail.find(is_code_span_sep).unwrap_or(tail.len());
            let at = newlines;
            let raw = &s[start..end];
            cursor = end;
            let token = trim_token_edges(raw);
            if !token.is_empty() {
                return Some((at, token));
            }
        }
        None
    })
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
    #[test]
    fn a_fenced_block_ref_reports_its_own_line_not_the_blocks_first() {
        // The core regression. Before the fix BOTH refs reported line 4 — the fenced
        // block's first CONTENT line — because pulldown-cmark hands a fence's whole
        // body over as one `Event::Text` and `md_line` was read off that event's span
        // start.
        //
        // The fence deliberately contains NO backtick, and that is this fixture's
        // load-bearing detail. The filed bug diagnosed a backtick desynchronising an
        // inline-code counter; this case is the falsifier, because the drift is here
        // without one. Add a backtick and the test still passes while no longer being
        // able to tell the two explanations apart.
        //
        // line:   1       2  3    4          5          6
        let md = "intro\n\n```\ndocs/a.md\ndocs/b.md\n```\n";
        let (cands, _) = parse(md);
        let got: Vec<(u32, &str)> = cands
            .iter()
            .map(|c| (c.md_line, c.raw_ref.as_str()))
            .collect();
        assert_eq!(got, vec![(4, "docs/a.md"), (5, "docs/b.md")]);
    }
    #[test]
    fn fenced_line_drift_grows_with_the_block_it_is_not_an_off_by_one() {
        // Pins the MAGNITUDE, which the sibling above cannot: across a two-line fence,
        // "attribute to the block's first line" and "subtract one" give the same
        // answer. The filed bug read the defect as an off-by-one for exactly that
        // reason — both fences it sampled happened to be two lines tall, so the number
        // it published was a property of its sample, not of the defect.
        //
        // The 40 filler lines are the whole point of the fixture: shrink them and this
        // silently stops discriminating a real fix from a `line + 1`, which would
        // answer 5 here.
        let filler = 40u32;
        let mut md = String::from("intro\n\n```\n"); // fence opens on line 3
        for _ in 0..filler {
            md.push_str("filler\n"); // lines 4..=43
        }
        md.push_str("docs/deep.md\n```\n"); // ref on line 44
        let want = 3 + filler + 1;
        let (cands, _) = parse(&md);
        let got: Vec<(u32, &str)> = cands
            .iter()
            .map(|c| (c.md_line, c.raw_ref.as_str()))
            .collect();
        assert_eq!(got, vec![(want, "docs/deep.md")]);
    }

    #[test]
    fn a_backtick_inside_a_fence_does_not_change_attribution() {
        // The control that keeps the falsified explanation falsified. A future reader
        // meeting a line-drift report here must not "fix" it by suspending an
        // inline-code counter inside fences: there is no such counter — pulldown-cmark
        // owns fence state — and this case would stay green while the real defect
        // returned. Drift here is identical in magnitude to the no-backtick sibling.
        //
        // line:   1       2  3    4             5          6
        let md = "intro\n\n```\nre = \"[`*]\"\ndocs/b.md\n```\n";
        let (cands, _) = parse(md);
        let got: Vec<(u32, &str)> = cands
            .iter()
            .map(|c| (c.md_line, c.raw_ref.as_str()))
            .collect();
        assert_eq!(got, vec![(5, "docs/b.md")]);
    }

    #[test]
    fn an_inline_code_span_folds_its_newline_so_the_row_offset_is_inert_there() {
        // INERT FIXTURE, annotated as inert so nobody credits it with coverage it does
        // not provide. `parse_refs`' `Event::Code` arm adds the same `line + row`
        // offset the fenced arm does, but pulldown-cmark normalises a newline inside
        // an inline code span to a SPACE — so `row` is observably always 0 there and
        // that arm's offset is never exercised by any input. Measured, not assumed:
        // without the fold this would return [(1, a), (2, b)].
        //
        // Kept as a TRIPWIRE rather than deleted. If pulldown-cmark ever stops
        // folding, this reds — and that red is the notice that the inline arm has
        // become live and now needs a real case. Deleting the fixture buys nothing and
        // removes the only thing that would ever say so.
        let md = "see `docs/a.md\ndocs/b.md` here\n";
        let (cands, _) = parse(md);
        let got: Vec<(u32, &str)> = cands
            .iter()
            .map(|c| (c.md_line, c.raw_ref.as_str()))
            .collect();
        assert_eq!(got, vec![(1, "docs/a.md"), (1, "docs/b.md")]);
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
    /// `symbols(symbol=…)` and `edit_code(symbol=…)`. It is not a path.
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

    // ---- artifact ids --------------------------------------------------------

    #[test]
    fn a_sixteen_hex_token_classifies_as_an_artifact_id() {
        let cands = parse_refs(
            "the queue cites `403e3fad0356f171` for this row\n",
            Path::new("docs/trackers/q.md"),
            PathSyntax::NoModules,
        )
        .0;
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].ref_kind, RefKind::ArtifactId);
        assert_eq!(cands[0].raw_ref, "403e3fad0356f171");
        assert_eq!(cands[0].position, RefPosition::InlineSpan);
    }

    /// A 40-hex git SHA must not contribute a 16-char prefix.
    ///
    /// Both identifiers are lowercase hex and they sit side by side throughout this
    /// corpus — every archived bug file cites a fix SHA and an artifact id within a few
    /// lines of each other. The length test is the whole discriminator, and it works only
    /// because the tokenizer hands over whole words; a substring scan would split every
    /// SHA into two and a half false artifact ids.
    #[test]
    fn a_forty_hex_git_sha_is_not_an_artifact_id() {
        let cands = parse_refs(
            "fixed in `0123456789abcdef0123456789abcdef01234567`\n",
            Path::new("docs/issues/x.md"),
            PathSyntax::NoModules,
        )
        .0;
        assert!(
            cands.iter().all(|c| c.ref_kind != RefKind::ArtifactId),
            "a 40-hex SHA classified as an artifact id: {cands:?}"
        );
    }

    #[test]
    fn near_misses_are_not_artifact_ids() {
        for raw in [
            "403e3fad0356f17",   // 15
            "403e3fad0356f1711", // 17
            "403E3FAD0356F171",  // uppercase — ids are minted lowercase
            "403e3fad0356f17g",  // non-hex
        ] {
            let text = format!("see `{raw}` here\n");
            let cands = parse_refs(&text, Path::new("docs/x.md"), PathSyntax::NoModules).0;
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::ArtifactId),
                "{raw} was classified as an artifact id"
            );
        }
    }

    /// A fenced id is still EXTRACTED — the fence is not an extraction filter, it is a
    /// severity cap (`cap_code_block`). Pinned because the two are easy to conflate, and
    /// conflating them would make someone "fix" the escape hatch by dropping the
    /// candidate, which silently removes it from the report as well as the gate.
    #[test]
    fn a_fenced_artifact_id_is_extracted_and_marked_fenced() {
        let cands = parse_refs(
            "```\n403e3fad0356f171\n```\n",
            Path::new("docs/trackers/q.md"),
            PathSyntax::NoModules,
        )
        .0;
        let hit = cands
            .iter()
            .find(|c| c.ref_kind == RefKind::ArtifactId)
            .expect("fenced artifact id should still be extracted");
        assert_eq!(hit.position, RefPosition::FencedBlock);
    }

    /// Non-vacuity: the classifier fires on the real corpus.
    ///
    /// A FLOOR, never a ratchet. `tests/doc_tool_refs.rs` already settled this for a docs
    /// population — the count grows with the corpus, so a ceiling reds on ordinary
    /// writing. What a floor catches is the failure that actually matters here: the
    /// classifier silently ceasing to fire, which looks exactly like a clean report.
    ///
    /// Measured 2026-09-14: 506 extracted artifact-id citations across `docs/**.md`.
    /// The floor sits far below that on purpose.
    #[test]
    fn the_artifact_id_classifier_is_not_vacuous_on_the_real_corpus() {
        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "md") {
                    out.push(p);
                }
            }
        }
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");
        let mut files = Vec::new();
        walk(&docs, &mut files);
        assert!(
            files.len() > 100,
            "corpus walk found only {} markdown files — the walk is broken, not the corpus",
            files.len()
        );
        // STOPS AT THE FLOOR instead of totalling the corpus, and the difference is not
        // cosmetic: parsing every file took >60s, which the harness flagged and which
        // every session would have paid on every gate run. A floor only needs enough
        // evidence to clear itself, so the healthy case reads a handful of files.
        //
        // The unhealthy case still walks everything and is slow — correct trade: a broken
        // classifier should cost time once, not a working one cost it every run.
        const FLOOR: usize = 100;
        let mut total = 0usize;
        let mut parsed = 0usize;
        for f in &files {
            let Ok(t) = std::fs::read_to_string(f) else {
                continue;
            };
            parsed += 1;
            total += parse_refs(&t, f, PathSyntax::NoModules)
                .0
                .iter()
                .filter(|c| c.ref_kind == RefKind::ArtifactId)
                .count();
            if total > FLOOR {
                break;
            }
        }
        assert!(
            total > FLOOR,
            "only {total} artifact-id citations found after parsing all {parsed} of \
             {} markdown files; measured 506 on 2026-09-14. A collapse to double digits \
             means the classifier or the walk stopped working, not that the corpus was \
             cleaned. This is a FLOOR, never a ratchet — the population grows with the \
             corpus, so a reading far above 506 is expected and is not a defect.",
            files.len()
        );
    }

    /// The collision this guard exists for, in both directions at once.
    ///
    /// Asserting only the suppression would pass equally if the rule swallowed every
    /// 16-hex token near the word "patch-id" — which is the false negative that costs a
    /// real citation. The `belongs to` case is drawn verbatim from the corpus.
    #[test]
    fn a_labelled_patch_id_is_not_a_citation_but_a_sentence_about_one_still_is() {
        let suppressed = [
            "Fixed in `9f743091`, patch-id `92db5adf65b7a748`.",
            "done 2026-09-01 — message half `0933bc95` (patch-id `42c3dcd17a52ed55`), rest",
            "shipped, patch-id **`0ba7a71b3f8462e8`** and the rest",
            "patch-id `5eb59d0d5ac2ffed` closed it",
        ];
        for text in suppressed {
            let cands = parse_refs(text, Path::new("docs/trackers/t.md"), PathSyntax::NoModules).0;
            assert!(
                cands.iter().all(|c| c.ref_kind != RefKind::ArtifactId),
                "labelled patch-id was claimed as an artifact id: {text}"
            );
        }

        // The label is the SUBJECT here, not a tag on the value — the token is a real bug.
        // Verbatim from docs/issues/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md
        let kept = "The patch-id belongs to `d5af3d3ceff1d08c`, a different bug.";
        let cands = parse_refs(kept, Path::new("docs/trackers/t.md"), PathSyntax::NoModules).0;
        assert!(
            cands
                .iter()
                .any(|c| c.ref_kind == RefKind::ArtifactId && c.raw_ref == "d5af3d3ceff1d08c"),
            "a sentence ABOUT a patch-id swallowed a genuine artifact citation"
        );
    }

    /// The gap rule is what separates the two cases above, so pin it directly rather than
    /// only through `parse_refs` — a second level asserting about its own re-implementation
    /// is indistinguishable from coverage until you break the thing that ships.
    #[test]
    fn the_patch_id_label_must_reach_the_token_through_punctuation_only() {
        let t = "a, patch-id **`aaaaaaaaaaaaaaaa`";
        assert!(labelled_as_patch_id(t, t.find('`').unwrap()));

        let u = "the patch-id belongs to `aaaaaaaaaaaaaaaa`";
        assert!(!labelled_as_patch_id(u, u.find('`').unwrap()));

        // No label at all, and a label on a PREVIOUS line must not reach across.
        // WRAPPED — label ends a line, value starts the next. A same-line scan misses
        // this, which is the archived defect this rule was written against.
        let wrapped = "**DONE** — `c2039a16`, patch-id\n   `aaaaaaaaaaaaaaaa`. 9 sidecars";
        assert!(labelled_as_patch_id(
            wrapped,
            wrapped.rfind("`aaaa").unwrap()
        ));

        let v = "plain `aaaaaaaaaaaaaaaa`";
        assert!(!labelled_as_patch_id(v, v.find('`').unwrap()));
        let w = "patch-id `bbbbbbbbbbbbbbbb`\nand then `aaaaaaaaaaaaaaaa`";
        assert!(!labelled_as_patch_id(w, w.rfind('`').unwrap() - 16));
    }
}
