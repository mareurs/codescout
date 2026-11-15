//! The hot view of a collection: the live entries that want attention now.
//!
//! An entry is hot when it is `open` (its status class) or when it is `blocked` or `deferred` and
//! its `revisit_by` date has passed (`entry::revisit_due`). The view ranks the hot entries, caps
//! them at the collection's `hot_cap`, flags the stale ones (an `open` entry idle for longer than
//! `stale_after`) and renders the result.
//!
//! Pure: it reads an already-loaded collection and a date, and reads no file.

use std::fmt;
use std::fmt::Write as _;

use chrono::NaiveDate;
use serde::Serialize;

use super::collection::LoadedCollection;
use super::entry::revisit_due;
use super::schema::StatusClass;
use super::table::{self, cell, EntryRow};
use crate::librarian::identity::key::Key;

/// The scope of the view, in the header line of [`render_markdown`] and as the `scope` of the JSON
/// the `hot-view` command prints: what the view covers and what it did not check.
pub const SCOPE: &str = "this collection only; revisit and stale flags; no doctor check ran";

/// Why an entry is hot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HotReason {
    /// Its status class is `open`.
    Open,
    /// It is `blocked` or `deferred` and its `revisit_by` is before today; the date is
    /// `row.revisit_by`.
    RevisitDue,
}

/// One hot entry, with the flags the view adds to its row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HotEntry {
    /// The entry's table row.
    pub row: EntryRow,
    /// Why the entry is hot.
    pub reason: HotReason,
    /// An `open` entry idle for longer than the collection's `stale_after`.
    pub stale: bool,
    /// Days from `status_since` to today; negative when `status_since` is after today.
    pub idle_days: i64,
}

/// The hot view of one collection.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HotView {
    /// The key of the collection.
    #[serde(serialize_with = "super::table::key_as_str")]
    pub collection: Key,
    /// Repo-relative folder of the collection.
    pub dir: String,
    /// At most `cap` entries, in rank order.
    pub entries: Vec<HotEntry>,
    /// Hot entries before the cap.
    pub hot: usize,
    /// `hot - entries.len()`.
    pub hidden: usize,
    /// Hidden entries that are revisit due or stale.
    pub hidden_flagged: usize,
    /// Live (not archived) entries of the collection, hot or not. An entry file the tree could not
    /// parse is not an entry of the tree, so it is counted in neither `live` nor `unclassified`
    /// (the CLI reports `problems` for it).
    pub live: usize,
    /// Live entries the view cannot judge: an undeclared status, or no `revisit_by` on a blocked
    /// or deferred entry.
    pub unclassified: usize,
    /// The collection's `hot_cap`; `None` means no cap.
    pub cap: Option<u32>,
    /// The collection's `stale_after` in days; `None` means no stale flag.
    pub stale_after_days: Option<u32>,
}

/// An archived collection has no hot view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedCollection {
    /// Repo-relative folder of the archived collection.
    pub dir: String,
}

impl fmt::Display for ArchivedCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the collection at {} is archived; an archived collection has no hot view",
            self.dir
        )
    }
}

impl std::error::Error for ArchivedCollection {}

/// The hot view of `loaded` as of `today`.
///
/// Rank: revisit-due entries first, then stale `open` entries, then the other `open` entries;
/// inside a group `status_since` descending, then key ascending. `hot_cap` keeps the first
/// `hot_cap` of that order.
pub fn hot_view(
    loaded: &LoadedCollection,
    today: NaiveDate,
) -> Result<HotView, ArchivedCollection> {
    let node = loaded.node();
    if node.archived {
        return Err(ArchivedCollection {
            dir: loaded.dir.clone(),
        });
    }
    let cap = node.schema.hot_cap;
    let stale_after_days = node.schema.stale_after_days;
    let rows = table::rows(&loaded.tree, &loaded.key, false)
        .expect("a LoadedCollection is only built from a tree that holds its collection");
    let live = rows.len();

    let mut unclassified = 0;
    // (rank group, entry): group 0 revisit due, 1 stale open, 2 other open.
    let mut hot: Vec<(u8, HotEntry)> = Vec::new();
    for row in rows {
        let Some(class) = row.class else {
            unclassified += 1;
            continue;
        };
        let idle_days = (today - row.status_since).num_days();
        match class {
            StatusClass::Open => {
                let stale = stale_after_days.is_some_and(|n| idle_days > i64::from(n));
                let group = if stale { 1 } else { 2 };
                hot.push((
                    group,
                    HotEntry {
                        row,
                        reason: HotReason::Open,
                        stale,
                        idle_days,
                    },
                ));
            }
            StatusClass::Blocked | StatusClass::Deferred => {
                if revisit_due(class, row.revisit_by, today).is_some() {
                    hot.push((
                        0,
                        HotEntry {
                            row,
                            reason: HotReason::RevisitDue,
                            stale: false,
                            idle_days,
                        },
                    ));
                } else if row.revisit_by.is_none() {
                    unclassified += 1;
                }
            }
            StatusClass::Terminal => {}
        }
    }
    hot.sort_by(|(ga, a), (gb, b)| {
        ga.cmp(gb)
            .then_with(|| b.row.status_since.cmp(&a.row.status_since))
            .then_with(|| a.row.key.cmp(&b.row.key))
    });
    let mut entries: Vec<HotEntry> = hot.into_iter().map(|(_, e)| e).collect();
    let hot_count = entries.len();
    let keep = cap.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let hidden_entries = entries.split_off(keep.min(hot_count));
    let hidden_flagged = hidden_entries
        .iter()
        .filter(|e| e.stale || e.reason == HotReason::RevisitDue)
        .count();
    Ok(HotView {
        collection: loaded.key.clone(),
        dir: loaded.dir.clone(),
        entries,
        hot: hot_count,
        hidden: hidden_entries.len(),
        hidden_flagged,
        live,
        unclassified,
        cap,
        stale_after_days,
    })
}

/// The view as the markdown the `hot-view` command prints: a one-line header, one line per shown
/// entry and, when their counts are above zero, a line for the hidden and one for the
/// unclassified entries. Every line ends with a newline.
pub fn render_markdown(view: &HotView) -> String {
    let mut out = String::new();
    // `write!` to a `String` cannot fail.
    let _ = writeln!(
        out,
        "hot view: {} ({}): {} of {} hot, {} live. Scope: {SCOPE}.",
        view.collection,
        view.dir,
        view.entries.len(),
        view.hot,
        view.live
    );
    for e in &view.entries {
        let _ = write!(out, "- [[{}]] {}: ", e.row.key, cell(&e.row.title));
        match e.reason {
            HotReason::Open => {
                let _ = write!(out, "open since {}", e.row.status_since);
                if e.stale {
                    let _ = write!(out, ", STALE {}d", e.idle_days);
                }
            }
            HotReason::RevisitDue => {
                let due = e.row.revisit_by.map(|d| d.to_string()).unwrap_or_default();
                let _ = write!(out, "{}, revisit was due {due}", e.row.status);
            }
        }
        out.push('\n');
    }
    if view.hidden > 0 {
        let cap = view.cap.map(|c| c.to_string()).unwrap_or_default();
        let _ = writeln!(
            out,
            "({} more hot entries hidden by hot_cap {cap}, {} of them flagged)",
            view.hidden, view.hidden_flagged
        );
    }
    if view.unclassified > 0 {
        let _ = writeln!(
            out,
            "({} live entries cannot be judged: an undeclared status, or no revisit_by on a blocked or deferred entry)",
            view.unclassified
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::NaiveDate;

    use super::*;
    use crate::librarian::lifecycle::collection::{load_collection_dir, LoadedCollection};
    use crate::librarian::lifecycle::testkit::d;

    const DIR: &str = "docs/trackers/ledgers/l";

    fn today() -> NaiveDate {
        d("2026-11-11")
    }

    /// A ledger readme declaring `{open: open, waiting: blocked, later: deferred, fixed: terminal}`;
    /// `extra` is more frontmatter lines (`hot_cap`, `stale_after`).
    fn readme(extra: &str) -> String {
        format!("---\ntitle: L\nentry_kinds: [f]\nstatuses: {{open: open, waiting: blocked, later: deferred, fixed: terminal}}\ninitial_status: open\n{extra}---\n")
    }

    /// An entry of kind `f` whose `created` and `status_since` are set independently: the shared
    /// helpers write both equal, and with such a fixture `table::rows` already returns the ranked
    /// order, so a wrong sort would pass. `extra` is more frontmatter lines.
    fn entry(status: &str, created: &str, status_since: &str, extra: &str) -> String {
        format!("---\nentry_kind: f\ntitle: t\nstatus: {status}\nstatus_since: {status_since}\ncreated: {created}\n{extra}---\n")
    }

    /// The collection at `dir`, loaded from a temp directory holding `readme_text` and the entries
    /// (`(path under dir without .md, text)`), as `collection/tests.rs` does.
    fn loaded_at(dir: &str, readme_text: &str, entries: &[(&str, String)]) -> LoadedCollection {
        let t = tempfile::tempdir().unwrap();
        let write = |rel: String, text: &str| {
            let path = t.path().join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(format!("{dir}/readme.md"), readme_text);
        for (stem, text) in entries {
            write(format!("{dir}/{stem}.md"), text);
        }
        load_collection_dir(t.path(), dir).unwrap()
    }

    fn loaded(readme_text: &str, entries: &[(&str, String)]) -> LoadedCollection {
        loaded_at(DIR, readme_text, entries)
    }

    fn view(readme_text: &str, entries: &[(&str, String)]) -> HotView {
        hot_view(&loaded(readme_text, entries), today()).unwrap()
    }

    fn keys(v: &HotView) -> Vec<String> {
        v.entries.iter().map(|e| e.row.key.to_string()).collect()
    }

    #[test]
    fn open_and_due_entries_are_hot_and_the_rest_are_not() {
        let v = view(
            &readme(""),
            &[
                ("a-open", entry("open", "2026-10-20", "2026-10-20", "")),
                (
                    "b-due",
                    entry(
                        "waiting",
                        "2026-09-01",
                        "2026-09-01",
                        "revisit_by: 2026-11-01\n",
                    ),
                ),
                (
                    "c-wait",
                    entry(
                        "waiting",
                        "2026-09-01",
                        "2026-09-01",
                        "revisit_by: 2026-12-01\n",
                    ),
                ),
                (
                    "d-later-due",
                    entry(
                        "later",
                        "2026-09-15",
                        "2026-09-15",
                        "revisit_by: 2026-11-05\n",
                    ),
                ),
                (
                    "e-fixed",
                    entry("fixed", "2026-10-01", "2026-10-30", "closed: 2026-10-30\n"),
                ),
            ],
        );
        assert_eq!(keys(&v), ["d-later-due", "b-due", "a-open"]);
        let reasons: Vec<HotReason> = v.entries.iter().map(|e| e.reason).collect();
        assert_eq!(
            reasons,
            [
                HotReason::RevisitDue,
                HotReason::RevisitDue,
                HotReason::Open
            ]
        );
        assert_eq!(v.entries[0].row.revisit_by, Some(d("2026-11-05")));
        assert_eq!((v.hot, v.hidden, v.hidden_flagged), (3, 0, 0));
        assert_eq!((v.live, v.unclassified), (5, 0));
        assert_eq!(v.cap, None);
    }

    #[test]
    fn a_blocked_entry_due_today_is_not_hot() {
        let v = view(
            &readme(""),
            &[(
                "due-today",
                entry(
                    "waiting",
                    "2026-09-01",
                    "2026-09-01",
                    "revisit_by: 2026-11-11\n",
                ),
            )],
        );
        assert!(v.entries.is_empty());
        assert_eq!((v.hot, v.live, v.unclassified), (0, 1, 0));
    }

    #[test]
    fn order_follows_status_since_not_created_and_ties_break_by_key() {
        // `table::rows` sorts by `created` and gives `n`, `m`, `p`: a sort by `created`, and a sort
        // with no key tiebreak, both fail here.
        let v = view(
            &readme(""),
            &[
                ("p", entry("open", "2026-10-01", "2026-10-25", "")),
                ("m", entry("open", "2026-10-05", "2026-10-20", "")),
                ("n", entry("open", "2026-10-06", "2026-10-20", "")),
            ],
        );
        assert_eq!(keys(&v), ["p", "m", "n"]);
    }

    #[test]
    fn no_hot_cap_shows_every_hot_entry() {
        let entries: Vec<(String, String)> = (0..30)
            .map(|i| {
                (
                    format!("open-{i:02}"),
                    entry("open", "2026-10-01", "2026-10-01", ""),
                )
            })
            .collect();
        let refs: Vec<(&str, String)> = entries
            .iter()
            .map(|(k, t)| (k.as_str(), t.clone()))
            .collect();
        let v = view(&readme(""), &refs);
        assert_eq!(v.cap, None);
        assert_eq!(v.entries.len(), 30);
        assert_eq!((v.hot, v.hidden), (30, 0));
    }

    fn capped_fixture() -> Vec<(&'static str, String)> {
        vec![
            ("old-stale", entry("open", "2026-09-01", "2026-09-01", "")),
            (
                "old-due",
                entry(
                    "waiting",
                    "2026-09-10",
                    "2026-09-10",
                    "revisit_by: 2026-11-01\n",
                ),
            ),
            ("new-a", entry("open", "2026-11-09", "2026-11-09", "")),
            ("new-b", entry("open", "2026-11-10", "2026-11-10", "")),
        ]
    }

    #[test]
    fn a_cap_keeps_the_flagged_entries_and_counts_the_flagged_it_hides() {
        let two = view(&readme("stale_after: 30d\nhot_cap: 2\n"), &capped_fixture());
        assert_eq!(keys(&two), ["old-due", "old-stale"]);
        assert_eq!((two.hot, two.hidden, two.hidden_flagged), (4, 2, 0));
        assert_eq!(two.cap, Some(2));

        let one = view(&readme("stale_after: 30d\nhot_cap: 1\n"), &capped_fixture());
        assert_eq!(keys(&one), ["old-due"]);
        assert_eq!((one.hot, one.hidden, one.hidden_flagged), (4, 3, 1));
    }

    #[test]
    fn a_hidden_revisit_due_entry_counts_as_flagged() {
        // Two due entries and one fresh open entry under `hot_cap: 1`: the due entry with the later
        // `status_since` is shown; the older due entry and the open one are hidden. No stale entry
        // is hidden (the open one is idle 1 day against 30), so only the due half of the
        // `hidden_flagged` rule can make the count 1.
        let v = view(
            &readme("stale_after: 30d\nhot_cap: 1\n"),
            &[
                (
                    "due-newer",
                    entry(
                        "waiting",
                        "2026-09-01",
                        "2026-09-20",
                        "revisit_by: 2026-11-01\n",
                    ),
                ),
                (
                    "due-older",
                    entry(
                        "waiting",
                        "2026-09-01",
                        "2026-09-10",
                        "revisit_by: 2026-11-02\n",
                    ),
                ),
                ("fresh", entry("open", "2026-11-10", "2026-11-10", "")),
            ],
        );
        assert_eq!(keys(&v), ["due-newer"]);
        assert_eq!((v.hot, v.hidden, v.hidden_flagged), (3, 2, 1));
        assert!(v.entries.iter().all(|e| !e.stale));
    }

    #[test]
    fn stale_flags_only_an_open_entry_past_stale_after() {
        let entries = [
            ("edge-open", entry("open", "2026-10-12", "2026-10-12", "")),
            ("over-open", entry("open", "2026-10-11", "2026-10-11", "")),
            (
                "old-due",
                entry(
                    "waiting",
                    "2026-10-01",
                    "2026-10-01",
                    "revisit_by: 2026-11-01\n",
                ),
            ),
        ];
        let v = view(&readme("stale_after: 30d\n"), &entries);
        let by_key = |key: &str| {
            v.entries
                .iter()
                .find(|e| e.row.key.to_string() == key)
                .unwrap()
        };
        assert_eq!(by_key("edge-open").idle_days, 30);
        assert!(!by_key("edge-open").stale, "idle 30 is not past 30");
        assert_eq!(by_key("over-open").idle_days, 31);
        assert!(by_key("over-open").stale);
        assert_eq!(by_key("old-due").idle_days, 41);
        assert!(!by_key("old-due").stale, "only an open entry is stale");
        assert_eq!(v.stale_after_days, Some(30));
        // Stale ranks before the other open entries.
        assert_eq!(keys(&v), ["old-due", "over-open", "edge-open"]);

        let plain = view(&readme(""), &entries);
        assert!(plain.entries.iter().all(|e| !e.stale));
        assert_eq!(plain.stale_after_days, None);
    }

    #[test]
    fn a_huge_stale_after_does_not_panic_and_flags_nothing() {
        let v = view(
            &readme("stale_after: 4294967295d\n"),
            &[("old", entry("open", "2026-01-01", "2026-01-01", ""))],
        );
        assert_eq!(v.stale_after_days, Some(u32::MAX));
        assert_eq!(v.entries.len(), 1);
        assert!(!v.entries[0].stale);
    }

    #[test]
    fn a_status_since_after_today_is_not_stale_and_has_negative_idle_days() {
        // `5d` and an idle of -9: a stale test on the absolute value would flag it.
        let v = view(
            &readme("stale_after: 5d\n"),
            &[("future", entry("open", "2026-11-01", "2026-11-20", ""))],
        );
        assert_eq!(v.entries[0].idle_days, -9);
        assert!(!v.entries[0].stale);
    }

    #[test]
    fn an_archived_entry_is_neither_hot_nor_live() {
        let v = view(
            &readme(""),
            &[
                ("a-open", entry("open", "2026-10-20", "2026-10-20", "")),
                (
                    "archive/gone-open",
                    entry("open", "2026-09-01", "2026-09-01", ""),
                ),
            ],
        );
        assert_eq!(keys(&v), ["a-open"]);
        assert_eq!((v.hot, v.live), (1, 1));
    }

    #[test]
    fn an_archived_collection_has_no_hot_view() {
        let dir = "docs/trackers/ledgers/archive/old";
        let l = loaded_at(
            dir,
            &readme(""),
            &[("a-open", entry("open", "2026-10-20", "2026-10-20", ""))],
        );
        let err = hot_view(&l, today()).unwrap_err();
        assert_eq!(err, ArchivedCollection { dir: dir.into() });
        assert_eq!(
            err.to_string(),
            "the collection at docs/trackers/ledgers/archive/old is archived; an archived collection has no hot view"
        );
    }

    #[test]
    fn a_live_entry_with_an_undeclared_status_is_unclassified_and_not_hot() {
        let v = view(
            &readme(""),
            &[("odd", entry("mystery", "2026-10-01", "2026-10-01", ""))],
        );
        assert!(v.entries.is_empty());
        assert_eq!((v.hot, v.live, v.unclassified), (0, 1, 1));
    }

    #[test]
    fn a_blocked_entry_without_revisit_by_is_unclassified_and_not_hot() {
        let v = view(
            &readme(""),
            &[
                ("stuck", entry("waiting", "2026-10-01", "2026-10-01", "")),
                ("parked", entry("later", "2026-10-01", "2026-10-01", "")),
            ],
        );
        assert!(v.entries.is_empty());
        assert_eq!((v.hot, v.live, v.unclassified), (0, 2, 2));
    }

    #[test]
    fn render_markdown_follows_the_contract() {
        let v = view(
            &readme("stale_after: 30d\nhot_cap: 2\n"),
            &[
                ("old-stale", entry("open", "2026-09-01", "2026-09-01", "")),
                (
                    "due",
                    entry(
                        "waiting",
                        "2026-09-10",
                        "2026-09-10",
                        "revisit_by: 2026-11-01\n",
                    ),
                ),
                ("new", entry("open", "2026-11-10", "2026-11-10", "")),
                ("odd", entry("mystery", "2026-10-01", "2026-10-01", "")),
            ],
        );
        let expected = "\
hot view: l (docs/trackers/ledgers/l): 2 of 3 hot, 4 live. Scope: this collection only; revisit and stale flags; no doctor check ran.
- [[due]] t: waiting, revisit was due 2026-11-01
- [[old-stale]] t: open since 2026-09-01, STALE 71d
(1 more hot entries hidden by hot_cap 2, 0 of them flagged)
(1 live entries cannot be judged: an undeclared status, or no revisit_by on a blocked or deferred entry)
";
        assert_eq!(render_markdown(&v), expected);

        // Nothing hidden, nothing unclassified: the header and the entry lines only.
        let plain = view(
            &readme(""),
            &[("new", entry("open", "2026-11-10", "2026-11-10", ""))],
        );
        assert_eq!(
            render_markdown(&plain),
            "\
hot view: l (docs/trackers/ledgers/l): 1 of 1 hot, 1 live. Scope: this collection only; revisit and stale flags; no doctor check ran.
- [[new]] t: open since 2026-11-10
"
        );
    }

    #[test]
    fn a_title_with_a_pipe_and_a_newline_stays_on_one_line() {
        let text = "---\nentry_kind: f\ntitle: \"a | b\\nc\"\nstatus: open\nstatus_since: 2026-11-10\ncreated: 2026-11-10\n---\n";
        let v = view(&readme(""), &[("x", text.to_string())]);
        assert_eq!(v.entries[0].row.title, "a | b\nc");
        let md = render_markdown(&v);
        let line = md.lines().nth(1).unwrap();
        assert_eq!(line, "- [[x]] a \\| b c: open since 2026-11-10");
        assert_eq!(md.lines().count(), 2, "{md}");
    }

    #[test]
    fn the_view_serialises_with_the_reason_in_snake_case_and_the_collection_as_text() {
        let v = view(
            &readme(""),
            &[(
                "due",
                entry(
                    "waiting",
                    "2026-09-10",
                    "2026-09-10",
                    "revisit_by: 2026-11-01\n",
                ),
            )],
        );
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["collection"], "l");
        assert_eq!(json["dir"], DIR);
        assert_eq!(json["entries"][0]["reason"], "revisit_due");
        assert_eq!(json["entries"][0]["row"]["revisit_by"], "2026-11-01");
        assert_eq!(json["entries"][0]["stale"], false);
        assert_eq!(json["cap"], serde_json::Value::Null);
    }
}
