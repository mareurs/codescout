---
kind: bug
status: fixed
title: "BUG: an entry's scope has no terminator, so every section after the last entry inherits its token"
tags:
  - cluster/addressing-without-an-escape-hatch
owners: []
severity: med
---

> **OWED:** written directly to disk because a peer session held the catalog write
> lock throughout the window in which this was found (7+ minutes, every write path).
> It therefore has **no catalog row, and its `cluster/` tag has not reached the
> catalog** — a frontmatter tag alone does not (BL-48), so `doc(action="find")`
> reports this bug unclassified until someone runs `librarian(action="reindex")` and
> then `doc(action="update", id=…, patch={tags:["cluster/addressing-without-an-escape-hatch"]})`.
> Delete this block once that is done.

## Summary

`entry_tokens_by_line` reports the entry token **in scope** at each line, and only an
entry-shaped heading (`## TOKEN — title`) resets that scope. An ordinary section
heading — `## History`, `## Corpus`, `## Monthly trend` — does not, so every section
after the last entry in a file inherits that entry's token, however far away it is.

Measured 2026-09-04 on `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`:
`GF-8`'s heading is at **L131**, `## History` is at **L425**, and the chunk starting
at 425 is stored with `entry_token: "GF-8"` — 294 lines and **15 non-entry `##`
sections** later.

## Symptom (Effect)

A live `doc(action="find", semantic=…)` hit:

```json
{
  "start_line": 425,
  "end_line": 426,
  "entry_token": "GF-8",
  "snippet": "## History\n",
  "retrieve": "doc(action=\"get\", id=\"ac8fbe339e66ade3\", heading=\"GF-8\")"
}
```

The snippet is the file's `## History` section. It is not GF-8, it is not about
GF-8, and following `retrieve` lands the reader in GF-8 — an entry they were never
shown and did not match.

Three consumers are affected, in increasing order of cost:

- **The token itself** is a wrong answer where `None` is the true one.
- **`retrieve`** (added `1e1fb026`) sends the reader to the wrong entry. It is
  strictly better than the line range it replaces — which is wrong for 378 of 3,729
  chunks (`7695ad877b44e96a`) — but it inherits this error rather than avoiding it.
- **The embedded text.** `indexer.rs:175-180` prepends the token to a chunk that
  does not open with a heading, so `## History` is embedded as `GF-8\n\n## History`.
  A query about GF-8 is pulled toward a section that has nothing to do with it.

## Reproduction

```
./target/release/codescout doc find --semantic "gate feedback latency" --limit 2 --json
```

Or directly against the catalog:

```sql
SELECT start_line, entry_token, substr(content,1,20) FROM artifact_chunk
 WHERE artifact_id = 'ac8fbe339e66ade3' AND start_line = 425;
```

## Environment

Observed on `target/release/codescout` at `1e1fb026`, catalog schema v12.
**Pre-existing:** `entry_tokens_by_line` has carried scope forward since it was
written, and nothing in `1e1fb026` changed it. That commit only made the consequence
*visible*, by publishing `retrieve`.

## Root cause

`src/librarian/entry_token.rs:30-45`. `entry_tokens_by_line` carries `current`
forward on every line and only reassigns it when `heading_defines_entry` matches:

```rust
if let Some(tok) = heading_defines_entry(line) { current = Some(tok); }
out.push(current.clone());
```

There is no branch that CLEARS `current`. The grammar correctly refuses to let a
non-entry heading DEFINE a token — `## History` has no `TOKEN — title` shape — but
"does not define" is then silently treated as "does not end the previous one".
Those are different claims and the code only implements the first.

This is `cluster/addressing-without-an-escape-hatch` (IC-6) from a third side.
CLAUDE.md's § *Parsers Over a Namespace* frames the two halves as no-escape and
no-disambiguator; the missing thing here is a **terminator**. The grammar can say
where an entry begins and has no way to say where one ends, so an entry ends only at
the next beginning or at EOF.

## Impact

Scoped, not corpus-wide: it costs nothing in a file whose entries run to EOF, which
is the common ledger shape. It bites files that put prose sections AFTER their
entries — audits, session logs with a trailing `## History`, anything with a
`## References` footer.

Not yet counted, and the count wants a definition first: "chunks whose token came
from a scope more than one non-entry heading back" is checkable, but whether a
`### sub-heading` inside an entry should terminate it is a judgement, and that
judgement changes the number. Deriving it before deciding the fix would be deriving
it under a rule the fix might not adopt.

## Fix — original assessment, SUPERSEDED (kept: it was wrong in a way worth seeing)

Unresolved, and the choice is a real one:

- **Terminate at any same-or-shallower heading that is not an entry.** Correct for
  `## History` after `## GF-8 — …`. Wrong for a ledger whose entries are `####` and
  which uses `###` groupers between them, if any such file exists — that needs
  checking, not assuming.
- **Terminate only at a heading of the same level as the defining one.** Narrower
  and safer; leaves a `### Notes` inside an entry attributed to that entry, which is
  right.

The second looks correct and is not yet verified against the corpus. Whichever
lands, `heading_defines_entry` already parses the level (it strips `##` then further
`#`) and currently discards it — the information needed is present and thrown away,
the same as the title text.

## Fix

**Fixed `919da0cb`, patch-id `78e942346e50ad13`.** An entry's scope now ends at the
next heading whose level is *at or above* its own — ordinary markdown sectioning.

**The superseded Fix section above had it backwards and was re-derived before writing.** It
proposed "terminate only at a heading of the same level" as the safer option; that
would let an `#` H1 run straight through a `##` entry. The correct rule is
`level <= entry_level`, which was the option this file called risky. The risk it
named — `###` groupers between `####` entries — is not a counter-example but the
rule working: a `### Group` genuinely does end a `#### T-5` entry.

Blast radius measured BEFORE the change, over the whole corpus: **6,983 lines
across 90 of 1,446 files** change owner, every one of them from a wrong entry to
`None`. Entry headings exist at `##` (1,082), `###` (545) and `####` (64), which is
why the terminator has to be level-aware rather than a constant.

Four guards added, including the fence case: a guide quoting `## History` must not
END a real entry any more than a quoted `## W-1 — x` may start one — the same escape
IC-6 already owed on the defining side.

## Tests added

Added in `919da0cb`: `a_non_entry_heading_at_the_same_level_ends_the_entry` (the
GF-8 shape reproduced), `a_deeper_heading_inside_an_entry_does_not_end_it`,
`a_shallower_heading_ends_a_deeper_entry`, and
`a_heading_inside_a_fence_neither_starts_nor_ends_an_entry`.

The original note stands unchanged, because it is the reason none of this was
caught earlier: the guard is a fixture with a non-entry heading after an entry,
asserting
the chunk starting at that heading carries `entry_token: None`. Every current test in
`entry_token.rs` seeds a body whose entries run to the end, so the terminating case
is not merely untested — it is **unrepresentable in the existing fixtures**, which is
why no assertion could have caught it.

## Workarounds

Read `matched.snippet` before trusting `matched.entry_token` on a hit whose snippet
opens with a `##` heading that is not the token's own. That is a per-hit human check
and does not scale; there is no query-side workaround.

## References

- `src/librarian/entry_token.rs:30-69`
- `src/librarian/indexer.rs:175-180` — the embed-text prepend that inherits the error
- `docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-but-published-as-file-lines.md`
  — the other half of "the published anchor does not agree with the entry"
- CLAUDE.md § *Parsers Over a Namespace — owe an escape and a disambiguator*
