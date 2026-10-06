---
id: '3ff543cad7b4fb22'
kind: bug
status: open
title: 'RESIDUAL: Author section-use signatures for the other nine guide topics and distinguish a missing guide file from ''no rules'' in topics_with_rules()'
tags:
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md
severity: low
unverified: 'PARTIAL FIX (34d39567): a missing guide file is now refused apart from ''no rules matched''. NOT done: signature authoring for the nine other topics. It is blocked on three design decisions and, for four topics, on evidence that does not exist; see ## Partial fix, Update 2026-10-06.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Author section-use signatures for the other nine guide topics and distinguish a missing guide file from 'no rules' in topics_with_rules().

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md` (status `fixed`):

> No regression test — `scripts/` has no test harness in this repo, so nothing gates the guard itself. The evidence is an observed refusal (exit 2) and an observed pass on the ruled topic, not a check that runs when nobody is looking; a later edit could remove the guard and no gate would fire. Also NOT done, deliberately: signatures for the other nine topics remain unauthored, so nine of ten topics are still unmeasurable — the guard converts a false answer into a refusal, which is the fix, but it does not extend coverage and this file should not be read as saying the probe now works for `librarian`. Finally, `topics_with_rules()` swallows a missing or unreadable guide file as 'unmeasurable' rather than distinguishing it from 'no rules' — correct for the refusal path, but it means a deleted guide would present as a rules gap.

## Fix

Missing-guide half done 2026-10-06 (see `## Partial fix (2026-10-06)`); nine-topic signature authoring not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## Partial fix (2026-10-06)

- **SHA:** `34d39567` (`experiments`)
- **patch-id:** `2781fa1df72532201ff8d2b618a4eda402f9fa50`

What is covered: the missing-guide half of the parent's caveat only. `scripts/probe_guide_section_use.py` gains `topics_with_missing_guide()` (registered topics whose guide file is absent or unreadable under `GUIDE_DIR`) and a pure `refusal_for_topic(topic)` that checks missing-first and gives the missing case its own message naming the path ("its guide file is missing or unreadable ... NOT a rules gap"). `main()` now prints that refusal and exits 2, and warns on stderr when any other registered topic's guide is missing. The no-rules message is unchanged for a guide that exists. `topics_with_rules()` still skips a missing guide; only its docstring changed.

Tests added: class `MissingGuideIsNotNoRules` in `tests/test_probe_mechanism_tools_registry.py`, with `GUIDE_DIR` pointed at a fabricated temp directory: `test_a_missing_guide_gets_its_own_refusal_not_the_no_rules_one`, `test_a_present_guide_with_no_matching_rule_keeps_the_no_rules_refusal` (twin), `test_a_present_guide_with_a_matching_rule_is_not_refused` (positive control), `test_missing_guides_are_listed_apart_from_ruleless_ones`. Not re-run by this bookkeeping pass.

What is NOT covered: authoring `SECTION_SIGNATURES` for the other nine topics, so nine of ten topics are still unmeasurable (the guard still refuses them). That is a separate measurement project.

**Correction to the triage and an important follow-up.** The triage claim that this registry test is the only Python unittest was WRONG. Counted 2026-10-06: `git ls-files 'tests/test_*.py'` lists 27 files (the sweep brief said 29; the 27 is what this pass measured, so reconcile before quoting a number). None is run by `scripts/gate.sh` (no `pytest`, `unittest` or `tests/test_` reference in it or in any `scripts/*.sh` other than comments in `scripts/mutation-probe.sh`) or by `.github/workflows/ci.yml` (its only Python step is `python3 scripts/probe_tool_surface.py`). `tests/python_test_entry_guard.rs` states the same: "No CI job, gate lane or hook runs `tests/test_*.py`; they are run by hand." So the tests added here, like the rest of that population, guard nothing unless someone runs them by hand (`python3 tests/test_probe_mechanism_tools_registry.py` or `unittest discover`).

**Update 2026-10-06 (authoring attempted and stopped; no signature written).** A fork tried the authoring and stopped, because it needs design decisions and four topics have no evidence. The figures below were measured by the fork and were not re-run by the author of this note.

- `scan_transcript` (`scripts/probe_guide_section_use.py:541-616`) counts only `doc`, `librarian` and `artifact` calls. Its docstring marks that limit as load-bearing, because a self-trigger once scored all six sections at 100%. Seven topics cannot be expressed in that scope: `error-handling`, `iron-laws-detail`, `progressive-disclosure`, `project-activation-bootstrap`, `symbol-navigation`, `untrusted-content` and `workspace-state`.
- `SECTION_SIGNATURES` is keyed by heading alone. Headings collide across the ten guides: `Related` appears in four, and `(preamble)` in all ten. Per-topic rules need a `(topic, heading)` key.
- Four topics were never injected in 3,487 transcripts: `error-handling`, `iron-laws-detail`, `librarian-runtime` and `untrusted-content`. No positive control can exist for them.
- The probe cannot see section-grain injections of `librarian`. That is filed as `9cf119d200328dfb`.

Decisions needed before anyone authors: widen the tool scope per topic, or declare those seven topics out of scope; key by `(topic, heading)`; and match the `§ <heading>` marker form. After those, `librarian` is the only topic with a feasible population.

## Resume

Missing-guide half is in on `experiments` (local, not pushed at the time of writing). Status stays `open` for the nine-topic signature authoring, a separate measurement project that Marius must decide to staff; until then nine topics stay refused, by design. Separately, and more consequential than this bug: the Python test population (`tests/test_*.py`, 27 files measured) has no gate or CI lane, so every Python regression test is hand-run only. Whether to add a lane (and which runner) is a decision for Marius and should be filed as its own bug.

## References

- `docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md` — parent
