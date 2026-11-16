//! An event as committed markdown: one `###` section under the document's `## Events` heading.
//!
//! The catalog keeps an event as a row. Plan C2 writes the same fact into the document that owns
//! it, so Plan D can export old rows into this format and Plan E can stop reading the rows. This
//! module renders, parses and splices that text and does no I/O.
//!
//! ```text
//! ### note — 2026-11-14T06:30:00Z — alice
//! <!-- event {"id":"…","kind":"note","created_at_ms":…,"author":"alice"} -->
//!
//! > first text line
//! > second text line
//! ```
//!
//! - The heading is for people. The parser never reads it. It holds the kind in lowercase, so it
//!   never has the ledger entry heading shape `<ID> — <title>`.
//! - The comment holds the event's fields on one line, as JSON. Every `>` in it is written as
//!   a backslash followed by `u003e` (six characters), so no field can close the comment.
//!   JSON reads it back to the
//!   same value.
//! - A `note` whose payload is exactly `{"text": <string>}` is a blockquote, one `> ` per line (an
//!   empty line is `>`). A blockquote cannot act as structure: no heading, label line or fence in
//!   the text starts at column 0. Any other payload is one fenced block with the info string
//!   `event-payload`, which holds pretty JSON; the fence is longer than any backtick run in it.
//!   A text with a lone carriage return also takes this form, because CommonMark reads a lone CR
//!   as a line end and would then see the rest of that line at column 0.
//! - `mutates` is one line, `**Mutates:** [[a]] [[b]]`, right after the comment. It is absent when
//!   the list is empty.

use crate::librarian::frontmatter;
use crate::util::markdown_fence::FenceState;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The heading that is the home of every event section of a document.
pub(crate) const EVENTS_HEADING: &str = "## Events";

/// The label of the line that names what an event changed.
const MUTATES_LABEL: &str = "**Mutates:**";

/// The info string of the fenced block that holds a payload which is not a plain note text.
const PAYLOAD_INFO: &str = "event-payload";

/// One event as the text twin of a catalog row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EventText {
    pub id: String,
    pub kind: String,
    pub created_at_ms: i64,
    pub author: Option<String>,
    pub anchor_commit: Option<String>,
    pub head_commit: Option<String>,
    pub parent: Option<String>,
    pub resolves: Option<String>,
    /// The event's source: kind, uri and the source's payload.
    pub source: Option<EventSource>,
    /// Grammar forms such as `[[key]]`, each one token without whitespace.
    pub mutates: Vec<String>,
    pub payload: Value,
}

/// The fields of the comment line, in the order they are written.
#[derive(Serialize, Deserialize)]
struct CommentJson {
    id: String,
    kind: String,
    created_at_ms: i64,
    #[serde(default)]
    author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resolves: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<EventSource>,
}

/// The source of an event, as the comment line writes it: the `sources` row's kind and uri, and its
/// payload when it has one. A comment written without `payload` reads back with `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EventSource {
    pub kind: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

/// The text a heading shows for `s`: control characters (line ends included) become spaces, so
/// no value can start a second line.
pub(crate) fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || c == '\u{2028}' || c == '\u{2029}' {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn timestamp(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("{ms}ms"))
}

/// True when `s` holds a `\r` that no `\n` follows.
fn has_lone_cr(s: &str) -> bool {
    s.match_indices('\r')
        .any(|(i, _)| s.as_bytes().get(i + 1) != Some(&b'\n'))
}

/// The text of a note whose payload is exactly `{"text": <string>}` and that a blockquote can
/// carry; `None` for any other payload.
fn plain_note_text(e: &EventText) -> Option<&str> {
    if e.kind != "note" {
        return None;
    }
    let map = e.payload.as_object()?;
    if map.len() != 1 {
        return None;
    }
    let text = map.get("text")?.as_str()?;
    (!has_lone_cr(text)).then_some(text)
}

/// The fence for a fenced block that holds `content`: the longest run of backticks in it plus one,
/// at least three, so no line of `content` can close the block.
pub(crate) fn fence_for(content: &str) -> String {
    let longest = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat((longest + 1).max(3))
}

/// The section for `e`, ending in one newline, with no blank line before or after it.
pub(crate) fn render_event_section(e: &EventText) -> String {
    let comment = CommentJson {
        id: e.id.clone(),
        kind: e.kind.clone(),
        created_at_ms: e.created_at_ms,
        author: e.author.clone(),
        anchor_commit: e.anchor_commit.clone(),
        head_commit: e.head_commit.clone(),
        parent: e.parent.clone(),
        resolves: e.resolves.clone(),
        source: e.source.clone(),
    };
    let json = serde_json::to_string(&comment)
        .expect("strings, one integer and a JSON value always serialize")
        .replace('>', "\\u003e");
    let author = e
        .author
        .as_deref()
        .map(one_line)
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());

    let mut out = format!(
        "### {} — {} — {}\n<!-- event {} -->\n",
        one_line(&e.kind.to_lowercase()),
        timestamp(e.created_at_ms),
        author,
        json
    );
    if !e.mutates.is_empty() {
        out.push_str(&format!("{MUTATES_LABEL} {}\n", e.mutates.join(" ")));
    }
    out.push('\n');
    match plain_note_text(e) {
        Some(text) => {
            for line in text.split('\n') {
                if line.is_empty() {
                    out.push_str(">\n");
                } else {
                    out.push_str(&format!("> {line}\n"));
                }
            }
        }
        None => {
            let pretty =
                serde_json::to_string_pretty(&e.payload).expect("a JSON value always serializes");
            let fence = fence_for(&pretty);
            out.push_str(&format!("{fence}{PAYLOAD_INFO}\n{pretty}\n{fence}\n"));
        }
    }
    out
}

/// One line of a body, with whether a code fence covers it.
struct Line<'a> {
    start: usize,
    /// The line without its `\n`; a `\r` stays.
    text: &'a str,
    /// A fence delimiter or a line inside a fence.
    fenced: bool,
}

fn scan(body: &str) -> Vec<Line<'_>> {
    let mut fence = FenceState::new();
    let mut start = 0;
    body.split_inclusive('\n')
        .map(|raw| {
            let text = raw.strip_suffix('\n').unwrap_or(raw);
            let line_start = start;
            start += raw.len();
            let delimiter = fence.feed(text.trim_start());
            Line {
                start: line_start,
                text,
                fenced: delimiter || fence.in_fence(),
            }
        })
        .collect()
}

/// `(level, text)` of an ATX heading line outside a fence.
fn heading_of<'a>(line: &Line<'a>) -> Option<(usize, &'a str)> {
    if line.fenced {
        return None;
    }
    let t = line.text.trim_start();
    let level = t.bytes().take_while(|b| *b == b'#').count();
    if level == 0 || level > 6 || t.as_bytes().get(level) != Some(&b' ') {
        return None;
    }
    Some((level, t[level + 1..].trim()))
}

/// The payload that starts `lines`: a blockquote of note text, or an `event-payload` fence.
fn payload_of(lines: &[Line<'_>]) -> Option<Value> {
    let first = lines.first()?;
    if first.text.starts_with('>') {
        let quoted: Vec<&str> = lines
            .iter()
            .take_while(|l| l.text.starts_with('>'))
            .map(|l| {
                let rest = &l.text[1..];
                rest.strip_prefix(' ').unwrap_or(rest)
            })
            .collect();
        return Some(json!({ "text": quoted.join("\n") }));
    }
    let opener = first.text.trim_end_matches('\r');
    let ticks = opener.bytes().take_while(|b| *b == b'`').count();
    if ticks < 3 || &opener[ticks..] != PAYLOAD_INFO {
        return None;
    }
    let close = lines[1..].iter().position(|l| {
        let t = l.text.trim();
        t.len() >= ticks && t.bytes().all(|b| b == b'`')
    })?;
    let json: Vec<&str> = lines[1..1 + close].iter().map(|l| l.text).collect();
    serde_json::from_str(&json.join("\n")).ok()
}

/// The event that the lines of one `###` section, heading excluded, hold; `None` when they are
/// not in this format.
fn read_section(lines: &[Line<'_>]) -> Option<EventText> {
    let comment = lines.first()?.text.trim_end_matches('\r');
    let json = comment.strip_prefix("<!-- event ")?.strip_suffix(" -->")?;
    let meta: CommentJson = serde_json::from_str(json).ok()?;
    let mut rest = &lines[1..];
    let mut mutates = Vec::new();
    if let Some(forms) = rest
        .first()
        .and_then(|l| l.text.trim().strip_prefix(MUTATES_LABEL))
    {
        mutates = forms.split_whitespace().map(str::to_owned).collect();
        rest = &rest[1..];
    }
    let blank = rest.iter().take_while(|l| l.text.trim().is_empty()).count();
    let payload = payload_of(&rest[blank..])?;
    Some(EventText {
        id: meta.id,
        kind: meta.kind,
        created_at_ms: meta.created_at_ms,
        author: meta.author,
        anchor_commit: meta.anchor_commit,
        head_commit: meta.head_commit,
        parent: meta.parent,
        resolves: meta.resolves,
        source: meta.source,
        mutates,
        payload,
    })
}

/// The body of `doc`, frontmatter excluded; the whole text when its frontmatter does not parse.
fn body_lenient(doc: &str) -> &str {
    frontmatter::parse(doc).map(|(_, b)| b).unwrap_or(doc)
}

/// Where one event section sits in its document: 1-based lines of the whole document, frontmatter
/// included, numbered as `grammar::extract` numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventSpan {
    /// The `###` heading line.
    pub heading_line: u32,
    /// The line before the next heading of level 3 or less, or the document's last line.
    pub end_line: u32,
}

/// Every event section in `doc`, in document order, with its span. A `###` heading counts when its
/// next line is an `<!-- event {json} -->` comment and the section holds a payload in one of the
/// two forms. A section that does not read back is skipped.
pub(crate) fn parse_event_sections_spanned(doc: &str) -> Vec<(EventText, EventSpan)> {
    let body = body_lenient(doc);
    // `body` is a suffix of `doc` in both arms of `body_lenient`, which `body_line_offset` needs.
    let offset = frontmatter::body_line_offset(doc, body);
    let lines = scan(body);
    let mut out = Vec::new();
    for (ix, line) in lines.iter().enumerate() {
        if !matches!(heading_of(line), Some((3, _))) {
            continue;
        }
        let end = lines[ix + 1..]
            .iter()
            .position(|l| matches!(heading_of(l), Some((level, _)) if level <= 3))
            .map_or(lines.len(), |p| ix + 1 + p);
        if let Some(event) = read_section(&lines[ix + 1..end]) {
            let span = EventSpan {
                heading_line: (offset + ix + 1) as u32,
                end_line: (offset + end) as u32,
            };
            out.push((event, span));
        }
    }
    out
}

/// Every event section in `doc`, in document order: [`parse_event_sections_spanned`] without the
/// spans.
pub(crate) fn parse_event_sections(doc: &str) -> Vec<EventText> {
    parse_event_sections_spanned(doc)
        .into_iter()
        .map(|(event, _)| event)
        .collect()
}

/// The title of a level-2 home heading: `Events` for `## Events`.
fn home_title(home: &str) -> &str {
    home.trim_start_matches('#').trim()
}

/// `doc` with `block` (text that ends in one newline) added at the end of the section under the
/// level-2 heading `home` (for example `## Events`), after one blank line. A document with no such
/// heading gets it at the end of its body, then the block. `body` is the body of `doc` (what
/// `frontmatter::parse` returned), and the frontmatter block is left byte for byte. A heading
/// inside a code fence is not the home. It does not check that the block reads back: an unclosed
/// fence before the insertion point swallows it, and each caller checks its own block.
pub(crate) fn splice_under_home(doc: &str, body: &str, home: &str, block: &str) -> String {
    let lines = scan(body);
    let title = home_title(home);
    let at = lines.iter().position(|l| heading_of(l) == Some((2, title)));
    let new_body = match at {
        None => {
            let head = body.trim_end_matches(['\n', '\r']);
            if head.is_empty() {
                // After the closing delimiter the caller owns the leading blank line.
                let lead = if body.len() == doc.len() { "" } else { "\n" };
                format!("{lead}{home}\n\n{block}")
            } else {
                format!("{head}\n\n{home}\n\n{block}")
            }
        }
        Some(ix) => {
            let end = lines[ix + 1..]
                .iter()
                .find(|l| matches!(heading_of(l), Some((level, _)) if level <= 2))
                .map_or(body.len(), |l| l.start);
            let head = body[..end].trim_end_matches(['\n', '\r']);
            let tail = &body[end..];
            if tail.is_empty() {
                format!("{head}\n\n{block}")
            } else {
                format!("{head}\n\n{block}\n{tail}")
            }
        }
    };
    frontmatter::replace_body(doc, &new_body).unwrap_or(new_body)
}

/// The lines under the level-2 heading `home` in `body`, up to the next heading of level 2 or
/// above, with code-fence lines left out; each keeps a `\r` it ends with. Empty when `body` has no
/// such heading outside a fence.
pub(crate) fn home_lines<'a>(body: &'a str, home: &str) -> Vec<&'a str> {
    let lines = scan(body);
    let title = home_title(home);
    let Some(ix) = lines.iter().position(|l| heading_of(l) == Some((2, title))) else {
        return Vec::new();
    };
    lines[ix + 1..]
        .iter()
        .take_while(|l| !matches!(heading_of(l), Some((level, _)) if level <= 2))
        .filter(|l| !l.fenced)
        .map(|l| l.text)
        .collect()
}

/// `doc` with the section for `e` added at the end of its `## Events` section, after the events
/// already there. A document with no `## Events` heading gets one at the end of its body. The
/// frontmatter block is left byte for byte.
///
/// Refuses an `id` that `doc` already holds, a `mutates` form that is not one `[[…]]` token
/// without whitespace (such a form could start a line of its own), and a document in which the
/// new section would not read back (for example an unclosed code fence before the insertion
/// point). The check re-parses the result and compares ids only.
pub(crate) fn splice_event(doc: &str, e: &EventText) -> Result<String> {
    let (_, body) = frontmatter::parse(doc)?;
    if parse_event_sections(doc).iter().any(|x| x.id == e.id) {
        bail!("event id {} is already in this document", e.id);
    }
    if let Some(form) = e
        .mutates
        .iter()
        .find(|f| !(f.starts_with("[[") && f.ends_with("]]") && !f.contains(char::is_whitespace)))
    {
        bail!("mutates form {form:?} is not one [[…]] token without whitespace");
    }
    let out = splice_under_home(doc, body, EVENTS_HEADING, &render_event_section(e));
    // An open code fence, or other text that swallows the new section, would make the event
    // invisible to the parser, and a retry would then add it again.
    if parse_event_sections(&out)
        .iter()
        .filter(|x| x.id == e.id)
        .count()
        != 1
    {
        bail!(
            "event {} would not be readable back from this document (an unclosed code fence \
             before the insertion point is the usual cause); nothing was added",
            e.id
        );
    }
    Ok(out)
}

/// The event kinds whose fact has a text twin (an `## Events` section of a single-writer document).
pub(crate) const TWIN_KINDS: &[&str] =
    &["note", "verdict", "intent", "reviewed", "external_signal"];

/// Why events of `kind` have no text twin at all, or `None` when the kind has one. `event_create`
/// asks this before it reads the target document.
pub(crate) fn twin_kind_refusal(kind: &str) -> Option<&'static str> {
    (!TWIN_KINDS.contains(&kind)).then_some("kind has no text twin")
}

/// `None` when the document with text `doc` is a home for an event of `kind`; else the reason
/// `event_create` gives for the skipped twin: `"kind has no text twin"`, `"not a single-writer
/// document"` or `"ledger or entry document"`, judged in that order.
///
/// A home is a document whose frontmatter parses and has `single_writer: true`, and that is neither
/// a ledger (`entry_prefix`) nor an entry document (`entry_kind`): a section appended to one of
/// those would be read as an entry or as a declared-field problem. A document whose frontmatter does
/// not parse has no flag to read, so it is not single-writer.
pub(crate) fn twin_home_refusal(doc: &str, kind: &str) -> Option<&'static str> {
    if let Some(reason) = twin_kind_refusal(kind) {
        return Some(reason);
    }
    let Ok((Some(fm), _)) = frontmatter::parse(doc) else {
        return Some("not a single-writer document");
    };
    if fm.extra.get("single_writer") != Some(&Value::Bool(true)) {
        return Some("not a single-writer document");
    }
    if !crate::librarian::catalog::augmentation::raw_declared_prefixes(Some(&fm)).is_empty()
        || fm.extra.contains_key("entry_kind")
    {
        return Some("ledger or entry document");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::lifecycle::relations::relations;
    use serde_json::json;

    const AT_MS: i64 = 1_794_637_800_000; // 2026-11-14T06:30:00Z

    fn note(id: &str, text: &str) -> EventText {
        EventText {
            id: id.to_owned(),
            kind: "note".to_owned(),
            created_at_ms: AT_MS,
            author: Some("alice".to_owned()),
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: Vec::new(),
            payload: json!({ "text": text }),
        }
    }

    fn lines_starting(doc: &str, prefix: &str) -> usize {
        doc.lines().filter(|l| l.starts_with(prefix)).count()
    }

    fn ids(doc: &str) -> Vec<String> {
        parse_event_sections(doc)
            .into_iter()
            .map(|e| e.id)
            .collect()
    }
    /// `twin_home_refusal` gives the reasons `event_create` gives, in its order: the kind first,
    /// then the flag (a string `"true"`, no flag and unparsable frontmatter all read as "no flag"),
    /// then a ledger or an entry document.
    #[test]
    fn twin_home_refusal_gives_event_creates_reasons() {
        let flagged = "---\nsingle_writer: true\n---\n\n# Doc\n";
        assert_eq!(twin_home_refusal(flagged, "note"), None);
        for kind in TWIN_KINDS {
            assert_eq!(twin_home_refusal(flagged, kind), None, "{kind}");
        }
        assert_eq!(
            twin_home_refusal(flagged, "field_patch"),
            Some("kind has no text twin")
        );
        // The kind is judged before the document: a non-twin kind on a non-home says the kind.
        assert_eq!(
            twin_home_refusal("# No frontmatter\n", "status_change"),
            Some("kind has no text twin")
        );
        for (label, doc) in [
            ("no frontmatter", "# Doc\n"),
            ("no flag", "---\ntitle: x\n---\n\n# Doc\n"),
            (
                "flag as a string",
                "---\nsingle_writer: \"true\"\n---\n\n# Doc\n",
            ),
            ("flag false", "---\nsingle_writer: false\n---\n\n# Doc\n"),
            (
                "unparsable frontmatter",
                "---\nsingle_writer: [true\n---\n\n# Doc\n",
            ),
        ] {
            assert_eq!(
                twin_home_refusal(doc, "note"),
                Some("not a single-writer document"),
                "{label}"
            );
        }
        for (label, doc) in [
            (
                "entry_prefix",
                "---\nsingle_writer: true\nentry_prefix: F\n---\n\n# Ledger\n",
            ),
            (
                "entry_prefix list",
                "---\nsingle_writer: true\nentry_prefix: [F, W]\n---\n\n# Ledger\n",
            ),
            (
                "entry_kind",
                "---\nsingle_writer: true\nentry_kind: friction\n---\n\n# Entry\n",
            ),
        ] {
            assert_eq!(
                twin_home_refusal(doc, "verdict"),
                Some("ledger or entry document"),
                "{label}"
            );
        }
        // An empty prefix declares no ledger.
        assert_eq!(
            twin_home_refusal(
                "---\nsingle_writer: true\nentry_prefix: \"\"\n---\n\n# Doc\n",
                "note"
            ),
            None
        );
    }

    /// The fence of a payload block is the longest backtick run of the content plus one, never
    /// fewer than three: a run of backticks inside the payload cannot close the block.
    #[test]
    fn fence_for_is_one_longer_than_the_longest_run() {
        assert_eq!(fence_for(""), "```");
        assert_eq!(fence_for("no ticks"), "```");
        assert_eq!(fence_for("a ` b"), "```");
        assert_eq!(fence_for("a `` b"), "```");
        assert_eq!(fence_for("a ``` b"), "````");
        assert_eq!(fence_for("x ````` y ``` z"), "``````");
    }

    #[test]
    fn a_note_section_has_the_documented_layout() {
        let doc = splice_event("# T\n", &note("ev-1", "hello\n\nworld")).unwrap();
        assert_eq!(
            doc,
            concat!(
                "# T\n",
                "\n",
                "## Events\n",
                "\n",
                "### note — 2026-11-14T06:30:00Z — alice\n",
                "<!-- event {\"id\":\"ev-1\",\"kind\":\"note\",\"created_at_ms\":1794637800000,",
                "\"author\":\"alice\"} -->\n",
                "\n",
                "> hello\n",
                ">\n",
                "> world\n",
            )
        );
    }

    #[test]
    fn a_note_round_trips_through_render_and_parse() {
        let full = EventText {
            id: "ev-9".to_owned(),
            kind: "note".to_owned(),
            created_at_ms: AT_MS,
            author: Some("bob".to_owned()),
            anchor_commit: Some("a1b2c3".to_owned()),
            head_commit: Some("d4e5f6".to_owned()),
            parent: Some("ev-3".to_owned()),
            resolves: Some("ev-2".to_owned()),
            source: Some(EventSource {
                kind: "file".to_owned(),
                uri: "docs/x.md".to_owned(),
                payload: None,
            }),
            mutates: vec!["[[a]]".to_owned(), "[[b:F-1]]".to_owned()],
            payload: json!({ "text": "two\nlines" }),
        };
        let doc = splice_event("---\ntitle: T\n---\n\n# T\n", &full).unwrap();
        assert!(
            doc.contains("**Mutates:** [[a]] [[b:F-1]]\n"),
            "the mutates line follows the comment: {doc}"
        );
        assert_eq!(parse_event_sections(&doc), vec![full]);
    }

    /// A source payload of each JSON shape survives render and parse, and the comment stays one
    /// line that no payload text can close (`-->` and a line end inside a string). A renderer or a
    /// parser that drops `payload` fails the equality.
    #[test]
    fn a_source_payload_round_trips_in_every_shape() {
        for payload in [
            json!({"sha": "abc", "n": 2}),
            json!("a plain string --> with an arrow\nand a line end"),
            json!({"outer": {"list": [1, {"deep": [true, null]}], "s": "<!-- x -->"}}),
        ] {
            let mut e = note("ev-1", "x");
            e.source = Some(EventSource {
                kind: "manual".to_owned(),
                uri: "https://example.org/a".to_owned(),
                payload: Some(payload.clone()),
            });
            let doc = splice_event("# T\n", &e).unwrap();
            assert_eq!(
                lines_starting(&doc, "<!-- event "),
                1,
                "the comment stays one line: {doc}"
            );
            assert_eq!(parse_event_sections(&doc), vec![e], "{payload}: {doc}");
        }
    }

    /// A comment written before `source.payload` existed (kind and uri only) still parses, with no
    /// payload. Removing `#[serde(default)]` from the field fails it.
    #[test]
    fn a_source_without_a_payload_still_parses() {
        let doc = concat!(
            "# T\n\n## Events\n\n",
            "### note — 2026-11-14T06:30:00Z — alice\n",
            "<!-- event {\"id\":\"ev-1\",\"kind\":\"note\",\"created_at_ms\":1794637800000,",
            "\"author\":\"alice\",\"source\":{\"kind\":\"manual\",\"uri\":\"u\"}} -->\n",
            "\n> x\n",
        );
        let parsed = parse_event_sections(doc);
        assert_eq!(parsed.len(), 1, "{doc}");
        assert_eq!(
            parsed[0].source,
            Some(EventSource {
                kind: "manual".to_owned(),
                uri: "u".to_owned(),
                payload: None,
            })
        );
        let mut rendered = note("ev-1", "x");
        rendered.source = parsed[0].source.clone();
        assert!(
            !splice_event("# T\n", &rendered)
                .unwrap()
                .contains("payload"),
            "no payload writes no `payload` key"
        );
    }

    #[test]
    fn a_none_author_is_unknown_in_the_heading_and_none_after_parsing() {
        let mut e = note("ev-1", "x");
        e.author = None;
        let doc = splice_event("", &e).unwrap();
        assert!(doc.contains(" — unknown\n"), "{doc}");
        assert_eq!(parse_event_sections(&doc)[0].author, None);
    }

    #[test]
    fn a_hostile_note_keeps_the_document_structure() {
        let text = "# h\n**Cites:** [[x]]\n```\n-->\r\nend";

        let raw = format!("# t\n\n{text}\n");
        assert!(
            !relations(&raw).is_empty(),
            "control: the same text written raw does forge a relation"
        );

        let doc = splice_event("# t\n", &note("ev-1", text)).unwrap();
        assert!(relations(&doc).is_empty(), "no relation from a note: {doc}");
        assert_eq!(lines_starting(&doc, "## "), 1, "{doc}");
        assert_eq!(lines_starting(&doc, "### "), 1, "{doc}");
        assert_eq!(
            lines_starting(&doc, "<!--"),
            1,
            "the comment is the only comment line: {doc}"
        );
        assert_eq!(
            parse_event_sections(&doc)[0].payload,
            json!({ "text": text }),
            "the text comes back verbatim, CRLF included"
        );
    }

    #[test]
    fn a_hostile_author_and_id_cannot_close_the_comment_or_break_the_heading() {
        let mut e = note("i>d", "x");
        e.author = Some("x-->\n# y\r\n**Cites:** [[z]]".to_owned());
        e.parent = Some("--> <!-- ".to_owned());
        let doc = splice_event("# t\n", &e).unwrap();

        let comment = doc
            .lines()
            .find(|l| l.starts_with("<!--"))
            .expect("one comment line");
        assert_eq!(
            comment.matches("-->").count(),
            1,
            "the comment holds only its own closer: {comment}"
        );
        assert_eq!(lines_starting(&doc, "<!--"), 1, "{doc}");
        assert_eq!(lines_starting(&doc, "### "), 1, "{doc}");
        assert_eq!(lines_starting(&doc, "# "), 1, "no forged h1: {doc}");
        assert!(relations(&doc).is_empty(), "{doc}");
        let back = &parse_event_sections(&doc)[0];
        assert_eq!(back.id, "i>d");
        assert_eq!(back.author, e.author);
        assert_eq!(back.parent, e.parent);
    }

    #[test]
    fn a_lone_carriage_return_in_a_note_goes_to_a_fenced_block() {
        // CommonMark reads a lone CR as a line ending, so `# h` below would become a heading.
        let text = "a\r# h";
        let doc = splice_event("# t\n", &note("ev-1", text)).unwrap();
        assert!(!doc.contains("> a"), "no blockquote for it: {doc}");
        assert!(doc.contains("```event-payload\n"), "{doc}");
        assert_eq!(
            parse_event_sections(&doc)[0].payload,
            json!({ "text": text })
        );
    }

    #[test]
    fn a_non_note_payload_is_a_fenced_block_and_round_trips() {
        let e = EventText {
            kind: "Verdict".to_owned(),
            payload: json!({ "verdict": "go", "why": "has ``` and ````` runs", "n": [1, 2.5, null] }),
            ..note("ev-2", "")
        };
        let doc = splice_event("# t\n", &e).unwrap();
        assert!(
            doc.contains("\n``````event-payload\n"),
            "the fence is longer than the longest backtick run (5): {doc}"
        );
        assert!(
            doc.contains("### verdict — "),
            "the heading holds the kind in lowercase: {doc}"
        );
        assert_eq!(lines_starting(&doc, "## "), 1);
        let back = parse_event_sections(&doc);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].payload, e.payload);
        assert_eq!(back[0].kind, "Verdict", "the JSON keeps the kind as given");
    }

    #[test]
    fn a_note_with_more_than_a_text_field_is_a_fenced_block() {
        let e = EventText {
            payload: json!({ "text": "x", "extra": 1 }),
            ..note("ev-1", "")
        };
        let doc = splice_event("", &e).unwrap();
        assert!(doc.contains("```event-payload\n"), "{doc}");
        assert_eq!(parse_event_sections(&doc), vec![e]);
    }

    #[test]
    fn a_second_event_follows_the_first_under_one_events_heading() {
        let one = splice_event("# T\n", &note("ev-1", "first")).unwrap();
        let two = splice_event(&one, &note("ev-2", "second")).unwrap();
        assert_eq!(lines_starting(&two, "## "), 1, "{two}");
        assert_eq!(ids(&two), vec!["ev-1", "ev-2"], "call order");
        assert!(
            two.starts_with(&one),
            "the first event's bytes are untouched: {two}"
        );
    }

    #[test]
    fn an_existing_events_heading_is_the_home() {
        let base = "# T\n\nintro\n\n## Events\n\n## Other\n\ntail text\n";
        let one = splice_event(base, &note("ev-1", "first")).unwrap();
        let two = splice_event(&one, &note("ev-2", "second")).unwrap();

        assert_eq!(lines_starting(&two, "## Events"), 1, "{two}");
        assert_eq!(ids(&two), vec!["ev-1", "ev-2"]);
        let events = two.find("## Events").unwrap();
        let second = two.find("\"id\":\"ev-2\"").unwrap();
        let other = two.find("## Other").unwrap();
        assert!(
            events < second && second < other,
            "events stay in their section: {two}"
        );
        assert!(
            two.contains("> second\n\n## Other\n\ntail text\n"),
            "a blank line, then the next section intact: {two}"
        );
        assert!(
            two.starts_with("# T\n\nintro\n\n## Events\n\n### note"),
            "{two}"
        );
    }

    #[test]
    fn a_heading_inside_a_code_fence_is_not_the_events_heading() {
        let base = "# T\n\n```\n## Events\n```\n";
        let doc = splice_event(base, &note("ev-1", "x")).unwrap();
        assert_eq!(
            doc.matches("## Events").count(),
            2,
            "the fenced one stays, a real heading is added: {doc}"
        );
        assert!(doc.ends_with("> x\n"), "{doc}");
        assert!(doc.find("\n## Events\n\n###").unwrap() > doc.find("```\n## Events\n```").unwrap());
    }

    #[test]
    fn a_crlf_document_keeps_one_events_heading() {
        let base = "# T\r\n\r\n## Events\r\n";
        let one = splice_event(base, &note("ev-1", "x")).unwrap();
        let two = splice_event(&one, &note("ev-2", "y")).unwrap();
        assert_eq!(two.matches("## Events").count(), 1, "{two}");
        assert_eq!(ids(&two), vec!["ev-1", "ev-2"]);
    }

    #[test]
    fn a_duplicate_event_id_is_refused() {
        let one = splice_event("# T\n", &note("ev-1", "first")).unwrap();
        let err = splice_event(&one, &note("ev-1", "again")).unwrap_err();
        assert!(format!("{err:#}").contains("ev-1"), "{err:#}");
        assert!(
            splice_event(&one, &note("ev-2", "other")).is_ok(),
            "a different id is fine"
        );
    }

    #[test]
    fn a_mutates_form_that_could_forge_a_line_is_refused() {
        let mut e = note("ev-1", "x");
        e.mutates = vec!["[[a]]\n**Cites:** [[x]]".to_owned()];
        assert!(splice_event("# T\n", &e).is_err());
        e.mutates = vec!["not a form".to_owned()];
        assert!(splice_event("# T\n", &e).is_err());
    }

    #[test]
    fn the_frontmatter_block_stays_byte_for_byte() {
        let base = "---\ntitle: \"Kept\"\ncreated: {YYYY-MM-DD}\n---\n\n# T\n";
        let doc = splice_event(base, &note("ev-1", "x")).unwrap();
        assert!(
            doc.starts_with(
                "---\ntitle: \"Kept\"\ncreated: {YYYY-MM-DD}\n---\n\n# T\n\n## Events\n"
            ),
            "{doc}"
        );
        assert_eq!(ids(&doc), vec!["ev-1"]);
    }

    #[test]
    fn an_event_section_defines_no_ledger_entry() {
        use crate::librarian::derived::build::parse_document;
        let rules = crate::librarian::classify::default_rules().unwrap();
        let entry_count = |doc: &str| parse_document("docs/ledger.md", doc, &rules).entries.len();

        let plain = splice_event("# Ledger\n", &note("ev-1", "F-1 — looks like an entry")).unwrap();
        assert_eq!(entry_count(&plain), 0, "{plain}");

        let with_entry = splice_event(
            "# Ledger\n\n## F-1 — a real entry\n\nbody\n",
            &note("ev-1", "x"),
        )
        .unwrap();
        assert_eq!(
            entry_count(&with_entry),
            1,
            "control: the parser is live and sees only the real entry: {with_entry}"
        );
    }
    #[test]
    fn a_document_with_an_unclosed_fence_refuses_an_event_that_would_not_read_back() {
        // The open fence runs to the end of the body, so anything spliced after it is code text.
        let unclosed = "# T\n\n```\nunclosed\n";
        let err = splice_event(unclosed, &note("ev-1", "x")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("ev-1"), "{msg}");
        assert!(msg.contains("would not be readable back"), "{msg}");

        let events_after = "# T\n\n```\nunclosed\n\n## Events\n";
        let err = splice_event(events_after, &note("ev-1", "x")).unwrap_err();
        assert!(
            format!("{err:#}").contains("would not be readable back"),
            "{err:#}"
        );

        // A closed fence before the insertion point is no problem.
        let closed = "# T\n\n```\nclosed\n```\n";
        let doc = splice_event(closed, &note("ev-1", "x")).unwrap();
        assert_eq!(ids(&doc), vec!["ev-1"]);
    }
    /// A span counts the lines of the whole document, frontmatter included, as `grammar::extract`
    /// numbers them: the heading line of a section is the 1-based index of its `###` line in
    /// `doc.lines()`, and its last line is the one before the next heading of level 3 or less.
    #[test]
    fn spanned_sections_carry_whole_document_lines() {
        // mutation: drop the frontmatter offset (every line is then off by three).
        let first = render_event_section(&note("e-1", "one"));
        let second = render_event_section(&note("e-2", "two\nlines"));
        let doc = format!(
            "---\ntitle: T\n---\n# Doc\n\n## Events\n\n{first}{second}\n## Later\n\ntext\n"
        );
        let line_of = |needle: &str| doc.lines().position(|l| l == needle).unwrap() as u32 + 1;
        let headings: Vec<u32> = doc
            .lines()
            .enumerate()
            .filter(|(_, l)| l.starts_with("### "))
            .map(|(i, _)| i as u32 + 1)
            .collect();
        assert_eq!(headings.len(), 2, "the fixture has two sections");

        let spanned = parse_event_sections_spanned(&doc);

        assert_eq!(
            spanned
                .iter()
                .map(|(e, s)| (e.id.as_str(), s.heading_line, s.end_line))
                .collect::<Vec<_>>(),
            vec![
                ("e-1", headings[0], headings[1] - 1),
                ("e-2", headings[1], line_of("## Later") - 1),
            ]
        );
        // `parse_event_sections` is the same parse without the spans.
        assert_eq!(
            parse_event_sections(&doc),
            spanned.into_iter().map(|(e, _)| e).collect::<Vec<_>>()
        );
    }

    /// The last section of a document that has no later heading ends at the document's last line,
    /// and a document without frontmatter has no offset.
    #[test]
    fn the_last_section_ends_at_the_last_line() {
        let section = render_event_section(&note("e-1", "one"));
        let doc = format!("## Events\n\n{section}");
        let spanned = parse_event_sections_spanned(&doc);
        assert_eq!(spanned.len(), 1);
        assert_eq!(spanned[0].1.heading_line, 3);
        assert_eq!(spanned[0].1.end_line, doc.lines().count() as u32);
    }
}
