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

**Recurrence:** 50 occurrences in the scanned session.

**Severity:** low (all 50 rows are observational; none blocked progress).

**Status:** open — promoted to H-1.

**Backing rows:** `pika_observations.id ∈ {1..50}`, `tool_call_id ∈
[20255, 20823]`, `subkind='iron_law_3'`, `verdict='slip'`, `severity='low'`.

**Shape distribution among the 50 rows:**

| Pipe target | Count |
|---|---|
| `\| head` | 26 |
| `\| tail` | 15 |
| `\| wc` | 5 |
| `\| grep` | 4 |

| Command family | Count |
|---|---|
| `git …` | 11 |
| `find …` | 8 |
| `cargo …` | 8 |
| `ls …` | 6 |
| `grep …` | 6 |
| `sqlite3 …` | 5 |
| other | 3 |
| `cat @<buffer> …` | 2 |
| `diff …` | 1 |
