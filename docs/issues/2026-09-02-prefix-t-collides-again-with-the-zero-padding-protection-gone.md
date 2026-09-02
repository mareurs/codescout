---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-02
owner: marius
related: []
severity: high
---

# `T-14`…`T-17` are defined in two ledgers, and the padding accident that kept the founding case disjoint is gone

## Symptom

`link_scan` reports `prefix_conflicts` containing `T`, declared by
`docs/trackers/tool-usage-patterns.md` (`f2ecdd76a6189efb`) and defined by **two**
artifacts:

```
T:  declared_by = [f2ecdd76a6189efb]
    defined_by  = [6f5ec09c63aef864, f2ecdd76a6189efb]
```

`6f5ec09c63aef864` is `docs/trackers/system-retrospective-improvements.md`, which
declares no `entry_prefix` at all and defines `## T-1` … `## T-17`.

**Four tokens are defined byte-identically in both files** — `T-14`, `T-15`, `T-16`,
`T-17` — and they are about entirely different things:

| token | `tool-usage-patterns.md` | `system-retrospective-improvements.md` |
|---|---|---|
| `T-14` | The ledger query that was never made | `artifact(create)` stamps an `id:` that guard-locks the file |
| `T-15` | A throwaway printing test beat reading the source | Wire the eight unannotated guides into `get_guide` |
| `T-16` | Reporting a three-command local subset as "the gate" | Overflow hint on a heading-scoped `artifact(get)` points at metadata |
| `T-17` | Following the overflow hint into an unsupported grammar | Both shell gates evaluate a per-command predicate over the whole string |

**17 live citations resolve to nothing** as a result — every one ambiguous, every one
with the same two candidates:

```
T-14 × 12,  T-16 × 1,  T-17 × 4      (ambiguous: 17, dangling: 0)
```

## Why this is a recurrence, not the original

`docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`
is the founding case, and it is `fixed`: `fable-tuning-tasks` renamed `T` → `FT`, and
`tool-usage-patterns` kept `T` as the wider-cited claimant.

That fix removed *one* co-definer. It did not, and could not, prevent the next one — and
the archived record said so in the sentence that matters:

> Their token spaces stayed disjoint only because the latter spells its first thirteen
> entries zero-padded (`T-001`…`T-013`) while its later ones are `T-14`…`T-24`, and the
> resolver matches token strings — **an accident nothing recorded or enforced.**

The accident has now stopped holding, in the only way it could: `tool-usage-patterns`
switched from padded to unpadded at `T-14`, and the new co-definer starts at `T-1` and
runs past that boundary. Every token below `T-14` is still disjoint by padding; every
token from `T-14` up collides. **The overlap is exactly the unpadded range**, which is
the founding record's own prediction coming true.

## Provenance

The colliding headings entered `system-retrospective-improvements.md` in one commit:

```
0933bc95   2026-09-01   (introduces ## T-14, T-15, T-16, T-17)
```

That is **a day before** `docs/trackers/issue-clusters/` was split out of
`issue-clusters.md`, so this is independent of that change — the split moved
`prefix_conflicts` by adding `IC`, and `T` moved it separately.

## What I could not establish

A same-day measurement recorded `prefix_conflicts: 2` (`design-backlog-session-log`
T11, 2026-09-02), which does not include `T` — yet the git record puts the collision at
2026-09-01 and both catalog rows existed by then (`6f5ec09c63aef864` created
2026-09-01 01:50). **One of those two is wrong and I have not settled which.** Either
the T11 run read a corpus that had not been reindexed since `0933bc95`, or
`prefix_conflicts` did not surface it for a reason not yet found. Do not cite either
number as a baseline until this is resolved; the git provenance above is the part that
is solid.

## Why `severity: high`

Not for the count — 17 broken citations is small. For what the count is *about*:
`tool-usage-patterns` is the ledger `CLAUDE.md` hard-codes `id_prefix="T"` for, and
`T-N` is one of the seven prefixes `docs/TAXONOMY.md` indexes. A bare `T-N` citation is
the documented form, written by every session that follows the guide, and it now
silently resolves to nothing for a quarter of the namespace.

## The fix is a rename, and it is not obviously mine to make

Precedent from the founding case: the **wider-cited claimant keeps the prefix**.
`tool-usage-patterns` declares `entry_prefix: T`, carries `entry_high_water_T: 32`, is
named in `CLAUDE.md` and `docs/TAXONOMY.md`, and holds 32 entries.
`system-retrospective-improvements` declares nothing and holds 17. So the rename falls
on the latter.

Deliberately **not done in this session**: that file is another session's live work
stream (it was modified in the working tree while this was being investigated), a
rename re-points 17 citations across many files, and doing it unannounced on a shared
checkout is the failure mode `docs/conventions/shared-checkout-commit-sequence.md`
exists to prevent. Filed for its owner instead.

## What this says about the check

`prefix_conflicts` currently reports 4, and the four are not one population:

| prefix | shape | real? |
|---|---|---|
| `F`, `W` | six session logs declare, seventeen define — the documented per-work-stream convention | **no**, structural |
| `IC` | one declarer, 22 companion class files, one counter | **no**, structural by design |
| `T` | two ledgers, one namespace, four overlapping tokens | **yes** |

Three of four are structural, and the two oldest were already recorded as such —
`resume-cross-machine-catalog-restore` (2026-08-28: *"structural, not drift"*) and
`design-backlog-session-log` T11 (2026-09-02). **A baseline bumped to 4 would bury the
one real finding inside the three structural ones**, and the next genuine collision
would move it 4 → 5, indistinguishable from a sixth session log or a second
companion-file ledger. Re-baseline to **3 after the rename**, never to 4.

Note also that `resolve.rs`'s own doc comment justifies the `declared` half of the rule
with *"Eight session logs define `F-N` and none declares `entry_prefix`"* — six of them
now do. The exemption was keyed on a property that `get_guide("tracker-conventions")`
later instructed every ledger author to set, so the comment describes a corpus that no
longer exists.

## References

- `src/librarian/tools/link_scan/resolve.rs` — `prefix_conflicts`, and the stale
  `F-N` justification in its doc comment
- `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`
  — the founding case, which predicted this
- `docs/trackers/tool-usage-patterns.md`, `docs/trackers/system-retrospective-improvements.md`
- `docs/TAXONOMY.md` — indexes `T-N` as a single-owner prefix
