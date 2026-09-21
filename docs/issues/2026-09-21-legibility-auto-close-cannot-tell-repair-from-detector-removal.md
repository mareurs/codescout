---
id: '2d08ac68f42a7c3c'
kind: bug
status: open
title: legibility_scan's auto-close cannot distinguish a repaired defect from a removed detector
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-21
owner: marius
related: []
severity: medium
---

# BUG: legibility_scan's auto-close reports a repair for a removed detector — the close predicate is monotone under detector removal

## Summary

`reconcile` (`src/librarian/tools/legibility_scan/mod.rs:242-312`) closes any prior `open` backlog
row whose key is **absent from the current scan** (`:300`). Absence is the whole predicate. Retiring
a detector makes every one of its candidates absent, so *retire the detector* and *repair every one
of its candidates* produce byte-identical `current` slices — and the rule reports the same success
for both, writing `status: "closed"`, a `closed_at`, and a `before → after` delta.

The tracker's own verdict already records this happening, seven times in one scan
(`docs/trackers/legibility-backlog.md:77`). What makes it a bug rather than a note is that those
seven rows are still in the params collection today and **no field distinguishes them** from the
twelve genuine closes sitting beside them — same `status`, same `defects`, same `closed_at` — while
`render_template.j2` files all nineteen under a heading that reads `### Closed (refactored — before →
after)` with a column headed `defects cleared`.

## Symptom (Effect)

Read 2026-09-21 from artifact `cd886c414f6751b4` via
`doc(action="get", id=…, entry_filter={"status": {"eq": "closed"}})`:

```
entry_total: 42        entries matching status == "closed": 25
  1 row  defects = ["over_budget_body", "name_collision"]
  5 rows defects = ["over_budget_body"]
 19 rows defects = ["name_collision"]        <- all nineteen closed_at == "2026-06-13"
```

Of the nineteen `name_collision` rows, **twelve are genuine** trait-impl relocations — the ten
`src/lsp/client.rs::LspClient/*` rows plus `LspManager/notify_file_changed` and
`LspManager/shutdown_all` — and **seven closed because `919dbe5c` deleted the detector**:

```
src/config/sensitive.rs::SensitiveString/fmt
src/config/sensitive.rs::SensitiveString/from
src/lsp/mux/process.rs::read_proc_memory
src/util/fs.rs::RepoPath/from
src/util/path_security.rs::DEFAULT_DENIED_EXACT
tests/fixtures/nav-eval-rust/src/trait_dispatch.rs::Counter/next
tests/fixtures/typescript-library/src/extensions/advanced.ts::BookMetadata
```

That seven-versus-twelve split is **derived by subtraction from prose**, not read off the rows.
Every discriminator a row carries is identical across the two sets:

| field | genuine twelve | detector-removed seven |
|---|---|---|
| `status` | `closed` | `closed` |
| `defects` | `["name_collision"]` | `["name_collision"]` |
| `closed_at` | `2026-06-13` | `2026-06-13` |
| `after` | a body-token measure | a body-token measure |

`after` is written by `crate::legibility::measure_target` (`:303`), which measures body tokens and
lines — a dimension a *collision* defect never concerned — so it is equally meaningless for both
sets and therefore separates neither.

**Can a reader tell them apart today? No — and the render asserts the wrong one.**
`src/librarian/tools/legibility_scan/render_template.j2` emits:

```
### Closed (refactored — before → after)

| key | defects cleared | before → after | closed |
```

and loops `{% for c in candidates if c.status == "closed" %}`. Both kinds render as **refactored**,
with `name_collision` **cleared**. The only discriminator anywhere in the corpus is the hand-written
verdict at `docs/trackers/legibility-backlog.md:77`, which names the genuine set by *description* —
*"the `LspClient` cluster + the two `LspManager` forwarders"* — and leaves the reader to subtract.

**One reading caveat, not part of this defect.** In this checkout the committed body's `### Closed`
table at `docs/trackers/legibility-backlog.md:68-70` is **empty**, and its header reads *"Scanned
2026-08-28 · 47 open"* while this catalog's `scan_meta` reads `last_scan_at: 2026-06-15`,
`n_candidates: 17`. Body and params disagree; the catalog is machine-local and gitignored
(`docs/conventions/cross-machine-catalog-resume.md`). Recorded so that a reader who opens the
markdown, sees no closed rows, and concludes the rows are gone does not draw the wrong conclusion —
the twenty-five rows are in params. Diagnosing that divergence is out of scope here.

## Reproduction

```
doc(action="get", id="cd886c414f6751b4",
    entry_filter={"status": {"eq": "closed"}})
```

then read `$.entries[*].defects`, `$.entries[*].closed_at`, `$.entries[*].key`. Nineteen rows carry
`["name_collision"]` and one `closed_at`. Cross-read `docs/trackers/legibility-backlog.md:77` for
which seven of them are meaningless. There is no third source.

The mechanism reproduces without the corpus: `reconcile(&prior, &[], …)` — the second call in
`reconcile_opens_then_auto_closes_with_delta` (`:565-595`) — is exactly the detector-removal input,
and it asserts `status == "closed"`.

## Environment

Branch `experiments`, HEAD `46c3aa57770421592a8e36573c4d57a530e3d74f`. Catalog
`~/.local/share/librarian/catalog.db` on this host. The code path is `librarian`-feature-gated, i.e.
compiled by the default lane and **not** by `--no-default-features` (`CLAUDE.md` § *Development
Commands*).

## Root cause

`src/librarian/tools/legibility_scan/mod.rs:249` builds the key set from the current candidate list
alone:

```rust
let current_keys: HashSet<&str> = current.iter().map(|c| c.key.as_str()).collect();
```

and `:300-308` closes on absence from it:

```rust
if row.status == "open" && !current_keys.contains(row.key.as_str()) {
    row.status = "closed".to_string();
    row.closed_at = Some(today.to_string());
    row.after = crate::legibility::measure_target(files, &row.rel_file, &row.name_path)
        .map(|(tokens, lines)| Measure { tokens, budget: crate::tools::MAX_INLINE_TOKENS, lines });
}
```

The condition the rule *means* is **this defect was repaired**. That event is outside `reconcile`'s
observation boundary: it is handed `prior`, `current`, `files` and `today`, and never the detector
roster, the previous roster, or the diff between them. It substitutes the proxy **the key is absent
from the current scan**, and the proxy is satisfied by at least three causes:

1. the code was refactored and the defect is gone — the intended one;
2. the detector that produced the candidate was deleted — measured, `919dbe5c`;
3. the threshold widened so a marginal candidate stopped qualifying — see below.

**The close predicate is monotone under detector removal.** This is `CLAUDE.md` § *Testing
Discipline*'s first law holding about a **production close-rule** rather than a test: a rule cannot
detect a change its predicate is monotone under. Removing a detector can only *shrink* `current`,
and shrinking `current` can only *satisfy* the predicate — so no reading of `current` can ever
separate cause 2 from cause 1. Widening the sample does not help; the refuting observation is not
recorded anywhere.

Measured 2026-09-21: `919dbe5c` (*"feat(legibility): remove name_collision defect (language-agnostic
by subtraction)"*, ADR `docs/adrs/2026-06-13-drop-name-collision-defect.md`) retired the detector.
`src/legibility/mod.rs:15-18` now declares exactly two `Defect` variants — `OverBudgetBody`,
`UnMappableFile` — and `src/legibility/mod.rs:495-514` carries a test literally named
`index_lane_does_not_flag_name_collisions`.

**Cause 3 is inferred, not measured.** The closed row's `after.budget` is written from
`crate::tools::MAX_INLINE_TOKENS` (`:306`) and `over_budget_bodies`
(`src/legibility/mod.rs:129-148`) flags a body against that budget, so raising the constant would
drop every marginal candidate out of `current` and auto-close it as refactored — with a
`before → after` delta that is a genuine measurement of a body nobody touched. Inferred from those
two sites; **not measured**, and deliberately not measured, because deriving it means mutating a
shared-checkout constant.

**Why this class rather than the neighbouring ones.** `IC-24`
(`value-correct-in-a-frame-its-name-does-not-state`) was checked first, because *"closed"* is exactly
a value published under a name that states more than it holds. It is rejected on `IC-24`'s own
discriminator — that the value be *"exactly right and exactly recoverable"*. The value here is not
recoverable: the reason for the absence is precisely what the row does not hold, which is the whole
defect. `IC-16` (`assertion-that-cannot-fail`) was checked and rejected — this is a production
predicate, not an assertion, and it *can* be false (a re-emitted key re-opens the row at `:274-278`).
`IC-2`'s claim — *a gate whose condition is an event outside its observation boundary substitutes a
proxy, and the substitution fails silently, because a proxy returns a plausible answer rather than an
error* — holds clause by clause. This member falls in `IC-2`'s **filesystem/repo-scoped** half: the
substrate that would answer the question (the detector roster) exists in-process and `reconcile` was
simply not given it.

## Evidence

### The close rule, `src/librarian/tools/legibility_scan/mod.rs:298-310`

```rust
for row in rows.iter_mut() {
    if row.status == "open" && !current_keys.contains(row.key.as_str()) {
        row.status = "closed".to_string();
        row.closed_at = Some(today.to_string());
        row.after = crate::legibility::measure_target(files, &row.rel_file, &row.name_path)
```

### Its doc comment, `:237-239` — the substitution stated as fact

```
/// 2. auto-close every prior `open` row whose key is absent from the current scan —
///    its defect is gone — recording `after` (re-measured) and `closed_at`.
```

*"its defect is gone"* is the proxy being read as the claim, in the code that performs the
substitution.

### The tracker's own verdict, `docs/trackers/legibility-backlog.md:77`

> **2026-06-13 — `name_collision` retired as a defect class.** (ADR
> `docs/adrs/2026-06-13-drop-name-collision-defect.md`, commit `919dbe5c`.) The 7 open
> `name_collision` rows that closed on this scan closed because the **detector was removed, not
> because the code was refactored** — their before→after deltas are not meaningful (they render as
> "structural").

### The test pins the behaviour on the failing input

`src/librarian/tools/legibility_scan/mod.rs:565-595`,
`reconcile_opens_then_auto_closes_with_delta`, scan 2:

```rust
let rows2 = reconcile(&prior, &[], &[small_file()], "2026-06-14");
assert_eq!(rows2[0].status, "closed");
assert_eq!(rows2[0].closed_at.as_deref(), Some("2026-06-14"));
```

`current = &[]` is an empty candidate list — *the* signature of a removed detector. So this is not a
coverage gap that a sharper test would catch: the suite specifies the behaviour, and any fix must
change this test, not merely add one.

### The render labels both kinds "refactored"

`src/librarian/tools/legibility_scan/render_template.j2`:

```
### Closed (refactored — before → after)

| key | defects cleared | before → after | closed |
|---|---|---|:--:|
{% for c in candidates if c.status == "closed" -%}
```

## Hypotheses tried

1. **Hypothesis:** this is already filed as a bug. **Test:**
   `doc(action="find", kind="bug", filter={"status": {"in": [open, taken, investigating, fixed,
   mitigated, wontfix, zombie]}})`, plus `grep -rln 'legibility_scan|legibility-backlog|name_collision'
   docs/issues/` and `grep -rln 'auto-close|auto_close|autoclose' docs/issues/`. **Verdict:**
   **rejected** — no bug file. The failure is stated in *prose* in two trackers:
   `docs/trackers/legibility-backlog.md:77` and
   `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md:174`. Neither is a filed record;
   neither is reachable by `doc(action="find", kind="bug", …)`.
2. **Hypothesis:** a reader can separate the seven from the twelve using the row data.
   **Test:** read `status`, `defects`, `closed_at`, `after` for all 25 closed rows via `entry_filter`.
   **Verdict:** **rejected** — identical on every field across both sets (§ *Symptom*).
3. **Hypothesis:** `render_template.j2` marks the difference. **Test:** read the template.
   **Verdict:** **rejected** — it does the opposite, filing both under *"Closed (refactored)"* with a
   *"defects cleared"* column.
4. **Hypothesis:** the defect is only historical, closed with the detector. **Test:** read
   `reconcile` at HEAD. **Verdict:** **rejected** — the predicate at `:300` is unchanged and applies
   to the two surviving detectors. Any future retirement, or a `MAX_INLINE_TOKENS` change, re-runs it.

## Fix

**Not attempted — filing only.** Two directions, neither adopted here:

- **Give `reconcile` the roster.** Pass the set of defect kinds the current scan is capable of
  emitting, and close on absence *only* for rows whose every defect is still producible. A row whose
  detector is gone takes a distinct terminal state — `retired` — with no `after` delta, because there
  is no delta to state. This is the fix that makes the two causes distinguishable rather than merely
  annotated.
- **At minimum, record the reason.** A `closed_reason` field on `CandidateRow`, written by
  `reconcile`, so the render can stop calling every close a refactor. Weaker: it still has to *infer*
  the reason, so without the roster it can only record "absent from scan".

Both are schema changes to `CandidateRow` plus a params migration, and both require editing
`reconcile_opens_then_auto_closes_with_delta`, whose current assertion is the defect.

Retagging the seven existing rows is a separate and smaller repair. If it is taken, the surface is
`doc(action="update_entry", id="cd886c414f6751b4", entry_collection="candidates", entry_id=…,
fields={…})` **per row** — never a `params` array rewrite, which replaces the collection wholesale
(`CLAUDE.md` § *Session Intelligence Trackers*, the 2026-08-16 19→1 loss).

## Tests added

N/A — no code change in this commit.

Naming the guard the fix would owe, because the obvious one is vacuous: asserting that a closed row
*has* a `closed_reason` is monotone under the field being defaulted, and asserting that the backlog
contains *no* `retired` rows is monotone under the whole branch being deleted. The discriminating
case is a two-scan fixture where scan 2 drops a **defect kind** rather than a candidate, and the
assertion is that the row's terminal state differs from the one produced by scan 2 dropping the
candidate with the kind still live — i.e. a test that the two causes are *distinguishable*, which is
the claim, rather than that either is labelled.

## Workarounds

Read `docs/trackers/legibility-backlog.md` § *Verdicts* before trusting any `closed` row dated
`2026-06-13`. There is no programmatic workaround: the information is not in the data.

## References

- `src/librarian/tools/legibility_scan/mod.rs:237-239`, `:242-312` (`:249`, `:274-278`, `:300-308`),
  `:565-595`
- `src/librarian/tools/legibility_scan/render_template.j2`
- `src/legibility/mod.rs:15-18`, `:129-148`, `:495-514`
- `docs/trackers/legibility-backlog.md:68-70`, `:77` (artifact `cd886c414f6751b4`)
- `docs/adrs/2026-06-13-drop-name-collision-defect.md`, commit `919dbe5c`
- `docs/trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md:174` — the prose statement this
  file converts into a record
- Class: `docs/trackers/issue-clusters/IC-2-gate-keyed-on-unobservable-event.md`
