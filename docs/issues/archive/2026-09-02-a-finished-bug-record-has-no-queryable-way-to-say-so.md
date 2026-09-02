---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-index-description-omits-the-verify-action.md
- docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md
severity: medium
unverified: the 36cb17ed section-scoping fix was verified against the live corpus by a transcription of its two functions, not by the rebuilt MCP binary — re-run librarian(action="doctor") after a rebuild to confirm the real check also reports 1 rather than 5
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


### The axis is narrower than "open understates" — terminality and archivability are different facts

Refinement from `codescout-20`, 2026-09-02, and it is a better statement of the structure than
the one above. Their `is_write` record is `fixed`-but-unarchived with the blocker in
frontmatter: *"archive move held — a committed spec cites this artifact id in a sentence the
fix falsified"*. `fixed` does **not** overstate there. The fix landed; what is owed is the
**move**.

So `status` encodes one axis — is the work done — while the archive flow depends on a second,
whether the file may move, and only the first has a field. Both directions of drift then leak:

| state | expressible? |
|---|---|
| `open`, not done | yes — the normal case |
| `fixed`, archived | yes — the normal case |
| `fixed`, unarchived, move merely owed | yes, but indistinguishable from the next row |
| `fixed`, unarchived, move **blocked** for a reason | only in prose, or by borrowing `unverified:` |
| **`open`, actually done** | **not at all** — this bug |

`unverified:` is defined as *what the status does not establish*, so a held archive move is
outside its stated purpose; `20` is using it because it is the only queryable field available.
That is worth recording as the same shortage rather than a second one: a record cannot say
"more finished than my status suggests" **or** "finished, but pinned here for a reason" in any
form a query reads.

The check shipped for this bug covers only the last row. The blocked-move row is left alone
deliberately — it is correctly labelled, its blocker is stated, and a check that fired on it
would be demanding a status change that is wrong.
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

**Implemented 2026-09-02.** `non_terminal_status_with_fix_anchor` in
`src/librarian/tools/doctor.rs`, beside `terminal_status_without_fix_anchor`. Mechanism
proposed by `codescout-20`.

```
status ∈ {open, investigating}  AND  body declares a patch-id  AND  not under archive/
  → report (never flip)
```

**Three design decisions, each of which changed after reading code rather than reasoning
about it:**

1. **It does NOT reuse `structured_fix_pointers`, and that is the whole difference between a
   working check and a decoration.** The obvious implementation reuses the sibling's detector.
   That helper reads `- **SHA:**` / `- **patch-id:**` list items — and measured 2026-09-02,
   **0 of the live `docs/issues/*.md` corpus used that shape.** Every live record writes
   provenance as prose, which is also what `get_guide("tracker-conventions")`'s own worked
   example teaches (*"Fixed at `38e0980b`"*); the structured form exists only under `archive/`.
   A check keyed on it would have returned **empty on both files that motivated this bug**, with
   a green suite — `CLAUDE.md` § *Testing Discipline*'s *"loudness is a property of a PATH"*,
   caught one step before shipping. `declared_patch_ids` reads both shapes.

2. **Keyed on the patch-id label, never a bare SHA.** A commit-like hash in prose is the decoy
   the sibling exists to name — measured 2026-08-19, 8 of its 9 findings carried one, usually
   the commit the bug was *observed* at. A 40-hex following the literal `patch-id` is produced
   by one act only, `git show <sha> | git patch-id --stable`, which nobody runs except to close
   a bug.

3. **`zombie` excluded.** It means *no longer observed, root cause unconfirmed* — fixed once,
   recurred, being watched. Its anchor is genuine and its non-terminal status is correct, so
   firing would demand a wrong change. Exact mirror of the sibling's `wontfix` exclusion.

**Discharge is a non-empty `unverified:`**, reusing the existing field rather than inventing a
second escape: a partial fix whose landed half has a patch-id is legitimately open, and *what
the status does not establish* is precisely that field's definition. Empty counts as absent,
matching the sibling and the guide.

**Side effect, and it is the reason the two motivating records now read correctly to the
machine as well as to a human:** flipping them to `fixed` at `8fb5f638` took
`terminal_status_without_fix_anchor` from 1 to 4, because their provenance was prose. Adding
proper `## Fix provenance` blocks took it back to 2. Both were true findings.

## Fix provenance

- **SHA:** `751d34a6` (`experiments`) — the check, its helper and seven tests.
- **patch-id:** `3c33c1cc212623101226371cfb1070cc7998d5b9` — content hash; survives rebase and cherry-pick.
- **SHA:** `36cb17ed` (`experiments`) — the section-scoping correction and its eighth test.
- **patch-id:** `2d76d739754f43dcbbe128f4434f130ab1d460a5` — content hash; survives rebase and cherry-pick.

**Two commits, neither superseding the other**, which is the shape `structured_fix_pointers`
was made plural to hold: `751d34a6` is the mechanism and `36cb17ed` is the premise correction
the live corpus forced. A record citing only the first would point at a version that produced
four false findings out of five; citing only the second would point at a diff that does not
contain the check. Both, in order.

This block is also the record closing its own loop. The check reads a patch-id **in a Fix
section** as an anchor, so writing this section is what makes this file visible to the
mechanism it describes — and flipping `status` to `fixed` in the same commit is what makes it
correctly silent again. Before that flip it fired on itself, which is the only end-to-end
confirmation available: the defect, the detector and the remedy exercised on one artifact.
## Tests added

Seven, in `src/librarian/tools/doctor.rs`, one discrimination each, following the sibling's
shape:

| test | pins |
|---|---|
| `..._fires_only_where_an_anchor_is_declared` | the positive, with an unanchored control |
| `..._is_silent_on_a_correctly_labelled_record` | `fixed`/`mitigated`/`wontfix` are not the subject |
| `..._needs_a_labelled_patch_id_not_a_bare_hash` | decoy SHA, short value, and fenced example all silent |
| `..._reads_prose_and_structured_alike` | both corpus shapes count |
| `..._ignores_zombie` | and `investigating` still fires |
| `..._is_discharged_by_a_non_empty_unverified` | empty counts as absent |
| `..._leaves_archived_records_alone` | the path-component skip |

**Verified by mutation, three sites, each killing precisely its intended test(s):**

| mutation | killed |
|---|---|
| narrow `declared_patch_ids` to the structured list form | 4 tests, incl. the prose test at 1-instead-of-2 |
| `s.len() == 40` → `>= 8` | the decoy test, via the short-value fixture |
| remove the archive path-component skip | the archive test |

The last one matters more than it looks. The archive fixture is **hand-seeded rather than using
`seed_archived_bug`**, because that helper hardcodes `status: fixed` — which this check's SQL
never selects, so the test would have passed without ever reaching the skip it exists to
verify. An inert fixture reading as coverage is the failure `CLAUDE.md` asks to be annotated on
the fixture line, and the mutation is what proves the annotation true.
## Workarounds

The `grep -l` pipeline in § *Reproduction*. It is not wired to anything and nobody runs it
unprompted, which is the whole defect.

## Resume

Flip this record to `fixed` with a `## Fix provenance` block once the implementing commit has a
SHA — the pair cannot be recorded before the commit exists, which is why it is a follow-up
rather than an omission. Until then this file is, deliberately and briefly, an instance of
itself: a record whose fix has landed and whose status does not say so. The check will not fire
on it, because the anchor it would key on is the thing that does not exist yet.
## References

- `docs/issues/archive/2026-09-02-index-description-omits-the-verify-action.md`,
  `docs/issues/archive/2026-09-02-memory-description-omits-the-refresh-anchors-action.md` — the two
  measured instances, both flipped at `8fb5f638`.
- `docs/issues/archive/2026-08-26-zombie-bug-files-are-reachable-by-no-query.md` — same family
  (a status no query reaches), same venue, already archived.
- `docs/trackers/observer-blindness.md` `OB-1` § *Sub-pattern — the third position: supplied
  CORRECTLY, to the wrong audience* — the observer structure, cited rather than duplicated.
- `get_guide("tracker-conventions")` § *Bug files* — `unverified:`, and the 2026-08-19
  measurement of the direction that does have a remedy.
