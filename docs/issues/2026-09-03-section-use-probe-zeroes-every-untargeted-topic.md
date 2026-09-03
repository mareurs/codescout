---
id: bcd3d846861f4a57
kind: bug
status: fixed
title: probe_guide_section_use.py reports 100% never-engaged by construction for nine of ten topics
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-03
opened: 2026-09-03
owner: marius
related: []
severity: high
unverified: 'No regression test — `scripts/` has no test harness in this repo, so nothing gates the guard itself. The evidence is an observed refusal (exit 2) and an observed pass on the ruled topic, not a check that runs when nobody is looking; a later edit could remove the guard and no gate would fire. Also NOT done, deliberately: signatures for the other nine topics remain unauthored, so nine of ten topics are still unmeasurable — the guard converts a false answer into a refusal, which is the fix, but it does not extend coverage and this file should not be read as saying the probe now works for `librarian`. Finally, `topics_with_rules()` swallows a missing or unreadable guide file as ''unmeasurable'' rather than distinguishing it from ''no rules'' — correct for the refusal path, but it means a deleted guide would present as a rules gap.'
---

# BUG: `probe_guide_section_use.py --topic <T>` scores every section zero unless T is `tracker-conventions`

## Summary
`scripts/probe_guide_section_use.py` accepts `--topic` for all ten registered guide topics, but its engagement rules (`SECTION_SIGNATURES`) are keyed exclusively by `tracker-conventions` section headings. For any other topic the two key sets are disjoint, so every section scores zero and the probe reports **100.0% never-engaged** — a confident figure produced by construction rather than by measurement. There is no guard, and the run exits 0.

## Symptom (Effect)

```
$ python3 scripts/probe_guide_section_use.py --topic librarian
TOPIC: librarian   sessions with >=1 injection: 261
MAIN     n=95:  delivered 2,191,650 B; never engaged 2,191,650 B (100.0%); 95/95 engaged NOTHING
SUBAGENT n=166: delivered 3,829,620 B; never engaged 3,829,620 B (100.0%); 166/166 engaged NOTHING
```

Exit code 0. No warning, no error. The two `100.0%` figures and both `engaged NOTHING` counts are structurally forced.

## Reproduction

```
git rev-parse HEAD                                        # 88311708 at filing
python3 scripts/probe_guide_section_use.py --topic librarian
python3 scripts/probe_guide_section_use.py                # default: tracker-conventions
```

The default run returns plausible, differentiated numbers (MAIN 39.6% never-engaged, SUBAGENT 94.2%, `Cross-linking` engaged by zero subagent sessions). Any other topic returns exactly 100.0%.

## Environment
Linux, Python 3, codescout `experiments` @ `88311708`. Reads Claude Code transcripts from all three profile roots. No MCP transport involved — standalone script.

## Root cause

`SECTION_SIGNATURES` (`scripts/probe_guide_section_use.py:205-257`) is a dict whose six keys are **`tracker-conventions`** section headings:

```
"Bug files (docs/issues/)"
"Tracker artifacts (docs/trackers/)"
"Declaring an augmentation"
"Entry-level standard — the shape INSIDE a tracker"
"Querying with the librarian"
"Cross-linking (edges are derived — cite in prose)"
```

The report loop joins `r["engaged"].get(section, 0)` (`scripts/probe_guide_section_use.py:501`, `:507`), where `section` iterates the headings of the **selected** topic's file, produced by `current_section_bytes()` (`:262`). For `--topic librarian` those are `## Artifact Model`, `## docs/trackers/ — Backing Store, Not a Docs Folder`, `## Filter Syntax`, `## Tracker Workflow`, `## Augmentation Lifecycle`, `## Body Editing Surfaces`, … — **disjoint** from the signature keys, so `.get(section, 0)` returns `0` for every section, for every session.

`--topic` validates membership in `TOPICS` (`:86-95`), which lists all ten registered topics. Nothing checks that rules exist for the chosen one.

measured 2026-09-03: `SECTION_SIGNATURES` keys printed and compared against `grep -n '^## ' src/prompts/guides/librarian.md` — **zero intersection**.

## Evidence

### The two key sets, side by side
Signature keys (from the source, `:205-257`) are the six `tracker-conventions` headings listed above. `librarian.md` `##` headings:

```
10:## Artifact Model
50:## docs/trackers/ — Backing Store, Not a Docs Folder
66:## Filter Syntax
113:## Tracker Workflow
140:## Augmentation Lifecycle
209:## Body Editing Surfaces
```

No key appears in both lists.

### Why the number is dangerous rather than merely wrong
It was produced while answering *"do served `librarian` sections actually get used?"* — a question whose answer would have justified or blocked moving ~830 chars of tool-schema prose into served guide sections. `100.0% never engaged` reads as a decisive **no**. The truthful answer is **unmeasured**. A reader acting on it would conclude section-grain serving is pure waste and delete working machinery.

## Hypotheses tried

1. **Hypothesis:** the 100% is real and `librarian` sections genuinely go unused.
   **Test:** printed `SECTION_SIGNATURES` keys; compared against `librarian.md` headings.
   **Verdict:** rejected — the sets are disjoint, so 0 is forced regardless of transcript content.
   **Evidence link:** § *The two key sets, side by side*.

## Fix

**Shipped 2026-09-03.**

- **SHA:** `d4ee86da` on **`experiments`** — positional; it dies when `experiments` is rebased.
- **patch-id:** `28ea0c83152f6b093b8bfb2d8935e28ab4ee307b` — content hash of the diff; survives rebase and cherry-pick. `git show d4ee86da | git patch-id --stable`.

**The guard shipped; the signatures deliberately did not.** `--topic` on a topic with no matching rule now exits **2** and names the topics that do have rules. Verified by running it:

```
$ python3 scripts/probe_guide_section_use.py --topic librarian --roots /tmp
REFUSING to report on `librarian`: no SECTION_SIGNATURES rule matches any section of
that topic, so every section would score zero and the run would report ~100%
never-engaged by construction rather than by measurement.
  topics with rules: tracker-conventions
  Authoring rules for `librarian` is a separate change -- the guard is the fix.
exit=2
```

and the ruled topic still runs (`--topic tracker-conventions` → exit 0).

`topics_with_rules()` is **derived from the guides on disk**, never hand-listed — a hand-maintained list of "topics that have rules" would be this file's own defect class, in the same file that just demonstrated how such a list goes stale. It independently confirms this bug's central claim: it returns exactly `['tracker-conventions']`.

**Secondary, also shipped:** per `docs/adrs/2026-08-27-negative-results-name-their-scope.md` the report now names the rule-set's scope on **every** run, not only when empty. That disclosed something previously unstated — **6 of 7** sections of `tracker-conventions` itself have rules, so one section's zero has always been a statement about the rules rather than about the sessions.

The original plan below is preserved as written.

---

Plan (not yet implemented): **refuse rather than report.** When `--topic T` is passed and no `SECTION_SIGNATURES` key belongs to T's section set, exit non-zero naming the topics that do have rules. A probe that cannot measure the requested population must say so; today it answers an adjacent proposition faithfully and silently.

Secondary: the report header should name the rule-set's scope on every run, per `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a zero that is suspicious must name the scope examined.

Do **not** "fix" this by authoring `librarian` signatures as a first step. The guard is the fix; signatures are a separate enhancement, and shipping them without the guard leaves the next eight topics silently broken.

## Tests added
None — filed, not fixed.

## Workarounds

Treat any `--topic` output other than `tracker-conventions` as **unmeasured**, never as "unused". Only the default topic produces a meaningful number today.

## Resume

Add the guard in `scripts/probe_guide_section_use.py` at the point `--topic` is resolved (`:86-95`): compute the intersection of `SECTION_SIGNATURES` keys with `current_section_bytes()` headings for the chosen topic; if empty, `sys.exit` non-zero listing topics with rules. Then re-run `--topic librarian` and confirm it refuses rather than printing 100.0%.

## References

- `scripts/probe_guide_section_use.py:86-95` (`TOPICS`), `:205-257` (`SECTION_SIGNATURES`), `:262` (`current_section_bytes`), `:501`/`:507` (the join)
- `docs/PROBES.md` — the row for this probe documents several traps but not this one
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
- `docs/trackers/issue-clusters.md` — IC-18, whose remedy column reads *"partial — nothing reaches an author-written selector"*
- Sibling filed the same day: `docs/issues/2026-09-03-probe-mechanism-filter-omits-the-renamed-doc-tool.md`
