---
id: '3870c801cf7bdf57'
kind: bug
status: open
title: 'RESIDUAL: Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` (status `mitigated`):

> Only this instance was repaired (codescout-companion bumped to 1.16.8, all three profiles re-synced and verified). Nothing gates the NEXT content edit without a version bump — durable options a/b/c in § Fix are all unshipped — and § Root cause records that remembering failed one commit after succeeding.

## Fix

Re-checked 2026-10-30 in the `claude-plugins` repo (read-only, HEAD `3b8e3a4`): `scripts/pre-push-guard.sh` has Guard 2 only (version change without profile parity); no guard covers a plugin content edit without a bump, and `.git/hooks` there holds only the `pre-push` shim. Not fixed here: the fix lives in `../claude-plugins`, which this session may not edit.

Exact change to make there (parent option b, a push-time form so it rides the existing hook): in `scripts/pre-push-guard.sh`, after Guard 2, add Guard 3. For each plugin directory `<p>/` whose files appear in the `git diff --name-only "$from" "$to"` list already computed at line 49, excluding `docs`, `tests`, `README*` and `CHANGELOG*` paths, require that `<p>/.claude-plugin/plugin.json` ALSO differs in its `"version"` between `$from` and `$to` (`git show "$from:<p>/.claude-plugin/plugin.json" | jq -r .version` vs the same at `$to`); otherwise print the plugin, the first content path, and `./scripts/release.sh <p> <version>` and exit 1. A new test in the repo's guard tests must show a content-only push refused and a content-plus-bump push allowed. Option a (a `content_matches_source` gather field in the `version-bump-checklist` augmentation, `cc8cb9e23ab5cc67`) is the parent's preferred first step and is not in place either. Needs the plugin-repo owner; no codescout change.

## References

- `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` — parent
