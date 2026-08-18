---
id: '52269554ea4f51a4'
kind: bug
status: open
title: 'BUG: link_scan''s dangling count is prefix-gated, so a namespace with zero definitions reports as healthy while every citation of it resolves to nothing'
owners:
- marius
tags:
- librarian
- link-scan
- observability
- entry-identity
- false-negative
topic: tracker-entry-identity
closed: ''
opened: 2026-08-18
related:
- docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
severity: medium
---

# BUG: a namespace with zero definitions reports as healthy, because dangling is prefix-gated

## Summary

`librarian(action="link_scan")` suppresses the `dangling` verdict for any `PREFIX` that has **no
definition anywhere in the corpus**. The gate is deliberate and correct in intent — without it every
`CI-2` or `PR-5` in prose would be reported as a broken citation. Its consequence is not: a ledger
that defines **none** of its entries has all of its broken citations silently reclassified as
ordinary prose, so `link_scan` reports the namespace as clean while every reference to it resolves
to nothing. The failure scales with severity — the *worse* the ledger, the quieter the report.

## Symptom (Effect)

Measured 2026-08-18 across four ledger backfills. Same operation each time (add defining headings),
opposite effect on the number, decided purely by whether the prefix already had one definition:

| ledger | prefix defined beforehand? | `dangling` before → after | `edges_added` |
|---|---|---|---|
| `windows-platform-support.md` | no | 621 → **621** | 22 |
| `open-issue-work-queue.md` | no | 621 → **622** | 33 |
| `provenance-subsystem.md` | **yes** (PV-2/4/5/7) | 622 → **574** | 4 |
| `prompt-hamsa-audit-log.md` | **yes** (A-1…A-14) | 574 → **547** | 8 |

The first row is the bug in one line: **129 citations of `WIN-N` from 27 files resolved to nothing,
and the total did not move by one.** The second row is the same mechanism inverted — defining `BL-N`
*raised* the count, because opening the gate exposed one previously-suppressed broken citation.

## Reproduction

At `bf60b5f3` on `experiments`, with a prefix that no artifact defines:

1. Cite `ZZ-7` from any tracker body.
2. `librarian(action="link_scan")` → the citation appears in **no** bucket: not `dangling`, not
   `ambiguous`, not `cross_repo`. `counts.citations` includes it; nothing reports it as broken.
3. Add `## ZZ-1 — anything` to any artifact, so prefix `ZZ` gains one definition.
4. Re-run → `ZZ-7` now appears under `dangling`.

The citation's brokenness never changed. Only whether it was reportable did.

## Environment

codescout `experiments` @ `bf60b5f3`, 2026-08-18. Pure string/regex logic in the resolver — not
platform- or state-sensitive.

## Root cause

`src/librarian/tools/link_scan/resolve.rs`:

- `DefinitionIndex.known_prefixes` is populated **only** from `def.token`'s prefix while walking
  definitions (`DefinitionIndex::build`), so a prefix with zero definitions is absent from it.
- `prefix_is_known()` tests membership, and the `Dangling` outcome is gated on it — pinned by the
  module's own `dangling_is_prefix_gated` test, whose comment states the intent: *"U-99: prefix U is
  defined somewhere → report dangling."*

So the gate's discriminator is *"does anything define this prefix"*, which is a reasonable proxy for
*"is this an id namespace at all"* — and is exactly wrong for the one case where a real namespace
has been created and never given a single definition. The proxy fails on the population it most needs
to catch.

Mechanism-language: brokenness is judged per-token, but reportability is judged per-prefix, and the
per-prefix judgement is derived from the very thing that is missing.

## Evidence

### The gate cannot distinguish "not a namespace" from "a namespace that is wholly broken"

Both look identical to `known_prefixes`: zero definitions. `WIN` was a registered prefix with 35
entries in the catalog, its own tracker, and 129 citations — and was indistinguishable from a stray
`AB-3` in prose.

### The catalog already holds the missing signal

`entry_prefix` in artifact frontmatter is a *declaration* that a namespace exists, read by
`declared_entry_prefixes` (`src/util/librarian_guard.rs:181`) and by the id allocator. `link_scan`
does not consult it. A declared prefix with zero definitions is a namespace that is entirely
unreachable, which is the strongest possible finding, and the resolver has the fact available.

### Only `doctor` ever saw it

`ledger_defines_nothing`
(`docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`,
shipped `758b37dc`) is the only surface that reported these ledgers. It works by comparing params ids
against body definitions and never consults the prefix gate. That check found 10 such ledgers on
first run.

## Hypotheses tried

1. **Hypothesis:** the gate is simply wrong and should be removed.
   **Test:** reasoned against the corpus — prose is full of uppercase-hyphen-number tokens
   (`CI-2`, `PR-10`, `WIN-`-shaped release names, `os error 5`-adjacent forms).
   **Verdict:** rejected. Removing it trades a quiet false negative for a loud false-positive flood,
   which is how advisories get ignored (the U-41 / U-44 pattern). The gate needs a better
   discriminator, not deletion.

2. **Hypothesis:** `doctor`'s `ledger_defines_nothing` already covers it, so this is not worth
   fixing.
   **Verdict:** partly true and still worth fixing. `doctor` catches the *ledger*; `link_scan`'s
   report is what a reader consults to ask "are my citations sound?", and it answers that question
   wrongly with a specific number. A report that says `dangling: 621` while 129 references are dead
   is not silent — it is confidently incorrect, which is worse.

## Fix

Not implemented. Two candidates; the first is preferred.

1. **Gate on the declaration, not on the definitions.** Add `entry_prefix` frontmatter declarations
   to `DefinitionIndex.known_prefixes` alongside prefixes learned from definitions. A declared
   namespace is then always reportable, so a wholly-undefined ledger's citations dangle loudly, while
   an undeclared `CI-2` in prose stays suppressed. `declared_entry_prefixes` already parses every
   YAML form and is already called elsewhere, so this is a wiring change rather than new logic.
   Caveat to check: not every ledger declares `entry_prefix` yet, so this improves coverage without
   completing it — which is honest, and pairs with the guide's push to declare.
2. **Report suppressed prefixes as their own bucket.** Emit `suppressed_prefixes: {"WIN": 129}` so
   the number is visible without being asserted as breakage. Weaker — it puts the burden on a reader
   noticing a second array — but it needs no declaration and cannot regress.

Whichever ships, **do not change what `edges_added` measures.** It was the only working progress
metric for these four backfills and its behaviour is now documented.

## Tests added

None yet. What the fix must pin:

- `a_declared_prefix_with_no_definitions_still_dangles` — the whole bug: an artifact declaring
  `entry_prefix: ZZ` and defining nothing, cited as `ZZ-7`, must report dangling.
- `an_undeclared_prefix_with_no_definitions_stays_suppressed` — the flood guard, and the reason
  `dangling_is_prefix_gated` exists. Both must be green simultaneously, which is what proves the
  discriminator changed rather than loosened.
- `dangling_is_prefix_gated` must keep passing unchanged — it pins the behaviour for prefixes that
  *are* defined somewhere.

## Workarounds

Do not read `counts.dangling` as a measure of citation health for a namespace you have not confirmed
has at least one definition. Use `librarian(action="doctor")` — `ledger_defines_nothing` and
`entry_without_definition` — and, when repairing, `link_scan`'s `edges_added` on a `write=true` run,
which enumerates exactly which files recovered a link. That pairing is what the four backfills used.

## Resume

Wire `declared_entry_prefixes` into `DefinitionIndex::build` so a declared `entry_prefix` marks the
prefix known even with zero definitions, then write `a_declared_prefix_with_no_definitions_still_dangles`
and `an_undeclared_prefix_with_no_definitions_stays_suppressed` and confirm `dangling_is_prefix_gated`
still passes. Expect the project dangling total to **rise** when this lands — that is the fix
working, not a regression, and it will surface the remaining `ledger_defines_nothing` ledgers' dead
citations for the first time.

## References

- `src/librarian/tools/link_scan/resolve.rs` — `DefinitionIndex::build`, `known_prefixes`,
  `prefix_is_known`, and the `dangling_is_prefix_gated` test that states the intent.
- `src/util/librarian_guard.rs:181` — `declared_entry_prefixes`, the signal a fix should consult.
- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`
  § Evidence *CORRECTION* — the four-ledger measurement table, and where this was first mistaken for
  a dangling population.

