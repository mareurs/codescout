---
id: null
kind: bug
status: mitigated
title: null
owners: []
tags:
- memory
- oom
- ops
- blast-radius
- cgroup
- hardening
topic: null
time_scope: null
closed: '2026-07-13'
opened: '2026-07-10'
owner: marius
related:
- '2026-06-19-mcp-server-oom-68gb.md'
- '2026-06-19-kotlin-lsp-uncapped-jvm-heap.md'
- docs/trackers/index-scope-default-ignores.md
severity: medium
---

# BUG: no blast-radius cap — a runaway codescout/LSP process can still OOM-kill the whole host

## Summary
The 68 GB MCP-server OOM root cause is fixed (streaming `sync_project`, see
`docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`), but nothing bounds the
*blast radius* of a **future** runaway allocation. A single codescout or LSP
process that grows unbounded is still able to exhaust host RAM+swap and trigger
the kernel global OOM-killer, taking down the desktop/session rather than just
the offending process. This tracks the deferred defense-in-depth items split out
of that ticket so they aren't lost.

## Symptom (Effect)
Not a live symptom — this is preventative hardening. The observed precedent:
the 2026-06-19 event where the MCP server hit 68.7 GB RSS and the **kernel
global** OOM-killer fired (host `ripper`), killing across the whole machine
instead of isolating the runaway. The codescout server was additionally running
with `oom_score_adj=200`, biasing the kernel toward killing *it* — but the host
still went down.

## Reproduction
N/A — preventative. To validate a fix: run the codescout MCP server under a
cgroup with `MemoryMax=` set, drive a deliberately unbounded allocation (e.g. a
very low `CODESCOUT_INDEX_FLUSH_BATCH` on a huge tree, or a synthetic balloon),
and confirm the process is cgroup-OOM-killed in isolation while the host session
stays responsive.

## Environment
- Platform: linux (systemd), host `ripper`
- Project: codescout (`/home/marius/work/claude/codescout`)
- Relevant: codescout MCP server + spawned LSP/mux subprocesses

## Root cause
N/A (not a code defect). Gap: codescout ships no deployment guidance or
self-limit to cap the memory blast radius of a runaway process, and does not
review/justify the `oom_score_adj=200` it runs with.

## Fix

### Item 2 — `oom_score_adj` review: **DONE. Not codescout's doing, and NOT a bug. Do not "fix" it.**

Measured on the host (2026-07-13):

```
/proc/<codescout>/oom_score_adj        = 200
/proc/<konsole>/oom_score_adj          = 200
/proc/<systemd --user>/oom_score_adj   = 100
/proc/<codescout>/cgroup
  → user.slice/…/app.slice/app-org.kde.konsole-*.scope/tab(*).scope
```

`oom_score_adj` is **inherited across fork/exec**. codescout never sets it — it is spawned by the MCP client, a child of the terminal, and inherits Konsole's `+200`. The desktop session sets `+200` on `app.slice` deliberately so the kernel, under pressure, prefers killing a *user application* over the shell / compositor / session manager.

That bias is **correct and desirable**. Lowering codescout's `oom_score_adj` would bias the kernel toward killing the desktop instead — the opposite of what this bug wants. The 2026-06-19 host outage was caused by the unbounded 68 GB allocation, not by the kill-preference: codescout's own `anon-rss` was by far the largest on the machine.

**No code change. Documented in `contrib/systemd/README.md` so it is not re-investigated.**

### Item 1 — cgroup blast-radius cap: **DRAFTED, opt-in, not installed.**

`contrib/systemd/README.md` ships a sample `codescout.slice` (`MemoryMax` / `MemoryHigh` / `MemorySwapMax=0`) plus the `systemd-run --user --pipe --slice=codescout.slice` wrapper for the MCP client config (`--pipe` is load-bearing — the MCP transport is stdio).

Deliberately **not** installed into live systemd and **not** set from codescout's own launcher: the right `MemoryMax` is host-specific, and silently cgroup-capping a user's editor tooling is not a decision a code change should make for them.

Note the structural reason a cap needs a wrapper at all: codescout inherits the *terminal's* cgroup scope, so it has no unit of its own to attach limits to.

### Item 3 — expand default ignores: **DONE, conservatively.**

`default_ignored_patterns()` (`src/config/project.rs`) grew from 10 entries to 25. Added: `venv`, `site-packages`, `.mypy_cache`, `.pytest_cache`, `.ruff_cache`, `.tox`, `.nox`, `.next`, `.nuxt`, `.svelte-kit`, `.turbo`, `.parcel-cache`, `.gradle`, `.terraform`, `.dart_tool`, `.stack-work`, `.direnv`, `.idea`.

**The selection criterion is strict, and it is the whole point.** These compile with **gitignore semantics** for the code index, so a bare name prunes at *any depth* — `target` also prunes `crates/foo/target`. So a default may only name a directory that **cannot plausibly contain hand-authored source**.

**Rejected despite being proposed** in `docs/trackers/index-scope-default-ignores.md`:
- **`models`** — the tracker lists it, but it is a very common *source* directory (Django / Rails / domain models). At any depth it would silently prune `src/models/`. That is precisely the *"default-on risks silently skipping code a user wants indexed"* failure the tracker itself warns about. Not shipping it.
- **`vendor` / `bin` / `obj` / `out`** — conventionally build output or vendored deps, but each is a legitimate source dir in some ecosystem. Left to per-project `[ignored_paths]`.
- **Binary weights** (`*.pt`, `*.safetensors`, `*.onnx`, `*.gguf`) — already excluded from *embedding* by the `lang_for_ext` allowlist. Listing them here would save walk time but not memory, and this list is directory-oriented.

The rejections are pinned by the regression test, not just by comment — so a future well-meaning edit cannot quietly add `models` and start dropping source from the index.

### Item 4 — soft RSS self-limit: **NOT done.** Still open; `src/heartbeat.rs` already provides the RSS sampling seam. Lower priority than items 1–3 and unchanged by this pass.
## Tests added

`default_ignores_cover_dependency_trees_but_never_plausible_source_dirs` — `src/config/project.rs`.

Asserts both directions, which is the point:
- the new dependency/cache trees ARE ignored by default;
- `models`, `vendor`, `bin`, `obj`, `out`, `src`, `lib` are NOT — a guard against a future edit re-adding a plausible source dir and silently pruning code at any depth;
- no duplicates (the list is unioned with the user's patterns).

Items 1 and 2 are documentation/ops and carry no test — item 2's finding was established empirically against `/proc` on the host, quoted verbatim above.

Full suite: 3204 passed, 0 failed. `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds
Run codescout under a systemd user slice with `MemoryMax=` today, e.g. a
drop-in setting `MemoryMax=16G` / `MemorySwapMax=0` on the unit/scope that
launches the MCP server, so a runaway is contained to that cgroup.

## Resume

Status is `mitigated`, not `fixed`: items 1–3 are addressed, but **item 4 (soft RSS self-limit) remains open**, and item 1 is guidance the operator must opt into rather than an enforced cap. The root cause (unbounded allocation) was already fixed under `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`; what is left here is defence-in-depth.

Next: item 4, if wanted — a periodic RSS self-check that logs and aborts cleanly before the host OOMs, built on the existing `src/heartbeat.rs` sampling seam.

Also still open in `docs/trackers/index-scope-default-ignores.md` and NOT addressed here: the **librarian matcher inconsistency** — the markdown indexer shares the `[ignored_paths]` list but compiles it with plain `globset::Glob`, so bare names do *not* match nested dirs there the way the code index's gitignore matcher does. The defaults expanded here therefore have *narrower* reach in the librarian than in the code index. Worth unifying.
## References
- Archived root-cause ticket: `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`
- Sibling LSP native-growth watchdog: `docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`
- Heartbeat RSS sampler: `src/heartbeat.rs`
