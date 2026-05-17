---
kind: tracker
status: active
title: Codescout Usage Frictions — U-N Log
owners: []
tags:
  - pika
  - iron-law
  - usage
---

# Codescout Usage Frictions — U-N Log

Observed tool-misuse violations. Each U-N is allocated by the Pika at scan
time. Format from `~/.claude/buddy/skills/codescout-pika/SKILL.md` § Tracker
Format. Backing rows live in `.codescout/usage.db::pika_observations`.

---

### U-1 — IL3 piped run_command, session 753e9a4a

**When:** First scoped Pika scan of this repo, 2026-05-17. Bound:
`cc_session_id='753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce'` (559 tool_calls).

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a filter
(`| head`, `| tail`, `| wc`, `| grep`) instead of running bare and querying
the `@cmd_*` buffer.

**Tool called:** `run_command` with command body containing `| {head,tail,wc,grep}`.

**Should have called:** `run_command(command)` bare, then in a follow-up
call query the returned `@cmd_*` buffer (e.g. `grep FAILED @cmd_abc`).

**Whistle delivered:** no (this is the first scan — whistles fire at
observation time, this U-N is a retrospective aggregate).

**Recurrence:** 45 occurrences in the scanned session (50 originally
observed; 5 self-matches retroactively removed 2026-05-17 — see
*Post-cleanup note* below).

**Severity:** low (all rows are observational; none blocked progress).

**Status:** open — promoted to H-1.

**Backing rows:** `pika_observations.id ∈ {1..50} \ {35, 36, 48, 49, 50}`,
`tool_call_id ∈ [20255, 20823]`, `subkind='iron_law_3'`, `verdict='slip'`,
`severity='low'`.

**Shape distribution among the 45 remaining rows (DB-authoritative,
2026-05-17 post-cleanup):**

| Pipe target | Count |
|---|---|
| `\| head` | 25 |
| `\| tail` | 12 |
| `\| wc` | 4 |
| `\| grep` | 4 |

| Command family | Count |
|---|---|
| `git …` | 11 |
| `find …` | 8 |
| `cargo …` | 8 |
| `ls …` | 6 |
| `grep …` | 6 |
| other | 3 |
| `cat @<buffer> …` | 2 |
| `diff …` | 1 |

**Post-cleanup note (2026-05-17):** Five rows (ids 35, 36, 48, 49, 50)
were retroactively deleted after the Pika scan SQL was discovered to
self-match — its own `LIKE '%|%'` discriminator and `INSERT INTO
pika_observations …` writes were being recorded as IL3 slips. All five
deleted rows were `sqlite3 …` invocations (Pika's own scan/insert
queries), which is why the `sqlite3` row dropped from 5 → 0 and is
omitted from the command-family table. The remaining 45 rows are real
IL3 slips. Discriminator fix: `INSTR(input_json, '''%|') = 0 AND
INSTR(input_json, 'pika_observations') = 0`. Filter mirrored to
`~/.claude/`, `~/.claude-sdd/`, `~/.claude-kat/` (md5 `670836e7`).
### U-2 — `read_file` on markdown, session 42874b1a

**When:** Second scoped Pika scan of this repo, 2026-05-17. Bound:
`cc_session_id='42874b1a-1ef5-44ce-ad64-4eb5b84cf93f'` (42 tool_calls).

**Iron Law / pattern:** Gates §"Tool gates" rule 2 — `read_file` on `.md`
should be `read_markdown`. SKILL Heuristic 6.

**Tool called:** `read_file(path="…/*.md")` — three times in one turn,
each hard-rejected by the in-server gate with hint
`"Use read_markdown for markdown files"`.

**Should have called:** `read_markdown(path)` first try; offers
heading-based navigation + slice-able body + smaller payload.

**Whistle delivered:** retrospective (scan-time, not real-time —
this is the first scan to surface the pattern).

**Recurrence:** 3 same-turn occurrences within session `42874b1a` (rows
`pika_observations.id ∈ {2,3,4}`). First slip = `med` severity, second
= `med`, third = `high` (same-turn recurrence escalates per Pika
Operating Principle 4).

**Severity:** high (same-turn frequency = the in-server error
message did not land; three round-trips wasted before correction).

**Status:** open — promotion candidate to H-2.

**Backing rows:** `pika_observations.id ∈ {2,3,4}`, `tool_call_id ∈
{21631, 21633, 21634}`, `subkind='read_file_markdown'`,
`verdict ∈ {slip, habit, habit}`, `cc_session_id='42874b1a-…'`.

**Shape — all 3 rows:**

| tool_call_id | path | gate response |
|---|---|---|
| 21631 | `…/buddy/data/memory-protocol.md` | `Use read_markdown for markdown files` |
| 21633 | `…/buddy/data/gates.md` | same |
| 21634 | `…/.buddy/memory/common/dont-fabricate-commit-rationale.md` | same |

**Notes:** All 3 paths are doc/config markdown, not source-adjacent.
The predicate is shape-only (`.md` suffix); no command-family
variation as with U-1.



### U-3 — IL3 piped `run_command`, session 2026-05-18 (×4)

**When:** Tracker backfill + jsonpath ship-prep session, 2026-05-18.
Bound: this conversation (continued from compacted 2026-05-17 fix work).

**Iron Law / pattern:** Iron Law 3 — `run_command` output piped to a
filter (`| head`, `| tail`, `| sort | uniq -c`, `&&`-chained `cat` →
`grep`) instead of running bare and querying the `@cmd_*` buffer.

**Confirming data:** four strikes in a single session, all flagged by
Pika's PreToolUse warning:

1. `git log --all --oneline | grep -E "^(808fe4b|a70816b5|66bee623)"`
   — looking up short SHAs. Fix: `git log --all --oneline -200` →
   `grep PATTERN @cmd_xxx`.
2. `diff trackers/X.md trackers/archive/X.md | head -20` — comparing
   two files. Fix: run `diff` bare, slice via buffer.
3. `cat .codescout/.../@tool_X | grep ... | sort | uniq -c` — counting
   status values in a JSON tool buffer. Fix: `grep PATTERN @tool_X`
   directly (the @tool_* handle works the same as @cmd_*).
4. `cat _TEMPLATE.md && echo "---" && grep -oE "..." trackers/X.md | tail -3`
   — fetching template + a small grep slice in one shot. Fix: run each
   read bare, query separately.

**Severity:** med — each strike added ~200-500 tokens of pipe output to
my context vs. the bounded buffer-query path. Cumulative drift over a
long session is the real cost; individual strikes look free.

**Status:** open — pattern recurs across sessions despite Pika warnings.

**Diagnosis (introspection):** the four strikes break down as:
- 2× reaching for `| head` / `| tail` to bound output size before
  it lands in context — buffer-query gives the same bound for free.
- 1× `sort | uniq -c` aggregation — habit from shell pipelines;
  buffer-query supports the same `grep` step but not the trailing
  `sort | uniq`, which means I'd need a follow-up run_command for
  the aggregation. The "single round-trip" instinct pushes me to
  pipe instead.
- 1× `&&`-chained two commands — saving a round-trip by bundling
  two reads into one call. Same root cause: round-trip aversion.

**Pointer:** Promotes H-1's warn→deny criterion. With ×4 in one session,
H-1 has 2 sessions of evidence (the U-1 baseline + this U-3 follow-up)
— close to deny-threshold.
