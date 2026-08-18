---
id: '52269554ea4f51a4'
kind: bug
status: fixed
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
closed: 2026-08-18
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

**SHIPPED `ff088630` (`experiments`)** — option 1, gate on the declaration rather than on the
definitions. `entry_prefix` declarations now feed `DefinitionIndex.known_prefixes` alongside
prefixes learned from definitions, so a declared namespace is always reportable while an
undeclared `CI-2` in prose stays suppressed.

**The wiring choice is the load-bearing part, and it is not the one the filed fix implied.**
Rather than threading declarations through `DefinitionIndex::build`'s arguments, the
declaration rides on `DocExtract.declared_prefixes`, populated inside `extract()`. `extract`
already receives the whole file text, frontmatter included — so the wire cannot be forgotten.
Threading it through `build` would have made a forgotten wire a silent no-op and touched 11
test call sites; this way there is nothing for a caller to remember.

Read with `librarian_guard::declared_entry_prefixes`, the guard's own parser, rather than a
second one: it compiles under `--no-default-features` where `librarian::frontmatter` does not
exist (verified: `cargo check --no-default-features` exit 0), and the two readers' agreement on
every YAML form is already pinned by `both_entry_prefix_readers_agree_on_every_yaml_form`.

Declarations count **regardless of `active`** — an archived ledger's namespace is still a
namespace, and archived definers already resolve.

`edges_added` is unchanged, as required. Option 2 (`suppressed_prefixes` bucket) was not
needed.

**Known limit, and it is inherent rather than an oversight.** The gate still cannot separate
prose that merely looks like a token from real breakage once a prefix is known — measured
instance: `bug-fix-session-log.md:467` says "a parallel session's T-13 commit", meaning an old
plan's task numbering, and `T` is a known prefix, so it is reported as dangling. Widening the
gate widens that. The filed fix said as much ("improves coverage without completing it"), and
it is the right trade: a false positive costs a reader one glance, a false negative cost this
project 129 silently-dead citations.
## Tests added

Five, across `extract.rs` and `resolve.rs`, written and watched fail first — 3 failed on their
assertions, and the 2 negative controls passed pre-fix, which is exactly right: they assert
behaviour that must NOT change.

| Test | What it kills |
|---|---|
| `frontmatter_entry_prefix_is_read_as_a_declaration` | `extract` not populating the field |
| `frontmatter_declares_nothing_without_an_entry_prefix_key` | over-reading — a `tags: [A-11, F-3]` list must not declare |
| `a_declared_prefix_dangles_even_though_nothing_defines_it` | `build` not reading the field |
| `declaring_one_prefix_does_not_un_suppress_the_others` | widening the gate globally instead of per-prefix |
| `a_row_only_declared_ledger_is_reportable_end_to_end` | **either half silently dropping the wire** |

The last one is the only test of the five that a unit test on either half cannot replace: it
runs a real row-only body through the real `extract()` into the real `DefinitionIndex`, and
asserts as a precondition that `definitions` is empty, so the declaration is provably the only
thing making the citation reportable.

Gate: `cargo fmt` 0, `cargo clippy --all-targets -- -D warnings` 0, **4142 passed / 0 failed**,
`cargo check --no-default-features` 0. `dangling_is_prefix_gated` — the test that pinned the old
behaviour — still passes, because suppression of undeclared prefixes is unchanged.
## Workarounds

Do not read `counts.dangling` as a measure of citation health for a namespace you have not confirmed
has at least one definition. Use `librarian(action="doctor")` — `ledger_defines_nothing` and
`entry_without_definition` — and, when repairing, `link_scan`'s `edges_added` on a `write=true` run,
which enumerates exactly which files recovered a link. That pairing is what the four backfills used.

## Resume

**Code is done and gated; the predicted measurement is NOT yet taken.** This bug file claims
the project dangling total will RISE, and that claim is currently unverified — `link_scan` runs
in the MCP server, there is no CLI subcommand for it, and the running server must be
reconnected (`/mcp`) after `cargo rb` before the new gate is on the wire.

Pre-fix baseline, measured on the old binary immediately before the rebuild so the comparison
is clean:

| metric | pre-BL-41 |
|---|---|
| `dangling` | **548** |
| `ambiguous` | 410 |
| `citations` | 3,649 |
| `edges_added` | 0 |
| `artifacts_scanned` | 1,055 |

Note 548, not the 547 recorded at the previous session close; a peer session shares this
machine-local catalog and one row moved between measurements. Not attributed — recorded as
measured.

**Next action:** `/mcp`, then `librarian(action="link_scan")`, and compare. Two effects are
confounded in that single number and should be reported separately:

- BL-41 makes previously-suppressed namespaces reportable — pushes dangling **up**;
- `c7bdfd22` gave 49 entries defining headings across four ledgers — pushes it **down**.

So a flat total would be two real effects cancelling, not a no-op. `edges_added` is the clean
read on the backfill half.

Do not archive this file until that measurement is recorded.

Fix SHA: `ff088630`, **`experiments`** — fast-forward path, so it is already the master-side SHA.
## References

- `src/librarian/tools/link_scan/resolve.rs` — `DefinitionIndex::build`, `known_prefixes`,
  `prefix_is_known`, and the `dangling_is_prefix_gated` test that states the intent.
- `src/util/librarian_guard.rs:181` — `declared_entry_prefixes`, the signal a fix should consult.
- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md`
  § Evidence *CORRECTION* — the four-ledger measurement table, and where this was first mistaken for
  a dangling population.
