//! Source gate for ADR-2026-10-07 (`docs/adrs/2026-10-07-one-measure-of-the-delivered-response.md`):
//! every inline-or-buffered decision OUTSIDE `src/tools/core/` that is made on a length the
//! caller computed must say what unit that length is in.
//!
//! The original byte-bound defect was a length in the wrong unit: a RAW body length compared
//! against a limit that applies to the ESCAPED, serialized response. Phase A makes the text
//! predicate `exceeds_inline_limit(&str)` private to `core`, so a raw-body gate outside `core` no
//! longer compiles. The length form, `exceeds_inline_limit_len(n)`, stays callable from the few
//! tools that compute a length from parts — and a length can still be a raw one. So:
//!
//! - **Rule A.** Every production call of `exceeds_inline_limit_len(` outside `src/tools/core/`
//!   carries `// inline-gate: <unit and why it is the delivered length>`, on the call's line or
//!   on one of the [`WINDOW`] lines above it, with a NON-EMPTY reason.
//! - **Rule B.** No production reference to the text form `exceeds_inline_limit` outside
//!   `src/tools/core/` — a call, an import or a function pointer. After Phase A the compiler
//!   already refuses this; the scan is the belt for a future re-export.
//!
//! ## What "production" means here
//!
//! A file's production region is the file minus every TEST-GATED item: an item whose attributes
//! include `#[cfg(test)]` or `#[cfg(all(…, test, …))]` — an inline `mod tests { … }`, a
//! `mod tests;` declaration, a test-only helper `fn`, a test-only field. Each is excised where it
//! sits ([`excise_test_items`]). The obvious rule — "stop at the first `#[cfg(test)] mod`" —
//! assumes the test module comes last, and that convention was MEASURED FALSE on 2026-10-07: five
//! tracked files carry production code after an inline test module
//! (`src/librarian/frontmatter.rs`, `src/librarian/mod.rs`,
//! `src/librarian/tools/audit_doc_refs/parser.rs`, `src/retrieval/embedder.rs` — over a thousand
//! lines of it — and `src/tools/symbol/call_graph/mod.rs`). A cut would have skipped them
//! silently. A test-gated item that never ends (the parser lost its place) is a loud error naming
//! the file, because everything after it would otherwise go unscanned.
//!
//! Comments, doc comments, block comments and string, raw-string and char literals are blanked by
//! [`lex`] before any token is matched, because docs name these functions constantly.
//!
//! ## Known blind spots (each fails toward silence)
//!
//! - A `_len` reference that is not a call (a function pointer, `map(exceeds_inline_limit_len)`)
//!   is not gated by Rule A. Imports are the reason: they are references too, and need no
//!   annotation.
//! - Only an attribute that BEGINS a line is read. A test-gated item written after other code on
//!   the same line is scanned as production (fails loud, not silent); a test-only file is scanned
//!   as production unless its name contains `tests`.
//! - The text scan cannot tell what a length MEANS; the annotation makes a person say it, and a
//!   reviewer reads it. That is the ADR's stated limit of the lint.

use std::path::PathBuf;
use std::process::Command;

/// Files under this prefix own the measure and are exempt from both rules.
const CORE_DIR: &str = "src/tools/core/";
/// The text predicate. Its name is a prefix of [`LEN_FORM`]'s.
const TEXT_FORM: &str = "exceeds_inline_limit";
/// The length predicate, `TEXT_FORM` + this suffix.
const LEN_SUFFIX: &str = "_len";
/// The annotation a `_len` call must carry.
const ANNOTATION: &str = "inline-gate:";
/// How many lines ABOVE a call its annotation may sit (the call's own line also counts).
const WINDOW: usize = 3;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// The scanner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A `_len` call outside core with no `// inline-gate:` in the window.
    Unannotated,
    /// A `_len` call whose nearest `// inline-gate:` has nothing after the colon.
    EmptyReason,
    /// A reference to the text form outside core.
    TextForm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    file: String,
    line: usize,
    kind: Kind,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = match self.kind {
            Kind::Unannotated => {
                "`exceeds_inline_limit_len(` with no `// inline-gate: <unit>` \
                                  on its line or the 3 above"
            }
            Kind::EmptyReason => {
                "`// inline-gate:` with an EMPTY reason — name the unit of the \
                                  length and why it is the delivered one"
            }
            Kind::TextForm => {
                "the text form `exceeds_inline_limit` outside src/tools/core/ — \
                               build the candidate and use `response_fits`/`response_room`"
            }
        };
        write!(f, "{}:{}: {what}", self.file, self.line)
    }
}

#[derive(Debug, Default)]
struct FileScan {
    findings: Vec<Finding>,
    /// Production `_len` calls outside core, annotated or not — the population Rule A judged.
    len_calls: usize,
    /// Test-gated items excised before scanning (inline test modules, test-only helpers).
    test_items: usize,
}

/// The source with every comment and literal body blanked, plus each line's plain `//` comment.
struct Lexed {
    /// Same line structure as the source; comment text and literal contents are spaces. String
    /// delimiters are kept, so a blanked literal still reads as a literal.
    code: String,
    /// Per line (0-based): the text after `//` of a PLAIN line comment. `///` and `//!` doc
    /// comments are not recorded — an annotation is a plain comment.
    comments: Vec<Option<String>>,
}

impl Lexed {
    fn keep(&mut self, c: char) {
        self.code.push(c);
        if c == '\n' {
            self.comments.push(None);
        }
    }

    fn blank(&mut self, c: char) {
        if c == '\n' {
            self.code.push('\n');
            self.comments.push(None);
        } else {
            self.code.push(' ');
        }
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// True when an `r` at `i` can begin a raw string: it starts a token, or follows a `b`/`c`
/// prefix that does.
fn raw_prefix_ok(chars: &[char], i: usize) -> bool {
    match i.checked_sub(1).map(|k| chars[k]) {
        None => true,
        Some(p) if !is_ident_char(p) => true,
        Some('b' | 'c') => i < 2 || !is_ident_char(chars[i - 2]),
        Some(_) => false,
    }
}

/// Blank comments and literal bodies. A small lexer, not a line heuristic: `result_caps`'s
/// `raw_string_lines` only has to stop a raw fixture reading as a declaration; this has to stop a
/// `'"'` char literal from opening a phantom string that hides the rest of the file.
fn lex(src: &str) -> Lexed {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let at = |k: usize| chars.get(k).copied();
    let mut out = Lexed {
        code: String::with_capacity(src.len()),
        comments: vec![None],
    };
    let mut i = 0;
    while i < n {
        let c = chars[i];
        // Line comment.
        if c == '/' && at(i + 1) == Some('/') {
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let doc =
                (text.starts_with("///") && !text.starts_with("////")) || text.starts_with("//!");
            if !doc {
                *out.comments.last_mut().expect("one entry per line") = Some(text[2..].to_string());
            }
            for &comment_ch in &chars[start..i] {
                out.blank(comment_ch);
            }
            continue;
        }
        // Block comment, nested.
        if c == '/' && at(i + 1) == Some('*') {
            let mut depth = 0usize;
            while i < n {
                if chars[i] == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    out.blank('/');
                    out.blank('*');
                    i += 2;
                } else if chars[i] == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    out.blank('*');
                    out.blank('/');
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    let block_ch = chars[i];
                    out.blank(block_ch);
                    i += 1;
                }
            }
            continue;
        }
        // Raw string: r"…", r#"…"#, br"…", cr#"…"#.
        if c == 'r' && raw_prefix_ok(&chars, i) {
            let mut j = i + 1;
            while at(j) == Some('#') {
                j += 1;
            }
            if at(j) == Some('"') {
                let hashes = j - i - 1;
                for &opener_ch in &chars[i..=j] {
                    out.keep(opener_ch);
                }
                i = j + 1;
                while i < n {
                    if chars[i] == '"' && (1..=hashes).all(|h| at(i + h) == Some('#')) {
                        for &closer_ch in &chars[i..=i + hashes] {
                            out.keep(closer_ch);
                        }
                        i += 1 + hashes;
                        break;
                    }
                    let raw_ch = chars[i];
                    out.blank(raw_ch);
                    i += 1;
                }
                continue;
            }
        }
        // Ordinary (or byte) string.
        if c == '"' {
            out.keep('"');
            i += 1;
            while i < n {
                match chars[i] {
                    '\\' => {
                        out.blank('\\');
                        i += 1;
                        if i < n {
                            out.blank(chars[i]);
                            i += 1;
                        }
                    }
                    '"' => {
                        out.keep('"');
                        i += 1;
                        break;
                    }
                    string_ch => {
                        out.blank(string_ch);
                        i += 1;
                    }
                }
            }
            continue;
        }
        // Char literal (a lifetime `'a` falls through and is kept as code).
        if c == '\'' {
            if at(i + 1) == Some('\\') {
                out.keep('\'');
                out.blank('\\');
                i += 2;
                if i < n {
                    out.blank(chars[i]);
                    i += 1;
                }
                while i < n && chars[i] != '\'' {
                    out.blank(chars[i]);
                    i += 1;
                }
                if i < n {
                    out.keep('\'');
                    i += 1;
                }
                continue;
            }
            if at(i + 1).is_some() && at(i + 2) == Some('\'') {
                out.keep('\'');
                out.blank(chars[i + 1]);
                out.keep('\'');
                i += 3;
                continue;
            }
        }
        out.keep(c);
        i += 1;
    }
    out
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The attribute starting at `i` (`#[…]`): its inner text and the offset after its `]`.
/// Bracket-matched, so a multi-line attribute is read whole.
fn parse_attr(code: &str, i: usize) -> Option<(&str, usize)> {
    if !code[i..].starts_with("#[") {
        return None;
    }
    let b = code.as_bytes();
    let mut depth = 0usize;
    for (j, &byte) in b.iter().enumerate().skip(i + 1) {
        match byte {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&code[i + 2..j], j + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Every attribute from `i` on: whether any is a test-only cfg, and the offset of the item after
/// them (equal to `i` when there is none).
fn parse_attrs(code: &str, mut i: usize) -> (bool, usize) {
    let mut test_only = false;
    loop {
        let at = skip_ws(code.as_bytes(), i);
        match parse_attr(code, at) {
            Some((inner, end)) => {
                test_only |= is_test_cfg(inner);
                i = end;
            }
            None => return (test_only, at),
        }
    }
}

/// The comma-separated arguments of a cfg list, split at depth 0.
fn top_level_args(s: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0;
    for (k, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..k]);
                start = k + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// True for `cfg(test)` and `cfg(all(…, test, …))` — the item exists only in a test build.
/// `cfg(not(test))` and `cfg(any(…, test))` are production.
fn is_test_cfg(attr: &str) -> bool {
    let compact: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
    let Some(pred) = compact
        .strip_prefix("cfg(")
        .and_then(|p| p.strip_suffix(')'))
    else {
        return false;
    };
    if pred == "test" {
        return true;
    }
    let Some(args) = pred.strip_prefix("all(").and_then(|p| p.strip_suffix(')')) else {
        return false;
    };
    top_level_args(args).contains(&"test")
}

/// Blank every test-gated item in `code` (an inline `#[cfg(test)] mod … { … }`, a test-only
/// helper `fn`, a test-only field), WHEREVER it sits, and count them. Only an attribute block that
/// begins a line is considered. `Err` carries the offset of a test-gated item that never ends.
///
/// Per item, not "everything after the first test module": the convention that the test module
/// comes last does NOT hold in this tree (measured 2026-10-07: five files carry production code
/// after an inline test module, `src/retrieval/embedder.rs` over a thousand lines of it), so a
/// cut would skip real production code without a word.
fn excise_test_items(code: &str) -> Result<(String, usize), usize> {
    let mut out = code.as_bytes().to_vec();
    let mut count = 0;
    let mut line_start = 0;
    let mut excised_to = 0;
    for line in code.split_inclusive('\n') {
        let start = line_start;
        line_start += line.len();
        if start < excised_to {
            continue;
        }
        let i = start + line.len() - line.trim_start_matches([' ', '\t']).len();
        if !code[i..].starts_with("#[") {
            continue;
        }
        let (test_only, item) = parse_attrs(code, i);
        if !test_only {
            continue;
        }
        let end = item_end(code, item).ok_or(i)?;
        for byte in &mut out[i..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        excised_to = end;
        count += 1;
    }
    // Every byte of each blanked range became ASCII, so no character was split.
    let out = String::from_utf8(out).expect("blanking whole ranges keeps UTF-8 valid");
    Ok((out, count))
}

/// True when the item text starts with a keyword that begins a Rust ITEM. For those a depth-0
/// comma is part of a generic list (`fn f<A, B>()`), not the end of the item.
fn starts_with_item_keyword(item: &str) -> bool {
    let word_len = item.find(|c: char| !is_ident_char(c)).unwrap_or(item.len());
    matches!(
        &item[..word_len],
        "fn" | "mod"
            | "impl"
            | "struct"
            | "enum"
            | "union"
            | "use"
            | "const"
            | "static"
            | "type"
            | "trait"
            | "extern"
            | "unsafe"
            | "async"
            | "pub"
            | "macro_rules"
    )
}

/// The offset just past the item starting at `start`: its `;` at depth 0, the `}` that closes
/// its outermost brace, a depth-0 `,` for a non-item (a field, a variant, an arm), or — before —
/// the closer of the ENCLOSING block when the item is the last thing in it. `None` when the text
/// runs out first, which compiling Rust never does: it means this parser lost its place.
fn item_end(code: &str, start: usize) -> Option<usize> {
    let b = code.as_bytes();
    let comma_ends = !starts_with_item_keyword(&code[start..]);
    let mut depth = 0i32;
    for (i, &byte) in b.iter().enumerate().skip(start) {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    return Some(i);
                }
                if depth == 0 && byte == b'}' {
                    return Some(i + 1);
                }
            }
            b';' if depth == 0 => return Some(i + 1),
            b',' if depth == 0 && comma_ends => return Some(i + 1),
            _ => {}
        }
    }
    None
}

fn line_of(code: &str, offset: usize) -> usize {
    code[..offset].matches('\n').count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// A reference to `exceeds_inline_limit` — call, import or pointer.
    Text,
    /// A CALL of `exceeds_inline_limit_len`.
    LenCall,
}

/// Token references to the two predicates in already-blanked `code`, with their offsets.
/// Definitions (`fn NAME`) and non-call `_len` references (imports) are not returned.
fn references(code: &str) -> Vec<(usize, Form)> {
    let b = code.as_bytes();
    let mut out = vec![];
    let mut from = 0;
    while let Some(off) = code[from..].find(TEXT_FORM) {
        let at = from + off;
        from = at + TEXT_FORM.len();
        if at > 0 && is_ident_byte(b[at - 1]) {
            continue;
        }
        let mut end = at + TEXT_FORM.len();
        let len_form = code[end..].starts_with(LEN_SUFFIX);
        if len_form {
            end += LEN_SUFFIX.len();
        }
        if end < b.len() && is_ident_byte(b[end]) {
            continue;
        }
        let before = code[..at].trim_end();
        if before
            .strip_suffix("fn")
            .is_some_and(|p| !p.ends_with(is_ident_char))
        {
            continue;
        }
        if !len_form {
            out.push((at, Form::Text));
            continue;
        }
        let next = skip_ws(b, end);
        if next < b.len() && b[next] == b'(' {
            out.push((at, Form::LenCall));
        }
    }
    out
}

/// The reason of the NEAREST plain `// inline-gate:` comment on `line` or the [`WINDOW`] lines
/// above it. `Some("")` is an annotation with an empty reason.
fn gate_reason(comments: &[Option<String>], line: usize) -> Option<&str> {
    (0..=WINDOW)
        .filter_map(|d| line.checked_sub(d))
        .find_map(|l| {
            comments
                .get(l)?
                .as_deref()?
                .trim_start()
                .strip_prefix(ANNOTATION)
        })
        .map(str::trim)
}

/// Scan one file's source. `Err` when a test-gated item never ends (the parser lost its place, so
/// the rest of the file would go unscanned).
///
/// Takes `&str` so the self-tests drive THIS function on fixtures, not a copy of it.
fn scan_file(src: &str, file: &str) -> Result<FileScan, String> {
    let lexed = lex(src);
    let (production, test_items) = excise_test_items(&lexed.code).map_err(|at| {
        format!(
            "{file}:{}: a test-gated item starts here and never ends — the scanner lost its place, \
             and everything after this line would go unscanned",
            line_of(&lexed.code, at) + 1
        )
    })?;
    let mut scan = FileScan {
        test_items,
        ..FileScan::default()
    };
    if file.starts_with(CORE_DIR) {
        return Ok(scan);
    }
    for (at, form) in references(&production) {
        let line = line_of(&production, at);
        let finding = |kind| Finding {
            file: file.to_string(),
            line: line + 1,
            kind,
        };
        match form {
            Form::Text => scan.findings.push(finding(Kind::TextForm)),
            Form::LenCall => {
                scan.len_calls += 1;
                match gate_reason(&lexed.comments, line) {
                    None => scan.findings.push(finding(Kind::Unannotated)),
                    // `gate_reason` trims, so a whitespace-only reason is `""` here too.
                    Some("") => scan.findings.push(finding(Kind::EmptyReason)),
                    Some(_) => {}
                }
            }
        }
    }
    Ok(scan)
}

/// Tracked `.rs` files under `src/`, minus test-only files: a name containing `tests`, and
/// `hint_probe.rs`. `git ls-files`, not a walk, for the reason `tests/result_caps.rs`'s
/// `tracked_src_files` gives (an untracked file is a peer's in-flight work); de-duplicated for its
/// other reason (an unmerged path is listed once per index stage).
fn tracked_src_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "src"])
        .current_dir(repo_root())
        .output()
        .expect("git ls-files failed to run — this gate needs a git checkout");
    assert!(
        out.status.success(),
        "git ls-files exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|p| p.ends_with(".rs"))
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            !name.contains("tests") && name != "hint_probe.rs"
        })
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Scan every tracked production file: (findings, scanner errors, files with a test-gated item,
/// `_len` calls judged).
fn scan_tree() -> (Vec<Finding>, Vec<String>, usize, usize) {
    let mut findings = vec![];
    let mut errors = vec![];
    let mut with_tests = 0;
    let mut len_calls = 0;
    for file in tracked_src_files() {
        let src = std::fs::read_to_string(repo_root().join(&file))
            .unwrap_or_else(|e| panic!("{file} is tracked but unreadable: {e}"));
        match scan_file(&src, &file) {
            Ok(scan) => {
                findings.extend(scan.findings);
                with_tests += usize::from(scan.test_items > 0);
                len_calls += scan.len_calls;
            }
            Err(e) => errors.push(e),
        }
    }
    (findings, errors, with_tests, len_calls)
}

// ---------------------------------------------------------------------------
// The real tree
// ---------------------------------------------------------------------------

/// The scanner keeps its place through every real file: no test-gated item runs off the end.
/// Runs on the real tree today; it does not depend on the sites being annotated.
#[test]
fn the_scanner_keeps_its_place_through_every_tracked_file() {
    let (_, errors, with_tests, _) = scan_tree();
    assert!(
        errors.is_empty(),
        "{} file(s) lost the scanner's place:\n{}",
        errors.len(),
        errors.join("\n")
    );
    // Non-vacuity: an excision that fires nowhere would make every test-only call production.
    assert!(
        with_tests > 100,
        "only {with_tests} files had a test-gated item excised — the excision has gone blind"
    );
}

#[test]
#[ignore = "enabled by the integration commit, after the sites migrate"]
fn every_length_gate_outside_core_names_its_unit() {
    let (findings, layout, _, len_calls) = scan_tree();
    let mut problems: Vec<String> = findings.iter().map(ToString::to_string).collect();
    problems.extend(layout);
    assert!(
        problems.is_empty(),
        "{} inline-gate violation(s) (ADR-2026-10-07):\n{}",
        problems.len(),
        problems.join("\n")
    );
    // Non-vacuity: the ADR counts 4 legitimate `_len` sites; zero means the scanner went blind.
    assert!(
        len_calls > 0,
        "no `exceeds_inline_limit_len(` call outside core was found — the scanner is blind"
    );
}

#[test]
fn tracked_src_files_lists_production_files_and_excludes_test_only_ones() {
    let files = tracked_src_files();
    assert!(files.iter().any(|f| f == "src/tools/run_command/output.rs"));
    assert!(files
        .iter()
        .all(|f| f.starts_with("src/") && f.ends_with(".rs")));
    assert!(!files
        .iter()
        .any(|f| f.rsplit('/').next().is_some_and(|n| n.contains("tests"))));
    assert!(!files.iter().any(|f| f.ends_with("/hint_probe.rs")));
}

// ---------------------------------------------------------------------------
// Self-tests: the scanner on fixtures
// ---------------------------------------------------------------------------

const OUTSIDE: &str = "src/tools/run_command/output.rs";

fn src(lines: &[&str]) -> String {
    lines.join("\n")
}

fn scan_ok(src: &str, file: &str) -> FileScan {
    scan_file(src, file).unwrap_or_else(|e| panic!("fixture must lay out cleanly: {e}"))
}

fn kinds(src: &str, file: &str) -> Vec<(usize, Kind)> {
    scan_ok(src, file)
        .findings
        .into_iter()
        .map(|f| (f.line, f.kind))
        .collect()
}

#[test]
fn an_unannotated_len_call_outside_core_is_reported() {
    let s = src(&[
        "fn fits(n: usize) -> bool {",
        "    !crate::tools::exceeds_inline_limit_len(n)",
        "}",
    ]);
    assert_eq!(kinds(&s, OUTSIDE), vec![(2, Kind::Unannotated)]);
    assert_eq!(scan_ok(&s, OUTSIDE).len_calls, 1);
}

#[test]
fn an_annotated_len_call_is_accepted_above_or_on_the_same_line() {
    let s = src(&[
        "fn a(n: usize) -> bool {",
        "    // inline-gate: n is the serialized response length",
        "    exceeds_inline_limit_len(n)",
        "}",
        "fn b(n: usize) -> bool {",
        "    exceeds_inline_limit_len(n) // inline-gate: n is delivered_len of the response",
        "}",
    ]);
    let scan = scan_ok(&s, OUTSIDE);
    assert_eq!(scan.findings, vec![]);
    // Positive control: both calls were SEEN, so the empty list is a judgement, not blindness.
    assert_eq!(scan.len_calls, 2);
}

#[test]
fn an_empty_reason_is_rejected() {
    let s = src(&[
        "fn a(n: usize) -> bool {",
        "    // inline-gate:",
        "    exceeds_inline_limit_len(n)",
        "}",
        "fn b(n: usize) -> bool {",
        "    exceeds_inline_limit_len(n) // inline-gate:    ",
        "}",
    ]);
    assert_eq!(
        kinds(&s, OUTSIDE),
        vec![(3, Kind::EmptyReason), (6, Kind::EmptyReason)]
    );
}

#[test]
fn a_doc_comment_or_a_prose_mention_is_not_an_annotation() {
    let s = src(&[
        "fn a(n: usize) -> bool {",
        "    /// inline-gate: a doc comment is not the annotation",
        "    exceeds_inline_limit_len(n)",
        "}",
        "fn b(n: usize) -> bool {",
        "    // see the inline-gate: rule in the ADR",
        "    exceeds_inline_limit_len(n)",
        "}",
    ]);
    assert_eq!(
        kinds(&s, OUTSIDE),
        vec![(3, Kind::Unannotated), (7, Kind::Unannotated)]
    );
}

#[test]
fn the_annotation_may_sit_three_lines_above_but_not_four() {
    let s = src(&[
        "fn three(n: usize) -> bool {",
        "    // inline-gate: three lines above the call",
        "    let a = 1;",
        "    let b = 2;",
        "    exceeds_inline_limit_len(n + a + b)",
        "}",
        "fn four(n: usize) -> bool {",
        "    // inline-gate: four lines above the call",
        "    let a = 1;",
        "    let b = 2;",
        "    let c = 3;",
        "    exceeds_inline_limit_len(n + a + b + c)",
        "}",
    ]);
    assert_eq!(kinds(&s, OUTSIDE), vec![(12, Kind::Unannotated)]);
}

#[test]
fn calls_inside_comments_are_ignored() {
    let s = src(&[
        "//! exceeds_inline_limit_len(n) in a module doc",
        "/// exceeds_inline_limit(text) in a doc comment",
        "// exceeds_inline_limit_len(n) in a line comment",
        "/* exceeds_inline_limit_len(n) in a block comment",
        "   exceeds_inline_limit(t) /* nested */ exceeds_inline_limit_len(n) */",
        "fn after(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    // Positive control on the last line: the lexer resumed after the nested block comment.
    assert_eq!(kinds(&s, OUTSIDE), vec![(6, Kind::Unannotated)]);
}

#[test]
fn calls_inside_string_literals_are_ignored() {
    let s = src(&[
        "fn f() {",
        r#"    let s = "exceeds_inline_limit_len(n) \" exceeds_inline_limit(t)";"#,
        "}",
        "fn after(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    assert_eq!(kinds(&s, OUTSIDE), vec![(4, Kind::Unannotated)]);
}

#[test]
fn calls_inside_raw_strings_are_ignored_and_a_quote_char_opens_nothing() {
    let s = src(&[
        "fn f() {",
        r##"    let r = r#"exceeds_inline_limit_len(n) "quoted" exceeds_inline_limit(t)"#;"##,
        r#"    let m = r""#,
        "exceeds_inline_limit_len(n)",
        r#"";"#,
        r#"    let q = '"';"#,
        "}",
        "fn after(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    // The `'"'` char literal must not open a string that swallows `after`.
    assert_eq!(kinds(&s, OUTSIDE), vec![(8, Kind::Unannotated)]);
}

#[test]
fn a_call_inside_the_inline_test_module_is_ignored() {
    for cfg in ["#[cfg(test)]", "#[cfg(all(test, unix))]"] {
        let s = src(&[
            "fn prod(n: usize) -> bool {",
            "    // inline-gate: n is the delivered length",
            "    exceeds_inline_limit_len(n)",
            "}",
            "",
            cfg,
            "mod tests {",
            "    #[test]",
            "    fn t() { assert!(exceeds_inline_limit_len(1) || exceeds_inline_limit(\"x\")); }",
            "}",
        ]);
        let scan = scan_ok(&s, OUTSIDE);
        assert_eq!(scan.findings, vec![], "under {cfg}");
        assert_eq!(scan.test_items, 1, "under {cfg}");
        assert_eq!(scan.len_calls, 1, "under {cfg}: only the production call");
    }
}

#[test]
fn a_test_module_declaration_or_a_non_test_cfg_does_not_cut() {
    let s = src(&[
        "#[cfg(test)]",
        "mod tests;",
        "#[cfg(not(test))]",
        "mod live {",
        "    fn f(n: usize) -> bool { exceeds_inline_limit_len(n) }",
        "}",
        "#[cfg(any(feature = \"server-stack\", test))]",
        "mod shared {",
        "    fn g(n: usize) -> bool { exceeds_inline_limit_len(n) }",
        "}",
    ]);
    let scan = scan_ok(&s, OUTSIDE);
    // The `mod tests;` declaration is excised and ends at its `;`, not at the end of the file.
    assert_eq!(scan.test_items, 1);
    assert_eq!(
        scan.findings
            .iter()
            .map(|f| (f.line, f.kind))
            .collect::<Vec<_>>(),
        vec![(5, Kind::Unannotated), (9, Kind::Unannotated)]
    );
}

#[test]
fn a_file_under_core_is_exempt_and_the_prefix_needs_its_slash() {
    let s = src(&[
        "fn f(n: usize, t: &str) -> bool {",
        "    exceeds_inline_limit_len(n) || exceeds_inline_limit(t)",
        "}",
    ]);
    assert_eq!(kinds(&s, "src/tools/core/types.rs"), vec![]);
    assert_eq!(kinds(&s, "src/tools/core/response_fit.rs"), vec![]);
    assert_eq!(
        kinds(&s, "src/tools/core_extra/x.rs"),
        vec![(2, Kind::Unannotated), (2, Kind::TextForm)]
    );
}

#[test]
fn the_text_form_outside_core_is_reported_even_when_annotated() {
    let s = src(&[
        "use crate::tools::exceeds_inline_limit_len;",
        "fn f(t: &str) -> bool {",
        "    // inline-gate: an annotation does not excuse the text form",
        "    crate::tools::exceeds_inline_limit(t)",
        "}",
        "fn g(v: &[&str]) -> bool {",
        "    v.iter().copied().any(crate::tools::exceeds_inline_limit)",
        "}",
        "fn h(t: &str) -> bool {",
        "    my_exceeds_inline_limit(t) || exceeds_inline_limit_lenient(t)",
        "}",
        "fn exceeds_inline_limit(t: &str) -> bool { t.is_empty() }",
    ]);
    assert_eq!(
        kinds(&s, OUTSIDE),
        vec![(4, Kind::TextForm), (7, Kind::TextForm)]
    );
}

/// The shape of five real files (`src/retrieval/embedder.rs` among them): production code AFTER
/// an inline test module. It must be scanned, not skipped.
#[test]
fn production_code_after_a_test_module_is_still_scanned() {
    let s = src(&[
        "fn prod() {}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    fn t() { let _ = '}'; }",
        "}",
        "",
        "fn more_prod(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    let scan = scan_ok(&s, OUTSIDE);
    assert_eq!(scan.test_items, 1);
    assert_eq!(
        scan.findings
            .iter()
            .map(|f| (f.line, f.kind))
            .collect::<Vec<_>>(),
        vec![(8, Kind::Unannotated)]
    );
}

#[test]
fn a_test_gated_item_that_never_ends_fails_loudly_naming_the_file() {
    let s = src(&[
        "fn prod() {}",
        "#[cfg(test)]",
        "mod tests {",
        "    fn t() {}",
        "",
        "fn later(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    let err = scan_file(&s, "src/tools/x.rs").expect_err("a runaway excision must not be silent");
    assert!(err.starts_with("src/tools/x.rs:2:"), "got: {err}");
}

#[test]
fn test_gated_items_anywhere_are_excised_and_stop_where_they_end() {
    let s = src(&[
        "#[cfg(test)]",
        "#[allow(dead_code)]",
        "fn helper<A, B>(t: &str) -> bool { exceeds_inline_limit(t) }",
        "struct S {",
        "    #[cfg(test)]",
        "    probe: Vec<u8>,",
        "    live: usize,",
        "}",
        "enum E {",
        "    Live,",
        "    #[cfg(all(test, feature = \"librarian\"))]",
        "    Probe",
        "}",
        "fn prod(n: usize) -> bool { exceeds_inline_limit_len(n) }",
    ]);
    let scan = scan_ok(&s, OUTSIDE);
    assert_eq!(scan.test_items, 3);
    // The helper's text-form call is test-only; the production call after the struct and the
    // enum is still seen, so each excision stopped at its own item's end.
    assert_eq!(
        scan.findings
            .iter()
            .map(|f| (f.line, f.kind))
            .collect::<Vec<_>>(),
        vec![(14, Kind::Unannotated)]
    );
}
