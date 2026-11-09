---
id: '427cace6a7ea5cb5'
kind: bug
status: open
title: 'BUG: a language-server descendant rewrites the tracked Cargo.lock with no tool call, so no session is recorded as its writer'
tags:
- cluster/authorship-unrecoverable-after-the-fact
opened: 2026-11-10
owner: marius
related:
- '91b5ffce06ede31a'
severity: low
unverified: 'CLEARED 2026-11-10. Was: the provenance verdict for a mux-written Cargo.lock was read from scripts/file-provenance.py, not run end to end. M3 ran it: UNKNOWN. Still true: the write itself was measured once, in a scratch workspace, not in this checkout.'
---

# BUG: a language-server descendant rewrites the tracked Cargo.lock with no tool call, so no session is recorded as its writer

## Summary

When a session adds a path dependency to a `Cargo.toml`, the `cargo check` that rust-analyzer starts below the codescout mux rewrites the tracked `Cargo.lock`. No tool call of any session writes that file. `scripts/file-provenance.py` keeps no record of a writer, so by the code the path reads `UNKNOWN`. No mechanical consequence was found: `scripts/fmt-mine.sh` handles only `.rs` files. Severity is low.

## Class fit

This bug shows the trait of the class: a tracked file changes in the shared tree and no channel records who caused it. It differs from the class's other members in one way. The writer is not a peer session. It is a process that the session's own language server started, so the cause is the session's own earlier edit of `Cargo.toml`, and that link is not recorded anywhere.

## Symptom (Effect)

After the manifest edit, `git status --porcelain` in the scratch checkout showed:

```
 M Cargo.lock
```

Before the edit, the same command showed only `!! target/`. No `.rs` file and no other non-ignored path changed.

## Reproduction

Measured once, on 2026-11-09, as M2 of the archived bug `91b5ffce06ede31a` (the `Fix provenance` and `Review trail` sections there). The script is `/home/marius/.cache/cc-tmp/claude-1000/-home-marius-work-claude-codescout/677278c4-68c9-4408-b2f4-94cb1701726b/scratchpad/m2_mux_writes.py`.

1. Create a scratch Cargo workspace with a committed `Cargo.lock`.
2. Start `~/.cargo/bin/codescout mux … -- rust-analyzer` under `strace -f`.
3. Send `didOpen`, then append a path dependency to `app/Cargo.toml`, then send `didSave` and `didChangeWatchedFiles`. Wait 45 s.
4. Run `git status --porcelain --ignored`.

Not reproduced in this repository's own checkout. `Cargo.lock` is clean there on 2026-11-10.

## Environment

Linux 7.2.8-zen1-2-zen. Mux started from `~/.cargo/bin/codescout`. The rust-analyzer version was not recorded. Scratch workspace, not this repository.

## Root cause

Measured 2026-11-09 (`strace -f` of the mux): pid 768298 ran `cargo check --workspace --message-format=json --manifest-path <checkout>/Cargo.toml --keep-going --all-targets`. It was a descendant of the mux. It opened `<checkout>/Cargo.lock` with `O_RDWR|O_CREAT` and added `dependencies = ["dep"]` to the `app` package. The command line has no `--locked` and no `--frozen`.

Why cargo rewrote the file is inferred, not measured: the lock file no longer matched the changed manifest.

On the provenance side, measured 2026-11-10: `grep` over `scripts/*.py` finds no cargo-aware attribution rule, and the only `Cargo.lock` hit is `scripts/architecture-boundary-probe.py`. The branch that prints `UNKNOWN` for a dirty path with no recorded writer is at `scripts/file-provenance.py:1168`. That branch states that absence of a record is a statement about coverage, not about ownership.

## Evidence

- M2 text: `docs/issues/archive/2026-11-09-fmt-mine-is-refused-for-as-long-as-the-rust-analyzer-mux-lives-because-it-inherits-the-provenance-lock.md`, line 109.
- `scripts/fmt-mine.sh:91` drops `SHARED`, `PEER` and `UNKNOWN` paths, but receives only `.rs` files, so `Cargo.lock` does not reach it.
- `docs/conventions/shared-checkout-commit-sequence.md` does not mention `UNKNOWN`. No convention in `docs/conventions/*.md`, `docs/RELEASE.md` or `scripts/*.sh` says what a session does with a dirty `Cargo.lock`, except `docs/RELEASE.md:42` (`git add Cargo.toml Cargo.lock` in the release step).

## Hypotheses tried

None yet.

## Fix

Unknown. Options, none evaluated:

1. Leave it. `UNKNOWN` is the honest verdict, and nothing acts on it.
2. Name the manifests that a session edited in the window, in the `UNKNOWN` output for `Cargo.lock`, without changing the verdict.
3. Pass `--locked` to the cargo commands that rust-analyzer starts. This stops the write, but rust-analyzer then reports an error after each dependency edit until someone updates the lock file.
4. Attribute `Cargo.lock` to the writers of the manifest. This infers ownership, which `file-provenance.py` refuses to do elsewhere.

## Tests added

N/A — not fixed.

## Workarounds

Before committing a manifest change, read `git diff Cargo.lock`. If the change only adds the dependency you added, it is yours.

## Resume

The verdict is measured (M3, below). The remaining question is whether this record should be fixed or closed, because M3 shows the tool already gives an honest answer and no consequence was found.

M3, 2026-11-10: a throwaway test in the session scratchpad (`test_m3_cargo_lock.py`) used the fixtures of `tests/test_harness_provenance.py`. It committed `Cargo.toml` and `Cargo.lock`, then wrote both on disk. One recorded `Write` named `Cargo.toml`. Nothing recorded the lock file. The real `scripts/file-provenance.py` printed `MINE      Cargo.toml` and `UNKNOWN   Cargo.lock`. Its text for `Cargo.lock` says that absence of a record is a statement about coverage and must not be read as "not mine".

Not established: whether the cargo run was rust-analyzer's build-script pass or flycheck, any other language server, a crate with a `build.rs`, and more than one run of M2.

## References

- Archived bug `91b5ffce06ede31a`, measurement M2.
- `scripts/file-provenance.py:1168`, `scripts/fmt-mine.sh:91`.
