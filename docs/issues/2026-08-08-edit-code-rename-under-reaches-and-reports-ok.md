---
kind: bug
status: fixed
tags:
- edit-code
- rename
- lsp
- kotlin
- write-path
- silent-failure
closed: 2026-08-08
opened: 2026-08-08
owner: marius
related: []
severity: high
---

# BUG: `edit_code(action="rename")` renames only the declaration and returns `status: "ok"` — call sites are left behind, and the response's own payload already names them

## Summary

`edit_code(action="rename")` derives its edit set from LSP reference resolution alone. When the language server returns no cross-file references — measured and reproducible for Kotlin in a real project — the rename edits only the declaration file and returns `status: "ok"`. The call sites it missed are present **in the same response**, under `textual_matches` with `kind: "source"`, and are neither edited nor flagged. The caller is handed a tree that no longer compiles, labelled a success.

This matters more than the equivalent read-path bug: `references` returning a false negative costs a wasted lookup, and it already **warns** when grep disagrees with it. `rename` computes the same disagreement, does not warn, and writes.

## Symptom (Effect)

Renaming a Kotlin `object` with four cross-file consumers. Verbatim response (trimmed to the decision-bearing fields):

```json
{
  "status": "ok",
  "old_name": "Stage1DateUtils",
  "new_name": "SchedulingDateUtils",
  "files_changed": 1,
  "total_edits": 2,
  "textual_match_count": 45,
  "textual_files_total": 13,
  "sweep_skipped": false,
  "verify_hint": "LSP rename may match occurrences inside string literals, comments, or macro arguments. Verify each changed file is still valid (e.g. cargo check / tsc --noEmit)."
}
```

`files_changed: 1` is the declaration file. The same response carried these entries, untouched:

```json
{ "file": ".../validation/MonthlyFeasibilityEvaluator.kt",
  "lines": [6, 982, 985, 986], "occurrence_count": 4, "kind": "source" },
{ "file": ".../validation/FeasibilityValidationService.kt",
  "lines": [19, 406], "occurrence_count": 2, "kind": "source" },
{ "file": ".../validation/FeasibilityValidationServiceSGCalendarTest.kt",
  "lines": [131, 133, 141, 233, 235, 351, 353], "occurrence_count": 7, "kind": "source" },
{ "file": ".../validation/InfeasibleSubjectsInvestigationTest.kt",
  "lines": [155, 249], "occurrence_count": 2, "kind": "source" }
```

Those are an `import` line plus fully-qualified call sites (`edu.planner.solver.utils.Stage1DateUtils.getAvailableDatesForMonth(...)`). Left as-is, `./gradlew compileKotlin` fails with `Unresolved reference`.

**The `verify_hint` points the wrong way.** It warns about *over*-reach (string literals, comments) in *changed* files. The actual failure was *under*-reach in *unchanged* files. A caller following the hint verifies the one file that was edited and concludes the rename is sound.

## Reproduction

Any project where the language server does not resolve cross-file references for the target symbol. Reproduced deterministically on Kotlin:

1. Open a Kotlin project whose consumers reach a top-level `object` via `import` or FQN.
2. `edit_code(symbol="<Object>", path="<decl file>", action="rename", new_name="<New>")`
3. Observe `status: "ok"`, `files_changed: 1`, and `kind: "source"` entries in `textual_matches` that were not edited.
4. Build. It fails.

Direct confirmation of the underlying resolution failure, independent of `rename`:

```
references(symbol="SchedulingDateUtils",
           path="ktor-server/src/main/kotlin/edu/planner/solver/utils/SchedulingDateUtils.kt")
→ 1 result (the declaration only)
→ warning: LSP returned 0 references outside the definition file, but `SchedulingDateUtils`
  appears as a whole word in 4+ other source file(s) … the reference index may still be
  warming after a reindex. Re-run, or corroborate with grep …
```

## Environment

- codescout **0.15.0** (binary), repo `aaebe4cf4f846cab270593de957c518e30b83ed1`, branch `experiments`
- Kotlin language server: `/usr/bin/kotlin-lsp`, server home `~/.cache/codescout/kotlin-lsp-home`
- Project: EDU-Planner `backend-kotlin`, Kotlin/Gradle, in a linked git worktree (`workspace` param pinned explicitly on the call)
- Linux, MCP over stdio
- Tree fully built (`./gradlew test` green) before and after — this is not a cold-project artifact

## Root cause

Two layers. The second is the reportable defect; the first only explains why it fired here.

**1. The Kotlin LSP resolves no cross-file references for this symbol.** `measured 2026-08-08: references(symbol="SchedulingDateUtils", …) → 1 result (declaration only) + grep-disagreement warning; re-run immediately after, identical result.` The warning text attributes this to a warming index and advises a re-run; the re-run reproduced it exactly, so warming does not explain it. The tree had been compiled green by Gradle many times in the same session.

**2. `rename` treats the LSP edit set as complete and its own textual sweep as advisory.** `sweep_skipped: false` — the sweep ran, classified four files as `kind: "source"`, and the rename neither incorporated nor flagged them, returning `status: "ok"`. `inferred from the response payload — the rename implementation was not read.`

The asymmetry is the point: **codescout already owns a grep-vs-LSP disagreement heuristic and applies it to the read tool but not the write tool.** `references` emits a warning naming the disagreeing files. `rename` has strictly more information (it has the classified `textual_matches` in hand) and says nothing.

## Evidence

### `rename` response, this session

Fields quoted verbatim under *Symptom*. Note `sweep_skipped: false` and `textual_match_count: 45` alongside `files_changed: 1`.

### `references` disagreement warning, same symbol, same session, twice

Quoted under *Reproduction*. Two consecutive calls, identical output — rules out the "index still warming" explanation the warning itself offers.

### Post-rename grep, which is what actually caught it

`grep(pattern="Stage1DateUtils", glob="*.kt", mode="files")` → `0 matches` **only after** four call sites were repaired by hand. Before the repair the same grep returned the four files. The rename was caught by an independent check the tool did not prompt for.

## Hypotheses tried

1. **Hypothesis:** the LSP index was still warming when `rename` ran, so this is transient.
   **Test:** `references` on the renamed symbol, twice in a row, on a fully-built tree well after the rename.
   **Verdict:** rejected — identical zero-reference result both times.
   **Evidence:** *references disagreement warning* above.

2. **Hypothesis:** the missed sites are FQN/import-only and therefore not real references the LSP is obliged to return.
   **Test:** inspect the missed lines.
   **Verdict:** rejected as a defence — `import edu.planner.solver.utils.Stage1DateUtils` and `edu.planner.solver.utils.Stage1DateUtils.getAvailableDatesForMonth(...)` are ordinary references, and the build breaks without them. It may still explain the *LSP's* behaviour, but it does not make `status: "ok"` correct.

3. **Hypothesis:** the caller is expected to verify, so the tool is behaving as designed.
   **Test:** read `verify_hint`.
   **Verdict:** rejected — the hint directs verification at over-reach in *changed* files ("string literals, comments, macro arguments"), which is the opposite failure. Nothing in the response suggests checking for *unchanged* files.

## Fix

**Implemented on `experiments`.** The implementation was read before choosing, and reading
it **falsified both of the options this section originally proposed**. Recorded here rather
than quietly substituted, because the reasoning is the useful part.

### What reading changed

**(b) — rename the union — is unsafe.** `TextualMatch::kind` classifies the **file**, not
the occurrence. A comment mentioning the name inside a `.rs` file is `kind: "source"` —
pinned by the pre-existing `text_sweep_finds_matches_in_comments_and_docs`. Renaming every
`kind: "source"` match would rewrite comments and any unrelated symbol that happens to
share the name: a silent **over**-reach traded for a silent under-reach.

**(a) — refuse and require an opt-in — is not available where the check lands.** The LSP
edits are committed with `std::fs::write` **before** the sweep runs (`edit_code.rs`, writes
at the `plan` loop, sweep ~80 lines later). Refusing would mean rolling back a successful
multi-file write on a heuristic. The rollback machinery exists (`PlannedWrite::pre_image`)
but is there for a *failed* write, and firing it on a guess is a worse trade than reporting
accurately.

**Gating on "any source match" is also wrong** — same reason as (b). It would refuse a
rename because a docstring says the word.

### What landed instead

The rename that happened is correct as far as it goes. What was wrong is that it was
**reported as complete**. So: turn a silent under-reach into a loud one, and leave the
writes alone.

**1. `EditCode::rename_under_reached`** — the decision, extracted as a pure function so it
is testable without a language server. It is the `references` rule ported to the write
path, not a new invention: *the LSP reached nothing outside the declaration file* **and**
*other source files spell the name*. Narrow by construction — it stays silent whenever the
LSP did resolve cross-file references, which is the common case.

**2. `status: "incomplete"`** instead of `"ok"`, plus `completeness_warning` naming the
files and `uncovered_source_files` listing them.

**3. `verify_hint` direction corrected.** When under-reach is present it now names it
first. The old text warned only about over-reach *in changed files*, so a caller following
it verified the one file that was edited and concluded the rename was sound — actively
misleading in exactly the case that hurts.

**4. `format_rename_symbol` — a defect this bug file had not found, and the worse one.**
The compact renderer **summed** `total_edits + textual_match_count` and called the result
"sites". Those are opposites: the first counts occurrences the rename rewrote, the second
counts occurrences it did not (the sweep excludes every LSP-edited file). The reported
case rendered as `→ SchedulingDateUtils · 47 sites` — 2 renamed, 45 missed, displayed as a
larger success, **on the surface an agent actually reads**. It now reports what was
renamed, and says INCOMPLETE with a count when the gate fires.

**Still separate — Kotlin cross-file reference resolution.** Untouched, as this section
originally argued: any language server can under-report and the write path must be robust
regardless. Now it is.
## Tests added

Two, both pure and both control-bearing:

- `rename_under_reach_fires_only_on_the_lsp_disagreement` — the measured failure, plus
  **three controls** that are the real content. Cross-file LSP edits present → silent (a
  leftover source match there is a comment, not an under-reach; without this clause the
  gate fires on most healthy renames). Documentation and config matches only → silent (a
  genuinely unused symbol; nothing to break). Nothing anywhere → silent.
- `rename_compact_output_does_not_add_unrenamed_matches_to_renamed_ones` — asserts the
  summed figure is gone (`!out.contains("47")`), that what was renamed is reported, that
  the incomplete state is legible, and — the control — that a clean rename still renders
  exactly `→ Renamed · 9 sites · 3 files`, so the warning cannot become ambient noise.

Not covered by a test: the end-to-end `do_rename` path, which needs a live language server
reproducing the under-report. The decision it turns on is fully covered above; what is not
is the wiring between them.

Gate: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 3576
passed / 0 failed / 44 ignored.
## Workarounds

After **every** `edit_code(action="rename")`, grep the old name to zero before trusting the result:

```
grep(pattern="<OldName>", glob="*.<ext>", mode="files")   → expect 0 matches
```

If it is non-zero, repair the remaining sites with `edit_file(replace_all=true)` per file. Do not rely on `files_changed`, on `status: "ok"`, or on the build being green in an IDE that resolves differently.

Renaming a class in a file-per-class language also needs `git mv` for the file itself — `rename` does not move it.

## Resume

Fixed on `experiments`. Remaining:

1. **Confirm CI**, then archive via `artifact(action="move", …)`. No master-side SHA to
   record — this cohort promotes by fast-forward (`docs/RELEASE.md` § *Large-Cohort
   Promotion*).
2. **File the Kotlin cross-file resolution failure separately** if it is not already
   tracked — `/usr/bin/kotlin-lsp` returning zero cross-file references for a top-level
   `object` in a tree that compiles green. It is a genuine second defect; this fix only
   makes the write path survive it.

A related surface left alone deliberately: the sweep is skipped entirely for names shorter
than 4 characters (`old_name_str.len() < 4`), so a short symbol gets no under-reach check
at all. That is a pre-existing cap on the sweep, not part of this defect, and narrowing it
needs its own noise/benefit measurement.
## References

- Encountered in EDU-Planner `backend-kotlin`, branch `optaplanner-removal`, commit `5279e8eb` ("refactor(p5d): rename Stage1DateUtils -> SchedulingDateUtils") — that commit message records the same finding from the caller's side.
- The project's own P2 surface survey had pre-flagged the hazard class: *"8 dependencies are FQN-only (no import line) — `references()` pass per symbol before final deletes."* The trap was known to the humans and invisible to the tool.
- Adjacent open issues on the same tool: `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`, `docs/issues/2026-07-27-edit-code-replace-drops-doc-comment-after-range-repair.md`, `docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md`.
