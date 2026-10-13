---
id: '5ed57996270880ed'
kind: bug
status: fixed
title: 'BUG: the Pi integration still names the retired read_markdown and edit_markdown, so its native-edit block can never arm and its tool map points at tools that do not exist'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: Pi integration drift after the tool-surface collapse
closed: 2026-10-13
opened: 2026-10-13
severity: medium
unverified: 'TRACKED 9490a30dbffb0fc9 — the claude-plugins copy of the extension and the local gitignored contrib/pi/mcp.json still name the retired tools. Full gate 2026-10-14: the first run was red on two tests outside this change (prompts::tests::reader_docs_contain_no_retired_call_forms, 46 hits, every one under the untracked .codescout/resident-private/codex-pilot-fix-20261013/commit-snapshot/; server::guide_hint_tests::native_context_wire_reaches_interactive_shell, open bug 098fca009ecbdd78). The second run, after only comment re-pointing, passed every lane: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0. Why the reader-docs hits did not recur was not established. The fix''s own test server::tests::pi_integration_references_only_real_tools passed in both gate lanes. Not observed in a live Pi session.'
---

# BUG: the Pi integration still names the retired `read_markdown` and `edit_markdown`, so its native-`edit` block can never arm and its tool map points at tools that do not exist

## Summary

`edit_markdown` and `read_markdown` were folded into `edit_file` and `read_file` on 2026-09-03 (`af974c0a`), with no alias. Nothing under `contrib/pi/` was updated. `codescout-mode.ts` still requires `codescout_edit_markdown` among the tools that must all be present before it blocks native `edit`. If that tool is absent, `hasAll(EDIT_TOOLS)` is false and the block never arms. `mcp.json.example`, the local `mcp.json` and `AGENTS.md` still list or recommend the retired names. A copy of the extension in the separate `claude-plugins` repository has the same line.

Reported 2026-10-13 by a user auditing `experiments` at `18a007aa`. Re-verified at `5ae71b35`.

## Symptom (Effect)

By the code: on a binary that has the 2026-09-03 collapse, Pi keeps its native `edit` and `write`-adjacent behaviour where the extension promised to replace it, and `AGENTS.md` tells the model to call `read_markdown` and `edit_markdown`, which return the MCP unknown-tool error (`CHANGELOG.md` "No aliases"). Not observed in a Pi session; see `unverified:`.

## Reproduction

Static, from the checkout:

1. `grep -n "edit_markdown\|read_markdown" contrib/pi/*` lists the lines under Evidence.
2. `grep -n "edit_markdown" src/tools/mod.rs` finds no registered tool of that name (the fold-in commit is `af974c0a`).

A live reproduction needs Pi with `pi-mcp-adapter` against a post-collapse binary and was not run.

## Environment

`experiments` at `5ae71b35`. Pi with `pi-mcp-adapter` (prefix `codescout_`). The extension is symlinked to `~/.pi/agent/extensions/codescout-mode.ts` per its own header.

## Root cause

- `contrib/pi/codescout-mode.ts:27`: `const EDIT_TOOLS = ["codescout_edit_code", "codescout_edit_file", "codescout_edit_markdown"];`
- `contrib/pi/codescout-mode.ts:71-72`: `has` tests `pi.getAllTools()` for a name; `hasAll` is `names.every(has)`.
- `contrib/pi/codescout-mode.ts:77` and `:103`: both the session-start drop and the per-call block of native `edit` are gated on `hasAll(EDIT_TOOLS)`.
- `contrib/pi/codescout-mode.ts:106` and `:121`: the refusal texts tell the model to use `edit_markdown` and `read_markdown`.
- `contrib/pi/mcp.json.example:15,18` lists both retired names in `directTools`. The untracked local `contrib/pi/mcp.json` lists them too; the extension's comment says its list "Must match" that file.
- `contrib/pi/AGENTS.md:11,27,31` map markdown reading and editing to the two retired tools.
- `CHANGELOG.md` (Unreleased, "tool surface collapse"): the six retired names return the MCP unknown-tool error; there is no shim.
- Last change to `contrib/pi/codescout-mode.ts` was `e58ab1f0` (2026-07-19), before the collapse.

Which of two things happens when a configured name is missing depends on `pi-mcp-adapter`, which was not read: if it registers only tools the server exposes, `hasAll` is false and the block is inert; if it registers every configured name, the model sees a tool that fails when called. Both are broken. Inferred from the code, not measured.

## Evidence

Lines above, read 2026-10-13. A second copy: `../claude-plugins/codescout-companion/.pi/extensions/codescout-mode.ts:3` (the same `EDIT_TOOLS`) and `:53` (the refusal text), and `codescout-mode.test.mjs:10` (the fixture list that makes its test pass with the stale name).

## Hypotheses tried

1. **Hypothesis:** the extension filters unknown names before `hasAll`. **Test:** read `codescout-mode.ts:70-72` and `:77`. **Verdict:** rejected; `has` only tests presence in `pi.getAllTools()`. **Evidence:** Root cause.

## Fix

Implemented 2026-10-13 and committed. Fix commit: `3fb612b9` on `experiments` (experiments only), patch-id `b2b2d7bb9849ea740821942f91193b33d6404b8a` (`git show 3fb612b9 | git patch-id --stable`; one commit that also fixes the two sibling bugs).

1. `contrib/pi/codescout-mode.ts`: `EDIT_TOOLS` is now `["codescout_edit_code", "codescout_edit_file"]`, so `hasAll` can be true on a post-collapse binary and the native `edit` block can arm. The two refusal texts name `read_file` / `edit_file` and say they address Markdown by heading.
2. `contrib/pi/mcp.json.example`: `read_markdown` and `edit_markdown` removed from `directTools`.
3. `contrib/pi/AGENTS.md`: the Markdown lines now describe `read_file` (heading map, `heading`) and `edit_file` (`heading` + `action`); `edit_markdown` is gone from the `approve_write` sentence.
4. The drift guard, `server::tests::pi_integration_references_only_real_tools`: reads the registered tool names and checks the three files. Its allowlist holds names that are not codescout tools (Pi's `bash`, `edit`, `read`, `write`, `tool_call`; `researcher_research_run`; shell utilities) and codescout parameters (`action`, `heading`, `include_body`).

NOT done: the untracked local `contrib/pi/mcp.json` (git ignores it; its owner must remove the two names by hand), and the copy of the extension in `../claude-plugins/codescout-companion/.pi/extensions/` with its test fixture. That is another repository and needs its own change.

## Fix provenance

- **SHA:** `3fb612b96d5c4ad4fcd192a7c889c4a36c85ea88` (`experiments`; one commit that also fixes the two sibling bugs of 2026-10-13)
- **patch-id:** `b2b2d7bb9849ea740821942f91193b33d6404b8a` (`git show 3fb612b9 | git patch-id --stable`, re-observed 2026-10-14)

## Tests added

`server::tests::pi_integration_references_only_real_tools`. Red before the fix with exactly the retired names: `read_markdown` and `edit_markdown` in `mcp.json.example`, `"codescout_edit_markdown"` and both refusal texts in `codescout-mode.ts`, and four mentions in `AGENTS.md` (plus two false positives, `tool_call` and `include_body`, which went on the allowlist). Green after the fix. Measured 2026-10-13: `cargo test --lib server::tests::pi_integration_references_only_real_tools`.

Not covered: the live behaviour in Pi. Which symptom was live (an inert block, or a model calling a missing tool) was not established, because `pi-mcp-adapter` was not read.

## Workarounds

Edit the local `~/.pi` copy and `mcp.json` by hand to remove the two names.

## Resume

Complete for this repository. Fixed in `3fb612b9`, public on `origin/experiments`. The `claude-plugins` copy of the extension and the local gitignored `contrib/pi/mcp.json` still name the retired tools; that work is tracked in `docs/issues/2026-10-14-the-claude-plugins-pi-extension-and-the-local-pi-mcp-json-still-name-the-retired-markdown-tools.md`. `contrib/pi/mcp.json` is matched by `.gitignore:137`; it is not merely untracked.

## References

- Commit `af974c0a` (2026-09-03), `CHANGELOG.md` Unreleased
- `contrib/pi/codescout-mode.ts`, `contrib/pi/mcp.json.example`, `contrib/pi/AGENTS.md`
- `../claude-plugins/codescout-companion/.pi/extensions/codescout-mode.ts`
- Companion plugin hooks `il4-deny-hook.sh` and `pre-tool-guard.sh` in the 1.12.2 cache name `read_markdown` too (reported, not read here).
