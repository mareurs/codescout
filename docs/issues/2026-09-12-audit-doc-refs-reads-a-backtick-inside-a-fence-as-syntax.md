---
kind: bug
status: open
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
opened: 2026-09-12
owner: marius
related: []
severity: medium
---

# BUG: `audit_doc_refs` reads a backtick INSIDE a fenced block as an inline-code delimiter

## Summary

A literal backtick inside a ```` ```fenced ```` block desynchronises `audit_doc_refs`'
scanner. Two symptoms, one cause: reported `md_line` shifts by one for refs after the
backtick, and the finding loses its `code_block` severity cap, falling back to
`policy_default`.

The fence exists precisely to say *"the bytes inside are data, not markup"* — and the
scanner reads one of those bytes as markup.

## Symptom (Effect)

1. **Line attribution off by one** for refs following the backtick within the fence.
2. **Severity cap lost** — a ref inside a fence should be capped `code_block`; the
   affected finding was `policy_default` instead.

Symptom 2 is the one with teeth. `policy_default` severity depends on `ref_kind`: the
observed case was `module_path` → `low`, harmless. A **path-shaped** token under
`policy_default` is `high`, and `high` reds CI (`CLAUDE.md` § *Parsers Over a Namespace*).
So a fenced code example that contains a backtick and a path-shaped token after it is
predicted to red CI despite being correctly fenced.

**That escalation is a PREDICTION, not a measurement.** What was measured is the cap
loss on a `module_path` ref. Nobody has yet constructed the path-shaped case.

## Reproduction

Tree `408709ea`. Scanning `docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`,
whose fence opens at line 83 and closes at 86, with line 84 holding a raw-string regex
that contains a literal backtick inside a character class.

```
librarian(action="audit_doc_refs",
          paths=["docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md"],
          emit_tracker=false)
```

**The control is what makes this a finding and not a miscount.** Three refs in one file,
two outside any fence and one inside the backtick-bearing fence:

| ref | actual line | reported `md_line` | in a fence? | severity_reason |
|---|---|---|---|---|
| `claimed.difference` | 33 | 33 | no | `policy_default` |
| `claimed.difference` | 39 | 39 | no | `policy_default` |
| `re.captures_iter` | **85** | **84** | **yes** | `policy_default` |

The two unfenced refs are attributed exactly, so the scanner is 1-indexed and correct in
general. Only the fenced one drifts, and only that fence contains a backtick. The same
finding should have read `code_block` and did not.

## Environment

- Tree `408709ea` on `experiments`.
- `librarian(action="audit_doc_refs")`, run against the live release binary built
  2026-09-12 20:06.

## Root cause

Not confirmed in code — the evidence above is black-box. The shape it implies is a
scanner that tracks inline-code state with a backtick counter that is not suspended while
inside a fenced block, so an odd backtick count within the fence flips it into a state the
fence should have made unreachable. One consumed byte explains both symptoms: the line
counter and the severity classifier read the same desynchronised state.

## Evidence

Table above, re-derivable from the single `audit_doc_refs` call. `grep -n` for each token
gives the actual lines.

## Hypotheses tried

- **"The scanner is 0-indexed"** — **rejected by the control.** Two refs in the same file
  report their exact 1-indexed lines. A uniform off-by-one would have shifted all three.
- **"`module_path` refs are never capped `code_block`"** — **not tested.** Would be
  refuted by any fenced `module_path` elsewhere in the corpus reporting `code_block`; that
  check has not been run and the entry does not assume its outcome.

## Fix

Not implemented. Suspend inline-code tracking for the span between fence open and fence
close, rather than counting backticks uniformly across the document.

## Tests added

None. The discriminating test is a two-ref fixture — one ref inside a fence containing a
literal backtick, one outside — asserting the fenced ref reports both its true line and
`code_block`. It reds on the line and on the severity independently, which is what
separates the two symptoms if they turn out to have different causes.

## Workarounds

None needed at observed severity. Do not "fix" a `high` finding on a correctly-fenced
example by unfencing or rewording it — check first whether the fence contains a backtick.

## Resume

Found while `audit_doc_refs` was scanning a bug file about a parser that cannot tell one
table from another (`docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`).
Same class, one level up: a construct meaning *"this is data"* misread as syntax.
`CLAUDE.md` § *Parsers Over a Namespace* names the pattern — *"ask what your parser's
heredoc is"* — and here the answer is its own fence.

## References

- `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator*, including
  the four-independent-shell-gates heredoc precedent.
- `docs/issues/2026-09-12-body-snapshot-row-indices-counts-rows-from-unrelated-tables.md`.
