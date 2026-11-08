---
id: f4926e0a61d4cd69
kind: bug
status: mitigated
title: Codex co-author trailer recorded as a bare Claude identity
tags:
- cluster/unclassified
- provenance
- publish-holds
closed: 2026-11-08
opened: 2026-11-06
owner: marius
severity: medium
unverified: The warning cannot tell a mistaken Codex id from a legacy bare Claude id, so it fires on both and refuses neither. Archive commit 867546ab6 stays published with the bare token. Tested in a disposable repo only; not seen on a real commit. The pre-commit and pre-push guards still resolve a bare token to a Claude hold ref.
---

# BUG: Codex co-author trailer recorded without its namespace

## Summary

The fix-branch instance is corrected: current commit `36fec1201b0f809d6f21cc31082def4d651b6cf8` replaces historical `30e4d928561410d55d390cc87323110eb698ae6f` and carries the proper namespaced Codex co-author token. Fresh tree comparison is empty and worktree clean.

A second observed instance remains: qualification-archive commit `867546ab6c54851177004eb11076c0658f3755b2` on experiments carries the same bare Codex UUID in Co-Authored-Session-Id. Its body identifies Codex correctly. Real identity-helper controls map that bare token to the Claude hold namespace. This issue remains open for the archive metadata and any guard validation gap. Update 2026-11-07: the archive commit is published as is on origin/experiments; see the remote verification below. No active-hold bypass is established.

## Class fit

Unclassified cross-harness provenance metadata error. Source content and validated behavior are unchanged; the identity discriminator is wrong. The archive instance is now confirmed in published history; whether an active hold was bypassed is not established.

## Symptom (Effect)

Actual commit trailers:

```
Co-Authored-Session-Id: 01a15ca9-f939-7603-acb5-01c6b02f68e2
Session-Id: b3485581-2540-4da8-a440-3bed2e9c517f
```

The message body correctly says the co-author is Codex. Correct namespaced co-author value is `codex:01a15ca9-f939-7603-acb5-01c6b02f68e2`.

## Reproduction

Read actual commit metadata with git log. Source the real scripts/publish-identity.sh and invoke publish_hold_ref with bare versus codex-prefixed UUID. Observed outputs:

```
refs/holds/01a15ca9-f939-7603-acb5-01c6b02f68e2
refs/holds/_codex/01a15ca9-f939-7603-acb5-01c6b02f68e2
```

This is an executed read-only helper control; no hold ref was created, released or changed.

## Environment

MAIN scripts/publish-identity.sh and scripts/pre-push-foreign-session-guard.sh; commit on linked worktree /tmp/codescout-tmp-exec-01a15ca9. Committing session was Claude Code, co-author Codex. All five committed files freshly compare byte-for-byte with validated archivea6f224468b952c814988dc784f1bf036d02ad71aea1ba22ab3408b0bfc405f38; worktree clean.

## Root cause

Confirmed metadata choice: the emitted co-author token lacks the Codex harness prefix. publish_hold_ref intentionally maps bare tokens to legacy Claude refs; held_sid_of passes trailer tokens through that helper. Whether the pre-commit co-author validation should have rejected this cross-harness representation needs further investigation.

## Evidence

Historical original observation: `30e4d928561410d55d390cc87323110eb698ae6f` carried the bare token. Correction verified on current `36fec1201b0f809d6f21cc31082def4d651b6cf8`: namespaced token present, old-to-new tree diff empty, same patch-id, clean worktree.

Additional actual Git observation: `867546ab6c54851177004eb11076c0658f3755b2` contains only qualification archive plus receipt and still has the bare co-author value. The earlier real publish_hold_ref control demonstrates differing namespaces. No active-hold/push experiment was performed.


### Published archive metadata — verified 2026-11-07

The peer reports another session published archive commit 867546ab6c54851177004eb11076c0658f3755b2. Independently executed git ls-remote origin refs/heads/experiments: server tip 267d8cd0178c772c64da014fbaba77f94bf45fc4. A standalone git merge-base --is-ancestor 867546ab6c54851177004eb11076c0658f3755b2 267d8cd0178c772c64da014fbaba77f94bf45fc4 exited 0. git show -s independently confirms the published archive commit retains the bare Codex co-author UUID and plain Claude author-session trailer.

Thus the historical statement that no actual publication was established is superseded for this archive instance. This check does not identify the pushing session or establish an active-hold bypass or operator authorization. This thread did not push, amend or rewrite history. Preserve the published commit; record the canonical codex:01a15ca9-f939-7603-acb5-01c6b02f68e2 attribution here while the guard-validation question remains open.

## Hypotheses tried

Bare and codex-prefixed tokens are equivalent: refuted by the actual helper returning different refs.

## Fix

Update 2026-11-08: `scripts/prepare-commit-msg-session-id.sh` names any `Co-Authored-Session-Id` token without a `codex:` or `claude-code:` prefix on stderr and edits nothing. It does not refuse, because the harness cannot be read from a uuid. There is no `commit-msg` hook; the check lives in the existing `prepare-commit-msg` hook, which sees hand-written trailers.

## Fix provenance

- **SHA:** `41bfc5311a4f8471009e383aab30b0f538868c0d` (`experiments`)
- **patch-id:** `a953d1729888baa126ba13c328f3acb91c300421`

The committing Claude Code session amended its isolated fix-branch commit: `30e4d9285` is historical; current `36fec1201` has the canonical co-author token. Source tree and stable patch-id `e97db759fc5d8684ce5dd7faed27309d59a106f1` are unchanged. Verified through actual Git metadata and tree comparison.

Archive commit `867546ab6` remains unchanged on the shared experiments history. Its repair must respect subsequent peer commits; this Codex session does not amend or rewrite it. Investigate a guard regression for known cross-harness co-author representation without inferring harness from UUID alone.

## Tests added

`tests/test_codex_publish_hold.py`: `test_a_bare_co_author_token_is_named_on_stderr_and_left_in_place` (red before the hook change), and `test_a_qualified_co_author_token_is_silent` for `codex:` and `claude-code:`. Mutation: dropping the prefix filter fails both qualified cases. Gate green 2026-11-08: FMT, CLIPPY, LEAN 6233, DEFAULT 9125, Python 1764.

None. Read-only helper comparison executed; source remains untouched.

## Workarounds

Record the correct namespaced co-author explicitly in coordination. Preserve the original commit and source evidence; do not independently amend another session's commit or alter hold refs.

## Resume

Status `mitigated`: new commits get a warning, history is unchanged. Do not repeat the fix-branch amendment (`36fec1201`). Archive commit `867546ab6` is published as is; preserve that history and do not amend another session's commits.

Still open, by design: the guards accept a bare token as a Claude session, because legacy trailers are bare Claude ids. A refusal would need a rule for those legacy ids, and a uuid alone cannot name its harness. Revisit if a bare Codex token again reaches a published commit after the warning.

## References

Native correction issue4d66b80cca2e6390. docs/conventions/shared-checkout-commit-sequence.md section A session that cannot publish must not commit.
