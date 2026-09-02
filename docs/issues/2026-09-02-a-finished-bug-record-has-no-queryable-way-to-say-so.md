---
status: open
opened: 2026-09-02
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-09-02-index-description-omits-the-verify-action.md
  - docs/issues/2026-09-02-memory-description-omits-the-refresh-anchors-action.md
tags:
  - cluster/selector-narrower-than-its-population
kind: bug
---

# BUG: a finished bug record has no queryable way to say so, and the terminal-status selector returns a correct zero

## Summary

Two bug files carried a fix SHA, a patch-id, a named regression test and a green gate in
their `## Fix` sections while their frontmatter said `status: open`. Every triage query
returned them as unstarted work, and a peer's *terminal-but-unarchived* scan returned **0**
— correctly, because its predicate is `status is terminal AND path is not archive/` and it
never examines a record whose status *understates* its own body.

`unverified:` exists so a `fixed` record can say *"less finished than my status suggests"*.
Nothing lets an `open` record say *"more finished than my status suggests"*, and the
asymmetry has a reason rather than being an oversight — see § *Root cause*.

## Symptom (Effect)

At `8fb5f638~1`, both files:

```
status: open
closed:
```

and, in the same files' bodies:

```
**Fixed on `experiments` at `655c0b6f`** (`655c0b6f6794be223f85fbad8360cd3002cc13d3`),
patch-id `2ae27c8a135edae59191b0b840b90956bb97ca6d`.
```

A peer's triage scan at that commit reported `0 terminal-but-unarchived` against a canonical
48. The true figure was 46 genuinely non-terminal, with these two the difference. The peer
had already carried the wrong number to its operator.

## Reproduction

```
grep -l '^status: open' docs/issues/*.md \
  | xargs grep -l 'patch-id `[0-9a-f]\{40\}`'
```

Any hit is a record whose body establishes a fix its status denies. At `8fb5f638~1` this
returned two files; after `8fb5f638` it returns none, because the flip was applied by hand
once a reader noticed.

## Environment

Not environment-dependent. Requires a git checkout of this repo.

## Root cause

**The two directions of the same defect cost different amounts to check, and only the cheap
one was built.**

The class is: *the queryable field and the prose disagree, and only the prose is true.*
`get_guide("tracker-conventions")` documents one direction and its remedy — an author writes
a caveat in prose where no query reads it, so a `fixed` record overstates, and `unverified:`
makes the caveat queryable. Measured 2026-08-19: 14 of 16 terminal-but-unarchived files
stated their blocker in prose.

The other direction has no field and no check. The reason is mechanical:

| direction | detectable from | cost |
|---|---|---|
| `fixed` overstates | `status` + presence of `unverified:` — **fields alone** | cheap |
| `open` understates | `status` + a fix SHA **inside the body** | must read bodies |

A predicate over frontmatter can express the first and cannot express the second, because
"actually finished" lives only in prose. So `src/librarian/tools/doctor.rs` carries
`terminal_status_with_caveat` and `terminal_status_without_fix_anchor` — both keyed on
terminal status — and nothing keyed the other way. That is not an omission someone forgot;
it is the boundary of what the cheap instrument can see.

**Why the author cannot catch it.** The party who writes the fix record knows the bug is
fixed. Re-reading their own file, the `open` field reads as a formality they have already
mentally discharged — *knowing it is done is exactly what removes the prompt to check*. The
party misled is a later triage session, which never talks to them. This is `OB-1`'s observer
structure (a claim omitting the parameter its author's context supplies) and specifically its
*third position* sub-pattern: the information is published correctly, to an audience that is
not the one reading the status.

Measured 2026-09-02 at `8fb5f638~1`: two files, both authored in this session, both with the
fix record written by the same party that left the field stale.

## Evidence

### The selector's own blind spot

The scan predicate is `status ∈ {fixed, mitigated, wontfix} AND path ∉ archive/`. It returns
a well-formed **0** over a population that excludes, by construction, every record whose
status understates. Per `IC-18`'s discriminator — *could the mechanism, unchanged, report how
many it missed?* — the answer is no: it never enumerated the non-terminal records, so there
is no count to emit and no surface on which to mark under-coverage.

### It propagated

The zero was not merely wrong on a screen. It was carried into a peer's backlog report to its
operator before a reader happened to open the bodies.

### The sibling checks already exist

`src/librarian/tools/doctor.rs` — 17 references to `terminal_status_without_fix_anchor`. The
proposed check has a home beside its own family, not a new module.

## Hypotheses tried

1. **Hypothesis:** this is an `OB-N` class in its own right.
   **Test:** the `observer-blindness.md` template's admission test — *would a more careful
   version of the same party have caught it?*
   **Verdict:** rejected. A more careful author would have flipped the field, and did, the
   moment it was pointed out. That is observed failure, not structural inability. The observer
   structure that *is* load-bearing is already `OB-1` and its third-position sub-pattern; cite
   across rather than open `OB-13`.

2. **Hypothesis:** this belongs to `IC-13` (a capped result presented as complete).
   **Test:** `IC-18`'s stated discriminator — could the mechanism report how many it missed?
   **Verdict:** rejected. Nothing was capped and nothing was dropped by a limit; the excluded
   records were never examined, so there is no truncation marker to emit.

3. **Hypothesis:** the remedy is a new frontmatter field, the mirror of `unverified:`.
   **Verdict:** deferred, and probably wrong. A field would have to be set by the same author
   who already failed to set `status`, on the same occasion — it inherits the defect it is
   meant to catch. A check that reads what the author *did* write is not subject to that.

## Fix

Plan, not implemented. Add a sibling check to `src/librarian/tools/doctor.rs`:

```
non_terminal_status_with_fix_anchor
  frontmatter status ∈ {open, investigating, zombie}
  AND body carries a fix SHA + patch-id
```

Mechanism proposed by `codescout-20`. It is machine-checkable, has a measured population of
at least 2, and fires when nobody is worried — which is the property `CLAUDE.md` §
*Observer Blindness* asks for, because the author is by construction not worried.

Two design notes for whoever takes it:

- **Key on the patch-id, not on the SHA alone.** A bare 40-hex string appears in prose for
  reasons other than a fix anchor; the pair is what `get_guide("tracker-conventions")`
  requires at archive time and is a far tighter signal.
- **Report, never flip.** The correct status may be `mitigated` rather than `fixed`, and a
  partial fix cancels terminality outright. This is a worklist check like
  `entry_dated_stale`, not a verdict.

## Tests added

None yet — the fix is unimplemented. Owed with it: a fixture pair (one record whose body
carries the anchor with `status: open`, one with `status: fixed`) so the check is verified in
**both** directions rather than only on the positive, since an existence assertion over
"records with an anchor" is monotone under widening.

## Workarounds

The `grep -l` pipeline in § *Reproduction*. It is not wired to anything and nobody runs it
unprompted, which is the whole defect.

## Resume

Implement `non_terminal_status_with_fix_anchor` in `src/librarian/tools/doctor.rs` beside
`terminal_status_without_fix_anchor`; add the two-direction fixture; re-run the triage figures
and tell whoever holds the current backlog number.

## References

- `docs/issues/2026-09-02-index-description-omits-the-verify-action.md`,
  `docs/issues/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` — the two
  measured instances, both flipped at `8fb5f638`.
- `docs/issues/archive/2026-08-26-zombie-bug-files-are-reachable-by-no-query.md` — same family
  (a status no query reaches), same venue, already archived.
- `docs/trackers/observer-blindness.md` `OB-1` § *Sub-pattern — the third position: supplied
  CORRECTLY, to the wrong audience* — the observer structure, cited rather than duplicated.
- `get_guide("tracker-conventions")` § *Bug files* — `unverified:`, and the 2026-08-19
  measurement of the direction that does have a remedy.
