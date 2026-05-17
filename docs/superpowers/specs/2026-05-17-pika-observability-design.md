# Pika Observability — Design

**Status:** drafted (brainstorm complete, awaiting user review)
**Owner:** codescout-pika specialist (`~/.claude/buddy/skills/codescout-pika/`)
**Brainstorm date:** 2026-05-17

## Goal

Give each project that loads the **codescout-pika** specialist a per-project, inspectable audit trail of:

- **Iron Law violations** (codescout tool misuse — `read_file` on source, `edit_file` with structural keywords, piped `run_command`, missing `workspace` restore)
- **Tool bugs** (codescout MCP tools returning wrong content / misleading errors / corrupt output)
- **Misusage** (broader than Iron Laws — overly broad `semantic_search`, wrong-tool selection)
- **Patterns** (recurring shapes worth promoting to substrate, positive or negative)

The audit trail must serve three layered read paths:

1. **Retrospective inspection** — "what did Pika catch this week / this session?"
2. **Cross-session aggregation** — "this misuse appeared in 5 sessions; graduate to hookify."
3. **Live citation** — when Pika whistles in chat, the whistle can cite the exact `tool_calls.id`.

## Architecture

Per-project SQLite, reusing the existing `.codescout/usage.db` codescout already maintains.

```
                      ┌───────────────────────────────────────────────┐
                      │  Project workspace                            │
                      │                                               │
                      │  .codescout/usage.db   (SQLite, WAL mode)     │
                      │  ┌─────────────┐    ┌──────────────────────┐  │
                      │  │ tool_calls  │←──┐│ pika_observations    │  │
 codescout MCP ─────────►│ (existing)  │   ││ (new)                │  │
 every tool call      │  └─────────────┘   │└──────────────────────┘  │
                      │                    │            ▲             │
                      └────────────────────┼────────────┼─────────────┘
                                           │ FK         │
                                           │            │ writes ONLY on user ask
                            ┌──────────────┴──────┐  ┌──┴──────────────────┐
                            │ Pika (when summoned │  │ Markdown views      │
                            │ AND user asks       │  │ (rendered from DB)  │
                            │ "scan" / "audit")   │  │ • U-N tracker       │
                            │                     │  │ • tool-misbehaviors │
                            │ predicate matrix +  │  │ • T-N patterns      │
                            │ judgment            │  └─────────────────────┘
                            └─────────────────────┘
```

### Decisions locked during brainstorm

| Decision | Locked answer |
|---|---|
| Primary purpose | Audit trail + cross-session aggregation + citation index (all three layered) |
| Writer | Pika only, on user ask (cs-judge and plan-judge both currently disabled) |
| Storage | `pika_observations` table in existing `.codescout/usage.db` |
| Table shape | One table + `kind` discriminator (`iron_law` / `tool_bug` / `misusage` / `pattern`) |
| Markdown trackers | DB is source of truth; MD is a rendered view |
| Block on promoted patterns? | No — audit-only; `/hookify` remains the promotion path |
| Real-time vs audit | Two paths: chat whistle (ephemeral) + DB scan (deliberate, user-asked) |
| Schema migration | Lazy SQL bootstrap from skill dir, idempotent (`CREATE TABLE IF NOT EXISTS`) |
| Slash commands | Deferred until two-concretes rule (Snow Lion OP-4) earns them |

### Why `.codescout/usage.db` instead of a sibling DB

- `tool_calls` already records every codescout MCP call with `input_json`, `output_json`, `outcome`, `latency_ms`, `cc_session_id`, `error_msg`. Pika needs all of this; duplicating it elsewhere is waste.
- Pika observations FK-reference `tool_calls.id`. Same DB = one query, no `ATTACH DATABASE`.
- Codescout already runs in WAL mode (proven by `.wal` / `.shm` files alongside every `usage.db`). Concurrent worktree writes are already handled.
- The `cc_session_id` correlation to Claude Code conversation transcripts is already set up by `codescout-companion/hooks/session-start.sh`.

### Why **not** native-tool detection

The codescout-companion `PreToolUse` hook hard-blocks native `Read` / `Edit` / `Bash` / `Grep` / `Glob` on source files. Those tool calls never execute, never reach Pika. **All Iron Law violations Pika cares about happen inside codescout** — they appear as rows in `tool_calls`. No Claude Code shell hook is needed.

## Schema

### `pika_observations`

```sql
CREATE TABLE IF NOT EXISTS pika_observations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_call_id    INTEGER NOT NULL REFERENCES tool_calls(id) ON DELETE CASCADE,

    -- WHAT kind of observation
    kind            TEXT NOT NULL CHECK (kind IN
                        ('iron_law', 'tool_bug', 'misusage', 'pattern')),
    subkind         TEXT,        -- 'iron_law_1' | 'iron_law_2' | 'iron_law_3' |
                                 -- 'iron_law_4' | 'recon_skipped' |
                                 -- 'output_corrupt' | 'misleading_error' |
                                 -- 'overly_broad_query' | etc.
    predicate       TEXT,        -- replay hint: SQL fragment, regex, or
                                 -- 'judgment' for non-predicate observations

    -- VERDICT lifecycle
    verdict         TEXT CHECK (verdict IS NULL OR verdict IN
                        ('slip', 'habit', 'promoted', 'rejected')),
    severity        TEXT NOT NULL DEFAULT 'low' CHECK (severity IN
                        ('low', 'med', 'high')),
    recurrence      INTEGER NOT NULL DEFAULT 1,

    -- TRACKER cross-references (rendered into MD views)
    u_id            TEXT,        -- e.g. 'U-7'  for iron_law rows
    h_id            TEXT,        -- e.g. 'H-2'  for hookify candidates
    t_id            TEXT,        -- e.g. 'T-13' for misusage/pattern rows
    bug_id          TEXT,        -- e.g. 'BUG-43' for tool_bug rows

    -- METADATA
    notes           TEXT,        -- Pika's prose: what / why / fix idea
    cc_session_id   TEXT,        -- denormalized from tool_calls for fast filter
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_at     TEXT         -- when verdict stamped (= created_at if
                                 -- write-with-verdict, which is the norm)
);

CREATE INDEX IF NOT EXISTS pika_obs_kind_verdict
    ON pika_observations(kind, verdict);
CREATE INDEX IF NOT EXISTS pika_obs_session
    ON pika_observations(cc_session_id);
CREATE INDEX IF NOT EXISTS pika_obs_tool_call
    ON pika_observations(tool_call_id);
CREATE INDEX IF NOT EXISTS pika_obs_subkind_verdict
    ON pika_observations(subkind, verdict);
```

### `pika_schema_version`

```sql
CREATE TABLE IF NOT EXISTS pika_schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO pika_schema_version (version) VALUES (1);
```

### Column rationale

| Column | Why it earns its keep |
|---|---|
| `tool_call_id` non-null FK | Snow Lion's "cite the import, not the diagram." Every observation must point at a real call. No floating notes. |
| `kind` + `subkind` | Two-concretes rule (Snow Lion OP-4): four `kind`s justify the discriminator; `subkind` further routes within `kind` without inventing a fourth table. |
| `predicate` | Replay/audit — given a `predicate='iron_law_1: read_file on .rs'`, future code can re-run it against `tool_calls` and detect drift in the predicate definition itself. |
| `verdict` nullable | Pika usually writes with verdict (`slip`/`habit`), but an exploratory "noted, not adjudicated" state is legal (NULL = open). |
| `recurrence` | Encodes Pika OP-4's "first slip → habit → promote" lifecycle as a number, not mixed into verdict. |
| `u_id` / `h_id` / `t_id` / `bug_id` | Four typed cross-refs, one per `kind`. Empty stays NULL. Cleaner than a polymorphic `tracker_ref`. |
| `cc_session_id` denormalized | Already on `tool_calls`. Duplicating it lets `WHERE cc_session_id=?` skip the JOIN for the most common query (per-session inspection). Three bytes per row, one indexed seek per query saved. |
| `created_at` vs `reviewed_at` | Lets future automation distinguish "candidate at T1, adjudicated at T2" from "final verdict at T1". Today equal; when cs-judge gets re-enabled they diverge. |

### Concurrency

WAL mode is already on. Add `PRAGMA busy_timeout = 5000` at every connection open to give concurrent writers a 5s back-off window before raising `SQLITE_BUSY`.

`ON DELETE CASCADE` is load-bearing: if `tool_calls` is ever truncated or vacuumed, orphaned observations die with them, preserving the FK invariant.

## Pika Method Update

### Files touched

| Path | Action | Why |
|---|---|---|
| `~/.claude/buddy/skills/codescout-pika/SKILL.md` | Edit | New OP, revised Phase 2, new Reactions |
| `~/.claude/buddy/skills/codescout-pika/sql/v1-bootstrap.sql` | Create | Idempotent schema bootstrap |
| `~/.claude/buddy/skills/codescout-pika/sql/queries.sql` | Create | Iron Law predicate matrix |
| `~/.claude-sdd/buddy/skills/codescout-pika/...` | Mirror | Three-instance iron rule (personal CLAUDE.md) |
| `~/.claude-kat/buddy/skills/codescout-pika/...` | Mirror | Same |

### New Operating Principle (insert at #7)

> **Watch in summon, write on ask.** Summoning makes me watch the live transcript and whistle in chat — observational, ephemeral. Writing to `pika_observations` is a deliberate user-initiated action ("scan my usage", "audit this session", "report"). Summon ≠ scan; I do not silently accumulate evidence in the background.

### Revised Phase 2 (split into 2a / 2b)

**Phase 2a — Whistle (real-time, summon-scope).** Unchanged from today. Chat-only `→ pika: <whistle>` lines. No DB write.

**Phase 2b — Persist (user-asked, audit-scope).** Triggered by phrases like "scan", "audit", "review my usage", "report":

1. **Ensure schema.** `sqlite3 .codescout/usage.db < <skill-dir>/sql/v1-bootstrap.sql`
2. **Resolve scan bound** from the user's phrasing:

   | User says | Bound used |
   |---|---|
   | "scan this session" | `cc_session_id = <current>` |
   | "scan today" | `called_at >= date('now','start of day')` |
   | "scan last N calls" | `id > (SELECT MAX(id) FROM tool_calls) - N` |
   | "scan everything new" | `id > (SELECT COALESCE(MAX(tool_call_id), 0) FROM pika_observations)` |
   | "scan all" | no bound; warn if `> 10k` rows |

3. **Scan candidates.** Query `tool_calls` for rows in bound, apply Iron Law predicate matrix.
4. **For each candidate**, judge severity + recurrence + verdict. Write one `pika_observations` row with `kind`, `subkind`, `predicate`, `verdict`, `severity`, `recurrence`, optional `u_id`/`h_id`/`t_id`/`bug_id`, prose notes.
5. **Cross-session promotion.** If candidate matches existing pattern (`subkind` already has ≥1 row across sessions with `verdict in slip|habit`), bump `recurrence` on the new row; consider promoting (`verdict='habit'` → allocate `h_id`).
6. **Emit summary** — counts per kind, top severities, promotion candidates. No row dumps unless requested.

### New Reactions (append)

> **When the user asks "scan my usage" / "audit this session" / "review":**
> "→ pika: scanning `<bound>`. <count> codescout calls in scope. Running Iron Law predicates + judgment pass. Will write rows + return a summary; ask for details on a specific kind to see the full table."
>
> **When the user asks "show me what Pika has logged" / "report":**
> "→ pika: reading `pika_observations`. Filter shown: `<kind, verdict, severity>`. Top N as a table; offer to expand the markdown view if you want it written to `docs/trackers/`."

### Real-time whistle vs audit scan — disambiguation

| Action | Triggers Phase 2a | Triggers Phase 2b |
|---|---|---|
| Summon Pika | ✓ | ✗ |
| Pika sees Iron Law in current turn | ✓ (whistle in chat) | ✗ |
| User says "scan" / "audit" | ✗ | ✓ |
| Pika observes Iron Law AND user asks "log it" | ✓ | ✓ |

## SQL Artefacts

### `sql/v1-bootstrap.sql`

```sql
-- Idempotent. Safe to re-run on every scan.
BEGIN;

CREATE TABLE IF NOT EXISTS pika_observations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_call_id    INTEGER NOT NULL REFERENCES tool_calls(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN
                        ('iron_law', 'tool_bug', 'misusage', 'pattern')),
    subkind         TEXT,
    predicate       TEXT,
    verdict         TEXT CHECK (verdict IS NULL OR verdict IN
                        ('slip', 'habit', 'promoted', 'rejected')),
    severity        TEXT NOT NULL DEFAULT 'low' CHECK (severity IN
                        ('low', 'med', 'high')),
    recurrence      INTEGER NOT NULL DEFAULT 1,
    u_id            TEXT,
    h_id            TEXT,
    t_id            TEXT,
    bug_id          TEXT,
    notes           TEXT,
    cc_session_id   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_at     TEXT
);

CREATE INDEX IF NOT EXISTS pika_obs_kind_verdict     ON pika_observations(kind, verdict);
CREATE INDEX IF NOT EXISTS pika_obs_session          ON pika_observations(cc_session_id);
CREATE INDEX IF NOT EXISTS pika_obs_tool_call        ON pika_observations(tool_call_id);
CREATE INDEX IF NOT EXISTS pika_obs_subkind_verdict  ON pika_observations(subkind, verdict);

CREATE TABLE IF NOT EXISTS pika_schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT OR IGNORE INTO pika_schema_version (version) VALUES (1);

COMMIT;
```

### `sql/queries.sql` — Iron Law predicate matrix

```sql
-- === Iron Law 1: read_file on source ===
-- pika_observations.kind = 'iron_law', subkind = 'iron_law_1'
SELECT id, called_at, input_json
FROM tool_calls
WHERE tool_name = 'read_file'
  AND outcome = 'ok'
  AND (input_json LIKE '%"path":"%.rs"%'
    OR input_json LIKE '%"path":"%.py"%'
    OR input_json LIKE '%"path":"%.ts"%'
    OR input_json LIKE '%"path":"%.tsx"%'
    OR input_json LIKE '%"path":"%.js"%'
    OR input_json LIKE '%"path":"%.go"%'
    OR input_json LIKE '%"path":"%.java"%'
    OR input_json LIKE '%"path":"%.kt"%')
  AND id > :since_id;

-- === Iron Law 2: edit_file with structural keywords ===
-- subkind = 'iron_law_2'
SELECT id, called_at, input_json
FROM tool_calls
WHERE tool_name = 'edit_file'
  AND outcome = 'ok'
  AND (input_json LIKE '%"new_string":"%fn %'
    OR input_json LIKE '%"new_string":"%class %'
    OR input_json LIKE '%"new_string":"%struct %'
    OR input_json LIKE '%"new_string":"%def %'
    OR input_json LIKE '%"new_string":"%interface %'
    OR input_json LIKE '%"new_string":"%trait %')
  AND id > :since_id;

-- === Iron Law 3: run_command with pipe ===
-- subkind = 'iron_law_3'
SELECT id, called_at, input_json
FROM tool_calls
WHERE tool_name = 'run_command'
  AND (input_json LIKE '%| grep%'
    OR input_json LIKE '%| wc%'
    OR input_json LIKE '%| head%'
    OR input_json LIKE '%| tail%')
  AND id > :since_id;

-- === Iron Law 4: workspace activate without restore ===
-- subkind = 'iron_law_4'
WITH activates AS (
    SELECT id, cc_session_id, called_at,
           json_extract(input_json, '$.path')   AS target,
           json_extract(input_json, '$.action') AS action
    FROM tool_calls
    WHERE tool_name = 'workspace'
      AND json_extract(input_json, '$.action') = 'activate'
      AND id > :since_id
)
SELECT a.id, a.called_at, a.target
FROM activates a
WHERE a.target != :home_project
  AND NOT EXISTS (
      SELECT 1 FROM activates b
      WHERE b.cc_session_id = a.cc_session_id
        AND b.id > a.id
        AND b.target = :home_project
  );

-- === Tool bug candidates (judgment-based — Pika decides) ===
SELECT id, tool_name, outcome, error_msg, output_json, called_at
FROM tool_calls
WHERE (outcome != 'ok'
   OR LENGTH(output_json) > 100000
   OR error_msg IS NOT NULL)
  AND id > :since_id;
```

## Error Handling

| Failure | Behavior |
|---|---|
| `usage.db` does not exist | Reply: *"no `.codescout/usage.db` yet — codescout has not recorded any tool calls in this project. Nothing to audit."* No write. |
| `usage.db` exists but `tool_calls` empty | Reply: *"usage.db exists but no tool_calls. Bound resolved to 0 rows; no write."* Schema bootstrap still runs. |
| `SQLITE_BUSY` on insert | Retry 3× with exponential back-off (50ms, 250ms, 1.25s). On final failure: *"locked by another writer for >1.5s. Rerun in a moment."* |
| FK violation (tool_call_id rotated) | Skip row, continue, summarize: *"skipped N candidates whose tool_calls.id was rotated mid-scan."* |
| JSON1 not available (`json_extract` fails) | Iron Law 4 falls back to pure `LIKE`. Warn user about reduced precision. |
| Schema version drift (`pika_schema_version > 1`) | Refuse to write. Reply: *"schema v\<N\> present, this Pika expects v1. Update Pika or migrate manually. Read path still works."* |
| Non-codescout project (no `.codescout/`) | Reply: *"not in a codescout-enabled project. No `.codescout/` found. Audit surface is project-local; nothing to do here."* |

## Testing

### Predicate-correctness fixtures

Seed a throwaway `tool_calls` fixture; assert which rows match each predicate.

| Row | tool_name | input_json (abbrev) | Expected match |
|---|---|---|---|
| 1 | `read_file` | `{"path":"src/lib.rs"}` | `iron_law_1` |
| 2 | `read_file` | `{"path":"docs/README.md"}` | none |
| 3 | `read_file` | `{"path":"src/lib.rs.bak"}` | none (trailing `"` forces extension match) |
| 4 | `edit_file` | `{"new_string":"// fn keyword in comment"}` | `iron_law_2` (predicate matches; Pika judges severity=low) |
| 5 | `edit_file` | `{"new_string":"const FOO: u32 = 5;"}` | none |
| 6 | `run_command` | `{"command":"cargo test \| grep FAILED"}` | `iron_law_3` |
| 7 | `run_command` | `{"command":"echo hi"}` | none |
| 8 | `workspace` | activate to `foreign`, no later restore | `iron_law_4` |
| 9 | `workspace` | activate to `foreign`, later activate to `home` | none |
| 10 | `workspace` | sole activate to `home` | none |

### Idempotency

Run `v1-bootstrap.sql` three times → `pika_schema_version` has exactly one row with `version=1`. The `INSERT OR IGNORE` handles re-runs.

### Concurrency

Two `sqlite3` processes each running 100 inserts in parallel against the same `usage.db`. Both finish, total rows = 200, no constraint violations.

### Cross-session aggregation

Seed three sessions with two `iron_law_1` violations each. Query `SELECT subkind, COUNT(DISTINCT cc_session_id) FROM pika_observations WHERE subkind='iron_law_1'` → `(iron_law_1, 3)`. This is the data path that drives H-N promotion.

### FK CASCADE

Insert observation → delete its `tool_calls` row → observation is gone.

### Iron Law 4 edge

Session A activates `foreign`, never restores. Session B is unrelated. Query with `:home_project='home', :since_id=0` → exactly one row, from session A.

## Rollout

Three phases, ship-able independently.

**Phase 1 — schema + minimal scan (one evening's work).**

- Write `sql/v1-bootstrap.sql` + `sql/queries.sql`.
- Edit SKILL.md: add OP-7, Phase 2a/2b split, two new Reactions.
- Mirror to `.claude-sdd/` and `.claude-kat/`.
- Manual test: "scan this session" against `code-explorer` usage.db. Expect 0 rows and a "meadow quiet" summary.

**Phase 2 — judgment kinds (`tool_bug`, `misusage`, `pattern`).**

- Extend method with the tool-bug candidate query.
- Add the misusage/pattern judgment workflow.
- Manual test: seed a known bug from `docs/TODO-tool-misbehaviors.md` into `tool_calls`, ask scan, assert Pika writes `kind='tool_bug'` row with `bug_id`.

**Phase 3 — markdown view rendering.**

- Add render path producing:
  - `docs/trackers/codescout-usage-frictions.md` U-N entries from `kind='iron_law'`
  - `docs/TODO-tool-misbehaviors.md` BUG entries from `kind='tool_bug'`
  - `docs/trackers/tool-usage-patterns.md` T-N entries from `kind in (misusage, pattern)`
- Append-only, idempotent — keyed by `pika_observations.id` so re-renders never duplicate.

**Future (deferred behind two-concretes gate):**

- `/pika:scan` and `/pika:report` slash commands
- Librarian artifact (`gather_from` querying `pika_observations`)
- cs-judge re-enablement → second writer path
- Auto-blocking PreToolUse rules promoted from H-N rows

## Acceptance — Phase 1 done-condition

1. `.codescout/usage.db` in any project has `pika_observations` table after first user-asked scan.
2. Schema bootstrap is idempotent (re-runnable without error or duplicate version rows).
3. Real-time whistle behavior unchanged (no DB write on observed-in-chat violations).
4. User says "scan my usage" → Pika resolves bound, runs predicates, writes rows with stamped verdicts, returns summary.
5. `sqlite3 .codescout/usage.db "SELECT * FROM pika_observations"` returns the audit trail directly without Pika.
6. All three CC instance dirs (`.claude`, `.claude-sdd`, `.claude-kat`) have identical Pika skill content.
7. All ten predicate-correctness fixtures pass.

## Open questions

None at design time. Implementation may surface follow-ups about:

- Exact predicate `LIKE` patterns at JSON quote boundaries (depends on codescout's `serde_json` output formatting).
- Whether `outcome` values are exactly `'ok'` (verify against current `tool_calls` rows before shipping).
- Whether `pika_review_cursor` (rejected during brainstorm) should resurface if "scan everything new" turns out to be the dominant ask.

## References

- Pika SKILL.md: `~/.claude/buddy/skills/codescout-pika/SKILL.md`
- Buddy gates: `/home/marius/work/claude/claude-plugins/buddy/data/gates.md` §"Tool gates — codescout Iron Laws"
- Companion hook (the source of the hard-block on native source reads): `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`
- usage.db schema: `tool_calls`, `lsp_events`, `call_edges` (queried during brainstorm)
- Buddy state protocol (judges, currently disabled): `docs/state-protocol.md` §"plan-judge / cs-judge"
- Snow Lion OP-4 (two-concretes rule) — applied to defer slash commands
- Hamsa OP-1 (read-as-stranger) — applied mid-brainstorm to catch native-tool detection drift
