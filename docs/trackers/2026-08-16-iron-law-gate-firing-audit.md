---
id: ac8fbe339e66ade3
kind: tracker
status: active
title: Iron-Law Gate Firing Audit — 2026-08-16 (GF-N)
owners:
- marius
tags:
- investigation
- usage-db
- iron-laws
- run_command
- gate-correctness
- agent-guidance
topic: iron-law-gate-firing
entry_prefix: GF
---

# Iron-Law Gate Firing Audit — 2026-08-16

> **Prefix:** `GF-N` — a finding from this audit. Work-stream-scoped, defined here, not a
> project-wide namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). `IL-N` was
> deliberately NOT used: `IL-1..IL-6` already name the Iron Laws themselves.

## What this is

One empirical pass over **codescout's own** `.codescout/usage.db`, asking a question the
2026-08-15 TU-N investigation did not ask.

**This does not contradict TU-7 — it is orthogonal to it.** TU-7 measured *recovery* and
correctly concluded `il3_pipe_to_trimmer` is a healthy guard. This pass replicates that
result and then asks a second question:

| Axis | Question | TU-7 | This audit |
|---|---|---|---|
| Recovery | Did the agent fix the call? | 85% recovery, 3% immediate repeat | **replicated** — 96% / 3% |
| Learning | Did the agent learn the rule? | not measured | **47% per-session repeat** |
| Firing correctness | *Should the gate have fired?* | not measured | **24% of IL-3 pipe refusals are self-bounded `git`** |

A guard can have an excellent corrective message *and* fire on commands its own rationale
says to allow. Those are independent properties. TU-7 established the first; this establishes
the third.

## Findings index

> Mirrored from the catalog's `findings` collection. The catalog is **machine-local and not in
> git**, so this table is the only form a reader outside this machine can see — keep them in sync
> when a row's status changes (`docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`,
> which bit this very tracker on its first status update).

| ID | Finding | Sev | Status | Target |
|---|---|---|---|---|
| GF-1 | `git` is unconditionally unbounded — 50% of its IL-3 refusals are self-bounded (24% of the family). **Incomplete fix** of `2026-05-18-il3-overtriggers-bounded-lhs`, whose remedy created this very list | high | **fixed** `06a53ad3` | `src/util/path_security.rs` |
| GF-2 | The flag-conditional mechanism already exists for `grep`/`find` — `git` never got it; the fix extends, not invents | high | **fixed** `06a53ad3` | `src/util/path_security.rs` |
| GF-3 | `shell_on_source` refuses 23% out-of-project paths where the suggested `symbols` alternative cannot serve, plus 16% `wc` | med | **fixed** `be4a679b` + `433100bd` | `src/util/path_security.rs` |
| GF-4 | Refusals teach the CALL, not the PREDICATE: 96% immediate compliance, 3% immediate repeat, **47% per-session repeat** | med | **fixed** `ba591f12` | `src/prompts/mod.rs` `refusal_predicate` |
| GF-5 | Guide delivery is success-path-only — **premise was wrong**: true of `call_content`, not of the system. The assembly point in `call_tool_inner` already supports a second block on an error | high | **fixed** `ba591f12` | `src/server.rs` |
| GF-6 | IL-1 is the sick family on the learning axis — 71% per-session repeat, 7.4 per session, worst 33 over 7 hours | med | open | `src/util/path_security.rs` |
| GF-7 | **Negative result:** IL-4/IL-5 repeat 0% across 18 sessions. A total, file-intrinsic predicate teaches on first contact — do not "improve" these; keep as regression control | info | closed | — |
| GF-8 | `untrusted-content` fetched 0 times in 30 days — now **declared** rather than silently missing: `PULL_ONLY_GUIDE_TOPICS` marks it *PENDING BL-25* | med | mitigated | `src/prompts/mod.rs` |

### Reconciliation with BL-25 phase 1 (shipped mid-audit)

`72f39849` + `73ccb495` landed while this audit was being written. `PULL_ONLY_GUIDE_TOPICS`
now names every untriggered topic with a documented reason, and
`every_guide_topic_is_triggered_or_declared_pull_only` fails the build **both** ways —
untriggered-and-unlisted, *and* a stale entry that later gains a trigger. The allowlist names
exactly the four topics this audit measured as untriggered, so the two passes agree on the
inventory and disagree on one rationale:

> `iron-laws-detail` — *"The gate text and its exceptions … A caller who needs the detail has
> already read the pointer."*

This audit measured that caller. **1 fetch in 30 days against 557 Iron-Law violations**, with
the pointer present in *both* pull surfaces. That is A-10's failure mode — on-demand guidance
is obeyed as reliably as always-visible guidance *once fetched*, and its failure is never being
fetched — and GF-5 gives the structural reason it cannot be fetched at the one moment it is
wanted. Recorded as a conflict, not a correction: the allowlist entry shipped **before** this
measurement existed, and the call is the user's.

## Findings — per-entry anchors

> **Added 2026-08-18, headings only.** This audit's augmentation prompt tells readers to "read its body section" for a GF-N row, and told maintainers not to rewrite body sections. Both were right and they could not both be satisfied: no `GF-N` heading existed anywhere in this body, so `link_scan` bound none of the eight tokens and every citation of them resolved to nothing. These anchors add the missing definitions and nothing else — no number is re-run, no existing section is touched. Each entry's evidence stays where the audit put it, named below.
>
> Mechanism: `docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`. A table row defines no token, which is why the index above could be complete and the entries still uncitable.

### GF-1 — `git` is unconditionally unbounded, so half its IL-3 refusals are self-bounded

**Severity:** high · **Status:** fixed `06a53ad3` · **Target:** `src/util/path_security.rs`

50% of `git`'s IL-3 refusals were self-bounded — 24% of the whole family. An **incomplete fix** of `2026-05-18-il3-overtriggers-bounded-lhs`, whose remedy created this very list. Evidence: § *IL-3 `pipe_to_trimmer` — composition*; § *Prior art — this is an incomplete fix, not a new defect*; § *Refactor data pack — IL-3*.

### GF-2 — The flag-conditional mechanism already existed for `grep`/`find`; `git` never got it

**Severity:** high · **Status:** fixed `06a53ad3` (as `git_output_is_bounded`) · **Target:** `src/util/path_security.rs`

The fix extends an existing mechanism rather than inventing one. Evidence: § *Refactor data pack — IL-3*.

### GF-3 — `shell_on_source` refused paths where its own suggested remedy cannot serve

**Severity:** med · **Status:** fixed `be4a679b` + `433100bd` — 43 of 111 retired · **Target:** `src/util/path_security.rs`

23% of refusals were out-of-project paths where the suggested `symbols` alternative cannot serve, plus 16% `wc` — a count, not content. Evidence: § *IL-3 `shell_on_source` — composition (110)*.

### GF-4 — Refusals teach the CALL, not the PREDICATE

**Severity:** med · **Status:** fixed `ba591f12` (`refusal_predicate`, ~150 bytes per family) · **Target:** `src/prompts/mod.rs`

96% immediate compliance and 3% immediate repeat against **47% per-session repeat** — the agent stops making *that* call and keeps making calls of that shape. Efficacy remains UNMEASURED until the repeat rate is re-run. Evidence: § *The Iron-Law population*; acceptance criteria in § *Baselines — for re-measurement*.

### GF-5 — Guide delivery is success-path-only — premise wrong, finding still real

**Severity:** high · **Status:** fixed `ba591f12` · **Target:** `src/server.rs`

True of `call_content` (the `?`), false of the system: `call_tool_inner` already assembles both branches into one `CallToolResult`, so a second block on an error was always supported. Structural reason `iron-laws-detail` could not be fetched at the one moment it was wanted. Evidence: § *Guide reachability — the other end of the same finding* and its § *Resolved 2026-08-16 — `ba591f12`*.

### GF-6 — IL-1 is the sick family on the learning axis

**Severity:** med · **Status:** open · **Target:** `src/util/path_security.rs`

71% per-session repeat, 7.4 refusals per session, worst session 33 refusals over 7 hours. The one finding here still open. Evidence: § *The Iron-Law population*; § *Baselines — for re-measurement*.

### GF-7 — Negative result: IL-4/IL-5 repeat 0% across 18 sessions

**Severity:** info · **Status:** closed · **Target:** —

A total, file-intrinsic predicate teaches on first contact. **Do not "improve" these** — they are the regression control, and must stay at 0% after any IL-3 change. Evidence: § *The Iron-Law population*; § *Baselines — for re-measurement*.

### GF-8 — `untrusted-content` fetched 0 times in 30 days

**Severity:** med · **Status:** mitigated · **Target:** `src/prompts/mod.rs` `PULL_ONLY_GUIDE_TOPICS`

Now DECLARED rather than silently missing: the allowlist marks it *"PENDING BL-25, candidate trigger is whichever surface first admits third-party text, not identified"*. Evidence: § *Guide reachability*; § *Reconciliation with BL-25 phase 1*, which also records the one rationale this audit and the allowlist disagree on.
## Corpus

Single project (codescout), avoiding the project-confounding TU-N § Method caveats records.

| | |
|---|---|
| Window | `2026-07-17 16:16` → `2026-08-16 15:57` |
| Total calls | 20,323 |
| Errors | 894 (`outcome='error'`) |
| Unclassified (`err_family` empty) | **26 (2.9%)** |
| Iron-Law violations | **557 (62% of all errors)** |

**`err_family` coverage improved sharply since TU-5**, which measured 31% unclassified on the
merged 13-project corpus. TU-5's recommendation (extend `normalize_err_family`) appears to
have landed. Rankings here describe 97% of the error population.

## The Iron-Law population

| Family | Refusals | Sessions | Once only | **Repeated** | 5+ | Worst |
|---|---:|---:|---:|---:|---:|---:|
| `il3_pipe_to_trimmer` | 193 | 47 | 25 | **22 (47%)** | 10 | 28 |
| `il1_read_overlaps_symbol` | 178 | 24 | 7 | **17 (71%)** | 8 | **33** |
| `il3_shell_on_source` | 109 | 42 | 26 | 16 (38%) | 5 | 22 |
| `il2_structural_edit` | 57 | 27 | 16 | 11 (41%) | 2 | 13 |
| `il5_edit_markdown_routing` | 14 | 14 | 14 | **0** | 0 | 1 |
| `il4_read_markdown_routing` | 4 | 4 | 4 | **0** | 0 | 1 |

**IL-3 is the largest class at 302** (both families), ahead of IL-1's 178.

Grouped by `session_id`, not `cc_session_id` — the latter was mis-attributed before `06498ed2`
(see `docs/issues/archive/2026-08-16-usage-db-attributes-calls-to-a-shared-session-id-file.md`).
An `/mcp` reconnect splits one conversation into two rows, so **per-session repeat rates are
biased downward** and are conservative.

## Monthly trend

| Month | IL-1 | IL-2 | IL-3 | Total | IL-3 sessions | IL-3 per session |
|---|---:|---:|---:|---:|---:|---:|
| 2026-07 (from the 17th) | 88 | 8 | 66 | 163 | 11 | 6.0 |
| 2026-08 (to the 16th) | 89 | 51 | 237 | 394 | 49 | 4.8 |

IL-3 volume tripled while intensity fell — more sessions, not worse behaviour. IL-2 rose 6×.

## IL-3 `pipe_to_trimmer` — composition (192 with `input_json`)

**Left-hand side (what produced the output):**

| Producer | Count | Share |
|---|---:|---:|
| **`git`** | **91** | **47%** |
| `cargo test` | 59 | 31% |
| `cargo clippy` | 21 | 11% |
| `cargo build` / `rb` | 6 | 3% |
| recursive `grep` / `rg` | 1 | <1% |

**Right-hand side (the trimmer):** `tail` 98 · `head` 60 · `grep` 34 · `sort` 6 · `wc` 6.
**61% (117) also redirect stderr** (`2>&1`) — the agent wants the failure summary.

**The finding: 47 of the 94 `git` refusals (50%) carry a self-bounding flag** — an explicit
count (`-n`, `--max-count`, or the `-3` shorthand: 22), `--stat` (19), `--porcelain`/`--short`
(13), `--show-current` (2). That is **24% of all IL-3 pipe refusals** landing on commands
whose output is already a handful of lines.

**Corrected 2026-08-16 — the first number was 58/91 (64%), and it was wrong twice.** It
matched against the whole command string, so the trimmer's own `head -60` counted as the
producer's limit; and it counted `--oneline` (25 of 94), which bounds line *width*, not line
*count* — `git log --oneline` still emits one line per commit for every commit. Re-measured
against the left-hand side only, with `--oneline` excluded. The inflated figure reached a
commit message and this file before the LHS check was run; it is the same wrong-query-shape
class this audit is otherwise about.

Representative refused commands, all self-bounded:

```
git log --oneline -3 && git status --porcelain | grep -E "temp_write_guard|tools/mod"
git branch --show-current && git status --short | head -30
git show --stat --oneline <sha> | head -20
```

## IL-3 `shell_on_source` — composition (110)

**Re-measured by leading verb** — the first pass used overlapping `LIKE`s and counted
`head`/`tail` as "read-only metadata". They are content reads and stay blocked; `ls`/`stat`
were never in the blocklist at all. Corrected classification of the 111:

| Class | Count | Share | Verdict |
|---|---:|---:|---|
| Target outside the project | 25 | 23% | false positive — **fixed `433100bd`** |
| `wc` (returns a count, not content) | 18 | 16% | false positive — **fixed `be4a679b`** |
| Mutating (`sed -i`, `>`, `tee`) | 16 | 15% | **correct** — e.g. `sed -i '258,269d' src/librarian/catalog/gc.rs` |
| In-project content reads (`grep`/`cat`/`head`/`tail`/`sed`/`awk`) | rest | ~46% | **correct** — the gate's premise holds |

Leading verbs, for reference: `grep` 39, `cd` 21 (compound), `git` 12, `wc` 8, `sed` 6,
`echo` 5, `cat` 5, `ls` 4, `awk` 2, `ps` 2. The non-reader heads (`cd`, `git`, `echo`, `ps`)
are blocked by a *later* segment, not by themselves.

**43 of 111 (39%) retired**, not the 44% first claimed. The distinction the two fixes encode:
*content vs a measurement of content* (`wc` allowed, `head` not), and *a remedy that exists vs
one that does not* (`symbols` cannot serve a path outside the active project).

The outside-project class matters because the gate's whole premise is *"route to `symbols`
instead"* — and `symbols` cannot navigate a crate in `~/.cargo/registry`, a `.py` file in
`~/.local/share`, or a TOML config. The suggested alternative does not exist for 22% of what
it refuses.

## The classifier

`is_unbounded_lhs` — `src/util/path_security.rs:906`

```rust
const UNBOUNDED_PREFIXES: &[&str] = &[
    "cargo", "npm", "pnpm", "yarn", "python", "python3", "pytest", "go", "mvn", "gradle",
    "git", "rg", "fd",
];
if UNBOUNDED_PREFIXES.contains(&head) { return true; }

if head == "grep" { return has_recursive_flag(lhs); }   // flag-conditional
// find defaults to recursive; -maxdepth bounds it       // flag-conditional
```

Two facts make this a cheap refactor rather than a redesign:

1. **The mechanism already exists.** `grep` and `find` are already flag-conditional. `git`
   simply never got the same treatment — it is a head-token match with no flag inspection.
2. **The function's own doc states the intended bias**, and the data violates it:

   > *"Conservative: when shape parsing is ambiguous, treat as bounded (allow the pipe) —
   > false negatives cost a buffer dance, false positives cost user friction."*

## Guide reachability — the other end of the same finding

Explicit `get_guide` fetches over the same 30-day window (28 calls, 15 sessions):

| Topic | Push trigger | Fetches |
|---|---|---:|
| `tracker-conventions` | librarian adapter | 14 |
| `error-handling` | none | 4 |
| `symbol-navigation` | 3 sites | 3 |
| `workspace-state` | `workspace` (added 2026-08-16) | 2 |
| `librarian-runtime` | none (chained ×2) | 1 |
| **`iron-laws-detail`** | **none** | **1** |
| **`untrusted-content`** | **none** | **0** |

`iron-laws-detail` — the guide holding per-law gate text and exceptions — was opened **once**
against **557 Iron-Law violations**. It is advertised in *both* pull surfaces
(`server_instructions` and `get_guide`'s tool description) and still got one fetch, which is
A-10's "never fetched" failure mode measured at n=557 opportunities.

One agent called `get_guide(topic="edit_code")` — an invalid topic. They guessed the **tool
name**, which is a discoverability signal from someone who *was* reaching deliberately.

**Structural cause:** guide injection lives in `Tool::call_content`
(`src/tools/core/types.rs:600`), and line 601 is `let mut val = self.call(input, ctx).await?;`
— the `?` propagates before the injection code. **A refusal can never carry a guide.** That is
why `progressive-disclosure` has 9 push sites (overflow is a *successful* result shape) while
`iron-laws-detail` has zero: half the event space is invisible to the hook.


### Resolved 2026-08-16 — `ba591f12`

**GF-5's premise did not survive being checked.** "Guide delivery is success-path-only" is
true of `Tool::call_content` — the `?` after `self.call(..)` means an `Err` never reaches the
injection hook — but false of the system. One layer up, `call_tool_inner` assembles *both*
branches into a single `CallToolResult`, and its own comment says so: *"success or error both
produce a CallToolResult so we can apply post-processing in one place."* Pushing a second
`Content` block onto an error is the same move `post_process` already makes for the path
banner. The finding was a true observation about a narrow scope reported as a claim about the
whole.

**What shipped is the predicate, not the guide.** ~150 bytes per family against
`iron-laws-detail`'s 9.9 KB, stating the rule *and its exceptions* — the half a refusal cannot
convey. Four families whose condition is not inferable from the message:
`il1_read_overlaps_symbol` (71% repeat), `il3_pipe_to_trimmer` (47%), `il3_shell_on_source`,
`il2_structural_edit`. The table is deliberately partial; an unrecognised family attaches
nothing, so this cannot become a per-error tax.

**Two placement facts that decided the design**, both of which would have produced a silent
no-op:

- `post_process` returns early for `run_command` — exactly where IL-3 fires. The append had to
  go at the assembly point, not there.
- `GuideLedger::emitted.is_empty()` is the session opener's trigger, so a sentinel key in
  `emitted` would cost every session that *begins* with a refusal its orientation guide.
  `notice_once` exists for precisely this and says so in its doc. Mutation-verified: swapping
  it for `insert` turns exactly one test red, and that test's output shows the following call
  arriving with no opener.

**Still open on this axis:** whether the predicate actually reduces the 47% per-session repeat
rate. That is a re-measurement against the § Baselines row, not an inspection, and it needs a
comparable window of sessions after this commit.
## Refactor data pack — IL-3

Everything a change to `is_unbounded_lhs` needs:

- **Target:** `src/util/path_security.rs:906`, `UNBOUNDED_PREFIXES`.
- **Highest-yield single change:** make `git` flag-conditional like `grep`/`find`. Expected
  effect: **−47 refusals (24% of IL-3 pipe)** over a comparable window.
  **SHIPPED 2026-08-16 — `06a53ad3`**, as `git_output_is_bounded`. Nine tests, mutation-verified
  (restoring `git` to `UNBOUNDED_PREFIXES` turns all five positive cases red while the
  quoted-separator case stays green).
- **Bounding flags observed in the corpus:** `--oneline`, `--short`, `--porcelain`,
  `--show-current`, `--stat`, `-1`, `-2`, `-3`, `-5`. Note `git log` without a count is
  genuinely unbounded — the predicate is *"does a count/format flag bound it?"*, not
  *"is it git?"*.
- **Do not relax `cargo`.** 59 `cargo test` + 21 `cargo clippy` refusals are correct: output
  is genuinely unbounded, and `run_command` already returns a **structured test envelope**
  (`{type:"test", passed, failed, failures}`) that is strictly better than `| tail -40`.
  The agent's problem there is not knowing the payoff exists, which is a message question,
  not a classifier question.
- **`shell_on_source`:** two separable carve-outs — read-only metadata verbs (26) and
  out-of-project paths (24). The second is the more principled: the gate should not refuse
  what `symbols` cannot serve.
- **Regression guard:** the 0%-repeat control below must stay at 0%.

## Prior art — this is an incomplete fix, not a new defect

Found after this audit was first written, and it reframes GF-1.

**`docs/issues/archive/2026-05-18-il3-overtriggers-bounded-lhs.md`** (opened and closed the
same day, status `fixed`) reported: *"IL3 over-triggers on bounded LHS commands, forcing
buffer-dance overhead."* Its root cause was that the `IL3_LHS` regex *"treats every LHS as
unbounded"*, and its fix was **"split LHS into bounded/unbounded"** — which is precisely the
`UNBOUNDED_PREFIXES` list this audit is now measuring.

So GF-1 is not a regression and not a new discovery. It is the **same failure class at finer
grain**: the May fix moved from *"everything is unbounded"* to *"these thirteen binaries are
unbounded"*, and `git` — whose real-world use is 64% self-bounded — landed on the wrong side
of the new line. The remedy is the same shape a third time: move from *binary* to
*binary + flags*, which the same function already does for `grep` and `find`.

Independent corroboration from a live tracker: `docs/trackers/codescout-usage-frictions.md`
keeps a running tally of IL-3 false positives and had already reached **"13 of 15 pure 'show
me less'"**, concluding that *"the gate has a real false-positive rate of its own, which the
narrower rule would reduce rather than add to."* That entry reached this audit's conclusion
by a different route — reading refused commands one at a time rather than aggregating them.

**Sibling defect filed this session:**
`docs/issues/archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md` — `run_command`
hands its string to `sh -c` unexamined, so backticks in a `git commit -m` message are
substituted, and the failure reports `Argument list too long`, which is the wrong cause. Same
function family, same theme as GF-4: the shell-shape knowledge lives in one place and never
reaches the caller.

## Baselines — for re-measurement

Re-run on codescout only, same window length.

| What | Baseline |
|---|---|
| Iron-Law share of all errors | 62% (557/894) |
| `err_family` unclassified | 2.9% (26/894) |
| IL-3 pipe: `git` share of LHS | 47% (91/192) |
| IL-3 pipe: self-bounded `git` | 50% of git, 24% of family (47/94) — LHS only, `--oneline` excluded |
| IL-3 pipe: immediate repeat | 3% (6/193) |
| IL-3 pipe: per-session repeat | 47% (22/47 sessions) |
| IL-1: per-session repeat | 71% (17/24 sessions) |
| IL-4 + IL-5: per-session repeat | **0% (0/18 sessions)** |
| `iron-laws-detail` fetches / 30d | 1 |
| `untrusted-content` fetches / 30d | 0 |

## Method — how to repeat this

```sql
-- population
SELECT err_family, COUNT(*) FROM tool_calls
WHERE outcome='error' GROUP BY err_family ORDER BY 2 DESC;

-- per-session repeat (the learning axis)
SELECT err_family, COUNT(*) sessions,
       SUM(CASE WHEN n=1 THEN 1 ELSE 0 END) once_only,
       SUM(CASE WHEN n>=2 THEN 1 ELSE 0 END) repeated, MAX(n) worst
FROM (SELECT err_family, session_id, COUNT(*) n FROM tool_calls
      WHERE err_family LIKE 'il%' AND session_id IS NOT NULL
      GROUP BY err_family, session_id)
GROUP BY err_family;

-- recovery (the TU-7 axis)
SELECT n.tool_name, COUNT(*), SUM(n.outcome='success'), SUM(n.err_family LIKE 'il3%')
FROM tool_calls r JOIN tool_calls n
  ON n.id=(SELECT MIN(x.id) FROM tool_calls x
           WHERE x.session_id=r.session_id AND x.id>r.id)
WHERE r.err_family='il3_pipe_to_trimmer' GROUP BY n.tool_name;
```

**Trap hit during this pass:** `outcome` values are `success` / `error` — **not `ok`**. A
filter of `outcome!='ok'` matches every row and silently reports successes as errors. One
intermediate claim in this audit ("31% of errors unclassified") was wrong for exactly that
reason before the vocabulary was checked. Read `SELECT DISTINCT outcome` before filtering.

## History

### 2026-08-16 — opened

Prompted by a review of all ten `get_guide` topics: 4 of 10 have no push trigger, and the
trigger distribution is lopsided (9 of 12 push sites point at `progressive-disclosure`).
The base-arm query — *"does the same session re-violate the same law after a refusal?"* — was
run as a pre-registered no-ship test and came back 38–71% for four families, refuting no-ship.
TU-7 was then read and reframed the result: recovery was already known-good, so the finding
moved to firing-correctness and to the learning-vs-fixing distinction.
