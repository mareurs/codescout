# Judge gate, first run — controller note

Companion to `gate.txt`, which is the gate's full output, committed unedited. Every figure below was derived from that run's result and its per-vote logs. The logs and the full JSON result live outside the repo, as spec Amendment 8 (b) requires. **Verdict: `passed` is False, so the measurement is INCONCLUSIVE by rule** (spec § Judge protocol, the gate; plan Task 9 Step 6). The prompt is not re-tuned against this result.

## Provenance

- Launched 2026-09-29T04:17:19Z and finished 04:37:48Z, at repo HEAD `0f3bc7c1` (Amendment 8), with `scripts/measure/` clean.
- The prompt was `scripts/measure/judge_prompt.md`, sha256 `3137920a8d4645540b9cff7bced95671a9c6fbb282581cddc5eb5391d4de8c54`, the value Amendment 8 (a) registered.
- 81 items × 3 votes gave 243 calls (`complete() calls: 243`), and the run exited 1 because `passed` is False.

## Frozen corpora — 2026-09-29

Task 12 Step 1 ran on 2026-09-29 at the operator's request, after the stop, as preservation only: Steps 2–5 did not run. Corpora live privately under `~/work/claude/measurement-corpora/<corpus-id>/` (mode 700). Only their manifests are committed, under `corpora/`. Each was checked by `archive.verify()` right after freezing, and `observability.finalize_bounds` set the bounds.

| corpus id | top-level / subagent transcripts | usage rows | retained window (UTC) | sessions kept / excluded |
|---|---|---|---|---|
| `2026-09-29-codescout` | 162 / 823 | 77,189 | 2026-08-03T20:49:16.290Z → 2026-09-29T09:42:08Z | 122 / 40 (25 `sdk-cli`, 10 `duplicate-prefix-of`, 5 `excluded-by-spec`) |
| `2026-09-29-mrv-poc` | 40 / 309 | 19,383 | 2026-08-26T06:17:40.545Z → 2026-09-29T09:43:03Z | 32 / 8 (8 `duplicate-prefix-of`) |
| `2026-09-29-judge-gate-1` | 243 vote logs + `gate.json` + run files (254 files) | — | — | — |

- **codescout's sources:** the three spec profiles, two `--worktrees-` project dirs whose worktrees are gone from disk, and four scratchpad dirs. All 18 scratchpad transcripts are `sdk-cli` and are excluded under that higher-priority reason, so none is kept.
- **The contrast project is MRV-poc.** It keeps 32 sessions, above the 15 at which backend-kotlin would replace it.
- **The exclusions are provisional.** They are recomputed here with `join.SPEC_EXCLUDED_SIDS`. The manifest's `exclusions` field stays empty until `join.build_events` records it (Task 12 Step 2).
- **The gate evidence was cross-checked against an independent record.** Its `gate.json` and all 243 vote logs match the hashes Codex recorded in `codex-review-2026-09-29/saved-diagnostics.json`.
- **Three frozen files contain GitHub-token-shaped strings.** That count is from a pattern match over the corpus directory on 2026-09-29, and it does not show whether they hold one token or several. So the corpus directory must never be published, and the token rotation already owed still stands.

## Channel checks (Amendment 8 (b), R133/R137)

- **All 243 vote logs have one event sequence:** thread started, turn started, one agent message, turn completed. There were 0 tool or exec events.
- **Retries:** 0 of 243 votes needed a retry, and there were 0 failed attempts of any kind.
- **Warning:** every call printed one identical stderr warning. It says codex declined to create PATH helper binaries under a temporary CODEX_HOME, which does not concern the model or the prompt.
- **Usage (from the logs' `turn.completed` events):** 7,652,195 input tokens, of which 2,799,360 were cached, and 66,385 output tokens, of which 37,820 were reasoning.
- **Auth:** the operator's Codex auth file kept its modification time from 2026-09-22, so no token refresh wrote through the jail's bind.

## Private-text scan (R146)

The scan looks for ≥20-char whitespace-normalised substrings of the operator's three private global `CLAUDE.md` files that are not committed anywhere at HEAD. Its positive control scored 21 hits.
- `gate.txt`: 14 raw hits, 0 of them uncommitted. The 14 are text the global file shares with `docs/trackers/operator-rules.md`.
- The JSON result: 0 raw hits.
- Verdict: PASS.

## RTD-2, the pre-registered structural disagreement (Amendment 8 (h))

RTD-2 came out `in-trace` against the expected `obtainable`, as pre-registered. Detectability agreement is 14/21 with RTD-2 and 14/20 without it. That second figure is descriptive only. The decisive score is 14/21 against the required 16, so the check FAILS either way.

## Pre-registered predictions against outcomes (detectability, correction mode)

| case | pre-registered | outcome |
|---|---|---|
| RTD-2 | structural disagreement: `in-trace` | `in-trace` — as predicted |
| RTD-6 (`no`) | most plausibly `external` | `obtainable` — disagreed |
| RTD-13 (`no`) | near a coin flip, `obtainable` against `external` | no majority — disagreed |
| RTD-14 (`no`) | most plausibly `external` | `obtainable` — disagreed |
| RTD-21 (`no`) | most plausibly `external` | no majority — disagreed |
| RTD-1, RTD-4, RTD-11, RTD-12, RTD-19 | at risk | all agreed |
| RTD-17, RTD-20 | not predicted | no majority; `obtainable` — disagreed |

So 3 of the 4 `text_detectable: no` cases were not labelled `external` by majority, even under the signal-axis wording (R147).

## The four checks

| check | result | need | |
|---|---|---|---|
| detectability agreement (correction mode) | 14/21 | ≥ 16 | FAIL |
| `peer × yes` flagged (audit) | 4/4 | ≥ 3 | ok |
| `yes` flagged (audit) | 5/8 | ≥ 6 | FAIL |
| controls firing (audit) | 17/52 | ≤ 5 | FAIL |

- **Controls:** the 17 control fires are the largest miss. By prompt section they are RTD-10: 6, RTD-3: 4, RTD-9: 3, RTD-8: 2 and contradiction: 2. None of the 17 fires, and none of the 5 majority flags, carries a `quote_not_verbatim` vote.
- **The `yes` cases that went unflagged:** RTD-17 and RTD-18 came out `is_mistake` False, and RTD-20 got no majority.
- **Lesson assignment,** reported and not a pass condition: 3 of 12 scoreable.

## Codex review erratum — 2026-09-29

The original gate output and registered decision above remain unchanged: INCONCLUSIVE. The following corrects the controller's interpretation, not the scoring rule.

- All **four**, not three, `text_detectable: no` cases miss `external`: RTD-6/14 return `obtainable`; RTD-13/21 return null.
- Null here does not mean no majority. RTD-13 has three `is_correction: false` votes; RTD-17 and RTD-21 have two. Their null detectability follows the prompt's conditional contract. RTD-20's audit has two null `is_mistake` votes and one false: majority abstention.
- Controls split into **17 true, 13 false, 22 null** on majority `is_mistake`. The 17/52 is a **firing rate, not an established false-positive rate**: the control corpus was not independently adjudicated as error-free. Null is not a clean verdict.
- RTD-17's source labels an unstamped, then-true statement as detectable in form and explicitly describes decay rather than error. Reusing that label as gold for mistake detection requires justification; registration alone does not supply it. This observation does not license post-hoc relabelling.

Saved-response recount and code fixes: [Codex review](../../../research/2026-09-29-codex-system1-judge-review.md). No new model calls were made.

## Controller erratum — RTD-13 and RTD-21 gold labels, 2026-09-29

**RTD-13 and RTD-21 have the same gold-label flaw as RTD-17, and the flaw is the controller's.** The gate scored every RTD correction pair as a correction to detect. In these two, the extracted negative does not reverse the extracted positive. RTD-13's negative calls `link_scan` the sole mechanism, which confirms what the positive describes; RTD-21's negative adds prose-side exceptions that do not make the positive's params counts false. Codex's derived labels mark both context-unresolved for this reason (`docs/research/2026-09-29-codex-system1-derived-adjudications.md`). These two and RTD-17 are exactly the cases where the judge's majority said "not a correction", so the judge may have been right on all three. The positive span was chosen as the sentence *near* a correction instead of the sentence the correction *reverses*; RTD-17's own entry warned about that shape.

**Not rescored.** Removing items after seeing results is not a licensed rescoring, and the gate stays as `gate.txt` records it. As a robustness check only: without the three, detectability would be 14/18 against a bar of 16/21, which could pass proportionally. But `yes_flags` would be 5/7 (only RTD-17 is in that check) against 6/8, and `control_fires` fails regardless. **The INCONCLUSIVE stop does not depend on these disputes.**

**One control shown to be incorrect.** Performing the check the derived labels suggest for CTL3-7 showed its premise was stale at the controls' tree: `CLAUDE.md` listed three default features, while `Cargo.toml` has four. Fixed in `c67f4552` (bug `96745f0ce9636580`). A never-corrected control is not a verified-correct one, which is the reason `control_fires` is reported as a firing rate.
