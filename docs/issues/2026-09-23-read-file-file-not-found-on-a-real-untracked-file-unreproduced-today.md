---
id: '8cb280efc3a94a8a'
kind: bug
status: zombie
title: 'BUG: read_file reportedly returned "file not found" on a real, untracked-but-not-ignored file — verified today it does not reproduce'
owners:
- marius
tags:
- cluster/unclassified
topic: read_file path resolution vs. git-tracked state
last_observed: 2026-09-23
unverified: root cause not established -- could not reproduce; original incident not independently corroborated beyond the reporting session's own transcript
---

## Summary

A prior session reported that `read_file` failed with `"file not found"` on a
file that demonstrably existed on disk, was untracked by git, and was **not**
gitignored, in the sibling repo `/home/marius/work/claude/claude-plugins`. A
fresh reproduction attempt today, against the currently-running codescout MCP
server and this repo's current `src/` (HEAD `1a8cfcfe1c84c366fcbe26435f2462b4edd91536`),
**does not reproduce** the failure — `read_file` succeeded on freshly created
untracked, non-ignored files at repo root in both markdown and plain-text
form. Filed as `zombie` per this repo's bug-tracking convention (observed
once, not currently reproducible, root cause not confirmed) rather than
closed outright, because the original report was specific and detailed
(exact error string, exact byte count, exact git-status output) and I cannot
rule out an environment/version/timing difference between that session and
this one.

## Symptom (Effect)

Original report (relayed, not independently witnessed by this session):
`read_file(path="TEMP-codescout-routing-handoff.md")`, run from a session
with `claude-plugins` as the active project, returned:

```
{"ok": false, "error": "file not found: 'TEMP-codescout-routing-handoff.md' (searched /home/marius/work/claude/claude-plugins/TEMP-codescout-routing-handoff.md)"}
```

even though `ls -la` and `cat` on that exact path succeeded (5573 bytes, real
content), `git check-ignore -v` returned nothing (not ignored), and
`git status --porcelain` showed `?? TEMP-codescout-routing-handoff.md`
(untracked, not ignored). The original file has since been deleted (a
consumed handoff note), so the exact original file could not be re-read —
only a fresh, equivalent file could be tested.

## Reproduction

**Verified today, live, against the running MCP server:**

1. Created `TEMP-bug-repro-read-file-untracked.md` at the root of
   `/home/marius/work/claude/claude-plugins` via `create_file` (real file,
   135 bytes, markdown).
2. Confirmed via `run_command`:
   - `ls -la TEMP-bug-repro-read-file-untracked.md` → real file, 135 bytes.
   - `cat TEMP-bug-repro-read-file-untracked.md` → real content.
   - `git status --porcelain -- TEMP-bug-repro-read-file-untracked.md` →
     `?? TEMP-bug-repro-read-file-untracked.md` (untracked).
   - `git check-ignore -v TEMP-bug-repro-read-file-untracked.md` → exit 1,
     no output (**not** gitignored).
3. `read_file(path="TEMP-bug-repro-read-file-untracked.md", workspace="/home/marius/work/claude/claude-plugins")`
   → **succeeded**, returned the file's real content (markdown route).
4. Repeated with a non-markdown file, `TEMP-bug-repro-plain.txt` (plain text,
   1 line) — same setup (real, untracked, not ignored), same result:
   `read_file` **succeeded** (raw-file route, not the markdown route).
5. Both scratch files deleted afterward (`rm`, acknowledged via `@ack_*`);
   `git status --porcelain` confirmed clean.

**Conclusion: the specific failure mode described in the original report did
not reproduce today**, for either a markdown or a plain-text untracked,
non-ignored file at repo root, in the same project (`claude-plugins`) the
original report named.

## Environment

- codescout repo HEAD at time of this investigation:
  `1a8cfcfe1c84c366fcbe26435f2462b4edd91536` (2026-09-23,
  "docs(issues): formalise 22 fix anchors, lifting pairs the records already
  stated").
- The `codescout` binary is not on `$PATH` in this shell (`which codescout`
  fails), so the MCP server actually answering these calls is launched some
  other way (e.g. a fixed binary path in MCP config); **its exact build/version
  was not independently confirmed** — this is the main gap in ruling out "already
  fixed between the original report and now" vs. "never actually reproducible
  as described".
- Target project for the repro: `/home/marius/work/claude/claude-plugins`
  (same project the original report named).
- Original report's exact session/build context is unknown to this
  investigation — relayed via task instructions, not independently witnessed.

## Root cause

**Not established — reproduction failed, so there was nothing to root-cause
today.** Code inspection lead (unverified against the failure — the failure
never occurred to verify it against):

- The error string in the original report
  (`file not found: '<path>' (searched <resolved>)`) matches
  `read_file_text` in `src/tools/read_file.rs:573-594` verbatim (and an
  identical sibling in `src/tools/markdown/read_markdown.rs:76` for the
  markdown route).
- The path that reaches `read_file_text` is produced by
  `crate::util::path_security::validate_read_path`
  (`src/util/path_security.rs:283-325`). Read in full today: it joins the raw
  path onto `project_root`, canonicalizes it (`best_effort_canonicalize`,
  resolving symlinks/`..`), checks it against a deny-list, and returns the
  resolved `PathBuf`. **Nothing in this function consults git status, a
  tracked-files list, or any index** — it is plain filesystem path
  arithmetic. `read_file_text` then does a plain `std::fs::read_to_string`.
  Neither function has any git-awareness, so on today's code there is no
  mechanism by which "untracked" as such could cause a false "file not
  found" — which is consistent with the failed reproduction.
- A different, already-known mechanism produces the *same error string* for
  a different reason: `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
  and `docs/trackers/bug-fix-session-log.md:7199` both cite a prior incident
  (fix `76e287f8`) where `read_file` → `file not found` because **a subagent
  had reassigned `default_workspace_root`**, so the tree actually searched
  was not the one the caller believed was active — nothing to do with git
  tracking, everything to do with which project root was in effect. This is
  a **lead, not a confirmed explanation** of the original report: I have no
  evidence the original session had a multi-workspace/subagent root mismatch
  in play, only that this is a mechanism proven capable of producing the
  identical error string for a non-git reason, and that no other mechanism
  in the current code produces that string for an untracked file specifically.

## Evidence

- `src/tools/read_file.rs:577` and `:594` (`read_file_text`) — error string
  source, read in full.
- `src/tools/markdown/read_markdown.rs:76` — identical error string on the
  markdown route.
- `src/util/path_security.rs:283-325` (`validate_read_path`) — read in full;
  no git-status/tracked-file check present.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md:42` and
  `docs/trackers/bug-fix-session-log.md:7199` — prior, confirmed, *different*
  mechanism (`default_workspace_root` reassignment by a subagent) producing
  the same error string.
- Live repro transcript today: `create_file` → `ls -la` / `cat` / `git status
  --porcelain` / `git check-ignore -v` → `read_file` succeeding, for both a
  `.md` and a `.txt` untracked, non-ignored file at
  `/home/marius/work/claude/claude-plugins` root.
- Searched the bug ledger first
  (`doc(action="find", kind="bug", filter={"status":{"in":["open","taken","investigating","zombie"]}})`
  and title-contains searches for "read_file" / "untracked" / "file not
  found") — no existing open bug matches this exact symptom. Related but
  distinct: `4a154d7effff259b` (open, unclaimed — delete-preview calls an
  untracked file git-restorable) and the archived
  `79b0a012716ff390` (committed-scripts gate scans the filesystem so an
  untracked file reds it for everyone) are different subsystems (delete
  preview / commit gate, not `read_file`).

## Hypotheses tried

1. **Plain untracked file at repo root, markdown.** Did not reproduce —
   `read_file` succeeded.
2. **Plain untracked file at repo root, non-markdown (`.txt`).** Did not
   reproduce — `read_file` succeeded (rules out the markdown-routing branch
   specifically as the site of the bug).
3. **Code-level: does `validate_read_path` or `read_file_text` consult git
   state?** No — both read in full; purely filesystem-based path resolution
   and `std::fs::read_to_string`. Rules out an untracked-file blind spot in
   the *current* code, but does not establish what the original session's
   build did.

Not tried (out of scope / not reproducible without more information):
subagent-induced `default_workspace_root` reassignment concurrent with the
original `read_file` call, which is the one known mechanism that produces
the identical error string for a non-git reason.

## Fix

N/A — no fix; the defect did not reproduce, so there is nothing to fix
against today's code. If this recurs, the priority repro variant is a
concurrent multi-workspace/subagent session (per the `76e287f8` lead above),
not a single-session untracked-file test (already ruled out here).

## Tests added

None — no reproducible failure to write a regression test against. Adding a
test asserting "read_file succeeds on an untracked, non-ignored file" would
only guard the hypothesis already ruled out (a plain git-status dependency),
not the unconfirmed multi-workspace lead.

## Workarounds

None needed today (no reproduced failure). If it recurs: per the existing
hint text on this exact error
(`src/tools/read_file.rs`'s `RecoverableError::with_hint`), call
`workspace(action="status")` to check whether a subagent sharing the session
process has changed the active project root.

## Resume

If this bug recurs:

1. Capture the **exact** `read_file` call (path, `workspace` param if any)
   and the **exact** error payload, verbatim, before the file or workspace
   state changes.
2. Immediately run `workspace(action="status")` in the same
   session/subagent to capture the active project root at the moment of
   failure — this is the one piece of evidence today's investigation could
   not gather retroactively, and it is exactly what would confirm or refute
   the `default_workspace_root`-reassignment lead.
3. Note whether the failing call ran in a subagent, and whether any sibling
   subagent/session had recently called `workspace(action="activate")` with
   a different path.
4. If confirmed as a plain untracked-file issue (not a workspace-root issue),
   re-open this file and change status from `zombie`; otherwise this can stay
   `zombie` or move to `wontfix` citing the workspace-root explanation
   instead.

## References

- `src/tools/read_file.rs:573-594` (`read_file_text`)
- `src/tools/markdown/read_markdown.rs:76` (`resolve_markdown_source`)
- `src/util/path_security.rs:283-325` (`validate_read_path`)
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` (row citing fix `76e287f8`)
- `docs/trackers/bug-fix-session-log.md:7199`
- Related, distinct bugs found via ledger search: `4a154d7effff259b` (open),
  `79b0a012716ff390` (archived, fixed)
