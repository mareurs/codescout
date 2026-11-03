# Closing advisory: `doc update` tells the caller when a closed bug lacks its fix anchor

Date: 2026-11-03. Bug: `b9eec23561e3299b`
(`docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`).
Size: a small change.

## Intent

When an agent closes a bug, it should learn the closing rule at that moment and decide for
itself. The operator chose "learn and decide" over "refuse" on 2026-11-03.

The measurement in the bug record (§ *Measured 2026-11-03*) is the why:

- Since 2026-10-17, 95 of 165 `fixed`/`mitigated` closings (58%) carry no fix anchor.
- Every archived bug carries one, because the archive guard refuses the rest.
- Auto-injection already reaches 101 of 121 closing sessions, but with other sections. Only 18
  saw `Closing a bug — patch-id and fix provenance`. Those left 17% unanchored, against 42%
  for the rest. That is a correlation on n=18, not a proof.

## Goals

- A `doc(action="update")` call that sets a bug's status to `fixed` or `mitigated`, and leaves
  the file without the recoverable fix pair, returns a short advisory in its response.
- The advisory says what is missing and names the guide section that explains it.
- The write always succeeds. The advisory never refuses.

## Non-goals

- No change to `Shape`, `emit_guide_sections`, the overlap gate or the byte ceilings.
- No delivery of the full section text. The caller fetches it if it wants it.
- No change to the archive guard, to `move`, or to `doctor`.
- Step 2 of the bug (`check~<name>` for `doctor`) is not addressed.

## Anti-goals

- A refusal, or anything that makes a closing call fail.
- An advisory on a record that is already correct. It must be silent when the pair is present.
- A second definition of "anchored". The advisory, `doctor` and the archive guard must agree.

## Constraints

- The predicate is `doctor::declares_fix_pair` / `declares_fix_anchor`
  (`src/librarian/tools/doctor.rs:7462` and nearby), the same one the archive guard
  `refuse_unanchored_archive` (`doctor.rs:7527`) uses. Checked against `new_content`, the file
  as written, so a call that closes and adds `## Fix provenance` together is silent.
- The trigger point is in `update::call` (`src/librarian/tools/update.rs:539`), next to the
  archive guard (`update.rs:813`). The top-level `status` shortcut is lifted into
  `patch.status` at `update.rs:553`, so one check covers both spellings.
- The response today is `{"id", "updated": true}` plus optional fields (`update.rs:941`).
  The p50 ceiling fixture updates to `active`, so it does not reach the advisory.

## Approaches

Considered on 2026-11-03, in conversation:

- **A. Deliver the section by status value.** Grammar amendment, multi-topic emission, overlap
  gate change. Ships 3,212 B once per session, also to correct closings. Rejected: four coupled
  changes on the path that carries the anti-wipe guard.
- **B. Advisory field in the `update` response, only on a violation.** Chosen. It fires exactly
  in the case the measurement shows is wrong, reuses the existing predicate, and touches no
  emission code.
- **C. Result-field predicate in `Shape` plus multi-topic emission.** Would deliver full text
  only on a violation and unify with step 2. Rejected for now: it keeps all of A's changes.
  B's field is the hook C would read, so C stays open as a later step.

## Decided

- Approach B (operator, 2026-11-03).
- Inform, never refuse (operator, 2026-11-03).

## My calls (check these)

1. **Trigger.** Fires only when THIS call sets the status to `fixed` or `mitigated`, on
   `kind: bug`. A later body edit on an already-closed bug stays silent, to keep noise low.
2. **Two texts, as the archive guard has.** A record with nothing owes both bullets. A record
   with a SHA but no patch-id owes the patch-id. The text is shared with
   `refuse_unanchored_archive`, not copied, so the two cannot drift.
3. **Field name** `fix_anchor_advisory`, a string. A stable name lets the transcript scan
   from the measurement count it later.
4. **Content.** About 300 B: what is missing, the two remedies (`## Fix provenance` with
   `- **SHA:**` and `- **patch-id:**`, or `extra.no_fix_commit`), that archiving will be refused
   until then, and `get_guide("tracker-conventions")` § `Closing a bug — patch-id and fix
   provenance`.
5. **Caveats left out.** `terminal_status_with_caveat` (116 today) is not covered. Its
   predicate needs a catalog lookup of the tracked home, and the measurement did not test
   whether `Settled caveats` changes behaviour. A follow-up can add it to the same field.

## Success

- Unit tests: fires on `fixed` and `mitigated` with no anchor; the SHA-only text; silent with
  the pair; silent with `no_fix_commit`; silent on `wontfix`, `active` and non-bug kinds;
  silent when the call sets no status; fires through the top-level `status` spelling; the
  write succeeds in every case.
- The existing ceiling tests stay green with no change to their constants.
- Re-run the transcript scan two weeks after release. The rate of unanchored closings should
  fall below 42%. If it does not, approach C is the next step.

## Left to the builder

- Where the shared message builder lives, and its exact wording within the content above.
- Which prompt surfaces describe `update`'s response fields and need the new one
  (`src/prompts/README.md` lists them).
- Updating the bug record when it ships.
