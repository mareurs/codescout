# Design: Incremental Index Rebuilding with Hash-Based Change Detection

## Problem

The current `build_index` hashes **every** file on every run (O(n) file reads)
even when nothing changed, never cleans up deleted files, and only runs when
explicitly called via `index_project`. There is no way to know the index is
stale until a `semantic_search` returns outdated results.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Trigger model | Layered: smart explicit + hook-driven + future watcher | Each layer adds coverage without coupling |
| Change detection | Git-diff + mtime hybrid | Git handles tracked files cheaply; mtime handles untracked |
| Deleted files | Git-informed purge + full scan fallback | Git diff reports deletions for free; full scan catches untracked |
| Checkpoint storage | Commit SHA in `meta` + per-file mtime in `files` | Best fallback chain: git → mtime → hash |
| Hook granularity | Diff from last-indexed-commit to HEAD | Same code path as explicit; hooks are just triggers |
| Filesystem watcher | Deferred to future work | Layers 0+1 cover commit-oriented workflows; YAGNI |
| Search staleness | Warning + hint in `semantic_search` response | Cheap check, actionable for LLM agents |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Trigger Layer                         │
│                                                         │
│  index_project (explicit)    commit/pre-push hook       │
│         │                          │                    │
│         └──────────┬───────────────┘                    │
│                    ▼                                    │
│          diff_and_reindex(root)     ◄── future: watcher │
└─────────────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│               Change Detection Pipeline                  │
│                                                         │
│  1. Load last_indexed_commit from meta table            │
│  2. git diff last_indexed..HEAD → tracked changes       │
│     • Modified/Added → candidate list                   │
│     • Deleted → immediate purge from DB                 │
│     • Renamed → purge old path, index new path          │
│  3. Walk untracked files → mtime vs stored mtime        │
│     • mtime changed → SHA-256 confirm → candidate       │
│     • mtime same → skip                                 │
│  4. Fallback: if last_indexed_commit unreachable         │
│     (rebase/amend) → mtime scan for ALL files           │
└─────────────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│              Existing Pipeline (unchanged)                │
│                                                         │
│  candidates → ast_chunker::split_file                   │
│            → embedder.embed (concurrent, semaphore)     │
│            → delete_file_chunks + insert_chunk           │
│            → upsert_file_hash (now includes mtime)      │
│            → update last_indexed_commit in meta          │
└─────────────────────────────────────────────────────────┘
```

## Layers

### Layer 0: Smart Explicit (`index_project`)

The existing `index_project` tool gains a dramatically faster code path:

1. Load `last_indexed_commit` from `meta` table
2. If present and reachable → `git diff last_indexed..HEAD`
   - `Modified`/`Added` → add to candidate list
   - `Deleted` → `delete_file_chunks()` immediately
   - `Renamed` → purge old path, add new path to candidates
3. Walk untracked files (not in git) → compare `mtime` to stored `mtime`
   - mtime changed → SHA-256 hash to confirm → candidate if hash differs
   - mtime same → skip
4. **Fallback**: if `last_indexed_commit` is unreachable (rebase/amend) or
   missing (first run) → mtime scan for ALL files, with SHA-256 as final
   arbiter
5. Scan `files` table for paths that no longer exist on disk →
   `delete_file_chunks()` (catches deleted untracked files)
6. Feed candidates into existing embed pipeline (chunk → embed → insert)
7. Update `last_indexed_commit` = HEAD, update mtime for processed files

The `force: true` flag still bypasses all of this and does a full rebuild.

### Layer 1: Hook-Driven (Claude Code / git hooks)

A Claude Code hook or git post-commit hook triggers the **same
`diff_and_reindex` code path** as Layer 0. The hook is just a trigger; the
logic is: diff from `last_indexed_commit` to current HEAD, re-index what
changed.

**Claude Code hook** (`.claude/settings.json`):
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "pattern": "git commit|git push",
        "command": "code-explorer index --project ."
      }
    ]
  }
}
```

**Git hook** (`.git/hooks/post-commit`):
```bash
#!/bin/sh
code-explorer index --project . &
```

The `&` makes it non-blocking so the commit is not slowed down.

### Layer 2: Filesystem Watcher (Future Work)

Deferred. When implemented:

- Use the `notify` crate (cross-platform: inotify/FSEvents/ReadDirectoryChangesW)
- Spawn a background `tokio::spawn` task in the MCP server
- Debounce events (2s window), filter via `.gitignore` + `ignored_paths`
- Call the same `diff_and_reindex` with a per-file candidate list
- Opt-in via `project.toml`: `[index] watch = true`
- Consider resource limits on large repos (inotify has a per-user watch limit
  on Linux, configurable via `fs.inotify.max_user_watches`)

The internal `diff_and_reindex` API is designed so the watcher plugs in with
no changes to the core pipeline.

## Fallback Chain

```
Try git diff (cheapest, handles deletes/renames natively)
  │ fails: no last_indexed_commit, or commit unreachable (rebase)
  ▼
Try mtime comparison (cheap stat() calls, no file reads)
  │ mtime unavailable or unreliable
  ▼
SHA-256 hash comparison (current behavior, always correct, most expensive)
```

This chain means:
- **Best case** (typical commit workflow): git diff finds 3 changed files,
  rest are skipped. O(1) git operation + O(k) file reads where k is the
  number of changed files.
- **Degraded case** (after rebase): mtime scan skips most files, only hashes
  those with changed mtime. O(n) stat calls + O(k) file reads.
- **Worst case** (first run or `force: true`): hash everything, same as
  current behavior. O(n) file reads.

## Schema Changes

### `files` table — add `mtime` column

```sql
-- Migration (applied in open_db if column missing):
ALTER TABLE files ADD COLUMN mtime INTEGER;

-- Full schema after migration:
CREATE TABLE IF NOT EXISTS files (
    path   TEXT PRIMARY KEY,
    hash   TEXT NOT NULL,
    mtime  INTEGER          -- Unix epoch seconds, NULL for legacy rows
);
```

`INTEGER` for mtime (Unix seconds) is OS-agnostic. Rust's
`std::fs::metadata().modified()` returns `SystemTime` which converts to epoch
seconds on all platforms via `duration_since(UNIX_EPOCH)`.

**Note on mtime precision:** We store seconds, not nanoseconds. This means two
writes within the same second would share an mtime. The SHA-256 hash is the
final arbiter in this case — mtime is a pre-filter, not the source of truth.

### `meta` table — new key

```
key: "last_indexed_commit"
value: full 40-char SHA of HEAD at last successful index
```

This reuses the existing `meta` table and `get_meta`/`set_meta` functions.

## `semantic_search` Staleness Warning

On every `semantic_search` call, one cheap comparison:

```rust
fn check_index_staleness(conn: &Connection, root: &Path) -> Result<Staleness> {
    let repo = git2::Repository::open(root)?;
    let head_oid = repo.head()?.peel_to_commit()?.id().to_string();
    let last_indexed = get_meta(conn, "last_indexed_commit")?;

    match last_indexed {
        None => Ok(Staleness { stale: true, behind_commits: 0 }),
        Some(ref stored) if stored == &head_oid => {
            Ok(Staleness { stale: false, behind_commits: 0 })
        }
        Some(ref stored) => {
            // Count commits between last indexed and HEAD
            let behind = count_commits_between(&repo, stored, &head_oid)
                .unwrap_or(0);
            Ok(Staleness { stale: true, behind_commits: behind })
        }
    }
}
```

Response includes:
```json
{
  "results": [...],
  "stale": true,
  "behind_commits": 5,
  "hint": "Index is behind HEAD. Run index_project to update."
}
```

The `stale` and `hint` fields only appear when the index is actually behind,
keeping the happy-path response unchanged.

## Core Function: `diff_and_reindex`

This replaces the walk-and-hash-everything loop in `build_index`. Both
`index_project` and the commit hook call the same function.

```
diff_and_reindex(project_root, force) -> Result<IndexReport>

  if force:
    → existing full-scan path (walk everything, hash everything)
    → return

  conn = open_db(project_root)
  config = ProjectConfig::load_or_default(project_root)
  last_commit = get_meta(conn, "last_indexed_commit")
  head_commit = resolve HEAD via git2

  // ── Phase 1: Determine candidates ──────────────────────

  if last_commit is Some and reachable in repo:
    diff_entries = git_diff(last_commit, head_commit)
    for entry in diff_entries:
      match entry.status:
        Deleted  → delete_file_chunks(entry.path)
        Renamed  → delete_file_chunks(entry.old_path)
                   candidates.push(entry.new_path)
        Modified | Added → candidates.push(entry.path)

    // Also check untracked files via mtime
    for file in walk_untracked_files(project_root):
      stored_mtime = get_file_mtime(conn, file.rel_path)
      current_mtime = file.metadata().modified()
      if stored_mtime != current_mtime:
        current_hash = hash_file(file.path)
        stored_hash = get_file_hash(conn, file.rel_path)
        if stored_hash != current_hash:
          candidates.push(file.rel_path)

  else:
    // Fallback: mtime scan for everything
    for file in walk_all_files(project_root):
      stored_mtime = get_file_mtime(conn, file.rel_path)
      current_mtime = file.metadata().modified()
      if stored_mtime is None or stored_mtime != current_mtime:
        current_hash = hash_file(file.path)
        stored_hash = get_file_hash(conn, file.rel_path)
        if stored_hash is None or stored_hash != current_hash:
          candidates.push(file.rel_path)

  // ── Phase 1b: Purge deleted untracked files ────────────
  // (Only on explicit index_project, not hook-triggered)
  for path in all_paths_in_files_table(conn):
    if not exists_on_disk(project_root.join(path)):
      delete_file_chunks(path)
      deleted_count += 1

  // ── Phase 2: Chunk + embed candidates ──────────────────
  // (Identical to current build_index Phase 1-3)
  for candidate in candidates:
    chunks = ast_chunker::split_file(...)
    embeddings = embedder.embed(chunks)
    delete_file_chunks(candidate)
    insert_chunk(each chunk with source="project")
    upsert_file_hash(candidate, hash, mtime)

  // ── Phase 3: Update checkpoint ─────────────────────────
  set_meta(conn, "last_indexed_commit", head_commit)

  return IndexReport { indexed, skipped, deleted }
```

## Coordination with Library Search

The library search feature (in progress on `feat/library-search`) modifies
overlapping files but different columns/functions:

| Area | Library Search | This Design | Conflict? |
|---|---|---|---|
| `chunks` table | Added `source` column | No change | None |
| `files` table | No change | Adds `mtime` column | None |
| `CodeChunk` struct | Added `source` field | No change | None |
| `build_index` body | Adds separate `build_library_index` | Refactors change-detection loop | Independent |
| `delete_file_chunks` | Uses unchanged | Uses unchanged | None |
| `semantic_search` | Adds `scope` filter | Adds staleness warning | Additive (both modify return JSON) |
| `open_db` schema | Added `source` to `chunks` | Adds `mtime` to `files` | Independent DDL |

**Future consideration:** `build_library_index` (Library Search Task 14, not
yet implemented) should eventually adopt the same mtime + hash change
detection pattern. Libraries change rarely (only on dependency version bumps),
but when they do, re-embedding the entire library is wasteful. The `files`
table can store library file hashes too (they already have `source` in
`chunks`). The `last_indexed_commit` checkpoint is project-specific; library
staleness would be detected via lockfile hash changes (e.g. SHA-256 of
`Cargo.lock`) rather than git commit diffs.

## OS Compatibility

All components are OS-agnostic:

| Component | Implementation | Platform notes |
|---|---|---|
| SHA-256 hashing | `sha2` crate (pure Rust) | All platforms |
| mtime reading | `std::fs::metadata().modified()` | Returns `SystemTime` on all platforms |
| mtime storage | Unix epoch seconds (`INTEGER`) | `SystemTime` → `duration_since(UNIX_EPOCH)` works everywhere |
| Git operations | `git2` crate (libgit2 bindings) | All platforms, already a dependency |
| File walking | `ignore` crate | All platforms, already a dependency |
| SQLite | `rusqlite` | All platforms, already a dependency |

No `inotify`, `FSEvents`, or platform-specific APIs are used. The deferred
filesystem watcher (Layer 2) would introduce `notify` as the first
platform-dependent dependency, but `notify` itself abstracts the platform
differences.

## What Doesn't Change

- The embedding pipeline (chunker → embedder → DB insert) is untouched
- `force: true` on `index_project` still does a full rebuild
- The `files` table schema is additive (`mtime` column, NULL-safe for legacy)
- All existing tests continue to work
- The `CodeChunk` and `SearchResult` structs are not modified
- The `open_db` migration is safe (SQLite `ALTER TABLE ADD COLUMN` is a
  no-op if column exists, and `NULL` default is fine for legacy rows)
