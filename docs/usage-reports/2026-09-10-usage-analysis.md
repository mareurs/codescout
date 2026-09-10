# Usage Analysis — 2026-09-10 (3-day window)

**Window:** `called_at >= datetime('now','-3 days')`, i.e. 2026-09-07 → 2026-09-10.
**Deliberately scoped**, not all-time: the question was "what are the current frictions", and an
all-time report mixes in code that no longer exists (`reconnaissance-patterns:R-40` — usage.db is
commit-mixed and time-spanning).

**Tool-name folding:** not required in this window. The 2026-09-02 collapse (`artifact`→`doc`,
`read_markdown`→`read_file`, …) predates it, so no row splits a tool's history. Verified rather
than assumed — a window crossing that date would need the `UNION` the skill prescribes.

**Instrument blind spot, stated up front:** usage.db records **MCP calls only**. Native `Bash`
work is invisible to it, and both `Bash` and `run_command` are permitted in this project right
now, so every count below is a lower bound on activity and an unbiased sample of nothing in
particular. It is still the only instrument that sees tool-level outcome classes.

## Cross-Project Summary

Projects with activity: **6** of 79 discovered DBs.

| project | calls | success | recoverable_error | error | rec-err % |
|---|---:|---:|---:|---:|---:|
| codescout | 8,749 | 8,251 | 496 | 3 | 5.7% |
| whitelabel-crawler | 5,909 | 5,674 | 236 | 0 | 4.0% |
| MRV-poc | 1,939 | 1,837 | 102 | 0 | 5.3% |
| prompt-engineering | 317 | 304 | 13 | 0 | 4.1% |
| backend-kotlin | 189 | 181 | 8 | 0 | 4.2% |
| claude-plugins | 17 | — | — | — | (<20 calls) |
| **total** | **17,120** | **16,247** | **855** | **3** | **5.0%** |

**Three hard errors in three days across 17,120 calls.** The failure surface is essentially all
`recoverable_error` — a teaching refusal the caller can act on — which is what the
repair-and-continue ADR intended. The 5.0% rate tracks the **5.9%** that ADR measured in July, so
the law has not moved the *rate*: it converts errors into repairs, it does not reduce mistakes.

### Top Issues

1. **[codescout] The IL-3 source-file gate is the single largest friction: 75 calls.** Three
   distinct hint variants (`grep`, `read_file`/`symbols`, `symbols(name=…)`). One of those 75 was
   a false positive with a filed cause —
   `docs/issues/2026-09-10-source-gate-joins-an-unexpanded-var-path-onto-the-project-root.md`.
2. **[codescout] `append_entry` unpushed-commits refusal: 19 calls.** Filed high severity
   (`24691692`); the remedy it names is a push, which no session may perform unasked.
3. **[codescout] `missing 'path' parameter`: 21 calls** (`read_file` 12, `edit_code` 5,
   `edit_file` 4). Not repairable — genuinely absent input — so the teaching hint is correct. But
   see the alias section: the schema currently states **nothing** about path being required.
4. **[codescout] IL-3 pipe limiter: ~20 calls**, every one a `cargo … | tail` or
   `git … | head`. The natural idiom, refused.
5. **[codescout] `read_file` source-range/symbol overlap: 23 calls**, redirected to `symbols`.
6. **[codescout] Worktree not activated: 19 calls** (`doc` 15, `edit_file` 4) —
   `Write blocked: git worktrees detected but workspace(action='activate') has not been called`.
7. **[codescout] Direct read/edit of a librarian ledger: 15 calls.** Correct refusals; the
   frequency says the ledger boundary is not discoverable before you hit it.

---

## The measurement usage.db could not make, and how to make it

**Alias repairs are invisible in the error data.** The repair-and-continue law repairs a synonym
silently and classes the call `success`, so a parameter mistake happening hundreds of times per
window produces **zero** rows in every error query above. `/analyze-usage` — this project's own
instrument — reports no friction for it.

That is not a gap in the queries; it is a consequence of the design. **Silent repair removed the
telemetry that would say whether the repair matters.** Recovered here by querying `input_json`
directly, which is the only surface that still carries the caller's actual spelling:

```sql
SELECT COUNT(*) FROM tool_calls
WHERE called_at >= datetime('now','-3 days')
  AND json_extract(input_json,'$.file_path') IS NOT NULL;
```

Across the three busiest projects, same 3-day window:

| parameter | codescout | crawler | MRV-poc | **total** |
|---|---:|---:|---:|---:|
| `path` (canonical) | 4,347 | 3,007 | 1,127 | **8,481** |
| `output_id` | 119 | 11 | 22 | **152** |
| `file_path` | 56 | 47 | 44 | **147** |
| `relative_path` | 0 | 0 | 0 | **0** |
| `file` | 0 | 0 | 0 | **0** |
| `symbols` · `name` | 269 | 199 | 98 | **566** |
| `symbols` · `symbol` | 211 | 75 | 24 | **310** |
| `name_path` | 255 | 54 | 61 | **370** |
| `symbols` · `query` | 6 | 2 | 3 | **11** |

### What follows from it

**1. Two of the four path aliases are dead.** `relative_path` and `file`: **zero** calls in three
days across 12,597 tool calls. They are carried in every path-taking schema, in a `FIXTURE NOTE`
comment in each, and in the alias gate's `EXPECTED_ALIAS_COUNTS_BY_TOOL` — a maintained apparatus
around two parameters nobody uses. Removing them from the advertised surface has **zero observed
cost**.

**2. `file_path` and `output_id` are live** — 299 calls / 3 days ≈ 100/day. Under the collapse's
"announce every correction" cadence that is ~100 `corrections` notes per day. Sustainable, and
worth knowing before it ships rather than after.

**3. `symbols` has a real ambiguity cost, and the canonical pick in the spec was backwards.**
`name` beats `query` **566 to 11** — a 51:1 preference for the *alias*. And `name_path` vs
`symbol` is **370 vs 310**, a near coin-flip, which is the ambiguity cost that could not be
argued either way without this number. The 2026-09-10 spec had proposed keeping `query` and
`symbol`; on this data that would convert the overwhelming majority form into the corrected one.
`symbols` stays out of the current pass (it carries no top-level `required`, so no false claim to
repair), and if it is reopened the canonical must be picked from usage, not from family
consistency.

**4. The 21 `missing 'path'` refusals sit next to a schema that says nothing.** After
`2735df73` removed the API-illegal top-level `anyOf`, no path-taking schema states that a path is
required — the honest options were "say nothing" or an illegal construct. Collapsing to one
advertised name makes `required: ["path"]` *true*, which is the only shape that both states the
requirement and is legal on the wire.

---

## Project: codescout

**DB:** `/home/marius/work/claude/codescout/.codescout/usage.db`

### Error breakdown (3-day window, top 25 by count)

| n | tool | error |
|---:|---|---|
| 52 | `run_command` | shell access to source files is blocked → `grep` hint |
| 19 | `doc` | `append_entry`: ledger has commits not on upstream |
| 18 | `edit_file` | edit contains a symbol definition (`"fn "`) → `edit_code` |
| 16 | `read_file` | source range overlaps named symbol `'call'` → `force=true` |
| 16 | `run_command` | shell access to source files is blocked → `read_file`/`symbols` hint |
| 15 | `doc` | Write blocked: worktrees detected, `workspace(activate)` not called |
| 12 | `read_file` | missing `'path'` parameter |
| 7 | `read_file` | `issue-clusters.md` is a librarian-managed ledger |
| 7 | `run_command` | shell access to source files is blocked → `symbols(name=…)` hint |
| 6 | `run_command` | IL3: piped `cargo check … \| tail -60` |
| 5 | `edit_code` | missing `'path'` parameter |
| 4 | `doc` | `body_edits[0]`: missing required `'heading'` field |
| 4 | `edit_file` | `bug-fix-session-log.md` is a librarian-managed ledger |
| 4 | `edit_file` | Write blocked: worktrees detected |
| 4 | `edit_file` | missing `'path'` parameter |
| 4 | `read_file` | `bug-fix-session-log.md` is a librarian-managed ledger |
| 4 | `read_file` | source range overlaps `'run_fix'` |
| 4 | `run_command` | IL3: piped `cargo build --lib --tests \| tail -200` |
| 4 | `run_command` | IL3: piped `git … ls-files \| head -50` |
| 3 | `edit_file` | `edit[2]`: old_string not found — batch aborted |
| 3 | `read_file` | file not found: a `.superpowers/sdd/…` task report |
| 3 | `read_file` | source range overlaps `'tests'` |
| 3 | `run_command` | IL3: piped `cargo test --lib … \| tail -n 100` |
| 3 | `run_command` | IL3: piped `cargo test --workspace … \| tail -1` |
| 2 | `doc` | `append_entry`: id allocation unsupported from a worktree checkout |

### Reading the two IL-3 classes together

75 source-gate + ~20 pipe-limiter ≈ **19% of all recoverable errors in this project** come from
two gates. Neither is malfunctioning by its own predicate. What the number says is that both
refuse a *natural idiom* — `cat $VAR/file.sh`, `cargo test | tail` — and the cost lands on every
session, repeatedly, rather than being learned once. Both have documented escapes
(`acknowledge_risk: true`; run bare and query the `@cmd_*` buffer), and the frequency suggests
the escape is not reaching the caller at the moment of refusal.

## Improvement candidates, ranked by measured cost

1. **IL-3 source gate (75/3d)** — fix the unexpanded-`$VAR` false positive
   (`docs/issues/2026-09-10-source-gate-joins-an-unexpanded-var-path-onto-the-project-root.md`),
   then re-measure before touching the heuristic itself. A false-positive fix is safe; loosening
   the heuristic is not.
2. **`append_entry` push refusal (19/3d)** — move the check to push-time
   (`24691692` § Fix). Highest severity of the set because it blocks the only write path into
   every ledger.
3. **Drop `relative_path` and `file` from the advertised surface (0/3d)** — free.
4. **Make `required: ["path"]` stateable via the alias collapse (21/3d of missing-path)** — the
   in-flight plan `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md`.
5. **IL-3 pipe limiter (~20/3d)** — no fix proposed; the guard is correct and the measured cost
   is the price of the `@cmd_*` discipline. Recorded so a future proposal argues against a number.
6. **Ledger-boundary discoverability (15/3d)** — sessions reach for `read_file`/`edit_file` on a
   ledger and are refused. The refusals are right; nothing advertises the boundary in advance.

## Not done

- **No session drill-down.** The skill's Step 7 triggers on `total_calls > 50` or
  `errors/total > 10%`; the second is met by no project here and the first would drill nearly
  every session in a 17k-call window without narrowing anything.
- **No LSP queries.** Out of scope for the frictions question asked.
- **Counts are unit'd but single-instrument.** Every number is "MCP calls in a 3-day window on
  this machine". No `Bash` activity, no other host.
