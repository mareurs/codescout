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
/// A refused acquisition names who holds the lock instead of saying only "would block".
/// Bug `91b5ffce06ede31a`.
#[cfg(target_os = "linux")]
#[test]
fn a_refused_acquisition_names_the_process_holding_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let _participant = Fence::try_participate(dir.path()).unwrap();
    let message = Fence::try_acquire(dir.path()).unwrap_err().to_string();
    assert!(
        message.contains("would block"),
        "the original text must survive: {message}"
    );
    assert!(
        message.contains(&format!("pid {}", std::process::id())),
        "the holder must be named: {message}"
    );
    assert!(
        !message.contains("not running"),
        "this process is running, so nothing may be called dead: {message}"
    );
}

/// The case that prompted the change, rebuilt for real: a process takes the lock shared and starts
/// a background child that inherits the descriptor; the parent exits. The lock's owner is then
/// gone and its child keeps the lock, which is what the `rust-analyzer` mux did for hours. The
/// refusal must say the owner is not running and must name the child.
///
/// Needs the `flock` utility; skips quietly where it is absent.
#[cfg(target_os = "linux")]
#[test]
fn a_lock_kept_by_a_child_of_a_dead_owner_is_named_as_such() {
    use std::process::{Command, Stdio};
    if Command::new("flock")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // Creates the lock file, then releases it: only the file is wanted.
    drop(Fence::try_participate(dir.path()).unwrap());
    let lock = dir
        .path()
        .canonicalize()
        .unwrap()
        .join(".codescout")
        .join(LOCK_NAME);
    let output = Command::new("flock")
        .arg("-s")
        .arg(&lock)
        .args(["sh", "-c", "sleep 30 >/dev/null 2>&1 </dev/null & echo $!"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let child: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("the shell prints the background child's pid");

    let message = Fence::try_acquire(dir.path()).unwrap_err().to_string();
    let _ = Command::new("kill").arg(child.to_string()).status();

    assert!(
        message.contains(&format!("pid {child}")),
        "the child that kept the lock must be named: {message}"
    );
    assert!(
        message.contains("(not running"),
        "the owner that took the lock has exited and must be called not running: {message}"
    );
}
