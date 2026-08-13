---
status: open
opened: 2026-08-13
closed:
severity: medium
owner: marius
related: [ec39936568425950, 0aa3860453ea8053, dce1189f5b382c92]
tags: [edit_code, mcp-tool, silent-data-loss, attributes, derive]
kind: bug
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

Not planned yet. Two candidate directions, neither validated:

1. **Merge instead of replace** — treat `attributes` as an upsert keyed by
   attribute name, so unnamed attributes survive. Changes existing behavior for
   any caller relying on replace semantics.
2. **Report what was removed** — keep replace semantics but return a
   `removed_attributes` field, the way the librarian's body writes return
   `replaced_subsections`. That precedent exists precisely because a
   silent-destructive write that grows the file passes every size guard by
   construction; the same argument applies here.

Direction 2 is the cheaper and more consistent-with-precedent option and does
not break callers.

## Tests added

None yet — the bug is not fixed. A regression test belongs with the fix and
must assert the *preserved* attribute, not just the added one: a test that only
checks the new attribute is present passes whether or not the others survived.

## Workarounds

- **Always pass the complete intended attribute list**, not just the delta.
- **Re-read the symbol after any `edit_code` call that passes `attributes`**
  (`symbols(name=X, include_body=true)`) and compare the attribute list. This is
  what caught it here.
- Prefer `edit_code(action="replace")` with the full item text, or `edit_file`
  for a purely textual attribute change, when the attribute list is non-trivial.

## Resume

Run the Reproduction section's four steps on a scratch copy of a struct with two
attributes and record the actual before/after attribute lists — that converts
this file's root cause from hypothesis to measured mechanism. Then check whether
the behavior is Rust-specific by repeating on a Kotlin or TypeScript symbol.

Only after that, read `edit_code`'s attribute-handling path and decide between
the two Fix directions.

Do not archive this file on the strength of the Task 2 fix — Task 2 restored the
derive by hand; the tool behavior is untouched.

## References

- Sibling bugs, same failure class:
  - `docs/issues/archive/2026-07-09-edit-code-replace-drops-visibility-modifier.md` (`ec39936568425950`, mitigated)
  - `docs/issues/2026-07-27-edit-code-replace-drops-doc-comment-after-range-repair.md` (`0aa3860453ea8053`, fixed)
  - `docs/issues/archive/2026-08-08-edit-code-rename-under-reaches-and-reports-ok.md` (`dce1189f5b382c92`, fixed) — `edit_code` under-reaching and reporting `ok`
- Origin: `.superpowers/sdd/2026-08-13-worktree-semantic-search/task-2-report.md`
- Plan under execution when observed: `docs/superpowers/plans/2026-08-13-worktree-semantic-search.md`
- The `replaced_subsections` precedent for reporting destructive writes:
  `get_guide("librarian")` § Body Editing Surfaces
