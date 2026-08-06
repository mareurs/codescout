// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate, RefKind, RefPosition};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub fn parse_refs(text: &str, md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    // Forward-slash normalize so md_file keys are consistent across platforms.
    let md_file = crate::util::fs::RepoPath::from(md_path).into_string();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut candidates = Vec::new();
    let warnings = fence_warnings(text, &md_file);

    let mut in_code_block = false;
    let parser = Parser::new_ext(text, opts).into_offset_iter();
    for (event, span) in parser {
        let line = byte_offset_to_line(text, span.start);
        match event {
            // A span that renders a code span literally is showing what a
            // reference looks like, not making one. See `is_markup_display`.
            Event::Code(content) if !is_markup_display(content.as_ref()) => {
                for raw in tokenize_code_span(content.as_ref()) {
                    if let Some(kind) = classify(raw, true) {
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
            Event::Text(content) if in_code_block => {
                for raw in tokenize_code_span(content.as_ref()) {
                    if let Some(kind) = classify(raw, true) {
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
            Event::Start(Tag::Link { dest_url, .. }) if !is_placeholder(dest_url.as_ref()) => {
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
fn classify(s: &str, in_code_context: bool) -> Option<RefKind> {
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
    if in_code_context && is_module_path(s) {
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

/// Trim trailing sentence punctuation (period, brackets, braces) that often
/// sticks to a path-like token in prose: `See foo.md.` → `foo.md`.
/// Does NOT trim `:` (significant for FileLine refs like `file.rs:42`) or `/`.
fn trim_token_edges(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '[' | ']' | '{' | '}'))
        .trim_end_matches('.')
}

fn is_module_path(s: &str) -> bool {
    s.contains('.')
        && !s.contains('/')
        && !s.contains(char::is_whitespace)
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
        && s.split('.').all(|part| !part.is_empty())
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
            let single_root_segment = s.starts_with('/') && !s[1..].contains('/');
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
        parse_refs(text, &PathBuf::from("test.md"))
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
