---
status: open
opened: 2026-08-08
closed:
severity: high
owner: marius
related: []
tags: [edit-code, rename, lsp, kotlin, write-path, silent-failure]
kind: bug
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

Proposal, not implemented.

**Primary — make the write path honour the disagreement it already computes.** When the textual sweep yields `kind: "source"` occurrences outside the LSP edit set, `rename` must not return a bare `status: "ok"`. Either:
- **(a)** refuse, returning the un-covered files and requiring an explicit opt-in to proceed (consistent with how other destructive paths gate on an `@ack_*` handle); or
- **(b)** complete the rename over the union (LSP edits + `kind: "source"` textual matches) and report which came from which.

(a) is the safer default; (b) matches what a caller almost always wants and what a human doing this by hand would do. Either beats silence.

**Secondary — fix `verify_hint`'s direction.** It should name under-reach first when un-covered `kind: "source"` matches exist: *"N source files matched textually but were not renamed — verify or re-run."* Its current text is actively misleading in exactly the case that hurts.

**Separate concern — Kotlin cross-file reference resolution.** Worth its own investigation (`/usr/bin/kotlin-lsp` returning zero cross-file refs for a top-level `object`), but the primary fix must not depend on it: any language server can under-report, and the write path should be robust to that regardless.

## Tests added

None — not yet fixed. A regression test should assert that a rename whose textual sweep finds un-covered `kind: "source"` matches does **not** return `status: "ok"`. That test does not need a real LSP failure: stub the reference resolver to return only the declaration and assert the response shape.

## Workarounds

After **every** `edit_code(action="rename")`, grep the old name to zero before trusting the result:

```
grep(pattern="<OldName>", glob="*.<ext>", mode="files")   → expect 0 matches
```

If it is non-zero, repair the remaining sites with `edit_file(replace_all=true)` per file. Do not rely on `files_changed`, on `status: "ok"`, or on the build being green in an IDE that resolves differently.

Renaming a class in a file-per-class language also needs `git mv` for the file itself — `rename` does not move it.

## Resume

Decide between fix (a) and (b) above, then locate where `rename` assembles its edit set and where the textual sweep result is attached to the response — the two are already adjacent, since `sweep_skipped` and `textual_matches` ride on the same payload. Port the disagreement check that `references` already performs (it produces the "LSP returned 0 references outside the definition file, but X appears as a whole word in N other source file(s)" warning) onto the rename path, upgraded from a warning to a gate. Separately, file the Kotlin cross-file resolution failure as its own issue if it is not already tracked.

## References

- Encountered in EDU-Planner `backend-kotlin`, branch `optaplanner-removal`, commit `5279e8eb` ("refactor(p5d): rename Stage1DateUtils -> SchedulingDateUtils") — that commit message records the same finding from the caller's side.
- The project's own P2 surface survey had pre-flagged the hazard class: *"8 dependencies are FQN-only (no import line) — `references()` pass per symbol before final deletes."* The trap was known to the humans and invisible to the tool.
- Adjacent open issues on the same tool: `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md`, `docs/issues/2026-07-27-edit-code-replace-drops-doc-comment-after-range-repair.md`, `docs/issues/2026-08-07-edit-code-remove-ast-repair-over-deletes.md`.
