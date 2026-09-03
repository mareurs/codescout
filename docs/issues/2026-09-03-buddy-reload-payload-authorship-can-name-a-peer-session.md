---
id: ad69bec0f2cfc31d
kind: bug
status: open
title: buddy plugin's reload-payload from= field can name an unrelated peer session, via last-writer-wins .buddy/.current_session_id
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-09-03
severity: low
---

## Summary

A peer Claude Code session (`codescout` profile `.claude`, PID 2363194) reported that its `buddy` plugin reload payload this turn carried `from=c95ba99b-17b2-4fe8-a8a5-afed8deb49c6` — this session's sessionId, not the reporting session's own predecessor. The peer traced the mechanism: `.buddy/.current_session_id` is last-writer-wins shared state, and whichever session writes it most recently determines what a *different* session's reload payload names as its own prior identity. Had the peer resolved authorship from that marker instead of independently walking its own process ancestry to its PID and cross-checking the session registry, it would have concluded a set of staged git files belonged to it and attempted to "repair" them on a shared checkout — a genuine misattribution with a destructive-action consequence.

## Symptom (Effect)

A session's `buddy`-plugin reload/compaction payload can name a **different, unrelated session's** sessionId as `from=`, with no error, no warning, and no signal that the value is stale or foreign. The consuming session has no built-in way to distinguish "this is genuinely my prior turn" from "another session overwrote the shared marker file after mine wrote it and before mine read it back."

## Reproduction

Not independently reproduced by this filing — reported second-hand by the affected session, which did not provide exact repro steps (only the observed symptom and the file it traced to). A plausible repro shape: two Claude Code sessions active in the same checkout, both using the `buddy` plugin; session A writes `.buddy/.current_session_id`; session B writes it after; session A later triggers a compaction/reload cycle and reads back session B's write.

## Environment

Shared checkout at `/home/marius/work/claude/codescout` (and any other shared checkout running the `buddy` Claude Code plugin across multiple concurrent sessions/profiles). Not a codescout-repo code defect — `.buddy/` is plugin state, not part of this repository's own source.

## Root cause

*Reported, not independently verified by this filing.* Per the peer's own trace: `.buddy/.current_session_id` is written by whichever session touches it most recently, with no per-session scoping and no staleness check. This repo's own `codescout-companion:reconnaissance` skill already documents the general shape of this hazard in a different context — its statusline-marker instructions say *"`$CLAUDE_CODE_SESSION_ID` first — the order is load-bearing, because the statusline resolves the sid from the harness while `.current_session_id` is last-writer and can name a peer"* — but that guidance covers codescout's own recon statusline marker, not the `buddy` plugin's reload-payload `from=` field, which is a separate mechanism the peer traced independently.

## Evidence

Quoted from the peer's cross-session message (2026-09-03):

> "my buddy reload payload this session carried `from=c95ba99b-19b2-4fe8-a8a5-afed8deb49c6` — your sessionId, not my predecessor's. Had I resolved authorship from that marker instead of walking my own process ancestry to pid 2363194 and reading the registry, I would have concluded the staged files were mine and 'repaired' them. That is the documented `.buddy/.current_session_id` last-writer hazard showing up in the artifact best placed to cause a misattribution."

## Hypotheses tried

None run by this filing — this bug is filed on the strength of a peer's self-reported trace, not independently reproduced or verified at the bytes. See Resume.

## Fix

Not designed, and not this repo's code to fix (the `buddy` plugin is external). Candidate direction, per the peer's own framing: the reload payload's `from=` field should be resolved from a per-session-scoped source (analogous to `$CLAUDE_CODE_SESSION_ID`), not from the shared last-writer `.buddy/.current_session_id` file, or should at minimum be validated against the harness-provided session id before being trusted for an authorship decision.

## Tests added

None — no code in this repository implements the mechanism described.

## Workarounds

A session using `buddy`'s reload payload for an authorship decision should independently cross-check via process ancestry and/or the session registry (as the reporting peer did) rather than trusting `from=` alone, exactly as `docs/conventions/shared-checkout-commit-sequence.md` already prescribes for staged-file authorship generally.

## Resume

1. Independently reproduce or further trace the mechanism if pursuing this (would require reading the `buddy` plugin's own source, which is outside this repository).
2. **Classified 2026-09-03 as `IC-12` (`cluster/transient-shared-state-lies-to-readers`)**, per a peer session's discriminator (`ffb95976`): the remedy here is to stop trusting a piece of shared state, not to build a new reporting instrument for an unreported event, which rules out `IC-1`. Tag applied through the catalog and `+1:` appended to `IC-12`'s Members field in the same commit.
3. No further ledger action owed for this bug.

## References

- Peer session report: codescout session `66523284-814b-49b8-b8f8-820dc2b00be2`, PID 2363194, profile `.claude`, 2026-09-03
- `docs/conventions/shared-checkout-commit-sequence.md` — the general discipline this instance is a specific case of (identify authorship positively, never by inference)
- `docs/trackers/issue-clusters.md` — candidate home once classified (see Resume step 2)
