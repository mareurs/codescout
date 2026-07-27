# Index Lock + Embedder Batching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop concurrent `codescout index` runs from duplicating the entire embedding workload, then raise embedding throughput by batching and pipelining requests the servers were already sized for.

**Architecture:** Two sequenced stages. Stage 1 adds a per-project `flock` acquired at the top of `RetrievalClient::sync_project`, keyed on `project_id` and stored outside any repo. Stage 2 reworks `EmbedderHttp::embed_batch` to discover the sparse server's real per-request cap from its `/info` endpoint and issue sub-batches through a bounded, order-preserving stream instead of one at a time. Stage 1 lands first because every throughput number in the spec was measured while four indexers shared the servers — a clean Stage 2 measurement is impossible until duplication is.

**Tech Stack:** Rust, tokio, `fs4 0.12` (advisory file locks), `futures 0.3` (`stream::buffered`), `reqwest 0.13`, `sha2`. Tests: `mockito 1`, `tempfile 3`, `#[tokio::test]`.

**Spec:** `docs/superpowers/specs/2026-07-27-embedder-batch-concurrency-design.md`
**Bugs:** `docs/issues/2026-07-25-concurrent-index-no-project-lock.md` (Stage 1), `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (context only — not fixed here)

## Global Constraints

- **Pre-commit gate, every task:** `cargo fmt`, then `cargo clippy -- -D warnings`, then `cargo test`. A task is not done until all three are clean.
- **Branch:** all work on `experiments`. `master` is protected — never commit there.
- **Error handling:** expected, input-driven failures return `RecoverableError` (`src/tools/core/types.rs:227`) so the MCP layer emits `isError: false`. Genuine failures use `anyhow::bail!` / `?`. Full tree: `get_guide("error-handling")`.
- **`fs4` is pinned at `0.12`** (`Cargo.toml:45`), where `try_lock_exclusive(&self) -> std::io::Result<()>`. In `fs4 0.13+` this returns `Result<bool>` and `.context(...)?` would silently discard a failed lock acquisition. Do **not** bump `fs4` as part of this work.
- **No echo writes:** write tools return `json!("ok")`, not the written content.
- Existing tests must keep passing. Named explicitly where relevant: `stream_index_force_reembeds_all_present_chunks`, `chunk_id_normalizes_native_separators` (both `src/retrieval/sync.rs`).

---

## File Structure

| File | Responsibility | Stage |
|---|---|---|
| `src/retrieval/index_lock.rs` | **New.** Owns the per-project index lock: path derivation, acquisition, RAII release. Self-contained, no knowledge of syncing. | 1 |
| `src/retrieval/mod.rs` | **Modify.** Register `pub mod index_lock;`. | 1 |
| `src/retrieval/sync.rs` | **Modify.** Acquire the lock at the top of `sync_project` (line ~196), before `chunk_refs`. Three lines; no other change. | 1 |
| `src/retrieval/embedder.rs` | **Modify.** Split `embed_batch` into a per-sub-batch worker, an order-preserving concurrent driver, and lazy `/info` cap discovery. All three live here — they change together and share `EmbedderHttp`'s private state. | 2 |

`index_lock.rs` is a new file rather than a function inside `sync.rs` for one concrete reason: the lock must be unit-testable without constructing a `RetrievalClient` (which needs a live Qdrant connection). A free function in its own module is directly testable; a private step inside `sync_project` is not.

---

## Stage 1 — Per-project index lock

### Task 1: `index_lock` module

**Files:**
- Create: `src/retrieval/index_lock.rs`
- Modify: `src/retrieval/mod.rs`
- Test: `src/retrieval/index_lock.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub struct IndexLock` — RAII handle; releases the `flock` on drop.
  - `pub fn IndexLock::path(&self) -> &std::path::Path`
  - `pub fn lock_path(project_id: &str) -> std::path::PathBuf`
  - `pub fn acquire(project_id: &str) -> anyhow::Result<IndexLock>`

**Design notes the implementer must not "fix":**

1. **Not `.codescout/write.lock`.** That file is locked per write-tool call by `WriteGuard` (`src/agent/write_guard.rs`). An index holding it for hours would block every edit tool — strictly worse than the duplication being prevented. These are different locks protecting different things.
2. **Keyed on `project_id`, not on `root`.** The contended resource is the `(collection, project_id)` pair in Qdrant. Library syncs (`src/tools/semantic/index.rs:133`, `src/agent/mod.rs:1587`) pass a library checkout as `root` with their own `project_id`; creating a `.codescout/` directory inside a third-party checkout would pollute it, which the codebase already avoids deliberately (see `SyncOpts::record_index_state`). **This deviates from the spec's `.codescout/index.lock`** — the spec did not consider the library-sync path.

2b. **The lock file lives in `crate::socket_discovery::per_user_runtime_dir()`, NOT bare `std::env::temp_dir()`.** Revised 2026-07-27 after review. A predictable path in world-writable `/tmp` is exploitable two ways: a local user pre-creates the path as a symlink and `set_len(0)` truncates the victim's file, or they simply hold the flock and every index run reports "already running" while `pgrep` shows nothing. `per_user_runtime_dir()` is already cross-platform — `XDG_RUNTIME_DIR` else `/tmp/codescout-{uid}` at `0o700` on Unix, `std::env::temp_dir()` on Windows (already per-user there). codescout's mux and peer locks already use it (`src/lsp/mux/mod.rs:23-29`, `src/socket_discovery.rs:49-55`).

   Two consequences to accept, not "fix": inside a directory we own at `0o700` an attacker cannot plant the symlink, so **no `mode(0o600)` and no `O_NOFOLLOW` are needed** — both would require `cfg(unix)` gating and a `libc` call for no gain. And cross-user exclusion is given up on Unix; that is the same trade codescout already accepted for its mux and peer locks, and bare `temp_dir()` never provided it on Windows anyway, so this makes the two platforms consistent rather than losing a property that was uniformly held.
3. **Open without truncating, then truncate after acquiring.** `File::create` truncates immediately, which would erase the *current holder's* PID line before we even try to lock. `src/lsp/mux/process.rs:76` has this latent bug; do not copy it.
4. **No stale-lock recovery.** `flock` is released by the kernel when a process dies. A leftover lock *file* is inert. Do not add PID-liveness checks — Task 1 Step 1 pins this with a test.

- [ ] **Step 1: Write the failing tests**

Create `src/retrieval/index_lock.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Unique per test so concurrent `cargo test` threads never share a lock file.
    fn unique_project(tag: &str) -> String {
        format!("test-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id())
    }

    #[test]
    fn acquire_succeeds_for_fresh_project() {
        let pid = unique_project("fresh");
        let lock = acquire(&pid).expect("first acquire must succeed");
        assert!(lock.path().exists(), "lock file should exist on disk");
    }

    #[test]
    fn second_acquire_fails_while_first_is_held() {
        let pid = unique_project("contend");
        let _first = acquire(&pid).expect("first acquire must succeed");
        let second = acquire(&pid);
        assert!(
            second.is_err(),
            "a second acquire for the same project_id must fail while the first is held"
        );
        let msg = format!("{:#}", second.unwrap_err());
        assert!(
            msg.contains("already running"),
            "error must tell the operator what is happening, got: {msg}"
        );
    }

    #[test]
    fn different_projects_do_not_contend() {
        let a = unique_project("proj-a");
        let b = unique_project("proj-b");
        let _lock_a = acquire(&a).expect("project a");
        let _lock_b = acquire(&b).expect("project b must not contend with a");
    }

    #[test]
    fn lock_is_released_on_drop() {
        let pid = unique_project("release");
        {
            let _held = acquire(&pid).expect("first acquire");
        } // drop releases
        acquire(&pid).expect("must be re-acquirable after the guard drops");
    }

    /// A leftover lock *file* must never block a new run. flock is released by the
    /// kernel on process death, so this passes with no recovery logic — the test
    /// exists so nobody adds PID-liveness checks "to be safe".
    ///
    /// The planted PID is our OWN, deliberately: a liveness check would pass if we
    /// wrote a dead pid like 999999 (above `pid_max` on most Linux configs), so that
    /// value would not actually pin the stated intent.
    #[test]
    fn preexisting_lock_file_does_not_block() {
        let pid = unique_project("stale");
        let path = lock_path(&pid);
        std::fs::write(&path, format!("{}\n", std::process::id()))
            .expect("simulate a lock file left by a dead process");
        let lock = acquire(&pid).expect("a stale lock file must not block acquisition");

        // The PID write must TRUNCATE, not overwrite in place. Without `set_len(0)`
        // a shorter pid leaves the old tail behind (e.g. "42\n999\n"), and an
        // operator inspecting the lock during an incident reads a bogus second line.
        drop(lock);
        let contents = std::fs::read_to_string(&path).expect("read lock file");
        assert_eq!(
            contents.trim(),
            std::process::id().to_string(),
            "lock file must contain exactly the holder's pid, with no stale tail"
        );
    }

    #[test]
    fn lock_path_is_deterministic_and_filename_safe() {
        let a = lock_path("some/project:with weird*chars");
        let b = lock_path("some/project:with weird*chars");
        assert_eq!(a, b, "lock_path must be deterministic");

        let name = a.file_name().unwrap().to_str().unwrap();
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'),
            "filename must be safe regardless of project_id, got: {name}"
        );
        assert_ne!(
            lock_path("project-one"),
            lock_path("project-two"),
            "distinct project ids must map to distinct lock files"
        );
    }
}
```

Register the module — add to `src/retrieval/mod.rs` alongside the other `pub mod` lines:

```rust
pub mod index_lock;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib retrieval::index_lock`
Expected: FAIL to compile — `cannot find function 'acquire' in this scope`, `cannot find function 'lock_path'`, `cannot find type 'IndexLock'`.

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)] mod tests` block in `src/retrieval/index_lock.rs`:

```rust
//! Per-project exclusive lock for the retrieval index pass.
//!
//! Without it, N concurrent `codescout index --project <same>` runs each execute
//! the full `stream_index` pipeline against the same Qdrant collection and
//! `project_id`, duplicating the entire embedding workload. Observed 2026-07-27
//! with four simultaneous runs (3h24m / 2h02m / 1h08m / 1h05m), all orphaned to
//! `systemd --user`. See docs/issues/2026-07-25-concurrent-index-no-project-lock.md
//!
//! Deliberately NOT `.codescout/write.lock`: that lock is taken per write-tool
//! call by `crate::agent::write_guard::WriteGuard`. An index holding it for hours
//! would block every edit tool for the duration.
//!
//! Keyed on `project_id` rather than on the filesystem root, and stored outside
//! any repository: the contended resource is the `(collection, project_id)` pair
//! in Qdrant, and library syncs pass a third-party checkout as `root` that must
//! not gain a `.codescout/` directory.

use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::path::{Path, PathBuf};

/// RAII handle for the per-project index lock.
///
/// `#[derive(Debug)]` is required: the contention test calls `unwrap_err()` on
/// `Result<IndexLock, _>`, and `Result::unwrap_err` needs `T: Debug`.
///
/// The `flock` is released on drop, and by the kernel if the process dies — so a
/// leftover lock file is inert and needs no recovery logic.
#[derive(Debug)]
pub struct IndexLock {
    file: File,
    path: PathBuf,
}

impl IndexLock {
    /// Filesystem path this lock occupies. For diagnostics and tests.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for IndexLock {
    fn drop(&mut self) {
        // Explicit unlock documents intent; closing the fd would also release it.
        let _ = FileExt::unlock(&self.file);
    }
}

/// Deterministic lock-file path for `project_id`.
///
/// Hashed so any `project_id` — including one with path separators or spaces —
/// maps to a safe, fixed-length filename.
///
/// Sited in the per-user runtime directory rather than bare `temp_dir()`: a
/// predictable path in world-writable `/tmp` lets a local user pre-create it as a
/// symlink (which `set_len(0)` below would then truncate) or simply hold the flock
/// to wedge every index run. `per_user_runtime_dir()` handles both platforms —
/// `0o700` dir on Unix, already-per-user `temp_dir()` on Windows.
pub fn lock_path(project_id: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(project_id.as_bytes());
    let digest = format!("{:x}", h.finalize());
    crate::socket_discovery::per_user_runtime_dir()
        .join(format!("codescout-index-{}.lock", &digest[..16]))
}

/// Acquire the exclusive index lock for `project_id`, or fail immediately.
///
/// Fail-fast rather than queue. A queued second run would be nearly free — every
/// `chunk_id` would already be present, so nothing re-embeds — but it would hide
/// the duplication instead of surfacing it, which is how this bug went unnoticed
/// for hours.
pub fn acquire(project_id: &str) -> Result<IndexLock> {
    let path = lock_path(project_id);

    // create(true) + truncate(false): `File::create` truncates on open, which
    // would erase the current holder's PID line before we even try to lock.
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open index lock file: {}", path.display()))?;

    file.try_lock_exclusive().with_context(|| {
        format!(
            "another codescout index is already running for project '{project_id}' \
             (lock: {}). Wait for it to finish, or inspect with \
             `pgrep -af 'codescout index'`.",
            path.display()
        )
    })?;

    // PID for diagnostics, mirroring src/lsp/mux/process.rs:81. Only after the
    // lock is held, so we never clobber another holder's record. Best-effort:
    // a failed write must not fail an otherwise-valid lock.
    use std::io::Write;
    let _ = file.set_len(0);
    let _ = writeln!(&file, "{}", std::process::id());

    Ok(IndexLock { file, path })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib retrieval::index_lock`
Expected: PASS, 6 tests.

If `second_acquire_fails_while_first_is_held` fails, check `fs4`'s version — at `0.13+`, `try_lock_exclusive` returns `Result<bool>` and `.context(...)?` discards the failure. `Cargo.toml:45` must read `fs4 = "0.12"`.

- [ ] **Step 5: Run the full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/index_lock.rs src/retrieval/mod.rs
git commit -m "feat(retrieval): add per-project index lock module

Keyed on project_id and stored in the temp dir, not .codescout/ --
library syncs pass a third-party checkout as root and must not gain a
.codescout/ directory. Distinct from write.lock, which is per
write-tool call and would block every edit tool for a multi-hour index.

Fail-fast rather than queue: a queued run would be nearly free but
would hide the duplication that motivated this.

Refs docs/issues/2026-07-25-concurrent-index-no-project-lock.md"
```

---

### Task 2: Acquire the lock in `sync_project`

**Files:**
- Modify: `src/retrieval/sync.rs` (in `RetrievalClient::sync_project`, which begins at line ~196)
- Test: manual two-process verification (see Step 3) — the unit-level behaviour is already covered by Task 1

**Interfaces:**
- Consumes: `crate::retrieval::index_lock::{acquire, IndexLock}` from Task 1.
- Produces: no new public API. All five existing `sync_project` call sites inherit the lock: `src/tools/semantic/index.rs:133` and `:322`, `src/agent/mod.rs:1587`, `src/bin/sync_project.rs:33`, `src/main.rs:278`.

- [ ] **Step 1: Add the acquisition**

In `src/retrieval/sync.rs`, inside `sync_project`, immediately after the `chunk_target` / `flush_batch` env resolution and the `"retrieval sync starting"` log, and **before** `let started = std::time::Instant::now();`:

```rust
        // Serialize index passes per project. MUST be acquired before the
        // `chunk_refs` call below: that read establishes the drift baseline, and
        // `stream_index` then mutates it. Two overlapping runs would each diff
        // against a snapshot the other is invalidating.
        //
        // Bound to `_index_lock` (not `_`) so it lives until the end of this
        // function — `let _ = ...` would drop it immediately and release the lock.
        let _index_lock = crate::retrieval::index_lock::acquire(project_id)?;
```

- [ ] **Step 2: Verify it compiles and nothing regressed**

Run: `cargo test --lib retrieval::`
Expected: PASS. `stream_index_force_reembeds_all_present_chunks` still passes — it calls `stream_index` directly and never touches the lock.

- [ ] **Step 3: Verify the real behaviour with two processes**

```bash
cargo build --release
./target/release/codescout index --project /tmp/some-small-repo &
./target/release/codescout index --project /tmp/some-small-repo
```

Expected: the second invocation exits non-zero with
`another codescout index is already running for project '<id>'`.
Then confirm only one survives:

```bash
pgrep -af 'codescout index'
```

Expected: exactly one process.

- [ ] **Step 4: Run the full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/retrieval/sync.rs
git commit -m "fix(retrieval): serialize index passes per project

sync_project now acquires the per-project index lock before reading the
drift baseline, so overlapping runs cannot each diff against a snapshot
the other invalidates. Covers all five call sites.

Closes docs/issues/2026-07-25-concurrent-index-no-project-lock.md"
```

---

## Stage 2 — Batch sizing and bounded concurrency

### Task 3: Extract `embed_one_batch` (pure refactor)

**Files:**
- Modify: `src/retrieval/embedder.rs:328-440`

**Interfaces:**
- Consumes: nothing from Stage 1.
- Produces: `async fn EmbedderHttp::embed_one_batch(&self, chunk: Vec<String>) -> Result<Vec<EmbedOutput>>` — private to the impl. **Takes the sub-batch by value on purpose:** Task 5 drives it from a `futures` stream, and a returned future that borrows its input cannot be expressed with a single generic `Fut` type parameter. Owning the `Vec` means the future borrows only `&self`, whose lifetime is fixed for the whole call. The clone is one `Vec<String>` per sub-batch against an HTTP round-trip — immaterial. Embeds exactly one sub-batch: dense and sparse legs joined with `tokio::try_join!`, empty inputs omitted from the sparse request and re-expanded, dim validated. Task 5 calls this.

This task changes **no behaviour**. It moves the body of the existing `for chunk in texts.chunks(BATCH)` loop into a method so Task 5 can drive it concurrently and so it can be tested in isolation.

- [ ] **Step 1: Extract the method**

Add to `impl EmbedderHttp`, taking the entire body of the existing hybrid loop verbatim, with `inputs` derived from the `&[String]` parameter:

```rust
    /// Embed exactly one sub-batch: dense and sparse legs concurrently.
    ///
    /// Split out of `embed_batch` so the sub-batches can be pipelined (see the
    /// `buffered` driver there) and so this unit is testable on its own.
    async fn embed_one_batch(&self, chunk: Vec<String>) -> Result<Vec<EmbedOutput>> {
        let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        // ... body of the existing `for chunk in texts.chunks(BATCH)` loop,
        // unchanged, but pushing into a local `out` and returning it instead of
        // appending to the outer `out`.
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let mut out = Vec::with_capacity(inputs.len());
        // <existing nonempty / sparse_body / try_join! / re-expand / dim-check code>
        Ok(out)
    }
```

Then make `embed_batch`'s hybrid path call it:

```rust
        let batch = 8; // replaced in Task 4
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(batch) {
            out.extend(self.embed_one_batch(chunk.to_vec()).await?);
        }
        Ok(out)
```

- [ ] **Step 2: Verify no behaviour changed**

Run: `cargo test --lib retrieval::`
Expected: PASS, with no test modified. If any test needed changing, the extraction was not faithful — revert and redo.

- [ ] **Step 3: Run the full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/retrieval/embedder.rs
git commit -m "refactor(retrieval): extract embed_one_batch from embed_batch

Pure extraction, no behaviour change. Prepares the sub-batch unit for
concurrent driving and isolated testing."
```

---

### Task 4: Lazy `/info` batch-cap discovery

**Files:**
- Modify: `src/retrieval/embedder.rs` (`EmbedderHttp` struct, `with_config`, new method)
- Test: `src/retrieval/embedder.rs` inline tests, using `mockito`

**Interfaces:**
- Consumes: `embed_one_batch` from Task 3.
- Produces: `async fn EmbedderHttp::resolve_batch_size(&self) -> usize` — memoised per instance. Task 5 calls it once per `embed_batch`.

Resolution order: `CODESCOUT_EMBED_BATCH` → `/info`'s `max_client_batch_size` → `8`. The `8` fallback preserves today's behaviour exactly whenever discovery cannot answer.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/retrieval/embedder.rs`:

```rust
    /// RAII env guard — mirrors src/librarian/indexer.rs:1074. Without it a test
    /// mutating CODESCOUT_EMBED_BATCH leaks the value into the rest of the process.
    struct EnvGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, original }
        }
        fn unset(key: &'static str) -> Self {
            let original = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[tokio::test]
    async fn batch_size_discovered_from_info() {
        let _g = EnvGuard::unset("CODESCOUT_EMBED_BATCH");
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"max_client_batch_size":32,"max_input_length":512}"#)
            .create_async()
            .await;

        let e = EmbedderHttp::new("http://unused.invalid", server.url(), 768);
        assert_eq!(e.resolve_batch_size().await, 32);
    }

    #[tokio::test]
    async fn batch_size_falls_back_to_8_when_info_missing() {
        let _g = EnvGuard::unset("CODESCOUT_EMBED_BATCH");
        let mut server = mockito::Server::new_async().await;
        let _m = server.mock("GET", "/info").with_status(404).create_async().await;

        let e = EmbedderHttp::new("http://unused.invalid", server.url(), 768);
        assert_eq!(
            e.resolve_batch_size().await,
            8,
            "a non-TEI sparse server must keep today's behaviour"
        );
    }

    #[tokio::test]
    async fn env_override_wins_over_info() {
        let _g = EnvGuard::set("CODESCOUT_EMBED_BATCH", "4");
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_body(r#"{"max_client_batch_size":32}"#)
            .create_async()
            .await;

        let e = EmbedderHttp::new("http://unused.invalid", server.url(), 768);
        assert_eq!(e.resolve_batch_size().await, 4);
    }

    #[tokio::test]
    async fn batch_size_is_memoised() {
        let _g = EnvGuard::unset("CODESCOUT_EMBED_BATCH");
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_body(r#"{"max_client_batch_size":32}"#)
            .expect(1)
            .create_async()
            .await;

        let e = EmbedderHttp::new("http://unused.invalid", server.url(), 768);
        assert_eq!(e.resolve_batch_size().await, 32);
        assert_eq!(e.resolve_batch_size().await, 32);
        m.assert_async().await; // exactly one /info request
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib retrieval::embedder`
Expected: FAIL to compile — `no method named 'resolve_batch_size'`.

- [ ] **Step 3: Implement discovery**

Add the field to `EmbedderHttp`:

```rust
    /// Memoised sparse-server per-request cap. Resolved on first use because
    /// `EmbedderHttp::new` is synchronous and the probe is async.
    sparse_batch_cap: tokio::sync::OnceCell<usize>,
```

Initialise it in `with_config` alongside the other fields:

```rust
            sparse_batch_cap: tokio::sync::OnceCell::new(),
```

Add the method to `impl EmbedderHttp`:

```rust
    /// Per-request input count for both legs.
    ///
    /// `CODESCOUT_EMBED_BATCH` → the sparse server's advertised
    /// `max_client_batch_size` → 8. The 8 preserves the historical value for any
    /// server that does not answer `/info`.
    ///
    /// Discovered rather than hardcoded on purpose: the previous `const BATCH = 8`
    /// was justified by a comment citing a cap that only `sparse-amd` ever
    /// imposed, and it silently survived that service's removal.
    async fn resolve_batch_size(&self) -> usize {
        const FALLBACK: usize = 8;
        *self
            .sparse_batch_cap
            .get_or_init(|| async {
                if let Some(n) = std::env::var("CODESCOUT_EMBED_BATCH")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|&n| n > 0)
                {
                    tracing::info!(batch = n, source = "env", "embed batch size");
                    return n;
                }
                let url = format!("{}/info", self.sparse_base);
                let discovered = async {
                    let resp = self.client.get(&url).send().await.ok()?;
                    if !resp.status().is_success() {
                        return None;
                    }
                    let v: serde_json::Value = resp.json().await.ok()?;
                    v.get("max_client_batch_size")?.as_u64().map(|n| n as usize)
                }
                .await
                .filter(|&n| n > 0);

                match discovered {
                    Some(n) => {
                        tracing::info!(batch = n, source = "info", "embed batch size");
                        n
                    }
                    None => {
                        tracing::info!(batch = FALLBACK, source = "fallback", "embed batch size");
                        FALLBACK
                    }
                }
            })
            .await
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib retrieval::embedder`
Expected: PASS, 4 new tests.

- [ ] **Step 5: Run the full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/retrieval/embedder.rs
git commit -m "feat(retrieval): discover embed batch size from sparse /info

The running sparse server advertises max_client_batch_size 32; the
client hardcoded 8, citing a cap only the deleted sparse-amd service
imposed. Discover it instead, with CODESCOUT_EMBED_BATCH as an escape
hatch and 8 as the fallback for non-TEI servers."
```

---

### Task 5: Bounded, order-preserving concurrency

**Files:**
- Modify: `src/retrieval/embedder.rs`
- Test: `src/retrieval/embedder.rs` inline tests

**Interfaces:**
- Consumes: `embed_one_batch` (Task 3), `resolve_batch_size` (Task 4).
- Produces: `pub(crate) async fn embed_chunks_ordered<F, Fut>(texts: &[String], batch: usize, inflight: usize, embed_one: F) -> Result<Vec<EmbedOutput>>` where `F: Fn(Vec<String>) -> Fut` and `Fut: std::future::Future<Output = Result<Vec<EmbedOutput>>>`. A free function in `src/retrieval/embedder.rs`.

  **The closure takes an owned `Vec<String>`, not `&[String]`.** With a single generic `Fut`, a future that borrows its input is inexpressible — the input lifetime varies per call while `Fut` is one fixed type. Owning the sub-batch keeps `&self` the only borrow. Do not "simplify" this to a slice; it will not compile.

**Why a free function taking a closure, rather than inlining the stream in `embed_batch`:** the ordering property must be tested with sub-batches that *finish out of order*, and forcing that over HTTP is not deterministic. With this shape the test injects a closure that sleeps longest for the first sub-batch — guaranteeing out-of-order completion — and the test genuinely fails if `buffered` is changed to `buffer_unordered`. That is the whole point of the test.

**The invariant:** `flush_pending` (`src/retrieval/sync.rs:75`) zips embeddings onto payloads positionally. Misordering attaches every vector to the wrong chunk — no error, no crash, a silently corrupt index.

- [ ] **Step 1: Write the failing test**

Add to the tests module in `src/retrieval/embedder.rs`:

```rust
    /// Sub-batches must be reassembled in input order even when they COMPLETE out
    /// of order. The closure sleeps longest for the first sub-batch, so under
    /// `buffer_unordered` the results arrive reversed and this test fails.
    #[tokio::test]
    async fn embed_chunks_ordered_preserves_input_order() {
        let texts: Vec<String> = (0..9).map(|i| format!("text-{i}")).collect();

        let out = embed_chunks_ordered(&texts, 3, 3, |chunk: Vec<String>| {
            async move {
                // First sub-batch ("text-0") sleeps longest, last sleeps least.
                let idx: u64 = chunk[0]
                    .trim_start_matches("text-")
                    .parse()
                    .expect("numeric suffix");
                tokio::time::sleep(std::time::Duration::from_millis(60 - idx * 5)).await;
                Ok(chunk
                    .iter()
                    .map(|t| {
                        let n: f32 = t.trim_start_matches("text-").parse().unwrap();
                        EmbedOutput {
                            dense: vec![n],
                            sparse: SparseVector { indices: vec![], values: vec![] },
                        }
                    })
                    .collect::<Vec<_>>())
            }
        })
        .await
        .expect("ordered embed");

        let got: Vec<f32> = out.iter().map(|e| e.dense[0]).collect();
        assert_eq!(
            got,
            (0..9).map(|i| i as f32).collect::<Vec<f32>>(),
            "output order must match input order regardless of completion order"
        );
    }

    #[tokio::test]
    async fn embed_chunks_ordered_propagates_error() {
        let texts: Vec<String> = (0..4).map(|i| format!("t{i}")).collect();
        let res = embed_chunks_ordered(&texts, 2, 2, |_c: Vec<String>| async {
            Err::<Vec<EmbedOutput>, _>(anyhow!("boom"))
        })
        .await;
        assert!(res.is_err(), "a failing sub-batch must fail the whole call");
    }

    #[tokio::test]
    async fn embed_chunks_ordered_handles_empty_input() {
        let out = embed_chunks_ordered(&[], 8, 4, |_c: Vec<String>| async {
            Ok::<Vec<EmbedOutput>, anyhow::Error>(vec![])
        })
        .await
        .expect("empty input");
        assert!(out.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib retrieval::embedder`
Expected: FAIL to compile — `cannot find function 'embed_chunks_ordered'`.

- [ ] **Step 3: Implement the driver and wire it in**

Add near the top of `src/retrieval/embedder.rs`:

```rust
use futures::stream::{StreamExt, TryStreamExt};
```

Add the free function:

```rust
/// Drive `embed_one` over `texts` in `batch`-sized sub-batches, at most `inflight`
/// concurrently, reassembling results in **input order**.
///
/// Uses `buffered`, never `buffer_unordered`: `flush_pending`
/// (`src/retrieval/sync.rs:75`) zips embeddings onto payloads positionally, so
/// reordering here attaches every vector to the wrong chunk — silently.
pub(crate) async fn embed_chunks_ordered<F, Fut>(
    texts: &[String],
    batch: usize,
    inflight: usize,
    embed_one: F,
) -> Result<Vec<EmbedOutput>>
where
    F: Fn(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<EmbedOutput>>>,
{
    let batch = batch.max(1);
    let inflight = inflight.max(1);
    let nested: Vec<Vec<EmbedOutput>> =
        futures::stream::iter(texts.chunks(batch).map(|c| embed_one(c.to_vec())))
            .buffered(inflight)
            .try_collect()
            .await?;
    Ok(nested.into_iter().flatten().collect())
}
```

Add the inflight default next to it:

```rust
/// Concurrent in-flight sub-batches. 4 is where both legs saturated in the
/// 2026-07-27 sweep (sparse 2.7 → 9.2 chunks/s; both regressed at 8). That sweep
/// ran under contention from four concurrent indexers — re-measure on an idle
/// card before treating 4 as final.
const DEFAULT_INFLIGHT: usize = 4;

fn resolve_inflight() -> usize {
    std::env::var("CODESCOUT_EMBED_INFLIGHT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_INFLIGHT)
}
```

Replace the hybrid path of `embed_batch` (the sequential loop added in Task 3) with:

```rust
        let batch = self.resolve_batch_size().await;
        let inflight = resolve_inflight();
        embed_chunks_ordered(texts, batch, inflight, |chunk| self.embed_one_batch(chunk)).await
```

Also convert the `dense_only` path. Extract its loop body into a sibling worker and
drive it with the same function:

```rust
    /// Dense-only sub-batch (lite stack: no sparse server contacted).
    async fn embed_one_batch_dense(&self, chunk: Vec<String>) -> Result<Vec<EmbedOutput>> {
        let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        let mut out = Vec::with_capacity(inputs.len());
        for dense in self.dense_batch(&inputs).await? {
            if dense.len() != self.expected_dim {
                return Err(anyhow!(
                    "embed dim mismatch: got {}, expected {}",
                    dense.len(),
                    self.expected_dim
                ));
            }
            out.push(EmbedOutput {
                dense,
                sparse: SparseVector { indices: vec![], values: vec![] },
            });
        }
        Ok(out)
    }
```

and in `embed_batch`'s `dense_only` branch:

```rust
            // No sparse server to probe on this path: honour an explicit override,
            // else 32. llama-server advertises no per-request cap and the 2026-07-27
            // sweep showed it healthy through 128, so 32 is deliberately conservative.
            const DENSE_ONLY_BATCH: usize = 32;
            let batch = std::env::var("CODESCOUT_EMBED_BATCH")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|&n| n > 0)
                .unwrap_or(DENSE_ONLY_BATCH);
            return embed_chunks_ordered(texts, batch, resolve_inflight(), |chunk| {
                self.embed_one_batch_dense(chunk)
            })
            .await;
```

Do **not** call `resolve_batch_size()` here — it probes `self.sparse_base`, which on the
lite stack points at a server that does not exist.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib retrieval::embedder`
Expected: PASS.

Then deliberately break it to confirm the ordering test has teeth: change `.buffered(inflight)` to `.buffer_unordered(inflight)`, re-run, and confirm `embed_chunks_ordered_preserves_input_order` FAILS. Change it back.

- [ ] **Step 5: Improve the 413 error message**

In `embed_one_batch`'s sparse error path, where the non-retryable branch builds its error, include the resolved batch size so a wrong cap is self-diagnosing:

```rust
                                  return Err(anyhow!(
                                      "embed_batch sparse status {} (inputs={}): {}. \
                                       If this is 413, the server's max_client_batch_size is \
                                       below the resolved batch size — set CODESCOUT_EMBED_BATCH \
                                       to override discovery.",
                                      status,
                                      nonempty.len(),
                                      body.chars().take(200).collect::<String>()
                                  ));
```

- [ ] **Step 6: Run the full gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/retrieval/embedder.rs
git commit -m "perf(retrieval): pipeline embed sub-batches, order-preserving

embed_batch issued 8 inputs per request, one request at a time, against
a sparse server advertising 32 and a dense server running --parallel 16.
Sub-batches now go through a bounded `buffered` stream (default 4
in-flight, CODESCOUT_EMBED_INFLIGHT).

buffered, never buffer_unordered: flush_pending zips embeddings onto
payloads positionally, so reordering corrupts the index silently. The
ordering test forces out-of-order completion and fails under the wrong
combinator.

Vectors are byte-identical -- per-input embedding and per-input
truncation mean batch composition does not affect output."
```

---

### Task 6: Measure and record

**Files:**
- Modify: `docs/superpowers/specs/2026-07-27-embedder-batch-concurrency-design.md` (Verification section)

**Interfaces:** none — this task produces evidence, not code.

- [ ] **Step 1: Confirm the GPU is idle**

Run: `pgrep -af 'codescout index'` → expect no matches.
Run: `nvidia-smi --query-gpu=utilization.gpu,memory.used --format=csv,noheader`

- [ ] **Step 2: Re-run the batch sweep on an idle card**

Run the sweep from this session's scratchpad (`batch_sweep.py`), or re-derive it: time `POST /v1/embeddings` and `POST /embed_sparse` at batch 8/16/32 and at concurrency 1/2/4/8.

Record whether `inflight = 4` is still the saturation point. If the idle ceiling differs, update `DEFAULT_INFLIGHT` and the comment beside it.

- [ ] **Step 3: Time a full forced re-index**

```bash
time ./target/release/codescout index --project /home/marius/work/mirela/backend-kotlin --force
```

`--force` is deliberate: its delete pass also clears the ~4.1% of superseded chunks left by the four interrupted runs.

- [ ] **Step 4: Confirm search is unchanged**

Run a fixed set of `semantic_search` queries before and after and compare top-5 results. Vectors should be byte-identical, so any difference indicates the ordering invariant broke — check Task 5's combinator first.

- [ ] **Step 5: Record results and commit**

Replace the Verification section's projections with measured numbers, then:

```bash
git add docs/superpowers/specs/2026-07-27-embedder-batch-concurrency-design.md
git commit -m "docs(specs): record measured throughput after batching change"
```

---

## Self-Review

**Spec coverage.** Every numbered scope item maps to a task: Stage 1 item 0 → Tasks 1–2. Stage 2 item 1 (`/info` discovery) → Task 4. Item 2 (bounded concurrency) → Task 5. Item 3 (`embed_one_batch` extraction) → Task 3. Item 4 (env overrides) → Tasks 4 and 5. Item 5 (clearer 413) → Task 5 Step 5. The spec's Verification section → Task 6.

**Deliberate deviation from the spec.** The spec specifies `.codescout/index.lock`; this plan uses a hashed filename in the temp dir, keyed on `project_id`. Reason: library syncs pass a third-party checkout as `root`, and creating `.codescout/` inside one contradicts the existing `record_index_state` policy of not polluting library checkouts. Recorded in Task 1's design notes and flagged at handoff.

**Type consistency.** `embed_one_batch(&self, chunk: Vec<String>) -> Result<Vec<EmbedOutput>>` is defined in Task 3 and consumed with that exact signature in Task 5's closure. `embed_one_batch_dense` has the same shape (Task 5). `resolve_batch_size(&self) -> usize` is defined in Task 4 and called in Task 5's hybrid path only — never on the `dense_only` path, which has no sparse server to probe. `embed_chunks_ordered`'s `F: Fn(Vec<String>) -> Fut` matches both workers' parameter type by value, which is what makes a single fixed `Fut` expressible. `IndexLock` / `acquire` / `lock_path` are defined in Task 1 and used in Task 2.

**Corrected during pre-flight (2026-07-27).** The first draft specified
`F: Fn(&[String]) -> Fut`, which cannot compile: the returned future borrows its input,
so its lifetime varies per call while `Fut` is a single fixed type. Caught by the
pre-flight scan, before any implementer was dispatched. The by-value signature is now
pinned in both tasks with a "do not simplify this to a slice" note, because the slice
form is the natural thing for a reviewer or a later implementer to suggest.

**Known risk carried forward.** `DEFAULT_INFLIGHT = 4` comes from a sweep run under four-indexer contention. Task 6 Step 2 re-measures it. This is called out in the constant's own comment so it cannot be mistaken for a clean measurement.
