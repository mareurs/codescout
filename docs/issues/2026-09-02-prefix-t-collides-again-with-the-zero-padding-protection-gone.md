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

**The unit is `(source, token)` pairs — not mentions, and the difference is not cosmetic.**
`link_scan` reports one occurrence per citing artifact per token, so a file citing `T-17`
three times contributes **1**. Anyone reading `17` as "places to edit" will be short, and
the shortfall grows with how carefully a document repeats itself — `tracker-conventions.md`
alone holds two mentions inside one of these 17. `tracker-hygiene-log:HY-21` measured it
directly: a predicted −3 came back −2 because one citer's two mentions had always been a
single finding.

So `17` is the count of **broken citation sites**, which is the right number for *how much
is broken* and the wrong one for *how many edits repair it*. The second has not been
derived.

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

## Propagation surfaces — where `T-17` is used as a worked example

Raised by a peer session (`codescout-0a`), who found the first of these and framed it as
*"hints get copied"*. Verified, and the population is larger than the one they named — the
stronger instance is in a **served prompt surface**, and it is already one of the broken
citations counted above.

**1. `src/prompts/guides/tracker-conventions.md` — served to every agent** via
`include_str!` at `src/prompts/mod.rs:576`, returned by `get_guide("tracker-conventions")`.
Two uses:

- `:565`, under the heading *Citing an entry — bare, or qualified*:

  > Cite by **bare token** when the prefix has exactly one ledger: `R-98`, `HY-10`,
  > `T-17`, `CAP-5`.

  `T-17` is the guide's own example of *a prefix with exactly one ledger*, and it is one of
  the four tokens that now has two. The guide instructs, by example, the single citation
  form that cannot resolve for this token — and it is the document every session is told to
  read before touching a tracker.

- `:389`, listing example entry ids (`F-3`, `R-91`, `T-17`, `BUG-40`).

**This surface is catalogued** (`e0802ffca04e9bf7`, `kind: doc`) and **is one of the 17
ambiguous citers measured above** — `link_scan` reports it at line 390. Both occurrences
fold into that one finding, because the scanner reports one occurrence per
`(source, token)` pair rather than per mention (`tracker-hygiene-log:HY-21`). So the guide
teaching the rule is itself an instance of the rule being broken.

**2. `src/librarian/tools/update_entry.rs:53`** — the `RecoverableError` hint for a missing
required param uses `entry_id="T-17"` in its worked example. Weaker than the peer's framing
suggested, and the distinction is worth keeping: an `entry_id` is a *params-row key*, not a
prose citation, so no scanner reaches it and copying it produces a wrong-row patch rather
than a broken link. It is a bad example inside a contested namespace, not a citation vector.

**Why this belongs on this bug rather than a separate one.** CLAUDE.md's
§ *Parsers Over a Namespace* already lists *"a documentation example of citation syntax
counted as a real citation"* as an instance of this class. The guide is that sentence about
itself: it cannot demonstrate a bare citation without making one.

**Remedy, not applied here.** Drop `T-17` from the `:565` list and the `:389` list — the
other three examples in each (`R-98`, `HY-10`, `CAP-5`) are single-definer and verified so
by the same `prefix_conflicts` run, so no substitute is needed and none should be invented
without re-checking. Deliberately deferred: this is a served prompt surface with gated byte
ceilings (`a_p50_session_stays_under_the_committed_emission_byte_ceiling`,
`tool_surface_under_budget`), it is read by several concurrent sessions, and 32 commits are
unpushed on this branch. The correction is one line in each place and needs the gate run,
not a design decision.

## References

- `src/librarian/tools/link_scan/resolve.rs` — `prefix_conflicts`, and the stale
  `F-N` justification in its doc comment
- `docs/issues/archive/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md`
  — the founding case, which predicted this
- `docs/trackers/tool-usage-patterns.md`, `docs/trackers/system-retrospective-improvements.md`
- `docs/TAXONOMY.md` — indexes `T-N` as a single-owner prefix


## The rename is BLOCKED for an augmented ledger, and the fix section does not say so

Scoped 2026-09-11 to execute § *The fix is a rename*. Its two stated blockers had both cleared —
the worktree was quiet, and every original author of `system-retrospective-improvements.md` has
exited (socket enumeration at 17:49Z; the only live session that has ever committed to it is the
one writing this). **A third blocker, not named in this file, stops it anyway.**

`system-retrospective-improvements.md` is **augmented**: `entry_collection: tasks`, 17 rows keyed
`T-1` … `T-17`, under `params_schema` pattern `^T-\d+$`. A rename therefore has to move two
keyspaces, and the second one is deliberately immutable:

- **Headings** — movable. `link_scan` binds a citable token to `## PREFIX-N — <title>` and to
  nothing else, so renaming the headings is what actually fixes the ambiguity.
- **Params ids** — **not movable through any sanctioned surface.** `update_entry` rejects `id` by
  design (`src/librarian/catalog/augmentation.rs:273`, `:309`): *"Entry ids key `entry_cite` rows
  (`<slug>:<local>`), so re-keying one would strand its citations."* There is no `rename_entry`
  or `rekey_entry` action.

**And renaming only the headings is worse than doing nothing**: the 17 params rows would then
anchor ids the body no longer defines, which is `doctor`'s `entry_without_definition` — trading
one finding for seventeen. The only surface that moves both is the wholesale
`doc(action="augment", params={tasks: […]})` write, which `CLAUDE.md` § *Session Intelligence
Trackers* forbids outside its single `params_behind_body` carve-out — and that collection's
`notes` carry verified SHA + patch-id provenance (`T-14`, `T-16`), so a partial write is exactly
the 2026-08-16 loss shape.

**Not performed.** A session that reads § *The fix is a rename* and starts work will reach this
point after the scoping and not before, so it is recorded here.

### What the citation surface actually measures

`link_scan` at HEAD `37e1ba78`, 2026-09-11 — read from the instrument, not counted by eye:

| token | definers | citing sources | citing mentions |
|---|---:|---:|---:|
| `T-14` | 2 | 15 | 34 |
| `T-15` | 2 | 3 | 5 |
| `T-16` | 2 | 4 | 6 |
| `T-17` | 2 | 6 | 19 |

**But the repointing surface is far smaller than those 64 mentions, and it is smaller than this
file's own § *The fix is a rename* estimate of 17.** Classifying every LIVE mention of the four
colliding tokens — 25 of them, after excluding `docs/**/archive/**` (historical snapshots the
convention says not to rewrite) and this file plus its founding predecessor (where the tokens are
the SUBJECT, not citations):

- **Zero cite `system-retrospective-improvements`.** Every one resolves elsewhere: `tool-usage-patterns`
  verdicts (`legitimate` / `wrong-tool` is *that* ledger's schema), ids only it defines (`T-18`…`T-24`),
  meta-mentions describing this very collision, or `docs/superpowers/plans/2026-05-17-i1-refactor.md`'s
  own May task numbering — a **third** namespace, which is the founding bug's *three ledgers* still
  standing.
- **Every live citation of this ledger sits in `T-1`…`T-13`**, which does **not** collide —
  `tool-usage-patterns` spells that range zero-padded. They are concentrated in the
  catalog-audit-trail stream (session log, two plan titles, a design spec) and include two already
  **qualified** citations and one executable `update_entry(entry_id="T-1")` snippet.

**So the collision is real and currently costs nothing in resolved citations** — it is a
future-ambiguity hazard, not present breakage, and the rename's cost falls entirely on correct,
working citations in the non-colliding range. That does not argue against fixing it; it argues
that `severity: high` is carried by the *unresolvable* half (`defined_by=2` makes all 64 mentions
resolve to nothing) rather than by any wrong resolution observed today.

**What a fix would need, offered not claimed:** either an entry-id rename path that moves
`entry_cite` rows with the id — the reason the current refusal exists, so the refusal is a
missing feature rather than a wrong rule — or a decision that this ledger drops its augmentation
before renaming, which discards the `notes` provenance and is worse. The first is a real design
question and is not this file's to settle.
