# Rule-tell campaign data, 2026-09-23/24

These are raw rows and run logs behind `docs/evals/rule-tell-scoring-2026-09-23.md` and its two pre-registrations: `docs/evals/rule-injection-timing-preregistration.md` for phases 0–2, and `docs/evals/phase1-local-classifier-preregistration.md` for the phase-1 selectors and the local route. They were copied out of session `571eb3d6`'s scratchpad at the end of the campaign, unchanged. Each file's result is the one the scoring doc reports; this directory is the evidence, not the reading.

## Phase 0: detector judges over the control corpus

| file | what |
|---|---|
| `blind-tasks.jsonl` | the blind task set scored by the judges |
| `scored-claude.jsonl`, `scored-claude-textpath.jsonl` | earlier judge forms |
| `scored-reasoned-n10.jsonl` | the reasoned form, n = 10: 2,850 rows |
| `native.jsonl` | native-judge verdicts |

## Phase 1: rule + claim selectors

| file | what |
|---|---|
| `phase1-jev.jsonl`, `phase1-haiku.jsonl` | phase-1A menu picks by Jev and Haiku |
| `p1s-gate.txt`, `p1s-gate2.txt`, `p1s-offline.txt` | first two span-selector gates (failed) and the offline span-check probe |
| `p1s-H0clean-gate.txt`, `p1s-H0clean-span.txt` | Haiku on the clean channel |
| `p1s-S0-gate.txt`, `p1s-S0-span.txt` | S0 (Sonnet 5), form 2 |
| `p1s-S0b-gate.txt` | S0 form 2b gate, 10/10. The `.DISCARDED-interrupted` copy is an interrupted run, discarded and never scored |
| `p1s-S0b-corpus.{txt,jsonl}` | S0 form 2b Score A, 924 rows |
| `form3-gate.txt`, `form3-span.txt`, `p1s-S0f3-corpus.{txt,jsonl}` | S0 form 3, the generic clause ablated: gates and Score A |
| `form4-gate.txt` | S0 form 4, three widened specs, judged alone: the gate that failed 6/8, so no Score A exists |
| `form4q-gate.txt`, `p1s-S0f4q-corpus.{txt,jsonl}`, `form4q-readout.{py,txt}` | S0 form 4q, `question_asked` widened alone: gate 6/6, Score A (the other 21 rules' rows carry `carried_from`, taken from form 3), and the registered readout |
| `l0-gate.{txt,jsonl}`, `l0-span.{txt,jsonl}` | local Stage 1, L0-frozen (JevK5 zero-shot); the `.jsonl` holds every rule's `noul` probability |

## Phase 2: API route (Messages-API replays)

| file | what |
|---|---|
| `phase2-pilot.jsonl` | pilot, arms 0 and 3, n = 3 |
| `phase2-dp1-n10.jsonl` | DP1 RTD-8 arms 0 / 1a / 1b / 2 / 3, n = 10 |
| `phase2-dp1-rtd910-n10.jsonl` | the RTD-9 and RTD-10 arms |
| `phase2-dp1-stripped-n10.jsonl` | stripped arms. 17 of 30 are usage-cap error rows; **discarded as registered, never scored** |
| `phase2-dp1-score.txt`, `score-rtd910.txt`, `score-rtd9*-sub.txt`, `score-rtd10*-sub.txt` | the original scores, on the Messages-API judge or the contaminated subscription channel |
| `api-score-rtd8c.txt`, `api-score-rtd9.txt`, `api-score-rtd10.txt` | the clean-channel re-score (`db6a5f0e`) |

## Phase 2: fork route (`claude -p --resume --fork-session`)

| file | what |
|---|---|
| `fork-dp1-n10.jsonl` | DP1 arms 0, 1b, s0, s1a, s1b. The `.superseded-date` copy holds the 3 date-contaminated forks, discarded |
| `fork-rtd3-arm0.jsonl`, `fork-rtd3-arms.jsonl`, `fork-rtd3-old-all.jsonl`, `fork-rtd3r.jsonl` | RTD-3, record 1498; `fork-rtd3r` is the reply-text re-registration |
| `fsc-*.txt` | fork-route scores. `EXPLORATORY` is labelled as such in the scoring doc |

## Phase 1 end to end (Score B, arm `e2s`)

| file | what |
|---|---|
| `e2s-dp1.jsonl`, `e2s-rtd3.jsonl` | S0's injection per draft: the claims it bound and the rendered text |
| `e2s-*-build.txt`, `e2s-*-fork.txt` | build and fork logs |
| `fork-e2s-dp1.jsonl`, `fork-e2s-rtd3.jsonl` | the e2s forks |
| `e2s-score-*.txt`, `rtd8c-score.txt` | e2s scores; `rtd8c-score.txt` is the rtd8c gate plus the clean re-score of every DP1 fork arm |
| `score-b-s0.sh` | the pipeline script as run. Its absolute paths point at a session scratchpad that no longer exists |

## Phase 1: the `partial` excerpt audit (`eb48fe28`)

Added after the campaign copy, by the same session. These files are the audit itself, not copies from the scratchpad.

| file | what |
|---|---|
| `partial-audit-verdicts.json` | the auditor's V / S / N verdict, quote and (a)/(b) reading per case, committed **before** the operator's blind check was asked |
| `partial-audit-check.py` | the two registered mechanical checks (quote verbatim in the positive, absent from the negative), then S0's rows for the same 9 positives. No model call |
| `partial-audit-check.txt` | its output |

## Local route, Stage 2: mined-pair candidates (`stage2/`)

A candidate build, not frozen data; see the pre-registration's § *Stage 2 status*. No model call.

| file | what |
|---|---|
| `stage2/mine_pairs.py` | the miner: correction-marker list and its reasons in the docstring, incident and document grouping, the held-out shingle filter |
| `stage2/mined-candidates.jsonl` | 940 candidate pairs, each `rule: null` with a keyword `rule_hint` for a labeller, and both `context_before` (the positive's own side) and `context_after` |
| `stage2/summary.txt` | the run's counts, drops by reason, hint distribution and a 10-row sample |

The 97 MB `git log -p` extract the miner read is not kept; the script regenerates it.

## Checker fixtures

`rtd8-corrected.txt`, `rtd-9-corrected.txt`, `rtd-9-positive.txt`, `rtd-10-corrected.txt`, `rtd-10-positive.txt`, `rtd-3-corrected.txt`, and `rtd3-recorded.json` (RTD-3's recorded first-turn output).

## Research

`jev-research.md` and `jev-oss-clones-research.md` are the two deep-research reports, on configuring Jev and on the open-source JevBench clones, that the local route's design rests on.

## Deliberately not included

- **The recorded API requests** (`dp1-request.json` and its stripped copy). They are full system prompts, including the operator's private global `CLAUDE.md` and memory index. A re-run needs them regenerated from the transcript, not copied.
- **Raw `claude -p` init streams** from the channel-contamination measurement. They carry account and session metadata. The measurement itself (2,778 against 249 tokens) is in the phase-1 pre-registration.
- **The judge config dirs.** They hold a credentials symlink.
- **Concatenated row files** (`score-b-*-rows.jsonl`, `api-dp1-rows.jsonl`). Each is the `cat` of files that are here.
- **Everything else in the scratchpad**, which belonged to unrelated work in the same session.

Every copied file was scanned for API keys, bearer tokens and the operator's email addresses before commit; there were no hits.
