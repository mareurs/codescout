---
kind: doc
status: active
title: Memory writes - what a client-side guard may assume of the server
---

# Memory writes: overlap policy for client-side guards

A harness (Pi, or any other) that intercepts `memory(action="write" | "delete")` before dispatch may rely on the server for the guarantees in the first list and must not assume the second. Decision of 2026-10-16: **rely on the server; no client guard is imported** ([the Pi guard issue](../issues/2026-10-14-pi-memory-write-guard-reencodes-invalid-utf8-before-backup.md)). The server contract itself is [`memory-write-history.md`](memory-write-history.md); this page is only the line between it and a client.

Verified 2026-10-16 against `src/memory/write_transaction.rs`, `src/memory/guarded.rs` and `src/tools/memory/mod.rs`, and by the tests named below.

## A client may assume

- **An overwrite keeps the exact previous bytes first, or fails.** The backup is `<memories>/.history/<topic path>.md/<uuid>.md.bak` in the store the write lands in (public, private, selected project), with and without `force`. BOM, CRLF and non-ASCII survive. `tests/memory_write_history.rs`: `forced_overwrite_preserves_exact_bytes_and_history_is_not_a_topic`, `private_and_explicit_stores_keep_history_with_the_selected_store`; `src/tools/memory/tests.rs`: `forced_writes_preserve_history_in_the_selected_store_and_refuse_invalid_bytes`.
- **An existing topic file that is not valid UTF-8 is refused, even with `force`, and stays byte-identical with no backup** (`write_transaction.rs:74-77`). Tests: `invalid_utf8_is_refused_even_with_force_without_touching_old_bytes` and the `src/tools` test above. A client must not decode and re-encode the old file itself: that is the defect that produced the Pi issue. The new `content` is a JSON string, so it is always valid UTF-8.
- **Shrink refusal.** An existing topic of 200 bytes or more that would lose over half its bytes or lines is refused without `force`; a refused write makes no backup and changes nothing (`shrink_refusal_creates_no_backup_and_changes_no_content`). `force: true` bypasses only this refusal, never a backup or read failure.
- **History is git-ignored.** `.history/.gitignore` holds `*`, created under the lock on every transaction that reaches the server (`prepare` runs `ensure_ignore` before the shrink check, so shrink-refused ones included; read from source, untested); the project `.gitignore` is not touched; history directories are 0700 and files 0600 (`history_is_gitignored_without_changing_the_project_gitignore`).
- **Symlinks are refused** at the topic leaf, a topic parent, `.history`, a backup directory, `write.lock` and `.history/.gitignore` (`write_transaction.rs:68`, `private_dir`, `safe_topic_parent`, `open_lock`, `ensure_ignore`); `.history` as a topic namespace is reserved (`symlink_topics_parents_and_history_are_refused`, `reserved_history_topics_cannot_overwrite_internal_files`). A client that reads a topic itself must `lstat` first, or it copies bytes the server would refuse to touch.
- **Writes serialize** under a store-wide advisory `flock` on `<memories>/.history/write.lock`; cooperating writers wait up to 5 s, then fail with "memory write lock timed out". Each concurrent writer's predecessor is kept (`cross_process_writes_preserve_each_predecessor`). The 5 s timeout has no test.
- **The server names the file, not the client.** The topic goes through `sanitize_topic` and `.md` replaces any extension, so `../topic.txt` writes `topic.md` (`raw_overwrite_also_backs_up_and_aliases_select_the_actual_target`).

## A client must not assume

- **Delete keeps nothing.** `begin_delete` reads no existing content (`write_transaction.rs:26`) and `delete` removes the file; the final bytes are not in `.history`, and `guarded_delete` removes the anchor sidecar too. Tested by `delete_keeps_no_backup_of_the_final_bytes`. A client that wants protection on delete must ask for confirmation or copy the file itself.
- **Any content marker.** The server has no override marker; the only bypass is the `force` parameter, which covers the shrink refusal alone. A marker in `content` (Pi's `memory-write-guard-override`) is not interpreted and is written as document text; the string has 0 hits in this repository. Its meaning belongs to the client.
- **The caller's cwd or HOME.** The target store comes from the `workspace=` pin, `project_id` or the focused project: the root project `<root>/.codescout/memories`, a sub-project `<root>/.codescout/projects/<id>/memories`, and `private: true` the private store. No memory code reads `$HOME` (`resolve_memory_dirs`, `Workspace::memory_dir_for_project`). A guard that looks in the cwd, or in a HOME store, checks a different file than the one written. Read from source; no test of a subdirectory cwd.
- **Its own lock.** `.history/write.lock` is cooperative: a client or other program writing the file directly is outside the guarantee, and the lock inode must not be unlinked or replaced. A client check made before dispatch is not atomic with the server's write; the server re-reads under the lock for its own checks and has no stale-read or merge detection, so a client needing read-modify-write safety must carry its own version check.
- **Anything inside `<memories>/.history/`.** A file or symlink where a history directory belongs, a symlinked `write.lock` or `.gitignore`, or a `.gitignore` with content other than `*` makes the server refuse writes (`private_dir`, `open_lock`, `ensure_ignore`). A second backup layout beside the server's is a duplicate, and a client that blocks before dispatch leaves its backups outside the server's `.gitignore`.
- **A restore or purge action.** `memory` has none; history is never purged and recovery is a manual file read plus an ordinary topic write. Semantic `remember`/`forget` use another database and are outside all of this.
