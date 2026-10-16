---
id: '9928e5450ba5b859'
kind: bug
status: open
title: 'BUG: the artifact-id ADR promises full fidelity on any developer machine; a checkout at another path gets the CI false positives'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
- adr
- documentation
- librarian
opened: 2026-10-16
owner: marius
related:
- docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md
severity: low
---

# BUG: the artifact-id ADR promises full fidelity on any developer machine; a checkout at another path gets the CI false positives

## Summary
`docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md` says the dead-artifact-id check runs "only where a machine-wide librarian catalog exists — a developer machine" and that it "keeps full fidelity where it runs — on a developer machine". The code disables the check only when the catalog is empty. Nothing compares the checkout's absolute path with the citing authors', so a developer whose checkout sits at a different path has a populated catalog with different ids and gets the false-positive wall the ADR's own measurement shows for a clone at another path.

## Symptom (Effect)
On such a machine `audit_doc_refs` reports live artifact-id citations as dead, while the ADR tells the reader the check is trustworthy on any developer machine.

## Reproduction
At `a16f7d42`:
1. ADR lines 8-9 ("runs **only where a machine-wide librarian catalog exists** — a developer machine") and 71-72 ("The check keeps full fidelity where it runs — on a developer machine it resolves cross-repo ids correctly").
2. `src/librarian/tools/audit_doc_refs/mod.rs:519-523`: `fn live_ids_or_disabled(ids) -> Option<…> { (!ids.is_empty()).then_some(ids) }` — the only condition.
3. The ADR's own property 2 and its measurement row "a clone at a different path | 1682 | 50 — the display cap, all `artifact_id`".

## Environment
codescout, branch experiments, HEAD `a16f7d42`. ADR accepted.

## Root cause
The ADR states the fidelity for the frame "the machine and path the ids were minted at" under the scope word "a developer machine". Its property 2 is exactly the reason the narrower frame matters.

## Evidence
See Reproduction. Also in the same ADR, a one-number inconsistency recorded here rather than in its own file: lines 22, 24 and 56 say "8" cross-repo citations / "8 live counter-examples", line 39 says "9 — all cross-repo"; the code comment in `src/cli/audit_doc_refs.rs` and commit `66cce698` say 9. Provenance: both found by the effort-steering Stage A critic experiment (on the ADR at its own commit `66cce698`), verified there, and re-checked at HEAD on 2026-10-16.

## Hypotheses tried
N/A.

## Fix
Not started. Qualify the fidelity claim ("on the machine and at the path where the ids were minted"), and reconcile 8 versus 9 (or say what each counts).

## Tests added
N/A — documentation.

## Workarounds
N/A.

## Resume
Edit the ADR's Decision, Consequences and the 8/9 counts.

## References
- `docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md`
- `src/librarian/tools/audit_doc_refs/mod.rs` (`live_ids_or_disabled`)
