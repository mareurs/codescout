---
status: open
opened: 2026-10-17
closed:
severity: medium
owner: marius
related: [docs/issues/2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: `per_user_runtime_dir` trusts `$TMPDIR/codescout-<uid>` and `$XDG_RUNTIME_DIR` without checking owner, mode or symlink, and the peer, mux and index-lock sockets live in it

## Summary

`per_user_runtime_dir` (`src/socket_discovery.rs:17-40`) returns `$XDG_RUNTIME_DIR` if it exists, else
`<temp_dir>/codescout-<uid>`, creating the latter with mode 0700 and IGNORING the result of the create. It never
looks at what is at that name afterwards. The peer socket and lock, the LSP mux socket and the index lock all live
in the returned directory (`src/socket_discovery.rs:44,52`, `src/lsp/mux/mod.rs:12`,
`src/retrieval/index_lock.rs:138,203,222,481`).

## Class fit

Same class as the attribution temp-directory issue (trust placed in a name in a shared location, with the check
missing on the unix owner and mode bits), which is why it is filed unclassified beside it: a directory another user
created first, or a link to one, is used as if it were ours.

## Symptom (Effect)

Measured, not hypothetical, for the three states a local attacker (or a misconfigured host) can leave. Another
UID cannot be created without a second account, so the planted states were made as the same user; what is shown is
what the function ACCEPTS, which is the missing check.

## Reproduction

Measured 2026-10-17 on this branch at `c18a0dbe`, with a throwaway `#[test]` (reverted, not committed) that called
`per_user_runtime_dir()` and printed `symlink_metadata` of the result, run as
`env -u XDG_RUNTIME_DIR TMPDIR=<scratch> cargo test --lib <probe> -- --nocapture`:

```
A  symlink at <TMPDIR>/codescout-1000 -> another directory (0755)
   returned=<TMPDIR>/codescout-1000 is_symlink=true mode=777 canonical=<the other directory>
B  real directory at <TMPDIR>/codescout-1000, mode 0777
   returned=<TMPDIR>/codescout-1000 is_symlink=false mode=777            (mode left at 0777)
C  XDG_RUNTIME_DIR=<world-writable directory>
   returned=<that directory> is_symlink=false mode=777
D  TMPDIR="" (what std::env::temp_dir returns verbatim)
   returned="codescout-1000"  canonical=<current working directory>/codescout-1000
```

A and B are accepted outright. D creates the directory in the process's working directory. C is the XDG spec's own
guarantee (the directory is meant to be the user's, 0700) and is the weakest of the four: a host that violates it is
already broken.

## Environment

Linux, `std::env::temp_dir()` (a shared `/tmp` is the exposure). The cross-user step, another account creating
`/tmp/codescout-<uid>` first, was NOT run: no second account. The name is computable from the victim's uid alone.

## Root cause

`src/socket_discovery.rs:17-40`: `DirBuilder::recursive(true).mode(0o700).create(&dir)` with its `Result` discarded
(`let _ =`), then `dir` returned whatever it is. `recursive(true)` treats an existing entry as success, so neither
owner, mode nor "is it a symlink" is ever consulted. Mechanism read from the code and confirmed by the probe above.

What an attacker gains is inferred, not exercised: a directory they control at that name lets them pre-create the
socket and lock files the peer, mux and index-lock code expect to create or connect to, so a client may connect to
the attacker's listener, and an `acquire` may block or be satisfied by their lock file.

## Evidence

See the Reproduction output.

## Hypotheses tried

None needed.

## Fix

Not applied. The shape that fixed the sibling: create 0700 without `recursive`, then verify on every use that the
entry is a real directory (`symlink_metadata`), owned by `geteuid`, with no group/other write bit, falling back to a
fresh private directory (or refusing). `src/tools/run_command/attribution.rs::verify_private_dir` is that check;
it is private to that module today. Also refuse a non-absolute `temp_dir()`. Unlike the attribution engine this
directory is used for long-lived sockets, so a silent fallback to a different directory would split peers that must
find each other: refusing with a clear error is the likelier right answer here.

## Tests added

N/A: filed, not fixed.

## Workarounds

Use a per-user `TMPDIR` (for example under `$XDG_RUNTIME_DIR`), which also avoids the shared `/tmp` exposure.

## Resume

Lift `verify_private_dir` (and its `DirRefusal`) out of `src/tools/run_command/attribution.rs` into a shared
`src/util/` helper, call it from `per_user_runtime_dir`, decide refuse-versus-fallback for the socket case, and
test with the same seams (a symlink, each write bit, a uid seam).

## References

- `docs/issues/2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md`
- `src/socket_discovery.rs:17-40`
