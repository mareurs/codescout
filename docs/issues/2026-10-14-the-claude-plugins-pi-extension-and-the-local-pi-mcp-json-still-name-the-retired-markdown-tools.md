---
id: '9490a30dbffb0fc9'
kind: bug
status: open
title: 'BUG: the claude-plugins Pi extension and the local contrib/pi/mcp.json still name the retired read_markdown and edit_markdown'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: Pi integration drift after the tool-surface collapse
closed: ''
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

Not done. Two changes: in `claude-plugins`, replace `codescout_edit_markdown` in `EDIT_TOOLS`, in the refusal text and in `codescout-mode.test.mjs`, then run that repository's own tests. Locally, drop the two names from `contrib/pi/mcp.json`. The `claude-plugins` change belongs in that repository's own commit.

## Tests added

None yet.

## Workarounds

Edit the local `mcp.json` by hand.

## Resume

In `claude-plugins`, change the three lines above and run `codescout-mode.test.mjs` there. Then read `il4-deny-hook.sh` and `pre-tool-guard.sh` for the same names.

## References

- `docs/issues/archive/2026-10-13-pi-integration-names-the-retired-read-and-edit-markdown-tools-so-its-native-edit-block-cannot-arm.md`
- `contrib/pi/codescout-mode.ts` (the fixed in-repo copy)
