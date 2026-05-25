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
            Event::Code(content) => {
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
            Event::Start(Tag::Link { dest_url, .. }) => {
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
    if s.contains('<') || s.contains('>') {
        // Template placeholders (<date>-<slug>.md, <topic>-session-log.md,
        // YYYY-MM-DD-<slug>.md) are documentation, not real paths.
        return false;
    }
    if s.contains('$') {
        // Shell expressions ($(pwd), ${VAR}, $HOME/foo).
        return false;
    }
    if s.contains('/') {
        // `/foo` with no further structure (no second segment, no extension)
        // is almost always a slash-command or shell shorthand in prose, not a
        // file path. Require either a second path segment or a known extension.
        let single_root_segment = s.starts_with('/') && !s[1..].contains('/');
        if single_root_segment {
            return has_known_ext(s);
        }
        return true;
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
