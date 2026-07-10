---
status: open
opened: 2026-07-10
closed:
severity: medium
owner: marius
related:
- '2026-06-19-mcp-server-oom-68gb.md'
- '2026-06-19-kotlin-lsp-uncapped-jvm-heap.md'
tags: [memory, oom, ops, blast-radius, cgroup, hardening]
kind: bug
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
Deferred defense-in-depth items (plan first):

1. **cgroup blast-radius cap.** Run codescout MCP servers under a systemd slice
   with `MemoryMax=` and `MemorySwapMax=0` so a runaway is cgroup-OOM-killed in
   isolation instead of taking down the desktop. Decide: ship a sample
   `.slice`/drop-in + document it, and/or set it from codescout's own launcher.
2. **`oom_score_adj` review.** Establish why the server runs with
   `oom_score_adj=200` and whether that is the intended kill-bias; document or
   change it.
3. **Expand the default-ignore set.** Broaden the `[ignored_paths]` defaults
   (already honoured by `sync_project` + `check_index_scope` via
   `build_ignore_matcher`) beyond the current `.venv`/`node_modules`/`target`/…
   set so more dependency trees are excluded from indexing out of the box.
4. **(Optional) soft self-limit.** A periodic RSS self-check that logs + aborts
   cleanly before the host OOMs (the durable heartbeat in `src/heartbeat.rs`
   already provides the RSS sampling seam).

## Tests added
N/A — not yet started.

## Workarounds
Run codescout under a systemd user slice with `MemoryMax=` today, e.g. a
drop-in setting `MemoryMax=16G` / `MemorySwapMax=0` on the unit/scope that
launches the MCP server, so a runaway is contained to that cgroup.

## Resume
Decide delivery for item 1 (sample slice + docs vs. launcher-set cgroup), then
item 2 (`oom_score_adj` review). Items 3–4 are independent and lower priority.
Cross-refs: sibling native-memory watchdog work in
`docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md` (Fix 2), archived root
cause in `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`.

## References
- Archived root-cause ticket: `docs/issues/archive/2026-06-19-mcp-server-oom-68gb.md`
- Sibling LSP native-growth watchdog: `docs/issues/2026-06-19-kotlin-lsp-uncapped-jvm-heap.md`
- Heartbeat RSS sampler: `src/heartbeat.rs`
