---
id: ca4f9e373548c2bb
kind: bug
status: open
title: probe_tool_surface enumerates wire fields, so annotations counted as zero
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- probes
- tool-surface
- instrument
topic: measurement instruments
opened: 2026-09-03
severity: medium
unverified: 'no CI-level guard: the unmodelled-field alarm fires only on a probe run, never in the test lane'
---

## Summary

`scripts/probe_tool_surface.py` — the instrument `docs/PROBES.md` names for the
`tools/list` surface — sized each tool by **enumerating** the wire fields it knew
(`description`, `inputSchema`). When `annotations` shipped on 2026-09-03 (`71c827f9`), the
new field was not in that enumeration, so it was counted as **zero** rather than reported.
For four commits the probe published `TOTAL 55719` against the in-tree gate's `56476` — a
757-char shortfall in the number every compaction decision is measured against.

## Symptom (Effect)

Two instruments over the same payload, disagreeing, with neither raising anything:

```
$ python3 scripts/probe_tool_surface.py
tools 21   schema 48722   desc 6997   TOTAL 55719

$ cargo test --lib tool_surface_report_lengths -- --nocapture
  TOTAL (21 tools)          6997    48722    757    56476
  budget 56476, headroom 0
```

`schema` and `desc` agree exactly. The whole delta is `annot`, which the probe has no
column for. Exit code `0` from both; no warning, no diagnostic.

## Reproduction

At `0647a6da` (four commits after `71c827f9`), on `experiments`:

```
cargo build
python3 scripts/probe_tool_surface.py                                  # TOTAL 55719
cargo test --lib tool_surface_report_lengths -- --nocapture            # TOTAL 56476
```

The probe **prints its own cross-check instruction** three lines under the total —
*"must print the same three numbers … a mismatch means this probe and list_tools have
diverged, and the delta is meaningless until they agree"*. It was correct, it was
on-screen at every run, and it was never acted on.

## Environment

`experiments` @ `0647a6da`, Linux, python3, `target/debug/codescout` driven over stdio.
Not client-specific — the probe reads the real wire.

## Root cause

`main()` built its per-tool record from a **fixed field list**
(`scripts/probe_tool_surface.py`, the `surface[t["name"]] = {...}` literal), and `TOTAL`
summed exactly those keys. The set is *default-exclude*: a field absent from the literal
contributes nothing and announces nothing, because the code that would have sized it does
not exist. There is no zero to notice — the field is not a member of any population the
probe iterates.

The in-tree gate was updated in the same commit that introduced the field
(`src/server.rs`, `advertised_surface` → `(name, desc, schema, annot)`); the standalone
probe was not. Both are described in `docs/PROBES.md` as measuring the same thing.

*Measured 2026-09-03: both commands above, run back to back, at `0647a6da`.*

## Evidence

### The commit that caused it argued against this exact failure

`71c827f9`'s message reads:

```
`advertised_surface` now returns `annotation_chars` beside description and schema,
because `list_tools` attaches annotations after `input_schema()` returns, and a payload
the gate cannot see is one that can grow without limit — the failure its own doc comment
warns about.
```

The reasoning was right and was applied to one of the two instruments. This is the
§ *Observer Blindness* pattern in CLAUDE.md — *"every one was committed by an author
actively writing about that class"*. Knowing the class prevented nothing.

### The new detector, mutation-verified

With `annotations` removed from `MODELLED_WIRE_KEYS`, the probe reports rather than drops:

```
  !! 1 WIRE FIELD(S) OUTSIDE MODELLED_WIRE_KEYS, 757 chars:
       annotations             757 chars
     TOTAL does not count these unless a sizing line adds them, so treat
     it as a FLOOR and model the field before trusting a delta (trap 5).
```

Restored, the probe agrees with the gate on all four numbers.

## Hypotheses tried

1. **Hypothesis** — the binary was stale, so the probe legitimately read a pre-annotation
   surface. **Test** — compared `schema`: probe `48722`, gate `48722`, which includes the
   +200 `force` description from `19c0fc09` (two commits *after* annotations). **Verdict**
   — rejected; the binary was current and the wire carried annotations the probe ignored.

## Fix

Two changes, and the second is the one that matters:

1. `annot` added to the per-tool record, to `TOTAL`, to `--json`, and to the
   cost-per-call rows. Computed as `len(dj(annot)) if annot is not None else 0` to match
   the Rust side's `.unwrap_or(0)` exactly — an absent `annotations` is `0`, not
   `len("null")`, and getting that wrong would make the cross-check compare two different
   quantities and call them equal.
2. **The enumeration is inverted from default-exclude to default-report.**
   `MODELLED_WIRE_KEYS` names what the probe can size, and every key outside it is printed
   with its byte cost. Adding `annot` fixes today's number; this is what makes the *next*
   field (`outputSchema`, `title`, …) announce itself instead of silently subtracting.

The docstring's trap list gained trap 5 and **lost its count** — the header read
`THREE WAYS` while listing four, which is the same defect CLAUDE.md § *Testing Discipline*
names for its own laws: *"a tally of the section's own contents is a premise that every
addition falsifies"*.

Fix commit: *(recorded on archive)*

## Tests added

`MODELLED_WIRE_KEYS` + the unmodelled-field report, in
`scripts/probe_tool_surface.py`. **Mutation-verified, not merely written**: removing
`annotations` from the set produces the RED quoted under *Evidence*; restoring it produces
agreement with the gate on all four numbers.

**Naming the observer honestly, per CLAUDE.md § *Testing Discipline*** (*"Loudness is a
property of a PATH, not of a failure"*): this alarm is on stdout of a probe run, not in
CI. It fires for whoever runs the probe — which is exactly the population a silent
shortfall would mislead, so the observer is the right one, but a session that never runs
the probe is not covered. A CI-level guard would need `python3` in the test lane and is
not in place.

## Workarounds

Trust `cargo test --lib tool_surface_report_lengths -- --nocapture` over the probe's
TOTAL when the two disagree — the gate is the surface that is actually enforced.

## Resume

N/A once the gate is green and the fix is committed.

## References

- `scripts/probe_tool_surface.py` — the instrument
- `src/server.rs` § `advertised_surface` — the in-tree counterpart, and the
  `annot_chars` comment explaining why the field is counted there
- `docs/PROBES.md` — names both as measuring this surface
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- `docs/trackers/resume-tool-surface-structural-mechanisms.md` — SM-1 shipped the
  annotations this failed to count

