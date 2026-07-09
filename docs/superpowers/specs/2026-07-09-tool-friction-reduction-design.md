# Codescout Tool-Friction Reduction — Design Spec

**Date:** 2026-07-09
**Branch:** experiments
**Status:** draft

## Problem

Two silent-success bugs were found and fixed this session in `artifact(action="get")`
(`src/librarian/tools/get.rs`): an off-by-one line-numbering bug (blank frontmatter
separator counted as line 1) and an exact-only heading matcher inconsistent with
`read_markdown`/`edit_markdown`'s documented fuzzy matching. Both returned `outcome:
"success"` while doing something other than what the caller asked — invisible to any
error-gated monitoring.

Mining `.codescout/usage.db` across three repos (codescout, backend-kotlin,
claude-plugins) to find more of this class surfaced a broader picture:

1. **The `read_markdown` tool is the single largest untagged error source** — 191
   errors in backend-kotlin alone sit in the `err_family IS NULL` bucket, already
   flagged in a prior pika observation ("fix is observability, not deny/warn") but
   never implemented.
2. **There is no standing, repeatable detector for the "success but hollow content"
   bug class** the two `get.rs` bugs belong to — today's `queries.sql` only has a
   two-step, judgment-heavy detector for a *different* class (undeclared params
   silently dropped).
3. **The Iron Law gates work, but "prevented" isn't "frictionless."** `il1_read_overlaps_symbol`
   (reading source via a `read_file` line-range that overlaps a named symbol) is the
   single largest *tagged* error family in all three repos — 699 lifetime / 204 in
   the last 7 days in backend-kotlin alone — despite the gate being deployed and
   correctly firing every time. Whether the right fix is a smarter error message, a
   server-side auto-redirect, or a dispatch-briefing fix is genuinely unknown; no
   evidence exists yet to choose between them.

## Scope

Three independent, sequenced components. Each is scoped so it can ship (or be
evaluated) without the others landing first.

**Cross-repo note:** this spec lives in codescout's `docs/superpowers/specs/` (the
investigating project), but Component 2 changes files in the sibling `claude-plugins`
repo (the `codescout-pika` buddy specialist). Per this project's cross-repo citation
convention, implementation commits in that repo are cited here as `claude-plugins:<sha>`.

**Explicitly out of scope** (see "What Is Not Changing"): deciding whether codescout
should ever auto-redirect instead of fail-loud; building a general anomaly detector;
touching the already-shipped IL2/IL3 hookify gates.

---

## Changes

### 1. `src/usage/db.rs` + `src/usage/mod.rs` — tool-scoped `err_family` classification

**Current shape:** `normalize_err_family(msg: &str) -> Option<&'static str>`
(`src/usage/db.rs:159-222`) is a substring-match cascade over the error message alone,
with no tool-name context. Called from `UsageRecorder::write_content`
(`src/usage/mod.rs:70`) and replayed for historical rows in `backfill_legacy_rows`
(`src/usage/db.rs:245-273`, which currently `SELECT`s only `(id, error_msg)` —
needs `tool_name` added to that query).

**Change:** widen the signature to `normalize_err_family(tool_name: &str, msg: &str)`.
This closes two gaps at once:

- **New arms for `read_markdown`'s own errors**, scoped to `tool_name ==
  "read_markdown"` (verified against `src/tools/markdown/read_markdown.rs`'s actual
  error strings): `"read_markdown only supports .md files"`, `"file not found:
  '{}'"`, `"'{}' is a directory, not a file"`, the two `"... spans {} lines —
  exceeds inline threshold"` variants (combined-headings and single-section), the
  three mutual-exclusivity messages (`heading`/`headings`/`start_line`+`end_line`
  conflicts — bucket as one low-cardinality family per the classifier's own
  low-cardinality design goal), and the line-range validation pair (`"invalid line
  range"`, `"exceeds file length"`).
- **Tool-scope the shared `"heading '{}' not found"` message** (from
  `file_summary::resolve_section_range`, used by both `read_markdown` and
  `edit_markdown`) to those two tool names specifically — this message is NOT
  raised by `artifact(get)`'s heading lookup (that path swallows the same error into
  `body_meta.heading_missing` and stays `outcome: success` by design), so no
  cross-tool collision there, but the classifier should encode that scoping rather
  than assume it.
- **Audit the 12 existing arms** for the same latent cross-tool ambiguity while the
  signature is already being touched (e.g. does any other tool emit a message
  containing `"symbol not found"` or `"old_string not found"` for an unrelated
  reason?) — one pass, not a follow-up task.
- **One arm stays deliberately tool-agnostic:** `"buffer reference not found"`
  (seen in `read_markdown.rs:24` and elsewhere) is a shared `@ref` subsystem
  message, not a `read_markdown`-specific failure — leave it unscoped
  (`mux_startup_fail`-style infra family), since the mechanism is identical
  regardless of which tool referenced the expired buffer.

**Test:** extend `tests::normalize_err_family_maps_iron_law_routing_errors`
(`src/usage/db.rs:1340-1388`) with cases for each new arm, plus a case proving the
`heading '{}' not found` scoping doesn't fire for an unrelated tool name.

### 2. `claude-plugins:buddy/skills/codescout-pika/sql/queries.sql` — silent-hollow-output detector

**Current shape:** the file has one detector for a related-but-distinct bug class
(`silent_param_drop`, Heuristic 11 in the specialist's `SKILL.md`) — a two-step,
judgment-heavy process (SQL enumerates candidate keys, Pika manually diffs against
declared schemas). No standing query exists for "this call succeeded but the content
looks hollow."

**Change:** add a new anchor, following the file's existing `-- === Section Name
===` / `-- Anchor:` / `-- INTENT:` commenting convention:

```sql
-- === Silent hollow-output candidates (kind='tool_bug', subkind='silent_hollow_output') ===
-- INTENT: flag outcome='success' calls whose output_json contains a known hollow-content
-- marker, cross-referenced against whether the caller's OWN input shows clear intent to
-- get real content (a specific json_path, heading, or line range) rather than a
-- legitimately-empty artifact. Two known markers today — this is a closed, curated list,
-- not a general anomaly detector (see spec 2026-07-09-tool-friction-reduction-design.md).
SELECT id, tool_name, called_at, input_json, output_json
FROM tool_calls
WHERE outcome = 'success'
  AND (output_json LIKE '%"0 lines"%'
    OR output_json LIKE '%"heading_missing":true%'
    OR output_json LIKE '%"line_count":0%')
  AND id > :since_id;
```

Scoped as a **closed list of two known marker shapes**, matching Operating Principle
4 (resist abstraction without two concretes) — this is pattern-matching on the two
bugs actually found, not a fuzzy anomaly detector. Adding a third marker later, when a
third concrete shape is found, is the expected extension path.

**New Heuristic 13 in `SKILL.md`** (following the existing Heuristic 11 pattern):
names the marker list, states the closed-list scope explicitly, and notes the same
`tool_name` disambiguation question Component 1 raised (`"0 lines"` could, in
principle, describe a legitimately-empty file's real `read_file` output — Pika's
judgment pass, not pure SQL, distinguishes the two by checking whether the *previous*
call in the same `cc_session_id` produced the buffer being read).

### 3. IL1 evidence-gathering pass (diagnosis only — no fix, no code change)

Not a queries.sql addition — a one-time analysis pass, because these questions don't
need to be a standing predicate; they need to be answered once. Four measurements
against the existing corpus (all three repos), written up as a new
`docs/trackers/il1-friction-diagnosis-session-log.md` (copied from
`docs/templates/session-log.md`, per this project's session-log convention for
multi-session work streams — this qualifies since the pass spans three repos' data):

1. **Recovery-cost distribution** — for each `il1_read_overlaps_symbol` error, count
   tool calls and elapsed time until the same file/symbol gets a successful
   `symbols()` fetch.
2. **Ambiguity rate** — does the requested line range overlap exactly one symbol
   (unambiguous) or multiple / a symbol-plus-glue-code boundary (ambiguous)? This
   number alone can rule out server-side auto-redirect as a general fix if ambiguous
   cases dominate.
3. **Repeat-offender pattern** — same `cc_session_id`, IL1 on multiple *different*
   files (standing habit) vs. once-and-corrected (acceptable one-call tax).
4. **Subagent-dispatch correlation** — is IL1 concentrated in the first few calls of
   a session (a dispatch-briefing gap, not a tool or gate problem)?

**Deliverable:** an evidence memo, not a fix. It feeds a *follow-up* brainstorm on the
fail-loud-vs-auto-redirect fork named in Problem/Scope above — that fork is not
decided by this spec.

---

## What Is Not Changing

- **The fail-loud-vs-auto-redirect fork stays undecided.** Explicitly deferred per
  this session's brainstorm — Component 3 produces the evidence a future design
  needs to choose; this spec does not choose for it.
- **No general silent-success anomaly detector.** Component 2's detector is a closed,
  curated marker list (two markers today), not a fuzzy "output smaller than typical"
  heuristic — no second concrete shape beyond today's two bugs has been found yet to
  justify that generalization (Operating Principle 4).
- **IL2 (`edit_file` structural), IL3 (shell/pipe misuse), `edit_stale_match`.**
  These already have shipped hookify gates (H-1/H-2/H-3 in
  `docs/trackers/codescout-usage-hookify.md`) and are a known, already-addressed
  class — still high-volume, but not this spec's job; a candidate for a future
  spec once Component 3's methodology has been validated once on IL1.
- **`run_command`'s expired-buffer-ref and worktree-relative-`cwd` errors** — noticed
  during the mining pass, real but minor, not sequenced into this spec; worth a
  standalone `docs/issues/` bug file if picked up later.

## Prompt Surface Note

None of these three components touch a prompt surface (`server_instructions.md`,
`builders.rs`, or the codescout-companion hooks) — Component 1 and 2 are pure
observability/tooling; Component 3 produces no code change at all. No
`ONBOARDING_VERSION` bump needed.
