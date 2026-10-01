---
id: 8b07225a28b29f6f
kind: bug
status: fixed
title: 'BUG: the session-log template''s own append_entry recipe is refused on a fresh copy'
tags:
- cluster/doc-contradicted-by-code
- session-log
- template
---

# BUG: the session-log template's own append_entry recipe is refused on a fresh copy

**Valid:** dated 2026-09-27

## Summary

`docs/templates/session-log.md` ships without frontmatter and prescribes `doc(action="append_entry", …, id_prefix="F")`. On a fresh copy that call is refused (`allocate_entry_id: … does not declare an entry_prefix`) until the writer declares `entry_prefix: [F, W]` — a step the template does ask for, in its own prose, as a precondition. Five session logs in this repo never took that step and sit on the recipe gate's shrink-only exemption list (`TEMPLATE_EXEMPT` in `src/librarian/tools/append_entry.rs`). While a newly copied log is undeclared, the gate reds every session's run in this shared tree, correctly: the recipe really would be refused.

## Fix

Shipped 2026-10-01: `docs/templates/session-log.md` now carries `entry_prefix: [F, W]` in its frontmatter, so a fresh copy's own recipe is accepted from the moment it exists. This was the operator's choice, made over keeping the template directly editable and scripting the copy-and-declare step. Fifteen session logs already declare F and W, so the change removes a manual step rather than adding a claim.

**Reproduced first, which also observes what an earlier note here only derived.** A real copy of the template placed in `docs/trackers/` made the ledger-recipe scan refuse both of its calls (copy lines 12 and 272, "does not declare an entry_prefix"), and the augmentation bootstrap test refused `F`.

**What it costs, found by trying it rather than by reading.** A declared file is a ledger to `edit_file`'s guard even with no stamped `id:`: editing the template's own prose through `edit_file` was refused by name and had to go through `doc(update, body_edits)`. So a copy is guarded from its first moment, which is the property the bug named as the price ("directly editable until declared") now ending at birth. An earlier reading of `is_librarian_artifact`, which requires a stamped `id:`, suggested otherwise and was wrong about the tool. The template's paragraph that said a fresh copy is directly editable is rewritten to say this.

**Tests.** `fresh_session_log_template_bootstrap_allocates_f1_and_w1` no longer prepends its own declaration, which would have passed whatever the template said. New: `a_fresh_copy_of_the_session_log_template_passes_the_recipe_gates` (the ledger-recipe scan and the F/W-both template check, on an untouched copy). Dropping `W`, dropping `F`, and an undeclared prefix in the template's own call were each mutated once and each killed by the test naming that guard.

**Not done, and why it is not here.** `codescout-companion/skills/reconnaissance/SKILL.md:103` in the `claude-plugins` repo still says a fresh template "ships without `entry_prefix`, so it is directly editable until the ledger is guarded". That is now false, it is another repository, and it was not edited. Existing session logs that carry the old paragraph are copies and are history.


## Fix provenance

- **SHA:** `cedbd0fe0099bb3228bf4d46e27f2d06cebfddc3` (`experiments`)
- **patch-id:** `7423697fb7369bd8ea78f360458694efa10a3ea2`


## References

- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` — § *Out of scope*
- `bug-fix-session-log:F-176`
