//! Gates on the `IC-N` defect-class ledger: every tracked open bug file declares exactly one
//! known class, and every count the ledger publishes matches the corpus it summarises.
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
//!
//! The **count** gate below reads a deliberately WIDER population — open *and* archive — because
//! a class's `n` is its whole membership, and promotion reads that number. The two scopes must
//! not be merged: [`tracked_open_bug_files`] for tag validity, [`tracked_all_bug_files`] for
//! counts. Merging them would either red the gate on 416 legitimately untagged archive files, or
//! silently under-count every class by its archived members.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
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

/// The `cluster/<slug>` inside the first backtick pair of a `**Slug:**` line's tail.
///
/// Shared by [`valid_slugs`] and [`parse_member_counts`] so the two cannot disagree about what
/// a slug declaration looks like — the second reads the same lines to key its counts, and a
/// private copy here would be the exact drift the hook-agreement tests exist to prevent one
/// layer up.
fn backticked_cluster_slug(rest: &str) -> Option<String> {
    let start = rest.find('`')? + 1;
    let end = rest[start..].find('`')? + start;
    rest[start..end].strip_prefix("cluster/").map(str::to_owned)
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
        .filter_map(backticked_cluster_slug)
        // The ledger's own template declares `cluster/<slug>`; a placeholder is not a class.
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        .collect()
}

/// [`valid_slugs`] over caller-supplied text, for fixture-driven tests.
fn valid_slugs_from(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("**Slug:**"))
        .filter_map(backticked_cluster_slug)
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

// ---------------------------------------------------------------------------
// The Index table's `n` column, checked against the corpus it summarises.
// ---------------------------------------------------------------------------

/// Every tracked bug file — open corpus **and** archive.
///
/// Deliberately wider than [`tracked_open_bug_files`]; see the module header for why the two
/// populations must stay separate.
fn tracked_all_bug_files() -> Vec<String> {
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
        .filter(|p| p.ends_with(".md") && !p.ends_with("_TEMPLATE.md"))
        .map(str::to_owned)
        .collect()
}

/// `slug -> n`, parsed from the ledger's Index table.
///
/// A row is ``| IC-N | <class> | `<slug>` | <n> | <promotes to> | <mechanism> |``. The count is
/// read from the cell **immediately after** the slug cell, never "the first number in the row":
/// the mechanism column carries digits of its own (`**10 share one layer**`), so a
/// scan-for-a-number parser would return a plausible wrong value rather than failing.
///
/// Pure over `text` so [`the_index_row_parser_discriminates`] can feed it a table whose right
/// answers are known — a parser that only ever runs against the live file cannot be shown to
/// read the right cell.
fn parse_index_counts(text: &str, valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if !line.starts_with("| IC-") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        for (i, cell) in cells.iter().enumerate() {
            let Some(inner) = cell.strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
                continue;
            };
            if !valid.contains(inner) {
                continue;
            }
            // A row whose count cell does not parse is left ABSENT rather than defaulted to 0:
            // `every_declared_class_has_an_index_row` then reports it, where a 0 would silently
            // become a real comparison against a number nobody wrote.
            if let Some(n) = cells.get(i + 1).and_then(|c| c.parse::<usize>().ok()) {
                out.insert(inner.to_owned(), n);
            }
            break;
        }
    }
    out
}

/// The FIRST `n=<N>` on a `**Members:**` line, ignoring later ones.
///
/// Three entries quote a superseded count in the same sentence (*"Was 16 until…"*, *"It read
/// n=…"*), so "any `n=`" would read a historical figure as the live one — the same defect
/// [`parse_index_counts`] avoids by taking the cell immediately after the slug rather than the
/// first number in the row.
fn first_n_claim(rest: &str) -> Option<usize> {
    let mut hay = rest;
    while let Some(at) = hay.find("n=") {
        let digits: String = hay[at + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(n) = digits.parse::<usize>() {
            return Some(n);
        }
        hay = &hay[at + 2..];
    }
    None
}

/// `slug -> n`, parsed from each `## IC-` section's `**Members:**` line.
///
/// **Why this is a second parser and not a widening of [`parse_index_counts`].** The Index cell
/// and the `**Members:**` claim are different assertions about the same quantity, published in
/// different places, and they drift independently: measured 2026-09-01, the Index cells were
/// gated from 14:27 while four prose judgement fields silently kept reasoning from superseded
/// counts until a peer swept them by hand (`0c5bab41`). The gated number is not the one a reader
/// acts on.
///
/// The slug is taken from the section's own `**Slug:**` line and RESET at every `## IC-`
/// heading, so a section that declares a slug but no `**Members:**` line cannot leak its slug
/// into the next section's count. Pure over `text` so
/// [`the_member_claim_parser_discriminates`] can feed it a ledger whose right answers are known.
fn parse_member_counts(text: &str, valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    let mut slug: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("## IC-") {
            // A new section invalidates the previous one's slug, whether or not it was used.
            slug = None;
        } else if let Some(rest) = line.strip_prefix("**Slug:**") {
            slug = backticked_cluster_slug(rest).filter(|s| valid.contains(s));
        } else if let Some(rest) = line.strip_prefix("**Members:**") {
            if let (Some(s), Some(n)) = (slug.take(), first_n_claim(rest)) {
                out.insert(s, n);
            }
        }
    }
    out
}

/// [`parse_member_counts`] over the live ledger.
fn member_counts(valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let path = repo_root().join(LEDGER);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parse_member_counts(&text, valid)
}

/// [`parse_index_counts`] over the live ledger.
fn declared_counts(valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let path = repo_root().join(LEDGER);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    parse_index_counts(&text, valid)
}

/// The corpus side: how many bug files actually carry each slug.
///
/// Seeded with every valid slug at 0, so a class with no members is compared against its row
/// rather than dropping out of the comparison — `IC-12` is legitimately 0 and its row must still
/// be checked.
fn actual_counts(valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = valid.iter().map(|s| (s.clone(), 0)).collect();
    for rel in tracked_all_bug_files() {
        let Ok(content) = std::fs::read_to_string(repo_root().join(&rel)) else {
            continue;
        };
        let Some(fm) = frontmatter(&content) else {
            continue;
        };
        for tag in cluster_tags(fm) {
            if let Some(c) = out.get_mut(&tag) {
                *c += 1;
            }
        }
    }
    out
}

/// Every `n` in the ledger's Index table equals the class's real membership.
///
/// The counts are a decision input, not decoration: a class clearing n≥3 across ≥2 subsystems is
/// what promotes it into a rule, and `IC-10` crossed exactly that line on 2026-09-01. A stale
/// cell is a wrong input to a live decision.
///
/// This is a gate rather than a documented habit because the cells go stale by **concurrency**,
/// not by neglect. Measured 2026-09-01: three separate hand re-derivations were invalidated
/// inside one session by peer sessions filing bugs into the same checkout, so a sweep's own
/// result is falsified by the next commit and no amount of care holds it
/// (`cluster-promotion-session-log:F-4`).
/// Both ledger parsers agree with the hook script on shapes the live corpus does not contain.
///
/// [`the_hook_script_agrees_with_this_gate`] compares the two derivations over the **live**
/// ledger, and that corpus is not adversarial: every real section declares a `**Slug:**` before
/// its `**Members:**`, so the section-boundary reset is unreachable from it. Measured — deleting
/// that reset from either language leaves the corpus-driven comparison green. This feeds both
/// implementations the same adversarial fixture instead.
///
/// The fixture is [`the_member_claim_parser_discriminates`]'s, deliberately: the Rust parser is
/// pinned against known answers there, so agreeing with it here means the Python is pinned to
/// the same answers rather than merely to whatever Rust currently does.
///
/// Mutation that must kill this: drop the `## IC-` reset, or the first-`n=` rule, from the
/// PYTHON side — the Rust side is already covered by the discriminator.
#[test]
fn the_ledger_parsers_agree_on_a_fixture() {
    let ledger = "\
## IC-1 — first
**Slug:** `cluster/alpha`
| IC-1 | first | `cluster/alpha` | 7 | not yet — **10 share one layer** | none yet |
**Members:** `filter={...}` — **n=7, 2026-09-01, re-derived**. Was 16 until n=3 moved out.

## IC-2 — declares a slug but never states a count
**Slug:** `cluster/beta`
**Claim:** something.

## IC-3 — states a count but declares no slug
**Members:** `filter={...}` — n=99, by query.

## IC-4 — fourth
**Slug:** `cluster/gamma`
| IC-4 | fourth | `cluster/gamma` | 2 | not yet | none yet |
**Members:** `filter={...}` — n=2, by query.
";

    let mut child = Command::new("python3")
        .args(["scripts/pre-commit-ledger-counts.py", "--fixture-ledger"])
        .current_dir(repo_root())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("python3 failed to spawn");
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(ledger.as_bytes())
        .expect("write fixture");
    let out = child.wait_with_output().expect("hook script failed");
    assert!(
        out.status.success(),
        "script exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let got: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("script must emit JSON");

    let valid = valid_slugs_from(ledger);
    for (field, mine) in [
        ("declared", parse_index_counts(ledger, &valid)),
        ("claimed", parse_member_counts(ledger, &valid)),
    ] {
        let theirs: BTreeMap<String, usize> =
            serde_json::from_value(got[field].clone()).expect("map of slug -> count");
        assert_eq!(
            mine, theirs,
            "`{field}` disagrees on the adversarial fixture — the live-corpus agreement test \
             cannot see this, because the corpus contains no section that consumes a slug it \
             did not set.\n\
             Reproduce: printf '%s' \"<fixture>\" | python3 scripts/pre-commit-ledger-counts.py \
             --fixture-ledger"
        );
    }

    // The fixture must actually exercise the reset, or this test degrades into the one above.
    assert_eq!(
        parse_member_counts(ledger, &valid).get("beta"),
        None,
        "fixture no longer reaches the section-boundary leak; see \
         the_member_claim_parser_discriminates for why the IC-2/IC-3 ORDER is load-bearing"
    );
}

/// The hook script reads BOTH YAML tag styles, like [`cluster_tags`] does.
///
/// Split from [`the_hook_script_agrees_with_this_gate`] because that one runs against the live
/// corpus, and the corpus cannot reach this branch: measured 2026-09-01, **zero** bug files
/// carry a `cluster/` tag in flow style (`tags: [a, b]`), so deleting the inline arm from the
/// Python leaves the corpus-driven check **green**. Verified by mutation, not assumed — that
/// deletion was made and the other test passed.
///
/// The fixtures are [`both_yaml_tag_styles_are_read`]'s, deliberately: the two tests must
/// agree about what the corpus is allowed to contain, not merely each about itself.
///
/// Mutation that must kill this: drop either arm of the Python's `cluster_tags`.
#[test]
fn the_hook_script_agrees_on_both_yaml_tag_styles() {
    let block = "---\nkind: bug\ntags:\n- cluster/alpha\n- x\n---\n\n# t\n";
    let inline = "---\nkind: bug\ntags: [cluster/alpha, x]\n---\n\n# t\n";

    for (label, fixture) in [("block", block), ("inline", inline)] {
        let mut child = Command::new("python3")
            .args(["scripts/pre-commit-ledger-counts.py", "--fixture-tags"])
            .current_dir(repo_root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("python3 failed to spawn");
        child
            .stdin
            .take()
            .expect("stdin piped")
            .write_all(fixture.as_bytes())
            .expect("write fixture");
        let out = child.wait_with_output().expect("hook script failed");
        assert!(out.status.success(), "{label}: script exited non-zero");

        let theirs: Vec<String> =
            serde_json::from_slice(&out.stdout).expect("script must emit a JSON list");
        let mine = cluster_tags(frontmatter(fixture).expect("fixture has frontmatter"));
        assert_eq!(
            mine, theirs,
            "{label} style: this gate and scripts/pre-commit-ledger-counts.py disagree about \
             which cluster tags a frontmatter block declares"
        );
    }
}

/// Every `**Members:** … n=N` prose claim equals the class's real membership.
///
/// The sibling of [`every_index_count_matches_the_corpus`], and NOT redundant with it. That one
/// gates the Index table cell; this gates the number a reader actually acts on. They drift
/// independently and did: the Index cells were gated from 14:27 on 2026-09-01, and four `IC-N`
/// judgement fields went on reasoning from superseded counts until a peer repaired them by hand
/// hours later (`0c5bab41`) — one of them claiming a member that a blind second read had already
/// moved to another class.
///
/// A gated cell beside an ungated restatement is worse than neither, because the green tick
/// reads as covering both.
#[test]
fn every_member_claim_matches_the_corpus() {
    let valid = valid_slugs();
    let claimed = member_counts(&valid);
    let actual = actual_counts(&valid);

    let mut drift: Vec<String> = claimed
        .iter()
        .filter(|(slug, n)| actual.get(*slug).copied().unwrap_or(0) != **n)
        .map(|(slug, n)| {
            format!(
                "cluster/{slug} — **Members:** claims n={n}, corpus has {}",
                actual.get(slug).copied().unwrap_or(0)
            )
        })
        .collect();
    drift.sort();

    assert!(
        drift.is_empty(),
        "a `**Members:**` line disagrees with the corpus it summarises:\n  {}\n\n\
         The Index cell may be correct at the same moment — it is gated separately, and the two \
         drift independently. Re-derive rather than adjust by the delta:\n    \
         git grep -clE '^[[:space:]]*-[[:space:]]*cluster/<slug>[[:space:]]*$' -- 'docs/issues/*.md' | wc -l\n\n\
         Then check the `**Promotes to:**` field of the SAME entry in the same pass: it reasons \
         from this number, is not gated by anything, and is where the four 2026-09-01 drifts \
         actually did their damage.",
        drift.join("\n  ")
    );
}

/// Every declared class states a `**Members:**` count — the vacuity guard for the test above.
///
/// [`every_member_claim_matches_the_corpus`] is monotone under parser failure: a parser that
/// matches nothing produces an empty drift list and passes. Measured by deliberate break — the
/// same property [`every_declared_class_has_an_index_row`] exists for one layer up.
#[test]
fn every_declared_class_states_a_member_count() {
    let valid = valid_slugs();
    let claimed = member_counts(&valid);
    let missing: Vec<&String> = valid.iter().filter(|s| !claimed.contains_key(*s)).collect();
    assert!(
        missing.is_empty(),
        "declared classes with no parseable `**Members:** … n=N` line: {missing:?}\n\
         Either the entry is missing its count, or `parse_member_counts` stopped matching — \
         which would make `every_member_claim_matches_the_corpus` pass vacuously.\n\
         Note the template placeholder is NOT in this set: its `## IC-N — <the class…>` heading \
         declares `cluster/<slug>`, which `valid_slugs` rejects."
    );
}

/// The member-claim parser reads the right count and keys it to the right class.
///
/// Pure-fixture, because a parser that only ever runs against the live ledger cannot be shown to
/// read the right thing — it can only be shown not to have complained. Three discriminations,
/// each a real shape in the corpus:
///
/// 1. **First `n=` wins.** Three live entries quote a superseded count in the same sentence.
/// 2. **The slug is the section's own.** A `**Members:**` line is keyed by the `**Slug:**` above
///    it, not by whichever slug was seen last.
/// 3. **A slug does not survive its section.** `IC-2` declares one and states no count; `IC-3`
///    states a count and declares no slug. Only the `## IC-` boundary reset stops `IC-3`'s 99
///    being filed under `beta` — a silent wrong answer, the worst shape available here.
///
/// **The ORDER of `IC-2` and `IC-3` is what makes this test able to fail, and it is not
/// cosmetic.** `slug.take()` already clears after any successful match, so a countless section
/// anywhere else is harmless and the reset reads as dead code. The leak needs a section that
/// *sets* the slug without consuming it, immediately followed by one that consumes without
/// setting. Separate them and mutation F below passes with the reset deleted — measured, not
/// argued: the first version of this fixture put `IC-3` last and did exactly that.
///
/// Mutation that must kill this: replace the `line.starts_with("## IC-")` arm with `false`.
#[test]
fn the_member_claim_parser_discriminates() {
    let valid: BTreeSet<String> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let ledger = "\
## IC-1 — first
**Slug:** `cluster/alpha`
**Members:** `filter={...}` — **n=7, 2026-09-01, re-derived**. Was 16 until n=3 moved out.

## IC-2 — declares a slug but never states a count
**Slug:** `cluster/beta`
**Claim:** something.

## IC-3 — states a count but declares no slug
**Members:** `filter={...}` — n=99, by query.

## IC-4 — fourth
**Slug:** `cluster/gamma`
**Members:** `filter={...}` — n=2, by query.
";
    let got = parse_member_counts(ledger, &valid);

    assert_eq!(
        got.get("alpha"),
        Some(&7),
        "must take the FIRST n=, not the 16 or the 3 later in the same sentence"
    );
    assert_eq!(
        got.get("gamma"),
        Some(&2),
        "gamma's own count must survive the two malformed sections between them"
    );
    assert_eq!(
        got.get("beta"),
        None,
        "beta states no count. ABSENT, never 0 — a 0 becomes a real comparison against a number \
         nobody wrote — and never 99, which is IC-3's count leaking across a section boundary \
         into the last slug that happened to be set"
    );
    assert_eq!(got.len(), 2, "exactly alpha and gamma: {got:?}");
}

/// The pre-commit hook script derives the same counts this gate does.
///
/// `scripts/pre-commit-ledger-counts.py` duplicates the parse logic above on purpose: a cargo
/// invocation in the commit path costs ~7s warm and blocks unboundedly on the shared
/// `target/` lock, which on a ten-session checkout is a worse failure than the one it guards.
/// The duplication is the price; **this test is what stops it becoming drift.** Change either
/// derivation and this reddens until you change the other — a mechanism, not a resolution to
/// keep them in sync.
///
/// **Why `--source=worktree` and not the hook's own `index` mode.** The two answer different
/// questions by design — the hook reads the INDEX, so it sees what a commit *ships*, which is
/// exactly the state `every_index_count_matches_the_corpus` structurally cannot reach
/// (`reconnaissance-patterns:R-155`). Comparing them against different substrates would make
/// this test fail on any dirty tree, which is most of the time. Pointing both at the working
/// tree isolates the thing actually at risk: the **parse logic**.
///
/// Mutation that must kill this: change `parse_index_counts` to read `cells[i + 2]`, or drop
/// the inline-`[a, b]` arm from `cluster_tags`, in EITHER language.
#[test]
fn the_hook_script_agrees_with_this_gate() {
    let out = Command::new("python3")
        .args([
            "scripts/pre-commit-ledger-counts.py",
            "--source=worktree",
            "--json",
        ])
        .current_dir(repo_root())
        .output()
        .expect("python3 failed to run — the hook script needs it, so this gate does too");
    assert!(
        out.status.success(),
        "hook script exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let got: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("hook script must emit JSON under --json");

    let valid = valid_slugs();
    for (field, mine) in [
        ("declared", declared_counts(&valid)),
        ("actual", actual_counts(&valid)),
        ("claimed", member_counts(&valid)),
    ] {
        let theirs: BTreeMap<String, usize> =
            serde_json::from_value(got[field].clone()).expect("map of slug -> count");
        assert_eq!(
            mine, theirs,
            "`{field}` disagrees between this gate and scripts/pre-commit-ledger-counts.py.\n\
             The two derivations have drifted — fix whichever is wrong, in BOTH languages.\n\
             Reproduce: python3 scripts/pre-commit-ledger-counts.py --source=worktree --json"
        );
    }
}

#[test]
fn every_index_count_matches_the_corpus() {
    let valid = valid_slugs();
    let declared = declared_counts(&valid);
    let actual = actual_counts(&valid);

    let mut drift = Vec::new();
    for (slug, n) in &declared {
        let got = actual.get(slug).copied().unwrap_or(0);
        if got != *n {
            drift.push(format!("cluster/{slug} — table says {n}, corpus has {got}"));
        }
    }

    assert!(
        drift.is_empty(),
        "the Index table's `n` column disagrees with the corpus:\n  {}\n\n\
         Re-derive rather than adjust by the delta — per slug:\n    \
         git grep -clE '^[[:space:]]*-[[:space:]]*cluster/<slug>[[:space:]]*$' -- 'docs/issues/*.md' | wc -l\n\n\
         `git grep -l` counts FILES. `grep -o | sort | uniq -c` counts OCCURRENCES and \
         double-counts any bug file that also names its own slug in prose, which is why every \
         `n` in that table is a file count. If a file moved between classes, re-derive every \
         judgement quoting either count in the same pass — the `**Members:**` line and the \
         `**Promotes to:**` field of BOTH classes, since a count and the judgement that reads \
         it move independently.",
        drift.join("\n  ")
    );
}

/// The count check must actually compare something.
///
/// If [`parse_index_counts`] matched nothing — a renamed column, a reformatted table, a slug cell
/// that stopped being backticked — every comparison in [`every_index_count_matches_the_corpus`]
/// is skipped and it passes green forever. That is zero coverage wearing a passing test's
/// clothes, the shape this very ledger tracks as `IC-16`, so it is guarded rather than assumed.
/// The per-slug half also catches the narrower case: one row whose count cell stopped parsing.
///
/// **Measured, not argued.** Stripping the backticks off every slug cell — a one-line `sed` —
/// leaves [`every_index_count_matches_the_corpus`] *passing* and reds only this test, which then
/// names all 17 classes. Its sibling is monotone under parser failure by construction: an empty
/// map means an empty loop means no assertion. This test is the whole of what stands between
/// that and a gate that is green forever for the wrong reason.
///
/// **The count gate sees TRACKED files only, so a local green defers rather than clears.**
/// [`tracked_all_bug_files`] shells out to `git ls-files`, so a bug file that exists but has not
/// been `git add`ed is invisible to the count while its ledger row may already have been updated
/// — the pair agrees, the test passes, and the disagreement surfaces at CI once the file is
/// staged. That is deliberate and matches the module header's reason for gating on tracked files
/// only (an untracked file is a peer's in-flight work, and gating on it lets one session red
/// another's build). Reported from the receiving end by a peer session on 2026-09-01, who hit
/// exactly this: green locally while their new bug file was untracked, red once staged. The
/// failure text cannot say this, so it is said here.
#[test]
fn every_declared_class_has_an_index_row() {
    let valid = valid_slugs();
    let declared = declared_counts(&valid);

    let missing: Vec<&String> = valid
        .iter()
        .filter(|s| !declared.contains_key(*s))
        .collect();
    assert!(
        missing.is_empty(),
        "these classes declare a `**Slug:**` but have no parseable Index row: {missing:?}\n\
         Their counts are checked by nothing. Either the row is absent, or its `n` is not a bare \
         integer in the column immediately after the slug.\n\n\
         On a SHARED CHECKOUT there is a third possibility, and it is not your defect: a peer \
         session is mid-write. An entry section and its Index row are two writes, so slugs that \
         are theirs and in flight appear here until the second one lands. This message cannot \
         tell the cases apart — `git diff HEAD -- docs/trackers/issue-clusters.md` can. Reported \
         twice in one afternoon by a peer who worked it out unaided."
    );
    assert!(
        declared.len() > 10,
        "only {} Index rows parsed — the table format moved and the count gate is now comparing \
         almost nothing",
        declared.len()
    );
}

/// The row parser reads the right cell, and only real rows.
///
/// Feeds a table whose answers are known, covering the two ways a looser parser goes wrong:
/// taking any numeric cell in the row (`42` sits one column past the count), and treating a
/// non-`IC-` line that happens to mention a slug as a row.
#[test]
fn the_index_row_parser_discriminates() {
    let valid: BTreeSet<String> = ["alpha-slug", "beta-slug", "gamma-slug"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let table = "\
| id | class | slug | n | promotes to | mechanism |
|---|---|---|---:|---|---|
| IC-1 | some class | `alpha-slug` | 7 | 42 | none yet |
| IC-2 | another | `beta-slug` | 0 | not yet | **10 share one layer** (of 14) |
| note | prose naming `gamma-slug` | 99 | 99 | | |
";
    let got = parse_index_counts(table, &valid);

    assert_eq!(
        got.get("alpha-slug"),
        Some(&7),
        "must read the cell immediately after the slug, not any number in the row"
    );
    assert_eq!(
        got.get("beta-slug"),
        Some(&0),
        "a zero-membership class is a real row — skipping it would hide a class whose members \
         all moved away"
    );
    assert_eq!(
        got.get("gamma-slug"),
        None,
        "only `| IC-` lines are rows; prose naming a slug must not define a count"
    );
    assert_eq!(got.len(), 2);
}

/// The count scan must actually reach the archive.
///
/// [`actual_counts`] walks a different population from the tag gate, so it needs its own positive
/// control. A filter that silently excluded `archive/` would make every count read low, and the
/// comparison would then demand the ledger be rewritten *wrong*.
#[test]
fn the_count_scan_reaches_the_archive() {
    let files = tracked_all_bug_files();
    let archived = files.iter().filter(|f| f.contains("/archive/")).count();
    assert!(
        archived > 100,
        "expected the archive inside the count population; found {archived} archived files — \
         the scan is looking in the wrong place"
    );
    assert!(
        files.len() > tracked_open_bug_files().len(),
        "the count population must be strictly wider than the open-corpus gate's"
    );
}
