---
id: '1afba4dac700e5d1'
kind: bug
status: open
title: 'BUG: the red-attribution hook fires only on a non-zero exit, which the gate shape CLAUDE.md mandates never produces'
tags:
- cluster/selector-narrower-than-its-population
---

# BUG: the red-attribution hook fires only on a non-zero exit, which the gate shape CLAUDE.md mandates never produces

## Summary

`run_command` runs `scripts/attribute-red.py` **on every non-zero exit** and attaches the
answer as `wip_authors`. CLAUDE.md § *Reaching a Peer Session* builds a standing
instruction on that: *"read the `wip_authors` line the failure already carries"* rather
than *"think to go looking"*, and names exactly one ceiling — native `Bash` bypasses
`run_command`.

There are two more ceilings, both **inside** `run_command`, and one of them is the shape
the same file mandates:

- a trailing `; echo "... $?"` makes the shell's exit status the `echo`'s, so a failing
  command reports `exit_code: 0` and the hook never runs;
- the `run_in_background` form returns a handle before the process exits, so there is no
  exit status to hook on at return time.

The four-command gate in § *Development Commands* is written as
`cargo test … ; echo "LEAN exit=$?" ; cargo test … ; echo "DEFAULT exit=$?"` — it **ends
in `echo`**. So the attribution is silent on the gate, which is the single most likely
place to meet a peer's red.

The cost is not a missing convenience. The instruction it supports tells the reader to
stop going looking, so an unnamed ceiling converts into a confident wrong answer.

## Symptom (Effect)

Same red text, same dirty file, same stream. Only the trailing `echo` differs:

```
A)  sh -c '<red naming a dirty file>; exit 101'
    -> exit_code: 101   wip_authors: PRESENT

B)  sh -c '<same red>' ; echo "LEAN exit=$?"
    -> exit_code: 0     stdout: "LEAN exit=101"     wip_authors: ABSENT
```

In B the `101` is sitting in stdout **as text**. A reader has the full failure in front of
them and no attribution beside it, in a form indistinguishable from "no peer is involved".

## Reproduction

Measured 2026-09-12, foreground `run_command`, against a genuinely dirty path
(`scripts/architecture-boundary-probe.py`, untracked and owned by another session):

```
run_command("sh -c 'printf \"error[E0425]: ...\\n  --> scripts/architecture-boundary-probe.py:3:1\\n\" >&2; exit 101'")
  -> exit_code 101, wip_authors PRESENT, naming the path

run_command("sh -c '<identical>' ; echo \"LEAN exit=$?\"")
  -> exit_code 0, stdout "LEAN exit=101", wip_authors ABSENT
```

Minimal form of the same mechanism, no diagnostic needed:

```
run_command("false ; echo \"inner status was $?\"")  -> exit_code: 0   (stdout: "inner status was 1")
run_command("false")                                  -> exit_code: 1
```

**Two things a re-runner needs, each of which cost a probe to learn.**

1. **The `-->` must start its own line.** `DIAGNOSTIC_PATH`'s rustc-span pattern is
   `^\s*-->\s+([^\s:]+):\d+:\d+` with `re.M` — indented is fine, same-line is not. A
   synthetic repro written as `error: ... --> path:3:1` on ONE line produces no
   `wip_authors` and looks like a second defect. It is not one; it is cargo's real format
   and a parser matching it. (Cost three probes to a peer session before they spotted it.)
2. **The repro has a dirty-file precondition that decays continuously.** The named path
   must be uncommitted *at the moment of the run* (`dirty_paths` reads
   `git status --porcelain`). A path that has since landed produces a correct silence,
   which reads exactly like the bug. One probe was burned naming a file committed minutes
   earlier. Re-derive a currently-dirty path before concluding anything.

## Environment

codescout 0.15.0, branch `experiments`, Linux. Both feature lanes — nothing here is
feature-gated. Foreground `run_command` via MCP.

## Root cause

`run_command`'s trigger is the **process exit status of the shell it spawns**, which is a
proxy for *"did this run contain a failure"*. The proxy is exact for a bare command and
wrong for any pipeline whose last statement succeeds.

`;` sequencing makes the last statement's status the pipeline's. The gate's trailing
`echo` always succeeds. So the mandated shape is precisely the shape that defeats the
trigger — and the two rules are in **direct tension without either text knowing about the
other**:

- § *Development Commands*: *"Chain the two test lanes with `;`, never `&&`"* — and for a
  good reason that has nothing to do with exits: `&&` would skip the default-lane rebuild
  that clears the librarian-less-binary trap for the next session.
- § *Reaching a Peer Session*: the hook runs *"on every non-zero exit"*.

Neither is wrong. Their intersection is empty for the command both describe.

The `run_in_background` form fails for a **different** reason rather than the same one: the
call returns an `output_id` as soon as the process is detached, so at return time no exit
status exists yet to trigger on.

## Evidence

### Three invocation forms, all through `run_command`

| form | exit_code seen | `wip_authors` | mechanism |
|---|---|---|---|
| bare failing command | 101 | **present** | trigger works |
| `<cmd> ; echo "… $?"` | 0 | **absent** | `echo` is the last statement |
| `run_in_background: true` | *(none at return)* | **absent** | returns before the exit exists |

### The live incident this class produced, earlier the same day

A compile red (`E0425` in `display.rs`, plus a warning in `symbols.rs`) was hit by a
`run_command` call written in the trailing-`echo` form. No `wip_authors` was attached. With
all three error lines and both paths on screen and no attribution beside them, the reporter
routed the report **by adjacency** — to the session that had most recently announced work
in `symbols.rs` — which is the one move CLAUDE.md's § *Never route by adjacency* forbids,
and it named the wrong owner. `file-provenance.py` later put `display.rs` on a third
session.

That is the shape worth keeping: the instruction *"read the line the failure carries"*
is correct, and silently inapplicable, and the failure mode is a **confident** wrong
answer rather than a missing one.

### The extractor itself is not implicated

`named_paths` is deliberately conservative — it requires a cargo-shaped span, a panic site,
or an `error:` line naming a `.rs`, with a comment at the site stating that a token which
merely *appears* is not a token the red is about. A peer independently confirmed
`attribute-red.py` correctly splits a two-file, two-author red, returning both files each
with its own author and socket, and retracted an earlier claim that it could not
(`452e55e6`). **The instrument is fine; the invocation the docs require prevents it
running.**

## Hypotheses tried

1. **Hypothesis:** the attribution was computed and lost when summarised into a
   cross-session message. **Test:** re-read the original tool response — no `wip_authors`
   key at all, while two earlier responses in the same session (both `exit_code: 101`)
   each carried one. **Verdict:** rejected; nothing was dropped in the relay.
2. **Hypothesis:** the call used native `Bash`, the one ceiling CLAUDE.md names.
   **Test:** the call was `mcp__codescout__run_command`. **Verdict:** rejected.
3. **Hypothesis:** `attribute-red.py` cannot attribute a red naming two files with
   different authors. **Test:** run by a peer against a two-author red. **Verdict:**
   rejected and retracted by them — it returns both, each with its socket.

## Fix

Not started. The trigger is one line; the decision is which signal replaces it.

- **Trigger on the OUTPUT, not the status** — run the hook when the combined output
  contains a `DIAGNOSTIC_PATH` match whose file is dirty, regardless of exit code. Covers
  all three forms including background. The extractor is already conservative enough to
  make false positives unlikely, and a spurious `wip_authors` is cheap next to a silent
  one. Cost: the hook runs on some successful commands that merely printed a diagnostic.
- **Trigger on status OR output** — narrower version of the above, keeping today's
  behaviour and adding the output path only when `exit_code == 0`.
- **Attach on background completion** — separate change, needed for the third form
  whichever of the above lands: run the hook when the detached process exits and attach to
  the buffer.
- **Name the ceilings in CLAUDE.md** — the minimum, and it does not fix the silence. If
  only this ships, § *Reaching a Peer Session* must stop saying the standing instruction
  *replaces* going looking, because for the gate it does not.

## Tests added

None yet. A regression test is writable without a server and should assert the pair, not
one arm: a failing command whose output names a dirty path must attach `wip_authors` in
**both** the bare and the trailing-`echo` forms. Asserting only the bare form re-passes on
today's code and proves nothing.

## Workarounds

Read `exit_code` **and** the echoed text. On the mandated gate the real lane statuses are
in stdout as `LEAN exit=N` / `DEFAULT exit=N`; a `0` in the envelope alongside a non-zero
there means the hook did not run and the silence carries no information. Then run the
manual route: `scripts/file-provenance.py` intersected with the socket enumeration.

## Resume

Pick a trigger from § Fix. Before implementing, re-derive the three-form table above
against a **currently** dirty path — the precondition decays, and a stale path yields a
correct silence that reads as the bug.

One sub-claim is deliberately **not** established and should not be inherited: for the
background form I measured only that the immediate return carries no `exit_code` and no
`wip_authors`, and that reading the buffer afterwards reports the *reading* command's exit.
Whether a completion notification for a genuinely failing background job could carry
`wip_authors` was not observed — the one backgrounded gate run seen this session had a real
exit of 0 (trailing `echo`), so it discriminates nothing.

## References

- `scripts/attribute-red.py` — `DIAGNOSTIC_PATH`, `named_paths`, `dirty_paths`
- `CLAUDE.md` § *Development Commands* (the `;`-chained gate) and § *Reaching a Peer
  Session* (the hook and its one named ceiling)
- `docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`
  — the problem the hook exists to solve
- Measured with sessionId `b80a27d4` (codescout-87), who supplied the independent
  controlled pair and both repro gotchas.
