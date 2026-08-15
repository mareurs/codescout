---
kind: bug
status: fixed
tags:
- edit_code
- mcp-tool
- silent-data-loss
- attributes
- derive
closed: 2026-08-15
opened: 2026-08-13
owner: marius
related:
- ec39936568425950
- '0aa3860453ea8053'
- dce1189f5b382c92
severity: medium
---

# BUG: edit_code's `attributes` parameter replaces the whole attribute list, so an incomplete list silently deletes the attributes you did not name

## Summary

Passing `attributes=[...]` to `edit_code` appears to substitute the supplied
list for the symbol's *entire* existing attribute list rather than merging into
it. An agent that names only the attribute it wants to add silently deletes
every attribute it did not repeat — observed dropping `#[derive(Debug, Clone)]`
from a struct. For `derive` the compiler usually catches the loss at a use site;
for `#[cfg(...)]`, `#[serde(...)]`, or `#[allow(...)]` there may be **no compile
error at all** and the behavior change is silent.

This is the third member of a family already in the ledger: the same
"omitted element is deleted rather than preserved" shape as
`edit_code(action="replace")` dropping a symbol's **visibility modifier**
(`ec39936568425950`, mitigated) and dropping a symbol's **doc comment**
(`0aa3860453ea8053`, fixed). The pattern, not this instance, is the finding.

## Symptom (Effect)

Reported by the Task 2 implementer subagent during SDD execution of
`docs/superpowers/plans/2026-08-13-worktree-semantic-search.md`, in its own
words in `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-2-report.md`:

> an `edit_code(attributes=[...])` call briefly dropped `#[derive(Debug, Clone)]`
> from `LocalChunk` when I supplied an incomplete attributes list — caught
> immediately by re-reading the symbol before running any tests, fixed, and
> documented in the report as a tooling lesson (no test currently pins
> `LocalChunk: Clone`, so this class of mistake wouldn't have failed the suite)

No error, no warning, and no `status` field indicating a removal. The agent
discovered it only because it re-read the symbol after the edit.

## Reproduction

**Not independently reproduced by the controller.** Best lead — the exact
sequence to run:

1. `git rev-parse HEAD` → `1f100bc6` (branch `experiments`) or later.
2. Pick a struct carrying two or more attributes, e.g. `LocalChunk` in
   `src/retrieval/drift.rs`, which carries `#[derive(Debug, Clone)]`.
3. Call `edit_code` on it with `attributes` naming only ONE attribute that is
   not already present (or a strict subset of the existing list).
4. Re-read the symbol with `symbols(name="LocalChunk", include_body=true)` and
   compare the attribute list against step 2.

Expected: the named attribute is added or updated, the others are preserved.
Reported: the list is replaced by what was supplied.

## Environment

- codescout v0.15.0, branch `experiments`, tip at time of observation `1f100bc6`
- Linux 7.1.5-zen1-2-zen, MCP over stdio, Claude Code
- Language: Rust. Whether the behavior is language-agnostic or specific to the
  Rust AST extractor is **unknown** — untested on Kotlin/TS/Python.

## Root cause

**Unknown — inferred from the reported symptom, NOT measured.** The hypothesis
is that the `attributes` parameter is implemented as a set-the-list operation
(the supplied vector is written in place of the node's attribute span) rather
than a merge, matching the mechanism of its two ledger siblings, where an
element absent from the caller's input is treated as "delete" instead of
"leave alone".

No `path:line` citation is offered deliberately: the controller has not opened
`edit_code`'s attribute-handling code, and a mechanism read out of the code but
never observed at runtime is a hypothesis wearing a conclusion's clothes. The
first step of any work on this bug is the Reproduction section above, not a
code read.

## Evidence

### Task 2 implementer report

`.superpowers/sdd/2026-08-13-worktree-semantic-search/task-2-report.md` —
the "tooling lesson" section, quoted in Symptom above. Self-reported by the
agent that made the mistake and caught it; the controller did not observe the
intermediate broken state.

### The coverage gap that would have hidden it

Nothing in the suite pins `LocalChunk: Clone`. At the time of the edit no call
site cloned it, so the derive could vanish and both test lanes would stay green
(3666 / 3675 passing). Task 6 is the first consumer, so the loss would have
surfaced there as a confusing compile error one task downstream of its cause.

### SECOND OCCURRENCE — same session, different symbol, different lost element

**2026-08-13, SDD Task 6 fix round 1** (`.superpowers/sdd/2026-08-13-worktree-semantic-search/task-6-report.md`, "## Fix round 1"). While extracting a shared walk helper, an `edit_code(attributes=[...])` call **dropped a doc comment from `stream_index`** in `src/retrieval/sync.rs`. Caught by re-reading the symbol immediately after the edit, and fixed before proceeding.

Three things this occurrence establishes that the first one did not:

1. **It is not confined to `#[derive(...)]`.** The first occurrence lost a derive; this one lost a **doc comment**. That widens the blast radius to every attribute-adjacent element the tool rewrites, and it rhymes with the already-fixed sibling `0aa3860453ea8053` (`edit_code` structural replace dropping a doc comment after an AST range repair) — the same element is now reachable through a second parameter.
2. **Care alone does not prevent it; it only catches it.** The agent that hit this had been warned about this exact footgun *by name, with this file path, in its dispatch prompt*, and instructed to pass the complete attribute list and re-read the symbol afterwards. It still triggered the drop. The re-read caught it. So the documented workaround is **effective as detection and useless as prevention** — which is the strongest available argument for fixing the tool rather than documenting around it.
3. **Two independent hits in a single day, by two different agents, on ordinary edits.** This is not an exotic path. Both were on production Rust code in the same repo during routine structural edits.

**Severity should be raised from `medium` to `high`.** The original rating assumed a single observation and leaned on the compiler as a backstop. Two occurrences in one session, plus the fact that the lost element can be a doc comment (no compile error) or `#[cfg(...)]` / `#[serde(...)]` / `#[allow(...)]` (no compile error, and a real behaviour change), means the compiler backstop covers only the `derive` case. The frontmatter still reads `medium`; treat this section as the argument for changing it.
## Hypotheses tried

1. **Hypothesis:** this is a rediscovery of `ec39936568425950` (edit_code replace
   drops the visibility modifier).
   **Test:** compared the mechanisms — that bug is `action="replace"` losing a
   modifier absent from the replacement *body*; this is the `attributes`
   *parameter* replacing a list.
   **Verdict:** rejected as a duplicate, recorded as a sibling. Different
   parameter, different code path, same failure class.
   **Evidence link:** the `related:` frontmatter list.

## Fix

**Direction 2, shipped in `edbdf4d9`** — report what was removed, keep replace
semantics.

Direction 1 (merge instead of replace) was rejected on the reasoning this file
already sketched: it changes behaviour for every existing caller in order to fix
what is really a *discoverability* problem. The semantics are correct and the tool
description states them; what was missing is the write telling you what it cost.

The precedent is now three-for-three in this codebase, and they all exist for one
reason: **a destructive write that GROWS the file passes every size guard by
construction**, so the only defence is naming the casualties.

| Surface | Field |
|---|---|
| librarian body writes | `replaced_subsections` |
| `edit_code(remove)` (`3baa993d`) | `removed_descendants` |
| `edit_code(replace, attributes=[…])` | **`removed_attributes`** |

Two design points that are the actual content of the fix:

1. **Casualties, not the region.** An attribute the caller re-states in the
   supplied list survives, so it is not reported. Listing the whole lead region
   would be easier and wrong.
2. **Absent when nothing was lost.** A `removed_attributes: []` on every replace
   is noise, and noise is how a warning field stops being read — the same end
   state as reporting nothing at all.

Doc comments are deliberately excluded from the field. It answers *"which
attributes did I lose"*; folding documentation and attributes into one list is
precisely how the lead region caused trouble before (`do_replace` carries its own
comment on treating the two classes independently).

Severity lowered `high` → `medium`: the loss is no longer silent, which was the
whole of the severity. The write is still destructive by design.
## Tests added

Two in `tests/symbol_lsp.rs`, **mock-backed on purpose**. The existing attribute
tests (`u19_*`, `u21_*` in `tests/bug_regression.rs`) sit behind `#[ignore]`
because they need a real rust-analyzer, so a regression there would not fail the
normal gate — this is the coverage gap § Evidence already named.

- `replace_with_attributes_names_the_attributes_it_dropped` — the supplied list
  **re-states** `#[deprecated]` and adds `#[inline]`, so only
  `#[allow(dead_code)]` is genuinely lost. Asserting the exact one-element vector
  is what separates *reporting casualties* from *reporting the lead region*; a
  `contains` check would pass for both.
- `replace_reports_no_removed_attributes_when_nothing_was_lost` — both no-loss
  arms in one loop: no `attributes` param at all, and a list re-stating exactly
  what was already there.

Gate: `cargo test --workspace` → 3812 passed / 0 failed / 50 ignored; clippy
`--workspace --all-targets -D warnings` clean.
## Workarounds

- **Always pass the complete intended attribute list**, not just the delta.
- **Re-read the symbol after any `edit_code` call that passes `attributes`**
  (`symbols(name=X, include_body=true)`) and compare the attribute list. This is
  what caught it here.
- Prefer `edit_code(action="replace")` with the full item text, or `edit_file`
  for a purely textual attribute change, when the attribute list is non-trivial.

## Resume

Closed. The related open question about `replace` and *descendants* (should a
replacement body that defines fewer children report rather than refuse?) is
tracked separately and is the same argument one level up — report what changed
rather than guess at intent.

Worth keeping from this one: the fix is not the field, it is **which entries the
field contains**. A destructive-write warning that lists everything in range, or
fires on every call, decays into noise and stops being read — which lands in the
same place as the silence it replaced.
## References

- Sibling bugs, same failure class:
  - `docs/issues/archive/2026-07-09-edit-code-replace-drops-visibility-modifier.md` (`ec39936568425950`, mitigated)
  - `docs/issues/2026-07-27-edit-code-replace-drops-doc-comment-after-range-repair.md` (`0aa3860453ea8053`, fixed)
  - `docs/issues/archive/2026-08-08-edit-code-rename-under-reaches-and-reports-ok.md` (`dce1189f5b382c92`, fixed) — `edit_code` under-reaching and reporting `ok`
- Origin: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-2-report.md`
- Plan under execution when observed: `docs/superpowers/plans/2026-08-13-worktree-semantic-search.md`
- The `replaced_subsections` precedent for reporting destructive writes:
  `get_guide("librarian")` § Body Editing Surfaces
