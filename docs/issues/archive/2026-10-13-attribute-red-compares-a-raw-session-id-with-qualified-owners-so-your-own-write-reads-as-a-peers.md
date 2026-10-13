---
id: bf051af0e4dc6bc6
kind: bug
status: fixed
title: 'BUG: attribute-red compares a raw session id with qualified owners, so a session''s own uncommitted write reads as a peer''s'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
topic: authorship attribution at a red
closed: 2026-10-13
opened: 2026-10-13
severity: medium
unverified: 'TRACKED f1e8539dbf3cb90f — tests/attribute-red.sh still has no CI or gate runner. Full gate 2026-10-14: the first run was red on two tests outside this change (prompts::tests::reader_docs_contain_no_retired_call_forms, 46 hits, every one under the untracked .codescout/resident-private/codex-pilot-fix-20261013/commit-snapshot/; server::guide_hint_tests::native_context_wire_reaches_interactive_shell, open bug 098fca009ecbdd78). The second run, after only comment re-pointing, passed every lane: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0. Why the reader-docs hits did not recur was not established. tests/attribute-red.sh passed 43 of 43 by hand on 2026-10-14. A Codex session''s own write is not covered, because that needs a Codex transcript fixture.'
---

# BUG: attribute-red compares a raw session id with qualified owners, so a session's own uncommitted write reads as a peer's

## Summary

Since `eed25579` (2026-10-11) the provenance scan returns owners as qualified principals, `claude-code:<id>` or `codex:<id>`. `scripts/attribute-red.py` still builds its own identity from the raw `CLAUDE_CODE_SESSION_ID` and its live-session map from raw ids. So "is this write mine" is always false, and the live-session lookup always misses. A session's own dirty file is reported as written by a peer, with the "ask before editing" advice. A Codex session has no identity on this path at all.

Reported 2026-10-13 by a user auditing the `experiments` branch at `18a007aa`. Re-verified at `5ae71b35` by reading the code. Not reproduced against a real transcript history.

## Symptom (Effect)

Predicted from the code, not observed on a real red: when the session that wrote the dirty file reads the red, the output names `claude-code:<own id>` under `written by …` and sets `foreign_any`, instead of the line `written by THIS session (…) — your own uncommitted work, so this red is likely yours.`

## Reproduction

Not yet reproduced end to end. Best lead: seed `owners` with one entry `("claude-code:S", <time after last commit>)` for a dirty path, set `CLAUDE_CODE_SESSION_ID=S`, and call `attribute-red.py --paths <that path>`. By the code the foreign branch runs. This needs the helper to import, so run it from `scripts/` until the sibling-module bug is fixed (`docs/issues/archive/2026-10-13-embedded-attribution-helper-ships-two-of-its-six-modules-so-a-red-loses-its-author-silently.md`).

## Environment

Linux, Python 3.12, branch `experiments`, `5ae71b35`.

## Root cause

- `scripts/claude_history.py:116,146` and `scripts/codex_history.py:180,244` build every principal with `principal_key(<harness>, <sid>)`, and `scripts/session_context.py:40-41` defines it as `harness + ":" + session_id`. `scan_provenance` (`scripts/file-provenance.py`) stores that principal as the owner.
- `scripts/attribute-red.py:220` sets `me = os.environ.get("CLAUDE_CODE_SESSION_ID", "")`, the raw id, and `:248` compares `peers = [w for w in who if w != me]` against the qualified owner.
- `scripts/attribute-red.py:219` takes `live` from `fp.live_sessions()`, which is keyed by raw session id (`file-provenance.py:415`). Line 259 then calls `fp.address_lines(w, live, …)` with a qualified `w`, so no live row matches.
- `scripts/file-provenance.py:1164-1170`, the sibling consumer, does it right: `me = principal_key(resolution.context.harness, resolution.context.session_id)` from `resolve_context`, and `live` re-keyed with `principal_key("claude-code", sid)`.
- The `me` line dates from `de546287` (2026-09-08), when owners were raw ids. `eed25579` changed the producer and left this consumer.

Measured 2026-10-13: read at the lines above; `git log -S` places the producer change at `eed25579`. Inferred from the code, not measured: the visible effect on a real red.

A second, separate gap: the compatibility API `scan()` (`file-provenance.py:997-999`) returns only owners, so `attribute-red.py` never sees the coverage gaps that `scan_provenance` returns. A path with no owner can then read as `no session on record wrote this`, although the scan flagged its coverage as incomplete. That message already says it is about coverage, but it cannot say which gap.

## Evidence

Source lines above. No transcript run.

## Hypotheses tried

1. **Hypothesis:** `fp.scan` re-keys its owners to raw ids before returning. **Test:** read `scan` → `scan_with_evidence` → `scan_provenance`; `owners.setdefault(rel, []).append((who, when))` with `who = op.principal`. **Verdict:** rejected. **Evidence:** root cause section.

## Fix

Implemented 2026-10-13 and committed. Fix commit: `3fb612b9` on `experiments` (experiments only), patch-id `b2b2d7bb9849ea740821942f91193b33d6404b8a` (`git show 3fb612b9 | git patch-id --stable`; one commit that also fixes the two sibling bugs).

In `scripts/attribute-red.py`, stage 3:

1. `me` is now `principal_key(harness, session_id)` from `resolve_context(root, os.environ)`, the same two lines `file-provenance.py` `main()` uses. When the context does not resolve, `me` stays empty, as it did when the environment variable was absent.
2. `live` is re-keyed with `principal_key("claude-code", sid)`, and the contact line uses `fp.owner_contact`, which also answers for a non-Claude principal (`[contact unknown; liveness unknown]`).
3. The "written by THIS session" line uses `fp.short_principal(me)`.

NOT done, from the original plan: a message for an unresolved identity, and the switch to `scan_provenance` so coverage gaps reach the output. Both are separate changes.

## Fix provenance

- **SHA:** `3fb612b96d5c4ad4fcd192a7c889c4a36c85ea88` (`experiments`; one commit that also fixes the two sibling bugs of 2026-10-13)
- **patch-id:** `b2b2d7bb9849ea740821942f91193b33d6404b8a` (`git show 3fb612b9 | git patch-id --stable`, re-observed 2026-10-14)

## Tests added

No new case was needed for the main defect: `tests/attribute-red.sh` already held it and was red. At `5ae71b35` it passed 36 and failed 5 (my-own-write named as mine, `likely yours`, `THIS session` after the window, `[LIVE]`, the `uds:` address). After the fix it passes all cases.

Added to `tests/attribute-red.sh`:

- `the HARNESS is part of the identity`: a Codex session whose native id equals a Claude writer's is not that writer. Mutation-checked on a copy: making the engine ignore the harness fails both assertions.
- `run` now unsets `CODEX_SESSION_ID`, `CODEX_THREAD_ID` and `CODESCOUT_CONTEXT_FILE`, because `resolve_context` reads them and the suite would otherwise depend on the harness it runs in. `run_as_codex` is the matching helper.

Measured 2026-10-13: 41 of 41 pass after the fix, 43 of 43 with the two new cases.

No case covers a Codex session's OWN write, because that needs a Codex transcript fixture.

## Workarounds

Run `scripts/file-provenance.py` directly. It resolves the identity correctly.

## Resume

Complete. Fixed in `3fb612b9`, public on `origin/experiments`. The missing runner is tracked in `docs/issues/2026-10-14-tests-attribute-red-sh-is-run-by-no-ci-job-or-gate-lane.md`.

Found while fixing, kept here as the record: `tests/attribute-red.sh` has no runner. Verified 2026-10-13 and again 2026-10-14 by a repository grep: the only hit is a code comment. `.github/workflows/ci.yml` job `shell-tests` runs `hooks-discrimination.sh`, `file-provenance.sh` and `peer-sessions.sh` only, and `scripts/gate.sh` does not run it. That is how the suite sat 5 cases red from `eed25579` (2026-10-11) with every gate green.

## References

- `scripts/attribute-red.py`, `scripts/file-provenance.py`, `scripts/session_context.py`
- Commits `eed25579`, `de546287`
- `docs/issues/2026-09-19-file-provenance-answers-at-session-grain-so-sibling-subagents-are-one-writer.md` — the open neighbour on the same consumer.
