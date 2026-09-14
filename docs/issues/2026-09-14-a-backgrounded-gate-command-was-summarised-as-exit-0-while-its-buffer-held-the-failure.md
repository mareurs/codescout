---
id: '60ad8dda4d0b7ec3'
kind: bug
status: open
title: 'BUG: a backgrounded gate command was summarised as exit 0 while the buffer it pointed at held the compile failure'
tags:
- cluster/unclassified
- run-command
- progressive-disclosure
- shared-checkout
- observability
---

## Summary

A `run_command` call made with `run_in_background: true` returned a response whose summary
read `✓ exit 0`, for a `cargo clippy` invocation that had **failed to compile**. The real
failure was present in the referenced buffer the whole time. The caller — this session — read
the summary, treated the gate as green, and moved on with a truncated source file in the tree
that was redding the shared build for every other session.

Three controlled variants failed to reproduce it. The observation is recorded verbatim because
it is a *false green on a gate command*, which is the most expensive direction this class can
fail in, and because the same session's later foreground run of the identical command reported
the failure correctly.

## Symptom (Effect)

The call:

```
run_command(
  command: "cargo clippy --workspace --all-targets --features local-embed -- -D warnings 2>&1",
  run_in_background: true,
)
```

The response, verbatim:

```json
{
  "output_id": "@tool_9ffd17c9",
  "summary": "✓ exit 0  (query @bg_00000001)",
  "buffered_bytes": 363348
}
```

The referenced buffer, read later in the same session:

```
error: this file contains an unclosed delimiter
    --> src/util/path_security.rs:2919:90559
error: could not compile `codescout` (lib) due to 1 previous error; 1 warning emitted
error: could not compile `codescout` (lib test) due to 1 previous error
```

`cargo` exits `101` on that. The summary said `✓ exit 0`.

## Reproduction

**Not reproduced.** Three variants, all `run_in_background: true`, all in the same session
minutes afterward — every one returned `"hint": "Process running. Output captured in @bg_…"`
with **no exit claim at all**, which is the documented and correct shape:

| variant | command | response |
|---|---|---|
| slow fail | `echo starting; sleep 3; echo failing >&2; exit 7` | `Process running`, no exit claim |
| fast fail | `echo out; echo err >&2; exit 7` | `Process running`, no exit claim |
| large output + fail | `seq 1 80000; echo boom >&2; exit 7` | `Process running`, no exit claim |
| cargo, cold compile, type error | `cargo build` in a throwaway crate | `Process running`, errors inline in `stdout`, no exit claim |
| cargo, cached, fails instantly | same crate, second run | `Process running`, errors inline in `stdout`, no exit claim |

The last two are sessionId `9403d62d`'s, run at this file's request against the probe
§ *Resume* originally named — deliberately through codescout's `run_command` rather than the
native `Bash` backgrounding they normally use, so the two of us were exercising the same path.
**Five non-reproductions across two sessions and two backgrounding habits.**

The third was chosen to test the one variable visibly different about the clippy call — its
response was an **overflow envelope** (`output_id` + `summary` + `buffered_bytes`, 363 KB)
rather than the `Process running` shape. Output size alone did not produce it.

So the discriminating variable is **not** identified. What distinguishes the clippy call and
is not yet ruled out: it was a `cargo` invocation (the tool type-classifies these — the later
foreground run came back `"type": "build"`), it carried `2>&1`, and it ran while an MCP server
restart was imminent.

## Environment

`experiments`, 2026-09-14, live MCP over stdio. The same command run in the **foreground**
minutes later, after the tree was repaired, reported `"type": "build", "exit_code": 0`
correctly — and a foreground `cargo test` chain in the same session correctly surfaced
`exit 101` shapes elsewhere. The defect is not "this session cannot read exit codes".

## Root cause

**Not established, and deliberately left open.** Naming one without a reproduction would close
the inquiry — the failure mode this corpus already pays for. What is measured is the outcome:
a summary field asserted success while the buffer it pointed at held a compile failure.

**But the five non-reproductions establish something the observation alone could not, and it
narrows the search rather than merely failing to.** Raised by sessionId `9403d62d` and verified
here independently against this session's own three variants: **the correct response shape
carries no exit-status field at all.** Not a wrong one — absent. It is `output_id`, `hint`,
`stdout`, and nothing else.

So `✓ exit 0` **cannot be a misread exit code, because there is no exit code in that response to
misread.** It has to be *synthesized* by whichever branch emits `summary` instead of `hint`. That
changes the shape of the defect from *"a status was read wrong"* to **"a completion-shaped
response fabricates a default status"**, and it points the search at the summary-emitting path
rather than at process reaping — two places that would have cost very different amounts to
search.

**The corollary is the part with teeth, and it is true of the CORRECT path too:** a backgrounded
command's exit status is simply **not available** through this tool, even when everything works.
The honest shape declines to assert one; the defect asserts a false one. Anything gating on a
backgrounded command's success must therefore capture the status **in band** — see
§ *Workarounds*. This is not a mitigation for the bug, it is the standing contract the bug
violates.
## Evidence

The bracket is worth more than the headline, because it is a **positive** identification rather
than an elimination, and it re-dates a separate incident:

1. `cargo test --lib util::path_security` → **216 passed**. The file compiled here.
2. `scripts/fmt-mine.sh` → `formatted 1 file(s) written by this session` — rustfmt wrote
   `src/util/path_security.rs`.
3. `cargo clippy … run_in_background: true` → summary `✓ exit 0`; **buffer holds the compile
   error**. The file was already truncated (4954 → 2918 lines) at this point.

Step 3's buffer is what proves the truncation predates the clippy run, which places the
truncating write at step 2 and **not** at the MCP server restart that followed. This session
had earlier told a peer the truncation was "at or after the server restart"; that was wrong,
and the buffer is what corrected it.

**A peer independently proposed the weaker reading** — that the green was merely *stale*, a
correct result about an earlier instant — and reasoned it well from the ordering alone:
clippy ran after rustfmt and passed, which would place the break later. Reading the buffer
settles it the other way. Recorded because the weaker reading is the one that sounds more
rigorous, and accepting it would have sent the investigation somewhere there was nothing to
find.

## Hypotheses tried

1. **Hypothesis:** the buffer was overwritten and I am quoting a later run.
   **Test:** the quoted lines name `path_security.rs:2919`, a state that existed only in the
   pre-repair window; the post-repair file is 4966 lines and compiles.
   **Verdict:** rejected.
2. **Hypothesis:** any backgrounded non-zero exit is reported as `✓ exit 0`.
   **Test:** three variants above. **Verdict:** rejected — all three returned `Process running`.
3. **Hypothesis:** overflow-sized output is the trigger.
   **Test:** variant 3 (80 000 lines, non-zero exit). **Verdict:** rejected.

## Fix

Not designed — the mechanism is unidentified and a fix aimed at a guess would be unfalsifiable.
What the record asks for first is the discriminating variable.

One observation that should survive whatever the mechanism turns out to be: the `✓` glyph and
the `exit 0` are composed into the **summary**, the field a caller reads *instead of* the
buffer. Progressive disclosure exists so the buffer need not be read; that is exactly why a
wrong summary is not a cosmetic defect here. A summary that cannot establish an exit status
should decline to assert one — the `Process running` shape, which the three repro variants
returned, already does this correctly.

## Tests added

None — there is nothing to regress against until the trigger is known. A test asserting
"backgrounded non-zero exit is not summarised as `✓ exit 0`" would pass today against all three
reproducible variants and say nothing about the observed one, which is coverage in the
direction the corpus calls vacuous.

## Workarounds

**Run gate commands in the foreground.** This is independently required: `CLAUDE.md`
§ *Reaching a Peer Session* records that `run_in_background: true` returns before an exit
status exists to hook on, so the `attribute-red` hook never fires either. Confirmed live the
same day by a second session from the other side — they ran the gate backgrounded, got no
attribution, and had to run `scripts/file-provenance.py` by hand.

That ceiling and this defect are **different failures that share a trigger**: backgrounding
costs you the attribution channel (measured, documented), and — once — also handed back a green
that was not one. Do not conflate them; the first is established and the second is one
observation.

**If you must background something you intend to gate on, capture the status IN BAND.** Per
§ *Root cause*, no backgrounded response carries an exit status even when the tool is behaving,
so reading one out of a summary is unsound whether or not this bug fires. Write it from inside
the same shell and read it back:

```
cargo test --workspace > run.log 2>&1; echo "EXIT=$?" >> run.log
```

Then `grep EXIT= run.log`. Credit to sessionId `9403d62d`, whose gate results survived this
entire incident for exactly this reason — and note their own framing, that it was **not**
foresight about this bug: they had adopted it for the unrelated `;`-ends-in-`echo` trap
(`CLAUDE.md` § *Reaching a Peer Session*), and it happened to be immune to this one too. A habit
that survives a failure mode its author had not imagined is worth more than one aimed at a known
bug.

## Resume

Find the discriminating variable. Per § *Root cause* the search is now scoped to **the branch
that emits `summary` instead of `hint`** for a backgrounded command — that is where a status is
synthesized, and no other path has one to get wrong.

**One candidate is sharpened and one is falsified, both from this session's own transcript.**

*Falsified — restart PROXIMITY alone.* The response carrying `✓ exit 0` also carried the
signature of an MCP server restart (a `project-activation-bootstrap` guide hint reading *"first
call this session"*, plus a `no project has been explicitly activated` workspace notice). That
looked like the trigger. It is not sufficient: the **fast fail** repro carried the *same* two
markers and returned the correct `Process running` shape.

*Sharpened — a process that SPANS a restart.* The distinction the above leaves standing is that
the fast-fail repro was *launched after* the restart, whereas the clippy run was in flight
across one. A server that comes back unable to reap a process it no longer tracks is exactly the
situation in which a default status would get synthesized. **Untested**, and it is the cheapest
remaining probe: background a long `cargo` build that will fail, restart the MCP server while it
runs, and read what comes back.

### Running that probe — blast radius, and DECIDE THE PASS CONDITION FIRST

**Blast radius is one session, not the checkout.** The codescout MCP server is **per-session**:
raised by sessionId `9403d62d` (who notes they had assumed the opposite until the process table
said otherwise) and verified here by `$PPID` — this session's shell reports parent `720167`,
which is the pid they had independently attributed to this session. Seven distinct
`codescout start --debug` processes were live at the moment of that check, one per session. So a
restart-mid-run probe disturbs the session running it and no peer.

**Two refinements to that enumeration, because the population is not what a process name
suggests.** `pgrep codescout` also returns **shared** LSP mux processes — one `codescout mux …
rust-analyzer` and one `codescout mux … kotlin-lsp`, both keyed on a per-checkout socket hash and
shared across sessions. They are a different population from the per-session servers and are not
disturbed by one session's `/mcp`; say which population you counted. And the counts differ by
instant, not by instrument — `9403d62d` reported eight live, this check found seven servers,
taken minutes apart. That is ordinary churn (`CLAUDE.md` § *Reaching a Peer Session*: stamp the
instant), not a disagreement to debug.

**Pre-register the pass condition, because the two outcomes look nothing alike and only one
reproduces this bug.** Raised by `9403d62d` and adopted here: the restart eats the handle *and*
the process's parent — the old server dies with the run, so the new one may never have held a
record of it at all. That predicts two distinguishable results needing different fixes:

| outcome | reading | fix direction |
|---|---|---|
| a synthesized `✓ exit 0` | **reproduces this bug** — the summary branch defaults a status it never had | the `summary`-emitting branch |
| the handle simply does not resolve | **a clean, honest failure** — and a genuine negative result | none; the tool is behaving |

The second will *look* like a failed probe while actually being an answer. Write down which one
you are calling a pass **before** running it, or a negative result gets discarded as a botched
attempt.

**How the restart must be performed, and why that is a constraint rather than a detail.** It has
to come from a user-typed `/mcp`; no tool call reaches it. The alternative — a session killing
its own server process from a shell mid-call — severs the tool access the probe is trying to
observe through, which is both the wrong instrument and a session-level disruption. `9403d62d`
declined to run it on exactly that ground and surfaced it to their operator instead; this
session has done the same. **Whoever runs it needs a human at the keyboard**, which is the real
reason this is still open rather than any difficulty in the probe itself.

Still unseparated, lower-ranked: `cargo`-classified commands (`"type": "build"`) versus plain
shell, and a trailing `2>&1`.

**Note on the evidence's durability — `@bg_*` handles do not survive an MCP server restart.**
The original claim here was that handles are "recycled", which was right about the consequence
and vague about when. Refined by sessionId `9403d62d`, whose handles ran `@bg_00000001` →
`@bg_00000002` monotonically within one uninterrupted session, against this session's
`@bg_00000009` → `@bg_00000001` — and that reset falls exactly on a restart, identified by the
same two markers listed above. **The counter appears to reset on server restart rather than
handles being reused arbitrarily.** Treat as an inference from four observations across two
sessions, not as a read of the source; neither of us has checked the counter's lifetime in the
code.

The practical rule is unchanged but now has a *when*: a captured handle goes stale **across a
restart**, which is precisely when you are least likely to notice, because the restart is not
announced in the response you are about to read. Capture buffers to a file, not to a handle.
The buffer quoted in § *Symptom* was transcribed before its handle was reissued; it no longer
exists to re-read.
## References

- `docs/issues/archive/2026-09-14-the-fmt-refusal-names-an-owner-who-holds-none-of-the-bytes.md` —
  the same day's `fmt-mine.sh` work; step 2 of the bracket is that script's rustfmt call.
- `CLAUDE.md` § *Reaching a Peer Session* — the documented `run_in_background` attribution
  ceiling, the sibling half of this trigger.
- `get_guide("progressive-disclosure")` — the summary/buffer contract this violates.
