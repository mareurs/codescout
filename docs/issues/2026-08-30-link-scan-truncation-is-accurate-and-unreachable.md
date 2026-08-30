---
id: bcb93414c507da24
kind: bug
status: open
title: 'BUG: link_scan caps its finding arrays at 8% of the population and reports the truncation somewhere nobody reads — so absence from a bucket looks like a clean result'
tags:
- librarian
- link-scan
- negative-results
- silent-truncation
- progressive-disclosure
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

Not applied. The information already exists; only its placement is wrong. In rough
order of value:

1. **Put it in `hint`** — the one field a caller always reads.
   `"dangling: showing 50 of 637 (truncated) — absence from this list is not evidence."`
2. **Put it in the `summary` array line**: `dangling[50 of 637]` rather than
   `dangling[50]`.
3. Consider a `limit`/`offset` on the finding arrays so a caller who needs the whole
   population can page it, rather than being silently handed a prefix.

(1) alone closes the incident above and is a one-line change to the envelope.

## Tests added

None yet. A regression test should assert that a truncated bucket's truncation appears
in the **caller-visible** envelope, not merely in `counts` — the whole defect is that
asserting on `counts` passes while the bug is live.

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

