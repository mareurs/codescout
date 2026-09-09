//! A `// SAFETY:` comment must sit in front of the `unsafe` it justifies.
//!
//! Why this exists. `// SAFETY:` is a *justification for a specific construct*, not a
//! general note, and the thing it justifies can move out from under it. On 2026-03-08
//! `3d462282` wrote a five-line `SAFETY:` block for
//! `unsafe { libc::kill(pid as i32, libc::SIGTERM) }` in `Drop for LspClient`.
//! `bedeb7c0` then replaced that call with the safe `platform::terminate_process` and
//! left the comment behind, so it annotated a safe call. Three days before this file
//! was written, `01b185d6` decided the `u32 as i32` cast was the bug — it can turn a
//! single-process SIGTERM into a group-wide one — and replaced it with
//! `addressable_pid`, which **refuses** rather than casts. That fix swept
//! `src/platform/unix.rs` and `src/tools/rendezvous.rs` and never touched the comment,
//! which went on teaching the rejected rationale verbatim, three lines above the call.
//! Full account:
//! `docs/issues/archive/2026-09-09-a-safety-comment-outlived-both-its-unsafe-block-and-its-own-rationale.md`.
//!
//! Nothing could catch it. The compiler does not read comments; clippy's
//! `undocumented_unsafe_blocks` is not enabled here and runs the *opposite* direction
//! anyway (it finds `unsafe` without a comment, never a comment without `unsafe`); and
//! the party best placed to notice is the author of a refactor that moves an `unsafe`
//! elsewhere — for whom a comment above the call site reads as belonging to the call
//! site. Care is the wrong instrument; this scan is the right one.
//!
//! **The rule is structural, not a window.** An earlier draft asked whether `unsafe`
//! appeared within N lines. That is an existence assertion, and existence assertions
//! are monotone under *widening* (`CLAUDE.md` § *Testing Discipline*) — every increase
//! of N makes the guard pass more easily, so the guard's own tuning knob is a way to
//! silence it. Instead the span is the **statement itself**: start at the next line
//! that is neither blank nor a line comment, and run to where its delimiters balance.
//! `unsafe` must appear inside that span. There is no constant to raise.
//!
//! **That span rule replaced a first-line-only rule, and the correction is the
//! load-bearing history here — do not "simplify" it back.** The shipped v1 asked only
//! whether the *first* code line contained `unsafe`, and it red on
//!
//! ```ignore
//! // SAFETY: live fd owned by `holder`, unlocked below before it drops.
//! assert_eq!(
//!     unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
//!     0
//! );
//! ```
//!
//! — a correct comment over a real `unsafe`, wrapped in a multi-line macro call, where
//! the first code line is `assert_eq!(`. v1 measured **16** comments and reported 15
//! pass / 1 fail / zero false positives; that corpus contained no macro-wrapped
//! `unsafe`, so **it could not falsify the rule** (§ *Testing Discipline*: a population
//! selected so no member can refute). The first file outside it produced two false
//! positives within the hour — and v1's failure text told the reader to *drop the
//! `SAFETY:` prefix*, which would have deleted a correct justification for a live
//! `libc::flock`. The predicate and the remedy were wrong in the same direction.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Roots to scan, relative to the repo.
///
/// Deliberately an explicit list rather than a walk from the repo root: this checkout
/// carries `.worktrees/` with full checkouts of other branches, and `target/` holds
/// vendored sources. A walk from the root would scan another branch's copy of this very
/// file and report findings the working tree cannot fix.
const ROOTS: [&str; 3] = ["src", "crates", "tests"];

/// One `// SAFETY:` comment and the statement it sits in front of.
struct Site {
    file: String,
    line: usize,
    /// 1-indexed line where the justified statement starts, and its full text.
    stmt: Option<(usize, String)>,
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `target` can appear under any crate root; skip build output wherever it is.
        if entry.file_name() == "target" {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A line with its `//` comment tail and its string-literal contents removed, so that
/// delimiter counting is not fooled by punctuation that is data rather than syntax.
///
/// Char literals are deliberately NOT special-cased: in Rust a bare `'` is far more
/// often a lifetime (`&'a str`) than the start of a literal, and a scanner that guessed
/// would mis-handle the common case to fix the rare one. **The residual failure runs
/// toward a false NEGATIVE** — a `'{'` miscounts depth upward, which extends the span
/// and makes `unsafe` *easier* to find, so the guard under-reports rather than
/// accusing correct code. That direction is the acceptable one for a gate that reds a
/// shared build; v1 failed the other way and cost a peer an interruption.
fn code_only(line: &str) -> String {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i] as char;
        if in_str {
            if c == '\\' {
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '/' && i + 1 < b.len() && b[i + 1] == b'/' {
            break;
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

fn depth_delta(code: &str) -> i32 {
    code.chars().fold(0, |d, c| match c {
        '(' | '[' | '{' => d + 1,
        ')' | ']' | '}' => d - 1,
        _ => d,
    })
}

/// The statement a `SAFETY:` comment at `from` (0-indexed) sits in front of: from the
/// next code line, to the line where delimiters balance.
fn justified_statement(lines: &[&str], from: usize) -> Option<(usize, String)> {
    let start = from
        + 1
        + lines[from + 1..].iter().position(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with("//")
        })?;

    let mut depth = 0;
    let mut buf = String::new();
    for line in &lines[start..] {
        depth += depth_delta(&code_only(line));
        buf.push_str(line);
        buf.push('\n');
        if depth <= 0 {
            break;
        }
    }
    Some((start + 1, buf))
}

/// Every `// SAFETY:` line under [`ROOTS`], paired with the statement beneath it.
///
/// The comment must *start* the line (after indentation) to count. A `SAFETY:` inside a
/// string literal or trailing another expression is not the convention this guards and
/// matching it would red on prose.
fn safety_sites() -> Vec<Site> {
    let root = repo_root();
    let mut files = Vec::new();
    for r in ROOTS {
        rust_files(&root.join(r), &mut files);
    }
    files.sort();

    let mut sites = Vec::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let lines: Vec<&str> = text.lines().collect();
        for i in 0..lines.len() {
            if !lines[i].trim_start().starts_with("// SAFETY:") {
                continue;
            }
            sites.push(Site {
                file: rel.clone(),
                line: i + 1,
                stmt: justified_statement(&lines, i),
            });
        }
    }
    sites
}

#[test]
fn every_safety_comment_precedes_an_unsafe_construct() {
    let orphans: Vec<String> = safety_sites()
        .into_iter()
        .filter(|s| !s.stmt.as_ref().is_some_and(|(_, t)| t.contains("unsafe")))
        .map(|s| {
            let found = match &s.stmt {
                Some((n, t)) => format!(
                    "statement at {n} has no `unsafe`: {}",
                    t.lines().next().unwrap_or("").trim()
                ),
                None => "<end of file>".to_string(),
            };
            format!("  {}:{} -> {found}", s.file, s.line)
        })
        .collect();

    assert!(
        orphans.is_empty(),
        "`// SAFETY:` comment with no `unsafe` in the statement beneath it:\n{}\n\n\
         A `SAFETY:` block justifies one specific `unsafe` construct. Orphaned, it \
         outlives what it described and keeps vouching for reasoning the code may since \
         have rejected — which is exactly how `src/lsp/client.rs` came to teach a cast \
         that `platform::unix::addressable_pid` was written to refuse.\n\n\
         Check FIRST that this is not a miss by this scan: the span runs from the next \
         code line to where its delimiters balance, so an `unsafe` in a following, \
         separate statement is correctly not counted — but if you can see an `unsafe` \
         that belongs to this comment and the scan did not, that is a bug in \
         tests/safety_comments.rs and NOT in your code. Say so rather than editing the \
         comment; v1 of this gate accused two correct comments and told their author to \
         delete them.\n\n\
         If it is a real orphan, two fixes, both performable by whoever reads this:\n\
         (1) the `unsafe` moved — move or delete the comment with it; the new home owns \
         the argument now;\n\
         (2) the note is about safe code — drop the `SAFETY:` prefix and write it as an \
         ordinary comment. There is deliberately no marker to exempt a line: `SAFETY:` \
         means one thing in Rust, and an escape hatch here would re-admit the defect \
         under a new spelling.",
        orphans.join("\n")
    );
}

/// The span rule, on inputs chosen so each one can fail.
///
/// This calls the real `justified_statement` rather than re-deriving it — a second level
/// asserting about its own re-implementation is indistinguishable from coverage
/// (§ *Testing Discipline*), and here that function IS the whole production path.
///
/// Case 4 is the one that keeps the rule honest. Following the statement to delimiter
/// balance is a widening of v1, and a widening needs a case that still fails, or the fix
/// for a false positive silently becomes a rule that accepts everything.
#[test]
fn the_span_stops_at_the_statement_it_justifies() {
    fn probe(src: &str) -> bool {
        let lines: Vec<&str> = src.lines().collect();
        let i = lines
            .iter()
            .position(|l| l.trim_start().starts_with("// SAFETY:"))
            .expect("fixture must contain a SAFETY comment");
        justified_statement(&lines, i).is_some_and(|(_, t)| t.contains("unsafe"))
    }

    // 1. directly in front of the `unsafe` — the ordinary case
    assert!(probe("// SAFETY: x\nlet r = unsafe { f() };\n"));

    // 2. wrapped in a multi-line macro call — the false positive v1 shipped
    assert!(probe(
        "// SAFETY: x\nassert_eq!(\n    unsafe { libc::flock(fd, LOCK_EX) },\n    0\n);\n"
    ));

    // 3. a genuine orphan — the defect this gate exists for
    assert!(!probe(
        "// SAFETY: x\nlet _ = crate::platform::terminate_process(*pid);\n"
    ));

    // 4. `unsafe` in a LATER, separate statement must NOT count. Delete the balance
    //    check and this is the case that reds; without it the rule would swallow the
    //    rest of the file and pass on anything.
    assert!(!probe(
        "// SAFETY: x\nlet a = plain(1);\nlet b = unsafe { f() };\n"
    ));

    // 5. a delimiter inside a string literal must not extend the span past its
    //    statement — otherwise `code_only` is dead and case 4 stops discriminating.
    assert!(!probe(
        "// SAFETY: x\nlet a = log(\"(((\");\nlet b = unsafe { f() };\n"
    ));
}

/// The guard must not pass by finding nothing.
///
/// Three separate ways this scan could silently return an empty set — a broken root
/// list, a walk that never descends, a match string that stopped matching — and every
/// one of them produces the same green tick as a clean tree. Following
/// `tests/feature_lanes.rs` § `the_guard_is_not_vacuous`, assert the inputs are
/// populated *and* that the scan reaches the specific file where this defect class
/// lives, so a root list that silently stops covering `src/lsp/` is a failure rather
/// than a pass.
///
/// Named differently from that precedent on purpose: it already owns
/// `the_guard_is_not_vacuous`, and the gate ritual in `CLAUDE.md` is "read YOUR OWN test
/// names out of the default lane". Two identical names in one lane's output cannot
/// answer that — you have to resolve them by line adjacency, which is not a
/// disambiguator.
#[test]
fn the_safety_scan_is_not_vacuous() {
    let sites = safety_sites();

    assert!(
        sites.len() >= 10,
        "expected a populated set of `// SAFETY:` comments, found {} — the scan has \
         stopped reaching source, and an empty set passes the guard above silently",
        sites.len()
    );

    let files: BTreeSet<&str> = sites.iter().map(|s| s.file.as_str()).collect();
    assert!(
        files.contains("src/platform/unix.rs"),
        "expected the scan to reach src/platform/unix.rs, which holds the `unsafe \
         libc::kill` this class was found on; scan covered {files:?}"
    );

    // ...and the resolver must actually resolve. If `stmt` were always `None` the guard
    // above would red on everything, but a subtler break — a trim that eats every line,
    // say — could make every site look like an `unsafe` line and pass.
    assert!(
        sites.iter().all(|s| s.stmt.is_some()),
        "every `SAFETY:` comment should be followed by some statement; a `None` here \
         means the span resolver is broken, not that the tree is"
    );
}
