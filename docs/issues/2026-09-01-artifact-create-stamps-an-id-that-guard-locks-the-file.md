---
status: investigating
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
unverified: "PARTIAL FIX ONLY — the refusal message now names the stamp and its mechanism (tested, born red), but the BEHAVIOUR is unchanged: a tool-created plain doc is still refused for reads and body writes. What remains is a decision between three scopes (see Fix § Still owed), not more investigation — deliberately left to the operator because it changes protection semantics for 263 existing files (57 of 120 trackers, 206 of the bug corpus). CLUSTER TAG IS ARGUABLE and flagged rather than hidden: this defect fails LOUDLY, and IC-2 carries an explicit falsification clause for exactly that. Tagged IC-2 on the exact match to the monotone-stamp proxy shape its claim names; IC-14 rejected as inverted (this guard is broader than its name, not narrower). A second read should rule: either IC-2 widens to fail-closed proxies, or this needs a new class — and filing honestly is blocked by the absence of a cluster/unclassified slug."
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

### Direction (a) — stop stamping on create: DEAD, ruled out 2026-09-01 by enumeration

The frontmatter `id:` has real consumers. `grep(pattern="frontmatter_id_mismatch|repair_frontmatter_id", glob="src/**/*.rs")` — **60 matches across 5 files**, concentrated in
`src/librarian/tools/doctor.rs` (54), plus `mv.rs` (3), `adapter.rs`, `catalog/artifact.rs`,
`tools/librarian.rs`. `doctor`'s `repair_frontmatter_id` fix mode exists to write this field,
and `get_guide("tracker-conventions")` requires a ledger's identity to travel with the repo.
Removing the stamp would break cross-machine identity recovery to fix a guard bug. **The stamp
is correct; the guard's reading of it is the defect.**

### Direction (b) — narrow the `stamped` predicate: correct, and contained

`references(symbol="is_librarian_artifact", path="src/util/librarian_guard.rs")` — **exactly
one production caller**, `src/util/librarian_guard.rs:95`; the other ten hits are its own
tests. So the predicate can be narrowed without touching anything else.

**What `stamped` actually protects, established by reading the three arms against each other.**
The guard collapses three different scopes into one blanket read-and-write refusal:

| arm | what is elsewhere | reads wrong? | body writes wrong? | frontmatter writes wrong? |
|---|---|---|---|---|
| `augmented` | the params — file is a rendered snapshot | **yes** (stale, unsignalled) | yes | yes |
| `ledger` | the `PREFIX-N` counter | no | no (`body_edits` is fine) | yes |
| `stamped` | nothing — the catalog *indexes* frontmatter | no | no | **yes** (BL-48 drift) |

Only `augmented` justifies refusing a read. `stamped`'s real concern is BL-48: a direct
frontmatter edit does not reach the catalog, so `status`/`tags`/`title` drift between disk and
index. That is a **frontmatter-write** concern, and the guard applies it to reads and body
writes as well.

### The argument that settles it: `stamped` guards an arbitrary subset

**Measured 2026-09-01.** `docs/issues/_TEMPLATE.md` carries **no `id:`** — so every bug file
created the *documented* way (copy the template) is unstamped and therefore unguarded. Only
tool-created files are stamped:

```
git grep -lE "^id: '?.?[0-9a-f]{16}" -- 'docs/trackers/*.md' | wc -l   #  57 of 120
git grep -lE "^id: '?.?[0-9a-f]{16}" -- 'docs/issues/*.md'   | wc -l   # 206 of 552
```

So the `stamped` arm protects **~47% of trackers and ~37% of the bug corpus, selected by
creation route rather than by any property of the file.** A bug file's frontmatter tags are
exactly as drift-prone whether a human or the tool created it, and the human-created majority
is unprotected. Whatever `stamped` is for, it is not coherently achieving it.

**This also raises the blast radius of the write-side change**, which is why it is not being
made unilaterally: 263 existing files are currently refused, and narrowing `stamped` un-guards
all of them at once.

### Shipped 2026-09-01 — the message half

The `stamped` arm's `why` was the empty string, so the arm most likely to fire unintended was
the only one that refused **anonymously**. It now names the stamp and the mechanism it
protects. No behaviour change; a refusal a reader can act on and judge.

- `src/util/librarian_guard.rs` — the `why` arm in `guard_with_oracle`
- Regression test `a_stamped_refusal_names_the_stamp_as_its_reason`, **verified born red**:
  with the `""` restored it fails with `got: 'docs/TEAM-ONBOARDING.md' is a librarian-managed
  artifact — do not read or edit it directly`, then passes with the fix. 21/21 in the module.
- The pinned test `a_catalogued_but_unaugmented_file_stays_directly_editable` gained a
  fixture annotation naming its load-bearing detail — neither row carries an `id:`, and that
  absence is the whole discriminator, so a tidy-up "completing" either frontmatter block
  would leave it passing and testing nothing.

SHA: `0933bc95` (**`experiments`**) — the message half only.
patch-id: `42c3dcd17a52ed55f082c14de843173f88a2fb2c`

### Still owed — the behavioural decision, for a human

Three defensible scopes for `stamped`, in increasing blast radius:

1. **Allow reads, keep refusing writes.** A read cannot drift. Needs a read/write intent
   parameter on `guard_not_librarian_managed` — three call sites (`read_markdown`,
   `edit_markdown`, `edit_file`). Strictly capability-increasing; nothing that works today
   stops working.
2. **Allow reads and body writes; refuse frontmatter writes.** Matches what BL-48 actually
   describes. `edit_markdown` already takes a `frontmatter` param, so the caller's intent is
   knowable at the call site.
3. **Drop `stamped` entirely**, leaving `augmented || ledger`. Simplest, and the subset
   argument above says it protects nothing coherent — but it un-guards 263 files, and bug-file
   tag drift (BL-48) is a real cost CLAUDE.md warns about explicitly.

Recommendation: **(2)**, because it is the only one that matches the mechanism. Not taken here
because it changes protection semantics for 263 existing files, which is a call for the
operator rather than a drive-by.

## Tests added

`a_stamped_refusal_names_the_stamp_as_its_reason` — in `src/util/librarian_guard.rs`'s
`mod tests`. **Born red and verified so**, not assumed: the fix was reverted, the test failed
on the anonymous message, the fix was restored, 21/21 green. It asserts on two things (the
word `stamped`, and `frontmatter` naming the mechanism) and carries two precondition asserts
so it cannot silently start passing via the `ledger` or `augmented` arm.

**Still owed for the behavioural fix:** a new row in
`a_catalogued_but_unaugmented_file_stays_directly_editable`'s table built by
`artifact(action="create", kind="doc", ...)`, which must red against today's tree before any
of the three scopes above is implemented.
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

**Enumeration is done — do not redo it.** (a) is dead (60 consumers of the frontmatter id),
(b) is contained (one production caller of `is_librarian_artifact`), and the population is
263 files. The message half is shipped and tested.

What remains is one decision, then a small implementation: pick scope 1, 2 or 3 from
*Fix § Still owed*. Recommendation is (2). Then add the failing table row named in
*Tests added* and thread a read/write (or frontmatter/body) intent through the three call
sites — `read_markdown`, `edit_markdown`, `edit_file`, per
`librarian_guard_fires_on_every_edit_file_write_path`.

Also still owed: a ruling on the cluster tag (see `unverified:`), and a decision on whether
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
