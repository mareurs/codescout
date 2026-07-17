# Design — `edit_markdown` batch-mode order-independence

**Date:** 2026-07-17
**Status:** proposed
**Work-stream log:** `docs/trackers/edit-markdown-batch-ordering-session-log.md` (F-1)
**Author context:** brainstormed with the Architecture Snow Lion; scouted via reconnaissance (F-1).

---

## 1. Problem

`edit_markdown` batch mode (`edits: [...]`) applies edits **sequentially over a
single mutating buffer**. Each edit resolves its `heading` against the buffer
*as already mutated by prior edits in the same batch* (`edit_markdown.rs` batch
loop, ~lines 690–745; `new_content` is reassigned each iteration and fed to the
next).

This makes a batch **order-dependent**, and the dependency is invisible until it
fails. The observed failure:

```jsonc
edits: [
  { heading: "The 7 cases", action: "edit",
    old_string: "## The 7 cases", new_string: "## The 8 cases" },   // renames the heading
  { heading: "The 7 cases", action: "edit",
    old_string: "| 7 | ... |", new_string: "| 7 | ... |\n| 8 | ... |" }, // adds a row under it
]
```

edit[0] renames the heading in the buffer; edit[1] then resolves `heading="The
7 cases"` against that mutated buffer, finds nothing, and the whole batch rolls
back atomically. The error even leaks the intermediate state (it lists `## The
8 cases` as an available heading), confirming edit[0] applied before edit[1]
failed.

The user's workaround — reorder so the row edit precedes the rename, or split
into two calls — works but is a footgun the tool should absorb.

## 2. Non-goals (explicitly rejected)

- **A global message/queue system for "all actions".** Cross-process write
  serialization already exists and is live: every mutating tool call passes
  through `acquire_write_guard_if_writing` (`server.rs:748`) →
  `write_guard::acquire` (`agent/write_guard.rs:47`), which holds an in-process
  async mutex **and** a cross-process `flock` on `.codescout/write.lock` across
  the whole call. The librarian adds a finer per-artifact `WriteLockRegistry`
  (`librarian/tools/event_create.rs:231`). A queue would duplicate this
  guarantee at higher operational cost (it needs a broker/durable-queue process;
  codescout is deliberately N independent processes) — and, critically, it would
  **not** fix this bug, which is an *intra-call* logic flaw already running under
  one lock atomically. Revisit only if a concrete need the `flock` cannot meet
  is named: durable/replayable action history, ordering fairness under
  starvation, cross-*project* serialization, or write observability.
- **Auto-reorder heuristic.** Detecting "heading-mutating edits" and applying
  them last is fragile (chained renames, insert-then-address). Snapshot
  resolution (§3) achieves order-independence deterministically without guessing.
- **Changing single-edit mode.** The `heading`+`action` single-edit path is
  unaffected; only the `edits: [...]` batch path changes.

## 3. The model — resolve against the original snapshot, apply by write-span

Two coupled ideas:

### 3.1 Resolve all addresses against the original snapshot

Before any mutation, resolve **every** edit's `heading` against the *original*
`file_content` (the snapshot as the call arrived), not the running buffer. Every
heading reference in the batch therefore means "the heading as it was when the
call arrived." This removes order-dependence at the root and is **deterministic**
— no reordering, no chained-rename ambiguity.

This aligns with codescout's cross-cutting **repair-and-continue input law** (ADR
`2026-07-10-repair-and-continue-input-handling`): when intent is deterministically
inferable, the tool proceeds silently; it errors only on genuine ambiguity.
Snapshot resolution *is* that deterministic inference for batch edits.

### 3.2 Write-span conflict model (byte offsets)

Each edit, resolved against the original, produces one or more **planned edits**:

```
PlannedEdit { span: Range<usize> /* byte offsets into the original */, replacement: String }
```

Byte offsets (not line indices) are required because a scoped `action="edit"`
rewrites a byte range that may be mid-line or multi-line, and must compose in one
batch with line-aligned section operations. A one-time line-start offset table
over the original converts the existing line-index logic (`resolve_section_range`
returns 1-indexed lines; `compute_section_end` returns an exclusive line index)
into byte offsets. This is the LSP `TextEdit[]` model — non-overlapping ranges,
applied end-to-start — which `edit_code` already uses.

**Per-action write-span** (all computed against the original; each reuses the
*existing* apply logic and its guards, restated here only as the span each
produces):

| Action | Write-span (byte range into original) | Replacement | Preserve (existing behavior) |
|---|---|---|---|
| `edit` (scoped) | The byte range(s) of `old_string` **within** the resolved section. `replace_all` ⇒ multiple spans; else the first. | `new_string` | Exact whitespace-sensitive match within section; "not found in section" error unchanged. |
| `replace` | `[heading_start .. replace_end]`, where `replace_end` excludes a trailing HR separator + its leading blank lines (F-3). | heading-line + separator + body, or body-only when new content's first line is a same-level heading (`replace_heading`). | F-7 surface-marker guard; `replace_heading` same-level logic; separator + trailing-newline rules. |
| `insert_before` | Zero-width point at `heading_start`. | `content` (trailing-newline-normalized) | — |
| `insert_after` | Zero-width point at `end_idx` (default `at="end-of-section"`) or at `heading_line+1` (`at="after-heading-line"`). | `content` (trailing-newline-normalized) | `at` validation; heading-fusion guard. |
| `remove` | `[heading_start .. remove_end]`, `remove_end` consuming one trailing blank line if present. | `""` | trailing-blank consumption. |

### 3.3 Overlap detection + application order

1. **Collect** all planned edits from all edits in the batch.
2. **Reject genuine overlaps.** Two spans conflict iff their byte ranges
   intersect. A zero-width insert point at offset `X` conflicts with a
   non-empty span only if `X` is strictly interior to it; boundary-touching
   inserts are allowed. Equal-offset zero-width inserts apply in batch-array
   order (deterministic tie-break). On conflict → `RecoverableError` naming both
   edit indices and the shared region, with the hint "these edits rewrite the
   same region; split into separate calls or make them disjoint." (This is the
   residual teaching error — the former "A3" diagnostic, now scoped to the
   irreducible case only.)
3. **Apply** planned edits sorted by **descending `span.start`** (end-to-start),
   splicing into the original string. Descending order guarantees an applied
   splice never shifts the offsets of not-yet-applied (lower-offset) spans.
4. **Normalize once.** `normalize_trailing_newline` runs a single time on the
   final document (today it runs per-edit; in batch it must run once at the end).
   `ensure_trailing_newline` on inserted *content* stays per-planned-edit.

**Worked check (the pg36 case):** both edits are scoped `edit`s addressing
heading "The 7 cases", which exists in the original. edit[0]'s span = bytes of
`## The 7 cases` (heading line); edit[1]'s span = bytes of the row-7 line (body).
Disjoint ⇒ allowed. Applied end-to-start ⇒ both land. Works regardless of array
order. ✅

**Conflict example (correctly rejected):** a whole-section `replace` plus a
scoped `edit` on the same section — the `edit`'s span is interior to the
`replace`'s span ⇒ intersect ⇒ rejected with the teaching error. ✅

## 4. Interface changes

- **No public tool-schema change.** `edits: [...]` accepts the same shape; the
  behavior becomes order-independent and the failure mode narrows to genuine
  overlap.
- **Internal refactor.** Each per-action arm of `perform_section_edit_ext` and
  `perform_scoped_edit` splits into:
  - a **plan** function: `(original, SectionRange, params) -> Result<Vec<PlannedEdit>>`
    (computes span + replacement, runs the guards, does **not** splice), and
  - a shared **apply** step: sort-descending + splice + single normalize.
  Single-edit mode calls plan-then-apply with a one-element vector, so both modes
  share the same core and cannot diverge.
- A `LineOffsets` helper (line-index ⇄ byte-offset over the original) lives next
  to the apply step.

## 5. Invariants to preserve (regression surface)

These are load-bearing and already tested; the refactor must keep them green,
not reimplement them:

- **F-3** trailing-HR-separator preservation on `replace`.
- **F-5 / F-7** surface-marker (`<!-- @surface -->`) preservation guard on
  `replace` (breaks `build.rs` slice extraction if dropped).
- `replace_heading` same-level-heading replacement (see
  `docs/issues/2026-07-02-edit-markdown-replace-drops-target-heading-on-heading-shaped-content.md`).
- `compute_section_end` fenced-code-block awareness (headings inside ``` ``` are
  not section boundaries; unbalanced fences degrade gracefully).
- Heading-fusion guards (`ensure_trailing_newline`) on `insert_after` / `replace`.
- Body-shrink guard and the frontmatter-then-body atomic-write ordering
  (unchanged; they wrap the batch as today).

## 6. Testing strategy

- **Regression:** the existing `src/tools/markdown/tests.rs` suite must pass
  unchanged (proves single-edit behavior + all guards preserved).
- **New — order-independence:** the pg36 batch in *both* orders (rename-first and
  row-first) produces byte-identical output and succeeds.
- **New — disjoint same-section edits:** two scoped `edit`s in one section (heading
  line + a body row) both apply.
- **New — genuine overlap rejected:** `replace` + scoped `edit` on the same section
  → `RecoverableError` naming both indices; file unchanged (atomic).
- **New — mixed actions bottom-to-top:** `insert_after` in an early section +
  `edit` in a late section + `remove` of a middle section, in scrambled array
  order, all land correctly against original offsets.
- **New — normalize-once:** a batch of N inserts yields a single trailing newline,
  not N.
- **Property (optional):** for any batch with pairwise-disjoint spans, output is
  invariant under permutation of the edits array.

## 7. Open questions

- **Zero-width insert tie-break at a shared boundary** — array order is chosen;
  confirm no existing test depends on a different resolution.
- **`replace_all` scoped edit producing many spans** — confirm all occurrence
  spans within one section are individually overlap-checked against other edits
  (they are disjoint from each other by construction).

## 8. Decision record (ADR summary)

**Decision:** Batch `edit_markdown` resolves all heading addresses against the
original snapshot, compiles each edit to byte-offset write-span(s), rejects only
genuinely overlapping spans, and applies end-to-start with a single final
normalize.
**Alternatives:** global queue (rejected — duplicates live `flock`, wrong layer);
auto-reorder (rejected — non-deterministic); diagnostic-only (folded in as the
residual overlap error).
**Consequences:** batches become order-independent with no schema change; cost is
a bounded internal refactor (plan/apply split + offset table) that must preserve
the F-3/F-7/`replace_heading`/fence guards.
**Change scenarios absorbed:** any batch that mutates a heading and addresses
under it; any batch where an earlier edit shifts a later edit's anchor.
**Confidence:** high on the model; residual risk is in preserving the existing
guards through the plan/apply split — mitigated by the unchanged regression suite.
