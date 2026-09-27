---
id: '66842613e8620a86'
kind: spec
status: draft
title: TAXONOMY append_entry recipes — doc-to-code test — design
tags:
- tests
- taxonomy
- append-entry
- doc-to-code
- ic-11
topic: taxonomy-recipe-gate
---

# TAXONOMY `append_entry` recipes — doc-to-code test — design

**Valid:** dated 2026-09-27

**Status:** design approved section by section with the operator in session `b4de6398-fed1-4c1d-a359-2b9a42554e10`, 2026-09-27. Nothing below is implemented.

## Goal

Close residual `96b2b1b9a25bb1b0` (`docs/issues/archive/2026-09-24-residual-taxonomy-recipes-declare-entry-prefix-test.md`): a writer who copies an `append_entry` recipe from the *Main taxonomy* table of `docs/TAXONOMY.md` must not be refused.

Why that matters: the parent (`docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md`) records what a refused writer reaches for next — `doc(action="augment", merge=true, …)`, which replaces the whole collection and once took the T-N queue from 19 entries to 1.

The party who cannot see the drift is whoever edits a tracker's frontmatter or sidecar, or a TAXONOMY row: they never open the other surface, and nothing in the compiler relates the two. The gate runs whether or not anyone suspects a problem.

## Decisions (operator-approved)

1. **Assert the code contract, not the residual's literal wording.** The residual asks that every recipe target declare `entry_prefix`; the code requires that only on the prose path. See `bug-fix-session-log:F-176`.
2. **The F-N template recipe uses a named, shrink-only exemption list** for session logs that declare no prefix.
3. **Scope is the *Main taxonomy* table of `docs/TAXONOMY.md` only.** The other surfaces that route writers to `append_entry` get a follow-up issue.
4. **Approach A:** an in-crate `#[cfg(test)]` test in `src/librarian/tools/append_entry.rs`, calling production readers directly. Rejected: an integration test in `tests/doc_tool_refs.rs` (widens `pub(crate)` readers to `pub` and reworks a shared per-line parser), and a `doctor` check (runs only when someone thinks to run it).

## Facts this design rests on

Measured 2026-09-26/27 against `experiments` HEAD; re-verify before implementing.

- **Prose path:** `append_entry`'s `call()` reaches `allocate_entry_id` only when `entry_collection` is absent. `allocate_entry_id` is the sole production caller of `declared_prefixes_from_frontmatter` and refuses when the declared set is empty **or** lacks the recipe's `id_prefix`.
- **Params path:** `augmentation::append_entry` checks no declaration. It refuses when no augmentation row exists (`append_entry_rejects_missing_augmentation`) or the named collection is unknown (`append_entry_rejects_unknown_entry_collection`).
- **Above both branches:** `is_citable_entry_prefix` refuses a prefix that is not one to three uppercase letters.
- **Recipe ids are not portable.** Every `id="…"` in the table is `sha256` of this machine's absolute path (checked for 5 of 11 by hashing), so a CI-run join keyed on them resolves nothing — `docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md`. The join key is the row's *Lives in* path.
- **Population at design time:** 17 table rows; 13 are recipes — 8 prose (R, U, SKF, H, OB, HY, OP, IC), 4 params (T, WIN, PV, DC), 1 template (F). A-N, CAP-N and BUG carry no recipe; W-N says "Same". Every concrete recipe satisfies the code contract today.
- **The F row holds unescaped `|` inside a code span** (`index_row="| {id} | … |"`), so splitting cells on pipes misreads it.
- **Session logs:** 20 match `docs/trackers/*-session-log.md`; 15 declare exactly `[F, W]`; 5 declare nothing. The template (`docs/templates/session-log.md`) ships without frontmatter and tells writers to declare `entry_prefix` before appending.

## §1 Population and parsing

- **Input:** `docs/TAXONOMY.md` via `env!("CARGO_MANIFEST_DIR")`.
- **Scope:** lines under `## Main taxonomy` up to the next `##` heading, outside fenced blocks, tracked with `crate::util::markdown_fence::FenceState`.
- **Row:** a line starting with `| **`; its label is the bold text. **Cells are never split on `|`.**
- **Recipe:** a row containing `doc(action="append_entry"`. A small walker reads the call span, tracking double-quoted strings and `{[(` depth to the matching `)`, and extracts `id_prefix="…"` and optional `entry_collection="…"`.
- **Shape:** `entry_collection` present → params; target path contains `<topic>` → template; otherwise → prose.
- **Target:** the first backticked `docs/trackers/…md` token in the row's *Lives in* cell (the second cell, which never holds a pipe) — never elsewhere in the row, so a tracker named in the Captures column cannot become the target; the template shape expands to every `docs/trackers/*-session-log.md`.
- **W row:** carries no call. It is covered by the template check's `[F, W]` requirement, and the test's doc comment says so.
- **Unparseable is loud.** A row containing the call opener whose span never closes, lacks `id_prefix`, lacks a target path, or holds two calls with different prefixes is recorded with its line number and fails.
- **Non-vacuity:** at least one recipe of each shape must be found, or the test fails saying the parser lost a shape. No exact count is asserted.

## §2 Checks and failure messages

- **All shapes:** `is_citable_entry_prefix(id_prefix)` must accept the recipe's prefix.
- **Prose:** the target file exists, and `frontmatter::parse` → `declared_prefixes_from_frontmatter` contains the recipe's `id_prefix`. The failure names file, prefix and declared set, quotes `allocate_entry_id`'s refusal wording, and offers both repairs — declare the prefix, or correct the TAXONOMY row — because the test cannot know which side is wrong.
- **Params:** `expects_augmentation` → `parse_declaration` must be `Declared { sidecar: Some(rel) }`, and `augmentation_sidecar::read(rel)`'s `entry_collection` must equal the recipe's. `Absent`, bare `true` and `Unparseable` fail: a fresh clone re-attaches nothing, so the recipe is refused. Remedy: `librarian(action="doctor", fix="export_augmentations")`. **`entry_prefix` is deliberately not asserted here**, and the doc comment says why.
- **Template:** each `docs/trackers/*-session-log.md` not on the exemption list declares a superset of `[F, W]`. Each listed log must still declare nothing; a listed log that now declares fails with "remove it from the list", which keeps the list shrink-only. A new undeclared log fails with a two-addressee remedy: if it is yours, declare `entry_prefix: [F, W]` as the template instructs; if it is not, it is likely a peer's in-progress log — attribute it with `scripts/file-provenance.py` and ask them, do not declare it for them.
- **Reporting:** failures are collected and reported together, one line per recipe, plus a population line (rows examined, recipes by shape, non-recipe rows).

**Exemption list at design time:** `local-onnx-embedding-session-log.md`, `pr-review-session-log.md`, `release-promotion-session-log.md`, `structural-edit-gate-session-log.md`, `worktree-semantic-search-session-log.md` — each with the same reason: it declared no prefix at design time, so the template's declare step was never taken there. Why each skipped it is not recorded and not needed — the list only has to shrink.

## §3 Verification

- **Walker unit tests:** a span closes over `entry={…}`; a `)` inside a quoted string does not close it; an unclosed span is unparseable; two calls with different prefixes are ambiguous.
- **One observed red per guarded site,** each via `./scripts/mutation-probe.sh` in an isolated worktree, and each chosen so every *other* guard admits the input:

| # | Mutation | Expected guard |
|---|---|---|
| 1 | drop `entry_prefix: R` from `docs/trackers/reconnaissance-patterns.md` | prose, nothing declared |
| 2 | `entry_prefix: OP` → `OQ` in `docs/trackers/operator-rules.md` | prose, declared but not the recipe's prefix |
| 3 | WIN sidecar `entry_collection: issues` → `issuez` | params, collection mismatch |
| 4 | remove `expects_augmentation:` from `docs/trackers/provenance-subsystem.md` | params, no sidecar declared |
| 5 | remove `[F, W]` from one declared session log | template, declared log missing F/W |
| 6 | declare `[F, W]` in one exempt log | template, stale exemption |
| 7 | delete one line from the test's exemption list (same guard input as a new undeclared log) | template, new undeclared log |
| 8 | break `id_prefix=` in one TAXONOMY row | unparseable |
| 9 | `WIN` → `WINX` in the TAXONOMY row only | citable only — a params row, because a prose prefix that is not citable is also dropped by `declared_prefixes_from_frontmatter` |
| 10 | make `declared_prefixes_from_frontmatter` return empty | proves the production reader is called |
| 11 | break the walker's `entry_collection` needle | non-vacuity |

- **Gate:** `./scripts/gate.sh`. The test sits under the librarian feature and runs only in the default lane; confirm by the test's own name in that lane's output, never by either lane's total.

## Bookkeeping

- Before implementing, record the deep-agent observation-window workflow entry (pre-action snapshot) in `docs/trackers/deep-agent-workflow-observations.md`.
- Claim `96b2b1b9a25bb1b0`; on completion fill its Fix and Tests sections, record SHA and patch-id, archive via `doc(action="move")`, repoint inbound citations.
- File two follow-up issues: the uncovered surfaces (CLAUDE.md, sidecar prompts, ledger templates), tagged `cluster/selector-narrower-than-its-population`; and the template shipping undeclared so its own recipe is refused on a fresh copy, tagged `cluster/doc-contradicted-by-code`.
- Pathspec commits of this session's paths only, each green on landing, so no gate/falsifier coupling applies.

**Amended 2026-09-27 while planning**, against the approved draft: the target is confined to the *Lives in* cell (a disambiguator the draft lacked); mutation 9 moved to a params row because `declared_prefixes_from_frontmatter` already filters non-citable prefixes, so the draft's prose mutation could not show the citable guard is load-bearing; mutation 7 is realised by deleting an exemption line; and the single commit became several green ones. Plan: `docs/superpowers/plans/2026-09-27-taxonomy-append-recipes-test.md`.

## Out of scope

- Recipe-id liveness — owned by the commit-time hook the ADR names.
- Other refusal conditions (for example a missing `anchor_heading`).
- TAXONOMY's own open F/W decision, and changing the session-log template.
