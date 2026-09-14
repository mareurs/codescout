---
id: 1f280b2def570b97
kind: bug
status: fixed
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

**ESTABLISHED 2026-09-14 at the bytes** — located in source by sessionId `9403d62d`, verified
here independently. `format_run_command`, `src/tools/run_command/output.rs:413-442`:

```rust
let mut s = if result["output_id"].is_string() {
    let exit = result["exit_code"].as_i64().unwrap_or(0);
    let check = if exit == 0 { "✓" } else { "✗" };
    …
    _ => format!("{check} exit {exit}  (query {output_id})"),
```

The guard is `result["output_id"].is_string()`, and **that predicate is true of two different
shapes**: a buffered COMPLETED result, which carries `exit_code`, and a backgrounded
STILL-RUNNING one, which does not — `output_id`, `hint`, `stdout` and nothing else, exactly as
all six non-reproductions showed. On the second, `as_i64()` is `None`, `unwrap_or(0)` yields
`0`, and `check` becomes `✓`. **Absent is rendered as success.** The `_ =>` arm at `:441` emits
`{check} exit {exit}  (query {output_id})`, which is the observed string
`✓ exit 0  (query @bg_00000001)` in shape, byte for byte.

**A second site carries the same defaulting** at `:446`, in the non-`output_id` branch. That one
is reached only by inline results, which do carry `exit_code`, so it is not currently reachable
with an absent status — but it is the same `unwrap_or(0)` and `CLAUDE.md` § *Testing Discipline*
is explicit that a mutation kill at one guarded site says nothing about another. Fix and test
both, or state why not.

---

*Retained — the state of the inquiry before the source was read, because the reasoning below is
what produced the prediction the code then confirmed.*

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

## Probe 2026-09-14 — restart-spanning FALSIFIED, and the defect localized to the summarizer

Run with a human at the keyboard, against the pass condition pre-registered in § *Resume*
**before** the probe — so the reading below is not a goalpost that moved. A throwaway crate
outside the checkout, `build.rs` sleeping 90 s, a lib with a deliberate `E0308`; `cargo build`
backgrounded with the status captured **in band** (`> log 2>&1; echo "EXIT=$?"`); the operator
typed `/mcp` inside the sleep window.

**Result: outcome 2 — the clean, honest failure. This bug did NOT reproduce.**

```
background job ref not found: @bg_00000006
hint: Buffer refs expire when the session resets. Re-run the original command to get a fresh handle.
```

No status was synthesized. The error is explicit, names the cause, and gives a remedy. Ground
truth from the in-band log was `EXIT=101`, so a real failure existed to be misreported and was
not — the tool declined to answer rather than answering wrongly. **Six non-reproductions now**
(three here, two by `9403d62d`, and this one aimed directly at the sharpest surviving
hypothesis), and *a process in flight across a restart* is eliminated as the trigger.

**Two predictions died, and the second is the useful one.**

*The process survives the restart.* `9403d62d`'s reasoning was that the old server dies with the
run, so the new one may never have held a record of it. Measured otherwise: pids `832995` (sh),
`832996` (cargo) and `833791` (build-script-build) were all still alive **after** the reconnect,
and the build ran to completion and wrote its own exit status. The job is orphaned and
reparented, not killed. So the handle's death is **bookkeeping in the server**, not the process
going away — which is why the honest error is both correct and cheap.

*The backgrounding path was never the defect.* This is the localization, and it rests on
evidence quoted into the transcript before the buffers expired. The overflow buffer
`@tool_9ffd17c9` — the **buffered response object** of the clippy call — contained:

```
  "output_id": "@bg_00000001",
  "hint": "Process running. Output captured in @bg_00000001 — use run_command(\"tail -50 @bg_00000001\") …",
```

That is the **correct** shape — the same one all six non-reproductions returned. The response
`run_command` produced was right. The `✓ exit 0` lived in the **`summary` rendered for that
buffered response**, not in the response itself.

So the defect sits in the **progressive-disclosure summarizer**, on the path where a
`run_command`-shaped payload is too large to inline and gets summarized: it emits a status glyph
and `exit 0` for a payload containing no exit status at all. That is `9403d62d`'s structural
inference (*"synthesized by whichever branch emits `summary` instead of `hint`"*) confirmed and
narrowed to a named branch. It also explains every non-reproduction at once: none of the six
produced a response large enough to overflow, so none of them ever reached the summarizer.

**Caveat on the evidence, stated because it cannot be re-read.** Both `@bg_00000006` and
`@tool_9ffd17c9` are now expired — confirmed by trying, each returning the same explicit
`Buffer refs expire when the session resets`. The quotation above was transcribed from a live
read earlier in the session. That is one level below a re-derivable artifact, and is exactly why
the handle-durability note below exists.
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

**APPLIED 2026-09-14 at `cc57cd28`.** `format_run_command` now matches on `exit_code` rather
than defaulting it: absent renders `… running  (query <id>)` and asserts nothing, agreeing with
the `hint`-shaped response that already worded it correctly. Gate green on both lanes, 9782
passed.

**The second site at `:446` is deliberately NOT changed, and the reason is at the site.** An
inline result is by construction a completed one, so no caller reaches that `unwrap_or(0)` with
an absent status. Adding a branch nothing reaches would be decoration, and untestable
decoration — `CLAUDE.md` § *Testing Discipline*, loudness is a property of a PATH. The comment
names it as the second site so a future pending inline shape finds it.

---

**DESIGNED, not applied.** § *Root cause* now names the site. The shape is not *"read the exit
code better"* — it is that **one predicate is being asked to separate three states**:
completed-and-passed, completed-and-failed, and still-running. `output_id.is_string()`
distinguishes the first two from inline results and collapses the third onto the first.

So the fix is to make the still-running case its own branch and emit **neither** checkmark — the
`hint`-shaped response already words this correctly (`Process running…`), so the summary has a
correct sibling to agree with. Distinguishing on the presence of `exit_code` rather than on
`output_id` is the obvious discriminator and is already in the payload, unused — which is
`CLAUDE.md` § *Testing Discipline*'s *"assert on the name, not on a proxy for it"*, arriving
here as *render* rather than *assert*.

Framing credited to `9403d62d`. Not taken by them (their operator has not pointed them at it,
and a peer cannot authorize work) and not taken here for the same reason; nothing is held, so
whoever is pointed at it should take it with these findings.

---

Not designed — the mechanism is unidentified and a fix aimed at a guess would be unfalsifiable.
What the record asks for first is the discriminating variable.

One observation that should survive whatever the mechanism turns out to be: the `✓` glyph and
the `exit 0` are composed into the **summary**, the field a caller reads *instead of* the
buffer. Progressive disclosure exists so the buffer need not be read; that is exactly why a
wrong summary is not a cosmetic defect here. A summary that cannot establish an exit status
should decline to assert one — the `Process running` shape, which the three repro variants
returned, already does this correctly.

## Tests added

**ADDED 2026-09-14 at `cc57cd28`**, two in `src/tools/run_command/tests.rs`, written *before*
the fix. The TDD red reproduced the defect verbatim — the failure message printed
`✓ exit 0  (query @bg_00000001)`, character for character the string the original clippy call
returned. That is a unit-level reproduction of a bug that six probe attempts could not
reproduce end-to-end, which is the useful shape: once the state was named, expressing it was
trivial.

- `a_still_running_background_result_never_asserts_an_exit_status` — the one that bites: NEITHER
  checkmark, and no `exit ` at all. It asserts against `✗` as well as `✓`, because guessing the
  other direction is the same defect mirrored rather than fixed.
- `a_completed_buffered_result_still_reports_its_exit_status` — the sibling that keeps the fix
  honest. A change that silenced the status for *every* `output_id`-bearing shape would satisfy
  the first test and destroy the reporting this function exists for.

**Mutation-verified, and the first attempt was VACUOUS in a way worth recording.** Restoring the
old `✓ exit 0` rendering is KILLED (`rc=101`, `running 1 test`). But the first probe run
reported **SURVIVED** — because `mutation-probe.sh` builds its isolated worktree at HEAD and
does not carry *other* dirty files, so the then-uncommitted test was absent: `running 0 tests`,
`5541 filtered out`. The script warns that dirty files are not carried, yet still prints
`SURVIVED` with two readings (*untested* / *unreachable*), neither of which is *"your test was
not in the tree"*. That is this corpus's own
`docs/issues/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md` arriving inside
the instrument built to check for it.

**REFINED 2026-09-14 by `9403d62d` — and the correction is to the PRESCRIPTION, not to the
mechanism above.** `scripts/mutation-probe.sh:172` is `cp "$ROOT/$FILE" "$TARGET"`: **the file
under test IS carried across, uncommitted and all.** Only *other* dirty files are built at HEAD,
which the paragraph above states correctly and the script's own comment at `:175` confirms.
Verified here at the bytes. What did not follow was the rule drawn from it:

| mutation and test live in | verdict |
|---|---|
| the **same** file (Rust inline `#[cfg(test)] mod tests`) | **sound, even uncommitted** |
| **different** files (`tests/*.rs`, or a script tested from Rust) | the test file is HEAD's — an uncommitted test is simply ABSENT |

This case was the second kind: mutation in `output.rs`, test in `tests.rs`. *"Commit the test
first"* is correct and bills a commit on **every** run; **"commit first when the test does not
live in the mutated file"** is the same protection billed only to the case that needs it.
`9403d62d` measured the base rate against their own three runs that day — two inline and sound,
one cross-file and vacuous. One in three here, not universal.

**The sharper reading is theirs and is the one to keep.** The script's `others > 0` NOTE *did*
fire on the vacuous run; it named `tests.rs` as dirty and not carried. The information was
present and simply **not joined to the verdict** — `SURVIVED` is computed without reference to a
note printed twenty lines earlier, and its two documented readings (*untested* / *unreachable*)
do not include the third. That is not a missing check but a check whose output does not reach
the conclusion it bears on: `CLAUDE.md` § *Observer Blindness* position 3 in miniature. So the
cheap remedy is **not another warning** — it is that **`SURVIVED` should refuse to render as a
finding when the run executed zero tests**, because a zero-test run cannot survive anything.
Not implemented; recorded here so whoever picks it up does not re-derive it.

**Design constraint, established by `9403d62d` 2026-09-14: there is NO count-free predicate, so
the refusal branch is load-bearing and must be written FIRST rather than last.** They went
looking for one and it does not exist. The script's contract is
`--file/--find/--replace -- <arbitrary test command>`, so it cannot know a priori how many tests
the caller's filter will select. **And exit codes cannot substitute** — on a zero-test run the
baseline exits 0 and the mutant exits 0 too, byte-identical to a genuine survival. That half is
confirmed directly by this file's own vacuous run, which printed `SURVIVED (rc=0)` over
`running 0 tests` and `test result: ok`: `cargo test` exits 0 when its filter matches nothing,
which is `docs/issues/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md` again.
`--list` moves the parse to a purpose-built listing rather than result prose — marginally more
stable, still a parse.

So the count must come from stdout and the format can change underneath it. Concretely:
**`SURVIVED` renders only when the count is both FOUND and GREATER THAN ZERO; anything else
renders `INCONCLUSIVE`, naming which of the two failed.** A reader who sees
`INCONCLUSIVE (test count unparseable)` knows to look at the script; one who sees `SURVIVED`
after a format change learns nothing and believes something false.

**And the shape is this file's own bug, in a second instrument, the same week — `9403d62d`'s
observation and the most portable thing here.** Absence rendered as a value. `unwrap_or(0)` made
*"no exit status"* into *"exited 0"*; a count parse that failed open would make *"no count"* into
*"tests ran and none caught it"*. In both cases the third state is real and simply has no arm.
That is why the refusal is the load-bearing half: a guard keyed on parsing another tool's stdout
is a proxy, and it goes quiet in the direction of rendering a green.

**Neither session took it.** `9403d62d`'s operator pointed them at Windows CI and the
artifact-id gate; this session's pointed it at the `run_command` fix. A peer calling work
unclaimed is not authorization to touch a shared instrument, and both surfaced it to their own
operator instead. The derivation above is complete enough to implement from.

The standing advice, in its narrow form: **commit the test first when it does not live in the
mutated file, and read the test count in the probe output, not only the verdict.**

---

**None yet, but the discriminating assertion is now known and it is not the obvious one.** Raised
by `9403d62d`, and it is the reason this survived: **a test asserting `✓` versus `✗` cannot catch
a third state rendered as the first.** Both checkmarks are about completed runs; the still-running
state is invisible to that axis however many cases you add. The assertion that bites is that the
**still-running shape produces NEITHER checkmark** — and, with the fix, neither an `exit N`.

This is `CLAUDE.md` § *Testing Discipline*'s population law in its input-side form, and this
corpus has the same finding under a different mechanism in
`docs/issues/2026-09-10-the-ack-note-reports-no-foreign-population-exactly-when-the-ack-covered-all-of-it.md`:
*"three states, two tested, and the untested one is the defect"*. Enumerate what the predicate can
see, not what the fixtures can vary. Two sites per § *Root cause*, so two mutations — `:415` is
the reachable one, `:446` currently is not.

---

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

**THE PROBE HAS RUN — see § *Probe 2026-09-14*.** Outcome 2, the honest failure. *In flight
across a restart* is eliminated, and the defect is localized to the progressive-disclosure
**summarizer** rather than to `run_command`'s backgrounding path. Everything below this
paragraph is retained as the state of the inquiry *before* that probe, because the
pre-registered pass condition is what makes its result readable.

**What is worth doing now, in order:**

1. **Read the summarizer's status-rendering branch.** The claim to check at the bytes: given a
   `run_command`-shaped payload with no exit-status field, what makes it emit `✓ exit 0` rather
   than decline? A default-on-absent is the obvious shape and would be a one-line fix — confirm
   it before assuming it.
2. **Reproduce through the summarizer, not through backgrounding.** The trigger is a response
   that **overflows the inline budget** (~10 KB — `get_guide("progressive-disclosure")`). All six
   non-reproductions stayed inline. Aim at a backgrounded command emitting >10 KB into the
   response itself; note the `seq 1 80000` variant did **not** achieve this, because its `stdout`
   was capped before the response was built — so output volume alone is not the lever.
3. **Then write the regression test**, impossible while the trigger was unknown and now
   straightforward: a summarizer given a payload with no exit status must not assert one.

---

Find the discriminating variable. Per § *Root cause* the search is now scoped to **the branch
that emits `summary` instead of `hint`** for a backgrounded command — that is where a status is
synthesized, and no other path has one to get wrong.

**One candidate is sharpened and one is falsified, both from this session's own transcript.**

*Falsified — restart PROXIMITY alone.* The response carrying `✓ exit 0` also carried the
signature of an MCP server restart (a `project-activation-bootstrap` guide hint reading *"first
call this session"*, plus a `no project has been explicitly activated` workspace notice). That
looked like the trigger. It is not sufficient: the **fast fail** repro carried the *same* two
markers and returned the correct `Process running` shape.

*Sharpened — a process that SPANS a restart.* **— FALSIFIED 2026-09-14 by the probe above;
retained because the pre-registration is what makes that result a measurement.** The distinction
the above leaves standing is that
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

**2026-09-14, later — the verdict text this record tells you to read can VANISH.**
`scripts/mutation-probe.sh` writes every verdict to **stderr**, and a `cargo test` wrapped by it
classifies `type: test` through `run_command` — an envelope carrying **no stderr field**. Found
by `9403d62d` on the fix they had shipped hours earlier, on its most common path. Verified
independently here, one step worse than first reported: `echo MARKER >&2; cargo test --lib
run_command::` returned `{type: test, exit_code: 0, output_id: …, passed: 187}` and the marker
was absent from the envelope **and from the buffer** — dropped, not merely unrendered. The
control is the same command one call earlier with a narrower filter: unclassified, inlined, and
its `stderr` field carried the marker intact.

**CORRECTION 2026-09-14, raised by `9403d62d`: the sentence that stood here claimed the
discriminator is the classification. That is true of the ENVELOPE and false of the BUFFER, and
the control above cannot establish either.** An inlined run creates no `@cmd_*` buffer at all,
so it discriminated envelope behaviour only — the buffer half was generalised onto it in
transit. Re-measured here on a **generic, buffered** run (`seq 1 5000; echo MARKER >&2`):

```
{type: generic, exit_code: 0, output_id: @cmd_a0c22c51, stderr: MARKER}
  grep -c MARKER @cmd_a0c22c51  ->  0
  grep -c 4999   @cmd_a0c22c51  ->  1     <- stdout control, same buffer
```

Same run, stderr demonstrably reached the server — it is in the envelope in full — and the buffer
still answers 0 while stdout answers 1. **So the buffer-level loss is UNIVERSAL, not
test-specific.** It went unnoticed only because the generic path also puts stderr in the
envelope, where a reader finds it without ever querying the buffer.

Two separate facts, and only the first is about classification:

| layer | behaviour |
|---|---|
| envelope | `type: test` omits stderr; `type: generic` carries it in full |
| buffer | stderr is never materialised, for any type |

Root cause read at the bytes by `9403d62d`: `grep.rs` and `read_file.rs`'s `read_from_buffer`
both project `BufferEntry` to `.stdout` at materialisation — `BufferEntry.stderr` is written by
`store()` and read by nobody. Filed as `4c433eb615bedf68`, separate from the envelope bug,
because the fix is a contract decision across three tools rather than a missing field.

**The error class is worth more than the correction.** A control that discriminates one layer,
read as discriminating the layer beneath it — the two measurements sat in adjacent sentences and
the scope slipped between them. Same shape as the prescription-vs-mechanism slip recorded
earlier in this file's § *Tests added*, and both were caught by an outside reader rather than by
re-reading, because the supporting fact is correct and a reader checks it and stops.

**Consequence:** an `exit_code: 0, passed: 0` envelope is the byte-identical rendering of a real
SURVIVED, so an INCONCLUSIVE verdict does not merely get skipped by a caller chaining `&&` — it
**never arrives**, and the reader gets a plausible wrong answer rather than a visible gap. The
exit code is the only channel surviving the envelope, which is why `--strict` shipped (opt-in,
remapping only the two INCONCLUSIVE branches to exit 3) and why the INCONCLUSIVE-exits-0 trade
recorded above is safe only for a human reading a terminal.

**Nothing above is retracted.** § *Tests added*'s claim that exit codes cannot substitute is
about **detecting** a zero-test run and still holds; this is about **reporting** the verdict once
detected. Different claims, both standing. Envelope bug filed separately by `9403d62d`.

**For anyone re-running this file's probes:** every mutation run recorded here appended `2>&1`,
which merges stderr into stdout **in the shell, before the tool classifies anything** — which is
why those verdicts were visible at all. Habit, not design. The conclusions do not rest on it:
each KILLED was read from `exit_code: 101` plus the named failing test in `failures`, both of
which survive the classified envelope.

## Fix provenance

- **SHA:** `cc57cd28` (experiments) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `37ef2b3feb70a79881f8c5bbd2ce64edae72cd5d` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id.
