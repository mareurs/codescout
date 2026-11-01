//! Windows-only proof that `long_form` expands a real 8.3 short name (`db5303eb3b9c2781`).
//! The pure comparison is pinned on every platform in `windows_spelling`; only this test
//! touches the Windows call. On a volume with short names switched off the spelling cannot be
//! made, so the test says so and returns.

use std::os::windows::ffi::{OsStrExt, OsStringExt};

use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

use super::windows_spelling::{long_form, same_location};

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
