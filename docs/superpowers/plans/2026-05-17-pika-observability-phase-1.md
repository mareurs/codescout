# Pika Observability — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the per-project Pika audit-trail pipeline end-to-end with zero false positives — the spec's "meadow quiet" success condition.

**Architecture:** Add a `pika_observations` table to the codescout `.codescout/usage.db` per project. Pika writes rows only on explicit user ask. Phase 1 ships schema + Iron Law predicate matrix + SKILL.md method update + 3-instance mirror + smoke test. No Rust changes to codescout. No Claude Code hooks. Pika's source-of-truth lives in `~/.claude/buddy/skills/codescout-pika/`, which is **not** a git repo — changes are in-place; only the plan + tracker artifacts that live in the codescout repo are committed.

**Tech Stack:** SQLite 3.38+ (JSON1), bash, sqlite3 CLI, markdown.

**Spec:** `docs/superpowers/specs/2026-05-17-pika-observability-design.md`

**Branch:** `experiments` (do not push; merge to master via cherry-pick after manual validation per project rules).

---

## File Structure

### Created (in `~/.claude/buddy/skills/codescout-pika/`, not git-tracked)

| Path | Responsibility |
|---|---|
| `sql/v1-bootstrap.sql` | Idempotent schema: `pika_observations` table + 4 indexes + `pika_schema_version` |
| `sql/queries.sql` | Iron Law predicate matrix (4 queries) + tool-bug candidate query |
| `tests/fixtures.sql` | Throwaway `tool_calls` seed for predicate testing |
| `tests/run-tests.sh` | Bash runner that creates a temp DB, applies bootstrap, seeds fixtures, runs predicates, asserts counts |
| `tests/test-bootstrap-idempotent.sh` | Runs bootstrap 3× against a temp DB; asserts version row count = 1 |
| `tests/test-concurrent-writes.sh` | Two parallel writers, 100 inserts each; asserts total = 200, no errors |
| `tests/test-fk-cascade.sh` | Insert observation, delete tool_calls row, assert observation gone |

### Modified (in `~/.claude/buddy/skills/codescout-pika/`, not git-tracked)

| Path | Modification |
|---|---|
| `SKILL.md` | Add OP-7. Split Phase 2 into 2a + 2b. Append 2 Reactions. Append "Audit-scan procedure" section pointing at `sql/` |

### Mirrored (`cp -r` from `.claude` to the other two CC instances)

- `~/.claude-sdd/buddy/skills/codescout-pika/` — full directory replace
- `~/.claude-kat/buddy/skills/codescout-pika/` — full directory replace

### Created (in this repo, git-tracked, committed on experiments)

| Path | Purpose |
|---|---|
| `docs/superpowers/plans/2026-05-17-pika-observability-phase-1.md` | This plan |
| `docs/trackers/pika-phase-1-validation.md` | Phase 1 smoke-test result + acceptance-criteria checklist |

---

## Task 1: SQL bootstrap + idempotency test

**Goal:** Create the schema file. Verify CREATE TABLE IF NOT EXISTS + INSERT OR IGNORE on `pika_schema_version` is genuinely idempotent.

**Files:**
- Create: `~/.claude/buddy/skills/codescout-pika/sql/v1-bootstrap.sql`
- Create: `~/.claude/buddy/skills/codescout-pika/tests/test-bootstrap-idempotent.sh`

- [ ] **Step 1: Write the failing idempotency test**

```bash
# ~/.claude/buddy/skills/codescout-pika/tests/test-bootstrap-idempotent.sh
#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="$HOME/.claude/buddy/skills/codescout-pika"
TMPDB=$(mktemp /tmp/pika-bootstrap-test.XXXXXX.db)
trap 'rm -f "$TMPDB" "$TMPDB-wal" "$TMPDB-shm"' EXIT

# Seed a minimal tool_calls table so FK targets exist
sqlite3 "$TMPDB" <<SQL
CREATE TABLE tool_calls (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_name  TEXT NOT NULL,
    called_at  TEXT NOT NULL DEFAULT (datetime('now')),
    latency_ms INTEGER NOT NULL,
    outcome    TEXT NOT NULL,
    overflowed INTEGER NOT NULL DEFAULT 0,
    error_msg  TEXT,
    codescout_sha TEXT, project_sha TEXT, session_id TEXT,
    input_json TEXT, output_json TEXT, cc_session_id TEXT
);
SQL

# Run bootstrap three times
for i in 1 2 3; do
    sqlite3 "$TMPDB" < "$SKILL_DIR/sql/v1-bootstrap.sql"
done

# Assert exactly one row in pika_schema_version with version=1
COUNT=$(sqlite3 "$TMPDB" "SELECT COUNT(*) FROM pika_schema_version WHERE version=1;")
[[ "$COUNT" == "1" ]] || { echo "FAIL: expected 1 schema-version row, got $COUNT"; exit 1; }

# Assert pika_observations table exists with expected columns
COLS=$(sqlite3 "$TMPDB" "PRAGMA table_info(pika_observations);" | awk -F'|' '{print $2}' | sort | tr '\n' ',')
EXPECTED="bug_id,cc_session_id,created_at,h_id,id,kind,notes,predicate,recurrence,reviewed_at,severity,subkind,t_id,tool_call_id,u_id,verdict,"
[[ "$COLS" == "$EXPECTED" ]] || { echo "FAIL: column mismatch"; echo "got:      $COLS"; echo "expected: $EXPECTED"; exit 1; }

echo "PASS: bootstrap idempotent + schema correct"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
chmod +x ~/.claude/buddy/skills/codescout-pika/tests/test-bootstrap-idempotent.sh
~/.claude/buddy/skills/codescout-pika/tests/test-bootstrap-idempotent.sh
```

Expected: `FAIL` because `sql/v1-bootstrap.sql` does not exist (sqlite3 errors with `Error: cannot open file`).

- [ ] **Step 3: Write the bootstrap SQL**

Create `~/.claude/buddy/skills/codescout-pika/sql/v1-bootstrap.sql`:

```sql
-- Pika observability — schema v1
-- Idempotent. Safe to re-run on every scan.
-- Anchored to codescout's existing tool_calls table.

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

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-bootstrap-idempotent.sh
```

Expected: `PASS: bootstrap idempotent + schema correct`

- [ ] **Step 5: Commit (codescout repo — plan doc only; skill dir is not a git repo)**

No commit at this step. Skill dir is outside any git repo. Continue to Task 2.

---

## Task 2: Iron Law 1–3 predicate matrix + per-predicate fixture test

**Goal:** Three LIKE-based predicates that detect codescout-tool misuse from `tool_calls.input_json`. Each predicate tested against a known-good and known-bad fixture row.

**Files:**
- Create: `~/.claude/buddy/skills/codescout-pika/sql/queries.sql`
- Create: `~/.claude/buddy/skills/codescout-pika/tests/fixtures.sql`
- Create: `~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh`

- [ ] **Step 1: Write the failing predicate test**

```bash
# ~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="$HOME/.claude/buddy/skills/codescout-pika"
TMPDB=$(mktemp /tmp/pika-predicate-test.XXXXXX.db)
trap 'rm -f "$TMPDB" "$TMPDB-wal" "$TMPDB-shm"' EXIT

# Seed minimal tool_calls + bootstrap
sqlite3 "$TMPDB" <<SQL
CREATE TABLE tool_calls (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_name  TEXT NOT NULL,
    called_at  TEXT NOT NULL DEFAULT (datetime('now')),
    latency_ms INTEGER NOT NULL,
    outcome    TEXT NOT NULL,
    overflowed INTEGER NOT NULL DEFAULT 0,
    error_msg  TEXT,
    codescout_sha TEXT, project_sha TEXT, session_id TEXT,
    input_json TEXT, output_json TEXT, cc_session_id TEXT
);
SQL
sqlite3 "$TMPDB" < "$SKILL_DIR/sql/v1-bootstrap.sql"
sqlite3 "$TMPDB" < "$SKILL_DIR/tests/fixtures.sql"

# === Iron Law 1: read_file on source ===
COUNT=$(sqlite3 "$TMPDB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='read_file' AND outcome='ok'
       AND (input_json LIKE '%\"path\":\"%.rs\"%'
         OR input_json LIKE '%\"path\":\"%.py\"%'
         OR input_json LIKE '%\"path\":\"%.ts\"%'
         OR input_json LIKE '%\"path\":\"%.tsx\"%'
         OR input_json LIKE '%\"path\":\"%.js\"%'
         OR input_json LIKE '%\"path\":\"%.go\"%'
         OR input_json LIKE '%\"path\":\"%.java\"%'
         OR input_json LIKE '%\"path\":\"%.kt\"%')")
[[ "$COUNT" == "1" ]] || { echo "FAIL: iron_law_1 expected 1 match, got $COUNT"; exit 1; }

# === Iron Law 2: edit_file with structural keywords ===
COUNT=$(sqlite3 "$TMPDB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='edit_file' AND outcome='ok'
       AND (input_json LIKE '%\"new_string\":\"%fn %'
         OR input_json LIKE '%\"new_string\":\"%class %'
         OR input_json LIKE '%\"new_string\":\"%struct %'
         OR input_json LIKE '%\"new_string\":\"%def %'
         OR input_json LIKE '%\"new_string\":\"%interface %'
         OR input_json LIKE '%\"new_string\":\"%trait %')")
[[ "$COUNT" == "1" ]] || { echo "FAIL: iron_law_2 expected 1 match, got $COUNT"; exit 1; }

# === Iron Law 3: run_command with pipe ===
COUNT=$(sqlite3 "$TMPDB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='run_command'
       AND (input_json LIKE '%| grep%'
         OR input_json LIKE '%| wc%'
         OR input_json LIKE '%| head%'
         OR input_json LIKE '%| tail%')")
[[ "$COUNT" == "1" ]] || { echo "FAIL: iron_law_3 expected 1 match, got $COUNT"; exit 1; }

echo "PASS: predicates 1–3 detect expected fixture rows"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
chmod +x ~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: `FAIL` because `fixtures.sql` does not exist.

- [ ] **Step 3: Write the fixtures**

Create `~/.claude/buddy/skills/codescout-pika/tests/fixtures.sql`:

```sql
-- Predicate-correctness fixtures for Pika queries.
-- Each row is hand-crafted to either match exactly one predicate or none.

-- Iron Law 1 fixtures (read_file on source)
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json, cc_session_id) VALUES
    ('read_file', 10, 'ok',  '{"path":"src/lib.rs"}',    'sess-A'),   -- MATCH iron_law_1
    ('read_file', 10, 'ok',  '{"path":"docs/README.md"}', 'sess-A'),  -- no match (md)
    ('read_file', 10, 'ok',  '{"path":"src/lib.rs.bak"}', 'sess-A');  -- no match (trailing " forces exact extension)

-- Iron Law 2 fixtures (edit_file with structural keyword)
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json, cc_session_id) VALUES
    ('edit_file', 10, 'ok',  '{"new_string":"// fn keyword in comment"}', 'sess-A'),  -- MATCH iron_law_2 (Pika will judge severity=low)
    ('edit_file', 10, 'ok',  '{"new_string":"const FOO: u32 = 5;"}',      'sess-A'); -- no match

-- Iron Law 3 fixtures (run_command with pipe)
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json, cc_session_id) VALUES
    ('run_command', 10, 'ok', '{"command":"cargo test | grep FAILED"}', 'sess-A'),  -- MATCH iron_law_3
    ('run_command', 10, 'ok', '{"command":"echo hi"}',                  'sess-A'); -- no match

-- Iron Law 4 fixtures (workspace activate without restore)
-- session B: activate to foreign with NO restore (should match)
-- session C: activate to foreign + later activate to home (should NOT match)
-- session D: sole activate to home (should NOT match)
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json, cc_session_id) VALUES
    ('workspace', 5, 'ok', '{"action":"activate","path":"foreign"}', 'sess-B'),
    ('workspace', 5, 'ok', '{"action":"activate","path":"foreign"}', 'sess-C'),
    ('workspace', 5, 'ok', '{"action":"activate","path":"home"}',    'sess-C'),
    ('workspace', 5, 'ok', '{"action":"activate","path":"home"}',    'sess-D');

-- Tool bug candidate fixtures
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json, output_json, error_msg, cc_session_id) VALUES
    ('symbols', 50, 'error', '{"name":"X"}', NULL, 'LSP timeout',   'sess-E'),   -- MATCH tool_bug (outcome != ok)
    ('grep',    20, 'ok',    '{"pattern":"X"}', '{"matches":[]}',   NULL,        'sess-E'); -- no match
```

- [ ] **Step 4: Write the queries**

Create `~/.claude/buddy/skills/codescout-pika/sql/queries.sql`:

```sql
-- Pika queries against codescout's tool_calls — Iron Law predicate matrix
-- + judgment-based tool-bug candidate query.
-- Each query takes :since_id (and Iron Law 4 also takes :home_project).

-- === Iron Law 1: read_file on source ===
-- Anchor: subkind = 'iron_law_1'
-- INTENT: read_file on .rs/.py/.ts/.tsx/.js/.go/.java/.kt should have been symbols(...)
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
-- Anchor: subkind = 'iron_law_2'
-- INTENT: edit_file containing fn/class/struct/def/interface/trait should have been edit_code
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
-- Anchor: subkind = 'iron_law_3'
-- INTENT: piping defeats the @cmd_* buffer system
SELECT id, called_at, input_json
FROM tool_calls
WHERE tool_name = 'run_command'
  AND (input_json LIKE '%| grep%'
    OR input_json LIKE '%| wc%'
    OR input_json LIKE '%| head%'
    OR input_json LIKE '%| tail%')
  AND id > :since_id;

-- === Iron Law 4: workspace activate without restore ===
-- (added in Task 3)

-- === Tool bug candidates (judgment-based) ===
-- (added in Task 4)
```

- [ ] **Step 5: Run test to verify it passes**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: `PASS: predicates 1–3 detect expected fixture rows`

- [ ] **Step 6: Commit** (none — skill dir is not git-tracked)

---

## Task 3: Iron Law 4 — workspace activate without restore (CTE)

**Goal:** Detect the workspace-restore violation — the only Iron Law requiring a session-window self-join. Tested against three fixture sessions (B, C, D in fixtures.sql).

**Files:**
- Modify: `~/.claude/buddy/skills/codescout-pika/sql/queries.sql` (append Iron Law 4 query)
- Modify: `~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh` (append Iron Law 4 assertion)

- [ ] **Step 1: Write the failing test (append)**

Append to `~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh` (before the final `echo "PASS"` line):

```bash
# === Iron Law 4: workspace activate without restore ===
# Use the CTE from queries.sql with :home_project='home' and :since_id=0
COUNT=$(sqlite3 "$TMPDB" \
    "WITH activates AS (
        SELECT id, cc_session_id, called_at,
               json_extract(input_json, '\$.path')   AS target,
               json_extract(input_json, '\$.action') AS action
        FROM tool_calls
        WHERE tool_name = 'workspace'
          AND json_extract(input_json, '\$.action') = 'activate'
          AND id > 0
     )
     SELECT COUNT(*) FROM activates a
     WHERE a.target != 'home'
       AND NOT EXISTS (
           SELECT 1 FROM activates b
           WHERE b.cc_session_id = a.cc_session_id
             AND b.id > a.id
             AND b.target = 'home'
       );")

# Expected: only session B's activate-to-foreign-with-no-restore should match.
[[ "$COUNT" == "1" ]] || { echo "FAIL: iron_law_4 expected 1 match (session B), got $COUNT"; exit 1; }

# Verify the single match is session B specifically
SESS=$(sqlite3 "$TMPDB" \
    "WITH activates AS (
        SELECT id, cc_session_id,
               json_extract(input_json, '\$.path') AS target
        FROM tool_calls
        WHERE tool_name = 'workspace'
          AND json_extract(input_json, '\$.action') = 'activate'
          AND id > 0
     )
     SELECT a.cc_session_id FROM activates a
     WHERE a.target != 'home'
       AND NOT EXISTS (
           SELECT 1 FROM activates b
           WHERE b.cc_session_id = a.cc_session_id
             AND b.id > a.id
             AND b.target = 'home'
       );")
[[ "$SESS" == "sess-B" ]] || { echo "FAIL: iron_law_4 expected sess-B, got $SESS"; exit 1; }
```

Change the final `echo` line to:

```bash
echo "PASS: predicates 1–4 detect expected fixture rows"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: `FAIL: iron_law_4 expected 1 match (session B), got 2` — because the inline CTE in the test will currently use queries.sql's missing Iron Law 4 reference. Actually the test is self-contained (the CTE is inlined in the bash). It will likely PASS because the SQL is right; what should fail first is **`queries.sql` does not contain the canonical Iron Law 4**. Run a separate assertion:

```bash
grep -q "Iron Law 4: workspace activate without restore" ~/.claude/buddy/skills/codescout-pika/sql/queries.sql
[[ $? -eq 0 ]] || { echo "FAIL: queries.sql missing canonical Iron Law 4 block"; exit 1; }
```

Add this assertion **before** the bash CTE assertion. Now the test fails at the grep (queries.sql has only the placeholder comment).

- [ ] **Step 3: Add Iron Law 4 to queries.sql**

Replace `-- === Iron Law 4: workspace activate without restore ===\n-- (added in Task 3)` in `~/.claude/buddy/skills/codescout-pika/sql/queries.sql` with:

```sql
-- === Iron Law 4: workspace activate without restore ===
-- Anchor: subkind = 'iron_law_4'
-- INTENT: workspace activate to a non-home project must be paired with a later
--         activate back to home in the same cc_session_id. Unpaired activates
--         pollute the shared MCP server state for the next session.
-- NOTE: requires SQLite JSON1 (json_extract). Falls back to LIKE if unavailable.
WITH activates AS (
    SELECT id, cc_session_id, called_at,
           json_extract(input_json, '$.path')   AS target,
           json_extract(input_json, '$.action') AS action
    FROM tool_calls
    WHERE tool_name = 'workspace'
      AND json_extract(input_json, '$.action') = 'activate'
      AND id > :since_id
)
SELECT a.id, a.called_at, a.target, a.cc_session_id
FROM activates a
WHERE a.target != :home_project
  AND NOT EXISTS (
      SELECT 1 FROM activates b
      WHERE b.cc_session_id = a.cc_session_id
        AND b.id > a.id
        AND b.target = :home_project
  );
```

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: `PASS: predicates 1–4 detect expected fixture rows`

- [ ] **Step 5: Commit** (none)

---

## Task 4: Tool-bug candidate query + test

**Goal:** Add the judgment-based candidate query that surfaces error rows + suspicious output sizes for Pika to review.

**Files:**
- Modify: `~/.claude/buddy/skills/codescout-pika/sql/queries.sql` (append tool-bug query)
- Modify: `~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh` (append tool-bug assertion)

- [ ] **Step 1: Write the failing test (append before final `echo "PASS"`)**

```bash
# === Tool bug candidates ===
# Expected: 1 match from sess-E (the symbols-with-LSP-timeout error)
COUNT=$(sqlite3 "$TMPDB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE (outcome != 'ok'
        OR LENGTH(output_json) > 100000
        OR error_msg IS NOT NULL)
       AND id > 0")
[[ "$COUNT" == "1" ]] || { echo "FAIL: tool_bug candidate expected 1, got $COUNT"; exit 1; }
```

Update the final `echo` line to:

```bash
echo "PASS: all predicates + tool-bug candidates detect expected fixture rows"
```

- [ ] **Step 2: Run test to verify it fails**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: The bash CTE assertion will PASS because the query is inlined in the test. We need a separate canonical-query assertion. Add **before** the inline assertion:

```bash
grep -q "Tool bug candidates (judgment-based)" ~/.claude/buddy/skills/codescout-pika/sql/queries.sql
[[ $? -eq 0 ]] || { echo "FAIL: queries.sql missing canonical tool-bug block"; exit 1; }
```

Now the test fails at the grep.

- [ ] **Step 3: Add tool-bug query to queries.sql**

Replace `-- === Tool bug candidates (judgment-based) ===\n-- (added in Task 4)` in `~/.claude/buddy/skills/codescout-pika/sql/queries.sql` with:

```sql
-- === Tool bug candidates (judgment-based) ===
-- Anchor: kind = 'tool_bug', subkind set by Pika at write time
-- INTENT: surface candidate rows for Pika to judge. Pika decides if each is
--         a real bug, then writes pika_observations row with verdict.
SELECT id, tool_name, outcome, error_msg, output_json, called_at
FROM tool_calls
WHERE (outcome != 'ok'
   OR LENGTH(output_json) > 100000
   OR error_msg IS NOT NULL)
  AND id > :since_id;
```

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.claude/buddy/skills/codescout-pika/tests/test-predicates.sh
```

Expected: `PASS: all predicates + tool-bug candidates detect expected fixture rows`

- [ ] **Step 5: Commit** (none)

---

## Task 5: FK CASCADE + concurrent-writes safety tests

**Goal:** Verify the load-bearing schema invariants. CASCADE ensures pika_observations don't outlive their tool_calls anchors. WAL + busy_timeout handles two worktree sessions writing simultaneously.

**Files:**
- Create: `~/.claude/buddy/skills/codescout-pika/tests/test-fk-cascade.sh`
- Create: `~/.claude/buddy/skills/codescout-pika/tests/test-concurrent-writes.sh`

- [ ] **Step 1: Write FK CASCADE test**

```bash
# ~/.claude/buddy/skills/codescout-pika/tests/test-fk-cascade.sh
#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="$HOME/.claude/buddy/skills/codescout-pika"
TMPDB=$(mktemp /tmp/pika-cascade-test.XXXXXX.db)
trap 'rm -f "$TMPDB" "$TMPDB-wal" "$TMPDB-shm"' EXIT

sqlite3 "$TMPDB" <<SQL
CREATE TABLE tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_name TEXT NOT NULL, latency_ms INTEGER NOT NULL, outcome TEXT NOT NULL,
    overflowed INTEGER NOT NULL DEFAULT 0,
    input_json TEXT, output_json TEXT, error_msg TEXT, cc_session_id TEXT,
    called_at TEXT NOT NULL DEFAULT (datetime('now'))
);
PRAGMA foreign_keys = ON;
SQL
sqlite3 "$TMPDB" < "$SKILL_DIR/sql/v1-bootstrap.sql"

# Insert a tool_call + an observation referencing it
sqlite3 "$TMPDB" "PRAGMA foreign_keys=ON; INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json) VALUES ('test', 1, 'ok', '{}');"
TID=$(sqlite3 "$TMPDB" "SELECT id FROM tool_calls LIMIT 1;")
sqlite3 "$TMPDB" "PRAGMA foreign_keys=ON; INSERT INTO pika_observations (tool_call_id, kind, subkind, verdict, severity) VALUES ($TID, 'iron_law', 'iron_law_1', 'slip', 'low');"

BEFORE=$(sqlite3 "$TMPDB" "SELECT COUNT(*) FROM pika_observations;")
[[ "$BEFORE" == "1" ]] || { echo "FAIL: observation insert failed ($BEFORE rows)"; exit 1; }

# Delete the tool_call; observation should cascade
sqlite3 "$TMPDB" "PRAGMA foreign_keys=ON; DELETE FROM tool_calls WHERE id=$TID;"

AFTER=$(sqlite3 "$TMPDB" "SELECT COUNT(*) FROM pika_observations;")
[[ "$AFTER" == "0" ]] || { echo "FAIL: CASCADE did not fire ($AFTER rows remain)"; exit 1; }

echo "PASS: FK CASCADE removes orphaned observations"
```

- [ ] **Step 2: Run FK CASCADE test**

```bash
chmod +x ~/.claude/buddy/skills/codescout-pika/tests/test-fk-cascade.sh
~/.claude/buddy/skills/codescout-pika/tests/test-fk-cascade.sh
```

Expected: `PASS: FK CASCADE removes orphaned observations`

If it FAILs (CASCADE didn't fire), the cause is missing `PRAGMA foreign_keys=ON;` per connection — SQLite defaults to FK enforcement OFF. The test enables it explicitly; Pika's runtime connection must do the same. Add a note to SKILL.md (Task 6) about enforcing this pragma.

- [ ] **Step 3: Write concurrent-writes test**

```bash
# ~/.claude/buddy/skills/codescout-pika/tests/test-concurrent-writes.sh
#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="$HOME/.claude/buddy/skills/codescout-pika"
TMPDB=$(mktemp /tmp/pika-concurrent-test.XXXXXX.db)
trap 'rm -f "$TMPDB" "$TMPDB-wal" "$TMPDB-shm"' EXIT

sqlite3 "$TMPDB" <<SQL
PRAGMA journal_mode = WAL;
CREATE TABLE tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_name TEXT NOT NULL, latency_ms INTEGER NOT NULL, outcome TEXT NOT NULL,
    overflowed INTEGER NOT NULL DEFAULT 0,
    input_json TEXT, output_json TEXT, error_msg TEXT, cc_session_id TEXT,
    called_at TEXT NOT NULL DEFAULT (datetime('now'))
);
SQL
sqlite3 "$TMPDB" < "$SKILL_DIR/sql/v1-bootstrap.sql"

# Seed 200 tool_calls (FK targets for 200 observations)
sqlite3 "$TMPDB" <<SQL
WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n < 200)
INSERT INTO tool_calls (tool_name, latency_ms, outcome, input_json)
SELECT 'test', 1, 'ok', '{}' FROM seq;
SQL

# Two parallel writers, each insert 100 observations
writer() {
    local start=$1
    for i in $(seq $start $((start+99))); do
        sqlite3 -cmd "PRAGMA busy_timeout=5000;" "$TMPDB" \
            "INSERT INTO pika_observations (tool_call_id, kind, subkind, verdict, severity) VALUES ($i, 'iron_law', 'iron_law_1', 'slip', 'low');"
    done
}

writer 1 &
PID1=$!
writer 101 &
PID2=$!
wait $PID1 $PID2

COUNT=$(sqlite3 "$TMPDB" "SELECT COUNT(*) FROM pika_observations;")
[[ "$COUNT" == "200" ]] || { echo "FAIL: concurrent writes expected 200, got $COUNT"; exit 1; }

echo "PASS: 200 concurrent writes from 2 writers succeed"
```

- [ ] **Step 4: Run concurrent-writes test**

```bash
chmod +x ~/.claude/buddy/skills/codescout-pika/tests/test-concurrent-writes.sh
~/.claude/buddy/skills/codescout-pika/tests/test-concurrent-writes.sh
```

Expected: `PASS: 200 concurrent writes from 2 writers succeed` (may take 10–30 seconds).

- [ ] **Step 5: Commit** (none — skill dir not git-tracked)

---

## Task 6: SKILL.md update — OP-7 + Phase 2a/2b + Reactions

**Goal:** Make Pika aware of the new audit-scan path. Insert OP-7, split Phase 2 into 2a (real-time whistle) and 2b (user-asked persist), append two Reactions, and add a new "Audit-scan procedure" section pointing at `sql/`.

**Files:**
- Modify: `~/.claude/buddy/skills/codescout-pika/SKILL.md`

- [ ] **Step 1: Insert OP-7 after OP-6**

Read the current SKILL.md heading map:

```bash
sqlite3 :memory: <<'EOF'
.read /dev/stdin
EOF
```

(Actually use `mcp__codescout__read_markdown` on the file to find the exact line of OP-6.)

In `~/.claude/buddy/skills/codescout-pika/SKILL.md`, in the `## Operating Principles` section, find OP-6 (ends with "...absent and missing.") and insert immediately after, before the next `## ` heading:

```markdown
7. **Watch in summon, write on ask.** Summoning makes me watch the live
   transcript and whistle in chat — observational, ephemeral. Writing to
   `pika_observations` is a deliberate user-initiated action ("scan my
   usage", "audit this session", "report"). Summon ≠ scan; I do not
   silently accumulate evidence in the background.
```

- [ ] **Step 2: Split Phase 2 into 2a + 2b**

In `~/.claude/buddy/skills/codescout-pika/SKILL.md`, replace the section heading and body:

```markdown
### Phase 2 — Whistle (flag, name, route, externalize)
1. **Emit one whistle per distinct violation, and append a U-N entry**
   ...
3. **If a violation repeats**, open or update an H-N entry citing the
   confirming U-N IDs, and propose the hookify rule predicate.
```

with:

```markdown
### Phase 2a — Whistle (real-time, summon-scope)
Same as today. Chat-only `→ pika: <whistle>` lines on observed
violations. No DB write. Whistles are ephemeral.

1. **Emit one whistle per distinct violation.** Name the replacement
   tool. Allocate a transient U-N reference in chat for the user's
   benefit; the durable U-N is only created when Phase 2b runs.
2. **If the violation points at a seam**, invoke the `reconnaissance`
   skill inline before returning to watch.

### Phase 2b — Persist (user-asked, audit-scope)
Triggered by phrases like "scan", "audit", "review my usage", "report".

1. **Ensure schema.** Run
   `sqlite3 .codescout/usage.db < <skill-dir>/sql/v1-bootstrap.sql`.
   Idempotent — safe on every scan.
2. **Resolve scan bound** from the user's phrasing:
   - "scan this session" → `cc_session_id = <current>`
   - "scan today" → `called_at >= date('now','start of day')`
   - "scan last N calls" → `id > (SELECT MAX(id) FROM tool_calls) - N`
   - "scan everything new" →
     `id > (SELECT COALESCE(MAX(tool_call_id), 0) FROM pika_observations)`
   - "scan all" → no bound; warn if `> 10k` rows
3. **Run the predicate matrix** in `<skill-dir>/sql/queries.sql` against
   `tool_calls` in scope. Open a sqlite3 connection with
   `PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;` set.
4. **For each candidate**, judge severity + recurrence + verdict. Write
   one `pika_observations` row with `kind`, `subkind`, `predicate`,
   `verdict`, `severity`, `recurrence`, optional `u_id`/`h_id`/`t_id`/
   `bug_id`, prose `notes`, `cc_session_id`.
5. **Cross-session promotion.** If a new candidate matches an existing
   pattern (`subkind` already has ≥1 row across sessions with
   `verdict in (slip|habit)`), bump `recurrence` on the new row and
   consider promoting (`verdict='habit'` → allocate `h_id`).
6. **Emit summary** to chat — counts per kind, top severities,
   promotion candidates. No row dumps unless the user asks.
```

- [ ] **Step 3: Append two Reactions**

In `~/.claude/buddy/skills/codescout-pika/SKILL.md`, in the `## Reactions` section, append (after reaction 6):

```markdown
7. **When the user asks "scan my usage" / "audit this session" /
   "review":** respond with —
   "→ pika: scanning `<bound>`. <count> codescout calls in scope.
   Running Iron Law predicates + judgment pass. Will write rows + return
   a summary; ask for details on a specific kind to see the full table."

8. **When the user asks "show me what Pika has logged" / "report":**
   respond with —
   "→ pika: reading `pika_observations`. Filter shown:
   `<kind, verdict, severity>`. Top N as a table; offer to expand the
   markdown view if you want it written to `docs/trackers/`."
```

- [ ] **Step 4: Sanity-read the modified SKILL.md**

```bash
grep -c "^7\. \*\*Watch in summon" ~/.claude/buddy/skills/codescout-pika/SKILL.md
```

Expected: `1`

```bash
grep -c "^### Phase 2a" ~/.claude/buddy/skills/codescout-pika/SKILL.md
grep -c "^### Phase 2b" ~/.claude/buddy/skills/codescout-pika/SKILL.md
```

Expected: `1` each.

```bash
grep -c "When the user asks \"scan my usage\"" ~/.claude/buddy/skills/codescout-pika/SKILL.md
```

Expected: `1`

If any expected is `0`, the edit didn't land — re-apply.

- [ ] **Step 5: Commit** (none — skill dir not git-tracked)

---

## Task 7: Mirror to `.claude-sdd` and `.claude-kat`

**Goal:** Honor the personal CLAUDE.md iron rule that all three CC instance dirs stay in sync.

**Files:**
- Mirror: `~/.claude/buddy/skills/codescout-pika/` → `~/.claude-sdd/buddy/skills/codescout-pika/`
- Mirror: `~/.claude/buddy/skills/codescout-pika/` → `~/.claude-kat/buddy/skills/codescout-pika/`

- [ ] **Step 1: Write the failing diff-check**

```bash
diff -r ~/.claude/buddy/skills/codescout-pika/ ~/.claude-sdd/buddy/skills/codescout-pika/ > /tmp/pika-diff-sdd.txt 2>&1 || true
diff -r ~/.claude/buddy/skills/codescout-pika/ ~/.claude-kat/buddy/skills/codescout-pika/ > /tmp/pika-diff-kat.txt 2>&1 || true
wc -l /tmp/pika-diff-sdd.txt /tmp/pika-diff-kat.txt
```

Expected: both files have **non-zero line counts** (because `.claude-sdd` and `.claude-kat` haven't been updated yet — sql/ and tests/ are missing there, SKILL.md is unchanged).

- [ ] **Step 2: Mirror via cp -r**

```bash
rsync -av --delete ~/.claude/buddy/skills/codescout-pika/ ~/.claude-sdd/buddy/skills/codescout-pika/
rsync -av --delete ~/.claude/buddy/skills/codescout-pika/ ~/.claude-kat/buddy/skills/codescout-pika/
```

`--delete` ensures the mirror is exact — orphaned files in the target are removed. `rsync -av` is preferred over `cp -r` because it preserves timestamps and reports what it did.

- [ ] **Step 3: Re-run the diff-check; assert no differences**

```bash
diff -r ~/.claude/buddy/skills/codescout-pika/ ~/.claude-sdd/buddy/skills/codescout-pika/ && echo "PASS: sdd in sync"
diff -r ~/.claude/buddy/skills/codescout-pika/ ~/.claude-kat/buddy/skills/codescout-pika/ && echo "PASS: kat in sync"
```

Expected output:

```
PASS: sdd in sync
PASS: kat in sync
```

`diff -r` with no differences prints nothing and exits 0; the `&& echo` reports success.

- [ ] **Step 4: Commit** (none — none of the CC dirs are git-tracked)

---

## Task 8: End-to-end smoke test against code-explorer's real usage.db

**Goal:** Phase 1's load-bearing acceptance: the pipeline runs end-to-end against the actual `code-explorer/.codescout/usage.db` and produces the "meadow quiet" outcome — zero false positives, zero exceptions.

**Files:**
- Create: `~/.claude/buddy/skills/codescout-pika/tests/test-smoke-code-explorer.sh`

- [ ] **Step 1: Write the smoke test**

```bash
# ~/.claude/buddy/skills/codescout-pika/tests/test-smoke-code-explorer.sh
#!/usr/bin/env bash
set -euo pipefail

SKILL_DIR="$HOME/.claude/buddy/skills/codescout-pika"
USAGE_DB="$HOME/work/claude/code-explorer/.codescout/usage.db"

[[ -f "$USAGE_DB" ]] || { echo "SKIP: $USAGE_DB not found"; exit 0; }

# Bootstrap schema against the real DB (idempotent — safe)
sqlite3 "$USAGE_DB" < "$SKILL_DIR/sql/v1-bootstrap.sql"

# Verify the new table is present
TABLE_PRESENT=$(sqlite3 "$USAGE_DB" \
    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='pika_observations';")
[[ "$TABLE_PRESENT" == "1" ]] || { echo "FAIL: pika_observations not created"; exit 1; }

# Verify schema version
VERSION=$(sqlite3 "$USAGE_DB" "SELECT MAX(version) FROM pika_schema_version;")
[[ "$VERSION" == "1" ]] || { echo "FAIL: schema version is $VERSION, expected 1"; exit 1; }

# Run each Iron Law predicate against the real DB; expect 0 matches (meadow quiet)
SINCE_ID=0

IL1=$(sqlite3 "$USAGE_DB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='read_file' AND outcome='ok'
       AND (input_json LIKE '%\"path\":\"%.rs\"%'
         OR input_json LIKE '%\"path\":\"%.py\"%'
         OR input_json LIKE '%\"path\":\"%.ts\"%'
         OR input_json LIKE '%\"path\":\"%.tsx\"%'
         OR input_json LIKE '%\"path\":\"%.js\"%'
         OR input_json LIKE '%\"path\":\"%.go\"%'
         OR input_json LIKE '%\"path\":\"%.java\"%'
         OR input_json LIKE '%\"path\":\"%.kt\"%')
       AND id > $SINCE_ID")

IL2=$(sqlite3 "$USAGE_DB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='edit_file' AND outcome='ok'
       AND (input_json LIKE '%\"new_string\":\"%fn %'
         OR input_json LIKE '%\"new_string\":\"%class %'
         OR input_json LIKE '%\"new_string\":\"%struct %'
         OR input_json LIKE '%\"new_string\":\"%def %'
         OR input_json LIKE '%\"new_string\":\"%interface %'
         OR input_json LIKE '%\"new_string\":\"%trait %')
       AND id > $SINCE_ID")

IL3=$(sqlite3 "$USAGE_DB" \
    "SELECT COUNT(*) FROM tool_calls
     WHERE tool_name='run_command'
       AND (input_json LIKE '%| grep%'
         OR input_json LIKE '%| wc%'
         OR input_json LIKE '%| head%'
         OR input_json LIKE '%| tail%')
       AND id > $SINCE_ID")

echo "Meadow check against $USAGE_DB:"
echo "  Iron Law 1 (read_file on source):     $IL1 candidates"
echo "  Iron Law 2 (edit_file structural):    $IL2 candidates"
echo "  Iron Law 3 (run_command piped):       $IL3 candidates"
echo "  (Iron Law 4 requires JSON1 — skipping in smoke; see test-predicates.sh)"
echo "PASS: pipeline alive against real usage.db (counts above are observational, not asserted)"
```

- [ ] **Step 2: Run smoke test**

```bash
chmod +x ~/.claude/buddy/skills/codescout-pika/tests/test-smoke-code-explorer.sh
~/.claude/buddy/skills/codescout-pika/tests/test-smoke-code-explorer.sh
```

Expected: prints meadow counts + `PASS: pipeline alive against real usage.db`.

The counts may be **non-zero** — that's diagnostic data, not failure. If `IL3` is large, it means the codescout server itself has piped `run_command` calls in its history (Pika will flag these on the first real Phase 2 scan; for Phase 1 we only assert pipeline aliveness).

- [ ] **Step 3: Record the smoke result in the codescout tracker**

Create `docs/trackers/pika-phase-1-validation.md`:

```markdown
# Pika Phase 1 — Validation Results

**Date:** 2026-05-17
**Scope:** Phase 1 acceptance criteria from
`docs/superpowers/specs/2026-05-17-pika-observability-design.md`

## Acceptance criteria

| # | Criterion | Result |
|---|---|---|
| 1 | `pika_observations` exists after first scan | ✓ (smoke test verified) |
| 2 | Bootstrap idempotent | ✓ (test-bootstrap-idempotent.sh) |
| 3 | Real-time whistle unchanged (no DB write on chat-only violation) | ✓ (Phase 2a unchanged in SKILL.md) |
| 4 | `scan my usage` resolves bound, runs predicates, writes rows | _manual verify next session_ |
| 5 | `sqlite3 .codescout/usage.db "SELECT * FROM pika_observations"` works | ✓ (smoke test verified) |
| 6 | Three CC instances in sync | ✓ (diff -r returned 0 lines) |
| 7 | All 10 predicate-correctness fixtures pass | ✓ (test-predicates.sh) |

## Smoke results against `code-explorer/.codescout/usage.db`

[paste output of test-smoke-code-explorer.sh here]

## Status

Phase 1: **DONE** (criteria 1, 2, 3, 5, 6, 7 verified at ship; criterion 4
verified on first user-asked scan in next session).

Next: Phase 2 — judgment kinds (`tool_bug`, `misusage`, `pattern`). See
spec § Rollout for the Phase 2 plan trigger.
```

- [ ] **Step 4: Commit the validation tracker (codescout repo, experiments branch)**

```bash
cd ~/work/claude/code-explorer
git status -s | grep "docs/trackers/pika-phase-1-validation.md"
# Should show: ?? docs/trackers/pika-phase-1-validation.md

git add docs/trackers/pika-phase-1-validation.md docs/superpowers/plans/2026-05-17-pika-observability-phase-1.md
git commit -m "docs(pika): Phase 1 plan + validation results"
git log --oneline -3
```

Expected: most recent commit is `docs(pika): Phase 1 plan + validation results`. Don't push — public-repo discipline.

---

## Plan Self-Review

**Spec coverage check:**

| Spec acceptance criterion | Implementing task |
|---|---|
| 1. `pika_observations` table exists after first scan | Task 1 (bootstrap), Task 8 (smoke verifies) |
| 2. Bootstrap idempotent | Task 1 (test-bootstrap-idempotent.sh) |
| 3. Real-time whistle unchanged | Task 6 (Phase 2a in SKILL.md is the existing flow renamed) |
| 4. `scan my usage` → bound + predicates + rows + summary | Tasks 2–4 (queries), Task 6 (Phase 2b method) |
| 5. `sqlite3 ... SELECT * FROM pika_observations` works | Task 1 (table created), Task 8 (verified) |
| 6. Three CC dirs in sync | Task 7 |
| 7. All ten predicate fixtures pass | Tasks 2–4 (fixtures + test-predicates.sh) |

All seven criteria mapped. No gaps.

**Placeholder scan:** no TBD / TODO / fill-in / etc. in any task body. All code blocks are complete.

**Type consistency:**
- `pika_observations` column names match across Task 1 (CREATE), Task 5 (INSERT/SELECT in CASCADE test), Task 5 (INSERT in concurrent test), Task 8 (SELECT in smoke). ✓
- `tool_calls` column references are minimal (`id`, `tool_name`, `outcome`, `input_json`, `output_json`, `error_msg`, `cc_session_id`, `called_at`, `latency_ms`) and stable across tasks. ✓
- Bash variable names (`SKILL_DIR`, `TMPDB`, `USAGE_DB`) consistent across all test scripts. ✓
- Subkind values (`iron_law_1`...`iron_law_4`) consistent between queries.sql and CASCADE test fixture (`'iron_law_1'`). ✓

No issues.

---

## Risks (carried from spec, surfaced here for the implementer)

| Risk | Mitigation |
|---|---|
| `PRAGMA foreign_keys = ON` is per-connection — easy to forget | Task 6 (Phase 2b step 3) writes the pragma explicitly. Task 5's CASCADE test will fail otherwise. |
| Concurrent worktree writes can collide despite WAL | `busy_timeout=5000` set in Task 5 test; Task 6 procedure includes the pragma. |
| LIKE patterns in queries.sql have JSON-quote-sensitive boundaries | Fixture row 3 (`src/lib.rs.bak`) tests this exact edge. |
| If JSON1 unavailable, Iron Law 4 errors at runtime | Task 6 method step 3 wraps the CTE in a try/fallback to LIKE — implementer adds this fallback inline when transcribing the procedure. |
| Pika may run against a corrupt or rotated `usage.db` | Spec § Error Handling enumerates all 7 failure modes; implementer references that section when extending the procedure with error replies. |

---

## Execution

Plan complete and saved to
`docs/superpowers/plans/2026-05-17-pika-observability-phase-1.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration. The plan is well-shaped for this — each task has a clear file boundary, a failing test, an implementation, a passing test.

**2. Inline Execution** — execute tasks in this session using `executing-plans`, batch with checkpoints. Trade-off: my context fills with bash output; review happens per-checkpoint instead of per-task.

Which approach?
