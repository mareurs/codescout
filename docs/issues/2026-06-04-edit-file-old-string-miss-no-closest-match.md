---
status: open
opened: 2026-06-04
closed:
severity: low
owner: marius
related:
  - docs/usage-reports/2026-05-29-usage-analysis.md
tags: [edit_file, error-ux, kotlin, whitespace, telemetry]
kind: bug
---

# BUG: edit_file old_string-not-found returns a non-actionable error despite holding the file content

## Summary
When `edit_file`'s `old_string` fails to match (exact-byte miss, almost always a
leading-whitespace/indentation difference on multi-line blocks), the tool returns a
generic `"Check whitespace and indentation — use grep to verify"` hint. The file
`content` is already in memory at that point, and the sibling `match_count > 1`
branch already demonstrates the actionable-error pattern — yet the zero-match branch
surfaces nothing about what the file *actually* contains. The agent is forced into a
`grep` + `read_file` + retry recovery loop (~2–3 wasted calls) on every miss.

## Symptom (Effect)
Verbatim error returned to the LLM on a zero-match `edit_file`:

```
old_string not found in ktor-server/src/main/kotlin/edu/planner/solver/services/stage2/WeeklyPatternFinder.kt — hint: Check whitespace and indentation — old_string must match exactly. Use grep to verify the exact text.
```

The hint names the *category* of problem (whitespace) but not the *location* or the
*actual text* near the intended edit — even though both are computable from the
already-loaded file content. Compare the `match_count > 1` branch, which DOES surface
specifics: `old_string found 3 times (lines 12, 88, 140)`.

## Reproduction
Deterministic. On any repo:

1. `git rev-parse HEAD` → `0930e3a6` (branch `experiments` at time of filing).
2. Pick a multi-line block in a source file that has leading indentation (e.g. a KDoc
   `* ` continuation block or a nested function body).
3. Call `edit_file(path, old_string=<that block, but with one line's leading whitespace
   off by a space, or a straight quote where the file has a curly quote>, new_string=…)`.
4. Observe: `old_string not found in <path> — hint: Check whitespace and indentation…`
   with no indication of what the file actually holds at the near-match region.

## Environment
- codescout `experiments` @ `0930e3a6`; `src/tools/edit_file/mod.rs`.
- Observed in production telemetry from `mirela/backend-kotlin` `weekly-pattern`
  worktree (`.codescout/usage.db`), 2026-05-27 → 2026-06-03, Kotlin source.
- Tool-agnostic: the branch has no language dependence; Kotlin just surfaces it most
  (deep nesting + KDoc continuation markers).

## Root cause
`perform_edit` in `src/tools/edit_file/mod.rs:453-539`. After
`let content = std::fs::read_to_string(&resolved)?;` and
`let match_count = content.matches(old_string).count();`, the zero-match branch
(`src/tools/edit_file/mod.rs:479-484`, the `format!`/hint at `:481-482`) returns a
static hint and discards `content` without searching it for a near-match:

```rust
if match_count == 0 {
    return Err(super::RecoverableError::with_hint(
        format!("old_string not found in {path}"),
        "Check whitespace and indentation — old_string must match exactly. Use grep to verify the exact text.",
    )
    .into());
}
```

The asymmetry is the tell: the `match_count > 1` branch immediately below computes and
surfaces exact line numbers for every match. The "spend a few cycles to make the error
actionable" pattern already exists in this function — it simply isn't applied to the
zero-match case, which is the more common and more expensive failure.

This is not a correctness defect — exact-byte matching is the intended, safe behavior.
It is an error-UX gap: the cheapest possible recovery information (what the file
actually holds near the intended edit) is in hand and thrown away.

## Evidence

### Telemetry — systemic, not localized (weekly-pattern usage.db)
10 `edit_file` "old_string not found / Check whitespace" failures across **10 distinct
files**, all under `ktor-server/src/main/kotlin/edu/planner/`, spread over 8 days
(2026-05-28 → 2026-06-03). Not a repeated hammer on one file — a steady tax on
multi-line Kotlin edits. Query:

```sql
SELECT id, substr(error_msg, instr(error_msg,'in ')+3,
       instr(error_msg,' — hint')-instr(error_msg,'in ')-3) file
FROM tool_calls
WHERE tool_name='edit_file' AND outcome='error'
  AND error_msg LIKE '%Check whitespace%' ORDER BY id;
-- 621,626,653,915,1218,1474,1480,1563,2518,3493 → 10 distinct files
```

### Recovery cost — bounded but repeated
Every one of the 10 self-heals in 1–3 calls via the same loop: `grep`/`read_file` to
fetch exact bytes → re-edit succeeds. e.g. ids 620–624:
`edit_code:err > edit_file:err(ws) > grep > read_file > edit_file:ok`. ≈2–3 wasted
calls × 10 ≈ ~25 calls over 8 days.

### The one recoverable payload (id 2518, `WeeklyPatternFinder.kt`)
`input_json` capture is version-gated (see below); 9 of 10 predate it. The single
captured `old_string` was a 4-line KDoc block — every line carries `     * `
(5 spaces + `*` + space) plus em-dashes (`—`) and curly quotes (`"`). The prose
reproduces easily from memory; the per-line leading whitespace and unicode punctuation
do not. This was a pure comment edit, so `edit_code` does not apply — `edit_file` was
the correct tool; only the exact-match brittleness + unhelpful error bit.

### Refuted hypotheses are in "Hypotheses tried" below.

### Telemetry caveat (secondary finding)
`tool_calls.input_json` / `output_json` capture is gated by codescout build SHA. Zero
capture before `ba11a959` (2026-05-30); zero again on `b0fd368b` (the
`feat/per-request-workspace-pinning` branch, which lacks the capture commit). Switching
the live binary to a branch without the capture commit silently regresses forensic
telemetry. Worth ensuring the capture commit is on every branch the live binary runs.

## Hypotheses tried
1. **Hypothesis:** 10 failures were repeated retries on one file (`Stage2WeeklySolverService.kt`).
   **Test:** full-path extraction per row vs the coarse `GROUP BY substr(error_msg,1,60)`.
   **Verdict:** rejected — the coarse group truncated at the shared path *prefix*; the
   true breakdown is 10 distinct files. **Evidence:** "systemic, not localized" above.
2. **Hypothesis:** whitespace miss is a symptom of a preceding `edit_code` failure
   (agent falls back to `edit_file`). **Test:** self-join each fail to its `id-1`
   predecessor. **Verdict:** rejected as the general cause — only 1 of 10 (id 621) was
   preceded by an `edit_code` error; the rest follow `symbols`/`read_file`/`edit_file`
   *successes*. **Evidence:** predecessor query, 3× symbols / 2× read_file / 3× edit_file.
3. **Hypothesis:** reading the file first prevents the miss. **Test:** check predecessors.
   **Verdict:** rejected — 2 of 10 (915, 1474) mismatched *immediately after* a successful
   `read_file`. Strongest evidence the failure is in `old_string` *construction*
   (copying leading whitespace), not staleness.

## Fix
**Plan (not yet implemented).** In the `match_count == 0` branch of `perform_edit`
(`src/tools/edit_file/mod.rs:479-484`), before returning, search the already-loaded
`content` for a near-match of `old_string`:

- Whitespace-normalized search (collapse/strip each line's leading whitespace on both
  sides) to locate candidate region(s).
- If a unique near-match is found, include its line range and the file's *actual* bytes
  for that region in the error (a compact "you sent X / file has Y" diff), mirroring the
  specificity the `match_count > 1` branch already provides.
- Do NOT auto-apply a normalized match (ambiguity risk → silent wrong edit). This is an
  error-payload improvement only; matching behavior stays exact-byte.

This collapses the `grep` + `read_file` + retry loop into the error response itself —
the agent corrects in one retry. Aligned with codescout's progressive-disclosure /
actionable-hint philosophy (`docs/PROGRESSIVE_DISCOVERABILITY.md`).

## Tests added
N/A — not yet fixed. When implemented: a unit test in `src/tools/edit_file/mod.rs`
`tests` module that (a) feeds an `old_string` differing only in leading whitespace from
file content, (b) asserts the error payload now contains the actual near-match line
range + bytes (not just the static whitespace hint).

## Workarounds
On a zero-match `edit_file`, the agent already recovers reliably (1–3 calls): `grep` the
distinctive content fragment, `read_file` the region to get exact bytes, retry
`edit_file`. For prose/comment edits, this is the only path; for structural edits,
prefer `edit_code` (whitespace-independent) where it applies.

## Resume
Implement the fix: edit the `match_count == 0` branch in
`src/tools/edit_file/mod.rs:479-484`. Add a helper that does a leading-whitespace-
normalized line-window search over `content` for `old_string`; on a unique hit, build a
`RecoverableError` whose message includes the actual region (`lines X-Y`) and a short
actual-vs-expected diff. Keep the static hint as a fallback when no near-match is found.
Add the regression test described in "Tests added". Verify no behavior change on the
exact-match and `match_count > 1` paths (`cargo test --lib edit_file`).

## References
- `src/tools/edit_file/mod.rs:453-539` (`perform_edit`); zero-match branch `:479-484`,
  actionable multi-match branch immediately below.
- `docs/usage-reports/2026-05-29-usage-analysis.md` — prior cross-project analysis;
  ranked `edit_file` frictions #1 (structural redirect) but did not surface this
  closest-match gap. Corroborates recurrence.
- Evidence corpus: `mirela/backend-kotlin/.worktrees/weekly-pattern/.codescout/usage.db`
  (`tool_calls`), 2026-05-27 → 2026-06-04.
- `docs/PROGRESSIVE_DISCOVERABILITY.md` — actionable-hint design philosophy this fix follows.
