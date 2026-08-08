---
id: e6697aea0ee3ea37
kind: tracker
status: active
title: Release Promotion Session Log
tags:
- session-log
- release-promotion
- reconnaissance
---

> Per-work-stream friction/win log for the `experiments` -> `master` promotion
> (79-commit fast-forward, local range `eca9902e..339cea47`). Copied from
> `docs/templates/session-log.md`; see that file for the full status vocabulary
> and entry templates.

> Per-work-stream friction/win log for the recurring `experiments` -> `master`
> promotion. Copied from `docs/templates/session-log.md`; see that file for the
> full status vocabulary and entry templates.
>
> **Round 1 — 2026-07-02.** 79-commit fast-forward, local range
> `eca9902e..339cea47`. Shipped. Produced F-1.
> **Round 2 — 2026-08-06.** 397-commit fast-forward. **Not shipped** — see the
> Resume block below. Produced F-2, F-3, W-1.

## Resume — round 9, written 2026-08-07. Supersedes rounds 2-8.

### First action: `TaskList`. Only #14 remains.

Every other task (#15-#32) is complete. Do not re-derive the backlog from this log — the task list is
authoritative and each entry carries its evidence.

### Round 17 addendum (2026-08-08) — CURRENT. Read this before the git-state section below.

Rounds 16 and 17 both landed on 2026-08-08. This block is round 17 and carries the current
state; round 16's numbers (tip `e20b3a04`, 6 open bugs) are superseded and kept only in F-20.

**Current state, verified 2026-08-08 — not remembered:**

- tip **`88316ac9`**, local == remote. **496 ahead / 0 behind local `master`** — still a clean
  fast-forward.
- CI run **`31249753080`: green, all 15 jobs**, on that exact SHA. The tip has not moved since.
- **PR #10 was merged into `experiments`** (merge commit `9be2ede4`) — option (b) of the
  promotion choice. Merged with `--merge`, deliberately: `--rebase`/`--squash` would have
  re-minted 18 SHAs and re-orphaned the citations repaired hours earlier (F-22).
- **Two masters.** Local `master` = `5a8b1469`; `origin/master` = `eca9902e`, 139 commits
  behind local. All 139 are contained in `experiments`, and `origin/master` IS an ancestor of
  `experiments`, so both hops are genuine fast-forwards — the push moves the remote 635
  commits, not 496. **Decided 2026-08-08: local `master` is the reference**, and
  `docs/RELEASE.md` is deliberately NOT changed. Do not re-raise this.
- `docs/trackers/pr-review-session-log.md` remains the concurrent session's. Never stage it.

### THE MERGE IS STILL THE MAINTAINER'S TO RUN

`#14` is the only open task. Everything gating it is satisfied. Do not run it.

```
git checkout master
git merge --ff-only experiments
git push
```

The gate is a green run on **whatever the tip is at that moment**, never a SHA from this log —
and note F-23: that rule is a **push**-event rule. A `pull_request` run tests
`refs/pull/N/merge`, a synthetic commit on no branch, which answers a different question.

### Open bugs — 10, and none blocks the merge

Three were filed today from the PR #10 review and are the newest:

- `feb6a21337316dfb` — the dangerous-command gate tokenizes with `split_whitespace` while
  commands execute under a POSIX shell; `posix_tokenize` was written for this and has **zero
  production callers**.
- `14f61fb867874a5b` — **the sharpest one.** `is_buffer_only` only recognises `/`-bearing
  words, so `cat @cmd_x && rm -rf ~` classifies as buffer-only and skips the dangerous-command
  gate *and* the source-file block entirely. Verified at the bytes
  (`output_buffer.rs:618`, `inner.rs:288`). Pre-existing on Unix; PR #10 makes it live on
  Windows by replacing `cmd.exe`, where `~` was literal.
- `f95e6841030e4e9c` — doctor's outside-roots sample is an unranked prefix and the 281 elided
  rows are unreachable by any parameter (`limit=100` accepted, silently ignored, byte-identical
  result).

All three were filed rather than fixed on purpose: the first two change a **security gate's**
behaviour and belong in a change whose reviewers are looking at that gate; the third changes an
output contract.

**A reindex was needed to see them.** Bug files created with `create_file` (rather than
`artifact(action="create")`) are not catalogued, so `artifact(find, kind="bug")` reported 7
open when there were 10. Run `librarian(action="reindex")` before trusting any "what's open?"
count that follows a session which wrote bug files by hand.

### Do NOT re-do — settled or falsified on 2026-08-08

- **Do not rebase PR #10's lineage.** It is merged. A rebase would orphan the SHA citations a
  third time (F-22).
- **Do not "fix" the local-vs-origin master gap.** Decided; local master is the reference.
- **Do not re-review PR #10.** Four lenses, five blockers, all fixed and merged (W-15).
- **Do not trust a `#[cfg(windows)]` test as evidence on Linux.** It is not compiled there.
  `cargo check --target x86_64-pc-windows-gnu --tests` is ~6 s warm (W-16).
- **Do not read a fixture that spells both sides the way the code spells them as coverage**
  (F-21).
- The `docs/PROGRESSIVE_DISCOVERABILITY.md` filename has no `DISCLOSURE` variant; the audit
  will not catch the wrong one inside `docs/issues/` because that surface drops one severity
  band.

### Post-merge debts

- master-side SHAs for bug files archived carrying `experiments` SHAs
  (`grep -rl <sha> docs/issues/`, including `archive/`).
- Unreleased callouts come off at **release**, not at merge.
- A fast-forward mints no SHAs, so every citation repaired today survives the promotion intact.
### THE MERGE IS UNBLOCKED AND IS THE MAINTAINER'S TO RUN

This inverts round 8's header. The deferral was *"no merge yet until we solve all issues"*; that
condition is met. Eleven decisions were taken on 2026-08-07 and every blocker cleared.

**Do not run the merge yourself.** `master` is protected and the promotion is the maintainer's.
What to hand over:

- Sequence: `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)* — ancestry check FIRST, then
  `--ff-only`.
- **The gate is a final green CI run on whatever the tip is at that moment. Never a SHA from this
  log.** Round 8 recorded this rule; it still holds.
- Owed *after* the promotion, both mechanical: master-side SHAs for every bug file archived carrying
  an `experiments` SHA (they orphan on rebase — `grep -rl <sha> docs/issues/` including `archive/`),
  and the unreleased callouts come off at **release**, not at merge.

### Git state (verified 2026-08-07, not remembered)

- tip `9886773e`, `experiments` == `origin/experiments`
- **471 ahead / 0 behind** `master`
- working tree clean EXCEPT `docs/trackers/pr-review-session-log.md`, which belongs to a **concurrent
  session** in the same checkout. Do not stage it. See F-15/F-18.
- toolchain now PINNED at 1.97.1 (`rust-toolchain.toml`), so the local gate reproduces CI exactly

### The eleven decisions, and what each became

| # | decision | outcome |
|---|---|---|
| 16 | idle-shutdown value | **none — stays disabled.** 18 servers, 18 live parents, 1.13 GiB of 125. Bug closed `wontfix` + archived |
| 17 | audit tally direction | ResolvedBasename counts as resolved; tree-sitter cross-check for cold LSP. Both shipped |
| 18 | fix the tally | `6e608545`, CI 15/15. Counters partition (residual 2426 -> **0**) |
| 19 | sparse leg | **back on, flag-gated.** The flag was saving nothing — the container held 2.69 GiB either way |
| 20 | reranker default | **off by default**, `CODESCOUT_RERANK` opt-in (`6ce49487`) |
| 21 | the reindex | ran: **8.06 min**, not the inherited ~2h. Sparse coverage 92.3% -> **100%** |
| 22 | chunker fix | **deferred**, so one rebuild served both |
| 23 | kotlin cherry-pick | **rides with the promotion** |
| 24 | kotlin item 6 | **dropped** |
| 25 | researcher reranker | **descoped** — different project, does not gate codescout |
| 26 | toolchain pin | **pinned 1.97.1** + components + targets; 5 CI install steps REMOVED |

### Open bugs — 5, and none blocks the merge

`artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})`

1. **AST chunker** — deferred by decision; title corrected (the "~2h" was 8.06 min)
2. **researcher rerank** — descoped; a `researcher` defect filed here for umbrella reasons
3. **kotlin heap** — retitled to what remains (mem-kill not counting toward the circuit breaker); the
   original "no `-Xmx`" premise was falsified by one `jcmd`
4. **`semantic_search` latency unaccounted** (NEW) — ~950 ms of a 990 ms round-trip is outside the
   measured retrieval stages. Next step written down: two `timer.lap` calls, seams already exist
5. **`edit_code remove` over-deletes** (NEW) — a real codescout defect; took a function's closing
   delimiters while "repairing" a truncated LSP range and returned `status: "ok"`

### Do NOT re-do — measured, settled, or falsified today

1. **Do not re-recommend capping `SymbolMissing` below `high`** — shipped long ago; that is why 99
   invocations never reproduced the gate flap.
2. **Do not chase `degraded` being "permanently true"** — it inverted; it was *silent*, now honest.
3. **Do not hunt the audit gate flap with back-to-back runs** — warm runs are exactly deterministic;
   42 attempts of that shape found nothing.
4. **Do not trust `cargo build --release` for anything touching retrieval** — it omits
   `server-stack`, so the backend silently becomes sqlite-vec and sparse is skipped. Use `cargo rb`
   (F-17).
5. **Do not reindex to "fix" sparse coverage before checking it** — 92.3% already had populated
   sparse vectors; `.env.gpu` claimed a rebuild was mandatory.
6. **Do not quote the reranker's "42x"** — it is 1.6x; ~569 ms absolute, and the denominator moved.
7. **Do not fold the first query of a benchmark into a mean** (W-14) — cold-start inflated one figure
   8.7x.
8. **Do not `pgrep -f`/`pkill -f` a pattern that appears in your own command** (F-19) — three
   failures in one hour, one of them a self-kill.
9. **Do not start a long job through `run_command` without `setsid`** (F-18) — a routine `/mcp` kills
   it and takes the `@bg_*` buffers.
10. **Do not put a `#[cfg(test)] mod` above other items** — clippy 1.97's `items_after_test_module`.
11. **Do not use inline `git commit -m` for bodies** (F-16) — backticks are command substitution.
12. **Do not `git add -A`** while the concurrent session has uncommitted work; and **push by
    refspec** (F-15).

### What round 9 changed — 15 commits, `5c209f28..9886773e`

Discharged W-12/W-13 into permanent docs; pinned the toolchain; re-enabled sparse; rebuilt the index;
fixed the audit tally and the cold-LSP false-missing; made the reranker opt-in; closed and archived
two bugs and filed two; corrected three bug titles that asserted numbers measurement had killed.
Added F-14 through F-19 and W-13/W-14.

### The pattern this round is about

**Premises decay faster than prescriptions, and numbers decay silently.** W-13 found five bug files
with false premises; round 9 found the same disease in *titles* (three of five), in an *inherited
estimate* (~2h vs 8.06 min), in a *ratio* (42x vs 1.6x), and in a *task I wrote myself* (#21's three
premises, one of which held). Nothing enforces re-checking a stated number, and a title is the part
nobody re-reads critically while being the part every triage reads first.

## Resume — round 8, written 2026-08-07 for session compaction

**Supersedes rounds 2–7.** The merge is **deliberately deferred** — see below. Everything actionable
without a maintainer decision is done; **there is a live task list, use it.**

### First action for the next session: `TaskList`

This round created a **16-task list (#14–#30)** which is the authoritative view of what is open, who
owns each item, and what blocks what. Read it before re-deriving anything from this log. #15 is the
only completed one. Every remaining task carries its bug path, the constraints already established,
and the traps not to re-walk — several are long because they encode a full investigation.

### Git state (verified, not remembered)

| | |
|---|---|
| `experiments` HEAD | **`5432f5c7`**, plus the docs-only commit carrying this Resume — confirm with `git log --oneline -1` |
| working tree | clean at time of writing |
| `master...HEAD` | **`0` left / `456` right** — `master` has no commits of its own, so the promotion is still a fast-forward |
| last CI-verified SHA | **`5432f5c7`** — 15/15, attempt 1 |

### THE MERGE IS DEFERRED BY DECISION — do not run it

2026-08-07, explicit instruction: *"no merge yet until we solve all issues."* Task **#14** is
blocked by the twelve bug-backed tasks and must stay that way. Do not offer the merge again until
those close.

Waiting costs nothing mechanically — `master` is protected and nothing else touches it, so the
fast-forward stays clean however long the branch grows. Two consequences to carry:

- **The merge must follow a FINAL green run on whatever the tip is then.** Every commit supersedes
  the previously-verified SHA. Do not reuse a SHA from this log.
- **#28 and #29 are trigger-gated on a recurrence, so "all issues" is not reachable by working
  harder.** Both were mitigated and made self-diagnosing but never reproduced. They resolve either
  when they fire again, or by a decision to close them `zombie` with the re-open triggers already
  written into each. Flagged to the user; unresolved.

### CI — four reading rules, each bought with a wrong conclusion

Read runs per-job, never from run-level fields:

```
gh run view <id> --json jobs --jq '.jobs[] | "\(if .status=="completed" then .conclusion else .status end)\tsteps=\(.steps|length)\t\(.name)"'
```

1. **Run-level `status` is an aggregate, not a result** — it reported `queued` for a run already
   holding six green jobs.
2. **`steps` separates scheduling from testing** — `steps=0` never executed; `steps=12` ran and
   really failed.
3. **`attempt` separates a clean green from a rescued one.**
4. **`cancelled` on a non-tip SHA is the concurrency block working**, not a failure — a newer push
   superseded it. Two of this round's runs read that way (`94f7660d`, `65075c6f`).

### What round 8 changed — 13 commits, `176a77e5..5432f5c7`

- **WIN-30 half fixed** (`176a77e5`). The LSP budget test moved to `#[tokio::test(start_paused =
  true)]` **and** `std::time::Instant` → `tokio::time::Instant`; the second half matters as much as
  the first, because `std::time::Instant` is not virtualised and would have left the ceiling
  trivially true. Now `0.00s`, 25/25, mutation-verified at 40 ms. The background-output test is
  *mitigated* — its poll no longer discards `Err`, bound 5 s → 15 s. Its first run under wine named
  the missing `py`, which **retired one member of WIN-27's twelve unknown-root-cause wine
  failures.**
- **`grep` zero-match false negative fixed** (`624f7f05`, refined `cdfbbe0f`, archived `4ddbcfca`).
  `WalkAudit` + `completeness_warning`, four counted error sites, directories sorted before dotfiles
  so truncation keeps the useful entries. Filed and closed same session.
- **audit-doc-refs non-determinism FORCED** (`b97a3f48`) — 57 paired warm/cold runs. Retitled: the
  tally is non-deterministic, the **gate is not**.
- **MCP orphans: nothing is orphaned** (`1e2883a9`). Retitled. Mechanism forced to an idle timeout.
- **kotlin heap capped twice over** (`2cfefc2e`) — distro `vmoptions` ships `-Xmx2048m`.
- **researcher rerank measured, then retracted** (`65075c6f`, `2a20977f`, `436f8710`).
- **`conventions` § Environment-Agnostic Tuning** (`94f7660d`) — never ship a measured constant as a
  default; we can only test for us.
- **Opt-in idle shutdown shipped** (`5432f5c7`) — `CODESCOUT_IDLE_SHUTDOWN_SECS`, disabled by
  default, mechanism only, threshold left to the operator.

Tracker entries added: **F-11, F-12, F-13, W-11, W-12, W-13, R-62, U-39**, U-33 refined to 16
instances, WIN-30 row → `mitigated`. Memories: `test-design-discipline` gained the cap/truncation
corollary; `conventions` gained Environment-Agnostic Tuning.

### Do NOT re-do

1. **Do not offer or run the merge** — deferred by decision, task #14 blocked.
2. **Do not re-derive CI state from run-level fields**; and `cancelled` on a superseded SHA is not
   a failure.
3. **Do not hunt the audit-doc-refs gate flap with back-to-back repetition.** 42 attempts of that
   shape found nothing and the reason is known: back-to-back runs are always mux-warm, and warm runs
   are byte-identical. The mux self-exits at `--idle-timeout 180`, which is the real oscillator.
4. **Do not look for a broken stdin-EOF path in `start`.** Verified correct —
   `start --debug < /dev/null` exits in ~30 ms. `ResilientStdin` absorbs only `WouldBlock`.
5. **Do not add `-Xmx` for kotlin-lsp** — the distro's `vmoptions` already caps at 2 GiB and is
   honored (`InitialHeapSize=128 MiB` proves it).
6. **Do not run the kotlin content-root scoping experiment** before deciding #24 — no heap pressure
   remains and #203 is open/unfixed/silent for 3 months.
7. **Do not tune `RERANK_MIN_SCORE`.** No constant works on `ms-marco-MiniLM-L-6-v2`; the bands
   overlap at n=28. And do not "make it relative" either — scale-free does not conjure absent signal.
8. **Do not make WIN-30's background test self-skip on a missing `py`** — a probe-gated early return
   would pass vacuously on `windows-latest` and disarm the only guard that matters.
9. **Do not run `index --force` before #19's sparse decision** — chunks written under
   `DISABLE_SPARSE=1` carry empty sparse vectors, so re-enabling later mandates a second rebuild.
10. **Do not conclude absence from a glob.** Three surfaces this session: `ignore`'s hidden-dir
    prune, `cargo test --lib` filtering integration tests to "0 passed", and shell `*.json` not
    matching `.claude.json`. Pass `include_hidden=true`; use `--test <target>`.

### Open bugs — 8, unchanged in count

```
artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})
```

One filed and closed this round (grep), so the count held. **None is blocked on investigation any
more** — every one is blocked on a decision, or on a recurrence. That is the round's real outcome.

### What is needed from the maintainer — 11 decisions

**One answer unblocks three:** #19, does the sparse leg come back? It gates #20 and #21. A VRAM
call, not a quality one — sparse was worth ≥ 2/75 and cost ~3.0 GiB of a 6 GiB card. Recommendation:
leave it off, which closes #19 as moot and makes #21 a plain 2 h rebuild.

**Genuinely needs thought:** #17 (audit-doc-refs — tally source + `degraded` semantics + reclassify
the unreproduced symptom) and #22 (chunker candidate, though its file orders the
embedder-batch-concurrency work first).

**A "yes" is enough, recommendation already in each task:** #16 (suggest 14400), #23 (ship, keep
`open`, drop severity to medium), #24 (drop item 6), #25 (delete the knob), #26 (pin the toolchain —
closes W-9 *and* F-9), #27 (add the live-surface step — promote-when met), #30 (label
`BM25_BOOST`, don't change it).

**Needs a close, not a fix:** #28, #29 — `zombie` with the recorded re-open triggers, else the merge
waits on a flake.

### Still owed

- **`index --force` rebuild** (~2 h) — blocked on #19, not on effort.
- **`docs/RELEASE.md` live-surface step** — W-12's promote-when is met; #27.
- **Bug-file template `## Premise check`** — W-13's promote-when is met; not yet filed as a task.
- **Master-side SHAs** for archived bug files, after the eventual cherry-pick.

## Resume — round 7, written 2026-08-07 for session compaction

**Supersedes rounds 2–6.** The promotion is one command away and that command is the user's.

### Git state (verified, not remembered)

| | |
|---|---|
| `experiments` HEAD | **`3bfa4025`** |
| working tree | clean |
| `master...HEAD` | **`0` left / `442` right** — `master` has no commits of its own, so the promotion is a fast-forward |
| pushed | yes |

### CI — and how to read it in this repo, which cost three wrong conclusions to learn

| run | SHA | result |
|---|---|---|
| 31151333288 | `2d824f15` | **15/15**, attempt **2** (two flaky jobs re-run) |
| 31152786383 | `f7fefa96` | **15/15**, attempt **1** — clean, no intervention |
| 31155578139 | `3bfa4025` | **15/15**, attempt **1** — clean, no intervention |

All three SHAs above are confirmed green. But **the docs-only tracker commit carrying this Resume
moves the tip past `3bfa4025`**, so by the time you read this the tip's own run is unverified —
structural, not an oversight: recording the verification invalidates the verification of the tip.
So the Git-state table's HEAD is `3bfa4025` **plus that docs commit**; confirm with
`git log --oneline -1`. Either wait for the tip's run, or fast-forward to a SHA in the table
(`git merge --ff-only 3bfa4025`), which promotes 442 commits with a clean 15/15 behind it and
leaves the docs-only commits for the next cycle. Read any run per-job:

```
gh run view <id> --json jobs --jq '.jobs[] | "\(if .status=="completed" then .conclusion else .status end)\tsteps=\(.steps|length)\t\(.name)"'
```

Three rules, each learned by getting it wrong first (F-7, F-8, WIN-30):

1. **Run-level `status` is an aggregate, not a result.** A run reports `queued` while its own
   jobs run, pass, and get cancelled — `gh run list` said `queued` for a run already holding six
   green jobs.
2. **`steps` separates scheduling from testing.** A non-success job with `steps=0` never executed
   (scheduling/outage); `steps=12` means it ran and really failed. Three runs on 2026-08-06
   reported run-level `failure`/`cancelled` with **zero** executed jobs.
3. **`attempt` distinguishes a clean green from a rescued one.** `2d824f15`'s 15/15 needed a
   re-run of two flaky jobs; `f7fefa96`'s did not.

GitHub Actions was in a **declared major outage** for most of 2026-08-06 (incident opened
15:22:49Z). It is resolved — "All Systems Operational" as of 2026-08-07 ~05:40Z, and runs are
being created within seconds of a push again.

### The promotion gate — all of `docs/RELEASE.md` § Large-Cohort Promotion except step 5

| step | state |
|---|---|
| 1. ancestry is a fast-forward | ✅ 0 left / 442 right |
| 2. clean working tree | ✅ |
| 3. local gate | ✅ fmt, `clippy --all-targets -D warnings`, **3515 passed / 0 failed / 44 ignored** — and separately green on **CI's own toolchain 1.97.1** across all three feature configs (8680 tests) |
| 4. documentation gate + fresh-clone audit | ✅ 0 high, `degraded: false`; `CHANGELOG.md` `[Unreleased]` covers the cohort; `experimental/index.md` no longer claims the branch is empty |
| **5. `git merge --ff-only experiments`** | ⬅ **the user's to run** — `master` is protected |

After the merge: nothing to rebase, both refs point at the same commit. The unreleased-cohort
callouts **stay** (they come off at release, not at promotion), and archived bug files citing
`experiments` SHAs stay valid because a fast-forward mints no new SHAs.

### What round 7 changed

- **`audit_doc_refs` gitignore cap consulted `.gitignore` but not the index** (`598624be`) —
  `/.github/` was ignored while five files beneath it were tracked, so a doc citing a *deleted*
  workflow was capped `high` → `med`, under the `fail_on: high` gate. Fixed with an index-derived
  `TrackedIgnore`; own-path rules still decide, and only a parent-only match consults the tracked
  set. Bug archived, verified by regression test + mutation + real binary + the live MCP path.
- **`/.github/` narrowed** to `/.github/copilot-instructions.md` (`6ca02767`) — the single
  generated file `4dac3a3c` was actually suppressing.
- **clippy 1.97 fix** (`fb3b1a05`) — found by installing CI's *resolved* toolchain side by side
  (`cargo +1.97.1`) during the outage. Local `stable` is 1.95.0; CI's floats and had moved to
  1.97.1. See **W-9**; this is F-5's first observed instance and its first mitigation.
- **kotlin bug: items 3 and 5 closed.** NMT live-verified by `jcmd` with a control; item 5 was
  already done twice over (host runs `262.9593.0`). Three concurrent instances measured flat at
  ~520 MiB / 506 MiB / **545 MiB at 10h53m** — four clean post-upgrade lifecycles now. Severity
  deliberately left `high`: #203 is still open upstream and all three captures were lightly used.
- **WIN-30** — two Windows timing flakes; both passed on an unchanged re-run.
- **`symbols` 0-match flake** (`3bfa4025`) — both walks used `walker.flatten()`, silently
  discarding every `ignore::Walk` error, so a truncated walk looked complete and zeroed both
  search paths at once. Now counted, and a zero declares whether it can be trusted.

### Do NOT re-do

1. **Do not re-derive CI state from run-level fields.** Use the per-job command above.
2. **Do not merge the two `symbols` walks.** They look redundant and are a recovery path — W-10.
3. **Do not rebuild an LSP-warming harness for the symbols flake.** W-8 killed that hypothesis
   from the `matches.is_empty()` gate.
4. **Do not reopen WIN-29** on its literal trigger; WIN-30 refined it to require surviving a
   re-run.
5. **Do not re-investigate the worktree `GRADLE_USER_HOME` asymmetry** — asserted behaviour,
   R-61.
6. **Do not trust `cargo test` green as evidence about the live MCP surface.** The binary is a
   separate artifact; a source fix is inert until `cargo rb` + reconnect.
7. **Do not assume a surviving mutation means a weak test** — verify the mutation landed first
   (F-10, U-38).
8. **Do not re-run the doc-drift sweep**; `audit_doc_refs` is 0 high on 883 files, fresh-clone
   verified.

### Open bugs — 8, and use this query

```
artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})
```

`status="open"` alone hides anything marked `investigating`. Two fixed today and archived; two
filed today (WIN-30, and the gitignore-cap one now archived). Of the 8, **six need a user
decision or a measurement**: reranker 42× (live-arm measurement + product call), MCP orphans
(definition of "idle"), AST chunker floor (its own precondition is the retrieval benchmark),
researcher rerank scale (sibling repo), kotlin items 1 and 6, and audit-doc-refs
non-determinism (consequence fixed, root cause unconfirmed by design). The `symbols` flake is
`open` on purpose: the harm is neutralised and the next occurrence is self-diagnosing, but the
original symptom was never reproduced.

### Still owed

- **`index(force=true)` rebuild** (~2h) — the ast-chunker change moved chunk boundaries and ids
  are content-addressed. Deliberately not started.
- **The toolchain pin-vs-float decision.** Both W-9's and F-9's promote-when criteria converge on
  it: pin `rust-toolchain.toml` with an explicit `components` list and both problems vanish by
  construction.
- **`262.9593.0`'s changelog** is unreviewed in the kotlin bug.
- **The merge itself.**

## Resume — round 6, written 2026-08-06 for session compaction

**Read this one.** Rounds 2–5 are superseded where they disagree; round 5's *"two decisions
left"* is closed (both taken), and its CI table is extended below.

### Git state (verified, not remembered)

- `experiments` at **`1f20de99`** (this handoff commit), tree clean, nothing unpushed.
- **426 ahead / 0 behind `master`**, `merge-base --is-ancestor` confirms **fast-forward
  promotion is available**. Use `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*,
  not the Standard Ship Sequence.
- Local gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  **3504 tests passed / 0 failed / 44 ignored**. Audit: `EXIT=0`, 0 high findings.

### CI — four confirmed 15/15 runs, then a runner-starved tail

| run | SHA | result |
|---|---|---|
| 31107853410 | `6348dfad` | 15/15 |
| 31108238052 | `db4b1968` | 15/15 |
| 31109236437 | `de4f7ccd` | 15/15 |
| 31109795037 | `382c3344` | 15/15 |
| 31122588792 | `e58ad463` | 4 green · 8 cancelled · 3 queued |
| 31122862704 | `fcb6598f` | 6 green (incl. **Audit Doc Refs**) · 9 queued |
| 31123110469 | `1f20de99` | 15 queued |
| 31123262588 | `e2cf177d` | 15 queued |
| 31123770393 | `8f5b72fb` | 15 queued |

**CORRECTION, 17:45Z — the previous version of this section was false, and it is the kind of
false that propagates.** It read: *"Four runs sat `queued` with none reaching `in_progress`
… a 30-minute watcher expired without a single job starting."* Jobs did start. **Ten jobs
completed successfully** across the two oldest of those runs, and **none failed**. The error
was reading the **run-level** `status` field as a per-job claim: a run stays `queued` while
its own jobs run, pass, and get cancelled — `gh run list` reported `queued` for a run that
already held six green jobs. Never quote a run-level status as job evidence; tally the jobs:

```
gh run view <id> --json jobs --jq '[.jobs[] | if .status=="completed" then .conclusion else .status end] | group_by(.) | map("\(.[0]):\(length)") | join(" ")'
```

**Mechanism — runner starvation, verified two ways.** Every cancelled job carries
`steps=0`, so it never executed a step; every green job carries `steps=12`. And
`.github/workflows/ci.yml` declares no `concurrency:` block, so newer-push supersession
cannot be the cause either. The original *verdict* — a GitHub capacity condition rather than
our code — therefore survives. Only the evidence offered for it was invented.

**What round 6's code has actually cleared.** `fcb6598f` is docs-only on top of `e58ad463`,
so its tree carries all three fixes. The union of distinct green jobs across those two runs
is **8 of 15**: Audit Doc Refs, Format, MSRV (1.88), Windows-gnu cross, Test ubuntu
no-features, and Test windows × default / local-embed / no-features. Still unexposed on CI:
**Clippy**, Tool Docs Sync, Test ubuntu default, Test ubuntu local-embed, and Test macos ×3.
Clippy is the one that matters — F-5 records that the local clippy is a different version
from CI's, so a local pass is not evidence for it.
### What round 6 changed

Triage of the open-bug ledger first exposed that **the ledger itself was under-reporting**:
`find(kind="bug", status="open")` returned 5 where the correct filter returns **8**. Two
independent causes, both fixed:

- `artifact(create, kind="bug")` defaulted `status` to `draft` — not in the bug vocabulary
  at all (`open|investigating|fixed|mitigated|wontfix|zombie`); `create.rs` read
  `unwrap_or("draft")` with no reference to `kind`. Now resolved per kind, and an
  out-of-vocabulary bug status is refused with the six listed. Filed as
  `docs/issues/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`.
- The **documented query** names one of two non-terminal states, so any bug marked
  `investigating` — what the guide instructs you to set while working it — is invisible.
  Fixed at all three prescribing sites, including
  `src/prompts/guides/project-activation-bootstrap.md`, which is auto-injected on
  activation. The two sites that merely *explain* the filter were left alone.

Then three fixes:

- **`scan_meta.degraded` de-saturated.** It was `true` on every run because
  `detect_language` returns `"unknown"` for any extension outside its six and that counted
  as degradation. Measured `true`/`["unknown"]` → **`false`/`[]`** on the same tree. This
  was the prerequisite that made the non-determinism bug's own earlier proposal ("exit
  non-zero when degraded") unusable — it would have failed all fifteen green CI jobs.
- **kotlin-lsp gets `-XX:NativeMemoryTracking=summary`** (item 3 of that bug). Inserted
  *before* `-Xmx2g` so codescout's cap stays the final one; the test asserts the **ordering**,
  because an edit appending after `-Xmx` would silently reopen the original bug.
- **`SymbolMissing` out of the gating band** (`c8efc17a`, round 5) — see round 5.

### One bug advanced with NO code change, deliberately — see W-8

The `symbols` search flake's filed hypothesis (LSP warming) is **refuted**. The tree-sitter
fallback is gated on `matches.is_empty()`, so a 0-match implies tree-sitter found nothing
either, and tree-sitter never touches the LSP. The surviving hypothesis is a `root`
resolution race at activation (`require_project_root_for`, `symbols.rs:225`) — a different
bug class, with precedent in the archived shared-server active-project race.

**If you pick that up: instrument the resolved `root` per 0-match, not LSP readiness.** The
harness as originally specified measures the wrong variable.

### Open bugs — use the CORRECT query

```
artifact(action="find", kind="bug",
         filter={"status": {"in": ["open", "investigating"]}})
```

Blocked on a **user decision or measurement**, not on work:

- reranker 42× latency — needs live-arm measurement and a product call;
- AST chunker minimum chunk size — its own precondition demands the retrieval benchmark
  first ("must be measured, not assumed");
- MCP orphans — needs an idle-timeout value and a definition of "idle";
- kotlin item 1 — cherry-pick to protected `master`;
- researcher rerank score scale — likely sibling-repo scope, unverified.

### Do NOT re-do

Everything in round 4's list still holds, plus:

- **Do not trust `find(kind="bug", status="open")`** — it hides `investigating`.
- **Do not build the symbols-flake harness around LSP readiness** — refuted, W-8.
- **Do not `replace_all` a line right after inserting a helper containing it** — U-37: it
  rewrote the helper's own body into a self-call, compiled clean, and surfaced only as a
  stack-overflow SIGABRT in one test.
- **Do not gate on `scan_meta.degraded` without checking what it contains** — that mistake
  is already recorded once in that bug file's history.

### Still owed

Unchanged: `index(force=true)` rebuild (ast-chunker moved chunk boundaries; ids are
content-addressed — ~2h, not started, nobody asked), the retrieval benchmark for the chunk
floor, reranker options 1–3, the toolchain pin-vs-float policy call, and the kotlin bug's
items 1/5/6. The merge itself is the user's to run.
## Resume — round 4, written 2026-08-06 for session compaction
> **Update, same day — round 5. Both decisions below were TAKEN, and
> `Audit Doc Refs` now reports 0 high findings locally (`EXIT=0`).** The section
> headed "The two decisions left" is superseded; read the round-5 addendum at the
> end of this Resume instead. Everything else here still holds.

**Read this one. Rounds 2 and 3 are superseded on every point of fact** — round 3's
magnitude estimate for the doc-drift backlog was wrong by ~5×, and its "one mechanism
decision" framing turned out to be two decisions plus five outright defects.

### Git state (verified, not remembered)

- `experiments` at **`8fffebb3`**, tree clean, nothing unpushed, in sync with origin.
- **412 ahead / 0 behind `master`** — still a strict ancestor, so promotion remains a
  fast-forward. `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*, not the
  Standard Ship Sequence.
- Two commits this round: `a68f412c` (audit_doc_refs code) and `8fffebb3` (docs). Split
  deliberately — the backlog bug itself warned that mixing extractor changes with dozens
  of doc edits makes both unreviewable.

### Gate state

- **Local gate green:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test` → **3495 passed, 0 failed, 44 ignored**.
- **CI on `8fffebb3` was still running at the time of writing** (7 green: Format, Clippy,
  Tool Docs Sync, MSRV, ubuntu no-features, ubuntu local-embed, macos local-embed;
  `Audit Doc Refs` and the platform matrix in flight). **Do not report a CI verdict from
  this document — re-query it.** `gh run list --branch experiments --limit 1`.
- Remember the local gate cannot predict CI (F-5): local clippy is two minor versions
  behind `dtolnay/rust-toolchain@stable`, and there is still no `rust-toolchain.toml`.

### What landed this round

Five `audit_doc_refs` defects, each with tests carrying over-match guards:

1. `resolve_file_line` / `resolve_file_symbol` had no outside-project guard that
   `resolve_file_path` always had — a doc citing `/etc/passwd:12` resolved **`Resolved`**,
   range-checked against the host's real file.
2. `ignore`'s `matched_path_or_any_parents` **panics** rather than errors on an
   out-of-root path; the first real run aborted with SIGABRT.
3. Link resolution knew one of the repo's **two** conventions. `docs/manual/src/**` is an
   mdBook (internal links must be page-relative); ~28 correct links were reported missing.
4. `path.md#fragment` links were stat'd whole — 8 findings calling existing pages missing.
5. `matches_archive` tested the literal `docs/archive/`, so `docs/trackers/archive/`,
   `docs/plans/archive/` and siblings got **no archive drop at all**.

Three severity caps: `code_block` (a block is a transcript, not a citation — reads
`RefPosition`, which the parser already populated and nothing consumed), `gitignored_path`
(absence is the normal state), and skipping markup-display spans (the audit was flagging
its own manual's example column).

Doc fixes, all confirmed against filesystem or `git log` first: ~12 genuine drift
citations; **two links broken in the rendered book**; **a rendering bug** in
`tool-selection.md` where stray duplicate fences swallowed two prose paragraphs; and two
substantive defects — `semantic-search-diversity.md` documented an **inactive** feature
(`MAX_CHUNKS_PER_FILE` exists nowhere; the cap is `#[allow(dead_code)]`), and
`librarian-mcp.md` cited a file in a crate **dissolved** in `d48bf992`.

### Measured state of the one red job

Per-directory `--paths` counts (the only trustworthy method — see F-6):

- **Zero high findings:** `docs/{adrs,architecture,archive,conventions,evals,issues,spikes,templates}`.
- `docs/manual`: **22 → 11**.
- Residue: `docs/research` 37, `docs/usage-reports` 20, and `docs/{plans,reviews,superpowers,trackers}`
  each ≥50 (capped, so unknown-but-larger). **Several hundred total**, overwhelmingly one
  class: dated point-in-time documents whose refs were correct when written.

### The two decisions left — both yours, both gate-semantics

They are written out in full, with recommendations and the arguments against, in
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` § Resume.
In brief: (1) do dated documents gate CI — extend the drop policy (recommended) or
narrow the scan set; (2) what scope should the `<!-- audit-doc-refs:ignore -->` marker
have for the last 11 manual findings — section-scoped recommended, because a line-scoped
marker cannot live inside a markdown table.

**Neither was taken unilaterally**, and that is deliberate: three caps were already added
this round on defect grounds, and these two change what the gate *means* rather than
fixing what it gets wrong.

### Do NOT re-do

- **Do not blanket-exclude `docs/manual/src/concepts/**`.** Measured: this round found
  two book-breaking links, a rendering bug, and an inactive-feature page in there.
  Excluding it hides all three.
- **Do not "fix" a finding by editing prose to satisfy the lint.** Confirm at the
  filesystem and `git log --all --follow`; if the reference is correct, the extractor is
  what needs changing. That discipline is what produced all five defects above.
- **Do not trust an aggregate read through the 50-finding cap** (F-6). Use `--paths`.
- **Do not trust a single green `Audit Doc Refs` run** — the gate is non-deterministic:
  `docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`.
- **Do not trust a stale release binary.** It was two days old and predated all four
  source files of the tool under test. `find src crates -name '*.rs' -newer target/release/codescout`.

### Owed / unverified

- `index(force=true)` rebuild still owed — the ast-chunker change moved chunk boundaries
  and ids are content-addressed.
- Retrieval benchmark for the chunk floor; reranker options 1–3 still unchosen (needs a
  live-arm measurement).
- Toolchain pin vs float: still a policy call.
- MCP orphan idle-timeout value and the definition of "idle".
- The merge itself is the user's to run.
## Resume — round 5 addendum, written 2026-08-06

**`Audit Doc Refs` reports 0 high findings.** `./target/release/codescout
audit-doc-refs --no-emit-tracker --fail-on high --json --project .` → `EXIT=0`.
Branch at `297e1074`, **415 ahead / 0 behind `master`**, clean, pushed. Local gate:
fmt, `clippy --all-targets -D warnings`, **3498 tests passed / 0 failed**.

### The two decisions, as taken

1. **Dated documents no longer gate** — `historical_drop` covers
   `docs/{plans,research,reviews,spikes,superpowers,trackers,usage-reports}/**` and
   root-level `docs/review-*.md`. The argument that settled it: `issues_drop`
   already does exactly this for bug files, which are acted on constantly. What a
   ledger *is* — a dated record — is what decides the band, not how often it is
   read. Still gating, and asserted in the test: `docs/manual/**`, root
   `docs/*.md`, `CLAUDE.md`, `**/README.md`, `architecture/`, `conventions/`,
   `adrs/`, `evals/`, `templates/`.
2. **`<!-- audit-doc-refs:ignore -->` is section-scoped** — marker to the next
   heading of any level. Line scope was impossible where it was most needed (an
   HTML comment between table rows breaks the table); file scope was rejected
   because the same pages cite real modules.

### Not a silencing — check this before trusting the green

The audit still reports **8388 broken refs**, all at `med`, spread across
`archive_drop` (35 of the shown 50), `inferred_path` (10), `basename_ambiguous`
(4), `gitignored_path` (1). Every finding is still emitted; only the band moved.
If a future change makes the *count* fall too, that is the thing to distrust.

### What else landed in round 5

- **156 stale archive citations repointed** across 62 tracked files by a rule that
  needs no prose reading (`<dir>/<name>.md` absent + `<dir>/archive/<name>.md`
  present → insert `archive/`). 195 insertions / 195 deletions, a pure 1:1 swap;
  re-scan reports 0 remaining. Idempotent by construction.
- **Marked sections**, each with its reason inline — fictional teaching paths,
  correctly-documented user/runtime files, and configuration values that look
  like paths. Including the audit's own manual page, whose "Reference kinds"
  table the tool was reporting as drift.
- **Remaining genuine drift fixed**: `PROGRESSIVE_DISCOVERABILITY.md`'s File
  References table (one row had to become two — the three functions now live in
  `src/symbol/query.rs` and `src/tools/symbol/symbols.rs`), and five dead ROADMAP
  pointers, three of which were **deleted with no successor** and are now stated
  as deleted rather than repointed at a guess.

### Still owed

Unchanged from round 4: `index(force=true)` rebuild (ast-chunker moved chunk
boundaries; ids are content-addressed), the retrieval benchmark for the chunk
floor, reranker options 1–3, the toolchain pin-vs-float call, the MCP orphan
idle-timeout definition. The merge itself is still the user's to run.

**CI CONFIRMED: run 31107853410 on `6348dfad` — 15/15 jobs green.** The first fully-green
run on `experiments`; `Audit Doc Refs` had been red for weeks. `Windows-gnu cross (MinGW +
wine)` is green too.

Two commits landed after that run and are verified against a fresh clone rather than only
locally (`de4f7cc`: 0 high, 881 files, 46726 refs, 8906 broken at `med`):

- `c8efc17a` — **`SymbolMissing` no longer gates.** The flap mechanism turned out to be a
  band straddle inside one match in `resolve_file_symbol`: LSP answers and symbol absent →
  `SymbolMissing`/high; LSP does not answer → `Unknown`/low. A single unanswered request
  moved the exit code. `high` is now reserved for deterministic filesystem verdicts
  (`Missing`, `FileMissing`). Nothing had asserted the old value, so the map is now pinned
  by a test.
- `de4f7ccd` — backlog bug archived through the librarian; the three inbound citations that
  the archive broke were repointed by the same mechanical rule.

**Three consecutive fully-green CI runs, confirmed:**

| run | SHA | result |
|---|---|---|
| 31107853410 | `6348dfad` | 15/15 — trailing-slash cap fix |
| 31108238052 | `db4b1968` | 15/15 |
| 31109236437 | `de4f7ccd` | 15/15 — first run including the `SymbolMissing` band change |

`Audit Doc Refs` passed in all three, and `Windows-gnu cross (MinGW + wine)` with them. The
run for the final docs commit was still in flight at writing time — **check it rather than
trusting this line**, `gh run list --branch experiments --limit 1`.

The streak matters more than any single green, given
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`: that bug is still open,
and three independent runs is the strongest available evidence that the band change removed
the flap from the exit code rather than merely not tripping it once. `gh run list --branch experiments --limit 1`. And per
`docs/issues/2026-08-06-audit-doc-refs-gate-is-nondeterministic.md`, do not treat a
single green `Audit Doc Refs` as proof — that bug is still open.
## Resume — round 3, written 2026-08-06 for session compaction

> **Read this one. Rounds 1 and 2 below are kept for the record but are superseded on
> every point of fact.** Round 2's item 1 ("push, then re-read CI") is done; its
> "remaining issues" list is down from five to one.

### Git state

- Branch `experiments` @ `e6d8ffa8`, **409 commits ahead of `master`, still a strict
  ancestor** (`git rev-list --left-right --count master...experiments` → `0 409`).
  Tree clean, nothing unpushed.
- Promotion is a **fast-forward**, not a cherry-pick. Use `docs/RELEASE.md`
  § *Large-Cohort Promotion (Fast-Forward)*, NOT the Standard Ship Sequence.
- The merge itself is the user's to run ("Prepare only — I run the merge").

### CI state — 14 of 15 green

Run `31098286970` on `cd643d58`. Baseline for comparison: `30852803569` (2026-08-03)
was 4 green / 11 red, measured on code 21 commits stale.

| Green (14) | Red (1) |
|---|---|
| Format, Clippy, Tool Docs Sync, MSRV (1.88) | **`Audit Doc Refs`** |
| ubuntu × default / no-features / local-embed | |
| macos × default / no-features / local-embed | |
| windows × default / no-features / local-embed | |
| Windows-gnu cross (MinGW + wine) | |

`Test (windows-latest / default)`: **3283 passed, 0 failed.**

### The one open item: `Audit Doc Refs`

Diagnosed and measured, not unknown. Tracked in
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`.

After the extractor fixes and the `docs/lessons/**` exclusion, >50 high findings remain
in **three sub-classes needing three different mechanisms**:

1. **Fictional example paths** (~42 of the current window) — src/services/auth.rs ×11,
   src/foo.rs ×5, docs/trackers/my-tracker.md, ./gradlew (left un-code-spanned here on
   purpose: this file gates, and code-spanning a nonexistent example path would add it
   to the very backlog this entry describes). Almost all in
   `docs/manual/src/concepts/**`. Nothing syntactic separates "illustrative" from
   "stale", so these need an author-supplied marker
   (`<!-- audit-doc-refs:ignore-file -->` or similar).
2. **Gitignored runtime state** (~11) — `.worktrees/*` ×8, .codescout/embeddings.db
   ×3. A gitignore-aware severity cap clears these with no markers and no prose edits:
   a doc naming generated state is not citing a tracked file.
3. **Genuine drift** (~8) — including citations to the six bug files archived earlier
   today. Mechanical: repoint to `docs/issues/archive/…`. Note only the
   non-`docs/issues/` citers gate, since `docs/issues/**` gets `issues_drop` High→Med.

**Recommended order:** (2) then (1) then (3). **Mechanism choice was deliberately NOT
taken** — it is a design decision on a user-facing lint with several defensible answers,
and the user was asked.

**Do NOT blanket-exclude `docs/manual/src/concepts/**`.** The manual also cites real
codescout paths, and that is exactly where doc drift hurts readers most. Excluding it
would trade the gate's whole value for a green check.

### Do NOT re-do these — decisions made with evidence

- **`~80 ms` reranker figure in `retrieval-stack.md` stays.** Its bug file says "correct
  it regardless", but that line predates a **CORRECTED** note at the top of the same
  file which retracts the comparison it rests on. The row is labelled TEI; the 3091 ms
  was measured on llama-server. The two available TEI numbers disagree impossibly
  (tracker p50 ~150 ms vs manual p95 ~80 ms). A callout was added instead. See F-4.
- **Reranker options 1–3 stay unchosen.** Needs a measurement of the *live* arm
  (dense + sparse + rerank), which was never benchmarked.
- **MCP-orphan direction 1 ("exit on stdio EOF") is already implemented** —
  `ResilientStdin` absorbs only `WouldBlock`, so a 0-byte EOF propagates and the process
  exits. The orphans are never *sent* one. The fix is direction 2 (idle timeout), which
  needs a timeout value + a definition of idle; firing on a live-but-quiet session is
  worse than the leak. Still reproducing: 16 processes, oldest 2d22h, ~1.05 GB RSS.
  **Do not bulk-kill by pattern** — the list always includes the killer's own server.
- **`derive_dead_roots`' `is_absolute()` guard stays.** Relaxing it to green a POSIX
  fixture makes a prune `WHERE` match every absolute row. See W-3.
- **`count_dead_root_counts_rows_under_root` keeps its POSIX literals.** It calls
  `count_dead_root` directly, so it never reaches that guard; its subject is
  prefix-sibling LIKE semantics, not paths on disk.
- **The seven `⚠ Unreleased` manual callouts stay through the merge.** Removal trigger
  is the *release*, not `master` — both their claims stay true on master.
- **`symbols` Bug A is closed** (retracted; `include_body` IS honoured). Only Bug B
  (intermittent search-mode 0-match) keeps that file open, and it needs a scripted
  post-activation repro that hand-issued MCP calls cannot express.

### Owed / unverified

- **`index(force=true)` rebuild is owed.** The ast-chunker window fix changes chunk
  boundaries and ids are content-addressed, so the existing semantic index holds the old
  (duplicated) chunk set. This is a run, not a code change.
- **Retrieval benchmark for the chunk floor** — the only thing that flips
  `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` to fixed. Precondition
  stated in that file and still unmet.
- **Toolchain skew is unresolved and will recur on its own schedule.** No
  `rust-toolchain.toml` + `dtolnay/rust-toolchain@stable` ⇒ CI re-resolves `stable` every
  run and can go red with zero commits (that is how Clippy broke on 1.97 against
  unchanged code). Pin vs float is a policy call. See F-5.
- **A dedicated ordering test is owed** for the findings-cap fix — see the Resume of
  `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`.
- **mdbook is not installed**, so the manual was never build-verified this session.

### Techniques worth keeping

- **Per-job CI logs while a run is still in progress:**
  `gh api --allow-escape-sequences "/repos/mareurs/codescout/actions/jobs/<id>/logs"`.
  `gh run view --log-failed` refuses until the whole run finishes, and a stalled sibling
  blocks it indefinitely (`Windows-gnu cross` sat 41+ min on "Install MinGW + wine").
- **Polling a job's conclusion:** GitHub returns `conclusion: ""` for pending, and jq's
  `//` only defaults on `null`/`false` — so `.conclusion // "pending"` never fires. Guard
  with `[ -n "$C" ]`.
- **`--all-features` is structurally invalid here** — `codescout-embed` has a
  `compile_error!` for mutually-exclusive ONNX backends.

### Where the rest of the record lives

| Surface | Holds |
|---|---|
| `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` | the one open CI blocker, with the bisect method |
| `docs/issues/archive/2026-08-06-windows-*.md` | WIN-28 (nine panics, three causes) + WIN-29 duplicate proof |
| `docs/trackers/windows-platform-support.md` | WIN-28/29 index rows + a History entry |
| this file, F-4 / F-5 / W-2 / W-3 / W-4 | the session's transferable lessons |
| `docs/trackers/reconnaissance-patterns.md` R-55, R-56 | recon-skill proposals |
| `docs/trackers/codescout-usage-frictions.md` U-30, U-33/34/35 | tool frictions; IL3 now ×7 in one session |

## Resume — round 2, written 2026-08-06 for session compaction

> **CI IS NOW 14/15 GREEN.** Run `31098286970` on `cd643d58` — baseline was 4 green / 11 red
> (`30852803569`, 2026-08-03, on code 21 commits stale).
>
> | Green (14) | Red (1) |
> |---|---|
> | Format, Clippy, Tool Docs Sync, MSRV (1.88) | `Audit Doc Refs` |
> | ubuntu × default / no-features / local-embed | |
> | macos × default / no-features / local-embed | |
> | **windows × default / no-features / local-embed** | |
> | **Windows-gnu cross (MinGW + wine)** | |
>
> `Test (windows-latest / default)`: **3283 passed, 0 failed.**
>
> **WIN-28 fixed and archived.** Nine failures, three root causes, **zero product defects** —
> two assertions were failing against *correct* implementations and the rest were fixtures
> encoding POSIX-only path shapes. Fixing the product to satisfy cluster A would have relaxed
> `derive_dead_roots`' absolute-path guard, whose absence makes a prune `WHERE` match every
> row. Full account in
> `docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`.
>
> **WIN-29 confirmed a duplicate, empirically.** It was closed on an inference (identical
> failing-test sets) and has now gone green from the same nine fixture fixes, with no
> MinGW/wine-specific change. That is the prediction the duplicate hypothesis made.
>
> **The one remaining red is `Audit Doc Refs`**, and it is NOT undiagnosed. Measured population
> after the extractor fixes and the `docs/lessons/**` exclusion: >50 findings, ~42 of them
> illustrative paths inside `docs/manual/src/concepts/**` (src/services/auth.rs, src/foo.rs,
> .worktrees/my-feature, docs/trackers/my-tracker.md — un-code-spanned, see round 3). Three sub-classes needing three
> different mechanisms — see
> `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`:
>
> 1. **Fictional example paths** — need an author-supplied marker; nothing syntactic separates
>    "illustrative" from "stale".
> 2. **Gitignored runtime state** (`.worktrees/*` ×8, .codescout/embeddings.db ×3) — a
>    gitignore-aware severity cap handles these with no markers and no prose edits.
> 3. **Genuine drift** (~8), including citations to the six bug files archived earlier today.
>
> **Do NOT blanket-exclude `docs/manual/src/concepts/**`.** The manual also cites real codescout
> paths, and that is precisely where doc drift hurts readers most. Mechanism choice is an open
> decision, deliberately not taken unilaterally on a user-facing lint.
>
> Also unresolved and unrelated: no `rust-toolchain.toml` + `dtolnay/rust-toolchain@stable`
> means CI re-resolves `stable` every run and can acquire lints with zero commits (that is how
> Clippy went red on 1.97 against unchanged code). Pin vs float is a policy call.

> **SUPERSEDED in part, 2026-08-06 — item 1 ("push, then re-read CI") is DONE.** Pushed
> `d7988aca..99695a10` (22 commits) to `origin/experiments`. CI now runs on current HEAD
> for the first time since 2026-08-03; every verdict before this described code 21
> commits stale, which is why the old "11 of 15 red" picture was unactionable.
>
> **CI state now: 10 green / 5 red** (run `31091169757` on `99695a10`), from a baseline of
> **4 green / 11 red** (run `30852803569`, 2026-08-03).
>
> | Cleared | Still red |
> |---|---|
> | Tool Docs Sync | Audit Doc Refs — `docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md` |
> | ubuntu/no-features | Test (windows-latest / default) — WIN-28 |
> | ubuntu/local-embed | Test (windows-latest / no-features) — WIN-28 |
> | macos/no-features | Test (windows-latest / local-embed) — WIN-28 |
> | macos/local-embed | Windows-gnu cross (MinGW + wine) — WIN-29 |
> | Clippy | |
>
> **The state change that matters for the merge decision: there is no longer an
> undiagnosed red cell.** All five have an open bug file, and three of the five are one
> bug (WIN-28) wearing three hats. Previously six of the reds were feature-gate rot,
> invisible to a default-features build by construction.
>
> **Clippy needed a fix nobody could have predicted locally.** It stayed red on run
> `31089996833` against code nobody had touched: the log's help URLs name
> `rust-clippy/rust-1.97.0` while this machine runs `clippy 0.1.95`. Three pre-existing
> patterns (`question_mark` ×2 in `src/tools/symbol/call_edges/resolver.rs`, `for_kv_map`
> in `src/tools/symbol/call_graph/mod.rs`) are lints 1.97 emits and 1.95 does not. Fixed
> in `99695a10`; Clippy green on the next run.
>
> **Standing hazard this exposed, unresolved and needing a policy call:** there is no
> `rust-toolchain.toml`, and CI uses `dtolnay/rust-toolchain@stable`, so CI re-resolves
> `stable` on every run and can acquire new lints with **zero commits**. Clippy therefore
> breaks on the calendar, and a locally-green `cargo clippy -- -D warnings` is not a
> predictor of the CI job. Two options, both the maintainer's to pick: pin a toolchain in
> the repo (local == CI, upgrades become deliberate), or keep floating and accept periodic
> lint debt. Nothing was imposed here.
>
> Items 2-5 of the Resume below stand unchanged.

Wipe and rewrite this block each session. Everything below is verified, not assumed.

### Git state

- Branch `experiments` at **`553e618e`**; 10 commits from this session
  (`ca442498..553e618e`).
- `git rev-list --left-right --count master...experiments` → **`0  397`**. Master is a
  strict ancestor, so the promotion is a **fast-forward**, not a cherry-pick. Re-check
  this before merging — a non-zero left number means master diverged.
- **17 commits unpushed.** This matters: the last CI run (`30852803569`, 2026-08-03) is
  from *before* this session, so its verdicts are stale for 6 of the jobs.
- Procedure to follow: `docs/RELEASE.md` § *Large-Cohort Promotion (Fast-Forward)*.
  Do **not** use the Standard Ship Sequence — it cherry-picks single commits and would
  mint 397 new SHAs, orphaning every SHA citation in `docs/issues/` and the trackers.

### Gate state — green locally, all six steps exit 0 at `553e618e`

```bash
cargo fmt --check
cargo clippy -- -D warnings                              # CI's exact invocation
cargo test                                               # 3344 lib tests
cargo check --no-default-features --all-targets           # warning-free
cargo test --no-default-features                         # 2477 tests
cargo test --features local-embed --no-default-features
```

The last three are the ones that matter and the ones CLAUDE.md's three-command line
omits — they are where this session found five defects. See F-3, T-16.

### What still blocks a green merge (the "remaining issues")

Ranked by what to do first.

1. **Push, then re-read CI.** Cheapest possible move and it may close two items for
   free. The stale run predates `7938d68b` (feature-gate compile fixes) and `be75e705`
   (three behaviour-gated tests). Expected to clear: `Clippy`, `Tool Docs Sync`, and the
   four non-Windows `no-features` / `local-embed` cells. Expected to stay red: the three
   Windows cells, `Audit Doc Refs`, and possibly `windows-gnu`.
2. **`docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md`**
   (open, high; WIN-28). Nine tests fail on `windows-latest`, all in code this cohort
   added — 7 catalog `rehome`/`prune_missing`, the `like_escape` idiom guard, the index
   lock. Linux and macOS pass the same config, so path semantics not logic. **Blocked on
   a Windows runner**: the per-test panic output was never captured, and guessing at path
   normalisation risks fixing the tests rather than the code. Its Resume names
   `validate_rehome_gates` as the narrowest starting point.
3. **`docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md`**
   (open, high). The `audit-doc-refs` job is a hard gate (`--fail-on high`, no
   `continue-on-error`) and all 18 high-severity findings are **extractor false
   positives**: `Type/method` (codescout's own `name_path` syntax, 8 of 18), GitHub
   `org/repo` slugs, ellipsis-elided external paths, plus an mdBook relative-link class
   at `med`. **Do NOT "fix" this by editing the three ADRs** — their prose is correct.
   Fix the extractor (start with the `name_path` shape: 18 → 10) or drop the gate and say
   so in the workflow comment, which currently claims all hi-sev findings are reconciled.
   Cross-references the earlier `2026-07-28-audit-doc-refs-json-pointer-false-positive.md`,
   which has priority.
4. **`docs/issues/archive/2026-08-06-windows-gnu-cross-job-red-undiagnosed.md`** (open, medium;
   WIN-29). Undiagnosed by choice. Leading hypothesis after a ledger query: WIN-28's nine
   failures are not in `scripts/build-windows.sh`'s wine skip-list, so this is likely
   item 2 wearing a second hat — confirm before fixing twice, and `graft` it into WIN-28's
   file if so.
5. **`docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`** (open,
   medium). Pre-existing, not a regression, does not block the merge. The inner-node
   recursion re-derives gaps against the whole file with `prev_end` reset to 0, emitting a
   chunk that duplicates every line before the container. Fix plan is in the file; it
   changes the emitted chunk set for every decomposed container in every language and
   invalidates existing indexes, so it wants its own change.

### Do NOT re-do these — decisions already made with evidence

- **`docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` stays `open`.** Its Fix
  candidate 1 was implemented at `ca442498` (7 tests, both behaviours mutation-verified),
  but that file requires validation against
  `docs/research/2026-05-06-retrieval-stack-benchmark.md` before landing — *"must be
  measured, not assumed"* — and **the benchmark was never run**. It also records an
  ordering decision (throughput work first, being vector-identical) that `ca442498`
  jumped. Running that benchmark is the only thing that turns it `fixed`. See F-2.
- **The seven new manual pages keep their `⚠ Unreleased` callout through the merge.**
  Removal triggers at **release**, not at merge — master is not crates.io. Mechanical:
  `grep -rl 'Unreleased — on the `experiments` branch only' docs/manual/src/`, then
  `docs/RELEASE.md` § *Release Cycle* step **1b**.
- **New subsystem docs go straight into the main manual**, not staged under
  `docs/manual/src/experimental/`. The staging-then-move flow measured 0/62 compliance
  this cohort; see the revised buddy memory `experimental-docs-lifecycle`.
- **`docs/FEATURES.md` is archived** to `docs/archive/FEATURES.md` (zero live inbound
  refs; moved through the catalog, id preserved).

### Where the rest of the record lives

| Surface | What it holds from this round |
|---|---|
| `CHANGELOG.md` `[Unreleased]` | All ten feature clusters + notable fixes; the canonical cohort list |
| `docs/RELEASE.md` | Large-Cohort Promotion procedure; Release Cycle step 1b |
| `docs/ROADMAP.md` standing backlog | The local five-step gate alias proposal |
| `docs/trackers/reconnaissance-patterns.md` | R-55 (`miss → proposal`): query the ledger by file identity, not task category |
| `docs/trackers/codescout-usage-frictions.md` | U-30 (IL3 ×4 + the orphaned deny hook), U-31 (shell-on-source blocks CI-gate repro), U-32 (`.buddy` write asymmetry) |
| `docs/trackers/codescout-usage-hookify.md` | H-1 stale flag **resolved** — deny hook orphaned by the `.sh` → `.mjs` port |
| `docs/trackers/tool-usage-patterns.md` | T-14/15/16, plus T-013 params row backfilled |
| `docs/trackers/windows-platform-support.md` | WIN-28, WIN-29 |
| `docs/RELEASE-TODO.md` | CI-gate record corrected (`audit-doc-refs` is a hard gate, and it is red) |

### Unverified / owed

- `mdbook` is not installed here, so the book was never build-verified. Cross-link
  targets were checked against `SUMMARY.md` by hand.
- The retrieval benchmark for the chunk floor (see *Do NOT re-do*).
- `docs/RELEASE-TODO.md`'s "Error message path sanitization" item could not be verified —
  `strip_project_prefix` does not exist under that name; the mechanism was not chased.

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-02 | med | codescout-tool | promoted-to-bug-tracker | `audit_doc_refs` flags legitimate cross-repo hook paths as `missing`/`high`, inconsistently |
| F-2 | 2026-08-06 | high | process | mitigated | Bug ledger never queried at the seam — reimplemented a filed fix and skipped its stated precondition |
| F-3 | 2026-08-06 | med | process | fixed-verified | Treated CLAUDE.md's three local commands as "the gate"; the merge gate is 15 CI jobs, red for 3 weeks |
| F-4 | 2026-08-06 | med | process | fixed-verified | Three bug files' own `## Fix` sections carried wrong premises — stale-by-superseding, wrong severity, already-implemented |
| F-5 | 2026-08-06 | high | process | open | Local gate structurally cannot predict CI — clippy 1.95 vs 1.97, separator bugs invisible on Linux, `--all-features` unusable |
| F-6 | 2026-08-06 | med | measurement | mitigated | The 50-finding cap mis-sized the backlog plan by ~5× — third instance of a capped view corrupting a number the gate acts on |
| F-7 | 2026-08-06 | med | measurement | fixed-verified | Quoted a run-level CI `status` as per-job evidence — wrote "not a single job starting" into this log while ten jobs had already passed |
| F-8 | 2026-08-06 | med | process | fixed-verified | Diagnosed queued CI from run metadata for 70 minutes; GitHub had declared an Actions **major outage** two hours before the first affected run |
| F-9 | 2026-08-07 | med | measurement | fixed-verified | A `--profile minimal` toolchain install produced 12 test failures that read as a compiler regression; the cause was a missing `rust-analyzer` component |
| F-10 | 2026-08-07 | med | process | fixed-verified | A mutation that failed to kill a test almost got a *correct* test rewritten — the mutation was incomplete (two `Err` arms at different indentation depths), not the test weak |
| F-11 | 2026-08-07 | med | process | fixed-verified | A bug file's prescribed fix was already implemented — the test it said to convert to a bounded poll already polled for 5 s, so shipping the fix as written would have been a no-op that closed the bug and left the flake |
| F-13 | 2026-08-07 | high | measurement | fixed-verified | A two-point probe produced a published recommendation that a 28-point probe inverted — the two samples happened to be the extremes of a bimodal distribution, and the middle turned out to overlap the bottom |
| F-12 | 2026-08-07 | med | test-design | fixed-verified | Seven tests and a green gate shipped a warning whose truncation hid the entry it existed for — every fixture had exactly one hidden entry, so the `more > 0` branch was never exercised; the project's own "both sides of every condition" lens would have caught it |
| F-14 | 2026-08-07 | med | process | fixed-verified | Memory `conventions` § Bug Tracking still gated bug archiving on reaching `master` — the superseded rule — while `CLAUDE.md`, `_TEMPLATE.md`, and `docs/RELEASE.md` all say "verified on `experiments`, master not required"; memory is the one surface with no lint, and it is the one loaded at session start |
| F-15 | 2026-08-07 | high | process | fixed-verified | A concurrent session changed the checked-out branch between commit and push; the bare `git push` reported `Everything up-to-date` at exit 0 while the commit sat undelivered, and a later `edit_markdown` then wrote to the foreign branch's tree and returned `ok` because its anchor existed on both -- refspec push is the non-destructive fix, and after a detected foreign checkout every write is suspect and must be verified from the object store |
| F-16 | 2026-08-07 | high | process | mitigated | Backticks in a double-quoted `git commit -m` body ran as command substitution -- cost one word here, but the construct executes arbitrary commands while `git commit` still exits 0, so the only trace is an unexplained gap in the message; `-F <file>` is the only safe form |
| F-17 | 2026-08-07 | high | process | fixed-verified | A `--force` reindex ran 7 min dense-only against sqlite-vec instead of hybrid-against-Qdrant, because `cargo build --release` omits `server-stack` and `VectorBackend::resolve()` then defaults to lite — caught only by noticing the sparse container had zero traffic while dense had 73,232 lines; the start log reported chunk_target and flush_batch but neither of the two fields that were wrong |
| F-18 | 2026-08-07 | high | process | mitigated | A routine `/mcp` killed a running rebuild 4 min in, because `run_command` children inherit the MCP server's lifetime — and took both `@bg_*` buffers with it; second time in one session that server-side state died with a reconnect, the first being the very argument used in the #16 decision. Long jobs must be `setsid`-detached and log to a file |
| F-19 | 2026-08-07 | med | process | mitigated | Three `pgrep`/`pkill -f` process checks matched their own argv: one falsely reported an indexer running (the exact safety precondition it was checking), one made a poll loop unable to ever exit, and one killed the command running it. `ps -C <binary>` cannot self-match because the checker is not named after the binary |
| F-20 | 2026-08-08 | high | process | fixed-verified | Archiving three bug files moved their paths and left 25 dangling citations across 15 files; the one in the published manual failed `Audit Doc Refs` on `f244ad17` — the exact commit handed over as merge-ready. Archiving is a bug file's normal end state, so every `docs/issues/<slug>.md` citation is a scheduled break. Reading the audit's severity policy (drops key on the CITING document) rather than only its verdict is what sized the fix correctly: 12 live surfaces re-pointed, 13 historical ones deliberately left. Prevention shipped in the archive flow (`get_guide("tracker-conventions")`); fixed in `e20b3a04` |
| F-21 | 2026-08-08 | high | testing | fixed-verified | A test named `resolve_git_bash_never_selects_the_wsl_launcher` passed because it injected `PATH=C:\Windows\System32` — the spelling the CODE constructs — while Windows setup writes `C:\Windows\system32`, and `Path` equality folds case only on the drive-letter prefix. The exclusion never fired on a real host, so every `run_command` would run under the WSL launcher, silently. A fixture written from the code rather than the environment cannot discriminate. Fixed by `platform::windows_dir_eq` + a four-spelling sweep + a positive control (`6f261da9`) |
| F-22 | 2026-08-08 | med | process | mitigated | A rebase onto `f244ad17` orphaned every SHA PR #10's bug files cited — `git cat-file -t` resolved none of `b5c8bbb0`, `94a63c32`, `d564c9bb`, `20d12b5f`; a sweep found 14 dead 8-hex tokens (5 PR-introduced, 9 pre-existing). The hazard `docs/issues/_TEMPLATE.md` documents, realized, and ungated: `audit_doc_refs` checks paths and symbols, not SHAs. Merging with `--rebase`/`--squash` would have re-orphaned them an hour after repair, so `--merge` became load-bearing |
| F-23 | 2026-08-08 | med | process | fixed-verified | Predicted PR #10's CI would fail on a break inherited from its merge-base; it passed. `actions/checkout` on a `pull_request` event checks out `refs/pull/N/merge` — a synthetic merge commit on no branch, which already contained the fix. The session's "green run on whatever the tip is" rule is a PUSH-event rule; a PR run answers *is the merge result good*, not *is this commit good* |
| F-24 | 2026-08-08 | high | process | fixed-verified | A bug file measured its symptom correctly and mis-attributed its cause: 8 `??` dirs blamed on foreign-workspace registration, when `memory_dir_for_project` derives the path from `self.root` so a foreign workspace cannot write under another repo's root. Sweep of all 15 registered roots — 9 repos with the tree, 0 untracked entries. All three proposed fixes encoded the mis-attribution; Option B reintroduced what `6f261da9` removed. The premise convention asks "is the claim measured?" and there are two questions |
| F-25 | 2026-08-08 | med | process | fixed-verified | Recommended the `~/personal` structural `.gitignore` pattern as "strictly better than A/B/C" and had it approved as a task; `git check-ignore` in a throwaway repo then reported the phantom's `memories/` file NOT ignored — identically to a real one's. Also proposed VDI workspace-root misresolution as the mechanism before two `memory()` calls falsified it (recorded as hypothesis 4, withdrawn) |
| F-26 | 2026-08-08 | med | test-design | fixed-verified | `memory_write_accepts_project_alias_for_project_id` wrote `mcp-server/package.json` as `{}` — too empty for discovery — so the sub-project its own comment claimed to create never existed, and its path assertion was satisfied entirely by the defect under test. Third instance of F-21's shape in two rounds |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-06 | med | Dump actual output of an *internal pure function*, don't derive it from source | Reasoning predicted the wrong padding chunk; the dump surfaced a whole-file duplicate + a second pre-existing bug | validated |
| W-2 | 2026-08-06 | high | If a truncated list feeds a pass/fail verdict, sort by the verdict's key before truncating | Would have "fixed all 18" and stayed red with no visible cause; the 18 was itself a windowed miscount of a >50 population | validated |
| W-3 | 2026-08-06 | high | Read the product code before believing a platform-specific red test | Relaxing `derive_dead_roots`' guard to green the test makes a prune `WHERE` match every row — data loss shipped to fix a test | validated |
| W-4 | 2026-08-06 | med | A duplicate closure states its own falsification test | `Windows-gnu cross` stayed the one red cell with no known cause; the diff collapsed 4 cells into 1 bug and predicted its green | validated |
| W-8 | 2026-08-06 | high | Read the fallback gate before building the harness a bug file asks for | Every signal pointed at LSP warming, including the source's own comment; one line (`matches.is_empty()`) proved a 0-match implies tree-sitter also found nothing, so server readiness cannot cause it — the harness would have measured the wrong variable | validated |
| W-6 | 2026-08-06 | high | Mechanise the decidable half first; the residue is the judgement, and its size is the estimate you should have had | Sample of 11 sized a class that was 156 across 62 files and included a tracker the sample missed; ~300 tool calls avoided, and a live tracker's "Active bug files" label pointing at the archive became visible only once the paths were right | validated |
| W-16 | 2026-08-08 | med | Cross-compile `#[cfg(windows)]` tests before pushing them — `cargo check --target x86_64-pc-windows-gnu --tests` | `cargo test` on Linux never compiles them, so a type error first appears as a red CI leg ~6 min out; the cross-check costs ~6 s with deps warm because `rust-toolchain.toml` already pins the target. Returned green — a verification step is not paid for only by the times it fires | validated |
| W-17 | 2026-08-08 | high | Run the thing rather than reason about it, whenever the behaviour is cheap to invoke | Four in one session: PR #9's 12/12-green guard gave 9-of-9 divergence under a nine-case probe; two `memory()` calls inverted a diagnosis; a second probe found a worse symptom on the read path; `git check-ignore` falsified a fix already recommended and approved. Joins W-5, W-12, W-13 — at promotion threshold | validated |
| W-18 | 2026-08-08 | high | When a side effect lives in a CONSTRUCTOR, every caller is a distinct seam — `references()` it and probe a caller the report did not name | `MemoryStore::from_dir` calls `create_dir_all` in its constructor; 5 call sites. The report named `write`; probing `read` found `available_topics: []` for a non-existent project — the worse symptom, and the thing that explained why one host showed 8 `??` entries and another 2 invisible ones | validated |
| W-15 | 2026-08-08 | high | Fan out a review by LENS not by file, brief each with the refs + established-facts-marked-challengeable, and mandate refute-before-reporting | Four lenses on PR #10 (19 files, no prior reviews) found five blockers; three landed twice independently (`summary.total` partition, dead `posix_tokenize`, the job-assignment race). The refutation passes killed real candidates including one of the controller's own seeded premises, and one reviewer corrected two premises in its own brief | validated |
| W-14 | 2026-08-07 | high | The first measurement after idle is a warm-up artifact — take the second one, and in a benchmark discard iteration one and say so | Three instances in one session: SPLADE sparse embed read 146.8 ms cold vs 16.8 ms warm (8.7x, and it feeds the reranker latency comparison in task #20); the audit_doc_refs tally migrates by up to 69 refs cold but is byte-identical warm; and `resolve_file_symbol` returns `SymbolMissing` for symbols that exist when the server answers before finishing indexing — the same trap promoted from a latency error into a false claim about the code | validated |
| W-13 | 2026-08-07 | high | Verify a bug file's PREMISE before working it, with the cheapest measurement that could falsify it | Five bugs worked this session; all five had a false premise or a wrong prescription, and four were falsified by a single command — `ps -o ppid` killed "18 orphaned processes", `jcmd VM.flags` killed "spawns with no -Xmx", one `curl` replaced a Langfuse-span plan, and reading the test killed "replace the fixed wait with a bounded poll" | promoted-to-permanent-docs |
| W-12 | 2026-08-07 | high | Run the fix against the real tree as a step distinct from the gate — a green suite says the branches you wrote tests for work, not that the output is useful | Seven tests passed and clippy was clean, yet the shipped `grep` warning read ".buddy/, .cargo/, .claude/, .env, .env.amd and 11 more" — alphabetical truncation cut `.github/`, the entry the whole fix existed to surface. Every fixture had one hidden entry; the repo has 16 | promoted-to-permanent-docs |
| W-11 | 2026-08-07 | high | Scout a bug file's `## Fix` before implementing it — a fix written from a CI log line has never touched the code | WIN-30's Fix prescribed replacing a fixed wait with a bounded poll; the poll was already there, so the change would have been inert, the bug archived, and the next flake would have read as a regression of a fix that never existed | validated |
| W-10 | 2026-08-07 | high | Before merging two code paths that look redundant, ask what *triggers* the second one | The two `symbols` walks share an identical filter and look duplicated; a truncated first walk empties `matches`, which is precisely what triggers the fallback — merging them would have deleted the recovery path and made the bug under repair strictly worse | validated |
| W-9 | 2026-08-07 | high | When CI is unavailable and its toolchain floats, install CI's *resolved* toolchain side-by-side and run the gate through it | Local `stable` was 1.95.0 while CI's `@stable` had moved to 1.97.1; clippy 1.97 rejects two `assert!` borrows that 1.95 accepts, so the Clippy job was going to fail on the first run created after the outage | validated |
| W-5 | 2026-08-06 | high | Invoke the tool under test; verify the binary is newer than its sources | Reading it had missed all five: a SIGABRT, a path escape resolving `/etc/passwd:12` as Resolved, a false premise about which half of the corpus was scanned, an unused field that already was the fix, and mdBook link semantics | validated |

---

## F-1 — `audit_doc_refs` flags legitimate cross-repo hook paths as `missing`/`high`, inconsistently

**Observed:** 2026-07-02, pre-dispatch reconnaissance before trusting a fork's doc-staleness sweep of the 79-commit `experiments`->`master` promotion.

**When:** About to rely on a fork subagent's brief that told it to treat every `audit_doc_refs` finding with `verdict=missing AND severity=high` (outside a named ADR exclusion list) as real staleness, across all 58 changed markdown files, including `docs/architecture/companion-plugin.md`.

**Expected:** A `missing`/`high` finding means the referenced path genuinely doesn't exist under the active project and is real drift worth fixing.

**Got (scouted):** Ran `librarian(action="audit_doc_refs", paths=["docs/RELEASE.md","docs/architecture/companion-plugin.md"])` directly before trusting the fork. `docs/architecture/companion-plugin.md` cites five paths inside the sibling `../claude-plugins/codescout-companion/` repo (`hooks/hooks.json`, `hooks/session-start.sh`, `hooks/subagent-guidance.sh`, `hooks/pre-tool-guard.sh`, `.claude/codescout-companion.json`) — all real, correct references to a repo outside the active project root. All five came back `verdict=missing, severity=high`, no explanatory note. Meanwhile other refs to the exact same external repo on the exact same page (`../claude-plugins/codescout-companion/` itself, and unrelated example paths like `/path/to/sibling`) came back `verdict=unknown, severity=low` WITH a helpful `notes: "path outside active project; scope=umbrella required"`. Same root cause (path outside `scope=project`), inconsistent verdict/severity/notes treatment depending on `ref_kind` classification.

**Probable cause:** The classifier's "outside active project" carve-out (which downgrades to `unknown`+note) appears to fire for some `ref_kind`s (bare directory-looking refs) but not others (relative sub-paths one level under an already-flagged, out-of-scope parent directory), so those fall through to the default `missing`/`high` policy.

**Workaround:** Corrected the in-flight fork's brief via `SendMessage`: told it to also exclude any `missing`/`high` hit in `docs/architecture/companion-plugin.md` referencing the `../claude-plugins/codescout-companion/` hook paths, and to flag (not silently trust) any other file whose "high" hits are all rooted under a path that itself resolves `verdict=unknown` with the umbrella-scope note.

**Severity:** med — without the scout, the fork would have reported `docs/architecture/companion-plugin.md` (a file already read and trusted this session) as carrying 5 high-severity broken refs, a false regression entering the promotion punch list.

**Status:** promoted-to-bug-tracker (2026-08-06; was `mitigated`) — fork corrected mid-flight; `audit_doc_refs`'s own inconsistent path handling is unfixed and now tracked in the bug ledger rather than here. See the promote-when note below.

**Fix idea / Pointer:** Candidate for a U-N entry in `docs/trackers/codescout-usage-frictions.md` if this recurs on a second file/session — any doc describing `codescout-companion` internals will likely trip the same false positive. Promote once a second datapoint lands.

**Promote-when criterion FIRED 2026-08-06 (second datapoint).** Same tool, same class: the extractor classifies non-local tokens as local file paths and the severity policy bands them `high` regardless of classification confidence. New tokens observed: `Type/method` (codescout's own `name_path` symbol syntax), GitHub `org/repo` slugs, and ellipsis-elided external paths (`…/rocks/v492/LOCK`). All 18 high-severity findings on `experiments` are false positives of this class, and the `audit-doc-refs` CI job is **red** on them with no `continue-on-error`. Promoted past a U-N entry straight to the bug ledger, which is the better destination: `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` (which itself cross-references the earlier `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md`). Status moved to `promoted-to-bug-tracker`.

---

## F-2 — Bug ledger never queried at the seam; reimplemented a filed fix and skipped its stated precondition

**Observed:** 2026-08-06, during `experiments` -> `master` merge preparation (394-commit fast-forward). Surfaced ~60 tool calls into the session, by accident, during an unrelated verify-open pass.

**When:** The session opened with an uncommitted +96-line change in `src/embed/ast_chunker.rs`. I scouted the *code* seam properly (read `nodes_to_chunks`, ran the module tests, dumped real chunk output) and shipped a fix with 7 mutation-verified tests as `ca442498`. What I never scouted was the *decision* seam: what the project had already decided about this code.

**Expected:** the uncommitted change was fresh work whose only gap was a missing test, and completing it was mine to do.

**Got (scouted, far too late):** `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` (`a8c0361cec54e6e2`, `status: open`, `severity: high`) already specified this exact change as **Fix candidate 1** — verbatim *"Introduce `AST_CHUNK_MIN` (~200-300 chars) and coalesce consecutive inner declarations below it into one chunk, keeping the container header."* I implemented 250. The same file carried two constraints I violated:

1. *"Any of these should be validated against the retrieval benchmark (`docs/research/2026-05-06-retrieval-stack-benchmark.md`) before landing — smaller chunks were chosen deliberately for precision, so a floor trades recall sharpness for cost and **must be measured, not assumed**."* No benchmark was run.
2. An explicit sequencing decision in its Resume: *"the throughput work lands first because it leaves vectors byte-identical and needs no score re-validation, whereas this change does."* That ordering was jumped.

Second consequence, same root cause: I filed `docs/issues/archive/2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` as new when `docs/issues/archive/2026-07-28-audit-doc-refs-json-pointer-false-positive.md` already held the same extractor + severity-policy root cause.

**Probable cause:** the `project-activation-bootstrap` guide was **auto-injected into the first tool response of the session** and states, verbatim: *"Bug or regression work: `artifact(action=\"find\", kind=\"bug\", status=\"open\")` — the known-bug ledger. Don't re-file a filed bug as new; mark a rediscovery KNOWN and cite the ledger path."* The guidance was present, positioned at Phase 0, and still missed. The reason is the trigger shape: the rule fires on **task category** ("bug or regression work"), and I had classified the session as *documentation + merge prep*. A category-triggered rule cannot fire for someone who has categorised their task differently — and "am I doing bug work?" is exactly the question a merge-prep framing answers "no" to, right up until it edits a file that has an open bug against it.

**Workaround:** `a8c0361cec54e6e2` updated in place rather than archived — records that candidate 1 is implemented at `ca442498` with a green 5-config gate, that the benchmark precondition is **unmet**, that the sequencing was jumped, and the explicit criteria to close. The two `audit_doc_refs` bugs now cite each other, mine noting the earlier filing has priority and carrying the `graft` command to fold them.

**Severity:** high — a corpus-invalidating change (chunk ids are content-addressed, so every boundary change re-embeds everything) landed out of its planned sequence with no evidence that recall held. Cherry-picking `ca442498` to `master` would force a full re-index on every consumer while the question its own bug file raises stays unanswered. The duplicate ledger entry is the minor half.

**Status:** mitigated — nothing archived falsely, both preconditions documented, close criteria written down. The benchmark run is still owed and is the only thing that turns this `fixed-verified`.

**Fix idea / Pointer:** Recon needs a **file-identity** ledger trigger, not a category one: before editing a file, `artifact(action="find", kind="bug")` constrained to that file or subsystem. Filed as R-55 `miss` + `proposal` in `docs/trackers/reconnaissance-patterns.md`.

---

## F-3 — Treated CLAUDE.md's three local commands as "the gate"; the real merge gate is 15 CI jobs, red for three weeks

**Observed:** 2026-08-06, ~40 tool calls into merge preparation, after all documentation work had already landed.

**When:** The user's task was "prepare for merging to master." Every subsequent action depended on the current state of the merge gate. I ran `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` early, reported them green, and proceeded to documentation.

**Expected:** gate state unknown but probably fine; CLAUDE.md's *"Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` before completing any task"* is the gate.

**Got (scouted):** `gh run list --branch experiments` returns `"conclusion": "failure"` for **every run** back to 2026-07-13. The most recent (`30852803569`, 2026-08-03) had **11 of 15 jobs red**: Clippy, Tool Docs Sync, Audit Doc Refs, Windows-gnu cross, and 7 of 9 test-matrix cells. `.github/workflows/ci.yml` runs a 3x3 matrix (ubuntu/macos/windows x default/local-embed/no-features) plus four non-test jobs. CLAUDE.md's three commands cover **one** of nine test cells.

The two most consequential defects were invisible to the local three by construction: `tests/link_scan.rs` and `src/server.rs`'s `make_server` helper reference `codescout::librarian` / `ServerEnv.librarian` with no `#[cfg(feature = "librarian")]`, so they fail to *compile* under `--no-default-features` and `--features local-embed`. Nobody builds those configs locally, so six matrix cells were red on code that a default-features `cargo test` reports as perfectly green.

**Probable cause:** CLAUDE.md's pre-commit instruction is a *per-task hygiene* rule and reads like a complete gate. `docs/RELEASE.md`'s Standard Ship Sequence reinforced it — step 1 is "tests passing, clippy clean" with no mention of feature configs or of consulting CI at all. Nothing in either surface says "the gate is CI, and here is how to check it."

**Counterfactual:** one `gh run list` call in the first five tool calls would have reordered the entire session — CI repair first (it is what actually blocks the merge), documentation second. Instead documentation landed first, and two of the three CI defects I ended up fixing are unrelated to documentation entirely. Mid-session "fmt clean, clippy clean" claims were true but narrower than the framing implied.

**Severity:** med — no wrong work was produced and nothing had to be redone, but the sequencing inverted the user's stated goal and the gate-green claims were misleadingly scoped.

**Status:** fixed-verified — all five CI-equivalent steps now run locally and exit 0 at `be75e705` (`fmt`, `clippy -- -D warnings`, and `cargo test` in all three feature configs). `docs/RELEASE.md`'s new *Large-Cohort Promotion* section lists the full gate including the two non-default configs and puts the ancestry check first, so the next person inherits the correct definition rather than the three-command subset.

**Fix idea / Pointer:** the durable half is already promoted into `docs/RELEASE.md`. The remaining candidate is adding `cargo check --no-default-features` to the CLAUDE.md pre-commit line, or a `cargo xtask gate` alias that runs all five — a local subset that silently omits six of nine cells will keep producing this friction.

---

## W-1 — Dumping an internal pure function's real output beat reasoning from its source

**Observed:** 2026-08-06, finishing the uncommitted `AST_CHUNK_MIN` change in `src/embed/ast_chunker.rs`.

**Pattern:** When the question is *"what does this pipeline actually emit?"*, add a throwaway test that prints the real output and read it — even when the code is an internal pure function whose source is open in front of you.

The existing guidance covers the *external* case: this skill's Phase 1 says *"For tools / external APIs: read the actual response shape, not docs"*, and `get_guide("project-activation-bootstrap")` Phase 2 says *"A claim about how a TOOL behaves needs the call run once and the real output read."* Both are framed around tools and APIs. The extension this datapoint supports: the rule matters **more**, not less, for an internal pure function, because the source being right there makes deriving the answer feel authoritative.

**Counterfactual:** I had already reasoned from `nodes_to_chunks` that the coalesce run was padded by the container's **trailing `}`** gap chunk. The dump showed the run was padded by a **leading** gap spanning lines 1-8 — a full duplicate of every line before the container, emitted because the recursion re-derives gaps against the whole file with `prev_end` reset to 0. I had not predicted that shape. Reasoning alone would have produced a plausible fix for the metadata loss and **missed the duplication entirely**; it is now its own bug file, `docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`. The dump also handed me the exact `metadata` strings (`src/mystore.rs :: impl MyStore ::     pub fn build(&self)`, note the preserved leading whitespace) that 7 new tests assert on, instead of guessed ones.

**Confirming data points:**
1. This session — 2 tool calls (insert dump test, run with `--nocapture`) surfaced one regression mechanism, one unrelated pre-existing bug, and the exact assertion strings for 7 tests.
2. Pending: a second case where a dump-vs-derive scout on an internal function surfaces something the source read missed.

**Impact:** med — caught a second bug for free and prevented seven tests being written against guessed strings.

**Promote-when:** at a second datapoint, promote to memory `reconnaissance` as a bounded rule: *"Before asserting what a chunker/formatter/serializer emits, print its real output once — internal purity is not a reason to skip it (W-1)."* Craft-shaped enough to also justify widening this skill's Phase 1 bullet from "tools / external APIs" to "tools, APIs, and any function whose output shape you are about to assert on."

**Status:** validated — single datapoint, drift caught and a second bug found before the commit landed. Awaiting promotion criterion.

---

## F-4 — Three bug files' own `## Fix` sections carried wrong premises; following them would have produced worse code

**Observed:** 2026-08-06, working the open-bug ledger one entry at a time.

**When:** Reading each bug's `## Fix` section as the plan before implementing it.

**Expected:** A filed bug's Fix section is the most reliable available guidance — it was written by someone holding the evidence.

**Got:** Three of the six were materially wrong, in three different ways:

1. **Stale-by-superseding.** `2026-07-28-reranker-costs-42x...` closes with *"That figure needs correcting regardless"*, but a **CORRECTED** note added later at the top of its own Summary retracts the comparison that claim rests on. The Fix section predates its own file's retraction. Following it would have replaced a possibly-stale `~80 ms` with a guess, across a runtime boundary (TEI vs llama-server) the two numbers do not share.
2. **Wrong severity assessment.** `2026-08-06-ast-chunker-recursion-duplicates-leading-gap` states the trailing-gap branch is *"harmless in size — it emits the container's closing brace"*. A failing test written before the fix showed it emits the closing brace **plus every line after the container to EOF**. A floor-only fix, which is what that framing invites, would have left half the bug.
3. **Already implemented.** `2026-07-28-mcp-servers-outlive-their-clients` ranks *"exit on stdio EOF"* as the cheapest fix and says the shutdown path is unread. Reading it showed `ResilientStdin` absorbs only `WouldBlock`, so a 0-byte EOF already propagates and the process already exits. The orphans are never *sent* an EOF, so the fix is direction 2 (idle timeout) and direction 1 should be struck.

**Probable cause:** A Fix section is written at peak context and then never re-read against later edits to the same file. Nothing links a Summary-level retraction to the Fix section it invalidates, and nothing re-checks a "not implemented" claim against the code.

**Workaround:** Treat a bug file's Fix section as a *hypothesis with a citation*, not a plan. Verify its premises the same way any other claim gets verified — read the code it names, and check whether a later dated note in the same file contradicts it. All three were caught this way; each correction is recorded in the bug file itself.

**Severity:** med — each would have produced a wrong or half fix that passed local tests. Not high only because the verification that caught them is the same reconnaissance pass already required before editing.

**Status:** fixed-verified — all three corrected in their bug files, with the correction stated as a correction rather than silently applied.

**Fix idea / Pointer:** Worth a line in `get_guide("tracker-conventions")`: when a bug file gains a dated retraction, re-read its Fix section in the same edit. See also R-56.

## F-5 — The local gate structurally cannot predict CI: three independent skews, one of them silent

**Observed:** 2026-08-06, after a locally-green six-step gate met a red CI three times in a row.

**When:** Every push this session. The local gate (`fmt`, `clippy -- -D warnings`, `cargo test`, plus three feature-gate builds) exited 0 each time.

**Expected:** A green local gate predicts a green CI, modulo the known Windows and doc-lint gaps.

**Got:** Three distinct classes of local-vs-CI divergence, none of which any amount of local testing would surface:

1. **Toolchain version skew.** Local `clippy 0.1.95`; CI's `dtolnay/rust-toolchain@stable` resolved **1.97.0**. Three pre-existing patterns (`question_mark` ×2, `for_kv_map`) are lints 1.97 emits and 1.95 does not. There is no `rust-toolchain.toml`, so CI re-resolves `stable` every run and can go red **with zero commits**. Clippy breaks on the calendar.
2. **Platform-invisible bugs.** Linux has one path separator, so a forward-slash-vs-backslash mismatch is *unobservable* there. 3488 tests passed locally before each of the three Windows pushes; the separator bug needed CI to exist at all — and one of them needed two CI round-trips, because normalising the seed moved the mismatch downstream into the test's own comparison.
3. **A gate command that cannot run locally.** `cargo clippy --all-features` fails on this workspace by construction — `codescout-embed` has a `compile_error!` for mutually-exclusive ONNX backends. Reaching for `--all-features` as a "stronger" local check produces a build failure unrelated to the code under test.

**Probable cause:** The local gate was specified as a command list (CLAUDE.md's three commands) rather than as an equivalence claim against CI. Nobody pinned the toolchain, so the strongest local check drifts out of alignment silently.

**Workaround:** None applied — recorded rather than fixed, because the remedy is a policy call (pin a toolchain so local == CI and upgrades are deliberate, vs keep floating and absorb periodic lint debt). Two operational mitigations that did work: push early to get a real verdict, and read a failing job's log via the REST endpoint rather than waiting for the whole run.

**Severity:** high — it converted "gate green, ready to merge" into three false readiness claims in one session, and the toolchain half will recur on its own schedule.

**Status:** open — the three lints are fixed and Windows is green, but the *skew* is unaddressed and needs the pinning decision.

**Fix idea / Pointer:** Add `rust-toolchain.toml` pinning the channel CI uses, or add a CI job that fails when `cargo --version` differs from a pinned expectation. Either makes the skew loud instead of silent.

## W-2 — Ordering a capped findings list by severity turned an unactionable gate into a self-explaining one

**Observed:** 2026-08-06, after fixing all 18 findings a bug file tabulated and watching the gate still exit 1.

**Pattern:** When a tool truncates a result list *and* computes a pass/fail verdict over the untruncated set, order the shown slice by whatever drives the verdict. `audit_doc_refs` did `findings.iter().take(50)` in scan order while the exit code scanned all 46 572 — and since most refs resolve, the shown 50 were almost always `resolved`/`low`. The gate reported failure and displayed nothing that caused it.

**Counterfactual:** Without this, the next step was a 16-run per-subdirectory bisect just to see *which files* had findings, repeated after every fix. Worse, the truncation had already corrupted the record: the "18 findings, all false positives" table in `2026-08-06-audit-doc-refs-misreads-symbol-paths-as-files.md` was the count *inside the window*, not the population. Fixing exactly those 18 and declaring victory was the live failure mode — and it is what I was about to do. After the ordering fix the shown 50 were 50/50 high, and the real population turned out to be >50 across 11 subdirectories.

**Confirming data points:**
1. Baseline run: 18 high in the window, `n_refs_broken: 10487`. Post-fix run: 0 high in the window, exit still 1 — proof the window was never representative.
2. The same defect had already been filed once as a family: `docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md`.

**Impact:** high — a gate that hides its own cause trains people to bypass it, and it silently corrupted a bug file's central measurement.

**Promote-when:** A second silent-cap instance appears in a tool that also gates. At two datapoints, promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` as "if a truncated list feeds a verdict, sort by the verdict's key before truncating".

**Status:** validated — fixed in `45669701`, verified live (window went 0/50 high to 50/50 high), and filed as `docs/issues/2026-08-06-audit-doc-refs-gate-hides-its-own-cause.md`.

## W-3 — Reading the product code before believing a red test prevented turning a test failure into a data-loss bug

**Observed:** 2026-08-06, diagnosing nine Windows test failures (WIN-28).

**Pattern:** When a test fails only on one platform, read the product code it exercises before changing anything. The failure may be the test asserting something false *about a correct implementation*.

**Counterfactual:** Two of the three clusters were exactly that shape, and the obvious fix was harmful in both:

- Cluster A's four tests failed because `derive_dead_roots` skips non-absolute `abs_path` rows and `"/gone/old"` is not absolute on Windows. The tempting fix — relax the guard — is guarded by its own test and its own comment: without it the ancestor climb bottoms out at an empty `PathBuf` whose prune `WHERE` **matches every absolute row**. That is a catalog-wide data-loss bug, shipped to make a red test green.
- `lock_path_is_not_sited_in_bare_temp_dir` failed because `per_user_runtime_dir()` returns bare `temp_dir()` on Windows — deliberately, since `%LOCALAPPDATA%\Temp` is already per-user, as `lock_path`'s own doc comment states. The assertion was wrong, not the code. Relaxing the assertion would have deleted a real Unix-side guard against a symlink-truncation attack.

End state: nine failures, three root causes, **zero product changes** — and CI went 14/15 green.

**Confirming data points:**
1. WIN-28, this session: 2 of 3 clusters were correct-implementation cases.
2. Pairs with F-4 — in both cases the written artifact (a test, a bug file's Fix section) was the thing that was wrong, and the code was right.

**Impact:** high — prevented a data-loss regression and preserved a security guard.

**Promote-when:** A second platform-specific failure resolves as "assertion wrong, code right". At two datapoints, promote to CLAUDE.md as "a platform-specific test failure is a claim about the code, not a fact about it — read the implementation and its guard comments first".

**Status:** validated — `cd643d58`, CI run `31098286970`, windows/default 3283 passed 0 failed.

## W-4 — Stating a duplicate hypothesis with its own falsification test, then letting CI run it

**Observed:** 2026-08-06, closing WIN-29 (`Windows-gnu cross`) as a duplicate of WIN-28.

**Pattern:** When closing a bug as a duplicate on inference rather than proof, write the falsification test into the closure — and name the observation that would reopen it. Here: dump both jobs' failing-test sets and diff them; reopen if the cross job ever fails a test `windows-latest` passes.

**Counterfactual:** Without the diff, `Windows-gnu cross` stayed the only red cell with no known cause, which made four red cells look like two independent problems and left the merge picture ambiguous. Guessing "probably the same" without the diff would have been indistinguishable from the truth right up until it wasn't. And the prediction was checkable: the duplicate claim says the cross job goes green from fixture fixes alone, with no MinGW- or wine-specific change. It did.

**Confirming data points:**
1. Failing sets identical, nine tests byte for byte (run `31092134665`).
2. `Windows-gnu cross` green on run `31098286970` after only the nine fixture fixes.

**Impact:** med — collapsed four red cells into one bug and produced a checkable prediction instead of an assumption.

**Promote-when:** A second duplicate closure carries a falsification recipe and it later fires (or holds). At two datapoints, promote to `get_guide("tracker-conventions")` as "a duplicate closure names the observation that reopens it".

**Status:** validated — prediction made before the evidence existed, then confirmed.

## F-6 — A capped findings list corrupted a magnitude estimate for the third time, in the document written to warn about it

**Observed:** 2026-08-06, round 4, planning the doc-drift work from
`docs/issues/archive/2026-08-06-docs-ref-drift-backlog-across-eleven-subdirs.md`.

**Expected (the plan):** "~11 gitignore-aware + ~42 marker + ~8 by hand" ≈ **61** findings,
sized as one focused session.

**Got:** per-directory `--paths` counts totalling **several hundred**, with four
directories each hitting the 50-cap so their true counts are still unknown. The estimate
was ~5× low.

**Probable cause:** the estimate was read off the JSON `findings` array, which
`OutputGuard` truncates to 50 while the exit code is computed over all of them. So the
plan described the *window*, not the population — and after the ranked-ordering fix (W-2)
the window is the 50 most severe, which reads even more like a complete list.

**Two aggravations specific to this instance:**

- `overflow.total` reports **46683**, which is `n_refs_found` — total *references scanned*,
  not total findings. Sitting beside `shown: 50` it invites reading 46683 as the finding
  count. Neither number is the one a planner needs.
- Scan order decides which of the ≥N high findings land in the window. The first top-50
  looked manual-dominated; after the manual was fixed, an entirely different 50 appeared
  from directories that had shown nothing. Progress was invisible in the aggregate for
  three consecutive rounds of real fixes.

**Severity:** med — no wrong code, but it mis-scoped the work twice and would have led to
reporting "backlog cleared" after clearing one directory's worth.

**Status:** mitigated — the method is now pinned in the backlog bug's § Resume and in
round 4's *Do NOT re-do*: count with `--paths` per directory, never from the top-level run.

**Fix idea / Pointer:** the audit should report an uncapped `n_findings_by_severity`
alongside the capped array — three integers, no truncation risk. This is the same defect
class as `2026-08-06-audit-doc-refs-gate-hides-its-own-cause` (already fixed by ranking):
the gate's own summary does not carry the number the gate acts on. Third instance overall
— the first corrupted "18 findings" in a bug file, the second hid the gating cause, this
one mis-sized the plan. Kin R-50 ("the view is not the set").

## W-5 — Invoking the tool under test, instead of reading it, produced five defects that reading had missed

**Observed:** 2026-08-06, round 4, working the `audit_doc_refs` backlog. The plan of
record had been written by *reading* the extractor; this round ran it.

**Pattern:** when the seam is a **tool**, the scout is one real invocation whose output
you read — preceded by confirming the artifact you invoked was built from current source.

**Counterfactual, with the concrete finds:**

1. The local release binary was **two days old and predated all four source files** of the
   tool under test. Every number from it would have described the *old* extractor — the
   exact error that made three prior CI verdicts meaningless (F-5 kin). Caught by
   `find src crates -name '*.rs' -newer target/release/codescout` before any measurement.
2. Running it died with **SIGABRT** — `ignore`'s `matched_path_or_any_parents` panics
   rather than errors outside its root. Unknowable from the signature, which returns
   `Match`, not `Result`. Reading the crate docs would not have surfaced it either.
3. A test written for that panic asserted on the real verdict and caught a **pre-existing**
   escape: `/etc/passwd:12` resolved **`Resolved`**, range-checked against the host's file.
   Two ref kinds lacked a guard the third had always carried.
4. The backlog bug's own written premise — "the extractor only reads code spans and
   links" — was **false**: `parse_refs` walks code-block text too. Measured split of one
   top-50: 18 prose / 18 fenced / 2 indented, so half the population was invisible to the
   plan built on that sentence.
5. `RefPosition` was populated by the parser and **read by nothing**. The discriminator
   the fix needed already existed, unused — visible only by grepping its uses, not by
   reading the parser that writes it.

Without the invocation: a SIGABRT shipped into a CI gate, a path escape left in place, a
mechanism hand-rolled beside the unused one already there, and a plan built on a false
premise about which half of the corpus it covered.

**Impact:** high — four of the five are correctness defects in a gate, and the fifth would
have produced duplicate machinery.

**Promote-when:** already at threshold with W-1 ("dumping an internal pure function's real
output beat reasoning from its source") — same lesson, one level up: W-1 was a function,
this is a whole tool. Captured as **R-57** in `docs/trackers/reconnaissance-patterns.md`
with a proposed Phase 1 addition. Promote to the reconnaissance SKILL.md if a third
tool-behaviour seam repeats it, since the lesson is craft-shaped, not project-shaped.

**Status:** validated — five independent finds in one session, each confirmed by a
failing test or a crash, not by inference.
## W-6 — Mechanising the decidable half of a cleanup is what makes the undecidable half visible

**Observed:** 2026-08-06, round 5, clearing the stale archive-citation class.

**Pattern:** when a cleanup has a part that is decidable *without reading prose*,
mechanise exactly that part and run it over the whole corpus first. The residue is
then, by construction, the part that needed judgement — and it is small enough to read.

The decidable rule here:

> ref is `<dir>/<name>.md`, that path does not exist, `<dir>/archive/<name>.md` does
> → insert `archive/`.

**Counterfactual, concrete:** the plan of record sized this class from a sample of 11
and called it "mostly bug files archived today". Run over the corpus it was **156
citations across 62 tracked files** — 14× the sample — and it included a *tracker*
(`vdi-reliability-session-log.md`), not only bug files, which the sample had missed
entirely. Fixing 156 sites by hand at a conservative two calls each is ~300 tool
calls; the script was three.

**The part worth keeping:** the mechanical pass then surfaced something no rule could
have decided. `windows-platform-support.md` listed three files under **"Active bug
files"** — and after the repoint those three read
`docs/issues/archive/…`, i.e. "active" pointing at the archive, in a section that
already had a separate "Archived CI-Windows bugs" line beside it. The contradiction was
invisible while the paths were merely *wrong*; it became legible the moment they were
*right*. Same shape in reverse for four other sites, which on inspection were dated
`**Observed:** <date>` statements where "the only open bug" was true when written — those
were correctly left alone.

**Impact:** high — 14× scope correction, ~300 tool calls avoided, and one live tracker's
present-tense claim corrected that a hand pass would very likely have re-typed.

**Two guards the mechanisation needed, both non-obvious:**

1. **Idempotence by construction.** The replacement inserts `archive/` into the very
   substring being matched, so a second pass matches nothing. Verified rather than
   assumed: `grep -rc "archive/archive"` → 0.
2. **Shape assertion on the diff.** 195 insertions / 195 deletions across 62 files — a
   pure 1:1 line swap is what a path repoint must look like, and any other ratio would
   mean the sed had eaten something. Cheaper and stricter than reading 62 diffs.

**Promote-when:** a second cleanup where a decidable rule is separable from a judgement
call. At that point promote to the reconnaissance SKILL.md, since the lesson is
craft-shaped: *mechanise the decidable part first; the residue is the judgement, and its
size is the estimate you should have had.*

**Status:** validated — 156 sites, 0 remaining, diff shape asserted, one semantic
contradiction recovered that the mechanical rule could not have found.
## W-7 — Verify a filesystem-dependent gate against a fresh clone, not the working tree

**Observed:** 2026-08-06, round 5. `Audit Doc Refs` went red in CI on a tree where the
local gate had exited 0 six times in a row.

**Pattern:** for any gate whose verdict depends on *what exists on disk*, verify against
`git clone --depth 1 file://$PWD /tmp/x` and scan that, with the same binary. A working
tree is strictly more populated than a clean checkout — untracked runtime state, worktrees,
build output, tool caches — so every "does this path exist" check is optimistic locally,
and optimistic in the direction that produces a false pass.

**Counterfactual, and it is sharper than the usual F-5 skew:** the four CI findings were
`.git/worktrees/`, `.claude/worktrees/` and `.codescout/private-memories/`. Locally those
directories exist, so the refs **resolved** — they produced no finding *at all*. The local
run could not report them even in principle, at any severity. This is not "local is a
weaker gate"; it is "local cannot see this class". Six clean local runs were six
measurements of the wrong tree.

The clone cost one command and ~30 seconds, and it converted the next push from a guess
into proof: 881 files, 46673 refs, **0 high, EXIT=0**, with 8882 broken refs still
reported at `med`. Without it the alternative was another ~12-minute CI round-trip per
attempt, and one had already been spent.

**The second lesson, which is about the test rather than the gate.** The
`gitignored_path` cap *had* a passing test. It asserted `.worktrees/my-feature` — a file
*inside* an ignored directory. Docs name the directory itself, `.claude/worktrees/`, and
the matcher reads the final path component, which a trailing separator leaves empty. So
every directory-shaped ref escaped the cap while the suite stayed green. The fixture shape
was the blind spot, not the assertion: **when a rule is about paths, cover both `foo` and
`foo/`** — they are the same location and they are not the same string.

**Impact:** high — caught a whole silently-escaping class in the cap, and established a
verification step that removes CI round-trips from the loop for any filesystem-dependent
gate.

**Promote-when:** immediately for the project-shaped half — the fresh-clone check belongs
in `docs/RELEASE.md` beside the `Audit Doc Refs` step, since it is this repo's gate. The
craft-shaped half (`cover foo and foo/`; verify environment-dependent gates against a
clean checkout) is **R-58** in `docs/trackers/reconnaissance-patterns.md`.

**Status:** validated — one CI failure diagnosed, reproduced locally, fixed, and the fix
proven against a CI-equivalent tree before pushing.
## W-8 — Reading the fallback gate eliminated the hypothesis a whole harness was going to measure

**Observed:** 2026-08-06, round 6, working the open-bug ledger. The `symbols` search flake
(`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`) was
filed with an explicit instruction: *"Not proposed — root cause unconfirmed. Needs a
controlled, scripted reproduction (N parallel `symbols(name=X)` calls immediately
post-activation, repeated across several projects/runs) before a fix can be targeted."*

**Pattern:** when a bug names a suspected mechanism, find the **fallback or guard that
would mask that mechanism** and read its condition *before* costing a reproduction. An
unreachable hypothesis is cheaper to eliminate than to measure.

**Counterfactual.** Everything pointed at LSP warming, and not weakly:

- the source's own comment names the exact state — *"a pathological LSP state (silent
  `workspace/symbol` on a still-indexing server …)"*;
- three degradation paths really are invisible to the agent: budget-exceeded yields
  `Ok(Vec::new())` with only a `tracing::warn!`, and both a task error and a join error are
  silently `continue`d;
- the symptom (0-match, succeeds on retry) is textbook cold-index behaviour.

One line ended it. The tree-sitter fallback is gated on `matches.is_empty()` — the
aggregate of *already-filtered* matches — so it runs whenever the final answer would be
zero, no matter which languages degraded, and non-matching LSP results never enter
`matches` and so cannot suppress it. **A 0-match therefore implies tree-sitter found
nothing either, and tree-sitter never touches the LSP.** Server readiness cannot produce
this symptom at all.

So the harness as specified — parallel calls instrumented for LSP readiness — would have
run, produced a flake, and measured the wrong variable. The surviving hypothesis is a
different bug class entirely: both paths key off one `root` from
`require_project_root_for` (`symbols.rs:225`), and a root not yet settled post-activation
makes the LSP query one tree while the walker walks the same wrong tree — both correctly
returning nothing. Kin to the archived shared-server active-project race, which is the
same class already seen and fixed here once.

**Impact:** high — avoided building a harness around a refuted mechanism, and redirected
the instrument from LSP readiness to the resolved `root`, which is a one-line log rather
than a parallel-call rig.

**The restraint is part of the win.** No code change was made. `symbols` is the most-used
tool in the server, the cause is narrowed but *not* confirmed, and the bug's own rule is
"not proposed until confirmed". Patching the disclosure now would have papered over
whichever path actually fires. What did land is the diagnosis plus the house convention to
mirror once it is confirmed (`references.rs`'s `completeness_warning`, `list_overview`'s
`"lsp": "warming"`) — so the next session inherits the answer and the pattern, not a guess.

**Promote-when:** a second bug where reading a guard refutes the filed hypothesis. Captured
as **R-59**; craft-shaped, so it graduates to the reconnaissance SKILL.md rather than to
project memory.

**Status:** validated — hypothesis refuted from code, replacement hypothesis named with its
precedent, and the experiment redesigned before any of it was built.
## F-7 — A run-level CI `status` was quoted as per-job evidence, and the false claim was committed

**Observed:** 2026-08-06 ~17:39–17:45Z, resuming after compaction to re-query CI for the
round-6 commits.

**When:** Writing the round-6 compaction handoff. `gh run list --branch experiments` returned
`status: queued` for five runs, so the handoff recorded that no job had started and that the
round-6 fixes had local verification only.

**Expected:** A run whose `status` is `queued` has not begun executing.

**Got:** Run `31122862704` reported `status: queued` while **six of its jobs had already
completed `success`** — including `Audit Doc Refs`, the job this whole work stream exists to
turn green. Run `31122588792` reported `queued` with 4 success / 8 cancelled / 3 queued. The
run-level field is an aggregate that stays `queued` until every job leaves the queue; it
carries no information about whether jobs ran.

**Probable cause:** The claim was taken from the cheapest field that looked authoritative
instead of from the per-job tally, and the field's *name* invites exactly that reading. Same
shape as W-5 (reading the tool instead of invoking it) and R-19 (asserting a checkable fact
without reading it this session): the real answer was one `gh run view --json jobs` away.

**Severity:** med — no code was wrong, but the false claim was committed in `8f5b72fb` to a
document whose entire purpose is to be trusted by the next session, and it understated the
branch's real CI coverage from 8 green jobs to zero. A later session reading it would have
re-run work already proven green, or blocked the promotion for want of evidence it had.

**Status:** fixed-verified — the CI section above is rewritten from per-job data, and the
tally command is pinned there so the shortcut is not available next time.

**Fix idea / Pointer:** For any CI claim, the unit of evidence is the **job**, never the run.
A run-level status answers "is this run finished", never "did anything run".

## F-8 — Seventy minutes diagnosing a CI outage from run metadata, without reading the provider's status page

**Observed:** 2026-08-06, 17:15–18:30Z.

**When:** Five, then six runs sat `queued`. I inferred the mechanism from `gh` output twice:
first "no job ever started" (F-7 — false), then "arrival-order contention: our own six
pushes are starving the tip".

**Expected:** The queue behaviour was ours to reason about from run and job metadata.

**Got:** `githubstatus.com` had carried **`Actions: major_outage`** since **15:22:49Z — two
hours before the first affected run**, and before any of my reasoning. The incident text
describes exactly what we observed: *"Workflow runs are still failing or delayed in starting,
and some queued jobs may time out."* That single fact accounts for every symptom: jobs
cancelled at `steps=0` (queued jobs timing out), HTTP 500 then 502 on three cancel attempts
against one run, and **no run created at all** for `a53f1760` thirteen minutes after the push
was confirmed on `origin/experiments`.

**Consequence — a retraction.** The arrival-order-contention mechanism is withdrawn.
Cancelling three superseded runs to "free ~45 job slots" was harmless (all three were
docs-only commits contained in the tip) but it did not buy what it claimed, because runners
were not being allocated at all. Queue depth was never the binding constraint. The
falsification test stated alongside that claim was answered by external evidence rather than
by the observation it proposed — which is the cheaper way, and was available the whole time.

**Probable cause:** Three rounds of internal inference were run and one authoritative external
check was not. The status page is cheaper than any single `gh` call made during those seventy
minutes, and it outranks all of them: an open provider incident reframes every downstream
observation, so it belongs *first*, not last.

**Severity:** med — no code was harmed and nothing shipped wrong, but the cost was ~70 minutes
of misdirected diagnosis, one false claim committed to this log (F-7), one retracted
mechanism, and three cancellations justified by a cause that was not operative.

**Status:** fixed-verified — the outage is confirmed as the cause. The `concurrency` block
added in `a53f1760` stands on its own merits (it is correct and still wanted) but **cannot be
validated until Actions recovers**, since supersession only demonstrates itself on the next
push that lands while a run is live.

**Fix idea / Pointer:** Before diagnosing ANY CI symptom — queued, cancelled, missing run,
API 5xx — read the provider's status page first. One call, and it is authoritative:

```
curl -s https://www.githubstatus.com/api/v2/summary.json
```

Pairs with F-7: both are the same failure at different depths — reaching for the reading that
is available rather than the one that is authoritative.

## W-9 — Running CI's own toolchain locally, during an outage, caught a red-CI-in-waiting

**Observed:** 2026-08-06/07, while GitHub Actions was in a declared major outage and no runs
were being created for this repo. Two of the fifteen jobs — `Clippy` and
`Test (ubuntu-latest / default)` — had never executed on any commit carrying the round-6
code.

**Pattern:** When CI is unavailable *and* its toolchain floats, install the version CI's
`@stable` currently resolves to as a side-by-side rustup toolchain and run the gate through
`cargo +<version>`. The default toolchain stays untouched, so the cost is a download and
nothing else — no risk to the dev environment, and it needs no CI at all.

**Counterfactual:** local `stable` is **1.95.0**; `dtolnay/rust-toolchain@stable` now resolves
to **1.97.1**, and there is no `rust-toolchain.toml` to hold them together. Clippy 1.97 flags
`clippy::useless_borrows_in_formatting` at two sites in `src/tools/markdown/tests.rs` that
1.95 accepts, and the workflow gates on `-D warnings`. So the `Clippy` job was going to fail
on the first run GitHub created after the outage — on a branch whose promotion is blocked on
15/15, after a full day of work whose entire purpose was getting that gate green. Worse, it
would have arrived amid the `cancelled`/`failure` noise the outage was already producing, so
the first reading would plausibly have been "outage artefact" rather than "real lint error" —
the misread F-7 and F-8 both already made once today.

**Confirming data points:**
1. F-5 predicted exactly this class — its text names "clippy 1.95 vs 1.97" — and now has its
   first *observed* instance rather than a hypothesis. The mitigation F-5 lacked is this
   pattern.
2. The fix is two characters. Finding it through CI would have cost a full 15-job round trip
   plus a tip-moving commit, at a moment when runs were not being created at all.

**Impact:** high — converts F-5 from an unmitigated structural gap into one with a known,
cheap workaround, and it is the only form of CI substitution available during an outage.

**Promote-when:** a second CI-only failure is caught this way. At two datapoints, promote to
`docs/RELEASE.md`'s gate as an explicit step — run the gate on whatever `@stable` resolves to,
not merely on the local default — or resolve the standing pin-vs-float decision by pinning
`rust-toolchain.toml`, which removes the divergence by construction rather than papering over
it.

**Status:** validated.

**Extended 2026-08-07 to CI's full Linux matrix.** The first run covered `default` features
only. CI's test job is 3 OS × 3 configs, and its greens for the other two Linux configs were
recorded on `fcb6598f` — six commits behind the tip, predating both of the day's code fixes.
Re-run on 1.97.1 with `LIBRARIAN_WORKSPACE` seeded empty exactly as the workflow does:

| config | flags | result |
|---|---|---|
| `default` | *(none)* | 3505 passed / 0 failed / 44 ignored |
| `no-features` | `--no-default-features` | 2587 passed / 0 failed / 39 ignored |
| `local-embed` | `--features local-embed --no-default-features` | 2588 passed / 0 failed / 39 ignored |

**8680 tests, zero failures.** `default` matches the earlier run's counts exactly, so seeding
an empty librarian workspace changed nothing — no test was leaning on the developer's real
`workspace.toml`, which is the hidden env dependency `docs/conventions/test-env-isolation.md`
exists to prevent.

Two incidental readings worth keeping. Reading the workflow *before* running is what surfaced
both the `components: rust-analyzer` line — independent confirmation of F-9's diagnosis, since
CI installs precisely the component `--profile minimal` omitted — and the empty-workspace
seeding the first run had skipped. And `local-embed` runs exactly **one** test more than
`no-features` (2588 vs 2587), so that feature's entire suite-level delta is a single case;
thin coverage for a flag that swaps the embedding backend.

Still not locally reproducible, and still owed to real CI: the macOS and Windows rows,
`Windows-gnu cross`, and `Tool Docs Sync`.

## F-9 — A minimal-profile toolchain install produced 12 test failures that read as a compiler regression

**Observed:** 2026-08-07, immediately after installing 1.97.1 to run CI's gate locally (W-9).

**When:** the first `cargo +1.97.1 test` run.

**Expected:** either a clean run, or failures attributable to the compiler change.

**Got:** 12 failures, every one in `lsp::manager` or `tools::symbol`, each panicking with
`rust-analyzer is unreachable. The rustup shim is on PATH but the component is not installed`.
The toolchain had been installed `--profile minimal --component clippy,rustfmt`, so it carries
no `rust-analyzer`, while the shim already on PATH resolves against whichever toolchain is
active. Adding the component made the run clean at 3505 passed / 0 failed.

**Probable cause:** the install command was written for the two components *the gate* needed
and ignored what *the suite* needs. Note the failure mode is a third state — **shim present,
component absent** — distinct from "not installed at all", and it arises only because another
toolchain created the shim.

**Severity:** med — no code was wrong, but for several minutes the evidence read as "CI's
toolchain breaks 12 tests on the branch we are trying to promote", which is precisely the
shape that triggers a wrong revert. The tell was the clustering: 12 failures confined to the
two rust-analyzer-dependent modules is an environment shape, not a compiler change. Reading
the panic text beat inferring from the count.

**Status:** fixed-verified — `rustup component add rust-analyzer --toolchain 1.97.1`, then
3505 / 0.

**Fix idea / Pointer:** when installing a toolchain to reproduce CI, install the components
the *suite* needs, not just the gate's: `--component clippy,rustfmt,rust-analyzer`. Better,
pin `rust-toolchain.toml` with an explicit `components` list so CI and local get the same set
by construction — the same fix W-9's promote-when proposes, and the same standing pin-vs-float
decision.

## W-10 — Apparent duplication was the recovery path; merging it would have worsened the bug being fixed

**Observed:** 2026-08-07, fixing the `symbols` 0-match flake
(`docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`).

**Pattern:** Before merging two code paths that apply the same filter and look redundant,
establish what *triggers* the second one. If the second path runs precisely when the first
fails, the duplication is a retry, and collapsing it removes the recovery rather than the waste.

**Counterfactual:** `search_project_symbols` walks the project twice with the identical filter
(`is_file` + `detect_language`) — once to build the `accepted_files` set that gates every LSP
match via `in_walk`, once for the tree-sitter fallback. Reusing the first set for the second
walk is an obvious-looking win on the server's most-used tool, and it is wrong: the fallback's
gate is `if matches.is_empty()`, so a walk-1 truncation — which filters out every LSP symbol —
is exactly what *triggers* walk 2. A complete walk 2 then still finds the symbol. Merging them
would have converted a recoverable truncation into an unconditional zero, in the same commit
that claimed to fix zeroes. I was one edit from doing it.

**Confirming data points:**
1. This session, `symbols`: the merge was scoped, then abandoned on reading the fallback gate.
2. W-8 (same bug, earlier): reading that *same* `matches.is_empty()` gate is what eliminated
   the LSP-warming hypothesis. One gate, two saves — which is itself the argument for reading
   the gate rather than the call site.

**Impact:** high — the change would have passed review as an optimisation, passed the whole
test suite, and silently removed the retry that the bug's own recoveries depended on.

**Promote-when:** a second case where a "redundant" path turns out to be failure-triggered. At
two datapoints, promote to the codescout `reconnaissance` memory as: *before deduplicating two
paths, read the condition that selects between them.*

**Status:** validated.

## F-10 — An unkilled mutation nearly got a correct test rewritten

**Observed:** 2026-08-07, mutation-testing the new `symbols` walk-error guard.

**When:** After `an_unreadable_directory_is_counted_rather_than_silently_dropped` passed, I
mutated the error counting to confirm the test would fail without the fix.

**Expected:** the test dies.

**Got:** it passed. The immediate reading — "the test is vacuous, probably my root-guard is
returning early" — was wrong. `edit_file(replace_all=true)` had matched **one of two**
occurrences, because the two `Err` arms sit at different indentation depths (the fallback walk
is nested inside `if matches.is_empty()`), and my `old_string` carried one indentation. The
surviving counter kept the warning present and the test green.

**Probable cause:** two compounding things. `replace_all` reports no match count (see U-38), so
a partial application is silent; and "mutation survived" has two explanations — weak test, or
incomplete mutation — of which only the first is the one people reach for.

**Workaround:** two cheap checks separated them in one round trip: `id -u` plus an actual
`chmod 000` probe proved the test's precondition really held (not root, directory genuinely
unreadable), and grepping for the mutation marker showed it had landed at only one of the two
sites. With both counters neutralised the test died as intended.

**Severity:** med — no wrong code shipped, but the wrong reading would have meant rewriting a
correct regression test for a real defect, weakening it in the name of strengthening it. That
is worse than having no mutation step at all, because it launders a bad edit through a rigorous
looking process.

**Status:** fixed-verified — mutation completed, test killed, both counters restored, full
suite green at 3515.

**Fix idea / Pointer:** when a mutation fails to kill a test, verify the mutation landed
**before** doubting the test — grep for the marker and count the sites. Corollary for
`replace_all` on Rust: identical statements at different nesting depths are *different strings*.

## F-11 — A bug file prescribed a fix that the code already had

**Observed:** 2026-08-07, starting the WIN-30 fix (two Windows CI timing flakes).

**When:** Before editing, reading the two tests the bug file named.

**Expected (bug file `## Fix`):** *"For the background-output test — replace the fixed wait with a
bounded poll on the captured output (deadline plus small sleep, fail on deadline)."*

**Got (scouted reality):** `background_command_with_quotes_captures_output`
(`src/tools/run_command/tests.rs`) already polled — `for _ in 0..50` with a 100 ms sleep, a 5 s
bound, breaking on the expected substring. There was no fixed wait to replace. The same item's
*Root cause* ("a race between the spawned background process writing its output and the assertion
reading it") was equally written from the CI log line rather than from the source.

**Probable cause:** the bug was filed during the promotion crunch from two CI log excerpts.
`background command output not captured` is exactly the message a fixed-wait test *would* emit, so
the diagnosis was plausible and never checked. Nothing in the bug-file workflow requires the
`## Fix` section to have read the code it prescribes changing.

**Workaround:** none needed — the scout relocated the real defect before any edit. The poll's
`if let Ok(v) = out` **discarded the `Err` arm**, so "the command never ran" and "the output is
still flushing" ended the loop identically and produced the same contentless message. That is why
the CI red carried no information — and it is the same defect class as the `walker.flatten()` bug
fixed the same day in `symbols`: an error arm dropped, a false negative indistinguishable from a
true one.

**Severity:** med — the prescribed change would have compiled, passed, and been committed as a
fix. The bug would have been archived and the next MSVC flake would have read as a regression of a
fix that never existed. No wrong code, but a wrong ledger, which is harder to detect later.

**Status:** fixed-verified — the bug file's Root cause and Fix are corrected in place, the real fix
landed, and it proved itself under wine on its first run.

**Fix idea / Pointer:** treat a bug file's `## Fix` as plan text, subject to the same
reconnaissance as any other plan (R-62). Cheap tell: a Fix section that names no `path:line` for
the code it changes has probably not opened it.

## W-11 — Scouting the prescribed fix turned an inert commit into a diagnostic that paid off within the hour

**Observed:** 2026-08-07, WIN-30.

**Pattern:** Before implementing a fix designed in an earlier session — a bug file's `## Fix`, a
plan task, your own prior note — read the code it prescribes changing. Prescriptions written from
*failure output* are the highest-risk kind, because the output is precisely what the
plausible-but-wrong mechanism would have produced.

**Counterfactual:** Implementing WIN-30's Fix as written meant adding a bounded poll to a test
that already had one — a diff that changes nothing, a green suite, a commit reading like a fix,
and an archived bug. The flake survives. Discovery cost would then be the *next* MSVC red,
misread as a regression of a fix that had never existed, with the bug file by then asserting the
poll was fixed and actively steering the reader wrong.

Instead the scout found the discarded `Err` arm. The replacement assertion carries last stdout,
last error, and an error count — and on its **first** execution, run under wine, it printed
`last stdout: "Can't recognize 'py -c \"print('bg-ok', 2+2)\"' as an internal or external
command…"`, which (a) explained the wine failure as a missing Python launcher rather than a race,
(b) proved spawn, capture, and the `type @bg_*` read path all work under wine, and (c) retired one
member of WIN-27's twelve-test *root cause unknown* wine cluster, open since 2026-07-02.

**Confirming data points:**

1. F-3 (this log) — a plan cited a `RecoverableError.hint` field that did not exist; caught
   pre-dispatch.
2. F-11 (this log) — a bug file prescribed a fix the code already had.

Both are one shape: a **design artifact describing code its author had not read**.

**Impact:** high — converted an inert commit into the single piece of evidence that closed a
five-week-old unknown root cause.

**Promote-when:** criterion already met at two datapoints. Promote to the reconnaissance skill as
an explicit Phase 1 bullet — *a bug file's or plan's prescribed fix is scouted like plan code:
read the code it names before implementing it.* Recorded as R-62 carrying that proposal.

**Status:** validated.

## F-12 — Seven tests, a green gate, and the shipped output was still useless

**Observed:** 2026-08-07, immediately after `624f7f05` (the `grep` hidden-skip warning) reached the
live MCP surface.

**When:** First real call against the codescout repo after the rebuild — a deliberate
live-verification step, not a test.

**Expected:** the warning names `.github/`, since surfacing exactly that entry is why the fix
exists. Its bug file, its commit message, and its first test all say so.

**Got:**

```
0 matches

warning: this zero describes what was searched, not the pattern. Hidden paths were not
searched, including .buddy/, .cargo/, .claude/, .env, .env.amd and 11 more at the search
root. …
```

`.github/` is twelfth of sixteen alphabetically, behind five `.env*` files, and the cap was 5. The
warning was as unhelpful as the bare zero it replaced — while reporting, accurately, that it had
found something.

**Probable cause:** every one of the seven fixtures created **exactly one** hidden entry, so
`hidden.len() > 5` never held and the `more > 0` branch was never executed by any test. The
truncation and the ordering were both written, both shipped, and neither was exercised.

The uncomfortable part: `memory("test-design-discipline")` already carries the lens that catches
this — *"One test per branch; both sides of every condition. Each new `if` / `match`-arm /
`Some`-vs-`None` branch needs a test that REACHES that branch."* I applied it carefully to the
`Option` returned by `completeness_warning` (three tests pin the `None` side) and did not apply it
to the `if more > 0` two lines below. Having the lens is not the same as sweeping every new branch
with it.

**Workaround:** none — fixed in `cdfbbe0f`. Directories sort before files, cap raised to 8, and
`a_pruned_directory_survives_truncation_of_the_hidden_list` builds the pathology in miniature (nine
dotfiles that all sort before a directory). Mutation-verified: reverting to `names.sort()` kills
only that test and its failure output reproduces the live symptom.

**Severity:** med — no wrong behaviour and no false claim, but the feature did not deliver its one
purpose, and the green gate actively argued that it had. That is the expensive kind of
incompleteness: a passing suite is evidence, and here it was evidence for the wrong proposition.

**Status:** fixed-verified — `cdfbbe0f`, gate green at 3523, mutation-killed, and confirmed on the
real tree after rebuild.

**Fix idea / Pointer:** when a change adds a **cap, a truncation, an aggregation, or an ordering**,
the fixture must exceed the cap — a one-element fixture proves only that the empty and
single-element cases work. Pairs with W-12: the live-tree call is what made the gap visible, and it
is a distinct step from the gate rather than a nicer way of running it.

## W-12 — The live-tree call is a distinct verification step, not a nicer way to run the tests

**Observed:** 2026-08-07, verifying `624f7f05` after `cargo rb` + `/mcp`.

**Pattern:** After a fix that changes what a tool **says** — warnings, hints, summaries, rendered
output — call it once against the real repository and read the actual bytes. Do this even when the
gate is green, and treat it as its own step with its own possible outcome, because unit fixtures
are built to be minimal and real trees are not. Minimality is precisely what hides cap,
truncation, ordering, and aggregation defects.

**Counterfactual:** Without the live call, `624f7f05` ships as done — seven tests, clippy clean,
3522 green, CI 15/15 on attempt 1, bug file archived with a fix SHA. Every gate the project has
would have agreed. The warning would then have fired on real zero-matches showing `.buddy/,
.cargo/, .claude/, .env, .env.amd` — never `.github/` — so the next reader hits the identical
false-negative the bug was filed for, now with a warning present and pointing elsewhere, which is
worse than silence because it looks like the question was already answered. Discovery would have
required someone re-walking U-39 from scratch.

**Confirming data points:**

1. This session, round 7 — the round-6 `create.rs` fix was proved inert in a live MCP session
   despite a green suite, because the served binary is a separate artifact. Same lesson, one layer
   lower (*is the code running?* rather than *is the output useful?*).
2. F-12 (this log) — green gate, useless output.

**Impact:** high — it is the only step that caught a defect defeating the feature's entire purpose,
and it cost one tool call.

**Promote-when:** met at two datapoints, and DISCHARGED 2026-08-07 (see Status). The Standard Ship
Sequence in `docs/RELEASE.md` now names a live-surface call as an explicit step for any change to
tool-facing output, distinct from `cargo rb` + reconnect (which only establishes that the new code
is running).

**Status:** promoted-to-permanent-docs — `docs/RELEASE.md` § *Before cherry-pick: read the live output of any tool-facing change (required)*, with a pointer at step 1 of the Standard Ship Sequence so a reader who scans only the code block still meets it. Marked **required** rather than advisory — unlike the mutation-test step beside it — because it costs one tool call and no cheaper check covers the class.

## F-13 — A two-point probe produced a recommendation a 28-point probe inverted

**Observed:** 2026-08-07, calibrating researcher's rerank `min_score`.

**When:** Immediately after probing the reranker with one on-topic and one deliberately absurd
document, and again an hour later after widening the sample because the task itself said to.

**Expected:** the two-point probe bounds the scale. On-topic scored `0.9965708`, absurd scored
`0.0000107` — four clean orders of magnitude, so any threshold in `0.001`–`0.05` looked safe with
enormous margin. I wrote that into the bug file, the task, and a commit message.

**Got:** at 28 documents across four relevance tiers, the bands **overlap**. A same-domain document
("Cargo resolves a dependency graph into Cargo.lock") scored `0.0000108` — *below* banana bread at
`0.0000114`. Ten documents that all genuinely answered their query included a hedged phrasing at
`0.0000105`, a terse correct answer at `0.0000114`, and a raw `error[E0502]` demonstrating the exact
mechanism at `0.0000111`. The `on`-tier floor came out **below** the off-topic ceiling, so the
recommended range would have dropped correct answers.

**Probable cause:** the two samples were, by construction, the extremes — I chose "clearly relevant"
and "absurd" precisely to bound the range. On a **bimodal** distribution the extremes are exactly
the two points that carry no information about the middle, and the middle is where a threshold
lives. `cross-encoder/ms-marco-MiniLM-L-6-v2` (22M params, MS MARCO, lexically driven) rewards
surface-token overlap with the query and collapses everything else into a `1e-5` floor — so two full
correct answers to different queries scored `0.934` and `0.0000989`.

**Workaround:** none needed — retracted in place, in the bug file and the task, before anyone acted
on it. Both retractions are recorded as their own Evidence subsections rather than edited away, so
the wrong advice and its correction sit next to each other.

**Severity:** high — higher than F-12. The recommendation was *published* (bug file, task
description, commit message `65075c6f`) and was specific enough to act on directly. Acting on it
would have silently dropped same-domain sources from every research result, and the symptom — fewer
sources — is one the threshold is *supposed* to produce, so it would have looked like the fix
working.

**Status:** fixed-verified — retracted with the 28-point evidence, and the corrected conclusion is
stronger than the original question: no threshold works on this model, which redirects the bug from
tuning to model selection.

**Fix idea / Pointer:** when characterising an unknown numeric scale, **the first probe must span
the middle, not the extremes.** Two hand-picked endpoints measure the range and say nothing about
the distribution's shape, and "clearly A" versus "clearly not-A" is the natural pair to reach for.
Minimum viable shape is three tiers with the middle one adversarial — the case a threshold must NOT
catch. Kin to F-12 (a one-element fixture never reaches the truncation branch): same failure, moved
from test fixtures to measurement samples, and in both the sample was chosen for convenience and
happened to omit the only region that mattered.

## W-13 — Every bug premise checked this session was wrong, and four fell to one command each

**Observed:** 2026-08-07, working five open bugs in sequence.

**Pattern:** Before working a bug, treat its **premise** as a claim rather than as context, and
spend the cheapest measurement that could falsify it. Bug files are written under time pressure
from a plausible mechanism plus a symptom; the mechanism is the part that goes stale, and it is
usually the part the whole file is organised around.

**Counterfactual, per bug — all five had a false premise or a wrong prescription:**

| bug | premise | falsified by | what the work would have been |
|---|---|---|---|
| MCP orphans | "18 **orphaned** `start --debug` processes" | `ps -o ppid=` — every one has a living `claude` parent | hunting a broken stdin-EOF path that is in fact correct (verified: `start --debug < /dev/null` exits in ~30 ms) |
| kotlin heap | "codescout spawns kotlin-lsp with **no `-Xmx`**" | `jcmd VM.flags` — `InitialHeapSize=128 MiB`, which only the distro's own `vmoptions` sets | adding a cap that the installed package already applies, twice over |
| researcher rerank | "run real queries, read Langfuse's `rerank` span in/out counts" | one `curl` to `/rerank` | building query + Langfuse plumbing to count drops, when the defect was the score **scale** |
| WIN-30 | "replace the fixed wait with a bounded poll" | reading the test — it already polled 50 × 100 ms | a no-op committed as a fix, bug archived, flake surviving (F-11) |
| audit-doc-refs | "the high-finding count varies; the gate is non-deterministic" | 57 paired warm/cold runs — exit code and high count never moved | chasing a gate flap that does not happen, instead of the tally defect that does |

Four of the five cost **one command**. Only the audit-doc-refs case needed real work (57
invocations), and even there the expensive part bought a *forcing recipe*, which the file itself
named as the precondition for any falsifiable fix.

**Confirming data points:**

1. W-11 / F-11 (this log) — a bug file's `## Fix` prescribed a change the code already had.
2. W-13 (this entry) — the same disease one level up, in the premise rather than the prescription.
3. F-2 (this log) — the bug ledger was never queried at the seam, so a filed fix was reimplemented.

**Impact:** high — five bugs, five avoided wrong turns, and three of the five ended with a *better*
understanding than the original question: the MCP fix reduced to a threshold, the kotlin bug lost
its severity justification, and the researcher bug moved from tuning to model selection.

**Promote-when:** criterion met, and DISCHARGED 2026-08-07 (see Status). This belonged in the bug-file template itself — a
`## Premise check` line, or a note under `## Root cause` that a mechanism recorded without a
measurement is a hypothesis. The complement is already there in spirit ("Root cause — if unknown,
write `Unknown — see Hypotheses tried`"); what is missing is that a **stated** root cause also
decays, and nothing marks when it was last checked.

**Status:** promoted-to-permanent-docs — landed 2026-08-07 in `docs/issues/_TEMPLATE.md` § Root cause: every stated mechanism now also cites what **measured** it (command + date), and a mechanism read out of the code but never observed at runtime says so explicitly (`inferred from src/x.rs:12 — not measured`). One line mirrored into memory `conventions` § Bug Tracking so it is in context at session start rather than only at template-copy time. This is option (b) of task #31 — cite the command rather than add a `## Premise check` section — chosen because the template already demands `path:line` on every root-cause claim, so it extends an existing habit instead of adding a section that gets left as `N/A`.

## F-14 — Memory `conventions` still gated bug archiving on `master`, three surfaces after the rule changed

**Observed:** 2026-08-07, reading memory `conventions` as prerequisite context for labelling the tuning constants (task #30) — not looking for drift.

**When:** Session-start memory load, before any edit.

**Expected:** Memory `conventions` § Bug Tracking agrees with the project's other three statements of the same rule.

**Got:** *"Archive to `docs/issues/archive/` only after the fix ships to `master` (verify via `git branch --contains`)"* — the superseded rule. `CLAUDE.md` § Bug Tracking, the comment block in `docs/issues/_TEMPLATE.md`, and `docs/RELEASE.md` step 4 all say the opposite: archive once the fix is **verified on `experiments`**; reaching `master` is explicitly NOT required, because `experiments` is never deleted. The memory also omitted both safety clauses the other surfaces carry — archive via `artifact(action="move")` rather than `git mv` (`id = sha256(abs_path)`), and the experiments-SHA-orphans-on-rebase caveat.

**Probable cause:** The rule moved and three of four surfaces were updated. Memory is the surface with no lint: `audit_doc_refs` checks path / link / line refs, not semantic agreement between two prose statements of one rule, and `.codescout/memories/` is not in its corpus at all. Same fix-then-forget root cause as the verify-open cadence and the bug-file-archive discipline — `CLAUDE.md` names three bookkeeping surfaces that leak this way, and memory is a fourth it does not name.

**Severity:** med — the stale copy is strictly more conservative, so following it holds archives back rather than archiving falsely. What makes it more than cosmetic is *precedence*: the memory is loaded at session start and the correct rule is not, so it wins by default, and the pile of `fixed`-but-unarchived bugs the current rule exists to prevent is exactly what it reinstates.

**Status:** fixed-verified — memory `conventions` § Bug Tracking rewritten this session to match the other three, both safety clauses included, plus the new W-13 root-cause measurement line.

**Fix idea / Pointer:** The durable fix is a *pointer*, not a fourth copy — memory should cite `get_guide("tracker-conventions")` for archiving rather than restate it, so there is nothing left to drift. Deferred as its own pass: the same argument applies to several other sections of that memory, and collapsing them is a judgement call about how much a session-start surface should carry inline.
## F-15 — A concurrent session changed the checked-out branch mid-task, and `git push` reported success

**Observed:** 2026-08-07, immediately after committing `8a631282` on `experiments`.

**When:** Between `git commit` -- which printed `[experiments 8a631282]` -- and the very next command, `git push`.

**Expected:** `git push` delivers `experiments` to `origin/experiments`.

**Got:** `Everything up-to-date`, **exit 0**. The same command's `git status --short --branch` printed `## feat/pi-secret-guard`, with an untracked `models/` that had not been there a minute earlier. `git reflog -1`: `09170aeb HEAD@{0}: checkout: moving from experiments to feat/pi-secret-guard`. This session ran no checkout. The commit was intact (`git branch --contains 8a631282` -> `experiments`) but undelivered -- and the push had genuinely *succeeded*, against a branch that was already in sync.

**Probable cause:** Another of the machine's three Claude Code profiles is working in the same checkout (`/home/marius/work/claude/codescout`) and switched branches. `docs/RELEASE.md` § Concurrent-Work Rules covered a concurrent session moving HEAD *along* a branch (reset / rebase / amend) but not one changing *which* branch is checked out. A bare `git push` resolves its target at execution time from the current branch, so this failure is silent by construction.

**Severity:** high -- the failure mode is a **silent no-op that looks like success**, on the one operation whose entire purpose is durability. Had the turn ended at the push, the work would have been reported as pushed while sitting local-only, and the next session's `456 ahead / 0 behind` verification would have read as agreement rather than as a discrepancy. The obvious recovery is also destructive: `git checkout experiments` to retry would have yanked the working tree out from under the other session mid-task.

**Second instance, same turn -- the more dangerous half:** after the refspec push, an `edit_markdown` on `docs/RELEASE.md` succeeded against the FOREIGN branch's working tree and returned `ok`. Its anchor text (`**Before any git rebase...**`) exists on both branches, so nothing failed and nothing warned. It was caught only when the *next* edit's anchor -- `## F-14`, committed on `experiments` minutes earlier -- came back "heading not found" from a file that demonstrably contained it on `experiments`. Reverted after reading the full diff and confirming every line was this session's. The narrower lesson: once a foreign checkout is detected, every subsequent working-tree write is suspect **including the ones that report success**, and an anchor present on both branches will never reveal it. Landed content has to be verified from the object store (`git show <sha>:<path>`), not from the tree.

**Third exposure, avoided:** the follow-up commit could not use `git add -A` -- the shared tree carried 57 uncommitted insertions in `docs/trackers/pr-review-session-log.md` belonging to the other session. Explicit paths only, while any concurrent work is uncommitted in the same tree.

**Workaround (applied):** `git push origin experiments:experiments`. A refspec push needs no checkout, cannot target the wrong branch, and leaves the shared working tree exactly where the other session left it. Branch refs are shared per-repo rather than per-worktree, so the commit itself was never at risk -- only its delivery.

**Status:** fixed-verified -- `docs/RELEASE.md` § Concurrent-Work Rules gained five rules covering all three exposures (name the refspec; do not check out to recover; treat post-detection writes as suspect and verify from the object store; explicit paths not `-A`; worktree isolation with its `merge_worktree` cost stated). This entry itself had to wait for the shared checkout to return to `experiments` -- filed as task #32 and landed once it did -- which is the friction demonstrating its own severity.

**Fix idea / Pointer:** The durable form is *"on a shared checkout, always name the refspec"* -- cheap, and correct even when nothing is concurrent, which is what makes it a habit rather than a special case. A mechanical backstop is available if this recurs: a companion-plugin PreToolUse guard that rejects a bare `git push` when `git rev-parse --abbrev-ref HEAD` disagrees with the branch named in the session's most recent commit, and that warns on any working-tree write while the current branch differs from the one the session has been committing to. All the data those guards need is local and free.
## F-16 — Backticks in a `git commit -m` body ran as command substitution and silently deleted a word

**Observed:** 2026-08-07, committing `da5176d5`.

**When:** Sixth commit of the session. Four earlier ones used `git commit -F <file>`; two used an inline double-quoted `-m`.

**Expected:** the body reads *"per memory `conventions` § Environment-Agnostic Tuning."*

**Got:** *"per memory  § Environment-Agnostic Tuning."* — note the double space — and on stderr, `sh: line 1: conventions: command not found`. The backticks were live command substitution inside the double-quoted argument, so the shell ran `conventions`, captured its empty output, and interpolated that.

**Probable cause:** `git commit -m "...`x`..."` is a double-quoted shell string, where backtick substitution is active. This project's commit style uses backticks constantly — branch names, tool names, memory topics, paths — so every inline `-m` message written here is exposed by default. Nothing in the workflow warns, because nothing fails.

**Severity:** high, on the failure MODE rather than on this instance's damage. Here it cost one word. But the construct **executes** whatever sits between the backticks, in the repo working directory, with the caller's privileges — a message quoting a command for documentation would run it, and the only trace left behind would be an unexplained gap in the commit body. It is silent in both directions: `git commit` exits 0, the hook chain runs, the push goes out, and a later reader cannot tell that anything was removed.

**Workaround (applied, and already the session's majority practice):** write the message to a file and use `git commit -F <path>`. The body is then never shell-interpreted. Four of this session's six commits already did that; the two that did not are the exposed ones, and one of them contained backticks.

**Deliberately NOT fixed by amending:** correcting `da5176d5` needs a force-push to a shared branch that another session has uncommitted work in — precisely the destructive op `docs/RELEASE.md` § Concurrent-Work Rules gained rules about earlier the same day (F-15). One missing word in a commit body does not justify it. The gap stands, documented here instead.

**Status:** mitigated — the rule is stated in memory `conventions` § Commit Style and was already the majority practice; the corrupted message itself stays.

**Fix idea / Pointer:** `-F` is now the documented form. A mechanical backstop is available if this recurs: a companion-plugin PreToolUse guard rejecting `git commit -m` whose argument contains a backtick — all the data that guard needs is in the command string, and unlike most such guards it has no false-positive class, since a backtick inside an inline `-m` is never what the author meant.
## W-14 — The first measurement after idle is a warm-up artifact; three of them bit in one session

**Observed:** 2026-08-07, three independent times, on three unrelated subsystems.

**Pattern:** Never record the first observation after an idle period as a steady-state value. Take a
second one and report that, and when writing a benchmark, discard the first iteration explicitly and
**say in the writeup that you did**. The trap is not that warm-up exists — everyone knows that — it
is that a cold number is indistinguishable from a legitimate steady-state number by inspection. It
arrives with units, it is internally consistent, and nothing marks it as anomalous.

**The three instances, and what each would have cost:**

| subsystem | cold reading | warm reading | what the cold number would have caused |
|---|---|---|---|
| SPLADE sparse query embed | 146.8 ms | **16.8 ms** | "sparse fusion costs ~147 ms/query" — overstated **8.7x**, and it feeds the central comparison in the reranker bug (task #20), where the whole question is sparse-vs-reranker latency |
| `audit_doc_refs` resolved/broken tally | migrates by up to 69 refs | byte-identical warm | 57 paired runs chasing "non-determinism" that is a cold-LSP artifact, and a `degraded` flag that reports `false` throughout |
| `resolve_file_symbol` verdicts | `SymbolMissing` | `Resolved` | a doc-ref audit reporting symbols as MISSING because the server answered before it finished indexing — the branch comment asserts "the LSP responded, so an empty symbol list means the symbol genuinely isn't there" |

**Confirming data points:**

1. The sparse number, caught only because 147 ms for an eight-word query looked implausible for a
   small model — i.e. caught by suspicion, not by process. That is the argument for making it a
   rule.
2. The audit tally, which cost 57 paired invocations before the mechanism was found in code.
3. The `SymbolMissing` verdict, which is the same phenomenon promoted into a *correctness* bug
   rather than a latency one — the strongest form of the lesson, because there the cold reading is
   not merely inaccurate, it is a false claim about the codebase.

**Impact:** high — instance 1 was one command away from being written into a bug file as the basis
for a shipping decision, and instances 2 and 3 are already-filed defects whose root cause is this
exact discrimination failure.

**Promote-when:** criterion arguably already met at three same-session datapoints, but they are
narrow — all three are "a service or index that warms up". Promote on the first instance OUTSIDE
that shape (a cache, a JIT, a connection pool, a filesystem page cache). At that point it belongs in
memory `test-design-discipline` alongside the existing cap/truncation corollary, as: *a measurement
taken once, on a cold path, is a warm-up artifact until a second one agrees — and a benchmark that
reports a mean over a run that started cold has folded a one-off into every per-item number.*

**Status:** validated — three datapoints, one of them caught before it was recorded, two of them
already costing filed-bug time.
## F-17 — A `--force` reindex ran for seven minutes against the wrong backend, announcing nothing

**Observed:** 2026-08-07, starting the ~2 h `index --force` rebuild (task #21).

**When:** Immediately after building with `cargo build --release --bin codescout`, which is the
command CLAUDE.md explicitly says NOT to use for this.

**Expected:** a hybrid rebuild — dense + sparse vectors, written to the Qdrant `code_chunks`
collection.

**Got:** a dense-only run writing to sqlite-vec. Caught by comparing container traffic rather than
by anything the tool said: the dense embedder logged 73,232 lines in two minutes while the sparse
embedder logged **zero**, and `ss -tnp` showed the process holding exactly one connection — to
48081 (dense). No connection to 48084 (sparse), and none to 6334 (Qdrant).

**Root cause:** `cargo build --release` omits the `server-stack` feature
(`default = ["remote-embed", "http", "librarian"]`), so `VectorBackend::resolve()`
(`src/retrieval/code_store.rs`) falls through to its `#[cfg(not(feature = "server-stack"))]` arm and
returns `SqliteVec`. That sets `lite = true` in `RetrievalClient::from_env`
(`src/retrieval/client.rs`), and `dense_only = lite || config.disable_sparse` — so the sparse leg is
skipped *and* the store is `.codescout/embeddings/project.db` rather than Qdrant. `cargo rb` is
aliased to `build --release --features server-stack` precisely for this, and `.cargo/config.toml`
says so in a six-line comment.

**Measured 2026-08-07, both directions**, which is what makes this a mechanism rather than a guess:
aborted run held one connection (48081) with zero sparse traffic; the rebuilt run holds 48081 +
48084 + three to 6334, and the sparse container logged 61 lines in the first 60 s.

**Severity:** high, and specifically because of the **silence-times-duration** shape. Two hours of
work would have produced nothing usable, and every signal available said it was fine: exit code
pending, no errors, no warnings, the dense embedder visibly busy, and a first log line reporting
`chunk_target=1200 flush_batch=256 force_reindex=true` — three fields, none of them the two that
were wrong. Not data loss: because the lean build never targets Qdrant, the existing 92.3% sparse
coverage was never at risk, and a 500-point sample after the abort read 92.2%. (I called it
destructive when aborting; that was wrong, and the abort was still correct on time alone.)

**Two failures, and only one of them is mine to stop repeating.** I ignored a documented rule —
CLAUDE.md states the release build is `cargo rb`, not `cargo build --release`. But a rule that is
only in a doc has to be remembered every time, and this one is remembered under time pressure at the
start of a two-hour job. The deeper defect is that **the tool did not report its own load-bearing
configuration**, so violating the rule was undetectable from the tool's output.

**Status:** fixed-verified — `sync_project`'s start line now logs `backend` and `sparse`, so line one
of the corrected run reads `backend="qdrant" sparse="on"` and the aborted configuration would have
read `backend="sqlite-vec" sparse="SKIPPED"`. Diagnosis time next occurrence: one line, versus the
seven minutes plus a code read it took here.

**Fix idea / Pointer:** The general rule earned here — **a long-running job must log its
load-bearing configuration in line one, because the cost of a wrong config scales with runtime.**
`chunk_target` and `flush_batch` were logged and are tuning trivia; backend and sparse-mode decide
whether the run means anything, and were not. Worth applying to the other long jobs (the benchmark
harness, `audit_doc_refs` on a full corpus). A mechanical backstop is also available if this
recurs: have `index` refuse to run, or warn loudly, when the resolved backend is sqlite-vec while
`CODESCOUT_QDRANT_URL` is reachable — that combination is almost always a lean-build accident
rather than intent.
## F-18 — A routine `/mcp` killed a running rebuild, because the job was a grandchild of the MCP server

**Observed:** 2026-08-07, ~4 minutes into the corrected `index --force` rebuild.

**When:** The user ran `/mcp` to reconnect after a binary rebuild — a one-keystroke, entirely routine action.

**Expected:** the reconnect swaps the MCP server; the long-running rebuild is unaffected.

**Got:** the rebuild was **killed**. It had been launched via `run_command`, so its process tree was `MCP server -> sh -c -> codescout index`, and restarting the server took the whole tree with it. Points had reached 578,344 (a couple of batches flushed) out of a run that needed ~8 minutes. Also lost in the same instant: both `@bg_*` buffers holding the run's output and the watcher's, because those are server-side state.

**Probable cause:** anything started through a tool call inherits the tool server's lifetime. Nothing about `/mcp` warns that it is destructive to in-flight work, and nothing about `run_command` says its children die with the session — the coupling is invisible from both ends.

**Severity:** high on expected cost rather than on this instance. Here it cost 4 minutes and a relaunch. The same coupling applied to the 2-hour figure this rebuild was believed to need would have cost two hours, and it fires on an action people take without thinking — after every `cargo rb`, which is exactly when a long job is most likely to be running.

**This is the second time in one session that server-side state died with a reconnect**, and the first was the argument I had just used in a decision: § F-16's neighbour, the #16 close, records that `@cmd_*`/`@tool_*`/`@bg_*` buffers are server-side and that an idle-shutdown would dangle every `@ref` in the conversation. That reasoning was correct and I then failed to apply it forward to my own background job ten minutes later. Knowing a coupling exists is not the same as designing around it.

**Workaround (applied):** relaunch under `setsid nohup ... >> logfile 2>&1 < /dev/null &`, which puts the job in its own session and process group and writes to a **file** rather than a server-side buffer. Verified: the relaunched job's parent became a non-server pid and it survived the wrapper's exit and subsequent tool calls.

**Status:** mitigated — the pattern is stated below and was applied; nothing enforces it.

**Fix idea / Pointer:** The rule: **anything expected to outlive a single tool call must be detached (`setsid`) and must log to a file, never to an `@bg_*` buffer.** Two candidate backstops, neither built: `run_command`'s background mode could `setsid` by default (it already writes to a log file, so it is half-way there), or the companion plugin could warn when `/mcp` is invoked while a detached-looking long job is running. The first is strictly better — it removes the coupling rather than warning about it.

## F-19 — Three process-check commands matched their own argv; one killed the command running it

**Observed:** 2026-08-07, three times within an hour, while monitoring the rebuild.

**When:** Checking whether an indexer was running, then watching for it to finish, then cleaning up the watcher.

**Expected:** each command reports on the *index* process.

**Got, in order:**

1. `pgrep -af 'codescout index'` — matched the `sh -c` running it, because the pattern was in that shell's own command line. Reported a running indexer when there was none, and the false "clear to rebuild" answer it produced is precisely the precondition `docs/issues/2026-07-25-concurrent-index-no-project-lock` says to verify.
2. `pgrep -f 'codescout index --force'` inside a poll loop — matched the loop's own shell, so the loop's exit condition could never fire. It ran to its 170-iteration cap instead of exiting when the job finished, which is why the completion had to be noticed by hand.
3. `pkill -f 'INDEX FINISHED after'` — matched the command containing that string, i.e. itself, and **killed the run_command mid-execution** (`exit_code: -1`).

**Probable cause:** `pgrep`/`pkill` `-f` match against full command lines, and a checker's own argv contains the pattern by construction. The failure is silent in both directions — a false positive reads as "still running", a self-kill reads as a crash.

**Severity:** med. Instance 1 produced a wrong answer to a safety precondition, instance 2 broke an automation silently, instance 3 destroyed the command issuing it. None corrupted data, but instance 1 is the shape that matters: a process-liveness check that answers wrongly is worse than one that errors.

**Workaround (applied):** `ps -o pid=,args= -C <binary>` and filter afterward. `-C` matches on the executable NAME, so a shell or python checker is structurally excluded — the checker cannot match itself because it is not named `codescout`. That form was used correctly earlier in the same session, which is what makes the three failures avoidable rather than novel.

**Status:** mitigated — rule stated; nothing enforces it.

**Fix idea / Pointer:** Prefer `ps -C <name>` over `pgrep -f <pattern>` for liveness checks, and never `pkill -f` a pattern that appears in the command being typed. If a pattern match is unavoidable, exclude self explicitly (`pgrep -f pat | grep -v "^$$\$"`). Cheap enough to be a habit rather than a lookup.
## F-20 — Archiving a bug file broke the manual's citation and failed CI on the promotion tip

**Observed:** 2026-08-08, checking CI on `f244ad17` — the commit deliberately made last so
the final tip would get one clean run.

**When:** After round 9 closed. Three bug files had been archived the previous day via
`artifact(action="move")`, correctly and through the catalog.

**Expected:** Green. The archive moves were done the documented way, and `docs/RELEASE.md`'s
documentation gate lists no citation step.

**Got:** `Audit Doc Refs` **failed**, exit 1, one high finding:
`docs/manual/src/concepts/retrieval-stack.md:275` citing
`docs/issues/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md` — the pre-archive
path. Every other job passed; the four `cancelled` runs above it were the concurrency block
working, not failures.

**Probable cause:** Archiving is a bug file's **normal end state**, so every citation of
`docs/issues/<slug>.md` in a durable surface is a scheduled break — and the move is the moment
it fires. Nothing in the archive flow said to re-point them. Three moves produced 25 dangling
refs across 15 files.

**Why only one was reported** — and why fixing all 25 would have been wrong: the audit's
severity policy keys on the **citing** document (`severity.rs:145`), dropping one band for
`archive/` and one for `docs/issues/`. So 24 siblings sat at `med` **by design**, because
`archive_drop` exists precisely so a retired record citing a moved path does not gate.
Rewriting an archived snapshot to satisfy a linter that already ignores it would falsify the
record. Reading the policy instead of only the verdict is what produced the right scope: 12
live surfaces fixed, 13 historical ones left. (Recorded as **R-66** in
`docs/trackers/reconnaissance-patterns.md`.)

**Workaround → fix:** `e20b3a04` re-points the manual page, `CHANGELOG.md` (2),
`docs/trackers/retrieval-benchmark.md`, the audit design spec, `.env.example` (2), `.env.gpu`,
and eight doc comments across four source files. Prevention landed in the same commit:
`get_guide("tracker-conventions")` § Bug files now carries a re-point-the-citations step in
the archive flow — with the grep, the instruction to leave archived snapshots alone, and the
warning that a green CI run is not the check for this (the audit cannot see `CHANGELOG.md`
at all).

**Severity:** high — it failed the gate on the exact commit handed over as merge-ready, and
the handover said "check the tip rather than trusting this line", which is the only reason it
was caught rather than merged red.

**Status:** fixed-verified — CI run `31219510777` on `e20b3a04` green across all 15 jobs;
`audit-doc-refs --fail-on high` exits 0 locally, top finding `med`.

**Fix idea / Pointer:** The durable one is already shipped (the guide step). The stronger,
unbuilt version: teach `resolve_file_path` to try `<dir>/archive/<basename>` before reporting
`Missing`, which would make the whole class self-healing — noted here rather than filed,
because it trades a true-positive signal (the citation *is* stale) for silence.
## F-21 — A test asserted the guarantee in the one spelling that made the bug invisible

**Observed:** 2026-08-08, reviewing PR #10 after its author had already fixed the CI-visible
half of the review findings.

**When:** Reading `resolve_git_bash` (`src/platform/windows.rs`), which picks the shell
`run_command` executes through on Windows.

**Expected:** The `%SystemRoot%\System32` exclusion — the load-bearing step of the whole
resolution order, because that directory's `bash.exe` is the WSL launcher — was documented,
implemented, and covered by a test literally named
`resolve_git_bash_never_selects_the_wsl_launcher`.

**Got:** The exclusion never fired. `if dir == system32` compares `PathBuf`s, and `Path`
equality folds case only on the drive-letter prefix — normal components compare byte-exactly.
Windows setup writes the system PATH entry as `C:\Windows\system32`, while
`%SystemRoot%`.join("System32") produces `C:\Windows\System32`. The test injected
`PATH=C:\Windows\System32`, the **constructed** spelling, so the comparison matched, the skip
fired, and the test passed against a guard that does nothing on a real host.

**Probable cause:** The fixture was written from the code rather than from the environment.
Both sides of the comparison were spelled the way the code spells them — precisely the input
that cannot discriminate.

**Impact:** On a WSL host with Git installed outside the three probed roots, every
`run_command` would run in the WSL namespace: different `$HOME`, different toolchain, project
reachable only via `/mnt/c`. As the function's own doc says, "it would not fail loudly."

**Severity:** high — silent wrong-shell execution, wearing a passing test named after the
guarantee it did not provide. Strictly worse than no test: a reviewer greps for WSL coverage,
finds it, and stops.

**Status:** fixed-verified — `platform::windows_dir_eq` (folds ASCII case, either separator,
one trailing separator; same-directory, never a prefix test). The test now sweeps four
spellings including the lowercase one, with `|_| true` as the probe so only the exclusion can
reject, plus `resolve_git_bash_still_accepts_a_non_system32_path_entry` as the positive
control — without it, "never selects the wrong shell" and "never selects a shell" are the same
assertion. Shipped `6f261da9`, merged `9be2ede4`.

**Fix idea / Pointer:** Generalised as R-67. Placement mattered as much as the fix:
`windows_dir_eq` lives in `platform/mod.rs`, not `windows.rs`, because that module is
`#[cfg(windows)]` and anything asserted there is verified on one CI leg of four — which is how
the original passed. Clippy then rejected `pub(crate)` as dead on Linux; `pub` matches the
file's siblings (`posix_tokenize`, `shell_path_str`) for the same reason.

## F-22 — A rebase orphaned twelve SHA citations, and the merge method then became load-bearing

**Observed:** 2026-08-08, while repairing a stale `## Resume` in one of PR #10's bug files.

**When:** The branch had been rebased onto `f244ad17` between 00:13 and 00:21 that night —
committer dates 00:13-00:21 against author dates 12:43-20:27 is the signature.

**Expected:** The bug files' `## Fix` sections cite their fix commits, so the audit trail
resolves.

**Got:** `git cat-file -t` resolved **none** of `b5c8bbb0`, `94a63c32`, `d564c9bb`,
`20d12b5f`. A sweep of every 8-hex token across the PR's docs found **14 dead** — 5
PR-introduced, 9 pre-existing (WIN-2..WIN-17, dated 2026-06-09).

**Probable cause:** Exactly the hazard `docs/issues/_TEMPLATE.md` documents — *"an
`experiments` SHA orphans on rebase, and nothing re-reads `archive/` to repair it"* —
realized. Nothing gates it: `audit_doc_refs` checks paths, symbols, anchors and line numbers,
not commit SHAs.

**Consequence for the merge:** `--rebase` or `--squash` on PR #10 would have re-minted all 18
SHAs and orphaned the citations a second time, an hour after repairing them. `gh pr merge
--merge` preserved them — `6f261da9` and `d8eddb7b` are still reachable from `9be2ede4`.

**Severity:** med — no code defect, but the entire audit trail of four bug files pointed at
nothing, and the repair window was one merge-method flag wide.

**Status:** mitigated — the 5 PR-introduced SHAs remapped by commit subject to `a8253b62`,
`e4b86447`, `b142a514`, `bc94e67f` in `6f261da9`, and the artifact-move `## Resume` now states
they are pre-merge SHAs that a further rebase re-orphans. The 9 pre-existing ones are untouched
and still dead.

**Fix idea / Pointer:** The sweep is cheap and mechanical — extract 8-hex tokens, `git
cat-file -t` each, report the misses. Candidate for the ship sequence, or for `audit_doc_refs`
as a `ref_kind: commit_sha`. Until then the rule is: **once a commit's SHA is cited in a
durable doc, the merge method stops being cosmetic.**

## F-23 — Predicted a PR's CI would fail on an inherited break; `pull_request` tests the merge ref, not the tip

**Observed:** 2026-08-08, checking PR #10 before reviewing it.

**When:** The branch's merge-base was `f244ad17` — the commit whose `Audit Doc Refs` failure
had been fixed on `experiments` in `e20b3a04`, which the branch did not contain.

**Expected (my claim, stated to the user):** the PR's CI would fail `Audit Doc Refs` with the
same single high finding, since `retrieval-stack.md:275` on the branch still carried the
pre-archive citation — which I had verified against the branch tree.

**Got:** `Audit Doc Refs` **passed**. `actions/checkout` on a `pull_request` event checks out
`refs/pull/N/merge`: a synthetic "Merge \<head\> into \<base\>" commit existing on no branch.
Fetching it settled it — `a42f124b`, contains `e20b3a04`, and line 275 reads the corrected
`archive/` path.

**Probable cause:** This session's own rule — *"the gate is a green run on whatever the tip is
at that moment"* — is correct for **push** events, where `GITHUB_SHA` is the pushed commit.
For **pull_request** events the tested SHA is the merge result, regenerated whenever either
side moves. A push-event rule was applied to a PR run.

**Severity:** med — wrong in the user-facing direction (it argued for an unnecessary rebase),
and caught only by fetching the merge ref instead of reasoning from the branch tree.

**Status:** fixed-verified — corrected in the same session, before anything was done about it.

**Fix idea / Pointer:** Two events, two questions. A push run says *this commit is good*; a PR
run says *the result of merging is good*. When a branch is behind its base, the PR run is the
more useful of the two and its greenness already accounts for the gap.

## W-15 — A four-lens adversarial fan-out found five blockers; three landed twice independently

**Observed:** 2026-08-08, reviewing PR #10 (19 files, +1468/-123, zero prior reviews) at the
user's explicit request for a multi-agent review.

**Pattern:** Split by **lens, not by file** — platform/security, librarian correctness, test
rigor under a mutation frame, docs/conventions — and brief each with three things: (a) the
refs, because the working tree was the BASE and not the PR, so `symbols`/`grep` would have
reviewed pre-PR bytes; (b) what the controller already established, explicitly marked
challengeable; (c) a mandate to try to REFUTE each finding before reporting it, and to list
what was dropped.

**Counterfactual:** The two highest-severity findings are not ones a single-pass read
produces. `summary.total` no longer partitioning `by_check` requires holding three line
numbers (200, 217, 272) at once; the untested `'` fixture requires asking *what mutation
survives this suite?* rather than *is there a test?*. Both landed **twice, independently** —
as did `posix_tokenize` having no production callers. That corroboration is what made them
actionable without re-deriving each from scratch.

**Confirming data points:**
1. Three findings duplicated across independent reviewers (`summary.total`, dead
   `posix_tokenize`, the job-assignment race).
2. The refutation passes killed real candidates — including one of the controller's own seeded
   premises (the `temp_path_strings` form mismatch: `shell_words` strips quotes, so the match
   survives) and "the doctor count is computed from the truncated rows" (false — only `total`
   was).
3. The docs reviewer corrected two premises in its own brief: 3 promoted memory rules not 2,
   and rows WIN-32..35 not WIN-30/31.

**Impact:** high — five blockers on a PR that had no other reviewer, two of them regressions of
fixes made two days earlier.

**Promote-when:** A second fan-out of this shape yields ≥1 finding no single lens would have.
At two datapoints, promote the briefing template — refs + established-facts-marked-challengeable
+ refute-first — to a reusable skill or a CLAUDE.md note.

**Status:** validated — single datapoint; all five blockers fixed and merged in `9be2ede4`.

## W-16 — Cross-compiling the tests before pushing found nothing, and that is the point

**Observed:** 2026-08-08, immediately before pushing `6f261da9` to PR #10.

**Pattern:** Two of the six fixes were `#[cfg(windows)]` tests. `cargo test` on Linux does not
compile them at all, so a type error would have made its first appearance as a red CI leg
~6 minutes out. `cargo check --target x86_64-pc-windows-gnu --tests` compiles them locally in
~6 s with deps warm, because `rust-toolchain.toml` already pins that target (W-9's fix).

**Counterfactual:** One typo in either test = one red CI run, one fixup commit or force-push,
and a review cycle spent on a mechanical error on someone else's PR. It returned green, which
is what makes it worth keeping: **a verification step is not paid for only by the times it
fires.**

**Confirming data points:**
1. This session — nothing found, one command, ~6 s.
2. W-9 is the same shape one layer out: running CI's own toolchain locally during a provider
   outage caught a red-CI-in-waiting.

**Impact:** med — bounded, but it closes the one gap where the local gate is structurally blind
(F-5's third skew: code the host platform never compiles).

**Promote-when:** A cross-target check catches a real error. Then promote to CLAUDE.md's
pre-commit gate as a conditional step — *if the diff touches `#[cfg(windows)]` code, also run
`cargo check --target x86_64-pc-windows-gnu --tests`.*

**Status:** validated — awaiting a second datapoint.

## F-24 — A bug file passed the premise check on "is the claim true?" and skipped "is the claim's subject the mechanism?"

**Observed:** 2026-08-08, reviewing PR #11 (`7f1b6ddc`), a bug file filed by a concurrent session against `6f261da9` — our own commit.

**Expected:** the premise-check convention added in this work stream (task #31, now in `docs/issues/_TEMPLATE.md`) exists to stop a bug file from resting on an unverified claim. The file honoured it well: it measured `git status --porcelain .codescout/projects` → 8 untracked dirs, quoted a stub verbatim, and correctly identified that our `.gitignore` comment's "has no untracked content at all" was false on a developer host.

**Got:** the claim was true and its subject was wrong. The file attributed the 8 dirs to *foreign-workspace registration* — "the normal state for the cross-project flows CLAUDE.md documents" — and built three fix options (A/B/C) on that attribution. Sixteen lines of `Workspace::memory_dir_for_project` say the path is derived from `self.root`, so a foreign workspace cannot write under another repo's root. A sweep of all 15 registered roots found **nine** repos with a `.codescout/projects/` tree and **zero** untracked entries in any of them — those flows run here and produce nothing. The real cause was an unvalidated `project_id`, one layer up.

**Probable cause:** the convention asks for one check ("is the claim measured?") and there are two. A measured symptom plus an unexamined attribution reads exactly like a well-evidenced bug report, because every number in it is real.

**Severity:** high — all three proposed fixes encoded the mis-attribution, and two of them were actively harmful: Option B re-enumerated project ids, reintroducing what `6f261da9` had removed. A session following the file would have written a `.gitignore` glob and closed the ticket, leaving the actual defect shipping.

**Status:** fixed-verified — re-diagnosed in `feb4c4e4`, measured in `fccdd95b`, root cause fixed in `c0bdeec7`, all on `experiments`.

**Fix idea / Pointer:** the template's premise line should ask for two things, not one — what measured the *symptom*, and what read the *mechanism*. `docs/issues/archive/2026-08-08-gitignore-projects-rule-premise-false-on-a-real-host.md` carries the full re-diagnosis.

## F-25 — Recommended a fix as "strictly better", then falsified it with a six-line experiment after the user had already approved it

**Observed:** 2026-08-08, reviewing PR #11 and then executing the resulting task #43.

**Expected:** having caught F-24's mis-attribution by reading the code, the replacement recommendation would be held to the same standard.

**Got:** two unmeasured claims in one round, both mine.

1. **The diagnosis.** I proposed that the VDI's 8 entries came from workspace-root misresolution — sibling repos enumerated as sub-projects — and wrote it into the bug file's addendum as the mechanism. Two `memory()` calls later it was plainly a bad `project_id` on one host, no misresolution involved. Recorded as *hypothesis 4, withdrawn* rather than deleted.
2. **The fix.** I recommended the structural `.gitignore` pattern running in `~/personal` as "strictly better than Options A/B/C", told the user to apply it, and they approved it as task #43. Executing it, a throwaway repo with a real fixture project and a phantom side by side gave: `git check-ignore` reports the phantom's `memories/zz-probe.md` as **NOT ignored** — identically to a real project's memory file. A phantom's `memories/` is structurally identical to a real one's, so no glob can hide one and keep the other. The pattern's only real effect is ignoring non-`memories` content under `projects/<id>/`, and this repo has none.

**Probable cause:** the pattern was *observed in production* in a sibling repo, which felt like evidence. It was evidence that the pattern works — for what that repo needed. Nothing checked it against the symptom here, and the two differ in exactly the way that matters.

**Severity:** med — caught before landing, and the correction (`927d75c0`) records the experiment so it is not re-tried. Had it landed it would have been worse than a no-op: a rule that reads as though the litter were handled.

**Status:** fixed-verified — task #43 narrowed to the comment correction alone; `.gitignore` now carries the check-ignore result and a do-not-retry note.

**Fix idea / Pointer:** "it works in another repo of ours" is a hypothesis about *this* repo. For a `.gitignore` rule the test is six lines and thirty seconds: throwaway repo, both populations, `git check-ignore -v`.

## F-26 — A fixture declared a sub-project it never created, and the bug under test made the assertion pass anyway

**Observed:** 2026-08-08, running `cargo test --lib memory::` after adding validation for unknown `project_id` (`c0bdeec7`).

**Expected:** a green suite, or a failure in the new tests.

**Got:** `memory_write_accepts_project_alias_for_project_id` failed — `No project 'mcp-server'. — hint: Valid project ids: .tmpXh0H7C`. Its fixture wrote `mcp-server/package.json` as `{}`, which is too empty for discovery to register a project, so the `mcp-server` its own comment claimed to create ("Multi-project: root gradle project + mcp-server sub-project") never existed. Its assertion — that the write lands in `.codescout/projects/mcp-server/memories/` — was satisfied **entirely by the defect**, because an unknown id produced exactly that path.

**Probable cause:** the assertion cannot distinguish the two worlds. Under the bug, a real sub-project and an invented one route to the same path, so the test passed either way and never demonstrated what its name claims. The sibling `memory_write_routes_to_project_dir` has the same blind spot; it still passes because its manifest happens to be substantive.

**Severity:** med — the test was not protecting the routing behaviour it is named for. No production impact, but a regression in discovery would not have been caught here.

**Status:** fixed-verified — manifest now matches the sibling fixture (`{"scripts":{"build":"tsc"}}`), so the alias is tested against a project that exists. The new tests' `hint.contains("mcp-server")` assertions are the first thing in the suite that can tell a discovered project from an invented one.

**Fix idea / Pointer:** third instance of F-21's shape in two rounds (F-21, F-24, F-26) — an assertion written in the one form where the defect is invisible. The tell is an assertion whose expected value the *bug* also produces. `src/tools/memory/tests.rs`.

## W-17 — Running the thing beat reasoning about it, four times in one session

**Observed:** 2026-08-08, across PR #9, PR #11, and task #43.

**Pattern:** when a claim is about behaviour and the behaviour is cheap to invoke, invoke it. Four instances, four different tools:

1. **PR #9's secret-guard.** Its own suite was 12/12 green on Node 26. A nine-case probe written against the same extension returned **9 of 9 divergent** — six bypasses (one nine characters long) and three false blocks.
2. **PR #11's mechanism.** Two `memory()` calls settled what an addendum had asserted from reading, and inverted the attribution.
3. **The read path.** A second probe — "does `read` also create the directory?" — found the worse symptom, invisible to `git status`.
4. **The `.gitignore` glob.** `git check-ignore -v` in a throwaway repo falsified a fix I had already recommended and had approved.

**Counterfactual:** without (1) the PR merges with a documented one-token bypass and an `AGENTS.md` asserting "hard gate" into every Pi session. Without (2) the fix lands one layer too low. Without (3) the worse of the two symptoms ships unrecorded. Without (4) a no-op `.gitignore` rule lands reading as though the litter were handled.

**Confirming data points:** W-5 (invoking the tool under test found five defects reading had missed), W-12 (the live-tree call is a distinct verification step), W-13 (every bug premise checked was wrong; four fell to one command each), and now four more in a single session.

**Impact:** high.

**Promote-when:** already at promotion threshold. The rule to promote is narrow enough to be actionable: *before recommending a fix whose effect is observable in under a minute — a glob, a regex, a guard, a CLI flag — observe it.* Candidate home is CLAUDE.md alongside the premise-check convention.

**Status:** validated.

## W-18 — Asking which other callers reach the same line found a worse bug than the one reported

**Observed:** 2026-08-08, probing the unvalidated `project_id`.

**Pattern:** after confirming a defect on the path a report names, enumerate the *other* callers of the same line and probe one. Here the report was about litter from `memory(write)`. `references(from_dir)` showed five call sites — `write`, `read`, two list paths, `delete` — all constructing `MemoryStore::from_dir`, which `create_dir_all`s in its constructor. Probing `read` produced: `available_topics: []` and the hint *"no memory topics exist yet"* for a project that does not exist.

**Counterfactual:** probing only `write` would have shipped a fix that closed the litter and left the misleading read in place — arguably the worse half, since litter is noise you eventually notice while a confident empty answer is acted on immediately, and the read leaves no `??` behind to hint anything went wrong. It also would have left the read/write asymmetry unexplained, and that asymmetry is the entire reason this host showed 2 phantoms and the VDI showed 8. Two hosts would have looked like two configurations instead of two traffic patterns.

**Confirming data points:** R-17 (spot-check sibling callers of a just-fixed shared helper), R-47 (enumerate the delegate's callers too — the report walked one call path). This is the same lesson arriving through a constructor side effect rather than a delegate.

**Impact:** high — changed both the symptom ranking and the causal account.

**Promote-when:** a third instance where a sibling caller of a shared constructor carried a distinct symptom. At that point the rule generalises past helpers to *any* shared construction with a side effect in it.

**Status:** validated.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
