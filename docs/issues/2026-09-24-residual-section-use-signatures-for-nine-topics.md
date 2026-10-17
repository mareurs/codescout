---
id: '3ff543cad7b4fb22'
kind: bug
status: wontfix
title: 'RESIDUAL: Author section-use signatures for the other nine guide topics and distinguish a missing guide file from ''no rules'' in topics_with_rules()'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-17
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md
severity: low
unverified: 'STANDING Eight guide topics are refused, not measured: the user decided 2026-10-17 not to widen scan_transcript''s tool scope for progressive-disclosure, symbol-navigation and workspace-state (01b1a82b); error-handling, iron-laws-detail, librarian-runtime and untrusted-content have no injection anywhere, so no positive control can exist; project-activation-bootstrap is the session opener and would credit every later call by construction. Only librarian and tracker-conventions are scored.'
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
- The probe cannot see section-grain injections of `librarian`. That is filed as `15d81831913d045e`, and fixed and archived on 2026-10-10 (see the archived record).

Decisions needed before anyone authors: widen the tool scope per topic, or declare those seven topics out of scope; and key by `(topic, heading)`. The third decision, matching the `§ <heading>` marker form, was made and built on 2026-10-10. After those, `librarian` is the only topic with a feasible population.

## Partial fix (2026-10-16)

- **SHA:** `ec439798` (`worktree-agent-a547c2640763139eb`, to be rebased onto `experiments` by the lead)
- **patch-id:** `57b119b84b0b47a7b989f001c049b0f94e37e64b`

What is covered, each verified at the bytes on real transcripts (3,094 transcript files copied to a scratch directory, scripts run with `python3 -I`):

- **`(topic, heading)` key.** `SECTION_SIGNATURES` is `topic -> heading -> rules`; `signatures_for(topic)` is the one reader. Collisions measured in the guides: `Related` in four, `Anti-patterns` and `Path-relative annotation` in two, `(preamble)` in all ten. Mutated back to a heading-only lookup, four of the new tests fail.
- **`librarian` signatures.** 11 of its 14 `##` sections have rules, read off the section text and its `serves:` declaration. Run over 712 sessions (258 main, 454 subagent): main 44.2% never-engaged, subagent 88.1%, sampled by hand with `--explain`: `merge_worktree`, `event_create/list`, `link`, `move`, `status: archived` and `commit_refresh` calls each credit the section they should. Self-trigger control: a `create_file` carrying the same words engages nothing (unit test). Unruled, with the reason in the source: `Common Mistakes` (prose across every shape) and `Runtime tips` (a pointer).
- **`tracker-conventions` unchanged.** Same run before and after: 141 main sessions, 104/105/64/19/41/132 sessions engaging the six ruled sections, 70 subagent sessions 1/2/0/0/0/30. The new `###` headings do not move the probe's `##` grain.
- **The other eight topics are refused with a recorded reason** (`UNMEASURABLE_TOPICS`), and `topics_unaccounted_for()` is empty, so a new guide registered in `TOPICS` with neither rules nor a reason is a failing test, not a silent gap. Four have **no population** (error-handling, iron-laws-detail, librarian-runtime, untrusted-content: 0 injections in 2,411 sessions, so no positive control can exist). Four are injected but **out of the probe's tool scope** (progressive-disclosure 57% of sessions, symbol-navigation 62%, workspace-state 10%, project-activation-bootstrap 95% as the session opener).
- **`report()` scores only ruled sections** and names the rest `NOT SCORED`, so an unruled section no longer counts its bytes as never-engaged by construction. It now also states that `delivered` assumes whole-topic delivery, which is false for `librarian` and, since d34c398e, for `tracker-conventions`.

Tests: `tests/test_probe_section_signatures.py` (13 cases, hand-run like the rest of `tests/test_*.py`).

**Reading the librarian non-zeros:** a `librarian` section is delivered BY the call shape its rule then matches, so a later call of that shape scores it engaged. Only its zeros carry information; e.g. `Worktree overlay` was engaged by 0 of 454 subagent sessions and 11 of 258 main sessions.

**Not done, and not an authoring task:** a tool scope per topic for progressive-disclosure, symbol-navigation and workspace-state. Widening `scan_transcript` beyond doc/librarian is what once scored all six tracker-conventions sections at 100% from the `create_file` that wrote the script, so each tool added needs its own self-trigger control. Options and evidence were sent to the lead; at the time of writing those three stayed refused with `OUT OF SCOPE (decision pending)`. **Decided 2026-10-17, see `## Resume`: not widened; the refusal now records the decision.**

## Resume

Missing-guide half is in on `experiments` (local, not pushed at the time of writing). Status stays `open` for the nine-topic signature authoring, a separate measurement project that Marius must decide to staff; until then nine topics stay refused, by design. Separately, and more consequential than this bug: the Python test population (`tests/test_*.py`, 27 files measured) has no gate or CI lane, so every Python regression test is hand-run only. Whether to add a lane (and which runner) is a decision for Marius and should be filed as its own bug.


**Decision 2026-10-17 (user): REFUSE all eight remaining topics; do not widen `scan_transcript`'s tool scope.** The three that were `OUT OF SCOPE (decision pending)` (progressive-disclosure, symbol-navigation, workspace-state) now carry `OUT OF SCOPE (decided 2026-10-17: widening the tool scope needs its own self-trigger control per tool; not staffed)` in `UNMEASURABLE_TOPICS`, so a refusal no longer sends a reader to wait on a decision that was made. Landed at `01b1a82b` (patch-id `a8585300c548f0ba438914ca88bd0c7f08f747ad`, on `experiments`' line, not yet on `master`); `tests/test_probe_section_signatures.py` gained `test_the_three_tool_scope_topics_carry_the_decided_reason_not_a_pending_one` (red before the change, 14 cases green after) and `topics_unaccounted_for()` is still empty (`test_every_topic_is_ruled_or_has_a_recorded_reason`). The other five unmeasurable topics are unchanged: four have no injection anywhere (a population cannot be created by deciding), and `project-activation-bootstrap` credits every later mechanism call by construction.

**Status `wontfix` for the remainder; `librarian` is done.** Signatures for `librarian` and `tracker-conventions` exist and are verified on real transcripts (`ec439798`, patch-id `57b119b84b0b47a7b989f001c049b0f94e37e64b`); the missing-guide half is `34d39567`. What the user chose not to do is the tool-scope widening for the three topics, which is the only authoring left. If it is ever staffed, reopen with a self-trigger control per tool (a `create_file` carrying the guide's own words must engage nothing), as `ec439798` did for `librarian`.

The separate Python-lane question in the previous Resume is overtaken: `scripts/gate.sh` now runs `python3 ./scripts/python-tests.py --lane light` (read at lines 144-146), so `tests/test_*.py` outside the heavy list are no longer hand-run only.

## References

- `docs/issues/archive/2026-09-03-section-use-probe-zeroes-every-untargeted-topic.md` — parent
