---
id: '3870c801cf7bdf57'
kind: bug
status: fixed
title: 'RESIDUAL: Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump'
tags:
- cluster/unclassified
closed: 2026-10-31
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md
severity: low
unverified: The guard was driven by the test with synthetic stdin and on real ranges. It has not been seen refusing a real `git push`, and the commit is local on claude-plugins `main`, not pushed. Until it is pushed it protects only the clones that hold the commit.
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Ship one of the durable options (a/b/c) that gates a codescout-companion content edit made without a plugin version bump.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` (status `mitigated`):

> Only this instance was repaired (codescout-companion bumped to 1.16.8, all three profiles re-synced and verified). Nothing gates the NEXT content edit without a version bump — durable options a/b/c in § Fix are all unshipped — and § Root cause records that remembering failed one commit after succeeding.

## Fix

Shipped 2026-10-31 as option b of the parent record, in the push-time form, in `claude-plugins` commit `32ce0ff` (local on `main`, not pushed). `scripts/pre-push-guard.sh` has a Guard 3: when a push to `main` changes a plugin's content and that plugin's `version` is the same at both tips, the push is refused. The refusal names the plugin, the first content path and `./scripts/release.sh <plugin> <version>`. Override: `SKIP_VERSION_BUMP_CHECK=1`.

Content here is every changed path under the plugin except `.claude-plugin/plugin.json`, `docs/`, `tests/`, and the top-level `README*` and `CHANGELOG*`. The version is compared between the two tips, so content commits followed by a bump commit in one push pass. The guard skips a branch the remote does not have yet, a plugin added in the push, and a version it cannot read (no `jq`, bad JSON). Option a, the `content_matches_source` field in the `version-bump-checklist` augmentation, was not built.

The guard runs from the installed `pre-push` shim, which calls the script, so an edit takes effect for every session that pushes from that checkout. `scripts/check-hooks-installed.sh` reports the hook installed here.

**What it found on real history.** Run over `HEAD~10..HEAD` of `claude-plugins` it refuses `codescout-companion`: commit `7a3c3b1` (2026-10-31, `fix(wake): accept a profileRoot that resolves to the selected profile`) changed `codescout-companion/background-wake/config.mjs` after the 1.20.26 bump (2026-10-30) with no bump of its own, and is already on `main`. That commit was not touched here. Releasing it needs `release.sh` and an update of the install records, which was not done.


## Tests added

12 cases in `claude-plugins` `tests/test-pre-push-guard.sh`, each in a fresh synthetic repo with two plugins: a content edit with no bump is refused (positive control, with the refusal's plugin, path and repair); the escape hatch; a non-protected branch; a new remote branch; content plus a bump in one commit; content commit then bump commit in one push; docs, tests, README, CHANGELOG and non-plugin paths only; a manifest edit that keeps the version plus content; two plugins with one bumped, naming only the other; a plugin added in the push. The suite is 31 of 31, and `tests/run-all.sh` passes.

Mutations: disabling Guard 3 fails five deny cases; removing the `docs/` and `tests/` exclusion fails the exclusion case. The real-history Guard 1 and 2 cases now run with `SKIP_VERSION_BUMP_CHECK=1`, so a deny there cannot come from Guard 3.

## Fix provenance

- **SHA:** `claude-plugins:32ce0ff` (`main`, local, not pushed)
- **patch-id:** `2a30b7ff2157de8581532e3322d1d5a8570dae91`

## References

- `docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md` — parent
