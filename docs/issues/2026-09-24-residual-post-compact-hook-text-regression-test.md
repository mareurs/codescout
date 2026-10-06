---
id: f30564da22944782
kind: bug
status: fixed
title: 'RESIDUAL: Add a test asserting the post-compact hook text so the corrected messaging cannot silently regress'
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Add a test asserting the post-compact hook text so the corrected messaging cannot silently regress.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md` (status `mitigated`):

> The MESSAGING is fixed and verified live in all three profiles; the MECHANISM is untouched. (a) the prewarm is deliberately unshipped — its prescribed form is a no-op for this bug's own Rust reproduction (PREWARM_LANGUAGES is JVM-only) and the workspace-keyed mux makes the cold window far narrower than this record assumed, so the flush still does not prewarm and a genuinely cold single-session workspace still pays. Also NO REGRESSION GUARD: nothing asserts the hook text, so the sentence can regress silently — which is why this is not archived. The original 60s timeout has still never been reproduced with the mux confirmed down; that measurement remains owed.

## Fix

The parent's "nothing asserts the hook text" was overtaken by a sibling-repo commit 2026-08-31, before this residual was filed (2026-09-24): `claude-plugins:65dc320` ("test(session-start): guard the post-compact injection in both directions") added to `codescout-companion/hooks/session-start.test.sh` (lines ~339-360) a paired guard over the post-compact block that `codescout-companion/hooks/session-start.mjs` injects when `source === 'compact'` (lines ~345-351). Verified at the bytes on 2026-10-06 against claude-plugins HEAD `c7e19c2`:

- Four positive assertions, one per sentence of the block: the `POST-COMPACT: Context was just compacted.` header (so deleting the whole block fails), the remedy `workspace(post_compact=true)`, the cost `pays the language-server`, and the condition `shared per workspace, not per session`.
- One negative assertion: the block must not contain `no disruption` (the original wording this bug was about). The positives exist because a negative-only guard is monotone under removal of the block; the comment in the test says so.
- A startup-source case asserts the block is NOT injected (gate exercised).
- Run on 2026-10-06 with `TMPDIR=/dev/shm bash codescout-companion/hooks/session-start.test.sh` (exit 0; the eight compact-related lines PASS, including the five above). `/tmp` was full on this machine, hence `TMPDIR`.

The same commit also edited `docs/trackers/version-bump-checklist.md` in claude-plugins (not part of this fix).

No code change in this sweep; the record is bookkeeping. The parent's other caveat items are untouched: the mechanism (no prewarm on flush) is deliberately unshipped, and the 60 s timeout has still never been reproduced with the mux confirmed down; that measurement remains owed and lives in the parent.

## Tests added

`codescout-companion/hooks/session-start.test.sh` in claude-plugins, the `COMPACT=$(ctx compact)` block (lines ~339-360), added by `claude-plugins:65dc320`: four positive assertions plus the `no disruption` negative described under Fix. No test was added in this sweep.

## Fix provenance

- **SHA:** `claude-plugins:65dc320` (branch: `main`; also contained in `feat/effort-steering-adapter`, `feat/effort-steering-core`, `fix/buddy-codex-summon`, `tool-collapse`)
- **patch-id:** `79b9d5160a067c5bffb6f81f79fc6c6cb9a9ce8e`

## Resume

Closed on 2026-10-06; nothing to resume. Residual follow-ups, listed and not filed: (1) `docs/manual/src/concepts/post-compact-cache-flush.md` (lines ~52-58) quotes the hook's block verbatim and no test guards that copy; it matches the hook today (checked 2026-10-06), so this is a drift risk, not drift. (2) The 60 s timeout measurement with the mux confirmed down is still owed, in the parent `docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md`.

## References

- `docs/issues/archive/2026-08-28-post-compact-flush-leaves-first-nav-call-to-pay-cold-start.md` — parent
