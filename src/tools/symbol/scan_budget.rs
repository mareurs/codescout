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
//! The cap is not the only way a walk fails to look. A candidate (same-family) file can be
//! admitted and then not be read (not UTF-8, permissions) or parsed; a symlink to a file or
//! directory outside the project is not followed; and the directory walker can fail to
//! enter a directory at all. Each is recorded here too ([`ScanBudget::record_unexamined`],
//! [`ScanBudget::note_symlink`], [`ScanBudget::entry`]), in the same unit the cap uses —
//! files — and all of it comes out as one [`ScanCoverage`]. A walk whose coverage
//! [`is_complete`](ScanCoverage::is_complete) examined everything it was asked to; a
//! negative from any other is unconfirmed.
//!
//! A walk that cannot start at all (the seed's language has no definition parser) is a
//! pre-flight refusal, [`ScanCoverage::no_parser`]: it names that cause once and walks
//! nothing, rather than touching every file of the language and naming an I/O cause.
//!
//! Files that are not candidates (another language family, `node_modules/`, a compiled
//! twin of a TypeScript file) were never in the population and count for nothing.

use std::path::Path;

/// What a walk covered. `Default` is "everything": the one state that earns a bare
/// negative (ADR 2026-08-27 clause 2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ScanCoverage {
    /// `Some(cap)` when the file cap refused a candidate: files remain that were never
    /// looked at.
    pub cut_short_at: Option<usize>,
    /// Candidate files that were admitted but could not be read or parsed.
    pub unexamined: usize,
    /// Symlinks to a candidate file or to a directory that lead OUTSIDE the project, which
    /// the walk does not follow (the repo's other walkers do not either). A link whose
    /// target is inside the project costs nothing: the walk reaches the target by its own
    /// path.
    pub symlinks: usize,
    /// Directory-walk entries the walker could not enter or stat. Not necessarily files:
    /// what an unenterable directory held is unknown.
    pub walk_errors: usize,
    /// The language the walk was asked about has no definition parser, so it was not run:
    /// it would have opened files it could not read anything from.
    pub no_parser: Option<String>,
    /// The walk admitted no candidate at all. The seed's own file is a candidate, so a walk
    /// over a tree that contains it and examined nothing did not look where it thinks it
    /// did (a vocabulary mismatch, a seed under an ignored directory). Only the callers
    /// whose population must contain the seed set this.
    pub empty_walk: bool,
    /// How many walker entries were pulled. Not a gap: a test and diagnostic figure that
    /// shows the cap bounds the traversal, and that a refused walk touched nothing.
    pub entries_visited: usize,
}

impl ScanCoverage {
    /// The coverage of a walk that was refused before it started because `lang` has no
    /// definition parser.
    pub(crate) fn no_parser(lang: &str) -> Self {
        Self {
            no_parser: Some(lang.to_string()),
            ..Self::default()
        }
    }

    /// True when nothing was left unexamined, for any reason.
    pub(crate) fn is_complete(&self) -> bool {
        self.cut_short_at.is_none()
            && self.unexamined == 0
            && self.symlinks == 0
            && self.walk_errors == 0
            && self.no_parser.is_none()
            && !self.empty_walk
    }

    /// Fold another walk's coverage in. Walks over one tree see the same unreadable files
    /// and the same cap, so the larger figure is the honest one; summing would count a file
    /// once per symbol searched for.
    pub(crate) fn merge(&mut self, other: ScanCoverage) {
        self.cut_short_at = self.cut_short_at.max(other.cut_short_at);
        self.unexamined = self.unexamined.max(other.unexamined);
        self.symlinks = self.symlinks.max(other.symlinks);
        self.walk_errors = self.walk_errors.max(other.walk_errors);
        self.entries_visited = self.entries_visited.max(other.entries_visited);
        self.empty_walk |= other.empty_walk;
        if self.no_parser.is_none() {
            self.no_parser = other.no_parser;
        }
    }

    /// The clause a warning splices into its sentence ("the walk <this>"), or `None` when
    /// the walk was complete. States only what was observed: how many files, not why a
    /// given one failed (ADR clause 3). A refused walk says only that it was refused.
    pub(crate) fn describe(&self) -> Option<String> {
        if let Some(lang) = &self.no_parser {
            return Some(format!(
                "has no definition parser for {lang}, so it looked in no file"
            ));
        }
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
        if self.symlinks > 0 {
            parts.push(format!(
                "did not follow {} symlink(s) that lead outside the project",
                self.symlinks
            ));
        }
        if self.walk_errors > 0 {
            parts.push(format!("could not enter {} path(s)", self.walk_errors));
        }
        if self.empty_walk {
            parts.push("found no same-language file to look in".to_string());
        }
        (!parts.is_empty()).then(|| parts.join(", and "))
    }
}

/// Counts the candidate files a walk has admitted and records everything it did not
/// examine.
#[derive(Debug, Clone)]
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

    /// A candidate file went unexamined: it could not be read or parsed.
    pub(crate) fn record_unexamined(&mut self) {
        self.coverage.unexamined += 1;
    }

    /// A symlink was met and not followed. Counts it only when that loses something: it
    /// leads to a directory, or to a file `is_candidate` accepts, and its target is OUTSIDE
    /// `root` (a target inside is reached by its own path). A broken link has nothing to
    /// examine.
    pub(crate) fn note_symlink(
        &mut self,
        path: &Path,
        root: &Path,
        is_candidate: impl Fn(&Path) -> bool,
    ) {
        let Ok(meta) = std::fs::metadata(path) else {
            return;
        };
        let leads_somewhere_that_matters = meta.is_dir() || (meta.is_file() && is_candidate(path));
        if !leads_somewhere_that_matters {
            return;
        }
        let (Ok(target), Ok(root)) = (std::fs::canonicalize(path), std::fs::canonicalize(root))
        else {
            return;
        };
        if !target.starts_with(&root) {
            self.coverage.symlinks += 1;
        }
    }

    /// Funnel for the walker's items: an entry the walker could not produce is counted
    /// and dropped, never silently flattened away; every pulled entry is counted.
    pub(crate) fn entry(
        &mut self,
        item: Result<ignore::DirEntry, ignore::Error>,
    ) -> Option<ignore::DirEntry> {
        self.coverage.entries_visited += 1;
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
        self.coverage.clone()
    }

    /// [`coverage`](Self::coverage) for a walk whose population must contain at least one
    /// candidate: zero admitted is an incomplete walk, not a complete empty one.
    pub(crate) fn coverage_requiring_a_candidate(&self) -> ScanCoverage {
        let mut c = self.coverage.clone();
        if self.admitted == 0 && c.unexamined == 0 {
            c.empty_walk = true;
        }
        c
    }
}

/// Directory names the walks never enter. Deliberately short: only names that are never
/// project source anywhere. `build/`, `out/`, `dist/`, `vendor/` and `target/` are NOT
/// here although `path_in_excluded_dir` (which filters the LSP half of `references`)
/// drops them: each is a legitimate source directory in some ecosystem (Go's `go/build`
/// and `cmd/dist`, go-ethereum's `build/ci.go`), and a pruned directory is invisible to
/// the walk's coverage, so a definition living only there would be a clean miss. A tree
/// that burns the file budget there says so through the cap's disclosure instead; the
/// build directories that matter in practice are gitignored, which the walk honours.
/// Hidden directories (`.git`, `.gradle`, caches) are already skipped by the walker.
const PRUNED_DIRS: &[&str] = &["node_modules"];

/// The directory walk both fallbacks use: hidden entries and gitignored paths skipped, and
/// [`PRUNED_DIRS`] pruned, so a project whose `node_modules/` is not gitignored does not
/// spend the whole file budget there.
///
/// The prune judges the path BELOW `root`: a project that merely lives under a directory
/// of that name must still be walked.
pub(crate) fn scan_walk(root: &Path) -> ignore::Walk {
    let walk_root = root.to_path_buf();
    ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .filter_entry(move |e| {
            let rel = e.path().strip_prefix(&walk_root).unwrap_or(e.path());
            !rel.components().any(|c| {
                matches!(c, std::path::Component::Normal(name)
                    if PRUNED_DIRS.iter().any(|p| name == std::ffi::OsStr::new(p)))
            })
        })
        .build()
}

/// True for a compiled twin of a TypeScript file: a `.js`/`.jsx`/`.mjs`/`.cjs` file with a
/// `.ts` or `.tsx` sibling of the same stem (`tsc` writes `foo.js` next to `foo.ts`, and it
/// is not gitignored by default). The twin carries the same definitions and the same
/// identifiers as its source, so counting it as a candidate corroborates a symbol with its
/// own build output and spends the file budget twice.
pub(crate) fn is_compiled_twin(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("js" | "jsx" | "mjs" | "cjs") => ["ts", "tsx"]
            .iter()
            .any(|ext| path.with_extension(ext).is_file()),
        _ => false,
    }
}

/// Fixture support for tests that need a directory the process cannot enter.
#[cfg(all(test, unix))]
pub(crate) mod test_support {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    /// Restores a directory's mode on drop, so a failing assertion cannot leave an
    /// unreadable directory behind for the temp-dir cleanup to trip over.
    pub(crate) struct LockedDir(PathBuf);

    impl Drop for LockedDir {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// `chmod 000` `dir` and return the guard, or `None` after saying so on stderr when
    /// the process reads it anyway (root, some filesystems): the precondition cannot be
    /// established there, and a silent `return` would let the test pass proving nothing.
    pub(crate) fn lock(dir: &Path, test: &str) -> Option<LockedDir> {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let guard = LockedDir(dir.to_path_buf());
        if std::fs::read_dir(dir).is_ok() {
            eprintln!(
                "SKIPPED {test}: this process can read a chmod-000 directory (root?), so the \
                 unenterable-directory precondition cannot be established"
            );
            return None;
        }
        Some(guard)
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

        assert!(!ScanCoverage::no_parser("cpp").is_complete());
        assert!(!ScanCoverage {
            symlinks: 1,
            ..ScanCoverage::default()
        }
        .is_complete());
        assert!(!ScanCoverage {
            empty_walk: true,
            ..ScanCoverage::default()
        }
        .is_complete());
    }

    /// The traversal counter is not a gap: a complete walk that pulled entries is still
    /// complete.
    #[test]
    fn pulling_entries_alone_does_not_make_a_walk_incomplete() {
        let c = ScanCoverage {
            entries_visited: 99,
            ..ScanCoverage::default()
        };
        assert!(c.is_complete());
        assert_eq!(c.describe(), None);
    }

    #[test]
    fn describe_is_silent_when_complete_and_names_each_gap_otherwise() {
        assert_eq!(ScanCoverage::default().describe(), None);
        let all = ScanCoverage {
            cut_short_at: Some(5000),
            unexamined: 2,
            symlinks: 3,
            walk_errors: 1,
            empty_walk: true,
            ..ScanCoverage::default()
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
        assert!(
            text.contains("did not follow 3 symlink(s) that lead outside the project"),
            "{text}"
        );
        assert!(text.contains("could not enter 1 path(s)"), "{text}");
        assert!(
            text.contains("found no same-language file to look in"),
            "{text}"
        );
    }

    /// A walk that never started names its one cause and nothing else: no file count, and
    /// above all not the I/O-or-parse wording, which would name a cause nobody checked.
    #[test]
    fn a_refused_walk_names_the_missing_parser_and_no_io_cause() {
        let text = ScanCoverage::no_parser("cpp").describe().unwrap();
        assert_eq!(
            text,
            "has no definition parser for cpp, so it looked in no file"
        );
        assert!(!text.contains("read or parse"), "{text}");
    }

    #[test]
    fn merging_takes_the_larger_figure_not_the_sum() {
        let mut a = ScanCoverage {
            unexamined: 3,
            ..ScanCoverage::default()
        };
        a.merge(ScanCoverage {
            cut_short_at: Some(10),
            unexamined: 3,
            walk_errors: 1,
            ..ScanCoverage::default()
        });
        assert_eq!(
            a,
            ScanCoverage {
                cut_short_at: Some(10),
                unexamined: 3,
                walk_errors: 1,
                ..ScanCoverage::default()
            },
            "the same three unreadable files seen by two walks are still three"
        );
    }

    /// The seed's own file is a candidate: zero admitted is incomplete for a caller that
    /// says so, and `coverage()` for a caller whose population may be empty stays complete.
    #[test]
    fn a_walk_that_admitted_nothing_is_incomplete_only_for_a_caller_that_needs_a_candidate() {
        let b = ScanBudget::new(10);
        assert!(b.coverage().is_complete());
        let c = b.coverage_requiring_a_candidate();
        assert!(c.empty_walk && !c.is_complete());

        let mut admitted = ScanBudget::new(10);
        assert!(admitted.admit());
        assert!(admitted.coverage_requiring_a_candidate().is_complete());
    }

    #[test]
    fn a_js_file_next_to_a_ts_source_is_a_compiled_twin_and_a_lone_js_file_is_not() {
        let dir = tempfile::tempdir().unwrap();
        for f in [
            "foo.ts", "foo.js", "bar.js", "baz.tsx", "baz.jsx", "baz.mjs",
        ] {
            std::fs::write(dir.path().join(f), "").unwrap();
        }
        assert!(is_compiled_twin(&dir.path().join("foo.js")));
        assert!(is_compiled_twin(&dir.path().join("baz.jsx")));
        assert!(is_compiled_twin(&dir.path().join("baz.mjs")));
        assert!(
            !is_compiled_twin(&dir.path().join("bar.js")),
            "no .ts source"
        );
        assert!(
            !is_compiled_twin(&dir.path().join("foo.ts")),
            "the source itself"
        );
    }

    /// The `chmod 000` fixture must give the mode back when a test fails halfway, or the
    /// temp-dir cleanup trips over a directory it cannot enter. Unwinds a panic through the
    /// guard and checks the directory is readable again.
    #[cfg(unix)]
    #[test]
    fn the_locked_dir_guard_restores_the_mode_when_the_test_unwinds() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("d");
        std::fs::create_dir(&sub).unwrap();
        let Some(guard) = test_support::lock(
            &sub,
            "the_locked_dir_guard_restores_the_mode_when_the_test_unwinds",
        ) else {
            return;
        };
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = guard;
            panic!("an assertion failed while the directory was locked");
        }));
        assert!(unwound.is_err());
        assert!(
            std::fs::read_dir(&sub).is_ok(),
            "the guard must restore the mode on unwind"
        );
    }

    /// A symlink costs nothing when the walk reaches its target anyway, and one coverage
    /// unit when the target is outside the project.
    #[cfg(unix)]
    #[test]
    fn only_a_symlink_that_leads_outside_the_project_is_a_gap() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("ext.rs"), "fn ghost() {}\n").unwrap();
        let proj = tempfile::tempdir().unwrap();
        std::fs::write(proj.path().join("real.rs"), "fn real() {}\n").unwrap();
        std::os::unix::fs::symlink(outside.path().join("ext.rs"), proj.path().join("out.rs"))
            .unwrap();
        std::os::unix::fs::symlink(proj.path().join("real.rs"), proj.path().join("in.rs")).unwrap();
        std::os::unix::fs::symlink(outside.path(), proj.path().join("extdir")).unwrap();
        std::os::unix::fs::symlink(proj.path().join("nowhere"), proj.path().join("dangling.rs"))
            .unwrap();
        let rs_only = |p: &Path| p.extension().is_some_and(|e| e == "rs");

        let mut b = ScanBudget::new(10);
        for name in ["out.rs", "in.rs", "extdir", "dangling.rs"] {
            b.note_symlink(&proj.path().join(name), proj.path(), rs_only);
        }

        assert_eq!(
            b.coverage().symlinks,
            2,
            "the outward file link and the outward directory link; not the inward link, \
             not the dangling one"
        );
    }
}
