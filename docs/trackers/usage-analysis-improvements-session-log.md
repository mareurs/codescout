# Session Log — Usage-Analysis Improvements (2026-05-27)

> **Topic:** Improvements derived from /analyze-usage report 2026-05-27.
> **Report:** `docs/usage-reports/2026-05-27-usage-analysis.md`
> **Scope:** I-1 (prompt-surface anti-pattern leads), I-5 (grep overflow hint), I-6 (read_file start_line default).

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-05-27 | med | architectural | mitigated | Cargo incremental cache masked prompt-surface drift; snapshot test passed on stale binary |
| F-2 | 2026-05-27 | med | codescout-tool | open | IL3 piping habit recurred 6× in single turn despite warn-mode prompts (hookify candidate H-1) |
| F-3 | 2026-05-27 | low | plan-prose | open | CLAUDE.md cites non-existent Kotlin LSP issue tracker path |
| F-3a | 2026-05-27 | med | architectural | open | I-4 retry coverage incomplete for edit_code (27/43 disconnect errors unaddressed; pairs with I-2 deeper redesign) |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-27 | med | scout-response-shape-before-adding-field | would have added invisible/redundant by_file field, ~1 round-trip cost | validated |
| W-2 | 2026-05-27 | med | scout-for-existing-helper-before-designing-new | would have built duplicate retry helper, ~half-day rework | validated |
| W-3 | 2026-05-27 | med | reframe-redesign-as-selection-policy | I-2 "redesign" → 10-line cost-aware LRU change instead of multi-day Kotlin LSP rewrite | validated |
---

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## W-1 — Recon caught that grep already exposed per-file counts; pivoted from "add by_file field" to "enrich vague hint"

**Observed:** 2026-05-27, mid-implementation of usage-analysis backlog item I-5
("grep overflow includes by_file distribution"). About to edit
`src/tools/grep.rs:Grep::call` to add a new `by_file` array to the `overflow`
JSON object.

**Pattern:** Before adding a new field to a tool's response shape, scout (a) what
the response already carries and (b) the text-rendering path that surfaces JSON
fields to the LLM. Two adjacent reads (`groups_to_json`, `format_overflow`)
revealed that a new JSON field would have been invisible AND redundant.

**Counterfactual:** Without this scout, the change would have:
1. Added a `by_file: [...]` array to the `overflow` block in `Grep::call`.
2. Failed user-visible verification because `format_overflow` only renders
   `shown`/`total`/`hint` — the new array would appear in JSON but not in the
   text Claude Code actually sees. The follow-up complaint ("the hint still
   says 'narrow the pattern', the by_file field doesn't show up") would have
   triggered a second round of code edits to either change `format_overflow`
   or move the data into `hint`.
3. Surfaced redundant information — `file_groups[{file, count, items}]` is
   already present in simple-mode responses; a separate `by_file` would duplicate
   the per-file counts already there. PROGRESSIVE_DISCOVERABILITY.md
   Anti-Pattern 1 ("early exit without metadata") and Pattern 2 ("show
   distribution before content") both already satisfied for the JSON consumer;
   the gap is purely in the human/LLM-visible *text* surface.

**Confirming data points:**
1. `src/tools/file_group.rs:165-189 groups_to_json` — `file_groups` already emits
   `{file, count, items}` per file, sorted by count desc (via `group_by_file` at
   line 24).
2. `src/tools/format.rs:32-41 format_overflow` — text rendering reads only
   `shown` / `total` / `hint`. Any new JSON field is dropped.
3. `docs/PROGRESSIVE_DISCOVERABILITY.md` Pattern 1: "Every overflow hint must be
   a concrete, copy-paste-ready tool call example." The current grep overflow
   hint "Many matches. Narrow the pattern or use a more specific path." is the
   exact anti-example Pattern 1 forbids — vague, no parameter name with a real
   value.

**Impact:** med — saves ~1 round-trip of failed surface change + correct
diagnosis on first pass. Also surfaces the deeper finding: even tools that
satisfy progressive-disclosure on the JSON axis can violate Pattern 1 on the
text axis.

**Promote-when:** A second instance where the text rendering of a tool fails
to surface JSON data the model needs. At 2 datapoints, promote to
`docs/PROGRESSIVE_DISCOVERABILITY.md` as Pattern 1b: "Hints are text-rendered;
any data the model must act on must appear inside the `hint` string, not as a
sibling JSON field."

**Status:** validated — single datapoint, pivot landed before any code-shape
change. Awaiting promotion criterion.
## F-1 — Cargo incremental cache masked prompt-surface drift; snapshot test passed on stale binary

**Observed:** 2026-05-27, mid-verification of the usage-analysis bundle.
Just edited `src/prompts/source.md` (the @surface server_instructions slice)
to add gate-error mnemonics to the Iron Laws.

**When:** Running `cargo build` + `cargo clippy --all-targets` + `cargo test --lib`
in sequence. Touched `src/prompts/source.md` between iterations.

**Expected (assumed):** After editing source.md, the next `cargo test` rebuilds
the binary; build.rs re-slices source.md into OUT_DIR; SERVER_INSTRUCTIONS
constant (via include_str!) reflects new content; the snapshot test
(`prompts::tests::prompt_surfaces_server_instructions_snapshot`) panics with
"prompt surface drift" because the fixture file at
`tests/fixtures/prompt_surfaces/server_instructions.md` still has old content.

**Got (reality):** `cargo test --lib` reported the snapshot test as `ok` THREE
TIMES in a row — once after the initial edit, once after `touch source.md`,
once after the second edit. Disk state confirmed:
- source.md: NEW content (21145 bytes, hash d92cf...)
- OUT_DIR `target/debug/build/codescout-fa2c25c4bf00d339/out/`: NEW content
  (hash 4bbd104...) — proving build.rs DID re-run with new source
- OUT_DIR `target/debug/build/codescout-2be8f56cb543e677/out/` (the most-
  recently written, matching test binary mtime): OLD content (hash 0684a5d...)
- fixture file: OLD content (1755 bytes, hash 0684a5d...) — matches the
  stale OUT_DIR exactly
- test binary `strings` output: contains BOTH old phrase and new phrase

**Diagnostic:** Cargo's incremental compilation has multiple OUT_DIRs per
package (one per fingerprint hash — 10 dev OUT_DIRs were present here for
codescout alone). The build script ran in one OUT_DIR with new content, but
the test binary was linked against include_str! pointing at a different,
stale OUT_DIR that still had old content. cargo's fingerprint didn't invalidate
the link, so the binary kept reading old content via the stale OUT_DIR path
baked at compile time. Even `touch source.md && cargo test` didn't surface
the drift — possibly because of how mtime granularity interacts with
fingerprint hashing.

**Resolution:** `cargo clean -p codescout` (cleared 45GB of artifacts). The
very next `cargo test --lib prompt_surfaces` then FAILED with the expected
"prompt surface drift" panic:
```
expected: 1755 bytes (old fixture)
actual:   2201 bytes (new slice)
```
Confirming the new edit DID land — only the test build was caching the old
binary linkage.

**Severity:** med — a regression in snapshot test reporting was masked.
Any prompt-surface change without `cargo clean` could ship without snapshot
review. Caught here only because the user invoked the codescout-pika
specialist mid-turn, who flagged the suspicious "test passed despite edit"
state.

**Status:** mitigated — verification now requires `cargo clean -p codescout`
before snapshot tests when prompt surfaces have been edited.

**Fix idea / Pointer:** Either (a) document this in CLAUDE.md under "Prompt
Surface Consistency" as a required step before claiming snapshot tests pass,
or (b) wire build.rs to emit a stable cargo:rerun-if-env-changed signal that
forces relinking, or (c) bake a content hash of source.md into a separate
const that the snapshot test cross-checks. Option (a) is cheapest and aligns
with the existing "Always run `cargo fmt`, `cargo clippy`, and `cargo test`
before completing any task" rule.

## F-2 — IL3 piping habit recurred 6 times in single turn despite warn-mode prompts

**Observed:** 2026-05-27, throughout the usage-analysis bundle implementation.

**When:** Each time I needed to inspect command output (test results, file
contents, search hits). Reflexively wrote `cmd | tail -N`, `cmd | head -N`,
`cmd 2>&1 | tail`, etc.

**Expected (Iron Law 3):** Run command bare → result lands in `@cmd_*`
buffer → query the buffer with `grep PATTERN @cmd_xxx` or `tail -N @cmd_xxx`.

**Got (reality):** Six warn-mode IL3 hits in one turn — `grep | head -20`,
`cargo fmt --check 2>&1 | tail -20`, `find tests | head -20`,
`strings ... | grep ...` (twice), `wc -c file | md5sum | echo | cargo test ...`.
One actual BLOCK (the cargo fmt one — the gate fired hard, not just warned).

**Diagnostic:** Hand-of-habit. The mental model that produces shell commands
defaults to "pipe to a trimmer" because that's the Linux idiom outside
codescout. The gate's warn message is informational but the model's pipeline-
construction reflex is faster than the gate-text reading reflex.

**Severity:** med — every slip costs a wasted tool call (the unfiltered_output
buffer is still created) AND a warn/deny that goes in the transcript. The
buffer-query workflow is genuinely faster than re-piping; the slip is pure
muscle memory.

**Status:** open — needs substrate fix or stronger session-start prompting.

**Fix idea / Pointer (Hookify proposal H-1, candidate for codescout-companion):**

> **Predicate:** Bash tool call where command matches
>   `(cargo|npm|yarn|pytest|pnpm|go|cmake|make|git|rg|fd) [^|]* \| (head|tail|wc|sort|uniq|grep -[clm])`
>   AND the LHS would normally produce >1KB output.
> **Decision:** deny (currently warn).
> **Reason text:** "IL3: run `<lhs>` bare → query the returned @cmd_* buffer
>   with `<rhs> @cmd_xxx`. Six slips this session — promoting to deny."

The current warn → deny escalation works in principle but the warn->deny gate
threshold seems set too high (5+ slips in a turn didn't promote to deny).
A faster escalation (2 slips → deny for the rest of the turn) would land
the habit sooner. Tracker artifact for hookify candidates lives at
`docs/trackers/codescout-usage-hookify.md` if/when promoted.
## F-3 — CLAUDE.md cites non-existent Kotlin LSP issue tracker path

**Observed:** 2026-05-27, during I-2 (Kotlin LSP scoping). CLAUDE.md line 570
cites `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` as the
authoritative tracker for the Kotlin multi-instance issue.

**Got:** Path returns "file not found". `tree --glob "docs/issues/**/*kotlin*"`
finds only `docs/issues/archive/2026-04-24-find-symbol-kotlin-multi-session.md`
(different topic, archived). The cited tracker either never existed or was
archived under a different name; no path-rewrite was applied to CLAUDE.md.

**Diagnostic:** Classic doc-drift — CLAUDE.md was authored with a forward
reference, the file was never created (or got renamed/archived), and the
audit_doc_refs check didn't run on CLAUDE.md (the default scan covers
`docs/**/*.md` plus `CLAUDE.md`, so it should have caught this — possibly
silenced by `verdict=ambiguous_basename` resolution).

**Severity:** low — wastes an LLM round-trip when it tries to follow the
cite, and could mislead future investigation.

**Status:** open — needs (a) a real Kotlin-LSP tracker file at the cited path
OR (b) a CLAUDE.md edit to remove the dangling reference. Defer to next
`audit_doc_refs` run.

**Fix idea:** Run `librarian(action="audit_doc_refs")` after this commit
ships; the audit's `missing` verdict on this ref will surface the same issue.

## W-2 — Pre-edit scout surfaced existing retry_on_mux_disconnect infrastructure

**Observed:** 2026-05-27, planning I-4 (LSP disconnect single-retry).
Initial plan: design retry logic from scratch wrapping each tool's LSP calls.

**Pattern:** Before designing new infrastructure for a problem that "feels
like" existing code might already handle, grep for related verbs (here:
"retry", "reconnect", "alive") across the LSP layer. Took one
`semantic_search(query=...)` and one `grep -context=3 retry_on_`.

**Counterfactual:** Without the scout, I would have:
1. Designed a new retry helper (~30-60 min of design)
2. Implemented it in a new module
3. Migrated tool call sites to use it
4. Possibly introduced a duplicate-purpose helper that the next code review
   would have to consolidate against the existing `retry_on_mux_disconnect`.

**Confirming data points:**
1. `src/fs/mod.rs:299` already exposes `retry_on_mux_disconnect<F, Fut, T>`.
2. `src/tools/symbol/symbol_at.rs` already uses it for `goto_definition`
   and `hover` — production callers established the pattern.
3. `is_mux_disconnect` (line 288) is the gate; broadening it to match the
   broader "LSP server disconnected" error was a 4-line change.

**Impact:** med — saved a ~half-day rework cycle. More broadly: the codebase
has a culture of small, well-named helpers; semantic_search + grep for
related verbs is high-yield before designing new infra.

**Promote-when:** Three observations of "I almost built X from scratch; a
two-minute scout found X already existed". At three, promote to CLAUDE.md
as "Before designing new helper/wrapper code, semantic-search the related
verb across the touched layer — codescout's helper culture is dense."

**Status:** validated — single datapoint; awaiting recurrence to promote.

## W-3 — Cost-aware LRU eviction with tier helper, ~10-line change for I-2

**Observed:** 2026-05-27, implementing I-2 (Kotlin LSP cold-start cost).
The "redesign" framing in the original report suggested multi-day work
(redesigning startup, lazy indexing, JVM warm cache). Reality: most of the
cost surfaces as `lru_evicted` (avg 24s) — Kotlin getting kicked out of
the pool by cheap-to-restart languages.

**Pattern:** Frame expensive-resource-management problems as a *selection*
choice before a *redesign* choice. Often a single helper (`restart_cost_tier`)
and a one-line change to a sort comparator (`min_by_key`) shifts the
trade-off entirely without touching the expensive subsystem.

**Counterfactual:** Without the cost-aware tier framing, the "real" I-2
fix would have required: investigating kotlin-language-server startup,
profiling JVM init, experimenting with lazy stdlib loading, possibly
forking kotlin-language-server. Multi-day. Instead: 1 helper fn + 1 sort
key change in `src/lsp/manager.rs::LspManager::get_or_start`. Targets the
exact failure mode (3 lru_evicted cases × 24s avg) without addressing
the deeper cold-start time.

**Confirming data points:**
1. Pool LRU selector at `src/lsp/manager.rs:312-318` was strict-LRU
   (`min_by_key(|(_, t)| *t)`).
2. `ttl_for_language` already had Kotlin-specific tuning (2-hour idle TTL
   vs default), so per-language behavior was already a established pattern
   in the file.
3. The 3-case lru_evicted in the report × 24s = 72s of friction targeted
   by a 10-line change.

**Impact:** med — eliminates one of three Kotlin friction modes (lru_evicted
specifically). Idle_evicted (11.7s avg) and new_session (5.3s avg) remain
as full-redesign territory.

**Promote-when:** A second case where reframing "redesign" → "selection
policy" produces a 10x effort reduction. Promote to a CLAUDE.md heuristic
"Before estimating a redesign, ask whether the cost lives in the selection
criterion rather than the underlying resource."

**Status:** validated — single datapoint; awaiting recurrence to promote.

## F-3a (deferral) — I-4 retry coverage incomplete for edit_code

**Observed:** 2026-05-27, while landing I-4 (LSP disconnect single-retry).

**When:** Implementing retry_on_mux_disconnect wraps across tools.

**Expected:** Apply retry to all LSP-touching tools — including edit_code
(27 errors in usage report, the highest count of any tool for "LSP server
disconnected").

**Got (deferred):** edit_code's four action methods (do_rename, do_replace,
do_remove, do_insert) each make 2-4 LSP calls in sequence, sometimes via
`fetch_validated_symbol` which has its own internal retry budget for stale
positions (3 tries with did_change + backoff). Wrapping these with retry
correctly requires:
- Either move the entire action body into the retry closure (LARGE change,
  needs careful idempotency analysis — the file write at the end is NOT
  idempotent)
- Or wrap each LSP call individually with retry, fragmenting the existing
  fetch_validated_symbol cohesion

Both paths warrant a small ADR. Deferring to a follow-up that pairs naturally
with I-2's continued Kotlin LSP work (most edit_code disconnects are Kotlin).

**Severity:** med — leaves 27 of 43 disconnect errors (63%) unaddressed by
the cheap retry; symbols (16 errors) and references are covered, but the
highest-volume offender is not.

**Status:** open — pinned for follow-up alongside the deeper I-2 redesign.

**Fix idea / Pointer:** When implementing the full I-2 (lazy Kotlin indexing
or single-instance pinning), revisit edit_code retry strategy as part of
that work — the LSP behavior under the new design will inform whether
retry is even needed, or whether the disconnects disappear at the source.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
