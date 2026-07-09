---
status: open
opened: 2026-07-09
closed:
severity: low
owner: marius
related: []
tags: [read_markdown, error-handling, usage-db]
kind: bug
---

# BUG: `read_markdown`'s heading-not-found message uses double-quoted `{:?}` formatting, inconsistent with `RecoverableError`'s single-quoted convention

## Summary
`read_markdown_single_heading` re-derives its own "heading not found" message via
`format!("heading {:?} not found", heading_query)` instead of propagating the
underlying `RecoverableError.message`, producing a double-quoted string
(`heading "X" not found`) that diverges from every other heading-not-found message
in the codebase (single-quoted, `heading 'X' not found`). Found while reviewing
Task 1 of the tool-friction-reduction plan, whose new `usage::db::normalize_err_family`
arm for this family was written against the single-quoted convention and silently
never matches read_markdown's actual output.

## Symptom (Effect)
`read_markdown(path, heading="nonexistent")` returns (success-shaped, not an error):
```json
{"ok": false, "error": "heading \"nonexistent\" not found", "headings": [...], "hint": "..."}
```
Every other heading-not-found producer in the codebase (the underlying
`extract_markdown_section`/`resolve_section_range` error, and `edit_markdown`, which
propagates that error unchanged) uses single quotes: `heading 'nonexistent' not found`.

## Reproduction
At `experiments` HEAD (post commit `c76937ee`): call `read_markdown(path="<any .md
file>", heading="does-not-exist")`. Compare against `file_summary::extract_markdown_section`'s
error text or `edit_markdown` with the same nonexistent heading.

## Environment
codescout, Rust, branch `experiments`, `src/tools/markdown/read_markdown.rs`.

## Root cause
`read_markdown_single_heading` (`src/tools/markdown/read_markdown.rs:170-277`) catches
`extract_markdown_section`'s `Err(e)` and, on a "not found" message, does NOT return
`Err(e)` (which would surface `e.message` verbatim via `RecoverableError`'s `Display`
impl). Instead it returns `Ok(json!({"error": format!("heading {:?} not found",
heading_query), ...}))` at line ~189 — `{:?}` is `Debug` formatting for `&str`, which
wraps the value in double quotes. The canonical message, `format!("heading '{}' not
found", heading_query)`, lives in `src/tools/file_summary/file_summary.rs:314` (single
quotes) and is what `edit_markdown` (`src/tools/markdown/edit_markdown.rs:90,316,411`)
propagates unchanged via `.map_err(|e| anyhow::anyhow!("{}", e))`.

## Evidence
### Confirmed via `symbols`/`grep` during Task 1 review (2026-07-09)
- `read_markdown.rs:189` (approx): `format!("heading {:?} not found", heading_query)`
- `file_summary.rs:314`: `format!("heading '{}' not found", heading_query)` inside
  `RecoverableError::with_hint(...)`
- `edit_markdown.rs:90,316,411`: `.map_err(|e| anyhow::anyhow!("{}", e))` — propagates
  `RecoverableError`'s `Display` (= `self.message` verbatim), i.e. single-quoted.

## Hypotheses tried
1. **Hypothesis:** Both tools produce the same message text for this error family.
   **Test:** Read both code paths directly (`symbols(include_body=true)`, `grep`).
   **Verdict:** rejected — `read_markdown` re-derives via `{:?}` (double quotes),
   `edit_markdown` propagates the original (single quotes). Confirmed by an
   independent task-reviewer subagent during Task 1's review, then re-verified
   directly by the controller.

## Fix
Not yet implemented. Two options, either acceptable:
1. Make `read_markdown_single_heading` reuse `e.message` (or reformat with single
   quotes) instead of re-deriving via `{:?}`, matching `edit_markdown`'s approach and
   removing the inconsistency at the source.
2. Leave both message shapes as-is but ensure any classifier reading these messages
   (e.g. `usage::db::normalize_err_family`) matches both quote styles — this was
   applied as an immediate, narrow fix in Task 1's follow-up commit (tool-friction-
   reduction plan) without touching `read_markdown.rs`, since that file was outside
   Task 1's declared scope. Option 1 (the actual root-cause fix) remains open.

## Tests added
N/A — no fix landed yet against `read_markdown.rs` itself. Task 1's follow-up commit
added test coverage for the classifier's tolerance of both quote styles in
`src/usage/db.rs`'s `normalize_err_family_maps_iron_law_routing_errors` test, which is
a mitigation, not a fix for this bug's root cause.

## Workarounds
None needed for callers — the double-quoted message is still human-readable and
still triggers the same downstream behavior (returning the `headings` list + hint).
Only automated classifiers keying off exact message text (like
`normalize_err_family`) need to account for both quote styles until this is fixed.

## Resume
Edit `src/tools/markdown/read_markdown.rs`'s heading-not-found branch (~line 189) to
build its message from `e.message` (or reformat with single quotes to match
`file_summary.rs:314`) instead of `format!("heading {:?} not found", heading_query)`.
Add a regression test asserting the exact message text (single-quoted) so a future
change can't silently reintroduce the mismatch.

## References
- `docs/superpowers/plans/2026-07-09-tool-friction-reduction.md` Task 1
- `docs/trackers/tool-friction-reduction-session-log.md`
- Task 1 reviewer report (this session), which found the mismatch
