use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::operator_rules::render::{BEGIN, END};

/// The Claude Code profile directories whose `CLAUDE.md` receives the resident
/// block. Machine-scoped by design — the spec puts cross-machine sync out of
/// scope.
pub const PROFILE_DIRS: [&str; 3] = [".claude", ".claude-sdd", ".claude-kat"];

/// Resolved profile paths.
///
/// Constructed literally in tests with tempdir paths; [`Self::from_env`] is the
/// only thing that reads the environment, per
/// `docs/conventions/test-env-isolation.md` option A. That boundary is why the
/// test suite cannot overwrite the operator's real `~/.claude/CLAUDE.md`.
#[derive(Debug, Clone)]
pub struct OperatorProfiles {
    pub paths: Vec<PathBuf>,
}

impl OperatorProfiles {
    /// Read the home directory once, at the edge. The only env access here.
    pub fn from_env() -> Result<Self> {
        let home = crate::platform::home_dir().context("cannot resolve the home directory")?;
        let home = require_absolute(home)?;
        Ok(Self {
            paths: PROFILE_DIRS
                .iter()
                .map(|d| home.join(d).join("CLAUDE.md"))
                .collect(),
        })
    }
}

/// Outcome of scanning a document for the generated block. A plain
/// `Option<&str>` cannot distinguish "no markers at all" from "a BEGIN with no
/// matching END" (X4) or "more than one block" (X2), and `check` needs to
/// report each of those as its own, differently-worded [`Drift`] rather than
/// collapsing them into one silent `None`.
pub enum BlockScan<'a> {
    /// No BEGIN marker anywhere in the document.
    Absent,
    /// A BEGIN marker with no matching END marker after it.
    Malformed,
    /// Exactly one well-formed block.
    Present(&'a str),
    /// A well-formed block, plus another BEGIN marker later in the document.
    /// Which one is authoritative is not this scanner's call to make.
    Duplicate(&'a str),
}

/// Locate a marker that occupies an entire line by itself (surrounding
/// whitespace on the line ignored), scanning forward from byte offset `from`.
///
/// Line-anchored (X1): `doc.find(marker)` is a substring search with no line
/// anchoring, so a rule whose rendered text merely *quotes* the marker mid-line
/// — inside a longer sentence, or as an example — was mistaken for the real
/// delimiter. Requiring `line.trim() == marker` fixes that. Returns the byte
/// offset of the marker text itself (not of the physical line), matching what
/// the old substring search returned so callers slice the document the same
/// way.
fn find_marker_start(doc: &str, from: usize, marker: &str) -> Option<usize> {
    let mut pos = from;
    for line in doc[from..].split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if content.trim() == marker {
            let offset_in_line = content.find(marker).unwrap_or(0);
            return Some(pos + offset_in_line);
        }
        pos += line.len();
    }
    None
}

/// Scan `doc` for the generated block. See [`BlockScan`] for what each
/// outcome means.
pub fn extract_block(doc: &str) -> BlockScan<'_> {
    let Some(start) = find_marker_start(doc, 0, BEGIN) else {
        return BlockScan::Absent;
    };
    let Some(end_start) = find_marker_start(doc, start, END) else {
        return BlockScan::Malformed;
    };
    let end = end_start + END.len();
    let block = &doc[start..end];
    if find_marker_start(doc, end, BEGIN).is_some() {
        BlockScan::Duplicate(block)
    } else {
        BlockScan::Present(block)
    }
}

/// Reject a non-absolute home directory rather than silently joining profile
/// paths onto it.
///
/// `crate::platform::home_dir()` (via `std::env::var_os("HOME")` on unix) does
/// not guarantee an absolute result: an empty `$HOME` resolves to `Some("")`,
/// which turns every `home.join(d)` into a path resolved against the current
/// working directory instead of the operator's actual home — a silent
/// wrong-target write, not an error. This is the boundary check for that case,
/// factored out so it can be tested without calling
/// [`OperatorProfiles::from_env`] or reading `$HOME` (see
/// `docs/conventions/test-env-isolation.md` option A).
fn require_absolute(home: PathBuf) -> Result<PathBuf> {
    if !home.is_absolute() {
        bail!(
            "$HOME resolved to a non-absolute path ({}); refusing to guess profile locations",
            home.display()
        );
    }
    Ok(home)
}

/// Replace the generated block, or append it when absent.
///
/// Everything outside the markers is preserved byte for byte — Gate 1. Marker
/// matching is line-anchored (X1) via [`find_marker_start`] — a line must equal
/// `BEGIN`/`END` after trimming surrounding whitespace to count as a
/// delimiter, so a rule whose rendered text merely contains the marker string
/// mid-line cannot be mistaken for one. A second BEGIN marker after the first
/// block's END is refused rather than silently ignored (X2) — same voice as
/// the unterminated-BEGIN error below: this, too, refuses to guess.
pub fn splice(doc: &str, block: &str) -> Result<String> {
    let Some(start) = find_marker_start(doc, 0, BEGIN) else {
        let mut out = doc.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(block);
        return Ok(out);
    };
    let Some(end_start) = find_marker_start(doc, start, END) else {
        bail!(
            "document has a BEGIN operator-rules marker with no matching END marker; \
             refusing to guess where the generated block ends"
        );
    };
    let end = end_start + END.len();
    if find_marker_start(doc, end, BEGIN).is_some() {
        bail!(
            "document has a second BEGIN operator-rules marker after the first block \
             ends; refusing to guess which block is authoritative"
        );
    }
    let mut out = String::with_capacity(doc.len() + block.len());
    out.push_str(&doc[..start]);
    out.push_str(block.trim_end_matches('\n'));
    out.push_str(&doc[end..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> String {
        format!("{BEGIN}\n<!-- rules: OP-1 -->\n\nVerify.\n\n{END}\n")
    }

    #[test]
    fn splice_appends_the_block_when_no_markers_are_present() {
        let doc = "# My notes\n\nHand-written.\n";
        let out = splice(doc, &block()).unwrap();
        assert!(
            out.starts_with("# My notes\n\nHand-written.\n"),
            "preserves the original: {out}"
        );
        assert!(out.contains("<!-- rules: OP-1 -->"), "{out}");
    }

    #[test]
    fn splice_replaces_only_the_block_and_preserves_everything_else() {
        let doc = format!("# Head\n\nBefore.\n\n{}\nAfter.\n", block());
        let new_block = block()
            .replace("OP-1", "OP-1, OP-2")
            .replace("Verify.", "Verify.\n\nAlso this.");
        let out = splice(&doc, &new_block).unwrap();
        // Equality, not prefix/suffix: `starts_with`/`ends_with` each leave the
        // other half of the document unconstrained, so a mutation confined to
        // the interior of `doc[end..]` (e.g. eating a blank line from the
        // operator's hand-written prose after the block) can pass both while
        // still damaging the document. `new_block` ends in exactly one `\n`,
        // `splice` trims it, and `doc[end..]` restores it — so the expected
        // value below is byte-exact.
        assert_eq!(
            out,
            format!("# Head\n\nBefore.\n\n{new_block}\nAfter.\n"),
            "{out}"
        );
    }

    /// Gate 1 — compile is a fixed point after the first pass.
    #[test]
    fn splice_is_idempotent() {
        let doc = "# Head\n\nBefore.\n";
        let once = splice(doc, &block()).unwrap();
        let twice = splice(&once, &block()).unwrap();
        assert_eq!(once, twice, "second compile must be a no-op");
    }

    #[test]
    fn extract_block_returns_absent_without_markers_and_present_with_them() {
        assert!(matches!(
            extract_block("no markers here"),
            BlockScan::Absent
        ));
        let doc = format!("x\n{}y\n", block());
        let BlockScan::Present(got) = extract_block(&doc) else {
            panic!("markers present");
        };
        // Equality, not `contains`: `contains("<!-- rules: OP-1 -->")` is true
        // of the block, of the block plus trailing "y\n", and of the entire
        // document — it doesn't pin the boundary. `+ END.len()` at the END
        // offset (dropped by an off-by-one mutation) or `&doc[start..]`
        // (dropping the upper bound entirely) both still contain the marker
        // string and would pass a `contains` check; only exact equality to
        // `block()` minus its single trailing newline catches them.
        assert_eq!(got, block().trim_end_matches('\n'), "{got}");
    }

    /// X1 — a marker string quoted mid-line (not occupying the whole line) must
    /// not be mistaken for the real delimiter. Before line-anchoring, `find(END)`
    /// would match this and truncate the block early.
    #[test]
    fn a_marker_string_quoted_mid_line_is_not_a_delimiter() {
        let doc = format!(
            "Operator prose that quotes {BEGIN} inline, not as a real marker.\n{}",
            block()
        );
        let BlockScan::Present(got) = extract_block(&doc) else {
            panic!("the real, line-anchored block must still be found: {doc}");
        };
        assert_eq!(got, block().trim_end_matches('\n'), "{got}");
    }

    /// X4 — a BEGIN with no matching END is a distinct outcome from "no markers
    /// at all", so `check` can tell operators the actual condition.
    #[test]
    fn extract_block_reports_malformed_for_an_unterminated_begin() {
        let doc = format!("{BEGIN}\ndangling\n");
        assert!(matches!(extract_block(&doc), BlockScan::Malformed), "{doc}");
    }

    /// X2 — a second well-formed block is surfaced, not silently dropped.
    #[test]
    fn extract_block_reports_duplicate_for_a_second_block() {
        let second = block().replace("OP-1", "OP-99");
        let doc = format!("{}\n{second}", block());
        assert!(
            matches!(extract_block(&doc), BlockScan::Duplicate(_)),
            "{doc}"
        );
    }

    /// X2 — `splice` refuses rather than silently keeping only the first block.
    #[test]
    fn splice_refuses_a_document_with_a_second_begin_marker() {
        let second = block().replace("OP-1", "OP-99");
        let doc = format!("{}\n{second}", block());
        let err = splice(&doc, &block()).unwrap_err().to_string();
        assert!(err.contains("second"), "names the second marker: {err}");
    }

    #[test]
    fn splice_refuses_a_document_with_an_unterminated_begin_marker() {
        let doc = format!("{BEGIN}\ndangling\n");
        let err = splice(&doc, &block()).unwrap_err().to_string();
        assert!(err.contains("END"), "names the missing marker: {err}");
    }

    #[test]
    fn profiles_are_constructible_without_touching_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let p = OperatorProfiles {
            paths: vec![dir.path().join("CLAUDE.md")],
        };
        assert_eq!(p.paths.len(), 1);
    }

    #[test]
    fn splice_appends_a_separating_newline_when_the_document_has_none() {
        // Append branch: `doc` has no trailing newline at all. `splice` must
        // terminate `doc` with one newline, then add a blank-line separator
        // before the block, rather than gluing the block onto the same line
        // or dropping/duplicating a newline.
        let doc = "# Head\n\nBefore.";
        let out = splice(doc, &block()).unwrap();
        assert_eq!(out, format!("{doc}\n\n{}", block()), "{out}");
    }

    /// Pins current behaviour for a malformed document where a stray `END`
    /// appears *before* the (unterminated) `BEGIN`. `splice` only searches for
    /// `END` within `doc[start..]` — i.e. after the `BEGIN` it found — so an
    /// earlier, unrelated `END` is invisible to that search and this collapses
    /// into the same "unterminated BEGIN" error as
    /// `splice_refuses_a_document_with_an_unterminated_begin_marker`. This is
    /// not asserted to be the *correct* behaviour, only the *actual* one: no
    /// silent corruption occurs (it errors, it does not fabricate an END), but
    /// whether END-before-BEGIN should be flagged as its own distinct error is
    /// an open call.
    #[test]
    fn splice_with_a_stray_end_before_an_unterminated_begin_still_errors() {
        let doc = format!("{END}\nstray leftover\n\n{BEGIN}\ndangling\n");
        let err = splice(&doc, &block()).unwrap_err().to_string();
        assert!(err.contains("END"), "{err}");
    }

    /// Tests the guard, not [`OperatorProfiles::from_env`] — the safety rule
    /// (no test may call `from_env` or read `$HOME`) takes priority over
    /// testing the guard in situ, so `require_absolute` is exercised directly
    /// on a literal `PathBuf` instead.
    #[test]
    fn require_absolute_rejects_a_non_absolute_home() {
        let err = require_absolute(PathBuf::from("")).unwrap_err().to_string();
        assert!(err.contains("non-absolute"), "{err}");
    }

    #[test]
    fn require_absolute_accepts_an_absolute_home() {
        let dir = tempfile::tempdir().unwrap();
        let home = require_absolute(dir.path().to_path_buf()).unwrap();
        assert_eq!(home, dir.path());
    }
}
