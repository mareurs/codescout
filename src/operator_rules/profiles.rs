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
        let home = dirs::home_dir().context("cannot resolve the home directory")?;
        Ok(Self {
            paths: PROFILE_DIRS
                .iter()
                .map(|d| home.join(d).join("CLAUDE.md"))
                .collect(),
        })
    }
}

/// The generated block, markers included, or `None` when the document has none.
pub fn extract_block(doc: &str) -> Option<&str> {
    let start = doc.find(BEGIN)?;
    let end = doc[start..].find(END)? + start + END.len();
    Some(&doc[start..end])
}

/// Replace the generated block, or append it when absent.
///
/// Everything outside the markers is preserved byte for byte — Gate 1.
pub fn splice(doc: &str, block: &str) -> Result<String> {
    let Some(start) = doc.find(BEGIN) else {
        let mut out = doc.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(block);
        return Ok(out);
    };
    let Some(rel_end) = doc[start..].find(END) else {
        bail!(
            "document has a BEGIN operator-rules marker with no matching END marker; \
             refusing to guess where the generated block ends"
        );
    };
    let end = start + rel_end + END.len();
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
        assert!(
            out.starts_with("# Head\n\nBefore.\n\n"),
            "prefix intact: {out}"
        );
        assert!(out.ends_with("\nAfter.\n"), "suffix intact: {out}");
        assert!(out.contains("OP-2") && out.contains("Also this."), "{out}");
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
    fn extract_block_returns_none_without_markers_and_the_block_with_them() {
        assert!(extract_block("no markers here").is_none());
        let doc = format!("x\n{}y\n", block());
        let got = extract_block(&doc).expect("markers present");
        assert!(got.contains("<!-- rules: OP-1 -->"), "{got}");
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
}
