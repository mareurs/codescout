---
kind: bug
status: open
tags:
- cluster/doc-contradicted-by-code
- librarian
- append_entry
- entry-id
- tracker
- guard
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
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

Not written. The refusal names its own remedy:

```
artifact(action="update", id="f2ecdd76a6189efb", patch={extra: {"entry_prefix": "T"}})
artifact(action="update", id="1dcfdd70de0fcc73", patch={extra: {"entry_prefix": "I"}})
```

**But that carries a numbering decision, and it should be taken deliberately.** Existing
entries are zero-padded (`### T-001` …). `body_claimed_indices`
(`src/librarian/catalog/augmentation.rs:1282-1292`) parses `T-001` as **1**, so the
allocator would issue the next *integer*, rendered unpadded: `T-16`, not `T-016`. Under
the resolver's `\b[A-Z]{1,3}-\d+\b` those are **distinct tokens**, so declaring the prefix
without deciding the format forks the namespace by spelling — the zero-padding hazard
`IC-6` already names. Decide first: adopt unpadded and accept a mixed ledger, normalise
the existing ids, or render padded.

## Tests added

None — no fix written. A regression test for the fixed state would assert that every
tracker named in `docs/TAXONOMY.md` with an `append_entry` recipe declares an
`entry_prefix`. That is a doc-to-code join, and it is the mechanizable sub-shape `IC-11`
already records.

## Workarounds

Reading is fine via `artifact(action="get", id=…)`, optionally with `entry_filter`. For
**writing** there is no safe documented route today. Do **not** reach for
`artifact_augment(merge=true, params={observations: […]})` — it replaces the collection
wholesale, and CLAUDE.md records that call destroying 18 of 19 T-N entries on 2026-08-16.

## Resume

Take the numbering decision first (padded / unpadded / normalise), then declare
`entry_prefix` on both artifacts with the two `artifact(action="update")` calls above.
Verify by re-running `librarian(action="doctor")` and by one `append_entry` against a
scratch ledger — not against these two, until the format is settled.

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
