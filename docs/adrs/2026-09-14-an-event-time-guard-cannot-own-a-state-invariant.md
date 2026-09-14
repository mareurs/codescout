---
kind: adr
status: active
title: ADR-2026-09-14 — An event-time guard cannot own a state invariant
owners:
- marius
tags:
- design-principle
- audit-doc-refs
- guards
- artifact-identity
topic: tool-contracts
---

# ADR-2026-09-14 — An event-time guard cannot own a state invariant

## Status

**Accepted and shipped.** `ArtifactMissing` moved to the `High` arm of
`default_severity` once the live corpus measured 0 gating instances.

## Decision

`audit_doc_refs` at `--fail-on high` is **the** control for dead artifact-id
citations. `doc(action="move")`'s `inbound_id_citations` stays a convenience that
tells an author what to fix — never the thing that ensures it got fixed.

## Context

`id = sha256(abs_path)[:16]`, so archiving an artifact re-keys it and every
citation of the old id silently resolves to nothing. Two instruments could own
this, and they differ on an axis that is easy to miss:

| | `doc(move)`'s `inbound_id_citations` | `audit_doc_refs` at `High` |
|---|---|---|
| observes | the **event** (the re-key) | the **state** (does every cited id resolve?) |
| window | the instant the move runs | any later moment, unbounded |
| output | an informational response field | an exit code |

Seven orphaned citations were measured on 2026-09-14 and sort into three modes:

| mode | n | move-time scan | gate at High |
|---|---|---|---|
| **A** archived before the scan existed (pre-`38f7490e`, 2026-09-10) | 5 | could not see | catches |
| **B** citation **written 85s after** the move | 1 | cannot see, **by construction** | catches |
| **C** citation existed 35min before; commit shipped without the repoint | 1 | would have reported it | catches |

**Mode B is what settles it.** A move-time scan is monotone under time — it can
only observe citations that already exist. The single most likely moment to write
a citation of a bug is immediately after archiving it: `4f0a7689` archived at
08:50:45, `fec8f3c9` wrote a ledger entry citing its now-dead id at 08:52:10. The
scan's blind spot is aligned with the highest-density moment for the very event it
watches. That window cannot be widened; it is the wrong axis.

**Mode C is § *Testing Discipline*'s loudness law met in the field.** The
procedure already existed — the archive flow says to repoint the old path *and*
the old 16-hex id in the same commit — and the report already existed. Both were
in place and the citation died anyway. What is *not* claimed here: whether the
report was read. Only that it was available and the repoint did not happen.

## Alternatives considered

- **Extend the move-time scan.** Rejected: no change scenario it absorbs that the
  gate does not, and blind to mode B by construction. Adding output to a field
  with no consumer is a wall in an empty field.
- **Enforce at move time** — refuse the move while citations are live. Rejected:
  still blind to B, and it blocks a correct archive on work that belongs in the
  same commit rather than before it.
- **Exempt `docs/trackers/**` from the id check.** Rejected: the ledgers are the
  corpus's institutional memory and `CLAUDE.md` cites them. Measured repair cost
  was 6 repoints across ~5 weeks of archives. The exemption buys silence, not
  time.

## Consequences

- **now easier:** one time-independent instrument. A citation that rots for any
  reason, at any time, through any re-key path — including one that never goes
  through `doc(move)` — reds the same way.
- **now harder:** every archive that a ledger entry cites creates a repoint. It is
  mechanical (the archive preimage inverts exactly: hash `docs/issues/<basename>`
  for each file in `docs/issues/archive/` and match), but recurring, and it lands
  on whoever runs CI rather than on whoever archived.

## Change scenarios absorbed

- A bug archived by a session that never reads the move response.
- A ledger entry written minutes after its subject was archived.
- A citation that goes dead through a re-key path that does not call `doc(move)`.

## Revisit-when

- The recurring repoint cost exceeds ~1 per archive on average.
- A re-key path appears that the gate cannot see.

## Confidence

**High** on the gate as primary control. **Medium** on declining move-time
enforcement: mode C would be caught earlier, by the author who still holds the
context, and that is a real cost traded away.

## The generalizable form

An event-time guard and a state-time guard are **not** redundant, and picking the
wrong one looks like diligence — the event guard is cheaper, fires earlier, and
names the author. But it can only see the world as of its instant, so any defect
created after the event is outside its reach forever.

The tell: **ask whether this defect could be introduced one minute after the guard
ran.** If yes, the guard is an assistant and something time-independent has to be
the control.

## Note on how the precondition was measured

The tightening was gated on *"`--fail-on high` exits 0"*. That check is **vacuous
while `ArtifactMissing` sits at `med`** — no such finding can reach `high` to be
counted, so the exit code is 0 whether the corpus is clean or has 13 dead ids. It
was read as a pass alongside a second derivation that had silently been scoped to
3 files, and two instruments agreeing that way is one blind spot counted twice.

The population was 13, not 0. The only instrument that could express the failure
was the production binary with the arm already moved. **Measure a gate's
precondition with the gate armed, in a throwaway build** — and verify the throwaway
does not outlive the measurement, since `target/release/` is shared.
