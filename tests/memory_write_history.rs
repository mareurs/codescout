//! Regression contract for exact, mandatory backups of topic overwrites.
use codescout::memory::{guarded::guarded_write, MemoryStore};
use std::path::{Path, PathBuf};

// This worker is invoked explicitly by the contention test in a separate process.
#[test]
#[ignore = "child-process worker; invoked by cross_process_writes_preserve_each_predecessor"]
fn subprocess_writer() {
    let root = std::env::var_os("CODESCOUT_HISTORY_TEST_ROOT").expect("worker root");
    let marker = std::env::var_os("CODESCOUT_HISTORY_TEST_MARKER").expect("worker marker");
    let value = std::env::var("CODESCOUT_HISTORY_TEST_VALUE").expect("worker value");
    let store = MemoryStore::from_dir(PathBuf::from(root)).unwrap();
    std::fs::write(marker, b"ready").unwrap();
    guarded_write(&store, None, "topic", &value, false).unwrap();
}

#[test]
fn cross_process_writes_preserve_each_predecessor() {
    use fs4::fs_std::FileExt;
    use std::time::{Duration, Instant};
    struct Workers(Vec<std::process::Child>);
    impl Drop for Workers {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::from_dir(root.path().to_owned()).unwrap();
    let initial = "initial content\n".repeat(100);
    let a = "writer A content\n".repeat(100);
    let b = "writer B content\n".repeat(100);
    store.write("topic", &initial).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(store.dir().join(".history/write.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let mut workers = Workers(vec![]);
    for (i, value) in [&a, &b].into_iter().enumerate() {
        workers.0.push(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "subprocess_writer", "--ignored", "--nocapture"])
                .env("CODESCOUT_HISTORY_TEST_ROOT", store.dir())
                .env(
                    "CODESCOUT_HISTORY_TEST_MARKER",
                    root.path().join(format!("ready-{i}")),
                )
                .env("CODESCOUT_HISTORY_TEST_VALUE", value)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while !(root.path().join("ready-0").exists() && root.path().join("ready-1").exists()) {
        assert!(
            Instant::now() < deadline,
            "both child processes must reach the held lease"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
    for child in &mut workers.0 {
        assert!(
            child.try_wait().unwrap().is_none(),
            "writer bypassed held lease"
        );
    }
    assert_eq!(
        store.read("topic").unwrap().as_deref(),
        Some(initial.as_str())
    );
    FileExt::unlock(&lock).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut done = true;
        for child in &mut workers.0 {
            match child.try_wait().unwrap() {
                Some(status) => assert!(status.success(), "worker failed: {status}"),
                None => done = false,
            }
        }
        if done {
            break;
        }
        assert!(Instant::now() < deadline, "workers did not complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    let final_value = store.read("topic").unwrap().unwrap();
    assert!(final_value == a || final_value == b);
    let saved: Vec<_> = backups(store.dir())
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    assert_eq!(saved.len(), 2);
    assert!(saved.contains(&initial));
    assert!(
        saved.contains(if final_value == a { &b } else { &a }),
        "missing intermediate writer version"
    );
}

#[cfg(unix)]
#[test]
fn unreadable_existing_memory_is_refused_even_with_force() {
    use std::os::unix::fs::PermissionsExt;
    // This condition is about the running instrument, not a passing scenario.
    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "run permission regression as an unprivileged user"
    );
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    let path = store.dir().join("topic.md");
    std::fs::write(&path, "old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = guarded_write(&store, None, "topic", "new", true);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "old");
}

#[cfg(unix)]
#[test]
fn staging_failure_keeps_original_and_a_valid_backup() {
    use std::os::unix::fs::PermissionsExt;
    assert_ne!(
        unsafe { libc::geteuid() },
        0,
        "run permission regression unprivileged"
    );
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    store.write("topic", "old").unwrap();
    std::fs::set_permissions(store.dir(), std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = guarded_write(&store, None, "topic", "new", true);
    std::fs::set_permissions(store.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(store.read("topic").unwrap().as_deref(), Some("old"));
    let files = backups(store.dir());
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "old");
}

#[cfg(unix)]
#[test]
fn an_existing_legacy_tmp_symlink_is_never_followed() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    let outside = root.path().join("outside");
    std::fs::write(&outside, "outside").unwrap();
    store.write("topic", "old").unwrap();
    std::os::unix::fs::symlink(&outside, store.dir().join("topic.tmp")).unwrap();
    store.write("topic", "new").unwrap();
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
    assert_eq!(store.read("topic").unwrap().as_deref(), Some("new"));
}

#[cfg(unix)]
#[test]
fn configured_store_root_symlink_keeps_target_and_history_together() {
    let root = tempfile::tempdir().unwrap();
    let actual = root.path().join("actual");
    std::fs::create_dir(&actual).unwrap();
    std::os::unix::fs::symlink(&actual, root.path().join("configured")).unwrap();
    let store = MemoryStore::from_dir(root.path().join("configured")).unwrap();
    store.write("nested/topic", "old").unwrap();
    guarded_write(&store, None, "nested/topic", "new", true).unwrap();
    assert_eq!(
        std::fs::read_to_string(actual.join("nested/topic.md")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(&backups(&actual)[0]).unwrap(),
        "old"
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "child-process worker; invoked by failed_backup_never_publishes_a_partial_history_file"]
fn subprocess_backup_failure() {
    let root = std::env::var_os("CODESCOUT_HISTORY_TEST_ROOT").expect("worker root");
    let store = MemoryStore::from_dir(PathBuf::from(root)).unwrap();
    // Limit only this fresh worker, never the parallel test runner.
    unsafe {
        assert_ne!(libc::signal(libc::SIGXFSZ, libc::SIG_IGN), libc::SIG_ERR);
        let limit = libc::rlimit {
            rlim_cur: 1024,
            rlim_max: 1024,
        };
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
    }
    let error = guarded_write(&store, None, "topic", "new", true).unwrap_err();
    let codescout::memory::guarded::GuardedWriteError::Failed(error) = error else {
        panic!("expected backup I/O failure");
    };
    let io = error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .expect("I/O cause");
    assert_eq!(
        io.raw_os_error(),
        Some(libc::EFBIG),
        "instrument must reproduce partial backup EFBIG"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn failed_backup_never_publishes_a_partial_history_file() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::from_dir(root.path().to_owned()).unwrap();
    let old = "old content must survive\n".repeat(300);
    store.write("topic", &old).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "subprocess_backup_failure",
            "--ignored",
            "--nocapture",
        ])
        .env("CODESCOUT_HISTORY_TEST_ROOT", store.dir())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(store.read("topic").unwrap().as_deref(), Some(old.as_str()));
    assert!(
        backups(store.dir()).is_empty(),
        "partial backups must not acquire a recoverable .md.bak name"
    );
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(dir.join(".history"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "bak"))
        .map(|e| e.into_path())
        .collect();
    files.sort();
    files
}

#[test]
fn forced_overwrite_preserves_exact_bytes_and_history_is_not_a_topic() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    let old = "\u{feff}# Română 🐈\r\n".repeat(80);
    store.write("nested/topic", &old).unwrap();
    guarded_write(&store, None, "nested/topic", "replacement", true).unwrap();
    let files = backups(store.dir());
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(&files[0]).unwrap(), old.as_bytes());
    assert_eq!(
        files[0].parent().unwrap(),
        store.dir().join(".history/nested/topic.md")
    );
    assert_eq!(store.list().unwrap(), vec!["nested/topic"]);
    assert_eq!(
        store.read("nested/topic").unwrap().as_deref(),
        Some("replacement")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in walkdir::WalkDir::new(store.dir().join(".history")) {
            let entry = entry.unwrap();
            if entry.file_type().is_dir() {
                assert_eq!(
                    entry.metadata().unwrap().permissions().mode() & 0o777,
                    0o700,
                    "{}",
                    entry.path().display()
                );
            }
        }
        assert_eq!(
            files[0].metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            store
                .dir()
                .join(".history")
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[test]
fn raw_overwrite_also_backs_up_and_aliases_select_the_actual_target() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemoryStore::from_dir(dir.path().to_owned()).unwrap();
    store.write("topic", "first").unwrap();
    store.write("../topic.txt", "second").unwrap();
    store.write("topic.md", "third").unwrap();
    let mut saved: Vec<_> = backups(store.dir())
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    saved.sort();
    assert_eq!(saved, vec!["first", "second"]);
    assert_eq!(store.list().unwrap(), vec!["topic"]);
}

#[test]
fn private_and_explicit_stores_keep_history_with_the_selected_store() {
    let root = tempfile::tempdir().unwrap();
    let public = MemoryStore::open(root.path()).unwrap();
    let private = MemoryStore::open_private(root.path()).unwrap();
    let explicit = MemoryStore::from_dir(root.path().join("member/memories")).unwrap();
    for (store, value) in [
        (&public, "public"),
        (&private, "private"),
        (&explicit, "member"),
    ] {
        store.write("same", value).unwrap();
        guarded_write(store, None, "same", "new", true).unwrap();
        assert_eq!(
            std::fs::read_to_string(&backups(store.dir())[0]).unwrap(),
            value
        );
    }
}

#[test]
fn invalid_utf8_is_refused_even_with_force_without_touching_old_bytes() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    let old = [0x61, 0xff, 0x62, 0x0a];
    std::fs::write(store.dir().join("topic.md"), old).unwrap();
    assert!(guarded_write(&store, None, "topic", "new", true).is_err());
    assert_eq!(std::fs::read(store.dir().join("topic.md")).unwrap(), old);
    assert!(backups(store.dir()).is_empty());
}

#[test]
fn history_failure_blocks_force_and_preserves_the_topic() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    std::fs::write(store.dir().join("topic.md"), "old").unwrap();
    std::fs::write(store.dir().join(".history"), "not a directory").unwrap();
    assert!(guarded_write(&store, None, "topic", "new", true).is_err());
    assert_eq!(store.read("topic").unwrap().as_deref(), Some("old"));
}

#[test]
fn shrink_refusal_creates_no_backup_and_changes_no_content() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    let old = "long memory line\n".repeat(100);
    store.write("topic", &old).unwrap();
    assert!(guarded_write(&store, None, "topic", "tiny", false).is_err());
    assert_eq!(store.read("topic").unwrap().as_deref(), Some(old.as_str()));
    assert!(backups(store.dir()).is_empty());
}
/// A deletion is the one mutation the history contract does NOT cover: the topic's
/// final bytes are gone and nothing in `.history` holds them. A client guard that
/// treats "the server backs up before it destroys" as covering `delete` is wrong
/// (`docs/conventions/memory-write-client-overlap.md`). Both delete entry points reach
/// `WriteTransaction::delete`, so both are exercised. The overwrite first makes
/// one backup so that `backups()` is proved able to see one, which an `is_empty()`
/// alone could not (it is also what a broken reader returns).
#[test]
fn delete_keeps_no_backup_of_the_final_bytes() {
    use codescout::memory::guarded::guarded_delete;
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    for (topic, raw) in [("via-guarded", false), ("via-store", true)] {
        store.write(topic, "version one").unwrap();
        store.write(topic, "version two, the final bytes").unwrap();
        let before = backups(store.dir());
        assert!(
            before
                .iter()
                .any(|p| std::fs::read_to_string(p).unwrap() == "version one"),
            "control: the overwrite must have produced a visible backup of version one"
        );
        if raw {
            store.delete(topic).unwrap();
        } else {
            guarded_delete(&store, topic).unwrap();
        }
        assert_eq!(store.read(topic).unwrap(), None);
        let after = backups(store.dir());
        assert_eq!(after, before, "{topic}: delete must add no history file");
        assert!(
            !after
                .iter()
                .any(|p| std::fs::read_to_string(p).unwrap() == "version two, the final bytes"),
            "{topic}: the deleted final bytes must not be recoverable from history"
        );
    }
}

#[test]
fn history_is_gitignored_without_changing_the_project_gitignore() {
    let root = tempfile::tempdir().unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let store = MemoryStore::open(root.path()).unwrap();
    store.write("topic", "old").unwrap();
    store.write("topic", "new").unwrap();
    let files = backups(store.dir());
    assert_eq!(files.len(), 1);
    let output = std::process::Command::new("git")
        .args(["check-ignore", "--"])
        .arg(&files[0])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!root.path().join(".gitignore").exists());
}

#[cfg(unix)]
#[test]
fn symlink_topics_parents_and_history_are_refused() {
    use std::os::unix::fs::symlink;
    for kind in [
        "leaf",
        "parent",
        "history",
        "backup_parent",
        "lock",
        "ignore",
    ] {
        let root = tempfile::tempdir().unwrap();
        let store = MemoryStore::open(root.path()).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("topic.md"), "outside").unwrap();
        let topic = if kind == "parent" {
            "nested/topic"
        } else {
            "topic"
        };
        if kind == "leaf" {
            symlink(outside.join("topic.md"), store.dir().join("topic.md")).unwrap();
        } else if kind == "parent" {
            symlink(&outside, store.dir().join("nested")).unwrap();
        } else {
            std::fs::write(store.dir().join("topic.md"), "old").unwrap();
            if kind == "history" {
                symlink(&outside, store.dir().join(".history")).unwrap();
            } else {
                std::fs::create_dir(store.dir().join(".history")).unwrap();
                let name = match kind {
                    "lock" => "write.lock",
                    "ignore" => ".gitignore",
                    _ => "topic.md",
                };
                let dest = if kind == "backup_parent" {
                    outside.clone()
                } else {
                    outside.join("topic.md")
                };
                symlink(dest, store.dir().join(".history").join(name)).unwrap();
            }
        }
        assert!(
            guarded_write(&store, None, topic, "new", true).is_err(),
            "{kind}"
        );
        assert_eq!(
            std::fs::read_to_string(outside.join("topic.md")).unwrap(),
            "outside",
            "{kind}"
        );
        if kind != "leaf" && kind != "parent" {
            assert_eq!(
                store.read("topic").unwrap().as_deref(),
                Some("old"),
                "{kind}"
            );
        }
    }
}

#[test]
fn reserved_history_topics_cannot_overwrite_internal_files() {
    let root = tempfile::tempdir().unwrap();
    let store = MemoryStore::open(root.path()).unwrap();
    assert!(store.write(".history/write.lock", "corrupt").is_err());
}
