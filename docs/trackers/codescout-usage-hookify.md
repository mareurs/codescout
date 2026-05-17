# Codescout Usage Hookify Candidates — H-N Log

Patterns observed across U-N entries that earn substrate enforcement.
Format from `~/.claude/buddy/skills/codescout-pika/SKILL.md` § Tracker
Format. Each H-N is gated by `Promote-when` before graduating to a real
`/hookify` rule.

---

### H-1 — Deny piped `run_command` (warn first)

**Pattern:** `run_command` invoked with a shell command whose body matches
`\| (head|tail|wc|grep)\b` (and likely also `awk|sed`). The pipe filters
MCP `run_command` output instead of using the `@cmd_*` buffer system.

**Confirming data:**
- **U-1** — 50 slips in one session (`753e9a4a`), single-shape predicate.
  Backing rows: `pika_observations.cc_session_id='753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce'`
  AND `subkind='iron_law_3'` (50 rows).
- **Smoke-scan observational** — 3090 historical pipe-shaped `run_command`
  calls in `.codescout/usage.db` across the whole project, recorded in
  `docs/trackers/pika-phase-1-validation.md`. This is observational
  (no per-call judgment), not verdict-bearing. Used only as
  cross-session shape confirmation, not as the sole basis for promotion.

**Proposed hookify rule:**

- **Predicate:** tool name `run_command`, command body regex
  `\|\s*(head|tail|wc|grep|awk|sed)\b`.
- **Decision:** `warn` (not `deny` at first ship — pipes are legitimate
  inside `bash -c "…|…"` script bodies; deny would punish script-internal
  pipelines that have nothing to do with Iron Law 3).
- **Reason text:** *"Iron Law 3: `run_command` output piped to a filter.
  Run the command bare and query the returned `@cmd_*` buffer in a
  follow-up call (e.g. `grep FAILED @cmd_abc`). The buffer system exists
  to save context — use it."*

**Promote-when:**
- A second user-asked scan (different session) writes ≥10 IL3 slip rows
  with the same predicate shape, AND
- The `warn` rule has shipped and run for ≥1 session without false-positive
  complaints on script-internal pipes; then promote `warn` → `deny`.

**Status:** proposed.

**Notes:**
- The 50-row evidence covers 9 command families (`git`, `find`, `cargo`,
  `ls`, `grep`, `sqlite3`, `cat @<buffer>`, `diff`, other) — the predicate
  is command-family-agnostic, which means a single regex catches all of
  them without per-family tuning.
- 2 of the 50 (cat-buffer family) already use a `@file_*` reference but
  then pipe its content through `jq | wc -c` or `jq | head`. The hookify
  rule still applies — the violation is the trailing pipe, not the input.
