//! Every page under `docs/manual/src/` must be reachable from `SUMMARY.md`.
//!
//! **Why a separate gate from `doc_tool_refs.rs`.** That file walks `docs/manual/**` and checks
//! what the pages *say*. This one checks whether anyone can *reach* them, and the two populations
//! are not the same set. mdBook renders exactly what `SUMMARY.md` lists, so an unlisted page is
//! never built into the book — it exists on disk, is scanned by every content gate, and is read by
//! nobody.
//!
//! That combination is what this exists to prevent, and it is not hypothetical. Measured
//! 2026-09-06: `concepts/librarian-mcp.md` had been orphaned since the librarian tool collapse. It
//! documented librarian as a separate MCP server with its own binary, carried 18 retired tool
//! names, and told the reader to run a build against a crate that is an empty directory and not a
//! workspace member. Every content gate over `docs/manual/` was green the whole time, correctly:
//! those names sit in table cells and prose, never in the anchored call form
//! `doc_tool_refs.rs` matches. Two independent misses — outside the human review path, inside an
//! automated one that could not see it — and either alone would have been recoverable.
//!
//! **The generalisable part:** absence from the TOC is a *structural* property, so no check over
//! file *contents* can observe it however carefully written. A doc gate that walks a directory is
//! not a gate over the documentation a reader can reach.
//!
//! **No allowlist, deliberately.** An entry here would be how the known orphan became permanent —
//! the page would stay unreachable and the gate would report health. A page that should not be in
//! the book should not be in `docs/manual/src/`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manual_src() -> PathBuf {
    repo_root().join("docs/manual/src")
}

/// Every `.md` under `docs/manual/src/`, as a path relative to that directory.
///
/// `SUMMARY.md` is excluded: it is the index itself and cannot list itself.
fn manual_pages() -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                if let Ok(rel) = p.strip_prefix(root) {
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    if rel != "SUMMARY.md" {
                        out.push(rel);
                    }
                }
            }
        }
    }
    let root = manual_src();
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out.sort();
    out
}

/// Link targets `SUMMARY.md` actually points at, parsed rather than substring-matched.
///
/// Substring matching would let `(concepts/foo.md)` be "found" inside a longer unrelated path and
/// report a page reachable when it is not — a false green, which is the one direction this gate
/// must not fail in. Anchors and query suffixes are trimmed so `foo.md#section` still counts.
fn summary_targets() -> HashSet<String> {
    let text = std::fs::read_to_string(manual_src().join("SUMMARY.md"))
        .expect("docs/manual/src/SUMMARY.md must exist");
    let re = regex::Regex::new(r"\]\(([^)]+)\)").unwrap();
    re.captures_iter(&text)
        .map(|c| c.get(1).unwrap().as_str())
        .map(|t| t.split(['#', '?']).next().unwrap_or(t).trim().to_string())
        .filter(|t| t.ends_with(".md"))
        .collect()
}

#[test]
fn every_manual_page_is_reachable_from_summary() {
    let targets = summary_targets();
    let orphans: Vec<String> = manual_pages()
        .into_iter()
        .filter(|p| !targets.contains(p))
        .collect();

    assert!(
        orphans.is_empty(),
        "{} manual page(s) exist on disk but are not linked from SUMMARY.md, so mdBook never \
         renders them and no reader can reach them:\n  {}\n\nAdd the page to SUMMARY.md, or delete \
         it. Do not add an allowlist to this test: an unreachable page that a gate reports as \
         healthy is the state this gate exists to prevent.",
        orphans.len(),
        orphans.join("\n  ")
    );
}

/// Non-vacuity guard: the two sets above must both be non-empty and must overlap.
///
/// Without it, a broken walk or a `SUMMARY.md` this test failed to parse yields **zero orphans**
/// and a green tick — the sibling of `doc_tool_refs.rs`'s `the_scan_is_not_reading_an_empty_corpus`
/// and for the same reason: an empty numerator and an empty denominator agree perfectly.
#[test]
fn the_toc_scan_is_reading_both_sides() {
    let pages = manual_pages();
    let targets = summary_targets();

    assert!(
        pages.len() > 50,
        "expected the manual to hold many pages; found {} — the directory walk is broken \
         or the manual moved",
        pages.len()
    );
    assert!(
        targets.len() > 50,
        "expected SUMMARY.md to list many pages; found {} — the link parser is broken \
         or SUMMARY.md moved",
        targets.len()
    );
    let reachable = pages.iter().filter(|p| targets.contains(*p)).count();
    assert!(
        reachable > 50,
        "only {reachable} page(s) matched a SUMMARY.md target — the two sides are being \
         compared in different path shapes, so the orphan test above proves nothing"
    );
}
