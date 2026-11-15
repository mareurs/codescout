---
kind: entry
status: open
title: Stage a migrated collection by exact file names, never by directory (the commit guard refuses it)
created: 2026-11-15
entry_kind: lesson
status_since: 2026-11-15
---

Found 2026-11-15 while committing this collection. `git add -- docs/trackers/streams/context-migration` (a directory) was accepted, but the shared-checkout commit guard then refused the pathspec commit: "it captures content another session wrote". The recorder cannot attribute a directory add, so it stamps the rows as unrecorded and over-refuses. The fix that worked: `git reset -q -- <the two paths>` (index only), then `git add -- <each file by exact name>`, read `git diff --cached --name-only`, then commit with `-m` before `--` and the same paths.

Why it matters for the migration: the skill's step 8 commits a whole collection. A migrated tracker can be hundreds of files (`bug-fix-session-log` alone has 398 records), so the skill must stage by explicit file list (for example from `git status --porcelain` filtered to the collection's folder), never by directory, and must run the staging and the commit as separate calls. The shared index also holds other sessions' staged paths, which is fine for a pathspec commit but means `git diff --cached --name-only` is not "only mine".
