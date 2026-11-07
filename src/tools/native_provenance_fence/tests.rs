use super::*;
use std::io::{BufRead, BufReader};

fn acquire_after_release(root: &std::path::Path) -> Fence {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match Fence::try_acquire(root) {
            Ok(fence) => return fence,
            Err(error) if std::time::Instant::now() >= deadline => panic!("{error}"),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
}

#[test]
fn one_canonical_checkout_has_one_nonreentrant_fence() {
    let dir = tempfile::tempdir().unwrap();
    let fence = Fence::try_acquire(dir.path()).unwrap();
    assert!(Fence::try_acquire(dir.path()).is_err());
    assert_eq!(fence.generation(), 0);
    drop(fence);
    let _released = acquire_after_release(dir.path());
}

#[test]
fn generations_change_durably_before_effects() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_generation(dir.path()).unwrap(), 0);
    let mut fence = acquire_after_release(dir.path());
    assert_eq!(fence.bump().unwrap(), 1);
    assert!(read_generation(dir.path()).is_err());
    drop(fence);
    let mut fence = acquire_after_release(dir.path());
    assert_eq!(fence.generation(), 1);
    assert_eq!(fence.bump().unwrap(), 2);
}

#[test]
fn malformed_generation_never_resets_to_zero() {
    let dir = tempfile::tempdir().unwrap();
    let fence = Fence::try_acquire(dir.path()).unwrap();
    let path = fence.path().with_extension("counter");
    drop(fence);
    std::fs::write(path, b"broken").unwrap();
    assert!(Fence::try_acquire(dir.path()).is_err());
    assert!(read_generation(dir.path()).is_err());
}

#[cfg(unix)]
#[test]
fn linked_fence_is_refused_without_touching_target() {
    let dir = tempfile::tempdir().unwrap();
    let fence = Fence::try_acquire(dir.path()).unwrap();
    let path = fence.path().to_owned();
    drop(fence);
    std::fs::remove_file(&path).unwrap();
    let other = dir.path().join("other");
    std::fs::write(&other, b"private").unwrap();
    std::os::unix::fs::symlink(&other, path).unwrap();
    assert!(Fence::try_acquire(dir.path()).is_err());
    assert_eq!(std::fs::read(other).unwrap(), b"private");
}

#[cfg(unix)]
#[test]
fn dropping_parent_guard_does_not_release_a_live_formatter() {
    let dir = tempfile::tempdir().unwrap();
    let fence = Fence::try_acquire(dir.path()).unwrap();
    let mut command = std::process::Command::new("python3");
    command.args([
        "-u",
        "-c",
        "import sys; print('formatter-alive',flush=True); sys.stdin.readline()",
    ]);
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    fence.apply_to_command(&mut command).unwrap();
    let mut child = command.spawn().unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "formatter-alive");
    drop(command);
    drop(fence);
    assert!(Fence::try_acquire(dir.path()).is_err());
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    let _released = acquire_after_release(dir.path());
}

#[test]
fn ordinary_participants_coexist_but_exclude_a_formatter() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = Fence::try_participate(dir.path()).unwrap();
    let mut second = Fence::try_participate(dir.path()).unwrap();
    assert_eq!(first.bump().unwrap(), 1);
    assert_eq!(second.bump().unwrap(), 2);
    assert!(Fence::try_acquire(dir.path()).is_err());
    assert!(read_generation(dir.path()).is_err());
    drop(first);
    drop(second);
    assert_eq!(acquire_after_release(dir.path()).generation(), 2);
}
