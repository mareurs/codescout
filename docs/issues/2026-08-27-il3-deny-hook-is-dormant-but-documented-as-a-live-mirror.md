---
id: d20c47bf11899706
kind: bug
status: open
title: il3-deny-hook.sh is dormant, has its own test suite, and was documented as a live mirror to keep in sync
tags:
- il3
- companion-plugin
- dead-code
- cross-repo
---

---
kind: bug
status: open
closed:
unverified:
---

# `il3-deny-hook.sh` is dormant, has its own test suite, and was documented as a live mirror to keep in sync

**Found:** 2026-08-27, incidentally, while fixing
`docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md`.
**Affects:** `claude-plugins:codescout-companion/hooks/il3-deny-hook.sh` (the dead file);
`src/util/path_security.rs` (the stale instruction, already corrected in `18f8f9d1`).

## Summary

`detect_il3_violation`'s doc comment ended with *"Mirrors the regex in
`codescout-companion/hooks/il3-deny-hook.sh`"*, and `git_output_is_bounded`'s branch in
that hook carries *"keep the two in sync"*. The hook does not run.

## Evidence — two independent signals, deliberately of different kinds

**Registration.** Every `hooks.json` PreToolUse entry invokes `node`, i.e. a `.mjs`
file; `il3-deny-hook.sh` is bash. None of the 8 PreToolUse matchers targets
`run_command` at all — they cover `edit_code`, `edit_file`, `create_file`,
`read_file`, `Bash`, `Edit`, `Write`, `Agent`.

**Behaviour.** A command the two implementations classify differently settles it
without reference to any registry. The hook does not split on `;` / `&&`, so its
`PRE_PIPE` is everything before the *first* pipe in the whole command:

```
cargo --version; ls docs | head -3
```

- hook: `PRE_PIPE` = `cargo --version; ls docs`, head token `cargo` → **deny**
- server: segment 2 is `ls docs | head -3`, `ls` is bounded → **allow**

Measured: allowed, end to end. The hook did not fire.

A registration listing alone is a wiring inventory and can be wrong in both
directions; a single allowed command alone could have been allowed for some other
reason. Together they settle it.

## Why it matters

Not that enforcement is missing — the server-side guard is the one that should be
authoritative, and it is (`detect_il3_violation` is called pre-exec, covers every MCP
client, and reaches subagent contexts that Claude-Code-specific hooks miss). The harm is
**maintenance instructions pointing at a dead file**:

- Both surfaces told a reader to keep a second implementation in sync. Anyone obeying
  that would have spent effort on code that cannot execute — and, worse, might have
  *trusted* it as a second line of defence.
- The hook has its own 8.2 KB test suite (`il3-deny-hook.test.sh`). A green run there
  certifies logic nothing invokes. This is the self-validating-gate shape: a check that
  reads where the writer wrote.
- The hook's logic is now **stale** as well as dormant. It lacks segment splitting, the
  collapser rule, the field-selector exemption and the single-line-plumbing allowlist. If
  it were ever re-registered as-is, it would silently re-impose every false positive
  `18f8f9d1` removed.

## Environment

- Claude Code 2.1.247, codescout `experiments` @ `18f8f9d1`, measured 2026-08-27.

## Fix options

**Option A — delete the hook and its test.** Honest: the server-side guard is
authoritative and the hook cannot add coverage the server lacks. Costs the
defence-in-depth story, which was never real here since the hook was unregistered.

**Option B — re-register and re-sync.** Restores a pre-flight block that fires before
the tool call is made. Costs a permanent two-implementation sync burden in two repos,
which is exactly the burden that went unpaid and produced this bug.

**Option C (recommended) — delete, and record why in the server-side doc comment.**
`18f8f9d1` already did the second half: the doc comment now states the measurement
instead of the instruction. This option finishes it by removing the file and its test
from `claude-plugins`.

Not done here: the file lives in another repo, and deleting it is a separate,
reviewable change rather than a rider on a codescout fix.

## References

- `docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md`
  — the fix during which this was found; its § Fix records the same measurement
- `src/util/path_security.rs` — `detect_il3_violation`, whose doc comment now carries
  the correction

