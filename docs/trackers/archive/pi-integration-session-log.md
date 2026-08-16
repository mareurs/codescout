---
status: archived
---
# Session Log — pi-integration

> Two-sided observation log for the codescout<->Pi integration work stream.
> Frictions (F-N) and wins (W-N) captured during reconnaissance so future
> sessions inherit the lesson. Append above the template marker; update the
> Index. Status vocabulary: see `docs/templates/session-log.md`.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-06-20 | med | plan-prose | fixed-verified | codescout `grep` directTool collides with Pi built-in `grep`; setActiveTools rejects on bad input |
| F-2 | 2026-06-20 | high | release-pipeline | fixed-verified | Pi mcp.json `command` must be an absolute path — codescout not on PATH |
| F-3 | 2026-07-15 | high | plan-prose | fixed-verified | pi-mcp-adapter's default `toolPrefix: "server"` silently defeated codescout-mode's `has()` guard; native edit/write/read/bash never touched |
| F-4 | 2026-07-15 | high | plan-prose | fixed-verified | `grep` recursive-flag detection false-positived on ordinary paths/patterns containing "r" |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| _none yet_ | | | | | |

---

## F-1 — codescout `grep` directTool collides with Pi's built-in `grep`; setActiveTools rejects on bad input

**Observed:** 2026-06-20, pre-execution reconnaissance of the codescout<->Pi integration plan (`docs/superpowers/plans/2026-06-19-codescout-pi-integration.md`), before any install/setup ran.

**When:** Scouting Pi's extension API + tool registry against the cloned source (`../pi`) to confirm the plan's `mcp.json` directTools and `codescout-mode.ts` API calls were real.

**Expected (plan):** codescout's hot-set — incl. `grep` — registers as first-class Pi directTools under bare MCP names with no collisions; `pi.setActiveTools([...])` is a safe fire-and-forget.

**Got (scouted reality):**
- Pi's tool registry contains built-in `grep`/`find`/`ls` (`packages/coding-agent/CHANGELOG.md:3361` — "Tool registry now contains all built-in tools (read, bash, edit, write, grep, find, ls)…"). codescout's `grep` directTool therefore collides by name — `has("grep")` / `setActiveTools` resolution is ambiguous (codescout's vs Pi's). `grep` is the ONLY hot-set name that collides (symbols/symbol_at/tree/semantic_search/references/read_file/read_markdown/edit_* are distinct from read/write/edit/bash/grep/find/ls).
- `setActiveTools` is async and REJECTS with `invalid_argument` on unknown OR duplicate tool names (`packages/agent/src/harness/agent-harness.ts:941`; `packages/agent/test/harness/agent-harness.test.ts:498-501`). The plan's extension called it fire-and-forget.

**Probable cause:** Plan written from `extensions.md` docs prose + Claude Code's `mcp__codescout__`-prefixed tool names; did not scout Pi's own built-in tool registry or `setActiveTools` failure modes.

**Workaround / fix (landed this session, pre-execution):**
- Dropped `grep` from `directTools` (mcp.json) and `CODESCOUT_HOT_SET` (extension). codescout's `grep` stays reachable via the `mcp` proxy. To keep it first-class, use the adapter's server-wide `toolPrefix` (renames all codescout tools `cs_*`) — documented as the contingency.
- Wrapped the `setActiveTools` call in `await` + `try/catch` so a stale/ambiguous name degrades to "native tools kept" instead of an unhandled rejection.

**Severity:** med — would have caused an ambiguous/failed tool registration or a `setActiveTools` rejection at `session_start`, silently defeating curation (Pi's native `edit` would stay active) with no error surfaced to the user.

**Status:** fixed-verified — plan corrected before any execution (directTools/hot-set drop + try/catch landed in `2026-06-19-codescout-pi-integration.md`, this session). Behavioral confirmation deferred to the plan's Task 7 dogfood.

**Fix idea / Pointer:** plan Task 4 (mcp.json) + Task 5 (extension), this session. Reconnaissance hit.

---

## F-2 — Pi mcp.json `command` must be an absolute path; `codescout` is not on PATH

**Observed:** 2026-06-20, executing-plans Task 1 (preflight) of the codescout<->Pi integration.

**When:** Verifying `codescout` resolves before installing Pi.

**Expected (plan):** `mcp.json` uses `"command": "codescout"`, resolved via PATH for the Pi-spawned adapter.

**Got (reality):** `command -v codescout` is empty and `codescout --help` -> "command not found" in both the sandbox shell and `bash -lc`. The symlink `~/.cargo/bin/codescout` EXISTS (-> `target/release/codescout`, a fresh 39MB binary) but `~/.cargo/bin` is NOT on PATH (PATH carries `/usr/lib/rustup/bin`, not `~/.cargo/bin`). Claude Code works only because `~/.claude.json` launches codescout by absolute path. Invoked absolutely (`/home/marius/.cargo/bin/codescout --help`) the binary runs and shows `start`.

**Probable cause:** Plan assumed `~/.cargo/bin` on PATH; this machine uses rustup shims and the codescout symlink dir is not on PATH.

**Workaround / fix (landed this session):** `mcp.json` uses the absolute path `"command": "/home/marius/.cargo/bin/codescout"` (rebuild-safe symlink). README notes the path is machine-specific.

**Severity:** high — bare `"command": "codescout"` fails at adapter-spawn time; codescout never connects and (lazy spawn) the integration is silently dead until the first tool call errors. Caught at preflight, before any install.

**Status:** fixed-verified — plan + `contrib/pi/mcp.json` use the absolute path this session. Connection itself confirmed at the Task 7 dogfood.

**Fix idea / Pointer:** plan Task 4 (mcp.json) + Global Constraints, this session. Preflight/recon hit.

---
## F-3 — pi-mcp-adapter's default `toolPrefix: "server"` silently defeated codescout-mode's `has()` guard; native edit/write/read/bash were never actually touched

**Observed:** 2026-07-15, revisiting the integration to extend it from edit-only curation to a full read/write/edit/bash replacement.

**When:** Auditing why the live tool list (inside an active Pi session using this integration) still showed native `edit` alongside `codescout_edit_file` etc., contradicting AGENTS.md's claim that native `edit` is disabled.

**Expected (design):** `codescout-mode.ts`'s `session_start` guard (`has(\"edit_code\") || has(\"symbols\")`) checks the directTools names as registered, matching the bare names listed in `mcp.json`'s `directTools` and the `CODESCOUT_HOT_SET` array — so the guard passes and native `edit` gets dropped via `setActiveTools`.

**Got (reality):** `pi-mcp-adapter` (`direct-tools.ts`, confirmed at v2.11.0) computes `const prefix = config.settings?.toolPrefix ?? \"server\"` — the default is `\"server\"`, not `\"none\"`. Every codescout direct tool therefore registers as `codescout_<name>` (`codescout_edit_code`, `codescout_symbols`, …), never as the bare name. `has(\"edit_code\")` and `has(\"symbols\")` in the deployed extension always evaluated `false`, so the `session_start` handler returned early on every single session. Net effect: `setActiveTools` was never called, the \"codescout tools active\" notify never fired, and native `edit` (plus `read`/`write`/`bash`, which were never curated to begin with) stayed fully active the entire time this integration has been \"live\". This was invisible — no error, no crash, just silent no-op.

**Probable cause:** `mcp.json`'s `directTools` example block in the original design doc (§5.2) predates whatever `pi-mcp-adapter` version/behavior introduced the `\"server\"` default; the guard was written against the design doc's assumed bare names and never re-verified against the actually-installed adapter version's behavior.

**Workaround / fix (this session):** Rewrote `codescout-mode.ts` to check the `codescout_`-prefixed names (`EDIT_TOOLS`/`WRITE_TOOL`/`READ_TOOL` constants). Also widened scope per user request: native `write` now drops alongside `edit` (guarded on `codescout_create_file`, new in codescout 0.15.0 and not available at original design time); native `read` and `bash` are now hard-blocked via a `tool_call` handler (`{block: true, reason}`) instead of soft-curated/nudged — `read` allowed only for image extensions (codescout can't view images), `bash` allowed except for source-search/dump patterns already redundant with codescout's read/search tools, with a `# codescout-override` escape hatch.

**Severity:** high — the entire curate-and-substitute mechanism (the core of this integration) had been a complete no-op since at least the last `pi-mcp-adapter` upgrade; AGENTS.md's tool-map guidance was actively wrong about what was blocked.

**Status:** fixed-verified — code changed this session (`codescout-mode.ts`, `mcp.json`/`mcp.json.example` directTools, `AGENTS.md`). Behaviorally confirmed live across two reloads: native `edit`/`write` return "Tool not found" (fully removed from the active set); native `read` blocks on a text file and allows an image (`red-circle.png` rendered); native `bash` blocks `cat <source-file>`/`rg <pattern>`/`find ... -name` with the codescout-equivalent reason, allows `echo`, and allows the same blocked `cat` command through when `# codescout-override` is appended. No side effect from the blocked `write` attempt (target file never created). A grep false-positive found during this verification is tracked separately as F-4.

**Fix idea / Pointer:** `contrib/pi/codescout-mode.ts` (rewrite), `contrib/pi/mcp.json`/`mcp.json.example` (added `create_file`, `run_command` to `directTools`), `contrib/pi/AGENTS.md` (tool-map rewrite), this session.

---
## F-4 — `grep` recursive-flag detection false-positived on ordinary paths/patterns containing "r"

**Observed:** 2026-07-15, live-testing F-3's fix immediately after the first reload.

**When:** Running `grep -n "F-3" docs/trackers/pi-integration-session-log.md` (a plain, non-recursive grep) to check whether the F-3 heading had landed correctly.

**Expected:** Only `grep -r`/`-R`/`--recursive`-style recursive/directory greps should be blocked; a single-file, non-recursive `grep -n pattern file` should pass through untouched.

**Got:** Blocked. Root cause: `isRedundantBashCommand`'s grep detection was `grep\s+[^|]*-[a-zA-Z]*r` — a single regex intended to catch a `-r` flag, but `[^|]*` greedily matches almost anything up to the next pipe, so the "`-` then letters then `r`" tail can be satisfied by an unrelated substring anywhere later in the command. The actual match here was `-integ` + `r` inside the path segment `pi-integration-session-log.md`. This regex was carried over unmodified from the pre-existing "nudge" version of `codescout-mode.ts` (see design doc, superseded by F-3's rewrite) — as a soft nudge a false positive was cosmetic; hardened into a `tool_call` block (this session's change), the same false positive became a hard failure, blocking a legitimate command. Confirmed in isolation with an inline Node repro before touching the file, then again with a 14-case regression table (recursive/regex greps, plain greps, cat/rg/ag/find-name true positives, override marker, and non-redundant commands) — all 14 passed after the fix.

**Workaround / fix (this session):** Replaced the single-regex grep check with `isRecursiveGrep()` — splits the command on `|`, requires `grep` to be the literal first token of a pipeline segment, then checks each subsequent whitespace-delimited token against `-r`/`-R`/`--recursive` (exact) or a combined short-flag cluster (`^-[a-zA-Z]+$` containing `r`/`R`). Also widened `find ... -name` detection to allow multiple tokens between `find` and `-name` (was previously under-detecting `find . -type f -name "*.rs"`, a distinct minor gap noticed while fixing the grep issue).

**Severity:** high — false positive blocked a legitimate, common command shape (`grep <flags> pattern file`) with no way to tell true/false positives apart except the override marker; would have been a persistent daily-driver annoyance if shipped as-is.

**Status:** fixed-verified — re-tested live after a second reload: the exact failing command now passes, and the full regression set (true-positive blocks, override marker, non-redundant passthrough) still holds.

**Fix idea / Pointer:** `contrib/pi/codescout-mode.ts` (`isRecursiveGrep`, widened `FIND_NAME`), this session.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via
     edit_markdown(action="insert_before", heading="## Template for new entries", ...)
     and update the Index / Wins Index tables. Status vocabulary: docs/templates/session-log.md -->
