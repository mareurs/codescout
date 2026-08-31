---
kind: bug
status: fixed
tags:
- atomic-write
- disk-full
- tmp-file-leak
closed: 2026-08-31
opened: 2026-08-28
owner: marius
related: []
severity: low
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

**Fixed 2026-08-31.** The `std::fs::write` call now carries the same `inspect_err`
cleanup the `rename` below it already had:

```rust
std::fs::write(&tmp, content).inspect_err(|_| {
    let _ = std::fs::remove_file(&tmp);
})?;
```

- **SHA:** `f671c3a1` (on `experiments`)
- **patch-id:** `a4ca9f56c791c604e68205bac2b1fd2d32a3b3a0`

**Green-gate caveat discharged 2026-08-31T11:13.** This was committed while
`server::tests::tool_surface_under_budget` was failing, so the file carried an
`unverified:` noting the fix had never been observed under a fully green gate. That is
now closed: a full four-lane run reports **0 failures, 0 errors**, and the `unverified:`
field has been removed rather than left empty, because presence is what a query filters
on.

The budget failure was never this change, and it was never `tree.rs` either — which is
worth recording, because the obvious attribution was wrong twice over. The overage came
from in-flight prompt work elsewhere, since trimmed (`52712759`, `e245f983`). The author
of the tree change found that by stubbing their own schema description down to a single
character and watching the advertised surface go **up**, 56698 → 56845 — an arithmetic
attribution rather than a "who touched it last" one, and the only reason the repair did
not land on the wrong file. `tree.rs` shipped clean at `799e5dc6` (patch-id
`67f9094a40e4a2872fac52d316b22d641fbd7ba1`, verified here by independent derivation).

**Why the partial guard was the trap, and not merely an omission.** A reader skimming
`atomic_write` for "is there cleanup?" *finds* cleanup. The defect lived in the one path
that had none. Reading for the presence of a remedy is not reading for the absence of the
defect, and only the second answers the question — which is why this survived in a
sixteen-line function that had been read many times.

**A sibling defect in the same two lines is deliberately NOT fixed here**, and is filed
separately as `docs/issues/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`:
`path.with_extension("tmp")` *replaces* the extension rather than appending, so `x.md` and
`x.rs` in one directory both resolve to `x.tmp`. Different severity class — it is
cross-file corruption rather than a leak — and it changes a filename, so it deserves its
own reproduction rather than riding along here.
## Tests added

`atomic_write_removes_its_tmp_file_when_the_write_itself_fails` — `src/util/fs.rs`,
beside the existing `atomic_write_preserves_exec_bit`. Linux-only
(`#[cfg(target_os = "linux")]`): `/dev/full` does not exist on macOS and CI runs a macOS
lane.

**The reproduction this file originally prescribed could not reproduce the bug.** The old
Resume proposed forcing the write to fail "e.g. write to a directory instead of a file, or
a read-only tmp path". Both make `File::create` fail — and if create fails, nothing is
created, so nothing leaks and the test **passes against the unfixed function**. That is a
vacuous regression test, specified by the artifact meant to prevent the bug. Recorded
because a record can carry a defect forward with more authority than no record: the next
reader implements what it says.

The leak needs create to **succeed** and `write_all` to **fail** — the ENOSPC shape. A
symlink at the tmp path pointing to `/dev/full` gives exactly that: the open succeeds,
every write returns ENOSPC. Verified at the shell before any Rust was written.

Four properties are pinned, each with the mutation it dies on named in a comment:

| # | assertion | the mutation it catches |
|---|---|---|
| 1 | `Err` is returned (`expect_err`) | a version swallowing the error and returning `Ok` would satisfy cleanup while breaking the function's purpose |
| 2 | `raw_os_error() == Some(28)` | **positive control** — a mechanism that began yielding EACCES would satisfy 3 and 4 while exercising a different path |
| 3 | target **byte-identical** | pins "not modified if present", not merely "not created if absent" — the atomicity guarantee, previously pinned by nothing on the failure path |
| 4 | tmp entry gone (`symlink_metadata`) | the leak itself. `exists()` follows the link and would report on `/dev/full`, which always exists |

Plus a loud-absence guard: a missing `/dev/full` **asserts** rather than skipping, because a
graceful skip is a clean `0 passed` character-identical to real coverage.

**Demonstrated, not claimed.** With the fix, 30/30 in `util::fs`. With the fix reverted,
the test FAILS at assertion (4): *"atomic_write leaked /tmp/.tmpQ0RXKJ/target.tmp after the
write failed"*. Assertions (1)–(3) **pass** against the unfixed function — the broken
version really does return `Err`, really does return ENOSPC, and really does leave the
target intact. Only cleanup is missing, so the test discriminates on the defect rather than
incidentally. The source mutation ran under a `trap … EXIT` installed *before* it, and the
restore was verified by hash rather than assumed.
## Workarounds
Stray `<name>.tmp` files left by this bug are always zero-length (or partially written) and
safe to delete manually once free disk space is confirmed; the real target file is never
touched by this failure mode, so no data recovery is needed. `git status` will show them if
they land inside a tracked directory that isn't `.gitignore`d for `*.tmp`.

## Resume

N/A — fixed at `f671c3a1` with a regression test that is demonstrated to fail against the
unfixed function.

One thread deliberately left open and owned elsewhere: the `with_extension` tmp-path
collision, filed as
`docs/issues/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`.

One caveat carried in `unverified:` rather than here, so a query can read it: this fix has
not been observed under a fully green gate, because an unrelated uncommitted `tree.rs` was
failing `tool_surface_under_budget` at commit time.
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
