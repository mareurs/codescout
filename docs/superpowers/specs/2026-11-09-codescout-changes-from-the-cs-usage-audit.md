# Codescout changes from the cs-usage audit — spec

Date: 2026-11-09. Status: draft for the operator. Nothing here is implemented yet.

## Goal

Make codescout cheaper and less obstructive for an agent doing a bug-fix task, without weakening what the iron laws protect. Measure it with the Setup A/B harness, with the arm profile that already exists (`arms.py` profile `v2`, commit 8a44c4eca).

## Evidence (all from `research/setup-ab-audit-cs-usage-20261109`; "verified" = the controller re-read the bytes)

| # | Observation | Status |
|---|---|---|
| E1 | cs costs 2.36x and full 3.28x their bare twins (median); turns 2.1x and 2.5x | audit, medians recomputed by auditor E over 108 runs |
| E2 | The fixed first-turn context is 7.4k tokens in bare, 31k in cs, 43k in full; it is re-read each turn: 38 to 83 percent of the extra input tokens | audit A, B, C, D (one measurement each, same direction) |
| E3 | Guard refusals: piped test in a `;`/`&&` list 54, multi-hunk `edit_file` touching a `fn` 30, line range overlapping a symbol 36 (74 percent of `read_file` calls pass `force=true` anyway), `cat`/`sed` on source 20. Removing the wasted turns lowers the cs/bare turn ratio only from 2.09 to 1.89 | audit E |
| E4 | `references` 4 calls, `call_graph` 0, `symbol_at` 0, `semantic_search` 0, `memory`/`onboarding`/`get_guide` 0 over 56 runs | verified (controller counted `references` calls in the first pass; auditors the rest) |
| E5 | The server instructions are mostly prohibitions; the `references` description is one line; the "when to use" text is only in `long_docs`, which no run read | audit E, quoting `src/prompts/source.md:5-36` |
| E6 | `run_command`: a filtered `cargo test` of a failed build returned `type: test, exit_code: 0, passed: 0, stdout: ""`, `producer_exit_code: 101` | verified in one transcript line; bug 762b5810f84398cf |
| E7 | Failures in the audited runs are not navigation failures; no verified case where `references` or `call_graph` would have prevented one | audit A to D |

Not verified: that removing the guards or adding "when" text lowers cost or raises pass rates. Treat each change below as a hypothesis to be measured, not a fix.

## Proposed changes

Each change names the future change it absorbs (Snow Lion rule 1) and what it gives up.

1. **Reply shape of a filtered run (bug 762b5810f84398cf).** When `producer_exit_code` is non-zero and differs from `exit_code`, the reply puts it first and is not typed `test` with `passed: 0`. Absorbs: any future filter on any producer. Gives up: a reply field order that scripts may rely on (check the dashboard and the tests first). Confidence high; smallest change, a real defect.
2. **"When to use" text in the descriptions of `references`, `semantic_search`, `symbols`.** One sentence each, moved up from `long_docs`. Absorbs: a model that reads only the short description. Gives up: description bytes (the prompt-surface caps in `src/prompts/README.md` apply). Confidence medium: evidence E4 and E5 say the text is absent, not that adding it changes behaviour.
3. **Bug-fix rule in the quickref of the server instructions, and drop the codescout-only deeper-guidance topics when the active project is not codescout.** Absorbs: projects other than this one (every study task). Gives up: nothing for this repo. Confidence medium.
4. **Guards hint and let the call through, instead of refusing, for the four refusals in E3.** Absorbs: wasted turns of any client. Gives up: the iron laws' protection (IL1 against full reads of source, IL2 against structural edits through `edit_file`, IL3 against unbounded pipes): the refusal is what stops an agent that would otherwise burn context. Confidence low: E3's own arithmetic says removing wasted turns closes only about a tenth of the gap. **This is a decision for the operator, not a ruling I make.** Smaller option: keep the refusals, make them cheaper (one message that names the corrected call, so one retry).
5. **Companion-plugin hook texts (other repository, bug 08eb4c18228dca64):** compose skills and tool names from the session's tool list; drop the `acknowledge_risk` advice and the tracker-hygiene nag outside this repository. Out of this checkout; listed so it is not forgotten.

## Rulings (this spec's author; revisit on the operator's word)

- Order: 1, then 2 and 3 together (text only), then measure, then decide 4 with the numbers in hand.
- 4 is not started until the measurement of 1 to 3 exists, because E3 says it cannot explain most of the gap.
- Every change is one commit with a regression test, gate green, per `CLAUDE.md`. Surfaces under `src/prompts/` follow `src/prompts/README.md` (cap, `ONBOARDING_VERSION` bump rule).

## How to measure

1. Same tasks, both models, arms `bare`, `cs`, `full`, with arm profile `v2` for all three (so the arms differ only in what they should).
2. One rep first on the pool v2 tasks (4 tasks, about 0.6 USD bare, about 2 to 3 USD cs) after each of: profile v2 alone; changes 1 to 3; change 4 if it is made. Record cost, turns, tool-error count, guard-refusal count, first-turn context size.
3. A change counts as helpful only if cost or turns fall without the pass rate falling, over at least two reps per cell; with n this small say "no detectable change" instead of a direction.

## Open decisions for the operator

- Change 4: refuse as now, make the refusal cheaper, or hint and pass through.
- Whether the daily setup's native-tool deny list is the intended reference for the cs arm (profile v2 assumes so; it was read from the full arm's user settings, not from the daily profile itself).
- Plumbing of `profile` through `run.py` and the registration pins is not done: `arm_invocation` accepts it, `run.py` does not pass it yet, so a registered screen cannot select v2 until that is added.
