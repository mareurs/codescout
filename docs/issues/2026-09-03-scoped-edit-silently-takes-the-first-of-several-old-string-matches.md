---
id: '03556b44c4fd145f'
kind: bug
status: open
title: scoped markdown edit silently takes the first of several old_string matches
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- edit_file
- markdown
- data-loss
topic: markdown editing
opened: 2026-09-03
severity: high
unverified: not fixed; no regression test. The remedy (refuse an ambiguous old_string) is proposed, not implemented, and the corrupting call still succeeds today
---

## Summary

`edit_file` in **markdown scoped-edit** mode (`heading` + `action="edit"` +
`old_string`/`new_string`) silently replaces the **first** match when `old_string` occurs
more than once inside the target section. It reports `status: "ok"` with no ambiguity
warning, and there is no parameter that can select a later occurrence — `occurrence`
disambiguates *headings*, not `old_string`. A caller who anchors on a short, common token
gets a well-formed write to the wrong place.

## Symptom (Effect)

Observed 2026-09-03 while appending a row to a Wins Index table. The call:

```
edit_file(path="docs/trackers/prompt-surface-compaction-session-log.md",
          heading="## Wins Index", action="edit",
          old_string="---", new_string="<row>\n---")
```

Intent: the section's trailing `---` rule. `---` also occurs inside the table's separator
row, `|----|------|-------:|---------|----------------|--------|`, three characters in.
That one is first, so the write landed there:

```
| ID | Date | Impact | Pattern | Counterfactual | Status |
|| W-17 | 2026-09-03 | high | ... | validated |
----|------|-------:|---------|----------------|--------|
| W-1 | ...
```

The header row and separator are now split by an inserted row; the table no longer renders.
The response was:

```json
{"status": "ok", "wrote_to": "...", "rel_path": "..."}
```

No warning, no match count, no mention that `old_string` was not unique.

## Reproduction

Any markdown section containing a GFM table **and** a `---` horizontal rule:

1. `edit_file(path=<md>, heading=<section>, action="edit", old_string="---",
   new_string="X\n---")`
2. The table's `|----|` separator is hit, not the rule.

Deterministic — no timing or environment dependency.

## Environment

`experiments` @ `636eab37`, codescout MCP over stdio, project `codescout`.

## Root cause

**Established at the code 2026-09-03 (scout), and it is narrower and worse than the
symptom suggested.** `plan_scoped_edit` (`src/tools/markdown/edit_markdown.rs:744-847`)
resolves `old_string` by first occurrence:

```rust
while let Some(rel) = section[search_from..].find(old_string) {
    edits.push(PlannedEdit { span: mstart..mend, ... });
    search_from += rel + old_string.len().max(1);
    if !replace_all { break; }        // first match, no count, no warning
}
```

**The guard this needs already exists in the same tool, and this grammar bypasses it.**
`edit_file`'s TEXT grammar refuses an ambiguous `old_string` in two places
(`src/tools/edit_file/mod.rs`, the single-edit path ~`:975` and the batch path ~`:610`),
and the single-edit one already produces exactly the remedy this file proposed:

```rust
if match_count > 1 && !replace_all {
    let line_numbers: Vec<usize> = content.match_indices(old_string)
        .map(|(byte_offset, _)| content[..byte_offset].lines().count() + 1).collect();
    return Err(RecoverableError::with_hint(
        format!("old_string found {match_count} times (lines {lines_str}). ..."), ...));
}
```

**And `plan_scoped_edit` states the principle in its own body, 30 lines above the defect.**
Its CRLF-tolerant fallback carries:

> *"only kicks in when the exact match failed and there's exactly one tolerant match (same
> conservative uniqueness gate edit_file uses), so it never silently picks among ambiguous
> candidates"*

`if crlf_ranges.len() == 1` enforces that for the **fallback** path. The **primary**
exact-match path immediately below does the thing the comment says it never does. So the
rule is known, cited, and applied to the rarer branch.

**One site, three entry points.** `plan_scoped_edit` is the single funnel for the markdown
grammar: `perform_scoped_edit` → it, `plan_batch` → it, `edit_file`'s `action="edit"` →
`perform_scoped_edit` → it, and `doc(action="update", patch={body_edits})`
(`src/librarian/tools/update.rs:277`) → `perform_scoped_edit` → it. One fix covers all four
callers, and the "mutate once per guarded SITE" law is satisfied by one mutation because
there is genuinely one site.

**The two bugs filed today COMPOSE, and that is why this was reachable at all.** On a
stamped or augmented artifact the text grammar — the one that HAS the gate — is refused
(`docs/issues/2026-09-03-librarian-guard-refuses-text-grammar-while-promising-it-works.md`).
So for every guarded tracker in `docs/trackers/`, the only available grammar is the one
missing the ambiguity check. The guard bug is not merely adjacent; it **routes callers into
the ungated path**, which is exactly how this session reached it while editing a guarded
ledger.

**Reclassified `IC-6` → `IC-14` on this finding.** Filed from the symptom as
`cluster/addressing-without-an-escape-hatch` (*"no disambiguator exists"*); the code says the
disambiguator exists in this tool and covers a subset of the tool's grammars. That is
`cluster/guard-narrower-than-its-name` verbatim: *"the uncovered remainder is protected by
nothing, and the guard's own green result is what conceals the gap."*

*Measured 2026-09-03: the corrupting call run once against the live tracker; corrupted state
read back with `awk`; the three call sites read at the bytes during the scout.*

## Evidence

The repair had to reconstruct the separator row by hand, because the inserted content had
been spliced **into** it:

```
old: "|| W-17 | ... | validated |\n----|------|-------:|---------|----------------|--------|"
new: "|----|------|-------:|---------|----------------|--------|"
```

Byte arithmetic confirming first-match: the separator line is `|`,`-`,`-`,`-`,`-`,`|`…, so
replacing characters 1–3 (`---`) with `<row>\n---` yields `|` + row + `\n---` + `-|------…`
— exactly the observed two lines. The rule at the section's end was untouched.

## Hypotheses tried

1. **Hypothesis** — the librarian guard rejected the edit and the corruption came from
   elsewhere. **Test** — the response was `status: "ok"` and the corrupted bytes were read
   back from disk. **Verdict** — rejected; the write succeeded and did the wrong thing.

## Fix

**Shipped: refuse, do not resolve.** `plan_scoped_edit` now counts matches within the
section before planning any edit, and when `replace_all` is false and the count exceeds
one it returns a `RecoverableError` naming the count, the section, and every match's
**file-relative** line. The count is section-scoped because the edit is; the lines are
file-relative because that is the coordinate a caller navigates to.

One site covers all four callers (`perform_scoped_edit`, `plan_batch`, `edit_file`'s
heading form, `doc(action="update", patch={body_edits})`), so the *mutate once per guarded
site* rule is satisfied by one mutation — there is genuinely one site.

**`occurrence`-style selection was considered and rejected.** It lets a caller who has not
*noticed* the ambiguity keep writing to the wrong place; it preserves the defect for
exactly the population that has it. `replace_all: true` remains the escape hatch, and
expanding the anchor is the other. Refusal is also the only remedy with no silent failure
direction: it cannot break a call that currently succeeds correctly.

**Two documented contracts were updated, because the behaviour they describe is gone.**
`plan_scoped_edit`'s *"first-only, or one per non-overlapping match"* and
`perform_scoped_edit`'s *"otherwise only the first"* would have become fresh doc-vs-code
drift (`IC-11`) the moment this shipped.

**A second, adjacent defect was fixed in the same commit**, because shipping it would have
left one tool giving two answers to "which line": `edit_file`'s existing text-grammar gate
computed `content[..offset].lines().count() + 1`, which is correct only when the match
starts at a line start — Rust's `lines()` counts a partial trailing line, so a mid-line
match reported N+1. Both sites now count newlines. No test pinned the old numbers.

Fix commit: *(recorded on archive)*

## Tests added

Four, in `src/tools/markdown/tests.rs`, and **every one was verified by an observed RED
against the production path**, not by its own existence.

- `scoped_edit_refuses_an_ambiguous_old_string_and_names_every_line` — the former
  `scoped_edit_first_occurrence`, keeping its input deliberately so the retired contract
  stays visible. Asserts the count, every line, and that the neighbouring section's match
  is **not** counted.
- `scoped_edit_refuses_the_dash_anchor_that_corrupted_a_live_tracker` — the real shape
  reduced: a GFM table separator containing `---` beside a `---` rule. Also asserts
  `replace_all: true` still edits both.
- `scoped_edit_still_edits_when_the_anchor_is_unique` — the **anti-vacuity floor**. A gate
  that refused everything would satisfy both tests above; this is the only one that can
  tell those apart, and it is the one that stayed green under the disabling mutation.
- `batch_edit_ambiguous_old_string_is_refused_and_located` — the refusal REACHES a caller
  with `edits[0]` located and the line list intact. `prefix_scoped_error` has to downcast
  and preserve the `RecoverableError`; a plain `anyhow` would drop the count and lines to
  a generic hint. An alarm nothing reaches is as informative as no alarm.

`plan_scoped_edit_first_only_matches_legacy` became
`plan_scoped_edit_and_legacy_agree_on_refusal_and_on_edit` — parity between planner and
legacy wrapper still matters, but "first-only" is no longer a mode, so parity is asserted
on **both** sides of the new contract rather than only the surviving one.

**The two mutations, and what each proved:**

| mutation | result |
|---|---|
| line formula reverted to `.lines().count() + 1` | RED — `lines 6, 8` instead of `5, 8`. Proves the fixture's mid-line match actually discriminates the two formulas, which is the claim its FIXTURE NOTE makes. |
| `hits.len() > 1` → `> 1_000_000` (gate disabled) | RED on all three refusal tests; the anti-vacuity test stayed **green**. That asymmetry is the point. |

The `|---|` fixture line is annotated **on the fixture line** with what breaks if it is
tidied: widening the cell is safe, changing it to `|:-:|` silently stops testing the gate,
and moving the table to a line start makes the formula regression invisible.

## Workarounds

Anchor on a string that is unique **within the section**, not merely unique-looking. For a
table, anchor on the last row's trailing text rather than on a structural token. After any
scoped edit whose anchor was short, read the region back — the write reports `ok` either
way, so verification is the only signal.

## Resume

Decide between remedy 1 and 2 above; remedy 1 is preferred and is a pure refusal, so it
cannot corrupt anything it currently writes. The text-replacement path is in
`src/tools/markdown/`.

## References

- `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`
- `docs/trackers/prompt-surface-compaction-session-log.md` — the file corrupted and repaired
- CLAUDE.md § *Parsers Over a Namespace — owe an escape and a disambiguator*
