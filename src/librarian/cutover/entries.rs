//! Params entries and the other params of an augmentation (D-R13, D-R23).
//!
//! A params entry is an element of the augmentation's entry collection. When the document's body
//! defines no heading for its token, [`recover_params_entries`] gives it a section; when it does,
//! the entry is already exported, or skipped when its params fields are not in the section's text.
//! Everything else in params has no committed home: [`other_params`] names it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::librarian::catalog::augmentation::{
    body_entry_heading_level, entry_high_water_key, render_pending_section, AugmentationRow,
    PendingSection,
};
use crate::librarian::catalog::dual_write::frontmatter_mark;
use crate::librarian::catalog::event_text::{fence_for, one_line, EVENTS_HEADING};
use crate::librarian::catalog::link_text::RELATIONS_HEADING;
use crate::librarian::cutover::plan::{Class, Item, RewriteAdded, Status};
use crate::librarian::frontmatter;
use crate::librarian::preview::headings;
use crate::librarian::tools::link_scan::extract::entry_sections;

/// The info string of the fenced block that holds an entry's params.
const ENTRY_INFO: &str = "params-entry";

pub const REASON_ENTRY_RECOVERED: &str = "section recovered from params";
/// The reason of an entry whose section would not read back: an unclosed code fence before the
/// insertion point swallows it, so every run would add it again.
pub const REASON_SECTION_UNREADABLE: &str =
    "the section would not read back (an unclosed code fence before it?)";
pub const REASON_ENTRY_DEFINED: &str = "entry is defined in the text";
pub const REASON_NOT_A_TOKEN: &str = "entry id is not a token";
pub const REASON_NOT_AN_OBJECT: &str = "params entry is not an object";
pub const REASON_NOT_AN_ARRAY: &str = "the entry collection is not an array";
pub const REASON_FRONTMATTER: &str =
    "frontmatter does not parse; the high-water mark cannot be kept";
pub const REASON_PARAMS_NOT_JSON: &str = "params is not JSON";
pub const REASON_PARAMS_NOT_AN_OBJECT: &str = "params is not a JSON object";
/// The reason of a params key outside the entry collection: neither a sidecar nor a text holds it.
pub const REASON_PARAMS_OTHER: &str = "params outside the entry collection have no committed home";

/// What an augmentation's `params` text holds.
pub(crate) enum Params {
    /// No text, or JSON `null`.
    Empty,
    Object(Map<String, Value>),
    NotJson,
    /// JSON, but not an object.
    NotObject,
}

/// Read `row.params`. The catalog default is `{}`; an empty text and `null` carry nothing.
pub(crate) fn params_of(row: &AugmentationRow) -> Params {
    let raw = row.params.trim();
    if raw.is_empty() {
        return Params::Empty;
    }
    match serde_json::from_str::<Value>(raw) {
        Err(_) => Params::NotJson,
        Ok(Value::Null) => Params::Empty,
        Ok(Value::Object(map)) => Params::Object(map),
        Ok(_) => Params::NotObject,
    }
}

pub(super) fn item(
    rel_path: &str,
    class: Class,
    status: Status,
    reason: &str,
    detail: Option<String>,
) -> Item {
    Item {
        class,
        status,
        reason: reason.to_owned(),
        path: Some(rel_path.to_owned()),
        line: None,
        detail,
        suggestion: None,
    }
}

/// Every element of the entry collection as one `unrecoverable` item with `reason`: the document
/// cannot take them (it is frozen, or not in `HEAD`). The detail is the entry's id when it has one.
pub fn refuse_entries(rel_path: &str, row: &AugmentationRow, reason: &str) -> Vec<Item> {
    let Params::Object(params) = params_of(row) else {
        return Vec::new();
    };
    let Some(collection) = row.entry_collection.as_deref() else {
        return Vec::new();
    };
    let Some(value) = params.get(collection) else {
        return Vec::new();
    };
    let Some(entries) = value.as_array() else {
        let reported = item(
            rel_path,
            Class::ParamsEntry,
            Status::Unrecoverable,
            REASON_NOT_AN_ARRAY,
            None,
        );
        return vec![reported];
    };
    entries
        .iter()
        .map(|entry| {
            let id = entry.get("id").map(id_text);
            item(
                rel_path,
                Class::ParamsEntry,
                Status::Unrecoverable,
                reason,
                id,
            )
        })
        .collect()
}

/// The text of an `id` value for a report line: a string as it is, anything else as JSON.
fn id_text(id: &Value) -> String {
    match id {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `(prefix, number)` of a token `[A-Z]{1,3}-<digits>`; `None` for any other text.
fn parse_token(id: &str) -> Option<(&str, u64)> {
    let (prefix, digits) = id.split_once('-')?;
    let letters = (1..=3).contains(&prefix.len()) && prefix.bytes().all(|b| b.is_ascii_uppercase());
    let number = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    if !(letters && number) {
        return None;
    }
    Some((prefix, digits.parse().ok()?))
}

/// The text each entry token owns, by token: the sections the index reads ([`entry_sections`],
/// the reader of [`defined_tokens`](crate::librarian::identity::index::defined_tokens)), so a
/// token is a key exactly when the index reads the text as defining it. Frontmatter, a line inside
/// a code fence and a line of seven or more `#` define nothing. A line inside a nested section is
/// owned by the innermost one; a token defined twice owns both regions.
fn section_texts(text: &str) -> BTreeMap<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    // Sections come in document order, so a later section that covers a line is nested in the
    // earlier ones that do: it overwrites them.
    let mut owners: Vec<Option<String>> = vec![None; lines.len()];
    for section in entry_sections(text) {
        let first = section.heading_line as usize - 1;
        let last = (section.end_line as usize).min(lines.len());
        for owner in &mut owners[first..last] {
            *owner = Some(section.id.clone());
        }
    }
    let mut sections: BTreeMap<String, String> = BTreeMap::new();
    for (line, owner) in lines.iter().zip(owners) {
        if let Some(token) = owner {
            let section = sections.entry(token).or_default();
            section.push_str(line);
            section.push('\n');
        }
    }
    sections
}

/// A value that says nothing: `null`, an empty string, array or object.
fn says_nothing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// The `params-entry` block of a section (its JSON object) and the prose lines of the section:
/// every line outside the block, outside heading lines and outside the `**Valid:**` line.
fn block_and_prose(section: &str) -> (Option<Map<String, Value>>, Vec<&str>) {
    let mut block: Option<Map<String, Value>> = None;
    let mut prose = Vec::new();
    // `Some((fence length, collected JSON text))` while inside the block.
    let mut open: Option<(usize, String)> = None;
    for line in section.lines() {
        if let Some((fence, json)) = open.as_mut() {
            let ticks = line.trim();
            if ticks.len() >= *fence && ticks.bytes().all(|b| b == b'`') {
                if block.is_none() {
                    block = serde_json::from_str::<Value>(json)
                        .ok()
                        .and_then(|v| v.as_object().cloned());
                }
                open = None;
            } else {
                json.push_str(line);
                json.push('\n');
            }
            continue;
        }
        let trimmed = line.trim_start();
        let ticks = trimmed.bytes().take_while(|&b| b == b'`').count();
        if ticks >= 3 && trimmed[ticks..].trim() == ENTRY_INFO {
            open = Some((ticks, String::new()));
        } else if !trimmed.starts_with('#') && !trimmed.starts_with("**Valid:**") {
            prose.push(line);
        }
    }
    (block, prose)
}

/// Whether `line` holds `token` as a whole token: not inside a longer word, number or id.
fn holds_token(line: &str, token: &str) -> bool {
    line.match_indices(token).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + token.len()..].chars().next();
        let word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
        let digit_dot = |dot: Option<char>, beyond: Option<char>| {
            dot == Some('.') && beyond.is_some_and(|c| c.is_ascii_digit())
        };
        let before_ok =
            !before.is_some_and(word) && !digit_dot(before, line[..at].chars().rev().nth(1));
        let tail = line[at + token.len()..].chars().nth(1);
        let after_ok = !after.is_some_and(word) && !digit_dot(after, tail);
        before_ok && after_ok
    })
}

/// Whether a prose line says `key` and `value`: the key name in the line, and the value as text
/// (a string) or as a whole token (anything else).
fn line_says(line: &str, key: &str, value: &Value) -> bool {
    line.contains(key)
        && match value {
            Value::String(s) => line.contains(s.as_str()),
            other => holds_token(line, &other.to_string()),
        }
}

/// The fields of `entry` (other than `id`) that `section` does not hold, sorted. A field is held
/// when the section's `params-entry` block has the key with an equal value, or one prose line
/// (outside the heading and the validity stamp) says the key and the value. A value that says
/// nothing is always held.
fn missing_fields(entry: &Map<String, Value>, section: &str) -> Vec<String> {
    let (block, prose) = block_and_prose(section);
    let held = |key: &String, value: &Value| {
        block.as_ref().and_then(|b| b.get(key)) == Some(value)
            || prose.iter().any(|line| line_says(line, key, value))
    };
    let missing: BTreeSet<&String> = entry
        .iter()
        .filter(|(key, value)| key.as_str() != "id" && !says_nothing(value))
        .filter(|(key, value)| !held(key, value))
        .map(|(key, _)| key)
        .collect();
    missing.into_iter().cloned().collect()
}

/// The heading an entry gets: its `title`, else its `summary`, else its id, on one line.
fn title_of(entry: &Map<String, Value>, token: &str) -> String {
    for key in ["title", "summary"] {
        if let Some(text) = entry.get(key).and_then(Value::as_str) {
            let line = one_line(text);
            if !line.is_empty() {
                return line;
            }
        }
    }
    token.to_owned()
}

/// The byte offset in `doc` of the first `## Events` or `## Relations` heading, outside code
/// fences and outside the frontmatter block.
fn home_offset(doc: &str) -> Option<usize> {
    let body = frontmatter::parse(doc).map_or(doc, |(_, body)| body);
    let body_start = doc.len() - body.len();
    let heading = headings::parse(body).into_iter().find(|h| {
        h.level == 2
            && matches!(
                format!("## {}", h.text).as_str(),
                EVENTS_HEADING | RELATIONS_HEADING
            )
    })?;
    let before: usize = body
        .split_inclusive('\n')
        .take(heading.line - 1)
        .map(str::len)
        .sum();
    Some(body_start + before)
}

/// `doc` with `section` before the first `## Events` or `## Relations` heading, else at the end,
/// with a blank line between it and the text before it. At the end only the line ends of `doc` go:
/// its last line keeps its bytes (trailing spaces too), so the step changes no line of `doc` and
/// [`rewrite_added`](super::rewrite::rewrite_added) reads only the section as added.
fn insert_section(doc: &str, section: &str) -> String {
    match home_offset(doc) {
        Some(at) => {
            let (head, tail) = doc.split_at(at);
            let lead = if head.is_empty() || head.ends_with("\n\n") {
                ""
            } else if head.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{head}{lead}{section}{tail}")
        }
        None => {
            let head = doc.trim_end_matches(['\n', '\r']);
            if head.trim_end().is_empty() {
                format!("{}\n", section.trim_end())
            } else {
                format!("{head}\n\n{}\n", section.trim_end())
            }
        }
    }
}

/// Give every params entry of `row` whose token `text` does not define a section, and report each
/// entry (D-R13). Returns the new text and one item per entry, in the order of the collection; the
/// citation items of a recovered section follow its entry's item.
///
/// An entry whose token the text defines is `already_exported` when its fields are in the section,
/// and `skipped` with the fields it lacks otherwise. A field is in the section when the section's
/// `params-entry` block holds the key with an equal value, or when one prose line of the section
/// (outside its headings and its `**Valid:**` line) holds the key and the value; a value that says
/// nothing is always in. The section of a recovered entry is a `params-entry` block holding the
/// entry's JSON without `id` (keys sorted), under the heading and validity stamp of a live write.
/// `rewrite` reads the old citation forms of the new section in the context of the whole text: the
/// heading holds the entry's title, where no form is rewritten, so they are reported here as the
/// plan of the commit that holds the section reports them (D-R9).
///
/// A section that would not read back as the token's section (an unclosed code fence before it
/// swallows it) is not written: the entry is `unrecoverable`, so a rerun does not add it again.
/// `entry_high_water_<P>` rises to the highest recovered number when the document has a
/// frontmatter block, and never falls.
pub fn recover_params_entries(
    rel_path: &str,
    text: &str,
    row: &AugmentationRow,
    rewrite: &RewriteAdded,
) -> (String, Vec<Item>) {
    let unchanged = || (text.to_owned(), Vec::new());
    let Some(collection) = row.entry_collection.as_deref() else {
        return unchanged();
    };
    let Params::Object(params) = params_of(row) else {
        return unchanged();
    };
    let Some(value) = params.get(collection) else {
        return unchanged();
    };
    let Some(entries) = value.as_array() else {
        let reported = item(
            rel_path,
            Class::ParamsEntry,
            Status::Unrecoverable,
            REASON_NOT_AN_ARRAY,
            None,
        );
        return (text.to_owned(), vec![reported]);
    };
    // A block that exists and does not parse hides its mark: a line editor would lower
    // `entry_high_water_<P>` to the recovered number. The text stays as it is.
    if frontmatter::parse(text).is_err() {
        return (
            text.to_owned(),
            refuse_entries(rel_path, row, REASON_FRONTMATTER),
        );
    }

    let mut doc = text.to_owned();
    let mut sections = section_texts(&doc);
    let mut marks: BTreeMap<String, u64> = BTreeMap::new();
    let mut items = Vec::with_capacity(entries.len());
    let report = |status, reason: &str, detail: Option<String>| {
        item(rel_path, Class::ParamsEntry, status, reason, detail)
    };
    for entry in entries {
        let Some(entry) = entry.as_object() else {
            items.push(report(Status::Unrecoverable, REASON_NOT_AN_OBJECT, None));
            continue;
        };
        let id = entry.get("id");
        let Some((token, (prefix, number))) = id
            .and_then(Value::as_str)
            .and_then(|s| parse_token(s).map(|parsed| (s, parsed)))
        else {
            items.push(report(
                Status::Unrecoverable,
                REASON_NOT_A_TOKEN,
                id.map(id_text),
            ));
            continue;
        };
        let detail = Some(token.to_owned());

        if let Some(section) = sections.get(token) {
            let missing = missing_fields(entry, section);
            items.push(if missing.is_empty() {
                report(Status::AlreadyExported, REASON_ENTRY_DEFINED, detail)
            } else {
                let reason = format!("entry exists; fields not in text: {}", missing.join(", "));
                report(Status::Skipped, &reason, detail)
            });
            continue;
        }

        let payload: BTreeMap<&String, &Value> = entry
            .iter()
            .filter(|(key, _)| key.as_str() != "id")
            .collect();
        let json = serde_json::to_string(&payload).expect("a JSON value always serializes");
        let fence = fence_for(&json);
        let pending = PendingSection {
            title: title_of(entry, token),
            body: format!("{fence}{ENTRY_INFO}\n{json}\n{fence}\n"),
            anchor_heading: String::new(),
            index_row: None,
        };
        let level = body_entry_heading_level(&doc, prefix).unwrap_or(2);
        match render_pending_section(token, level, &pending) {
            Ok(section) => {
                let (next, cites) = rewrite(&doc, &insert_section(&doc, &section));
                let next_sections = section_texts(&next);
                if !next_sections.contains_key(token) {
                    items.push(report(
                        Status::Unrecoverable,
                        REASON_SECTION_UNREADABLE,
                        detail,
                    ));
                    continue;
                }
                doc = next;
                sections = next_sections;
                let mark = marks.entry(prefix.to_owned()).or_default();
                *mark = (*mark).max(number);
                items.push(report(Status::Moved, REASON_ENTRY_RECOVERED, detail));
                items.extend(cites);
            }
            Err(e) => {
                let reason = format!("section cannot be rendered: {e}");
                items.push(report(Status::Unrecoverable, &reason, detail));
            }
        }
    }

    for (prefix, number) in marks {
        if number > frontmatter_mark(&doc, &prefix).unwrap_or(0) {
            if let Some(raised) =
                frontmatter::upsert_int_line(&doc, &entry_high_water_key(&prefix), number)
            {
                doc = raised;
            }
        }
    }
    (doc, items)
}

/// One `params_other` item for the params keys outside the entry collection (every key when the
/// augmentation has no collection), with the key names, or `None` when there are none (D-R23).
/// Params that are not JSON, or not an object, are one item as well. The item is `unrecoverable`:
/// a sidecar has no params field and a text has no home for them.
pub fn other_params(rel_path: &str, row: &AugmentationRow) -> Option<Item> {
    let report = |reason: &str, detail: Option<String>| {
        item(
            rel_path,
            Class::ParamsOther,
            Status::Unrecoverable,
            reason,
            detail,
        )
    };
    match params_of(row) {
        Params::Empty => None,
        Params::NotJson => Some(report(REASON_PARAMS_NOT_JSON, None)),
        Params::NotObject => Some(report(REASON_PARAMS_NOT_AN_OBJECT, None)),
        Params::Object(params) => {
            let collection = row.entry_collection.as_deref();
            let mut keys: Vec<&str> = params
                .keys()
                .map(String::as_str)
                .filter(|key| Some(*key) != collection)
                .collect();
            if keys.is_empty() {
                return None;
            }
            keys.sort_unstable();
            Some(report(REASON_PARAMS_OTHER, Some(keys.join(", "))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::dual_write::frontmatter_mark;
    use crate::librarian::cutover::plan::{Class, Status};

    fn row(params: &str, collection: Option<&str>) -> AugmentationRow {
        AugmentationRow {
            artifact_id: "a".into(),
            prompt: "keep it current".into(),
            params: params.into(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: collection.map(str::to_owned),
            refreshed_at_commit: None,
        }
    }

    /// [`recover_params_entries`] with a rewrite that changes nothing and finds nothing.
    fn recover(rel_path: &str, text: &str, row: &AugmentationRow) -> (String, Vec<Item>) {
        let keep = |_: &str, after: &str| (after.to_owned(), Vec::new());
        recover_params_entries(rel_path, text, row, &keep)
    }

    /// I2: a document that ends inside an unclosed code fence swallows a section added at its end.
    /// The entry is reported and the text is not touched, so a rerun is a fixed point.
    #[test]
    fn an_entry_whose_section_would_land_in_an_unclosed_fence_is_unrecoverable() {
        let row = row(r#"{"e":[{"id":"F-2","title":"two"}]}"#, Some("e"));
        let text = format!("{LEDGER}\n```\nunclosed\n");
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(out, text, "the text is not touched");
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].status, Status::Unrecoverable);
        assert_eq!(items[0].reason, REASON_SECTION_UNREADABLE);
        assert_eq!(items[0].detail.as_deref(), Some("F-2"));
        let (again, items) = recover("l.md", &out, &row);
        assert_eq!(again, out, "a rerun adds nothing");
        assert_eq!(items[0].status, Status::Unrecoverable);
    }

    #[test]
    fn the_citation_items_of_a_recovered_section_follow_its_entry() {
        let row = row(r#"{"e":[{"id":"F-2","title":"see old-form"}]}"#, Some("e"));
        let cite = |before: &str, after: &str| {
            // One item when the added lines hold the form (the heading and the block both do).
            let found =
                usize::from(after.matches("old-form").count() > before.matches("old-form").count());
            let items = (0..found)
                .map(|_| {
                    item(
                        "l.md",
                        Class::Citation,
                        Status::Unrecoverable,
                        "inside a heading",
                        None,
                    )
                })
                .collect();
            (after.to_owned(), items)
        };
        let (out, items) = recover_params_entries("l.md", LEDGER, &row, &cite);
        assert!(out.contains("F-2 \u{2014} see old-form"), "{out}");
        let classes: Vec<Class> = items.iter().map(|i| i.class).collect();
        assert_eq!(classes, [Class::ParamsEntry, Class::Citation]);
    }

    /// A row whose entry collection `entries` holds the JSON array `entries`.
    fn entries_row(entries: &str) -> AugmentationRow {
        row(&format!(r#"{{"entries":{entries}}}"#), Some("entries"))
    }

    fn today() -> String {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    }

    fn statuses(items: &[Item]) -> Vec<Status> {
        items.iter().map(|i| i.status).collect()
    }

    /// The distinct entry tokens a text defines for the index, in order.
    fn defined(text: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for token in crate::librarian::identity::index::defined_tokens(text) {
            if !out.contains(&token) {
                out.push(token);
            }
        }
        out
    }

    const LEDGER: &str =
        "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n## F-1 — first\n\nbody one\n";

    #[test]
    fn an_entry_without_a_heading_gets_a_section_and_the_mark() {
        let row = entries_row(r#"[{"id":"F-1"},{"id":"F-2","title":"second","status":"open"}]"#);
        let (out, items) = recover("docs/l.md", LEDGER, &row);
        let stamp = format!("## F-2 — second\n\n**Valid:** dated {}\n\n", today());
        assert!(out.contains(&stamp), "{out}");
        assert!(
            out.contains("```params-entry\n{\"status\":\"open\",\"title\":\"second\"}\n```\n"),
            "{out}"
        );
        assert_eq!(frontmatter_mark(&out, "F"), Some(2), "{out}");
        assert!(
            out.contains("## F-1 — first\n\nbody one\n"),
            "F-1 is left alone: {out}"
        );
        assert_eq!(defined(&out), ["F-1", "F-2"]);
        assert_eq!(statuses(&items), [Status::AlreadyExported, Status::Moved]);
        assert!(items.iter().all(|i| i.class == Class::ParamsEntry));
        assert!(items.iter().all(|i| i.path.as_deref() == Some("docs/l.md")));
        assert_eq!(items[1].detail.as_deref(), Some("F-2"));
    }

    /// F1: recovery reads "the text defines the token" with the index's reader
    /// ([`defined_tokens`](crate::librarian::identity::index::defined_tokens)), and a seven-hash
    /// line is no heading for it.
    #[test]
    fn a_seven_hash_line_is_not_the_section_of_its_token() {
        let row = entries_row(r#"[{"id":"F-2","title":"two"}]"#);
        let text = format!("{LEDGER}\n####### F-2 — fake\n");
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(statuses(&items), [Status::Moved], "{items:?}");
        assert!(
            out.contains("\n####### F-2 — fake\n\n## F-2 — two\n"),
            "{out}"
        );
    }

    /// F2: the index reads only the body, so a frontmatter comment shaped like an entry heading is
    /// not the section of its token for recovery either.
    #[test]
    fn a_frontmatter_comment_is_not_the_section_of_its_token() {
        let row = entries_row(r#"[{"id":"F-2","title":"two"}]"#);
        let text = LEDGER.replace("entry_prefix: F\n", "entry_prefix: F\n## F-2 — two\n");
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(statuses(&items), [Status::Moved], "{items:?}");
        let (_, body) = frontmatter::parse(&out).unwrap();
        assert!(body.contains("\n## F-2 — two\n"), "{out}");
    }

    /// The other way round: a level-1 heading defines its token for the index, so recovery reads
    /// the entry as defined and writes no second section.
    #[test]
    fn a_level_one_heading_is_the_section_of_its_token() {
        let row = entries_row(r#"[{"id":"F-2"}]"#);
        let text = format!("{LEDGER}\n# F-2 — top\n\nx\n");
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(statuses(&items), [Status::AlreadyExported], "{items:?}");
        assert_eq!(out, text);
    }

    /// A nested entry owns its own lines: its heading defines it, and its parent's fields are not
    /// read from them.
    #[test]
    fn a_nested_entry_owns_its_lines() {
        let text = format!("{LEDGER}\n### F-2 — nested\n\nseverity: high\n");
        let row = entries_row(r#"[{"id":"F-1","severity":"high"},{"id":"F-2","severity":"high"}]"#);
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(out, text);
        assert_eq!(
            statuses(&items),
            [Status::Skipped, Status::AlreadyExported],
            "{items:?}"
        );
    }

    /// A heading with no line under it is the whole section of its token: the entry is defined, and
    /// no second section is written.
    #[test]
    fn a_heading_alone_is_the_section_of_its_token() {
        let row = entries_row(r#"[{"id":"F-2"},{"id":"F-3"}]"#);
        let text = format!("{LEDGER}\n## F-2 — two\n## F-3 — three");
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(out, text);
        assert_eq!(
            statuses(&items),
            [Status::AlreadyExported, Status::AlreadyExported],
            "{items:?}"
        );
    }

    #[test]
    fn a_defined_entry_names_the_fields_its_text_lacks() {
        let text = format!("{LEDGER}\n## F-2 — second\n\nsomething with high in it elsewhere\n");
        // `severity: high`: F-1's section does not hold "high", F-2's does. Another entry's text
        // does not count for F-1.
        let row = entries_row(r#"[{"id":"F-1","severity":"high"}]"#);
        let (out, items) = recover("l.md", &text, &row);
        assert_eq!(out, text);
        assert_eq!(statuses(&items), [Status::Skipped]);
        assert_eq!(
            items[0].reason,
            "entry exists; fields not in text: severity"
        );
        // The same field, now in the entry's own section, is exported.
        let held = text.replace("body one", "severity: high");
        let (out, items) = recover("l.md", &held, &row);
        assert_eq!(out, held);
        assert_eq!(statuses(&items), [Status::AlreadyExported]);
        // Every missing field is named, sorted; fields that carry nothing are not missing. A value
        // that is in the text without its key on the same line (`id2`) is missing.
        let row = entries_row(
            r#"[{"id":"F-1","zeta":"z1","alpha":3,"empty":"","none":null,"list":[],"id2":"body one"}]"#,
        );
        let (_, items) = recover("l.md", &text, &row);
        assert_eq!(
            items[0].reason,
            "entry exists; fields not in text: alpha, id2, zeta"
        );
    }

    #[test]
    fn a_value_with_backticks_gets_a_longer_fence() {
        let row = entries_row(r#"[{"id":"F-2","note":"a ``` b"}]"#);
        let (out, items) = recover("l.md", LEDGER, &row);
        assert_eq!(statuses(&items), [Status::Moved]);
        assert!(
            out.contains("\n````params-entry\n{\"note\":\"a ``` b\"}\n````\n"),
            "{out}"
        );
        assert!(!out.contains("\n```params-entry"), "{out}");
        let (again, items) = recover("l.md", &out, &row);
        assert_eq!(again, out);
        assert_eq!(statuses(&items), [Status::AlreadyExported]);
    }

    #[test]
    fn the_section_goes_before_events_or_relations() {
        let row = entries_row(r#"[{"id":"F-2","title":"t"}]"#);
        let body = "---\nentry_prefix: F\n---\n\n# L\n\n## F-1 — a\n\nx\n\n";
        let new = format!("## F-2 — t\n\n**Valid:** dated {}\n\n", today());
        for (tail, anchor) in [
            (
                "## Relations\n\n- r\n\n## Events\n\n- e\n",
                "## Relations\n",
            ),
            ("## Events\n\n- e\n\n## Relations\n\n- r\n", "## Events\n"),
        ] {
            let text = format!("{body}{tail}");
            let (out, _) = recover("l.md", &text, &row);
            let at = out.find(&new).unwrap_or_else(|| panic!("{out}"));
            let section_end = out[at..].find("```\n\n").unwrap() + at + 5;
            assert!(out[section_end..].starts_with(anchor), "{out}");
            assert!(out.find("## F-1 — a").unwrap() < at, "{out}");
        }
        // An anchor inside a code fence is not an anchor: the section goes at the end.
        let lead = "# L\n\n## F-1 — a\n\nx\n\n```\n## Events\n```\n";
        let fenced = format!("---\nentry_prefix: F\n---\n\n{lead}");
        let (out, _) = recover("l.md", &fenced, &row);
        assert!(out.contains(&format!("{lead}\n## F-2 — t\n")), "{out}");
        assert!(out.trim_end().ends_with("```"), "{out}");
        assert!(
            out.find(&new).unwrap() > out.find("## Events").unwrap(),
            "{out}"
        );
        // A body with no anchor and no trailing newline still gets a blank line before it.
        let bare = "# L\n\n## F-1 — a\n\nx";
        let (out, _) = recover("l.md", bare, &row);
        assert!(
            out.starts_with("# L\n\n## F-1 — a\n\nx\n\n## F-2 — t\n\n"),
            "{out}"
        );
        assert!(out.ends_with("```\n"), "{out}");
    }
    /// C1-b: the section goes after the text's line ends; the last line keeps its bytes.
    #[test]
    fn a_section_at_the_end_leaves_the_last_line_as_it_is() {
        assert_eq!(insert_section("a  ", "## S\n"), "a  \n\n## S\n");
        assert_eq!(insert_section("a  \r\n\n", "## S\n"), "a  \n\n## S\n");
        assert_eq!(insert_section("", "## S\n"), "## S\n");
        assert_eq!(insert_section("\n \n", "## S\n"), "## S\n");
    }

    #[test]
    fn the_heading_level_follows_the_ledger() {
        let row = entries_row(r#"[{"id":"F-3"}]"#);
        let text = "---\nentry_prefix: F\n---\n\n# L\n\n### F-1 — a\n\nx\n\n### F-2 — b\n\ny\n";
        let (out, _) = recover("l.md", text, &row);
        assert!(out.contains("\n### F-3 — F-3\n"), "{out}");
        // A ledger with no heading for the prefix has no level to follow: `##`.
        let (out, _) = recover("l.md", "# L\n\n## G-1 — a\n\nx\n", &row);
        assert!(out.contains("\n## F-3 — F-3\n"), "{out}");
    }

    #[test]
    fn a_document_without_frontmatter_gets_the_section_and_no_mark() {
        let row = entries_row(r#"[{"id":"F-2","title":"second"}]"#);
        let text = "# L\n\n## F-1 — first\n\nx\n";
        let (out, items) = recover("l.md", text, &row);
        assert!(out.starts_with(text), "{out}");
        assert!(out.contains("\n## F-2 — second\n"), "{out}");
        assert!(!out.contains("entry_high_water"), "{out}");
        assert_eq!(statuses(&items), [Status::Moved]);
    }

    #[test]
    fn the_mark_only_rises_and_is_kept_per_prefix() {
        let text = "---\nentry_prefix: [F, W]\nentry_high_water_F: 9\n---\n\n# L\n";
        let row = entries_row(r#"[{"id":"F-2"},{"id":"W-4"},{"id":"W-3"}]"#);
        let (out, _) = recover("l.md", text, &row);
        assert_eq!(
            frontmatter_mark(&out, "F"),
            Some(9),
            "a mark never drops: {out}"
        );
        assert_eq!(frontmatter_mark(&out, "W"), Some(4), "{out}");
        // Entries of one prefix keep the highest number, whatever the order.
        let row = entries_row(r#"[{"id":"W-7"},{"id":"W-3"}]"#);
        let (out, _) = recover("l.md", "---\nentry_prefix: W\n---\n", &row);
        assert_eq!(frontmatter_mark(&out, "W"), Some(7), "{out}");
    }

    #[test]
    fn an_entry_id_that_is_not_a_token_is_unrecoverable() {
        let row = entries_row(
            r#"[{"id":"nope"},{"id":7},{"title":"no id"},"a string",{"id":"f-1"},{"id":"ABCD-1"},{"id":"F-"},{"id":"F-1x"}]"#,
        );
        let (out, items) = recover("l.md", LEDGER, &row);
        assert_eq!(out, LEDGER);
        assert_eq!(items.len(), 8);
        assert!(items.iter().all(|i| i.status == Status::Unrecoverable));
        assert_eq!(items[0].reason, "entry id is not a token");
        assert_eq!(items[0].detail.as_deref(), Some("nope"));
        assert_eq!(items[1].detail.as_deref(), Some("7"));
        assert_eq!(items[2].reason, "entry id is not a token");
        assert_eq!(items[3].reason, "params entry is not an object");
    }

    #[test]
    fn the_title_falls_back_to_summary_then_the_id_and_is_one_line() {
        let row = entries_row(
            r#"[{"id":"F-2","title":"line one\nline two  more"},{"id":"F-3","summary":"from summary"},{"id":"F-4"},{"id":"F-5","title":"  ","summary":""},{"id":"F-6","title":7,"summary":"s"}]"#,
        );
        let (out, _) = recover("l.md", LEDGER, &row);
        for heading in [
            "## F-2 — line one line two  more\n",
            "## F-3 — from summary\n",
            "## F-4 — F-4\n",
            "## F-5 — F-5\n",
            "## F-6 — s\n",
        ] {
            assert!(out.contains(heading), "{heading:?} in {out}");
        }
        assert_eq!(defined(&out), ["F-1", "F-2", "F-3", "F-4", "F-5", "F-6"]);
    }

    #[test]
    fn recovering_twice_adds_nothing() {
        // Every value the JSON escapes must still be found in the text the first pass wrote.
        let row = entries_row(
            r#"[{"id":"F-2","title":"a \"quoted\" title\nsecond line","body":"back\\slash ``` fence","n":3,"obj":{"k":["v",1]}},{"id":"F-3"}]"#,
        );
        let (once, first) = recover("l.md", LEDGER, &row);
        assert_eq!(statuses(&first), [Status::Moved, Status::Moved]);
        let (twice, second) = recover("l.md", &once, &row);
        assert_eq!(twice, once);
        assert_eq!(
            statuses(&second),
            [Status::AlreadyExported, Status::AlreadyExported]
        );
    }

    #[test]
    fn an_unparseable_frontmatter_block_refuses_the_entries_and_keeps_the_mark() {
        // `title: Fix: thing` is not YAML: the mark cannot be read, and a line editor would lower
        // `entry_high_water_F: 50` to the recovered number. The text stays as it is.
        let text = "---\nkind: tracker\ntitle: Fix: thing\nentry_prefix: F\nentry_high_water_F: 50\n---\n\n# L\n\n## F-1 — a\n\nx\n";
        assert!(
            frontmatter::parse(text).is_err(),
            "the fixture must not parse"
        );
        let row = entries_row(r#"[{"id":"F-1"},{"id":"F-2","title":"t"}]"#);
        let (out, items) = recover("l.md", text, &row);
        assert_eq!(out, text);
        assert!(out.contains("entry_high_water_F: 50"));
        assert_eq!(
            statuses(&items),
            [Status::Unrecoverable, Status::Unrecoverable]
        );
        for item in &items {
            assert_eq!(item.class, Class::ParamsEntry);
            assert_eq!(
                item.reason,
                "frontmatter does not parse; the high-water mark cannot be kept"
            );
        }
        assert_eq!(items[1].detail.as_deref(), Some("F-2"));
    }

    const FIELD_DOC: &str =
        "---\nentry_prefix: F\n---\n\n# L\n\n## F-1 — x\n\n**Valid:** dated 2026-01-01\n\nprose\n";

    fn field_reasons(doc: &str, entry: &str) -> Vec<String> {
        let row = entries_row(&format!("[{entry}]"));
        let (out, items) = recover("l.md", doc, &row);
        assert_eq!(out, doc, "a defined entry never changes the text");
        items.into_iter().map(|i| i.reason).collect()
    }

    #[test]
    fn a_number_is_not_found_in_the_heading_or_the_stamp() {
        // The `1` of `F-1` and of the date must not make `severity: 1` present.
        let reasons = field_reasons(FIELD_DOC, r#"{"id":"F-1","severity":1}"#);
        assert_eq!(reasons, ["entry exists; fields not in text: severity"]);
        let reasons = field_reasons(FIELD_DOC, r#"{"id":"F-1","date":"2026-01-01"}"#);
        assert_eq!(
            reasons,
            ["entry exists; fields not in text: date"],
            "the stamp line is not prose"
        );
        let reasons = field_reasons(FIELD_DOC, r#"{"id":"F-1","note":"x"}"#);
        assert_eq!(
            reasons,
            ["entry exists; fields not in text: note"],
            "the heading line is not prose"
        );
    }

    #[test]
    fn a_field_is_present_as_a_prose_line_naming_key_and_value_or_as_its_block_entry() {
        let present = |extra: &str, entry: &str| {
            let doc = FIELD_DOC.replace("prose\n", extra);
            field_reasons(&doc, entry) == [REASON_ENTRY_DEFINED]
        };
        assert!(present("severity: 1\n", r#"{"id":"F-1","severity":1}"#));
        assert!(present(
            "severity: high\n",
            r#"{"id":"F-1","severity":"high"}"#
        ));
        assert!(present("- done: true.\n", r#"{"id":"F-1","done":true}"#));
        // Key without the value, value without the key, a longer token, a different line.
        assert!(!present("severity: 12\n", r#"{"id":"F-1","severity":1}"#));
        assert!(!present("severity: 1.5\n", r#"{"id":"F-1","severity":1}"#));
        assert!(!present("severity: F-1\n", r#"{"id":"F-1","severity":1}"#));
        assert!(!present(
            "severity: low\n",
            r#"{"id":"F-1","severity":"high"}"#
        ));
        assert!(!present("high\n", r#"{"id":"F-1","severity":"high"}"#));
        assert!(!present(
            "severity here\n\nand 1 there\n",
            r#"{"id":"F-1","severity":1}"#
        ));
        // The exported block form: the key with an equal value.
        let block = "```params-entry\n{\"severity\":1,\"tags\":[\"a\"]}\n```\n";
        assert!(present(block, r#"{"id":"F-1","severity":1,"tags":["a"]}"#));
        let other = "```params-entry\n{\"severity\":2}\n```\n";
        assert!(!present(other, r#"{"id":"F-1","severity":1}"#));
    }

    #[test]
    fn an_entry_listed_twice_is_recovered_once() {
        let row = entries_row(
            r#"[{"id":"F-2","title":"t"},{"id":"F-2","title":"t"},{"id":"F-2","title":"other"}]"#,
        );
        let (out, items) = recover("l.md", LEDGER, &row);
        assert_eq!(out.matches("\n## F-2 — ").count(), 1, "{out}");
        assert_eq!(
            statuses(&items),
            [Status::Moved, Status::AlreadyExported, Status::Skipped]
        );
        assert_eq!(items[2].reason, "entry exists; fields not in text: title");
    }

    #[test]
    fn no_entry_collection_or_no_entries_changes_nothing() {
        for row in [
            row(r#"{"entries":[{"id":"F-2"}]}"#, None),
            row(r#"{"other":1}"#, Some("entries")),
            row("{}", Some("entries")),
            row("not json", Some("entries")),
            row(r#"{"entries":[]}"#, Some("entries")),
        ] {
            let (out, items) = recover("l.md", LEDGER, &row);
            assert_eq!(out, LEDGER);
            assert!(items.is_empty(), "{items:?}");
        }
    }

    #[test]
    fn an_entry_collection_that_is_not_an_array_is_reported() {
        let row = row(r#"{"entries":{"F-2":{}}}"#, Some("entries"));
        let (out, items) = recover("l.md", LEDGER, &row);
        assert_eq!(out, LEDGER);
        assert_eq!(statuses(&items), [Status::Unrecoverable]);
        assert_eq!(items[0].class, Class::ParamsEntry);
        assert_eq!(items[0].reason, "the entry collection is not an array");
    }

    #[test]
    fn params_keys_outside_the_collection_are_reported_by_name() {
        let row = row(
            r#"{"entries":[{"id":"F-1"}],"gather_from":"x","notes":1}"#,
            Some("entries"),
        );
        let item = other_params("docs/l.md", &row).expect("two keys have no home");
        assert_eq!(item.class, Class::ParamsOther);
        assert_eq!(item.status, Status::Unrecoverable);
        assert_eq!(item.detail.as_deref(), Some("gather_from, notes"));
        assert_eq!(item.path.as_deref(), Some("docs/l.md"));
    }

    #[test]
    fn all_params_of_an_augmentation_without_a_collection_are_reported() {
        let row = row(r#"{"zeta":1,"alpha":[1,2],"entries":[]}"#, None);
        let item = other_params("l.md", &row).expect("every key has no home");
        assert_eq!(item.detail.as_deref(), Some("alpha, entries, zeta"));
    }

    #[test]
    fn empty_params_give_no_item() {
        for params in ["{}", "", "  ", "null"] {
            assert!(
                other_params("l.md", &row(params, None)).is_none(),
                "{params:?}"
            );
        }
        // Only the collection: nothing outside it.
        assert!(other_params("l.md", &entries_row("[{\"id\":\"F-1\"}]")).is_none());
    }

    #[test]
    fn params_that_are_not_an_object_are_reported_once() {
        let item = other_params("l.md", &row("{not json", None)).unwrap();
        assert_eq!(item.class, Class::ParamsOther);
        assert_eq!(item.status, Status::Unrecoverable);
        assert_eq!(item.reason, "params is not JSON");
        let item = other_params("l.md", &row("[1,2]", Some("entries"))).unwrap();
        assert_eq!(item.reason, "params is not a JSON object");
    }
}
