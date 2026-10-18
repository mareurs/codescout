---
status: fixed
opened: 2026-10-18
closed: 2026-10-18
severity: high
owner: marius
related: [docs/issues/2026-10-18-memory-read-follows-a-leaf-symlink-and-serves-the-target.md, docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md]
tags:
- cluster/unclassified
unverified: 'The Windows behaviour (a .gitignore that is a reparse point) was not run. The check is symlink_metadata then read, so a link swapped in between the two calls is not closed (the writer is atomic_write, which replaces rather than follows). The tracing::warn notice was not asserted by a test (no observer in the suite reads it). The fix SHA is on the review branch and may not survive a rebase: use the patch-id.'
kind: bug
---

# BUG: opening the private memory store copies a `.gitignore` symlink target's bytes into the project as a regular file

## Summary

`MemoryStore::ensure_gitignored` (`src/memory/mod.rs`, 94-112 before the fix) read `<root>/.gitignore` with
`exists()` + `read_to_string`, both of which follow a symlink, appended `.codescout/private-memories/`, and
wrote the result back with `atomic_write`, which replaces the link with a regular file. It runs on every
project activation (`Agent::new`, `Agent::load_project_resources`, `activate_within_workspace` through
`open_private`). A cloned repository whose `.gitignore` is a link to `~/.ssh/id_rsa` therefore had the key's
bytes copied into a project file that is committed or indexed.

## Class fit

No class names a read-then-rewrite that launders a link target into a regular file: the read is the
disclosure and the write is what makes it durable and committable. It is not the capped-result or
guard-narrower classes; no guard existed at this site. Filed unclassified for lack of a second instance of
"a rewrite helper reads a path through a link and republishes the bytes under the link's own name".

## Symptom (Effect)

Reviewer evidence 2026-10-18, activating through `Agent::new` with `<root>/.gitignore -> <outside>/secret`:

```
is_link=false content=Ok("SECRET-BYTES\n.codescout/private-memories/\n")
```

and with a link to an injected `home/.ssh/id_rsa`: `.gitignore` became `"PRIVATE\n.codescout/private-memories/\n"`.
Activation succeeded and printed nothing.

## Reproduction

Measured 2026-10-18 on branch `fix/memory-leaf-links-2026-10-18` at `0a8af94d7`, before the fix, by the new
test `agent::tests::activation_never_copies_a_gitignore_symlink_target_into_the_project`, which builds a
`tempdir()` sandbox (`home/.ssh/id_rsa` containing `PRIVATE\n`, `project/.gitignore` linked to it) and calls
`Agent::new(Some(project))`. Red line:

```
thread 'agent::tests::activation_never_copies_a_gitignore_symlink_target_into_the_project' panicked at src/agent/mod.rs:3916:9:
.gitignore was replaced by a regular file: Ok("PRIVATE\n.codescout/private-memories/\n")
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7426 filtered out
```

Same shape at the store level: `memory::tests::open_private_never_copies_a_gitignore_symlink_target_into_the_project`.

## Environment

Linux, `rustc 1.97.1`, MemoryStore unchanged since before the 2026-10-14 write-history work (that work covered
topic writes only).

## Root cause

`src/memory/mod.rs` `ensure_gitignored` (pre-fix 94-112): `gitignore_path.exists()` and
`std::fs::read_to_string(&gitignore_path)` follow a leaf link; `crate::util::fs::atomic_write` then stages a
sibling file and renames it onto `.gitignore`, which swaps the directory entry (documented on
`atomic_write`: "A target that is itself a symlink is replaced, not written through"). The replacement is a
regular file carrying the target's bytes, with the target's permission bits (its mode is read with
`metadata`, which follows). Measured 2026-10-18: red lines above.

## Evidence

The red line above; the reviewer's two outputs quoted in Symptom. Callers of `open_private`
(`references`): `src/agent/mod.rs` 637, 794, 1482.

## Hypotheses tried

1. **Hypothesis:** only a link to a readable regular file is affected. **Test:** new tests with a dangling
   link and with a directory at `.gitignore`. **Verdict:** rejected. A dangling link is "missing" to
   `exists()` and was silently replaced by a regular file; a directory made activation fail on the read
   (`Is a directory (os error 21)`). Both are fixed by the same check.

## Fix

Fixed on branch `fix/memory-leaf-links-2026-10-18` by `732178af8` (patch-id
`7113e260a6ec1d52b12efb5a160ed54c5cca292b`, `git show 732178af8 | git patch-id --stable`).

Chosen behaviour: `symlink_metadata` first. A missing `.gitignore` or a regular file is handled as before.
Anything else (link, dangling link, directory) is left exactly as found, not read and not replaced, and
activation still succeeds, because a refusal would make a project with a legitimate in-repo `.gitignore` link
impossible to activate. So that the private store is not committable by that omission, it ignores itself with
`.codescout/private-memories/.gitignore` containing `*`, created with `create_new` (a link planted at that name
is never written through; an existing entry is left). A `tracing::warn!` names the skipped path. Behaviour
change: for such a project `.gitignore` no longer gains the entry, and `private-memories/` gains a
`.gitignore`.

## Tests added

`src/memory/mod.rs`: `open_private_never_copies_a_gitignore_symlink_target_into_the_project`,
`open_private_self_ignores_when_the_root_gitignore_is_a_link`,
`open_private_leaves_a_dangling_gitignore_link_alone`,
`open_private_survives_a_gitignore_that_is_a_directory`,
`the_self_ignore_fallback_is_never_written_through_a_link`,
`open_private_appends_after_a_gitignore_with_no_trailing_newline` (regression guard for the normal path);
`src/agent/mod.rs`: `activation_never_copies_a_gitignore_symlink_target_into_the_project` (real entry point).
Mutation: seven guarded sites mutated one at a time, all killed (table in the session report).

## Workarounds

Do not activate a project whose `.gitignore` is a link to a file outside the repository, until on the fix.

## Resume

N/A once fixed. Open residuals: see `unverified` above.

## References

`src/memory/mod.rs`, `src/util/fs.rs` (`atomic_write`), `docs/conventions/memory-write-history.md`,
the sibling issue for the read side.
