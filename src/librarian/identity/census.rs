//! A census of this repo's tracked markdown under the new identity model.
//!
//! One `#[ignore]`d test that prints `census.<field>: <value>` lines. It exists to size
//! the migration (Plan E): how many documents have no valid key, how many keys collide,
//! and how the explicit `[[...]]` links resolve today. It changes no behaviour and
//! asserts only that the index built over at least one document.
//!
//! Run: `cargo test --lib census_of_this_repo -- --ignored --nocapture`.
//!
//! Test code reads files with `std::fs` and runs `git` through `std::process::Command`
//! directly; the librarian read-door rule governs production code only.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::path::Path;
use std::process::Command;

use super::grammar::{extract, Reference};
use super::index::{DocInput, IndexProblem, KeyIndex, Resolution};
use super::key::{is_folder_index, key_of, KeyError};
use super::repo_name::{self, RepoName};

/// Every `Resolution` variant, in the order the census prints them.
const VARIANTS: [&str; 10] = [
    "File",
    "Entry",
    "Local",
    "NotLocal",
    "UnknownKey",
    "KeyCollision",
    "UndefinedToken",
    "AmbiguousToken",
    "CrossRepo",
    "Malformed",
];

fn field(name: &str, value: impl Display) {
    println!("census.{name}: {value}");
}

fn variant_name(r: &Resolution) -> &'static str {
    match r {
        Resolution::File { .. } => "File",
        Resolution::Entry { .. } => "Entry",
        Resolution::Local { .. } => "Local",
        Resolution::NotLocal => "NotLocal",
        Resolution::UnknownKey { .. } => "UnknownKey",
        Resolution::KeyCollision { .. } => "KeyCollision",
        Resolution::UndefinedToken { .. } => "UndefinedToken",
        Resolution::AmbiguousToken { .. } => "AmbiguousToken",
        Resolution::CrossRepo { .. } => "CrossRepo",
        Resolution::Malformed { .. } => "Malformed",
    }
}

/// The tracked markdown files as `(rel_path, text)`, plus how many were skipped.
struct Corpus {
    docs: Vec<(String, String)>,
    skipped_non_utf8: usize,
    skipped_unreadable: usize,
}

/// `git ls-files -z '*.md'` from `root`. `-z` keeps non-ASCII names raw: without it git
/// quotes and escapes them, and the census would see a different path than the file has.
fn load_corpus(root: &Path) -> Corpus {
    let out = Command::new("git")
        .args(["ls-files", "-z", "--", "*.md"])
        .current_dir(root)
        .output()
        .expect("git ls-files must run");
    assert!(
        out.status.success(),
        "git ls-files failed: {:?}",
        out.status
    );
    let mut corpus = Corpus {
        docs: Vec::new(),
        skipped_non_utf8: 0,
        skipped_unreadable: 0,
    };
    for raw in out.stdout.split(|&b| b == 0).filter(|p| !p.is_empty()) {
        let Ok(rel_path) = String::from_utf8(raw.to_vec()) else {
            corpus.skipped_non_utf8 += 1;
            continue;
        };
        match std::fs::read(root.join(&rel_path)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => corpus.docs.push((rel_path, text)),
                Err(_) => corpus.skipped_non_utf8 += 1,
            },
            Err(_) => corpus.skipped_unreadable += 1,
        }
    }
    corpus
}

/// The repo name the index is built with, and how it was obtained.
fn pick_repo_name(root: &Path) -> (RepoName, &'static str) {
    match repo_name::read(root) {
        Ok(Some(name)) => (name, "read"),
        Ok(None) => (repo_name::suggest(root), "suggested"),
        Err(e) => {
            println!("census.repo_name_read_error: {e:#}");
            (repo_name::suggest(root), "suggested")
        }
    }
}

/// One citation: where it is, what it says, and how it resolved.
struct Cited {
    source: String,
    line: u32,
    reference: Reference,
    resolution: Resolution,
}

fn resolve_all(corpus: &Corpus, index: &KeyIndex) -> Vec<Cited> {
    let mut all = Vec::new();
    for (rel_path, text) in &corpus.docs {
        for c in extract(text) {
            let resolution = index.resolve(rel_path, &c.reference);
            all.push(Cited {
                source: rel_path.clone(),
                line: c.line,
                reference: c.reference,
                resolution,
            });
        }
    }
    all
}

/// Why a derived key is not canonical. A key can have several causes. The key is
/// already ASCII-lowercased, so an uppercase letter is never a cause.
fn causes(key: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    if !key.is_ascii() {
        found.push("non_ascii");
    }
    if key.contains('_') {
        found.push("underscore");
    }
    if key.contains('.') {
        found.push("dot");
    }
    if key.chars().any(char::is_whitespace) {
        found.push("whitespace");
    }
    if key.chars().any(|c| c.is_ascii_uppercase()) {
        found.push("uppercase");
    }
    if key.starts_with('-') || key.ends_with('-') || key.contains("--") {
        found.push("hyphen_shape");
    }
    if found.is_empty() {
        found.push("other");
    }
    found
}

fn report_documents(corpus: &Corpus, this_repo: &RepoName, how: &str) {
    field("repo_name_source", how);
    field("repo_name", this_repo);
    field("documents_read", corpus.docs.len());
    field("skipped_non_utf8", corpus.skipped_non_utf8);
    field("skipped_unreadable", corpus.skipped_unreadable);
}

fn report_invalid_keys(corpus: &Corpus, problems: &[IndexProblem]) {
    let invalid: Vec<&KeyError> = problems
        .iter()
        .filter_map(|p| match p {
            IndexProblem::InvalidKey(e) => Some(e),
            IndexProblem::DuplicateKey { .. } => None,
        })
        .collect();
    field("invalid_key.total", invalid.len());
    let mut not_markdown = 0;
    let mut hex_shaped = 0;
    let mut non_canonical = 0;
    let mut by_cause: BTreeMap<&str, usize> = BTreeMap::new();
    let mut key_with_uppercase = 0;
    for e in &invalid {
        match e {
            KeyError::NotMarkdown(_) => not_markdown += 1,
            KeyError::HexShaped { .. } => hex_shaped += 1,
            KeyError::NonCanonical { key, .. } => {
                non_canonical += 1;
                if key.chars().any(|c| c.is_ascii_uppercase()) {
                    key_with_uppercase += 1;
                }
                for cause in causes(key) {
                    *by_cause.entry(cause).or_default() += 1;
                }
            }
        }
    }
    field("invalid_key.NotMarkdown", not_markdown);
    field("invalid_key.NonCanonical", non_canonical);
    field("invalid_key.HexShaped", hex_shaped);
    for cause in [
        "non_ascii",
        "underscore",
        "dot",
        "whitespace",
        "uppercase",
        "hyphen_shape",
        "other",
    ] {
        field(
            &format!("invalid_key.NonCanonical.cause.{cause}"),
            by_cause.get(cause).copied().unwrap_or(0),
        );
    }
    // An uppercase-only name is valid: the key lowercases. These two numbers verify it.
    field(
        "invalid_key.NonCanonical.key_with_uppercase_letter",
        key_with_uppercase,
    );
    let uppercase_stems: Vec<&str> = corpus
        .docs
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
            stem.chars().any(|c| c.is_ascii_uppercase())
        })
        .collect();
    field("uppercase_stem_documents", uppercase_stems.len());
    field(
        "uppercase_stem_documents.with_valid_key",
        uppercase_stems.iter().filter(|p| key_of(p).is_ok()).count(),
    );
    for e in invalid.iter().take(20) {
        match e {
            KeyError::NotMarkdown(p) => field("invalid_key.sample", format!("NotMarkdown {p:?}")),
            KeyError::NonCanonical { rel_path, key } => field(
                "invalid_key.sample",
                format!("NonCanonical {rel_path:?} -> {key:?}"),
            ),
            KeyError::HexShaped { rel_path, key } => field(
                "invalid_key.sample",
                format!("HexShaped {rel_path:?} -> {key:?}"),
            ),
        }
    }
}

fn report_duplicates(problems: &[IndexProblem]) {
    let groups: Vec<(&str, &[String])> = problems
        .iter()
        .filter_map(|p| match p {
            IndexProblem::DuplicateKey { key, rel_paths } => {
                Some((key.as_str(), rel_paths.as_slice()))
            }
            IndexProblem::InvalidKey(_) => None,
        })
        .collect();
    let involved: usize = groups.iter().map(|(_, p)| p.len()).sum();
    let folder_index = groups
        .iter()
        .flat_map(|(_, p)| p.iter())
        .filter(|p| is_folder_index(p))
        .count();
    field("duplicate_key.groups", groups.len());
    field("duplicate_key.documents_involved", involved);
    field("duplicate_key.documents_folder_index", folder_index);
    field(
        "duplicate_key.documents_other_files",
        involved - folder_index,
    );
    let all_folder_index = groups
        .iter()
        .filter(|(_, p)| p.iter().all(|p| is_folder_index(p)))
        .count();
    let no_folder_index = groups
        .iter()
        .filter(|(_, p)| !p.iter().any(|p| is_folder_index(p)))
        .count();
    field("duplicate_key.groups_only_folder_index", all_folder_index);
    field("duplicate_key.groups_no_folder_index", no_folder_index);
    let mut largest: Vec<&(&str, &[String])> = groups.iter().collect();
    largest.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
    for (key, paths) in largest.into_iter().take(20) {
        field(
            "duplicate_key.largest",
            format!("{key} size={}", paths.len()),
        );
    }
    for (key, paths) in groups.iter().take(10) {
        field(
            "duplicate_key.group",
            format!("{key} [{}]", paths.join(", ")),
        );
    }
}

fn report_folder_indexes(corpus: &Corpus, problems: &[IndexProblem]) {
    let colliding: HashSet<&str> = problems
        .iter()
        .filter_map(|p| match p {
            IndexProblem::DuplicateKey { key, .. } => Some(key.as_str()),
            IndexProblem::InvalidKey(_) => None,
        })
        .collect();
    let indexes: Vec<&str> = corpus
        .docs
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| is_folder_index(p))
        .collect();
    let usable = indexes
        .iter()
        .filter(|p| key_of(p).is_ok_and(|k| !colliding.contains(k.as_str())))
        .count();
    field("folder_index.documents", indexes.len());
    field("folder_index.with_usable_key", usable);
}

fn report_resolutions(cited: &[Cited]) {
    field("citations.total", cited.len());
    let mut all: BTreeMap<&str, usize> = BTreeMap::new();
    let mut grammar_form: BTreeMap<&str, usize> = BTreeMap::new();
    for c in cited {
        let v = variant_name(&c.resolution);
        *all.entry(v).or_default() += 1;
        if !matches!(c.reference, Reference::Bare { .. }) {
            *grammar_form.entry(v).or_default() += 1;
        }
    }
    for v in VARIANTS {
        field(
            &format!("citations.by_resolution.{v}"),
            all.get(v).copied().unwrap_or(0),
        );
    }
    field(
        "grammar_form_citations.total",
        grammar_form.values().sum::<usize>(),
    );
    for v in VARIANTS {
        field(
            &format!("grammar_form_citations.by_resolution.{v}"),
            grammar_form.get(v).copied().unwrap_or(0),
        );
    }
}

fn report_samples(cited: &[Cited]) {
    for (variant, limit) in [
        ("Malformed", 20),
        ("UnknownKey", 10),
        ("UndefinedToken", 10),
        ("AmbiguousToken", 10),
        ("KeyCollision", 10),
    ] {
        for c in cited
            .iter()
            .filter(|c| variant_name(&c.resolution) == variant)
            .take(limit)
        {
            let what = match &c.resolution {
                Resolution::Malformed { raw } => format!("raw={raw:?}"),
                _ => format!("reference={:?}", c.reference),
            };
            field(
                &format!("sample.{variant}"),
                format!("{}:{} {what}", c.source, c.line),
            );
        }
    }
}
/// `W-16`, `CTX-2`: an entry token written as a wiki link, as trackers do today.
fn looks_like_entry_token(raw: &str) -> bool {
    raw.split_once('-').is_some_and(|(letters, digits)| {
        (1..=5).contains(&letters.len())
            && letters.bytes().all(|b| b.is_ascii_uppercase())
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
    })
}

/// What the `Malformed` citations are, by the shape of their raw target.
fn report_malformed_shapes(cited: &[Cited]) {
    let mut shapes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut other: BTreeMap<&str, usize> = BTreeMap::new();
    for c in cited {
        let Resolution::Malformed { raw } = &c.resolution else {
            continue;
        };
        let shape = if looks_like_entry_token(raw) {
            "entry_token"
        } else if raw.contains('/') {
            "path_like"
        } else {
            *other.entry(raw.as_str()).or_default() += 1;
            "other"
        };
        *shapes.entry(shape).or_default() += 1;
    }
    for shape in ["entry_token", "path_like", "other"] {
        field(
            &format!("malformed.shape.{shape}"),
            shapes.get(shape).copied().unwrap_or(0),
        );
    }
    let mut ranked: Vec<(&str, usize)> = other.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (raw, n) in ranked.into_iter().take(10) {
        field("malformed.other.top", format!("{raw:?} x{n}"));
    }
}

/// `docs/trackers` for `docs/trackers/x.md`, `src` for `src/a.md`, `(root)` for `a.md`.
fn top_dir(rel_path: &str) -> String {
    let parts: Vec<&str> = rel_path.split('/').collect();
    match parts.len() {
        0 | 1 => "(root)".to_string(),
        2 => parts[0].to_string(),
        _ => format!("{}/{}", parts[0], parts[1]),
    }
}

fn report_links_per_directory(cited: &[Cited]) {
    let mut per_dir: BTreeMap<String, usize> = BTreeMap::new();
    for c in cited
        .iter()
        .filter(|c| !matches!(c.reference, Reference::Bare { .. }))
    {
        *per_dir.entry(top_dir(&c.source)).or_default() += 1;
    }
    let mut ranked: Vec<(String, usize)> = per_dir.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (dir, n) in ranked.into_iter().take(15) {
        field("wiki_links_by_directory", format!("{dir} {n}"));
    }
}

#[test]
#[ignore = "exploratory census of this repo's tracked markdown: slow, prints a report, run by hand with --ignored --nocapture"]
fn census_of_this_repo_under_the_new_identity_model() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus = load_corpus(root);
    let (this_repo, how) = pick_repo_name(root);
    report_documents(&corpus, &this_repo, how);

    let index = KeyIndex::build(
        this_repo,
        corpus.docs.iter().map(|(p, t)| DocInput {
            rel_path: p.clone(),
            text: t.clone(),
        }),
    );
    let problems = index.problems();
    report_invalid_keys(&corpus, problems);
    report_duplicates(problems);
    report_folder_indexes(&corpus, problems);

    let cited = resolve_all(&corpus, &index);
    report_resolutions(&cited);
    report_samples(&cited);
    report_malformed_shapes(&cited);
    report_links_per_directory(&cited);

    assert!(!corpus.docs.is_empty(), "no tracked markdown was read");
}
