---
status: fixed
opened: 2026-06-01
severity: high
owner: marius
related: [2026-05-30-cross-worktree-kotlin-jvm-shared-system-path, 2026-05-30-shared-server-global-active-project-race]
tags: [kotlin, lsp, disk, storage, gradle]
kind: bug
closed: 2026-06-03
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

### Amplifiers discovered 2026-06-03 (systematic-debug pass)

**A. Snapshot pinning (Btrfs/snapper).** `~/.config` is on the `/home` subvolume, which is snapper-managed (`/home/.snapshots` present, modified hourly). The analyzer is a *continuously-rewritten* RocksDB store, so each `/home` snapshot pins a **different** multi-GiB state of the churn. Real on-disk cost ≈ `index_size × retained_snapshots`, far exceeding the live `du`. Confirmed empirically: `rm -rf ...analyzer/workspaces/*` dropped `du` 58 GiB → 0 but `df` reclaimed only ~4 GiB — the rest is pinned by /home snapshots (no open fds; not a deleted-but-open case). Implication: cleanup alone (fix #2/#4) does NOT reclaim space on snapshotted setups until snapshots rotate; the strongest lever is moving the index OFF the snapshotted subtree.

**B. tmpfs target trap.** `std::env::temp_dir()` resolves to `/tmp`, which is **tmpfs** (RAM-backed, `size=~63 GiB`) on this host. The existing `--system-path` / `GRADLE_USER_HOME` temp dirs stay tiny (27 MiB total) so tmpfs is fine for them — but redirecting the **30–60 GiB analyzer index** into `/tmp` (bug file's original fix #1 phrasing "into the per-workspace temp dir") would consume RAM and risk OOM. The redirect target must be **real disk AND not snapshotted** — neither `/home/*` (snapshotted) nor `/tmp` (tmpfs) qualifies as-is. This is a real design constraint, not a detail.
## Evidence
- Index path & format: `~/.config/JetBrains/analyzer/workspaces/<hash>/rocks/v492/*.sst` +
  `MANIFEST-004091`, `CURRENT` → RocksDB. Workspace `<hash>` matches codescout's
  `workspace_hash(workspace_root)` **granularity** (per worktree) — but NOT its key *value*. **Correction (2026-06-03):** the analyzer `<hash>` is **32 hex chars** (128-bit, IntelliJ path-hash, almost certainly MD5-of-path — unconfirmed), whereas codescout's `workspace_hash` is **16 hex chars** (`DefaultHasher`/SipHash-64; `src/socket_discovery.rs:10`). None of the 3 live `--system-path` hashes (`c85ec91bdbfd1aee`, `26a9e85d58931839`, `7e868829c00fa9b2`) appear among the 8 analyzer dirs. So codescout **cannot address** the analyzer dir from its own `ws_hash`. See `docs/trackers/kotlin-lsp-disk-session-log.md` F-1.
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

4. **Hypothesis:** setting `XDG_CONFIG_HOME` in the kotlin branch env redirects the analyzer index.
   **Test:** black-box LSP probe (2026-06-03) — launched kotlin-lsp with `XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`XDG_CACHE_HOME` pointed at a scratch dir (HOME intact), opened a minimal Kotlin project, watched where `JetBrains/analyzer/workspaces/<hash>` appeared.
   **Verdict:** **REJECTED** — analyzer dir created at `~/.config/JetBrains/analyzer/workspaces/4b16db1e…`; all three scratch XDG bases stayed empty. Server log: `IndexingKt - Index directory: ~/.config/JetBrains/analyzer/workspaces/…`. `XDG_CONFIG_HOME` is ignored.
5. **Hypothesis:** `--system-path` (which sets `idea.config.path` + `idea.system.path`) governs the analyzer dir.
   **Test:** same probe — `--system-path=<scratch>/system` derived `idea.config.path=<scratch>/system/config` and `idea.system.path=<scratch>/system/system` (per kotlin-lsp stderr), yet the analyzer still wrote to `~/.config`.
   **Verdict:** **REJECTED** — the analyzer index ignores `idea.config.path` / `idea.system.path`. (`--system-path` isolates the IntelliJ system/config dirs correctly; it just does not govern the analyzer store.)
6. **Hypothesis:** a dedicated JVM property overrides the analyzer storage root.
   **Test:** scanned 807 classes in `com.jetbrains.ls.snapshot.*` (product.jar — the RocksDB backend: `RocksDbStorageBackend`, `IndexingImplKt`) + `com.jetbrains.analyzer.*` for config/env keys.
   **Verdict:** **REJECTED (no clean lever)** — storage-root resolver keys off `user.home` (+ `APPDATA` on Windows); no `XDG` key, no dedicated `*.analyzer.*` path property surfaced. The only redirect lever is `-Duser.home=<dir>`, which has a broad blast radius (Gradle/JBR/java caches/prefs) and needs its own validation before use.
## Proposed fix (for triage — not yet implemented)
Candidate directions, smallest-blast-radius first:

1. **Redirect the analyzer storage into the per-workspace temp dir** by setting, in the kotlin
   branch `env`, `XDG_CONFIG_HOME=<system_dir>` (or the kotlin-lsp/IntelliJ-specific property
   for the analyzer/config root, e.g. an `-Didea.config.path` / dedicated analyzer-storage
   flag if kotlin-lsp exposes one — needs a quick check of kotlin-lsp's CLI/JVM options).
   This co-locates the index with the already per-`ws_hash` system-path, so it inherits
   isolation and any temp-dir sweeping. **Verify** kotlin-lsp honors `XDG_CONFIG_HOME` for the
   analyzer dir before committing.

   **Scout (2026-06-03):** `--system-path` sets `idea.system.path` and does NOT cover the analyzer dir. The analyzer lives at `~/.config/JetBrains/analyzer` — a JetBrains **common-data** root, parallel to (not under) the product config dir `~/.config/JetBrains/kotlin-server2026.2` (governed by `-Didea.paths.selector=kotlin-server2026.2`, set in `product-info.json`'s `additionalJvmArguments`). Because the analyzer dir is NOT under the selector config dir, `-Didea.config.path` is unlikely to relocate it. `XDG_CONFIG_HOME` is therefore the only plausible env lever — and is **still unverified**. Next concrete step: black-box test — launch kotlin-lsp with `XDG_CONFIG_HOME=<tmp>`, trigger indexing on a `.kt` file, observe whether `analyzer/` lands under `<tmp>/JetBrains` or stays in `~/.config`.
2. **Lifecycle cleanup:** on mux idle-timeout shutdown (and on workspace deactivate), remove
   that workspace's analyzer dir, or enforce a size cap (e.g. prune when `> N` GiB). **Caveat (2026-06-03):** codescout cannot locate "that workspace's" analyzer dir from its own `ws_hash` — the analyzer dir uses a distinct 128-bit IntelliJ path-hash (see Evidence + session-log F-1). Targeted per-workspace cleanup would require replicating IntelliJ's hash (fragile, version-coupled). Absent fix #1, cleanup must be **coarse**: sweep `~/.config/JetBrains/analyzer/workspaces/*` only when no kotlin-lsp JVM is live (size- or age-triggered).
3. **Narrow what gets indexed:** ensure the workspace root handed to kotlin-lsp excludes
   `build/`, `.gradle/`, produced `*.jar`, and sibling `.worktrees/` — a 14× index over source
   strongly implies build artifacts / worktree copies are being indexed.
4. **Backstop (ops):** a size-triggered cleanup of `~/.config/JetBrains/analyzer` is the
   symptomatic mitigation if the above can't land quickly.

Add regression tests mirroring the 2026-05-30 issue's `tests` module: assert the kotlin config
sets an analyzer/config-storage env keyed per `ws_hash` (red on current code), and that two
distinct workspace roots produce distinct analyzer storage roots.

## Upstream status & config surface (researched 2026-06-03)

**Version:** `gh api repos/Kotlin/kotlin-lsp/releases/latest` → **`kotlin-lsp/v262.4739.0` (published 2026-04-27)** — identical to the installed build (`kotlin-server-262.4739.0`). **We are on the latest release; there is no newer version to upgrade to.** The bug is present in HEAD.

**This bug is three known-open upstream issues:**
- **[#215](https://github.com/Kotlin/kotlin-lsp/issues/215)** — cache files placed in the wrong directory (OPEN, reported 2026-06-01). Reporter argues it should honor XDG (`~/.cache`/`$XDG_CACHE_HOME`). No maintainer reply, no workaround. → our "analyzer escapes to `~/.config`".
- **[#213](https://github.com/Kotlin/kotlin-lsp/issues/213)** — large projects → excessive cache sizes (OPEN). → our "unbounded growth".
- **[#203](https://github.com/Kotlin/kotlin-lsp/issues/203)** — LSP indexes ALL files in the workspace root, ignoring `contentRoots` (OPEN, on LS-262.4739.0). → our "14× over-index of `build/`/`.worktrees/`".

**Config surface (`intellij.*` namespace; VS Code extension settings, server-side via JVM args / initializationOptions):**
- `intellij.additionalJvmArgs` (array) — "additional JVM arguments to pass to the LSP server process." **This is the supported channel for the `-Duser.home=<dir>` redirect** (validated 2026-06-03: `JAVA_TOOL_OPTIONS=-Duser.home=<scratch>` moved the analyzer index to `<scratch>/.config/JetBrains/analyzer/...`).
- `intellij.buildTool` (gradle|maven|auto), `intellij.jdkForSymbolResolution` (JDK path), `intellij.dev.serverPort`, `intellij.trace.server`.
- **No user-facing setting** for index/cache **storage location**, **indexing scope**, or **directory exclusion** (confirmed via the config docs). The intended scope file `.kotlinlsp-modules.json` (`contentRoots`) exists but is **ignored** per #203.
- Config namespace migrated `kotlinLSP.*` → `intellij.*`; index storage migrated to RocksDB; the index is a *shared* per-workspace folder reused across LS instances (which is why `--system-path`, per-instance, does not govern it).

**Implication:** neither an upgrade nor a kotlin-lsp toggle fixes this. The redirect (`-Duser.home` via `additionalJvmArgs`-equivalent launch arg) is the only relocation lever; scope-narrowing is only reliable codescout-side (choose a root without `build/`/`.worktrees/`). Both are codescout-side workarounds for open upstream bugs — worth a watch-link so we can drop them if #215/#203 land.
## Fix (implemented 2026-06-03 on `experiments`, pending commit/ship)

Two coordinated, **codescout-side** changes (no kotlin-lsp upgrade or toggle exists — see Upstream status):

1. **Redirect the analyzer index off `~/.config`** — `src/lsp/servers/mod.rs`, kotlin branch of `default_config`. Sets `JAVA_TOOL_OPTIONS=-Duser.home=<cache>/codescout/kotlin-lsp-home/<ws_hash>` (cache root = `dirs::cache_dir()`, falling back to `data_local_dir` then `temp_dir`), keyed by the same `ws_hash` as `--system-path`. The analyzer then writes to `<that home>/.config/JetBrains/analyzer/...` — a codescout-owned XDG-cache location (what upstream #215 asks for), off the snapshotted `/home`'s `.config` and not tmpfs. `$HOME` env is left **real**, so JDK/Gradle/toolchain detection is unaffected (the JVM reads only the `user.home` *property*). Inherited `JAVA_TOOL_OPTIONS` is preserved (ours appended last → wins on duplicate `-D`). New helpers: `kotlin_lsp_home_root`, `kotlin_analyzer_home`, `is_codescout_kotlin_home`.

2. **Reclaim on mux shutdown** — `src/lsp/mux/process.rs`, `run()` after `event_loop`. Kills the LSP child, then `reclaim_kotlin_analyzer_home(server_env)`: parses the last `-Duser.home=<dir>` out of `JAVA_TOOL_OPTIONS` and `remove_dir_all`s it **iff** `is_codescout_kotlin_home(<dir>)` (guarded — only under `<cache>/codescout/kotlin-lsp-home/`, never a real home). Because the redirect makes codescout *own* the path, the sweep is precise — no need to replicate kotlin-lsp's 128-bit hash (the dead end that killed the original targeted-cleanup idea; see session-log F-1).

Net: per-session footprint is bounded (swept when the per-workspace mux idles out at its 300 s timeout), the index leaves the user's real `~/.config`, and the churning RocksDB store stops accumulating across snapshots (idle snapshots capture an empty home).

**Verification (2026-06-03):**
- **Unit** — 6 tests: 3 in `lsp::servers::tests` (redirect is per-workspace; guard rejects real/foreign paths) + 3 in `lsp::mux::process::kotlin_home_tests` (env parse, last-`-Duser.home`-wins, guard). `cargo test --lib kotlin` → 24 pass.
- **Component** — black-box LSP probe: `-Duser.home` redirects the index AND `hover` still infers `val x: Int` (JDK + stdlib resolve under the redirect).
- **Integration** — real release `codescout mux` on a minimal kotlin project, `--idle-timeout 2` → log `reclaimed kotlin-lsp analyzer home <dir>`; the seeded home is **gone** after idle-shutdown.
- `cargo clippy --all-targets -- -D warnings` clean. Full lib suite: 2603 pass; 1 **pre-existing, unrelated** failure (`get_guide` description 302 > 300 cap — tracked in `docs/issues/2026-06-03-get-guide-description-over-budget.md`).

**Committed on `experiments` 2026-06-03** (status flipped to `fixed`). **Pending:** ship to master via Standard Ship Sequence + frog audit. On master ship → `git mv` to `docs/issues/archive/` and cite the **master-side** SHA here. Watch upstream #215/#203 — drop the workaround if they land.

**Operational note:** kotlin muxes spawned by the OLD binary (e.g. the live `backend-kotlin` mux observed this session) keep writing to `~/.config/JetBrains/analyzer` until a `/mcp` restart loads the new binary; that pre-existing index is swept manually once its mux exits.
## Workarounds
- Periodically `rm -rf ~/.config/JetBrains/analyzer/workspaces/*` when no kotlin-lsp is running
  (`pgrep -af kotlin-lsp` empty; verify 0 open fds first). Regenerates on next activation.
- Minimize concurrently-active Kotlin worktrees — each adds its own multi-GiB analyzer index
  (in addition to the ~2 GiB JVM noted in the related RAM issue).

## Resume

**Fixed 2026-06-03 on `experiments`** — see ## Fix (redirect via `-Duser.home` into a codescout-owned cache HOME + mux idle-shutdown sweep; unit + component + integration verified, clippy clean, full lib suite green). **Not yet on master.**

Next: ship via Standard Ship Sequence + frog audit (do NOT push master without it), then `git mv` this file to `docs/issues/archive/` and cite the **master-side** SHA in ## Fix. Until then it stays here with `status: fixed`. Watch upstream #215/#203 — drop the workaround if they land. Pre-existing OLD-binary muxes keep filling `~/.config/JetBrains/analyzer` until a `/mcp` restart; sweep that index manually once its mux exits.
## References
- Related: `docs/issues/2026-05-30-cross-worktree-kotlin-jvm-shared-system-path.md` (RAM/system-path; same probe project & subsystem)
- Related: `docs/issues/2026-05-30-shared-server-global-active-project-race.md`
- `src/lsp/servers/mod.rs` kotlin branch (`default_config`, ~L59–90): `--system-path`,
  `GRADLE_USER_HOME`, `idle_timeout_secs=300` — no `XDG_CONFIG_HOME` / analyzer-storage override
- `src/lsp/mux/mod.rs:14,20` (per-path `workspace_hash` keying)
- `docs/manual/src/concepts/kotlin-lsp-multiplexer.md` (§ Gradle Isolation)
- Probe & measurements captured 2026-06-01 on host with 196 GiB Btrfs `/`.

- Upstream (kotlin-lsp, all OPEN as of 2026-06-03): [#215 cache wrong dir](https://github.com/Kotlin/kotlin-lsp/issues/215), [#213 excessive cache size](https://github.com/Kotlin/kotlin-lsp/issues/213), [#203 indexes whole workspace root / ignores contentRoots](https://github.com/Kotlin/kotlin-lsp/issues/203).
- Latest release: [kotlin-lsp/v262.4739.0](https://github.com/Kotlin/kotlin-lsp/releases) (2026-04-27) — installed build; no newer version.
- Config reference: [DeepWiki Configuration](https://deepwiki.com/Kotlin/kotlin-lsp/2.3-configuration); [kotlinlang.org Kotlin LSP](https://kotlinlang.org/docs/kotlin-lsp.html).
