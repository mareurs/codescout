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
//! **105 cap-shaped `const` declarations** in tracked `src/` (unit: `const`
//! declarations matching [`is_cap_shaped`], one count per declaration, not
//! per use site — a constant read at six call sites still counts once):
//! **66** `RESULT_CAP` across 66 distinct ids, **39** `NOT_A_CAP`. Derived by
//! running [`every_cap_constant_is_classified`]'s own parser over
//! `git ls-files src`, not by a shell grep — a second selector answers a
//! slightly different question, which is the `IC-18` mistake this gate
//! exists to catch.
//!
//! The census is a floor on the cap population, not a census of caps:
//! [`is_cap_shaped`] cannot see a bound named `PAGE_SIZE`, and instrument B
//! ([`truncation_sites`]) is what covers the operations it misses.

// Consumed by Task 5's probe_row_ids; allow until then.
#[allow(unused_imports)]
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

/// Cap-shaped `const` declarations in `src`, each with the `cap-class:`
/// annotation from the contiguous comment block directly above it.
///
/// Takes `&str` rather than reading the file so the meta-tests above drive
/// THIS function on fixtures — not a second copy that could drift from it
/// (`missing_index_rows` precedent, `tests/issue_clusters.rs:461-471`).
fn cap_constants(src: &str, file: &str) -> Vec<CapDecl> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = vec![];

    for (idx, raw) in lines.iter().enumerate() {
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
