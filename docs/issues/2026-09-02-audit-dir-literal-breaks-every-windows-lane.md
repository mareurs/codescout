---
id: '866d72c81745dc7b'
kind: bug
status: open
title: AUDIT_DIR's forward-slash literal fails create_dir_all on Windows — 21 tests red on every windows lane
tags:
- cluster/repro-env-diverges-from-gate-env
topic: cross-platform path handling
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
---

## Summary

`AUDIT_DIR` is a forward-slash string literal that is `join`ed onto a root and then passed
to `create_dir_all`. On Windows that is **one path component containing a `/`**, so
`create_dir_all` — which splits on the platform separator, `\` — never creates the
intermediate `.codescout`, and the call fails with `ERROR_PATH_NOT_FOUND`.

```rust
// src/librarian/catalog/audit/host.rs:26
pub(crate) const AUDIT_DIR: &str = ".codescout/audit";
```

**21 tests fail on every `windows-latest` lane** (no-features, default, local-embed) and
**2 on the `windows-gnu` wine lane**. Linux and macOS are entirely green, because `/` is
the separator there and the literal is correct by accident.

## Symptom (Effect)

Run `33570342471`, commit `a82026d7`, 2026-09-01T23:17Z.

```
test result: FAILED. 4798 passed; 21 failed          # windows-latest / default
test result: FAILED. 4799 passed;  2 failed          # windows-gnu (wine 11.16)

panicked at src\librarian\catalog\audit\shard.rs:1069:14:
called `Result::unwrap()` on an `Err` value:
  Os { code: 3, kind: NotFound, message: "Path not found." }
```

The tell is in an assertion payload that printed the path it built:

```
dest: "C:\\users\\runner\\AppData\\Local\\Temp\\.tmpzRdg4S\\.codescout/audit"
                                                          ^^^^^^^^^^^^^^^^^^
```

Backslashes throughout, then a forward slash in the final segment. That is `Path::join`
treating `".codescout/audit"` as a single opaque component rather than two.

Windows API calls mostly *tolerate* `/` as a separator, which is why reads elsewhere in the
codebase using the same literal style do not fail. `create_dir_all` is the exception: it
walks ancestors by splitting on `\`, sees one component, and calls `CreateDirectoryW` on the
full path while `.codescout` does not yet exist.

## Reproduction

Any `windows-latest` or `windows-gnu` CI lane on `experiments` at or after `94f53d75`.
Not reproducible on Linux or macOS at all — see *Root cause* for why that is the bug's
defining property rather than an inconvenience.

## Environment

`experiments` @ `a82026d7`. Failing: `Test (windows-latest / {no-features,default,
local-embed})`, `Windows-gnu cross (MinGW + wine)`. Green: all ubuntu and macos lanes,
and the full local gate (`cargo fmt`, `cargo clippy --workspace --all-targets --features
local-embed`, both `cargo test --workspace` lanes) on Linux.

## Root cause

One const, at `src/librarian/catalog/audit/host.rs:26`. Its consumers do
`repo_root.join(AUDIT_DIR)` and then `std::fs::create_dir_all(&dir)` —
`shard.rs:455`, `shard.rs:1148`, `shard.rs:1929`, `shard.rs:2238`, `audit_log.rs:310`,
`audit_log.rs:638`, among others.

**Why no reviewer could have caught it and no local gate could have failed:** the literal
is correct on the platform every author and every pre-commit hook runs on. The four-command
gate in `CLAUDE.md` is host-only, so a Linux session gets a complete green while three
Windows lanes are red. That is `IC-5` exactly — the reproduction environment is not the
gating environment — and it is worth noting the merge commit `bbee621c` was reported as
*"gate green"*, which was **true of the gate that ran** and false of CI.

## Fix

Not applied — the module's author (session `bf44ba81`) has exited, and this is their
in-flight subsystem rather than a drive-by.

The shape is a one-line change plus a guard:

```rust
// host.rs — build it from components so `join` produces real separators
pub(crate) fn audit_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".codescout").join("audit")
}
```

Keep a `&str` form only where a **display** or `.gitattributes` string is wanted
(`audit_log.rs:88` formats `"{AUDIT_DIR}/"` into JSON, and `shard.rs:414-427` matches the
literal `.codescout/audit/*.jsonl` against `.gitattributes` — both of those are correct as
text and must NOT be switched to a `PathBuf`).

**A regression test is owed and is not free.** Nothing in the suite fails on Linux for this,
by construction, so the guard cannot be a normal unit test. Either a `#[cfg(windows)]`
assertion that the built path contains no `/`, or a platform-independent assertion that
`audit_dir(root)` has exactly two components below `root` — the second being the one that
runs everywhere and therefore actually guards.

## Provenance

Found by pushing `experiments` for an unrelated reason: the `IC-5` wine pin (`58d85263`)
needed a lane run, and 179 commits went up with it. The pin itself succeeded — the lane
reports `>>> wine here: wine-11.16` and 4799 passing — and these failures are **not** the
pin's: the module did not exist at the pre-pin run's commit (`902fdf6a`), so this is its
first exposure to any Windows lane, and native `windows-latest` fails 21 where wine fails
2, which rules the emulator out entirely.

