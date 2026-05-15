use std::path::Path;
use std::process::{Command, Output};

/// Reset a fixture subtree to HEAD. Scoped — never call without a path.
///
/// The user's working tree may carry unrelated in-progress edits elsewhere;
/// a bare `git restore --` would clobber them.
pub fn git_restore<P: AsRef<Path>>(fixture_src: P) -> std::io::Result<Output> {
    Command::new("git")
        .arg("restore")
        .arg("--")
        .arg(fixture_src.as_ref())
        .output()
}

/// Run `cargo check` on a fixture crate. Returns Ok(()) on exit 0,
/// Err with stderr-summary on non-zero. Inherits the calling process's
/// stdout/stderr environment but does not propagate them.
pub fn cargo_check<P: AsRef<Path>>(fixture_root: P) -> Result<(), String> {
    let manifest = fixture_root.as_ref().join("Cargo.toml");
    let out = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--quiet")
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let tail: String = String::from_utf8_lossy(&out.stderr)
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Err(tail)
    }
}

/// Read a file relative to a fixture root. Returns None on I/O error
/// so the grader can report "disk read failed" rather than panicking.
pub fn read_fixture_file<P: AsRef<Path>>(fixture_root: P, rel: &str) -> Option<String> {
    std::fs::read_to_string(fixture_root.as_ref().join(rel)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_fixture_file_returns_none_on_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(read_fixture_file(tmp.path(), "nope.rs").is_none());
    }

    #[test]
    fn read_fixture_file_returns_content() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rs"), "fn x() {}").unwrap();
        assert_eq!(
            read_fixture_file(tmp.path(), "a.rs").as_deref(),
            Some("fn x() {}")
        );
    }
}
