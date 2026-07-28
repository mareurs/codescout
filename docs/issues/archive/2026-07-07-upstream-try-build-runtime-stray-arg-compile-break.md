---
status: fixed
opened: 2026-07-07
closed: 2026-07-07
severity: high
owner: marius
related: []
tags: [build, regression, librarian, windows]
kind: bug
---

# BUG: `origin/experiments` HEAD fails to compile — stray `lsp.clone()` argument added to zero-arg `try_build_runtime()`

## Summary
Commit `62457959` ("fix(server): normalize post_process's root_prefix to
forward-slash", part of the larger forward-slash-normalization commit series)
accidentally included an unrelated hunk that changed
`crate::librarian::try_build_runtime().await` to
`crate::librarian::try_build_runtime(lsp.clone()).await` in
[src/server.rs](../../src/server.rs), even though
`try_build_runtime`'s signature (`src/librarian/adapter.rs:20`) has taken zero
arguments since it was created (May 16, 2026) and was never changed. This
broke the build for anyone compiling with the `librarian` feature — which
`cargo rb` (`build --release --features server-stack`) enables — on
`origin/experiments` HEAD (`1a3b6fc2`) at the time this was found.

## Symptom (Effect)
```
error[E0061]: this function takes 0 arguments but 1 argument was supplied
   --> src\server.rs:153:38
    |
153 |             if let Some(lib_ctx) = crate::librarian::try_build_runtime(lsp.clone()).await {
    |                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ ----------- unexpected argument of type `Arc<dyn LspProvider>`
    |
note: function defined here
   --> src\librarian\adapter.rs:20:14
    |
 20 | pub async fn try_build_runtime() -> Option<Arc<LibToolContext>> {
    |              ^^^^^^^^^^^^^^^^^

error: could not compile `codescout` (lib) due to 1 previous error
```

## Reproduction
1. `git fetch && git checkout origin/experiments` (or any branch containing
   commit `62457959545c34694635f5631b9c32a1439af9e1`, confirmed present at
   tip `1a3b6fc2` via `git merge-base --is-ancestor 62457959... origin/experiments`
   → exit 0)
2. `cargo build --release --features server-stack` (i.e. `cargo rb`)
3. Compile error above.

## Environment
- OS: Windows, PowerShell 7
- Toolchain: stable Rust
- Feature set: `server-stack` (implies `librarian`), i.e. `cargo rb`
- Branch: `experiments` @ `1a3b6fc2` (before this session's local fix)

## Root cause
[src/server.rs:153](../../src/server.rs#L153) — the diff for commit
`62457959` shows three hunks: (1) adding a `to_forward_slash` import, (2) an
unrelated one-line change adding `lsp.clone()` to the `try_build_runtime`
call, and (3) the actual intended fix (`post_process`'s `root_prefix`
normalization). Hunk (2) does not correspond to anything in the commit
message and doesn't match any corresponding signature change in
`src/librarian/adapter.rs` — `try_build_runtime` has been a zero-arg function
since its creation. Most likely an accidental stray edit / bad diff-hunk
inclusion during that commit's authoring (possibly a leftover from an
abandoned local experiment that got swept into the same commit).

## Evidence
```
$ git log -p -S "try_build_runtime(lsp.clone())" -- src/server.rs
commit 62457959545c34694635f5631b9c32a1439af9e1
    fix(server): normalize post_process's root_prefix to forward-slash
...
-            if let Some(lib_ctx) = crate::librarian::try_build_runtime().await {
+            if let Some(lib_ctx) = crate::librarian::try_build_runtime(lsp.clone()).await {

$ git log -p -S "pub async fn try_build_runtime" -- src/librarian/adapter.rs
commit d48bf9922766a690a37516936c0b6270b535480e (May 16, 2026)
+pub async fn try_build_runtime() -> Option<Arc<LibToolContext>> {
(only one match — signature was never changed)
```

## Hypotheses tried
1. **Hypothesis**: a later commit (before `1a3b6fc2`) updated the
   `try_build_runtime` signature to accept `lsp` but a caller elsewhere was
   missed.
   **Test**: `git log -p -S "pub async fn try_build_runtime"` on
   `src/librarian/adapter.rs` returns only the original May 16 definition.
   **Verdict**: rejected — the signature was never touched; this is a
   pure caller-side error, not a partially-applied refactor.

## Fix
Reverted the stray argument in [src/server.rs:153](../../src/server.rs#L153):
`crate::librarian::try_build_runtime(lsp.clone()).await` →
`crate::librarian::try_build_runtime().await`, matching the actual (and
always-unchanged) function signature. `cargo rb` now compiles clean; local
commit (not pushed — repo convention is fetch/pull only, see
`/memories/repo/git-workflow.md`).

## Tests added
N/A — this is a compile-time argument-count mismatch; the type system
catches any regression immediately, no runtime test needed.

## Workarounds
Revert the single line locally as shown in Fix, or check out any commit
before `62457959` if building with `--features server-stack`/`librarian`.

## Resume
N/A — fixed. If this recurs, check `git log -p -S "try_build_runtime" -- src/server.rs` first.

## References
- [src/server.rs:150-155](../../src/server.rs#L150-L155) — the fixed call site
- [src/librarian/adapter.rs:20](../../src/librarian/adapter.rs#L20) — the unchanged zero-arg signature
- Upstream commit `62457959545c34694635f5631b9c32a1439af9e1` on `experiments`
