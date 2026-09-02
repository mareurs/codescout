---
id: e688e8f39825e228
kind: bug
status: open
title: 'BUG: a byte-ceiling test cannot see a member stop delivering, and neither remedy that suggests itself works'
tags:
- cluster/assertion-that-cannot-fail
closed: null
opened: 2026-09-02
owner: marius
severity: low
unverified: 'fix is implemented and mutation-verified on branch p50-absorption-demo (13ee893b) but NOT landed — src/server.rs carries a third party''s uncommitted instrumentation inside the same function. Also: the landed fix matches a STRING, so a reword of the injector''s marker would silently disarm it; the typed GuideDeliveryShape::Preamble is unreachable from the test''s vantage.'
---

# BUG: a byte-ceiling test cannot see a member stop delivering, and neither remedy that suggests itself works

## Summary

`a_p50_session_stays_under_the_committed_guide_byte_ceiling` (`src/server.rs`) sums guide
bytes over six tool shapes and asserts `total <= CEILING` and `total > 0`. A guide section
silently ceasing to be delivered fails **neither**: the sum absorbs one member going to zero,
and a vanished section makes the ceiling assertion *more* comfortable. The test already
computes the per-shape number that would discriminate and discards it.

## Symptom (Effect)

Mutation — rename `serves: artifact.append_entry` in `src/prompts/guides/librarian.md` so
that shape's section no longer matches — leaves the test green:

```
test server::guide_hint_tests::a_p50_session_stays_under_the_committed_guide_byte_ceiling ... ok
test result: ok. 1 passed; 0 failed
```

Measured per-shape distribution, unmutated:

```
create 2785   get 0   update 3193   append_entry 1643   find 2233   move 2018
total 11872   CEILING 12000
```

## Reproduction

At `experiments`, in a worktree with its own `target/`:

1. `cargo test --lib a_p50_session_stays_under_the_committed_guide_byte_ceiling` → green.
2. Rename `<!-- serves: artifact.append_entry -->` in `src/prompts/guides/librarian.md`.
3. Re-run → **still green**. `append_entry` now delivers 491 B rather than 1643 B.

## Environment

Linux, `experiments`, isolated worktree (own `target/`, so no shared build lock).

## Root cause

`total > 0` is a sum of six non-negative addends, so it fails only if **all six** are zero —
insensitive to five of them by construction. That half is a proof, not a measurement.

`total <= CEILING` is blind in the **same** direction: losing a section *reduces* the total.
So both assertions move the safe way when content disappears, which is worse than one blind
spot — the pair's failure modes agree.

The discriminating value is computed and thrown away: `shape_total` returns `bytes` and all
six call sites discard the return, silently, because `usize` is not `#[must_use]`. The author
had written the predicate in prose — *"`get` is the ONE shape expected to report 0 B here …
Any OTHER shape reporting 0 B is suspicious, not normal."* A guard that would have worked was
sitting in the function, unreferenced.

The adjacent mode **was** closed deliberately: every call goes through `call_tool_checked`
rather than `call_tool`, because a silently-failed call reports 0 B and is character-identical
to legitimate dedup. So call failure as a cause of a zero is covered; a zero arising any other
way is not.

Measured 2026-09-02: distribution and both mutation outcomes observed, not reasoned.

## Evidence

### Both obvious remedies are falsified, which is the reusable part

| remedy | result under the mutation |
|---|---|
| `total > 0` (existing) | **GREEN** — absorbed |
| `bytes > 0` per shape, exempting `get` | **GREEN** |
| assert no block carries the declared failure marker | **RED**, naming serves-drift |

The second is what the diagnosis implies, and two readers agreed on it independently. It fails
because a shape whose section is gone does **not** report 0 B: it receives a 491-byte fallback
whose own marker reads `no section declares this call's shape`
(`src/tools/core/guide_emit.rs`, `guide_blocks_for`'s unmatched branch). Every shape has a
floor above zero, so **no non-emptiness assertion at any grain can discriminate**.

### On the cluster tag — an adjacent fit, stated rather than stretched

Tagged `cluster/assertion-that-cannot-fail`. The clause's headline is *"an assertion has no
input that would make it fail"*, and read strictly that is **false** here: `total > 0` fails
if all six shapes are zero. The fit is to the clause's own gloss — *"zero coverage wearing a
passing test's clothes"* — with the difference worth recording: **no input within the
assertion's CLAIMED SCOPE would make it fail.** The per-member claim is unfalsifiable while the
population claim is not.

That distinction is the subject of the law added at `0d2ab2b1` (`CLAUDE.md` §
*Testing Discipline*, derivation in
[`docs/conventions/what-green-is-evidence-for.md`](../conventions/what-green-is-evidence-for.md)
§ *Scope, not direction*), which is deliberately **not** an `IC` class: it is a property of
assertions rather than a defect class instantiated by bug files. If a future reader decides
the scope variant deserves its own class, this file is its first member and the alias gate on
`tool-collapse` is its second.

## Hypotheses tried

1. **Hypothesis** — the aggregate is the only blind spot; a per-member assertion fixes it.
   **Test** — implemented `bytes > 0` per shape with a `get` exemption; re-ran the mutation.
   **Verdict** — rejected. Green. The 491-byte fallback floors every shape.

2. **Hypothesis** — the fallback is a bug in the injector.
   **Test** — read `guide_blocks_for`'s unmatched branch and its doc comment.
   **Verdict** — rejected. It is deliberate, documented, and self-describing: a preamble plus a
   `get_guide(topic)` pointer, *"never the whole topic, never silence"*. The defect is the
   test's, not the injector's.

3. **Hypothesis** — this is systemic; aggregates are generally unguarded here.
   **Test** — checked the two adjacent cases.
   **Verdict** — rejected, and recorded as a confirmation rather than a catch.
   `tool_descriptions_stay_under_budget` / `every_tool_description_under_cap` are two levels,
   and `server_registers_all_tools` / `server_tool_count_is_l3_target` close the empty-population
   hole in the latter. The split is usually guarded by a **pair**; this test is where the pair
   was missing.

## Fix

Implemented and verified, **not landed**: branch `p50-absorption-demo`, `13ee893b`, patch-id
`95fd1e5230f052448d99346801c659993d1941b9`.

Assert that no emitted guide block carries the no-section-matched marker. Cause-naming rather
than magnitude-guessing, and it needs no per-shape labels, no call-site edits and no ordering
assumption — all three of which the rejected shapes required.

**Held because `src/server.rs` carries a third party's uncommitted
`eprintln!("P50_TOTAL_BEFORE={total}")` inside this very function** — someone is instrumenting
the aggregate with an aggregate-only instrument. Verified not the `tool-collapse` session's (1
occurrence in the main checkout, 0 in that worktree). Landing would conflict with work in
flight; the branch is offered to whoever holds that line.

**The fix's own weakness is annotated at the fix**: it matches a **string**, so a reword of the
marker leaves it passing and no longer discriminating. The robust form is one layer down —
`GuideDeliveryShape::Preamble`, which `guide_blocks_for` already returns — but it is unreachable
from this test's vantage, which sees only emitted `Content` through `call_tool_checked`.

## Tests added

The fix *is* the test. Derivation, re-runnable: **A** unmutated → GREEN, **B**
`serves: artifact.append_entry` renamed → RED naming serves-drift, **C** reverted → GREEN.

## Workarounds

None needed — nothing is broken today. The cost is that a guide section could stop being
delivered and this test would keep passing, which is what it exists to prevent.

## Resume

Land `13ee893b` once `src/server.rs` is free in the main checkout — check
`git status --short src/server.rs` and the presence of `P50_TOTAL_BEFORE` first. Re-run the
A/B/C derivation after any rebase rather than trusting the pre-rebase run.

If the shape enum becomes reachable from the test's vantage, replace the string match with
`GuideDeliveryShape::Preamble` and delete the annotation about its brittleness.

## References

- `CLAUDE.md` § *Testing Discipline* — the law, added `0d2ab2b1`, sharpened `621d7dff`.
- [`docs/conventions/what-green-is-evidence-for.md`](../conventions/what-green-is-evidence-for.md)
  § *Scope, not direction — the p50 byte-ceiling run* — full derivation, both falsified remedies,
  and the second subsystem.
- `src/tools/core/guide_emit.rs` — `guide_blocks_for`'s unmatched branch, which emits the marker.
- Class named by the `tool-collapse` session from a symptom in its own gate; the located
  instance, both falsifications and the marker fix derived here.

