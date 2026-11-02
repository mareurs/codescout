//! The long-name form of a Windows path (`32d5b6837da926fc`).
//!
//! The runner's temp dir is spelled `C:\Users\RUNNER~1\…`, an 8.3 short name, while
//! `std::fs::canonicalize` answers the long names (`C:\Users\runneradmin\…`). A comparison of
//! the two spellings, by equality or by `starts_with`, needs the short names expanded first.
//! Pair it with [`crate::util::fs::strip_verbatim`], which removes the `\\?\` marker.

use std::path::{Path, PathBuf};

/// `path` with its 8.3 short names (`RUNNER~1`) expanded to the long names on disk. A link is
/// not followed. A path that does not exist is expanded through its deepest existing ancestor and
/// the missing tail is kept as written: `GetLongPathNameW` fails for a missing file, and would
/// otherwise leave the short spelling of every directory above it. Returns `path` unchanged when
/// no ancestor expands, which keeps a caller strict.
#[cfg(windows)]
pub fn long_form(path: &Path) -> PathBuf {
    expand_through_existing_ancestor(path, get_long_path_name)
}

/// `GetLongPathNameW` of `path`; `None` when the call fails, which it does for a path that does
/// not exist ("such as if the file does not exist" in its documentation).
#[cfg(windows)]
fn get_long_path_name(path: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is NUL-terminated and outlives both calls; a null buffer of length 0 asks
    // for the required size, and the second call is given a buffer of exactly that size.
    let needed = unsafe { GetLongPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut buf = vec![0u16; needed as usize];
    let written = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), needed) };
    if written == 0 || written >= needed {
        return None;
    }
    Some(PathBuf::from(std::ffi::OsString::from_wide(
        &buf[..written as usize],
    )))
}
/// `path` with `expand` applied to its deepest ancestor that `expand` accepts, and the components
/// below that ancestor appended as written. `expand` is called on `path` itself first, so a path
/// that exists is expanded exactly as `expand` alone would. A component that is not a plain name
/// (`..`, a root) ends the walk: the result is then `path` unchanged.
///
/// Platform-independent so that a test on any host drives it with a fake `expand`; only the
/// Windows `long_form` has a real one.
#[cfg(any(windows, test))]
fn expand_through_existing_ancestor(
    path: &Path,
    expand: impl Fn(&Path) -> Option<PathBuf>,
) -> PathBuf {
    use std::path::Component;

    let mut below: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cur = path;
    loop {
        if let Some(long) = expand(cur) {
            return below.iter().rev().fold(long, |acc, name| acc.join(name));
        }
        match (cur.parent(), cur.components().next_back()) {
            (Some(up), Some(Component::Normal(name))) => {
                below.push(name);
                cur = up;
            }
            _ => return path.to_path_buf(),
        }
    }
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
    use super::{comparable, expand_through_existing_ancestor};
    use std::path::{Path, PathBuf};

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
    /// A stand-in for `GetLongPathNameW`: it knows only the paths that exist, and spells them with
    /// the long name where the runner's temp directory has the short one.
    fn fake_get_long_path_name(p: &Path) -> Option<PathBuf> {
        let exists = [
            "/u/RUNNER~1",
            "/u/RUNNER~1/Temp",
            "/u/RUNNER~1/Temp/dir",
            "/u/RUNNER~1/Temp/dir/x.md",
        ];
        let s = p.to_str()?;
        exists
            .contains(&s)
            .then(|| PathBuf::from(s.replace("RUNNER~1", "runneradmin")))
    }

    #[test]
    fn a_path_that_exists_is_expanded_as_the_call_alone_would() {
        let p = Path::new("/u/RUNNER~1/Temp/dir/x.md");
        assert_eq!(
            expand_through_existing_ancestor(p, fake_get_long_path_name),
            fake_get_long_path_name(p).unwrap()
        );
    }

    #[test]
    fn a_missing_leaf_keeps_the_expansion_of_the_directories_above_it() {
        // The shape of the Windows failure: `GetLongPathNameW` fails for the missing leaf, and the
        // whole path used to keep `RUNNER~1`.
        assert_eq!(
            expand_through_existing_ancestor(
                Path::new("/u/RUNNER~1/Temp/dir/absent.md"),
                fake_get_long_path_name
            ),
            Path::new("/u/runneradmin/Temp/dir/absent.md")
        );
    }

    #[test]
    fn several_missing_components_are_appended_as_written() {
        assert_eq!(
            expand_through_existing_ancestor(
                Path::new("/u/RUNNER~1/Temp/dir/NEW~1/deeper/x.md"),
                fake_get_long_path_name
            ),
            Path::new("/u/runneradmin/Temp/dir/NEW~1/deeper/x.md")
        );
    }

    #[test]
    fn a_path_no_ancestor_of_which_expands_comes_back_unchanged() {
        let p = Path::new("/elsewhere/RUNNER~1/x.md");
        assert_eq!(
            expand_through_existing_ancestor(p, fake_get_long_path_name),
            p
        );
    }

    #[test]
    fn a_dotdot_component_ends_the_walk_with_the_path_unchanged() {
        let p = Path::new("/u/RUNNER~1/Temp/dir/../x.md");
        assert_eq!(
            expand_through_existing_ancestor(p, fake_get_long_path_name),
            p
        );
    }
}
