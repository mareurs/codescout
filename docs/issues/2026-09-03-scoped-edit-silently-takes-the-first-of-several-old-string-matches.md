---
id: '03556b44c4fd145f'
kind: bug
status: open
title: scoped markdown edit silently takes the first of several old_string matches
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
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

Text replacement inside a scoped markdown edit resolves `old_string` by first occurrence
and does not check uniqueness. The addressing scheme therefore has **no disambiguator** for
a repeated `old_string`: `replace_all` changes *how many* are replaced, never *which one*,
and `occurrence` is documented as *"1-indexed selector when `heading` matches several
sections"* — the heading axis, not this one.

`---` is a token this repo's own markdown makes ambiguous by construction: it is a GFM
horizontal rule, a frontmatter delimiter, and a substring of every table separator row.
That is the `cluster/addressing-without-an-escape-hatch` disambiguator half, in a tool that
**writes** rather than one that parses.

*Measured 2026-09-03: the call above, run once against the live tracker; corrupted state
read back with `awk 'NR>=85 && NR<=90'` and repaired in the same session.*

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

Not fixed. Two candidate remedies, in preference order:

1. **Refuse an ambiguous `old_string`** the way Claude Code's native `Edit` does, naming
   the match count and the line of each — the caller then re-anchors on something unique.
   This is the disambiguator the class asks for and needs no new parameter.
2. Accept an `occurrence`-style selector on the text axis. Weaker: it lets a caller who
   *has not noticed* the ambiguity keep writing to the wrong place.

`replace_all=true` is not a fix — it would have written the row into *both* sites.

## Tests added

None — not fixed. The reproduction above is deterministic and needs no fixture beyond a
markdown section holding a table and a rule.

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

