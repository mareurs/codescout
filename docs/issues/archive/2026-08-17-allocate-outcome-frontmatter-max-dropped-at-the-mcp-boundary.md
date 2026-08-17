---
kind: bug
status: fixed
tags:
- librarian
- ledger
- entry-ids
- progressive-disclosure
- mcp-surface
closed: 2026-08-17
opened: 2026-08-17
owner: marius
related: []
severity: low
---

# BUG: `append_entry` computes `frontmatter_max` and then drops it, hiding the one signal that says a ledger was compacted

## Summary

`AllocateOutcome` carries three derivation inputs — `body_max`, `reserved_max`,
`frontmatter_max` — but the prose branch of `append_entry` puts only `body_max` in
its MCP response. So an agent cannot see which input governed the allocation, and in
particular cannot see the one interesting case: `body_max < frontmatter_max`, which
means entries have been compacted out of the live body and the committed mark is
carrying the history alone.

## Symptom (Effect)

Measured 2026-08-17 on the live surface, first allocation after `cargo rb`:

```
artifact(action="append_entry", id="7e498b6dcb45b924", id_prefix="HY")
->
{
  "id": "HY-12",
  "artifact_id": "7e498b6dcb45b924",
  "reserved": true,
  "body_max": 11,
  "next_step": "Reserved HY-12 and recorded the ledger's high-water mark ..."
}
```

`frontmatter_max` was `None` here (the mark did not exist yet, so `body_max` governed)
— but it would be equally invisible when it is `Some(N)` and larger than `body_max`,
which is the state worth reporting.

## Reproduction

Commit `dc2d1dd8`, branch `experiments`.

1. Allocate once on any ledger — the mark is written.
2. Compact the ledger: move its entries to an archive companion through a body edit,
   which preserves frontmatter.
3. Allocate again. The response reports `body_max: null` and says nothing about the
   mark that actually produced the id.

## Environment

Linux, `experiments` @ `dc2d1dd8`. Affects every prose ledger — the five declaring
`entry_prefix` today.

## Root cause

`src/librarian/tools/append_entry.rs`, the prose-ledger early return, names four
fields and omits the third input:

```rust
return Ok(json!({
    "id": outcome.id,
    "artifact_id": target,
    "reserved": true,
    "body_max": outcome.body_max,
    "next_step": format!(...),
}));
```

`outcome.frontmatter_max` exists — added in `0364c23a` alongside the fix — and is
simply not read. *Measured 2026-08-17: the live response above, against the struct
definition in `src/librarian/catalog/augmentation.rs`.*

The archived bug file that specified the fix asserts the opposite, and is now half
wrong at the MCP boundary: *"Both fields are returned by `AllocateOutcome` precisely
so the caller can notice this"*
(`docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md`
§ Workarounds). True of the Rust struct, false of the tool.

## Evidence

### The interesting comparison is the one not surfaced

The three inputs fail in different directions, and `next` is their max. Which one won
is diagnostic:

| Relation | Means |
|---|---|
| `body_max` governs | normal steady state |
| `reserved_max` > `body_max` | an id was handed out and the body has not caught up |
| `frontmatter_max` > `body_max` | ~~the ledger has been compacted~~ — **wrong as filed, corrected while implementing.** This relation is also true immediately after *any* ordinary reservation, because the reservation writes the mark. The second call in `reservation_reports_all_three_derivation_inputs` has `frontmatter_max` 42 against `body_max` 41 and is nothing but a normal allocation. The honest condition is the mark leading **both** other inputs |

Only the first two are inferable from the current response, and only because
`reserved_max` is absent too — so in practice a caller sees `body_max` and nothing to
compare it against.

## Hypotheses tried

1. **Hypothesis:** the omission is deliberate output-budget trimming.
   **Test:** compare against the params branch and the guide's stance on hints.
   **Verdict:** rejected — the response already carries a multi-sentence `next_step`
   string, so one integer is not what is being economised. `body_max` is present
   precisely because it is diagnostic; `frontmatter_max` is more so.

## Fix

**Landed `4cdafd9a` (experiments).**

All three inputs now ship as data on the prose branch — `body_max`, `reserved_max`,
`frontmatter_max` — **present even when null**, because an absent key reads as zero to
anything scanning JSON.

**The warning question: no, and Pattern 5a is why.** `docs/PROGRESSIVE_DISCOVERABILITY.md`
defines `warning` as *off-golden-path — result is suboptimal but valid, reconsider before
proceeding*. A compacted ledger is none of those: it is a correct state the archive cadence
produced on purpose. Tagging it would train agents to "repair" it, which is precisely what
this file's own Fix section warned about. The three severity keys are also mutually
exclusive, and this branch already speaks through `next_step` — so the fact goes there, as
a fact.

**The discriminator in this file's Evidence table was wrong, and implementing it is what
caught that.** `frontmatter_max > body_max` does not isolate compaction. The shipped
condition is the mark leading **both** the live body and the reservation table, so the mark
alone accounts for the number.

And the note names *alternatives* rather than asserting one: compaction, a fresh clone, and
an `artifact(move)` all produce that state, and three integers cannot tell them apart.
Asserting "compacted" would have been Anti-Pattern 5 — in the file that gained
Anti-Pattern 5 the same morning.
## Tests added

Both in `src/librarian/tools/append_entry.rs`, red observed first, each on the thing that
was genuinely missing:

- `reservation_reports_all_three_derivation_inputs` — asserts the two new keys are
  **present** on a first allocation (where both are null) and carry values on the second,
  where the first call's own reservation and mark have populated them. Failed on
  `reserved_max` absent from the payload.
- `reservation_names_compaction_without_calling_it_a_warning` — a ledger whose live body
  heads nothing and whose committed mark is 11. Asserts `frontmatter_max` is reported, the
  governing input is named in words rather than left as integers to compare, and that no
  `warning` key appears. Failed on `frontmatter_max` returning null where 11 was due.

The second test's `warning`-absence assertion is the load-bearing one: it pins the design
decision, not just the payload, so a later "helpful" warning cannot be added silently.
## Workarounds

Read the mark straight out of the file — `entry_high_water_<PREFIX>` is committed
frontmatter, so `head` on the ledger shows it. The allocation is correct either way;
only the observability is missing.

## Resume

N/A — fixed and archived.
## References
- `src/librarian/tools/append_entry.rs` — the prose branch's response
- `src/librarian/catalog/augmentation.rs` — `AllocateOutcome`, `allocate_entry_id`
- `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md` —
  the fix this field was added for, and the claim that is now half wrong
- `docs/trackers/tracker-hygiene-log.md` — HY-12
