---
id: b103f410811fe376
kind: bug
status: fixed
title: il3-deny-hook.sh is dormant, has its own test suite, and was documented as a live mirror to keep in sync
tags:
- il3
- companion-plugin
- dead-code
- cross-repo
- cluster/declared-not-wired
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


> **CORRECTION, 2026-08-27, before any fix was applied.** The framing above and the
> three fix options below were **wrong on their central claim**. This file asserted that
> the hook's dormancy was an oversight and that its "keep the two in sync" instruction
> was dead. Neither is true, and both were checkable in under a minute:
>
> - **The parking is deliberate and documented in two places.**
>   `claude-plugins:docs/trackers/version-bump-checklist.md:742` records that `50282a8`
>   downgraded the deny hook to warn-only **at the user's request** ("deny was
>   high-friction") and that the "Deny hook + its unit test [were] kept in-repo,
>   unwired, for re-promotion." codescout's own
>   `docs/architecture/companion-plugin.md:58` says the same: "kept for possible
>   re-promotion."
> - **The sync instruction was being honoured.** `bb85c55` and `5f6b336` both synced
>   this file to server-side changes *while it was already unwired*.
>
> So the deletion this file recommended would have destroyed a deliberate decision and
> a working practice. What is actually defective is narrower and the opposite in
> direction: **`18f8f9d1` broke the sync practice.** It changed the server's IL3
> semantics five ways without touching the mirror, so re-promoting the hook as it stood
> would have silently reimposed 19 of 703 measured refusals as false positives.
>
> This is the reconnaissance law that governs this whole repo, applied to this file's
> own text: *a proposed fix is a claim about CURRENT STATE — verify it before designing
> around it.* This file claimed "nobody decided this" and never looked. Being right in
> general about dead code is exactly what stops you checking whether it is true here.
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

**Not Option A, B or C.** All three rested on the mistaken premise corrected above.
The fix that shipped is the one the record actually prescribes: **sync the mirror**,
which is what `bb85c55` and `5f6b336` did before it.

Shipped in **`claude-plugins:88f1e29`**. Three of `18f8f9d1`'s five findings are
expressible in this hook's regex form and are now mirrored — field selectors out of
`DENY_PIPE`, a collapse-anywhere exemption, and the single-line git plumbing allowlist.
The codescout-side half (the doc comment that called this a live mirror without saying
it was unwired) shipped earlier, in `18f8f9d1` itself.

**Test suite 41 → 58, all green — and verified against the PRE-sync hook** in a temp
copy built from `git show HEAD:…`: exactly the **11** new ALLOW cases fail there, and
all **5** new DENY controls pass on both. Without that check the 17 new cases would
have been indistinguishable from 17 cases a permissive hook waves through.

**The remaining divergence is now stated in the file rather than carried silently.**
`il3-deny-hook.sh` opens with a `STATUS: PARKED AND UNWIRED` header naming when and why
it was parked, that syncing-while-dormant is the practice, and its one known gap: it
does **not** split on `;` / `&&`, so `PRE_PIPE` is everything before the first pipe in
the whole command. That gap predates `18f8f9d1` and is what a re-promotion must fix
first. `docs/architecture/companion-plugin.md` carries the same note.

**No version bump.** The file does not execute, so a bump would claim a behavioural
change that does not exist. Whoever re-promotes it bumps then, and the version-keyed
plugin cache picks up both changes together.

### Why this file exists at all, given it was wrong

Kept rather than deleted because the correction is the useful part. A dormant mirror
that silently rots is worse than no mirror — nothing fails, and the cost lands on
whoever re-promotes it, who is exactly the person least placed to notice. The header
and this record are what make that cost visible in advance.
## References

- `docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md`
  — the fix during which this was found; its § Fix records the same measurement
- `src/util/path_security.rs` — `detect_il3_violation`, whose doc comment now carries
  the correction
