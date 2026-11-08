---
id: d7b3d8a0d8756370
kind: bug
status: open
title: create_file refuses an existing path and discards its input, so the caller resends the whole payload with overwrite
tags:
- cluster/unclassified
opened: 2026-11-08
---

# BUG: create_file refuses an existing path and discards its input, so the caller resends the whole payload with overwrite

## Summary
`create_file` on an existing path refuses with `file already exists: <path>` and the hint "pass overwrite: true to replace". The refusal keeps nothing, so the caller sends the full `content` again with `overwrite: true`.

## Class fit
No roster class fits without forcing it. The refusal is a correct decision; the defect is that it discards the input it refused, so recovery costs a full resend. That is a cost of recovery, not a wrong gate decision, and the roster has no class for it.

## Symptom (Effect)
```
file already exists: /home/marius/.cache/cc-tmp/claude-1000/.../scratchpad/mutate.py
hint: Use edit_file to modify, or pass overwrite: true to replace. create_file is for new files only.
```

The text is built in `src/tools/create_file.rs` and normalised to the `target_already_exists` family by `src/usage/db.rs` (`msg.starts_with("file already exists:")`).

## Reproduction
1. `create_file(path=<existing file>, content=<large>)`. It refuses.
2. `create_file(path=<same>, content=<same large>, overwrite=true)`. The payload crosses the wire twice.

Not re-run for this file; the symptom is taken from `usage.db` rows.

## Environment
codescout on branch `experiments`, 2026-11-08. In this project's `usage.db` the sampled rows are scratchpad scripts, so overwrite is the usual intent.

## Root cause
`CreateFile::call` checks `!overwrite && resolved.exists()` and returns `RecoverableError::with_hint(..)` at `src/tools/create_file.rs:78-84` (read this session). It keeps no copy of the input. Inferred from the source; not measured at runtime.

## Evidence
Measured 2026-11-08 over every `usage.db` under `~/work` larger than 1 MB and modified after 2026-10-01, same method as the sibling files: failed write-tool calls with input over 1000 bytes whose next call of the same tool in the same session (within 6 calls) had an input length within 30% of the failed one.

| family | failed | resent | resent bytes |
|---|---|---|---|
| `target_already_exists` | 100 | 93 | 437,729 |

Limits: the match is by length, so an edited resend counts the same as an identical one, and an agent that correctly switched to `edit_file` with a similar-sized diff is counted too. Upper bound on avoidable bytes, not a measured waste.

## Hypotheses tried
1. **Hypothesis:** callers could use `edit_file` instead. **Verdict:** not tested. For a script rewritten from scratch the whole content is the change.

## Fix
Not done. Candidate: keep the input under an `@ack_*` handle and let `ack` replay it with `overwrite=true`. Two points differ from the read-only case. Overwrite destroys the existing file, so the refusal should state what would be lost (size and mtime of the existing file), and the replay must re-check that the file is unchanged since the refusal. And the replay is not a pin, so `lift_read_only_root` does not apply; the handle needs its own marker.

## Tests added
None. This file records the defect only.

## Workarounds
Resend with `overwrite: true`, or use `edit_file`.

## Resume
Start from `docs/issues/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md` and `src/server/read_only_ack_tests.rs` for the test shape.

## References
`src/tools/create_file.rs`; `src/usage/db.rs`; `src/tools/core/write_ack.rs`; `docs/issues/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`.
