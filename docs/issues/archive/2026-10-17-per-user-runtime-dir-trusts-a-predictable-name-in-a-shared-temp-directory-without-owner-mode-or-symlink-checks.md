---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md
severity: medium
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

Fixed 2026-10-24. Fix SHA (experiments): `8d1bd4917e2bd22947bebfaa5bc5a05f1c0239eb`. Patch-id (`git show <sha> | git patch-id --stable`): `6f2bd8b2b5fece6d05896f2db884f9f4e3a15708`.

Verified on the committed bytes in a clean detached worktree of `ccf9e54ef` (no peer work): `./scripts/gate.sh` exited 0 with the lean lane at 5248 passed and the default lane at 7890 passed, 0 failed, and the Python lane at 1274 passed. The gate on the shared tree had been red only because of a peer's uncommitted `src/tools/grep.rs`.

The operator chose to refuse with an error. In detail:

- `verify_private_dir`, `create_private_dir`, `effective_uid`, `Uid` and `DirRefusal` moved verbatim from `src/tools/run_command/attribution.rs` into the new `src/util/private_dir.rs`. `attribution.rs` imports them.
- `per_user_runtime_dir()` now returns `Result<PathBuf, RuntimeDirError>`. `RuntimeDirError` has three variants: `NotAbsolute`, `Refused { dir, why }` and `Create { dir, source }`. Its text names the path and tells the user to set `XDG_RUNTIME_DIR` or `TMPDIR`.
- The pure core is `runtime_dir_from(xdg, temp, name_uid, verify_uid)`. It ignores a relative or missing `XDG_RUNTIME_DIR`. It verifies an absolute existing one and refuses it unless private. It requires an absolute `temp_dir()`. It creates `codescout-<uid>` without `recursive`, then verifies it.
- The signature change reaches every caller. `peer_socket_path_for_workspace`, `peer_lock_path_for_workspace`, `lsp::mux::{socket,lock}_path_for_workspace`, `index_lock::{lock_path, acquire}`, `PeerEntry::socket_path`, `peer::launch::ensure_peer_serve`, `peer::server::run`, `main` (`PeerServe`) and `LspManager::get_or_start_via_mux_waiting` propagate the error with `?`. `index_lock::peek` returns `None` and `LspManager::count_mem_kill_marker` returns early on a refused directory.

Behaviour change: an `XDG_RUNTIME_DIR` that is not private (for example `/tmp`, mode 1777) was accepted before. It is now refused with an error.

## Tests added

Eight tests in `socket_discovery::runtime_dir_trust` (`src/socket_discovery.rs`), run through the `runtime_dir_from` seam so none touches the real runtime directory:

- `a_missing_directory_is_created_private_and_reused`
- `a_symlink_at_the_name_is_refused`
- `a_group_or_world_writable_directory_is_refused`
- `a_directory_owned_by_another_uid_is_refused`
- `an_empty_or_relative_temp_dir_is_refused_and_creates_nothing`
- `a_world_writable_xdg_directory_is_refused`
- `a_private_xdg_directory_is_used`
- `the_refusal_names_the_path_and_the_way_out`

They were written first and failed against a stub before the implementation (RED), then passed (GREEN). The foreign-owner case uses a uid seam, so it needs no second account.

Verified 2026-10-24: 129 targeted tests pass (`socket_discovery`, `index_lock`, `lsp::mux`, `peer::`, `private_dir`, `attribution`) and `cargo check --workspace --all-targets` is clean. The full gate was NOT green on the final bytes: one test, `server::tests::every_declared_alias_is_absent_from_the_schema`, fails on a peer's uncommitted `src/tools/grep.rs`, and `fmt-mine` refuses that file. Neither failure touches this change.

## Workarounds

Use a per-user `TMPDIR` (for example under `$XDG_RUNTIME_DIR`), which also avoids the shared `/tmp` exposure.

## Resume

Closed. Nothing to resume. The behaviour change (a non-private `XDG_RUNTIME_DIR` is refused with an error) is in the commit message of the fix SHA above.


## Fix provenance

- **SHA:** `8d1bd4917e2bd22947bebfaa5bc5a05f1c0239eb` (`experiments`)
- **patch-id:** `6f2bd8b2b5fece6d05896f2db884f9f4e3a15708`

## References

- `docs/issues/2026-10-17-attribution-helper-runs-python-from-a-predictable-shared-temp-directory.md`
- `src/socket_discovery.rs:17-40`
