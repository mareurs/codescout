//! One predicate for "would this write destroy the document?", shared by every
//! surface that replaces a whole file.
//!
//! Three surfaces overwrite documents wholesale — `doc(update)` with a
//! `body`, `edit_markdown`, and `memory(write)` — and until 2026-08-29 each
//! carried its own copy of the same byte-ratio test and its own
//! `SHRINK_GUARD_MIN_BYTES = 200`. Three copies is how the gap below survived
//! being fixed once.
//!
//! # Why two dimensions
//!
//! A byte ratio alone is blind to a truncation that keeps the *front* of a
//! document whose long lines are at the front. Measured 2026-08-28 on
//! `docs/trackers/prompt-hamsa-audit-log.md`: a write that kept the first 500
//! of 1553 lines lost **68% of the lines** but only **29% of the bytes**,
//! because the retained prefix was an index table whose rows run 3–7 KB each.
//! The byte guard declined to fire, correctly by its own terms, and 1047 lines
//! were deleted by a call that reported success. See
//! `docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md`.
//!
//! The two ratios only diverge when line lengths are uneven — which is the
//! normal shape of prose, tables and code, not an exotic case. Any test fixture
//! built from uniform-length lines moves both ratios together and cannot tell
//! the arms apart.

/// Writes over content smaller than this skip the guard entirely.
///
/// A ratio over a handful of bytes is noise: a 3-byte stub replaced by a 1-byte
/// one is a 66% "loss" and nobody cares. Files this small are typically
/// just-created frontmatter shells.
pub const SHRINK_GUARD_MIN_BYTES: usize = 200;

/// Which measurement tripped the guard.
///
/// Worth reporting rather than collapsing: telling a caller that bytes shrank
/// when bytes did not is how a warning trains people to ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkDimension {
    /// Bytes fell by more than half; lines did not.
    Bytes,
    /// Lines fell by more than half; bytes did not. The truncation-keeping-the-
    /// front case.
    Lines,
    /// Both fell by more than half — an ordinary wholesale deletion.
    Both,
}

impl ShrinkDimension {
    pub fn as_str(self) -> &'static str {
        match self {
            ShrinkDimension::Bytes => "bytes",
            ShrinkDimension::Lines => "lines",
            ShrinkDimension::Both => "bytes and lines",
        }
    }
}

/// What a refused write would have done, in both dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShrinkReport {
    pub old_bytes: usize,
    pub new_bytes: usize,
    /// Percentage of bytes lost, truncated toward zero — so it reads one point
    /// worse than the true figure, which is the safe direction for a warning.
    pub byte_pct: usize,
    pub old_lines: usize,
    pub new_lines: usize,
    /// Percentage of lines lost, same truncation.
    pub line_pct: usize,
    pub dimension: ShrinkDimension,
}

impl ShrinkReport {
    /// The shared phrase every surface embeds, so the three messages stay
    /// consistent and always name the dimension that actually tripped.
    ///
    /// Both dimensions are always shown, including the one that held: a reader
    /// deciding whether to pass `force=true` needs to see that bytes were fine
    /// precisely *because* that is the surprising part.
    pub fn describe(&self) -> String {
        format!(
            "would reduce {} → {} bytes ({}%) and {} → {} lines ({}%) — over the threshold on {}",
            self.old_bytes,
            self.new_bytes,
            self.byte_pct,
            self.old_lines,
            self.new_lines,
            self.line_pct,
            self.dimension.as_str(),
        )
    }
}

/// `Some(report)` when replacing `original` with `new` would cut either bytes
/// or lines by more than half; `None` when the write is safe.
///
/// Non-mutating and policy-free: it reports, and the caller decides. Each
/// surface owns its own refusal text and its own `force` escape, because the
/// way forward differs — `body_edits` for artifacts, `action='edit'` for
/// markdown, read-modify-write for memories.
pub fn check(original: &str, new: &str) -> Option<ShrinkReport> {
    if original.len() < SHRINK_GUARD_MIN_BYTES {
        return None;
    }

    let old_lines = original.lines().count();
    let new_lines = new.lines().count();

    let bytes_shrank = new.len() * 2 < original.len();
    // A single-line document (minified JSON, one long paragraph) can never trip
    // this arm; the byte arm still covers it.
    let lines_shrank = new_lines * 2 < old_lines;

    let dimension = match (bytes_shrank, lines_shrank) {
        (true, true) => ShrinkDimension::Both,
        (true, false) => ShrinkDimension::Bytes,
        (false, true) => ShrinkDimension::Lines,
        (false, false) => return None,
    };

    Some(ShrinkReport {
        old_bytes: original.len(),
        new_bytes: new.len(),
        byte_pct: 100 - (new.len() * 100 / original.len().max(1)),
        old_lines,
        new_lines,
        line_pct: 100 - (new_lines * 100 / old_lines.max(1)),
        dimension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines of deliberately uneven length: fat ones at the front carrying most
    /// of the bytes, thin ones behind carrying most of the lines.
    fn front_loaded() -> (String, String) {
        let fat: Vec<String> = (0..10).map(|_| "X".repeat(1000)).collect();
        let thin: Vec<String> = (0..90).map(|_| "y".repeat(10)).collect();
        let whole = format!("{}\n{}", fat.join("\n"), thin.join("\n"));
        let front = fat.join("\n");
        (whole, front)
    }

    #[test]
    fn catches_a_line_truncation_that_keeps_the_bytes() {
        let (whole, front) = front_loaded();
        // The premise of the whole fixture, asserted rather than assumed.
        assert!(
            front.len() * 2 >= whole.len(),
            "fixture must keep a majority of BYTES or it proves nothing"
        );

        let r = check(&whole, &front).expect("a 90% line truncation must be caught");
        assert_eq!(r.dimension, ShrinkDimension::Lines);
        assert_eq!(r.old_lines, 100);
        assert_eq!(r.new_lines, 10);
        assert_eq!(r.line_pct, 90);
        assert!(
            r.describe().contains("lines"),
            "the message must name the dimension that tripped: {}",
            r.describe()
        );
    }

    #[test]
    fn a_uniform_fixture_cannot_tell_the_arms_apart() {
        // Documents this test's own trap: with equal-length lines the two
        // ratios are the same number, so a "line truncation" test built this
        // way passes with the line arm deleted. Kept as an executable warning
        // to whoever edits the fixture above.
        let whole = vec!["z".repeat(50); 100].join("\n");
        let half = vec!["z".repeat(50); 40].join("\n");
        let r = check(&whole, &half).expect("both arms trip here");
        assert_eq!(
            r.dimension,
            ShrinkDimension::Both,
            "uniform lines move both ratios together — that is why the \
             front-loaded fixture exists"
        );
    }

    #[test]
    fn catches_an_ordinary_byte_shrink() {
        let whole = "X".repeat(600);
        let r = check(&whole, "tiny").expect("a 600 -> 4 byte overwrite must be caught");
        // One line either side, so the line arm cannot be what fired.
        assert_eq!(r.old_lines, 1);
        assert_eq!(r.dimension, ShrinkDimension::Bytes);
        // Integer division floors 4*100/600 to 0, so this reads a flat 100%
        // rather than the true 99.3%. Overstating a loss is the safe direction
        // for a warning; understating it would be the bug.
        assert_eq!(r.byte_pct, 100);
    }

    #[test]
    fn permits_removing_exactly_half() {
        // The boundary is `new * 2 < old`, so exactly half is allowed. Pinned
        // from both sides: one byte less must trip.
        let whole = "X".repeat(600);
        assert!(check(&whole, &"Y".repeat(300)).is_none(), "half is allowed");
        assert!(
            check(&whole, &"Y".repeat(299)).is_some(),
            "one byte under half must trip"
        );
    }

    #[test]
    fn permits_a_growing_write() {
        let whole = "X".repeat(400);
        assert!(check(&whole, &"Y".repeat(900)).is_none());
    }

    #[test]
    fn is_silent_below_the_byte_floor() {
        let stub = "X".repeat(SHRINK_GUARD_MIN_BYTES - 1);
        assert!(
            check(&stub, "y").is_none(),
            "a ratio over a handful of bytes is noise"
        );
    }

    #[test]
    fn a_single_line_document_relies_on_the_byte_arm() {
        // No newlines anywhere: old_lines == new_lines == 1, so the line arm is
        // structurally unable to fire and the byte arm must carry it.
        let whole = "X".repeat(1000);
        let r = check(&whole, &"Y".repeat(100)).expect("byte arm must still fire");
        assert_eq!((r.old_lines, r.new_lines), (1, 1));
        assert_eq!(r.dimension, ShrinkDimension::Bytes);
    }
}
