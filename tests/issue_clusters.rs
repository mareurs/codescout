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
