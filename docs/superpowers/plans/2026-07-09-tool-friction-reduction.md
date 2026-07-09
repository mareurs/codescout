# Tool Friction Reduction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the read_markdown observability gap, add a repeatable "silent-success"
detector to the pika query matrix, and produce the evidence needed to later decide
`il1_read_overlaps_symbol`'s fix — without deciding that fix now.

**Architecture:** Three independent tasks, sequenced cheap-to-expensive. Task 1 widens
an existing Rust classifier (`normalize_err_family`) to take tool-name context. Task 2
adds one new SQL anchor + one new heuristic to a sibling repo's pika specialist. Task 3
is a one-time analysis pass with no code change — output is a session-log tracker with
real numbers.

**Tech Stack:** Rust (codescout, `rusqlite`), SQL (SQLite, pika query matrix), markdown
(session-log tracker).

## Global Constraints

- Task 1 lives in codescout (`/home/marius/work/claude/codescout`); Task 2 lives in the
  sibling repo `claude-plugins` (`/home/marius/work/claude/claude-plugins`) — switch
  workspace context explicitly for Task 2, restore codescout as home afterward.
- The fail-loud-vs-auto-redirect fork for IL1 is explicitly out of scope — Task 3
  produces evidence only, never a fix.
- No general anomaly detector — Task 2's detector is a closed, two-marker list.
- Spec: `docs/superpowers/specs/2026-07-09-tool-friction-reduction-design.md`.

---

### Task 1: Tool-scope `normalize_err_family`, tag `read_markdown`'s own errors

**Files:**
- Modify: `src/usage/db.rs:159-222` (`normalize_err_family`)
- Modify: `src/usage/db.rs:245-273` (`backfill_legacy_rows`)
- Modify: `src/usage/mod.rs:70` (`UsageRecorder::write_content` call site)
- Test: `src/usage/db.rs:1340-1388` (extend existing test in place)

**Interfaces:**
- Produces: `pub(crate) fn normalize_err_family(tool_name: &str, msg: &str) -> Option<&'static str>`
  — signature change from the current single-argument form. Any future caller needs
  both the tool name and the error message.

- [ ] **Step 1: Extend the existing test with tool-scoped cases (will not compile yet)**

Replace the full body of `normalize_err_family_maps_iron_law_routing_errors` in
`src/usage/db.rs`:

```rust
#[test]
fn normalize_err_family_maps_iron_law_routing_errors() {
    // The families that dominate the real error population — previously all NULL.
    let cases = [
        (
            "read_file",
            "source range overlaps named symbol(s): 'open_db'",
            Some("il1_read_overlaps_symbol"),
        ),
        (
            "read_file",
            "Use read_markdown for markdown files",
            Some("il4_read_markdown_routing"),
        ),
        (
            "edit_file",
            "Use edit_markdown for markdown files",
            Some("il5_edit_markdown_routing"),
        ),
        (
            "edit_file",
            "edit contains a symbol definition (\"def \") — use symbol tools",
            Some("il2_structural_edit"),
        ),
        (
            "edit_file",
            "edit_file is blocked for structural edits on source code files",
            Some("il2_structural_edit"),
        ),
        (
            "run_command",
            "shell access to source files is blocked",
            Some("il3_shell_on_source"),
        ),
        (
            "run_command",
            "IL3 violation — piped `cargo test` to a log-trimmer. BLOCKED.",
            Some("il3_pipe_to_trimmer"),
        ),
        (
            "create_file",
            "write denied: '/x/INDEX.md' is outside the project root",
            Some("write_scope_denied"),
        ),
        (
            "read_file",
            "unsupported json_path segment '[*]'",
            Some("json_path_unsupported"),
        ),
        (
            "edit_file",
            "old_string not found in src/x.rs",
            Some("edit_stale_match"),
        ),
        (
            "edit_markdown",
            "old_string not found in section 'Foo'. The text must match exactly (whitespace-sensitive).",
            Some("edit_stale_match"),
        ),
        // Pre-existing families still resolve.
        ("symbols", "LSP server disconnected", Some("lsp_disconnect")),
        (
            "symbols",
            "symbol not found: Foo/bar",
            Some("symbol_not_found"),
        ),
        ("read_file", "some unrecognized failure", None),
        // New: read_markdown's own errors, previously untagged (the biggest
        // untagged bucket in usage.db — see the design spec's Problem section).
        (
            "read_markdown",
            "read_markdown only supports .md files",
            Some("read_markdown_wrong_ext"),
        ),
        (
            "read_markdown",
            "file not found: 'docs/MISSING.md'",
            Some("read_markdown_file_not_found"),
        ),
        (
            "read_markdown",
            "'docs/trackers' is a directory, not a file",
            Some("read_markdown_path_is_directory"),
        ),
        (
            "read_markdown",
            "combined headings span 812 lines — exceeds inline threshold",
            Some("read_markdown_overflow_threshold"),
        ),
        (
            "read_markdown",
            "section \"## Foo\" spans 900 lines — exceeds inline threshold",
            Some("read_markdown_overflow_threshold"),
        ),
        (
            "read_markdown",
            "heading and headings are mutually exclusive",
            Some("read_markdown_param_conflict"),
        ),
        (
            "read_markdown",
            "both start_line and end_line are required",
            Some("read_markdown_param_conflict"),
        ),
        (
            "read_markdown",
            "invalid line range: start_line=5 end_line=2",
            Some("read_markdown_invalid_line_range"),
        ),
        (
            "read_markdown",
            "start_line 900 exceeds file length 500",
            Some("read_markdown_invalid_line_range"),
        ),
        (
            "read_markdown",
            "heading 'SI-99' not found",
            Some("heading_not_found"),
        ),
        (
            "edit_markdown",
            "heading 'SI-99' not found",
            Some("heading_not_found"),
        ),
        // Scoping proof: the same message text from an unrelated tool must NOT
        // pick up a read_markdown-specific family.
        (
            "some_other_tool",
            "read_markdown only supports .md files",
            None,
        ),
    ];
    for (tool_name, msg, want) in cases {
        assert_eq!(
            normalize_err_family(tool_name, msg),
            want,
            "tool={tool_name} msg={msg}"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails (compile error — signature mismatch)**

Run: `cargo test --lib normalize_err_family_maps_iron_law_routing_errors 2>&1`
Expected: FAIL — `error[E0061]: this function takes 2 arguments but 1 argument was supplied`
(the old `normalize_err_family(msg)` call sites in the crate, and the new
2-argument test calls, mismatch until Step 3 lands).

- [ ] **Step 3: Widen `normalize_err_family` and its call sites**

Replace the full body of `normalize_err_family` in `src/usage/db.rs`:

```rust
pub(crate) fn normalize_err_family(tool_name: &str, msg: &str) -> Option<&'static str> {
    // infra / tool-class (excluded from the probe's code-class score)
    if msg.contains("index is locked") {
        return Some("lsp_index_locked");
    }
    if msg.contains("Failed to spawn mux") || msg.contains("mux startup failed") {
        return Some("mux_startup_fail");
    }
    if msg.contains("LSP server is not running") {
        return Some("lsp_not_running");
    }
    if msg.contains("LSP server disconnected") {
        return Some("lsp_disconnect");
    }
    // read_markdown's OWN errors — tool-scoped so an unrelated tool emitting
    // similar-looking text never mis-attributes. Distinct from
    // `il4_read_markdown_routing` below, which is read_file's redirect-TO-
    // read_markdown message, not read_markdown's own failure.
    if tool_name == "read_markdown" {
        if msg.contains("only supports .md files") {
            return Some("read_markdown_wrong_ext");
        }
        if msg.starts_with("file not found:") {
            return Some("read_markdown_file_not_found");
        }
        if msg.contains("is a directory, not a file") {
            return Some("read_markdown_path_is_directory");
        }
        if msg.contains("exceeds inline threshold") {
            return Some("read_markdown_overflow_threshold");
        }
        if msg.contains("mutually exclusive")
            || msg.contains("both start_line and end_line are required")
        {
            return Some("read_markdown_param_conflict");
        }
        if msg.contains("invalid line range") || msg.contains("exceeds file length") {
            return Some("read_markdown_invalid_line_range");
        }
    }
    // Shared heading-resolution error (file_summary::resolve_section_range) —
    // raised by read_markdown and edit_markdown; NOT by artifact(get), which
    // swallows the same miss into body_meta.heading_missing and stays success.
    if (tool_name == "read_markdown" || tool_name == "edit_markdown")
        && msg.starts_with("heading '")
        && msg.ends_with("' not found")
    {
        return Some("heading_not_found");
    }
    // iron-law routing / wrong-tool class — the agent reached for the wrong tool
    // and the server gate rejected + re-routed it. These dominate the real error
    // population; the original taxonomy missed them all, leaving err_family NULL
    // on ~90% of errors even on fresh rows.
    if msg.contains("overlaps named symbol") {
        return Some("il1_read_overlaps_symbol");
    }
    if msg.contains("Use read_markdown") {
        return Some("il4_read_markdown_routing");
    }
    if msg.contains("Use edit_markdown") {
        return Some("il5_edit_markdown_routing");
    }
    if msg.contains("contains a symbol definition")
        || msg.contains("is blocked for structural edits")
    {
        return Some("il2_structural_edit");
    }
    if msg.contains("shell access to source files is blocked") {
        return Some("il3_shell_on_source");
    }
    if msg.contains("IL3 violation") {
        return Some("il3_pipe_to_trimmer");
    }
    // security / scope class — deliberately tool-agnostic: `write denied` comes
    // from `path_security.rs`'s shared write-gate, reused by every write tool.
    // Same underlying mechanism regardless of caller, so one family is correct.
    if msg.contains("write denied") {
        return Some("write_scope_denied");
    }
    // input-shape / extractor class
    if msg.contains("unsupported json_path") {
        return Some("json_path_unsupported");
    }
    // deliberately tool-agnostic: `edit_file` and `edit_markdown` both raise
    // "old_string not found" for the identical root cause (stale re-read before
    // editing) — confirmed by grep across src/tools/edit_file/mod.rs and
    // src/tools/markdown/edit_markdown.rs; one family is correct here too.
    if msg.contains("old_string not found") {
        return Some("edit_stale_match");
    }
    // code / extractor-shape class
    if msg.contains("AST parse failed") || msg.contains("cannot determine end of") {
        return Some("ast_extent_fail");
    }
    if msg.contains("ambiguous name_path") {
        return Some("ambiguous_name_path");
    }
    if msg.contains("dropped sibling") || msg.contains("dropped the symbol") {
        return Some("replace_dropped_sibling");
    }
    if msg.contains("symbol not found") {
        return Some("symbol_not_found");
    }
    None
}
```

Update `backfill_legacy_rows` (`src/usage/db.rs:245-273`) to select and pass
`tool_name`:

```rust
let unclassified: Vec<(i64, String, String)> = {
    let mut stmt = conn.prepare(
        "SELECT id, tool_name, error_msg FROM tool_calls \
         WHERE err_family IS NULL AND error_msg IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    rows.collect::<std::result::Result<_, _>>()?
};
for (id, tool_name, msg) in unclassified {
    if let Some(family) = normalize_err_family(&tool_name, &msg) {
        conn.execute(
            "UPDATE tool_calls SET err_family = ?1 WHERE id = ?2",
            params![family, id],
        )?;
    }
}
```

Update the call site in `src/usage/mod.rs:70`:

```rust
let err_family = error_msg
    .as_deref()
    .and_then(|m| db::normalize_err_family(tool_name, m));
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib normalize_err_family_maps_iron_law_routing_errors 2>&1`
Expected: PASS — `test usage::db::tests::normalize_err_family_maps_iron_law_routing_errors ... ok`

- [ ] **Step 5: Verify the untouched `backfill_fills_project_root_and_err_family_once` test still passes**

Run: `cargo test --lib backfill_fills_project_root_and_err_family_once 2>&1`
Expected: PASS unchanged — this test's fixture rows (`il1_read_overlaps_symbol`,
`il5_edit_markdown_routing`) never touch the new tool-scoped arms, confirming the
change is additive, not a behavior change for existing families.

- [ ] **Step 6: Full suite + lint**

Run: `cargo fmt && cargo clippy --lib -- -D warnings && cargo test --lib`
Expected: clean fmt diff, zero clippy warnings, full suite green.

- [ ] **Step 7: Commit**

```bash
git add src/usage/db.rs src/usage/mod.rs
git commit -m "feat(usage): tool-scope err_family classifier, tag read_markdown errors

normalize_err_family now takes tool_name alongside the message, closing the
largest untagged error bucket in usage.db (read_markdown's own failures —
191 untagged rows in one repo's corpus). Audited existing arms for the same
cross-tool ambiguity: old_string-not-found and write-denied are correctly
tool-agnostic (same root cause, same remedy, regardless of caller); the new
heading-not-found arm is correctly scoped to read_markdown/edit_markdown
only, since artifact(get) never raises this as an error."
```

---

### Task 2: Silent-hollow-output detector + Heuristic 13 (claude-plugins repo)

**Files (in `/home/marius/work/claude/claude-plugins`):**
- Modify: `buddy/skills/codescout-pika/sql/queries.sql:132` (insert after line 132,
  before the `-- === Recency rollup` anchor at line 133)
- Modify: `buddy/skills/codescout-pika/SKILL.md:221` (insert after line 221, before
  `## Reactions` at line 222)

**Interfaces:**
- Consumes: the `tool_calls.output_json` column (populated whenever debug logging is
  on — confirmed >99% coverage across all three mined repos).
- Produces: a new pika anchor queryable by `kind='tool_bug'`,
  `subkind='silent_hollow_output'` — the same `pika_observations` row shape Task 1's
  and today's earlier session's bugs already used.

- [ ] **Step 1: Switch workspace context to claude-plugins**

```
workspace(action="activate", path="/home/marius/work/claude/claude-plugins")
```

- [ ] **Step 2: Add the new SQL anchor**

Insert after line 132 of `buddy/skills/codescout-pika/sql/queries.sql` (right before
`-- === Recency rollup: lifetime vs live (Heuristic 12 / Self-Trap 6) ===`):

```sql

-- === Silent hollow-output candidates (kind='tool_bug', subkind='silent_hollow_output') ===
-- INTENT: flag outcome='success' calls whose output_json contains a known hollow-content
-- marker. Two known markers today — this is a closed, curated list (see Heuristic 13),
-- not a general anomaly detector. Pika's judgment pass (not pure SQL) distinguishes a
-- real bug from a legitimately-empty file by checking whether the PREVIOUS call in the
-- same cc_session_id produced the buffer being read here.
SELECT id, cc_session_id, tool_name, called_at, input_json, output_json
FROM tool_calls
WHERE outcome = 'success'
  AND (output_json LIKE '%"0 lines"%'
    OR output_json LIKE '%"heading_missing":true%'
    OR output_json LIKE '%"line_count":0%')
  AND id > :since_id;
```

- [ ] **Step 3: Add Heuristic 13 to SKILL.md**

Insert after line 221 of `buddy/skills/codescout-pika/SKILL.md` (right before
`## Reactions`):

```markdown

13. **If a tool call SUCCEEDED but `output_json` contains a known hollow-content
    marker (`"0 lines"`, `heading_missing:true`, `line_count:0`), whistle "silent
    hollow output" — but cross-check the previous call in the same session that
    produced the buffer before concluding it's a bug.** A genuinely empty file
    legitimately produces `"0 lines"` too; the tell is whether the CALLER's own
    input shows clear intent to get real content (a specific `heading=`,
    `start_line`/`end_line`, or `json_path`) that a healthy artifact should have
    satisfied. This is the `artifact(get, start_line/heading)` class (codescout
    fix shipped 2026-07-09; see `docs/issues/2026-07-09-artifact-get-line-slice-
    blank-separator-offset.md` and the sibling heading-exact-match bug file in
    codescout). Closed marker list — extend only when a new concrete shape is
    found, not speculatively (see the design spec's "What Is Not Changing").
```

- [ ] **Step 4: Verify the new query against real data**

Run against backend-kotlin's usage.db (has 7 known historical hits from earlier
mining this session):

```bash
sqlite3 /home/marius/work/mirela/backend-kotlin/.codescout/usage.db "SELECT COUNT(*) FROM tool_calls WHERE outcome='success' AND (output_json LIKE '%\"0 lines\"%' OR output_json LIKE '%\"heading_missing\":true%' OR output_json LIKE '%\"line_count\":0%')"
```

Expected: a count ≥ 7 (the known historical hits from this session's investigation,
now discoverable by a standing query instead of hand-tracing buffer IDs).

- [ ] **Step 5: Commit**

```bash
git add buddy/skills/codescout-pika/sql/queries.sql buddy/skills/codescout-pika/SKILL.md
git commit -m "feat(pika): add silent-hollow-output detector (Heuristic 13)

Closed two-marker query catching the artifact(get) silent-success bug class
found and fixed in codescout 2026-07-09 — turns the manual buffer-ID tracing
done that session into a repeatable check."
```

- [ ] **Step 6: Restore codescout as home project**

```
workspace(action="activate", path="/home/marius/work/claude/codescout")
```

---

### Task 3: IL1 evidence-gathering pass (no code — analysis + write-up)

**Files:**
- Create: `docs/trackers/il1-friction-diagnosis-session-log.md` (copied from
  `docs/templates/session-log.md`)

**Interfaces:**
- Consumes: Task 2's new query pattern as a style reference (not a dependency —
  Task 3's four queries are one-off, not added to `queries.sql`).
- Produces: four F-N entries in the new session-log tracker, each with a real number
  from the three-repo corpus (codescout, backend-kotlin, claude-plugins).

- [ ] **Step 1: Create the session-log tracker from the template**

Read `docs/templates/session-log.md`, copy its structure to
`docs/trackers/il1-friction-diagnosis-session-log.md`, with the Index table headed
by this work stream's four rows (Category: `codescout-tool` for all four).

- [ ] **Step 2: Measurement 1 — recovery-cost distribution**

Run per repo (example for backend-kotlin — requires SQLite JSON1, already relied on
by the pika query matrix's Iron Law 4 query):

```sql
WITH il1_errors AS (
  SELECT id, cc_session_id, json_extract(input_json, '$.path') AS path
  FROM tool_calls
  WHERE err_family = 'il1_read_overlaps_symbol'
),
next_symbols AS (
  SELECT e.id AS error_id, MIN(s.id) AS recovery_id
  FROM il1_errors e
  JOIN tool_calls s
    ON s.cc_session_id = e.cc_session_id
   AND s.tool_name = 'symbols'
   AND s.id > e.id
   AND json_extract(s.input_json, '$.path') = e.path
   AND s.outcome = 'success'
  GROUP BY e.id
)
SELECT AVG(recovery_id - error_id) AS mean_gap,
       MAX(recovery_id - error_id) AS max_gap,
       COUNT(*) AS recovered_count,
       (SELECT COUNT(*) FROM il1_errors) AS total_errors
FROM next_symbols;
```

Run against each repo's `.codescout/usage.db`. Write `mean_gap`, `max_gap`, and
`recovered_count / total_errors` (the fraction that ever self-corrected via
`symbols` on the same file, vs. never — a distinct number from the gap itself) into
F-1's **Got:** field.

- [ ] **Step 3: Measurement 2 — ambiguity rate**

For the same 30-error sample, for each `read_file(path, start_line, end_line)` input,
run `symbols(path=<file>)` (overview mode) and count how many named symbols the
requested `[start_line, end_line]` range overlaps. Record `unambiguous (exactly 1) /
total` as a percentage in F-2's **Got:** field — this is the number that gates any
future auto-redirect design.

- [ ] **Step 4: Measurement 3 — repeat-offender pattern**

```sql
SELECT cc_session_id, COUNT(DISTINCT json_extract(input_json, '$.path')) AS distinct_files
FROM tool_calls
WHERE err_family = 'il1_read_overlaps_symbol'
GROUP BY cc_session_id
HAVING distinct_files > 1
ORDER BY distinct_files DESC
LIMIT 10;
```

Record the fraction of sessions with `distinct_files > 1` (standing habit) vs. exactly
1 (once-and-corrected) in F-3's **Got:** field.

- [ ] **Step 5: Measurement 4 — subagent-dispatch correlation**

```sql
SELECT cc_session_id, MIN(id) AS first_call_id
FROM tool_calls
GROUP BY cc_session_id;
```

Join against `il1_read_overlaps_symbol` error ids to compute, per session, how many
tool calls preceded the FIRST IL1 error. Record the distribution (e.g. "N% of
sessions hit IL1 within their first 5 calls") in F-4's **Got:** field.

- [ ] **Step 6: Write up all four F-N entries + Index rows, commit**

Use `edit_markdown(action="insert_before", heading="## Template for new entries",
content=...)` for each F-N entry, and add matching rows to the `## Index` table.
Severity per entry: `high` if the number suggests a live, costly pattern; `low` if it
suggests the friction is already cheap/self-healing.

```bash
git add docs/trackers/il1-friction-diagnosis-session-log.md
git commit -m "docs(trackers): IL1 evidence-gathering pass — 4 measurements

Recovery-cost distribution, ambiguity rate, repeat-offender pattern, and
subagent-dispatch correlation for il1_read_overlaps_symbol across codescout,
backend-kotlin, and claude-plugins usage.db. Feeds a follow-up brainstorm on
the fail-loud-vs-auto-redirect fork — does not decide it."
```
