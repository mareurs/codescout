---
id: c801b657b50c87e3
kind: tracker
status: archived
title: Tool Surface Budget — SHIPPED IN FULL 2026-08-18 (TB-N, closed record)
owners:
- marius
tags:
- resume-queue
- prompt-surfaces
- budget
- tools-list
- gate
topic: prompt-surfaces
entry_high_water_TB: 5
entry_prefix: TB
---

# Tool Surface Budget — SHIPPED IN FULL (TB-N, closed record)

**Status: ARCHIVED. There is no work in this queue.** It was opened 2026-08-28 as a
resume queue on the belief that the spec was unimplemented. That was wrong — every
component had shipped on 2026-08-18. The file is kept, corrected, because it now
documents a live gate nobody else documents, and because the way the error was made is
worth not repeating.

**Spec:** `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` (`0e0316e9036d7f16`).
**Code:** `src/server.rs` — the whole gate lives in the test module.

## The correction, and how the error was made

The sweep that opened this file searched `src/` for
`TOOL_SURFACE_BUDGET|tool_surface_budget|surface_budget|tools_list_bytes` and got zero
matches. **Those symbol names were invented, not read.** The spec's own § *Components*
names the constant `TOOL_SURFACE_CHAR_BUDGET` and the functions
`tool_surface_under_budget()` / `tool_surface_report_lengths()` / `advertised_surface()`.
`TOOL_SURFACE_CHAR_BUDGET` does not contain the substring `TOOL_SURFACE_BUDGET`, so the
grep could not have matched however much code existed.

Two things let a false zero become a durable record:

1. **A negative was accepted without a positive control.** The `grep` tool's own response
   said *"this zero describes what was searched, not the pattern"*. A search that returns
   zero for everything proves nothing; one that returns hits for a term you know exists
   and zero for the term in question proves something.
2. **The spec's `status: draft` was read as a fact about the code.** It is a fact about
   the spec's frontmatter, which nobody flipped after shipping. (Fixed 2026-08-28 —
   the spec is now `active`.)

Found by accident, running `git log -S'TOOL_SURFACE'` for an unrelated question about
work-stream dates. Nothing in the sweep would have caught it.

## What the gate is, for whoever trips it

`tool_surface_under_budget()` (`src/server.rs:2587`) sums every advertised tool's
`description` + serialized `input_schema` and asserts the total is at or under
`TOOL_SURFACE_CHAR_BUDGET` (`:2584`). It runs under plain `cargo test`.

**The rule is lower-only.** If your change trips it, you do not raise the constant — you
pay for the addition by trimming prose elsewhere, then lower the constant to the new
total. Run `cargo test --lib tool_surface_report_lengths -- --nocapture` for the per-tool
table showing where the bytes are.

## TB-1 — Components 1–3: the helper, the constant, the report

**Status:** done — `598b92f2` (2026-08-18), +135 lines to `src/server.rs`
**Valid:** dated 2026-08-28

- `advertised_surface(server)` — `src/server.rs:2512`. Returns per-tool
  `(name, description_len, schema_len)`, serialized to match the wire.
- `TOOL_SURFACE_CHAR_BUDGET` — `:2584`, and `tool_surface_under_budget()` at `:2587`.
- `tool_surface_report_lengths()` — `:2610`, the always-passing companion that prints
  the per-tool table and the remaining headroom.

All three landed as specified, in one commit, on the day the spec was written.

## TB-2 — Component 4 shipped, and paid for itself at the budget

**Status:** done — `01194e21` (2026-08-18)
**Valid:** dated 2026-08-28

The F-1 fix to `Artifact::input_schema()`: `anchor_heading` declared
(`src/librarian/tools/artifact.rs:199-202`, including the tri-field wording *"all three
or none, a partial set is refused naming what is missing"*), with `title` (`:108`) and
`body` (`:109`) re-scoped to name their `append_entry` role.

The commit subject is the design working out loud: *"advertise the append_entry section
writer, **and pay for it at the budget**"*. The spec predicted this addition would breach
the gate and instructed *"do not raise the budget"*. It did breach, and it was paid for.

## TB-3 — Component 5 shipped, at 132 chars

**Status:** done
**Valid:** dated 2026-08-28

`inject_workspace_param` (`src/server.rs:626-638`) carries a **132-character**
description, down from the 259 the spec measured. The spec's target was ~95; the resting
point is 132, which is the right call — see `resume-workspace-pinning-phase-4b-5.md` WP-3,
which argues this param is currently **under**-documented on the prose surfaces, not over.

`pinnable_tools_advertise_workspace_param` asserts presence only, so the trim did not
break it.

## TB-4 — The ratchet's value history: three steps, all downward, all on one day

**Status:** done — the gate is holding
**Valid:** dated 2026-08-28

| Date | Commit | Budget | What moved it |
|---|---|---:|---|
| 2026-08-18 | `598b92f2` | 58,572 | gate lands, set at the measured total |
| 2026-08-18 | `01194e21` | 57,148 | Component 4 added ~450 chars **and paid for them** |
| 2026-08-18 | `338f8ea7` | 56,266 | 882 chars of `artifact_augment` restatement cut — *"measured, not trimmed by eye"* |

Stable at **56,266** for ten days as of 2026-08-28.

**Note for anyone auditing this history:** `git log -S'TOOL_SURFACE_CHAR_BUDGET'` shows
only the first of these three. `-S` counts *occurrences* of a string, and changing a
constant's value does not change its occurrence count. Use `-G'TOOL_SURFACE_CHAR_BUDGET:
usize'` to see value changes. That distinction hid two thirds of this table on the first
look.

The spec's headline 58,882 figure is superseded by the 58,572 the harness actually
measured — the spec warned about exactly this (*"do not hardcode 58,882… the harness's
capability set and project fixture may advertise a different tool set"*), and the
implementation obeyed it.

## TB-5 — The measurement discipline this spec imposes on itself

**Status:** invariant — read before quoting any number from this stream
**Valid:** invariant
**Rests on:** spec § *What this measurement does NOT license*

`usage.db` spans 30 days across **25 distinct `codescout_sha` values**, 96.7% of calls
from one project and one developer. Therefore:

- **No `chars/call` ratio may be computed.** It divides today's bytes by historical counts
  taken against a substrate that changed 25 times. Bytes are a property of today; rates
  are a property of a workload; their product is a forecast, not a measurement.
- **No trim may be justified by a usage rate** until a fixed-SHA measurement exists.

**Exempt:** the cache economics, which are the standing justification for the gate. Four
Claude Code sessions (2026-07-16 → 08-18, three models) showed **100.0% cache_read** in
all four; at $0.30/M cache-read against $3/M fresh, prefix re-reading was **68% of session
cost** ($84 of $123 on `55515bc5`). That is a property of the Anthropic API, replicated
across three models, involving no codescout substrate. A short session pays double the
share of a long one (10.0% vs ~5%) — a fixed tax against a growing denominator.

## Template for new entries

```
## TB-N — <one-line title>

**Status:** open | in-progress | done | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened as a resume queue, corrected and archived the same hour

Opened on a false negative (see § *The correction*), then corrected against
`src/server.rs` and the commit history once `git log -S'TOOL_SURFACE'` surfaced
`598b92f2`. Every claim above is now anchored to a commit or a `path:line`. The spec's
`status` was flipped `draft` → `active` in the same pass, so the signal that produced the
error does not produce it again.

