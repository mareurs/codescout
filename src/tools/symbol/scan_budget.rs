//! A file budget, and a coverage record, for the LSP-independent source walks that
//! `references` and `call_graph` fall back on.
//!
//! Both walks bound themselves by file count so a monorepo cannot stall the call. The
//! bound is a performance guard and stays; what it must not do is turn "stopped early"
//! into "nothing there". [`ScanBudget::admit`] is where a walk is told it may not look at
//! one more candidate file, and it records that, so a caller can tell a walk the CAP
//! stopped from one that ran out of files (ADR 2026-08-27, a negative result names its
//! scope).
//!
//! The cap is not the only way a walk fails to look. A candidate (same-language) file can
//! be admitted and then not be read (not UTF-8, permissions) or parsed, can be of a
//! language with no grammar to parse it with, and the directory walker can fail to enter a
//! directory at all. Each is recorded here too ([`ScanBudget::record_unexamined`],
//! [`ScanBudget::entry`]), in the same unit the cap uses — files — and all of it comes out
//! as one [`ScanCoverage`]. A walk whose coverage [`is_complete`](ScanCoverage::is_complete)
//! examined everything it was asked to; a negative from any other is unconfirmed.
//!
//! Files that are not candidates (another language family, or under a pruned vendored
//! directory, see [`scan_walk`]) were never in the population and count for nothing.

use std::path::Path;

/// What a walk covered. `Default` is "everything": the one state that earns a bare
/// negative (ADR 2026-08-27 clause 2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ScanCoverage {
    /// `Some(cap)` when the file cap refused a candidate: files remain that were never
    /// looked at.
    pub cut_short_at: Option<usize>,
    /// Candidate files that were admitted (or needed no admission, as when the language
    /// has no grammar) but could not be read or parsed.
    pub unexamined: usize,
    /// Directory-walk entries the walker could not enter or stat. Not necessarily files:
    /// what an unenterable directory held is unknown.
    pub walk_errors: usize,
}

impl ScanCoverage {
    /// True when nothing was left unexamined, for any reason.
    pub(crate) fn is_complete(&self) -> bool {
        self.cut_short_at.is_none() && self.unexamined == 0 && self.walk_errors == 0
    }

    /// Fold another walk's coverage in. Walks over one tree see the same unreadable files
    /// and the same cap, so the larger figure is the honest one; summing would count a file
    /// once per symbol searched for.
    pub(crate) fn merge(&mut self, other: ScanCoverage) {
        self.cut_short_at = self.cut_short_at.max(other.cut_short_at);
        self.unexamined = self.unexamined.max(other.unexamined);
        self.walk_errors = self.walk_errors.max(other.walk_errors);
    }

    /// The clause a warning splices into its sentence ("the scan <this>"), or `None` when
    /// the walk was complete. States only what was observed: how many files, not why a
    /// given one failed (ADR clause 3).
    pub(crate) fn describe(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(cap) = self.cut_short_at {
            parts.push(format!(
                "stopped after {cap} same-language files with more left unexamined"
            ));
        }
        if self.unexamined > 0 {
            parts.push(format!(
                "could not read or parse {} same-language file(s)",
                self.unexamined
            ));
        }
        if self.walk_errors > 0 {
            parts.push(format!("could not enter {} path(s)", self.walk_errors));
        }
        (!parts.is_empty()).then(|| parts.join(", and "))
    }
}

/// Counts the candidate files a walk has admitted and records everything it did not
/// examine.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScanBudget {
    cap: usize,
    admitted: usize,
    coverage: ScanCoverage,
}

impl ScanBudget {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap,
            admitted: 0,
            coverage: ScanCoverage::default(),
        }
    }

    /// Ask to examine one more candidate file. `false` means the budget is spent: the
    /// file will NOT be examined and the walk is recorded as cut short, so the caller
    /// should stop walking.
    pub(crate) fn admit(&mut self) -> bool {
        if self.admitted >= self.cap {
            self.coverage.cut_short_at = Some(self.cap);
            return false;
        }
        self.admitted += 1;
        true
    }

    /// A candidate file went unexamined: it could not be read, could not be parsed, or
    /// its language has no grammar to parse it with.
    pub(crate) fn record_unexamined(&mut self) {
        self.coverage.unexamined += 1;
    }

    /// Funnel for the walker's items: an entry the walker could not produce is counted
    /// and dropped, never silently flattened away.
    pub(crate) fn entry(
        &mut self,
        item: Result<ignore::DirEntry, ignore::Error>,
    ) -> Option<ignore::DirEntry> {
        match item {
            Ok(entry) => Some(entry),
            Err(_) => {
                self.coverage.walk_errors += 1;
                None
            }
        }
    }

    /// What the walk covered so far. See [`ScanCoverage::is_complete`].
    pub(crate) fn coverage(&self) -> ScanCoverage {
        self.coverage
    }
}

/// The directory walk both fallbacks use: hidden entries and gitignored paths skipped, and
/// the directories `path_in_excluded_dir` names (`target/`, `node_modules/`, `vendor/`,
/// `build/`, `out/`, `dist/`, ...) pruned, which is what the LSP half of `references`
/// already drops. Without the prune, a project whose vendored tree is not gitignored
/// spends the whole file budget there.
///
/// The prune judges the path BELOW `root`: `path_in_excluded_dir` tests every component,
/// so the absolute path would exclude every file of a project that merely lives under a
/// directory named `build` or `out`.
pub(crate) fn scan_walk(root: &Path) -> ignore::Walk {
    let walk_root = root.to_path_buf();
    ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .filter_entry(move |e| {
            let rel = e.path().strip_prefix(&walk_root).unwrap_or(e.path());
            !crate::fs::path_in_excluded_dir(rel)
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_that_ends_exactly_on_its_cap_is_not_cut_short() {
        let mut b = ScanBudget::new(2);
        assert!(b.admit());
        assert!(b.admit());
        assert_eq!(b.coverage().cut_short_at, None, "no candidate was refused");
        assert!(b.coverage().is_complete());
    }

    #[test]
    fn the_first_refused_candidate_marks_the_walk_cut_short() {
        let mut b = ScanBudget::new(2);
        assert!(b.admit());
        assert!(b.admit());
        assert!(!b.admit(), "the third candidate is over budget");
        assert_eq!(b.coverage().cut_short_at, Some(2));
        assert!(!b.coverage().is_complete());
    }

    #[test]
    fn a_zero_budget_refuses_the_first_candidate() {
        let mut b = ScanBudget::new(0);
        assert!(!b.admit());
        assert_eq!(b.coverage().cut_short_at, Some(0));
    }

    /// Each way a walk can fail to look is its own dimension of the coverage, and each one
    /// alone makes the walk incomplete — a clean-looking `cut_short_at` must not mask the
    /// others.
    #[test]
    fn every_kind_of_unexamined_candidate_makes_the_walk_incomplete_on_its_own() {
        let mut unread = ScanBudget::new(10);
        assert!(unread.admit());
        unread.record_unexamined();
        assert_eq!(unread.coverage().cut_short_at, None);
        assert_eq!(unread.coverage().unexamined, 1);
        assert!(!unread.coverage().is_complete());

        let mut walk = ScanBudget::new(10);
        let io = ignore::Error::from(std::io::Error::other("denied"));
        assert!(walk.entry(Err(io)).is_none());
        assert_eq!(walk.coverage().walk_errors, 1);
        assert!(!walk.coverage().is_complete());
    }

    #[test]
    fn describe_is_silent_when_complete_and_names_each_gap_otherwise() {
        assert_eq!(ScanCoverage::default().describe(), None);
        let all = ScanCoverage {
            cut_short_at: Some(5000),
            unexamined: 2,
            walk_errors: 1,
        };
        let text = all.describe().unwrap();
        assert!(
            text.contains("stopped after 5000 same-language files"),
            "{text}"
        );
        assert!(
            text.contains("could not read or parse 2 same-language file(s)"),
            "{text}"
        );
        assert!(text.contains("could not enter 1 path(s)"), "{text}");
    }

    #[test]
    fn merging_takes_the_larger_figure_not_the_sum() {
        let mut a = ScanCoverage {
            cut_short_at: None,
            unexamined: 3,
            walk_errors: 0,
        };
        a.merge(ScanCoverage {
            cut_short_at: Some(10),
            unexamined: 3,
            walk_errors: 1,
        });
        assert_eq!(
            a,
            ScanCoverage {
                cut_short_at: Some(10),
                unexamined: 3,
                walk_errors: 1
            },
            "the same three unreadable files seen by two walks are still three"
        );
    }
}
