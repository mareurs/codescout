---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
- librarian
- append_entry
- entry-id
- tracker
- guard
closed: 2026-09-02
high_water_derivation: 'T=32 and I=8 were pinned in the same patch as the prefix, rather than left for the allocator to derive, so a compaction between declaration and first use cannot lower body_max and cause a reissue (the 2026-08-17 silent-repoint defect). Both were checked equal to the live maximum before pinning: 32 params rows and 32 body headings topping at ### T-32; 8 rows and 8 headings topping at ### I-8. Equal counts on both surfaces is what rules out an existing compaction gap.'
opened: 2026-09-02
owner: marius
related: []
severity: high
unverified: 'The PRECONDITION is verified at the bytes; the ALLOCATION is not. Both files now declare entry_prefix (T / I) and a matching entry_high_water, read back from disk. But no append_entry was run against either, because a successful call allocates a real id and writes a real entry, and that is a content decision rather than a probe. So the claim ''append_entry now works here'' rests on allocate_entry_id''s frontmatter check being the only thing that was failing — read at augmentation.rs:971-983, not observed. The next person to append verifies it for free; if it still refuses, the cause is downstream of the declaration and this record is reopened rather than re-derived. No regression test either: the durable form is a doc-to-code join asserting that every tracker with an append_entry recipe in docs/TAXONOMY.md declares an entry_prefix, which is IC-11''s mechanizable sub-shape and is not written.'
---

# BUG: T-N and I-N have no open append path — both documented routes refuse, leaving only the one that destroys the collection

## Summary

`docs/trackers/tool-usage-patterns.md` (T-N) and `docs/trackers/test-escape-hardening.md`
(I-N) each declare an `entry_collection` in a committed augmentation sidecar but **no
`entry_prefix` in frontmatter**. `append_entry` therefore refuses, and the librarian guard
refuses `edit_markdown` because both files are *augmented*. CLAUDE.md § *Session
Intelligence Trackers* and `docs/TAXONOMY.md` both route T-N and I-N to `append_entry`.
The only write route left open is `artifact_augment(merge=true, params={…})`, which
**replaces** the collection — the exact call CLAUDE.md records as taking the T-N queue
from 19 entries to 1 on 2026-08-16.

## Symptom (Effect)

`read_markdown` on the tracker, run 2026-09-02 against server pid 874583 on a fresh
release image:

```
'docs/trackers/tool-usage-patterns.md' is a librarian-managed artifact (augmented — its
params live in the catalog, and this file is only a rendered snapshot of them) — do not
read or edit it directly
```

`append_entry`'s refusal, quoted from the code that raises it rather than from a run (see
*Reproduction* for why it was not run):

```
allocate_entry_id: `<abs_path>` does not declare an entry_prefix
```

## Reproduction

At `450d34fd`, main checkout, MCP over stdio:

1. `read_markdown(path="docs/trackers/tool-usage-patterns.md")` → the refusal above.
   Same for `docs/trackers/test-escape-hardening.md`.
2. `append_entry(id="f2ecdd76a6189efb", id_prefix="T", …)` — **deliberately not run.** A
   successful call allocates a real id on a shared tracker, so the probe is not free the
   way the read is. The refusal is established from the check itself, cited below, and
   this asymmetry is why one half of this record is measured and the other inferred.

## Environment

Linux, codescout `0.15.0`, branch `experiments` at `450d34fd`, MCP stdio, server pid
874583 running `target/release/codescout` with no `(deleted)` marker — freshness confirmed
before the probe, per memory `gotchas` § MCP Binary Symlink.

## Root cause

Two independent guards, each correct alone, composing into a closed door.

- **`allocate_entry_id`** (`src/librarian/catalog/augmentation.rs:971-983`) resolves the
  namespace with `declared_prefixes_from_frontmatter(fm.as_ref())` and hard-refuses when
  the result is empty. Frontmatter is the *only* source; an `entry_collection` in the
  augmentation does not supply one.
- **`librarian_guard::Access`** (`src/util/librarian_guard.rs:32-41`) documents a matrix
  in which the `augmented` reason refuses read, body write **and** frontmatter write.
  Both files are augmented, and the same `doctor` run reports
  `augmentation_declared_but_absent: 0`, so every declared sidecar is attached in this
  catalog.

**The guard reason is `augmented`, not `stamped`** — worth stating because the obvious
reading is wrong. `stamped` was narrowed on 2026-09-01
(`docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md`)
to frontmatter writes only, so blaming the stamped `id:` predicts a closed door for every
unaugmented stamped file, where there is none.

measured 2026-09-02: `read_markdown` run once, refusal quoted verbatim above; frontmatter
read directly with `sed -n '1,20p'` on both files, neither declaring `entry_prefix`;
`grep -l "^entry_prefix:" docs/trackers/*.md` returns 37 ledgers and neither file is among
them. The `allocate_entry_id` branch is **inferred from
`src/librarian/catalog/augmentation.rs:971-983` — not measured**, for the reason in
*Reproduction*.

## Evidence

### The population this sits in

Of 23 committed sidecars under `docs/augmentations/`, 14 declare an `entry_collection`.
Only **6** also declare a frontmatter `entry_prefix` and can therefore allocate at all:
`GF`, `DC`, `FND`, `FT`, `A`, `SD`. The other 8 cannot. Six of those 8 are not obviously
affected because no documentation routes an append to them; T-N and I-N are the two that
CLAUDE.md and `docs/TAXONOMY.md` name.

### The entries that already exist

`artifact(action="get", id="f2ecdd76a6189efb")` lists `### T-001` … `### T-009` (+6 more)
— zero-padded, at `###` level. So the ledger has been appended to historically, and
whatever route did that is not one of the two documented ones today.

## Hypotheses tried

1. **Hypothesis:** the stamped `id:` in frontmatter is what closes `edit_markdown`.
   **Test:** read the `Access` doc-comment matrix at `src/util/librarian_guard.rs:32-41`.
   **Verdict:** rejected — `stamped` refuses frontmatter writes only, since 2026-09-01.
   The closure is `augmented`. **Evidence:** *Root cause*, second bullet.
2. **Hypothesis:** `entry_prefix` might be sourced from the catalog augmentation, making
   the frontmatter absence harmless. **Test:** read `allocate_entry_id`.
   **Verdict:** rejected — `declared_prefixes_from_frontmatter` is the only source, and
   its own hint tells the caller to write frontmatter via `patch={extra: …}`.
   **Evidence:** *Root cause*, first bullet.

## Fix
**Provenance.** Recorded 2026-09-02, at archive time rather than fix time — this file had no fix
pointer at all until then, and the only hash in it (`450d34fd`, on both Environment lines) is the
branch tip at reproduction, not a fix.

- **SHA:** `44a2d0ab` on **`experiments`** — positional; it dies when `experiments` is rebased.
- **patch-id:** `76a60568db56cb086f54132315099e70003330c4` — content hash of the diff; survives
  rebase and cherry-pick. `git show 44a2d0ab | git patch-id --stable`.

Identified positively, not by proximity: `44a2d0ab` modifies this bug file plus exactly `+2` lines
each to `docs/trackers/tool-usage-patterns.md` (the `T` prefix) and
`docs/trackers/test-escape-hardening.md` (the `I` prefix) — the two declarations this bug is about,
and nothing else.

Applied 2026-09-02. Both artifacts now declare their namespace in committed frontmatter,
written through the catalog so the change reaches the index as well as the file:

```
artifact(action="update", id="f2ecdd76a6189efb", patch={extra: {"entry_prefix": "T", "entry_high_water_T": 32}})
artifact(action="update", id="1dcfdd70de0fcc73", patch={extra: {"entry_prefix": "I", "entry_high_water_I": 8}})
```

**The numbering question dissolved on contact with the data.** This record originally posed
it as a decision between padded and unpadded. The params show the ledger *already* switched:
`T-001` … `T-013` padded, then `T-14` … `T-32` unpadded. So unpadded is the established form
for 19 of 32 entries and there was nothing to decide — only something to discover. The
decision I was about to ask for had been taken, by someone else, some time ago.

**The high-water marks were pinned rather than derived**, and the reason is a hazard this
fix could otherwise have introduced. `allocate_entry_id` computes the next id from
`max(entry_high_water, reservation, body_max)` and **never reads the params collection**. Had
the body carried fewer headings than the collection has rows — the ordinary result of
compaction — the first allocation would have reissued a live id, silently re-pointing every
citation of it (`docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`).
Checked before pinning: 32 body headings against 32 params rows, highest `### T-32`; 8
against 8, highest `### I-8`. No gap on either, so `T-33` and `I-9` are next.

**Declaring the prefix adds the guard's `ledger` reason to both files, and that changes
nothing** — the `augmented` reason already refused read, body write and frontmatter write on
both. No capability was withdrawn by this fix.
## Tests added

None, and the absence is the weaker half of this fix rather than an oversight worth
excusing. The durable regression test is a doc-to-code join: assert that every tracker
`docs/TAXONOMY.md` gives an `append_entry` recipe for declares an `entry_prefix`. That is
precisely the mechanizable sub-shape `IC-11` already records itself as owning, and it would
have caught this the day the frontmatter requirement landed. Not written here because it
needs a parser for TAXONOMY's recipe table, which is its own change.

What exists instead is weaker and worth naming as weaker: `librarian(action="doctor")` will
now treat both files as ledgers, so `ledger_defines_nothing` and `entry_without_definition`
cover them — but neither of those fires on the defect this record is about, which was a
ledger that never declared itself at all.
## Workarounds

Reading is fine via `artifact(action="get", id=…)`, optionally with `entry_filter`. For
**writing** there is no safe documented route today. Do **not** reach for
`artifact_augment(merge=true, params={observations: […]})` — it replaces the collection
wholesale, and CLAUDE.md records that call destroying 18 of 19 T-N entries on 2026-08-16.

## Resume

N/A for the declaration itself. One thing is outstanding and it is a verification, not a
task: the first real `append_entry` against either tracker confirms the allocation path, and
nobody needs to make a special trip — the next `T-N` or `I-N` someone files is the probe. If
it refuses, reopen this record rather than re-deriving it; the cause would be downstream of
the declaration, which is verified on disk.

A third documentation surface turned up during the fix and is worth knowing about if this
recurs elsewhere: `test-escape-hardening.md`'s own **augmentation prompt** instructs
`artifact(append_entry, entry_collection="interventions", id_prefix="I")`. So the artifact
was serving the instruction that its own frontmatter made impossible — a fourth place to
check when auditing whether a documented route still exists.
## References

- `src/librarian/catalog/augmentation.rs:971-983` — the refusal
- `src/librarian/catalog/augmentation.rs:1282-1292` — `body_claimed_indices`, the padding hazard
- `src/util/librarian_guard.rs:32-41` — the `Access` matrix
- `docs/issues/archive/2026-09-01-artifact-create-stamps-an-id-that-guard-locks-the-file.md` — why `stamped` is not the cause
- CLAUDE.md § *Session Intelligence Trackers* and `docs/TAXONOMY.md` — the two surfaces that route T-N and I-N to `append_entry`

**Cluster note.** Tagged `cluster/doc-contradicted-by-code`. The Index table's one-line
headline reads *"documentation denies a capability the code has since gained"*, which looks
like the opposite of this record — but the class file's own title ends *"because the prose
was true when written"*, and the class already contains
`advertised-required-overstates-what-the-code-enforces`, prose asserting **more** than the
code does. So the fit is on the mechanism, and the direction is not novel. Worth recording
only because the compressed headline in the Index mispredicts membership for the
assert-direction half of its own class — a doc surface disagreeing with the thing it
summarises, which is this class's shape one surface over.
