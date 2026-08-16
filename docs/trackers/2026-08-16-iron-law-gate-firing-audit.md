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
| Firing correctness | *Should the gate have fired?* | not measured | **30% of IL-3 pipe refusals are self-bounded `git`** |

A guard can have an excellent corrective message *and* fire on commands its own rationale
says to allow. Those are independent properties. TU-7 established the first; this establishes
the third.

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

**The finding: 58 of the 91 `git` refusals (64%) carry a self-bounding flag** —
`--oneline`, `--short`, `--porcelain`, `--show-current`, `--stat`, `-1`/`-2`/`-3`/`-5`.
That is **30% of all IL-3 pipe refusals** landing on commands whose output is already a
handful of lines.

Representative refused commands, all self-bounded:

```
git log --oneline -3 && git status --porcelain | grep -E "temp_write_guard|tools/mod"
git branch --show-current && git status --short | head -30
git show --stat --oneline <sha> | head -20
```

## IL-3 `shell_on_source` — composition (110)

| Class | Count | Share | Verdict |
|---|---:|---:|---|
| Mutating (`sed -i`, `>`) | 39 | 35% | **correct** — e.g. `sed -i '258,269d' src/librarian/catalog/gc.rs` |
| Read-only metadata (`wc`, `ls`, `stat`) | 26 | 24% | false positive — already filed, `30365fe50974fa6b` |
| Target outside the project | 24 | 22% | false positive — `~/.cargo/registry`, `~/.config`, sibling repos |
| `grep` as LHS | 74 | 67% | mixed (overlaps the above) |

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

## Refactor data pack — IL-3

Everything a change to `is_unbounded_lhs` needs:

- **Target:** `src/util/path_security.rs:906`, `UNBOUNDED_PREFIXES`.
- **Highest-yield single change:** make `git` flag-conditional like `grep`/`find`. Expected
  effect: **−58 refusals (30% of IL-3 pipe)** over a comparable window.
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

## Baselines — for re-measurement

Re-run on codescout only, same window length.

| What | Baseline |
|---|---|
| Iron-Law share of all errors | 62% (557/894) |
| `err_family` unclassified | 2.9% (26/894) |
| IL-3 pipe: `git` share of LHS | 47% (91/192) |
| IL-3 pipe: self-bounded `git` | 64% of git, 30% of family (58/192) |
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

