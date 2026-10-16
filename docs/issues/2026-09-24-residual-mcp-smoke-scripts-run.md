---
id: '7f97ee751bf72bc4'
kind: bug
status: fixed
title: 'RESIDUAL: Run the MCP smoke scripts once (their get_symbols_overview call was already removed in 9406f3c4)'
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md
severity: low
unverified: 'TRACKED 5697348ca3d72700 — tests/mcp-smoke-kotlin.sh was NOT run: the project it targets no longer has the files it asserts on. Its helper-level repairs are applied but unrun.'
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

## Resolution (2026-10-16)

**Rust script run, found 18 of 26 steps red, repaired, re-run green. Kotlin script not run.**

Run: `git archive HEAD` copy of the worktree in a scratch dir (never the home checkout), isolated `HOME` (so `mcp alias` wrote a scratch `~/.mcpt/aliases.json`), binary built with `scripts/with-slot.sh cargo build --release` and copied out of the leased slot, then `./tests/mcp-smoke-rust.sh`. First run, before any edit: `8/26 passed (18 failed) in 27 steps`.

At the bytes, the SERVER was right and the SCRIPT stale, in four ways (each reproduced with a hand `mcp call` against the same binary):

1. `mcp call` prints plain text, never JSON. `read_file` of a 1-line file prints `1 line`, a blank line, `test`. `assert_json_has "content"` (5 steps) and `assert_symbols_found` (6 steps) parse `json.load(sys.stdin)` and could never pass: 11 of the 18 failures.
2. `read_file` no longer refuses source files. `debug_enforce_symbol_tools` was retired (`stale_debug_enforce_symbol_tools_key_is_ignored_not_rejected`, src/config/project.rs; `is_source_path` now gates only multi-line `edit_file`). A large file comes back as `Outline only`, a short one whole. The 5 `read_file blocks .rs/.py/.ts/.go/.sh` steps asserted a refusal that does not exist.
3. `symbols` overview of `src/server.rs` is capped, so `from_parts` and `call_tool` (children of `impl CodeScoutServer`) are not in the default reply (2 steps). `depth=0` lists top-level symbols; `symbols name=CodeScoutServer/from_parts` finds the method.
4. Every `mcp call` starts a fresh server, so every reply carried the injected `get_guide` body after an `<!-- auto-injected` marker. That body contains `symbols(name=`, `index` and `error`, so `blocked error includes symbol tool hints` and `semantic_search` (whose reply was an empty `{"results": [], "total": 0}`) PASSED on the guide's text, not the tool's.

Fix: `call()` now cuts the reply at the `<!-- auto-injected` marker in both scripts; the JSON helpers are deleted; the refusal steps are replaced by outline / whole-file steps matching the server; `relative_path`/`symbol` payload keys became `path`/`name`; `replace_content` now requires the server's `unknown tool` text (it passed on any non-zero exit, an unreachable server included); `semantic_search` asserts the `results` envelope only (the scratch project has no index, stated in the script).

Result: live `23/23 passed (0 failed)`. Mutation (server binary replaced by `/bin/false`): `0/23 passed`, so no step passes on an unreachable server. New static gate `no_smoke_script_parses_the_reply_as_json` (tests/mcp_smoke_scripts_reference_real_tools.rs) was RED on both scripts before the edits and is green after; the other three tests in that file stay green.

One environmental flake seen once: a cold `symbols name="impl Tool for Symbols/call"` returned `0 matches` with the server's own warning `Mux connection lost — so this 0 is not evidence the symbol is absent`; three further runs found it. Each `mcp call` is a cold LSP start, so that step can flake.

Kotlin: the target project no longer has `src/main/kotlin/.../AuthService.kt`, `build.gradle.kts` or `gradle.properties` at its root, and running it would write `.codescout/` state into the live mirela project. Tracked in `5697348ca3d72700`.

## References

- `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md` — parent
