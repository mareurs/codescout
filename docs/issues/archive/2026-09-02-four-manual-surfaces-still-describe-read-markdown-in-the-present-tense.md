---
id: 8cddaf991ea51a6a
kind: bug
status: fixed
title: 'BUG: four manual surfaces still describe read_markdown in the present tense, and no gate reaches them'
tags:
- cluster/doc-contradicted-by-code
claimed_at: 2026-09-06
claimed_by: 4a2f34f7-0669-487d-9ce9-39b77881642f
---

## Summary

Task 7 deleted the `read_markdown` MCP tool. Four manual surfaces still describe it in the present
tense — including an entire reference page with two JSON call examples and **no tombstone**. No gate
reaches any of them, and the project's own precedent for this exact situation is to open a bug file
rather than widen a gate.

## Symptom (Effect)

A reader following `docs/manual/` is told to call a tool that no longer exists, and the manual's
canonical tool table lists it as live. The full gate is green.

## Reproduction

At `87216a54` on `tool-collapse`:

| surface | what it still says |
|---|---|
| `docs/manual/src/tools/read-markdown.md` | a whole present-tense page — *"`read_markdown` now selects output detail based on file size"*, *"`read_markdown` now accepts `@file_*` buffer refs"* — plus two JSON call examples. **No tombstone.** |
| `docs/manual/src/SUMMARY.md:113` | `- [read_markdown](tools/read-markdown.md)` — still in the table of contents |
| `docs/manual/src/tools/overview.md:34` | the manual's canonical tool table lists it as live |
| `docs/manual/src/tools/peer.md:75` | lists it as peer-exposed — **directly contradicted by the same commit**, which removed it from `PEER_EXPOSED_TOOLS` |

Then confirm no gate fires: `cargo test --test doc_tool_refs` → 3 passed.

Measured 2026-09-02 by the Opus task review of `87216a54`.

## Environment

Branch `tool-collapse` at `87216a54`. The manual pages are shared with `experiments`; the staleness
is branch-local, created by Task 7.

## Root cause

`tests/doc_tool_refs.rs` **does** walk `docs/manual/` (`:263`), so this is not a directory-enumeration
gap. It catches **anchored call forms** — `tool(param=…)` — and bare prose mentions are outside its
regex by its own module doc, deliberately.

So the narrowing is **grammatical**: a sentence saying *"`read_markdown` now accepts `@file_*` buffer
refs"* names a dead tool without ever writing a call, and a TOC entry or a table row names one
without so much as a verb.

`document-section-editing.md:41` received a tombstone in the same commit; `read-markdown.md` did not.
That asymmetry is the tell — the author was fixing pages the gate reded on, and the gate reded only
where a call form appeared.

## Why a bug file rather than a gate change

**The project has already ruled on this exact situation.** `doc_tool_refs.rs`'s module doc cites the
identical case — a manual page describing a collapsed server — being filed as a bug rather than
prompting a widened regex:
`docs/issues/2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed.md`.

The reason widening is unattractive is the same one that makes
`prompt_surfaces_reference_only_real_tools` backtick-scoped (`463b1cb984c715b0`): an unanchored scan
for tool-shaped tokens over prose matches ordinary words, and a gate nobody can keep green gets
deleted. Recording the instance is the cheaper correct move; **what is owed is the repair, not a new
gate.**

## Evidence

### `peer.md` is contradicted by its own commit

`87216a54` removed `"read_markdown"` from `PEER_EXPOSED_TOOLS` in `src/peer/server.rs`. A peer client
calling it now receives `AccessDenied`. `docs/manual/src/tools/peer.md:75` still advertises it as
available. Code and doc were edited in the same commit and disagree.

### `SUMMARY.md` was not on the implementer's own list

The implementer reported three manual pages as deliberately left (`read-markdown.md`, `overview.md`,
`peer.md`). `SUMMARY.md:113` appears in none of its lists — so it is not a scoped deferral but an
omission, which is worth separating: the first is a decision, the second is the thing decisions miss.

## Hypotheses tried

1. **Hypothesis:** `docs/manual/` is outside the gate's walk, like `docs/architecture/`
   (`3f0e7733ae77c707`).
   **Test:** read `tests/doc_tool_refs.rs:263`.
   **Verdict:** rejected — `docs/manual/` **is** walked. The narrowing is the call-form regex, not the
   directory list. Distinct from that bug.
   **Evidence:** § Root cause.

2. **Hypothesis:** the tombstone convention was applied and one page was missed by accident.
   **Test:** compared `document-section-editing.md:41` (tombstoned) against `read-markdown.md` (not).
   **Verdict:** confirmed as a pattern, not an accident — the tombstoned page carried a call form the
   gate reded on; the untombstoned one did not. **The gate selected which pages got fixed.**
   **Evidence:** § Root cause.

## Fix

1. **Tombstone `read-markdown.md`** in the style of `document-section-editing.md:41`, pointing at
   `read_file`'s heading-addressed mode.
2. **`SUMMARY.md:113`** — remove or re-point the TOC entry.
3. **`overview.md:34`** — drop the row; the tool table is the manual's canonical inventory and a stale
   row there is worse than a stale page, because it is the surface a reader consults to learn what
   exists.
4. **`peer.md:75`** — remove; it is contradicted by `PEER_EXPOSED_TOOLS` as of `87216a54`.

**Sequence with Task 8.** `edit_markdown` is folded next by the same plan, and it has the same four
surfaces. Doing this once, after Task 8, costs one pass instead of two — but **do not defer past the
plan's docs sweep (Task 10)**, or it becomes that task's silent debt rather than this one's recorded
work.

**Done 2026-09-06 — and three of the four items above were already done by other work.** The
reproduction was run before this plan was read, which is the only reason that surfaced: item 1's
tombstone, item 2's `SUMMARY.md` re-point, item 3's `overview.md` row and item 4's `peer.md` row had
all landed since 2026-09-02, and nothing had closed this file.

What actually remained, none of it named above:

- **`read-markdown.md:23,37`** — the body stayed present-tense over a dead name *underneath* the
  tombstone, which instructed the reader to substitute mentally (*"read `read_markdown` here as
  `read_file`"*). Made current instead. A softened historical mention left in prose is
  indistinguishable from a live citation to any parser over that namespace, and to the next reader.
- **`cross-process-write-serialization.md:35`** — `edit_markdown` listed in a present-tense set of
  tools that acquire the write lock.
- **`augmentation-render-template.md` ×3** — `librarian_context`, from the **2026-05-02** collapse
  rather than the 2026-09-02 one. A served page (`SUMMARY.md:60`), so reader-facing.

Two look-alikes deliberately left: `src/tools/markdown/edit_markdown.rs` is a **live source path**
(the tool retired, the module did not), and the past-tense blockquote tombstones in
`document-section-editing.md` and `markdown-tools.md` are already the correct shape.

**Not fixed here, filed instead:** `docs/manual/src/concepts/librarian-mcp.md` carries 18 retired
names in a tool-inventory table and documents a separate MCP server that no longer exists. It is
absent from `SUMMARY.md`, so it needs a delete/tombstone/rewrite decision rather than a substitution
— `36ff17248b2c6ec7`.

Fix SHA: `1f982a34f0a2371fd92d233f4aedf576c29f5c31`
Patch-id: `c2ae9f523f258d1175f9704ff9a478cd7ff63aba`

## Tests added

None, and deliberately: § *Why a bug file rather than a gate change* argues the gate should not be
widened for this. The acceptance criterion is the repair itself. If a future session does decide to
gate bare prose mentions, that is a separate change with its own false-positive budget to measure
first.

## Workarounds

Treat `docs/manual/src/tools/` as unverified for tool existence during the tool-surface collapse.
The authoritative inventories are `tools/list` and `src/librarian/tools/mod.rs::all_tools()`.

## Resume

After Task 8 folds `edit_markdown`, repair all four surfaces for both tools in one pass, and check
whether `edit-markdown.md` has the same missing tombstone. Then re-run
`cargo test --test doc_tool_refs` to confirm it was green before and after — the point being that it
cannot tell you whether you succeeded, which is why this file lists the surfaces explicitly.

## References

- Found by the Opus task review of `87216a54` (Task 7 of the tool-surface-collapse plan), 2026-09-02,
  as finding M2.
- Precedent this follows:
  `docs/issues/2026-09-01-librarian-mcp-page-describes-a-separate-server-that-was-collapsed.md`.
- Sibling with a *directory* gap rather than a grammatical one: `3f0e7733ae77c707`.
- `CLAUDE.md` § *Testing Discipline* — "Loudness is a property of a PATH, not of a failure."
