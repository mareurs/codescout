---
id: 2995a0a648a4c4db
kind: bug
status: fixed
title: 'BUG: the progressive-disclosure guide says run_command stdout is never rewritten, in the section a reader consults to decide whether to trust it'
tags:
- cluster/doc-contradicted-by-code
claimed_at: 2026-09-12
claimed_by: 05841db2-4ba0-4cb2-a22f-c0bc2f771e20
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

Fixed on `experiments` — SHA `0787ca8107845232711dd97ddc5b2be2cd3dc456`, patch-id
`deb8947f5f74904bcc5b43f5b55d6f080dfcb2f8`.

`src/prompts/guides/progressive-disclosure.md` § *Path-relative annotation*. The sentence is
**scoped rather than deleted**, because its content closes the archived `be25f85bdc3777a3`
regression (relativization will not corrupt a path literal in shell output) and a reader needs both
facts, not neither:

- *"Relativization never rewrites `run_command` output"* — the true half, now carrying its scope.
- *"That is a claim about relativization, not a property of the channel"* — the missing qualifier.
- `inject_notice` named as the one mechanism that does write there, with the consequence spelled
  out: `stdout` is the command's own bytes, optionally preceded by a `⚠ …` advisory and a blank
  line, so do not hash, diff or positionally parse it without allowing for that.

**Deliberately NOT a change to `inject_notice`.** The prepend is correct, and `b3c0fe6ee49d9b57`
cites it approvingly as the precedent whose general case was never carried across; "fixing" it
would re-open `2026-08-17-worktree-reads-resolve-against-the-old-project`.

**Surface budget checked, not assumed.** `MAX_DECLARED_SECTION_BYTES` binds only `serves:`-declaring
sections, and `grep serves: src/prompts/guides/*.md` returns 13 matches, **all in `librarian.md`**.
This section declares none, so the cap does not reach it.
## Tests added

None, and the reason is unchanged from the filing: no assertion compares guide prose against runtime
behaviour, and a test pinning this sentence would red on every rewording — which this repo rejects
for prose. What stands in for it is verification **against the shipped artifact** rather than the
source, which for a compiled-in guide is the stronger check:

```
grep -c 'Relativization never rewrites'  target/release/codescout   -> 1
grep -c 'raw shell bytes and is'         target/release/codescout   -> 0
```

**Gate green — but on an ISOLATED WORKTREE, and that distinction is load-bearing.** `cargo test
--workspace` on the shared checkout could not be made green: four consecutive attempts hit four
DIFFERENT blockers from three sessions' uncommitted work (a bare count in a cluster ledger; a 5-arg
refactor with stale call sites; that refactor's own new test failing; a missing `#[derive(Debug)]`
in a third session's test file). None was this change and none was mine to repair. Retrying was not
converging — with several sessions editing Rust concurrently a clean whole-suite window narrows
rather than arrives.

So the lane ran on `git worktree add --detach … 9e09c3bc`, holding HEAD's committed state plus this
fix and, by construction, nobody's dirty files — with its own target dir, so the shared `target/`
was neither thrashed nor left holding a librarian-less binary. **Result: 5815 passed, 0 failed,
exit 0.** Read by NAME rather than by total:
`a_p50_session_stays_under_the_committed_emission_byte_ceiling` and
`declared_sections_are_within_the_size_cap` both ran — the two byte budgets this edit could trip,
and the two `src/prompts/README.md` notes print nothing on success — plus 103 `prompts::` tests.

**The control that makes the isolation a measurement rather than a dodge:**
`a_language_whose_server_never_answered_is_covered_wholesale`, the test that was red on the shared
tree, returns **0** occurrences in this lane. That zero is not a coverage gap; it is proof the
worktree excluded exactly the in-flight work it was meant to exclude. Earlier lanes on the shared
tree also passed for this change — clippy `--all-targets --features local-embed` clean, lean lane
3792 — but each was taken while peers held the tree dirty, so the worktree run is the citable one.
## Resume

Found during a post-rebuild reconnaissance whose actual subject was a different fix
(`1e11cf9357136e0e`). The notice had gone live mid-session because a linked worktree appeared, so
the same call shape returned different `stdout` before and after — and the guide asserting
byte-faithfulness had auto-injected earlier in that same session.
