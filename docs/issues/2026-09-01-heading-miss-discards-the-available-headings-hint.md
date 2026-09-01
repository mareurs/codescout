---
status: fixed
opened: 2026-09-01
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md
tags:
  - cluster/declared-not-wired
kind: bug
---

# BUG: a heading miss discards the "Available headings" hint the resolver already built

## Summary

`artifact(action="get", heading=…)` on a heading that does not exist returns
`body_meta: {"heading": …, "heading_missing": true}` and nothing else. The resolver has
already computed the list of available headings and attached it to the error as a hint —
`heading_miss_meta`'s absent arm simply does not read it. The sibling **ambiguous** arm,
three lines above, *does* forward `err.hint()`. So the capability is built, wired on one
branch, and dropped on the other.

## Symptom (Effect)

Verbatim, observed 2026-09-01 in a live MCP session:

```
"body_meta": {
  "line_count": 0,
  "source_line_count": 1242,
  "bytes": 0,
  "heading": "## IC-15 — a parameter is accepted then silently dropped",
  "heading_missing": true
}
```

The caller learns *that* the heading is absent and nothing about what would work. In the
observed case the heading had been renamed by a concurrent session, and the correct text
was recoverable only by a second call.

## Reproduction

```
artifact(action="get", id="<any artifact>", heading="## No Such Heading")
```

Observe `heading_missing: true` with no `heading_hint` key. Contrast with an **ambiguous**
heading on the same tool, which returns `heading_ambiguous`, `occurrences`, **and**
`heading_hint`.

## Environment

Linux, MCP stdio transport, project `codescout`, branch `experiments`, HEAD
`e171dd1e841df4fe0f9aeb7d1c146c9dd19a7431` at observation.

## Root cause

Two sites, and the mismatch between them is the defect.

**The hint is built.** `src/tools/file_summary/file_summary.rs:447-450`:

```rust
Err(RecoverableError::with_hint(
    format!("heading '{}' not found", heading_query),
    format!("Available headings: {available}"),
))
```

`available` is assembled just above (`:443-446`) with head/tail elision, so it is already
bounded for large files.

**The hint is dropped.** `src/librarian/tools/get.rs:50-60`, `heading_miss_meta`:

```rust
match err.extra.get("occurrences") {
    Some(occurrences) => json!({
        …,
        "heading_hint": err.hint().unwrap_or_default(),   // ← forwarded
    }),
    None => json!({ "heading": name, "heading_missing": true }),   // ← discarded
}
```

The asymmetry inside a single `match` is what makes this a slip rather than a design
choice: whoever added `heading_hint` to the ambiguous arm had the hint in hand and did not
add it to its neighbour.

Measured 2026-09-01: both sites read directly (`symbols include_body` on
`heading_miss_meta`; `grep` on `file_summary.rs`), and the empty-hint response quoted above
observed live.

## Evidence

Filed after a code review flagged it while assessing an unrelated change
(`docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`, finding I2).
**Both halves were verified before filing rather than taken from the review** — this repo's
record contains a filed bug that asserted a missing capability the code already had, and a
bug file is durable and re-checked by nothing.

## Hypotheses tried

1. **Hypothesis:** the hint is absent because the resolver does not compute one for the
   absent case.
   **Test:** read `file_summary.rs:443-450`.
   **Verdict:** rejected — it is computed, with elision, and passed to `with_hint`.

2. **Hypothesis:** `heading_miss_meta` deliberately withholds it to keep the response small.
   **Verdict:** rejected — the ambiguous arm forwards it unconditionally, and the elision at
   the build site already bounds the size. No comment anywhere states a size rationale.

## Fix

**Fixed 2026-09-01 on `experiments`.** SHA and patch-id below.

`heading_miss_meta` (`src/librarian/tools/get.rs`) now hoists `err.hint()` above the match, so both
arms forward it from one expression — the two arms cannot drift on this field the way they already
did once.

**A SECOND SITE existed and this file did not name it.** The plural `headings=[…]` branch builds its
own `missing` list and never calls `heading_miss_meta`, so fixing the helper alone would have left
half the defect — the same shape as the `create`/`update` pair fixed earlier the same day, and the
same law: *mutate once per guarded SITE, not once per feature*. It now captures the hint once and
emits it as `headings_hint`. **Once, deliberately:** the hint is derived from the *document*, not
the query, so every missing member in one call would yield a byte-identical string, and N copies of
one fact is exactly the envelope bloat this work stream exists to remove.

**The prediction in this section held, with one correction.** *"That change re-masks this defect —
the caller gets the map instead of the hint"* was right, and is why this stayed low-priority rather
than being bundled in. What it under-stated: on a **plural** miss the map is not merely brute-force,
it is the *only* signal, because that branch reported bare member names with no hint at all.
## Tests added

`a_missing_heading_forwards_the_available_headings_hint`,
`missing_plural_headings_forward_one_shared_hint`, and `a_resolved_heading_carries_no_hint`, all in
`src/librarian/tools/get.rs`.

**The warning this section wrote in advance is implemented, not just heeded.** Both miss-tests
assert the hint **names `Alpha` and `Beta`** — the fixture's real headings — rather than that the key
exists. `unwrap_or_default()` on a hintless error yields `""`, which is present and useless, so a
presence check would pass in exactly the broken world.

**Three mutations, three sites/directions, each killing exactly one test:**

| # | mutation | result |
|---|---|---|
| D | singular arm drops `heading_hint` again | 56 passed / 1 failed — `a_missing_heading_forwards_…` **only** |
| E | plural branch drops `headings_hint` | 56 passed / 1 failed — `missing_plural_headings_…` **only** |
| F | emit the hint key on SUCCESS too | 56 passed / 1 failed — `a_resolved_heading_carries_no_hint` **only** |

**F is why the third test exists.** D and E are presence assertions and are monotone under
widening: emitting `heading_hint` unconditionally, on every call including fully successful ones,
satisfies both completely. Only the third mutates that way — and a hint on a success is noise in the
very envelope this work stream shrank.

**E's load-bearing fixture detail** is that it names **two** missing members rather than one. That
is what makes "emitted once, not per member" an observable property instead of an untested claim.

Source restored byte-exactly after each run (`diff -q` → identical), verified rather than assumed.
## Workarounds

Call `read_markdown(path)` for the heading map, or `artifact(action="get", id=…)` with no
selector. Once the sibling change lands, a body-selected miss returns the full
`preview.headings` array, which answers the same question more expensively.

## Resume

Edit `src/librarian/tools/get.rs:50-60`: add `heading_hint` to the `None` arm. Add a test
asserting the hint text is non-empty and names at least one real heading from the fixture.

## References

- `src/tools/file_summary/file_summary.rs:447-450` — where the hint is built.
- `src/librarian/tools/get.rs:50-60` — `heading_miss_meta`, where it is dropped.
- `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md` — the review of
  that change surfaced this.
- **Cluster reasoning.** Tagged `cluster/declared-not-wired` (IC-3): `with_hint` declares
  the capability and the absent branch never reaches it. Two alternatives were considered
  and rejected — `cluster/hint-composed-without-the-request` (IC-22) does not fit because
  the hint here is correctly composed and then discarded, not composed from the wrong
  input; and `cluster/accepted-parameter-silently-dropped` (IC-15) does not fit because the
  `heading` parameter *was* honoured — the lookup ran and reported truthfully. **Tag settled by the ledger's own discriminator — *what fix does the defect require?*** This
one is repaired by wiring an existing in-code declaration to a live route: the hint is
already built by `with_hint`, and the fix adds one line to `heading_miss_meta`'s `None` arm
so the route reaches it. Nothing new is declared at any surface. That is IC-3's remedy, so
the tag holds. Had the repair instead required *declaring* something at a surface that
never declared it, that is a different remedy and would want a different class. Recorded
because an earlier draft of this file left the tag flagged as contestable on the basis of
in-code-vs-surface *description*, and description decides nothing here — the remedy does.
