---
id: 9a0157e63fe559b8
kind: bug
status: open
title: 'BUG: the served claim hint names the claim call concretely and the release call not at all'
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

**The title carried the retracted framing until 2026-09-19, after the body retraction landed**, and that is this file's own defect one layer up. `doc(action="find")` returns `title`, not bodies, and this file has no H1 — so `title:` was its only headline, and every list query, including the triage queries that found this bug, rendered the framing § *Root cause* had already withdrawn. The correction was complete on the surface written and absent from the surface read. Caught by a peer session reading the frontmatter rather than the prose.

- **"Two deletion sentinels collide inside one MCP server."** FALSIFIED by
  `git grep -l '__delete__'` returning one file, this one. The sentinel is the harness's,
  not codescout's; this was a cross-TOOL carry-over, not an intra-server collision. Independently
  falsified TWICE, by different commands over different scopes: a tracked-tree
  `git grep -l '__delete__'` returning 0, and a scoped search of `src/**/*.rs` plus positive
  identification of the real convention — RFC 7396 `null`, implemented at
  `src/librarian/catalog/augmentation.rs:426`. Only the first was sent; the second is
  corroboration after the fact. **The second is the stronger half**, and the asymmetry is
  the reusable part: an absence claim is monotone under "I searched the wrong place", while
  a located implementation is positive and re-checkable at the line. METHODS are recorded
  rather than parties — a session name is registry-minted and re-minted by compaction,
  resume or a restart under another profile, so it decays while the commands stay runnable.
- **"`doc()` silently dropped the field."** FALSIFIED by reading the file — it wrote it. A
  dropped field leaves the frontmatter unchanged, which is a different and cheaper failure.

## Fix

**Precedent for the narrower guard, which strengthens it beyond what this file first argued:** `extra` is NOT a contractually-anything-goes field today. It already refuses reserved keys, and `src/librarian/tools/update.rs:2162` asserts "a reserved key is refused whatever its value". So refusing the `{"__delete__": …}` shape is an ADDITION to an existing refusal set, not a new kind of validation imposed on a free field — which answers the obvious objection that round-trip-safety means `extra` must accept everything. (Supplied by a peer session; verified at the cited line.)

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


**A general check was proposed from how this file's own stale title was caught, MEASURED, and found not viable in its naive form — recorded so nobody builds it.**

The detector was not diligence. `doc(action="find")` returns `title` and `abs_path` adjacent in one row, and here they disagreed: the FILENAME already carried the corrected framing while the TITLE carried the retracted one, so the row was self-contradicting on its face. A reader who opened the file would have seen only the corrected body and moved on. The list view put two fields derived from one understanding side by side, where a mismatch is visible without opening anything.

That suggests a corpus check: stem and `title:` are authored from the same understanding, so disagreement means one is stale. Measured over all **922** bug files, flagging any whose stem shares under a third of its >3-char tokens with its title: **46 fire, and NONE is the defect this file had.**

The first explanation — terse legacy slugs against full-sentence titles — was itself wrong and was discarded on the numbers: the 46 span 2026-04 through 2026-09 with **8 in the most recent month**, and their median stem length (5.5 tokens) matches the corpus (6). Inspecting those 8 shows every one is PARAPHRASE rather than staleness — `a-filename-matching-an-ack-handle-is-unreachable` against "a file whose name matches `@ack_<8hex>` is unreachable" is one claim in two vocabularies.

So token overlap detects DISAGREEMENT and cannot tell which side is stale — and disagreement is the normal case, because a title is a prose refinement of a stem rather than a restatement of it. At 46 hits and no true positives the ratio is unusable. Note this file's own instance runs the OTHER way (stem correct, title stale), which a symmetric similarity score cannot express at all.

**A second proxy was proposed — ask GIT rather than the text — and it is ALSO falsified. Both are recorded, because a file carrying one dead proxy and one plausible successor is what sends the next reader to build the successor.**

The form: flag a commit that edits `title:` without renaming the file, or renames without editing `title:`. Two arms, both measured.

*Arm 1, renames without a title edit.* **419** commits renamed a `docs/issues` file since 2026-04-01; of 60 sampled, **59 renamed without touching `title:` — 98%.** Not sloppiness: the dominant rename in this corpus is ARCHIVING via `doc(action="move")`, which legitimately keeps the title. The same unusable ratio as the token check, reached by a different route.

*Arm 2, and it is decisive because it is this file.* `git log --follow --name-status` shows **A** at `8f1dfbd2`, then `M` at `d17453bc`, then `M` at `eab0c2e9`. No rename, ever. Title lines changed: 1 at creation, **2 at `d17453bc` — which is the REPAIR** — and 0 after. So across the entire defect window, `8f1dfbd2` to `d17453bc`, neither arm fires; the arm that does fire, fires on the fix. **The check detects the repair and is silent on the bug.**

**WHY BOTH FAIL, and the reason is one level below either.** Both assume stem and title once AGREED in a recorded state and later drifted, so history or similarity can locate the divergence. Here the rename DID happen — `doc(action="move")` renamed the stem and left the title — but it happened between `doc(action="create")` and the first `git commit`. Git history begins at the first commit, so the divergence is PRE-HISTORIC: the only state git ever saw was the already-inconsistent one. In this workflow — create, revise, rename, commit — that window is where most authoring happens, which makes it exactly where divergences are introduced and exactly what no history-based check can reach.

So similarity sees a disagreement it cannot direct, and history looks for an event it never recorded. **This class has no cheap check, and that is the finding.** The non-cheap form is SEMANTIC rather than lexical — do the stem and the title make the same CLAIM? — which a linter cannot judge and an agent can. That is a review prompt at archive time, not a gate.

**What actually worked was not a detector at all, and it already exists.** `doc(action="find")` returns `title` and `abs_path` adjacent in one row; here they contradicted each other on their face. Nothing ran. The adjacency did the work, and it costs nothing.


**A THIRD proxy was proposed and falsified, and the recurring SHAPE is the finding — not any of the three checks.**

The third: compare the catalog's `slug` against its `title`. It looked like the one both prior failures pointed at. `slug` is MECHANICALLY derived from `title`, so a disagreement is definitionally staleness with no paraphrase problem — what killed proxy 1 — and it is current-state, so the pre-first-commit blindness that killed proxy 2 does not apply. The catalog audit trail even WITNESSES the divergence git could not: this row's `seq 134008` set the slug, `seq 134050` changed the title, and no slug change ever followed.

**It is wrong by DESIGN, not by data.** A slug is immutable once minted — pinned by `ensure_slug_mints_dedups_and_is_idempotent`, `mint_missing_slugs_is_idempotent_and_never_re_mints` and `mint_missing_slugs_leaves_an_already_minted_slug_alone` in `src/librarian/catalog/artifact.rs` — because `entry_cite`'s primary key is `(src_slug, src_local, dst_ref, rel)`, so re-minting would break every citation pointing at the row. `graft_rows` carries the old slug through a move deliberately. So slug-diverged-from-title is CORRECT BEHAVIOUR, and a check over it flags every artifact whose title was ever edited after minting: the same false-positive generator, third flavour.

It was also MEASURED before the design was read — "56 of 1208 fire" — and those numbers were noise about the measurer's own hand-rolled normalisation disagreeing with `slugify`, plus degenerate `BUG:`-only titles taking fallback slugs and `-2` collision suffixes. **A measurement over a proxy whose mechanism you have not checked measures your normalisation, not the corpus.**

**The shape, now at n=2 across two sessions:** each falsification was published together with a successor proposed in the same breath, and the successor read as viable PRECISELY BECAUSE it had not been checked. The second instance was produced by the party who had named the shape one message earlier, which is the same finding as § *Observer Blindness*'s opening measurement — knowing the class prevented none of the four instances. A refutation creates an empty slot, and the next idea falls into it carrying the authority of the work that emptied it.

**Four candidates, three falsified, and the survivor is not a check.** `doc(action="find")` rendered `title` and `abs_path` adjacent and they contradicted on their face — no detector, no derivation, no normalisation. It is the only one of the four that caught a real instance, and it already exists.

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
