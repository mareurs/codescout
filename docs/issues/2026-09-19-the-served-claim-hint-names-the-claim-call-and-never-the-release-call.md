---
id: 9a0157e63fe559b8
kind: bug
status: open
title: 'BUG: doc(update) persists ArtifactData''s __delete__ sentinel into frontmatter as literal data instead of deleting or refusing'
owners:
- marius
tags:
- cluster/unclassified
topic: artifact frontmatter deletion protocols
---

## Summary

`doc(action="find")`'s `claimable` hint serves a **complete, concrete** claim call and an
**incomplete, prose** release instruction. The claim writes three fields; the release names
one. The reader has to invent the other two, and every wrong invention lands as *data*
rather than as an error, because `extra` is arbitrary YAML by contract.

Half a lifecycle on a served surface, and the missing half is the one with the silent
failure mode.

## Symptom (Effect)

`src/librarian/tools/find.rs` formats this and fills the sessionId in for you:

```
doc(action="update", id="<id>", patch={"status": "taken", "extra": {"claimed_by": "<sid>", "claimed_at": "<today>"}})
```

and beside it the note ends: *"Release with status `investigating` when you stop."*

That sentence is about `status`. It says nothing about `claimed_by` or `claimed_at` — the
two keys the call it sits next to just wrote. So a reader who follows the served surface
exactly claims with three fields and releases with one, leaving two stale claim keys on
the artifact.

## Reproduction

2026-09-19, eleven bug files in one release sweep. Having claimed via the served call, the
release was invented — and the invention used the deletion sentinel of a **different tool
in the same session** (the harness's Artifact/ArtifactData tooling, which deletes a field
by writing `{"__delete__": true}` as its value):

```
doc(action="update", id="980b98ef4dc66eac", patch={"extra": {"claimed_by": {"__delete__": true}}})
```

Every call returned `updated: true`. The frontmatter became:

```yaml
claimed_by:
  __delete__: true
```

The key is still there, the claim is still live to any reader, and the value now looks like
a pending deletion that nothing will ever honour. `doc(action="update", …, patch={"extra":
{"claimed_by": null}})` removes it correctly.

## Environment

codescout `experiments` at `b71b2407`.

## Root cause

**Not an ambiguity inside codescout, and this file said otherwise in its first draft.**
`git grep -l '__delete__'` over the tracked tree returns exactly one file — this one.
codescout has a single deletion convention, stated in `patch`'s schema (*"`extra` is a map
of custom frontmatter keys … a `null` value deletes a key"*) and identical to the RFC 7396
`null` that `params` already uses. There is no second sentinel here to disambiguate against,
so "make the two agree" is not an available fix.

`doc()` did exactly what it was told. `extra` is contractually arbitrary YAML, a map is a
legal value for a custom key, and `updated: true` was honest.

The gap is upstream, on the **served hint**. It names the claim call concretely — including
the sessionId, on the stated reasoning that *"the sessionId is the part they cannot cheaply
supply, so it is the part that must arrive concrete"* — and then hands back the release as
a sentence about a different field. The one call it spells out is the one with the loud
failure mode; the one it leaves to the reader is the one where a wrong guess is written
rather than refused.

## Evidence

Eleven files written that way in a single sweep, every call reporting success. Found by
`grep -l "claimed_by" docs/issues/*.md` afterwards — a check run because the campaign wanted
a claim count, not because anything looked wrong.

Had the sweep not been followed by a count, eleven finished bugs would have kept live-looking
claims from an exited session, and `librarian(action="doctor")`'s `claim_liveness` check
would have reported eleven dead claims at its next run: a true finding pointing at the wrong
cause.

The null path is confirmed working by an independent party — a peer session's release on
`4dcca1d8309fa224` used `extra={"claimed_by": null, "claimed_at": null}` and left no claim
fields behind. So the correct form exists and works; it is simply never printed.

## Hypotheses tried

- **"Two deletion sentinels collide inside one MCP server."** FALSIFIED by
  `git grep -l '__delete__'` returning one file, this one. The sentinel is the harness's,
  not codescout's; this was a cross-TOOL carry-over, not an intra-server collision. Raised
  by a peer session against this file's first draft and verified at the bytes before
  rewriting — the wrong framing would have sent the fix to a non-existent ambiguity.
- **"`doc()` silently dropped the field."** FALSIFIED by reading the file — it wrote it. A
  dropped field leaves the frontmatter unchanged, which is a different and cheaper failure.

## Fix

Not fixed. The direction that matches the defect: **print the release call, not a sentence
about it.** The hint already formats one concrete call; formatting its inverse costs the
same and removes the invention step entirely:

```
doc(action="update", id="<id>", patch={"status": "investigating", "extra": {"claimed_by": null, "claimed_at": null}})
```

A secondary, narrower guard is available and is **not** a substitute: `doc()` could refuse
an `extra` value of the exact shape `{"__delete__": …}`, naming `null`. That catches this
particular wrong guess and no other, which is why it is second — the reader is guessing
because the surface asked them to, and only the printed call stops that. Do not widen it to
"reject any object-valued `extra`": nested maps are legitimate values under the
round-trip-safety contract.

## Tests added

None.

## Workarounds

Release with `extra={"claimed_by": null, "claimed_at": null}` alongside the status change.
After any release sweep, verify with `grep -l "claimed_by" docs/issues/*.md` — the absence
of an error is not evidence the field is gone.

## Resume

Open. Locus: `src/librarian/tools/find.rs`, the `claimable` hint's `note` and `call` fields.
The change is to emit a second formatted call rather than to extend the prose.

## References

- `src/librarian/tools/find.rs` — the served claim hint, and the comment explaining why the
  sessionId arrives concrete while `<id>` stays a placeholder.
- `docs/issues/_TEMPLATE.md` — prescribes the claim through `extra`, and is silent on
  releasing in the same way.
- `get_guide("librarian")` § *The shrink guard, `force`, and `patch`'s accepted keys* —
  where `null` is documented.
