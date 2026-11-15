//! Events and observations as `## Events` sections of a single-writer document (D-R6, D-R12).
//!
//! The functions are pure text steps: the planner hands each one a document's text and its rows,
//! and gets the new text and the items back. A section is written with the same code as a live
//! `event_create` (`event_text::splice_event`), and the home rules are `event_text::twin_home_refusal`,
//! so a live writer and the cutover share one format and one rule.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::librarian::catalog::event_text::{
    parse_event_sections, splice_event, twin_home_refusal, EventSource, EventText,
};
use crate::librarian::cutover::entries::item;
use crate::librarian::cutover::old_rows::{OldEvent, OldObservation};
use crate::librarian::cutover::plan::{Class, Item, Status};

/// The start of the reason of a row that stays a row because its document is no home for it.
const REASON_NO_HOME: &str = "no committed home yet";
const REASON_CARRIED: &str = "carried by its note event";
const REASON_HELD: &str = "the text already holds it";
const REASON_WRITTEN: &str = "section written";
const REASON_UNNAMEABLE: &str = "a mutates target has no name in the key space";

/// `text` with the events and observations of one document written as sections, and one item per
/// row (D-R6, D-R12).
///
/// `events` are the document's events of the kinds a text can hold (the planner reports the others
/// itself); `notes_of_artifact` are its `note` events, which carry an observation whose text they
/// repeat (C2-R2: a live `note` call records the observation as well). `form_of` names a target
/// document by its grammar form, or `None` when the grammar cannot name it.
///
/// Events go in `created_at_ms` order (ties by id), then observations (the same order), after the
/// sections the text already holds, whatever the order of the input. A row is:
/// - `skipped` with `no committed home yet: <why>` when the text is no home for a `note` (an
///   unflagged document, a ledger, an entry document; `event_create`'s rules) or the kind has no
///   twin; the row stays in the old catalog;
/// - `already_exported` when the text holds a section with its id (`observation-<rowid>` for an
///   observation); `splice_event` is not called, so a rerun is quiet;
/// - `skipped` as carried, for an observation whose note is in `notes_of_artifact`;
/// - `unrecoverable` when a `mutates` target has no form (the event is not written with half its
///   targets), or `splice_event` refuses (an unclosed code fence before the insertion point is the
///   usual cause); the error text carries no path, and the text stays as it was;
/// - `moved` otherwise.
pub fn export_events(
    rel_path: &str,
    text: &str,
    events: &[&OldEvent],
    observations: &[&OldObservation],
    notes_of_artifact: &[&OldEvent],
    form_of: &dyn Fn(&str) -> Option<String>,
) -> (String, Vec<Item>) {
    let mut text = text.to_owned();
    let mut items = Vec::new();
    let mut held: BTreeMap<String, EventText> = parse_event_sections(&text)
        .into_iter()
        .map(|section| (section.id.clone(), section))
        .collect();

    for event in sorted_events(events) {
        let detail = Some(event.kind.clone());
        let report =
            |status, reason: &str| item(rel_path, Class::Event, status, reason, detail.clone());
        if let Some(why) = twin_home_refusal(&text, &event.kind) {
            items.push(report(Status::Skipped, &no_home(why)));
            continue;
        }
        if held.contains_key(&event.id) {
            items.push(report(Status::AlreadyExported, REASON_HELD));
            continue;
        }
        let Some(mutates) = event
            .mutates
            .iter()
            .map(|id| form_of(id))
            .collect::<Option<Vec<String>>>()
        else {
            items.push(report(Status::Unrecoverable, REASON_UNNAMEABLE));
            continue;
        };
        let section = EventText {
            id: event.id.clone(),
            kind: event.kind.clone(),
            created_at_ms: event.created_at_ms,
            author: event.author.clone(),
            anchor_commit: event.anchor_commit.clone(),
            head_commit: event.head_commit.clone(),
            parent: event.parent.clone(),
            resolves: event.resolves.clone(),
            source: event.source.as_ref().map(|s| EventSource {
                kind: s.kind.clone(),
                uri: s.uri.clone(),
                payload: s.payload.clone(),
            }),
            mutates,
            payload: event.payload.clone(),
        };
        let (status, reason) = write_section(&section, &mut text, &mut held);
        items.push(report(status, &reason));
    }

    for row in sorted_observations(observations) {
        let report =
            |status, reason: &str| item(rel_path, Class::Observation, status, reason, None);
        if let Some(why) = twin_home_refusal(&text, "note") {
            items.push(report(Status::Skipped, &no_home(why)));
            continue;
        }
        if notes_of_artifact.iter().any(|note| carries(note, row)) {
            items.push(report(Status::Skipped, REASON_CARRIED));
            continue;
        }
        let mut payload = json!({ "text": row.text });
        if let Some(source) = &row.source {
            payload["source"] = Value::String(source.clone());
        }
        let section = EventText {
            id: format!("observation-{}", row.id),
            kind: "note".to_owned(),
            created_at_ms: row.created_at_ms,
            author: None,
            anchor_commit: None,
            head_commit: None,
            parent: None,
            resolves: None,
            source: None,
            mutates: Vec::new(),
            payload,
        };
        if let Some(holder) = held.get(&section.id) {
            // An observation id is a rowid of one machine's catalog: a section another machine wrote
            // under the same id is another fact, so the content is compared (events carry ULIDs).
            if same_content(holder, &section) {
                items.push(report(Status::AlreadyExported, REASON_HELD));
            } else {
                let reason = format!("another section holds id {}", section.id);
                items.push(report(Status::Unrecoverable, &reason));
            }
            continue;
        }
        let (status, reason) = write_section(&section, &mut text, &mut held);
        items.push(report(status, &reason));
    }
    (text, items)
}

/// One `unrecoverable` item per event and observation of a document the plan cannot edit
/// (`reason` is why: not in `HEAD`, outside the key space, frozen).
pub fn refuse_events(
    rel_path: &str,
    events: &[&OldEvent],
    observations: &[&OldObservation],
    reason: &str,
) -> Vec<Item> {
    let events = sorted_events(events).into_iter().map(|event| {
        item(
            rel_path,
            Class::Event,
            Status::Unrecoverable,
            reason,
            Some(event.kind.clone()),
        )
    });
    let rows = sorted_observations(observations).into_iter().map(|_| {
        item(
            rel_path,
            Class::Observation,
            Status::Unrecoverable,
            reason,
            None,
        )
    });
    events.chain(rows).collect()
}

fn sorted_events<'a>(events: &[&'a OldEvent]) -> Vec<&'a OldEvent> {
    let mut sorted = events.to_vec();
    sorted.sort_by(|a, b| (a.created_at_ms, &a.id).cmp(&(b.created_at_ms, &b.id)));
    sorted
}

fn sorted_observations<'a>(rows: &[&'a OldObservation]) -> Vec<&'a OldObservation> {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|row| (row.created_at_ms, row.id));
    sorted
}

fn no_home(why: &str) -> String {
    format!("{REASON_NO_HOME}: {why}")
}

/// Whether `note` is a `note` event whose text is exactly the observation's.
fn carries(note: &OldEvent, row: &OldObservation) -> bool {
    note.kind == "note"
        && note.payload.get("text").and_then(Value::as_str) == Some(row.text.as_str())
}

/// Splice `section` into `text`: the status and reason of the item. The text is left as it was when
/// the splice is refused.
fn write_section(
    section: &EventText,
    text: &mut String,
    held: &mut BTreeMap<String, EventText>,
) -> (Status, String) {
    match splice_event(text, section) {
        Ok(next) => {
            *text = next;
            held.insert(section.id.clone(), section.clone());
            (Status::Moved, REASON_WRITTEN.to_owned())
        }
        Err(err) => (
            Status::Unrecoverable,
            format!("the section cannot be added to the text: {err:#}"),
        ),
    }
}

/// Whether the section the text holds is the one `section` would write: the payload (text and
/// source) and the creation time. The format reads a written section back exactly, whatever the
/// text (the awkward-text test pins it), so a plain comparison is enough.
fn same_content(held: &EventText, section: &EventText) -> bool {
    held.payload == section.payload && held.created_at_ms == section.created_at_ms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::event_text::{parse_event_sections, EventSource, EventText};
    use crate::librarian::cutover::old_rows::OldSource;
    use crate::librarian::cutover::plan::{Class, Status};
    use serde_json::{json, Value};

    const FLAGGED: &str = "---\nsingle_writer: true\n---\n\n# Doc\n";
    const AT: i64 = 1_794_637_800_000;
    const REL: &str = "docs/doc.md";

    fn event(id: &str, kind: &str, at: i64, payload: Value) -> OldEvent {
        OldEvent {
            id: id.to_owned(),
            artifact_id: "art".to_owned(),
            kind: kind.to_owned(),
            payload,
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at_ms: at,
            parent: None,
            resolves: None,
            mutates: Vec::new(),
            source: None,
        }
    }

    fn note(id: &str, text: &str) -> OldEvent {
        event(id, "note", AT, json!({ "text": text }))
    }

    fn observation(id: i64, text: &str) -> OldObservation {
        OldObservation {
            id,
            artifact_id: "art".to_owned(),
            text: text.to_owned(),
            source: None,
            created_at_ms: AT + 5,
        }
    }

    fn form_of(id: &str) -> Option<String> {
        (id == "target").then(|| "[[target]]".to_owned())
    }

    fn run(
        text: &str,
        events: &[&OldEvent],
        observations: &[&OldObservation],
        notes: &[&OldEvent],
    ) -> (String, Vec<Item>) {
        export_events(REL, text, events, observations, notes, &form_of)
    }

    fn ids(text: &str) -> Vec<String> {
        parse_event_sections(text)
            .into_iter()
            .map(|e| e.id)
            .collect()
    }

    fn only(items: &[Item]) -> &Item {
        assert_eq!(items.len(), 1, "{items:?}");
        &items[0]
    }

    #[test]
    fn a_note_on_a_single_writer_document_is_spliced_with_every_field() {
        let mut e = note("ev-1", "hello\nworld");
        e.author = Some("alice".to_owned());
        e.anchor_commit = Some("aaaa".to_owned());
        e.head_commit = Some("bbbb".to_owned());
        e.parent = Some("ev-0".to_owned());
        e.resolves = Some("ev-i".to_owned());
        e.mutates = vec!["target".to_owned()];
        e.source = Some(OldSource {
            kind: "session".to_owned(),
            uri: "session:1".to_owned(),
            payload: Some(json!({"k": 1})),
        });
        let (out, items) = run(FLAGGED, &[&e], &[], &[&e]);
        let item = only(&items);
        assert_eq!(item.status, Status::Moved, "{item:?}");
        assert_eq!(item.class, Class::Event);
        assert_eq!(item.path.as_deref(), Some(REL));
        assert_eq!(item.detail.as_deref(), Some("note"));
        assert_eq!(
            parse_event_sections(&out),
            [EventText {
                id: "ev-1".to_owned(),
                kind: "note".to_owned(),
                created_at_ms: AT,
                author: Some("alice".to_owned()),
                anchor_commit: Some("aaaa".to_owned()),
                head_commit: Some("bbbb".to_owned()),
                parent: Some("ev-0".to_owned()),
                resolves: Some("ev-i".to_owned()),
                source: Some(EventSource {
                    kind: "session".to_owned(),
                    uri: "session:1".to_owned(),
                    payload: Some(json!({"k": 1})),
                }),
                mutates: vec!["[[target]]".to_owned()],
                payload: json!({"text": "hello\nworld"}),
            }]
        );
        assert!(out.starts_with(FLAGGED), "{out}");
    }

    #[test]
    fn an_event_whose_id_the_text_holds_is_already_exported() {
        let e = note("ev-1", "hello");
        let (once, _) = run(FLAGGED, &[&e], &[], &[&e]);
        let (twice, items) = run(&once, &[&e], &[], &[&e]);
        assert_eq!(twice, once, "the text is unchanged");
        let item = only(&items);
        assert_eq!(item.status, Status::AlreadyExported, "{item:?}");
        assert_eq!(ids(&twice), ["ev-1"]);
    }

    #[test]
    fn a_note_on_an_unflagged_document_has_no_home() {
        let e = note("ev-1", "hello");
        for text in ["# Doc\n", "---\ntitle: x\n---\n\n# Doc\n"] {
            let (out, items) = run(text, &[&e], &[], &[&e]);
            assert_eq!(out, text);
            let item = only(&items);
            assert_eq!(item.status, Status::Skipped);
            assert_eq!(
                item.reason,
                "no committed home yet: not a single-writer document"
            );
            assert_eq!(item.class, Class::Event);
            assert_eq!(item.path.as_deref(), Some(REL));
            assert_eq!(item.detail.as_deref(), Some("note"));
        }
    }

    #[test]
    fn a_flagged_ledger_has_no_home() {
        let e = note("ev-1", "hello");
        for text in [
            "---\nsingle_writer: true\nentry_prefix: F\n---\n\n# Ledger\n",
            "---\nsingle_writer: true\nentry_kind: friction\n---\n\n# Entry\n",
        ] {
            let (out, items) = run(text, &[&e], &[], &[&e]);
            assert_eq!(out, text);
            let item = only(&items);
            assert_eq!(item.status, Status::Skipped);
            assert_eq!(
                item.reason,
                "no committed home yet: ledger or entry document"
            );
        }
    }

    #[test]
    fn a_kind_with_no_text_twin_has_no_home() {
        let e = event("ev-1", "decision", AT, json!({}));
        let (out, items) = run(FLAGGED, &[&e], &[], &[]);
        assert_eq!(out, FLAGGED);
        let item = only(&items);
        assert_eq!(item.status, Status::Skipped);
        assert_eq!(item.reason, "no committed home yet: kind has no text twin");
        assert_eq!(item.detail.as_deref(), Some("decision"));
    }

    #[test]
    fn an_unnameable_mutates_target_makes_the_event_unrecoverable() {
        let mut e = note("ev-1", "hello");
        // One nameable target and one that is not: the event is not written with half its targets.
        e.mutates = vec!["target".to_owned(), "gone".to_owned()];
        let (out, items) = run(FLAGGED, &[&e], &[], &[&e]);
        assert_eq!(out, FLAGGED);
        let item = only(&items);
        assert_eq!(item.status, Status::Unrecoverable);
        assert_eq!(item.reason, "a mutates target has no name in the key space");
        assert_eq!(item.detail.as_deref(), Some("note"));
    }

    #[test]
    fn an_unclosed_fence_makes_the_event_unrecoverable_and_the_text_unchanged() {
        let text = "---\nsingle_writer: true\n---\n\n# Doc\n\n```\nnever closed\n";
        let e = note("ev-1", "hello");
        let (out, items) = run(text, &[&e], &[], &[&e]);
        assert_eq!(out, text);
        let item = only(&items);
        assert_eq!(item.status, Status::Unrecoverable);
        assert!(
            item.reason.contains("would not be readable back"),
            "{}",
            item.reason
        );
        assert!(!item.reason.contains('/'), "no path: {}", item.reason);
    }

    #[test]
    fn events_go_in_creation_order_then_observations_whatever_the_input_order() {
        let late = event("a-late", "verdict", AT + 20, json!({"v": 1}));
        let early = event("z-early", "note", AT, json!({"text": "first"}));
        let tie_b = event("tie-b", "intent", AT + 10, json!({"i": 1}));
        let tie_a = event("tie-a", "intent", AT + 10, json!({"i": 2}));
        let mut first = observation(9, "c");
        first.created_at_ms = AT;
        let mut tie_5 = observation(5, "a");
        tie_5.created_at_ms = AT + 1;
        let mut tie_4 = observation(4, "b");
        tie_4.created_at_ms = AT + 1;
        let (forward, _) = run(
            FLAGGED,
            &[&late, &early, &tie_b, &tie_a],
            &[&tie_5, &first, &tie_4],
            &[&early],
        );
        let (backward, _) = run(
            FLAGGED,
            &[&tie_a, &tie_b, &early, &late],
            &[&tie_4, &tie_5, &first],
            &[&early],
        );
        assert_eq!(forward, backward);
        // The observations come after every event, though the first one is older than two.
        assert_eq!(
            ids(&forward),
            [
                "z-early",
                "tie-a",
                "tie-b",
                "a-late",
                "observation-9",
                "observation-4",
                "observation-5"
            ]
        );
    }

    #[test]
    fn an_observation_its_note_carries_is_skipped() {
        let n = note("ev-1", "seen");
        let o = observation(1, "seen");
        let (out, items) = run(FLAGGED, &[&n], &[&o], &[&n]);
        assert_eq!(ids(&out), ["ev-1"], "no second section for the observation");
        let item = items
            .iter()
            .find(|i| i.class == Class::Observation)
            .expect("an observation item");
        assert_eq!(item.status, Status::Skipped);
        assert_eq!(item.reason, "carried by its note event");
        assert_eq!(item.path.as_deref(), Some(REL));
    }

    #[test]
    fn a_note_with_other_text_or_another_kind_does_not_carry_an_observation() {
        let other_text = note("ev-1", "something else");
        let other_kind = event("ev-2", "verdict", AT, json!({"text": "seen"}));
        let o = observation(1, "seen");
        let (out, items) = run(
            FLAGGED,
            &[&other_text, &other_kind],
            &[&o],
            &[&other_text, &other_kind],
        );
        assert!(ids(&out).contains(&"observation-1".to_owned()), "{out}");
        let item = items
            .iter()
            .find(|i| i.class == Class::Observation)
            .unwrap();
        assert_eq!(item.status, Status::Moved);
    }

    #[test]
    fn an_observation_without_a_note_becomes_a_note_section() {
        let mut o = observation(7, "seen\nit");
        let (out, items) = run(FLAGGED, &[], &[&o], &[]);
        let item = only(&items);
        assert_eq!(item.class, Class::Observation);
        assert_eq!(item.status, Status::Moved, "{item:?}");
        assert_eq!(
            parse_event_sections(&out),
            [EventText {
                id: "observation-7".to_owned(),
                kind: "note".to_owned(),
                created_at_ms: AT + 5,
                author: None,
                anchor_commit: None,
                head_commit: None,
                parent: None,
                resolves: None,
                source: None,
                mutates: Vec::new(),
                payload: json!({"text": "seen\nit"}),
            }]
        );
        // A source rides in the payload next to the text.
        o.source = Some("review".to_owned());
        let (out, _) = run(FLAGGED, &[], &[&o], &[]);
        assert_eq!(
            parse_event_sections(&out)[0].payload,
            json!({"text": "seen\nit", "source": "review"})
        );
    }

    #[test]
    fn an_observation_whose_section_the_text_holds_is_already_exported() {
        let o = observation(7, "seen");
        let (once, _) = run(FLAGGED, &[], &[&o], &[]);
        let (twice, items) = run(&once, &[], &[&o], &[]);
        assert_eq!(twice, once);
        let item = only(&items);
        assert_eq!(item.status, Status::AlreadyExported, "{item:?}");
        assert_eq!(item.class, Class::Observation);
    }
    /// Observation ids are the rowids of one machine's catalog: another machine's observation 7 is
    /// another fact. A held id with other content is reported, never overwritten and never skipped.
    #[test]
    fn an_observation_whose_id_holds_other_content_is_unrecoverable_and_the_text_unchanged() {
        let theirs = observation(7, "seen on the other machine");
        let (held, _) = run(FLAGGED, &[], &[&theirs], &[]);
        let mine = observation(7, "seen here");
        let (out, items) = run(&held, &[], &[&mine], &[]);
        assert_eq!(out, held, "nothing is overwritten and no id is invented");
        let item = only(&items);
        assert_eq!(item.class, Class::Observation);
        assert_eq!(item.status, Status::Unrecoverable, "{item:?}");
        assert_eq!(item.reason, "another section holds id observation-7");
        assert_eq!(item.path.as_deref(), Some(REL));
        assert_eq!(ids(&out), ["observation-7"]);
        // The same text at another time is another fact too.
        let mut later = observation(7, "seen on the other machine");
        later.created_at_ms += 1;
        let (out, items) = run(&held, &[], &[&later], &[]);
        assert_eq!(out, held);
        assert_eq!(only(&items).status, Status::Unrecoverable);
        // A source that differs is other content.
        let mut sourced = observation(7, "seen on the other machine");
        sourced.source = Some("review".to_owned());
        let (_, items) = run(&held, &[], &[&sourced], &[]);
        assert_eq!(only(&items).status, Status::Unrecoverable);
        // The row the text was written from is still already exported.
        let (again, items) = run(&held, &[], &[&theirs], &[]);
        assert_eq!(again, held);
        assert_eq!(only(&items).status, Status::AlreadyExported);
    }

    #[test]
    fn an_observation_with_awkward_text_is_already_exported_on_a_rerun() {
        // What the text holds is compared with what would be written, so a text the format
        // reads back differently (CRLF, trailing blanks, empty lines, a lone CR) is not "other content".
        for text in [
            "two  \r\nlines  ",
            "a\n\n\nb\n",
            "lone\rcr",
            "  leading and trailing  ",
            "unicode \u{e9}\u{1f600} and `ticks` ```",
            "",
            "\n\nstarts and ends with empty lines\n\n",
            "> quoted\n# heading\n```\nfence",
            "\r\n",
            "line\u{2028}sep\u{85}nel\ttab",
            "    indented\n\ttabbed",
        ] {
            let o = observation(3, text);
            let (once, items) = run(FLAGGED, &[], &[&o], &[]);
            assert_eq!(only(&items).status, Status::Moved, "{text:?}");
            let (twice, items) = run(&once, &[], &[&o], &[]);
            assert_eq!(twice, once, "{text:?}");
            assert_eq!(only(&items).status, Status::AlreadyExported, "{text:?}");
        }
    }

    #[test]
    fn an_observation_on_an_unflagged_document_has_no_home() {
        let o = observation(7, "seen");
        let (out, items) = run("# Doc\n", &[], &[&o], &[]);
        assert_eq!(out, "# Doc\n");
        let item = only(&items);
        assert_eq!(item.class, Class::Observation);
        assert_eq!(item.status, Status::Skipped);
        assert_eq!(
            item.reason,
            "no committed home yet: not a single-writer document"
        );
    }

    #[test]
    fn an_observation_that_cannot_be_spliced_is_unrecoverable_and_leaves_the_text() {
        let text = "---\nsingle_writer: true\n---\n\n# Doc\n\n```\nnever closed\n";
        let o = observation(7, "seen");
        let (out, items) = run(text, &[], &[&o], &[]);
        assert_eq!(out, text);
        let item = only(&items);
        assert_eq!(item.class, Class::Observation);
        assert_eq!(item.status, Status::Unrecoverable);
    }
}
