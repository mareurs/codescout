//! Shared coherence test harness: spawn two `Agent` instances sharing one
//! mux, let one write, verify the other observes the fresh state.
//!
//! Per-language tests live in sibling `coherence_<lang>.rs` modules and
//! supply:
//!   • the path to a fixture project (see `tests/fixtures/lsp-mux/<lang>/`),
//!   • the language name passed to `get_or_start`,
//!   • a pre-write + post-write symbol name pair to assert on.

use std::path::{Path, PathBuf};

/// Copy a fixture directory into a fresh tempdir.
/// Returns the tempdir (keep it alive) and the root path.
pub(crate) fn stage_fixture(fixture: &Path) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = dir.path().to_path_buf();
    copy_dir_all(fixture, &dest).expect("copy fixture");
    (dir, dest)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_all(&src_child, &dst_child)?;
        } else {
            std::fs::copy(&src_child, &dst_child)?;
        }
    }
    Ok(())
}
