//! Regression probes for Albert's file, preview and embedding safeguards.
use super::*;
use crate::migrate::memories::MigrationEmbedder;
use anyhow::{bail, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::path::PathBuf;

struct Fixture {
    dir: tempfile::TempDir,
    db: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        crate::sqlite_vec_ext::register();
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fixture.memories.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE memory_item (
                point_id TEXT PRIMARY KEY, project_id TEXT NOT NULL,
                bucket TEXT NOT NULL, updated_at TEXT NOT NULL, json TEXT NOT NULL, future BLOB);
             CREATE VIRTUAL TABLE memory_vec USING vec0(
                point_id TEXT PRIMARY KEY, embedding FLOAT[768]);",
        )
        .unwrap();
        let json = r#" {
          "project_id":"fixture","bucket":"structured","title":"old title","content":"synthetic body",
          "anchors":[],"created_at":"old created","updated_at":"old updated",
          "unknown":{"keep":"literal"}
        } "#;
        conn.execute(
            "INSERT INTO memory_item VALUES (?1,'fixture','structured','SQL timestamp',?2,?3)",
            params![
                "00000000-0000-0000-0000-000000000064",
                json,
                vec![0_u8, 255]
            ],
        )
        .unwrap();
        let mut old = vec![0.0_f32; 768];
        old[767] = 1.0;
        conn.execute(
            "INSERT INTO memory_vec VALUES (?1,?2)",
            params![
                "00000000-0000-0000-0000-000000000064",
                crate::sqlite_vec_ext::dense_blob(&old)
            ],
        )
        .unwrap();
        Self { dir, db }
    }

    fn options(&self) -> RebuildOptions {
        RebuildOptions {
            dimensions: 384,
            backup: self.dir.path().join("rollback.db"),
            dry_run: false,
        }
    }

    fn files(&self) -> Vec<(std::ffi::OsString, Vec<u8>)> {
        let mut files: Vec<_> = std::fs::read_dir(self.dir.path())
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                (
                    path.file_name().unwrap().to_owned(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    }

    fn old_index(&self) {
        let conn = Connection::open(&self.db).unwrap();
        assert_eq!(vector_dimensions(&conn).unwrap(), Some(768));
        let bytes: Vec<u8> = conn
            .query_row("SELECT embedding FROM memory_vec", [], |r| r.get(0))
            .unwrap();
        let mut old = vec![0.0_f32; 768];
        old[767] = 1.0;
        assert_eq!(bytes, crate::sqlite_vec_ext::dense_blob(&old));
        assert_eq!(
            conn.query_row("SELECT count(*) FROM memory_item", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

struct Good;
#[async_trait]
impl MigrationEmbedder for Good {
    async fn embed(&self, _: &str) -> Result<Vec<f32>> {
        Ok(vec![0.25; 384])
    }
}

struct Zero;
#[async_trait]
impl MigrationEmbedder for Zero {
    async fn embed(&self, _: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0; 384])
    }
}

struct PrivateFailure;
#[async_trait]
impl MigrationEmbedder for PrivateFailure {
    async fn embed(&self, _: &str) -> Result<Vec<f32>> {
        bail!("SYNTHETIC_PRIVATE_MARKER echoed by an embedding backend")
    }
}

struct SourceReplacement(PathBuf);
#[async_trait]
impl MigrationEmbedder for SourceReplacement {
    async fn embed(&self, _: &str) -> Result<Vec<f32>> {
        let displaced = self.0.with_extension("displaced");
        std::fs::rename(&self.0, &displaced)?;
        std::fs::copy(&displaced, &self.0)?;
        Good.embed("").await
    }
}

#[tokio::test]
async fn sqlite_sidecar_backup_destinations_are_refused_without_touching_any_file() {
    for suffix in ["-wal", "-shm", "-journal", "-mj123"] {
        let f = Fixture::new();
        let before = f.files();
        let mut options = f.options();
        options.backup = PathBuf::from(format!("{}{suffix}", f.db.display()));
        let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Good), &options).await;
        assert!(
            result.is_err(),
            "accepted SQLite sidecar backup {suffix}: {result:?}"
        );
        assert_eq!(
            f.files(),
            before,
            "sidecar refusal changed files for {suffix}"
        );
        f.old_index();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_parent_directory_alias_cannot_hide_a_sqlite_sidecar_destination() {
    let f = Fixture::new();
    let alias_parent = tempfile::tempdir().unwrap();
    let alias = alias_parent.path().join("alias");
    std::os::unix::fs::symlink(f.dir.path(), &alias).unwrap();
    let mut options = f.options();
    options.backup = alias.join("fixture.memories.db-shm");
    let before = f.files();
    let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Good), &options).await;
    assert!(
        result.is_err(),
        "accepted aliased source shared-memory path as backup"
    );
    assert_eq!(f.files(), before);
    f.old_index();
}

#[tokio::test]
async fn a_byte_identical_source_replacement_is_refused_before_creating_a_backup() {
    let f = Fixture::new();
    let options = f.options();
    let result =
        rebuild_sqlite_memory_vectors(&f.db, Some(&SourceReplacement(f.db.clone())), &options)
            .await;
    assert!(
        result.is_err(),
        "accepted a different source file under the same path"
    );
    assert!(
        !options.backup.exists(),
        "backup created after source pathname replacement"
    );
    f.old_index();
}

#[tokio::test]
async fn a_strict_wal_preview_refuses_before_sqlite_or_sidecar_changes() {
    let f = Fixture::new();
    let keeper = Connection::open(&f.db).unwrap();
    keeper
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
         UPDATE memory_item SET future=x'012345';",
        )
        .unwrap();
    let before = f.files();
    let mut options = f.options();
    options.dry_run = true;
    let result = rebuild_sqlite_memory_vectors(&f.db, None, &options).await;
    assert!(result.is_err(), "strict preview accepted WAL storage");
    assert_eq!(f.files(), before);
    assert!(!options.backup.exists());
}

#[tokio::test]
async fn all_zero_embeddings_are_refused_before_backup_or_publication() {
    let f = Fixture::new();
    let before = f.files();
    let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Zero), &f.options()).await;
    assert!(result.is_err(), "published a zero embedding");
    assert_eq!(f.files(), before);
    f.old_index();
}

#[tokio::test]
async fn embedding_failure_diagnostics_do_not_echo_private_backend_text() {
    let f = Fixture::new();
    let before = f.files();
    let error = rebuild_sqlite_memory_vectors(&f.db, Some(&PrivateFailure), &f.options())
        .await
        .unwrap_err();
    assert!(
        !format!("{error:#}").contains("SYNTHETIC_PRIVATE_MARKER"),
        "backend input leaked through error chain"
    );
    assert!(format!("{error:#}").contains("original index retained"));
    assert_eq!(f.files(), before);
    f.old_index();
}

struct Pending(std::sync::Arc<std::sync::atomic::AtomicBool>);
#[async_trait]
impl MigrationEmbedder for Pending {
    async fn embed(&self, _: &str) -> Result<Vec<f32>> {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        std::future::pending().await
    }
}

#[tokio::test]
async fn cancellation_during_embedding_keeps_original_files_and_no_partial_backup() {
    let f = Fixture::new();
    let before = f.files();
    let options = f.options();
    let entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let model = Pending(entered.clone());
    let mut work = Box::pin(rebuild_sqlite_memory_vectors(&f.db, Some(&model), &options));
    tokio::select! {
        result = &mut work => panic!("pending embedding unexpectedly completed: {result:?}"),
        _ = async {
            while !entered.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        } => {}
    }
    drop(work);
    assert_eq!(f.files(), before);
    assert!(!options.backup.exists());
    f.old_index();
}

fn interrupted_child(db: &std::path::Path, stage: &str) {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "migrate::sqlite_memory_vectors::hardening_tests::crash_helper",
            "--ignored",
        ])
        .env("CODESCOUT_TEST_VECTOR_CRASH_DB", db)
        .env("CODESCOUT_TEST_VECTOR_CRASH_STAGE", stage)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(
        result.code(),
        Some(77),
        "crash helper did not reach {stage}"
    );
}

#[test]
#[ignore = "subprocess helper, executed by the interruption tests"]
fn crash_helper() {
    let db = PathBuf::from(std::env::var("CODESCOUT_TEST_VECTOR_CRASH_DB").unwrap());
    let stage = std::env::var("CODESCOUT_TEST_VECTOR_CRASH_STAGE").unwrap();
    crate::sqlite_vec_ext::register();
    let mut conn = Connection::open(db).unwrap();
    if stage == "wal-source" {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
             UPDATE memory_item SET future=x'1234';",
        )
        .unwrap();
        std::process::exit(77);
    }
    let version = data_version(&conn).unwrap();
    let snapshot = read_snapshot(&conn).unwrap();
    let replacements: Vec<_> = snapshot
        .ids
        .iter()
        .map(|id| (id.clone(), vec![0.25; 384]))
        .collect();
    publish_vectors(
        &mut conn,
        version,
        &snapshot,
        &replacements,
        384,
        |moment| {
            if (stage == "after-ddl" && moment == PublicationStage::AfterDdl)
                || (stage == "after-insert" && moment == PublicationStage::AfterInsert)
            {
                std::process::exit(77);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("requested interruption point was not reached");
}

#[test]
fn real_process_exit_after_ddl_or_insert_restores_delete_and_wal_indexes() {
    for wal in [false, true] {
        for stage in ["after-ddl", "after-insert"] {
            let f = Fixture::new();
            let keeper = Connection::open(&f.db).unwrap();
            if wal {
                keeper
                    .execute_batch(
                        "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
                    UPDATE memory_item SET future=x'4455';",
                    )
                    .unwrap();
            }
            let before = read_original_index(&keeper).unwrap();
            interrupted_child(&f.db, stage);
            assert_eq!(
                read_original_index(&keeper).unwrap(),
                before,
                "wal={wal}, stage={stage}"
            );
            f.old_index();
        }
    }
}

#[tokio::test]
async fn read_only_sqlite_recreates_missing_shm_but_strict_preview_changes_no_files() {
    let f = Fixture::new();
    interrupted_child(&f.db, "wal-source");
    let shm = PathBuf::from(format!("{}-shm", f.db.display()));
    assert!(shm.exists());
    std::fs::remove_file(&shm).unwrap(); // child exited; no live connection retains it
    let before = f.files();
    let mut options = f.options();
    options.dry_run = true;
    assert!(rebuild_sqlite_memory_vectors(&f.db, None, &options)
        .await
        .is_err());
    assert_eq!(f.files(), before);
    assert!(!shm.exists());

    // Narrow characterization of the upstream assumption that motivated refusal.
    let conn =
        Connection::open_with_flags(&f.db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(read_snapshot(&conn).unwrap().ids.len(), 1);
    assert!(shm.exists(), "upstream read-only SQLite behavior changed");
}

#[cfg(target_os = "linux")]
enum BackupMutation {
    MemoryRow,
    UnrelatedTable,
    ReplaceIdentical,
}

#[cfg(target_os = "linux")]
fn start_backup_corruption(
    source: PathBuf,
    backup: PathBuf,
    mutation: BackupMutation,
) -> std::thread::JoinHandle<Result<()>> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let parent_tid = unsafe { libc::syscall(libc::SYS_gettid) };
    let (ready, receiver) = std::sync::mpsc::channel();
    let actor = std::thread::spawn(move || -> Result<()> {
        let watched = std::ffi::CString::new(backup.parent().unwrap().as_os_str().as_bytes())?;
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
        anyhow::ensure!(fd >= 0, "inotify initialization failed");
        let mut events = unsafe { std::fs::File::from_raw_fd(fd) };
        let watch = unsafe { libc::inotify_add_watch(fd, watched.as_ptr(), libc::IN_CLOSE_WRITE) };
        anyhow::ensure!(watch >= 0, "backup watch failed");
        let conn = Connection::open(&source)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        ready.send(())?;
        let mut closes = 0;
        let mut reserved = false;
        let mut buffer = [0_u8; 4096];
        while closes < 1 {
            let mut poll = libc::pollfd {
                fd: events.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let available = unsafe { libc::poll(&mut poll, 1, 5_000) };
            anyhow::ensure!(available > 0, "no completed sealed-backup event");
            let n = std::io::Read::read(&mut events, &mut buffer)?;
            let mut offset = 0;
            while offset + 16 <= n {
                let mask = u32::from_ne_bytes(buffer[offset + 4..offset + 8].try_into()?);
                let len = u32::from_ne_bytes(buffer[offset + 12..offset + 16].try_into()?) as usize;
                anyhow::ensure!(offset + 16 + len <= n, "invalid watch event");
                let name = &buffer[offset + 16..offset + 16 + len];
                let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
                if mask & libc::IN_CLOSE_WRITE != 0
                    && name == backup.file_name().unwrap().as_encoded_bytes()
                {
                    closes += 1;
                    if !reserved {
                        // VACUUM INTO requires the source writer reservation.
                        // Acquire ours only after its destination closes. The
                        // padded backup keeps verification observable; reject
                        // a run if publication already escaped this coordination.
                        conn.execute_batch("BEGIN IMMEDIATE")?;
                        anyhow::ensure!(
                            vector_dimensions(&conn)? == Some(768),
                            "publication escaped the corruption probe's reservation"
                        );
                        reserved = true;
                    }
                }
                offset += 16 + len;
            }
        }
        // The public call is synchronous between backup verification and the
        // IMMEDIATE publication. A clock_nanosleep here is SQLite's busy
        // wait for our reservation, after the initial backup has been sealed.
        let waiting_task = format!("/proc/self/task/{parent_tid}/syscall");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut publication_waiting = false;
        while std::time::Instant::now() < deadline {
            let state = std::fs::read_to_string(&waiting_task)?;
            if state
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<libc::c_long>().ok())
                == Some(libc::SYS_clock_nanosleep)
            {
                publication_waiting = true;
                break;
            }
            std::thread::yield_now();
        }
        anyhow::ensure!(
            publication_waiting,
            "public rebuild did not reach the coordinated writer wait"
        );
        match mutation {
            BackupMutation::ReplaceIdentical => {
                let before = std::fs::read(&backup)?;
                let old_identity = same_file::Handle::from_path(&backup)?;
                let detached = backup.with_extension("detached");
                std::fs::rename(&backup, &detached)?;
                std::fs::copy(&detached, &backup)?;
                anyhow::ensure!(
                    std::fs::read(&backup)? == before,
                    "replacement changed bytes"
                );
                anyhow::ensure!(
                    old_identity != same_file::Handle::from_path(&backup)?,
                    "replacement did not change file identity"
                );
            }
            BackupMutation::MemoryRow | BackupMutation::UnrelatedTable => {
                let changed = Connection::open(&backup)?;
                if matches!(mutation, BackupMutation::UnrelatedTable) {
                    changed.execute(
                        "UPDATE other_data SET state='changed after verification'",
                        [],
                    )?;
                } else {
                    changed.execute("UPDATE memory_item SET future=x'887766'", [])?;
                }
            }
        }
        conn.execute_batch("COMMIT")?;
        Ok(())
    });
    receiver
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    actor
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn same_inode_backup_corruption_is_caught_through_the_public_rebuild_call() {
    let f = Fixture::new();
    Connection::open(&f.db)
        .unwrap()
        .execute_batch(
            "CREATE TABLE padding(data BLOB); INSERT INTO padding VALUES (zeroblob(8388608));",
        )
        .unwrap();
    let options = f.options();
    let actor = start_backup_corruption(
        f.db.clone(),
        options.backup.clone(),
        BackupMutation::MemoryRow,
    );
    let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Good), &options).await;
    let injected = actor.join().unwrap();
    assert!(
        injected.is_ok(),
        "probe did not inject corruption: {injected:?}; rebuild={result:?}"
    );
    assert!(
        result.is_err(),
        "published despite a corrupt reserved backup"
    );
    f.old_index();
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn byte_identical_backup_replacement_is_refused_through_the_public_rebuild_call() {
    let f = Fixture::new();
    Connection::open(&f.db)
        .unwrap()
        .execute_batch(
            "CREATE TABLE padding(data BLOB); INSERT INTO padding VALUES (zeroblob(8388608));",
        )
        .unwrap();
    let options = f.options();
    let actor = start_backup_corruption(
        f.db.clone(),
        options.backup.clone(),
        BackupMutation::ReplaceIdentical,
    );
    let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Good), &options).await;
    let injected = actor.join().unwrap();
    assert!(
        injected.is_ok(),
        "probe did not replace backup: {injected:?}; rebuild={result:?}"
    );
    assert!(
        result.is_err(),
        "published despite replacement of the reserved backup"
    );
    f.old_index();
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn empty_rebuild_still_rechecks_every_backup_byte_before_commit() {
    let f = Fixture::new();
    let conn = Connection::open(&f.db).unwrap();
    conn.execute_batch(
        "DELETE FROM memory_item;
         CREATE TABLE other_data(state TEXT,padding BLOB);
         INSERT INTO other_data VALUES ('original',zeroblob(8388608));",
    )
    .unwrap();
    drop(conn);
    let options = f.options();
    let actor = start_backup_corruption(
        f.db.clone(),
        options.backup.clone(),
        BackupMutation::UnrelatedTable,
    );
    let result = rebuild_sqlite_memory_vectors(&f.db, Some(&Good), &options).await;
    let injected = actor.join().unwrap();
    assert!(
        injected.is_ok(),
        "probe did not inject corruption: {injected:?}; rebuild={result:?}"
    );
    assert!(
        result.is_err(),
        "empty rebuild skipped the final backup fingerprint"
    );
    let conn = Connection::open(&f.db).unwrap();
    assert_eq!(vector_dimensions(&conn).unwrap(), Some(768));
    assert_eq!(
        conn.query_row("SELECT state FROM other_data", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "original"
    );
}
