---
id: ea302e013cf7c85a
kind: bug
status: fixed
title: 'BUG: CLAUDE.md''s "never hand-build a params array" rests on a premise doctor''s own repair path refutes'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
topic: tracker params integrity
closed: 2026-09-11
opened: 2026-09-11
related: []
severity: high
---

## Summary

`doctor`'s `params_behind_body` check prescribes, as the **only** repair, the exact
`doc(action="augment", merge=true, augment={params: …})` call that `CLAUDE.md` § *Session
Intelligence Trackers* forbids absolutely — and the justification `CLAUDE.md` gives for the
prohibition (*"`append_entry` / `update_entry` exist so it is never needed"*) is the precise
claim `doctor`'s own remedy text refutes. A session obeying both surfaces cannot repair the
finding at all; a session splitting the difference performs the call that already cost this
very tracker 19 of 20 entries.

## Symptom (Effect)

`librarian(action="doctor")` reports, against `docs/trackers/tool-usage-patterns.md`:

```
"check": "params_behind_body",
"detail": "1 of 34 `observations` ids are anchored in the body but have no row in `params`:
 T-34. The body ran ahead of the catalog, so these entries are absent from `entry_filter` and
 every params-based query, and the committed body is their only record. Neither `append_entry`
 nor `update_entry` can repair it: the first always allocates the next free id (folding the
 body's max in), so it mints a NEW row rather than the missing ones, and the second only
 patches a row that already exists. Write the whole collection instead —
 `doc(action=\"augment\", id=…, merge=true, augment={params: {\"observations\": [ …every row… ]}})`,
 or the CLI's `--params @<file>` past the inline budget — and note a params patch REPLACES the
 array, so a partial one drops the rest."
```

`CLAUDE.md` § *Session Intelligence Trackers* states the opposite, as one of two rules it
flags as carrying data-loss consequences:

```
⚠ Never hand-build a params array. doc(action="augment", id=…, merge=true,
augment={params:{observations: [...everything...]}}) replaces the collection rather than
merging into it, and the catalog is not in git. That call took the T-N queue from 19 entries
to 1 on 2026-08-16. append_entry / update_entry exist so it is never needed.
```

Both name the same artifact: the `T-N` queue **is** `docs/trackers/tool-usage-patterns.md`.

## Reproduction

```
librarian(action="doctor")
# read summary.by_check.params_behind_body  -> 1
# read the violation's `detail` field
```
Measured 2026-09-11 at HEAD `1a34a131`, branch `experiments`. Reproduced twice in one session
(byte-identical report across a peer's three intervening commits).

## Environment

codescout `experiments`, catalog built on this host (`ripper-65e654`). Not host-specific —
both surfaces are in git.

## Root cause

Not a defect in either mechanism; a defect in a **premise**. `CLAUDE.md`'s prohibition is
absolute *because* it asserts a covering claim — `append_entry` / `update_entry` handle every
case, so the replacing call is never needed. That claim is true for the case the rule was
written from (2026-08-16: someone bulk-rewrote a collection they could have appended to) and
false for `params_behind_body`, where the body holds ids `params` does not, and:

- `append_entry` folds the body's max into its allocation, so it mints a **new** id rather
  than backfilling the missing one;
- `update_entry` patches only rows that already exist.

So the one finding whose repair genuinely requires the full-array write is the one the rule
forbids without carve-out. Neither surface cites the other: the ⚠ lives in `CLAUDE.md`, the
refutation lives in a `detail` string emitted at runtime by
`scan_params_behind_body` in `src/librarian/tools/doctor.rs`, and nothing reconciles them.
*Inferred from both texts and confirmed by running the check — the failure mode itself is not
re-measured here, because doing so means performing the destructive call.*

## Evidence

### Both surfaces, side by side

Quoted verbatim above. The overlap is exact, down to the `observations` key and the
`merge=true` flag.

### The prohibition's own evidence is this tracker

`CLAUDE.md` cites the 2026-08-16 incident as *"took the T-N queue from 19 entries to 1"*.
`doctor`'s finding is against `docs/trackers/tool-usage-patterns.md`, which is the T-N queue.
So the remedy is prescribed **on the artifact the prohibition's evidence comes from**.


### doctor's remedy is not an oversight — it is test-pinned

`params_behind_body_names_a_remedy_that_can_actually_repair_it`
(`src/librarian/tools/doctor.rs:13401`) asserts that the `detail` string **must** name the
forbidden call:

```rust
assert!(
    detail.contains("doc(action=\"augment\""),
    "must name the wholesale params write — the only surface that can create a row \
         at a GIVEN id: {detail}"
);
```

Its doc comment states the reasoning and forecloses the easy fix: *"`append_entry`
unconditionally overwrites `entry["id"]` with the id it allocates … `update_entry` patches a
row that already exists and is pinned never to change the row count. Naming either is worse
than naming nothing: it sends the reader to a tool that reports success and repairs nothing."*

So the two surfaces are hard-wired in opposite directions: a test **requires** `doctor` to
name the call, and `CLAUDE.md` **forbids** it without exception. Neither side is drifting.

The same doc comment also records that this test **replaced** one asserting
`detail.contains("append_entry")`, which *"could not tell 'append_entry is the fix' from
'append_entry cannot fix this' — it passes under both readings, and the wrong one shipped."*
That is § *Testing Discipline*'s monotone-assertion law, already paid for on this exact
string.
## Hypotheses tried

1. **Hypothesis:** `doctor` is simply wrong and `append_entry` can backfill a missing id.
   **Test:** read the `append_entry` contract in `get_guide("librarian")` and the tool schema
   — `id_prefix` allocation is described as *"computed from the live max across both existing
   params entries and ids the markdown body already claims"*. Then read
   `params_behind_body_names_a_remedy_that_can_actually_repair_it`
   (`src/librarian/tools/doctor.rs:13401`), which asserts the same thing as an invariant and
   names the arithmetic: `append_entry` allocates `params_next.max(body_max + 1)`.
   **Verdict:** rejected, twice over. `doctor` is right, and its being right is test-pinned.
2. **Hypothesis:** `CLAUDE.md` is simply wrong and the ⚠ should be deleted.
   **Verdict:** rejected. The ⚠ describes a real, measured loss and is correct for the case it
   was written from. The defect is the **absolute** form, not the rule.
3. **Hypothesis:** this is `IC-15` (*a parameter is accepted then silently dropped*).
   **Verdict:** rejected — nothing is silently dropped; both surfaces state their semantics
   correctly. The disagreement is about **necessity**, which is `IC-11`'s shape.

## Fix

Not applied. Three candidate directions, in preference order — this is a plan, not a
prescription, and the choice is the maintainer's:

1. **Carve out the rule at its own site.** `CLAUDE.md`'s ⚠ gains one clause: the full-array
   write is forbidden *except* as `params_behind_body`'s repair, where it is the only path —
   and then only with the whole collection read back first. Cheapest, and puts the exception
   where the reader of the prohibition will meet it.
2. **Make `doctor` name the conflict.** The `detail` string already warns that a partial patch
   drops the rest; it could add that `CLAUDE.md` forbids this call generally and that this
   finding is the documented exception. This is `bug-claim-liveness-session-log:F-3`'s remedy
   shape — a check whose prose names its own known conflict. **Constraint:** the test at
   `src/librarian/tools/doctor.rs:13401` pins the *presence* of `doc(action="augment"` in the
   detail, so this direction must **add** to the string, never substitute for it.
3. **Close the gap in the tool.** An `append_entry` variant that backfills a *named* id rather
   than allocating the next one would make the ⚠ true again as written. Largest change;
   removes the exception rather than documenting it, and is the only direction that lets the
   pinning test at `:13401` be retired rather than amended.

Directions 1 and 2 are complements, not alternatives: the rule and the finding are read by
different people at different times.

## Tests added

**Shipped 2026-09-11 — two guards, both shape rather than wording, both mutation-verified
against the PRODUCTION path rather than their own inputs.** § *Testing Discipline* demands an
observed RED, never an assertion's existence, so each was run against a reverted fix:

| guard | mutation applied | result |
|---|---|---|
| `claude_md_params_array_rule_names_its_one_exception` (`src/prompts/mod.rs`) | reverted `CLAUDE.md`'s ⚠ to its absolute pre-fix form | **RED** — and the panic prints the offending blockquote, so the next reader need not reconstruct what is missing |
| the `CLAUDE.md` assertion added to `params_behind_body_names_a_remedy_that_can_actually_repair_it` (`src/librarian/tools/doctor.rs`) | removed the cross-reference from the `detail` string | **RED**, printing the full detail |

Both assert *shape*: that the exception is NAMED, never how it is phrased. Pinning either
sentence would red on every legitimate rewrite — the failure mode § *Testing Discipline* calls
out for remedy text — while these red on the one regression that is actually plausible,
someone tidying the ⚠ back to its absolute form.

The first scopes its search to the ⚠'s blockquote by `>` continuation. **That scoping is
redundant today** — `params_behind_body` occurs exactly once in `CLAUDE.md`, inside the
carve-out itself — and is written that way because the redundancy is the half that decays: the
day anything else in the file mentions the check, a file-wide `contains` silently stops
discriminating and nothing reports that it has.

**Also verified at runtime, which the unit tests cannot give.** After a rebuild, the served
`librarian(action="doctor")` output carries `CLAUDE.md` and `one exception` **and still
carries** `doc(action="augment"` — confirming the addition did not displace the half pinned by
the pre-existing test. A claim about how a tool behaves needs the call run once and the real
output read.
## Workarounds

Leave `T-34` params-absent. The body is committed and is the entry's record; `entry_filter`
and params-based queries do not see it, which is the stated cost. `doctor` will keep
reporting it — that is the correct state until the rule is carved out, and it is preferable to
either horn of the dilemma.

## Resume

Decide between Fix directions 1 and 2 (they compose). If 1: edit the ⚠ in `CLAUDE.md`
§ *Session Intelligence Trackers* — note it sits inside a blockquote the prompt-surface gate
does not read, so no `ONBOARDING_VERSION` bump is implied. If 2: `scan_params_behind_body` in
`src/librarian/tools/doctor.rs` owns the `detail` string.

## References

- `CLAUDE.md` § *Session Intelligence Trackers* — the ⚠ block
- `src/librarian/tools/doctor.rs` — `scan_params_behind_body`
- `docs/trackers/tool-usage-patterns.md` — the artifact both surfaces name
- `docs/trackers/bug-claim-liveness-session-log.md` — `F-3` (doctor `detail` text asserting a
  false cause) and `W-1` (the sweep this was found in)


## Fix provenance

Fixed on `experiments` by **directions 1 and 2 together** — complements, not alternatives,
because the rule and the finding are read by different people at different times. Direction 3
(an `append_entry` variant that backfills a *named* id) was **not** taken: it would remove the
exception rather than document it, and is the only path that would let the pinning test in
`src/librarian/tools/doctor.rs` be retired rather than amended. It stays available if the
carve-out proves too subtle in practice.

- **SHA:** `aa39c1e0` (`experiments`)
- **patch-id:** `9f5c1eaa27ba8a6ec8177526307f3b2681c1fc7d`

The SHA is positional and dies when `experiments` is rebased; the patch-id is a content hash
of the diff and survives rebase and cherry-pick. Recorded as a pair at fix time, so there is
no promotion path to check and nothing owed later.

**One caveat, stated rather than left for a reader to discover.** The gate was **red** when
this landed, on a single failure that is not from this change and predates it at `1fe07709`:
`every_open_bug_file_declares_one_known_defect_class`, against another session's committed bug
file carrying no `cluster/` tag. `fmt-mine` and `clippy` were both `0`, none of the three
changed files feeds that test, and both guards above passed in the default lane. Landed on the
operator's explicit call rather than holding verified work in a shared working tree.
