---
id: fc491a58e7a9b561
kind: bug
status: open
title: 'BUG: append_entry recipes outside TAXONOMY are not checked by the recipe gate'
tags:
- cluster/selector-narrower-than-its-population
- append-entry
- doc-to-code
---

# BUG: append_entry recipes outside TAXONOMY are not checked by the recipe gate

**Valid:** dated 2026-09-27

## Summary

The TAXONOMY recipe gate (`every_taxonomy_append_entry_recipe_is_one_the_code_accepts`, in `src/librarian/tools/append_entry.rs`) reads only the *Main taxonomy* table of `docs/TAXONOMY.md`. Three other surfaces also route writers to `append_entry` and are unchecked: `CLAUDE.md`, the committed augmentation-sidecar prompts under `docs/augmentations/`, and each ledger's own template section. The parent of residual `5820a75840dd2d52` found a sidecar prompt (test-escape-hardening's) instructing a call that its own frontmatter made impossible at the time.

## Fix

Not started. Reuse the gate's scanner and checks with a second input per surface. The sidecar prompts are YAML strings, not table rows, so the row scanner does not apply there as-is; the call walker and the three checks do.

## References

- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` — § *Out of scope* and decision 3
- `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` — names the four surfaces
