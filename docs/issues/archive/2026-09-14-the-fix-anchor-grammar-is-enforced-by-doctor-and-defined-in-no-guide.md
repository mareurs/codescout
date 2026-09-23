---
id: c1b002a3874742fd
kind: bug
status: fixed
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
(`docs/issues/archive/2026-09-13-fix-anchor-check-reads-a-cited-patch-id-as-a-claim.md`,
`docs/issues/archive/2026-09-13-fix-anchor-check-reports-absent-when-it-means-unparseable.md`), so
the check is actively worked; neither is about the missing documentation.

## Fix

**Shipped 2026-09-14.** The guide now states the grammar, immediately after the existing
SHA-and-patch-id discipline it was missing from.

**The budget caveat this section carried was WRONG, and correcting it is part of the fix.**
It read: *"the guide is a prompt surface with a documented character budget … size the
addition against that budget first."* Measured rather than assumed:

| bound | applies to | this guide |
|---|---|---|
| `MAX_DECLARED_SECTION_BYTES` = 2500 | sections carrying a `serves:` declaration | **0 such sections** (control: `librarian.md` has 13) |
| whole-guide cap | — | **none exists** |

Guides are the documented **overflow destination** for content evicted from
`server_instructions`' 2200-byte cap — the opposite of a scarce surface. The caution was
reasonable and cost nothing to hold, but a reader inheriting it would decline a free edit,
which is why it is corrected here rather than quietly dropped.

**What the guide now says, derived from the parser rather than its fixtures.** Reading
`structured_fix_pointers` surfaced two properties no fixture states alone:

- **Plural** — repeat the pair; each `- **patch-id:**` binds to the `- **SHA:**` above it.
  The singular parser this replaced pushed one author into a table, which read well to a
  human and was invisible to every check, so both anchors it recorded were verified by
  nothing.
- **Fence-aware** — a fenced example is a quotation, not a declaration. That is why quoting
  the shape in the guide costs nothing, and it is also the trap worth stating: a block
  fenced inside your own bug file declares nothing while still reading as anchored.

Plus the cross-repo `<repo>:<sha>` form, and the `no_fix_commit:` escape with the detail
that an empty value does not discharge it — pinned by the check's own test name,
`terminal_status_without_fix_anchor_is_discharged_by_a_non_empty_declaration`.

**Still not done, and still correct not to do:** teaching `doctor`'s remedy to print the
grammar inline. It serves whoever already tripped the check and does nothing for the reader
writing a correct record the first time, who is the population this bug was about.
## Tests added

`src/prompts/mod.rs::tracker_conventions_states_the_fix_anchor_grammar_doctor_enforces`.

Asserts the guide still NAMES each token the checker or its parser depends on — the heading
its refusal text cites, the two bullet prefixes `structured_fix_pointers` strips, and
`no_fix_commit`. **Tokens, not prose:** reds on the regression that actually happened (the
block going missing, or `doctor`'s format changing without the guide following) while
leaving every rewording free.

**What it deliberately does not check:** that the guide's *description* is correct. A token
present beside a wrong explanation passes. That half is unreachable from a string search and
is left to review rather than faked with a stricter-looking assertion.

Mutation-verified with the mutation asserted to have APPLIED before its result was read
(`context-injection-session-log:F-7`): replacing `## Fix provenance` with `## Fix pointer`
in the guide — 2 occurrences, 4 diff lines — fails the test with its intended message;
restoring leaves the guide byte-identical.

**Why this guard exists where the sibling one cannot.** `doctor.rs` and the guide compile
together, so one test couples them. The same shape across a repo boundary —
`principal-stamp.mjs`'s key against `session_key::PRINCIPAL_ARG_KEY`, shipped earlier the
same day — is checkable only on a machine holding both checkouts and is enforced by no
automated gate anywhere.
## Fix provenance

- **SHA:** `d83601ef` (`experiments`)
- **patch-id:** `c941237ce7209f4ce35b85bf3390f7df9bb79250`
- **SHA:** `824a6f60` (`experiments`)
- **patch-id:** `d17739d909b1693df583495863818bd0154c27ea`

Two commits, neither superseding the other: the first states the grammar in the guide, the
second couples the two surfaces so they cannot drift apart again. Recorded in the plural
form this very fix documented — which is the shape's own first use, and the reason the
singular parser it replaced was a defect.

Gate green on the second: `fmt-mine.sh` clean, clippy clean, lean lane 35 suites, default
lane 38 suites, zero failures. The default lane first aborted at 31/1 on a peer's
uncommitted `tests/issue_clusters.rs`; `attribute-red` named the holder, the file was not
touched, and the red cleared on re-run. Not claimed as caused by the report — repair and
stale report emit the same observable, and the owner holds the distinguishing fact
(`bug-fix-session-log:F-143`).
## References

- `src/librarian/tools/doctor.rs` — `scan_terminal_status_without_fix_anchor`, and the
  test fixtures that are the format's only executable specification
- `src/prompts/guides/tracker-conventions.md` — the guide that omits it
- `docs/issues/2026-09-14-the-dirty-check-reports-any-write-it-did-not-mediate-as-another-sessions.md`
  — the record whose finding surfaced this
