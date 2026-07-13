---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- error-handling
- recoverable-error
- path-security
- doc-drift
topic: null
time_scope: null
closed: '2026-07-13'
opened: '2026-07-10'
owner: marius
related: []
severity: high
---

# BUG: path_security.rs module doc promises RecoverableError / isError:false, but every validator uses anyhow::bail! (hard failure)

## Summary
`src/util/path_security.rs`'s module doc (§ Agent Safety, lines 28-33) states: *"Violations return `anyhow::Error` wrapping a `crate::tools::RecoverableError`, which the MCP layer surfaces as `isError: false` … does not abort sibling parallel tool calls."* The implementation contradicts it: `validate_read_path`, `validate_write_path`, and `validate_approve_path` raise every violation via plain `anyhow::bail!`, which `route_tool_error` (`src/server.rs:1046`) routes to `isError: true`.

## Symptom (Effect)
An agent hitting a protected path, an out-of-root write, or an empty path gets a hard failure that aborts sibling parallel tool calls — even though the messages are written as corrective guidance (e.g. `"write denied: '<path>' is outside the project root. Call approve_write('<dir>') first…"`).

## Reproduction
1. `edit_file(path="/etc/hosts", …)` or any write outside the project root without approval.
2. Observe `isError: true` on the response instead of the documented soft envelope.

## Environment
codescout MCP server, branch `experiments`, 2026-07-10. All transports.

## Root cause
Doc-vs-code drift. `bail!` sites verified at `src/util/path_security.rs:216, 219, 229, 240, 248, 377-382, 399, 402, 418, 427, 489-496`; callers (`src/tools/read_file.rs`, `src/fs/mod.rs`, `src/tools/tree.rs`, `src/tools/grep.rs`, `src/tools/markdown/read_markdown.rs`, `src/tools/ast.rs`, `src/tools/symbol/edit_code.rs`) all propagate via bare `?` with no conversion. Either the doc was written for an intended-but-never-shipped conversion, or a refactor dropped it.

## Evidence
- Module doc lines 28-33 + all bail! sites read directly this session (grep with context over `src/util/path_security.rs`).
- Found by subagent B3 (recon arm, doc-vs-code compare) in the 2026-07-10 3×3 bug-hunt experiment; re-verified at the bytes by the main agent.

## Hypotheses tried
1. **Hypothesis:** a caller converts bail! errors to RecoverableError before routing. **Test:** B3 traced all callers listed above; bare `?` throughout. **Verdict:** confirmed drift.

## Fix

**Shipped on `experiments` in `2fbcff80`** (`docs(path_security): correct module doc to match hard-fail behavior`). Archive after cherry-pick to `master`.

Resolution: **fix the doc**, not the code. Investigation showed the hard-`bail!` (isError:true) behavior is intentional and pinned by `validate_write_path_still_bails_outside_with_unchanged_message` — a path/security-boundary breach should fail loudly rather than be silently absorbed by sibling parallel calls (a deliberate exception to the input-driven→RecoverableError convention). The module doc (which wrongly promised RecoverableError/isError:false) was the stale side and is now corrected. No behavior change.
## Tests added

None — doc-only change. The intended hard-fail behavior is already guarded by `validate_write_path_still_bails_outside_with_unchanged_message` and the `validate_approve_path_rejects_*` / `classify_*` tests (125 `util::path_security` tests pass, unchanged).
## Workarounds
None needed for correctness; agents should treat these hard failures as correctable despite `isError:true`.

## Resume
Pick (a) or (b); if (a), convert the ~11 bail! sites (list in Root cause) and re-run the path-security test suite; add the end-to-end routing regression test.

## References
- `docs/issues/2026-07-10-librarian-recoverable-error-downcast-never-matches.md` — sibling error-routing bug found the same session.
- Experiment provenance: session 5efbda5f, agent "B3 recon: error contract bugs".
