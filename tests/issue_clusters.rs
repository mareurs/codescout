//! Gate: every tracked open bug file declares exactly one known defect class.
//!
//! `docs/issues/` answers what is broken; the `IC-N` ledger
//! (`docs/trackers/issue-clusters.md`) answers what a set of bugs has in common. That
//! second question is only answerable if every bug carries a `cluster/<slug>` tag, and a
//! convention that depends on being *read* is the failure that retired this repo's
//! hand-maintained bug index in 2026-05-18 and rotted the `BL-N` queue's sequencing notes.
//! This is the check that runs when nobody is worried.
//!
//! Scope is deliberately narrow, and each bound was measured on 2026-08-31:
//!
//! - **Tracked files only.** An untracked file is a peer session's in-flight work on a
//!   shared checkout. Gating on the working tree would let one session's unfinished bug
//!   file red another session's build, which is the very class `IC-1` describes.
//! - **`docs/issues/*.md` only, never `archive/`.** 416 archived files legitimately carry
//!   no tag: the nine classes were derived from the open backlog, and 279 archived files
//!   in the backfilled window match none of them. Forcing a fit would corrupt the counts
//!   that promotion reads, so absence there is a deliberate answer rather than a gap.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const LEDGER: &str = "docs/trackers/issue-clusters.md";

/// What is wrong with one bug file's class declaration, if anything.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Ok(String),
    /// No `cluster/` tag in frontmatter.
    Missing,
    /// More than one. Counts stop being additive, and the counts drive promotion.
    Multiple(Vec<String>),
    /// A slug the ledger does not define — a typo reads as a real class of size one.
    Unknown(String),
}

/// The closed set of slugs, read from the ledger's `**Slug:**` declarations.
///
/// The ledger is the single source of truth: renaming a class there is what makes the
/// gate accept the new name, so the two can never drift.
fn valid_slugs() -> BTreeSet<String> {
    let path = repo_root().join(LEDGER);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter_map(|l| l.strip_prefix("**Slug:**"))
        .filter_map(|rest| {
            let start = rest.find('`')? + 1;
            let end = rest[start..].find('`')? + start;
            rest[start..end].strip_prefix("cluster/").map(str::to_owned)
        })
        // The ledger's own template declares `cluster/<slug>`; a placeholder is not a class.
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        .collect()
}

/// The YAML frontmatter block of a markdown file, without its `---` fences.
///
/// Returns `None` when the file has no frontmatter at all, which for a bug file is
/// itself a defect — reported as `Missing` rather than skipped.
fn frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

/// Every `cluster/` tag declared in a frontmatter block.
///
/// Reads both YAML forms, because the corpus uses both: 20 files write a block sequence
/// under `tags:` and 11 write an inline flow list. Reading only one silently under-reports.
fn cluster_tags(fm: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = false;
    for line in fm.lines() {
        if let Some(rest) = line.strip_prefix("tags:") {
            let rest = rest.trim();
            if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                out.extend(
                    inner
                        .split(',')
                        .map(|t| t.trim().trim_matches(['"', '\'']).to_owned()),
                );
            } else {
                in_block = rest.is_empty();
            }
            continue;
        }
        if in_block {
            match line
                .strip_prefix('-')
                .or_else(|| line.trim_start().strip_prefix("- "))
            {
                Some(item) => out.push(item.trim().trim_matches(['"', '\'']).to_owned()),
                // A non-item line ends the block; a later key is not a tag.
                None => in_block = false,
            }
        }
    }
    out.into_iter()
        .filter_map(|t| t.strip_prefix("cluster/").map(str::to_owned))
        .collect()
}

/// Judge one bug file's frontmatter against the ledger's closed set.
fn verdict(content: &str, valid: &BTreeSet<String>) -> Verdict {
    let tags = frontmatter(content).map(cluster_tags).unwrap_or_default();
    match tags.len() {
        0 => Verdict::Missing,
        1 => {
            let slug = tags.into_iter().next().expect("len checked");
            if valid.contains(&slug) {
                Verdict::Ok(slug)
            } else {
                Verdict::Unknown(slug)
            }
        }
        _ => Verdict::Multiple(tags),
    }
}

/// Tracked bug files directly under `docs/issues/` — never `archive/`, never untracked.
///
/// Depth is filtered here rather than in the pathspec: `git ls-files 'docs/issues/*.md'`
/// matches across `/` and returns the whole archive (529 paths, measured 2026-08-31).
fn tracked_open_bug_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "docs/issues"])
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
        .filter_map(|p| p.strip_prefix("docs/issues/"))
        .filter(|rest| !rest.contains('/') && rest.ends_with(".md") && *rest != "_TEMPLATE.md")
        .map(|rest| format!("docs/issues/{rest}"))
        .collect()
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn every_open_bug_file_declares_one_known_defect_class() {
    let valid = valid_slugs();
    let mut found = Vec::new();
    for rel in tracked_open_bug_files() {
        let path = repo_root().join(&rel);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        match verdict(&content, &valid) {
            Verdict::Ok(_) => {}
            Verdict::Missing => found.push(format!("{rel} — no cluster/ tag")),
            Verdict::Multiple(t) => found.push(format!("{rel} — {} cluster/ tags: {t:?}", t.len())),
            Verdict::Unknown(s) => found.push(format!("{rel} — unknown slug: cluster/{s}")),
        }
    }
    assert!(
        found.is_empty(),
        "open bug files with a bad defect-class declaration:\n  {}\n\n\
         Every bug carries exactly one `cluster/<slug>` tag from the closed set in {LEDGER}. \
         Write it THROUGH THE CATALOG — `artifact(action=\"update\", id=…, patch={{tags:[…]}})` \
         or `codescout artifact update <id> --tags …` — because a direct frontmatter edit does \
         not reach the catalog (BL-48), leaving the tag on disk and invisible to every `find`. \
         If no existing class fits, add one to the ledger rather than forcing a fit: a wrong \
         class corrupts the counts that promotion reads.",
        found.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Guards on the gate itself — a check that cannot fail is not a check
// ---------------------------------------------------------------------------

/// The scan must be able to return each verdict, or the gate is decoration.
#[test]
fn the_cluster_scan_discriminates() {
    let valid: BTreeSet<String> = ["alpha", "beta"].iter().map(|s| s.to_string()).collect();
    let doc = |tags: &str| format!("---\nkind: bug\n{tags}\n---\n\n# t\n");

    let one = doc("tags:\n- cluster/alpha\n- unrelated");
    assert_eq!(verdict(&one, &valid), Verdict::Ok("alpha".into()));

    let none = doc("tags:\n- unrelated");
    assert_eq!(verdict(&none, &valid), Verdict::Missing);

    let two = doc("tags:\n- cluster/alpha\n- cluster/beta");
    assert_eq!(
        verdict(&two, &valid),
        Verdict::Multiple(vec!["alpha".into(), "beta".into()])
    );

    let bogus = doc("tags:\n- cluster/gamma");
    assert_eq!(verdict(&bogus, &valid), Verdict::Unknown("gamma".into()));

    // No frontmatter at all is Missing, not a silent skip.
    assert_eq!(verdict("# just a body\n", &valid), Verdict::Missing);
}

/// Both YAML tag styles must be read: the corpus uses both, 20 block and 11 inline.
///
/// Reading only the block form would report 11 files as untagged; reading only the
/// inline form would report 20. Either way the gate fires on files that are fine.
#[test]
fn both_yaml_tag_styles_are_read() {
    let valid: BTreeSet<String> = ["alpha"].iter().map(|s| s.to_string()).collect();

    let block = "---\nkind: bug\ntags:\n- cluster/alpha\n- x\n---\n\n# t\n";
    let inline = "---\nkind: bug\ntags: [cluster/alpha, x]\n---\n\n# t\n";
    assert_eq!(verdict(block, &valid), Verdict::Ok("alpha".into()));
    assert_eq!(verdict(inline, &valid), Verdict::Ok("alpha".into()));
}

/// A slug written in PROSE is not a declaration.
///
/// Measured 2026-08-31: a `grep -rho "cluster/[a-z-]*" docs/issues/` over this corpus
/// returned two slugs that do not exist — `cluster/blast` and a bare `cluster/` — picked
/// up from a tool log and from a bug file discussing the convention. A grep-based gate
/// would have reported both as real classes.
#[test]
fn a_cluster_slug_in_prose_is_not_a_declaration() {
    let valid: BTreeSet<String> = ["alpha"].iter().map(|s| s.to_string()).collect();
    let doc =
        "---\nkind: bug\ntags:\n- unrelated\n---\n\n# t\n\nSee cluster/alpha for the class.\n";
    assert_eq!(verdict(doc, &valid), Verdict::Missing);
}

/// The ledger's own template declares `cluster/<slug>`; a placeholder is not a class.
///
/// Without the shape filter the valid set gains `<slug>`, and a file tagged with the
/// literal placeholder would pass the gate.
#[test]
fn the_slug_set_excludes_the_template_placeholder() {
    let slugs = valid_slugs();
    assert!(
        !slugs.contains("<slug>"),
        "the template placeholder leaked into the valid slug set"
    );
    assert!(
        slugs.contains("blast-radius-exceeds-visibility"),
        "expected a known class in the slug set; got {slugs:?}"
    );
    assert!(
        slugs.len() >= 5,
        "only {} slugs parsed from {LEDGER} — the declaration format changed and the \
         unknown-slug check is now vacuous",
        slugs.len()
    );
}

/// The scan must actually reach files.
///
/// A wrong `CARGO_MANIFEST_DIR` join, a git failure, or a too-strict path filter would
/// make the gate pass over an empty list — indistinguishable from a clean corpus.
#[test]
fn the_scan_actually_reads_files() {
    let files = tracked_open_bug_files();
    assert!(
        files.len() > 10,
        "expected docs/issues/ to hold many tracked bug files; found {} — the scan is \
         looking in the wrong place",
        files.len()
    );
    assert!(
        files.iter().all(|f| !f.contains("/archive/")),
        "archive files leaked into the open-corpus scan; 416 of them legitimately carry \
         no tag and would red this gate"
    );
    assert!(
        files.iter().all(|f| !f.ends_with("_TEMPLATE.md")),
        "the bug template is not a bug and must not be gated"
    );
}
