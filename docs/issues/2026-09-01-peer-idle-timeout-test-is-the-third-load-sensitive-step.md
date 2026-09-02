---
id: ee9d8d80ad5ecdc8
kind: bug
status: open
title: peer idle-timeout test is the third load-sensitive step in a class fixed twice per-instance
tags:
- cluster/repro-env-diverges-from-gate-env
closed: ''
last_observed: 2026-09-01
opened: 2026-09-01
owner: marius
related:
- '64efc41ac6686afb'
severity: low
unverified: 'Pre-existence is NOT established — no bisect against 0ed6cb18 was run, and it remains INFEASIBLE: the rate is too low to separate signal from noise. The no-call-path argument makes causation by the T-7 diff implausible. Obs. 5-6 (peer, 2026-09-01) add two more full-`--workspace` failures on a ~6-session box but supply NO condition — the reporter''s "concurrent cargo holding the target lock" claim was retracted the same day by their own second run, which had zero lock waits and failed anyway. This session''s four runs refute total load (the HEAVIEST run passed, a lighter one failed). Sole surviving candidate is async-executor churn, and it does not survive its own first check either (equal other-failure counts produced opposite outcomes). No discriminating factor is known.'
---

## Summary
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections` failed once in a full
parallel `cargo test --workspace` during the T-7 SDD run. This is the **third** step of the
class first filed as `docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md`
(archived `fixed`), and the second recurrence — that file already records one, headed
*"the HOLDER step flakes too; fix covered only the reclaim step."*

The pattern is the finding. Each fix raised the real-time budget of **the step that had
flaked**, and the class then produced a different step. `2026-07-03` raised connect budgets
(`connect_with_retry` 50×20ms → 250×20ms, two inline loops, plus `peer/client.rs`);
`2026-07-05` added the mux holder step. Today's test measures neither — it asserts the server
**exits** after an idle period, so no connect budget covers it.

## Symptom (Effect)
One failure in one `cargo test --workspace` run. Not reproduced since.

## Reproduction
Not reliably reproducible. Measured 2026-09-01 on the fix commit `37f45079`:

| probe | result |
|---|---|
| the test in isolation, ×5 | 5/5 pass (1.13–1.42s each) |
| full `cargo test --workspace`, ×2 | 2/2 clean, exit 0, every result line `ok` |
| full workspace `--test-threads=1` (implementer) | 5030/5030 |
| full `cargo test --workspace` (implementer, parallel) | 4809 pass, this one fails |

So: real, load-sensitive, and low-rate.

## Environment
codescout worktree `audit-shards-t7` @ `37f45079`, forked from `experiments` `0ed6cb18`.
Linux, `nproc`-wide default parallelism, concurrent with a release build.

## Root cause
Not established, and deliberately not asserted. The 2026-07-03 file's own root cause reads
*"Unknown — not investigated beyond isolation"*, and nothing since has improved on that.

The working hypothesis that fits all three steps: **a test asserting a real-time property
under `nproc`-wide parallelism is asserting about the scheduler, not about the code.** Connect
readiness, flock reclaim and idle-timeout expiry are three different observations of the same
underlying quantity — whether this process got CPU inside a fixed wall-clock window.

## Evidence
### Why this is a class instance, not a new bug
The prior fix's own text names the shape: *"the original fix had covered only the reclaim
step; the holder step flaked under a full parallel run."* That is fix-per-observed-instance
against a class with several members — CLAUDE.md § Testing Discipline's *"Mutate once per
guarded SITE, not once per feature"*, in its remediation form. Three sites, three separate
discoveries, no pass that enumerated them.

### Why it is not attributable to the T-7 change
Stated as an argument, not a proof. `host.rs` is a new module whose every item is
`#[expect(dead_code)]` because **nothing outside its own tests calls it** — Tasks 2 and 3 wire
it in. There is no call path from it to `peer::server`. What the change does do is add ~5 tests
to the binary, marginally increasing parallel load, which could aggravate a load-sensitive
timer without causing it.

**What was NOT done:** a bisect against `0ed6cb18`. At an unmeasured rate that did not
reproduce in two full runs, establishing pre-existence would need many runs. So "pre-existing"
is the hypothesis the mechanism supports and is *not* something this file asserts.


### Fourth observation — independent session, 2026-09-01, with load DIRECTLY observed

Added by `codescout-17` (not this file's author) because it bears on the `unverified:` field's
exact gap: *"2/2 clean full runs at a low flake rate cannot distinguish pre-existing from
aggravated."*

- **Where:** main checkout, `HEAD 72484f8d5817e4675191d84caaaad869abf78f71`, working tree
  carrying an unrelated `src/util/librarian_guard.rs` change and a peer's uncommitted
  `doctor.rs` edits. **No T-7 audit-shard code in the tree** — that work is in
  `.worktrees/audit-shards-t7`. Same failure, same message.
- **Result:** `cargo test --workspace` → 4804 passed / **1 failed**, the failure being this
  test: `run() did not exit within 10s of a 1s idle timeout`. Re-run in isolation
  (`cargo test --workspace run_exits_after_idle_timeout_with_no_connections`) → **ok in
  1.19s**.
- **The load was observed, not inferred.** Both cargo invocations printed
  `Blocking waiting for file lock on build directory`, and `ListAgents` reported two other
  interactive sessions **both `busy`** at the time. So this run has direct evidence for the
  scheduler hypothesis in *Root cause* rather than an appeal to plausibility — the process
  demonstrably was not getting the CPU it wanted.

**What this does and does not settle.** It does **not** establish pre-existence: no bisect was
run here either, so the `unverified:` field stands as written. What it adds is that the failure
reproduces in a tree **without** the T-7 diff, which strengthens the no-call-path argument from
implausible-by-reading to not-observed-to-need-it. Attribution to load is now backed by a
measured contention signal in at least one instance.

**Method note for whoever bisects.** This instance was nearly re-derived from scratch: the
isolation re-run had already been launched before a peer pointed out this file existed. The
file was **unreadable by `read_markdown`** — it is a tool-created bug file and therefore
stamped, so the librarian guard refused it, which is
`docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md` firing on
the corpus's newest file. Reaching it needed `artifact(action="find")` then
`artifact(action="get")`. Worth knowing if a future session looks for this record by path and
concludes it is absent.

Two further reproductions, reported by `fix-subagent-guide-injection-brief` (`.claude-kat`) on
2026-09-01, who deliberately did not edit this file. Recorded here because they are the first
observations to arrive with a **named condition** rather than a rate:

| probe | result | condition |
|---|---|---|
| full `cargo test --workspace` | fails | 6 live sessions **and** a concurrent cargo holding the target lock |
| full `cargo test --workspace` | fails | same |
| the test in isolation | passes, 1.13s against a 10s budget | quiet |

Their own reading: this "moved the rate from ~1-in-N to 2-of-2". They also confirm
`src/peer/server.rs` last changed 2026-08-18, so neither the T-7 diff nor their own
`src/prompts/` + `src/tools/guide.rs` change is in the call path — an independent restatement
of § *Why it is not attributable to the T-7 change*, from a session with a different diff.

**Why these two matter more than the rate they add.** Every prior observation reported a
frequency; these report a *condition*. A 2-of-2 rate under a condition someone can recreate is
the first thing here that makes the bisect in `unverified:` actually runnable — at ~1-in-N a
bisect against `0ed6cb18` cannot separate signal from noise, which is precisely why nobody has
run it. If the condition holds, the method changes from "re-run until it flakes" to "hold the
target lock and run once per candidate commit".

**And this session's own four runs cut partly AGAINST a simple load reading, which is why the
condition is still a candidate.** Measured 2026-09-01 during the `30b6fc41` work:

| run | scope | other failures | this test |
|---|---|---|---|
| mutation A (default returns `None`) | `--workspace --lib` | 11 | **failed** |
| mutation B (one tool opts out) | `--workspace --lib` | 1 | passed |
| gate, lean lane | `--workspace --no-default-features` | 0 | passed |
| gate, default lane | `--workspace` (full, heaviest) | 1 (a peer's snapshot) | passed |

The heaviest run passed and a lighter one failed. So *total* load is not sufficient to predict
it, and the peer's more specific factor — a **concurrent cargo build holding the target lock**,
not merely concurrent sessions — survives this data where a general load hypothesis does not.
Stated as a discriminating candidate to test, not as the cause: four runs on one box cannot
separate lock contention from scheduler luck, and the mutation-A run had 11 other failing tests
churning the same executor, which is its own confound.

**The next probe is therefore specified, where before it was not:** run the test under a
deliberately held target lock and count. If that reproduces at a high rate, bisect; if it does
not, the peer's 2-of-2 was coincidence and we are back to a rate, having learned which variable
is innocent.

#### RETRACTED, same day, by the reporter — and the retraction is the finding

`fix-subagent-guide-injection-brief` re-checked their own raw buffers and withdrew the
condition within the hour:

- **Run 1** (chained `clippy && lean && default`): two `Blocking waiting for file lock` lines,
  at buffer lines 4 and 3643 — immediately after `===CLIPPY-OK===` and `===LEAN-OK===`. Each
  was a wait at a lane's **build start** that resolved before any test ran, not a lock held
  through test execution.
- **Run 2** (standalone `cargo test --workspace`): **zero** lock-wait lines. Failed anyway.

So lock contention is **not necessary** for the failure, refuted by the reporter's own second
reproduction. Their summary: *"I had a coincidence and named a mechanism for it."*

**Everything above that rests on the condition is withdrawn**, specifically: that these were
observations with a *named condition* rather than a rate; that a bisect is thereby feasible;
and the "next probe is specified" paragraph, which would have spent effort probing a variable
the reporter had already accidentally controlled for, in the direction of innocence. The
superseded text is kept rather than deleted because what was believed, and why it was wrong,
is the transferable part.

**What survives at the strength the evidence supports:** two more failures of
`cargo test --workspace` at full scope (integration binaries included) on a box with ~6 live
sessions. One started behind a transient build-lock wait; the other did not. No discriminating
factor. Rate, not condition — so the bisect is **infeasible** exactly as before, and
`unverified:` is restored to say so.

**The surviving candidate is executor churn, and it comes from this session's table rather than
from either reporter's hypothesis.** Mutation A had 11 other tests failing in the same async
executor; no passing run shared that. But it does not survive its own first check: this
session's default-lane run had exactly **one** other failure and this test passed, and the
peer's two runs almost certainly also had exactly one other failure — their own
`prompt_surfaces_server_instructions_snapshot`, which was red in the tree at the time — and
this test failed. Same other-failure count, opposite outcomes. That is not a refutation at
n=1 either way, and the discriminating question is now precise enough to ask rather than
model: **how many other tests failed in each of their two runs?** Asked; unanswered at the
time of writing.

**Method note, and the reason this subsection exists at all.** I recorded a peer's condition
after verifying their *arithmetic* on an unrelated claim earlier the same evening, and did not
ask how they had established *this* one. A characterisation of one's own observation is a
claim like any other; that it comes with a number attached ("2-of-2") makes it read as
measurement rather than as attribution. The hedging in the text above — "candidate, not a
finding" — was applied to the *causal* reading and not to the *observational* one, which is
exactly where it was needed.

### Reproduced 2026-09-02, and the confirmation is the point

Two further observations, both under `cargo test --workspace` while a peer session was
building concurrently on the same machine: failed in the full run, **passed in isolation
immediately after**, twice. Same assertion, same 10s-vs-1s message.

Recorded because this repo's testing discipline treats a re-derivation that *confirms* as a
denominator rather than a non-event: without it the file accumulates only the runs that
failed, and "how often does this fire under load" has no divisor. It adds no new
diagnosis — the load-sensitivity was already the diagnosis — and it is **not** a second
per-instance fix request.

Incidental, and the reason it was noticed: it is the only red in an otherwise clean gate
(lean `--no-default-features` 3441 pass / 0 fail; default 4833 pass / 1 fail), so it reads
to an unrelated author exactly like their own regression until they check the ledger. That
cost is paid per author, not per occurrence.

### Fifth observation, 2026-09-02 — the signature is NOT confined to this test, and that falsifies the paragraph above

Gate run during unrelated work (`GG-3`, a guide-emission refactor touching only
`src/tools/core/`). Three runs, back to back, same tree, same machine:

| run | command | reds |
|---|---|---|
| 1 | `cargo test --workspace` | **14** |
| 2 | `cargo test --workspace --lib` (alone) | **0** — 4905 passed |
| 3 | `cargo test --workspace` again | **1** — this test only |

Run 1's other thirteen, all previously unrecorded on this file:

- `retrieval::sync::tests::*` — **twelve**, including both
  `sync_project_holds_index_lock_for_its_full_duration` and
  `sync_worktree_holds_index_lock_for_its_full_duration`,
  `sync_project_fails_fast_on_a_dim_mismatch_before_touching_the_store`, and
  `sync_project_sizes_a_fresh_collection_from_the_unpinned_local_embedder`.
- `retrieval::index_state::tests::a_live_binary_does_not_report_itself_deleted` — one.

**What this corrects.** The section immediately above says this test *"is the only red in an
otherwise clean gate, so it reads to an unrelated author exactly like their own regression
until they check the ledger."* That was true of every run recorded here before today and is
false of run 1. The reading cost it names gets **worse**, not better: fourteen reds across
three modules do not read as one known flake, they read as a broken build — and the author
who sees them is, by construction, someone who just changed something else.

**What is established, and what is not.** Established: all fourteen pass in isolation and
twelve of the thirteen new ones sit in one module. **Not** established: that they share this
file's mechanism. The assertion text was **not captured** — the isolating re-run passed, so
the stdout blocks that would name the failure do not exist, and run 3 did not reproduce
them. Two lock-duration tests failing together is *suggestive* of contention on the index
lock, which is a different resource from this test's idle timer. Treat the shared signature
(red under `--workspace`, green under `--lib`) as a **lead**, not as membership.

**Next, and deliberately not done here:** capture the assertion text by looping
`cargo test --workspace` with stdout retained until the cluster recurs. If it names the
index lock rather than a timeout, it is a **separate bug file**, not a step of this class —
and recording it here as membership would be the elimination-by-plausibility this repo
keeps paying for. Recorded on this file only because the signature was observed in the same
run as this test, which is a fact about the run rather than about the cause.
### Sixth observation, 2026-09-02 — the assertion text the fifth asked for, and it names the timeout

Gate run during unrelated work (the `description`↔`action`-enum parity gate, touching only
`src/server.rs` tests and two description string literals — nothing under `src/peer/`).

| fact | value |
|---|---|
| result | `4927 passed; 1 failed` — **this test only**, not the fifth's 14-red cluster |
| assertion | `run() did not exit within 10s of a 1s idle timeout`, `src/peer/server.rs:746:9` |
| load | directly observed — a peer session held the cargo build lock; this session's own earlier commands logged `Blocking waiting for file lock on build directory` |
| isolated re-run | `cargo test --lib run_exits_after_idle_timeout_with_no_connections` → **ok in 1.14s**, no code change between runs |
| full re-run | `cargo test --workspace` → **exit 0**, whole lane green |

**This is the capture the fifth observation asked for and deliberately did not do.** The
stdout block exists this time, and it names *the idle timer* — not the index lock. So for
this occurrence the mechanism is the one this file has claimed since it was opened, and the
`retrieval::sync` lock-duration lead is **neither confirmed nor implicated**: those thirteen
did not recur here at all.

**What that does and does not settle.** It settles that a lone red with the timeout assertion
still happens, under load, on a tree with no `src/peer/` change — one more datapoint for the
original class, with the evidence attached rather than inferred. It settles **nothing** about
whether the fifth's cluster shares this mechanism; a run in which the cluster did not appear
cannot speak to the cluster. The fifth's `next` step stands unclaimed.

**Published as a denominator, not as a catch** (`CLAUDE.md` § *Testing Discipline*): the
re-derivation *confirmed*, and a confirmation that goes unrecorded makes the population look
self-correcting. Counting only the surprises is how a load-sensitive class stays at "flaked
once".

### Seventh observation, 2026-09-02 — twice in one session, plus a directly-observed mechanism for the FIFTH's cluster

Gate runs during the `read_only`-inert fix (`docs/issues/archive/2026-09-02-read-only-true-is-inert-at-every-root.md`).
Three full gate runs on one tree, ~40 minutes apart, with peers committing throughout (HEAD
moved 6 commits in ~22 minutes and the tree carried peer *uncommitted* edits to
`src/prompts/mod.rs`, `src/server.rs`, `src/librarian/tools/doctor.rs`).

| run | lean | default | failures |
|---|---|---|---|
| 1 | `0` | `101` | **this test** + `tool_surface_under_budget` (that one genuinely mine) |
| 2 | `101` | `101` | `build_with_workspace_appends_project_table` (peer's in-flight fix) + `a_live_binary_does_not_report_itself_deleted` + **5× `retrieval::sync::tests::*`** — this test did **not** fail |
| 3 | `0` | `101` | **this test only**, `src/peer/server.rs:752`, `run() did not exit within 10s of a 1s idle timeout` |
| 4 | `0` | `101` | **this test only**, on a diff that is **comment-only across every file** and does not touch `src/peer/server.rs` at all — the archive-move sweep, ~40 min after run 3 |

**Denominator: 7 isolated re-runs, 7 green** (4 after run 1, 3 after run 3), no code change
between any of them. Recorded per § *Sixth observation* — a confirmation that goes unrecorded
makes the population look self-correcting.

**A sharper non-attribution than previous observations could offer.** Runs 1 and 3 happened on
a tree where `src/peer/server.rs` **was** modified — so "nothing under `src/peer/`" was not
available as the argument. Instead: `git diff -U0 src/peer/server.rs` piped through a
comment/header filter left **zero** changed lines, i.e. every changed line is a `///` doc
comment. A comment cannot change timing, so the diff is excluded by mechanism rather than by
adjacency.

**Run 2 is the interesting one, and it speaks to the fifth observation's open question.** The
`retrieval::sync` cluster reappeared, and this time it arrived next to an assertion that names
a mechanism:

```
---- retrieval::index_state::tests::a_live_binary_does_not_report_itself_deleted ----
assertion `left == right` failed: the test runner's own executable exists, so this must be
a definite false — None would mean 'could not tell' and true would mean the predicate is
inverted
  left: Some(true)
 right: Some(false)
```

The predicate reported the running test binary as **deleted** — which is *correct* when a peer's
concurrent `cargo` replaces `target/debug/deps/*` mid-run, since the executing inode is
unlinked. This session logged `Blocking waiting for file lock on build directory` on five
separate commands, so concurrent builds are directly observed, not inferred.

**What that does and does not settle.** It offers the fifth's cluster a candidate mechanism
that is **not** the idle timer and not lock duration: a shared `target/` mutated by another
process during the run, which would plausibly disturb any test that stats build artefacts or
index state. It is a *lead with an observed instance*, not a confirmation — nobody ran the
cluster against a quiescent `target/` to see it disappear, and that is the next step. It also
settles that the two signatures are **dissociable**: run 2 produced the cluster without this
test, runs 1 and 3 produced this test without the cluster. A single shared cause would have to
explain that dissociation.
### Eighth observation, 2026-09-02 — load COUNTED rather than inferred, and a diff with no Rust in it at all

Gate run during the `GIT_INDEX_FILE` recorder fix
(`docs/issues/archive/2026-09-02-a-refused-pathspec-commit-stamps-your-own-content-unowned.md`).

| run | lean | default | failures |
|---|---|---|---|
| 5 | `0` | `101` | **this test only**, `src/peer/server.rs:752`, `run() did not exit within 10s of a 1s idle timeout` |
| 6 | *(not re-run)* | `101` | **this test only**, same assertion — a deliberate re-run of the default lane alone, ~6 minutes after run 5, no code change |

**Denominator: 8 isolated re-runs, 8 green.** The eighth took **1.14s against a 10s timeout** —
a ~9× margin — immediately after run 5, no code change between.

**Run 6 is the new shape here, and the denominator above does not describe it.** Every prior
observation pairs a red full lane with a green isolated re-run, which reads as "transient".
Run 6 shows the full lane failing **twice consecutively on the same test** while the isolated
re-run stays green — so under *sustained* load the failure is not intermittent at lane grain,
only at test grain. That distinction matters for the fix discussion: a retry-once mitigation
would not have cleared this session, and "re-run it and it passes" is true of the test in
isolation and false of the lane.

**The load figure is a count this time, not an inference.** Previous observations argue load from
`Blocking waiting for file lock on build directory` lines and from peers committing. This run has
a number: **19 live sessions across 3 profiles, 7 of them in this checkout (6 peers plus me)**, by
socket enumeration at the time of the run (`scripts/peer-sessions.sh`'s method — enumerate
`/run/user/<uid>/cc-socks/*.sock`, resolve each PID's `CLAUDE_CONFIG_DIR`). Two peers were
observed staging into the shared index during the same window, and a peer `cargo` had the build
lock. That is the first membership count attached to an instance of this bug rather than a
qualitative "peers were active".

**Non-attribution is by mechanism and is stronger here than in run 4.** Run 4 could say the diff
was comment-only. This one can say something simpler and tighter: `git status --short -- '*.rs'`
returned **empty**. The working tree contained no Rust modification of any kind — the change was
one shell script, one shell test, and markdown. There is no line for a reader to inspect and
therefore no adjacency argument to make; the diff cannot reach `src/peer/server.rs` because it
cannot reach any `.rs` file.

**What this adds to the class, and what it does not.** It supplies the quantified load the earlier
observations wanted and a cleaner exclusion, so it strengthens *load-sensitivity* as the reading.
It does **not** advance the fix: nobody re-ran this against a quiescent machine to watch it
disappear, which remains the step § *Seventh observation* named and nobody has taken. Recording a
confirmation that changes no verdict is the point — per § *Sixth observation*, an unrecorded
confirmation makes the population look self-correcting.

## Hypotheses tried
- *Named in a prior flake file?* No — `2026-08-26-wine-lane-flakes-under-load-on-three-tests`
  narrowed itself to one unrelated test (`run_migrations_is_safe_under_concurrent_connections`).
- *Deterministic break from the T-7 diff?* Ruled out: 2/2 clean full runs and 5/5 isolated on
  the same commit.

## Fix
Two options, and the second is the one worth taking.

1. **Raise this test's budget**, as 2026-07-03 and 2026-07-05 each did for their step. Cheap,
   and it is the move that produced two recurrences — it treats the instance.
2. **Enumerate the class once.** Grep `src/peer/` and `src/lsp/` for every test asserting a
   real-time property (a sleep, a deadline, a bounded retry loop, an "exits within N"
   assertion) and decide per site: generous budget, or an event-driven wait that does not
   assert about the scheduler at all. An idle-timeout test in particular can usually observe
   the exit rather than time it.

Explicitly rejected again, for the reason the prior file gives: `#[serial]` does not help,
because the contention is CPU starvation from the *other* ~4800 tests, which serializing these
among themselves does not relieve.

## Tests added
N/A — not started. Note the prior file recorded *"Tests added: None — the regression signal is
the existing suite staying green"*, and that signal is precisely what a low-rate flake defeats.

## Workarounds
`--test-threads=1` for a decisive full-suite answer (5030/5030 today). Do not read a single
green parallel run as evidence the class is gone.

## Resume
Start with option 2's enumeration — the count is the thing nobody has. If it is three sites,
fix all three now; if it is fifteen, that is a different conversation and worth knowing before
raising a fourth budget.

## References
- `docs/issues/archive/2026-07-03-parallel-test-suite-peer-and-mux-lock-flakiness.md` (fixed,
  with its own 2026-07-05 recurrence note)
- `src/peer/server.rs` `connect_with_retry`; `src/lsp/manager.rs`
  `claim_mux_lock_some_when_free_none_when_held`
- `docs/trackers/catalog-audit-trail-session-log.md` — observed during the T-7 SDD run
