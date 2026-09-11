---
id: ec568decb057b874
kind: bug
status: open
title: 'BUG: the progressive-disclosure guide says run_command stdout is never rewritten, in the section a reader consults to decide whether to trust it'
tags:
- cluster/doc-contradicted-by-code
---

## Summary

`get_guide("progressive-disclosure")` § *Path-relative annotation* ends with an unqualified
sentence: **"`run_command` output is raw shell bytes and is never rewritten."** That is true of
the mechanism the section describes (path relativization) and false of the response an agent
actually receives — `inject_notice` prepends `⚠ <notice>` and a blank line into `stdout` whenever
a workspace notice is live.

The guide is a SERVED surface that auto-injects the first time a call overflows into a buffer, so
the claim and its counterexample can reach an agent in the same session. They did in this one.

## Symptom (Effect)

Observed live 2026-09-11, on a checkout where a linked worktree exists and no project was
explicitly activated:

```
run_command("git log -2 --format='%h %cI %s'")
  -> stdout: "⚠ Reads are resolving against \"/home/marius/work/claude/codescout\". This repo
              also has linked git worktrees […]" + blank line + "f4c01d93 2026-09-11T17:24:12…"
```

The first line of `stdout` is not the command's output. Every `run_command` call in the session
carried it once the notice went live, including `echo` and `stat` calls whose output was being
read positionally.

## Reproduction

1. Have a linked git worktree present and no project explicitly activated — the condition
   `worktree_read_notice` fires on.
2. Make any `run_command` call that produces stdout.
3. Read `stdout`: it begins with the `⚠ Reads are resolving against …` advisory, a blank line,
   then the command's own bytes.

The control is in the same session: `get_guide("progressive-disclosure")` states the opposite,
and the guide body at `src/prompts/guides/progressive-disclosure.md` § *Path-relative annotation*
carries the sentence verbatim.

## Environment

`experiments`, 2026-09-11. Release binary rebuilt 17:57 +03:00; binary identity established
positively from a behaviour only the new build emits, not from an mtime.

## Root cause

**Two mechanisms write the same channel, and the section describes one of them.**

- Relativization is field-aware and allowlist-driven and genuinely does not touch `stdout`. The
  sentence is a correct claim *about relativization*.
- `inject_notice` (`src/tools/core/types.rs`) special-cases `run_command`, reading the `stdout`
  string out of the response object and reinserting it with the advisory and a blank line
  prefixed.

**The prepend is not the defect, and that is the load-bearing half of this file.** It is
deliberate, and `b3c0fe6ee49d9b57` cites it approvingly as the precedent whose general case was
never carried across — *"so the warning sits in the channel that is actually read"*. Anyone
reading this file should not go "fix" `inject_notice`; doing so would re-open the archived
`2026-08-17-worktree-reads-resolve-against-the-old-project`.

The defect is that a claim scoped to one mechanism is **phrased as a property of the channel**,
in the one section a reader consults to decide whether `stdout` can be trusted.

## Why this class and not its neighbour

`cluster/doc-contradicted-by-code`. The code is right and the doc is wrong, which is the class as
written.

Deliberately **not** `cluster/hint-composed-without-the-request`: that class is about guidance
composed without consulting the caller's request, and this sentence consults nothing at all — it
is a static claim that was accurate when the section was written about relativization and was
never re-scoped once a second writer of the same field landed.

## Impact

Bounded but real. An agent that trusts the sentence may hash, diff, or positionally parse
`stdout` and get a value contaminated by a leading advisory. The IL-3 refusal text already steers
agents toward a file redirect for whole-input work because a *buffer* can mislead; this is a
second way `stdout` can mislead, and the guide explicitly disclaims it. Severity is capped by the
advisory being conspicuous to a human reader and by the notice being gated on a worktree existing.

## Fix

Not attempted — filed on notice, per the capture-on-notice rule.

Smallest correct change is at the guide source, `src/prompts/guides/progressive-disclosure.md`
§ *Path-relative annotation*: name relativization as the sentence's scope, and name the one
mechanism that does rewrite `stdout` — rather than deleting the sentence. Its useful content is
that relativization will not corrupt a path literal in shell output, which is the archived
`be25f85bdc3777a3` regression it was written to close. A reader needs both facts, not neither.

Check `src/prompts/README.md` before editing: this is a prompt surface, and the 1900-character
slice cap may apply.

## Tests added

None. The gate that would catch this is not a unit test — no assertion compares guide prose
against runtime behaviour, and `prompt_surfaces_reference_only_real_tools` checks tool NAMES, not
claims. Worth stating rather than inventing: a test pinning this sentence would red on every
rewording, which this repo rejects for prose.

## Resume

Found during a post-rebuild reconnaissance whose actual subject was a different fix
(`1e11cf9357136e0e`). The notice had gone live mid-session because a linked worktree appeared, so
the same call shape returned different `stdout` before and after — and the guide asserting
byte-faithfulness had auto-injected earlier in that same session.
