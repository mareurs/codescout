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
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-27 | med | scout-response-shape-before-adding-field | would have added invisible/redundant by_file field, ~1 round-trip cost | validated |
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
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
