//! The long-name form of a Windows path (`db5303eb3b9c2781`).
//!
//! The runner's temp dir is spelled `C:\Users\RUNNER~1\…`, an 8.3 short name, while
//! `std::fs::canonicalize` answers the long names (`C:\Users\runneradmin\…`). A comparison of
//! the two spellings, by equality or by `starts_with`, needs the short names expanded first.
//! Pair it with [`crate::util::fs::strip_verbatim`], which removes the `\\?\` marker.

use std::path::{Path, PathBuf};

/// `path` with its 8.3 short names (`RUNNER~1`) expanded to the long names on disk. A link is
/// not followed. Returns `path` unchanged when the call fails, which keeps a caller strict.
#[cfg(windows)]
pub fn long_form(path: &Path) -> PathBuf {
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
pub fn long_form(path: &Path) -> PathBuf {
    path.to_path_buf()
}

/// `path` as a comparison key: short names expanded and the `\\?\` marker removed. Two paths
/// that name one location on Windows have one key; off Windows the key is the path itself.
pub fn comparable(path: &Path) -> PathBuf {
    crate::util::fs::strip_verbatim(&long_form(path)).into_owned()
}

#[cfg(test)]
mod tests {
    use super::comparable;
    use std::path::Path;

    #[test]
    fn a_path_off_windows_is_its_own_key() {
        if cfg!(windows) {
            return;
        }
        assert_eq!(
            comparable(Path::new("/libs/tokio")),
            Path::new("/libs/tokio")
        );
    }
}
