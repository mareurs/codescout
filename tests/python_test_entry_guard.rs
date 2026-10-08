//! A Python test file must not define anything below its `if __name__ == "__main__":` guard.
//!
//! `unittest.main()` runs the tests defined *so far* and then calls `sys.exit`, so a `class
//! ...(TestCase)` written below the guard is never reached by a direct `python3 tests/test_x.py`
//! while import-based runners (`pytest`, `unittest discover`) reach all of it. The two disagree
//! silently and both exit 0. Measured 2026-09-25 on `tests/test_stage2_synthetic.py`: a direct
//! run reported `Ran 16 tests ... OK`, discovery saw 21, and the five tests in the gap were the
//! regressions for the fix under review.
//! `docs/issues/archive/2026-09-25-codex-freeze-tests-after-main.md` has the bug; the fix that
//! moved the guard shipped without anything to stop the next class being appended below it, which
//! is what this file is.
//!
//! **Why it began as a Rust test.** Python suites were run by hand when this guard was added,
//! so a Python guard would have shared the unwired path it guarded. Since 2026-10-08,
//! `scripts/python-tests.py` runs the light suites in the local gate and both light/heavy lanes
//! in CI. This structural guard remains in the Rust gate.
//!
//! **What it checks, and the ceiling.** It checks the MECHANISM, not the symptom: nothing at
//! column 0 may follow the first entry guard, because a direct run exits before reaching it. It
//! does not compare test counts, so it does not reach a count that diverges for another reason (a
//! `unittest.main()` called from inside a function, a test class built conditionally, a runner
//! other than the two named). It covers `tests/test_*.py` only, which is where every Python test
//! file in the repo lives today.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 0-based index of the first line that is a module-level `if __name__ == "__main__"` guard.
///
/// Column 0 only: a guard nested inside another block is not recognised, and
/// `a_guard_nested_inside_another_block_is_not_the_entry_guard` pins that as a stated limit
/// rather than leaving it to be assumed. No trailing `:` is required, so the one-line form
/// `if __name__ == "__main__": unittest.main()` is recognised too.
fn entry_guard_line(source: &str) -> Option<usize> {
    source.lines().position(|line| {
        line.starts_with("if __name__")
            && (line.contains("\"__main__\"") || line.contains("'__main__'"))
    })
}

/// Every module-level line that follows the entry guard's own body, as `(1-indexed line, text)`.
/// A direct run never reaches any of them.
fn code_after_entry_guard(source: &str) -> Vec<(usize, String)> {
    let Some(guard) = entry_guard_line(source) else {
        return Vec::new();
    };
    source
        .lines()
        .enumerate()
        .skip(guard + 1)
        .filter(|(_, line)| {
            let starts_at_column_zero = !line.starts_with(char::is_whitespace);
            let is_comment = line.starts_with('#');
            // A multi-line call in the guard's body closes at column 0 (`)`), and that is the
            // body continuing, not new code.
            let closes_a_bracket = line.starts_with([')', ']', '}']);
            !line.trim().is_empty() && starts_at_column_zero && !is_comment && !closes_a_bracket
        })
        .map(|(i, line)| (i + 1, line.trim_end().to_string()))
        .collect()
}

/// `(file name, source)` for every `tests/test_*.py`, sorted by name.
fn python_test_files() -> Vec<(String, String)> {
    let dir = repo_root().join("tests");
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("test_") && name.ends_with(".py")) {
                return None;
            }
            let path = entry.path();
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            Some((name, source))
        })
        .collect();
    files.sort();
    files
}

// ---- the scanner, on fixtures -------------------------------------------------------------

/// The 2026-09-25 shape: a second test class written below the guard.
#[test]
fn a_definition_below_the_entry_guard_is_reported() {
    let src = "\
import unittest

class First(unittest.TestCase):
    def test_a(self):
        pass

if __name__ == \"__main__\":
    unittest.main()

class Second(unittest.TestCase):
    def test_b(self):
        pass
";
    // Only the column-0 `class` line is reported, not the indented `def` under it: the
    // exact-equality assertion is what keeps a widened scan (reporting every line below the
    // guard) from satisfying this.
    assert_eq!(
        code_after_entry_guard(src),
        vec![(10, "class Second(unittest.TestCase):".to_string())]
    );
}

/// The same defect with no `class` in it. The bug's own file had module-loading statements below
/// the guard too, and those are skipped by a direct run exactly as the class was.
#[test]
fn module_level_code_below_the_entry_guard_is_reported() {
    let src = "\
import unittest

if __name__ == \"__main__\":
    unittest.main()

helper = load_helper()
";
    assert_eq!(
        code_after_entry_guard(src),
        vec![(6, "helper = load_helper()".to_string())]
    );
}

/// The guard is recognised in both quote styles. Without this row the single-quote branch of the
/// recogniser is untested, and a file written that way would be invisible to the scan.
#[test]
fn a_single_quoted_entry_guard_is_recognised() {
    let src = "\
import unittest

if __name__ == '__main__':
    unittest.main()

class Late(unittest.TestCase):
    pass
";
    assert_eq!(
        code_after_entry_guard(src),
        vec![(6, "class Late(unittest.TestCase):".to_string())]
    );
}
/// The one-line form, `if __name__ == "__main__": unittest.main()`, is an entry guard like any
/// other. A recogniser that required the line to end in `:` would miss it, and a file written
/// that way would be invisible to the scan.
#[test]
fn a_one_line_entry_guard_is_recognised() {
    let src = "\
import unittest

if __name__ == \"__main__\": unittest.main()

class Late(unittest.TestCase):
    pass
";
    assert_eq!(
        code_after_entry_guard(src),
        vec![(5, "class Late(unittest.TestCase):".to_string())]
    );
}

/// A one-line guard whose call is continued over the following lines closes at column 0. That
/// is the guard continuing, not code below it; without the bracket exemption this correct file
/// would be reported.
#[test]
fn a_one_line_guard_whose_call_closes_at_column_zero_is_not_code_below_it() {
    let src = "\
import unittest

class Only(unittest.TestCase):
    pass

if __name__ == \"__main__\": unittest.main(
    verbosity=2,
)
";
    assert!(code_after_entry_guard(src).is_empty());
}

/// A stated limit, pinned so it is a decision and not an accident: only a guard at column 0 is
/// the entry guard. One nested inside another block is not recognised, so code below it is not
/// reported. If this is ever widened, this row is the one to change.
#[test]
fn a_guard_nested_inside_another_block_is_not_the_entry_guard() {
    let src = "\
import unittest

try:
    if __name__ == \"__main__\":
        unittest.main()
finally:
    pass

class Late(unittest.TestCase):
    pass
";
    assert_eq!(entry_guard_line(src), None);
    assert!(code_after_entry_guard(src).is_empty());
}

/// The control for the detection rows above: a correct file reports nothing. Without it those
/// rows would also pass on a scanner that reports every file.
#[test]
fn a_guard_that_is_the_last_statement_reports_nothing() {
    let src = "\
import unittest

class Only(unittest.TestCase):
    def test_a(self):
        pass

if __name__ == \"__main__\":
    unittest.main()
";
    assert!(code_after_entry_guard(src).is_empty());
}

/// The guard's own body (including a call continued over indented lines), comments and blank
/// lines are not "code below the guard". This is the over-reporting direction: a scan that
/// flagged any of them would red on correct files, and the corpus test below, being an absence
/// assertion, cannot tell a working scan from a dead one, so only this row pins the
/// exemptions. (A continuation that closes at column 0 is the next row.)
#[test]
fn the_guard_body_and_its_continuation_lines_are_not_code_below_it() {
    let src = "\
import sys
import unittest

class Only(unittest.TestCase):
    pass

if __name__ == \"__main__\":
    # run the suite
    unittest.main(
        argv=sys.argv,
    )

# trailing note at column 0
";
    assert!(code_after_entry_guard(src).is_empty());
}

/// A file with no entry guard (a pytest-only module) has nothing a direct run could skip.
#[test]
fn a_file_with_no_entry_guard_reports_nothing() {
    let src = "\
import unittest

class A(unittest.TestCase):
    pass

class B(unittest.TestCase):
    pass
";
    assert_eq!(entry_guard_line(src), None);
    assert!(code_after_entry_guard(src).is_empty());
}

// ---- the corpus ---------------------------------------------------------------------------

#[test]
fn no_python_test_defines_anything_below_its_entry_guard() {
    let mut offenders = Vec::new();
    for (name, source) in python_test_files() {
        for (line, text) in code_after_entry_guard(&source) {
            offenders.push(format!("tests/{name}:{line}  {text}"));
        }
    }
    assert!(
        offenders.is_empty(),
        "these lines sit below a `if __name__ == \"__main__\":` guard, so \
         `python3 tests/<file>.py` exits before reaching them while pytest and `unittest \
         discover` run them:\n  {}\n\n\
         Move the guard below the last definition. Nothing else needs to change.",
        offenders.join("\n  ")
    );
}

/// Non-vacuity, naming the member rather than counting: the file this guard exists for must be in
/// the population, and the recogniser must find its guard in the real bytes (not only in the
/// fixtures above), or the corpus test above would report clean over a scan that finds nothing.
#[test]
fn the_corpus_scan_reaches_the_file_that_motivated_it() {
    let files = python_test_files();
    let (_, source) = files
        .iter()
        .find(|(name, _)| name == "test_stage2_synthetic.py")
        .unwrap_or_else(|| {
            panic!(
                "tests/test_stage2_synthetic.py is not among the {} scanned files: the directory \
                 or the `test_*.py` filter is wrong, and the corpus test checked nothing there",
                files.len()
            )
        });
    assert!(
        entry_guard_line(source).is_some(),
        "no entry guard was recognised in tests/test_stage2_synthetic.py, which has one: the \
         recogniser stopped matching the real file's shape"
    );
}
