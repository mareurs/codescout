//! Store tests catch default capture, unsafe opens, corrupt evidence and append races.
#![cfg(target_os = "linux")]
use codescout::observations::contract::ObservationV1;
use codescout::observations::store::*;
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::sync::Arc;

fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git2::Repository::init(d.path()).unwrap();
    d
}
fn event(root: &Path) -> SanitizedEvent {
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/operation-observations-v1.json")).unwrap();
    let r = v["record"].as_object_mut().unwrap();
    for key in [
        "observation_id",
        "producer_instance",
        "sequence",
        "store_epoch",
        "observed_at_unix_ms",
    ] {
        r.remove(key);
    }
    r.insert("workspace".into(), json!(root.canonicalize().unwrap()));
    serde_json::from_value(json!(r)).unwrap()
}
fn active(root: &Path, limits: Limits) -> Arc<ObservationStore> {
    match ObservationStore::open_if_enabled(root, limits) {
        StoreAvailability::Enabled(s) => s,
        _ => panic!("enabled store unavailable"),
    }
}
#[test]
fn disabled_does_not_create_state_and_enable_is_idempotent() {
    let d = repo();
    assert!(matches!(
        ObservationStore::open_if_enabled(d.path(), Limits::default()),
        StoreAvailability::Disabled
    ));
    assert!(!d.path().join(".codescout").exists());
    let epoch = enable(d.path()).unwrap();
    assert_eq!(enable(d.path()).unwrap(), epoch);
    let s = active(d.path(), Limits::default());
    s.append(event(d.path())).unwrap();
    disable(d.path()).unwrap();
    assert!(matches!(
        ObservationStore::open_if_enabled(d.path(), Limits::default()),
        StoreAvailability::Disabled
    ));
    assert!(
        s.append(event(d.path())).is_err(),
        "cached stores must honor disable"
    );
    assert_eq!(read_store(d.path(), Limits::default()).records.len(), 1);
}
#[test]
fn private_modes_and_actual_checkout_binding() {
    let d = repo();
    enable(d.path()).unwrap();
    let s = active(d.path(), Limits::default());
    let record = s.append(event(d.path())).unwrap().observation;
    let actual: ObservationV1 = record;
    assert_eq!(
        actual.workspace,
        d.path().canonicalize().unwrap().to_str().unwrap()
    );
    let dir = d.path().join(".codescout/observations/v1");
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for p in fs::read_dir(dir).unwrap() {
        let p = p.unwrap().path();
        assert_eq!(fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let other = repo();
    assert!(s.append(event(other.path())).is_err());
}
#[test]
fn refuses_symlinked_ancestors_leaves_and_non_regular_entries() {
    for component in [
        ".codescout",
        ".codescout/observations",
        ".codescout/observations/v1",
    ] {
        let d = repo();
        let out = tempfile::tempdir().unwrap();
        let target = d.path().join(component);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        symlink(out.path(), &target).unwrap();
        assert!(enable(d.path()).is_err());
        assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
    }
    let d = repo();
    enable(d.path()).unwrap();
    let dir = d.path().join(".codescout/observations/v1");
    fs::remove_file(dir.join("control.json")).unwrap();
    let out = tempfile::NamedTempFile::new().unwrap();
    symlink(out.path(), dir.join("control.json")).unwrap();
    assert!(matches!(
        ObservationStore::open_if_enabled(d.path(), Limits::default()),
        StoreAvailability::Unavailable(_)
    ));
    assert_eq!(fs::metadata(out.path()).unwrap().len(), 0);
    let d = repo();
    enable(d.path()).unwrap();
    fs::create_dir(
        d.path()
            .join(".codescout/observations/v1/not-a-record.jsonl"),
    )
    .unwrap();
    assert!(!read_store(d.path(), Limits::default())
        .diagnostics
        .is_empty());
}
#[test]
fn bounded_rotation_and_quota_never_delete_old_evidence() {
    let d = repo();
    enable(d.path()).unwrap();
    let limits = Limits {
        shard_bytes: 2048,
        total_bytes: 8192,
        ..Limits::default()
    };
    let s = active(d.path(), limits);
    let mut count = 0;
    for _ in 0..64 {
        if s.append(event(d.path())).is_err() {
            break;
        }
        count += 1;
    }
    assert!((2..64).contains(&count));
    let read = read_store(d.path(), limits);
    assert_eq!(read.records.len(), count);
    let seq: Vec<_> = read.records.iter().map(|r| r.sequence).collect();
    assert_eq!(seq, (0..count as u64).collect::<Vec<_>>());
    assert!(read.diagnostics.iter().any(|d| d == "quota"));
    let entries: Vec<_> = fs::read_dir(d.path().join(".codescout/observations/v1"))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        entries
            .iter()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
            .count()
            >= 2
    );
    let size: u64 = entries.iter().map(|e| e.metadata().unwrap().len()).sum();
    assert!(size <= 8192);
}
#[test]
fn record_limit_rejects_without_losing_existing_records() {
    let d = repo();
    enable(d.path()).unwrap();
    let s = active(
        d.path(),
        Limits {
            record_bytes: 128,
            ..Limits::default()
        },
    );
    assert!(s.append(event(d.path())).is_err());
    assert!(read_store(d.path(), Limits::default()).records.is_empty());
}
#[test]
fn reader_reports_torn_checksum_sequence_and_duplicate_damage() {
    for damage in ["torn", "checksum", "gap", "duplicate", "conflict"] {
        let d = repo();
        enable(d.path()).unwrap();
        active(d.path(), Limits::default())
            .append(event(d.path()))
            .unwrap();
        let p = fs::read_dir(d.path().join(".codescout/observations/v1"))
            .unwrap()
            .map(Result::unwrap)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|s| s == "jsonl"))
            .unwrap();
        let original = fs::read_to_string(&p).unwrap();
        if damage == "torn" {
            fs::write(&p, original.trim_end()).unwrap();
        }
        if damage == "checksum" {
            let mut v: Value = serde_json::from_str(original.trim()).unwrap();
            v["checksum"] = json!("0".repeat(64));
            fs::write(&p, format!("{v}\n")).unwrap();
        }
        if damage == "gap" {
            let mut v: Value = serde_json::from_str(original.trim()).unwrap();
            v["record"]["sequence"] = json!(2);
            rewrite(&p, &v);
        }
        if damage == "duplicate" {
            fs::write(&p, format!("{original}{original}")).unwrap();
        }
        if damage == "conflict" {
            let mut v: Value = serde_json::from_str(original.trim()).unwrap();
            v["record"]["outcome"] = json!("unknown");
            let second = line(&v);
            fs::write(&p, format!("{original}{second}")).unwrap();
        }
        let read = read_store(d.path(), Limits::default());
        if damage == "duplicate" {
            assert_eq!(read.records.len(), 1);
            assert!(read.diagnostics.is_empty());
        } else {
            assert!(
                !read.diagnostics.is_empty(),
                "{damage} was silently accepted"
            );
        }
    }
}

#[test]
fn deleting_a_whole_producer_shard_keeps_a_sequence_gap() {
    let d = repo();
    enable(d.path()).unwrap();
    let first = active(d.path(), Limits::default())
        .append(event(d.path()))
        .unwrap();
    active(d.path(), Limits::default())
        .append(event(d.path()))
        .unwrap();
    let directory = d.path().join(".codescout/observations/v1");
    let shard = fs::read_dir(&directory)
        .unwrap()
        .map(Result::unwrap)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&first.observation.producer_instance)
        })
        .unwrap();
    fs::remove_file(shard).unwrap();
    let read = read_store(d.path(), Limits::default());
    assert_eq!(read.records.len(), 1);
    assert!(
        read.diagnostics.iter().any(|d| d == "sequence-gap"),
        "a removed producer cannot look like it never existed"
    );
}

#[test]
fn empty_shard_from_dead_writer_is_not_empty_coverage() {
    use std::os::unix::fs::PermissionsExt;
    let d = repo();
    enable(d.path()).unwrap();
    let file = d
        .path()
        .join(".codescout/observations/v1")
        .join(format!("{}-000000.jsonl", uuid::Uuid::new_v4()));
    fs::write(&file, []).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(read_store(d.path(), Limits::default())
        .diagnostics
        .iter()
        .any(|d| d == "torn-record"));
}

fn line(v: &Value) -> String {
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(
        codescout::observations::contract::canonical_json(&v["record"])
            .unwrap()
            .as_bytes(),
    ));
    format!("{}\n", json!({"record":v["record"],"checksum":hash}))
}
fn rewrite(p: &Path, v: &Value) {
    fs::write(p, line(v)).unwrap();
}

#[test]
fn concurrent_producers_have_untorn_distinct_records() {
    let d = repo();
    enable(d.path()).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let root = d.path().to_path_buf();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            let s = active(&root, Limits::default());
            barrier.wait();
            let mut acknowledged = 0usize;
            for _ in 0..12 {
                // The store waits at most 100 ms for its lock and then answers `unavailable` before it
                // writes anything. Two producers in a tight loop may lose that wait on a slow disk
                // (bug c052a7c89e3b4a4d), so a timeout is not a defect here. Any other error is.
                match s.append(event(&root)) {
                    Ok(_) => acknowledged += 1,
                    Err(e) => assert_eq!(e, "unavailable", "unexpected append error: {e}"),
                }
            }
            acknowledged
        }));
    }
    barrier.wait();
    let acknowledged: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let r = read_store(d.path(), Limits::default());
    assert!(acknowledged > 0, "no producer got a single append through");
    // Every acknowledged append is stored once, and nothing unacknowledged is stored.
    assert_eq!(r.records.len(), acknowledged);
    assert!(r.diagnostics.is_empty());
    let ids: std::collections::HashSet<_> = r.records.iter().map(|v| &v.observation_id).collect();
    assert_eq!(ids.len(), acknowledged);
}
#[test]
fn a_held_quota_lock_times_out_as_unavailable_and_leaves_no_trace() {
    let d = repo();
    enable(d.path()).unwrap();
    let s = active(d.path(), Limits::default());
    let holder = fs::File::open(d.path().join(".codescout/observations/v1/quota.lock")).unwrap();
    holder.lock().unwrap();
    let started = std::time::Instant::now();
    assert_eq!(s.append(event(d.path())).unwrap_err(), "unavailable");
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(100),
        "a contended lock is waited for, not refused at once: {:?}",
        started.elapsed()
    );
    assert!(read_store(d.path(), Limits::default()).records.is_empty());
    drop(holder);
    let stored = s.append(event(d.path())).unwrap().observation;
    assert_eq!(
        stored.sequence, 0,
        "the timed-out append left a sequence gap"
    );
    let r = read_store(d.path(), Limits::default());
    assert_eq!(r.records.len(), 1);
    assert!(r.diagnostics.is_empty());
}

#[test]
fn local_cli_stays_disabled_and_rejects_unfiltered_ingress() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let d = repo();
    let run = |verb: &str| {
        Command::new(env!("CARGO_BIN_EXE_codescout"))
            .args(["observations", verb, "--json", "--workspace"])
            .arg(d.path())
            .current_dir(d.path())
            .output()
            .unwrap()
    };
    let status = run("status");
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let value: Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["enabled"], false);
    assert_eq!(value["coverage"], "unknown");
    assert!(!d.path().join(".codescout/observations").exists());
    assert!(run("enable").status.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_codescout"))
        .args(["observations", "ingest-hook", "--workspace"])
        .arg(d.path())
        .current_dir(d.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut v = serde_json::to_value(event(d.path())).unwrap();
    v["raw_body"] = json!("privacy-canary");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&v).unwrap().as_bytes())
        .unwrap();
    assert!(!child.wait_with_output().unwrap().status.success());
    assert!(read_store(d.path(), Limits::default()).records.is_empty());
}

#[test]
fn exact_record_limit_and_control_binding() {
    let d = repo();
    enable(d.path()).unwrap();
    active(d.path(), Limits::default())
        .append(event(d.path()))
        .unwrap();
    let dir = d.path().join(".codescout/observations/v1");
    let length = fs::read_dir(&dir)
        .unwrap()
        .map(Result::unwrap)
        .find(|e| e.path().extension().is_some_and(|s| s == "jsonl"))
        .unwrap()
        .metadata()
        .unwrap()
        .len() as usize;
    active(
        d.path(),
        Limits {
            record_bytes: length,
            ..Limits::default()
        },
    )
    .append(event(d.path()))
    .unwrap();
    assert!(active(
        d.path(),
        Limits {
            record_bytes: length - 1,
            ..Limits::default()
        }
    )
    .append(event(d.path()))
    .is_err());
    let mut c: Value =
        serde_json::from_slice(&fs::read(dir.join("control.json")).unwrap()).unwrap();
    c["workspace"] = json!("/different/checkout");
    fs::write(dir.join("control.json"), serde_json::to_vec(&c).unwrap()).unwrap();
    assert!(matches!(
        ObservationStore::open_if_enabled(d.path(), Limits::default()),
        StoreAvailability::Unavailable(_)
    ));
    assert!(read_store(d.path(), Limits::default())
        .diagnostics
        .iter()
        .any(|d| d == "checkout-mismatch"));
}

#[test]
#[ignore = "subprocess fixture; exercised by two_process_writers_and_abrupt_exit"]
fn subprocess_producer() {
    let root = std::path::PathBuf::from(std::env::var_os("OBS_TEST_ROOT").expect("fixture root"));
    let tag = std::env::var("OBS_TEST_TAG").unwrap();
    let s = active(&root, Limits::default());
    fs::write(root.join(format!("ready-{tag}")), b"ready").unwrap();
    wait_file(&root.join("go"));
    for n in 0..8 {
        fs::write(root.join(format!("round-{tag}-{n}")), b"ready").unwrap();
        let other = if tag == "a" { "b" } else { "a" };
        wait_file(&root.join(format!("round-{other}-{n}")));
        s.append(event(&root)).unwrap();
    }
    // Abrupt process death skips Rust destructors; committed records must remain readable.
    if tag == "a" {
        std::process::exit(9);
    }
}
fn wait_file(path: &Path) {
    let t = std::time::Instant::now();
    while !path.exists() {
        assert!(
            t.elapsed() < std::time::Duration::from_secs(5),
            "barrier timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn two_process_writers_and_abrupt_exit() {
    use std::process::{Command, Stdio};
    let d = repo();
    enable(d.path()).unwrap();
    let mut children: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|tag| {
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "subprocess_producer", "--ignored"])
                .env("OBS_TEST_ROOT", d.path())
                .env("OBS_TEST_TAG", tag)
                .stdout(Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for tag in ["a", "b"] {
        wait_file(&d.path().join(format!("ready-{tag}")));
    }
    fs::write(d.path().join("go"), b"go").unwrap();
    assert_eq!(children[0].wait().unwrap().code(), Some(9));
    assert!(children[1].wait().unwrap().success());
    let r = read_store(d.path(), Limits::default());
    assert_eq!(r.records.len(), 16);
    assert!(r.diagnostics.is_empty());
    let mut producers = std::collections::HashMap::<_, Vec<_>>::new();
    for r in r.records {
        producers
            .entry(r.producer_instance)
            .or_default()
            .push(r.sequence);
    }
    assert_eq!(producers.len(), 2);
    for seq in producers.values() {
        assert_eq!(seq, &vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }
}

#[test]
fn activation_keeps_private_state_out_of_git_status() {
    let d = repo();
    enable(d.path()).unwrap();
    active(d.path(), Limits::default())
        .append(event(d.path()))
        .unwrap();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(d.path())
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(
        status.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&status.stdout)
    );
}
#[test]
fn activation_does_not_let_cached_store_write_after_directory_swap() {
    let d = repo();
    enable(d.path()).unwrap();
    let s = active(d.path(), Limits::default());
    let before = s.append(event(d.path())).unwrap();
    let old = d.path().join(".codescout/observations/v1");
    let moved = d.path().join("moved-observations");
    fs::rename(&old, &moved).unwrap();
    enable(d.path()).unwrap();
    assert!(
        s.append(event(d.path())).is_err(),
        "cached descriptor wrote into displaced store"
    );
    let p = fs::read_dir(&moved)
        .unwrap()
        .map(Result::unwrap)
        .find(|e| e.path().extension().is_some_and(|s| s == "jsonl"))
        .unwrap()
        .path();
    assert_eq!(fs::read_to_string(p).unwrap().lines().count(), 1);
    assert!(read_store(d.path(), Limits::default()).records.is_empty());
    assert_eq!(before.observation.sequence, 0);
}

#[test]
fn deleting_capture_state_remains_unavailable_instead_of_fresh_disabled() {
    let d = repo();
    enable(d.path()).unwrap();
    active(d.path(), Limits::default())
        .append(event(d.path()))
        .unwrap();
    fs::remove_dir_all(d.path().join(".codescout/observations")).unwrap();
    assert!(matches!(
        ObservationStore::open_if_enabled(d.path(), Limits::default()),
        StoreAvailability::Unavailable(_)
    ));
    assert!(read_store(d.path(), Limits::default())
        .diagnostics
        .iter()
        .any(|d| d == "reset"));
    enable(d.path()).unwrap();
    assert!(
        read_store(d.path(), Limits::default())
            .diagnostics
            .iter()
            .any(|d| d == "reset"),
        "new epoch cannot erase lost-evidence uncertainty"
    );
}
#[test]
fn linked_checkout_capture_and_reset_state_are_independent() {
    let base = tempfile::tempdir().unwrap();
    let main = base.path().join("main");
    let linked = base.path().join("linked");
    let git = git2::Repository::init(&main).unwrap();
    let tree_id = git.index().unwrap().write_tree().unwrap();
    let tree = git.find_tree(tree_id).unwrap();
    let signature = git2::Signature::now("fixture", "fixture@example.invalid").unwrap();
    git.commit(Some("HEAD"), &signature, &signature, "fixture", &tree, &[])
        .unwrap();
    git.worktree("linked", &linked, None).unwrap();
    enable(&main).unwrap();
    assert!(matches!(
        ObservationStore::open_if_enabled(&linked, Limits::default()),
        StoreAvailability::Disabled
    ));
    let main_epoch = read_store(&main, Limits::default()).store_epoch.unwrap();
    assert_ne!(enable(&linked).unwrap(), main_epoch);
    active(&linked, Limits::default())
        .append(event(&linked))
        .unwrap();
    fs::remove_dir_all(main.join(".codescout/observations")).unwrap();
    assert!(read_store(&main, Limits::default())
        .diagnostics
        .iter()
        .any(|d| d == "reset"));
    let other = read_store(&linked, Limits::default());
    assert_eq!(other.records.len(), 1);
    assert!(other.diagnostics.is_empty());
}
