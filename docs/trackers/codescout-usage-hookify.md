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
- **U-1** — 45 slips in one session (`753e9a4a`), single-shape predicate.
  Backing rows: `pika_observations.cc_session_id='753e9a4a-a81f-4cf2-aeaa-a3877d35d1ce'`
  AND `subkind='iron_law_3'` (45 rows; originally 50, 5 self-matches
  retroactively deleted 2026-05-17 — see U-1 *Post-cleanup note*).
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
- The 45-row evidence covers 8 command families (`git`, `find`, `cargo`,
  `ls`, `grep`, `cat @<buffer>`, `diff`, other) — the predicate is
  command-family-agnostic, which means a single regex catches all of them
  without per-family tuning. (`sqlite3` was a 9th family pre-cleanup but
  all 5 of its rows were Pika self-matches and were deleted.)
- 2 of the 45 (cat-buffer family) already use a `@file_*` reference but
  then pipe its content through `jq | wc -c` or `jq | head`. The hookify
  rule still applies — the violation is the trailing pipe, not the input.


---

### H-2 — Deny `read_file` on `.md` (direct deny, no warn stage)

**Pattern:** `read_file` invoked with a path ending in `.md`. Already
hard-rejected by the in-server tool gate (`"Use read_markdown for
markdown files"`), but the rejection costs a tool round-trip + leaves
a row in `tool_calls`. Hookify catches it pre-call.

**Confirming data:**
- **U-2** — 3 same-turn slips in session `42874b1a`, all blocked by
  the in-server gate. Backing rows:
  `pika_observations.subkind='read_file_markdown'` (3 rows).
- **Cross-session shape confirmation (deferred):** no second-session
  data yet. H-2 stays `proposed` until a second session writes ≥1
  more `read_file_markdown` slip.

**Proposed hookify rule:**

- **Predicate:** tool name `read_file`, `path` matches regex `\.md$`.
- **Decision:** `deny` straight off (skip the `warn` stage that H-1
  used). Justification: the in-server tool gate *already* hard-rejects
  this — there is no legitimate `read_file(*.md)` call. `warn` is
  redundant; `deny` saves the round-trip.
- **Reason text:** *"Markdown files must use `read_markdown(path)` —
  heading-addressed, size-adaptive, slice-able. `read_file` on `.md`
  is hard-rejected by the in-server gate; calling it costs a wasted
  round-trip and a `tool_calls` row. Use `read_markdown` first try."*

**Promote-when:**
- A second user-asked scan (different `cc_session_id`) writes ≥1 more
  `read_file_markdown` slip row, confirming the pattern is not
  session-local quirk. (Lower bar than H-1's ≥10 because the in-server
  gate already certifies the predicate is universally invalid.)

**Status:** proposed.

**Notes:**
- Asymmetry with H-1: H-1 started `warn` because pipes are legitimate
  inside `bash -c "…|…"` script bodies. H-2 has no analogous
  false-positive — `.md` is `.md`. Direct-deny is correct first ship.
- Same-turn recurrence (3 slips in one turn) is the dominant signal,
  not cross-session count. The model did not learn from the first
  in-server rejection within the turn — memory route too slow;
  substrate route required.
