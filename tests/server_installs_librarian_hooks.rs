//! Wiring proof for the two process-wide hooks the server installs when it builds a librarian
//! runtime — `install_catalog_frontmatter_sync` (BL-48) and `install_augmentation_guard_oracle`
//! (BL-33), both in `CodeScoutServer::from_parts_with_env`.
//!
//! ## What was uncovered, and why
//!
//! Each hook has unit tests on the object it installs (`CatalogFrontmatterSyncer`,
//! `CatalogAugmentationOracle`) and `tests/edit_markdown_catalog_sync.rs` proves `edit_file`
//! calls whatever hook is installed. Nothing proved the SERVER installs the real one:
//! deleting the install line leaves every one of those green, because each installs its own
//! recorder or builds the object directly. A guard that is written, tested and never plugged
//! in reads as fully covered — `bug-fix-session-log:W-73`. The server-side install was named
//! as untested in
//! `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`
//! and tracked by `docs/issues/2026-09-24-residual-catalog-write-through-install-test.md`.
//!
//! ## Why a spawned binary and not `CodeScoutServer::from_parts_with_env`
//!
//! Both hooks live in process-global slots with last-writer-wins semantics. A unit test in
//! `src/server.rs` would race every other server-building test in that binary (the reason
//! `tests/edit_markdown_catalog_sync.rs` is its own binary). An in-process integration test
//! has no such race but has no way in either: `ServerEnv` is constructible from here, but
//! `call_tool_inner` is private and the `tools` list is a private field, so a built server
//! cannot be asked to run a tool without widening visibility for tests alone.
//!
//! The spawned binary is the production path end to end — `ServerEnv::from_env`,
//! `try_build_runtime_with`, the install calls, the real `doc` and `edit_file` tools — in its
//! own process, so the slots are its own. The observer is the catalog file itself, read with
//! a second SQLite connection, so a lazy re-sync in a read tool cannot flatter the result.
//!
//! ## How a file reaches the hooks
//!
//! `doc(create)` STAMPS an `id:` into the frontmatter, and the markdown guard refuses direct
//! frontmatter writes on a stamped file — so a freshly created artifact never reaches the sync
//! hook, and (being stamped) is refused regardless of the oracle. The population both hooks
//! exist for is the catalogued file whose frontmatter carries NO id, so each test creates the
//! artifact through `doc` and then strips the `id:` line from the file, leaving the catalog row
//! (its identity is the path, not the stamp).
//!
//! Run with: `cargo test --test server_installs_librarian_hooks`

use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Same `CARGO_BIN_EXE_` guarantee as `tests/cross_process_write_lock.rs`: Cargo builds the
/// binary before this runs, so there is no missing-binary skip that could pass without running.
fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codescout"))
}

/// One spawned codescout MCP server over stdio, with a scratch project, workspace and catalog.
struct Server {
    // Held, never read: dropping it kills the child (`kill_on_drop`), which is the cleanup.
    _child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
    next_id: u64,
    project: PathBuf,
    db: PathBuf,
    // Held so the scratch dirs outlive the child.
    _scratch: tempfile::TempDir,
    _state: tempfile::TempDir,
}

impl Server {
    async fn start() -> Self {
        let scratch = tempfile::tempdir().unwrap();
        // Canonical: the catalog identity is derived from the path, and the syncer
        // canonicalizes before deriving it. A symlinked temp prefix would make the two differ.
        let root = std::fs::canonicalize(scratch.path()).unwrap();
        let project = root.join("proj");
        std::fs::create_dir_all(project.join(".codescout")).unwrap();
        let ws = root.join("workspace.toml");
        // The legacy `[[roots]]` form: it is what `doc(create, repo=…)` resolves a root from.
        std::fs::write(
            &ws,
            format!(
                "[[roots]]\nname = \"r\"\npath = \"{}\"\n",
                project.display()
            ),
        )
        .unwrap();
        let db = root.join("catalog.db");

        // Guide-hint ledger and session id must not reach the developer's real state dir —
        // see the comment on the spawn in `tests/cross_process_write_lock.rs`.
        let state = tempfile::tempdir().unwrap();
        let mut cmd = Command::new(binary_path());
        cmd.args(["start", "--project", project.to_str().unwrap()])
            .env("XDG_STATE_HOME", state.path())
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env("LIBRARIAN_ENABLED", "1")
            .env("LIBRARIAN_WORKSPACE", &ws)
            .env("LIBRARIAN_DB", &db)
            .env("LIBRARIAN_CWD", &project)
            .env_remove("LIBRARIAN_EMBED_MODEL")
            .env_remove("LIBRARIAN_EMBED_URL")
            .env_remove("LIBRARIAN_EMBED_API_KEY")
            // No embedder: `doc(create)` would otherwise try to embed through whatever the
            // developer's shell configures. `tests/cli_doc.rs` clears the same family.
            .env("CODESCOUT_ENV_FILE", root.join("no-startup.env"))
            .env("CODESCOUT_QDRANT_URL", "http://127.0.0.1:1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        for name in codescout::config::embedding_env::all_names() {
            cmd.env_remove(name);
        }
        let mut child = cmd.spawn().expect("failed to spawn codescout");
        let stdin = child.stdin.take().unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());

        let mut s = Server {
            _child: child,
            stdin,
            out,
            next_id: 1,
            project,
            db,
            _scratch: scratch,
            _state: state,
        };
        s.handshake().await;
        s
    }

    async fn send(&mut self, msg: &serde_json::Value) {
        let mut line = msg.to_string();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    /// Read newline-delimited messages until the one with `id`. Lines that are not JSON are
    /// skipped: a library may print prose to stdout before the protocol starts.
    async fn recv_id(&mut self, id: u64) -> serde_json::Value {
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout(Duration::from_secs(60), self.out.read_line(&mut line))
                .await
                .expect("server did not answer within 60 s")
                .expect("failed to read from server stdout");
            assert!(n > 0, "server stdout closed unexpectedly");
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                    return v;
                }
            }
        }
    }

    async fn handshake(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0.1"}
            }
        }))
        .await;
        self.recv_id(id).await;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized", "params": {}
        }))
        .await;
    }

    /// Call a tool; returns the text of the FIRST content block, which is the tool's own
    /// result — the server appends guide bodies as further blocks on a session's first
    /// calls, and those are not part of the answer. A JSON-RPC level error fails the test.
    async fn call(&mut self, name: &str, args: serde_json::Value) -> String {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }))
        .await;
        let resp = self.recv_id(id).await;
        assert!(
            resp.get("error").is_none(),
            "{name} failed at the protocol level: {resp}"
        );
        resp["result"]["content"][0]["text"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| resp.to_string())
    }

    /// `doc(create)` a stamped artifact, then strip its `id:` line so the file is the plain
    /// catalogued kind the hooks are for. Returns `(catalog id, absolute path)`.
    async fn create_unstamped(
        &mut self,
        rel_path: &str,
        kind: &str,
        extra: serde_json::Value,
    ) -> (String, PathBuf) {
        let mut args = serde_json::json!({
            "action": "create", "repo": "r", "rel_path": rel_path,
            "kind": kind, "title": "T", "body": "# T\n\nbody text\n"
        });
        for (k, v) in extra.as_object().into_iter().flatten() {
            args[k] = v.clone();
        }
        let text = self.call("doc", args).await;
        let created: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| panic!("doc(create) did not return JSON: {text}"));
        let id = created["id"]
            .as_str()
            .unwrap_or_else(|| panic!("doc(create) returned no id: {text}"))
            .to_string();

        let path = self.project.join(rel_path);
        let content = std::fs::read_to_string(&path).expect("created file must exist");
        // The YAML writer quotes an id that would parse as a number (`id: '9ac1…'`) and leaves
        // the rest bare, so match the key and the id separately rather than the exact text.
        let is_id_line = |l: &str| l.starts_with("id:") && l.contains(&id);
        assert!(
            content.lines().any(is_id_line),
            "premise: doc(create) stamps the id into the frontmatter, got:\n{content}"
        );
        let stripped: String = content
            .lines()
            .filter(|l| !is_id_line(l))
            .map(|l| format!("{l}\n"))
            .collect();
        std::fs::write(&path, stripped).unwrap();
        (id, path)
    }

    /// The catalog row's `status`, read straight from the SQLite file with a second connection.
    fn catalog_status(&self, id: &str) -> Option<String> {
        let conn = rusqlite::Connection::open_with_flags(
            &self.db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("the catalog file must be openable");
        conn.query_row("SELECT status FROM artifact WHERE id = ?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .ok()
    }
}

/// BL-48, through the server: a direct frontmatter write on a plain catalogued file must move
/// the catalog row. Deleting `install_catalog_frontmatter_sync` from
/// `CodeScoutServer::from_parts_with_env` leaves the file saying `fixed` and the row saying
/// `open` — the triage query (`doc find kind=bug status=…`) then reports a value the file
/// contradicts.
#[tokio::test]
async fn the_server_wires_a_frontmatter_write_through_to_the_catalog_row() {
    let mut server = Server::start().await;
    let (id, path) = server
        .create_unstamped("docs/issues/a-bug.md", "bug", serde_json::json!({}))
        .await;

    // The observer works and the row exists, before anything is edited. Without this the
    // final assertion could be satisfied by a reader that returns `None` for everything.
    assert_eq!(
        server.catalog_status(&id).as_deref(),
        Some("open"),
        "premise: a new bug is catalogued as `open`"
    );

    let reply = server
        .call(
            "edit_file",
            serde_json::json!({
                "path": "docs/issues/a-bug.md",
                "frontmatter": { "set": { "status": "fixed" } }
            }),
        )
        .await;
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(
        on_disk.contains("status: fixed"),
        "premise: the frontmatter write must have landed in the file.\nreply: {reply}\nfile:\n{on_disk}"
    );

    assert_eq!(
        server.catalog_status(&id).as_deref(),
        Some("fixed"),
        "the file now says `fixed`; the catalog row must follow it. A row still saying \
         `open` means the server never installed the catalog write-through (BL-48)"
    );
}

/// BL-33, through the server: an AUGMENTED artifact whose frontmatter carries no id is
/// protected only by the oracle that asks the catalog — the stamp check cannot see it. With
/// `install_augmentation_guard_oracle` removed from `from_parts_with_env`, a direct edit lands
/// on what is only a rendered snapshot, and the next refresh silently overwrites it.
///
/// The plain artifact is the positive twin: the same edit on a catalogued file that is NOT
/// augmented must go through, so the refusal below is attributable to augmentation and not to
/// some other reason `edit_file` would refuse a `.md` here.
#[tokio::test]
async fn the_server_wires_the_augmentation_oracle_into_the_markdown_guard() {
    let mut server = Server::start().await;
    let edit = |p: &str| {
        serde_json::json!({
            "path": p, "old_string": "body text", "new_string": "changed text"
        })
    };

    let (_, plain) = server
        .create_unstamped("docs/plain.md", "spec", serde_json::json!({}))
        .await;
    let (_, augmented) = server
        .create_unstamped(
            "docs/tracked.md",
            "tracker",
            serde_json::json!({ "augment": { "prompt": "keep this fresh" } }),
        )
        .await;

    let reply = server.call("edit_file", edit("docs/plain.md")).await;
    assert!(
        std::fs::read_to_string(&plain)
            .unwrap()
            .contains("changed text"),
        "positive twin: a catalogued file that is not augmented stays directly editable.\nreply: {reply}"
    );

    let reply = server.call("edit_file", edit("docs/tracked.md")).await;
    let after = std::fs::read_to_string(&augmented).unwrap();
    assert!(
        after.contains("body text") && !after.contains("changed text"),
        "an augmented artifact's file is a rendered snapshot and must refuse a direct edit — \
         if it landed, the server never installed the augmentation oracle (BL-33).\nreply: {reply}\nfile:\n{after}"
    );
    assert!(
        reply.contains("augmented"),
        "the refusal must say why, so the caller reaches for `doc(update)`: {reply}"
    );
}
