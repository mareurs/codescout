---
id: '289b16a704c8e87f'
kind: bug
status: fixed
title: the entry-shape table prescribes the bare count its own gate refuses
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-02
opened: 2026-09-02
severity: low
unverified: 'No regression test covers the corrected row, and none is proposed — see § Fix. Recurrence is unguarded: nothing compares a field reference against the enforcement that reads that field, so a fourth divergence in this ledger would be found the same way this one was, by accident, while doing something else.'
---

## Summary

`docs/trackers/issue-clusters.md` § *The entry shape* defines the seven fields a class entry
carries. Its `**Members:**` row read:

> the query, plus `n=<count>` and the date it was run

`tests/issue_clusters.rs::no_class_field_states_a_bare_n` refuses exactly that. Its own doc
comment names the mutation it exists to catch:

> Mutation that must kill this: reintroduce a bare `n=` into any `**Members:**` or
> `**Promotes to:**` field.

So the ledger's field reference instructed an author to write the one thing its gate is built to
reject — the authoring surface and the enforcing surface disagreeing about the same field.

## Symptom (Effect)

An author following the documented entry shape writes a bare `n=` and is refused, by a test whose
failure text explains the policy but not why the manual told them otherwise. The cost is a round
trip plus the distrust that follows finding a repo's own reference wrong; it is not silent, which
is the only thing keeping this low severity.

The sharper cost is on the other side. The gate landed with a **replacement** forcing function —
a commit adding a member must change that class's `**Members:**` prose — and the field reference
never learned about it. A reader consulting the seven-field table to find out what `**Members:**`
owes gets the retired answer, so the surface most likely to be read by someone about to write the
field is the one that had not been updated.

## Reproduction

Read the two side by side:

```
docs/trackers/issue-clusters.md   § The entry shape, `**Members:**` row
tests/issue_clusters.rs:1036      no_class_field_states_a_bare_n
scripts/pre-commit-ledger-counts.py:479-486   CHECK 1 -- no STORED count
```

## Root cause

The count policy **inverted on 2026-09-02**: storing a derived count in a file 22 classes share
made every bug filer edit it, and it went stale by concurrency rather than neglect. Three
surfaces were updated in that change — the Index blockquote, the pre-commit script and the test
— and a fourth, the field reference two sections above the Index, was not.

## Evidence

This is the **third** instance of `IC-11` inside the ledger that defines `IC-11`. The Index
blockquote already records the first two in a parenthetical: *"`cluster/doc-contradicted-by-code`,
which is `IC-11`, twice, inside the ledger that defines it."*

That the class's own defining document keeps instantiating it is not irony but a symptom with a
cause: this ledger is the surface that *describes* its own machinery, so every change to the
machinery creates a documentation obligation here, and the obligation is discharged wherever the
author happened to be reading. The Index blockquote was updated because the change was about the
Index. The field reference was not, because nobody was looking at it.

Nothing detects this shape. The gate is an **absence** assertion over class files — monotone
under removal and blind to prose in the parent ledger — and no check compares a field reference
against the enforcement that reads that field. `librarian(action="audit_doc_refs")` resolves
paths, symbols and links, not claims about what a value may contain.

## Fix

The row now states the current rule and names the check that enforces it, so a reader who
disagrees has somewhere to look. Corrected in the same commit that filed this.

**No regression test covers it**, and none is proposed here: the assertion would be a prose
comparison between a markdown table cell and a test's doc comment, which is a parser over two
namespaces and would earn its own `IC-6` entry within the month. The honest mitigation is that
the field reference now cites the check by name, which is the cheapest thing that makes the next
divergence discoverable from either side.

## Status

Fixed as to the contradiction; unverified as to recurrence, per `unverified:`.

