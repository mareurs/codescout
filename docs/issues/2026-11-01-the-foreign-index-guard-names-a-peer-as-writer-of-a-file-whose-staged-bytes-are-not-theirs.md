---
id: '581b05bbe41134f3'
kind: bug
status: fixed
title: 'BUG: the foreign-index guard names a peer as the writer of a file when the staged bytes are not theirs, and its own remedy does not clear the refusal'
tags:
- cluster/unclassified
closed: 2026-11-01
opened: 2026-11-01
owner: marius
severity: low
unverified: The refusal was reproduced in a scratch repo by case 27a, not on the real shared index. The first live check is the next pathspec commit of a file whose peer write was reverted or never landed. The reported cause of the unclearable remedy (re-adding asks the same lookup again) is read from the code, not run against the old guard.
---

# BUG: the foreign-index guard names a peer as the writer of a file when the staged bytes are not theirs, and its own remedy does not clear the refusal

## Summary
On 2026-11-01 a pathspec commit of `src/tools/run_command/output.rs` was refused twice. The refusal said the commit "captures content another session wrote" and named session `f6c27cf1-225c-4791-9084-17cffa386faf`. The staged bytes were mine. The hook's own remedy (reset my staged path, add it again) did not clear it. The commit went in with `--no-verify`, on the user's explicit approval (`421b0f896`).

## Class fit
`cluster/unclassified`, meaning looked and nothing fits. The nearest class is `value-correct-in-a-frame-its-name-does-not-state`: the stage log holds a true fact (that session wrote this file earlier) under a name that states another (content staged now is theirs). I did not adjudicate the fit. It needs a second member before a class is claimed.

## Symptom (Effect)
- `git commit -F <file> -- src/tools/run_command/output.rs` stopped with `Refusing this pathspec commit: it captures content another session wrote.`, `theirs:` the path, and the writer `f6c27cf1-...`.
- The staged diff of `output.rs` was byte-identical to my worktree diff (45 lines).
- No peer commit touched the file after my base, and the file was clean before I applied my patch.
- The remedy the hook printed (reset my staged path, then add it again) did not clear the refusal.

## Reproduction
Not reproduced. One occurrence, twice in a row on the same staged blob.

## Environment
Linux, `experiments`, shared checkout with several live sessions. My real session id is `48d211eb-3bd8-4247-975b-631a73b1a87a`. The id named by the hook, `f6c27cf1-...`, belongs to a different, idle, live process (registry name `codescout-5a`, pid 2675141).

## Root cause
Partly measured, not complete.

Measured:
- `.git/session-stage-log` row 15 reads `f6c27cf1-225c-4791-9084-17cffa386faf	c4fb8e67e	src/tools/run_command/output.rs	named-foreign	retained`. Blob `c4fb8e67e083bc6f34c2429d3bfdd97c0fcc4353` is the `output.rs` committed in `421b0f896` (`git rev-parse 421b0f896:src/tools/run_command/output.rs`). So the log named that session as owner of my bytes.
- `scripts/post-index-change-stage-log.sh` records route `named-foreign` when the stager names the path and `foreign_writer "$path"` returns a peer. Its own comment says the grain is the FILE and that a `run_command` or native write leaves no path in the record.
- The tool-call record (`.codescout/usage.db`, table `tool_calls`) holds two successful `edit_file` calls by `cc_session_id` `f6c27cf1-...` on `src/tools/run_command/output.rs` in this checkout, at 2026-10-31 10:12:55 and 10:13:48. Each replaces `cut.total` with `cut.shown` in one `stdout_total` line. My staged blob is therefore judged by a file-level write record from a day earlier, and not by whether the staged bytes contain that session's bytes.
- `scripts/pre-commit-foreign-index.sh` reads the log keyed on `(blob, path)` and refuses on any `owner` that differs from the caller (line 306 onward).

Not established:
- Why my remedy (reset, add) did not change the row: the row carries `retained`, and I did not trace the recorder's handling of it.
- Whether the earlier edits are still in the file's history. At my base the file was clean.

## Evidence
Rows above, from `.git/session-stage-log` lines 15, 469, 471, 832, 856 (`grep -n "run_command/output.rs"`), and tool_calls ids 239423 and 239437.

## Hypotheses tried
None beyond reading the scripts.

## Fix

`300cb29ea`. `foreign_writer` in `scripts/post-index-change-stage-log.sh` now asks a second question after `write_in_head`: `write_absent_from_index`. A live write names its writer only while the version STAGED for the path holds some of the write's text. The probes are the same lines `write_in_head` uses, now printed by one shared function, `write_probes`.

The test stays conservative. The write stays the peer's when the call has no distinctive line (under 16 characters), when ANY probe line is in the staged blob, and when the path is not in the index (a staged deletion).

The cause was measured, not guessed. The peer's `new_string` was `response["stdout_total"] = json!(cut.shown);` (tool_calls 239423 and 239437). That line is in neither `HEAD` nor `421b0f896`, where line 1461 reads `stdout_shown`. So the write stayed live for its three-day window, and the file-level lookup named the peer. Re-adding the pair could not clear it because `foreign_writer` runs again on every add and answers the same.

## Tests added

`tests/hooks-discrimination.sh` section 27, cases 27a to 27e. Red first: on the old code only 27a failed (owner and route). Suite 429 of 429.

| Mutant | Killed by |
|---|---|
| drop the `write_absent_from_index` call | 27a |
| drop the `return 1` that guards the staged `git show` | 27e |
| the probe loop always says absent | 27b, 27c and 22 cases of section 24 |

Survives, and is equivalent: the `[ -n "$probes" ] || return 1` guard in `write_absent_from_index`. With no probe, the loop reads one empty line and `grep -F ""` matches it, which gives the same answer. I kept it as an explicit line.

Gate: run in a clean detached worktree at `HEAD` with only these two files copied in, exit 0 (5476 and 8192 lib tests, no failure). The shared tree's own gate run was red on a peer's uncommitted `src/usage/` change that does not compile (`MeasurementLinkage` missing `attribution_ms` and `attribution_outcome`).

## Workarounds
Commit with `--no-verify` after checking by hand: `rustfmt --check`, the staged diff equals the worktree diff, no peer commit touched the file. Only with the operator's explicit approval.

## Resume

Fixed in `300cb29ea`. Not yet seen on the real index (see `unverified`). Left open by design: a write whose text IS in the staged blob but was written by a peer in an earlier, since-reverted pass still names that peer, because text equality cannot tell the two apart.

## Fix provenance

- **SHA:** `300cb29ea` (`experiments`)
- **patch-id:** `ecc5728d0784f0984b642898f6e80a20661d1366`

## References
- `docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md` (the commit that was refused)
- `docs/issues/2026-09-02-staging-is-not-a-state-you-can-hold.md`
