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
    /// `NOT_A_CAP` with an empty reason. Distinguished from `Unclassified`
    /// because "the annotation exists" is not the property we want, and the
    /// two need different failure text.
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
            continue; // attributes sit between the doc block and the item
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

/// Split an annotation payload into its class and the text after the dash.
fn split_reason(payload: &str) -> Option<&str> {
    for sep in ["—", "–", "-"] {
        if let Some((_, rest)) = payload.split_once(sep) {
            return Some(rest.trim());
        }
    }
    None
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
            if id.is_empty() {
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
