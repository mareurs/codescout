---
id: '489339eef5f9e93b'
kind: research
status: draft
title: 'Plan D scout — the cutover tool: what exists, what is missing, what must come first'
owners:
- marius
tags:
- identity
- plan-d
- scout
- cutover
---

# Plan D scout — the cutover tool: what exists, what is missing, what must come first

Written 2026-11-14, after Plan C1 landed (`7db510676`). Method: I read the spec § Cutover and § Left to the builder; the module docs of `src/librarian/derived/diff.rs` and `src/cli/derive_shadow.rs`; `src/librarian/catalog/schema.sql`; and the symbol lists of `catalog/dual_write.rs`, `catalog/augmentation.rs`, `augmentation_sidecar.rs` and `derived/build.rs`. I ran nothing against the real catalog. Counts such as "38 params entries" and "160 of 335 hex ids" are the spec's 2026-11-11 measurements. I did not re-measure them. I did not read function bodies unless a finding says so.

## What the spec asks of Plan D

Spec § Cutover lists four steps, in this order:

1. **Export** every catalog-only fact into its committed home: params entries without markdown, augmentation shapes, notes, verdicts, intents, observations and manual links.
2. **Rename** non-canonical filenames and key collisions.
3. **Rewrite citations** to grammar forms, and report what cannot be rewritten.
4. **Build** a new catalog file from the files. Keep the old file read-only.

The export and the rewrite must finish before the read flip (Plan E). The tool needs a read-only open of the old file, because `Catalog::open` migrates.

## What exists now (read, not run)

| Need | What exists | Where |
|---|---|---|
| Read-only open of the old catalog | `open_old_read_only` (`SQLITE_OPEN_READ_ONLY`, never migrates) | `derived/diff.rs` |
| Compare old catalog with a derived index | `diff_with_rules`, `derive-shadow` CLI (`--project`, `--rev`, `--old-db`, `--json`) | `derived/diff.rs`, `cli/derive_shadow.rs` |
| Build the new catalog from git objects | `build_full(conn, &BuildInput)` into any connection | `derived/build.rs` |
| Write an entry into a ledger as a section | `write_entry_into_ledger`, `splice_pending_section` | `catalog/dual_write.rs`, `catalog/augmentation.rs` |
| Write an augmentation sidecar | `write_through`, `doctor fix=export_augmentations` | `augmentation_sidecar.rs`, `tools/doctor.rs` |
| Render a citation as grammar text | `cite_text::grammar_form_of` | `catalog/cite_text.rs` |
| Move one prefix with its citations | `rekey_prefix` | `catalog/rekey.rs` |

Old catalog tables that hold facts no file holds (from `schema.sql` and `catalog/mod.rs`): `artifact_augmentation` (prompt and params, which include the params entries), `artifact_observation`, `events` (ten kinds, including note, intent and verdict), `event_edges`, `artifact_link` (manual relations), `entry_cite` and `entry_reservation`.

## Findings

**D-1. The shadow diff has no old-root option.** `DeriveShadowArgs` has four fields: `project`, `rev`, `old_db`, `json`. `derived::diff` keeps an old row only when its `abs_path` lies under the git root. When rows exist and none lies under the root, it returns `NoOldRowsUnderRoot` and no diff. A catalog written under another spelling of the root (a symlink, a moved checkout) gets no measurement. The cutover tool meets the same case. Plan D needs an `--old-root <path>` mapping that the diff and the cutover share.

**D-2. The dual-write library is tied to the old catalog's write transaction.** The signatures take a `rusqlite::Transaction` and an `artifact_id`: `write_entry_into_ledger(tx, roots, artifact_id, …)` and `record_written_file(tx, artifact_id, …)`. I did not read the bodies. If they write rows to the old catalog, they conflict with the read-only open that the spec requires. Plan D must either add a variant that writes only the file, or run on a scratch copy of the old catalog. This choice needs a decision before the plan.

**D-3. Three export targets do not exist yet.** The spec leaves "the section format for events and observations" and "the rel-label vocabulary for manual links" to the builder. Plan C2 owns them. Plan D cannot export events, observations or manual links until C2 defines the formats. Entries and augmentation shapes can export now, because C1 built their writers.

**D-4. The rename step needs a rename that rewrites citations.** Bug note N-5: `doc(move)` with a stem rename leaves `[[oldkey]]` citations stale. The spec gives the rename tool to `codescout-37`'s move workflow. Step 2 of the cutover must call a rename that rewrites every `[[old-key…]]` in the same change. If that tool does not exist when Plan D starts, Plan D must carry its own rewrite.

**D-5. The citation rewrite must read file text, not catalog rows.** The diff module documents that `entry_cite.dst_ref` is rebuilt by the scanner (`link_scan::entry_dst_ref`). It is not what the author wrote. An id citation and a path link both become a 16-hex id, and a stem-qualified token loses its qualifier. A rewrite driven by rows would produce wrong text. The rewrite pass must scan the markdown and use the catalog only to resolve ownership. The rule from the C1 state document still holds: never drop a row whose text twin does not resolve; report it.

**D-6. Hex-id recovery from git history has no code that I found.** The spec says `sha256(<root>/<old path>)[:16]` recovers 160 of 335 distinct stale ids. I did not search the code for a helper. Treat it as new code until a search shows otherwise.

**D-7. The build step reads a commit, not the working tree.** `BuildInput` holds a `git2::Repository` and a `Commit`. The export and the rewrite write files. The new catalog can only see them after a commit. So the tool has two phases with the operator's commit between them: write and report, then build from `HEAD`. The spec does not say this. It is a design question for the operator.

**D-8. The spec asks for best effort and a report, but names no report format.** A migration window with all sessions stopped needs a report that lists what moved, what was skipped and why. A re-run of the tool after a partial run must not duplicate sections. C1's `find_unrecorded_section` adopts an unrecorded section on the params path, so the entry export may reuse it. I have not tested that.

## Proposed order (for the operator to confirm)

- **Plan C2 first** (events, observations, links to files). Then every catalog-only fact has a committed home.
- **Plan C3 is independent** of D. It can run in parallel or later.
- **Plan D in two parts if C2 slips:** D1 for entries, sidecars, renames and citation rewrite; D2 for events, observations and links after C2. The tool's dry run (counts per class, unresolved citations) comes first in D1, because it extends `derive-shadow`.
- Shared piece for both: the `--old-root` mapping from D-1.

## Questions for the operator

1. Should C2 land before D, or should D split into D1 and D2 and start now?
2. Should the cutover write files and stop for the operator's commit (D-7), or may it commit by itself inside the migration window?
3. For D-2: a file-only variant of the dual-write library, or a scratch copy of the old catalog?
