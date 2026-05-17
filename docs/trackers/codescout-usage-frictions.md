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
