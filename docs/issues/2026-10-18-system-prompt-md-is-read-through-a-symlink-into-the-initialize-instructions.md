---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-18-memory-read-follows-a-leaf-symlink-and-serves-the-target.md, docs/issues/2026-10-18-opening-the-private-memory-store-copies-a-gitignore-symlink-targets-bytes-into-the-project.md]
tags:
- cluster/unclassified
unverified: 'The text reaches the initialize instructions through Agent::project_status; the test drives project_status, not a full MCP initialize handshake. Windows behaviour was not run. The check is symlink_metadata then read, so a link swapped in between the two calls is not closed. The fix SHAs are on the review branch and may not survive a rebase: use the patch-ids.'
kind: bug
---

# BUG: `.codescout/system-prompt.md` is read through a symlink and its target goes into the `initialize` instructions

## Summary

`Agent::project_status` (`src/agent/mod.rs`, 1701-1702 before the fix) did `prompt_file.exists()` and then
`std::fs::read_to_string(&prompt_file)`, both of which follow a link. The text becomes the project's system prompt in
the server instructions the model receives. A cloned repository can ship `.codescout/system-prompt.md` as a link to
`~/.ssh/id_rsa` (git carries symlinks) and the key's text is placed in the model's context on activation. Found by
the Opus review of the sibling memory fix on 2026-10-18.

## Class fit

The same trait as the sibling memory-read issue: a reader of a path inside the project follows a link planted by the
repository. Filed unclassified, like its siblings, for lack of a class naming "a reader inside the project follows a
link a clone can ship".

## Symptom (Effect)

Before the fix, with `<project>/.codescout/system-prompt.md -> <sandbox>/home/.ssh/id_rsa`:

```
thread 'agent::tests::project_status_never_serves_a_system_prompt_symlink_target' panicked at src/agent/mod.rs:3702:13:
assertion `left == right` failed: a linked system-prompt.md must be treated as absent
  left: Some("SECRET-PROMPT-BYTES\n")
```

## Reproduction

Measured 2026-10-18 on branch `fix/memory-leaf-links-2026-10-18` at `a0a41e431`, before the fix, by the new test above
(`tempdir()` sandbox with a fake `home/.ssh/id_rsa`; activation through `Agent::activate`, then
`Agent::project_status`).

## Environment

Linux, `rustc 1.97.1`, branch above.

## Root cause

`src/agent/mod.rs` `project_status`, phase 2: `if prompt_file.exists() { std::fs::read_to_string(&prompt_file).ok() }
else { default_prompt }`. `exists()` and `read_to_string` follow a leaf link; nothing checked the file type.
Measured 2026-10-18: red line above.

## Evidence

Does a legitimate flow symlink `system-prompt.md`? Checked 2026-10-18: `git ls-files -s | awk '$1=="120000"'` lists one
tracked symlink in the whole repository, `.claude/skills/claude-traces`, unrelated; `find .codescout -maxdepth 2 -type l`
finds none; a search of `docs`, `src` and the scripts for `system-prompt.md` near `symlink`, `ln -s` or `link` finds no
description of a linked system prompt (the feature is documented as a plain file written by onboarding, and as a TOML
fallback). No legitimate flow was found, so a link is skipped. A user who symlinks their own prompt file loses it
silently except for a log warning; the TOML `system_prompt` is used instead.

## Hypotheses tried

None needed.

## Fix

Fixed on branch `fix/memory-leaf-links-2026-10-18` by `aa934e25c` (patch-id `ba23f9d56125d799dd3b8a592d445a4af46c686b`,
`git show aa934e25c | git patch-id --stable`): the file is read only when `symlink_metadata` says it is a regular
file. A link, a directory or any other type logs a warning and is treated as absent, so the `[project]
system_prompt` from `project.toml` (or nothing) is used. A missing file behaves as before. The hunk is small (one
`match` in `project_status`), to keep merges with the branches that also edit `src/agent/mod.rs` simple.

## Tests added

`agent::tests::project_status_never_serves_a_system_prompt_symlink_target` (both with and without a TOML fallback).
Mutation: both the type check and the `symlink_metadata` call were mutated and killed.

## Workarounds

Do not activate a repository you did not author whose `.codescout/system-prompt.md` is a link, until on the fix.

## Resume

N/A once fixed.

## References

`src/agent/mod.rs` (`project_status`), the two sibling issues listed in `related`.
