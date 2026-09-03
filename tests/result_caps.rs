//! Instruments for `IC-13` (`cluster/capped-result-presented-as-complete`).
//!
//! TWO instruments run here, and their scopes differ ON PURPOSE. `CLAUDE.md`
//! § *Observer Blindness*: two agreeing instruments are evidence only when
//! their scopes differ, because two same-scope instruments agreeing is one
//! blind spot counted twice and is indistinguishable from corroboration at
//! the point of use. Instrument A reads DECLARATIONS, instrument B reads
//! CALL SITES.
//!
//! Instrument A alone would ship `IC-18` (`selector-narrower-than-its-
//! population`) inside the gate for `IC-13`: its name regex cannot see a cap
//! called `PAGE_SIZE`, and cannot see a bare `.next()` at all. The
//! `indexer.rs` first-chunk-only member (fixed at `488192e8`) was exactly
//! that shape — no constant anywhere — which is why B exists.
//!
//! Scans `src/` and never `tests/`: this file contains `cap-class:` strings
//! as FIXTURES, and a scanner that read them would count a teaching example
//! as a declaration.
//!
//! ## Census — 2026-09-02
//!
//! **103 cap-shaped `const` declarations** in tracked `src/` (unit: `const`
//! declarations matching [`is_cap_shaped`], one count per declaration, not
//! per use site — a constant read at six call sites still counts once):
//! **66** `RESULT_CAP` across 66 distinct ids, **37** `NOT_A_CAP`. Derived by
//! running [`every_cap_constant_is_classified`]'s own parser over
//! `git ls-files src`, not by a shell grep — a second selector answers a
//! slightly different question, which is the `IC-18` mistake this gate
//! exists to catch.
//!
//! The census is a floor on the cap population, not a census of caps.
//! `LATEST_OBSERVATIONS: usize = 3` (`src/librarian/preview/memory.rs:9`) is
//! a live result cap — `memory.rs:18` truncates the observation list to it,
//! so a preview shows three of however many exist — and [`is_cap_shaped`]
//! cannot see it, because its name carries no `MAX`/`CAP`/`LIMIT`/`BUDGET`
//! token. It sits ONE LINE above `OBSERVATION_TEXT_MAX`, which this census
//! does count. A miss that close to a hit is the plainest statement of why
//! instrument B ([`truncation_sites`]) exists: it reads the truncating
//! OPERATION, which no name regex can be widened into.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapDecl {
    name: String,
    file: String,
    line: usize,
    /// Text after `cap-class:` on the last such line in the contiguous
    /// comment block directly above the declaration, trimmed.
    annotation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CapClass {
    ResultCap(String),
    NotACap(String),
    Unclassified,
    /// An annotation is present but unusable: either a `NOT_A_CAP` with an
    /// empty reason, or a `RESULT_CAP` whose id does not match the
    /// grammar's `[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*` production (e.g.
    /// `RESULT_CAP probed`, a bare word with no dot). Distinguished from
    /// `Unclassified` because "the annotation exists" is not the property
    /// we want, and the two need different failure text.
    MalformedReason,
}

#[test]
fn cap_constants_finds_a_cap_shaped_const_and_its_annotation() {
    let src = "\
// cap-class: RESULT_CAP grep.lines — probed
const GREP_LINE_LIMIT: usize = 50;
";
    let got = cap_constants(src, "src/x.rs");
    assert_eq!(got.len(), 1, "one cap-shaped const");
    assert_eq!(got[0].name, "GREP_LINE_LIMIT");
    assert_eq!(got[0].line, 2);
    assert_eq!(
        got[0].annotation.as_deref(),
        Some("RESULT_CAP grep.lines — probed")
    );
}

#[test]
fn cap_constants_ignores_a_const_whose_name_is_not_cap_shaped() {
    let src = "const EMBED_CONCURRENCY: usize = 8;\n";
    assert!(
        cap_constants(src, "src/x.rs").is_empty(),
        "EMBED_CONCURRENCY is not cap-shaped; instrument B is what covers \
         a bound like this, and that gap is the reason B exists"
    );
}

#[test]
fn cap_constants_accepts_pub_and_visibility_qualified_forms() {
    let src = "\
pub const A_MAX: usize = 1;
pub(crate) const B_LIMIT: usize = 2;
    const C_CAP: usize = 3;
";
    let names: Vec<String> = cap_constants(src, "src/x.rs")
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(names, vec!["A_MAX", "B_LIMIT", "C_CAP"]);
}

#[test]
fn cap_constants_does_not_read_an_annotation_through_a_blank_line() {
    let src = "\
// cap-class: RESULT_CAP stale.ref — belongs to something else

const OTHER_MAX: usize = 1;
";
    assert_eq!(
        cap_constants(src, "src/x.rs")[0].annotation,
        None,
        "a blank line ends the comment block; reading through it would let \
         one annotation silently cover an unrelated later constant"
    );
}

#[test]
fn cap_constants_skips_a_cap_class_line_inside_a_doc_fence() {
    let src = "\
/// Example for readers:
/// ```
/// // cap-class: RESULT_CAP example.only
/// ```
const REAL_MAX: usize = 1;
";
    assert_eq!(
        cap_constants(src, "src/x.rs")[0].annotation,
        None,
        "a fenced example must not classify its neighbour — the \
         documentation-example-as-real-token trap (CLAUDE.md § Parsers Over \
         a Namespace)"
    );
}

#[test]
fn cap_constants_reads_an_annotation_through_an_attribute() {
    let src = "\
// cap-class: RESULT_CAP grep.lines — probed
#[allow(dead_code)]
const GREP_LINE_LIMIT: usize = 50;
";
    let got = cap_constants(src, "src/x.rs");
    assert_eq!(got.len(), 1, "one cap-shaped const");
    assert_eq!(
        got[0].annotation.as_deref(),
        Some("RESULT_CAP grep.lines — probed"),
        "the #[allow(dead_code)] attribute sits between the doc comment and the \
         const; annotation_above must skip over it rather than stopping the \
         upward walk there. Deleting the `#[` skip branch in annotation_above \
         makes this fail: the walk hits the attribute line first, it does not \
         start with \"//\", so `break` fires before the comment above it is \
         ever collected and this assertion goes from Some(..) to None."
    );
}
#[test]
fn cap_constants_does_not_report_a_const_inside_a_raw_string_fixture() {
    // A parser test's INPUT DATA, not code. `src/ast/parser.rs` holds two of
    // these; before `raw_string_lines` the gate demanded `cap-class:`
    // annotations for them, which is `IC-6`'s no-escape half inside the gate
    // for `IC-13`.
    let src = "\
fn rust_symbols_are_extracted() {
    let source = r#\"
const MAX: u32 = 100;
\"#;
}
";
    assert!(
        cap_constants(src, "src/x.rs").is_empty(),
        "a const inside r#\"...\"# is fixture text, not a declaration"
    );
}

#[test]
fn cap_constants_still_reports_a_const_after_a_closed_raw_string() {
    // The direction the absence test above cannot see. Over-skipping — a
    // region that never closes, or a closer the scan misses — hides real
    // declarations from the gate, and the absence assertion is MONOTONE under
    // exactly that failure: it gets *more* satisfied as the skip widens. This
    // asserts the whole reported set, so a leak of INSIDE_MAX and a swallow of
    // OUTSIDE_MAX each fail it.
    let src = "\
fn f() {
    let source = r#\"
const INSIDE_MAX: usize = 1;
\"#;
}

const OUTSIDE_MAX: usize = 2;
";
    let names: Vec<String> = cap_constants(src, "src/x.rs")
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(names, vec!["OUTSIDE_MAX"]);
}

#[test]
fn unclosed_raw_opener_pairs_each_opener_with_its_own_closer() {
    // One line that opens and closes twice opens nothing: without per-opener
    // pairing, the second `r#"` would leave the region open and swallow the
    // rest of the file.
    assert_eq!(unclosed_raw_opener("    f(r#\"a\"#, r#\"b\"#);"), None);
    // Hash count is part of the delimiter: `"#` does not close `r##"`.
    assert_eq!(
        unclosed_raw_opener("    let s = r##\"x\"#;").as_deref(),
        Some("\"##")
    );
    // `r` must start a token — a word ending in `r` before a normal string is
    // not an opener.
    assert_eq!(unclosed_raw_opener("    let separator = \"x\";"), None);
}

#[test]
fn classify_reads_the_three_states_and_rejects_an_empty_reason() {
    let mk = |ann: Option<&str>| CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: ann.map(str::to_owned),
    };
    assert_eq!(
        classify(&mk(Some("RESULT_CAP grep.lines — probed"))),
        CapClass::ResultCap("grep.lines".into())
    );
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP — LSP handshake deadline"))),
        CapClass::NotACap("LSP handshake deadline".into())
    );
    assert_eq!(classify(&mk(None)), CapClass::Unclassified);
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP —   "))),
        CapClass::MalformedReason,
        "a bare NOT_A_CAP token is RED: an annotation that need not say why \
         is satisfied by writing the token, which is not the property wanted"
    );
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP"))),
        CapClass::MalformedReason,
        "no separator, no reason"
    );
}

#[test]
fn classify_accepts_all_three_dash_forms() {
    for sep in ["—", "–", "-"] {
        let decl = CapDecl {
            name: "X_MAX".into(),
            file: "src/x.rs".into(),
            line: 1,
            annotation: Some(format!("NOT_A_CAP {sep} a stated reason")),
        };
        assert_eq!(
            classify(&decl),
            CapClass::NotACap("a stated reason".into()),
            "separator {sep:?} must parse; house style is not uniform and a \
             gate that accepted only one would refuse correct annotations"
        );
    }
}

/// `RESULT_CAPACITY_THING` shares the literal prefix `RESULT_CAP` but is a
/// different token. `payload.strip_prefix("RESULT_CAP")` alone cannot tell
/// the two apart and would yield the garbage id `ACITY_THING` — the same
/// prefix-collision shape `CLAUDE.md` tracks as
/// `cluster/addressing-without-an-escape-hatch`. Requiring the character
/// after the token to be whitespace or end-of-string closes it at the
/// parser.
#[test]
fn classify_does_not_match_a_longer_token_sharing_the_prefix() {
    let decl = CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: Some("RESULT_CAPACITY_THING".into()),
    };
    assert_eq!(
        classify(&decl),
        CapClass::Unclassified,
        "RESULT_CAPACITY_THING must not be read as RESULT_CAP with id \
         ACITY_THING; without a word boundary after the token this falls \
         through to a garbage classification instead of Unclassified"
    );
}

#[test]
fn classify_rejects_a_hyphen_inside_a_word_with_no_real_separator() {
    let decl = CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: Some("NOT_A_CAP no-real-separator-here".into()),
    };
    assert_eq!(
        classify(&decl),
        CapClass::MalformedReason,
        "the hyphens here are inside a word, not a whitespace-delimited \
         separator immediately after NOT_A_CAP; a search unanchored to \
         position would find the first '-' anywhere in the payload and \
         wrongly split on it"
    );
}

#[test]
fn classify_takes_the_reason_after_the_first_separator_even_when_a_later_em_dash_exists() {
    let decl = CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: Some("NOT_A_CAP - reason with an em dash — inside it".into()),
    };
    assert_eq!(
        classify(&decl),
        CapClass::NotACap("reason with an em dash — inside it".into()),
        "the first separator positionally (a plain hyphen, right after \
         NOT_A_CAP) must win; a fixed-preference search that always tried \
         the em dash first would instead split on the later '—' and \
         truncate the reason to 'inside it'"
    );
}

#[test]
fn classify_rejects_a_result_cap_id_without_a_dot() {
    let decl = CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: Some("RESULT_CAP probed".into()),
    };
    assert_eq!(
        classify(&decl),
        CapClass::MalformedReason,
        "\"probed\" has no dot and does not match the id grammar \
         [a-z][a-z0-9_]*.[a-z][a-z0-9_]*; classify must not silently accept \
         it as ResultCap(\"probed\")"
    );
}

#[test]
fn tracked_src_files_returns_rust_files_under_src_and_excludes_tests() {
    let files = tracked_src_files();
    assert!(
        files.iter().any(|f| f == "src/tools/grep.rs"),
        "a known src file must be present; got {} files",
        files.len()
    );
    assert!(
        files
            .iter()
            .all(|f| f.starts_with("src/") && f.ends_with(".rs")),
        "only tracked .rs under src/"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("tests/")),
        "tests/ carries cap-class fixtures and must never be scanned"
    );
}

/// Tracked `.rs` files under `src/`.
///
/// `git ls-files`, not a walk: an untracked file is a peer's in-flight work
/// and gating on it lets one session red another's build. Same reasoning and
/// the same measured incident as `tracked_all_bug_files` in
/// `tests/issue_clusters.rs`.
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
        .map(str::to_owned)
        .collect()
}

/// True when a constant's name is cap-shaped.
///
/// A FLOOR, never a census. `PAGE_SIZE` and `DEFAULT_DEPTH` are caps this
/// predicate cannot see; instrument B is what covers them, and the two
/// instruments' disagreement is the signal.
fn is_cap_shaped(name: &str) -> bool {
    ["CAP", "LIMIT", "MAX", "BUDGET", "THRESHOLD"]
        .iter()
        .any(|t| name.contains(t))
}

/// Lines of `src` that sit INSIDE a raw string literal and must not be read
/// as declarations.
///
/// A LINE-LEVEL HEURISTIC, not a lexer. It exists because [`cap_constants`]
/// reads source as TEXT: without it a `const MAX: u32 = 100;` written inside
/// an `r#"..."#` test fixture — input DATA for a parser under test, never code
/// — is reported as an unclassified cap, and the gate demands an annotation
/// the fixture should not carry. That is `IC-6`'s *no escape* half occurring
/// inside the gate built for `IC-13`: a scanner over a namespace with no way
/// to say "this token is data". Two such fixtures live in
/// `src/ast/parser.rs`; they were annotated before this escape existed, and
/// that was harmless only because both are Rust (tree-sitter read the
/// inserted `//` as a comment) and both assert on symbol names rather than
/// line numbers. A Python or Go fixture, or one line-number assertion, would
/// have broken.
///
/// Rule: a line holding a raw-string opener (`r"`, `r#"`, `r##"`, …) with no
/// matching closer later on that same line opens a skipped region, which ends
/// on the first line holding the matching closer. The opener line is still
/// parsed; the closer line is skipped whole.
///
/// What it deliberately does NOT handle — each would need a real lexer:
/// - a raw-string delimiter appearing inside an ordinary `"..."` literal, or
///   inside a TRAILING comment, is read as a real delimiter. Only a LEADING
///   `//` exempts a line, so prose that mentions a raw-string opener cannot
///   open a phantom region;
/// - a `const` sharing a line with the closing delimiter is skipped with it;
/// - a raw string whose closer never appears (an unterminated literal, which
///   would not compile) skips the rest of the file.
///
/// Fails toward SILENCE rather than toward a wrong classification:
/// over-skipping hides a real declaration from the gate. So
/// `cap_constants_still_reports_a_const_after_a_closed_raw_string` is the
/// load-bearing test — an absence assertion alone is monotone under exactly
/// the over-skip this can cause, and would not fire.
fn raw_string_lines(lines: &[&str]) -> Vec<bool> {
    let mut out = vec![false; lines.len()];
    let mut open_closer: Option<String> = None;
    for (idx, raw) in lines.iter().enumerate() {
        if let Some(closer) = open_closer.clone() {
            out[idx] = true;
            if raw.contains(closer.as_str()) {
                open_closer = None;
            }
            continue;
        }
        if raw.trim_start().starts_with("//") {
            continue;
        }
        open_closer = unclosed_raw_opener(raw);
    }
    out
}

/// The closing delimiter of a raw string opened on `line` and not closed on
/// it, or `None` when the line opens no unclosed raw string.
///
/// Walks the line left to right, pairing each opener with its own closer, so a
/// line that opens AND closes several raw strings is correctly reported as
/// opening none. The `r` must start a token — the preceding byte may not be
/// alphanumeric or `_` — so an identifier ending in `r` is not an opener.
fn unclosed_raw_opener(line: &str) -> Option<String> {
    let b = line.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'r' || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && b[j] == b'#' {
            j += 1;
        }
        if j >= b.len() || b[j] != b'"' {
            i += 1;
            continue;
        }
        let closer = format!("\"{}", "#".repeat(j - i - 1));
        match line[j + 1..].find(&closer) {
            Some(off) => i = j + 1 + off + closer.len(),
            None => return Some(closer),
        }
    }
    None
}

/// Cap-shaped `const` declarations in `src`, each with the `cap-class:`
/// annotation from the contiguous comment block directly above it.
///
/// Lines inside a raw string literal are skipped — see [`raw_string_lines`].
///
/// Takes `&str` rather than reading the file so the meta-tests above drive
/// THIS function on fixtures — not a second copy that could drift from it
/// (`missing_index_rows` precedent, `tests/issue_clusters.rs:461-471`).
fn cap_constants(src: &str, file: &str) -> Vec<CapDecl> {
    let lines: Vec<&str> = src.lines().collect();
    let in_raw_string = raw_string_lines(&lines);
    let mut out = vec![];

    for (idx, raw) in lines.iter().enumerate() {
        if in_raw_string[idx] {
            continue;
        }
        let t = raw.trim_start();
        let after_vis = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub(super) "))
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t);
        let Some(rest) = after_vis.strip_prefix("const ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            || !is_cap_shaped(name)
        {
            continue;
        }

        out.push(CapDecl {
            name: name.to_string(),
            file: file.to_string(),
            line: idx + 1,
            annotation: annotation_above(&lines, idx),
        });
    }
    out
}

/// The last `cap-class:` payload in the contiguous comment block immediately
/// above `decl_idx`, skipping lines inside a fenced block within that
/// comment.
///
/// A blank line ends the block: reading through one would let a stray
/// annotation silently classify an unrelated later constant, which is a
/// wrong classification rather than a missing one.
fn annotation_above(lines: &[&str], decl_idx: usize) -> Option<String> {
    let mut block: Vec<&str> = vec![];
    for i in (0..decl_idx).rev() {
        let t = lines[i].trim_start();
        if t.starts_with("#[") {
            // Attributes sit between the doc block and the item. This only
            // recognizes single-line attributes: a multi-line attribute's
            // closing line (e.g. the `))]` of a wrapped `#[cfg(...)]`) does
            // not start with "#[", so it falls through to the `break` below
            // and ends the walk early, silently losing the annotation above
            // it. Fails safe — the decl becomes Unclassified, not a wrong
            // classification — but a future reader shouldn't have to
            // rediscover this by tracing the loop.
            continue;
        }
        if t.starts_with("//") {
            block.push(t);
            continue;
        }
        break;
    }
    block.reverse();

    let mut in_fence = false;
    let mut found = None;
    for t in block {
        let body = t
            .trim_start_matches('/')
            .trim_start_matches('!')
            .trim_start();
        if body.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((_, payload)) = body.split_once("cap-class:") {
            found = Some(payload.trim().to_string());
        }
    }
    found
}

/// Split a `NOT_A_CAP` annotation payload into the text after its
/// separator.
///
/// The separator must sit immediately after the `NOT_A_CAP` token
/// (whitespace-delimited: `" — "`, `" – "`, or `" - "`) — not merely occur
/// somewhere in the payload. Two defects that shape guards against:
/// unanchored `split_once` would treat a hyphen buried inside a word
/// (`"NOT_A_CAP LSP-handshake deadline"`) as the separator and truncate the
/// reason mid-word; and searching in a fixed dash-preference order across
/// the WHOLE payload would match a later em-dash inside the reason itself
/// before the real, earlier separator, silently discarding everything
/// before it. Checking each separator only as a prefix of the text
/// immediately following the token — at one fixed position — takes
/// whichever one is actually there positionally, since at most one can
/// match at that position.
fn split_reason(payload: &str) -> Option<&str> {
    let rest = payload.strip_prefix("NOT_A_CAP")?;
    let trimmed = rest.trim_start();
    for sep in ["—", "–", "-"] {
        if let Some(after_sep) = trimmed.strip_prefix(sep) {
            if after_sep.is_empty() || after_sep.starts_with(char::is_whitespace) {
                return Some(after_sep.trim());
            }
        }
    }
    None
}

/// True when `id` matches the annotation grammar's `<id>` production:
/// `[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*` — dotted lowercase, e.g. `grep.lines`.
/// A bare word with no dot (`probed`) does not match.
fn is_valid_cap_id(id: &str) -> bool {
    let Some((head, tail)) = id.split_once('.') else {
        return false;
    };
    let valid_segment = |s: &str| {
        let mut chars = s.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    valid_segment(head) && valid_segment(tail)
}

fn classify(decl: &CapDecl) -> CapClass {
    let Some(payload) = decl.annotation.as_deref() else {
        return CapClass::Unclassified;
    };
    let payload = payload.trim();

    if let Some(rest) = payload.strip_prefix("RESULT_CAP") {
        // A word boundary is required after the token: `strip_prefix` alone
        // also matches a payload beginning `RESULT_CAPACITY…`, and reading
        // whatever follows as the id would yield a garbage classification
        // rather than a correct refusal. See
        // `classify_does_not_match_a_longer_token_sharing_the_prefix`.
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            let id = rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_');
            if id.is_empty() || !is_valid_cap_id(id) {
                return CapClass::MalformedReason;
            }
            return CapClass::ResultCap(id.to_string());
        }
    } else if payload.starts_with("NOT_A_CAP") {
        return match split_reason(payload) {
            Some(r) if !r.is_empty() => CapClass::NotACap(r.to_string()),
            _ => CapClass::MalformedReason,
        };
    }

    CapClass::Unclassified
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TruncSite {
    op: String,
    file: String,
    line: usize,
    annotation: Option<String>,
}

#[test]
fn truncation_sites_finds_the_operations_instrument_a_cannot_see() {
    let src = "\
let head = items.take(10);
s.truncate(80);
let out = truncate_compact(&s, 80);
let first = chunk_markdown(body).next();
";
    let ops: Vec<String> = truncation_sites(src, "src/x.rs")
        .into_iter()
        .map(|s| s.op)
        .collect();
    assert_eq!(
        ops,
        vec![
            ".take(".to_string(),
            ".truncate(".to_string(),
            "truncate_compact(".to_string(),
            ".next()".to_string(),
        ],
        "every OPS entry must be reached by this fixture, and the assertion \
         must be an exact assert_eq! on the whole vector rather than a \
         per-entry `contains` — `contains` is monotone under widening (an \
         extra unrelated hit still passes), so deleting any ONE entry from \
         OPS must red THIS test, not pass silently"
    );
}

#[test]
fn truncation_sites_ignores_stream_next_which_is_iteration_not_capping() {
    let src = "while let Some(res) = stream.next().await {\n";
    assert!(
        truncation_sites(src, "src/x.rs").is_empty(),
        "`stream.next().await` drains an async stream — it caps nothing. \
         src/librarian/indexer.rs:918,1051 are exactly this and must stay \
         silent, or the gate cries wolf where the real member (a bare \
         `.next()` on a chunk iterator) was one line away"
    );
}

#[test]
fn truncation_sites_does_not_exclude_a_sync_next_on_a_receiver_ending_in_s() {
    let src = "let first = items.next();\n";
    let ops: Vec<String> = truncation_sites(src, "src/x.rs")
        .into_iter()
        .map(|s| s.op)
        .collect();
    assert_eq!(
        ops,
        vec![".next()".to_string()],
        "`items.next()` has no `.await` — it is a synchronous iterator call, \
         not a stream drain. Excluding it because the receiver's name ends \
         in `s` (as `.contains(\"s.next()\")` would) silently drops \
         `items.next()`, `lines.next()`, `rows.next()`, `chars.next()` — the \
         exact permissive-direction under-reporting this gate exists to catch"
    );
}

#[test]
fn truncation_sites_reads_an_annotation_the_same_way_declarations_do() {
    let src = "\
// cap-class: NOT_A_CAP — bounded by the caller's explicit line range
let head = items.take(n);
";
    assert_eq!(
        truncation_sites(src, "src/x.rs")[0].annotation.as_deref(),
        Some("NOT_A_CAP — bounded by the caller's explicit line range"),
        "one annotation grammar for both instruments — a second grammar is \
         a second thing to get wrong"
    );
}

#[test]
fn truncation_sites_does_not_report_a_fully_commented_out_call() {
    // Guards the `code.starts_with("//")` skip against silent removal: a
    // fully commented-out call is not code, and this is the only fixture
    // in the file where the commented line itself holds an OPS substring —
    // delete the skip and this line starts getting reported.
    let src = "// let head = items.take(10);\n";
    assert!(
        truncation_sites(src, "src/x.rs").is_empty(),
        "a line-leading `//` comment is not a live call site, commented-out \
         or not"
    );
}

#[test]
fn truncation_sites_reports_an_op_inside_a_trailing_comment_known_limitation() {
    // KNOWN LIMITATION, not a decision: the skip only recognizes a comment
    // that starts the line. A trailing `//` comment on a real code line is
    // not distinguished from code, so an op token mentioned there is
    // reported exactly as if it were a live call site. This pins the
    // CURRENT behavior (over-reporting — the safe direction for a gate
    // whose failure mode is a caller missing a marker) rather than
    // silently changing or worsening it; narrowing the skip to handle this
    // is out of scope for this pass.
    let src = "let n = x; // .take(5)\n";
    let ops: Vec<String> = truncation_sites(src, "src/x.rs")
        .into_iter()
        .map(|s| s.op)
        .collect();
    assert_eq!(
        ops,
        vec![".take(".to_string()],
        "current (unproven-safe) behavior: a trailing comment's op text is \
         reported because the skip is line-leading only"
    );
}

#[test]
fn truncation_sites_reports_the_correct_file_and_one_indexed_line() {
    // The op sits on line 3, not line 1 — a fixture with the op on line 1
    // cannot distinguish a 0-indexed `idx` from the correct 1-indexed
    // `idx + 1`, since both would read back as 1.
    let src = "\
let filler_one = 1;
let filler_two = 2;
let head = items.take(10);
";
    let sites = truncation_sites(src, "src/y.rs");
    assert_eq!(sites.len(), 1, "got {sites:?}");
    assert_eq!(sites[0].file, "src/y.rs");
    assert_eq!(
        sites[0].line, 3,
        "1-indexed line number, not the 0-indexed idx"
    );
}

#[test]
fn unclassified_decls_names_every_offender_and_is_not_a_bare_count() {
    let decls = vec![
        CapDecl {
            name: "A_MAX".into(),
            file: "src/a.rs".into(),
            line: 3,
            annotation: None,
        },
        CapDecl {
            name: "B_LIMIT".into(),
            file: "src/b.rs".into(),
            line: 9,
            annotation: Some("RESULT_CAP b.rows — probed".into()),
        },
        CapDecl {
            name: "C_CAP".into(),
            file: "src/c.rs".into(),
            line: 4,
            annotation: Some("NOT_A_CAP".into()),
        },
    ];
    let got = unclassified_decls(&decls);
    assert_eq!(
        got,
        vec![
            "src/a.rs:3 A_MAX — no cap-class annotation".to_string(),
            "src/c.rs:4 C_CAP — NOT_A_CAP with no reason".to_string(),
        ],
        "the classified one must not appear, and each offender must arrive \
         with its file:line — a count tells nobody which constant to go fix"
    );
}

/// THE GATE. Every cap-shaped constant in tracked `src/` is classified.
#[test]
fn every_cap_constant_is_classified() {
    let mut offenders = vec![];
    for file in tracked_src_files() {
        let path = repo_root().join(&file);
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        offenders.extend(unclassified_decls(&cap_constants(&src, &file)));
    }
    assert!(
        offenders.is_empty(),
        "{} cap constant(s) carry no usable `cap-class:` annotation.\n\n{}\n\n\
         Add ONE of these on the line above each, in its doc comment:\n  \
         // cap-class: RESULT_CAP <surface>.<what> — probed\n  \
         // cap-class: NOT_A_CAP — <why this never shapes a result>\n\n\
         RESULT_CAP means a caller can receive a partial result because of \
         this bound; it then needs a probe row in \
         src/tools/core/cap_probe.rs. NOT_A_CAP needs a REASON, not just \
         the token — a timeout, a batch size, a retry ceiling. Why this \
         gate exists: docs/trackers/issue-clusters/\
         IC-13-capped-result-presented-as-complete.md",
        offenders.len(),
        offenders.join("\n")
    );
}

/// Truncation OPERATIONS, the scope instrument A cannot reach.
///
/// `.next()` is included because the `indexer.rs` first-chunk-only member
/// (fixed at `488192e8`) was a bare `.next()` on a chunk iterator with no
/// constant anywhere — invisible to a declaration scan by construction.
///
/// Exactly one shape is excluded: `.next().await` — an async stream drain,
/// which caps nothing. The exclusion is anchored to `.await`, not to any
/// substring of the receiver's name. An earlier draft excluded
/// `.contains("s.next()")` to catch `stream.next()`, but that pattern
/// matches ANY receiver whose name ends in `s` — `items.next()`,
/// `lines.next()`, `rows.next()`, `chars.next()` — which is silent
/// under-reporting in the permissive direction, the exact defect class this
/// gate exists to catch, occurring inside the gate itself. `.await` is
/// unambiguous: a synchronous `.next()` never has one. Pinned narrow by
/// `truncation_sites_ignores_stream_next_which_is_iteration_not_capping`
/// (the real exclusion) and
/// `truncation_sites_does_not_exclude_a_sync_next_on_a_receiver_ending_in_s`
/// (the over-broad pattern this rejects), because an over-broad instrument
/// that fires on every iterator teaches readers to annotate noise, and an
/// annotation written to silence a gate classifies nothing.
fn truncation_sites(src: &str, file: &str) -> Vec<TruncSite> {
    const OPS: [&str; 4] = [".take(", ".truncate(", "truncate_compact(", ".next()"];
    let lines: Vec<&str> = src.lines().collect();
    let mut out = vec![];

    for (idx, raw) in lines.iter().enumerate() {
        let code = raw.trim_start();
        if code.starts_with("//") {
            continue;
        }
        for op in OPS {
            if !code.contains(op) {
                continue;
            }
            if op == ".next()" && code.contains(".next().await") {
                continue;
            }
            out.push(TruncSite {
                op: op.to_string(),
                file: file.to_string(),
                line: idx + 1,
                annotation: annotation_above(&lines, idx),
            });
        }
    }
    out
}

/// Declarations the gate refuses, each named with its location.
///
/// Extracted so [`every_cap_constant_is_classified`] and
/// `unclassified_decls_names_every_offender_and_is_not_a_bare_count` run the
/// SAME filter rather than two copies that could drift.
fn unclassified_decls(decls: &[CapDecl]) -> Vec<String> {
    let mut out: Vec<String> = decls
        .iter()
        .filter_map(|d| match classify(d) {
            CapClass::Unclassified => Some(format!(
                "{}:{} {} — no cap-class annotation",
                d.file, d.line, d.name
            )),
            CapClass::MalformedReason => Some(format!(
                "{}:{} {} — NOT_A_CAP with no reason",
                d.file, d.line, d.name
            )),
            CapClass::ResultCap(_) | CapClass::NotACap(_) => None,
        })
        .collect();
    out.sort();
    out
}

/// Parses the `id` field of every `ProbeRow { id: "...", ... }` entry out
/// of `cap_probe.rs`'s source TEXT — never a compiled import. `cap_probe`
/// is `#[cfg(test)]`-gated and its items are `pub(crate)`, neither of which
/// this integration-test binary can `use` at all; and even if it could, a
/// compiled read would see `PROBE_ROWS` change shape across
/// `--no-default-features` the moment a row moves behind
/// `#[cfg(feature = "librarian")]` — the same asymmetry `cap_constants`
/// reads around for the `RESULT_CAP` side. A line only contributes an id
/// when it starts (after trimming) with the literal `id:` — a `pub id:
/// &'static str,` field declaration doesn't match (it starts with `pub`),
/// and an `id:` substring appearing mid-line inside some other field's
/// value (e.g. a `JsonPath` string) doesn't match either, because the
/// match is anchored to the start of the trimmed line.
fn probe_row_ids(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for raw in src.lines() {
        let line = raw.trim_start();
        let Some(rest) = line.strip_prefix("id:") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        out.insert(rest[..end].to_string());
    }
    out
}

#[test]
fn probe_row_ids_reads_a_quoted_id_field_and_ignores_everything_else() {
    let fixture = r#"
        pub(crate) struct ProbeRow {
            /// Matches a `cap-class: RESULT_CAP <id>` annotation in `src/`.
            pub id: &'static str,
        }
        pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
            ProbeRow {
                id: "a.b",
                coverage: Coverage::Deferred("x"),
            },
            ProbeRow {
                id: "c.d",
                coverage: Coverage::Probed {
                    marker: Marker::JsonPath("$.id: not-a-row"),
                    mutation: Mutation::NotYet("y"),
                },
            },
        ];
    "#;
    let ids = probe_row_ids(fixture);
    let expected: BTreeSet<String> = ["a.b", "c.d"].iter().map(|s| s.to_string()).collect();
    assert_eq!(ids, expected);
}

/// Both directions of `IC-13`'s cross-check: every `RESULT_CAP` id declared
/// in tracked `src/` has exactly one [`ProbeRow`]-shaped entry in
/// `cap_probe.rs`, and every entry there names an id that's actually
/// declared. Naming offenders in both directions — rather than a bare
/// count — is what makes a failure here actionable instead of a second
/// puzzle.
///
/// [`ProbeRow`]: crate is not visible from an integration-test binary; see
/// `probe_row_ids`'s doc comment for why this reads `cap_probe.rs` as text.
#[test]
fn result_caps_and_probe_rows_correspond_in_both_directions() {
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for file in tracked_src_files() {
        let path = repo_root().join(&file);
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        for decl in cap_constants(&src, &file) {
            if let CapClass::ResultCap(id) = classify(&decl) {
                declared.insert(id);
            }
        }
    }

    let cap_probe_path = repo_root().join("src/tools/core/cap_probe.rs");
    let cap_probe_src = std::fs::read_to_string(&cap_probe_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", cap_probe_path.display()));
    let rows = probe_row_ids(&cap_probe_src);

    let missing_rows: Vec<&String> = declared.difference(&rows).collect();
    let orphaned_rows: Vec<&String> = rows.difference(&declared).collect();

    assert!(
        missing_rows.is_empty() && orphaned_rows.is_empty(),
        "RESULT_CAP ids and cap_probe.rs's ProbeRows have drifted apart.\n\
         Declared with no ProbeRow: {missing_rows:?}\n\
         ProbeRow with no matching RESULT_CAP declaration: {orphaned_rows:?}"
    );
}

/// Wires [`truncation_sites`] (instrument B) to the REAL corpus. Every
/// other test for it runs against a small crafted fixture string; this is
/// the one place a change to `OPS` or the `.next().await` exclusion that
/// only breaks on real code — not on the hand-written fixtures — has
/// somewhere to fail.
#[test]
fn truncation_sites_reach_the_real_corpus() {
    let mut all_sites: Vec<TruncSite> = vec![];
    for file in tracked_src_files() {
        let path = repo_root().join(&file);
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        all_sites.extend(truncation_sites(&src, &file));
    }

    // `.next().await` on a stream is iteration, not capping — these two
    // real drain sites must never be reported.
    for (file, line) in [
        ("src/librarian/indexer.rs", 918usize),
        ("src/librarian/indexer.rs", 1051usize),
    ] {
        assert!(
            !all_sites.iter().any(|s| s.file == file && s.line == line),
            "{file}:{line} is a `.next().await` stream drain, not a cap — \
             truncation_sites must not report it"
        );
    }

    // A real, unambiguous `.truncate(` cap site must be found by exact
    // file:line — proof this instrument reaches production code, not only
    // its own test fixtures. NOTE: `real_line` is a literal line number in
    // `symbols.rs`, which drifts with unrelated edits to that file — if this
    // assertion starts failing after such an edit, re-check the current line
    // of its `.truncate(` call before assuming the instrument regressed.
    let (real_file, real_line, real_op) = ("src/tools/symbol/symbols.rs", 48usize, ".truncate(");
    assert!(
        all_sites
            .iter()
            .any(|s| s.file == real_file && s.line == real_line && s.op == real_op),
        "expected to find {real_file}:{real_line} (`{real_op}`) in the real corpus scan, but \
         it was not among the {} sites found — if symbols.rs was edited, its `.truncate(` \
         call may have moved off line {real_line}",
        all_sites.len()
    );
}

/// One `Coverage::Probed` row's id, marker, and cited test name — parsed
/// from `cap_probe.rs`'s source TEXT for the same reason [`probe_row_ids`]
/// is: the module is `#[cfg(test)]`-gated with `pub(crate)` items this
/// integration-test binary cannot `use`. `Marker` itself is not visible
/// either, so [`CitedMarker`] is a local stand-in carrying only what this
/// gate needs.
#[derive(Debug, PartialEq)]
struct ProbedCitation {
    id: String,
    marker: CitedMarker,
    cited_test: String,
}

/// A cited marker's kind and payload — see [`ProbedCitation`] for why this
/// is a local type rather than `cap_probe::Marker`.
#[derive(Debug, PartialEq)]
enum CitedMarker {
    JsonPath(String),
    TextContains(String),
}

/// Finds the first line in `chunk`, trimmed, that starts with the literal
/// `prefix`, and returns the first quoted string that follows it on that
/// same line. Reused by [`probed_citations`] for `id:`, `cited_test:`, and
/// both `marker: Marker::...(` forms — same anchoring rule as
/// [`probe_row_ids`]: a match must start the trimmed line, so `prefix`
/// appearing mid-line (inside a comment, or as part of another field's
/// value) does not match.
fn line_field(chunk: &str, prefix: &str) -> Option<String> {
    for raw in chunk.lines() {
        let line = raw.trim_start();
        let Some(rest) = line.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        return Some(rest[..end].to_string());
    }
    None
}

/// Extracts every `Coverage::Probed` row's `(id, marker, cited_test)` from
/// `cap_probe.rs`'s source TEXT.
///
/// Splits on the literal `"ProbeRow {"` token: every row in `PROBE_ROWS`
/// opens with exactly that text, and so does the `ProbeRow` struct's own
/// definition earlier in the file — so the FIRST chunk kept after
/// `.skip(1)` is that struct's tail (its remaining fields, `Tally`,
/// `tally()`, and the Task 6 mutation-sweep comment block), not a row. That
/// chunk DOES contain the
/// literal `"Coverage::Probed {"` — `tally()`'s own two `matches!` calls
/// put it there, both AFTER `struct ProbeRow {`'s own line, not before it
/// as an earlier version of this comment claimed (verified 2026-09-02:
/// `struct ProbeRow {` at `cap_probe.rs:69`, `tally()`'s two occurrences at
/// `:95` and `:103`) — so this chunk passes the `"Coverage::Probed {"`
/// filter exactly like a real row's chunk would (verified 2026-09-02:
/// exactly 4 occurrences of `"Coverage::Probed {"` exist in `cap_probe.rs`
/// today — 2 inside `tally()`, 2 inside the two real `Probed` rows — and
/// this loop reaches all 4). The struct's tail chunk produces no citation
/// only because its `id` field is written `pub id: &'static str,`:
/// [`line_field`]'s `"id:"` search anchors to the START of a trimmed line,
/// and no line in that chunk starts with the bare literal `id:` — the
/// field's own line starts with `pub`.
///
/// Each of the three fields is then read by [`line_field`] using the same
/// anchored-line technique [`probe_row_ids`] uses for `id:`. A chunk
/// missing any one of the three fields is silently excluded from THIS
/// function's own output — that covers both the struct's tail chunk
/// (excluded for the `pub id:` reason above, by design) and a genuine
/// `Coverage::Probed` row that fails to parse for some other formatting
/// reason (NOT by design: this function alone cannot tell the two cases
/// apart, and does not itself surface the difference — a prior version of
/// this comment claimed it did). [`count_probed_chunks`] closes that gap:
/// it counts chunks that clear the SAME `"Coverage::Probed {"` filter AND
/// have a bare `id:` field this function can read, which is exactly the
/// population this function ought to turn into a citation. A mismatch
/// between that count and `probed_citations(src).len()` — checked by
/// `probed_rows_cite_a_real_test` — is how a row silently dropped for the
/// wrong reason actually gets surfaced; this function does not surface it
/// on its own.
fn probed_citations(src: &str) -> Vec<ProbedCitation> {
    let mut out = vec![];
    for chunk in src.split("ProbeRow {").skip(1) {
        if !chunk.contains("Coverage::Probed {") {
            continue;
        }
        let Some(id) = line_field(chunk, "id:") else {
            continue;
        };
        let marker = if let Some(v) = line_field(chunk, "marker: Marker::JsonPath(") {
            CitedMarker::JsonPath(v)
        } else if let Some(v) = line_field(chunk, "marker: Marker::TextContains(") {
            CitedMarker::TextContains(v)
        } else {
            continue;
        };
        let Some(cited_test) = line_field(chunk, "cited_test:") else {
            continue;
        };
        out.push(ProbedCitation {
            id,
            marker,
            cited_test,
        });
    }
    out
}

/// Counts the chunks [`probed_citations`] iterates that look like a
/// genuine `Coverage::Probed` row: the same `"Coverage::Probed {"` filter,
/// AND a bare `id:` field [`line_field`] can find — the same two gates
/// that exclude the `ProbeRow` struct's own tail chunk (see
/// `probed_citations`'s doc comment) from ever being counted here. Using
/// the `"Coverage::Probed {"` filter ALONE, without the `id:` gate, would
/// overcount by exactly one in `cap_probe.rs` today: the struct's tail
/// chunk always contains that literal text (`tally()`'s `matches!` calls
/// put it there) and is correctly excluded BY DESIGN, not by accident, so
/// counting it here would make this function permanently disagree with
/// `probed_citations` for a reason that is not a bug.
///
/// A chunk that clears both gates here but still yields no citation from
/// `probed_citations` — because its `marker` or `cited_test` field could
/// not be parsed — is exactly the silent drop `probed_rows_cite_a_real_test`
/// exists to surface; comparing this count against
/// `probed_citations(src).len()` there is how.
fn count_probed_chunks(src: &str) -> usize {
    src.split("ProbeRow {")
        .skip(1)
        .filter(|chunk| chunk.contains("Coverage::Probed {") && line_field(chunk, "id:").is_some())
        .count()
}

#[test]
fn probed_citations_reads_marker_and_cited_test_alongside_id_and_skips_deferred_rows() {
    let fixture = r#"
        pub(crate) struct ProbeRow {
            pub id: &'static str,
            pub coverage: Coverage,
        }
        fn tally() {
            let _ = matches!(c, Coverage::Probed { .. });
            let _ = matches!(c, Coverage::Probed { .. });
        }
        pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
            ProbeRow {
                id: "a.b",
                coverage: Coverage::Deferred("no test yet"),
            },
            ProbeRow {
                id: "c.d",
                coverage: Coverage::Probed {
                    marker: Marker::TextContains("needle"),
                    mutation: Mutation::NotYet("x"),
                    cited_test: "some_test_fn",
                },
            },
            ProbeRow {
                id: "e.f",
                coverage: Coverage::Probed {
                    marker: Marker::JsonPath("$.a.b"),
                    mutation: Mutation::NotYet("x"),
                    cited_test: "another_test_fn",
                },
            },
        ];
    "#;
    let citations = probed_citations(fixture);
    assert_eq!(
        citations,
        vec![
            ProbedCitation {
                id: "c.d".to_string(),
                marker: CitedMarker::TextContains("needle".to_string()),
                cited_test: "some_test_fn".to_string(),
            },
            ProbedCitation {
                id: "e.f".to_string(),
                marker: CitedMarker::JsonPath("$.a.b".to_string()),
                cited_test: "another_test_fn".to_string(),
            },
        ],
        "the Deferred row a.b must not appear, and both Probed rows must carry their own \
         marker and cited_test, not the other's"
    );
    // The struct-tail chunk mirrors cap_probe.rs's own real shape: a `tally()`-style
    // function containing "Coverage::Probed {" text sits between `struct ProbeRow {`
    // and the first real row, so this chunk PASSES the substring filter exactly like a
    // real row's chunk would, and is excluded only because its own `id` field reads
    // `pub id:`, not the bare `id:` line_field anchors on. Without this snippet the
    // fixture only exercised "chunk fails the substring filter entirely" — a different,
    // easier case than the one that actually occurs in cap_probe.rs (see
    // `probed_citations`'s doc comment).
    assert_eq!(
        count_probed_chunks(fixture),
        citations.len(),
        "the struct-tail chunk must be excluded from the count for the SAME reason \
         probed_citations excludes it from its output (pub id: not bare id:), not because \
         it fails the Coverage::Probed {{ substring filter — this is what proves the two \
         functions agree for the right reason"
    );
}

/// Reproduces the Finding #1 exploit: a `Coverage::Probed` row collapsed onto ONE
/// physical line breaks [`line_field`]'s per-field anchoring for every field after the
/// first, because `line_field` scans `chunk.lines()` and only the very first (and here,
/// only) line of the chunk can ever match a prefix. The collapsed row's `id` happens to
/// be first on that line and still parses; `marker` and `cited_test` do not, so the
/// ENTIRE row is silently dropped from `probed_citations`'s output — while
/// [`count_probed_chunks`] still counts it as a candidate, because counting only needs
/// `id:` to parse, not all three fields. The resulting mismatch is exactly what
/// `probed_rows_cite_a_real_test`'s count assertion exists to catch: before that
/// assertion existed, this fixture's dropped row was invisible to the gate —
/// `probed_citations` returned only the second, well-formed row, and a bare
/// `!citations.is_empty()` was satisfied by it alone.
#[test]
fn count_probed_chunks_exceeds_citations_when_a_row_is_collapsed_onto_one_line() {
    let fixture = r#"
        pub(crate) struct ProbeRow {
            pub id: &'static str,
            pub coverage: Coverage,
        }
        pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
            ProbeRow { id: "collapsed.row", coverage: Coverage::Probed { marker: Marker::TextContains("needle"), mutation: Mutation::NotYet("x"), cited_test: "fabricated_test_fn" } },
            ProbeRow {
                id: "well.formed",
                coverage: Coverage::Probed {
                    marker: Marker::TextContains("needle"),
                    mutation: Mutation::NotYet("x"),
                    cited_test: "some_test_fn",
                },
            },
        ];
    "#;

    let citations = probed_citations(fixture);
    assert_eq!(
        citations,
        vec![ProbedCitation {
            id: "well.formed".to_string(),
            marker: CitedMarker::TextContains("needle".to_string()),
            cited_test: "some_test_fn".to_string(),
        }],
        "the collapsed row's id parses (it is first on its line) but its marker and \
         cited_test do not, so probed_citations must drop the WHOLE row rather than emit \
         a half-parsed citation: {citations:?}"
    );
    assert_eq!(
        count_probed_chunks(fixture),
        2,
        "both rows look like a genuine Coverage::Probed row by the id-gated chunk count — \
         this is the population probed_rows_cite_a_real_test's count assertion compares \
         citations.len() against"
    );
    assert_ne!(
        citations.len(),
        count_probed_chunks(fixture),
        "the mismatch (1 citation vs 2 candidate chunks) is the signal a silent drop \
         happened — this is what a bare !citations.is_empty() check cannot see"
    );
}

/// True when a trimmed line begins a `fn` declaration, allowing the
/// visibility and `async` qualifiers this corpus actually writes (`pub`,
/// `pub(crate)`, `pub(super)`, `async`, and combinations). Not a full
/// grammar — a qualifier this corpus does not use (e.g. `pub(in path)`)
/// would not be recognised, and a line using one would be invisible to
/// [`extract_fn_body`] as an end-of-body marker.
fn declares_a_fn(trimmed: &str) -> bool {
    let mut rest = trimmed;
    for prefix in ["pub(crate) ", "pub(super) ", "pub "] {
        if let Some(r) = rest.strip_prefix(prefix) {
            rest = r;
            break;
        }
    }
    if let Some(r) = rest.strip_prefix("async ") {
        rest = r;
    }
    rest.starts_with("fn ")
}

/// Line indices in `src` whose text declares `fn <name>`, by exactly the rule
/// [`extract_fn_body`] uses to find its START line.
///
/// Factored out so that counting declarations and extracting one body can never
/// use different matchers. A second, separately-written predicate here would be
/// `IC-18` (`selector-narrower-than-its-population`) shipped inside the gate
/// built to catch it: [`resolve_cited_test`] would be certifying uniqueness over
/// a population that is not the one [`extract_fn_body`] draws its answer from,
/// and the two could disagree silently. Inherits every blind spot listed on
/// [`extract_fn_body`]'s doc comment (a decoy inside a comment or raw-string
/// fixture counts as a declaration here too) — deliberately, since a decoy able
/// to misdirect the extractor must also be visible to the uniqueness check.
fn fn_declaration_lines(src: &str, name: &str) -> Vec<usize> {
    let paren = format!("fn {name}(");
    let spaced = format!("fn {name} ");
    src.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(paren.as_str()) || line.contains(spaced.as_str()))
        .map(|(i, _)| i)
        .collect()
}

/// Extracts the heuristic "body" of `fn <name>` (any visibility, `async` or
/// not) from `src`: the line declaring it, through the line immediately
/// before the next line — at the SAME OR LOWER indentation — that itself
/// declares a `fn`, with that next function's own leading `#[...]`
/// attributes and plain `//`-prefixed comment lines (including `///` and
/// `//!` doc comments) backed out of the result (see the last bullet
/// below). Returns `None` when no line declares `fn <name>` at all.
///
/// This is a HEURISTIC, not a parser, and `probed_rows_cite_a_real_test`
/// relies on knowing exactly what it does not catch:
///
/// - **A comment or string literal containing the text `fn <name>(`** would
///   be misread as the declaration. Nothing here skips comments or string
///   bodies, unlike this file's `raw_string_lines` for the declaration
///   scanner — accepted for this gate because a cited test name is a real
///   identifier the compiler already forces to be unique among sibling
///   `fn`s, so a decoy occurring only in prose is a narrower risk than the
///   one `raw_string_lines` was written for. The SAME blind spot applies to
///   the END boundary below, not only the START: a line inside a raw-string
///   fixture (this very file's own `r#"..."#` bodies, for instance) that
///   happens to look like a `fn` declaration at the same-or-lower
///   indentation would end the scan early, exactly as a decoy at the START
///   would misdirect it.
/// - **A `fn` nested inside the body at STRICTLY GREATER indentation** (a
///   local helper function) is correctly kept as part of the body, because
///   ending the scan requires indentation `<=` the declaration's own — but
///   a `fn` nested at exactly the SAME indentation (legal Rust, unusual
///   style) would end the body early, before the outer function's own
///   closing brace.
/// - **The first matching declaration wins** when a name is declared more
///   than once across the scanned source — impossible for two sibling
///   `#[test]` functions (duplicate names in the same module do not
///   compile), but a real limitation of this function taken on its own,
///   independent of how `probed_rows_cite_a_real_test` uses it. That gate
///   no longer relies on it: [`resolve_cited_test`] counts declarations via
///   [`fn_declaration_lines`] first and REFUSES an ambiguous name outright,
///   so first-match is reached only after uniqueness is established.
/// - **A name that is a substring of another never matches upward**: the
///   search requires the literal `fn <name>(` or `fn <name> ` (for a name
///   followed by generics or unusual whitespace before punctuation), so
///   searching for `foo` cannot match a line declaring `fn foo_helper`.
/// - **The end boundary backs up over the NEXT function's leading `#[...]`
///   attributes and any plain `//`-prefixed comment line** (not only
///   `///`/`//!` doc comments — a bare `// note` immediately before a
///   `#[test]` backs up too), stopping at the first blank line or other
///   line found walking backward from the next `fn` declaration. Without
///   this, the next function's own `#[test]` and doc/plain comments landed
///   inside THIS function's extracted body (verified: a `#[test]`
///   attribute was included in a prior version's
///   `glob_explosion_returns_recoverable` extraction). This back-up is
///   itself a heuristic: a `/* block comment */`, or an attribute separated
///   from the `fn` line by something other than more attributes/comments,
///   is not recognised and would still leak into the body.
fn extract_fn_body(src: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    let start = *fn_declaration_lines(src, name).first()?;
    let start_indent = lines[start].len() - lines[start].trim_start().len();
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent <= start_indent && declares_a_fn(trimmed) {
            end = i;
            break;
        }
    }
    // Back up over the NEXT function's own leading attributes and comment
    // lines — the forward scan above only recognises that function's `fn`
    // line itself as an end marker, so without this its `#[...]` and any
    // `//`-prefixed line (plain comment or `///`/`//!` doc comment) land
    // inside THIS function's extracted body. Stops at the first blank line
    // (the real gap between the two functions) or any other line, so it
    // never walks into the CURRENT function's own content.
    while end > start + 1 {
        let trimmed = lines[end - 1].trim_start();
        if trimmed.starts_with("#[") || trimmed.starts_with("//") {
            end -= 1;
        } else {
            break;
        }
    }
    Some(lines[start..end].join("\n"))
}

#[test]
fn extract_fn_body_finds_the_declaration_and_stops_at_the_next_fn_at_same_or_lower_indent() {
    let fixture = "\
mod tests {
    fn unrelated_before() {
        let x = 1;
    }

    #[test]
    fn target_fn() {
        let marker_text = \"needle\";
        assert!(marker_text.contains(\"needle\"));
    }

    #[test]
    fn unrelated_after() {
        let y = 2;
    }
}
";
    let body = extract_fn_body(fixture, "target_fn").expect("target_fn should be found");
    assert!(
        body.contains("needle"),
        "body should include the fn's own content: {body}"
    );
    assert!(
        !body.contains("unrelated_after"),
        "body must stop before the NEXT fn at the same indentation: {body}"
    );
    // extract_fn_body's `start` search anchors on the line containing `fn target_fn(`
    // itself, never on any attribute preceding it — so target_fn's OWN `#[test]` is
    // never part of the extracted body either, before or after this fix. A body
    // containing zero occurrences of "#[test]" is therefore exactly what a correct
    // extraction produces; before the end-boundary fix this body contained ONE
    // (`unrelated_after`'s leaked attribute), never target_fn's own.
    assert!(
        !body.contains("#[test]"),
        "the NEXT function's own #[test] attribute must not leak into this body: {body}"
    );
}

/// [`extract_fn_body`]'s end boundary must back up over the next function's leading DOC
/// COMMENT too, not only its `#[...]` attribute — this fixture puts a `///` line (no
/// `#[test]`) directly before the next `fn`, a case the attribute-only fixture above does
/// not cover.
#[test]
fn extract_fn_body_excludes_the_next_functions_leading_doc_comment() {
    let fixture = "\
fn target_fn() {
    let x = 1;
}

/// Doc comment that belongs to the NEXT function, not this one.
fn next_fn() {
    let y = 2;
}
";
    let body = extract_fn_body(fixture, "target_fn").expect("target_fn should be found");
    assert!(
        !body.contains("belongs to the NEXT function"),
        "the next function's leading doc comment must not leak into this body: {body}"
    );
    assert!(
        !body.contains("next_fn"),
        "the next function's declaration must not leak into this body: {body}"
    );
}

#[test]
fn extract_fn_body_returns_none_when_the_name_is_not_declared_anywhere() {
    let fixture = "mod tests {\n    fn something_else() {}\n}\n";
    assert!(extract_fn_body(fixture, "does_not_exist_anywhere").is_none());
}

/// Guards M8: [`extract_fn_body`]'s end-boundary back-up must recognise a PLAIN `//`
/// comment line, not only `///`/`//!` doc comments — narrowing the check to
/// `starts_with("///") || starts_with("//!")` (dropping bare `//`) would leave this
/// exact fixture's plain comment inside `target_fn`'s extracted body, because the
/// fixture's only leading line before `next_fn` is a bare `// note`, not a doc comment.
#[test]
fn extract_fn_body_excludes_the_next_functions_plain_comment() {
    let fixture = "\
fn target_fn() {
    let x = 1;
}

// note that belongs to the NEXT function, not this one.
fn next_fn() {
    let y = 2;
}
";
    // The BARE `//` on the fixture's comment line is the whole point of this test, and
    // the detail is machine-checked rather than merely annotated: change it to `///` and
    // this test becomes a duplicate of
    // `extract_fn_body_excludes_the_next_functions_leading_doc_comment`, silently ceasing
    // to guard the bare-`//` half of `extract_fn_body`'s back-up walk. Measured in round
    // 3's re-review: `//` -> `///` in the fixture, plus narrowing the production check to
    // `starts_with("///") || starts_with("//!")`, left the suite fully green.
    //
    // This assertion guards the FIXTURE, not the production path — the two `!contains`
    // assertions below are what exercise `extract_fn_body`. It buys exactly one thing:
    // the load-bearing detail can no longer be removed silently.
    assert!(
        fixture.contains("\n// note"),
        "fixture must keep a BARE `//` comment line, not `///` or `//!`"
    );
    let body = extract_fn_body(fixture, "target_fn").expect("target_fn should be found");
    assert!(
        !body.contains("belongs to the NEXT function"),
        "the next function's leading plain comment must not leak into this body: {body}"
    );
    assert!(
        !body.contains("next_fn"),
        "the next function's declaration must not leak into this body: {body}"
    );
}

/// The lines of `body` that belong to an `assert!`/`assert_eq!`/`assert_ne!`/
/// `debug_assert!` call — from the line the macro name appears on (at paren-depth 0)
/// through the line that closes its outermost `(...)`, tracked by counting `(`/`)`
/// across each included line, and then narrowed by [`condition_args`] to just the
/// value/condition arguments (never the failure-message argument). A single physical
/// line is NOT enough: rustfmt wraps a long `assert!`/`assert_eq!` call so the macro
/// name is on its own line and the condition, actual marker text included, lands on
/// the NEXT line — the shape both real `cited_test` bodies this gate checks use. A
/// per-line-only filter (matching just the line containing the literal `assert`)
/// would exclude that condition line entirely, making every multi-line assertion in
/// this style invisible to [`marker_is_asserted`] — checked against both real call
/// sites before landing on this design.
///
/// The macro-name trigger is matched at a WORD BOUNDARY via [`opens_assert_call`], not
/// as a bare substring: an earlier version matched `line.contains("assert")`, which
/// also opened an inclusion window on `assertions`, `asserted`, `reassert`, or a
/// comment reading `// we assert so` — and the paren-depth counter would then swallow
/// following lines as if they were part of a real call. Lines whose trimmed form
/// starts with `//` are skipped entirely before the trigger check, so a comment can
/// never open (or, via its own stray `(`, corrupt the depth of) an assertion span. Two
/// separate guards, because the CALL SITE and the RULE are different sites and
/// `opens_assert_call`'s own test cannot see a dead call:
/// `assertion_lines_trigger_uses_the_word_boundary_matcher_at_its_call_site` (the call
/// below) and `assertion_lines_skips_a_comment_line_that_merely_mentions_assert` (the
/// `//` skip). Through round 3 the call site had no guard of its own and reverting it
/// alone left the suite green.
///
/// [`marker_is_asserted`] searches this text, not the whole body: searching the whole
/// body let deleting the very assertion a row cites still pass, because the marker's
/// text also occurred elsewhere in the body — a comment, a nearby `let`, the enclosing
/// test's setup code. And searching the FULL assertion span (condition plus failure
/// message) let deleting or weakening the condition still pass, because the marker
/// text lived in the message string instead — verified by mutation at both of this
/// gate's current call sites (reported by `probed_rows_cite_a_real_test`'s caller):
/// weakening `glob_explosion_returns_recoverable`'s condition to `msg.contains('5')`
/// alone left `TextContains("cap")` satisfied by that same `assert!`'s own message
/// argument ("error should name the **cap** and the actual count"); and deleting only
/// `a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped`'s
/// `packing` `assert_eq!` left both `$.overflow.packing` segments present via a
/// NEIGHBOURING `assert_eq!`'s own message text ("full-text **packing** would have
/// dropped most of them") plus its `v["overflow"]` indexing. [`condition_args`] closes
/// both DIRECTLY, keeping only the condition/value arguments of each recognised macro
/// — but round 2's `condition_args` reopened the same escape two other ways, both
/// fixed only in round 3: (1) its macro-TOKEN selection searched the whole block
/// (message included), so a message containing the literal text `"assert_eq!"` was
/// misread as the macro invocation itself and could shift the extracted span into the
/// message; (2) its scan indexed `char_indices()` (byte offsets) with `.skip(n)`
/// (which skips `n` CHARACTERS, not bytes) — identical only when everything before the
/// call is ASCII, so a single multi-byte character (e.g. an em dash) anywhere before
/// the macro's own `(` could misalign the scan. Both are demonstrated at
/// `probed_rows_cite_a_real_test`'s two live call sites by mutating
/// `glob_explosion_returns_recoverable`'s message text alone (condition unchanged from
/// its already-weakened form): adding `"(cf. assert_eq! docs)"` triggered (1), and
/// swapping a `;` for `—` triggered (2). A third round-2 gap — `condition_args`'
/// fallbacks returning `block.to_string()` (the block unchanged, message included) for
/// any shape they could not parse — is now `String::new()`, so an assertion this
/// parser cannot handle contributes no searchable text at all rather than falling back
/// to the lax direction.
///
/// Remaining known limits, in full — naming only one here would imply the others are
/// handled. Each is either pinned by a named test below or explicitly marked untested;
/// three consecutive rounds shipped a false claim in this very block, so an unmarked
/// claim here is a defect in its own right:
/// - **Cross-assertion laxity** *(untested — traced, no fixture)*: for `TextContains` and
///   a single-segment `JsonPath` (e.g. `JsonPath("$.packing")`), a marker string present
///   in the condition arguments of some OTHER assertion in the same body — not the one
///   actually exercising the cap — still satisfies [`marker_is_asserted`], because this
///   function concatenates every recognised assertion's condition text rather than
///   requiring one specific assertion to carry the whole marker. **Closed for a
///   multi-segment `JsonPath`** (e.g. `JsonPath("$.overflow.packing")`) by the one-line
///   rule at [`marker_is_asserted`]'s own doc comment, which requires every segment to
///   appear together on ONE assertion's condition line. Neither of
///   this gate's two live rows depends on this laxity (checked by reading both bodies;
///   no test enforces that it stays so).
/// - **Not string-literal-aware, in BOTH directions — and the false-GREEN half is wider
///   than string literals**: [`condition_args`] scans `block` as raw text, not as Rust
///   tokens, so a comma or paren INSIDE a string literal is indistinguishable from a real
///   argument separator. The common cases narrow (a false RED, the safe direction) —
///   `condition_args_string_literal_hazards`. **But not all of them**: an unbalanced `(`
///   anywhere in the condition's non-code text — a string literal, a raw string literal,
///   a char literal, or a trailing comment, paired with a `)` in the message — WIDENS the
///   kept text to include message content — a false GREEN, the dangerous direction,
///   recorded with a worked input by `condition_args_string_literal_widening_is_a_known_vector`.
///   Round 3's claim that this limit only ever produces a false RED was wrong and is
///   withdrawn; see [`condition_args`]' own doc comment for the other three measured
///   constructs and for what Task 5c must therefore avoid citing.
/// - **Fixture text that is not code** *(untested — a corpus property, not a code
///   property)*: the CORPUS's `src/tools/edit_file/tests.rs:3758` and `:3791` write
///   `assert!(x, "msg")` as Rust STRING DATA, which [`opens_assert_call`] still opens a
///   span on. Harmless while neither is a `cited_test`, and a real hazard for any future
///   citation search of the corpus. (Round 3 wrote "this file's own …" of a file that is
///   not this one.)
fn assertion_lines(body: &str) -> String {
    let mut out = String::new();
    let mut depth: i32 = 0;
    let mut block = String::new();
    for line in body.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let starts_call = depth == 0 && opens_assert_call(line);
        if depth > 0 || starts_call {
            block.push_str(line);
            block.push('\n');
            for ch in line.chars() {
                match ch {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= 0 {
                depth = 0;
                out.push_str(&condition_args(&block));
                out.push('\n');
                block.clear();
            }
        }
    }
    out
}

// `ASSERT_MACROS` is CODE, not a comment: `opens_assert_call` returns `true` on each of
// this table's four entry lines (each contains the literal text `"assert!"` etc. at a
// word boundary), and as of round 4 `condition_args` takes its token, that token's byte
// offset AND its `keep` count from this one table via `first_assert_token`.
// `tracked_src_files()` currently scopes the gate to `src/`, which excludes this file
// (`tests/result_caps.rs`) from its own scan — but if that scope is ever widened to
// include `tests/`, these lines become matchable assertion text and the gate's own token
// list would need re-checking against itself. No code change needed today; this is the
// annotation `tracked_src_files_returns_rust_files_under_src_and_excludes_tests` cannot
// express on its own.
//
// The second element is `keep`: how many LEADING arguments of that macro are
// condition/value arguments rather than the failure message. Each entry is deliberately
// its own mutable site — a `match tok { "assert_eq!" | "assert_ne!" => 2, _ => 1 }` would
// read identically and collapse four sites into two, making "is `assert_ne!`'s keep
// guarded?" unanswerable separately from `assert_eq!`'s. One test per entry:
// `condition_args_assert_keeps_only_the_first_argument`,
// `condition_args_assert_eq_keeps_only_the_first_two_arguments`,
// `condition_args_assert_ne_keeps_only_the_first_two_arguments`,
// `condition_args_debug_assert_keeps_only_the_first_argument`.
const ASSERT_MACROS: [(&str, usize); 4] = [
    ("assert!", 1),
    ("assert_eq!", 2),
    ("assert_ne!", 2),
    ("debug_assert!", 1),
];

/// The EARLIEST [`ASSERT_MACROS`] entry occurring in `line` at a word boundary — the
/// character immediately before the match, if any, is not alphanumeric or `_` — returned
/// as `(byte offset, token, keep)`.
///
/// This is the ONE matcher over the assert-macro namespace. [`opens_assert_call`] is its
/// `.is_some()`, and [`condition_args`] takes the offset and the `keep` count straight
/// from it, so the trigger that admits a block and the parser that narrows it can no
/// longer disagree. They did for three rounds: `opens_assert_call` was word-boundary
/// aware while `condition_args` ran its own bare `contains`/`find` pair, and every time
/// they disagreed the resolution was toward widening.
///
/// Selection is POSITIONAL, not by name preference — this is the round-4 fix. The
/// superseded chain (`if first_line.contains("assert_eq!") … else if …
/// contains("assert!")`, then `find`) picked the macro by NAME, so in
/// `assert!(x.is_ok(), "see assert_eq!(a, b) docs")` the `assert_eq!` branch won on a
/// substring of the failure MESSAGE, `find` landed inside that message, and the extracted
/// "condition" was the message's own text with the real condition dropped — a false
/// GREEN. Earliest-wins makes `assert!` at column 0 beat `assert_eq!` at column 20. Both
/// demonstrated vectors are pinned by
/// `condition_args_prefers_the_earliest_token_over_a_later_one_in_the_message` and
/// `condition_args_prefers_the_earliest_token_when_a_later_one_hides_in_a_trailing_comment`.
fn first_assert_token(line: &str) -> Option<(usize, &'static str, usize)> {
    ASSERT_MACROS
        .iter()
        .filter_map(|(tok, keep)| {
            line.match_indices(tok)
                .find(|(idx, _)| {
                    !matches!(line[..*idx].chars().next_back(), Some(c) if c.is_alphanumeric() || c == '_')
                })
                .map(|(idx, _)| (idx, *tok, *keep))
        })
        .min_by_key(|(idx, _, _)| *idx)
}

/// True when `line` contains one of [`ASSERT_MACROS`] at a word boundary — a thin
/// `.is_some()` over [`first_assert_token`], which is where the matching happens. Guards
/// [`assertion_lines`]'s trigger against `assertions`, `asserted`, `reassert`, and
/// similar tokens that contain `assert` as a substring but do not open a real macro call.
///
/// Two tests, because they answer different questions and neither answers the other's:
/// `opens_assert_call_requires_a_word_boundary_before_the_token` checks the RULE by
/// calling this function directly, and
/// `assertion_lines_trigger_uses_the_word_boundary_matcher_at_its_call_site` checks that
/// [`assertion_lines`] still CALLS it. A helper test cannot observe a dead call site —
/// reverting the call at [`assertion_lines`] to `line.contains("assert")` left the whole
/// suite green through round 3.
///
/// Recognises exactly the four tokens in [`ASSERT_MACROS`] — `assert!`, `assert_eq!`,
/// `assert_ne!`, `debug_assert!`. It does NOT recognise `debug_assert_eq!`,
/// `debug_assert_ne!`, or `assert_matches!` (from the `assert_matches` crate): none of
/// those tokens appears in the table, and the two `debug_assert_*` forms are additionally
/// rejected by the word-boundary rule, since the `assert_eq!`/`assert_ne!` inside them is
/// preceded by `_` — the half of the rule that `opens_assert_call_requires_a_word_boundary_before_the_token`'s
/// `debug_assert_eq!` and `my_assert!` cases pin, and the only half where the predicate
/// rather than the token list does the rejecting. The failure direction is SAFE rather
/// than silently lax, and the
/// reason is that THIS function gates entry into [`assertion_lines`]' block accumulation:
/// a line using an unrecognised macro never opens a span, so it contributes no text to
/// [`marker_is_asserted`]'s search at all, the marker is not found, and
/// `probed_rows_cite_a_real_test` REDs for that row. Verified on the production path by
/// `assertion_lines_contributes_nothing_for_an_unrecognised_assert_macro`.
///
/// (Round 3's doc comment attributed that safety to [`condition_args`]' `String::new()`
/// fallbacks. The DIRECTION claim was and remains true; the ATTRIBUTION was wrong —
/// [`condition_args`] is never reached at all for an unrecognised macro name, because
/// this function gates entry first.)
fn opens_assert_call(line: &str) -> bool {
    first_assert_token(line).is_some()
}

#[test]
fn opens_assert_call_requires_a_word_boundary_before_the_token() {
    // M9's reverted trigger (`line.contains("assert")`, no word-boundary check) would
    // fire on `assertions_ok` even though no `(` ever follows `assert` as a real macro
    // token here. Note what this pair does and does NOT discriminate: `assertions_ok`
    // contains no ASSERT_MACROS token as a SUBSTRING at all (no `!`), so it is rejected
    // by the token LIST and the boundary predicate is never consulted. Round 4 measured
    // this: deleting the boundary predicate outright left the whole suite green on these
    // two lines alone.
    assert!(!opens_assert_call("let assertions_ok = true;"));
    assert!(opens_assert_call("assert!(x.contains(\"cap\"));"));

    // These are the cases where the WORD-BOUNDARY PREDICATE is the only thing doing the
    // rejecting — the token IS present as a substring, preceded by `_`. Without the
    // predicate, `debug_assert_eq!` would be parsed as an `assert_eq!` starting at offset
    // 6 (keep 2), which is exactly the recognition `opens_assert_call`'s doc comment says
    // does not happen, and `my_assert!` as an `assert!` at offset 3.
    assert!(
        !opens_assert_call("debug_assert_eq!(a, b);"), // LOAD-BEARING: `assert_eq!` is a substring of `debug_assert_eq!` at offset 6, preceded by `_`. This is the only test in the file where the boundary predicate, not the token list, decides.
        "debug_assert_eq! is not in ASSERT_MACROS and its inner assert_eq! is preceded by `_`"
    );
    assert!(
        !opens_assert_call("my_assert!(x, \"msg\");"), // LOAD-BEARING: same shape for a project-local wrapper — `assert!` at offset 3 preceded by `_`.
        "a project-local wrapper must not be parsed as the bare macro it wraps"
    );
}

#[test]
fn assertion_lines_skips_a_comment_line_that_merely_mentions_assert() {
    // M9's second half drops the `line.trim_start().starts_with("//")` skip. Without
    // it, this COMMENT line's own `assert!(...)` text would open a real block, and
    // "cap" — present only in the comment, never actually asserted — would leak into
    // the searchable text.
    let body = "// assert!(x.contains(\"cap\")); -- just a note, not real code\nlet y = 1;\n";
    assert!(!assertion_lines(body).contains("cap"));
}

#[test]
fn assertion_lines_excludes_the_failure_message_argument() {
    // M10 bypasses `condition_args` entirely (`out.push_str(&block)`), which would
    // push the whole block — including this message argument — into the searchable
    // text `marker_is_asserted` scans.
    let body = "assert!(x.is_ok(), \"message_only_token\");\n";
    assert!(!assertion_lines(body).contains("message_only_token"));
}

#[test]
fn assertion_lines_trigger_uses_the_word_boundary_matcher_at_its_call_site() {
    // Guards the CALL SITE inside `assertion_lines`, not the RULE.
    // `opens_assert_call_requires_a_word_boundary_before_the_token` calls the helper
    // directly, so it stays green when the call below is reverted to
    // `line.contains("assert")` — an alarm nothing reaches. Through round 3 that revert,
    // done alone, left the whole suite green.
    //
    // Under the reverted trigger, line 0 opens a span (it contains "assert" inside
    // `reassert`) that swallows the real `assert!` on line 1; `condition_args` then reads
    // its token from line 0, finds none, and fails safe to an empty string — so the
    // marker the real trigger keeps is dropped.
    let body =
        "let reassert = compute(\n    assert!(z.contains(\"call_site_marker\"), \"msg\"),\n);\n"; // LOAD-BEARING, both details: "assert" appears only inside the longer identifier `reassert` (no word-boundary macro token), AND line 0 leaves a paren open. Close that paren, or rename `reassert`, and this test passes under the reverted trigger too, silently ceasing to guard the call site.
    assert!(
        assertion_lines(body).contains("call_site_marker"),
        "the real assert! on line 1 must open its own span; got: {:?}",
        assertion_lines(body)
    );
}

#[test]
fn assertion_lines_contributes_nothing_for_an_unrecognised_assert_macro() {
    // Pins the "fails SAFE" direction claimed on `opens_assert_call`'s doc comment, and
    // pins it on the production path rather than at the helper: `assert_matches!` is not
    // in `ASSERT_MACROS`, so this line never opens a span at all and contributes no
    // searchable text. A row citing such a test would RED in
    // `probed_rows_cite_a_real_test` rather than be silently certified on message text.
    let body = "assert_matches!(x, Ok(v) if v.contains(\"unrecognised_macro_marker\"));\n"; // LOAD-BEARING: the macro must be one absent from `ASSERT_MACROS`. Swap in `assert!` and this test asserts something UNRELATED to what it is named for, not the opposite — with `ASSERT_MACROS` unchanged the marker still would not be found (the macro syntax around it would differ, not the pass/fail direction this test pins). This test is also PARTLY INERT against the mechanism it names: adding `("assert_matches!", 1)` to `ASSERT_MACROS` reds two other tests but leaves this one GREEN, because `keep = 1` truncates at the first comma so the marker never appears either way. That gap is guarded elsewhere (M14, and the table-addition mutation), and `assertion_lines_trigger_uses_the_word_boundary_matcher_at_its_call_site` is the positive twin this absence assertion needs. Annotating a partly-inert fixture as such is the point — it stops a reader crediting it with coverage it does not provide.
    assert!(!assertion_lines(body).contains("unrecognised_macro_marker"));
}

/// Narrows a complete assertion span (as accumulated by [`assertion_lines`], one macro
/// call's lines with balanced parens) to just its value/condition arguments, splitting
/// the macro's argument list on TOP-LEVEL commas (paren depth 1, relative to the macro's
/// own opening `(`). WHICH macro, WHERE it starts and HOW MANY arguments to keep all come
/// from [`first_assert_token`] — the same matcher [`opens_assert_call`] uses, so this
/// function can no longer disagree with the trigger that admitted the block. This is what
/// excludes a macro's failure-message argument from the text [`marker_is_asserted`]
/// searches.
///
/// Any shape reaching one of the three fallback points below contributes an EMPTY string
/// rather than the block unchanged — the FAIL-SAFE direction, not "conservative":
/// returning the block let a failure-message argument (or, before round 3, an entire
/// unparseable span) stay searchable, which is the exact laxity this function exists to
/// remove. The three routes, each with what reaches it and the test that pins it:
///
/// 1. **No recognised token on line 0.** Pinned by
///    `condition_args_returns_empty_for_an_unrecognised_macro_rather_than_the_whole_block`.
///    UNREACHABLE from [`assertion_lines`] as of round 4, by construction rather than by
///    test: the block's line 0 is exactly the line `opens_assert_call` fired on, and both
///    now consult [`first_assert_token`], so a `Some` there implies a `Some` here.
///    Reachable only by calling this function directly, as that test does.
/// 2. **Token found, but no `(` follows it anywhere in the block.** Pinned by
///    `condition_args_returns_empty_when_the_token_is_never_invoked`, which asserts both
///    directly and through [`assertion_lines`]. Reachable from [`assertion_lines`]: a
///    line that NAMES a macro without calling it has balanced (zero) parens, so the span
///    closes on that same line and arrives here.
/// 3. **Paren-depth tracking never closes** — the `_` arm of the final `match`. Pinned by
///    `condition_args_returns_empty_when_the_call_never_closes`. This is the ONLY route
///    that reaches that arm; routes 1 and 2 `return` early. Reachable from
///    [`assertion_lines`] only by a CONTRIVED shape — the block's parens must balance (so
///    the span closes and this function runs) while the parens after the macro's own `(`
///    must not, which takes a stray `)` before the token. That test carries the worked
///    fixture and asserts it through [`assertion_lines`] as well as directly. A round-3
///    draft of this comment attributed the route to a string literal's unbalanced `(`;
///    that is wrong, because such a block does not close in [`assertion_lines`] either
///    and this function is then never called.
///
/// A FOURTH route existed through round 3 and is now structurally gone rather than merely
/// untested: `first_line.find(tok)` re-ran the same search on the same haystack that
/// `first_line.contains(tok)` had just succeeded on, so its `else` branch was dead code.
/// [`first_assert_token`] returns the offset together with the token; there is nothing
/// left to re-find.
///
/// **Not string-literal-aware, and this is a KNOWN FALSE-GREEN vector — a round-4
/// correction, not a restatement.** The scan below reads `block` as raw bytes, not Rust
/// tokens, so a comma or paren INSIDE a string literal is indistinguishable from a real
/// argument separator. Round 3's doc comment claimed *"every constructible case traces as
/// NARROWING … the failure direction is a false RED, never a false GREEN"*. **That claim
/// is FALSE and is withdrawn.** An UNBALANCED `(` inside a condition's string literal
/// raises the depth past 1, so the real argument comma is not counted; a later `)` inside
/// the MESSAGE brings depth back to 1, and the next comma there terminates the keep —
/// pulling message text into the searched region. Recorded, with a worked input, by
/// `condition_args_string_literal_widening_is_a_known_vector`, which asserts today's
/// WRONG output on purpose. The narrowing cases (the common ones) are traced by
/// `condition_args_string_literal_hazards`.
///
/// **No fix is attempted for that vector, deliberately.** A correct answer needs a Rust
/// tokenizer; widening this parser until the vector went green is what each of the
/// previous three rounds did in a different place, and each widening was the round after's
/// defect. Consequence for callers, stated at the refusal site rather than left implicit —
/// and wider than a single construct, because the scan is byte-level, not token-level: a
/// `Coverage::Probed` row whose `cited_test` asserts through a condition carrying an
/// unbalanced `(` **anywhere in its non-code text — a string literal, a raw string
/// literal, a char literal, or a trailing comment** — may be certified on message text
/// instead. Measured further instances, none a string literal: a CHAR literal
/// (`c == '(', "msg …"`), a trailing COMMENT after the condition (`x.is_ok() // stray
/// paren in comment: (`), and a RAW string (`x == r"(", "msg …"`) each reach the identical
/// false GREEN. So **Task 5c must not cite an assertion whose condition contains an
/// unbalanced `(` in any of these four forms** — a bare `'('` condition is not safe just
/// because it is not a string literal — and the guarantee `Coverage::Probed`'s doc comment
/// makes is bounded by this (see `condition_args_string_literal_widening_is_a_known_vector`
/// for the pinned worked case; the other three are recorded here, not as separate tests).
/// Neither of the two live rows is affected — not because either
/// condition lacks a string literal (both have one: `msg.contains("cap")` in
/// `glob_explosion_returns_recoverable`, and `json!("excerpted")` plus two
/// `md.contains(...)` calls in the packing row) but because neither literal contains an
/// unbalanced `(`, which is what the vector actually needs. (Rustfmt wrapping the macro
/// alone on line 0 is inert here: line-0 scoping governs token SELECTION; this scan reads
/// the whole block, so line-0-ness protects nothing against this vector.)
///
/// [`raw_string_lines`] was considered and is NOT reusable here: it classifies whole LINES
/// as inside/outside a RAW string literal (`r"..."`, `r#"..."#`) for a caller that reads
/// source declaration-by-line, where this function's hazard is an ordinary `"..."` literal
/// embedded WITHIN one already-extracted single- or multi-line block, and needs a
/// byte-position judgement inside a line, not a line classification — different literal
/// kind, different granularity, and the wrong axis to build on rather than duplicate.
///
/// The CORPUS's own `src/tools/edit_file/tests.rs:3758` and `:3791` write
/// `assert!(x, "msg")` as Rust STRING DATA — a real instance of [`opens_assert_call`]
/// opening a span on fixture text that is not code — harmless today because neither is a
/// `cited_test`, and a live hazard for Task 5c's citation search of that corpus. (Round 3
/// wrote "this file's own …", which reads as `tests/result_caps.rs`; those fixtures live
/// in a different file entirely.)
fn condition_args(block: &str) -> String {
    // Only the macro's own line can open its call — `assertion_lines` only ever opens
    // a span on a line where `opens_assert_call` fired (see its doc comment), so the
    // token lives on line 0 of `block` and nowhere else. Searching the WHOLE block
    // (message included) let a failure message that happens to contain the literal
    // text "assert_eq!" be misread as the macro invocation itself — fixed round 3,
    // guarded by `condition_args_selects_the_macro_token_from_the_first_line_only`.
    //
    // Round 4: WHICH token on that line is chosen positionally (earliest word-boundary
    // match wins) by `first_assert_token`, not by name preference. See its doc comment
    // for the two false-GREEN vectors the name-preference chain admitted.
    let first_line = block.lines().next().unwrap_or(block);
    let Some((tok_start, tok, keep)) = first_assert_token(first_line) else {
        return String::new();
    };
    let Some(open_rel) = block[tok_start + tok.len()..].find('(') else {
        return String::new();
    };
    let open_idx = tok_start + tok.len() + open_rel;
    let mut depth: i32 = 0;
    let mut arg_count: usize = 0;
    let mut args_start: Option<usize> = None;
    let mut kept_end: Option<usize> = None;
    // `open_idx` is a BYTE offset (from `find`); `skip_while` on the byte index of each
    // `char_indices()` item lands exactly there regardless of any multi-byte character
    // earlier in `block`. `.skip(open_idx)` on the same iterator skips `open_idx` ITEMS
    // (characters), which only coincides with the byte offset when everything before it
    // is ASCII — fixed round 3, guarded by
    // `condition_args_indexes_by_bytes_not_chars_when_a_multibyte_character_precedes_the_call`.
    for (i, ch) in block.char_indices().skip_while(|(i, _)| *i < open_idx) {
        match ch {
            '(' => {
                depth += 1;
                if depth == 1 {
                    args_start = Some(i + 1);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    if arg_count < keep {
                        kept_end = Some(i);
                    }
                    break;
                }
            }
            ',' if depth == 1 => {
                arg_count += 1;
                if arg_count == keep {
                    kept_end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    match (args_start, kept_end) {
        (Some(s), Some(e)) if e >= s => block[s..e].to_string(),
        // Route 3 of the three named in this function's doc comment, and the ONLY one
        // that reaches this arm: paren-depth tracking never closed. Routes 1 and 2
        // `return` early above, so `_` is not a catch-all for them. Fails SAFE: no
        // searchable text is contributed at all, the same direction an unrecognised
        // macro name already takes at `opens_assert_call`. Returning `block.to_string()`
        // here was the LAX direction fix round 3 removed (defect (c)); it restored the
        // round-1 behaviour of making a message argument searchable whenever this branch
        // was reached. Guarded by
        // `condition_args_returns_empty_when_the_call_never_closes`.
        _ => String::new(),
    }
}

#[test]
fn condition_args_assert_keeps_only_the_first_argument() {
    // M11 bumps `assert!`'s `keep` from 1 to 2, which would pull this message
    // argument into the kept text.
    let block = "assert!(x.contains(\"cap\"), \"unique_message_token\");\n";
    let kept = condition_args(block);
    assert!(kept.contains("x.contains(\"cap\")"));
    assert!(!kept.contains("unique_message_token"));
}

#[test]
fn condition_args_assert_eq_keeps_only_the_first_two_arguments() {
    // M11 bumps `assert_eq!`'s `keep` from 2 to 3, which would pull this message
    // argument into the kept text.
    let block = "assert_eq!(a, b, \"unique_message_token\");\n";
    let kept = condition_args(block);
    assert!(kept.contains('a'));
    assert!(kept.contains('b'));
    assert!(!kept.contains("unique_message_token"));
}

#[test]
fn condition_args_selects_the_macro_token_from_the_first_line_only() {
    // The literal text "assert_eq!" appears only in the MESSAGE, on line 1 — never on
    // the real macro's own line 0 (`assert!`). Round-2 defect (a) searched the WHOLE
    // block for the token, so this message text would be misread as the macro
    // invocation and shift the extracted span into the message.
    let block = "assert!(x.contains(\"cap\"),\n    \"see assert_eq!(a, b) docs\");\n"; // LOAD-BEARING: the decoy `assert_eq!` must sit on line 1 and NOT on line 0. Move it up and this test no longer distinguishes whole-block token search from line-0 token search.
    assert!(
        !block.lines().next().unwrap().contains("assert_eq!") && block.contains("assert_eq!"),
        "fixture must keep the decoy token OFF line 0 and present somewhere later"
    );
    let kept = condition_args(block);
    assert!(kept.contains("x.contains(\"cap\")"));
    assert!(!kept.contains("see assert_eq!"));

    // Second half, added round 4. With POSITIONAL selection the fixture above no longer
    // discriminates the line-0 restriction on its own: the earliest word-boundary token
    // in the whole block is the same `assert!` at offset 0 either way, so widening the
    // search to `let first_line = block;` left the suite green when measured. What the
    // restriction still buys is route 1's fail-safe — a block whose line 0 carries NO
    // recognised token must contribute nothing, even when a later line has one.
    let later_line = "assert_matches!(x, Ok(_) if y ==\n    assert_eq!(a, line_one_marker));\n"; // LOAD-BEARING: line 0's macro must be one ABSENT from ASSERT_MACROS, and line 1's must be present. Put a recognised token on line 0 and both selections agree again.
    assert_eq!(
        condition_args(later_line),
        "",
        "a token on a CONTINUATION line must not be mistaken for the one that opened the span"
    );
}

#[test]
fn condition_args_indexes_by_bytes_not_chars_when_a_multibyte_character_precedes_the_call() {
    // The em dash before `assert!` is 1 CHAR but 3 BYTES. `open_idx` (from `find`) is a
    // BYTE offset; round-2 defect (b) (`.skip(open_idx)` on `char_indices()`) skips
    // `open_idx` CHARACTERS instead, overshooting past the real opening `(` whenever a
    // multi-byte character precedes it on the line.
    let block = "— assert!(x.contains(\"cap\"), \"msg\");\n"; // LOAD-BEARING: the leading character is an EM DASH (U+2014, 3 bytes / 1 char). Swap it for any ASCII character and the byte offset equals the char count, so the byte/char mutation stops being observable here and this test silently ceases to discriminate. Checked below rather than only stated.
    assert!(
        block.chars().next().is_some_and(|c| c.len_utf8() > 1),
        "fixture must open with a MULTI-BYTE character; ASCII makes the byte/char \
         distinction unobservable"
    );
    assert_eq!(condition_args(block), "x.contains(\"cap\")");
}

#[test]
fn condition_args_returns_empty_for_an_unrecognised_macro_rather_than_the_whole_block() {
    // None of the four recognised tokens appears on the first line, so this reaches
    // the first fallback. Round-2 defect (c) returned `block.to_string()` here, making
    // the message text searchable; the fix returns an empty string instead.
    let block = "assert_matches!(x, Ok(_) if x.contains(\"cap\"));\n";
    assert_eq!(condition_args(block), "");
}

#[test]
fn condition_args_string_literal_hazards() {
    // `condition_args` scans `block` as raw bytes, not Rust tokens: the comma INSIDE
    // this string literal ("x, y") is indistinguishable from a real top-level
    // argument separator at paren depth 1, so the scan stops at it — a false RED
    // (narrowing: the kept text is truncated to `a, "x`, losing `, y"` entirely)
    // rather than a false GREEN. A real Rust tokenizer would keep both `a` and the
    // whole `"x, y"` literal.
    let block = "assert_eq!(a, \"x, y\");\n";
    assert_eq!(condition_args(block), "a, \"x");
}

#[test]
fn condition_args_prefers_the_earliest_token_over_a_later_one_in_the_message() {
    // Round-3 defect (item 1 of round 4): token selection was by NAME PREFERENCE
    // (`if first_line.contains("assert_eq!") … else if … contains("assert!")`), so the
    // `assert_eq!` inside this FAILURE MESSAGE won over the real `assert!` at column 0,
    // `find` landed inside the message, and the extracted "condition" came back as
    // `marker_from_message, b` with `x.is_ok()` dropped entirely — a false GREEN, the
    // dangerous direction. `first_assert_token` picks the EARLIEST word-boundary match
    // instead, so `assert!` at column 0 wins.
    let block = "assert!(x.is_ok(), \"see assert_eq!(marker_from_message, b) docs\");\n"; // LOAD-BEARING: the decoy must be a LATER, well-formed `assert_eq!(a, b)` call inside the message. Move it before `assert!`, or malform it, and the superseded name-preference chain would not have mis-selected it, so this test would pass under the defect it exists to catch.
    assert_eq!(condition_args(block), "x.is_ok()");
}

#[test]
fn condition_args_prefers_the_earliest_token_when_a_later_one_hides_in_a_trailing_comment() {
    // The second demonstrated vector for the same defect, and the one round 3's
    // "select from line 0 only" narrowing could NOT close: the decoy is a trailing
    // comment on line 0, so it is on line 0 too. `assertion_lines`' `//` skip does not
    // help either — this line does not START with `//`. Under name preference the token
    // resolved into the comment and the following `(` was then taken from LINE 1, so the
    // extracted "condition" was the message-side call `"marker_from_message"`.
    let block = "assert!(cond, // assert_eq!\n    f(\"marker_from_message\"));\n"; // LOAD-BEARING: the `//` must be MID-LINE (a line-leading `//` is skipped by `assertion_lines` and never reaches here), and line 0's paren must stay open so line 1 belongs to the same block.
    assert_eq!(condition_args(block), "cond");
}

#[test]
fn condition_args_assert_ne_keeps_only_the_first_two_arguments() {
    // `ASSERT_MACROS`' `("assert_ne!", 2)` entry is its own mutable site. Bumping that 2
    // to 3 pulls this message argument into the kept text, and no other test in this file
    // reads that entry — `condition_args_assert_eq_keeps_only_the_first_two_arguments`
    // reads the `assert_eq!` row, which is a different line.
    let block = "assert_ne!(a, b, \"unique_ne_message_token\");\n"; // LOAD-BEARING: exactly THREE arguments. With two, the closing paren supplies `kept_end` before the keep count matters and the mutation goes undetected.
    let kept = condition_args(block);
    assert!(kept.contains('a'));
    assert!(kept.contains('b'));
    assert!(!kept.contains("unique_ne_message_token"));
}

#[test]
fn condition_args_debug_assert_keeps_only_the_first_argument() {
    // `ASSERT_MACROS`' `("debug_assert!", 1)` entry is its own mutable site. Bumping that
    // 1 to 2 pulls this message argument into the kept text. It also pins the
    // word-boundary half of `first_assert_token` from the other side: `debug_assert!`
    // contains `assert!` at offset 6, preceded by `_`, so the shorter token must NOT be
    // the one selected — if it were, `tok.len()` would be 7 instead of 13 and the scan
    // would still land on the same `(`, but the `keep` taken would be `assert!`'s.
    let block = "debug_assert!(x.contains(\"cap\"), \"unique_debug_message_token\");\n"; // LOAD-BEARING: the macro must be `debug_assert!`, not `assert!` — `assert!`'s keep is a different table entry with its own test, and swapping them here leaves the `debug_assert!` row unguarded while this test still passes.
    let kept = condition_args(block);
    assert!(kept.contains("x.contains(\"cap\")"));
    assert!(!kept.contains("unique_debug_message_token"));
}

#[test]
fn condition_args_returns_empty_when_the_token_is_never_invoked() {
    // Fallback route 2 of the three named on `condition_args`' doc comment:
    // `first_assert_token` matched, but no `(` follows the token anywhere in the block,
    // so there is no argument list to narrow. Returning `block.to_string()` here (the
    // pre-round-3 form) would make every word of the block — `route_two_marker` included
    // — searchable by `marker_is_asserted`. Reachable from `assertion_lines`: a line that
    // NAMES a macro without calling it has balanced (zero) parens, so its span closes on
    // that same line and arrives here.
    let block = "assert! route_two_marker;\n"; // LOAD-BEARING: NO `(` anywhere after the token. Add one and the scan proceeds past this route, and the test stops exercising it.
    assert_eq!(condition_args(block), "");
    // Second assertion, and the reason the doc comment can call this route reachable
    // rather than only traced: the same fixture through the production path. It REDs
    // under the `block.to_string()` mutation, which is what shows `assertion_lines`
    // genuinely reaches this route rather than closing no span at all.
    assert!(!assertion_lines(block).contains("route_two_marker"));
}

#[test]
fn condition_args_returns_empty_when_the_call_never_closes() {
    // Fallback route 3, the `_` arm of `condition_args`' final `match` and the ONLY route
    // that reaches it — routes 1 and 2 `return` early. The opening `(` is found, but
    // paren depth never returns to 0 and no top-level comma is seen, so `kept_end` stays
    // `None`. Returning `block.to_string()` here was round 3's defect (c).
    let block = "assert!(route_three_marker\n"; // LOAD-BEARING: NO `)` and NO top-level `,`. Either one gives the scan a `kept_end`, the first `match` arm fires, and the `_` arm goes unexercised.
    assert_eq!(condition_args(block), "");
    // Reaching this route from `assertion_lines` needs a shape whose parens balance over
    // the whole block (so the span closes and `condition_args` runs) while the parens
    // AFTER the macro's own `(` do not (so depth never returns to 0). A stray `)` BEFORE
    // the token is the only construction that does both, and it is contrived — named
    // here rather than left as an unqualified "reachable in practice".
    let via_lines = "    ) assert!(z_route_three_marker\n"; // LOAD-BEARING: the leading `)` is what makes the line balance for `assertion_lines` while leaving the macro's own call open for `condition_args`.
    assert_eq!(condition_args(via_lines), "");
    assert!(!assertion_lines(via_lines).contains("z_route_three_marker"));
}

#[test]
fn condition_args_string_literal_widening_is_a_known_vector() {
    // KNOWN FALSE-GREEN VECTOR. This test asserts today's WRONG output on purpose, so the
    // limitation is recorded as a fixture and not only as prose. The `(` inside the
    // condition's string literal raises depth to 2, so the real argument comma is not
    // counted at depth 1; the `)` inside the MESSAGE drops depth back to 1, and the comma
    // after it terminates the keep — pulling message text into the searched region.
    //
    // Round 3's doc comment claimed this limit produces "a false RED ... never a false
    // GREEN". It does produce a false GREEN, here. Not fixed: a correct answer needs a
    // Rust tokenizer, and widening this parser until the vector went green is what each
    // of the previous three rounds did somewhere else. See `condition_args`' doc comment
    // for what Task 5c must therefore avoid citing.
    //
    // If a future round teaches `condition_args` about string literals, this test REDs.
    // Invert it to `!contains` then — do not delete it.
    let block = "assert!(x == \"(\", \"msg widening_marker_token ) , tail\");\n"; // LOAD-BEARING, three details: the condition's literal holds an UNBALANCED `(`; the message holds a `)`; and a `,` follows that `)`. Drop any one and the scan narrows instead, and this stops recording a widening vector.
    assert!(
        condition_args(block).contains("widening_marker_token"),
        "known vector: message text leaks into the kept condition text; got {:?}",
        condition_args(block)
    );
    assert!(
        assertion_lines(block).contains("widening_marker_token"),
        "and it reaches marker_is_asserted through the production path, not only the helper"
    );
}

/// True when `marker` is asserted inside `body` — the heuristic
/// `probed_rows_cite_a_real_test` runs against a cited test's extracted
/// body. Only searches [`assertion_lines`]`(body)`, not `body` itself, and
/// within that text only the CONDITION/value arguments of each recognised
/// assertion — never a failure-message argument, as of round 3's fix to
/// [`condition_args`] (three routes by which a message argument could still leak
/// through round 2's version are documented, and closed, on [`assertion_lines`]'s
/// doc comment). See that doc comment for the two mutations (weakened condition;
/// deleted assertion with a marker-bearing message left on a neighbour) that
/// motivated narrowing to the condition in the first place, and for the full set
/// of remaining known limits this function does not itself resolve.
///
/// `TextContains(s)` requires the literal `s` to appear anywhere across the
/// condition text: strict on the string (whitespace and all), lax on the
/// location within it — `s` need not be the direct argument to `assert!`
/// itself, so a bare mention in the condition of some OTHER assertion in
/// the same body would also satisfy it (see [`assertion_lines`]'s "Remaining
/// known limits" — cross-assertion laxity).
///
/// `JsonPath(p)` requires EVERY dot-separated segment of `p` after the
/// leading `$.` to appear together on ONE assertion's condition line — not
/// as one contiguous substring, not in path order, but no longer spread
/// across separate assertions or separate lines. Pinned by
/// `marker_is_asserted_false_when_json_path_segments_are_split_across_assertions`;
/// an empty segment (a malformed marker like `$.` or `$.a..b`) never matches, pinned by
/// `marker_is_asserted_false_for_a_json_path_with_no_segments`. This is a change from an
/// earlier version that accepted the segments anywhere across the whole
/// condition text regardless of line: that version was satisfied by two
/// DIFFERENT assert lines that each mentioned only one of "overflow" and
/// "packing", which is not what `JsonPath("$.overflow.packing")` claims to
/// check. A body asserting `v["overflow"]["packing"]` still satisfies
/// `$.overflow.packing` even though the literal text `"$.overflow.packing"`
/// never appears — this remains intentionally lax about the specific
/// bracket-indexing idiom, since neither of the two `Probed` rows this gate
/// checks today would be helped by requiring one.
fn marker_is_asserted(marker: &CitedMarker, body: &str) -> bool {
    let body = assertion_lines(body);
    match marker {
        CitedMarker::TextContains(s) => body.contains(s.as_str()),
        CitedMarker::JsonPath(p) => match p.strip_prefix("$.") {
            Some(rest) => body.lines().any(|line| {
                rest.split('.')
                    .all(|seg| !seg.is_empty() && line.contains(seg))
            }),
            None => false,
        },
    }
}

#[test]
fn marker_is_asserted_true_when_text_contains_marker_is_present() {
    let body = "assert!(msg.contains(\"cap\") && msg.contains('5'));";
    assert!(marker_is_asserted(
        &CitedMarker::TextContains("cap".to_string()),
        body
    ));
}

#[test]
fn marker_is_asserted_false_when_text_contains_marker_is_absent() {
    let body = "assert!(msg.contains(\"unrelated\"));";
    assert!(!marker_is_asserted(
        &CitedMarker::TextContains("cap".to_string()),
        body
    ));
}

#[test]
fn marker_is_asserted_true_when_every_json_path_segment_is_present() {
    let body = "assert_eq!(v[\"overflow\"][\"packing\"], json!(\"excerpted\"));";
    assert!(marker_is_asserted(
        &CitedMarker::JsonPath("$.overflow.packing".to_string()),
        body
    ));
}

#[test]
fn marker_is_asserted_false_when_one_json_path_segment_is_missing() {
    let body = "assert_eq!(v[\"overflow\"][\"omitted\"], json!(0));";
    assert!(!marker_is_asserted(
        &CitedMarker::JsonPath("$.overflow.packing".to_string()),
        body
    ));
}

/// The one-line rule is what `marker_is_asserted`'s doc comment claims for `JsonPath`,
/// and until round 4 nothing checked it: `body.lines().any(...)` reverted to a whole-text
/// `body.contains(seg)` left the suite fully green. The re-review that introduced the
/// tightening measured it against a COMBINED mutation (deleting the `packing`
/// `assert_eq!` at the live row), which is an aggregate — it cannot say whether this site
/// carries its own weight.
#[test]
fn marker_is_asserted_false_when_json_path_segments_are_split_across_assertions() {
    // Two REAL assertions, one segment each, both inside condition arguments — so
    // `assertion_lines` keeps both and only the per-line rule separates them. This is the
    // exact shape that made the live `context.max_tokens` row survive deletion of the
    // assertion it cites, via a neighbouring assert's own text.
    let body = "assert!(v[\"overflow\"].is_object());\nassert!(w[\"packing\"].is_string());\n"; // LOAD-BEARING: the two segments must be on SEPARATE assertion lines. Merge them onto one and this test passes under the whole-text mutation too.
    assert!(!marker_is_asserted(
        &CitedMarker::JsonPath("$.overflow.packing".to_string()),
        body
    ));
}

#[test]
fn marker_is_asserted_false_for_a_json_path_with_no_segments() {
    // `!seg.is_empty()` is its own site. Without it, `line.contains("")` is vacuously
    // true, so a malformed marker like `$.` (or `$.a..b`) would match ANY body containing
    // any assertion at all — a row could be certified by a marker that names nothing.
    let body = "assert_eq!(v[\"overflow\"][\"packing\"], json!(\"excerpted\"));";
    assert!(!marker_is_asserted(
        &CitedMarker::JsonPath("$.".to_string()),
        body
    ));
}

/// Scoping [`marker_is_asserted`] to [`assertion_lines`] is what THIS test proves: before
/// that scoping, a marker mentioned only in a comment (never inside any assertion) still
/// made this function return `true`, which is exactly how deleting the real `assert!`
/// site at either of this gate's two live call sites (`glob_explosion_returns_recoverable`,
/// `a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped`) failed to
/// turn the gate red. The fixture body deliberately avoids the literal substring
/// `assert` outside of what it is testing for, so the line-trigger itself cannot fire.
#[test]
fn marker_is_asserted_false_when_text_contains_marker_only_appears_outside_an_assert_line() {
    let body = "// cap is mentioned only in this comment, never checked\nlet x = 1;";
    assert!(!marker_is_asserted(
        &CitedMarker::TextContains("cap".to_string()),
        body
    ));
}

/// The `JsonPath` sibling of the test above: both segments appear in the body, but only
/// in a comment, never on any line that is part of an assert call.
#[test]
fn marker_is_asserted_false_when_json_path_segment_only_appears_outside_an_assert_line() {
    let body = "// overflow and packing are both mentioned only in this comment, never checked\nlet x = 1;";
    assert!(!marker_is_asserted(
        &CitedMarker::JsonPath("$.overflow.packing".to_string()),
        body
    ));
}

/// What resolving a `cited_test` name against tracked `src/` produced.
///
/// A named state per outcome, not an `Option` plus a count: `CLAUDE.md`
/// § *Testing Discipline* — where a system already names its own failure state,
/// assert on the NAME rather than on a proxy for it. `Ambiguous` is the state
/// this branch added, and the one a test can pin without re-deriving a number.
#[derive(Debug, PartialEq, Eq)]
enum CitedTestResolution {
    /// Exactly one `fn` declaration across tracked `src/`, with its body.
    Unique(String),
    /// No `fn` declaration anywhere in tracked `src/`.
    NotFound,
    /// More than one `fn` declaration. Carries every file that declares the
    /// name, in `git ls-files src` order, a file contributing more than one
    /// declaration suffixed `(xN)`.
    Ambiguous(Vec<String>),
}

/// Resolve a `cited_test` name to ONE body, or refuse.
///
/// The refusal is the point, and it is not hypothetical. Before it, the resolver
/// was `sources.iter().find_map(|src| extract_fn_body(src, &c.cited_test))` —
/// silent first-match over `git ls-files src` order — and the namespace already
/// holds a live collision: `heading_truncation_is_signaled` is declared THREE
/// times (`src/librarian/preview/default.rs:57`, `plan.rs:168`, `spec.rs:76`),
/// each driving a DIFFERENT cap past its bound. Citing it from the
/// `preview.plan_headings` row would have bound to `default.rs:57` — the SIBLING
/// row's test — and the gate would have gone green while certifying one cap
/// using another cap's evidence. No assertion downstream can detect that,
/// exactly as none can check the bound itself.
///
/// This is `IC-6`'s *no disambiguator* half (`CLAUDE.md` § *Parsers Over a
/// Namespace*): an unqualified name over a namespace that admits collisions owes
/// a way to say which one you mean, or a refusal. There is no qualified citation
/// syntax here, so this is the refusal.
///
/// Two research agents hit the collision independently from different rows
/// during the 2026-09-02 census; the round-4 re-review's note that it "checked
/// uniqueness for the two live rows and found them unique" is the shape of a
/// property held by habit rather than by mechanism — true at the time, and
/// nothing would have said so once it stopped being true.
fn resolve_cited_test(sources: &[(String, String)], name: &str) -> CitedTestResolution {
    let mut declarers = vec![];
    let mut total = 0usize;
    let mut first_body = None;
    for (file, src) in sources {
        let n = fn_declaration_lines(src, name).len();
        if n == 0 {
            continue;
        }
        total += n;
        declarers.push(if n == 1 {
            file.clone()
        } else {
            format!("{file} (x{n})")
        });
        if first_body.is_none() {
            first_body = extract_fn_body(src, name);
        }
    }
    if total == 0 {
        return CitedTestResolution::NotFound;
    }
    if total > 1 {
        return CitedTestResolution::Ambiguous(declarers);
    }
    match first_body {
        Some(body) => CitedTestResolution::Unique(body),
        None => CitedTestResolution::NotFound,
    }
}

/// The guard for the tightening [`resolve_cited_test`] adds.
///
/// A tightening CANNOT red an already-green suite — every live citation is
/// unique today, so `probed_rows_cite_a_real_test` is green with the new rule
/// and was green without it. Its existence is therefore no evidence at all. This
/// test drives the REAL helper (not a re-implementation) with input the OLD rule
/// accepts and the NEW rule must reject, which is the only shape that can red.
///
/// Observed RED, 2026-09-02: reverting `resolve_cited_test`'s `total > 1` branch
/// to first-match (`return CitedTestResolution::Unique(...)`) reds THIS test and
/// nothing else in the suite.
#[test]
fn resolve_cited_test_refuses_a_name_declared_in_more_than_one_file() {
    // LOAD-BEARING: both fixture bodies must ASSERT THE MARKER. That is what makes
    // this the silent-green case rather than a mere lookup failure — under
    // first-match the gate finds a body, finds the marker in it, and passes while
    // pointing at the wrong file. Delete the assert from either body and this
    // fixture stops reproducing the defect it exists for.
    let sources = vec![
        (
            "src/librarian/preview/default.rs".to_string(),
            "    fn heading_truncation_is_signaled() {\n        assert!(v[\"headings_truncated\"]);\n    }\n"
                .to_string(),
        ),
        (
            "src/librarian/preview/plan.rs".to_string(),
            "    fn heading_truncation_is_signaled() {\n        assert!(v[\"headings_truncated\"]);\n    }\n"
                .to_string(),
        ),
    ];

    let resolved = resolve_cited_test(&sources, "heading_truncation_is_signaled");
    assert_eq!(
        resolved,
        CitedTestResolution::Ambiguous(vec![
            "src/librarian/preview/default.rs".to_string(),
            "src/librarian/preview/plan.rs".to_string(),
        ]),
        "a cited_test declared in two files must resolve to Ambiguous naming BOTH files, \
         not to the first one silently: {resolved:?}"
    );

    // The other half of the claim, stated as an assertion rather than left to the
    // reader: the old rule really does accept this input. If `find_map` returned
    // `None` here the fixture would be proving nothing about the tightening.
    let old_rule = sources
        .iter()
        .find_map(|(_, src)| extract_fn_body(src, "heading_truncation_is_signaled"));
    assert!(
        old_rule.is_some_and(|body| marker_is_asserted(
            &CitedMarker::JsonPath("$.headings_truncated".to_string()),
            &body
        )),
        "the pre-tightening resolver must ACCEPT this input — find a body and find the \
         marker asserted in it — or this test is not guarding a tightening at all"
    );
}

/// The positive twin. Without it, `resolve_cited_test` could be mutated to return
/// `Ambiguous` unconditionally and the test above would still pass — an assertion
/// monotone in the wrong direction, which is the failure `CLAUDE.md`
/// § *Testing Discipline* names first.
#[test]
fn resolve_cited_test_returns_the_body_when_exactly_one_file_declares_it() {
    let sources = vec![
        (
            "src/librarian/preview/default.rs".to_string(),
            "    fn a_truncated_preview_still_names_its_final_heading() {\n        assert!(v[\"headings_truncated\"]);\n    }\n"
                .to_string(),
        ),
        (
            "src/librarian/preview/plan.rs".to_string(),
            "    fn something_else_entirely() {\n        assert!(true);\n    }\n".to_string(),
        ),
    ];

    match resolve_cited_test(
        &sources,
        "a_truncated_preview_still_names_its_final_heading",
    ) {
        CitedTestResolution::Unique(body) => assert!(
            body.contains("headings_truncated"),
            "the resolved body must be the declaring file's, not an empty or truncated \
             extraction: {body:?}"
        ),
        other => panic!("a uniquely-declared cited_test must resolve to Unique, got {other:?}"),
    }
}

/// A name nothing declares must stay distinguishable from one declared twice —
/// the two failures need different fixes (write the test vs. rename one of
/// three), so collapsing them into one state would send the reader the wrong way.
#[test]
fn resolve_cited_test_reports_not_found_when_no_file_declares_the_name() {
    let sources = vec![(
        "src/librarian/preview/default.rs".to_string(),
        "    fn something_else_entirely() {\n        assert!(true);\n    }\n".to_string(),
    )];

    assert_eq!(
        resolve_cited_test(&sources, "never_declared_anywhere"),
        CitedTestResolution::NotFound,
        "an undeclared name is NotFound, never Ambiguous(vec![]) — the two failures have \
         different remedies and the message must not conflate them"
    );
}

/// Two declarations inside ONE file are still ambiguous. `extract_fn_body` takes
/// the first and says nothing, and a file-granular uniqueness check would agree
/// with it — so counting DECLARATIONS rather than declaring FILES is its own
/// guarded site, not a stylistic choice.
#[test]
fn resolve_cited_test_refuses_two_declarations_inside_one_file() {
    let sources = vec![(
        "src/tools/run_command/tests.rs".to_string(),
        "mod a {\n    fn dup_test() {\n        assert!(x);\n    }\n}\nmod b {\n    fn dup_test() {\n        assert!(x);\n    }\n}\n"
            .to_string(),
    )];

    assert_eq!(
        resolve_cited_test(&sources, "dup_test"),
        CitedTestResolution::Ambiguous(vec!["src/tools/run_command/tests.rs (x2)".to_string()]),
        "two declarations in one file must be reported with their count — a file-granular \
         check would see one declaring file and call it Unique"
    );
}

/// Checks that every `Coverage::Probed` row's `cited_test` names a REAL
/// test — a `fn` that exists somewhere in tracked `src/` — and that the
/// row's marker is asserted somewhere in that test's extracted body. This
/// is the mechanism Task 5b adds: before it, a `cited_test`-shaped claim
/// was a prose comment nobody checked, and two of the original 25 `Probed`
/// rows had already drifted from what their cited test actually asserts —
/// both reclassified to `Deferred` rather than left citing evidence that
/// does not hold up (see `cap_probe.rs`'s row comments for
/// `markdown.headings_hard_cap` and `lsp.first_call_budget`).
///
/// Both extraction steps ([`extract_fn_body`] and [`marker_is_asserted`])
/// are HEURISTICS — read their own doc comments for exactly what each does
/// not catch; this test does not repeat that list, only relies on it. In
/// particular this gate CANNOT detect a citation that names a real test
/// whose body happens to contain the marker text for a reason unrelated to
/// asserting it (a comment, an unrelated string) — narrowing that further
/// is future work, not a claim this gate makes today.
///
/// A `cited_test` naming a `fn` that lives under `tests/` rather than
/// tracked `src/` is unrepresentable by this gate BY DESIGN, not a bug:
/// [`tracked_src_files`] scopes the search to `src/` (mirroring where a
/// `cap-class: RESULT_CAP` annotation itself must live), so a future probe
/// row whose only behavioural test is an integration test needs a citation
/// into `src/`, not a scope widening here — and the failure message below
/// already names that scope.
///
/// Before checking any individual citation, this test also asserts that
/// [`probed_citations`] did not silently drop one: [`count_probed_chunks`]
/// counts the chunks that look like a genuine `Coverage::Probed` row by the
/// same two gates `probed_citations` itself applies, and a mismatch against
/// `citations.len()` means a row's `marker` or `cited_test` field failed to
/// parse and vanished without a trace — the exact failure mode a bare
/// `!citations.is_empty()` cannot see (that check alone is satisfied by a
/// single surviving citation no matter how many others were silently
/// dropped).
///
/// Failure messages name the row id, the cited test, and the marker —
/// never a bare count — so a failure here is a single lookup, not a second
/// investigation.
///
/// A `cited_test` declared more than once in tracked `src/` is REFUSED by
/// [`resolve_cited_test`] rather than resolved to the first match. That is
/// this gate's `IC-6` *no disambiguator* obligation: the namespace holds a
/// live three-way collision (`heading_truncation_is_signaled`), and taking
/// the first match there certifies one cap using a different cap's test
/// while going green. The tightening is guarded by
/// `resolve_cited_test_refuses_a_name_declared_in_more_than_one_file`,
/// because a tightening cannot red an already-green suite and its presence
/// here proves nothing on its own.
#[test]
fn probed_rows_cite_a_real_test() {
    let cap_probe_path = repo_root().join("src/tools/core/cap_probe.rs");
    let cap_probe_src = std::fs::read_to_string(&cap_probe_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", cap_probe_path.display()));
    let citations = probed_citations(&cap_probe_src);

    assert!(
        !citations.is_empty(),
        "probed_citations found zero Coverage::Probed rows in cap_probe.rs — either every row \
         really is Deferred (check by hand before trusting this), or the parser's anchors \
         (\"ProbeRow {{\", \"Coverage::Probed {{\", \"id:\", \"cited_test:\") have drifted from \
         the file's actual formatting"
    );

    let expected_chunk_count = count_probed_chunks(&cap_probe_src);
    assert_eq!(
        citations.len(),
        expected_chunk_count,
        "probed_citations returned {} citations but {} chunks in cap_probe.rs look like a \
         genuine Coverage::Probed row (contain \"Coverage::Probed {{\" and have a bare id: \
         field) — at least one row was silently dropped because its marker or cited_test field \
         could not be parsed; diff the ids in `citations` against \
         `grep -n \"Coverage::Probed {{\" src/tools/core/cap_probe.rs` to find which row",
        citations.len(),
        expected_chunk_count
    );

    let mut sources = vec![];
    for file in tracked_src_files() {
        let path = repo_root().join(&file);
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {file}: {e}"));
        sources.push((file, src));
    }

    let mut failures = vec![];
    for c in &citations {
        let body = match resolve_cited_test(&sources, &c.cited_test) {
            CitedTestResolution::Unique(body) => body,
            CitedTestResolution::NotFound => {
                failures.push(format!(
                    "{}: cited_test \"{}\" names no `fn` found anywhere in tracked src/",
                    c.id, c.cited_test
                ));
                continue;
            }
            CitedTestResolution::Ambiguous(files) => {
                failures.push(format!(
                    "{}: cited_test \"{}\" is declared more than once in tracked src/ ({}) \
                     — an unqualified name cannot say which one this row means, and the \
                     resolver would otherwise take the first in `git ls-files src` order, \
                     certifying this cap with another cap's test. Give one declaration a \
                     distinguishing name and cite that.",
                    c.id,
                    c.cited_test,
                    files.join(", ")
                ));
                continue;
            }
        };
        if !marker_is_asserted(&c.marker, &body) {
            let marker_desc = match &c.marker {
                CitedMarker::JsonPath(p) => format!("JsonPath(\"{p}\")"),
                CitedMarker::TextContains(s) => format!("TextContains(\"{s}\")"),
            };
            failures.push(format!(
                "{}: cited_test \"{}\" exists but its body does not assert marker {}",
                c.id, c.cited_test, marker_desc
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Probed rows whose citation does not hold up:\n{}",
        failures.join("\n")
    );
}
