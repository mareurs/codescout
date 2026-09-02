---
id: d5788071fe38e536
kind: bug
status: fixed
title: LockFileEx needs GENERIC_READ/WRITE, and an append-only handle has neither — 21 tests red on every windows lane
tags:
- cluster/repro-env-diverges-from-gate-env
topic: cross-platform path handling
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: high
unverified: 'No regression test, and one is not obviously writable: the property — "the handle passed to lock_exclusive carries read or write access" — is not observable from a `File`, and flock(2) ignores access mode so no Linux test can express the failure. NOT ARCHIVED for that reason; the documented archive trigger requires one. The call-site comment naming what breaks and why the line looks redundant is the interim guard. Separately: 2 of the original 21 failures REMAIN after this fix and are a DIFFERENT defect, unfiled as of this closure — see the section ''What remains, and it is not this bug''.'
---

## Summary

`export` appends each shard line under an exclusive file lock. It opens the file
append-only and then locks it:

```rust
let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
FileExt::lock_exclusive(&f)?;      // <- fails here, on Windows only
```

`fs4`'s Windows path is `LockFileEx`, which **requires the handle to carry
`GENERIC_READ` or `GENERIC_WRITE`**. std maps an append-only open to
`FILE_GENERIC_WRITE & !FILE_WRITE_DATA` — which is neither — so `LockFileEx` returns
`ERROR_ACCESS_DENIED` and every export fails.

Invisible on Unix, where `flock(2)` ignores the descriptor's access mode entirely.
**21 tests red on every `windows-latest` lane, 2 on `windows-gnu`; Linux and macOS
entirely green.**

## Symptom (Effect)

Runs `33570342471` (`a82026d7`) and `33573899069` (`9a156bd4`) — identical, 21 failures
each, same test names, same panic lines.

```
test result: FAILED. 4806 passed; 21 failed
called `Result::unwrap()` on an `Err` value: Access is denied. (os error 5)
```

**The discriminating evidence is an absence.** Of 19 captured panics, **19 say
"Access is denied" and 0 mention `opening <path>`** — and the `.open()` call is wrapped
in exactly that `with_context`. So the failure cannot be the open; it is the next
line, `FileExt::lock_exclusive(&f)?`, which carried no context at all.

## Reproduction

Any `windows-latest` or `windows-gnu` lane at or after `94f53d75`. **Not reproducible on
Linux or macOS at any effort** — see *Root cause*; that is the bug's defining property.

## Environment

`experiments` @ `a82026d7` and `9a156bd4`. Failing: `Test (windows-latest /
{no-features,default,local-embed})`, `Windows-gnu cross (MinGW + wine)`. Green: every
ubuntu and macos lane, and the full local gate on Linux.

## Root cause

Two facts, both read from source rather than recalled:

1. `fs4-0.12.0/src/windows/sync_impl.rs` — `lock_exclusive` → `LockFileEx(handle,
   LOCKFILE_EXCLUSIVE_LOCK, …)`, returning a bare `Error::last_os_error()` with no
   context. Microsoft's contract for `LockFileEx`: the handle must have been created
   with `GENERIC_READ` or `GENERIC_WRITE`.
2. `library/std/src/sys/fs/windows.rs`, `get_access_mode`:
   - `(false, _, true, None)` → `FILE_GENERIC_WRITE & !FILE_WRITE_DATA` — the arm an
     append-only open selects. Neither `GENERIC_READ` nor `GENERIC_WRITE`.
   - `(true, _, true, None)` → `GENERIC_READ | (FILE_GENERIC_WRITE & !FILE_WRITE_DATA)`
     — the arm `.read(true)` selects, which ORs `GENERIC_READ` back in.

**One site.** Every other lock in the tree (`index_lock.rs`, `lsp/manager.rs`,
`peer/server.rs`, `peer/launch.rs`, `agent/write_guard.rs`, `lsp/mux/process.rs`) opens
with `.write(true)` or `File::create`, both of which yield `GENERIC_WRITE`. Only
`shard.rs` used `.append(true)` alone.

## Hypotheses tried

1. **Hypothesis: `AUDIT_DIR = ".codescout/audit"` — a forward-slash literal joined onto
   a root and handed to `create_dir_all` — produced a mixed-separator path that Windows
   rejected with `ERROR_PATH_NOT_FOUND`.**
   **Test:** shipped the split-components fix at `9a156bd4` and re-ran the full matrix
   (`33573899069`).
   **Verdict: REJECTED. Identical 21 failures, same tests, same panic lines.** The fix
   changed nothing, and this file's original title and root-cause section asserted it.

   **Worth recording is how it was reached, because the method was the defect.** Two
   observations were generalised from a 2-of-21 sample on the *wine* lane: one panic
   reading `Os { code: 3, kind: NotFound }`, and an assertion payload that printed
   `dest: "C:\…\.tmpzRdg4S\.codescout/audit"` — backslashes then a forward slash. The
   path shape was real; it was not a *cause*. The 19 native-Windows panics all said
   `os error 5`, and none had been read. A mixed-separator path is in fact fine here:
   the failure occurs *after* `create_dir_all` and *after* the file opens.

   The `9a156bd4` change is **kept, on hygiene grounds only**: building a path from
   components is correct, the derived display form removes a duplicated literal, and
   `audit_dir_parts_carry_no_separator` is a sound guard. **It fixed nothing**, and no
   claim that it did should survive.

## Fix

**Fixed at `6d89a69b`** (patch-id `798292c26c559177245374b53e0fe7658f3b93d5`), `experiments`.

Add `.read(true)` to the `OpenOptions`, selecting the access-mode arm that carries
`GENERIC_READ`. Append semantics are unchanged. Also wraps `lock_exclusive` in a
`with_context` naming the file — the missing context is what made the failing line
ambiguous for two rounds, and a bare `last_os_error()` under a `?` is the shape to avoid.

**VERIFIED on the lanes, which is the only place it can be.** Run `33574961971`
(`6d89a69b`) against run `33573899069` (`9a156bd4`), `Test (windows-latest / default)`:

| | pre-fix | post-fix |
|---|---|---|
| passed / failed | 4806 / **21** | 4825 / **2** |
| `Access is denied` panics | **19** | **0** |
| `opening <path>` panics | 0 | 0 |

The error string this bug is named for is gone, and 19 of 21 failures cleared. That is
the outcome the diagnosis predicted, stated before the run rather than fitted after it.

**Unverifiable locally, by construction** — `flock(2)` ignores access mode, so no Linux
test can distinguish fixed from broken.

## What remains, and it is not this bug

Two of the original 21 still fail, and they are a **different defect**, unfiled:

```
delete_row_is_attributed_from_its_payload_not_a_live_join
  assertion failed: a delete row with a usable payload must not fall through to
  unattributed: ExportReport { exported: 0, …, unattributed: 1 }   left: 1  right: 0

rows_from_different_months_land_in_different_files
  read_dir(audit_dir(tmp)) -> Os { code: 3, kind: NotFound,
                                   "The system cannot find the path specified." }
```

**They are consistent with one cause, though that is not yet proven:** `export` cannot
**attribute** rows on Windows (`unattributed: 1`), so it writes nothing, so the audit
directory is never created — and the second test's `read_dir` then fails with `NotFound`
for exactly that reason (`export` creates the directory only when it has something to
write; see the comment on the `lines` helper). Attribution compares an `abs_path` taken
from a row payload against `repo_root`, which is the kind of comparison that is
separator- and case-sensitive on Windows and not on Unix.

**These two are also the pair the WINE lane failed in the very first run** — the 2-of-21
sample this file's original, retracted diagnosis was drawn from. So that sample was not
merely small: it was taken from the *minority* failure mode, and described a defect that
was never the one causing the other 19.

**A regression test is owed and is not obviously writable.** The property — "the handle
passed to `lock_exclusive` carries read or write access" — is not observable from a
`File`, and asserting it on Windows only reproduces the blind spot in a second place. The
honest interim guard is the comment at the call site, which names what breaks and why the
line looks redundant.

## Provenance

Found by pushing `experiments` so the `IC-5` wine pin (`58d85263`) could be verified.
The pin itself succeeded — the lane reports `>>> wine here: wine-11.16` — and is
unrelated to this defect: the module did not exist at the pre-pin commit, and native
`windows-latest` fails 21 where wine fails 2, which rules the emulator out.
