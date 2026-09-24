---
id: '7f97ee751bf72bc4'
kind: bug
status: open
title: 'RESIDUAL: Run the MCP smoke scripts once (their get_symbols_overview call was already removed in 9406f3c4)'
tags:
- cluster/accepted-parameter-silently-dropped
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Fix the MCP smoke script's get_symbols_overview call (nonexistent tool) and run the smoke scripts once.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md` (status `fixed`):

> the symbols-parameter half is fixed but unrun; the get_symbols_overview half is not fixed at all

## Fix

**Half of this was already done before this file was opened.** Re-checked 2026-09-24 against HEAD `ea972b40`: `9406f3c4` (2026-09-11, `fix(tests): repair stale tool names/params in the MCP smoke scripts, add a static gate`, on origin) removed `get_symbols_overview` from both `tests/mcp-smoke-rust.sh` and `tests/mcp-smoke-kotlin.sh` (0 occurrences in each), and added `tests/mcp_smoke_scripts_reference_real_tools.rs` to keep it out. The parent caveat quoted above was stale when this residual was split out on the same day: the splitting session (09093108) copied it without the re-check this file tells readers to do.

**What remains:** run both smoke scripts once against a live server and record the result. That is the only unverified half. It needs a live LSP (rust-analyzer; a Kotlin LSP for the second script), so it is a manual run, not a code change.

## References

- `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md` — parent
