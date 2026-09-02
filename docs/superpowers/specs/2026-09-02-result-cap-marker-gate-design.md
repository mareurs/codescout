---
id: ed7c767669ca46e3
kind: spec
status: draft
title: Result-cap marker gate — classify the population, then probe it
owners:
- marius
tags:
- design
- progressive-disclosure
- truncation
- testing
- issue-clusters
- IC-13
topic: capped results and truncation markers
---

# Result-cap marker gate — classify the population, then probe it

## Problem

`IC-13` (`cluster/capped-result-presented-as-complete`, artifact `8a9dd5a27cd03480`) has
**13 members and no mechanism**. Its claim: a result is truncated by a limit and returned
without a marker the caller can see — so a partial answer reads as the whole one, and a
**zero** from a capped scan reads as *not present* rather than *not reached*.

The class file states the invariant and names the cost:

> **The invariant to gate:** a response that was capped must carry a marker saying so, and
> the marker must be reachable from the *shape the caller reads*.

> **No cross-cutting guard exists**: a sweep for one found 16 hits across 14 files, all
> per-site assertions, none a sweep.

Three members carry `status: open`, and **re-deriving their code state on 2026-09-02 found
two of the three already moved** — which is itself the argument for a gate rather than a
member-by-member sweep:

| member | file says | code says |
|---|---|---|
| `artifacts-are-embedded-from-their-first-chunk-only` | `open` | **fixed** at `488192e8`; `embed_queue_items` is now ONE PER CHUNK, with regression test `embed_queue_items_emits_every_chunk_not_just_the_first` whose comment reads *"Mutating the implementation back to `.next()` must fail HERE"*. The bug file is stale. |
| `symbols-renders-a-wrapped-signature-truncated-at-the-paren` | `open` | fix **in flight** in a concurrent session's uncommitted `src/tools/symbol/{symbols,tests}.rs`; `focus_single_symbol` is at `src/tools/symbol/symbols.rs:990` |
| `overflow-summary-promotes-the-count-and-elides-its-caveat` | `open` | open — `describe_payload_shape` reduces `hints` to a bare key name |

The two stale rows are the `verify-open` shape `CLAUDE.md` describes: `488192e8` is a
`feat(librarian):` commit naming no tracker entry, so nothing tripped and the file stayed
`open` by default. **Members get fixed; the mechanism that would stop the next one does not
arrive with them** — which is exactly what `Mechanism status: none yet` records, and why
this spec targets recurrence rather than the current three.

*(This paragraph replaced one written twenty minutes earlier that cited `indexer.rs:69` as
live. It was true when the class file recorded it and false when the spec quoted it — an
`IC-11` instance inside the spec for `IC-13`. Recorded rather than silently corrected,
because the ledger's own rule is that a claim carries its derivation.)*

### What is already solved, so nobody rebuilds it

The class file's *Where to start* points at `truncate_compact` and the buffer envelope.
**Both already carry markers**, and this was verified at the bytes before the spec was
written rather than taken from the field:

- `truncate_compact` (`src/tools/core/types.rs:511-525`) appends `"\n… (truncated)"`
  unconditionally when it cuts.
- `output_buffer.rs` carries `Truncation { kept_lines, total_lines }` **on the buffer
  entry** rather than on the response, plus a distinctive
  `--- codescout: BUFFER TRUNCATED` sentinel and one shared `truncation_marker()` used by
  both emission sites "so the two can never disagree." Its doc comment records why:
  `unfiltered_truncated` lives on the response, is minted once, and is gone by the next
  turn, while the handle keeps working for the session.

So the shared-layer half of the class's own remedy has **landed**. What remains is not one
layer, which the class file's own withdrawn-concentration paragraph predicted:

> At **4 of 9** it is no longer *most*, so the concentration argument is **withdrawn as
> stated**: the layer hosts a plurality of this class, not a majority, and a single
> invariant on it is a partial remedy rather than a near-complete one.

This spec therefore builds a **detector with auditable coverage**, not a production
invariant.

## Decision

Ship a two-part gate.

1. **Classification** — every result-shaping cap in tracked source is classified inline, or
   the gate reds. Coverage becomes auditable rather than asserted.
2. **Probing** — each cap classified as caller-visible has a behavioural row that drives the
   real tool past its own cap and asserts a marker *arrives*.

Rejected alternatives are recorded in § *Alternatives considered*.

## The invariant, made checkable

A probe row is a triple: `(surface, input that provably exceeds the cap, marker path)`. It
asserts a **conjunction**:

1. **The call was actually capped** — established from the response itself (`total >
   returned`, a `hit_cap` flag, a count mismatch), **never** from the fact that a large
   input was passed. An input believed to exceed the cap but which does not would make the
   row vacuous *while still passing* — the monotone-assertion failure `CLAUDE.md`
   § *Testing Discipline* describes.
2. **A marker is reachable from the shape the caller reads** — asserted **twice per row**:
   once on the inline response, once on the compact/overflow form when the payload is large
   enough to buffer.

The second assertion is load-bearing rather than belt-and-braces. `IC-13`'s clause was
widened on measurement because only **4 of 16** members matched *"without a marker"*; five
more had a marker computed **correctly** that never reached the reader. The class's own
`link-scan-truncation-is-accurate-and-unreachable` names the distinction — *"The
information exists and is simply not where the decision is made."* **Accuracy was never the
property that mattered; arrival is.** A gate asserting only on the full payload would leave
that arm — the larger one — untouched.

### Three outcomes a row must distinguish

| observed | verdict |
|---|---|
| succeeded, was capped, marker reachable in both forms | PASS |
| succeeded, was capped, marker absent from either form | **RED — the defect** |
| returned `{"ok": false}` (a `RecoverableError`) | **RED — bad row**, not a finding |

The third line is why rows reuse `call_tool_checked` (`src/server.rs:6430`) rather than a
fresh driver. codescout routes `RecoverableError` to a **success** result carrying
`{"ok": false}`, so checking `is_error` alone silently passes a failed call — that helper's
own doc comment records it, pinned by `recoverable_error_routes_to_success_not_is_error`. A
probe that scored a rejected call as *capped, no marker* would return a plausible finding
instead of an error, which is `IC-13` occurring inside the gate for `IC-13`.

### Deliberately out of scope

Whether a marker's **numbers** are right. `grep`'s self-refuting `Showing N of N` is
`IC-19`/`IC-20` territory: the class file holds a visible-but-wrong marker outside its
clause because it "defeats a different remedy" and its true total after `hit_cap` is
**unknowable rather than unreported**. This gate asserts arrival, not accuracy.

## Mechanism

### Classification lives inline

Next to the constant, never in a side manifest. A manifest is the *population published
where the reader never looks* defect (`CLAUDE.md` § *Observer Blindness*); a comment on the
declaration is read by whoever adds the next cap.

```rust
// cap-class: NOT_A_CAP — LSP handshake deadline, never shapes a result
const HANDSHAKE_TIMEOUT_MS: u64 = 5_000;

// cap-class: RESULT_CAP grep.lines — probed
const GREP_LINE_LIMIT: usize = 50;
```

The gate, over **tracked** `src/**/*.rs` only:

1. every cap-shaped constant carries a `cap-class`, or **RED**, listing the offenders;
2. `NOT_A_CAP` requires a **non-empty reason** — a bare token is RED, because "the
   annotation exists" is not the property wanted;
3. every `RESULT_CAP <id>` has a probe row with that id, **and** every row names a live
   `RESULT_CAP` — both directions, so a deleted constant orphaning a row also reds.

**Tracked, not the worktree.** `tests/issue_clusters.rs` established this and states why:
gating on untracked files lets one session red another's build, since an untracked file is
a peer's in-flight work. `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md`
is open because a sibling gate got this wrong.

### Two instruments, different scopes

The scan regex `const [A-Z_]*(CAP|LIMIT|MAX|BUDGET|THRESHOLD)[A-Z_]*` matches **105
constants across 52 files** (measured 2026-09-02). It misses a cap named `PAGE_SIZE`,
`K_NEIGHBOURS` or `DEFAULT_DEPTH`. **Building the gate on that regex alone would ship
`IC-18` (`selector-narrower-than-its-population`) inside the gate for `IC-13`.**

So two instruments run, and their scopes differ:

- **A — declarations.** The constant scan above.
- **B — call sites.** An independent scan for truncation *operations*: `.take(`,
  `.truncate(`, `truncate_compact(`, `.chars().take`, `&x[..n]`, and `.next()` on a chunk
  iterator.

A site found by **B** whose governing bound is not a classified constant from **A** is RED
as unclassified.

Two instruments are required rather than nice-to-have: `CLAUDE.md` § *Observer Blindness*
holds that two agreeing instruments are evidence **only when their scopes differ**, since
two per-profile instruments agreeing is one blind spot counted twice and is
indistinguishable from corroboration at the point of use. Two constant scans would be
exactly that. **B is the only instrument that would have reached the `indexer.rs` member**, whose cap was
a bare `.next()` governed by no constant at all. That member is now fixed, which makes it a
**worked example rather than a live target** — and a better argument for B than a live one
would be: the class demonstrably produces members that instrument A cannot see, so A alone
would have reported full coverage of a population it was structurally blind to.

### Placement

| artifact | needs | lane |
|---|---|---|
| `tests/result_caps.rs` — instruments A + B, classification, both-directions row check | filesystem + git only | **both lanes** |
| probe rows — drives real tools past their caps | `CodeScoutServer`, async, private helpers | feature-partitioned |

The probe table is a **declarative const array in one file**, which is what lets the gate
scan it as text so the two halves cannot drift.

**The gate is a source-text check, and that is a correctness requirement rather than a
convenience.** Were it to consult a *compiled-in* list, it would red spuriously on the lean
lane: `librarian` is a default feature, so caps declared inside `#[cfg(feature =
"librarian")]` exist in source while their rows compile out under
`--no-default-features`. The gate would then fail **by following `CLAUDE.md`'s documented
gate order** — the same shape as the `target/debug/codescout` trap that ordering exists to
defuse. Reading declared row ids out of the table's *text* is feature-independent by
construction and behaves identically on both lanes.

Rows touching `artifact`/`link_scan` carry `#[cfg(feature = "librarian")]`;
`grep`/`symbols`/`read_file` rows are core.

**`call_tool_checked` is lifted** from `guide_hint_tests` into a shared `pub(crate)` test
helper — not for tidiness, but because a second copy is a second place to get the
`RecoverableError` subtlety wrong, and that mistake makes a bad row look like a finding.

## Phase 1 scope

Instruments A + B ship over the **whole** population. Probe rows start as a stated subset.

### Positive controls are part of the design

`run_command`'s `unfiltered_truncated` and `link_scan`'s `counts.truncated` already emit
correctly. Rows for them are **not** redundant: without a row that *should* pass, a probe
reporting "marker missing" everywhere is indistinguishable from a probe that cannot read
markers at all. They are the **denominator** — § *Testing Discipline*'s rule that
instrumenting the confirmation is what stops a population looking self-correcting.

### Starter rows — no LSP, no embedder, deterministic

| surface | cap | marker |
|---|---|---|
| `run_command` | inline byte budget | `unfiltered_truncated` — **positive control** |
| `link_scan` | per-array findings | `counts.truncated` — **positive control** |
| `grep` | line limit | `hit_cap` |
| `artifact.find` | `limit` | truncation hint / `more_in_scope` |
| `artifact.get` | body bytes / heading count | `body_meta` |
| overflow envelope | `TOOL_OUTPUT_BUFFER_THRESHOLD` | summary retains the caveat, not only the count |

### Deferred, with the reason recorded rather than left as an omission

- **`symbols` wrapped-signature** (`focus_single_symbol`). The class file records this
  defect as **invisible without a warm language server**: with none, codescout falls back to
  AST extraction, reports the true range and inlines the body correctly. A row that did not
  wait for rust-analyzer would **pass vacuously**, which is worse than no row. Belongs
  behind `e2e-rust` with an explicit warm-up; filed `not-yet` with this reason.
- **`indexer.rs` first-chunk-only — no longer a target; fixed at `488192e8`.** Kept in this
  section because its *shape* sets a requirement: the cap was a bare `.next()` with no
  governing constant, so instrument **A** could never have classified it. Its landed
  regression test is also the pattern phase 1 should copy — it names the mutation that must
  red it, in the test body, which is what a `mutation: killed` row means.

**Phase 1 fixes none of the three open members.** It makes them visible, named and gated
against recurrence. `IC-13`'s `Mechanism status` is what changes; the member count is not.

## What counts as proof

`CLAUDE.md` demands an **observed RED**, produced by mutating the **production** path, and
separately: *"Mutate once per guarded SITE, not once per feature — one kill says nothing
about the other N−1."* Read literally, every `RESULT_CAP` needs its marker emission deleted
in production, its row observed red, and the deletion reverted.

Rather than claim that for all sites and perform it on three:

- each row lands **with** its per-site mutation result recorded — `mutation: killed` or
  `mutation: not-yet`;
- the gate asserts nothing about mutation status (it cannot) but **prints the per-site
  tally**, so a run reports `12 RESULT_CAP sites, 5 mutation-verified` rather than merely
  passing;
- that tally is published in `IC-13`'s `**Mechanism status:**` field, **not only** in the
  test module header — a bound living in the enforcement layer is published to an audience
  that never reads it, and the `29.5%` tag-coverage incident (`OB-1`,
  `reconnaissance-patterns:R-170`) is the precedent.

**The consequence, stated plainly:** a row marked `not-yet` is an assertion whose ability to
fail is **unproven**, and the tally is what stops it being credited as coverage. This is the
*annotate an inert fixture as inert* law applied to the gate's own rows — one direction
guards against silent removal, the other against silent credit.

## Alternatives considered

### A. Production invariant — a typed `Capped<T>` wrapper — **rejected for now**

A wrapper every capping site must return, whose serialization cannot omit the marker. This
is strictly stronger: it makes the defect **unrepresentable** rather than detectable, which
is the preferred shape.

Rejected on blast radius. It touches ~14 files and every response shape agents already
depend on, on a checkout with concurrent peer sessions — and `IC-13`'s membership is 13
files whose caps differ in kind (a byte budget, a page size, a heading count, a display
limit), so one wrapper type would need to span all four. **Revisit when** the gate's
`RESULT_CAP` classification has run over the full 105, since that pass produces the very
inventory this refactor needs and does not currently exist.

### B. Production registry — compiler-enforced via `inventory` — **rejected**

Capping sites register through a macro so the compiler refuses an unregistered cap. Removes
the selector problem entirely — no grep to be narrower than its population.

Rejected for the same ~14-file blast radius as A, with less benefit: it guarantees
*registration*, not marker *arrival*, so the probe rows would still be needed on top.

### C. Usage-ranked seed set, bound published — **rejected**

Probe the surfaces agents hit most by `usage.db` call count; publish covered and uncovered
sets. Cheapest to ship.

Rejected because the bound is a judgement that decays silently, and nothing reds when a new
capping surface lands outside the seed. That is the state `IC-13`'s `Mechanism status`
already describes — *"16 hits across 14 files, all per-site assertions, none a sweep"* — so
it would re-create the complaint in a new file.

## Success criteria

1. `tests/result_caps.rs` reds on: an unclassified cap constant; a `NOT_A_CAP` with an empty
   reason; a `RESULT_CAP` with no row; a row naming no live `RESULT_CAP`; a **B**-instrument
   truncation site whose bound is unclassified.
2. Each of those five reds is **observed**, not merely asserted to exist.
3. Every filter the live gate runs is an **extracted function** the meta-tests call —
   following `missing_index_rows` in `tests/issue_clusters.rs`, so no meta-test asserts
   about its own re-implementation.
4. Every exemption ships a paired test proving the exemption is **narrow**, following
   `missing_index_rows_exempts_only_unclassified`.
5. All 105 cap constants classified; the count of `RESULT_CAP` vs `NOT_A_CAP` published with
   its unit.
6. Both positive-control rows pass, and each is observed to red when its production marker
   is removed — otherwise the harness's ability to read a marker is unproven.
7. Gate green on **both** lanes, in `CLAUDE.md`'s documented order.

## Revisit-when

- The `RESULT_CAP` inventory is complete → re-price alternative **A**, which needs exactly
  that inventory.
- A member arrives whose cap is neither a constant nor one of instrument **B**'s operations
  → both instruments are narrower than the population and a third is owed.
- `mutation-verified` stays below half the `RESULT_CAP` sites for more than one cycle → the
  tally is being published and ignored, which is a different defect from the one this spec
  addresses.

## References

- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
  (`8a9dd5a27cd03480`) — the class, its two closed rulings, and the withdrawn concentration
  argument.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class
  instrument **A** would instantiate if it ran alone.
- `tests/issue_clusters.rs` — the classify-or-red precedent: extracted filters, narrow
  exemptions with paired tests, tracked-files-only scanning.
- `src/tools/output_buffer.rs` — the already-landed entry-level marker; the pattern for
  "on the object the caller re-reads, not on the response."
- `src/server.rs:6430` `call_tool_checked` — the driver, and the `RecoverableError` trap.
- `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md` —
  open; the mistake the tracked-files rule avoids.
- `docs/issues/2026-09-02-overflow-summary-promotes-the-count-and-elides-its-caveat.md` —
  the one member open in both its file and its code.
- `docs/issues/2026-09-02-artifacts-are-embedded-from-their-first-chunk-only.md` (fixed at
  `488192e8`, file stale) and
  `docs/issues/2026-09-02-symbols-renders-a-wrapped-signature-truncated-at-the-paren.md`
  (fix in flight in a peer session) — **not** reconciled by this spec; both belong to their
  authors.
