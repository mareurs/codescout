---
kind: bug
status: investigating
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: medium
---

# BUG: the second file template still propagates retired call forms — the fix covered `_TEMPLATE.md` and not `docs/templates/session-log.md`

## Summary

**This is the residue of a bug that was fixed and archived on 2026-09-03**
(`docs/issues/archive/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md`,
fixed by `3e8193a0`, archived at `c8447014`). Read that file first — it owns the class and the
site.

That fix reached the propagation argument for **one** template: it added
`docs/issues/_TEMPLATE.md` to the gate's `FILES` with a comment saying the template is
*"prescriptive, copied verbatim into every new bug file"*. `docs/templates/session-log.md` is
the other template, is copied verbatim into every new session log, still carries **2** retired
call forms, and is in neither `ROOTS` nor `FILES`. Two further reader surfaces —
`docs/TEAM-ONBOARDING.md` (9) and `docs/ROADMAP.md` (4) — are also still uncovered.

**15 occurrences across 3 files**, unit: occurrences, matching the gate's per-`(line, call)`
`bad` vector.

## Symptom (Effect)

`reader_docs_contain_no_retired_call_forms` is green while these are on disk:

```
docs/templates/session-log.md:12:> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
docs/templates/session-log.md:240:     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
```

plus 9 in `docs/TEAM-ONBOARDING.md` and 4 in `docs/ROADMAP.md`. Each is a call form — what the
gate's own assertion message defines as *"a claim that this is how you invoke the tool today"*
— naming a tool retired on 2026-09-02.

The session-log pair is the load-bearing one. `CLAUDE.md` § *Session Intelligence Trackers*
points at `docs/templates/session-log.md` as the template for every new per-work-stream log,
and routes non-Claude harnesses to it explicitly (*"any agent that reads markdown can use the
template — no plugin required"*). So the surface most likely to be copied by an agent that
cannot ask a follow-up question is the one still teaching a dead call.

## Reproduction

On `bug-claim-liveness` rebased onto `experiments` `c8447014`:

```
cargo test --lib reader_docs_contain_no_retired_call_forms   # passes
for f in docs/TEAM-ONBOARDING.md docs/ROADMAP.md docs/templates/session-log.md; do
  printf '%4d  %s\n' "$(grep -o -E 'artifact\(|artifact_event\(|artifact_augment\(|artifact_refresh\(|read_markdown\(|edit_markdown\(' "$f" | wc -l)" "$f"
done
# 9 docs/TEAM-ONBOARDING.md / 4 docs/ROADMAP.md / 2 docs/templates/session-log.md
```

Control, same command on the two surfaces the fix did reach — both `0`:
`docs/issues/_TEMPLATE.md`, `.codescout/system-prompt.md`.

## Environment

Linux. Branch `bug-claim-liveness` rebased onto `experiments` `c8447014`. Gate green at
filing: fmt 0, clippy 0, lean 0, default 5340 passed / 0 failed over 31 binaries.

## Root cause

The scope is two hand-written `const` arrays, `ROOTS` (`src/prompts/mod.rs:2041-2047`) and
`FILES` (`:2056-2073`). Nothing derives them; a file is scanned iff an author typed its path
there. Measured 2026-09-03 by classifying all git-tracked `*.md` against the gate's own
predicate: **153 of 1519 files scanned, 10%** — 0 in-scope occurrences, 2164 out-of-scope
across 407 files.

**The 2164 is not a defect count and must not be quoted as one.** The overwhelming majority
are historical records — session logs, archived issues, plans — which the gate's doc comment
deliberately and correctly tolerates. The defect is the live-instruction-surface subset,
enumerated by hand above, which is the same manual step this bug is about.

**The interesting part is which way the fix generalised.** `3e8193a0`'s own comment states the
rule correctly — a template is prescriptive, not a record — and then applies it to the single
template in front of it. The rule was written down and the population it ranges over was not
enumerated, so the second member of that population survived the commit that named the
category. This is `IC-18`'s mechanism note (*"nothing reaches an author-written selector"*) in
its least visible form: not a forgotten directory, but a **correctly stated general rule
instantiated once**.

## Evidence

### The predecessor gate's defect, now three generations deep

`tests/doc_tool_refs.rs`'s `present_tense_surfaces()` was an enumerated allowlist that missed
`docs/architecture/`, `docs/conventions/` and `docs/adrs/` — 60 of 160 mentions
(`docs/issues/2026-09-02-docs-architecture-is-in-neither-the-gates-inclusion-list-nor-its-exclusion-rationale.md`).
`reader_docs_contain_no_retired_call_forms` was written to backstop it, as another enumerated
allowlist. `3e8193a0` extended that allowlist by hand, and this file is what the extension
missed. Three passes, same shape each time.

### The one-token miscitation, fixed here

`src/prompts/mod.rs:2008` cited `IC-11` (`doc-contradicted-by-code`) for a defect its very next
clause describes accurately as `IC-18` (`selector-narrower-than-its-population`). Still present
at `c8447014`; corrected on this branch.

## Hypotheses tried

1. **Hypothesis:** these are lines this branch added and failed to sweep.
   **Test:** `git grep -n -E '<retired forms>' <base> -- <files>`.
   **Verdict:** rejected. All present at the merge base. The branch's own five added lines were
   swept before the rebase, and every file it touched inside `ROOTS` has zero residue.

2. **Hypothesis:** this duplicates the bug it supplements.
   **Test:** read that file's `## Fix sketch` and `## Why the gates missed it — the class`,
   then re-measure after its fix landed.
   **Verdict:** **it did, in part, and the file was rewritten twice because of it.** The first
   draft re-derived the class independently and was discarded on reading the peer's filing. The
   second listed five surfaces; re-measuring against `c8447014` showed the fix had cleared two
   of them. What survives is three surfaces and the generalisation gap above. Recorded rather
   than quietly dropped, because a third reader starting from the same observation would write
   the discarded draft again.

3. **Hypothesis:** the `IC-13` class (`capped-result-presented-as-complete`) fits.
   **Verdict:** rejected — nothing is capped or truncated. `IC-18`'s claim ends *"a zero reads
   as 'not present' rather than 'not looked at'"*, which is what a green gate over 10% of the
   corpus is.

## Fix

Two directions, and they are not equivalent:

- **Narrow:** add the three files to `FILES`. It is the same edit `3e8193a0` made, and this bug
  is what that edit missed — so choosing it again is choosing the same failure mode for a
  fourth pass.
- **Structural:** invert the selector. Scan **all** tracked `*.md` and maintain an explicit
  *exclusion* list of historical-record roots (`docs/issues/archive`, `docs/archive`,
  `docs/trackers`, `docs/superpowers`, `docs/evals`, `CHANGELOG.md`). A new file is then covered
  by default and every exemption is a written claim that the file is a record. The cost is
  honest: the exclusion list is itself hand-written, and the 2164 out-of-scope occurrences need
  classifying once. What changes is the failure mode — from *silently unscanned* to *loudly
  scanned until someone justifies an exemption*, the shape `CLAUDE.md` § *Observer Blindness*
  asks for.

Whichever is chosen, extend the per-root non-vacuity assertion to `FILES` too: every named file
must exist, so a rename cannot silently empty an entry. That guard covers roots today and not
files.

SHA: *pending.* patch-id: *pending.*

## Tests added

None for the scope gap. The `IC-11` → `IC-18` correction is a doc comment and carries no test.

## Workarounds

When copying `docs/templates/session-log.md`, translate by hand:
`artifact(action="append_entry"…)` → `doc(action="append_entry"…)`.

Treat a green `reader_docs_contain_no_retired_call_forms` as covering 153 of 1519 markdown
files; read `src/prompts/mod.rs:2041-2073` for which.

## Fix applied 2026-09-03

All 15 occurrences swept:

- `docs/templates/session-log.md:12,240` — `artifact(action="append_entry", …)` → `doc(action="append_entry", …)`
- `docs/TEAM-ONBOARDING.md` — 9 sites, all `artifact(` → `doc(` (find/get/append_entry/move/update)
- `docs/ROADMAP.md` — 4 sites, all `artifact(get)` → `doc(get)`

`reader_docs_contain_no_retired_call_forms`'s `FILES` list (`src/prompts/mod.rs`) extended with
`docs/ROADMAP.md`, `docs/TEAM-ONBOARDING.md`, and `docs/templates/session-log.md` — the latter
following the same "prescriptive, copied verbatim" reasoning as the bug-file template already
in that list.

**Verified:** zero remaining `artifact(`/`artifact_event(`/`artifact_augment(`/`artifact_refresh(`/
`read_markdown(`/`edit_markdown(` occurrences in all three files (grep, post-fix). Full gate
green: `cargo fmt --check`, `cargo clippy --workspace --all-targets --features local-embed -- -D
warnings` (exit 0), `cargo test --workspace --no-default-features` (exit 0), `cargo test
--workspace` (exit 0) — no failures in either lane.

**Not committed yet** — SHA and patch-id pending; archive after commit per convention.

## Resume

Settle narrow-vs-structural above before adding the three files, since picking narrow is
picking the same shape a fourth time. If structural, the actual work is classifying the 407
out-of-scope files into record vs instruction surface — needed once either way.

**Catalog note:** the `cluster/` tag is in frontmatter, which for a *new* file is the catalog's
source — the classifier reads it on first index, as it does `kind: bug`. This is not the BL-48
hazard, which concerns editing a file that already has a row. **Verified landed** — row
`192b99619018ed2c`, returned by
`doc(action="find", filter={"tags": {"contains": "cluster/selector-narrower-than-its-population"}})`
after the branch merged and the main checkout was reindexed (`added: 2`). Nothing owed here.

## References

- **Predecessor, fixed:** `docs/issues/archive/2026-09-03-retired-tool-names-survive-in-the-surfaces-that-actually-reach-agents.md`
  — fix `3e8193a0`, archived at `c8447014`, tagged `cluster/guard-narrower-than-its-name`
  (`IC-14`). Kept apart from this file on the standing test: that claim is about the guard's
  **name** overstating coverage, this one about the **selector** being narrower than its
  population.
- `src/prompts/mod.rs` — the gate `reader_docs_contain_no_retired_call_forms`; `ROOTS`
  `:2041-2047`, `FILES` `:2056-2073`, the `IC-11` miscitation `:2008`.
- [`docs/trackers/issue-clusters.md`](../trackers/issue-clusters.md) — `IC-18`, artifact
  `1b5a080fe2efcb6b`; mechanism column reads "partial — nothing reaches an author-written
  selector".
- Class precedents against the older `tests/doc_tool_refs.rs`:
  `docs/issues/2026-09-02-docs-architecture-is-in-neither-the-gates-inclusion-list-nor-its-exclusion-rationale.md`,
  `docs/issues/2026-09-02-both-doc-citation-guards-skip-half-the-corpus-without-saying-so.md`.
