---
id: 801e5b4c13198406
kind: bug
status: fixed
title: doctor's justification comment names link_scan as entry_cite's writer; link_scan never touches it
tags:
- doctor
- entry_cite
- doc-vs-code-drift
- link_scan
closed: 2026-08-21
---

# BUG: `doctor`'s justification comment names `link_scan` as `entry_cite`'s writer; `link_scan` never touches it

## Summary

`src/librarian/tools/doctor.rs:1631` justifies a live design decision — recomputing entry
citations from files rather than reading the `entry_cite` table — with a false premise about
which subsystem materializes that table. The decision it defends is correct; the reason it
gives is not. A later reader who trusts the comment will hold a wrong model of the entry
graph's write path, which is exactly the model needed to reason about backfilling it.

## Symptom (Effect)

`src/librarian/tools/doctor.rs:1629-1637`, verbatim:

```
/// Every entry token cited anywhere in the catalog, computed fresh from the files.
///
/// **Deliberately not `entry_cite`.** That table is materialized only by
/// `link_scan(write=true)`, so a check reading it would report against whatever the last
/// scan happened to leave behind — a stale-substrate diagnostic of exactly the kind
/// `doctor` exists to catch.
```

`link_scan` does not write `entry_cite`, and never has.

## Reproduction

```
grep(pattern="entry_cite|EntryCiteRow", glob="src/librarian/tools/link_scan/**", mode="content")
  → 0 matches

grep(pattern="entry_cite", glob="src/**/*.rs", mode="files")
  → 9 files: get.rs, gc.rs, catalog/mod.rs, augmentation.rs, append_entry.rs,
             migrate_v6.rs, entry_cite.rs, artifact.rs, doctor.rs
    (link_scan absent)
```

Measured 2026-08-20 on `experiments` @ `85e9b2da`.

## Environment

codescout `experiments`, branch tip `85e9b2da`, Linux. No runtime component — this is a
source-comment claim checkable by grep.

## Root cause

The Stage-2 entry-graph design (`docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md`
§ Scope) states the opposite of the comment and matches the code: *"a dedicated `entry_cite`
table for entry-grain edges; `link_scan` is **unchanged** (table separation keeps write-time
edges out of its `artifact_link` prune pass)."*

The sole writer is `append_entry` with a `cites` argument — pinned by
`append_entry.rs:1090` `append_with_cites_writes_entry_cite_and_not_artifact_link`, which
asserts one `entry_cite` row and **zero** `artifact_link` rows.

So the comment appears to describe the design as it stood in an earlier draft (where the
scanner owned the table), and was not updated when table separation was adopted. Inferred
from the spec's revision note and the test name — not measured against git history.

The conclusion the comment reaches is still right, for a different and stronger reason:
`entry_cite` is written **only** on the `append_entry(cites=…)` path, so it is not stale so
much as near-empty, and a `doctor` check reading it would under-report rather than
mis-report.

**2026-08-21 correction — this root cause is now stale too, and the fix it prescribes would
have been wrong.** Re-grepping `entry_cite|EntryCiteRow` in `src/librarian/tools/link_scan/**`
now returns 13 matches, not 0: `link_scan(write=true)` gained a real write path since this bug
was filed (`b750419a`, `1b19e0db`, `383b394e` — same-day concurrent work, `entry_cite::insert_with`
with `origin=ORIGIN_SCAN`, pruned and re-derived per scan pass via `entry_cite::prune_scan_rows`).
So the comment's premise ("materialized only by `link_scan(write=true)`") is no longer false in
the direction this bug describes — link_scan really does write it now. It's inaccurate in a
different way: it omits `append_entry(cites=…)` as a second, permanent writer (`origin="write"`,
which takes precedence over a later scan because the PK excludes `origin` — see
`entry_cite.rs:insert_with`'s doc comment). Following this bug's original `## Resume` instruction
("name `append_entry` as the writer and under-reporting, not staleness, as the hazard") would
have reintroduced a wrong claim, just inverted. This is a live instance of CLAUDE.md — Bug
Tracking's "run the reproduction before reading the fix plan" rule: the plan is a hypothesis
about the reproduction, and here the reproduction had moved out from under the plan between
filing and fix.

## Evidence

Live catalog, `/home/marius/.local/share/librarian/catalog.db`, measured 2026-08-20:

```
sqlite3 catalog.db "SELECT (SELECT COUNT(*) FROM entry_cite),
                           (SELECT COUNT(*) FROM artifact_link),
                           (SELECT COUNT(*) FROM artifact WHERE slug IS NOT NULL),
                           (SELECT COUNT(*) FROM artifact);"
→ 13|2789|2|4087
```

All 13 `entry_cite` rows carry `origin='write'` — consistent with `append_entry` being the
only writer, and with `link_scan` never having contributed a row. The two slugged artifacts
are `tool-usage-patterns` and `open-issue-work-queue-bl-n`.

## Hypotheses tried

1. **Hypothesis:** `link_scan` writes `entry_cite` through a helper in another module, so the
   comment is right and the grep is scoped wrong.
   **Test:** grep `entry_cite|EntryCiteRow` across `src/**/*.rs` (files mode) — the file list
   is the whole population, not a sample.
   **Verdict:** rejected. `link_scan` is absent from the 9 files; every `origin` value in the
   live table is `'write'`, which `append_entry` sets.

## Fix

One-line comment correction at `src/librarian/tools/doctor.rs:1631`. Replace the
`link_scan(write=true)` premise with the true one: the table is written only by
`append_entry(cites=…)`, so it is not a complete record of citations in prose and a check
reading it would under-report.

Not yet fixed — filed on notice during a brainstorm, per CLAUDE.md § Bug Tracking.

**Fixed 2026-08-21.** Corrected the comment at `corpus_cited_tokens` (moved to
`doctor.rs:1747-1758` by intervening commits) to describe both writers —
`append_entry(cites=…)`'s permanent `origin="write"` rows and `link_scan(write=true)`'s
pruned-and-re-derived `origin="scan"` rows — and to attribute the staleness risk to the
scan-origin half specifically, not to the table as a whole. Gate green: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test` (4421 passed, 46 ignored, 0 failed).

**experiments** SHA: `712a9f4da1a66e920dfc73cbd733d2eb577dab4d`
patch-id: `93e05d03cc7d24a42380649459d049f963f1e852`

## Tests added

None yet. A comment fix takes no regression test; the durable guard, if wanted, is a doc-ref
style assertion that no comment names a module that does not reference the symbol it claims
to write. That is out of proportion to this defect — noting the option rather than
recommending it.

## Workarounds

Read `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md` § Scope and
§ Design decisions (item 2) for the authoritative account of why `entry_cite` is a separate,
non-scanner-owned table.

## Resume

Edit the doc comment at `src/librarian/tools/doctor.rs:1629-1637` to name `append_entry`
as the writer and under-reporting (not staleness) as the hazard. Keep the surrounding
decision unchanged — recomputing from files via `link_scan::extract` remains correct.

## References

- `src/librarian/tools/doctor.rs:1629-1637` — the comment
- `src/librarian/tools/append_entry.rs:1090` — `append_with_cites_writes_entry_cite_and_not_artifact_link`
- `src/librarian/catalog/entry_cite.rs` — the table module
- `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md` — Stage-2 design
