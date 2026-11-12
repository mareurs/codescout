//! The entry table: one collection's entries as plain rows, and the markdown table that lists
//! them.
//!
//! Pure: `rows` reads an already-built `LifecycleTree`; `render_markdown` writes the rows as a
//! GFM table whose entry cell cites the entry (`[[<key>]] <title>`), so the rendered table is
//! itself valid grammar.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::entry::class_of;
use super::schema::StatusClass;
use super::tree::LifecycleTree;
use crate::librarian::identity::key::Key;

/// One entry of a collection, flattened for listing.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EntryRow {
    #[serde(serialize_with = "key_as_str")]
    pub key: Key,
    pub entry_kind: String,
    pub title: String,
    pub status: String,
    /// The class the collection's schema gives `status`; `None` when the schema names no class
    /// for the word.
    pub class: Option<StatusClass>,
    pub created: NaiveDate,
    pub closed: Option<NaiveDate>,
    pub revisit_by: Option<NaiveDate>,
    pub archived: bool,
    /// Every non-reserved extra frontmatter key of the entry, as read: declared in the
    /// collection schema or not.
    pub fields: BTreeMap<String, Value>,
}

pub(super) fn key_as_str<S: Serializer>(key: &Key, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(key.as_str())
}

/// The entries of the collection keyed `collection`, newest `created` first and then by key;
/// `None` when the tree has no such collection. Archived entries (every entry of an archived
/// collection included) are left out unless `include_archived`.
pub fn rows(
    tree: &LifecycleTree,
    collection: &Key,
    include_archived: bool,
) -> Option<Vec<EntryRow>> {
    let mut nodes = tree
        .collections
        .iter()
        .filter(|c| &c.key == collection)
        .peekable();
    nodes.peek()?;
    let mut out: Vec<EntryRow> = nodes
        .flat_map(|c| {
            c.entries
                .iter()
                .filter(move |e| include_archived || !e.archived)
                .map(move |e| EntryRow {
                    key: e.key.clone(),
                    entry_kind: e.meta.entry_kind.clone(),
                    title: e.meta.title.clone(),
                    status: e.meta.status.clone(),
                    class: class_of(&c.schema, &e.meta),
                    created: e.meta.created,
                    closed: e.meta.closed,
                    revisit_by: e.meta.revisit_by,
                    archived: e.archived,
                    fields: e.meta.fields.clone(),
                })
        })
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.key.cmp(&b.key)));
    Some(out)
}

/// One table cell's text: a newline or carriage return becomes a space, and `|` and `[` are
/// escaped, so the cell stays one cell and its text can never open a citation.
fn cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' | '\r' => out.push(' '),
            '|' => out.push_str("\\|"),
            '[' => out.push_str("\\["),
            c => out.push(c),
        }
    }
    out
}

fn date_cell(d: Option<NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_default()
}

/// The rows as a GFM table with the columns `Entry | Kind | Status | Created | Closed`. The
/// entry cell is `[[<key>]] <title>`.
///
/// Guaranteed: each row cites its own entry key once, as a wiki link; a title (or any other
/// text cell) cannot add a wiki-link citation or break its row, because `|` and `[` are
/// escaped and a newline becomes a space.
///
/// Not guaranteed: a bare token in a title (`R-91`) is still read as a bare token by
/// `grammar::extract`, and a bare token resolves only inside the document that hosts the table.
/// A caller that writes the table into a file must decide whether to neutralise bare tokens first.
pub fn render_markdown(rows: &[EntryRow]) -> String {
    let mut out =
        String::from("| Entry | Kind | Status | Created | Closed |\n|---|---|---|---|---|\n");
    for r in rows {
        out.push_str(&format!(
            "| [[{}]] {} | {} | {} | {} | {} |\n",
            r.key,
            cell(&r.title),
            cell(&r.entry_kind),
            cell(&r.status),
            r.created,
            date_cell(r.closed),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::identity::grammar::{self, Reference};
    use crate::librarian::lifecycle::testkit::{doc, k, LEDGER};
    use crate::librarian::lifecycle::tree::LifecycleTree;

    /// An entry of kind `f` whose status and created dates are `created`; `extra` is more
    /// frontmatter lines. The same shape as `testkit::entry`, with the created date a parameter.
    fn entry_created(status: &str, created: &str, extra: &str) -> String {
        format!("---\nentry_kind: f\ntitle: t\nstatus: {status}\nstatus_since: {created}\ncreated: {created}\n{extra}---\n")
    }

    /// An open entry whose `title:` line carries `title_yaml` verbatim (so a test chooses the
    /// YAML quoting).
    fn titled(title_yaml: &str) -> String {
        format!("---\nentry_kind: f\ntitle: {title_yaml}\nstatus: open\nstatus_since: 2026-10-01\ncreated: 2026-10-01\n---\n")
    }

    /// The markdown table of a ledger `l` holding the one entry `text`.
    fn table_of(text: &str) -> String {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc("docs/trackers/ledgers/l/2026-10-01-old.md", text),
        ]);
        render_markdown(&rows(&tree, &k("l"), false).unwrap())
    }

    /// Unescaped `|` in one table line: a backslash escapes the character after it.
    fn separators(line: &str) -> usize {
        let mut n = 0;
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '|' => n += 1,
                _ => {}
            }
        }
        n
    }

    /// How many table cells the Markdown parser finds in `md`.
    fn cells(md: &str) -> usize {
        use pulldown_cmark::{Event, Options, Parser, Tag};
        Parser::new_ext(md, Options::ENABLE_TABLES)
            .filter(|e| matches!(e, Event::Start(Tag::TableCell)))
            .count()
    }

    #[test]
    fn rows_sort_newest_first_and_hide_the_archive_by_default() {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-old.md",
                &entry_created("open", "2026-10-01", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-11-01-new.md",
                &entry_created("open", "2026-11-01", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-09-01-gone.md",
                &entry_created("fixed", "2026-09-01", "closed: 2026-09-02\n"),
            ),
        ]);
        let l = k("l");
        let live: Vec<String> = rows(&tree, &l, false)
            .unwrap()
            .iter()
            .map(|r| r.key.to_string())
            .collect();
        assert_eq!(live, ["2026-11-01-new", "2026-10-01-old"]);
        assert_eq!(rows(&tree, &l, true).unwrap().len(), 3);
        assert!(rows(&tree, &k("nope"), false).is_none());
    }

    #[test]
    fn the_markdown_table_cites_each_entry_in_the_grammar() {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-old.md",
                &entry_created("open", "2026-10-01", ""),
            ),
        ]);
        let md = render_markdown(&rows(&tree, &k("l"), false).unwrap());
        assert!(md.starts_with("| Entry | Kind | Status | Created | Closed |"));
        assert!(md.contains("[[2026-10-01-old]] t"));
        let cites = grammar::extract(&md);
        assert_eq!(cites.len(), 1);
    }

    #[test]
    fn a_row_carries_the_entry_and_its_class_and_serialises() {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-w.md",
                &entry_created(
                    "waiting",
                    "2026-10-01",
                    "revisit_by: 2026-12-01\nowner: sam\n",
                ),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-02-x.md",
                &entry_created("fixed", "2026-10-02", "closed: 2026-10-03\n"),
            ),
        ]);
        let r = rows(&tree, &k("l"), false).unwrap();
        let (x, w) = (&r[0], &r[1]);
        assert_eq!(x.key, k("2026-10-02-x"));
        assert_eq!(
            (
                x.entry_kind.as_str(),
                x.title.as_str(),
                x.status.as_str(),
                x.class,
                x.closed,
                x.revisit_by,
                x.archived
            ),
            (
                "f",
                "t",
                "fixed",
                Some(StatusClass::Terminal),
                Some(crate::librarian::lifecycle::testkit::d("2026-10-03")),
                None,
                false
            )
        );
        assert_eq!(w.class, Some(StatusClass::Blocked));
        assert_eq!(
            w.revisit_by,
            Some(crate::librarian::lifecycle::testkit::d("2026-12-01"))
        );
        assert_eq!(w.fields["owner"], serde_json::json!("sam"));

        let json = serde_json::to_value(w).unwrap();
        assert_eq!(json["key"], "2026-10-01-w");
        assert_eq!(json["class"], "blocked");
        assert_eq!(json["created"], "2026-10-01");
        assert_eq!(json["closed"], serde_json::Value::Null);
        assert_eq!(json["archived"], false);
        assert_eq!(json["fields"]["owner"], "sam");
    }

    #[test]
    fn the_table_prints_the_closed_date_and_cites_each_row() {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-b.md",
                &entry_created("fixed", "2026-10-01", "closed: 2026-10-04\n"),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry_created("open", "2026-10-01", ""),
            ),
        ]);
        let r = rows(&tree, &k("l"), false).unwrap();
        let md = render_markdown(&r);
        assert!(md.contains("| [[2026-10-01-a]] t | f | open | 2026-10-01 |  |"));
        assert!(md.contains("| [[2026-10-01-b]] t | f | fixed | 2026-10-01 | 2026-10-04 |"));
        assert_eq!(grammar::extract(&md).len(), 2);
    }

    /// The keys of `rows`, in order.
    fn keys(rows: &[EntryRow]) -> Vec<&str> {
        rows.iter().map(|r| r.key.as_str()).collect()
    }

    #[test]
    fn rows_of_one_day_sort_by_key_not_by_path() {
        // Path order puts `...-a-b.md` before `...-a.md` (`-` is 0x2D, `.` is 0x2E); key order is the opposite.
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a-b.md",
                &entry_created("open", "2026-10-01", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-a.md",
                &entry_created("open", "2026-10-01", ""),
            ),
        ]);
        let r = rows(&tree, &k("l"), false).unwrap();
        assert_eq!(keys(&r), ["2026-10-01-a", "2026-10-01-a-b"]);
    }

    #[test]
    fn a_live_and_an_archived_entry_of_one_day_sort_by_key_not_by_path() {
        // Path order puts the live `l/2026-...` before `l/archive/2026-...`; key order is the opposite.
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/l/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/l/2026-10-01-z.md",
                &entry_created("open", "2026-10-01", ""),
            ),
            doc(
                "docs/trackers/ledgers/l/archive/2026-10-01-a.md",
                &entry_created("fixed", "2026-10-01", "closed: 2026-10-02\n"),
            ),
        ]);
        let r = rows(&tree, &k("l"), true).unwrap();
        assert_eq!(keys(&r), ["2026-10-01-a", "2026-10-01-z"]);
        assert_eq!((r[0].archived, r[1].archived), (true, false));
    }

    #[test]
    fn two_nodes_with_one_collection_key_both_contribute_and_each_keeps_its_own_classes() {
        // The same word `parked` is `blocked` in the ledger `c` and `deferred` in the stream `c`.
        let ledger = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, parked: blocked, fixed: terminal}\ninitial_status: open\nreview_every: 30d\n---\n";
        let stream = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, parked: deferred, fixed: terminal}\ninitial_status: open\nexit_when: it ships\n---\n";
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/c/readme.md", ledger),
            doc(
                "docs/trackers/ledgers/c/2026-10-01-p.md",
                &entry_created("parked", "2026-10-01", "revisit_by: 2026-12-01\n"),
            ),
            doc("docs/trackers/streams/c/readme.md", stream),
            doc(
                "docs/trackers/streams/c/2026-10-02-q.md",
                &entry_created("parked", "2026-10-02", "revisit_by: 2026-12-01\n"),
            ),
        ]);
        assert_eq!(tree.collections.len(), 2);
        let r = rows(&tree, &k("c"), false).unwrap();
        assert_eq!(keys(&r), ["2026-10-02-q", "2026-10-01-p"]);
        assert_eq!(r[0].class, Some(StatusClass::Deferred));
        assert_eq!(r[1].class, Some(StatusClass::Blocked));
    }

    #[test]
    fn an_archived_collection_has_no_rows_until_the_archive_is_asked_for() {
        let tree = LifecycleTree::build([
            doc("docs/trackers/ledgers/archive/old/readme.md", LEDGER),
            doc(
                "docs/trackers/ledgers/archive/old/2026-10-01-a.md",
                &entry_created("open", "2026-10-01", ""),
            ),
        ]);
        let old = k("old");
        assert_eq!(rows(&tree, &old, false).unwrap().len(), 0);
        let all = rows(&tree, &old, true).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].archived);
    }

    #[test]
    fn an_empty_collection_renders_the_header_only() {
        let tree = LifecycleTree::build([doc("docs/trackers/ledgers/l/readme.md", LEDGER)]);
        let r = rows(&tree, &k("l"), false).unwrap();
        assert!(r.is_empty());
        let md = render_markdown(&r);
        assert_eq!(md.lines().count(), 2);
        assert!(grammar::extract(&md).is_empty());
    }

    #[test]
    fn a_title_with_a_pipe_stays_in_one_cell() {
        let md = table_of(&titled("\"a | b\""));
        let row = md.lines().nth(2).unwrap();
        assert!(row.contains("a \\| b"), "{row}");
        assert_eq!(separators(row), 6, "{row}");
        assert_eq!(cells(&md), 10, "{md}");
    }

    #[test]
    fn a_title_with_an_escaped_pipe_of_its_own_stays_in_one_cell() {
        // The title is `a\|b`: the backslash must not turn the pipe escape into an escaped backslash.
        let md = table_of(&titled("\"a\\\\|b\""));
        let row = md.lines().nth(2).unwrap();
        assert_eq!(cells(&md), 10, "{row}");
    }

    #[test]
    fn a_title_that_starts_with_a_backslash_still_makes_no_citation() {
        // The title is `\[[x]]`: every `[` is escaped, so the backslash opens no link.
        let md = table_of(&titled("\"\\\\[[x]]\""));
        let cites = grammar::extract(&md);
        assert_eq!(cites.len(), 1, "{md}");
    }

    #[test]
    fn a_title_with_a_wiki_link_makes_no_extra_citation() {
        let md = table_of(&titled("\"see [[other-doc]] now\""));
        let cites = grammar::extract(&md);
        assert_eq!(cites.len(), 1, "{md}");
        assert!(matches!(cites[0].reference, Reference::File { .. }));
        assert!(
            md.contains("[[2026-10-01-old]] see \\[\\[other-doc]] now"),
            "{md}"
        );
    }

    #[test]
    fn a_title_with_a_newline_stays_on_one_line() {
        let md = table_of(&titled("\"first\\nsecond\\rthird\""));
        assert_eq!(md.lines().count(), 3, "{md:?}");
        assert!(
            md.contains("[[2026-10-01-old]] first second third"),
            "{md:?}"
        );
        assert_eq!(cells(&md), 10);
    }

    #[test]
    fn the_other_text_cells_are_escaped_too() {
        let row = EntryRow {
            key: k("2026-10-01-a"),
            entry_kind: "k|[[x]]".into(),
            title: "t".into(),
            status: "s|[[y]]\n".into(),
            class: None,
            created: crate::librarian::lifecycle::testkit::d("2026-10-01"),
            closed: None,
            revisit_by: None,
            archived: false,
            fields: Default::default(),
        };
        let md = render_markdown(&[row]);
        assert_eq!(md.lines().count(), 3, "{md:?}");
        assert_eq!(grammar::extract(&md).len(), 1, "{md}");
        assert_eq!(cells(&md), 10, "{md}");
    }

    /// Prints a census of this repository's tracked markdown under the lifecycle model. Run by hand.
    #[test]
    #[ignore = "reads this repository's tracked markdown and prints; run by hand with --nocapture"]
    fn census_of_this_repo_under_the_lifecycle_model() {
        use crate::librarian::lifecycle::layout::{place, Lifecycle, TRACKERS_DIR};
        use crate::librarian::lifecycle::tree::{DocInput, TreeProblem};
        use std::collections::BTreeMap;

        let root = env!("CARGO_MANIFEST_DIR");
        let listed = std::process::Command::new("git")
            .args(["ls-files", "-z", "*.md"])
            .current_dir(root)
            .output()
            .expect("git ls-files runs");
        assert!(
            listed.status.success(),
            "git ls-files failed: {}",
            String::from_utf8_lossy(&listed.stderr)
        );
        let paths: Vec<String> = String::from_utf8_lossy(&listed.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();

        // Only files below the tracker root can be placed; the others are not read.
        let prefix = format!("{TRACKERS_DIR}/");
        let (mut docs, mut unreadable, mut flat_legacy, mut other_legacy) = (Vec::new(), 0, 0, 0);
        for p in paths.iter().filter(|p| p.starts_with(&prefix)) {
            if place(p).is_none() {
                if p[prefix.len()..].contains('/') {
                    other_legacy += 1;
                } else {
                    flat_legacy += 1;
                }
            }
            match std::fs::read_to_string(std::path::Path::new(root).join(p)) {
                Ok(text) => docs.push(DocInput {
                    rel_path: p.clone(),
                    text,
                }),
                Err(_) => unreadable += 1,
            }
        }
        let read = docs.len();
        let tree = LifecycleTree::build(docs);

        let mut per_lifecycle: BTreeMap<&str, usize> = BTreeMap::new();
        for c in &tree.collections {
            *per_lifecycle.entry(c.lifecycle.dir_name()).or_default() += 1;
        }
        let entries: usize = tree.collections.iter().map(|c| c.entries.len()).sum();
        let mut problems: BTreeMap<&str, usize> = BTreeMap::new();
        for p in &tree.problems {
            let name = match p {
                TreeProblem::Placement(_) => "Placement",
                TreeProblem::NestedCollection(_) => "NestedCollection",
                TreeProblem::DuplicateReadme(_) => "DuplicateReadme",
                TreeProblem::OrphanEntry(_) => "OrphanEntry",
                TreeProblem::Schema { .. } => "Schema",
                TreeProblem::Entry { .. } => "Entry",
                TreeProblem::Frontmatter { .. } => "Frontmatter",
            };
            *problems.entry(name).or_default() += 1;
        }

        println!("census: tracked .md files: {}", paths.len());
        println!("census: read {read} document(s) under {prefix} ({unreadable} unreadable)");
        for lifecycle in [Lifecycle::Ledger, Lifecycle::Stream, Lifecycle::Temp] {
            let n = per_lifecycle
                .get(lifecycle.dir_name())
                .copied()
                .unwrap_or(0);
            println!("census: collections in {}: {n}", lifecycle.dir_name());
        }
        println!("census: entries: {entries}");
        println!("census: tree problems: {}", tree.problems.len());
        for (name, n) in &problems {
            println!("census:   {name}: {n}");
        }
        println!("census: legacy docs/trackers/*.md outside the lifecycle folders: {flat_legacy}");
        println!("census: legacy files below docs/trackers/<subfolder>/ outside the lifecycle folders: {other_legacy}");
        assert!(read > 0, "the walk read no document");
    }
}
