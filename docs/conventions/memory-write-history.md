---
id: a93383e734aa93f3
kind: doc
status: draft
title: Memory overwrite history
---

# Memory overwrite history

Topic writes preserve their predecessor before publishing a replacement. This applies to MCP, the dashboard and internal MemoryStore writes, in public, private and explicitly selected stores. `force: true` bypasses shrink refusal; it does not bypass backup or read failures. Semantic remember/forget operations use their own database and are outside this topic-file contract.

## History and restoration

For a store at `<memories>`, a normalized target `nested/topic.md` has backups at `<memories>/.history/nested/topic.md/<uuid>.md.bak`. Each published .md.bak file contains the exact previous UTF-8 bytes, including BOM, Unicode and line endings. Backups are first written to a unique .pending file, synced, and then published without replacing an existing backup; interrupted partial writes never acquire a .md.bak name. History files use Unix mode0600, directories0700, and a local .gitignore excludes all history. Backups are excluded from memory list because their final extension is bak. The .history topic namespace is reserved.

There is no automatic purge. Inspect the backup as a file, choose the intended version, then restore it through a normal topic write; that write preserves the current version too. Do not rename, unlink or replace .history/write.lock while processes may write to the store: its stable inode coordinates them. Keep history with the store when taking a backup.

## Failure and concurrency behavior

Existing content must be readable, regular and valid UTF-8. A history failure refuses the overwrite even with force. Nested/leaf topic and history symlinks are refused; a configured store root symlink is resolved once and writes, backups and anchors use the pinned destination. New topic writes do not create a predecessor backup. Shrink refusals do not create one either.

A store-wide advisory lock covers reading, shrink evaluation, backup publication, topic replacement and anchor stamping. Other cooperating CodeScout writers wait up to five seconds; contention beyond that is a retryable save failure. Deletions participate in the lease and retain their existing deletion semantics; they do not create history. Every overwrite remains a whole-document replacement. Concurrent writers preserve each intermediate predecessor, but do not merge edits or detect stale reads. External programs that ignore this lock are outside the serialization guarantee.

Topic replacement uses a unique sibling temporary file, then an atomic rename. A failure before publication leaves the previous topic intact. Anchor failures after publication are returned as warnings. This is topic-level recovery, not a coordinated database/configuration backup or authorization to activate a laptop update.
