---
kind: bug
status: open
tags:
- cluster/unclassified
- fs
- windows
- edit-tools
closed: null
opened: 2026-08-14
owner: marius
related: []
severity: medium
---

# BUG: `atomic_write`'s temp-file-then-rename pattern is not detected by dev-server file watchers (uvicorn `--reload` / watchfiles) on Windows

## Summary
codescout's file writes (`edit_code`, `edit_file`, `create_file`, etc.) go
through `atomic_write()` (`src/util/fs.rs:62-77`): write to `<path>.tmp`, then
`std::fs::rename()` over the target. At least one downstream dev workflow —
`uvicorn --reload` (watchfiles backend) on Windows — never sees a change
notification for files edited this way, so the running server keeps serving
stale pre-edit code after a codescout edit, silently.

## Symptom (Effect)
Reported from `Mercury MRP Automation` (`docs/trackers/web-ui-fixes-session-log.md`
F-1, 2026-06-09, status `mitigated` in that repo): after editing `.py` files
(`template.py`, `main.py`, `followup.py`) via codescout `edit_code`/`edit_file`
while `scripts/dev.sh` ran `uvicorn --reload` + a Next.js dev server, **no**
`WatchFiles detected changes…` log line ever appeared for those edits. Only
files created/deleted via native `Write` or PowerShell ever triggered a
reload. A stale worker (observed PID 19616) kept serving pre-edit code on
port 8000 through three restart attempts, compounded by Windows `netstat`
reporting the listening port's owner as an already-dead PID, which broke
naive "kill by port" restart scripts.

## Reproduction
Not yet reproduced inside codescout's own repo/tooling — this bug file
captures an externally-observed, plausible-but-unconfirmed root cause so it
isn't lost. To reproduce directly:
1. In a Python project with `uvicorn --reload` (watchfiles backend) running
   on Windows, edit a watched `.py` file via codescout `edit_code` or
   `edit_file`.
2. Check whether the uvicorn/watchfiles process logs a change-detected line
   and actually reloads.
3. Compare against editing the same file with a plain in-place write (no
   temp-file+rename) and see if the watcher fires then.

## Environment
Windows 11. Downstream tool: `uvicorn --reload` using the `watchfiles`
library. codescout writes routed through `atomic_write` (`src/util/fs.rs`).
Reported from `Mercury MRP Automation`, a sibling project, not from
codescout's own codebase.

## Root cause
Unknown — inferred from `src/util/fs.rs:62-77`, not measured against
`watchfiles`' actual notification backend. `atomic_write` writes to a
sibling `.tmp` path (`std::fs::write(&tmp, content)`) then
`std::fs::rename(&tmp, path)`. On Windows, some file-watch backends
(ReadDirectoryChangesW-based, depending on library defaults) key change
detection off the target path's own write/modify event and can miss a
rename-over-target, since the final inode/handle history differs from an
in-place write. `watchfiles`' Rust `notify` backend specifically has known
platform-dependent behavior around rename vs. write events — not confirmed
here to be the same code path, only plausible.

## Evidence
### Mercury MRP Automation session log (2026-06-09)
`docs/trackers/web-ui-fixes-session-log.md` F-1 (external repo, cited for
cross-reference — see that file for the full incident writeup, not re-quoted
verbatim here).

## Hypotheses tried
1. **Hypothesis:** `atomic_write`'s rename-over-target is invisible to
   watchfiles' Windows notification backend, while in-place writes and
   create/delete are visible.
   **Test:** Only informal — observed in the MRP session that native
   `Write`/PowerShell edits reliably triggered reload and codescout edits
   never did, across multiple files in the same session.
   **Verdict:** deferred — plausible, not confirmed by an isolated
   controlled test (no A/B toggle of codescout's write path was run).
   **Evidence link:** Mercury MRP Automation Evidence section above.

## Fix
Not planned — root cause unconfirmed. If confirmed, options include: (a)
touch/set the target's mtime explicitly after rename in case some watchers
key off mtime rather than the rename event, (b) document this as a known
interaction for any dev-server-with-hot-reload workflow rather than change
`atomic_write`'s crash-safety property, since the temp-file+rename pattern
itself is deliberate (see `atomic_write`'s own doc comment).

## Tests added
N/A — root cause unconfirmed, no fix implemented.

## Workarounds
Full backend restart after every codescout edit when running a hot-reload
dev server on Windows, until this is confirmed/resolved (workaround
documented independently in the MRP project).

## Resume
Set up a minimal Windows `uvicorn --reload` (or a bare `watchfiles` Python
script) test harness, edit a watched file once via codescout `edit_file`
and once via a plain native write, and diff whether the watcher fires for
each. That single controlled test would confirm or reject the Hypothesis
above without needing the full MRP project.

## References
- Sibling-repo finding: `Mercury MRP Automation` `docs/trackers/web-ui-fixes-session-log.md` F-1 (2026-06-09)
- `src/util/fs.rs:62-77` — `atomic_write`
