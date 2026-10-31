# rust-analyzer: a spinning file watcher makes documentSymbol answer null for good

Found 2026-10-31. Bug file: `docs/issues/2026-10-31-rust-analyzer-watcher-walks-the-directory-symlinks-in-resident-private-without-end-so-documentsymbol-stays-null.md` (id `35bcf086deeb7cbe`).

## Symptom

- `symbols`, `references` and `edit_code` say "the language server answered textDocument/documentSymbol with null … on every attempt". It does not end after a minute.
- rust-analyzer holds 5 to 15 GB RSS and about one core.
- The first `documentSymbol` right after start can still return symbols. That is the syntax-only answer before the workspace loads. It is not proof that the server is healthy.

## Cause on this checkout

`.codescout/resident-private/` (gitignored experiment evidence) holds 32,136 real directories and 109 symlinks to directories. Most links point back into the same tree. rust-analyzer's `notify-rs inoti` thread walks them without end: 1,000,195 `inotify_add_watch` calls with 1,000,191 distinct paths in 87.7 s.

`files.excludeDirs` does NOT stop this walk. It only stops file loading. It cut RSS from about 5.4 GB to 756 MB and changed nothing else. Do not use it as the fix.

Not verified: that rust-analyzer's `notify` follows symlinks. The evidence is the path shape and a single-variable test (deleting the 109 links in a scratch copy ended the spin).

## How to diagnose in minutes

1. `top -H -b -n 1 -p <rust-analyzer pid>`. A thread named `notify-rs inoti` near 100% means this problem. The main `LspServer` thread stays near 0.
2. Read `/proc/<tid>/stat` after the `)`. A kernel time (field 13) that is about ten times the user time (field 12) fits a syscall loop.
3. Count watches: sum the `inotify` lines in `/proc/<pid>/fdinfo/*`. This checkout reaches 48,816. The limit is 524,288, so the limit is not the cause.
4. `strace` and `/proc/<tid>/syscall` are blocked on an unrelated process (`kernel.yama.ptrace_scope=1`). Start rust-analyzer as a child of your own script, and put `strace -f --seccomp-bpf -e trace=inotify_add_watch -o file` in front of it. A parent may trace its child.
5. Standalone driver: send `initialize` with `window.workDoneProgress` and `workspace.workspaceFolders`, no `didChangeWatchedFiles`. Then rust-analyzer uses its own watcher, as the codescout mux handshake does. Ask `documentSymbol` every 10 s.

## Controls that worked

- A `git archive HEAD` copy of the repo: 290 watches, idle watcher, symbols every time.
- The same copy plus `resident-private` with its symlinks: spin and null answers again.
- That copy with the 109 directory symlinks deleted: 32,426 watches, idle watcher, symbols after a 12 s load window.

## Related

- A killed rust-analyzer also ends its mux (the mux exits with its child). The next request starts a fresh pair that spins again until the tree changes.
- `earlyoom` runs here with `--prefer ...rust-analyzer`. It did not kill it in this case. Swap was full.
- Older, same family: `docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md` (nested worktrees stretched the null window; the exclusion in `src/lsp/init_options.rs` was that fix).
