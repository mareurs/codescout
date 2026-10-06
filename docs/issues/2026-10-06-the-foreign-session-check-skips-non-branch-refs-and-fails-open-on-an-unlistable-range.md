---
kind: bug
status: open
tags:
- cluster/guard-narrower-than-its-name
closed: null
opened: 2026-10-06
owner: marius
related:
- docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md
severity: medium
---

# BUG: the pre-push guard's foreign-session check skips every non-branch ref and fails open when its range cannot be listed

## Summary
`scripts/pre-push-foreign-session-guard.sh` refuses a push that carries another session's commits, but only for `refs/heads/*` pushes whose remote tip it can resolve locally. A tag push, a `refs/wip/*` push, and a branch push over a remote tip that is not in the local object store all exit 0 with no refusal and no note, publishing foreign commits. The publish-hold check added on `feat/publish-hold` (merged as `4766c07f`) was fixed for exactly these two paths; the older foreign-session check was deliberately left as it was.

## Symptom (Effect)
Reproduced 2026-10-06 against the real guard. The same history (alice base, bob's commit, alice on top), pusher = alice's session id, no `CODESCOUT_PUSH_ACK`, hook stdin as shown:

```
(a) refs/heads/main <tip> refs/heads/main <base>        -> exit 1; 4736 bytes; "REFUSING THE PUSH: it would publish commits belonging to another session."; bob's sid named
(b) refs/tags/x <tip> refs/tags/x 000...0               -> exit 0; 0 bytes of output
(c) refs/wip/x <tip> refs/wip/x 000...0                 -> exit 0; 0 bytes of output
(d) refs/heads/main <tip> refs/heads/main deadbeef...   -> exit 0; 0 bytes of output  (remote sha not in the repo)
(d2) refs/heads/main <tip> refs/heads/main 000...0      -> exit 1; 4783 bytes; refused  (control: new branch, same history)
```

(a) and (d2) are the controls: the same history is refused on a branch push. (b), (c) and (d) publish the same foreign commit in silence.

## Reproduction
At HEAD `4766c07f` (branch `experiments`), throwaway repo under `mktemp -d` (removed afterwards; nothing pushed, this checkout's refs untouched). The harness copies `new_repo` / `commit <sid> <subject>` / `run` from `tests/pre-push-foreign-session-guard.sh`: `git init -q -b main`, `core.hooksPath /dev/null`, three commits with `Session-Id` trailers (alice, bob, alice), then for each case:

```
printf '%s\n' "<local_ref> <tip> <remote_ref> <remote_sha>" | (cd "$REPO" && env -u CODESCOUT_PUSH_ACK CLAUDE_CODE_SESSION_ID=<alice-sid> HOME="$REPO" scripts/pre-push-foreign-session-guard.sh origin git@example.invalid:x)
```

`HOME` points at the throwaway repo so the guard never reads this machine's session registry. The harness lived in the filing session's scratchpad (`repro.sh`) and is not kept; the four stdin lines above are the whole input. Real-world forms that produce (b)/(c): `git push origin v1`, `git push origin HEAD:refs/tags/x`, `git push origin HEAD:refs/wip/x`; `docs/RELEASE.md:60-61` runs `git push` and then a separate `git push --tags`. Form (d): a force push over a remote tip never fetched here.

## Environment
Linux 7.2.8-zen1-2-zen, bash, git as installed on this machine, codescout checkout on `experiments` at `4766c07f`. The guard is run directly (the same invocation `.git/hooks/pre-push` makes), so no hook-install state moves the result.

## Root cause
Two separate fail-open paths in the foreign check, both in `scripts/pre-push-foreign-session-guard.sh`:

1. **Non-branch refs are skipped before the scan.** In the per-ref loop (`while read -r local_ref local_sha remote_ref remote_sha`, line 286), the filter at lines 306-307 is `case "$local_ref" in refs/heads/*) ;; *) case "$remote_ref" in refs/heads/*) ;; *) continue ;; esac ;; esac`. A tag or `refs/wip/*` ref matches neither arm, so the loop `continue`s and the foreign scan (range at 313-315, `git log` at 450) never runs for it. The comment at 295-304 states the skip ("Tag pushes still fall out") as intended behaviour for tags.
2. **An unlistable range lists zero commits.** The foreign scan's range is `"$remote_sha..$local_sha"` (line 315) and the scan is `done < <(git log ... "${range[@]}" 2>/dev/null)` (line 450). If `remote_sha` is not a commit in the local object store, `git log` fails, its stderr is discarded, the process substitution's status is not examined, the loop body runs zero times, `foreign_report` stays empty, and `[ -n "$foreign_report" ] || exit 0` (line 679) exits 0.

The hold check has neither defect because it has its own scan (`hold_scan_ref`, lines 245-283, between the `PUBLISH HOLD` markers). It is called for EVERY non-deletion ref before the branch filter (line 292), uses the range `<tip> --not --remotes` whenever the remote tip is not a local commit (lines 250-253), and when `git log` fails while any `refs/holds/` ref exists it sets `hold_unlistable` (lines 257-261), which the refusal block at lines 457-461 turns into a refusal. Its own comment says it is "a scan of its own" precisely because the foreign scan "never sees non-branch refs, and its `git log ... 2>/dev/null` fails OPEN".

measured 2026-10-06: cases (a)-(d2) above against the live script; the line numbers are from reading the script at `4766c07f`, not from instrumenting it.

## Evidence
- Symptom block above (exit codes and output sizes captured by the repro harness, 2026-10-06).
- `scripts/pre-push-foreign-session-guard.sh` lines 245-283 (hold scan, fail-closed), 286-315 (per-ref loop and the skip), 450 (silent `git log`), 679 (`exit 0` on empty report).
- The ruling to leave the foreign check unchanged is recorded in the filing session's brief: changing it silently turns on new refusals for everyone, which is an operator decision.

## Hypotheses tried
1. **Hypothesis:** tag and `refs/wip/*` pushes are skipped by the branch filter. **Test:** cases (b) and (c). **Verdict:** confirmed. **Evidence:** Symptom block.
2. **Hypothesis:** a remote sha absent from the local store makes the foreign scan list nothing. **Test:** case (d) with a made-up 40-hex remote sha, against the case (d2) control. **Verdict:** confirmed. **Evidence:** Symptom block.

## Fix
Not applied. Not decided: whether to apply it at all (see Resume).

## Tests added
N/A - nothing changed. If the foreign check is changed, the discrimination cases belong in `tests/pre-push-foreign-session-guard.sh` next to the existing hold cases for the same two paths (each asserting refusal on the tag / `refs/wip` / unlistable form AND silence on a tag that carries only the pusher's own commits).

## Workarounds
Before any tag or `refs/wip` push, or a force push over a tip you have not fetched, run the range yourself: `git log --format='%h %(trailers:key=Session-Id,valueonly) %s' <tag-or-sha> --not --remotes` and read the session ids. For releases, push the branch by refspec first (that path IS guarded), then the tag.

## Resume
Operator decision, not an implementation task: should the foreign check adopt the hold check's range logic? Concretely, `hold_scan_ref`'s range selection (`scripts/pre-push-foreign-session-guard.sh` lines 250-253) and its fail-closed branch (257-261, 457-461) would be applied to the foreign scan, and the branch-only filter at lines 306-307 would be dropped or narrowed. What it would newly refuse: (1) a tag or `refs/wip/*` push whose reachable-but-unpublished commits include another session's, including the second step of `docs/RELEASE.md:60-61`; (2) a branch push over an unfetched remote tip, which the guard would now scan over `<tip> --not --remotes` (it can over-refuse, never under-refuse); (3) with no holds in play, an unlistable range would need its own decision, since the hold check only fails closed when a hold exists. Each of these is a refusal that does not exist today for anyone. Check `tests/pre-push-foreign-session-guard.sh` for the cases that would change before touching the script.

## References
- `scripts/pre-push-foreign-session-guard.sh` (foreign check, `hold_scan_ref`), `tests/pre-push-foreign-session-guard.sh`, `docs/RELEASE.md:60-61`.
- `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` (the hold this check was built beside; `OB-20`).
- Merge of the branch that fixed the hold check for both paths: `4766c07f` (`feat/publish-hold`).
