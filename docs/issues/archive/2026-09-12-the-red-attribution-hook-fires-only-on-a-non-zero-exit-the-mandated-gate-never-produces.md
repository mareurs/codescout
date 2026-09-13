---
id: 5e246c4bfa47fea3
kind: bug
status: fixed
title: 'BUG: the red-attribution hook fires only on a non-zero exit, which the gate shape CLAUDE.md mandates never produces'
tags:
- cluster/selector-narrower-than-its-population
claimed_at: 2026-09-13
claimed_by: 05841db2-4ba0-4cb2-a22f-c0bc2f771e20
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
- the explicit `run_in_background: true` form returns a handle before the process exits,
  so there is no exit status to hook on, at return or afterwards.

**A command's DURATION is not one of them, and saying so is load-bearing** — the obvious
inference is that a gate which always exceeds 120s is auto-backgrounded and therefore
beyond the hook's reach, which would make this defect far larger than it is. That is false
for the generic completion envelope, which carries `exit_code` and a full `wip_authors`
through (§ Evidence, row 2).

**The qualifier is not decoration.** The `cargo test` completion envelope is a different
renderer and is unmeasured (§ Evidence, row 3). If it turns out to drop the field, duration
is *still* not the cause — the renderer is — so the sentence above stays true while the
reason a reader would infer from it would be wrong. Do not let row 2 be read as covering
the gate's own envelope.

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

   **Denominator, published because it did NOT reproduce and absorbing it as a catch would
   make this population look self-correcting** (`f3c594ce`, 2026-09-12): an
   auto-backgrounded `cargo test --workspace` exiting 101 returned no `wip_authors`. The
   named path (`src/server.rs`) was clean at that instant, so the precondition above is a
   sufficient explanation — but it is not the only one available, because that run was also
   in the test-summary envelope (§ Evidence, row 3). **Two candidate causes, and the
   observation separates neither.** It is not a counter-example to row 2 and not a
   confirmation of it. Recorded because it is the only observation either session has of
   that cell, and because the author of the warning in this very bullet read his own
   silence as a contradiction for about a minute before applying it.

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

### Five invocation forms, all through `run_command`

| form | exit_code seen | `wip_authors` | mechanism |
|---|---|---|---|
| bare failing command | 101 | **present** | trigger works |
| auto-backgrounded, **generic** envelope | 101 (on completion) | **present** | notification carries the exit |
| auto-backgrounded, **test-summary** envelope | 101 (on completion) | **carried and rendered** | resolved by reading 2026-09-13 — see below |
| `<cmd> ; echo "… $?"` | 0 | **absent** | `echo` is the last statement |
| explicit `run_in_background: true` | *(none, ever)* | **absent** | returns before an exit exists |

**Row 2 is the one that changes how this bug reads, and it was nearly filed the other
way.** The natural inference is that a gate always exceeding 120s can never produce a
hook-visible exit, which would make the defect far larger than it is. That is false for the
generic envelope: auto-backgrounding preserves both the status and the attribution end to
end. **Duration is not a cause.** Measured 2026-09-12 by sessionId `b80a27d4`, who reports
reasoning to the opposite conclusion and checking before sending it. Row 1 was corroborated
independently by sessionId `8bd791df` on a bare `cargo test --workspace`.

**Row 3 exists because row 2 does not reach it, and neither measuring session said so at
first — both wrote "backgrounded" when they had measured one envelope.** Row 2's run was
driven with `sh -c`, whose completion result is the generic `{exit_code, stderr,
wip_authors}`. A `cargo test` completion arrives in a **different** envelope — `{type:
"test", exit_code, output_id, passed, failed, ignored, failures}` — which carries no
`stdout` and has no `wip_authors` field at all in any observation to date.

Which renderer you get is selected by **backgrounding, not by the command**: observed 4/4
in one session (`f3c594ce`, 2026-09-12), two foreground `cargo test` runs returned the
generic envelope — one of them with `wip_authors` present at exit 101 — while two
auto-backgrounded `cargo test --workspace` runs returned the test-summary envelope. So no
foreground run can probe row 3, which is why it is still open: the only path to that cell
is a backgrounded `cargo test` that genuinely fails **and** names a currently-dirty file,
and neither session has one.

Rows 4 and 5 are silent for **different** reasons and only one is fixable by changing the
trigger: row 4 has an exit status that is the wrong one, row 5 has none at all.


**Row 3 is closed by READING, not by a probe, and saying which matters because the row spent a
day open for want of one.** Both call sites were traced:

- `src/tools/run_command/output.rs` attaches `wip_authors` **after** the whole envelope
  if/else, so it is not envelope-specific — the `type: "test"` object built by
  `summarize_test_output` reaches that line like any other shape.
- `format_run_command` appends it **unconditionally across output shapes**, after all branch
  logic, with a comment at the site giving the reason: it must be rendered there *"or it
  reaches nobody"*. So the buffered `type: "test"` compact render carries it too.

The envelope was therefore never the cause, and the two non-discriminating observations that
left this row open are each fully explained by their own stated precondition — one had a real
exit of 0, the other named a clean path. **What remains unmeasured is that cell live**, and
closing it that way still means arming a red on a shared checkout, which § Resume rightly calls
an operator's decision. The point worth keeping: two sessions deferred this row waiting for a
probe neither could safely run, and the answer was two `if let Some` sites apart.
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

### A second live incident, same day, through the trailing `echo` alone

Reported by sessionId `b80a27d4` about twenty minutes after helping measure the table
above: `cargo test --workspace` in the mandated form, auto-backgrounded. `LEAN exit=0`,
`DEFAULT exit=101` sitting in stdout as text, a genuine red
(`no_class_field_states_a_bare_n`, over a bolded `n=` in an uncommitted `IC-5` edit), and
**no `wip_authors`**. Attribution was recovered by hand — `file-provenance.py` on the path
out of the failure text — reaching sessionId `8bd791df`, who fixed it in minutes.

Two things this instance carries that the first does not:

- **The red was in a file with no relationship to anything the reporter had touched.** They
  were working in `src/tools/symbol/symbols.rs`; the failure was in a tracker ledger. This
  is the case where adjacency yields *nothing* rather than a wrong name, and the cost is
  not only misattribution: a session with less context would reasonably conclude its own
  change caused the red and start bisecting its own work.
- **It fired through the trailing `echo` alone.** The reporter had assumed the `echo` and
  the backgrounding compounded. They do not — one of those paths is silent and the other is
  loud, which is why the two must not be described as one "background" ceiling.

Attribution recorded as the parties asked: the incident and the backgrounded generic-envelope
measurement are `b80a27d4`'s, the bare-foreground corroboration of row 1 is `8bd791df`'s. The
envelope split that turned four rows into five, and the denominator under § Reproduction, are
`f3c594ce`'s.
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

**Chosen: trigger on status OR output** (§ Fix option 2), implemented as a cheap in-process
pre-filter rather than by moving the decision into Python.

```rust
if exit_code == 0 && !names_a_diagnostic(red_text) {
    return None;
}
```

**Why not "spawn whenever there is output".** Stage 0's economics are real and were already
defended by a TIMING test — *"without it every green command in the session pays a process
spawn"*. `names_a_diagnostic` is an allocation-free scan (`contains` for `-->` and
`panicked at`, one `lines()` pass for line-initial `error` / `Error:`), so a green command
printing ordinary cargo chatter still short-circuits in microseconds.

**The pre-filter is deliberately WIDER than the engine and must never be narrower.** It checks
only the distinctive literal each `DIAGNOSTIC_PATH` pattern requires, not the span, filename or
line-number parts. A false positive costs one python spawn and the engine then answers
correctly; a false negative is the silent wrong answer this whole bug is about. If the two ever
disagree, they must disagree in that direction.

**The Rust/Python duplication is guarded, not trusted** — the shape `build.rs`'s hand-copied
`extract_surface` already cost this repo once. Two tests hold it: one fixture per engine pattern
asserting the pre-filter admits it, and a count of `re.compile(` entries **read out of
`scripts/attribute-red.py` at test time** — derived, never stored — so adding a sixth pattern
reds with a message naming the file to edit rather than silently narrowing coverage.

**Not fixed, and stated rather than glossed:** the explicit `run_in_background: true` form
(row 5) is still silent and no trigger change reaches it — it returns before an exit status or a
complete output exists. Separate change.

**CLAUDE.md § *Reaching a Peer Session* updated in the same commit**, because it was the half
that turned silence into a confident wrong answer. It now states the real trigger (non-zero
**or** failure-shaped output), says explicitly that the gate ends in `echo` and why that
mattered, and names **two** remaining ceilings instead of one — native `Bash`, and explicit
`run_in_background`.

Fix SHA: `02e61230`
Patch-id: `0165f0030711f26ba0158de5405c93af365ac73d`
## Tests added

Seven assertions in `src/tools/run_command/attribution.rs`, and the two that matter were each
observed RED by mutating the production line — not by being written and passing.

| mutation of the gate | test that reds | direction |
|---|---|---|
| back to `exit_code == 0` alone | `the_same_red_at_exit_zero_still_answers` | coverage |
| removed entirely (`i32::MIN`) | `a_zero_exit_with_nothing_failure_shaped_spawns_nothing` | cost, at **110ms** — a real spawn |

Each killed **exactly one** test, and a different one. That pins the condition from both sides
by behaviour, which the previous arrangement did not: `exit_code == 0` was guarded against
deletion only by a TIMING assertion, *"exactly the kind that gets relaxed on a loaded machine"*
in its own words.

- **`the_same_red_at_exit_zero_still_answers`** is the inverted twin of
  `the_same_red_at_exit_zero_says_nothing`, which **asserted this defect as correct**. Its
  docstring records that, so the next reader does not re-add it. It opens by establishing the
  engine answers at exit 101 on the same fixture — without that, an absent `python3` yields the
  same `None` as a deleted gate and the test would pass proving nothing.
- **`a_zero_exit_with_nothing_failure_shaped_spawns_nothing`** keeps the cost guard but its old
  fixture had to change: it was `"error: --> src/lib.rs:1:1"`, correct while the gate was
  `exit_code == 0` alone and a **false pass** the moment the gate learned to read the text — it
  would have asserted a spawn is skipped using the exact input that must now cause one. The
  fixture's non-diagnostic property is now itself asserted, so a later edit cannot re-break it
  quietly.
- **`the_prefilter_admits_every_shape_the_engine_matches`** — one fixture per `DIAGNOSTIC_PATH`
  pattern, plus the derived-count check described in § Fix.

All green: `attribution::tests` 9/9, `run_command::` 184/184, the six `claude_md` prompt-surface
tests 6/6 (read by name, since this commit edits CLAUDE.md).

**One verification this session could NOT do, stated because its absence is invisible:** the fix
is Rust, so it is not in the running MCP binary until a `cargo rb` + `/mcp`. Every measurement
above is the test suite and the source; **no wire probe of the fixed behaviour exists yet**. The
reproduction in § Reproduction was re-run against the *unfixed* running binary and reproduced
exactly — bare form `wip_authors` present, `; echo` form absent — which establishes the defect,
not the fix.
## Workarounds

Read `exit_code` **and** the echoed text. On the mandated gate the real lane statuses are
in stdout as `LEAN exit=N` / `DEFAULT exit=N`; a `0` in the envelope alongside a non-zero
there means the hook did not run and the silence carries no information. Then run the
manual route: `scripts/file-provenance.py` intersected with the socket enumeration.

## Resume

Fixed and archived. Rows 1, 2, 3 and 4 of § Evidence are closed; **row 5 is not** — an explicit
`run_in_background: true` returns before an exit status or a complete output exists, so no
trigger change reaches it. That is a separate change and nobody holds it.

**One thing this fix created, filed rather than left:** it adds a sixth concurrent caller of
`wip_author_diagnostic`, which widens a pre-existing race —
`docs/issues/2026-09-13-an-env-var-set-across-an-await-races-every-sibling-test-that-reads-it.md`.
A sibling test holds `CODESCOUT_NO_WIP_ATTRIBUTION` across an `.await` and every caller reads it.
It reds `a_red_attaches_wip_authors_on_the_main_arm` intermittently and did so once on 2026-09-13.
The repo's own ruling (`src/config/global.rs`) rules out `#[serial]` as the remedy, so it needs a
pure seam.

**Verified on the wire 2026-09-13 09:34, serving pid 3143840** (binary built 09:27:41, i.e. after
the fix commit `02e61230` at 09:24:37). Two paired observations, both at `exit_code: 0`:

| command shape | names a dirty path | `wip_authors` |
|---|---|---|
| `sh -c 'printf "error[E0425]…\n  --> src/librarian/tools/mod.rs:10:5\n"' ; echo "LEAN exit=$?"` | yes | **present** — named the live holder and its socket |
| `echo "touched src/librarian/tools/mod.rs and it was fine" ; echo "LEAN exit=$?"` | yes | **absent** |

The first is an output the pre-fix code **cannot** produce: stage 0 returned `None` unconditionally
at a zero exit. The second is the control, and it is the half that carries the weight — a
`names_a_diagnostic` stuck returning `true` produces the first row byte-for-byte, so the probe
alone proves the hook fires and says nothing about whether it still discriminates. Run them as a
pair or neither.

**What this does NOT establish, and the third item is the one that reads as covered:** row 5
(`run_in_background: true`) is untouched and still open; a green gate is not evidence about the
race below, which is a different instrument; and **this verification is scoped to `run_command`,
which is a CHANNEL, not the trigger.** Native `Bash` never enters the hook at all, so widening the
trigger from non-zero-exit to failure-shaped output cannot reach it — different axis, not a
residual. Do not read "the trigger now covers the mandated gate shape" as "gate runs now carry
attribution": a gate run through `Bash` carries none, and its silence means nothing.

That matters more than a footnote because `Bash` is **deliberately permitted while the
`security.shell_command_mode` eval is in flight** (CLAUDE.md § *Companion Plugin*), so the
uninstrumented channel is a live arm rather than an edge case, and any coverage figure taken
through `run_command` alone is scoped to one arm of a running experiment. Demonstrated the same
day: a peer ran the four-command gate through `Bash`, hit a red in this session's uncommitted
`index.rs`, received no `wip_authors` line, and resolved the authorship by hand — correctly, via
`file-provenance.py --all` plus `fmt-mine`, which is exactly the manual route § *Reaching a Peer
Session* keeps for this case. The ceiling was named in that section by this author while fixing the
trigger, and then not applied by the same author one session later when a peer reported the
silence; a stale-binary hypothesis was offered instead and refuted by the peer
(sessionId of `render-snapshot-from-params`, socket pid 924391).

**Build identity is a precondition of the probe, not a footnote — settle it FIRST.** The earlier
attempt measured a binary built at 09:00:17 against a change written at 09:08:59, so both probes
that day reproduced the defect *exactly* and established the bug rather than the fix. **`"the fix
is broken"` and `"the fix is not in this binary"` produce byte-identical observations.** An mtime
comparison separates them by inference; **`readlink /proc/$PPID/exe` run inside `run_command`
separates them by reading the serving process**, because that shell is a child of the MCP server
itself. It is the right instrument here because several builds run at once — 18 of 25 live
`codescout` processes were on `(deleted)` inodes at the time of this probe, each an older binary
still answering some session. A newest-file-on-disk check cannot see that; the `$PPID` read cannot
miss it.

(Copying the probe: backticks inside the inner double-quoted `printf` are substituted by `sh`, so
a backticked identifier vanishes from stdout and adds a `command not found` line on stderr.
Harmless — the `-->` still trips the filter — but escape them if you want a clean transcript.)
## References

- `scripts/attribute-red.py` — `DIAGNOSTIC_PATH`, `named_paths`, `dirty_paths`
- `CLAUDE.md` § *Development Commands* (the `;`-chained gate) and § *Reaching a Peer
  Session* (the hook and its one named ceiling)
- `docs/issues/2026-09-08-the-author-of-a-tree-reddening-write-is-the-one-party-never-told.md`
  — the problem the hook exists to solve
- Measured with sessionId `b80a27d4` (codescout-87), who supplied the independent
  controlled pair and both repro gotchas.
