//! Shared test helpers for the lifecycle modules.

use super::tree::DocInput;
use crate::librarian::frontmatter::{self, Frontmatter};
use crate::librarian::identity::key::Key;

/// Parse `yaml` (the body of a frontmatter block, newline-terminated) into a `Frontmatter`.
pub(crate) fn fm(yaml: &str) -> Frontmatter {
    frontmatter::parse(&format!("---\n{yaml}---\n"))
        .unwrap()
        .0
        .unwrap()
}

/// An ISO `YYYY-MM-DD` date.
pub(crate) fn d(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A valid identity `Key`.
pub(crate) fn k(s: &str) -> Key {
    Key::parse(s).unwrap()
}

/// A tree input document.
pub(crate) fn doc(path: &str, text: &str) -> DocInput {
    DocInput {
        rel_path: path.into(),
        text: text.into(),
    }
}

/// A ledger readme: one entry kind `f`, a `blocked` word and a `terminal` word, reviewed every 30 days.
pub(crate) const LEDGER: &str = "---\ntitle: L\nentry_kinds: [f]\nstatuses: {open: open, waiting: blocked, fixed: terminal}\ninitial_status: open\nreview_every: 30d\n---\n";

/// A stream readme: one entry kind `f`, an exit condition.
pub(crate) const STREAM: &str = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal}\ninitial_status: open\nexit_when: it ships\n---\n";

/// An entry of kind `f` (status since and created 2026-10-01); `extra` is more frontmatter lines.
pub(crate) fn entry(status: &str, extra: &str, body: &str) -> String {
    format!("---\nentry_kind: f\ntitle: t\nstatus: {status}\nstatus_since: 2026-10-01\ncreated: 2026-10-01\n{extra}---\n{body}")
}
