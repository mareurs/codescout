//! Real SQLite regression for Albert R-02; no model/server or real user data.
use codescout::memory::semantic_store::SemanticMemoryStore;
use codescout::memory::sqlite_semantic_store::SqliteVecSemanticMemoryStore;
use codescout::retrieval::memory_payload::{MemoryAnchor, SemanticMemory};
use std::process::Command;

#[tokio::test]
async fn dry_run_reconciles_the_existing_sqlite_memory_rows_without_loading_a_model() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteVecSemanticMemoryStore::at(dir.path().to_path_buf());
    let memory = SemanticMemory {
        project_id: "fixture".into(),
        bucket: "structured".into(),
        title: "original topic".into(),
        content: "The original memory text.".into(),
        anchors: vec![MemoryAnchor {
            path: "src/original.rs".into(),
        }],
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-02-01T00:00:00Z".into(),
    };
    store.upsert(&memory, &vec![0.25; 768]).await.unwrap();
    let db = dir.path().join("fixture.memories.db");
    assert!(db.is_file());
    let before = std::fs::read(&db).unwrap();
    let backup = dir.path().join("original-backup.db");
    let output = Command::new(env!("CARGO_BIN_EXE_codescout"))
        .args(["rebuild-memory-vectors", "--db"])
        .arg(&db)
        .args([
            "--model",
            "local-dir:/a/model/that/does/not/exist",
            "--dimensions",
            "384",
            "--backup",
        ])
        .arg(&backup)
        .arg("--dry-run")
        .env_clear()
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .env("XDG_DATA_HOME", dir.path().join("data"))
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "dry-run must recognize the vector-only repair and need no model load: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["records"], 1);
    assert_eq!(report["previous_dimensions"], 768);
    assert_eq!(report["requested_dimensions"], 384);
    assert_eq!(report["published_dimensions"], serde_json::Value::Null);
    assert_eq!(report["vectors_written"], 0);
    assert_eq!(report["dry_run"], true);
    assert!(!backup.exists());
    assert_eq!(std::fs::read(&db).unwrap(), before);
}

fn command(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_codescout"));
    cmd.env_clear()
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_STATE_HOME", dir.join("state"))
        .env("XDG_CACHE_HOME", dir.join("cache"));
    cmd
}

#[tokio::test]
async fn cli_refuses_invalid_model_dimensions_and_missing_database_without_changes() {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteVecSemanticMemoryStore::at(dir.path().to_path_buf());
    let memory = SemanticMemory {
        project_id: "fixture".into(),
        bucket: "structured".into(),
        title: "topic".into(),
        content: "original text".into(),
        anchors: vec![],
        created_at: "created".into(),
        updated_at: "updated".into(),
    };
    store.upsert(&memory, &vec![0.25; 768]).await.unwrap();
    let db = dir.path().join("fixture.memories.db");
    let backup = dir.path().join("backup.db");
    let original = std::fs::read(&db).unwrap();
    for (model, dim, dry_run, expected) in [
        ("ollama:some-model", "384", true, "local-dir"),
        ("local-dir:/absent/offline-snapshot", "0", true, "positive"),
        ("local-dir:/absent/offline-snapshot", "384", false, "Error"),
    ] {
        let mut cmd = command(dir.path());
        cmd.args(["rebuild-memory-vectors", "--db"])
            .arg(&db)
            .args(["--model", model, "--dimensions", dim, "--backup"])
            .arg(&backup);
        if dry_run {
            cmd.arg("--dry-run");
        }
        let result = cmd.output().unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read(&db).unwrap(), original);
        assert!(!backup.exists());
    }
    let missing = dir.path().join("missing.db");
    let result = command(dir.path())
        .args(["rebuild-memory-vectors", "--db"])
        .arg(&missing)
        .args([
            "--model",
            "local-dir:/absent/offline-snapshot",
            "--dimensions",
            "384",
            "--backup",
        ])
        .arg(&backup)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!missing.exists());
    assert!(!backup.exists());
}

#[test]
fn cli_help_requires_explicit_database_model_width_and_backup() {
    let dir = tempfile::tempdir().unwrap();
    let help = command(dir.path())
        .args(["rebuild-memory-vectors", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    for option in ["--db", "--model", "--dimensions", "--backup", "--dry-run"] {
        assert!(text.contains(option), "{text}");
    }
    assert!(!command(dir.path())
        .arg("rebuild-memory-vectors")
        .output()
        .unwrap()
        .status
        .success());
    let legacy = command(dir.path())
        .args(["migrate-memories", "--help"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&legacy.stdout).contains("rebuild-memory-vectors"));
}

#[test]
fn cli_dry_run_refuses_legacy_sqlite_schema_without_creating_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("legacy.db");
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch("CREATE TABLE memories(content TEXT)")
        .unwrap();
    let before = std::fs::read(&db).unwrap();
    let backup = dir.path().join("backup.db");
    let result = command(dir.path())
        .args(["rebuild-memory-vectors", "--db"])
        .arg(&db)
        .args([
            "--model",
            "local-dir:/absent/offline-snapshot",
            "--dimensions",
            "384",
            "--backup",
        ])
        .arg(&backup)
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("memory_item"));
    assert_eq!(std::fs::read(&db).unwrap(), before);
    assert!(!backup.exists());
}

#[tokio::test]
#[ignore = "requires an explicit offline AllMiniLML6V2Q snapshot in CODESCOUT_TEST_REBUILD_MODEL_DIR"]
async fn real_local_snapshot_cli_rebuilds_and_reopened_store_recalls() {
    use codescout::retrieval::embedder::{CodeEmbedder, CodeEmbedderAdapter};
    let snapshot = std::env::var("CODESCOUT_TEST_REBUILD_MODEL_DIR")
        .expect("set CODESCOUT_TEST_REBUILD_MODEL_DIR to a verified local snapshot");
    let model = format!("local-dir:{snapshot}");
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteVecSemanticMemoryStore::at(dir.path().to_path_buf());
    for (title, content) in [
        (
            "rust",
            "Rust ownership uses a borrow checker to verify lifetimes.",
        ),
        (
            "cake",
            "Bake a chocolate cake with vanilla frosting and sugar.",
        ),
    ] {
        let memory = SemanticMemory {
            project_id: "fixture".into(),
            bucket: "structured".into(),
            title: title.into(),
            content: content.into(),
            anchors: vec![],
            created_at: "original".into(),
            updated_at: "original".into(),
        };
        store.upsert(&memory, &vec![0.25; 768]).await.unwrap();
    }
    drop(store);
    let db = dir.path().join("fixture.memories.db");
    let before: Vec<(String, String)> = {
        let conn = rusqlite::Connection::open(&db).unwrap();
        let mut stmt = conn
            .prepare("SELECT point_id, json FROM memory_item ORDER BY point_id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    let backup = dir.path().join("original.db");
    let result = command(dir.path())
        .args(["rebuild-memory-vectors", "--db"])
        .arg(&db)
        .args(["--model", &model, "--dimensions", "384", "--backup"])
        .arg(&backup)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["records"], 2);
    assert_eq!(report["vectors_written"], 2);
    assert_eq!(report["published_dimensions"], 384);
    assert!(backup.is_file());
    let conn = rusqlite::Connection::open(&db).unwrap();
    let mut stmt = conn
        .prepare("SELECT point_id, json FROM memory_item ORDER BY point_id")
        .unwrap();
    let after: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(after, before);
    let inner = codescout_embed::create_embedder_with_config(&model, None, None)
        .await
        .unwrap();
    let adapter = CodeEmbedderAdapter::new(inner, Some(384)).unwrap();
    let query = adapter
        .embed_dense_one("Rust ownership uses a borrow checker to verify lifetimes.")
        .await
        .unwrap();
    let reopened = SqliteVecSemanticMemoryStore::at(dir.path().to_path_buf());
    let hits = reopened.search("fixture", &query, 1, None).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].memory.title, "rust");
}
