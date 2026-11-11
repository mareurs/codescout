//! Shared test helpers for the lifecycle modules.

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
#[allow(dead_code)] // used by later lifecycle tasks
pub(crate) fn d(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A valid identity `Key`.
#[allow(dead_code)] // used by later lifecycle tasks
pub(crate) fn k(s: &str) -> Key {
    Key::parse(s).unwrap()
}
