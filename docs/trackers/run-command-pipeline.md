---
id: '5d022cd3b41009f4'
kind: tracker
status: 'draft — R2 recorded; #4/#5/#8 reclassified as blocked on #7'
title: run_command(pipeline=[...]) design
tags:
- run_command
- pipeline
- il3
---

# Tracker — `run_command(pipeline=[...])`

## Why

IL3 banned all pipes to log-trimmers because piping discards the buffer. The deny hook was promoted 2026-05-18 after 50+ slips. But the ban has a cost: legitimate **staged filtering** of live commands has no first-class API — agents either:

1. Pipe anyway (now blocked), or
2. Re-run from the buffer (clumsy, costs a second invocation per stage).

`pipeline=[...]` is the missing primitive: each stage's stdout buffered as its own `@cmd_*`, stage k+1 reads stage k via stdin. Funnel is materialized, debuggable, queryable per-stage.

The hook fix (shipped 2026-05-18) is the **read-side** half — pipes that start from a buffer (`grep PATTERN @cmd_x | sort`) are now allowed because the capture already happened. `pipeline=` is the **write-side** half — staging on a live command.

## Design surfaces (open)

1. **Schema shape.** `pipeline: Vec<String>` (additional stages after `command`)? Or
   `command` + `pipeline: Vec<String>` where pipeline[0] is stage 1? Or
   replace `command` entirely with a stages array when pipeline given?
   Lean: `command` = stage 0 (current behavior preserved), `pipeline` = stages 1..N.

2. **Mutual exclusivity.** Pipeline cannot co-exist with:
   - `run_in_background` (would need a streaming abstraction)
   - `interactive` (no stdin loop semantics for staged pipes)
   - `@ack_*` handle as `command` (ack flow is single-stage)

3. **Timeout policy.** Total wall-clock across all stages, or per-stage?
   Lean: total. Caller passes `timeout_secs`; each stage gets `remaining = total - elapsed`.
   Simpler, matches user mental model.

4. **Pipefail semantics.** Stop on first non-zero, keep prior buffers, surface failing
   stage clearly. (Matches `set -o pipefail`.)

5. **Output shape.**
   ```json
   {
     "stages": [
       {"stage": 0, "cmd": "cargo test", "ref": "@cmd_a", "exit_code": 0, "preview": "..."},
       {"stage": 1, "cmd": "grep FAILED",  "ref": "@cmd_b", "exit_code": 0, "preview": "..."},
       {"stage": 2, "cmd": "head -20",     "ref": "@cmd_c", "exit_code": 0, "preview": "..."}
     ],
     "final_ref": "@cmd_c",
     "stopped_at": null
   }
   ```
   On pipefail: `stopped_at: 2` + truncated stages array + error stage's stderr/exit_code.

6. **Per-stage dangerous-command gate?** Each stage runs `is_dangerous_command`?
   Lean: yes — `pipeline=["cargo test", "rm -rf /"]` should be caught at stage 1.
   But `@ack_*` flow per-stage gets ugly. Initial cut: any dangerous stage → outright reject
   the whole pipeline call with a clear hint. Defer ack-handle support.

7. **Implementation strategy.**
   - **A. Reuse `run_command_inner`.** Each stage spawns via inner with synthesized
     `<stage> < <tempfile_of_prev_stage>`. Buffer stage stdout via `OutputBuffer.store`.
     Pro: reuses timeout, killpg, security checks. Con: re-runs `resolve_refs` per stage.
   - **B. New `run_pipeline_inner`.** Spawns each stage directly, pipes stdouts in memory.
     Pro: cleaner data flow. Con: duplicates process-group/killpg/SIGPIPE machinery.
   Lean: **A**. Reuse machinery; the per-stage `resolve_refs` is cheap and consistent.

8. **format_compact display.**
   ```
   ✓ pipeline 3/3 stages  (queries @cmd_a @cmd_b @cmd_c)
   ✗ pipeline 2/3 — stage 1 (grep FAILED) exit 1  (query @cmd_b for err)
   ```

## Architectural review (Snow Lion, 2026-05-18)

`run_command_inner` is currently a 9-mode dispatcher (interactive · ack-redispatch · resolve_refs · dangerous-cmd gate · source-file block · shell-mode check · background spawn · tee injection · foreground exec). The original Strategy A added a 10th. Architectural concerns:

### Concern 1 — `inject_tee` is a parallel stage-buffering mechanism

`inject_tee` (`src/tools/run_command/inner.rs:175-228`, called at `:406` — re-anchored
2026-09-01; cited as `:145-186`/`:288` when written) already rewrites `... | grep FAILED` into `... | tee /tmp/unfiltered | grep FAILED`, capturing the pre-filter stream as a buffer. That is one-stage-deep pipeline buffering, in production today. Strategy A as drafted ignores this and builds a parallel mechanism — two systems for the same shape of input.

**Decision (proposed):** `pipeline=` rewrites stages into a single shell pipeline with per-stage tee taps; reuses the existing foreground-exec path. `inject_tee` generalizes from "tee the penultimate stage" to "tee every stage."

**Alternatives:**
- Strategy A (`run_command_inner` per stage) — rejected: two pipeline-buffering mechanisms in tree.
- Strategy B (greenfield `run_pipeline_inner` with own spawn) — rejected: duplicates killpg/SIGPIPE/timeout machinery.

**Consequences:**
- now easier: per-stage tee already debugged on Unix (process groups, SIGPIPE reset). Pipefail = `set -o pipefail` in shell wrapper, no Rust state machine.
- now harder: per-stage timeout impossible (single shell process); only total via existing `tokio::time::timeout`. Per-stage cancellation impossible.

**Change scenarios absorbed:** per-stage cwd (`(cd <dir> && <stage>) | tee ...`); pipefail policy change (drop `set -o pipefail` from wrapper).

**Revisit-when:** streaming-output requirement lands (`stream_tail` / live `@bg_*` peek) — breaks the single-shell-process assumption.

**Confidence:** medium. Bash-ism risk (`set -o pipefail` is not POSIX sh); per-stage control is impossible by construction.

### Concern 2 — extract `exec_one_stage` before adding any 10th mode

Nine dispatch modes in one function is past the "argues about where new features belong" heuristic. Before pipeline= goes anywhere, extract the foreground-exec block (spawn + killpg + timeout + SIGPIPE reset +
buffer-store) as `exec_one_stage`.

> **Re-anchored 2026-09-01.** The `inner.rs:251-394` cited here no longer names the block.
> `run_command_inner` is now `src/tools/run_command/inner.rs:279-605` (326 lines); the
> foreground-exec path is what follows the `inject_tee` call at `:406` and runs to the
> function's end at `:605`. That bound is derived from two verified anchors, not from a read
> of the block — confirm the exact span before cutting. **Still not done:**
> `symbols(name="exec_one_stage")` returns 0 matches. The "nine dispatch modes" count above
> is the 2026-05-18 claim and was not re-derived. Both current foreground path and pipeline= delegate to it.

**Confidence:** medium. Have not read the full block; coupling to surrounding `inject_tee` flow may force a larger refactor than 140 LOC.

### Concern 3 — companion hook is `command`-blind to a `command + pipeline` schema

> **Correction 2026-09-01 — the premise below is dead; the conclusion survives and is
> stronger.** The text of this concern argues from the *companion hook*. That hook is no
> longer wired: `detect_il3_violation`'s doc comment (`src/util/path_security.rs`) records
> *"That file still exists but is no longer wired: measured 2026-08-27, no `hooks.json`
> PreToolUse matcher targets `run_command` … This function is the only live enforcement."*
> There is no hook left to be blind, so **open item #10 is void** (struck below).
>
> Re-found the concern on the live gate instead: `detect_il3_violation(command)` takes a
> **single string** and runs in `RunCommand::call` (`src/tools/run_command/mod.rs:211`),
> before `resolve_refs`. Under a `command` + `pipeline` schema it would see stage 0 only —
> so the blindness is **codescout's own**, reaching every MCP client rather than just Claude
> Code. The `stages` XOR `command` decision is therefore better supported than the argument
> originally written for it, not weaker. Confidence stays **high**; the evidence changed.
>
> Analysis below kept as the dated record that argued for the decision. Do not read its
> present tense as current.

If schema lands as `command = stage 0, pipeline = [stages 1..N]`, the IL3 hook reads `tool_input.command`, sees only "cargo test," and allows. Actual behavior is a pipeline with a log-trimmer. **IL3 enforcement becomes blind to pipelines.**

**Decision (proposed):** schema is `stages: [str]` XOR `command: str`. Top-level, mutually exclusive.

**Alternatives:**
- `command` + `pipeline` (additional stages) — rejected: semantic overload, hook blindness, every downstream consumer must learn the overload.

**Consequences:**
- now easier: contract is "one of {command, stages}"; hook + telemetry + format_compact branch once on field presence.
- now harder: caller cannot append stages incrementally — must structure as `stages` from the start.

**Change scenarios absorbed:** hook enforces IL3 on pipelines (sees `stages` directly); telemetry distinguishes single-cmd vs pipeline.

**Confidence:** high.

### Concern 4 — hook coupling-across-repos is a load-bearing signal

The companion hook (`claude-plugins/codescout-companion/hooks/il3-{deny,warn}-hook.sh`) and codescout's `run_command` schema have now co-changed twice in 48h (buffer-op whitelist 2026-05-18, this design). The "two modules always change together" heuristic says they may be one module wearing two names. If a third co-change lands, the IL3 enforcement contract probably belongs **in codescout itself** (as a built-in pre-execution gate emitting `RecoverableError` with the same hint), not in a sibling repo where it drifts.

Not a decision for this tracker — flagged for the next IL3 evolution.

**Update 2026-05-18:** server-side enforcement landed. `detect_il3_violation`
in `src/util/path_security.rs` is called from `RunCommand::call` before
`resolve_refs`. The companion hook now becomes a belt-and-braces layer
covering only Claude Code; codescout itself rejects the shape for all MCP
clients (Claude Code, Copilot, Gemini, …). The "two modules always change
together" signal pointed at this fix correctly. See
`docs/issues/archive/2026-05-18-il3-pipe-violation-subagent.md` (fixed).

## Tracker updates

- Open #1 (schema) — leans `stages` XOR `command`, not `command + pipeline`. (Concern 3.)
- Open #7 (strategy) — add **Strategy C: shell-pipeline rewrite with per-stage tee taps**. Lean C unless per-stage timeout requirement emerges. (Concern 1.)
- New open #9 — extract `exec_one_stage` from `run_command_inner` before adding any new mode. Prerequisite for any strategy. (Concern 2.)
- ~~New open #10 — companion hook must read `tool_input.stages` if schema lands as `stages`.~~
  **VOID 2026-09-01** — no wired companion hook exists (no `hooks.json` PreToolUse matcher
  targets `run_command`, measured 2026-08-27). Nothing to update. The enforcement this item
  was protecting lives at `src/tools/run_command/mod.rs:211`; see the correction on Concern 3.


## Rulings

### R1 (2026-09-01, marius) — pipeline calls exec `bash -c`, not `sh -c`

**Settles the shell half of #4 (pipefail) and unblocks #7.** Strategy C's headline advantage
was *"pipefail = `set -o pipefail` in shell wrapper, no Rust state machine"*, and
`src/platform/unix.rs:71-86` execs `Command::new("sh")` — so the advantage rested on a shell
codescout does not use. **Ruled: use `bash -c` for pipeline calls.**

**Measured 2026-09-01, and the reason is NOT "pipefail is not POSIX".** That was the
rationale first recorded here and it is wrong twice over: POSIX Issue 8 (2024) adds
`pipefail`, and upstream dash implements it. The real discriminator is **distro packaging of
the same upstream version**:

| system | `/bin/sh` → | version | `set -o pipefail; false \| true` |
|---|---|---|---|
| this dev host (Arch) | bash | — | works (tautological — it *is* bash) |
| Debian 13 trixie | dash | 0.5.12-**12** | **works** — `pipeline_status=1` |
| Alpine 3.23 | busybox | 1.37.0 | **works** — `pipeline_status=1` |
| **Ubuntu 24.04 — `ubuntu-latest`, our CI runner** | dash | 0.5.12-**6ubuntu5** | **`sh: 1: set: Illegal option -o pipefail`, exit 2** |

Same upstream dash `0.5.12`; Debian's `-12` carries the pipefail patch and Ubuntu's
`-6ubuntu5` does not. `.github/workflows/ci.yml` runs `ubuntu-latest` on every non-matrix
job, so the one system where it fails is the one that gates merges. **R1 stands — on a
narrower and better-evidenced claim than the one it was ruled on.**

> **Do not re-derive this from the first two rows.** Both were reached for because the images
> happened to be local, and both *support* pipefail — a probe that stopped there would have
> retracted a correct ruling. The sample has to be chosen by relevance to the proposition (CI),
> not by availability. See `design-backlog-session-log:F-5`.

**Why this is nearly free — half of it is already shipped.** `src/platform/windows.rs:182-193`
already execs `git_bash_path()` with `-c`; Windows has been on bash the whole time, for the
reason its own doc comment gives — *"the shell is the same POSIX shell on every platform …
under `cmd.exe` none of those binaries exist, so the guidance codescout ships was unrunnable
on Windows."* This ruling extends that existing principle to Unix for one call shape rather
than introducing a new dependency direction.

**The one new obligation.** `shell_unavailable_hint()` returns `None` unconditionally on Unix
(`src/platform/unix.rs:67-69`) because *"POSIX guarantees `/bin/sh`"* — a guarantee that does
**not** extend to bash. A pipeline call therefore needs a bash-availability probe and a
`RecoverableError` naming the requirement when it is absent. The Windows twin is the exact
precedent: it already answers `Some(hint)` when no Git Bash is installed, and `run_command`
already turns that into a recoverable error rather than a bare `program not found`. Build the
Unix pipeline path the same way; do not let it fail as a spawn error.

**Consequence deliberately accepted, not waved away.** Single-command `run_command` stays
`sh -c` on Unix, so the tool will exec two different shells depending on which field is set.
A bashism then works in pipeline mode and fails bare — a real, small inconsistency. It is
accepted rather than resolved by moving *all* Unix shell use to bash, because that would make
bash a hard dependency for every `run_command` call rather than for the one shape that needs
it. **Revisit-when:** a user reports a command behaving differently bare vs. as stage 0, or
any second feature needs a bashism — at that point the divergence is paying no rent and
unifying on bash is the cheaper answer.

**Still open after this ruling:** the pipefail *semantics* half of #4 (stop-on-first-non-zero,
keep prior buffers, surface the failing stage) is unchanged and still decidable as written.
#7's choice between Strategy C and A/B is now unblocked but **not made** — C's remaining cost
is that per-stage timeout is impossible, which § *Resume* step 2 says to rule knowingly.

> **Note on why per-stage cancellation is NOT among C's costs.** An earlier framing of this
> tracker's open questions treated "kill one stage, keep the pipeline" as C's deciding
> drawback. It is not a requirement: `run_command` exposes no per-stage control surface, the
> MCP call is request/response with no mid-flight channel, #2 makes pipeline mutually
> exclusive with `run_in_background` (closing the one handle-based path), and the existing
> cancellation intent is explicitly the opposite — `src/tools/run_command/inner.rs:436-439`
> kills *"the entire pipeline … not just the shell"* on future-drop, by design. The
> SIGPIPE-driven behaviours callers actually rely on (`head` closing stdin kills the producer;
> killing a producer EOFs its consumers) are free in any shell pipeline and survive under C —
> `unix.rs:80` resets SIGPIPE to `SIG_DFL` in `pre_exec` precisely so they work. See
> `design-backlog-session-log:F-4`.

### Measured 2026-09-02 — #4, #5 and #8 are NOT "decidable as written"

These three were carried as *decidable as written*. They are not. Both reasons were found by
measuring rather than reading, and neither is visible in the surfaces text.

**(a) `set -o pipefail` verbatim reports FAILURE on this feature's primary use case.** Measured
under `bash -c`, the shell R1 mandates:

| pipeline | `pipefail` | exit | `PIPESTATUS` |
|---|---|---|---|
| `seq 1 100000 \| grep 5 \| head -3` | on | **141** | `141 141 0` |
| `seq 1 100000 \| grep 5 \| head -3` | off | 0 | `141 141 0` |
| `seq 1 100 \| grep ^5 \| wc -l` | on | 0 | `0 0 0` |

`head -3` exits after three lines and closes the pipe; upstream stages die of SIGPIPE (141 =
128+13). Under `pipefail` the pipeline inherits the rightmost non-zero, so a **fully successful**
`… | head -N` returns 141. That is the exact shape IL-3 exists to redirect (`cargo test | grep
FAILED | head`) and therefore the shape `pipeline=` is built to serve.

**§ *Tests needed* cannot catch this, and the reason is structural.** Its happy-path case is
`seq 1 100 | grep ^5 | wc -l`, and `wc` **consumes all input** — no early close, no SIGPIPE, exit
`0 0 0`. Its pipefail case plants a non-zero on a stage directly. Neither uses a *trimmer*, so
the suite is monotone under this defect: it passes whether or not the bug is present. Add a
`… | head -N` case whose assertion is `success`, or the first real user is the detector.

**Remedy available because R1 gave us bash: do not use bare `set -o pipefail`.** `PIPESTATUS`
exposes every stage's code (and is a bashism — Ubuntu's dash answers `Bad substitution`, so R1
is load-bearing here too, not only for `pipefail`). Read it and apply an explicit policy:
SIGPIPE (141) on a **non-final** stage is success when a later stage exited 0, because that is
`head` working. Everything else is a failure.

**(b) "Stop on first non-zero" presumes SEQUENTIAL stages; Strategy C runs them CONCURRENTLY.**
A shell pipeline starts every stage at once. By the time stage 0 exits non-zero, stages 1..N
have been running and consuming. There is no "stop" to perform and no *"truncated stages
array"* (#5) to emit — every stage ran. Under C, `stopped_at` is not a thing that happens; it is
a thing **derived** from `PIPESTATUS` after the fact, and the honest field is every stage's exit
code, which is strictly more information than the original design proposed.

**Consequence for sequencing.** #4, #5 and #8 are **downstream of #7**, not independent of it.
#5's envelope and #8's `✗ pipeline 2/3` display both encode stopping. Rule #7 first — as §
*Resume* step 2 already says — and these three follow from it. Only **#2** is genuinely
independent of the strategy choice.

### R2 (2026-09-02, marius) — #2 mutual exclusivity: adopt as written

**Ruled: a pipeline call cannot co-exist with `run_in_background`, `interactive`, or an `@ack_*`
handle as the command.** Adopted verbatim from surface #2; no revision needed.

**Why this one could be ruled now and the others could not.** #2 is the only surface whose
content does not encode an execution model. #4, #5 and #8 all presume *sequential* stages and
are downstream of #7 — see the measurement block above. #2 asks which other modes conflict, and
the answer is the same under every strategy.

**Substrate verified 2026-09-02** — all three named modes exist, so the exclusion list refers to
real things rather than to the tracker's memory of them:

| mode | site |
|---|---|
| `run_in_background` | `spawn_background_command`, `src/tools/run_command/inner.rs:89-144` |
| `interactive` | `run_command_interactive`, `src/tools/run_command/interactive.rs:25-238` |
| `@ack_*` re-dispatch | the `acknowledge_risk` flow on `RunCommand` |

**The list is complete, checked against the review's own inventory** rather than accepted as
given. § *Architectural review* enumerates nine dispatch modes: interactive, ack-redispatch,
`resolve_refs`, dangerous-cmd gate, source-file block, shell-mode check, background spawn, tee
injection, foreground exec. Of those, exactly three are *alternative execution modes* a caller
selects — the three above. The rest are gates or transforms every call passes through, which a
pipeline must also pass through rather than exclude. So there is no fourth candidate.

**Refusal shape.** `RecoverableError` naming the specific conflicting flag, per the project's
error-handling convention — not a silent precedence rule, and not a schema-level `oneOf` that
would be invisible in the tool description an agent reads. **Refuse before any stage runs**,
for the same reason #6 leans that way: a partially-executed pipeline whose refusal arrives at
stage 2 has already had side effects.

**Consequences.** Now easier: each mode keeps its current single-command semantics untouched,
and `pipeline` needs no interaction design with any of them. Now harder: a caller wanting a
backgrounded pipeline has no path — accepted, because that is the streaming abstraction
Concern 1's *Revisit-when* already names as the thing that would break the single-shell-process
assumption. If backgrounded pipelines are ever wanted, #7 gets reopened, not #2.

**Confidence:** high. The ruling restates the surface, its three referents are verified at
`path:line`, and its completeness was derived from the mode inventory rather than assumed.

### Measured 2026-09-02 — "per-stage timeout impossible" is false, and it was C's last remaining cost

Concern 1 lists, under *now harder*: **"per-stage timeout impossible (single shell process);
only total via existing `tokio::time::timeout`."** That is true of **Rust** and false of the
**capability**. Per-stage timeout composes in the shell string, which is the same layer
Concern 1 already uses for per-stage cwd (`(cd <dir> && <stage>) | tee …`).

Measured under `bash -c`:

    echo hi | timeout 1 sleep 30 | cat
    → exit=0   PIPESTATUS=0 124 0

Stage 1 was bounded independently and killed at 1s (`124` is `timeout`'s timed-out code) while
stages 0 and 2 completed normally. A per-stage budget is `timeout <n> <stage>` injected beside
the tee tap — no Rust handle needed, no second buffering mechanism, no change to the
single-shell-process model.

**Consequence: Strategy C now has no established cost.** Its two stated drawbacks were
per-stage cancellation and per-stage timeout. The first was withdrawn 2026-09-01 — no caller
can reach it (`design-backlog-session-log:F-4`). The second is reachable, as above. What
remains is Concern 1's original argument, which runs *for* C: `inject_tee` is already
one-stage-deep pipeline buffering in production, and A/B would put a second mechanism of the
same shape beside it.

**Two caveats, neither hidden.**

- **`timeout` is GNU coreutils and its presence on Windows Git Bash is NOT verified here.** The
  same package supplies `grep`/`head`/`tail`/`sed`, which `src/platform/windows.rs:182-193`
  already depends on by design — so it is *likely* present, and likely is not measured. Run
  `<git-bash> -c 'command -v timeout'` on a Windows host before relying on it. If absent,
  per-stage timeout degrades to unavailable-on-Windows rather than unavailable-everywhere, and
  total timeout still works on both.
- **`124` needs a policy slot next to `141`.** A timed-out stage exits 124 and its downstream
  neighbours then see a clean EOF and exit 0 — so the pipeline looks successful unless
  `PIPESTATUS` is read. That is the same reason the block above rules against bare
  `set -o pipefail`; the two findings compose into one rule: **decide from `PIPESTATUS`, never
  from the pipeline's aggregate status.**

**Both of Strategy C's costs were enumerated by reasoning about the mechanism rather than by
testing whether the capability was reachable at another layer, and both were wrong.**
*"Impossible by construction"* is a claim about **one construction**, not about the capability.
See `design-backlog-session-log:F-7`.
## Tests needed

- Happy: 3-stage `seq 1 100 | grep ^5 | wc -l` produces 11 (one "5", "50"-"59", "5"; 11 matches).
- Pipefail: stage 1 returns non-zero, stage 2 never runs; result has `stopped_at: 1` and only 2 stage entries.
- Stdin wiring: stage k stdin is exactly stage k-1 stdout (no leakage, no doubling).
- Dangerous stage rejected before any stage runs.
- Timeout total: `pipeline=["sleep 5", "cat"]` with `timeout_secs=2` → timeout error, stage 0 buffer kept.
- Mutex: pipeline + run_in_background → recoverable error.

## Prompt rewrites

- `src/prompts/source.md` IL3 section — already has buffer-op allowance; add `pipeline=` paragraph.
- `src/tools/run_command/mod.rs` `long_docs` — add `## Staged Pipelines` section with one example.

## Resume

Hook smartening shipped 2026-05-18 (see commit on `experiments`). IL3 prompt rewrite shipped
same day. Pipeline= design tracker opened, implementation pending its own dedicated session.

**Revised 2026-09-01 after a substrate scout** (`design-backlog-session-log:F-3`). The line
that stood here read *"resolve open items 1, 3, 6 → write `run_pipeline_inner` per strategy
A"*. **Strategy A is rejected** by § *Architectural review* Concern 1 — *"two
pipeline-buffering mechanisms in tree"* — and § *Tracker updates* records *"Lean C."* The
Resume was written before the review and never updated, so the file's entry point routed an
implementer to the design its own review threw out. Corrected:

Open the next session with:

1. **Read § *Architectural review* before § *Design surfaces*.** The review supersedes the
   leans stated in the surfaces list wherever the two differ (#1 and #7 both changed).
2. **Rule #7 first — it forces #3.** Strategy **C** (shell-pipeline rewrite with per-stage
   tee taps, generalizing `inject_tee`) makes per-stage timeout and per-stage cancellation
   *impossible by construction*. That is the one irreversible choice in the set: rule it
   knowing a long `cargo test` stage can then only be cancelled as a whole.
3. **#1 is `stages` XOR `command`** — see the Concern 3 correction; the argument now rests on
   `src/tools/run_command/mod.rs:211`, not on the retired companion hook.
4. **#9 is a prerequisite under every strategy** — extract `exec_one_stage` from
   `run_command_inner` first. Verified 2026-09-01: `symbols(name="exec_one_stage")` returns
   **0 matches**, and `run_command_inner` is `src/tools/run_command/inner.rs:279-605`
   (326 lines).
5. Then #2, #4, #5, #6, #8 — all decidable as written. Note #6 (per-stage dangerous-command
   gate) now has a sibling defect worth ruling once for both:
   `docs/issues/2026-09-01-source-gate-refuses-the-whole-compound-command.md`.
6. Tests (§ *Tests needed*), then prompt update (§ *Prompt rewrites*).

**Line references in § *Architectural review* were re-anchored 2026-09-01** and are accurate
as of that date; prefer the symbol names, which do not rot.
