//! How the read door tells one Windows spelling of a location from a path through a link.
//!
//! `std::fs::canonicalize` on Windows answers `\\?\C:\Users\runneradmin\…`: a verbatim prefix,
//! the on-disk letter case and long names. A caller's spelling can be `c:/users/RUNNER~1/…`.
//! The door's no-root arm refuses any spelling whose canonical form differs from it, which is
//! right for a link and wrong for these respellings of the same file
//! (`db5303eb3b9c2781`).
//!
//! [`same_location`] is the pure comparison and runs on every platform, so Linux tests can pin
//! it. [`long_form`] is the one Windows call that expands 8.3 short names; it does not resolve
//! links, so a link still leaves the two sides different.

// Off Windows only `agrees` is called; the rest is the Windows path, kept compiled so the
// Linux tests can pin it.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};

/// `p` with the respellings that name no other file removed: the `\\?\` or `\\?\UNC\` prefix,
/// `/` as `\`, a trailing separator, and letter case (NTFS is case-insensitive).
fn normalise(p: &str) -> String {
    let s = p.replace('/', "\\");
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    };
    s.trim_end_matches('\\').to_lowercase()
}

/// Whether `real` (a canonical path) and `spelled` (the caller's spelling, with any 8.3 short
/// names already expanded by [`long_form`]) name the same location on Windows.
pub(crate) fn same_location(real: &str, spelled: &str) -> bool {
    normalise(real) == normalise(spelled)
}

/// `path` with its 8.3 short names (`RUNNER~1`) expanded to the long names on disk. A link is
/// not followed. Returns `path` unchanged when the call fails, which keeps the door strict.
#[cfg(windows)]
pub(crate) fn long_form(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated and outlives both calls; a null buffer of length 0 asks
    // for the required size, and the second call is given a buffer of exactly that size.
    let needed = unsafe { GetLongPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return path.to_path_buf();
    }
    let mut buf = vec![0u16; needed as usize];
    let written = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return path.to_path_buf();
    }
    PathBuf::from(std::ffi::OsString::from_wide(&buf[..written as usize]))
}

/// Other platforms have no short names: the spelling is already its long form.
#[cfg(not(windows))]
pub(crate) fn long_form(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// Whether the door may treat `real` as the file the caller's `lexical` spelling names, with
/// no link between them. Off Windows that is exact equality, as it always was.
#[cfg(windows)]
pub(crate) fn agrees(real: &Path, lexical: &Path) -> bool {
    same_location(
        &real.to_string_lossy(),
        &long_form(lexical).to_string_lossy(),
    )
}

/// See the Windows version above.
#[cfg(not(windows))]
pub(crate) fn agrees(real: &Path, lexical: &Path) -> bool {
    real == lexical
}

#[cfg(test)]
mod tests {
    use super::same_location;

    #[test]
    fn a_verbatim_disk_prefix_names_the_same_location_as_the_plain_spelling() {
        assert!(same_location(
            r"\\?\C:\Users\runneradmin\x.md",
            r"C:\Users\runneradmin\x.md"
        ));
    }

    #[test]
    fn a_verbatim_unc_prefix_names_the_same_location_as_the_plain_share() {
        assert!(same_location(
            r"\\?\UNC\srv\share\p\x.md",
            r"\\srv\share\p\x.md"
        ));
    }

    #[test]
    fn letter_case_and_separator_direction_do_not_make_a_second_location() {
        assert!(same_location(
            r"\\?\C:\Users\RunnerAdmin\X.md",
            "c:/users/runneradmin/x.md"
        ));
    }

    #[test]
    fn a_trailing_separator_does_not_make_a_second_location() {
        assert!(same_location(r"\\?\C:\a\b", r"C:\a\b\"));
    }

    #[test]
    fn a_link_target_is_a_different_location_from_the_link() {
        // `real` is where the file is; the spelling went through `link`.
        assert!(!same_location(r"\\?\C:\target\f.md", r"C:\link\f.md"));
    }

    #[test]
    fn a_longer_final_component_is_not_a_prefix_match() {
        assert!(!same_location(r"\\?\C:\a\bc", r"C:\a\b"));
    }

    #[test]
    fn an_unexpanded_short_name_is_a_different_location() {
        // Expanding `RUNNER~1` is `long_form`'s job. This function must not guess: a short
        // name it was handed is a name it cannot tell from a link.
        assert!(!same_location(
            r"\\?\C:\Users\runneradmin\x.md",
            r"C:\Users\RUNNER~1\x.md"
        ));
    }
}
