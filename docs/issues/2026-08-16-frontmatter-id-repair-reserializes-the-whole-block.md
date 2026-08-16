---
id: '60b5323f5e11be66'
kind: bug
status: open
title: 'BUG: repairing a frontmatter id re-serializes the whole block, reformatting hand-authored YAML and expanding template placeholders'
tags:
- librarian
- frontmatter
- move
- doctor
- blast-radius
closed: null
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md
severity: medium
---

# BUG: repairing a frontmatter id re-serializes the whole block, reformatting hand-authored YAML and expanding template placeholders

## Summary

`mv::repair_frontmatter_id` rewrites one field — the `id:` — but does it by
`frontmatter::parse` → mutate → `frontmatter::write`, which re-emits the **entire**
frontmatter block in canonical form. Every other key is requoted, reordered, converted from
flow to block style, or dropped if null. On librarian-written files that is a no-op; on
hand-authored YAML it is a reformat, and on at least one shape it is destructive to intent:
a `{Placeholder}` in a template is valid YAML for a *flow mapping*, so it round-trips into a
nested key and stops reading as a placeholder.

This is not the id repair being wrong. It is the repair being **value-preserving without
being form-preserving**, which are different properties, and only the first was checked.

## Symptom (Effect)

`docs/adr/templates/adr-template.md` in a sibling repo, before:

```yaml
created: {YYYY-MM-DD}
updated: {YYYY-MM-DD}
```

after a single `id:` repair:

```yaml
created:
  YYYY-MM-DD: null
updated:
  YYYY-MM-DD: null
```

Semantically identical YAML. The template placeholder is gone.

Same class, less severe, across the same sweep:

```
-title: "Unified Error Dialog System"        ->  +title: Unified Error Dialog System
-tags: [solver, optaplanner, iel, prod]      ->  +tags:
                                                 +- solver
                                                 +- optaplanner …
-  - calendar                                ->  +- calendar          (list indent)
-topic: null / -time_scope: null / -owners: []   (dropped, no replacement)
```

## Reproduction

2026-08-16, repairing the BL-23 backlog across five repos. Churn per file separates
librarian-written corpora from hand-authored ones cleanly:

| repo | files | diff | lines/file |
|---|---:|---|---:|
| codescout | 91 | 122+/199− | 3.5 |
| MRV-poc | 10 | 11+/21− | 3.2 |
| backend-kotlin | 53 | 142+/106− | 4.7 |
| advisory-proposal-generator | 23 | 23+/89− | 4.9 |
| **eduplanner-ui** | **26** | **402+/370−** | **30** |

`eduplanner-ui`'s ADRs were written by hand, so nearly every line of every frontmatter block
moved. That repo's changes were reverted; the other four were kept and committed.

## Environment

`experiments` at `05cf7ed5`/`3db48ebf`. Reproduces through both callers — `artifact(move)`
and `librarian(doctor, fix="repair_frontmatter_id")`.

## Root cause

`src/librarian/frontmatter.rs:180-185`:

```rust
pub fn update_in_place(doc: &str, edit: impl FnOnce(&mut Frontmatter)) -> Result<String> {
    let (fm_opt, body) = parse(doc)?;
    let mut fm = fm_opt.unwrap_or_default();
    edit(&mut fm);
    Ok(write(&fm, body))
}
```

`write` re-emits every field from the parsed struct. The body is preserved byte-for-byte;
the frontmatter is regenerated. That is correct for a librarian-authored file — canonical in,
canonical out — and is exactly why the codescout corpus showed almost no churn. It is wrong
as a *repair* primitive, where the contract should be "change this one field, touch nothing
else".

**The blast radius is wider than the sweep.** `repair_frontmatter_id` runs on **every**
`artifact(move)` whose file carries an id, because `needs_repair` is
`fm.id.is_some_and(|id| id != new_id)` and a move always mints a different id. So archiving
any hand-authored artifact reformats its frontmatter as a side effect — silently, and with no
mention in the move's response.

*Verified 2026-08-16 by reading `frontmatter.rs:105-185` and `mv::repair_frontmatter_id`, and
by classifying every added and removed line of the five-repo diff — 100% accounted for, all
frontmatter keys, no body line touched.*

## Evidence

### The round-trip is the mechanism, not the sweep

`advisory-proposal-generator` — 23 files, 23 insertions, 89 deletions. Exactly one insertion
per file (the id) and the rest pure removals of null/empty keys. Those files were close to
canonical already. `eduplanner-ui` — same tool, same call, 30 lines per file. The tool did
not behave differently; the corpora differ.

### No data was lost, which is what made it easy to miss

Every imbalance in the five-repo diff resolves to a null or empty value:

- `title` 27 removed / 26 added → the one net removal was an empty `title:`; every real
  title survived, requoted.
- `tags` 13 removed / 11 added → two `tags: []`; every tag value reappears as a block item.

A reviewer checking only for loss would sign this off. The defect is in the diff's *size and
shape*, not its content.

## Hypotheses tried

1. **Hypothesis:** the churn is the sweep's fault and per-file review would catch it.
   **Test:** compare churn per file across five corpora run through the identical call.
   **Verdict:** rejected — 3.2 to 30 lines/file with the same code. The variable is how
   canonical the input already was.

2. **Hypothesis:** it only affects the doctor sweep, so scoping is enough.
   **Test:** read `needs_repair` in `mv::repair_frontmatter_id`.
   **Verdict:** rejected — a move always changes the id, so every move of an id-bearing file
   re-serializes. `3db48ebf`'s root scoping bounds *which repo*, not *what gets rewritten*.

## Fix

Not implemented. Replace the round-trip with a surgical single-line edit: locate the `^id:`
line inside the frontmatter delimiters and swap only that line, leaving every other byte
untouched. The value to write is already known, the region is already delimited by the same
`---` scan `parse` performs, and it makes the repair's diff exactly one line by construction —
which is also what makes it reviewable in a foreign repo.

Keep `update_in_place` for callers that *want* normalization (`artifact(update)` writing
frontmatter fields); it is the right primitive there.

Two smaller alternatives, both inferior:
- Report the extra churn in the response (`reformatted: true`) — honest, still reformats.
- Refuse when the round-trip would change more than the id line — a guard where a fix belongs.

## Tests added

None — not fixed. The regression test should assert **byte equality outside the `id:` line**
for a deliberately non-canonical fixture: flow-style `tags: [a, b]`, a double-quoted title, a
`{Placeholder}` value, a null key. Asserting only that the id changed is what let this
through; the fixture must be hostile to normalization or it proves nothing.

## Workarounds

Before pointing `fix=repair_frontmatter_id` at a repo of hand-authored docs, run the dry-run,
apply, and read `git diff --shortstat`. Under ~5 lines/file is a canonical corpus; anything
approaching 30 means the frontmatter is being rewritten and the repair is not worth it.
`git checkout -- .` is exact if the tree was clean.

## Resume

Implement the surgical `id:` line swap in `src/librarian/tools/mv.rs`
(`repair_frontmatter_id`), then re-run the sweep against
`/home/marius/work/mirela/eduplanner-ui` — the repo reverted on 2026-08-16 — and confirm the
diff is one line per file. That repo is the regression corpus: it is the one that exposed
this, and its two templates (`docs/adr/templates/adr-template.md`, `fdr-template.md`) carry
the `{Placeholder}` shape that must survive.

## References

- `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md` — BL-23, the repair this is a defect in
- `src/librarian/frontmatter.rs:180-185` — `update_in_place`
- `src/librarian/tools/mv.rs` — `repair_frontmatter_id`, the shared helper both callers use

