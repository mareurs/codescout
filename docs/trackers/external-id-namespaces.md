---
id: b6ffe823c76a5b80
kind: tracker
status: active
title: Id namespaces whose entries live outside the ledger heading grammar
tags:
- id-namespaces
- doctor
external_prefix:
  RTD: docs/evals/rule-tell-detection.md
  SG: docs/archive/2026-03-05-TODO-review.md
  CC: docs/archive/2026-03-05-TODO-review.md
  MF: docs/archive/2026-03-05-TODO-review.md
  SF: docs/archive/2026-03-05-TODO-review.md
---

# Id namespaces whose entries live outside the ledger heading grammar

`librarian(action="doctor")` reports `cited_prefix_with_no_definer` for an id prefix that is cited
but that no artifact defines (a `## <ID> — <title>` heading) or declares (`entry_prefix`). This
page is the answer for prefixes whose ids are real and owned, in a shape that grammar cannot
read. It declares each one in the frontmatter's `external_prefix` map, which makes `doctor` stay
silent for it for as long as the owning file still holds an id of that prefix.

It is a declaration, not a definition: it changes nothing about how a citation of these ids
resolves, and it must not be turned into `entry_prefix`, which would make the prefix known to the
resolver and could turn every citation of it into a dangling one.

## What is declared

| prefix | owner | what the ids are | why a heading would be wrong |
|---|---|---|---|
| `RTD` | `docs/evals/rule-tell-detection.md` | the rule-tell detection cases, one `### Case RTD-<n> — …` heading each | The leading word `Case` keeps the heading out of the definition grammar, and renaming the headings is not available: `scripts/measure/judge.py`, `scripts/phase1-rule-selection.py` and `scripts/blind-rule-tell-tasks.py` parse that literal form, and frozen review files cite the document by line number. |
| `MF` | `docs/archive/2026-03-05-TODO-review.md` | the Must-Fix findings of the 2026-03-05 code review | Checklist items (`- [x] **MF-1: …**`), not headings, in an archived document. |
| `SF` | same | the Should-Fix findings of that review | same |
| `SG` | same | the Suggestions of that review | same |
| `CC` | same | the Cross-Cutting Recommendations of that review | same |

## What is deliberately not declared

`KT`, `O`, `SEP` and `DRV` are also reported by `doctor`. None is declared, each for its own
reason, established 2026-10-01 by reading every citation:

- `SEP` is the MCP specification's proposal numbering (a SEP-<n> in the guide-ledger design spec and
  in the resume-tool-surface page). It is an external standard's namespace, not this corpus's, and
  no file here owns it, so declaring one as its owner would be false.
- `O` is a set of option labels (O-<n>) inside one archived design document, cited by that
  document's session log. That is document-local labelling, not a namespace; the volume-gate bug
  `docs/issues/archive/2026-09-01-citation-volume-gate-selects-for-the-prose-it-excludes.md`
  already files it as "option labels — borderline".
- `KT` has two unrelated uses: a table of Kotlin test cases (KT-<n>, up to four rows) in
  `docs/superpowers/specs/2026-04-20-impl-block-symbol-cluster-fix.md`, and a larger KT-<n> in the
  prompt-surface measurement log that no file in the repo defines. One declaration would
  misattribute the other.
- `DRV` is the id set of a probe's scratch ledger that no longer exists anywhere.

Add a prefix to this page only after confirming its owner file holds ids of that prefix and that
the prefix has one meaning.
