# scripts/measure — Stage 0 instrument check (spec A1.8)

Before building any pipeline stage, `docs/PROBES.md` gets checked for an instrument that already
measures it, per Amendment 1 A1.8: "reuse one only after checking its current predicate against
the live schema, or record why it was declined." This file is that record for Stage 0
(`archive.py`); Tasks 5-11 (Stages 1-4) follow the same discipline and extend this list rather
than re-deriving it.

## Instruments checked

- **claude-traces `cc.py`** (`docs/issues/archive/2026-08-20-cc-py-cost-double-counts-split-assistant-entries.md`,
  `docs/PROBES.md`). Dedupes `type:"assistant"` JSONL entries by `message.id` before summing
  `usage` — one model completion is often split across several lines (thinking/text/tool_use),
  each carrying an identical `usage` payload, and naive per-line summing overcounts 2.1-2.6x.
  **Declined as a dependency for Stage 0**: it lives in llm-proxy behind this repo's
  `.claude/skills/claude-traces` symlink, ships no test suite (the bug file's own "Tests added"
  section says so), and its hardcoded `~/.claude`-only base is a separately filed, still-open
  issue (`llm-proxy:docs/issues/2026-07-10-ccpy-config-dir-hardcoded-and-path-encoding.md`) — a
  `CLAUDE_CONFIG_DIR` claim in `docs/PROBES.md` could not be independently re-verified against
  the live script in this session, so it is recorded here as asserted-current rather than
  confirmed. **Reuse the predicate**, not the tool: Stage 1's own turn/completion counting needs
  the identical `message.id`-dedupe check, and should be validated against this bug file's two
  reproductions ($5.3861 on session `23b22760`, $108.4389 on `55515bc5`) as a positive control
  before being trusted on the frozen corpus.

- **`scripts/file-provenance.py`**. Scans transcripts across every discovered `~/.claude*`
  profile for tool-call inputs naming a write target, discovering profiles at run time rather
  than from a fixed list. **Declined** as a dependency for `freeze()` — it answers "who wrote
  this file", a different predicate and output shape than "list every transcript". **Reuse the
  profile-discovery pattern** when `sources.transcript_dirs` is assembled from a live profile
  scan rather than the explicit list this task takes as an argument.

- **`scripts/probe_guide_injection.py`** / **`scripts/probe_guide_section_use.py`**. Both read
  transcripts across profiles and separate main-session from subagent transcripts, and both
  record that the same `session_id` can appear under two profile directories (49 of 1,705 at
  their 2026-08-27 baseline). **Declined as a dependency** — scoped to guide-injection marker
  parsing, not general transcript indexing. **Reuse the caveat**: Stage 1's join must not
  double-count a session id duplicated across profiles; a collision found there is an
  `exclusions` entry (Task 5), not silent absorption.

- **`scripts/friction-probe.py`**, **`scripts/probe_librarian_scope.py`**,
  **`scripts/probe_entry_read_grain.py`**, and the `analyze-usage` skill's canonical SQL queries.
  All read a **live** `.codescout/usage.db` and key joins on `session_id` — never
  `cc_session_id`, which is wrong on roughly a third of rows, the same caveat repeated in all
  three. All also note the table is retention-swept, so a live count is a floor, not a total.
  **Declined as libraries** for Stage 0: `freeze()` snapshots `usage.db` with
  `sqlite3.Connection.backup` rather than querying it live, and every later stage reads only the
  frozen copy. **Reuse the `session_id`-not-`cc_session_id` predicate check** verbatim when
  Stage 1 joins tool-call rows from a frozen snapshot — re-verified against that snapshot's own
  schema first, per A1.8, since `_count_usage_rows` here already had to be corrected once (an
  assumed `usage` table name, versus the real `tool_calls` — confirmed by grepping
  `src/usage/db.rs`).

- **`docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py`**. Named directly by this spec's
  own § Architecture: Stage 2's correction miner "reuses the correction markers and parent-commit
  blame" from this script. **Reuse, after checking its predicate against the live schema** — a
  Task 6+ concern, recorded here per A1.8 rather than re-argued at that task.

- **The `git log … %(trailers:key=Session-Id,valueonly)` derivation**
  (`scripts/pre-push-foreign-session-guard.sh`). Merge-aware trailer reading; handles commits
  that carry no trailer without erroring. **Reuse the derivation, not this script** — Stage 0's
  `manifest.versions.codescout_sha` and `manifest.repos[name]` need only a plain `git rev-parse
  HEAD` (implemented via `subprocess` in `archive.py`), but if a later stage ever reads a *range*
  of commits for their trailers, use the `%(trailers:…)` form, never `grep '^Session-Id:'` — some
  commits carry the trailer in a body line rather than a header, which a literal grep misses.

- **`scripts/peer-sessions.sh`**, per-profile session registry files
  (`<profile>/sessions/<pid>.json`), `librarian(action="doctor")`'s `claim_liveness` checks, and
  `librarian(action="audit_log")`. **Declined** — all three describe *live* Claude Code
  processes or codescout's own machine-local catalog (`~/.local/share/librarian/catalog.db`), not
  already-written transcripts or `usage.db` rows. No overlap with a frozen, offline corpus.

## Not checked

The remainder of `docs/PROBES.md` (architecture-boundary probes, retrieval-quality benchmarks,
import-rename/string-dispatch density scans, and similar) measure codescout's own source tree or
retrieval quality — no overlap with Stages 0-4 of this measurement pipeline.
