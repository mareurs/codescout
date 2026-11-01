---
id: '9e199abce9e9ef3f'
kind: convention
status: draft
title: Resident scratch lives outside the repository tree
tags:
- conventions
- lsp
- rust-analyzer
- scratch
topic: where session and experiment scratch is written
---

# Resident scratch lives outside the repository tree

Session and experiment scratch goes under `~/.cache/codescout-resident/<repo>/<name>`, never under the repository. Get the path with `scripts/resident-scratch.sh <name>`. Bug `35bcf086deeb7cbe` is the incident.

## Why

rust-analyzer's file watcher walks every directory under the workspace root and follows symlinks. Scratch that holds links, some of them cycles, kept `documentSymbol` answering null.

- **Source:** rust-analyzer's `vfs-notify` registers a `RecursiveMode::Recursive` watch on each root and on each symlinked directory. `notify` follows links by default (`follow_symlinks: true`, `notify-8.2.0/src/config.rs:122`) and has no exclusion list. `files.excludeDirs` prunes only rust-analyzer's own load walk. This was read from rust-analyzer `master`, not from the shipped 1.97.1.
- **Measured on 2026-11-01:** one rust-analyzer held 34,425 of its 51,028 inotify watches on `resident-private` directories, although `files.excludeDirs` was sent. A controlled pair showed the option does cut memory (10.2 GB to 3.9 GB). It does not stop the watches.
- **Other walkers:** `librarian/tools/gather.rs` `gather_grep` walks the whole root with no ignore handling. A search with no match walks every entry under `.codescout`.

## Rules

1. Scratch goes outside every checkout of this repository. Use `scripts/resident-scratch.sh <name>`. It refuses a root that is inside a checkout.
2. A link inside scratch must not point at an ancestor, or at a sibling directory inside the same scratch tree. That shape made the cycles.
3. The root is keyed on the **main** checkout, so a linked worktree shares it. It is not under the gate pools: `slot_tend` reclaims `<pool>/slot-*`.
4. The codescout file tools refuse paths outside the project root. Write through the shell, or approve the directory, or add it to `extra_write_roots` in `.codescout/project.toml`. Nothing here adds that entry for you.
5. A memory or plan that names a scratch path must name the new path. A sealed directory is not moved.

## Moving what is already there

`scripts/resident-scratch-migrate.sh` is a dry run. `--apply` moves closed directories and leaves a pointer **file** at the old path, never a link, because a link would bring the walk back. It does **not** rewrite symlinks unless you pass `--retarget`: a seal can cover link targets (`closure.py` in the sealed experiment directories records `os.readlink`), so rewriting one changes sealed bytes. Links between moved directories keep their old absolute target and dangle. `--force-referenced` moves a directory a memory names; update that memory's path first.

It leaves alone a directory with a file changed in the last hour, a directory a memory names by path, and a destination that exists. A link in a directory that stays, pointing into one that moved, also dangles. A dangling link is not a directory link, so the watcher skips it. The script counts them.

## What stays in the code for now

Keep these until no checkout writes to the old path. Each costs nothing when the directory is absent.

- `LOCAL_EVIDENCE_DIRS` in `src/lsp/init_options.rs`.
- The `resident-private` skip in `collect_reader_docs`, `src/prompts/mod.rs`.
- The `.gitignore` rule and its crates twin. `every_root_codescout_ignore_rule_has_its_crates_twin` reds if only one is removed.

## What this does not do

- It cannot stop a session from writing in-tree. Sessions choose the path, outside this repository. The standing checks are the helper's note when the old directory exists, and the null-answer warning, which names the directory that holds most of the links.
- It does not prove the nulls end. The null rate in `usage.db` began on 2026-10-23, before any of these links existed. Nested worktrees explain that part. After the move, the success measure is the null-answer rate in `usage.db`, not LSP start failures. The rust start failures there are mux lock conflicts.
- After the move, purge call-graph edges whose file is under `.codescout/resident-private`. Five exist, with source `lsp`.
