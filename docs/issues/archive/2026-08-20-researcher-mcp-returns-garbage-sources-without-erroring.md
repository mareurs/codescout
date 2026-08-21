---
id: 23bd599cb74a1f12
kind: bug
status: wontfix
title: researcher MCP returns off-topic domains as sources and cites vendor marketing as evidence
tags:
- researcher-mcp
- external-tool
- measurement
- source-quality
closed: 2026-08-21
---

# BUG: `researcher` MCP returns off-topic domains as sources and cites vendor marketing as evidence

## Summary

The `researcher` MCP server (a sibling server, not codescout) returned adult-content and
parcel-tracking domains in a source list for an academic-intent query, and produced a
report whose only citations for a substantive claim were a vendor marketing page and a
notarisation service. Both results were returned as **successful** — no error, no
low-confidence signal, no empty result. A caller who does not independently re-verify
every citation will publish fabricated support.

## Symptom (Effect)

Two distinct failures in one research pass (2026-08-20), reported by the subagent that
ran it:

> *the `researcher` MCP returned adult-content and parcel-tracking sites as "sources" on
> one query*

> *The researcher MCP produced a report on "attestation theatre" whose cited sources were
> a Microsoft marketing page and a notarisation vendor — discarded as unverified.*

Exit status in both cases: success. The reports were well-formed and plausible.

## Reproduction

Not yet reproducible — the exact query strings were not preserved by the subagent's
report, and the tool was invoked inside a subagent whose transcript is a task output file
rather than a queryable log.

**Best lead:** the pass used `intent="academic"` with `mode="report"`/`"deep"` on queries
about verification/attestation of knowledge claims. The "attestation theatre" query is
named explicitly in the report and is the more valuable of the two to retry, because its
failure mode (authoritative-sounding vendor content standing in for research) is the
subtler one.

Retry path: re-run `research("attestation theatre", intent="academic", mode="report")`
and inspect the returned source domains before reading the synthesis.

## Environment

`researcher` MCP server, invoked via a `general-purpose` subagent from codescout's
`experiments` branch at `5917d72c`, 2026-08-20. Server source is outside this repo.

## Root cause

Unknown — the server is outside this repo and was not read. Two hypotheses worth
separating before any fix:

1. **Retrieval-side**: the domain filter / `domain_profile` is not applied, or is applied
   after ranking, so an unfiltered web index leaks junk domains into the source list.
2. **Synthesis-side**: sources are retrieved acceptably but the synthesis step accepts
   any fetched page as evidence, with no authority weighting for `intent="academic"`.

The two produce identical output shape and need different fixes. *Inferred from the
symptom — not measured; nothing in this repo was read to support either.*

## Evidence

The subagent's own instrument caveat, quoted verbatim in
`docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`
§ *Prior art* → *Instrument caveat*. Every citation that survived into that spec was
independently re-confirmed through a separate search or fetch, and the two that could not
be are marked inline.

## Hypotheses tried

None at filing time. **2026-08-21:** ran the file's own `## Resume` repro —
`research("attestation theatre", intent="academic", mode="report", raw_json=true)` from a
top-level session (not a subagent), twice in a row. Both calls returned
`Error: Vertex response contained no text candidate` — a hard error, not the garbage-but-successful
result the bug describes. This does not confirm or refute either retrieval-side/synthesis-side
hypothesis; it is a third, unrelated failure mode (the underlying model call itself failing),
and chasing it further would mean debugging a different defect in a repo this tracker doesn't
govern. The original symptom (silent success with off-topic sources) was not reproduced today.

## Fix

Not attempted. This is a sibling server, and the codescout-side mitigation is already in
place: the research brief that surfaced this required per-citation re-verification, and
the two unverified items are marked as such in the spec.

**The durable codescout-side lesson is already recorded** — CLAUDE.md's Measurement rule
covers exactly this shape (*"a tool's own output as much as a hand-rolled command … a
heuristic whose assumption the data violates degrades a value rather than raising"*).
What this instance adds is that it applies to *retrieval* tools, where the degraded value
is a citation, and a fabricated citation is the most publishable kind of wrong number.

**Disposition (2026-08-21):** closing as `wontfix` from codescout's side. The `researcher`
server's source lives outside this repo (`/home/marius/work/claude/researcher` on this
machine, confirmed present) — a real fix belongs to that repo's own tracker, not this one.
The codescout-side mitigation (independent per-citation re-verification before any citation
is used as evidence) is already in place and is the durable artifact of this bug. No
codescout code changes as a result of this bug.

## Tests added

N/A — no code changed, and the defect is in another repo's server.

## Workarounds

Re-verify every citation a `researcher` result returns before using it, through an
independent search or fetch. Treat a source list as a set of leads, never as evidence.
Mark anything that cannot be re-confirmed as unverified **inline, next to the claim** —
not in a footnote, because the claim is what travels.

## Resume

Re-run `research("attestation theatre", intent="academic", mode="report")` from a
top-level session (not a subagent, so the call and its raw result are inspectable), and
record the returned source domains. That separates hypothesis 1 from hypothesis 2 in one
call. If retrieval is clean and synthesis is not, the fix is in the server's ranking; if
retrieval is dirty, it is in the domain filter.

## References

- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` § Prior art
- CLAUDE.md § *Measurement — Never State a Count Your Instrument Did Not Measure*
