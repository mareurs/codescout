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
//! `docs/issues/2026-09-09-a-safety-comment-outlived-both-its-unsafe-block-and-its-own-rationale.md`.
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
//! silence it. Instead: the **next line that is neither blank nor a line comment** must
//! contain `unsafe`. Nothing to widen. Measured against HEAD `f10eefe2`: 16 `SAFETY:`
//! comments, 15 satisfy it, and the sole failure is the defect above.

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

/// One `// SAFETY:` comment and the next line of actual code beneath it.
struct Site {
    file: String,
    line: usize,
    next_code: Option<(usize, String)>,
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        // `target` can appear under any crate root; skip build output wherever it is.
        if name == "target" {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every `// SAFETY:` line under [`ROOTS`], paired with the next line that is neither
/// blank nor a line comment.
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
        for (i, raw) in lines.iter().enumerate() {
            if !raw.trim_start().starts_with("// SAFETY:") {
                continue;
            }
            let next_code = lines[i + 1..].iter().enumerate().find_map(|(off, l)| {
                let t = l.trim_start();
                if t.is_empty() || t.starts_with("//") {
                    None
                } else {
                    Some((i + 2 + off, (*l).to_string()))
                }
            });
            sites.push(Site {
                file: rel.clone(),
                line: i + 1,
                next_code,
            });
        }
    }
    sites
}

#[test]
fn every_safety_comment_precedes_an_unsafe_construct() {
    let orphans: Vec<String> = safety_sites()
        .into_iter()
        .filter(|s| {
            !s.next_code
                .as_ref()
                .is_some_and(|(_, code)| code.contains("unsafe"))
        })
        .map(|s| {
            let found = match &s.next_code {
                Some((n, code)) => format!("{n}: {}", code.trim()),
                None => "<end of file>".to_string(),
            };
            format!("  {}:{} -> next code line is {found}", s.file, s.line)
        })
        .collect();

    assert!(
        orphans.is_empty(),
        "`// SAFETY:` comment with no `unsafe` beneath it:\n{}\n\n\
         A `SAFETY:` block justifies one specific `unsafe` construct. Orphaned, it \
         outlives what it described and keeps vouching for reasoning the code may since \
         have rejected — which is exactly how `src/lsp/client.rs` came to teach a cast \
         that `platform::unix::addressable_pid` was written to refuse.\n\n\
         Two fixes, both performable by whoever reads this:\n\
         (1) the `unsafe` moved — move or delete the comment with it; the new home owns \
         the argument now;\n\
         (2) the note is about safe code — drop the `SAFETY:` prefix and write it as an \
         ordinary comment. There is deliberately no marker to exempt a line: `SAFETY:` \
         means one thing in Rust, and an escape hatch here would re-admit the defect \
         under a new spelling.",
        orphans.join("\n")
    );
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
/// Named differently from the precedent on purpose: `tests/feature_lanes.rs` already owns
/// `the_guard_is_not_vacuous`, and the gate ritual in `CLAUDE.md` is "read YOUR OWN test
/// names out of the default lane". Two identical names in one lane's output cannot answer
/// that question — you have to resolve them by line adjacency, which is not a
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

    // ...and the resolver must actually resolve. If `next_code` were always `None` the
    // guard above would red on everything, but a subtler break — a trim that eats every
    // line, say — could make every site look like an `unsafe` line and pass.
    assert!(
        sites.iter().all(|s| s.next_code.is_some()),
        "every `SAFETY:` comment should be followed by some code line; a `None` here \
         means the line resolver is broken, not that the tree is"
    );
}
