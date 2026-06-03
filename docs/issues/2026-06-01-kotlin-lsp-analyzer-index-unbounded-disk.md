---
status: open
opened: 2026-06-01
severity: high
owner: marius
related: [2026-05-30-cross-worktree-kotlin-jvm-shared-system-path, 2026-05-30-shared-server-global-active-project-race]
tags: [kotlin, lsp, disk, storage, gradle]
kind: bug
---

# BUG: kotlin-lsp analyzer index escapes `--system-path`, grows unbounded in `~/.config/JetBrains/analyzer`, fills the disk

## Summary
codescout spawns kotlin-lsp per workspace path and isolates the IntelliJ **system** dir per
worktree via `--system-path=$TMPDIR/codescout-mux-kotlin-lsp-<ws_hash>` (the fix from
`2026-05-30-cross-worktree-kotlin-jvm-shared-system-path`). But kotlin-lsp writes its
**analyzer workspace index** (a RocksDB store) to a *separate* location —
`$XDG_CONFIG_HOME/JetBrains/analyzer/workspaces/<hash>/rocks/` (default `~/.config`). That
path is **not** covered by `--system-path`, so it lands in the user's real home config,
**outside** codescout's temp-dir isolation. It is **unbounded** (no cap, no compaction
trigger, no cleanup on idle-shutdown) and **accumulates per worktree**. On a 2.2 GiB Kotlin
project it grew to **31 GiB for a single workspace** (~14×), and across worktrees to
**44–50 GiB total**, filling a 196 GiB disk to 98% — twice in one day.

## Symptom (Effect)
`~/.config/JetBrains/analyzer` ballooned and filled `/`:
```
$ du -shc ~/.config/JetBrains/analyzer/workspaces/*
31G   .../workspaces/3e0217afa7871f45a9f4fa93f3d68394   # mtime "now", actively growing
11G   .../workspaces/f6bcd1194ac5b3b3eb022041357d9aa9
1.5G  .../workspaces/6ec74d3317b558183a213dd06c4bfd1d
444M  .../workspaces/f209c79c6d7046e82c9fae9a9954304c    # 4 stale workspaces from a
426M  .../workspaces/c2fcc0f74ccb8c803ac4300eaf888d62    # prior day, never cleaned
424M  .../workspaces/5692c359e54c10ed19187f4df526087f
616K  .../workspaces/31aa2d2044c923cbb5fdfc8c665dc5dc
44G   total
```
Disk: `196G  189G  5.2G  98% /` before cleanup → `139G  55G  72%` after `rm -rf
~/.config/JetBrains/analyzer/workspaces/*` (reclaimed ~50 GiB). The store is pure RocksDB
(`rocks/v492/*.sst`, `MANIFEST-*`, `CURRENT`); individual `.sst` files were 0.24–0.34 GiB
written continuously over a multi-hour window. The same dir was observed at **24 GiB in the
morning and 44 GiB by late afternoon** — i.e. it doubled in a working day and re-filled the
disk hours after a previous cleanup.

No process held the files open at inspection time (`find /proc/*/fd -lname '*analyzer/workspaces/*'`
→ 0 fds), so the index outlives the kotlin-lsp JVM that wrote it — deletion frees space
immediately but the data is never reclaimed by codescout or kotlin-lsp itself.

## Reproduction
1. Open a Kotlin/Gradle repo with multiple git worktrees in codescout (probe:
   `/home/marius/work/mirela/backend-kotlin` + 4 worktrees, project ~2.2 GiB total).
2. Let codescout activate the workspace(s) → it spawns kotlin-lsp per worktree path.
3. Use Kotlin LSP features (symbols/edit_code) so kotlin-lsp builds/refreshes its analyzer index.
4. `du -sh ~/.config/JetBrains/analyzer` → tens of GiB; grows per active worktree, never shrinks.
5. `--system-path` (`$TMPDIR/codescout-mux-kotlin-lsp-<ws_hash>`) stays small — the bulk is in
   `~/.config/JetBrains/analyzer`, NOT under the temp system-path.

## Environment
Arch Linux, 30 GiB RAM, Btrfs on NVMe (196 GiB `/`), codescout (peer-delegation worktree
build, `src/lsp/servers/mod.rs` kotlin branch as of 2026-06-01), kotlin-lsp (JetBrains
intellij-server). Probe: `/home/marius/work/mirela/backend-kotlin` + worktrees
`weekly-pattern`, `cc-exp-3`, `prompt-tdd-integration`, `solver-config-refactor`.

## Root cause
`src/lsp/servers/mod.rs` kotlin branch (`default_config("kotlin", …)`, ~L59–90) isolates only
**two** of kotlin-lsp's storage roots:
```rust
let system_dir  = std::env::temp_dir().join(format!("codescout-mux-kotlin-lsp-{ws_hash}"));
let gradle_home = std::env::temp_dir().join(format!("codescout-mux-gradle-{repo_hash}"));
// args:  --stdio  --system-path=<system_dir>
// env:   GRADLE_USER_HOME=<gradle_home>
// idle_timeout_secs: Some(300)
```
kotlin-lsp / the IntelliJ analysis backend has a **third** storage root — the *analyzer
workspace index* — which is NOT controlled by `--system-path`. It resolves under
`$XDG_CONFIG_HOME/JetBrains/analyzer` (and `XDG_CONFIG_HOME` is **not** set in the spawned
env, so it defaults to the user's real `~/.config`). Consequences:

1. **Escapes isolation:** the index is written to the user's home config, not the per-workspace
   temp dir, so it is never namespaced, capped, or swept with the temp system-path.
2. **Unbounded growth:** ~14× the source tree for one workspace; no compaction/GC observed.
   RocksDB `.sst` files keep accumulating (likely re-indexing `build/`, the produced `.jar`,
   `.gradle`, and/or the sibling `.worktrees/` copies under the workspace root).
3. **No lifecycle cleanup:** the 300 s idle-timeout shuts the JVM down, but the analyzer dir is
   left behind in full — and `rm` of the system-path temp dir would not touch it anyway.
4. **Per-worktree multiplication:** one workspace hash per worktree → one analyzer index per
   worktree (7 hashes observed, 2 of them tens of GiB), compounding the total.

This is the disk-side analogue of `2026-05-30-cross-worktree-kotlin-jvm-shared-system-path`:
that fix correctly per-worktree-keyed `--system-path` (RAM/index-aliasing), but a storage root
the flag does not govern slipped the net.

## Evidence
- Index path & format: `~/.config/JetBrains/analyzer/workspaces/<hash>/rocks/v492/*.sst` +
  `MANIFEST-004091`, `CURRENT` → RocksDB. Workspace `<hash>` matches codescout's
  `workspace_hash(workspace_root)` granularity (per worktree).
- Sizes / mtimes: see Symptom — 31 GiB workspace last written at the moment of inspection
  ("16:19"), i.e. live growth during normal use; doubled 24→44 GiB across one day.
- Not held open: `find /proc/*/fd -lname '*analyzer/workspaces/<hash>*'` → 0 fds;
  `lsof -nP | grep -c 'JetBrains/analyzer.*deleted'` → 0. Orphaned-but-retained on disk.
- `--system-path` temp dir stayed small while `~/.config/JetBrains/analyzer` held the bulk →
  confirms the flag does not redirect the analyzer index.
- Project is only 2.2 GiB total (`du -xhd2 /home/marius/work/mirela/backend-kotlin`), of which
  `.worktrees` 1.1 GiB and `build` dirs are the likely over-indexed inputs.

## Hypotheses tried
1. **Hypothesis:** the bloat is codescout's own index (embeddings / tantivy / qdrant).
   **Test:** `du -sh .../backend-kotlin/.codescout`.
   **Verdict:** rejected — codescout's per-project store is ~322 MiB. The 44 GiB is entirely
   `~/.config/JetBrains/analyzer` (kotlin-lsp's store).
2. **Hypothesis:** `--system-path` already contains the analyzer index (so per-worktree keying
   bounds it).
   **Test:** compare sizes of `$TMPDIR/codescout-mux-kotlin-lsp-<ws_hash>` vs
   `~/.config/JetBrains/analyzer/workspaces/<hash>`.
   **Verdict:** rejected — the analyzer index lives under `~/.config/JetBrains`, not the
   system-path temp dir.
3. **Hypothesis:** a live process is holding/growing it and deletion won't reclaim space.
   **Test:** `find /proc/*/fd` + `lsof` for the workspace dir; `df` before/after `rm`.
   **Verdict:** rejected — 0 open fds; `rm` reclaimed ~50 GiB immediately (98%→72%).

## Proposed fix (for triage — not yet implemented)
Candidate directions, smallest-blast-radius first:

1. **Redirect the analyzer storage into the per-workspace temp dir** by setting, in the kotlin
   branch `env`, `XDG_CONFIG_HOME=<system_dir>` (or the kotlin-lsp/IntelliJ-specific property
   for the analyzer/config root, e.g. an `-Didea.config.path` / dedicated analyzer-storage
   flag if kotlin-lsp exposes one — needs a quick check of kotlin-lsp's CLI/JVM options).
   This co-locates the index with the already per-`ws_hash` system-path, so it inherits
   isolation and any temp-dir sweeping. **Verify** kotlin-lsp honors `XDG_CONFIG_HOME` for the
   analyzer dir before committing.
2. **Lifecycle cleanup:** on mux idle-timeout shutdown (and on workspace deactivate), remove
   that workspace's analyzer dir, or enforce a size cap (e.g. prune when `> N` GiB).
3. **Narrow what gets indexed:** ensure the workspace root handed to kotlin-lsp excludes
   `build/`, `.gradle/`, produced `*.jar`, and sibling `.worktrees/` — a 14× index over source
   strongly implies build artifacts / worktree copies are being indexed.
4. **Backstop (ops):** a size-triggered cleanup of `~/.config/JetBrains/analyzer` is the
   symptomatic mitigation if the above can't land quickly.

Add regression tests mirroring the 2026-05-30 issue's `tests` module: assert the kotlin config
sets an analyzer/config-storage env keyed per `ws_hash` (red on current code), and that two
distinct workspace roots produce distinct analyzer storage roots.

## Workarounds
- Periodically `rm -rf ~/.config/JetBrains/analyzer/workspaces/*` when no kotlin-lsp is running
  (`pgrep -af kotlin-lsp` empty; verify 0 open fds first). Regenerates on next activation.
- Minimize concurrently-active Kotlin worktrees — each adds its own multi-GiB analyzer index
  (in addition to the ~2 GiB JVM noted in the related RAM issue).

## Resume
Open. Next: (1) confirm whether kotlin-lsp respects `XDG_CONFIG_HOME` (or another flag) for the
analyzer dir; (2) pick fix shape (redirect vs cleanup vs both) — user decision; (3) implement in
`src/lsp/servers/mod.rs` kotlin branch + tests; (4) live-verify the analyzer index lands inside
the per-`ws_hash` temp dir and is swept on idle-timeout.

## References
- Related: `docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md` (RAM/system-path; same probe project & subsystem)
- Related: `docs/issues/2026-05-30-shared-server-global-active-project-race.md`
- `src/lsp/servers/mod.rs` kotlin branch (`default_config`, ~L59–90): `--system-path`,
  `GRADLE_USER_HOME`, `idle_timeout_secs=300` — no `XDG_CONFIG_HOME` / analyzer-storage override
- `src/lsp/mux/mod.rs:14,20` (per-path `workspace_hash` keying)
- `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (§ Gradle Isolation)
- Probe & measurements captured 2026-06-01 on host with 196 GiB Btrfs `/`.
