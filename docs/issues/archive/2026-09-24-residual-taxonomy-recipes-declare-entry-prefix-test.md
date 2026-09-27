---
id: 96b2b1b9a25bb1b0
kind: bug
status: fixed
title: 'RESIDUAL: Write the doc-to-code test asserting every tracker with an append_entry recipe in docs/TAXONOMY.md declares entry_prefix'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-27
fix_patch_id: dd6ac4fcdb13166039a54d45548bb2009f776875
fix_scanner_patch_id: 65af3b9768151c0da15f49ba1ea7ccb6f38c16fc
fix_scanner_sha: 95ff5e64107ae4de47a0f9ea1be18bce1ca7ae04
fix_sha: 8fd92e0a22ca733c8329c8cd28a163d6f69ab80c
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Write the doc-to-code test asserting every tracker with an append_entry recipe in docs/TAXONOMY.md declares entry_prefix.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` (status `fixed`):

> The PRECONDITION is verified at the bytes; the ALLOCATION is not. Both files now declare entry_prefix (T / I) and a matching entry_high_water, read back from disk. But no append_entry was run against either, because a successful call allocates a real id and writes a real entry, and that is a content decision rather than a probe. So the claim 'append_entry now works here' rests on allocate_entry_id's frontmatter check being the only thing that was failing — read at augmentation.rs:971-983, not observed. The next person to append verifies it for free; if it still refuses, the cause is downstream of the declaration and this record is reopened rather than re-derived. No regression test either: the durable form is a doc-to-code join asserting that every tracker with an append_entry recipe in docs/TAXONOMY.md declares an entry_prefix, which is IC-11's mechanizable sub-shape and is not written.

## Fix

Shipped as `#[cfg(test)] mod taxonomy_recipes` in `src/librarian/tools/append_entry.rs`, test `every_taxonomy_append_entry_recipe_is_one_the_code_accepts`, per the operator-approved design `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` and plan `docs/superpowers/plans/2026-09-27-taxonomy-append-recipes-test.md`.

**It asserts the CODE contract, not this file's literal wording.** The residual asked that every recipe target declare `entry_prefix`; at HEAD only the prose branch of `append_entry` reads that declaration (`allocate_entry_id`), while the params branch checks the augmentation instead — so the literal test would have redded WIN-N and PV-N, which work (`bug-fix-session-log:F-176`). Per recipe shape: prose ⇒ the recipe's `id_prefix` is in the target's declared set; params ⇒ the committed sidecar declares the named `entry_collection`; the F-N template ⇒ every `docs/trackers/*-session-log.md` declares `[F, W]`, except a shrink-only `TEMPLATE_EXEMPT` list of the five logs that declared nothing on 2026-09-27. All shapes also pass `is_citable_entry_prefix`. Joined on each row's *Lives in* cell, never its recipe `id` (machine-local `sha256` of an absolute path).

- `95ff5e64` — the scanner; patch-id `65af3b9768151c0da15f49ba1ea7ccb6f38c16fc`
- `8fd92e0a` — the checks and the corpus test; patch-id `dd6ac4fcdb13166039a54d45548bb2009f776875`

Follow-ups filed: `fc491a58e7a9b561` (the other surfaces that route to `append_entry` are unchecked), `4d25c5b252c36a70` (the session-log template's own recipe is refused on a fresh copy), `d1eff909c0d8a73a` (`fmt-mine.sh` reports formatted after a rustfmt pass that is not a fixed point — met while landing this).

## Tests added

13 tests in `taxonomy_recipes`: 9 scanner unit tests (call-span walker, fenced and foreign rows, the unescaped-pipe F row, the Lives-in disambiguator, a renamed section, CRLF), 3 check unit tests on temp dirs, and the corpus test.

**One observed red per guarded site**, each via `./scripts/mutation-probe.sh` in an isolated worktree, 2026-09-27. Each row counts only when KILLED **and** the failure names the expected guard; steps 1–9 each produced exactly one finding.

| # | Mutation | Verdict | Finding |
|---|---|---|---|
| 1 | drop `entry_prefix: R` (reconnaissance-patterns) | KILLED | R-N: does not declare an entry_prefix |
| 2 | `entry_prefix: OP` → `OQ` (operator-rules) | KILLED | `OP` is not declared by this ledger (it declares OQ) |
| 3 | WIN sidecar `issues` → `issuez` | KILLED | declares `Some("issuez")` |
| 4 | drop `expects_augmentation` (provenance-subsystem) | KILLED | PV-N: declares no `expects_augmentation` sidecar |
| 5 | response-envelope log `[F, W]` → `[F]` | KILLED | declares F — owns both F and W |
| 6 | declare `[F, W]` in exempt pr-review log | KILLED (re-run; first form refused as not-applied) | now declares F, W — delete its exemption |
| 7 | delete pr-review from `TEMPLATE_EXEMPT` | KILLED | declares no entry_prefix + two-addressee remedy |
| 8 | `id_prefix="OB"` → `idprefix=` in TAXONOMY | KILLED | OB-N: passes no id_prefix (unparseable) |
| 9 | `id_prefix="WIN"` → `"WINX"` in TAXONOMY | KILLED | refused before either branch; no params finding |
| 10 | `declared_prefixes_from_frontmatter` returns empty | KILLED | 23 findings = 8 prose + 15 declared logs |
| 11 | walker's `entry_collection` needle broken | KILLED | a recipe shape is missing |

Step 6's planned replacement contained its own find literal, so the probe correctly refused it as not applied (rc 2, nothing run); it was re-run with an equivalent input.

### Final review and fix pass — 2026-09-27

The Opus whole-branch review returned *with fixes*: 0 Critical, 5 Important, and 6 surviving mutations confirmed against a no-op baseline, one of them a real false pass (the template check hard-coded `F` instead of reading the recipe's own prefix). All closed in `59204281` (patch-id `419348f523e3becb41f5a993d6f53d8672f811a1`); the module now holds 21 tests. The fix-pass mutation round re-probed every bound, as CLAUDE.md requires after changing any: the no-op baseline SURVIVED and 24/24 mutations were KILLED with their named markers — the reviewer's survivors, one per new guard, and the original eleven above. Gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## References

- `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` — parent
