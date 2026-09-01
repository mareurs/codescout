---
status: open
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

Add `"heading_hint": err.hint().unwrap_or_default()` to the `None` arm of
`heading_miss_meta` (`src/librarian/tools/get.rs:50-60`), matching its sibling.

**Deliberately NOT bundled into the change that found it.** That change
(`a-scoped-read-is-billed-the-full-heading-map`) restores the full heading map on a miss,
which *re-masks* this defect — the caller gets the map instead of the hint. That makes this
lower-priority, not fixed: the hint is the targeted answer and the map is the brute-force
one, and a caller who wants only the hint still cannot have it.

SHA and patch-id to be recorded here at fix time.

## Tests added

None yet — not fixed. When fixed, the guard must assert the hint's **presence and content**
on the absent path, paired with the existing ambiguous-path behaviour. Asserting only
"`heading_hint` exists" is monotone under widening — `unwrap_or_default()` returns `""`,
which is present and useless.

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
