---
status: open
opened: 2026-08-17
closed:
severity: low
owner: marius
related: []
tags: [librarian, ledger, entry-ids, progressive-disclosure, mcp-surface]
kind: bug
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
| `frontmatter_max` > `body_max` | **the ledger has been compacted** — entries live in an archive companion |

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

Add `"frontmatter_max": outcome.frontmatter_max` and `"reserved_max":
outcome.reserved_max` to the prose branch's response. Then consider a `warning` field
on the `frontmatter_max > body_max` case, mirroring the params branch's existing
`warning` for "params lags the body" — the shapes are analogous and the params one is
already pinned by `call_warns_when_params_lags_the_body`.

Worth deciding at the same time: whether the warning should fire at all. A compacted
ledger is a *correct* state, not drift, so the warning must read as information rather
than a problem — otherwise it trains agents to "repair" a ledger that the archive
cadence policy deliberately shaped that way.

## Tests added

None yet. Wanted: extend `omitting_entry_collection_reserves_an_id_and_writes_no_entry`
(`src/librarian/tools/append_entry.rs`) to assert the two new response fields, and a
new case where the body has been emptied and the mark governs, asserting
`frontmatter_max` is reported.

## Workarounds

Read the mark straight out of the file — `entry_high_water_<PREFIX>` is committed
frontmatter, so `head` on the ledger shows it. The allocation is correct either way;
only the observability is missing.

## Resume

Add the two fields to the `json!` block in the prose branch of
`src/librarian/tools/append_entry.rs` and extend the test named above. Decide the
warning question in § Fix before adding a `warning` field — the answer changes whether
this is a two-line change or a prompt-surface one.

## References
- `src/librarian/tools/append_entry.rs` — the prose branch's response
- `src/librarian/catalog/augmentation.rs` — `AllocateOutcome`, `allocate_entry_id`
- `docs/issues/archive/2026-08-17-ledger-id-reissue-silently-repoints-citations.md` —
  the fix this field was added for, and the claim that is now half wrong
- `docs/trackers/tracker-hygiene-log.md` — HY-12
