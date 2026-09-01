---
status: fixed
opened: 2026-09-01
closed: 2026-09-01
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
unverified: "CLUSTER TAG IS ARGUABLE and flagged rather than hidden: this defect fails LOUDLY, and IC-2 carries an explicit falsification clause for exactly that (\"Falsified by a member whose proxy failure surfaced as an error rather than a plausible result\"). Tagged IC-2 on the exact match to the monotone-stamp proxy shape its claim names; IC-14 was rejected as inverted (this guard was broader than its name, not narrower). A second read should rule: either IC-2 widens to cover fail-closed proxies, or this needs a new class - and filing it honestly is blocked by the absence of a cluster/unclassified slug. ALSO: edit_file on a stamped file still refuses, by a deliberate conservative choice (it cannot bound its own edit extent); that is documented in Fix, not an oversight. The gate's default lane shows 1 failure, peer::server run_exits_after_idle_timeout_with_no_connections, which is an unrelated load-sensitive flake filed separately with three independent reproductions."
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

### SHIPPED 2026-09-01 — scope (2), decided by the operator

The guard now takes an `Access` argument (`Read` | `BodyWrite` | `FrontmatterWrite`) and the
**`stamped`-only** arm refuses `FrontmatterWrite` alone. `augmented` and `ledger` are
unchanged and still refuse every access — narrowing `ledger` is a separate question about who
may advance a `PREFIX-N` counter, and answering both at once would make neither reviewable.

Call sites pass what they actually know:

| call site | access | why |
|---|---|---|
| `read_markdown` | `Read` | a read desynchronises nothing |
| `edit_markdown` | `frontmatter` param present ? `FrontmatterWrite` : `BodyWrite` | the caller's own arguments are the discriminator |
| `edit_file` | `FrontmatterWrite` always | **conservative, not a claim** — it replaces raw text anywhere in the file and cannot bound its own extent, so `BodyWrite` would be asserting a negative it has not established |

So `edit_file` on a stamped file still refuses. That is a deliberate limitation rather than an
oversight: proving an `old_string` did not land in the frontmatter block is work `edit_file` does
not do today, and `edit_markdown` is the sanctioned route for markdown anyway (Iron Law 4).

**The refusal now names what is ALLOWED, not only what is not** — the stamped arm's hint gives
the `artifact(action="update", patch={status, tags})` route for frontmatter *and* states that
reads and body edits work directly. Without that, the narrowing would be invisible to the exact
caller it was made for.

SHA: `0933bc95` (message half) and `c26943b5` (behaviour half), both **`experiments`**.
patch-id: `42c3dcd17a52ed55f082c14de843173f88a2fb2c` (message half),
`f52fb2fc42fc03e58268187c0b1d4dc56b0b94d5` (behaviour half).

### Why direction (a) was rejected

**Stop stamping on create: DEAD**, ruled out by enumeration rather than by preference.
`grep(pattern="frontmatter_id_mismatch|repair_frontmatter_id", glob="src/**/*.rs")` — **60
matches across 5 files**, concentrated in `src/librarian/tools/doctor.rs` (54), plus `mv.rs`
(3), `adapter.rs`, `catalog/artifact.rs`, `tools/librarian.rs`. `doctor`'s
`repair_frontmatter_id` fix mode exists to *write* this field, and
`get_guide("tracker-conventions")` requires a ledger's identity to travel with the repo.
**The stamp is correct; the guard's reading of it was the defect.**

### Why (b) was contained

`references(symbol="is_librarian_artifact", path="src/util/librarian_guard.rs")` — **exactly one
production caller**, `src/util/librarian_guard.rs`'s own `guard_with_oracle`; the other ten hits
are its tests.

### The two arguments that decided the scope

**1. `stamped` guarded an arbitrary subset.** `docs/issues/_TEMPLATE.md` carries **no `id:`**, so
every bug file created the *documented* way is unstamped and was never guarded. Only
tool-created files were. Measured 2026-09-01:

```
git grep -lE "^id: '?.?[0-9a-f]{16}" -- 'docs/trackers/*.md' | wc -l   #  57 of 120
git grep -lE "^id: '?.?[0-9a-f]{16}" -- 'docs/issues/*.md'   | wc -l   # 206 of 552
```

A bug file's frontmatter tags are exactly as drift-prone whether a human or the tool created it,
and the human-created majority was unprotected. Whatever the blanket refusal was for, it was not
coherently achieving it.

**2. The read refusal never prevented a read.** `src/tools/read_file.rs` carries **no guard at
all** — verified by `grep(pattern="librarian_guard|is_librarian_artifact", path="src/tools/read_file.rs")`
returning 0. So a read of a stamped markdown file was always one tool away; the refusal did not
protect anything, it pushed the caller off the heading-addressed tool Iron Law 4 sends them to.
That is what made the read half of scope (2) strictly capability-increasing.

### What `stamped` protects, and what it does not

| arm | what lives elsewhere | read | body write | frontmatter write |
|---|---|---|---|---|
| `augmented` | the params — file is a rendered snapshot | unsafe | unsafe | unsafe |
| `ledger` | the `PREFIX-N` counter | refused | refused | refused |
| `stamped` | nothing — the catalog *indexes* the frontmatter | **safe** | **safe** | unsafe (BL-48) |

## Tests added

Three tests, all **mutation-verified rather than assumed green**.

1. `a_stamped_only_file_refuses_frontmatter_writes_and_permits_reads_and_body_edits` — the
   behavioural pin. A **table** over all three accesses with `expect_refused` as a column,
   because the claim is that the accesses are treated *differently*: a single-access test is
   satisfied by a guard that ignores `access` entirely.
2. `the_stamped_hint_names_the_catalog_route_and_says_body_edits_are_allowed` — pins that the
   refusal states what is now permitted, so the narrowing is not invisible to its caller.
3. `a_stamped_refusal_names_the_stamp_as_its_reason` — the message pin from the first pass.

**Two mutations run, both killed:**

| mutation | effect |
|---|---|
| disable the narrowing (`if false && stamped_only …`) — i.e. restore pre-change behaviour | kills (1) on its `Read` row |
| over-permit (`if stamped_only {` — let a frontmatter write through) | kills **7** tests, including `librarian_guard_fires_on_every_edit_file_write_path` |

The second is the informative one: it proves the BL-48 protection is genuinely load-bearing and
that `edit_file`'s conservative `FrontmatterWrite` is doing work rather than being cautious
decoration.

**Existing tests re-pointed deliberately, not mechanically.** Every "passes" assertion now uses
the **strictest** access (`FrontmatterWrite`), so the pass cannot come from the narrowing;
every "augmented / ledger must refuse" assertion uses `Access::Read`, the access those arms
would lose first if a later change narrowed them by accident. `a_catalogued_but_unaugmented_file_stays_directly_editable`
also gained a fixture annotation: neither of its rows carries an `id:`, and that absence is the
whole discriminator, so a tidy-up "completing" either frontmatter block would leave it passing
and testing nothing.
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

N/A for the reported defect — shipped.

Two follow-ups, neither blocking:

- **`edit_file` on a stamped file still refuses**, by the conservative choice documented in
  *Fix*. If that becomes a real cost, the work is to bound an edit's extent against the
  frontmatter block and pass `BodyWrite` when it provably does not overlap — not to relax the
  default.
- **A ruling on the cluster tag** (see `unverified:`): this defect fails loudly, which IC-2's
  own *Falsified by* clause excludes, yet its mechanism is exactly the "monotone stamp" proxy
  IC-2's claim names. Either IC-2 widens to fail-closed proxies or this needs a new class.
## References

- `src/util/librarian_guard.rs:83-160` — the three predicates and `is_librarian_artifact`
- `src/prompts/guides/tracker-conventions.md` § *Make the tracker guarded* — the ruling
- `docs/issues/archive/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md` — the
  repair that activated this, and the pinned test's rationale
- `docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md`
  — the retracted membership-guarding proposal; this bug is the same axis from the other side
- `docs/trackers/2026-09-01-fable-system-review.md` SR-13 item 4 — where this was first
  noted, as a much weaker claim ("read-refusal is paternalistic") that the probe corrected
