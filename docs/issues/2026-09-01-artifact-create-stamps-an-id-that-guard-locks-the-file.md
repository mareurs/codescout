---
status: open
opened: 2026-09-01
closed:
severity: high
owner: marius
related: []
tags:
  - cluster/gate-keyed-on-unobservable-event
  - librarian
  - guard
  - artifact-create
  - doc-vs-code
kind: bug
unverified: 'CLUSTER TAG IS ARGUABLE and deliberately flagged rather than hidden — this defect fails LOUDLY (a RecoverableError), and IC-2 carries an explicit falsification clause for exactly that ("Falsified by a member whose proxy failure surfaced as an error rather than a plausible result"). Tagged IC-2 anyway because the MECHANISM is an exact match — its claim names "a monotone stamp" as one of the three proxy shapes, and the id: stamp is one. IC-14 was considered and rejected as INVERTED (this guard is broader than its name, not narrower). A second read should rule: either IC-2 widens its claim to cover fail-closed proxies, or this is evidence for a new class. Filing honestly is blocked by the absence of a cluster/unclassified slug — tests/issue_clusters.rs requires exactly one known tag, so a novel defect must borrow the nearest class. No regression test yet.'
---

# BUG: artifact(create) stamps `id:` into frontmatter, so every file the tool creates becomes permanently un-editable by `edit_markdown` — including plain prose docs the pinned test says must stay editable

## Summary

`artifact(action="create")` writes `id: '<16-hex>'` into the new file's YAML
frontmatter. `librarian_guard`'s `stamped` predicate reads that field as "the librarian
owns this file", so **every artifact created through the tool is immediately refused by
`read_markdown` / `edit_markdown` / `read_file` / `edit_file`** — regardless of `kind`,
and regardless of whether it is augmented or a ledger. A plain `kind: doc` prose file
whose state lives entirely in the file is locked to the `artifact(get/update)` API for
the rest of its life. This contradicts a documented ruling that was tried, measured, and
reverted (`bb9a94d7`), and it converts files by hand into the case the guard's own pinned
test exists to forbid.

## Symptom (Effect)

Created `docs/TEAM-ONBOARDING.md` via `artifact(action="create", kind="doc", ...)` —
a plain prose onboarding guide, no augmentation, no `entry_prefix`. A one-line prose fix
one call later:

```
edit_markdown(path="docs/TEAM-ONBOARDING.md", action="edit", heading="## Layer 2: ...", ...)
→ {
    "ok": false,
    "error": "'docs/TEAM-ONBOARDING.md' is a librarian-managed artifact — do not read or edit it directly",
    "hint": "Use artifact tools instead:\n• Read:   artifact(action=\"get\", id=\"<id>\")..."
  }
```

Note the error text: it names **no reason**. The guard's `why` clause appends
`(augmented — …)` or `(a ledger — …)` for those two arms and **the empty string for the
`stamped` arm** (`src/util/librarian_guard.rs:106-108`), so the one arm whose firing is
most likely to be unintended is also the one that explains itself least.

## Reproduction

At `72484f8d5817e4675191d84caaaad869abf78f71` (`experiments`):

```
1. artifact(action="create", kind="doc", title="Scratch", rel_path="docs/scratch-probe.md",
            body="## A\n\nhello\n")
2. head -4 docs/scratch-probe.md
   → ---
     id: '<16-hex>'          <- written by create, not by the author
     kind: doc
     status: draft
3. edit_markdown(path="docs/scratch-probe.md", action="edit", heading="## A",
                 old_string="hello", new_string="world")
   → REFUSED: "is a librarian-managed artifact"
4. read_markdown(path="docs/scratch-probe.md")
   → REFUSED, same message
```

Control, in the same session: `edit_markdown` on `CONTRIBUTING.md` and `README.md`
**succeeded**. Both are catalog rows (`CONTRIBUTING.md` is `f16e5e8dabdf42ef`) — they are
simply not *stamped*, because they were indexed rather than created. Membership is not the
discriminator; the stamp is.

## Environment

- codescout `experiments` @ `72484f8d5817e4675191d84caaaad869abf78f71`
- Claude Code, `~/.claude-sdd` profile, MCP stdio
- Observed live during the 2026-09-01 system-review session

## Root cause

`guard_with_oracle` (`src/util/librarian_guard.rs:83-137`) refuses on **three independent
predicates**, documented at `:88-93` as "none implies another":

```rust
let augmented = matches!((abs_path, oracle), (Some(p), Some(o)) if o.is_augmented(p));
let stamped   = is_librarian_artifact(text);          // <- an `id: <16-hex>` in frontmatter
let ledger    = !declared_entry_prefixes(text).is_empty();
if !augmented && !ledger && !stamped { return Ok(()); }
```

`is_librarian_artifact` (`:141`) is a pure text predicate: frontmatter containing
`id:` with a 16-char lowercase hex value. The intent is *"the librarian wrote this file"*.

The defect is not in the guard alone — it is the **composition** with `artifact(create)`,
which stamps that same field into every file it writes. The stamp was meant as provenance;
the guard reads it as a claim about where state lives. One field, two meanings, and the
writer makes the reader's predicate true unconditionally.

**Measured 2026-09-01** (`head -4` on three files, output in *Symptom* / *Reproduction*):
`docs/TEAM-ONBOARDING.md` and `docs/trackers/2026-09-01-fable-system-review.md` — both
created via `artifact(create)` this session — carry `id: '…'`; `CONTRIBUTING.md`, indexed
only, has no frontmatter at all and edits fine. Guard behaviour confirmed at runtime by
the refusal above, not inferred from the source.

**Why it was invisible until now — the repair activated it.**
`docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md` fixed the
guard's blindness to *quoted* ids. `artifact(create)` emits the quoted form
(`id: '23421bbc5b226368'`). So before that fix, every tool-created artifact was
accidentally exempt; making the guard correct on its stated axis switched this defect on
for the whole population of created files. That is the retrospective's second-order note —
*silences regenerate under repair* — running in the other direction: a **refusal** was
generated under repair.

## Evidence

### The documented ruling this violates

`src/prompts/guides/tracker-conventions.md` § *Make the tracker guarded*:

> **Do NOT stamp `id: <16-hex>` into a tracker's frontmatter to guard it.** This guide
> recommended exactly that until 2026-08-18, and the advice was wrong in a way worth
> knowing rather than just deleting:
>
> - It guards on **catalog membership**, the axis the guard's own pinned test rejects.
>   `a_catalogued_but_unaugmented_file_stays_directly_editable` argues that
>   membership-guarding would refuse `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR
>   — all catalog rows — and it names a prose tracker that `CLAUDE.md` documents
>   `edit_markdown` for. Stamping an id converts a file by hand into the case that test
>   exists to forbid.
> - It was tried, and it broke a documented workflow. Stamping `id:` into
>   `reconnaissance-patterns.md` silently disabled `docs/TAXONOMY.md`'s `edit_markdown`
>   append path for R-N; confirmed by probe, reverted in `bb9a94d7`.

The prohibition is addressed to a *human* hand-editing frontmatter. Nothing audited the
**tool** against it, and the tool does it on every create.

### The pinned test's rationale, quoted from the archived bug that added it

`docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`:

> - `a_catalogued_but_unaugmented_file_stays_directly_editable` — two rows,
>   `skill-frictions.md` and `RELEASE.md`. **Green before the oracle was wired**, on
>   purpose: it pins the behaviour the widening must not break, and it is the test that
>   would have caught the catalog-membership design had I implemented it.

The test passes today, because both its rows are unstamped files. It cannot see this
defect: its population contains no *created* artifact. That is
CLAUDE.md § *Testing Discipline*'s recording-filter law — the refuting outcome leaves no
artifact in the chosen sample — and widening the sample **does** fix this one: add a row
built by `artifact(create)`.

### The consequence is documented as silent, and that is the load-bearing half

`bb9a94d7`'s precedent is the shape to worry about: stamping "**silently** disabled
`docs/TAXONOMY.md`'s `edit_markdown` append path for R-N". The tool call errors loudly,
but the *documented workflow* stops being available with no notice — CLAUDE.md's
`edit_markdown(...)` recipe for `docs/trackers/skill-frictions.md` is one stamp away from
being false, and the doc would keep prescribing it.

## Hypotheses tried

1. **Hypothesis:** the refusal came from the file being a catalog row.
   **Test:** `edit_markdown` on `CONTRIBUTING.md` and `README.md`, both catalog rows.
   **Verdict:** rejected — both succeeded. Membership is not the trigger.
2. **Hypothesis:** the refusal came from `kind: tracker` / ledger detection.
   **Test:** the refused file is `kind: doc` with no `entry_prefix`.
   **Verdict:** rejected.
3. **Hypothesis:** `artifact(create)` stamps `id:` and the `stamped` predicate fires.
   **Test:** `head -4` on two created files vs one indexed file; guard source at
   `src/util/librarian_guard.rs:95`.
   **Verdict:** confirmed.

## Fix

Two candidate directions; **the second is the one this file recommends**, and they are not
exclusive.

**(a) Stop stamping on create.** Let `reindex` associate path→id in the catalog, as it
already does for `CONTRIBUTING.md`. Cheapest, and it aligns the tool with the documented
prohibition. Risk to check before doing it: the frontmatter `id:` is what
`doctor`'s `frontmatter_id_mismatch` / `repair_frontmatter_id` read, and
`get_guide("tracker-conventions")` says a ledger's identity "must travel with the repo" —
so removing the stamp wholesale may cost cross-machine identity recovery. Verify what
actually consumes it before deleting it.

**(b) Narrow the `stamped` predicate — guard on the property, not the provenance.** The
guard's real question is *"does this file's state live somewhere other than this file?"*
`augmented` and `ledger` answer it directly. `stamped` answers a different question and is
the arm with no explanatory `why` text, which is itself a tell. Replace it with a
`kind`-aware check, or drop it and let the two precise predicates carry the guard.

Either way: **make the `stamped` arm say why it fired.** An empty `why`
(`src/util/librarian_guard.rs:106-108`) on the least-intentional arm is a
`docs/adrs/2026-08-27-negative-results-name-their-scope.md` violation in refusal form.

SHA: not yet fixed.
patch-id: not yet fixed.

## Tests added

None yet. The regression test is a new row in
`a_catalogued_but_unaugmented_file_stays_directly_editable`'s table: a file produced by
`artifact(action="create", kind="doc", ...)` must stay directly editable. **Demand a
deliberate break** — the row must fail against today's tree before the fix, or it is
pinning nothing (CLAUDE.md § SDD Rulings).

## Workarounds

Route every read/write of a tool-created artifact through the catalog:
`artifact(action="get", id=…)` and `artifact(action="update", id=…, patch={body_edits: […]})`.
Both work and are the sanctioned path for genuine trackers — the cost is on plain prose
docs, where the indirection buys nothing.

To un-lock a file deliberately: delete the `id:` line from its frontmatter and re-run
`librarian(action="reindex")` (the catalog re-associates by path). **Verify** the row
survives — `artifact(action="find", filter={"rel_path": {"contains": …}})` — before
relying on it.

## Resume

Decide (a) vs (b) — start by running
`grep(pattern="frontmatter_id|repair_frontmatter_id", path="src/librarian/")` and
`references(symbol="is_librarian_artifact", path="src/util/librarian_guard.rs")` to
enumerate what actually consumes the stamp. Then add the failing test row described in
*Tests added* against today's tree and confirm it reds before touching the guard.

Also owed: a ruling on the cluster tag (see `unverified:`), and a decision on whether
`docs/TEAM-ONBOARDING.md` — a teammate-facing prose doc — should keep its stamp at all.

## References

- `src/util/librarian_guard.rs:83-160` — the three predicates and `is_librarian_artifact`
- `src/prompts/guides/tracker-conventions.md` § *Make the tracker guarded* — the ruling
- `docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md` — the
  repair that activated this, and the pinned test's rationale
- `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
  — the retracted membership-guarding proposal; this bug is the same axis from the other side
- `docs/trackers/2026-09-01-fable-system-review.md` SR-13 item 4 — where this was first
  noted, as a much weaker claim ("read-refusal is paternalistic") that the probe corrected
