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

/// The per-class files. Since 2026-09-02 the ledger is an **Index file plus one file per
/// class**: `docs/trackers/issue-clusters.md` keeps the preamble, conventions and Index
/// table, and each `## IC-N — <title>` section lives in its own file here.
///
/// The split was a **pure relocation** — every section byte-identical to what it replaced.
/// It exists because that one file was the repo's contention head: **16 distinct sessions,
/// 53 commits in a day**, 3× the next file. Removing the stored count earlier the same day
/// fixed the *gate* coupling and not the *file* coupling (19 commits from 9 sessions in the
/// 2.5h after). Two sessions amending two classes now touch two files.
/// See `docs/adrs/2026-09-02-isolate-what-is-cheap-own-what-is-shared.md`.
const LEDGER_DIR: &str = "docs/trackers/issue-clusters";

/// Index file + every class file, concatenated in a stable order.
///
/// **The parsers are unchanged by the split — only what they are pointed at moved.** That is
/// deliberate: `the_ledger_parsers_agree_on_a_fixture` pins them against the Python mirror in
/// `scripts/pre-commit-ledger-counts.py`, and rewriting a parser while the corpus moved under
/// it would have needed that pinning re-established at the moment it was most needed.
///
/// Concatenation is sound because every parser here is **line-anchored** — `**Slug:**`,
/// `**Members:**`, `| IC-N |` rows and `## IC-N —` headings are all recognised at line start,
/// so none can straddle the join between two files. A parser matching across lines would need
/// a per-file loop instead.
fn ledger_text() -> String {
    let mut parts = vec![std::fs::read_to_string(repo_root().join(LEDGER))
        .unwrap_or_else(|e| panic!("cannot read {LEDGER}: {e}"))];
    let dir = repo_root().join(LEDGER_DIR);
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    // Sorted so the concatenation is deterministic: unordered read_dir would make any
    // failure message depend on filesystem iteration order.
    files.sort();
    // LOAD-BEARING, and the reason it is an assert rather than a comment: if this directory
    // were empty or misnamed, every count below would parse the Index alone and report a
    // clean zero. `no_class_field_states_a_bare_n` is an ABSENCE assertion, so it is monotone
    // under exactly that failure — it would pass, forever, on a corpus it never read. This
    // asserts the population is non-empty, without which absence means nothing.
    assert!(
        !files.is_empty(),
        "no class files under {} — every absence assertion below would pass vacuously",
        dir.display()
    );
    for f in files {
        parts.push(
            std::fs::read_to_string(&f)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", f.display())),
        );
    }
    parts.join("\n")
}

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
/// Shared by [`valid_slugs`] and [`parse_bare_n_claims`] so the two cannot disagree about what
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
    let text = ledger_text();
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
// The Index table: which classes have a row, and that no row stores a count.
//
// Until 2026-09-02 this section checked an `n` column against the corpus it summarised. The
// column is gone — counts are derived (`scripts/probe-cluster-census.py`) — so the parsers here
// serve two opposite questions: `parse_index_rows` asks whether the table is still being read at
// all, and `parse_index_counts` exists so `no_index_row_stores_a_count` can assert it comes back
// empty. One parser cannot serve both, because the emptiness that is a PASS for one is the
// failure mode of the other.
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

/// Which slugs have an Index row at all — the count-free twin of [`parse_index_counts`].
///
/// [`every_declared_class_has_an_index_row`] used to prove row presence *via* a parseable count,
/// which stopped being possible when the `n` column was removed on 2026-09-02. Presence and
/// storedness are now separate questions asked by separate parsers: this one answers "is the
/// table still being read", [`parse_index_counts`] answers "did a count come back", and
/// [`no_index_row_stores_a_count`] wants that second answer to be empty. One parser cannot
/// serve both, because the emptiness that is a PASS for one is the failure mode of the other.
fn parse_index_rows(text: &str, valid: &BTreeSet<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        if !line.starts_with("| IC-") {
            continue;
        }
        for cell in line.split('|').map(str::trim) {
            let Some(inner) = cell.strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
                continue;
            };
            if valid.contains(inner) {
                out.insert(inner.to_owned());
                break;
            }
        }
    }
    out
}

/// Every **bare** `n=<N>` in a field's tail; an `n=` inside any backtick span is skipped.
///
/// **The backtick is the escape, and it is now the ONLY signal separating a live claim from a
/// quotation.** The ledger's house style preserves superseded figures with their derivation
/// rather than overwriting them, so `**Promotes to:**` legitimately carries sentences like
/// *"the archive backfill took it from `n=2` to `n=27`"*. Position cannot discriminate there —
/// two entries OPEN the field with a historical citation — so a first-`n=`-wins rule reddens
/// two correct fields. `89697a15` backticked the corpus's historical quotes, which is the
/// disambiguator `IC-6` says a parser over a namespace owes.
///
/// **Counted with its unit, because two readings differ by one.** Eleven backticked `n=`
/// occurrences under the SPAN reading, of which ten disagree with their class's count and would
/// be flagged if bare; the eleventh quotes a figure that happens to equal today's. A tight-token
/// count (`` `n=N` `` only) gives **ten**, because one occurrence is `` `n=1 taggable` `` — a
/// backticked PHRASE. That phrase is corpus evidence for the span rule below: the house style
/// already wraps prose, and it worked under adjacency only because its `n=` happened to sit
/// first inside the span.
///
/// This replaced a positional rule rather than joining it. Measured over all 22 `**Members:**`
/// lines, independently by two sessions: exactly one bare `n=` per line, equal to the positional
/// answer, and no line opens with a backticked quote — so the delimiter is strictly safer than
/// position, not merely simpler.
///
/// **The escape is SPAN-based, not adjacency-based, and that distinction was a shipped bug.**
/// The first version tested only the byte before `n=`, so a tight `` `n=16` `` was skipped while
/// an `n=` inside a backticked *phrase* — `` `took it from n=99 to n=98` `` — came back as two
/// live claims. Found by `codescout-3e` testing the shipped parser, not by review. It failed
/// loud (a false drift report, never a missed defect) and the corpus was clean because
/// `89697a15` made every quote tight, so nothing would have surfaced it. The reason to fix
/// rather than document: the rule a reader carries is *"backticked means quoted"*, and the
/// natural way to quote a superseded line is to wrap the LINE — so the remembered rule was
/// broader than the code, which is `IC-14` inside the gate that polices `IC-14`.
///
/// `n≥` is deliberately not matched: it appears in the ledger's prose as the *promotion
/// threshold*, never as a class count, so checking it against the corpus would report drift on
/// a sentence stating a rule.
fn bare_n_values(rest: &str) -> Vec<usize> {
    // Complete backtick PAIRS only. A dangling backtick opens nothing, so everything after it
    // stays CHECKED — the failure direction matters more than the edge case, and a false drift
    // report is loud where a skipped live claim would be silent.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut open: Option<usize> = None;
    for (i, &byte) in rest.as_bytes().iter().enumerate() {
        if byte == b'`' {
            match open.take() {
                Some(start) => spans.push((start, i)),
                None => open = Some(i),
            }
        }
    }

    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = rest[i..].find("n=") {
        let start = i + at;
        let quoted = spans.iter().any(|&(a, b)| a < start && start < b);
        let digits: String = rest[start + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if !quoted {
            if let Ok(n) = digits.parse::<usize>() {
                out.push(n);
            }
        }
        i = start + 2;
    }
    out
}

/// Every bare `n=<N>` in a class's judgement fields, keyed to that class.
///
/// **Why this is a second parser and not a widening of [`parse_index_counts`].** The Index cell
/// and the prose that restates it are different assertions about the same quantity, published in
/// different places, and they drift independently: measured 2026-09-01, the cells were gated from
/// 14:27 while four judgement fields kept reasoning from superseded counts until a peer swept
/// them by hand (`0c5bab41`). The gated number was not the one a reader acts on.
///
/// Both fields are read. `**Members:**` states the count; `**Promotes to:**` *reasons* from it,
/// and is where the four measured drifts actually did their damage.
///
/// The slug is taken from the section's own `**Slug:**` line and RESET at every `## IC-`
/// heading. That reset is the only clearing mechanism — a slug now spans two fields, so it is
/// not consumed on use — which makes a section that states a count while declaring no slug the
/// shape to guard: without the reset its count is filed under whichever class was last seen.
/// Pure over `text` so [`the_bare_n_claim_parser_discriminates`] can feed it a ledger whose
/// right answers are known.
fn parse_bare_n_claims(text: &str, valid: &BTreeSet<String>) -> Vec<(String, String, usize)> {
    const FIELDS: [(&str, &str); 2] = [
        ("**Members:**", "Members"),
        ("**Promotes to:**", "Promotes to"),
    ];
    let mut out = Vec::new();
    let mut slug: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("## IC-") {
            slug = None;
        } else if let Some(rest) = line.strip_prefix("**Slug:**") {
            slug = backticked_cluster_slug(rest).filter(|s| valid.contains(s));
        } else if let Some(s) = slug.clone() {
            for (prefix, label) in FIELDS {
                if let Some(rest) = line.strip_prefix(prefix) {
                    out.extend(
                        bare_n_values(rest)
                            .into_iter()
                            .map(|n| (s.clone(), label.to_owned(), n)),
                    );
                }
            }
        }
    }
    out.sort();
    out
}

/// [`parse_bare_n_claims`] over the live ledger.
fn bare_n_claims(valid: &BTreeSet<String>) -> Vec<(String, String, usize)> {
    let text = ledger_text();
    parse_bare_n_claims(&text, valid)
}

/// [`parse_index_counts`] over the live ledger.
fn declared_counts(valid: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let text = ledger_text();
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
/// its fields, and every historical quote is already backticked. Measured — deleting the
/// section-boundary reset, or the backtick check, from either language leaves the corpus-driven
/// comparison green. This feeds both implementations the same adversarial fixture instead.
///
/// The fixture is [`the_bare_n_claim_parser_discriminates`]'s, deliberately: the Rust parser is
/// pinned against known answers there, so agreeing with it here pins the Python to the same
/// answers rather than merely to whatever Rust currently does.
///
/// Mutation that must kill this: drop the `## IC-` reset, the backtick check, or the
/// `**Promotes to:**` field, from the PYTHON side.
#[test]
fn the_ledger_parsers_agree_on_a_fixture() {
    let ledger = "\
## IC-1 — first
**Slug:** `cluster/alpha`
| IC-1 | first | `cluster/alpha` | 7 | not yet — **10 share one layer** | none yet |
**Members:** `filter={...}` — n=7, 2026-09-01. The backfill took it from `n=2` to `n=27`.
**Promotes to:** `not yet` — n=7, below the bar. This field read `n=4` until today.\n\
**Promotes to:** `not yet` — n=7 again. `The backfill took it from n=98 to n=99` overnight.

## IC-2 — declares a slug but states no count
**Slug:** `cluster/beta`
**Claim:** something.

## IC-3 — states a count but declares no slug
**Members:** `filter={...}` — n=99, by query.

## IC-4 — fourth
**Slug:** `cluster/gamma`
| IC-4 | fourth | `cluster/gamma` | 2 | not yet | none yet |
**Members:** `filter={...}` — n=2, by query.
**Promotes to:** A stray ` backtick opens here, and a stale n=42 follows it.
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

    let theirs_declared: BTreeMap<String, usize> =
        serde_json::from_value(got["declared"].clone()).expect("map of slug -> count");
    assert_eq!(
        parse_index_counts(ledger, &valid),
        theirs_declared,
        "`declared` disagrees on the adversarial fixture"
    );

    let theirs_claimed: Vec<(String, String, usize)> =
        serde_json::from_value(got["claimed"].clone()).expect("list of [slug, field, n]");
    assert_eq!(
        parse_bare_n_claims(ledger, &valid),
        theirs_claimed,
        "`claimed` disagrees on the adversarial fixture — the live-corpus agreement test cannot \
         see this, because the corpus contains no section that consumes a slug it did not set \
         and no un-backticked historical quote.\n\
         Reproduce: printf '%s' \"<fixture>\" | python3 scripts/pre-commit-ledger-counts.py \
         --fixture-ledger"
    );

    // The fixture must actually exercise both traps, or this degrades into the live-corpus test.
    let claims = parse_bare_n_claims(ledger, &valid);
    assert!(
        !claims.iter().any(|(slug, _, _)| slug == "beta"),
        "fixture no longer reaches the section-boundary leak; the IC-2/IC-3 ORDER is load-bearing"
    );
    assert!(
        !claims.iter().any(|(_, _, n)| matches!(n, 27 | 4 | 98 | 99)),
        "fixture no longer reaches the backtick escape — a quoted n= is being read as a claim.\n\
         27 and 4 are checked because they appear ONLY inside backticks in this fixture. `n=2` \
         is deliberately NOT checked: it appears both as a quotation on IC-1 and as gamma's real \
         bare claim, so its presence proves nothing either way — a guard naming it fires on a \
         correct parser, which is what the first version of this assertion did.\n\
         98 and 99 sit inside a backticked PHRASE and cover the span-vs-adjacency distinction \
         specifically: they are the two an adjacency check lets through."
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
// ---------------------------------------------------------------------------
// `Mechanism status:` must carry a basis
// ---------------------------------------------------------------------------

/// The `**Mechanism status:**` bodies in the ledger, as `(entry id, body)`.
///
/// Pure over `text` so [`the_mechanism_basis_scan_discriminates`] can feed it entries whose
/// right answers are known — the live ledger has only the shapes it happens to contain today.
fn mechanism_statuses(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            // The `## Template for new entries` block carries a specimen field. A worked
            // example teaching the syntax is not a declaration — the same rule
            // `**Valid:**` detection uses.
            continue;
        }
        if let Some(rest) = line.strip_prefix("## IC-") {
            id = rest
                .split_whitespace()
                .next()
                .map(|n| format!("IC-{}", n.trim_end_matches(|c: char| !c.is_ascii_digit())));
        } else if let Some(body) = line.strip_prefix("**Mechanism status:**") {
            if let Some(ic) = id.clone() {
                out.push((ic, body.trim().to_owned()));
            }
        }
    }
    out
}

/// A field is *unbasised* when it asserts a state and gives no way to check it.
///
/// **Length of what REMAINS after the verdict is stripped — the third formulation tonight,
/// and each earlier one was refuted by a measurement rather than a preference.**
///
/// 1. *Does it contain a path or a SHA* — rejected before shipping: it over-reported 14 of
///    22 and would have failed `IC-6` (which cites `edit_markdown` and `artifact(get)` by
///    name) and `IC-18` (which cites "the ADR above"). Real citations in forms no regex
///    enumerates.
/// 2. *Exact match against `"none yet" | "partial" | "designed" | "shipped" | "none"`* —
///    shipped, then caught by `codescout-3e54`: a word list is monotone under RENAMING, so a
///    bare `no mechanism yet` or `unbuilt` passes silently, and re-wording the ledger's own
///    `none yet` would have disarmed the gate rather than failed it.
/// 3. *Length of the whole field, threshold 40* — refuted by this file's own fixture, whose
///    `` `partial` — `src/thing.rs` does it. `` is a real basis in 35 characters.
///
/// **Why stripping first is what makes the word list safe here.** `VERDICTS` is used to
/// LOCATE a prefix, never to decide. An unknown verdict word simply is not stripped, the
/// remainder is then the whole field, and the length test still applies — so a novel bare
/// verdict is caught (`unbuilt` → 7) while a long field passes. The failure mode degrades
/// toward flagging rather than toward silence, which is the direction formulation 2 got
/// backwards.
///
/// **Threshold derived, not chosen.** Measured over all 22 `**Mechanism status:**` fields on
/// 2026-09-02: shortest remainder **67** characters (`IC-15`), next 119, longest 5835, none
/// under 20. Nearest cases either side of 20 are the fixture's 25 and a bare
/// `no mechanism yet` at 16 — so the margin is thin on the fixture side deliberately, that
/// fixture being minimal on purpose to prove the parser, and 47 characters on the corpus side.
///
/// **What it still cannot catch:** a LONG field that is nonetheless all verdict. Each
/// formulation dominated the last; none is complete.
fn is_unbasised(body: &str) -> bool {
    const VERDICTS: [&str; 6] = [
        "none yet", "not yet", "partial", "designed", "shipped", "none",
    ];
    let mut s = body.trim().trim_start_matches(['*', '`']).trim().to_owned();
    let low = s.to_ascii_lowercase();
    // Longest first: "none yet" must win over "none".
    let mut by_len = VERDICTS;
    by_len.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for v in by_len {
        if low.starts_with(v) {
            s = s[v.len()..].to_owned();
            break;
        }
    }
    // Tolerate a parenthetical qualifier, e.g. `shipped (partial)`.
    if let Some(rest) = s.trim_start().strip_prefix('(') {
        if let Some((_, after)) = rest.split_once(')') {
            s = after.to_owned();
        }
    }
    let remainder = s
        .trim()
        .trim_start_matches(['*', '`', '.', '—', '–', '-'])
        .trim();
    remainder.chars().count() < 20
}

/// No `**Mechanism status:**` may be a bare verdict with no basis.
///
/// **Why this shape and not "is the claim true".** Nothing can gate the truth of a sentence
/// about the code. What a gate CAN require is that the sentence carry a route back to the
/// thing it describes, so a reader re-checks it in a minute instead of re-deriving it — which
/// is `CLAUDE.md` § *Observer Blindness*'s "ship the derivation rather than the value".
///
/// **Measured, and the reason this is not hypothetical.** Three fields were checked against
/// their code on 2026-09-01/02 and **three were wrong**: `IC-5` read `none yet` while
/// `scripts/build-windows.sh` had shipped a mechanism two hours after the field was written
/// AND explicitly refused the one the field proposed; `IC-4` read `none yet` while
/// `scripts/install-hooks.sh` had carried the check since its original fix, unrun by anything;
/// `IC-3`'s partition described 18 members against a corpus of 22. A bare `none yet` reads as
/// an established absence and was, in every case examined, an unexamined one.
///
/// The six that failed this gate when it was written were literally the nine characters
/// `none yet.` — no date, no candidate, no reason. They now say they have not been checked,
/// which is a weaker claim and a true one; the point is that "nobody has looked" and "there is
/// nothing to find" stop being the same sentence.
#[test]
fn no_mechanism_status_is_a_bare_verdict() {
    let text = ledger_text();
    let bare: Vec<String> = mechanism_statuses(&text)
        .into_iter()
        .filter(|(_, body)| is_unbasised(body))
        .map(|(ic, body)| format!("{ic} — `{body}`"))
        .collect();

    assert!(
        bare.is_empty(),
        "a `**Mechanism status:**` states a verdict and gives no basis:\n  {}\n\n\
         A bare verdict cannot be re-checked, so it is read as established. Measured on this \
         ledger: of three such fields checked against their code, three were wrong — two \
         claimed `none yet` over a mechanism that had already shipped, one of them over a \
         mechanism that explicitly REFUSED the remedy the field proposed.\n\n\
         Add whatever makes it checkable in one minute, not a paragraph:\n    \
         `shipped`/`partial`/`designed` -> name where (a path, a symbol, a SHA, a tool, an ADR)\n    \
         `none yet`                     -> say when it was last checked against the code, or \
         that it has not been\n\n\
         \"Not checked\" is a legitimate and useful answer. It is the difference between \
         nobody having looked and there being nothing to find, and this gate exists because \
         those two had the same nine characters.",
        bare.join("\n  ")
    );
}

/// The scan and the predicate must each return both answers, or the gate is decoration.
#[test]
fn the_mechanism_basis_scan_discriminates() {
    // The predicate: bare verdicts in the forms the corpus actually uses.
    assert!(is_unbasised("none yet."));
    assert!(is_unbasised("none yet"));
    assert!(is_unbasised("`partial`"));
    assert!(is_unbasised("**designed**".trim_matches('*')));
    // ...and anything carrying a basis, including citation forms no regex enumerates.
    assert!(!is_unbasised(
        "none yet — not checked against the code as of 2026-09-02"
    ));
    assert!(!is_unbasised(
        "`shipped (partial)` — `edit_markdown` gained an `occurrence` selector"
    ));
    assert!(!is_unbasised(
        "partial, covered in principle by the ADR above"
    ));

    // The scan: keyed to the right entry, and blind to the template's specimen field.
    let doc = "\
## IC-1 — a class
**Mechanism status:** none yet.

## IC-2 — another
**Mechanism status:** `partial` — `src/thing.rs` does it.

## Template for new entries

```markdown
## IC-N — <the class>
**Mechanism status:** none yet | designed | shipped (<what>)
```
";
    let found = mechanism_statuses(doc);
    assert_eq!(
        found.iter().map(|(i, _)| i.as_str()).collect::<Vec<_>>(),
        vec!["IC-1", "IC-2"],
        "the fenced template specimen must not be collected — it is a worked example, and \
         collecting it would make this gate permanently red on a file that is correct"
    );
    assert!(is_unbasised(&found[0].1));
    assert!(!is_unbasised(&found[1].1));

    // The synonyms formulation 2 let through. None of these is in `VERDICTS`, so nothing is
    // stripped and the whole string is measured — which is the graceful-degradation property.
    for bare in [
        "no mechanism yet",
        "nothing yet.",
        "unbuilt",
        "not built yet",
        "TBD",
    ] {
        assert!(
            is_unbasised(bare),
            "a bare verdict must be caught whatever its wording; the word-list version passed \
             {bare:?} silently, which is how a rename would have disarmed this gate"
        );
    }

    // And the other direction: a terse but REAL basis must survive. 25 characters of
    // remainder, which is the nearest passing case to the threshold and the reason the
    // whole-field length formulation was withdrawn.
    assert!(!is_unbasised("`partial` — `src/thing.rs` does it."));
    // A parenthetical qualifier must not be mistaken for the basis.
    assert!(
        is_unbasised("`shipped (partial)`"),
        "`(partial)` qualifies the verdict; it is not a way to check it"
    );
    assert!(!is_unbasised(
        "`shipped (partial)` — `scripts/build-windows.sh` prints the pinned wine version"
    ));
}

/// No class field states a live bare `n=` — the ledger stores no derived count.
///
/// Renamed from `every_bare_n_in_a_class_field_matches_the_corpus` on 2026-09-02, when the
/// assertion inverted. Records elsewhere citing the old name are narrating events that happened
/// under it and are correct as written; this note is the disambiguator that lets the old name
/// still be grepped to its successor.
///
/// **This is the inversion of a gate, not its deletion.** It used to assert that every bare
/// `n=` *equalled* the corpus. The counts are exactly derivable
/// (`scripts/probe-cluster-census.py`), and storing a derived value in a file 22 classes share
/// made every bug filer edit it: three surfaces carried the number, and a peer's commit staled
/// yours between deriving it and committing it. Measured 2026-09-01 — three separate
/// re-derivations invalidated inside one session, so no amount of care held them.
///
/// A backticked `` `n=N` `` is untouched and still means what it always meant: a QUOTATION of a
/// superseded figure, preserved with its derivation. The migration wrapped every live claim in
/// backticks rather than deleting it, so no sentence lost its history — only its obligation to
/// stay current.
///
/// **Why this absence assertion is not vacuous.** It is monotone under parser failure by
/// construction: a `bare_n_values` that matches nothing yields an empty list and passes green
/// forever. What stands against that is [`the_bare_n_claim_parser_discriminates`], which runs
/// over a fixture with known answers and proves the parser still finds a bare `n=` that IS
/// there. The pairing is the coverage; neither half is worth anything alone, and the fixture
/// half must never be weakened to the live ledger — a corpus with no bare `n=` in it cannot
/// discriminate a working parser from a broken one.
///
/// **What replaced the forcing function this gate used to be.** The count is what made a ledger
/// edit *mandatory*: an author wrote the per-member derivation while satisfying the refusal.
/// Measured on `1b92a7de` — one bug filing added **1,508 characters** of hand-authored,
/// non-derivable prose across the three lines it had to touch for the number. Removing the
/// number alone would have removed the reason anyone writes that, and nothing would report the
/// thinning: a `**Members:**` with 22 members and no derivations reads identically to one with
/// full derivations, to every query. `scripts/pre-commit-ledger-counts.py` now asks for the
/// prose instead — a commit that adds a member to a class must change that class's
/// `**Members:**` line. Found by `codescout-17` (sessionId `9716a130`), which measured its own
/// commit rather than accepting the premise it was handed.
///
/// Mutation that must kill this: reintroduce a bare `n=` into any `**Members:**` or
/// `**Promotes to:**` field.
#[test]
fn no_class_field_states_a_bare_n() {
    let valid = valid_slugs();

    let stored: Vec<String> = bare_n_claims(&valid)
        .into_iter()
        .map(|(slug, field, n)| format!("cluster/{slug} — **{field}:** stores a bare n={n}"))
        .collect();

    assert!(
        stored.is_empty(),
        "these class fields store a derived count:\n  {}\n\n\
         The ledger no longer stores counts — derive them with \
         `python3 scripts/probe-cluster-census.py`. A stored count in a file 22 classes share \
         made every bug filer edit it, and a peer's commit staled yours between deriving it and \
         committing it.\n\n\
         If you meant to QUOTE a figure — which the house style encourages, with its derivation \
         — wrap it in backticks. A backticked count is a quotation and is deliberately not \
         checked. If you meant to state today's count, don't: cite the probe instead, so the \
         sentence cannot decay.",
        stored.join("\n  ")
    );
}

/// The bare-`n=` parser reads the right numbers and keys them to the right class.
///
/// Pure-fixture, because a parser that only ever runs against the live ledger cannot be shown to
/// read the right thing — it can only be shown not to have complained. Four discriminations,
/// each a real shape in the corpus:
///
/// 1. **Backticked is skipped, by SPAN not adjacency.** `IC-6` legitimately writes *"took it
///    from `n=2` to `n=27`"* with tight tokens; the fixture also carries a backticked PHRASE
///    wrapping two `n=`s, because the natural way to quote a superseded LINE is to wrap the
///    line. An adjacency check passes every other assertion in this test and lets those two
///    through — that version shipped, at `cd17a58c`.
/// 2. **Both fields are read.** `**Promotes to:**` is where the four measured drifts lived.
/// 3. **A slug spans its own section and no further.** `IC-2` declares one and states no count;
///    `IC-3` states a count and declares no slug. Only the `## IC-` reset stops `IC-3`'s 99
///    being filed under `beta` — a silent wrong answer, the worst shape available here.
/// 4. **A bare `n=` on a non-field line is not a claim.** `**Claim:**` prose is not gated.
/// 5. **A DANGLING backtick opens nothing — the tail stays CHECKED.** `IC-4`'s 42 follows a
///    lone backtick and must come back as a claim. This is the property the span rewrite could
///    have broken in the SILENT direction, which adjacency structurally could not: an unbalanced
///    span swallowing the rest of the line would skip real claims with nothing to show for it.
///    It was a comment on `bare_n_values` until `codescout-3e` measured it from outside; a
///    property worth an external measurement is worth a test.
///
/// **The ORDER of `IC-2` and `IC-3` is what makes discrimination 3 able to fail.** A slug now
/// spans two fields, so it is no longer consumed on use and the `## IC-` reset is the only
/// clearing mechanism — which makes the leak reachable from more shapes than before, not fewer.
/// Separate these two sections and the mutation survives: measured, on the previous version of
/// this fixture, which put them apart.
///
/// Mutations that must kill this: drop the backtick check in `bare_n_values`; drop the
/// `"**Promotes to:**"` entry from `FIELDS`; replace the `## IC-` arm with `false`.
#[test]
fn the_bare_n_claim_parser_discriminates() {
    let valid: BTreeSet<String> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let ledger = "\
## IC-1 — first
**Slug:** `cluster/alpha`
**Members:** `filter={...}` — n=7, 2026-09-01. The backfill took it from `n=2` to `n=27`.
**Promotes to:** `not yet` — n=7, below the bar. This field read `n=4` until today.\n\
**Promotes to:** `not yet` — n=7 again. `The backfill took it from n=98 to n=99` overnight.
**Claim:** a sentence mentioning n=1234 that is not a judgement field.

## IC-2 — declares a slug but states no count
**Slug:** `cluster/beta`
**Claim:** something.

## IC-3 — states a count but declares no slug
**Members:** `filter={...}` — n=99, by query.

## IC-4 — fourth
**Slug:** `cluster/gamma`
**Members:** `filter={...}` — n=2, by query.
**Promotes to:** A stray ` backtick opens here, and a stale n=42 follows it.
";
    let got = parse_bare_n_claims(ledger, &valid);

    assert_eq!(
        got,
        vec![
            ("alpha".to_string(), "Members".to_string(), 7),
            ("alpha".to_string(), "Promotes to".to_string(), 7),
            ("alpha".to_string(), "Promotes to".to_string(), 7),
            ("gamma".to_string(), "Members".to_string(), 2),
            ("gamma".to_string(), "Promotes to".to_string(), 42),
        ],
        "expected exactly alpha's two bare claims and gamma's one.\n\
         - `n=2`/`n=27`/`n=4` are BACKTICKED TOKENS and must be skipped\n\
         - n=98/n=99 sit inside a backticked PHRASE and must ALSO be skipped: the escape is \
           span-based, not adjacency-based. An adjacency check passes every other assertion \
           here and lets these two through as live claims — that shipped, and was found by \
           testing rather than by review\n\
         - n=1234 sits on a `**Claim:**` line, which is not a gated field\n\
         - n=99 belongs to a section that declares no slug; it must be DROPPED, never filed \
           under `beta` — that is IC-3's count leaking across a section boundary into the last \
           slug that happened to be set\n\
         - `beta` states no count and must appear nowhere, so the vacuity guard reports it"
    );
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

    let theirs_claimed: Vec<(String, String, usize)> =
        serde_json::from_value(got["claimed"].clone()).expect("list of [slug, field, n]");
    assert_eq!(
        bare_n_claims(&valid),
        theirs_claimed,
        "`claimed` disagrees between this gate and scripts/pre-commit-ledger-counts.py"
    );

    for (field, mine) in [
        ("declared", declared_counts(&valid)),
        ("actual", actual_counts(&valid)),
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

/// The Index table stores no count — every row is title, slug, verdict, mechanism, and nothing
/// derived.
///
/// Renamed from `every_index_count_matches_the_corpus` on 2026-09-02, when the assertion
/// inverted. Records elsewhere citing the old name narrate events that happened under it and are
/// correct as written; this note is the disambiguator that lets the old name be grepped to its
/// successor.
///
/// The `n` column was a stored copy of a value derived twice already —
/// [`actual_counts`] here and `actual_counts` in `scripts/pre-commit-ledger-counts.py`. Storing
/// it made every bug filer edit a file 22 classes share, and it went stale by **concurrency**
/// rather than neglect: measured 2026-09-01, three separate hand re-derivations were invalidated
/// inside one session by peers filing bugs into the same checkout, so the sweep's own result was
/// falsified by the next commit and no amount of care held it.
///
/// **What did NOT move out of the row, and why the distinction is the whole design.** Subsystem
/// spread, the promotion verdict and mechanism status are *adjudications* — no query derives
/// them, so they stay. The row used to mix a derived counter with a human verdict, which is
/// exactly why bumping the counter forced a write to the verdict's file.
///
/// **Vacuity.** This is an absence assertion and is monotone under parser failure — a
/// [`parse_index_counts`] that matched nothing would pass it green forever. What stands against
/// that is [`the_index_row_parser_discriminates`], which feeds a fixture with known answers and
/// proves the parser still finds a count that IS there. Its sibling
/// [`every_declared_class_has_an_index_row`] separately proves the rows are still being parsed
/// at all, over a count-free parser, so a table that stopped matching cannot hide here.
///
/// Live counts: `python3 scripts/probe-cluster-census.py`.
///
/// Mutation that must kill this: re-add an `n` cell to any Index row.
#[test]
fn no_index_row_stores_a_count() {
    let valid = valid_slugs();
    let stored = declared_counts(&valid);

    let rows: Vec<String> = stored
        .iter()
        .map(|(slug, n)| format!("cluster/{slug} — Index row stores {n}"))
        .collect();

    assert!(
        rows.is_empty(),
        "the Index table stores derived counts:\n  {}\n\n\
         The `n` column was removed on 2026-09-02. It was a stored copy of a value derived twice \
         already, and storing it made every bug filer edit a file 22 classes share — a peer's \
         commit staled your number between deriving it and committing it.\n\n\
         Read live counts with `python3 scripts/probe-cluster-census.py`. The row carries what \
         no query can derive: spread, verdict, mechanism status.",
        rows.join("\n  ")
    );
}

/// Every declared class has an Index row — the emptiness guard for [`no_index_row_stores_a_count`].
///
/// If [`parse_index_rows`] matched nothing — a renamed column, a reformatted table, a slug cell
/// that stopped being backticked — [`no_index_row_stores_a_count`] would pass green forever on
/// an empty map. That is zero coverage wearing a passing test's clothes, the shape this ledger
/// tracks as `IC-16`, so it is guarded rather than assumed.
///
/// **The guard survived the 2026-09-02 inversion by changing which parser it uses, and that is
/// worth reading.** It used to prove presence *via* a parseable count, so removing the `n`
/// column reddened it — correctly. That red is what made the migration fail loud instead of
/// silently converting a real gate into a vacuous one: the count could not be deleted by
/// deleting cells, because this test refuses a table it can no longer read. It now runs over
/// [`parse_index_rows`], which reads the slug cell and no number.
///
/// **Measured, not argued.** Stripping the backticks off every slug cell — a one-line `sed` —
/// leaves the sibling passing and reds only this test, which then names every class.
///
/// **The count gate sees TRACKED files only, so a local green defers rather than clears.**
/// [`tracked_all_bug_files`] shells out to `git ls-files`, so a bug file that exists but has not
/// been `git add`ed is invisible while the ledger may already describe it — the pair agrees, the
/// test passes, and the disagreement surfaces at CI once the file is staged. That is deliberate:
/// an untracked file is a peer's in-flight work, and gating on it lets one session red another's
/// build. Reported from the receiving end by a peer session on 2026-09-01, who hit exactly this.
/// The failure text cannot say this, so it is said here.
#[test]
fn every_declared_class_has_an_index_row() {
    let valid = valid_slugs();
    let text = ledger_text();
    let rows = parse_index_rows(&text, &valid);

    let missing: Vec<&String> = valid.iter().filter(|s| !rows.contains(*s)).collect();
    assert!(
        missing.is_empty(),
        "these classes declare a `**Slug:**` but have no parseable Index row: {missing:?}\n\
         Either the row is absent, or its slug cell stopped being backticked.\n\n\
         On a SHARED CHECKOUT there is a third possibility, and it is not your defect: a peer \
         session is mid-write. An entry section and its Index row are two writes, so slugs that \
         are theirs and in flight appear here until the second one lands. This message cannot \
         tell the cases apart — `git diff HEAD -- docs/trackers/issue-clusters.md` can. Reported \
         twice in one afternoon by a peer who worked it out unaided."
    );
    assert!(
        rows.len() > 10,
        "only {} Index rows parsed — the table format moved, and `no_index_row_stores_a_count` \
         is now asserting emptiness over a table nobody can read, which it would pass",
        rows.len()
    );
}

/// The split's own regression guard, and the reason the two-step filing flow is a MECHANISM
/// rather than a note somebody has to remember.
///
/// `append_entry`'s `PendingSection` splices the new section into **the artifact's own file**
/// (`src/librarian/tools/append_entry.rs`), and the artifact at `docs/trackers/issue-clusters.md`
/// is now the Index. So filing a new class through the guarded path — which is the correct way
/// to allocate the id, and the only way to get a `## IC-N — <title>` heading whose shape
/// `link_scan` accepts as a definition — writes the section back into the file this split
/// emptied. The parent would silently re-accrete class sections, one per new class, until it
/// was a monolith again.
///
/// The flow is therefore: **append through the parent, then move the section to
/// `docs/trackers/issue-clusters/IC-N-<slug>.md`.** This test is what makes step 2
/// non-optional. Without it, step 2 is a policy aimed at whoever files the next class — and
/// this project's own record is that a trigger the model must notice is not a mechanism
/// (`skill-frictions:SKF-22`).
///
/// Deliberately NOT solved by declaring `entry_prefix: IC` in the per-class files: 23
/// co-declarers would raise `link_scan`'s `prefix_conflicts` from its baseline of 2. That is
/// the tempting repair, and the instrument already rejects it. The real closure is a
/// target-file param on `append_entry`; filed separately.
///
/// Found by `codescout-dd` (sessionId `c45dd5ef`) reviewing the split before it landed — the
/// one claim in the plan that broke, and neither of the two the plan defended.
#[test]
fn the_index_file_holds_no_class_sections() {
    let text = std::fs::read_to_string(repo_root().join(LEDGER))
        .unwrap_or_else(|e| panic!("cannot read {LEDGER}: {e}"));
    let stray: Vec<&str> = text
        .lines()
        .filter(|l| {
            // `## IC-N — <the class…>` in the template is not a class section: the token
            // grammar is `[A-Z]{1,3}-\d+`, and `N` is not a digit. Requiring digits here is
            // what keeps the template from tripping this gate.
            l.strip_prefix("## IC-")
                .and_then(|r| r.split_once(' '))
                .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    assert!(
        stray.is_empty(),
        "{LEDGER} is the Index — class sections live in {LEDGER_DIR}/IC-N-<slug>.md.\n\
         Found {} section(s) that should have been moved:\n  {}\n\n\
         This is the expected state right after `append_entry` files a NEW class: the section \
         is spliced into the parent artifact's own file. Move it to its own file (the section \
         body verbatim, plus tracker frontmatter), leave the Index row behind, and commit both \
         together. Do NOT fix this by declaring `entry_prefix: IC` in the class file — that \
         raises link_scan's prefix_conflicts instead.",
        stray.len(),
        stray.join("\n  ")
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
