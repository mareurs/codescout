//! Windows-only proof that the door's comparison sees through the respellings a real runner
//! produces (`db5303eb3b9c2781`). The pure comparison is pinned on every platform in
//! `windows_spelling`; only these tests touch the Windows calls. On a volume with short names
//! switched off the short spelling cannot be made, so that test says so and returns.

use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, PathBuf};

use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

use super::windows_spelling::{agrees, long_form, same_location};

#[test]
fn long_form_expands_a_real_short_name_to_the_canonical_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("a_directory_with_a_long_name");
    std::fs::create_dir(&long).unwrap();

    let wide: Vec<u16> = long.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and `buf` has the length passed.
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), 1024) } as usize;
    assert!(n > 0 && n < 1024, "GetShortPathNameW failed");
    let short = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]));
    if short.file_name() == long.file_name() {
        eprintln!("short names are off on this volume; nothing to expand");
        return;
    }

    let real = std::fs::canonicalize(&long).unwrap();
    // Precondition: the raw short spelling really is not the canonical one.
    assert!(!same_location(
        &real.to_string_lossy(),
        &short.to_string_lossy()
    ));
    assert!(same_location(
        &real.to_string_lossy(),
        &long_form(&short).to_string_lossy()
    ));
}
/// The door judges a stored path whose file is gone, as the indexer test
/// `backfill_preserves_stored_chunks_when_the_source_file_is_absent` does. `GetLongPathNameW`
/// fails for a missing file, so before `long_form` walked up to the deepest existing ancestor the
/// whole spelling kept its short names and `agrees` called a plain missing path a link.
#[test]
fn a_missing_file_under_a_short_spelled_directory_agrees_with_its_canonical_form() {
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("a_directory_with_a_long_name");
    std::fs::create_dir(&long).unwrap();

    let wide: Vec<u16> = long.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buf = vec![0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and `buf` has the length passed.
    let n = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), 1024) } as usize;
    assert!(n > 0 && n < 1024, "GetShortPathNameW failed");
    let short = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]));
    if short.file_name() == long.file_name() {
        eprintln!("short names are off on this volume; nothing to expand");
        return;
    }

    let spelled = short.join("absent.md");
    let real = std::fs::canonicalize(&long).unwrap().join("absent.md");
    assert!(!spelled.exists(), "fixture: the file must be missing");
    // Precondition: the raw short spelling really is not the canonical one.
    assert!(!same_location(
        &real.to_string_lossy(),
        &spelled.to_string_lossy()
    ));
    assert!(agrees(&real, &spelled), "{spelled:?} names {real:?}");
}

/// A rooted path with no drive means the CURRENT drive's path. The catalog fixtures of the
/// librarian write `/test/r/a1.md` that way, and a user can type it. The directory is made
/// under the working directory so it is on the current drive, whichever drive the temp dir is on.
#[test]
fn a_rooted_spelling_without_a_drive_agrees_with_its_canonical_form() {
    let here = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&here).unwrap();
    let file = dir.path().join("rooted.md");
    std::fs::write(&file, "x").unwrap();
    let real = std::fs::canonicalize(&file).unwrap();

    // The same path with its prefix (`\\?\D:`) dropped: `\Users\…\rooted.md`.
    let rooted: PathBuf = std::iter::once(Component::RootDir.as_os_str().to_owned())
        .chain(
            real.components()
                .filter(|c| matches!(c, Component::Normal(_)))
                .map(|c| c.as_os_str().to_owned()),
        )
        .collect();
    assert!(
        rooted.has_root() && rooted.components().next() == Some(Component::RootDir),
        "fixture: {rooted:?} must be rooted with no drive"
    );
    assert_ne!(
        rooted.as_path(),
        real.as_path(),
        "precondition: the spellings differ"
    );
    assert!(agrees(&real, &rooted), "{rooted:?} names {real:?}");

    // A different file under the same directory is not the same location.
    let other = rooted.with_file_name("other.md");
    assert!(!agrees(&real, &other), "{other:?} is not {real:?}");
}
