---
id: '214c5e62d2509b51'
kind: bug
status: open
title: 'BUG: doctor enforces a `## Fix provenance` grammar that get_guide("tracker-conventions") never defines'
tags:
- cluster/doc-contradicted-by-code
- librarian
- doctor
- prompts
---

## Summary

`librarian(action="doctor")`'s `terminal_status_without_fix_anchor` refuses a `fixed` /
`mitigated` bug record that declares no `## Fix provenance` pointer, and its remedy text
names two concrete forms: the `## Fix provenance` section, and a `no_fix_commit:`
frontmatter key for records that owe nothing.

**Neither is defined in `get_guide("tracker-conventions")`** — the guide `CLAUDE.md` names
as *"the controlling convention (frontmatter, entry-id grammar, `**Valid:**` / `**Rests
on:**`, archiving)"*. Measured 2026-09-14 against `HEAD`:

| surface | `Fix provenance` | `no_fix_commit` |
|---|---|---|
| `src/prompts/guides/tracker-conventions.md` (the guide's source) | **0** | **0** |
| `src/librarian/tools/doctor.rs` (the enforcer) | 50 hits | present |
| `docs/issues/**` (the corpus) | ~150 files | 14 files |

So the convention is real, widely used, and machine-enforced — and a session that reads the
documented convention end to end never learns it exists.

## Symptom (Effect)

A record written to the guide's spec is flagged the moment `doctor` runs. The remedy text
names the missing structure but not its shape, so closing the finding means reverse-
engineering the grammar. Observed path, 2026-09-14, on this bug's sibling record
(`dd980371e235943e`):

1. `doctor` fires: *"no `## Fix provenance` pointer is declared"*.
2. `get_guide("tracker-conventions")` — 0 hits for the term. The guide is 53.9 KB and the
   grep was over the whole persisted payload, not a preview.
3. Grammar recovered from `doctor.rs`'s **test fixtures**, which are the only executable
   statement of it:

   ```
   ## Fix provenance

   - **SHA:** `abc1234`
   - **patch-id:** `deadbeefcafe`
   ```

   The cross-repo form (`- **SHA:** `claude-plugins:9169527``) is likewise discoverable only
   from `a_cross_repo_prefixed_pointer_is_skipped_rather_than_reported_dead`.

Three tool calls to learn a two-line format, and the route runs through Rust source that a
docs-only session has no reason to open.

## Reproduction

```
get_guide("tracker-conventions")           # read it end to end
# write a bug record to that spec, status: fixed
librarian(action="doctor")                 # terminal_status_without_fix_anchor fires
```

## Root cause

Inferred, not measured beyond the greps above. The check and the guide are maintained on
different surfaces — `src/librarian/tools/doctor.rs` and
`src/prompts/guides/tracker-conventions.md` — and nothing couples them. `doctor.rs:67`
even states the corpus split (*"54 of 350 archived files"* carry the triple), so the
convention's uneven adoption was known at the enforcer while the guide stayed silent.

This is `cluster/doc-contradicted-by-code` by omission rather than by conflict: the guide
does not say something false, it declines to say the thing the code requires, which a
reader cannot detect from inside the guide.

## Evidence

`terminal_status_without_fix_anchor` cleared on `dd980371e235943e` once the section was
added in the reverse-engineered form — 156 → 155 violations, the check's own count 1 → 0.
So the enforcement is correct and the gap is purely documentary.

Two sibling records already exist on this check's behaviour
(`docs/issues/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md`,
`docs/issues/2026-09-13-fix-anchor-check-reports-absent-when-it-means-unparseable.md`), so
the check is actively worked; neither is about the missing documentation.

## Fix

*Not written.* The obvious one is a `§ Fix provenance` block in
`src/prompts/guides/tracker-conventions.md` giving both forms and the cross-repo prefix.

**Worth checking before writing it:** the guide is a prompt surface with a documented
character budget, and `src/prompts/README.md` governs what may be added. A block that
pushes a slice over its cap trades this defect for a worse one. Size the addition against
that budget first; if it does not fit, the honest alternative is a pointer from the guide
to wherever the grammar is stated, which is currently nowhere.

**Explicitly not proposed:** teaching `doctor`'s remedy text to print the grammar inline.
That helps the reader who already tripped the check and does nothing for the reader
following the guide to write a correct record the first time — which is the population this
bug is about.

## Tests added

None. No fix attempted.

## References

- `src/librarian/tools/doctor.rs` — `scan_terminal_status_without_fix_anchor`, and the
  test fixtures that are the format's only executable specification
- `src/prompts/guides/tracker-conventions.md` — the guide that omits it
- `docs/issues/2026-09-14-the-dirty-check-reports-any-write-it-did-not-mediate-as-another-sessions.md`
  — the record whose finding surfaced this
