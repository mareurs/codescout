---
id: '0ca7439866e8f2b6'
kind: bug
status: open
title: 'BUG: the deep-agent observation window is declared in CLAUDE.md and nothing executes it — zero prospective samples in its first two days'
tags:
- cluster/declared-not-wired
topic: deep-agent-observation
---

# BUG: the deep-agent observation window is declared in CLAUDE.md and nothing executes it — zero prospective samples in its first two days

## Summary

The deep-agent observation window (opened 2026-09-18, mandated in `CLAUDE.md` § *Deep-agent observation window*) instructs every coordinating session to record its first eligible context decision and first substantive workflow. Two days in, both ledgers hold **zero prospective samples** — the only three entries are two historical seeds and one setup receipt, all written on day zero. The protocol is a well-formed declaration with no execution path: the only thing that can fire it is a model remembering to, which is a policy rather than a mechanism.

This matters beyond hygiene. The window is the stated gate on implementing the codescout deep agent (`docs/trackers/local-semantic-evaluator-design.md`), so an instrument that collects nothing does not merely lose data — it silently fails to arm the decision it exists to inform, and the 2026-09-25 review will read as "not enough evidence yet" rather than "the instrument never ran".

## Symptom (Effect)

Measured 2026-09-20T10:37:36Z against tree `4098ad3edeb15edee557f3f6ba66fddd17887858` (branch `experiments`):

```
$ git grep -c '^## DWF-'  HEAD -- docs/trackers/deep-agent-workflow-observations.md
HEAD:docs/trackers/deep-agent-workflow-observations.md:1
$ git grep -c '^## DCS-'  HEAD -- docs/trackers/deep-agent-workflow-observations.md
HEAD:docs/trackers/deep-agent-workflow-observations.md:1
$ git grep -c '^## DCX-' HEAD -- docs/trackers/deep-agent-context-observations.md
HEAD:docs/trackers/deep-agent-context-observations.md:1
```

Unit: **entry sections per ledger**, counted at that instant against that tree. All three are day-zero entries and none is a prospective sample:

| Entry | Heading | Self-declared sampling mode |
|---|---|---|
| DCX-1 | Historical seed — recipient-specific guide delivery | `historical-seed / retrospective; excluded from prospective collection counts` |
| DWF-1 | Historical seed — discriminate an edit-miss hypothesis | `historical-seed / retrospective; excluded from prospective collection counts` |
| DCS-1 | Collection setup — partial-session coverage | setup |

**Prospective sample count: 0.** Coverage receipts beyond the setup one: 0.

For scale, over the same checkout `CLAUDE.md` § *Reaching a Peer Session* describes roughly six concurrent sessions as routine, and the preceding 14-day baseline recorded 26,052 MCP calls — about 1,860/day — with no one deciding to record anything (`docs/research/2026-09-18-deep-agent-observation-baseline.md`).

## Reproduction

```
git rev-parse HEAD
date -u +"%Y-%m-%dT%H:%M:%SZ"
git grep -c '^## DCX-' HEAD -- docs/trackers/deep-agent-context-observations.md
git grep -c '^## DWF-'  HEAD -- docs/trackers/deep-agent-workflow-observations.md
git grep -c '^## DCS-'  HEAD -- docs/trackers/deep-agent-workflow-observations.md
```

Each returns `1`. Read the three sections; each is a seed or setup entry. The counts are expected to drift upward as the window runs — re-derive rather than citing these, and record the instant and tree alongside any new figure (`CLAUDE.md` § *Testing Discipline*).

## Environment

codescout `experiments` @ `4098ad3e`, Linux, MCP over the codescout server, profile `~/.claude-kat`. Window declared 2026-09-18, review 2026-09-25, capture ends 00:00 UTC 2026-10-02.

## Root cause

**The protocol has no call site.** `CLAUDE.md` § *Deep-agent observation window* declares what to capture, when, and in what shape, and both ledgers carry correct, detailed append recipes. Nothing dispatches any of it. There is no `PreToolUse`/`Stop` hook, no gate, no tool, and no scheduled job that either performs the capture or refuses to proceed without it. The sole execution path is a coordinating session noticing that the current moment is "the first eligible context decision" and choosing to spend a turn on it — and noticing competes directly with the task the session was actually asked to do.

That is `issue-clusters:IC-3` (`cluster/declared-not-wired`), promoted as `observer-blindness:OB-7` — *a declaration is well-formed, and nothing in production reaches it*. OB-7's own discriminator is the **remedy**, not the description: the remedy here is *find a caller*, which is IC-3's, not IC-15's round-trip-or-refuse. It is also the standing lesson `CLAUDE.md` attributes to `skill-frictions:SKF-22` — *a trigger the model must notice is a policy, not a mechanism* — applied to the very tracker built to study how context reaches a decision in time.

Measured 2026-09-20: the three counts above. The blind party is structural — the sessions that failed to sample are exactly the sessions that would have had to notice, and each of them read the instruction at session start.

## Evidence

### The ledgers' own framing predicts this failure and does not prevent it

`docs/trackers/deep-agent-context-observations.md` § *Sampling protocol* already anticipates non-participation: *"At session handoff/end, write one DCS coverage receipt in the companion ledger even if no opportunity was observed. Distinguish `none-observed` from `unknown/incomplete`. Missing receipts mean unknown participation, never zero incidents."*

That receipt is itself a policy requiring noticing, so it fails in the same direction as the thing it was meant to disambiguate. Two days in, no receipt distinguishes `none-observed` from `unknown` for any session, which is precisely the state the paragraph warns is uninterpretable.

### A second, independent instance of the same class, measured the same day

Probing native `Bash` in profile `~/.claude-kat` on 2026-09-20: a plain `echo` command **ran unblocked**. The codescout-companion guard declares a hard deny for that tool — its own header says it "Blocks native Bash/Grep/Glob/Read/Edit/Write in favour of codescout tools via permissionDecision:deny" and every branch calls an `enforce(...)` helper:

```
../claude-plugins/codescout-companion/hooks/pre-tool-guard.mjs
```

All three profiles carry `permissions.deny` empty or null with `defaultMode: "auto"` and allow lists of 472 / 183 / 536 entries. So a well-formed declaration of a deny exists and nothing in the runtime path reaches it — same class, different subsystem, found while investigating this bug. `CLAUDE.md` § *Companion Plugin* already records the general warning that for this guard "the source is not the ground truth here — the probe is"; this is a fresh datapoint for it, not a new discovery.

Recorded here as corroboration that the class is live in two places at once. **No configuration was changed**, and the fix for the permissions question is out of scope for this file.

### The substrate the window is collecting already exists, unattended

`UsageRecorder::write_content` (`src/usage/mod.rs`) persists `input_json` and `output_json` on every MCP call when debug capture is on, and the 2026-09-18 baseline measured them present on **25,435 of 26,052 calls (97.63%)** across the preceding 14 days. That corpus accumulated with nobody remembering anything.

## Hypotheses tried

1. **Hypothesis:** the window simply has not had time to collect. **Test:** compare elapsed window time against the sampling rule — the rule is *first eligible decision per coordinating session*, not per week, so any session at all should have produced an entry. **Verdict:** rejected. Two days at roughly six concurrent sessions is many sessions, and the rule fires per session.
2. **Hypothesis:** entries exist but live somewhere else (params rather than body). **Test:** both ledgers declare themselves prose ledgers with `body_is_canonical: true` and `entry_collection: null`; the context ledger states *"This is a prose ledger with prefix DCX; `params` holds collection settings only. Do not create a second observation array."* **Verdict:** rejected — the body is the canonical store and it holds three entries.
3. **Hypothesis:** an existing bug file already covers this. **Test:** `doc(action="find", kind="bug", filter={"status": {"in": ["open","taken","investigating","zombie"]}})` at 60 open records, scanned for observation/ledger/window/sampling terms. **Verdict:** rejected — no existing record covers the window's yield. Ledger checked.

## Fix

**Not implemented. Candidate direction only, recorded so the 2026-09-25 review has something to decide between rather than a blank.**

The window's *question* — when does context help, arrive too late, mislead, or add nothing — is sound and worth keeping. Its *instrument* is the defect. Two directions, neither costed:

1. **Derive rather than collect.** Reconstruct per-session call sequences from the existing `usage.db` capture, which already holds arguments and results on 97.63% of calls and requires no one to remember. Hand-written DCX/DWF entries then stop being the dataset and become adjudication labels for cases the query surfaces — far fewer entries, each worth more.
2. **Give the protocol a call site.** A hook or gate that performs (or refuses to proceed without) the capture, so compliance is the default path rather than an act of memory.

**The bound on direction 1, stated because it is the thing that makes it partial rather than a replacement:** `usage.db` records **MCP calls only**. Native `Bash`, `Read` and `Edit`, the host's own prompts and reasoning, and task outcomes are all invisible to it. A zero there is evidence about the instrumentation, not about work performed — the baseline's own § *Interpretation limits* says so. So a derived corpus answers the retrieval and timing questions far better than the current instrument and still cannot answer the outcome question without something else supplying it.

Choosing between these, or extending the window, is a decision for the review date, not for this file.

## Tests added

N/A — nothing was changed. A regression test is not meaningful for a defect whose subject is a documented protocol with no executable component; the check that would catch a recurrence is a count of prospective entries against elapsed window time, which is a review-time question rather than a suite-time one. If direction 2 is taken, the hook that lands is what becomes testable.

## Workarounds

None that survive the class. Asking sessions to try harder is the policy that already failed. Until an instrument lands, treat any statement about the window's yield as a statement about the instrument, and read a low prospective count as *unknown participation*, never as *few eligible moments* — the ledger's own § *Sampling protocol* makes exactly this distinction and nothing currently supplies the receipt that would settle it.

## Resume

Before the 2026-09-25 review: re-derive the three counts with the commands in § Reproduction, recording the fresh instant and tree, and check whether any prospective entry has landed since 2026-09-20T10:37:36Z. If the count is still zero, the review's question is not "is the data good enough yet" but "which of the two directions in § Fix do we take" — say so explicitly in the review note so elapsed calendar time is not read as collection effort. Do not extend the capture window as a first move: the window's length was never the binding constraint.

## References

- `CLAUDE.md` § *Deep-agent observation window* — the declaration
- `docs/trackers/deep-agent-context-observations.md` — DCX ledger
- `docs/trackers/deep-agent-workflow-observations.md` — DWF / DCS ledger
- `docs/research/2026-09-18-deep-agent-observation-baseline.md` — the 26,052-call baseline and its interpretation limits
- `docs/trackers/local-semantic-evaluator-design.md` — the design this window gates
- `docs/trackers/issue-clusters.md` — `IC-3`, `cluster/declared-not-wired`
- `docs/trackers/observer-blindness.md` — `OB-7`, the promoted class
- `src/usage/mod.rs` — `UsageRecorder`, the unattended substrate
