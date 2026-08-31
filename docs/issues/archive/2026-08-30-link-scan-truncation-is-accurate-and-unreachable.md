---
id: 43a4abe4f4397663
kind: bug
status: fixed
title: 'BUG: link_scan caps its finding arrays at 8% of the population and reports the truncation somewhere nobody reads — so absence from a bucket looks like a clean result'
tags:
- librarian
- link-scan
- negative-results
- silent-truncation
- progressive-disclosure
closed: 2026-08-31
found_by: codescout-f0 (pid 807989), filed here at their offer
opened: 2026-08-30
owner: marius
severity: high
---

# BUG: `link_scan` caps its finding arrays at 8% of the population and reports the truncation somewhere nobody reads

## Summary

`librarian(action="link_scan")` caps each finding array at **50**. The real counts live
in `counts`, and a per-array `counts.truncated` map is present and **accurate**. But
neither reaches the caller's eye: the overflow envelope's `summary` prints
`arrays: ambiguous[50], dangling[50]`, and the `hint` talks only about `write=true`.

So a reader who searches a bucket for a token and does not find it has searched
**under 10% of the findings**, with nothing on the surface saying so.

**Measured 2026-08-30 on this repo:**

| bucket | shown | actual | fraction visible |
|---|---|---|---|
| `dangling` | 50 | **637** | **7.8%** |
| `ambiguous` | 50 | **551** | **9.1%** |
| `cross_repo` | 48 | 48 | 100% |
| `malformed_qualifier` | 8 | 8 | 100% |

The two buckets a reader is most likely to interrogate are the two that are truncated,
and they are truncated hardest.

## Symptom (Effect)

The envelope actually returned:

```
summary: arrays: edges_missing[0], edges_stale[0], ambiguous[50], dangling[50],
                 cross_repo[48], malformed_qualifier[8], ...
hint:    report only — pass write=true to materialize/prune the cites edges above.
```

`ambiguous[50]` and `dangling[50]` read as *fifty findings*, not as *fifty of 551* and
*fifty of 637*. Nothing in `summary` or `hint` mentions truncation. To learn it you
must already suspect it and read `$.counts.truncated` — a nested key the envelope never
names.

## Reproduction

```
librarian(action="link_scan")
read_file("@tool_<id>", json_path="$.counts")
```

Compare `counts.dangling` (637) against the length of `$.dangling` (50). Then re-read
`summary` and `hint` and observe that neither says so.

## Root cause

Two correct behaviours composing into a gap, the same shape as the merge-commit
patch-id bug archived today:

1. **Capping the arrays is right.** 637 findings would blow the inline budget;
   progressive disclosure exists for exactly this.
2. **`counts.truncated` is right.** It is present, per-array, and correct.

What is missing is that the truncation is not on the surface the caller reads. The
information exists and is simply not where the decision is made — **the same failure
shape as the `git status` readback that prints after the commit it was meant to gate.**

## Evidence

### The incident

`codescout-f0` tried to verify that a new `reconnaissance-patterns:R-136` citation from
`docs/PROBES.md` resolves, by checking that `R-136` was absent from the `dangling`
bucket. It was absent. That reads as confirmation and is worth nothing — absence from a
list holding 7.8% of the dangling findings is indistinguishable from being past the cap.

They caught it themselves and downgraded the claim to *"well formed and resolvable by
construction, not verified by the scan"*. The bug is that the tool offered no signal;
catching it required knowing the cap existed.

### It violates this repo's own ADR, in the repo's own tooling

`docs/adrs/2026-08-27-negative-results-name-their-scope.md` requires a tool to **name
the scope it examined when a zero is suspicious**. `codescout`'s own `grep` already does
this well — its zero-match response volunteers *"this zero describes what was searched,
not the pattern"* and lists the excluded paths. `link_scan` does not, in a case where
the examined scope is **8%**.

## Fix

**Fixed 2026-08-31 in `4c063b4e`** (patch-id `c3adf9515a5a0b70292c1d973b993a1151affdcb`),
on `experiments`.

`librarian_compact_summary` (`src/librarian/adapter.rs`) now leads with a
`finding_truncation_summary` line naming every **cut** array as `name[shown of total]`.
Live output after `cargo rb` + `/mcp`:

```
TRUNCATED: ambiguous[50 of 551], dangling[50 of 637] — absence from a cut list is not evidence.
  18 keys: scope, write, counts, edges_missing, …
  arrays: edges_missing[2], edges_stale[0], ambiguous[50], dangling[50], …
```

It **leads** because `truncate_compact` cuts from the tail, so anything below can be
lost — the ordering rule was already documented on `librarian_compact_summary` and this
is simply the strongest incompleteness signal a librarian result carries. The generic
shape line survives **below** it rather than being displaced, per the same doc: returning
`Some` must not drop the key list a `json_path` is aimed with.

Keyed on **shape** (`counts.truncated`) rather than on the tool name, so any librarian
action adopting the convention is covered without a second edit, and it stays inert for
artifact-shaped results.

### The ranking below was wrong on the mechanism, and that is the useful part

The original list is kept verbatim because the error is instructive — the diagnosis in
this file was measured at the bytes, the remedy was not checked at all. Recorded against
`observer-blindness:OB-1` § *the remedy is the part that escapes verification*.

> 1. **Put it in `hint`** — the one field a caller always reads.
> 2. **Put it in the `summary` array line**: `dangling[50 of 637]`.
> 3. Consider a `limit`/`offset` on the finding arrays.
>
> *(1) alone closes the incident above and is a one-line change to the envelope.*

**(1) would not have worked.** `link_scan`'s `hint` is a key *inside the payload*
(`link_scan/mod.rs`), so it is buffered away by the very overflow that creates this
defect. Writing the warning there places it in the one location the caller demonstrably
does not read — and a test asserting on it would **pass while the bug stayed live**,
which is precisely the failure this file warns about two sections down for `counts`.

**(2) is right in effect but named the wrong owner.** `dangling[50]` is produced by the
shared generic describer in `src/tools/format.rs`, which renders every array for every
tool as `{k}[{len}]` and knows nothing of `counts.truncated`. Editing it there carries
repo-wide blast radius; the per-tool `format_compact` hook is the correct seam and
already existed.

**(3) remains open and unclaimed** — paging the finding arrays is still the only way a
caller can reach the other 92%. This fix makes the truncation *visible*; it does not make
the population *reachable*. `dangling_by_source` is complete (191 sources summing to 637)
and is the current workaround.
## Tests added

Two, in `src/librarian/adapter.rs`, both asserting on the **summary string** rather than
on the payload — which is what this section originally demanded, and the reason the
original fix (1) could not have been tested honestly.

- `compact_summary_names_the_real_total_for_a_truncated_finding_array`
- `compact_summary_is_silent_when_no_finding_array_was_cut`

Three mutations, each killing its own assertion:

| mutation | dies |
|---|---|
| remove the `cut.is_empty()` guard | the **silence** test |
| emit `name[shown]` without the total | `"50 of 637"`, printing `TRUNCATED: dangling[50]` beside the generic `arrays: dangling[50]` — the bug's own signature |
| accept any bool instead of `true` | the exclusion assertion |

**The silence mutation was run first, deliberately.** That test passed *vacuously* in RED
— the function returned `None` for every input — so its ability to fail was the thing in
doubt, not its result.

**The third mutation found a vacuous assertion and changed the fixture.** The original
result carried only a `dangling` array, so `ambiguous` was excluded by the
**missing-array** guard rather than by the truncated flag, and `!contains("551")` could
not fire under any mutation — it passed in GREEN *and* under the exact mutation it existed
to catch. `ambiguous` is now present and complete, so the flag is the only thing that can
exclude it, and the assertion reads the `TRUNCATED` line specifically rather than the
whole summary.
## Workarounds

Always read `$.counts.truncated` before drawing any conclusion from a finding array,
and never read absence from `ambiguous` or `dangling` as evidence. For a specific
token, check the definition directly — a `## <ID> — <title>` heading in the citing
file's ledger — rather than inferring from the scan.

## Provenance

Found by `codescout-f0` (pid 807989) while verifying a citation of mine; offered to me
rather than filed by them. Independently reproduced here before filing: the 551/637
figures and the `truncated` map are from my own run, not relayed.

## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the contract this breaks.
- `get_guide("tracker-conventions")` — documents the `counts.truncated` flag, which is
  where its existence is recorded and is not where it is needed.
- `docs/PROBES.md` rule 3 (*a zero is evidence about the search*) and rule 6
  (*propositional adjacency wants a positive control*) — this is an instance of both,
  produced by a tool rather than by a person.
