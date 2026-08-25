---
id: 1f2ecf2e460dde23
kind: bug
status: fixed
title: check_hidden scores a CLI error envelope as a content failure, fabricating a codescout landslide
tags:
- eval
- hidden-info
- checker
- measurement-validity
closed: 2026-08-25
opened: 2026-08-25
owner: marius
related:
- docs/superpowers/plans/2026-08-23-hidden-information-eval.md
severity: high
---

# BUG: `check_hidden` scores a CLI error envelope as a content failure

## Summary

When the Claude Code CLI fails before making an API call, it emits a result envelope with
`is_error: true` and zeroes everywhere. `check_hidden.py` parses that envelope as though it
were a model answer, finds no `## FINDINGS` block in it, and returns
**`FAIL(no-findings-block)`** — an *arm-accountable content class*. The run then enters the
aggregate as a legitimate zero. Discovered live during the Task 5 pilot, where it made a
totally-failed `hidden-native` arm look like an arm that searched and found nothing.

## Symptom (Effect)

`main-native/hidden-native.log`, both runs:

```
VERDICT FAIL(no-findings-block)
TOKENS prompt=0 output=0 turns=0 calls=0 guidechars=0
PROMPT_PER_TURN 0
COST_USD 0
TOOLS (none)
GUIDES (none)
```

The response body the checker was handed was not prose — it was this:

```json
{"is_error": true, "duration_api_ms": 0, "num_turns": 1,
 "terminal_reason": "api_error", "total_cost_usd": 0,
 "result": "Failed to authenticate: OAuth session expired and could not be refreshed"}
```

## Reproduction

1. Point an arm's `config_dir` at a Claude Code profile whose OAuth credential cannot
   refresh.
2. Run that arm. The CLI returns the envelope above; `assertions.py` hands it to the
   checker as `PROMPT_TDD_RESPONSE`.
3. The checker returns `no-findings-block`.

Reproduced twice at `git rev-parse HEAD` = `83f15cb`+ (post-R-29 tree), 2026-08-25.

## Root cause

`check_hidden.py`'s `make_predicate` has no notion of *"the harness never ran the model."*
Its instrument gates all check the harness's **own** bookkeeping — manifest present, facts
complete, log lines present, arm matched — and every one of them passes here, because
`surface_lib.collect_facts()` returns a dict literal whose keys are all unconditional and
simply carry zeros. Nothing inspects the response body for an error envelope. Measured
2026-08-25 by reading the logged facts block of both failed runs.

The `indeterminate` machinery is present and correct; this failure mode was simply never
enumerated. It is the same class as C4 from the Task 3 review — a run whose verdict should
not be arm-accountable — but arriving through the response body rather than through the
facts dict.

## Evidence

### The aggregate this would have produced

`hidden-cs` scored `recall 0.75 / precision 0.90 / f1 0.8182` on run 1. `hidden-native`
would have entered as `f1 = 0.0` twice. Gate 4 asks for
`|F1(hidden-cs) − F1(hidden-native)| >= 0.10`; it would have read **0.82** and passed
triumphantly. Gate 1 ("neither arm mean F1 < 0.15") would have failed and flagged it — but
gate 1 failing is documented as *"tune the fixture"*, not *"the arm never ran"*, so the
prescribed response would have been to make the task easier.

### Why the existing gates did not catch it

`have_trace` is `true` — `assertions.py:532-537` writes a well-formed trace document even
with no trace. `tool_names` is `[]`, which the R-29-era `no-tool-data` gate catches **only
for `arm == "cs"`**; the native arm has no such gate, by design, because a native run
legitimately reports no MCP tools.

## Hypotheses tried

1. **Hypothesis:** the native agent genuinely ignored the output-format instruction.
   **Test:** read the logged response body.
   **Verdict:** rejected — the body is a JSON error envelope, not prose, and
   `duration_api_ms` is 0.

## Fix

**Fixed 2026-08-25.** `_cli_error()` in `scenarios/hidden-info/check_hidden.py` gates on the
result envelope itself and returns `indeterminate:cli-error`, with the CLI's message in
`facts["indeterminate_detail"]`. Placed as the FIRST instrument gate — above the
re-score/live split and above the ground-truth gate, so it holds on a machine with no answer
key — and keyed on `text`, which is the model-response half on both paths.

It gates on the **envelope**, never on `cost_usd == 0` / `turns == 0` / empty `tool_names`.
Those are downstream symptoms a legitimately cheap or tool-less run can share, and gating on
them would make the new verdict swallow real content failures. That alternative was
implemented as a mutation and killed by the tests.

**Fix commit** — this repo is `prompt-engineering`, not codescout, so the reference is
prefixed per the cross-repo discipline in memory `gotchas`:

| | |
|---|---|
| SHA | `prompt-engineering:1e337988688be01180789cc7071e70a51bbe1de1` (branch `master`) |
| patch-id | `746bf2992e5e7d9009598289003f5b65f3ba17e1` |

`master` in that repo is not rebased, so the SHA is durable there — the patch-id is recorded
anyway, because a content hash costs nothing and outlives any history rewrite.
## Tests added

Five, in `scenarios/hidden-info/test_check_hidden.py` (suite 130 → **135**):

- `test_cli_error_envelope_is_indeterminate_not_a_content_class[cs]` / `[native]` —
  parametrised over both arms, using the exact body both `hidden-native` runs produced.
  Deliberately does **not** take the `gt_file` fixture, so it proves the gate returns above
  the ground-truth gate.
- `test_cli_error_gate_does_not_swallow_a_genuine_no_findings_answer` — the boundary. Real
  prose omitting the block stays arm-accountable.
- `test_cli_error_gate_does_not_fire_on_an_answer_that_merely_contains_json` — a findings
  block is still scored when the prose happens to include JSON.
- `test_cli_error_is_detected_on_the_rescore_path_too` — built through
  `surface_lib.FACTS_BEGIN`/`FACTS_END` with a real six-line tail.

**Both mutations killed:** deleting the gate, and gating on `cost_usd == 0` instead of the
envelope. Each kills the three cli-error tests while leaving the two boundary tests green.

**Verified against the real data**, not only fixtures: the actual halted-pilot log re-scores
`FAIL(indeterminate:cli-error)=2`, where it previously read `FAIL(no-findings-block)`.
## Workarounds

Read `TOOLS`, `TOKENS` and `COST_USD` before believing any arm's score. A run with
`prompt=0 output=0 turns=0 calls=0` and `COST_USD 0` did not happen, whatever its verdict
says.

## Resume

N/A — fixed and verified.

One follow-on, tracked elsewhere: a mutation harness making these checks first-class tests
rather than manual per-round exercises was dispatched 2026-08-25 and is recorded in the SDD
ledger's `## CURRENT STATE` under **IN FLIGHT**.
## References

- `.superpowers/sdd/2026-08-23-hidden-information-eval/progress.md` — R-30 records the
  auth blocker that surfaced this.
- `docs/superpowers/plans/2026-08-23-hidden-information-eval.md` § Task 5 — the four gates
  this defect would have corrupted.
