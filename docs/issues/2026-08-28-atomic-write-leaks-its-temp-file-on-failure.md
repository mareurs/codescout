---
status: open
opened: 2026-08-28
closed:
severity: low
owner: marius
related: []
tags: [atomic-write, disk-full, tmp-file-leak]
kind: bug
---

# BUG: `atomic_write` leaks its `.tmp` file when the initial write fails (e.g., disk full)

## Summary
`crate::util::fs::atomic_write` (`src/util/fs.rs:62-77`) writes to a sibling `.tmp` file
before renaming it over the target. If the initial `std::fs::write` to the tmp file fails —
observed with `ENOSPC` — the function returns the error via `?` without removing the tmp file
it just created, leaving a stray, typically zero-length, `.tmp` file next to the target on
every future write attempt. The target file itself is never touched in this failure mode, so
no data is corrupted or lost — only debris is left behind.

## Symptom (Effect)
Coordinator's own observation, verbatim, from a session where `/home` was at 0 bytes free:

> with `/home` at 0 bytes free, a codescout write failed with `No space left on device (os
> error 28)` and left behind `src/prompts/guide_index.tmp` and `.codescout/libraries.tmp`,
> both zero-length, while the real files were unmodified (`git diff --stat` empty).

## Reproduction
Not independently reproduced in this session (would require an artificially disk-full
filesystem, e.g. a small tmpfs mount, and is out of scope for this fix-wave item — diagnosis
only, no fix). Minimal reproduction sketch for a future session:
```
# mount a tiny tmpfs, fill it to 0 free bytes, point a codescout write at a path on it
mount -t tmpfs -o size=4k tmpfs /tmp/tinyfs
dd if=/dev/zero of=/tmp/tinyfs/filler bs=1k count=4 2>/dev/null || true
# then call atomic_write(Path::new("/tmp/tinyfs/target.txt"), "<content longer than remaining space>")
# expect: Err(ENOSPC), and /tmp/tinyfs/target.tmp left behind (0 bytes), target.txt absent/unmodified
```

## Environment
codescout, worktree `/home/marius/work/claude/codescout/.claude/worktrees/operator-rules-phase-2`,
branch `sdd/operator-rules-phase-2`. Filesystem: `/home` (host-level, not worktree-specific —
the condition is host disk exhaustion, reproducible from any project on the same filesystem).

## Root cause
`atomic_write` (`src/util/fs.rs:62-77`):
```rust
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;

    // Preserve original mode if the target already exists.
    #[cfg(unix)]
    if let Ok(meta) = std::fs::metadata(path) {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
    }

    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
```

Two failure points, only one of which is handled:

1. **`std::fs::write(&tmp, content)?` (line 63) — unhandled.** `std::fs::write` internally
   does the equivalent of `File::create(&tmp)` (creates/truncates the tmp file) followed by
   `write_all(content)`. Under `ENOSPC`, `File::create` can succeed (producing a 0-byte file)
   while the subsequent `write_all` fails partway or immediately. The `?` on this line
   propagates that `io::Error` straight out of `atomic_write` — there is no
   `inspect_err`/cleanup attached to this statement, so the just-created `tmp` file is never
   removed. This is the exact mechanism behind the observed zero-length `.tmp` files.
2. **`std::fs::rename(&tmp, path)` (lines 74-76) — handled.** This call *is* wrapped in
   `.inspect_err(|_| { let _ = std::fs::remove_file(&tmp); })`, so a failure at the rename
   step does clean up the tmp file correctly.

Because the real `path` is only ever touched by the final `rename`, and the leak happens
before that point is ever reached, the target file is guaranteed untouched whenever this bug
fires — consistent with the coordinator's `git diff --stat` being empty.

*Measured 2026-08-28: read `src/util/fs.rs:62-77` directly (`mcp__codescout__symbols`);
confirmed no other cleanup path exists for the `std::fs::write` failure branch by inspection —
not measured under an actual `ENOSPC` condition this session (see Reproduction).*

## Evidence

### Coordinator's observed failure (this session)
> with `/home` at 0 bytes free, a codescout write failed with `No space left on device (os
> error 28)` and left behind `src/prompts/guide_index.tmp` and `.codescout/libraries.tmp`,
> both zero-length, while the real files were unmodified (`git diff --stat` empty).

### Shared helper — every write path funnels through the same function
```
$ grep atomic_write src -r --files-with-matches
src/util/fs.rs
src/tools/edit_file/mod.rs
src/library/registry.rs
src/memory/mod.rs
src/operator_rules/mod.rs
src/tools/markdown/edit_markdown.rs
src/memory/anchors.rs
src/symbol/edit.rs
src/tools/symbol/edit_code.rs
```
`.codescout/libraries.tmp` traces to `src/library/registry.rs:69` (`Registry::save`);
`src/prompts/guide_index.tmp` traces to `src/tools/symbol/edit_code.rs:1363` — `edit_code`'s
write path calls the identical helper, confirming the task brief's open question ("check
whether `edit_code`'s write path shares it"): **yes, it shares it**, unmodified.

## Hypotheses tried
1. **Hypothesis:** The leak is specific to `operator_rules::compile`'s CLAUDE.md-profile
   writes (the call site named in the task brief).
   **Test:** Read `src/operator_rules/mod.rs:73-93` and cross-referenced the two
   actually-observed leaked paths (`src/prompts/guide_index.tmp`, `.codescout/libraries.tmp`)
   against every `atomic_write` call site in the tree.
   **Verdict:** rejected. Neither leaked path is a CLAUDE.md profile; both are through
   unrelated call sites (`edit_code`, `library::registry::save`) sharing the same underlying
   helper. The bug is in `atomic_write` itself, not in any one caller.
   **Evidence link:** Evidence § Shared helper.

## Fix
Not implemented, per this fix-wave item's scope. The mechanism suggests the smallest correct
fix is to wrap the `std::fs::write(&tmp, content)?` call the same way the `rename` call
already is — e.g.
`std::fs::write(&tmp, content).inspect_err(|_| { let _ = std::fs::remove_file(&tmp); })?;` —
but that has not been written, reviewed, or tested here.

- SHA: N/A (not fixed)
- patch-id: N/A (not fixed)

## Tests added
None. This item is diagnosis-only per its brief ("Do NOT implement the fix"). The existing
`atomic_write_preserves_exec_bit` test (`src/util/fs.rs:421-432`) is the only test touching
this function, and it does not exercise either failure path (it never induces an I/O error).

## Workarounds
Stray `<name>.tmp` files left by this bug are always zero-length (or partially written) and
safe to delete manually once free disk space is confirmed; the real target file is never
touched by this failure mode, so no data recovery is needed. `git status` will show them if
they land inside a tracked directory that isn't `.gitignore`d for `*.tmp`.

## Resume
Decide and implement the smallest fix: wrap `std::fs::write(&tmp, content)?` in
`.inspect_err` cleanup mirroring the existing `rename` handling, then add a regression test
that forces the write step to fail (e.g. write to a directory instead of a file, or a
read-only tmp path) and asserts the `.tmp` sibling does not survive. Also decide whether to
extend the fix to the (unrelated, pre-existing) profile-write path's own doc comment
(`src/operator_rules/mod.rs:76-79`), which already documents "crash or disk-full mid-write
must not be able to truncate" — that documented guarantee is currently only true for the
`rename` step, not the initial `write`.

## References
- `src/util/fs.rs:62-77` — `atomic_write`
- `src/util/fs.rs:421-432` — `atomic_write_preserves_exec_bit` (only existing test)
- `src/tools/symbol/edit_code.rs:1363` — `edit_code`'s write path (confirmed same helper)
- `src/library/registry.rs:64-71` — `Registry::save` (source of the observed
  `.codescout/libraries.tmp`)
- `src/operator_rules/mod.rs:73-93` — CLAUDE.md profile writes (named in the task brief;
  ruled out as the leak's source this session)
- `src/tools/edit_file/mod.rs:254-259,556-565,600-608` — `edit_file`'s three write sites
- `src/tools/markdown/edit_markdown.rs:1349-1473` — `edit_markdown`'s write site
- `src/memory/mod.rs:83-101`, `src/memory/anchors.rs:33-41`, `src/symbol/edit.rs:765-772` —
  remaining call sites
