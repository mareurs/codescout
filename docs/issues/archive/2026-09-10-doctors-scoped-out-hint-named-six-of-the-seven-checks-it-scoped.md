---
status: fixed
opened: 2026-09-10
closed: 2026-09-10
severity: medium
owner: marius
related: []
tags: [cluster/doc-contradicted-by-code]
kind: bug
---

# BUG: doctor's scoped-out hint named six of the seven checks it scoped out

## Summary
`doctor`'s `catalog_health.hint` explains that some row-grain findings were dropped
from the report and enumerates which checks that covers. The list was hand-written
prose and had drifted from `SCOPED_ROW_CHECKS`: it named six of seven, omitting
`frontmatter_status_mismatch`. Findings for that check were scoped out of every report
while the sentence explaining the drop never named it, so a reader handed a count had
six names to search for and no way to learn a seventh existed.

## Symptom (Effect)
With one foreign `frontmatter_id_mismatch` row seeded outside the active project, the
emitted hint reads:

```
1 row-grain finding(s) (frontmatter_id_mismatch / frontmatter_id_is_not_a_catalog_id /
ledger_defines_nothing / entry_without_definition / entry_defined_twice /
terminal_status_with_caveat) across 1 other project root(s) were scoped OUT of this
report — see catalog_health.row_checks_scoped_by_project.
```

Six names. `SCOPED_ROW_CHECKS` held seven.

## Reproduction
At `903e2332` on `experiments`, before the fix:

```
cargo test --workspace doctor::tests -- the_scoped_row_check_hint_names_every_check_it_scoped
```

Fails with `frontmatter_status_mismatch is scoped OUT of the report but is not named in
the sentence that explains the drop`.

## Environment
Linux, Rust 2021, branch `experiments`, `--features librarian` (default lane only — the
lean lane never compiles this code).

## Root cause
Two parallel enumerations of one set with nothing gating them against each other. The
authoritative list was `const SCOPED_ROW_CHECKS` (`src/librarian/tools/doctor.rs`,
seven entries); the reader-facing copy was a string literal in the `hint_parts.push`
at the `row_checks_scoped_by_project` site, six entries. `frontmatter_status_mismatch`
was added to the const when `scan_frontmatter_status_mismatches` landed (2026-09-07)
and the prose was not updated.

Measured 2026-09-10 by the test above, run before the fix.

This is the **second** instance of the same mechanism in the same file, and the first
is recorded in the code: `Violation::check`'s doc comment says a hand-maintained
enumeration there *"named eight checks and had gone stale by three ... before anyone
noticed, because nothing gates a doc comment against the `scan_*` functions that emit
these strings."* That instance was answered by deleting the enumeration. This one was
not, because the hint has to enumerate — a reader needs the names.

## Evidence
The const and the prose, side by side before the fix — seven versus six, with
`frontmatter_status_mismatch` present only in the first.

## Hypotheses tried
1. **Hypothesis:** the hint is generated and the drift is in the const.
   **Test:** read the `hint_parts.push` call at the `row_checks_scoped_by_project` site.
   **Verdict:** rejected — the names are a literal in the format string.

## Fix
Generate the sentence's check-name fragment from `SCOPED_ROW_CHECKS` via
`scoped_row_check_names()`, and type the const as `&[Check]` rather than `&[&str]` so a
renamed variant is a compile error instead of a string that silently stops matching.
The enumeration now has one source and one generator, so the drift cannot recur —
Observer Blindness position 3, a correct path that ends in a safe state rather than a
rule someone has to remember.

- **SHA** — `b057cc6d` on `experiments`
- **patch-id** — `06a04d1b73bb49423a25ff67be30f17435b190c5`

## Tests added
`the_scoped_row_check_hint_names_every_check_it_scoped`, `src/librarian/tools/doctor.rs`
`mod tests`. It asserts on `catalog_health.hint` from a real report.

**The first draft of this test asserted against `scoped_row_check_names()` and passed
instantly, against live drift** — a generator compared with itself is a tautology. It
was only caught because the RED was watched and did not arrive. The test also carries a
fixture guard (`hint.contains("were scoped OUT of this report")`) so it cannot pass by
asserting over an unarmed hint.

## Workarounds
Read `catalog_health.row_checks_scoped_by_project` for the per-project counts, and
`SCOPED_ROW_CHECKS` in source for the authoritative check list.

## Resume
N/A — fixed and verified, gate green both lanes.

## References
- `docs/issues/2026-09-09-doctor-accepts-a-scope-argument-and-never-reads-it.md` — the
  open bug this work sits under (`d4b61746950b86b7`); still open, still partial.
- `docs/superpowers/plans/2026-09-09-doctor-per-project-isolation.md` § Task 4 — carries
  the falsification banner from the same commit pair.
