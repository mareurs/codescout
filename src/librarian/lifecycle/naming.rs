//! Naming a new entry and finding where an archived one goes.
//!
//! Pure string logic: no filesystem access. Plan L-B calls [`entry_stem`] and
//! [`archive_rel_path`]; neither decides *whether* to create or archive anything.

use chrono::NaiveDate;

use crate::librarian::identity::key::{components, Key, KeyError};
use crate::librarian::lifecycle::layout::{Placement, Role, ARCHIVE_DIR, TRACKERS_DIR};
use crate::librarian::lifecycle::schema::KeyStyle;

/// The longest slug, in bytes (a slug is ASCII, so bytes are characters).
// cap-class: NOT_A_CAP — a deterministic bound on the length of a generated file-name slug; the full title is kept in the entry's own title field, so no result is presented as complete
pub const SLUG_MAX: usize = 60;

/// The stem of a collection's folder-index file (`readme.md`); no entry may take it, or its
/// path would place as the collection's readme (identity `is_folder_index`).
pub const RESERVED_STEM: &str = "readme";

/// Why no entry stem could be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StemError {
    /// The stem is not a valid identity key.
    BadKey(KeyError),
    /// The stem is [`RESERVED_STEM`].
    ReservedName(String),
}

impl std::fmt::Display for StemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // `Key::parse` puts the stem it was given in the error. The fix is a different
            // TITLE: no file exists yet, so `KeyError`'s own text ("rename the file") would
            // send the author to the wrong place.
            StemError::BadKey(e) => {
                let stem = match e {
                    KeyError::NotMarkdown(s) => s,
                    KeyError::NonCanonical { key, .. } | KeyError::HexShaped { key, .. } => key,
                };
                write!(
                    f,
                    "the title gives the stem {stem:?}, which must match \
                     ^[a-z0-9]+(-[a-z0-9]+)*$ and must not be 16 hex characters; \
                     choose another title"
                )
            }
            StemError::ReservedName(n) => write!(
                f,
                "the name {n:?} is reserved for a collection's folder-index file; \
                 choose another title"
            ),
        }
    }
}

impl std::error::Error for StemError {}

/// A canonical key fragment from a free-text title.
///
/// The title is ASCII-lowercased, and every run of characters outside `[a-z0-9]` (every
/// non-ASCII character included) becomes one `-`. Leading and trailing `-` are dropped. A
/// result over [`SLUG_MAX`] is cut at a `-` boundary: a `-` at index [`SLUG_MAX`] (the first
/// byte past the cap) keeps the first [`SLUG_MAX`] bytes whole, otherwise the cut falls at the
/// last `-` inside the first [`SLUG_MAX`] bytes, or hard at [`SLUG_MAX`] when there is none. A
/// trailing `-` is dropped again. `None` when no letter or digit is left.
pub fn slug(title: &str) -> Option<String> {
    let mut out = String::with_capacity(title.len());
    let mut pending_dash = false;
    for c in title.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    if out.len() > SLUG_MAX {
        // `out` is pure ASCII, so every byte index is a char boundary.
        let cut = if out.as_bytes()[SLUG_MAX] == b'-' {
            SLUG_MAX
        } else {
            out[..SLUG_MAX].rfind('-').unwrap_or(SLUG_MAX)
        };
        out.truncate(cut);
        out.truncate(out.trim_end_matches('-').len());
    }
    (!out.is_empty()).then_some(out)
}

/// The filename stem (and so the key) of a new entry.
///
/// `Dated` gives `<YYYY-MM-DD>-<slug>`, `Name` gives `<slug>`. The result goes through
/// [`Key::parse`], so an empty or non-canonical `slug` is an error for both styles: a dated
/// stem built from an empty slug ends in `-`, and a date prefix cannot make a bad tail valid.
/// A key equal to [`RESERVED_STEM`] is refused: `<collection>/readme.md` is the collection's
/// folder-index file, not an entry.
pub fn entry_stem(style: KeyStyle, created: NaiveDate, slug: &str) -> Result<Key, StemError> {
    let key = match style {
        KeyStyle::Dated => Key::parse(&format!("{}-{slug}", created.format("%Y-%m-%d"))),
        KeyStyle::Name => Key::parse(slug),
    }
    .map_err(StemError::BadKey)?;
    if key.as_str() == RESERVED_STEM {
        return Err(StemError::ReservedName(key.as_str().to_string()));
    }
    Ok(key)
}

/// The repo-relative path of an entry directly in `collection_dir`.
///
/// `collection_dir` must be `/`-joined, non-empty and without a trailing `/`, as
/// [`Placement::collection_dir`] is.
pub fn entry_rel_path(collection_dir: &str, stem: &Key) -> String {
    format!("{collection_dir}/{}.md", stem.as_str())
}

/// Where an un-archived file moves when it is archived; `None` when it is already archived.
///
/// An entry goes to `<collection_dir>/archive/<file name>`: grouping subfolders flatten. A
/// collection readme stands for its whole folder, which moves to
/// `docs/trackers/<lifecycle>/archive/<collection>`.
///
/// `placement` must be `place(rel_path)`; a mismatched pair gives a meaningless path. Grouping
/// subfolders flatten into `archive/`, so two entries with the same file name in different
/// subfolders map to the same target: the caller must check that the target does not exist
/// before moving.
pub fn archive_rel_path(rel_path: &str, placement: &Placement) -> Option<String> {
    if placement.archived {
        return None;
    }
    match placement.role {
        Role::Entry => {
            let file = components(rel_path).pop().unwrap_or_default();
            Some(format!("{}/{ARCHIVE_DIR}/{file}", placement.collection_dir))
        }
        Role::CollectionReadme => Some(format!(
            "{TRACKERS_DIR}/{}/{ARCHIVE_DIR}/{}",
            placement.lifecycle.dir_name(),
            placement.collection.as_str()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::identity::key::is_folder_index;
    use crate::librarian::lifecycle::layout::place;
    use crate::librarian::lifecycle::schema::KeyStyle;
    use crate::librarian::lifecycle::testkit::d;

    #[test]
    fn slugs_are_canonical_or_absent() {
        assert_eq!(
            slug("frictions/ is NOT one file per entry!").as_deref(),
            Some("frictions-is-not-one-file-per-entry")
        );
        assert_eq!(slug("???"), None);
        assert_eq!(slug("Ünïcode"), Some("n-code".into()));
        assert_eq!(slug("ÜÏ"), None);
        let long = slug(&"word ".repeat(30)).unwrap();
        assert!(long.len() <= SLUG_MAX && !long.ends_with('-'));
    }

    #[test]
    fn slugs_cut_at_a_word_boundary_or_hard_at_the_cap() {
        let long = slug(&"word ".repeat(30)).unwrap();
        assert_eq!(long, ["word"; 12].join("-"));
        // No `-` inside the first SLUG_MAX bytes: hard cut.
        let solid = slug(&"a".repeat(100)).unwrap();
        assert_eq!(solid, "a".repeat(SLUG_MAX));
        // A `-` at index SLUG_MAX is a boundary: all 60 bytes survive. Without that branch the
        // cut would fall at the last `-` inside the head and leave only `aaaa`.
        let exact = format!("aaaa {} tail", "b".repeat(55));
        assert_eq!(slug(&exact).unwrap(), format!("aaaa-{}", "b".repeat(55)));
        // Runs collapse and the ends are trimmed.
        assert_eq!(slug("  --A__b  ").as_deref(), Some("a-b"));
        assert_eq!(slug(""), None);
    }

    #[test]
    fn stems_follow_the_key_style() {
        assert_eq!(
            entry_stem(KeyStyle::Dated, d("2026-11-11"), "a-friction")
                .unwrap()
                .as_str(),
            "2026-11-11-a-friction"
        );
        assert_eq!(
            entry_stem(KeyStyle::Name, d("2026-11-11"), "declared-not-wired")
                .unwrap()
                .as_str(),
            "declared-not-wired"
        );
        assert!(entry_stem(KeyStyle::Name, d("2026-11-11"), "deadbeefcafe1234").is_err());
    }

    #[test]
    fn a_stem_refuses_an_empty_or_non_canonical_slug() {
        for style in [KeyStyle::Dated, KeyStyle::Name] {
            for bad in ["", "-", "-a", "a-", "a--b", "A", "a b", "ü"] {
                assert!(
                    entry_stem(style, d("2026-11-11"), bad).is_err(),
                    "{style:?} {bad:?}"
                );
            }
        }
    }

    #[test]
    fn a_bad_stem_message_gives_the_stem_and_advises_a_title_not_a_rename() {
        for slug_text in ["Bad_Stem", "a--b"] {
            let err = entry_stem(KeyStyle::Name, d("2026-11-11"), slug_text).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(slug_text), "{msg}");
            assert!(msg.contains("choose another title"), "{msg}");
            assert!(!msg.contains("rename the file"), "{msg}");
        }
        // A dated style names the whole stem, date included.
        let msg = entry_stem(KeyStyle::Dated, d("2026-11-11"), "")
            .unwrap_err()
            .to_string();
        assert!(msg.contains("2026-11-11-"), "{msg}");
        assert!(!msg.contains("rename the file"), "{msg}");
        // 16 hex characters is refused as the shape of a repo id.
        let msg = entry_stem(KeyStyle::Name, d("2026-11-11"), "deadbeefcafe1234")
            .unwrap_err()
            .to_string();
        assert!(
            msg.contains("deadbeefcafe1234") && msg.contains("16 hex"),
            "{msg}"
        );
        assert!(!msg.contains("rename the file"), "{msg}");
    }

    #[test]
    fn slugs_edge_cases() {
        assert_eq!(slug("2026").as_deref(), Some("2026"));
        assert_eq!(slug("   "), None);
        let sixty = "c".repeat(SLUG_MAX);
        assert_eq!(slug(&sixty).as_deref(), Some(sixty.as_str()));
        let sixty_one = "c".repeat(SLUG_MAX + 1);
        assert_eq!(slug(&sixty_one).as_deref(), Some(sixty.as_str()));
    }

    #[test]
    fn a_name_stem_cannot_be_the_folder_index() {
        assert!(matches!(
            entry_stem(KeyStyle::Name, d("2026-11-11"), "readme"),
            Err(StemError::ReservedName(_))
        ));
        assert_eq!(
            entry_stem(KeyStyle::Dated, d("2026-11-11"), "readme")
                .unwrap()
                .as_str(),
            "2026-11-11-readme"
        );
        assert!(matches!(
            entry_stem(KeyStyle::Name, d("2026-11-11"), ""),
            Err(StemError::BadKey(_))
        ));
        let msg = StemError::ReservedName("readme".into()).to_string();
        assert!(
            msg.contains("readme") && msg.contains("choose another"),
            "{msg}"
        );
    }

    #[test]
    fn the_reserved_stem_is_the_identity_folder_index() {
        assert!(is_folder_index(&format!("c/{RESERVED_STEM}.md")));
    }

    #[test]
    fn an_entry_path_is_never_a_collection_readme() {
        let dir = "docs/trackers/ledgers/c";
        let cases = [
            (KeyStyle::Dated, "readme"),
            (KeyStyle::Name, "readme-two"),
            (KeyStyle::Name, "a"),
        ];
        for (style, s) in cases {
            let stem = entry_stem(style, d("2026-11-11"), s).unwrap();
            let path = entry_rel_path(dir, &stem);
            let placed = place(&path).unwrap().unwrap();
            assert_eq!(placed.role, Role::Entry, "{path}");
        }
    }

    #[test]
    fn entry_paths_join_the_collection_dir_and_stem() {
        let stem = entry_stem(KeyStyle::Dated, d("2026-11-11"), "x").unwrap();
        assert_eq!(
            entry_rel_path("docs/trackers/ledgers/frictions", &stem),
            "docs/trackers/ledgers/frictions/2026-11-11-x.md"
        );
    }

    #[test]
    fn archive_paths_are_the_nearest_sibling_archive() {
        let e = place("docs/trackers/ledgers/frictions/reports/2026-01-01-r.md")
            .unwrap()
            .unwrap();
        assert_eq!(
            archive_rel_path(
                "docs/trackers/ledgers/frictions/reports/2026-01-01-r.md",
                &e
            )
            .as_deref(),
            Some("docs/trackers/ledgers/frictions/archive/2026-01-01-r.md")
        );
        let c = place("docs/trackers/streams/x/readme.md").unwrap().unwrap();
        assert_eq!(
            archive_rel_path("docs/trackers/streams/x/readme.md", &c).as_deref(),
            Some("docs/trackers/streams/archive/x")
        );
        let done = place("docs/trackers/streams/archive/x/readme.md")
            .unwrap()
            .unwrap();
        assert_eq!(
            archive_rel_path("docs/trackers/streams/archive/x/readme.md", &done),
            None
        );
    }

    #[test]
    fn archive_paths_normalise_backslashes_and_skip_archived_entries() {
        let e = place("docs/trackers/temp/t/a/b/2026-01-01-r.md")
            .unwrap()
            .unwrap();
        assert_eq!(
            archive_rel_path("docs\\trackers\\temp\\t\\a\\b\\2026-01-01-r.md", &e).as_deref(),
            Some("docs/trackers/temp/t/archive/2026-01-01-r.md")
        );
        let gone = place("docs/trackers/temp/t/archive/2026-01-01-r.md")
            .unwrap()
            .unwrap();
        assert_eq!(
            archive_rel_path("docs/trackers/temp/t/archive/2026-01-01-r.md", &gone),
            None
        );
        // An empty or `.` component is dropped before the file name is taken, so a trailing
        // `/.` still names the file.
        let dotted = "docs/trackers/ledgers/frictions/a.md/.";
        let p = place(dotted).unwrap().unwrap();
        assert_eq!(
            archive_rel_path(dotted, &p).as_deref(),
            Some("docs/trackers/ledgers/frictions/archive/a.md")
        );
    }
}
