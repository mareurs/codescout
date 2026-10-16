---
id: '9490a30dbffb0fc9'
kind: bug
status: fixed
title: 'BUG: the claude-plugins Pi extension and the local contrib/pi/mcp.json still name the retired read_markdown and edit_markdown'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: Pi integration drift after the tool-surface collapse
closed: 2026-10-16
unverified: 'Fix commit claude-plugins:f17afad96f0585e0cb80a40c413d6a0b4e2d5101 (patch-id a84e2afad3c1448893261965966c8d0ee5e5225e) is only on that repository branch effort-routing-r1, not cherry-picked to the shipping branch or pushed. No live Pi session was run.'
opened: 2026-10-14
owner: marius
severity: medium
---

# BUG: the claude-plugins Pi extension and the local contrib/pi/mcp.json still name the retired read_markdown and edit_markdown

## Summary

The 2026-10-13 fix removed the retired names from `contrib/pi/` in this repository. Two other copies still carry them: the extension in the separate `claude-plugins` repository, and the local gitignored `contrib/pi/mcp.json`. The first keeps the native-`edit` block from arming wherever it is installed. The second tells `pi-mcp-adapter` to expose tools that do not exist.

## Symptom (Effect)

Measured 2026-10-14:

```
claude-plugins/codescout-companion/.pi/extensions/codescout-mode.ts:3   const EDIT_TOOLS = ["codescout_edit_code", "codescout_edit_file", "codescout_edit_markdown"];
claude-plugins/codescout-companion/.pi/extensions/codescout-mode.ts:53  ... edit_code, edit_file, or edit_markdown instead of native edit.
claude-plugins/codescout-companion/.pi/extensions/codescout-mode.test.mjs:10  'codescout_edit_markdown',
contrib/pi/mcp.json:15  "read_markdown",
contrib/pi/mcp.json:18  "edit_markdown",
```

Not observed in a live Pi session. By the code, as in the fixed bug, a block that requires every name in `EDIT_TOOLS` cannot arm when one of them is absent.

## Reproduction

```
grep -n -E 'read_markdown|edit_markdown' ../claude-plugins/codescout-companion/.pi/extensions/codescout-mode.ts contrib/pi/mcp.json
```

`claude-plugins` is at `eb90bd3` and has other sessions' uncommitted changes under `buddy/`. `git status --short -- codescout-companion/.pi` printed nothing on 2026-10-14, so the files named above are unmodified there.

## Environment

Linux, `experiments` at `388416f2`. `contrib/pi/mcp.json` is matched by `.gitignore:137` and was last written 2026-07-15.

## Root cause

`edit_markdown` and `read_markdown` were folded into `edit_file` and `read_file` on 2026-09-03 (`af974c0a`) with no alias. The in-repo copies were fixed in `3fb612b9`. A copy in another repository and a gitignored local file are outside that commit, so no test in this repository reaches them. Measured 2026-10-14 by the grep above.

## Evidence

The grep output above. The companion hooks `il4-deny-hook.sh` and `pre-tool-guard.sh` in the 1.12.2 plugin cache were reported to name `read_markdown` too; this file did not read them.

## Hypotheses tried

None.

## Fix

Done 2026-10-14, in two places, none yet pushed or released:

- `claude-plugins:f17afad96f0585e0cb80a40c413d6a0b4e2d5101` (patch-id `a84e2afad3c1448893261965966c8d0ee5e5225e`), on the branch `effort-routing-r1` of that repository. `codescout-mode.ts` now requires `codescout_edit_code` and `codescout_edit_file` only, and its refusal names those two. `codescout-mode.test.mjs` uses the post-collapse tool set and asserts that the block arms and that the refusal names no markdown tool: red before (native `edit` stayed active), green after under `node` 26. The same commit drops `codescout_read_markdown` from `SKILL_READ_TOOLS` in `harnesses/pi/extensions/codescout-companion.ts`, and both retired names from the `directTools` examples in `harnesses/pi/README.md`.
- The local gitignored `contrib/pi/mcp.json` no longer lists `read_markdown` and `edit_markdown`; the file still parses and `grep` finds neither name.

The companion hooks in the `claude-plugins` working tree do not match the retired names as live tools: `worktree-write-guard.test.sh` names `edit_markdown` on purpose, as a stale-name sentinel. The `il4-deny-hook.sh` and `pre-tool-guard.sh` files in the 1.12.2 plugin cache were not read.


**Re-verified 2026-10-16.** `grep -rn -E 'read_markdown|edit_markdown'` over `contrib/pi/` (in this repository and in the home checkout, including the gitignored `mcp.json`) prints nothing. In `claude-plugins` the only hits are two comment lines in `codescout-mode.test.mjs` (28-29) that explain the removal. `contrib/pi/mcp.json.example` and the local `mcp.json` list `grep, symbols, symbol_at, tree, semantic_search, references, read_file, edit_code, edit_file, create_file, run_command`; each is a `Tool::name()` in `src/tools/` (`edit_file/mod.rs`, `symbol/edit_code.rs`, `read_file.rs`, `create_file.rs`, ...). `node --test codescout-mode.test.mjs` in `claude-plugins/codescout-companion/.pi/extensions` passes (1 of 1). The installed Pi extensions in `~/.pi/agent/extensions/` are symlinks into the `claude-plugins` working tree, which has `f17afad` checked out, so the installed copy carries the fix. `git show f17afad | git patch-id --stable` gives `a84e2afad3c1448893261965966c8d0ee5e5225e`, as recorded.

Not done, because it needs a commit in another repository: moving `f17afad` from `effort-routing-r1` to the shipping branch and pushing it. This record stays in `docs/issues/` until then; archive it with the SHA and patch-id above.

## Tests added

In `claude-plugins`: `codescout-mode.test.mjs` asserts the native-`edit` block arms with the post-collapse tool set and that the refusal names no markdown tool (red before, green after, in `f17afad`). Nothing in this repository can reach those two files, so no test is added here. Re-run 2026-10-16: passes.

## Workarounds

Edit the local `mcp.json` by hand.

## Resume

The commit is on `effort-routing-r1`, another work stream's branch, because that is the branch checked out in `claude-plugins`. Move it to the branch that ships (`git cherry-pick f17afad` there), push it, and check that the installed extension is the symlink to the repository copy. Then archive this bug with the SHA and patch-id above.

## References

- `docs/issues/archive/2026-10-13-pi-integration-names-the-retired-read-and-edit-markdown-tools-so-its-native-edit-block-cannot-arm.md`
- `contrib/pi/codescout-mode.ts` (the fixed in-repo copy)
