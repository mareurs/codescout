//! A file budget for the LSP-independent source walks that `references` and `call_graph`
//! fall back on.
//!
//! Both walks bound themselves by file count so a monorepo cannot stall the call. The
//! bound is a performance guard and stays; what it must not do is turn "stopped early"
//! into "nothing there". [`ScanBudget::admit`] is where a walk is told it may not look at
//! one more candidate file, and it records that, so a caller can tell a walk the CAP
//! stopped from one that ran out of files (ADR 2026-08-27, a negative result names its
//! scope). The disclosure is "cut short" only when an unexamined candidate actually
//! remained: a walk that ends exactly on its cap has covered the project and reports
//! nothing.
//!
//! What this does NOT record: an ADMITTED file can still go unexamined with nothing
//! noted, because it could not be read (not UTF-8, permissions), could not be parsed, or
//! the directory walker could not enter it. A walk that is not cut short is therefore
//! "the cap did not stop it", not "every file was examined"
//! (docs/issues/2026-10-17-scan-walks-skip-unreadable-files-and-report-a-clean-miss.md).

/// Counts the candidate files a walk has admitted and remembers whether it refused one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScanBudget {
    cap: usize,
    admitted: usize,
    cut_short: bool,
}

impl ScanBudget {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap,
            admitted: 0,
            cut_short: false,
        }
    }

    /// Ask to examine one more candidate file. `false` means the budget is spent: the
    /// file will NOT be examined and the walk is recorded as cut short, so the caller
    /// should stop walking.
    pub(crate) fn admit(&mut self) -> bool {
        if self.admitted >= self.cap {
            self.cut_short = true;
            return false;
        }
        self.admitted += 1;
        true
    }

    /// `Some(cap)` when a candidate file was refused, `None` when the cap never refused
    /// one. `None` is "the cap did not stop the walk", not "every file was examined":
    /// see the module doc for what an admitted file can still go through unrecorded.
    pub(crate) fn cut_short_at(&self) -> Option<usize> {
        self.cut_short.then_some(self.cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_that_ends_exactly_on_its_cap_is_not_cut_short() {
        let mut b = ScanBudget::new(2);
        assert!(b.admit());
        assert!(b.admit());
        assert_eq!(b.cut_short_at(), None, "no candidate was refused");
    }

    #[test]
    fn the_first_refused_candidate_marks_the_walk_cut_short() {
        let mut b = ScanBudget::new(2);
        assert!(b.admit());
        assert!(b.admit());
        assert!(!b.admit(), "the third candidate is over budget");
        assert_eq!(b.cut_short_at(), Some(2));
    }

    #[test]
    fn a_zero_budget_refuses_the_first_candidate() {
        let mut b = ScanBudget::new(0);
        assert!(!b.admit());
        assert_eq!(b.cut_short_at(), Some(0));
    }
}
