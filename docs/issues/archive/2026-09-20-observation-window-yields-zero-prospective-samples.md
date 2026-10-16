---
id: 39c6b3b95f6b9b82
kind: bug
status: wontfix
title: 'BUG: the deep-agent observation window is declared in CLAUDE.md and nothing executes it — zero prospective samples in its first two days'
tags:
- cluster/declared-not-wired
topic: deep-agent-observation
closed: 2026-10-16
no_fix_commit: 'superseded: capture window ended 2026-10-02 and the declaration left CLAUDE.md in 2edf6486; no code defect, nothing committed to fix'
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

**Closed 2026-10-16 as superseded (wontfix) — user decision via coordinator, option A. No code change.**

Verified at the bytes before closing:

- The declaration this defect is about no longer exists. `grep -n "Deep-agent|observation window" CLAUDE.md` returns 0 matches on `experiments` @ `f5f1c6f7`; the section was moved out of `CLAUDE.md` by `2edf6486` (`docs(claude-md): stage 2g`).
- The capture window itself ended at 00:00 UTC 2026-10-02 (stated in this file's own `## Environment`), 14 days before this closure, and the 2026-09-25 review date has passed.
- The defect is a process one (a protocol with no call site), and its subject is not in `src/usage/mod.rs`, which the file cites only as the unattended *substrate* (`UsageRecorder` capture of `input_json`/`output_json`). Nothing in that file is wrong.

What survives, deliberately not acted on here: the two candidate directions above (derive a corpus from `usage.db`; give a protocol a hook call site) remain valid *design* options for any future observation instrument, and the lesson — "a trigger the model must notice is a policy, not a mechanism" (`issue-clusters:IC-3`) — is already recorded in the cluster ledgers. If a new window is opened, build its instrument first.

## Tests added

N/A — closed as superseded; nothing was changed. The window ended and its declaration was removed from `CLAUDE.md`, so there is no executable component left to test.

## Workarounds

None that survive the class. Asking sessions to try harder is the policy that already failed. Until an instrument lands, treat any statement about the window's yield as a statement about the instrument, and read a low prospective count as *unknown participation*, never as *few eligible moments* — the ledger's own § *Sampling protocol* makes exactly this distinction and nothing currently supplies the receipt that would settle it.

## Resume

Nothing outstanding. Re-open only if a new observation window is declared without an instrument.

## References

- `CLAUDE.md` § *Deep-agent observation window* — the declaration
- `docs/trackers/deep-agent-context-observations.md` — DCX ledger
- `docs/trackers/deep-agent-workflow-observations.md` — DWF / DCS ledger
- `docs/research/2026-09-18-deep-agent-observation-baseline.md` — the 26,052-call baseline and its interpretation limits
- `docs/trackers/local-semantic-evaluator-design.md` — the design this window gates
- `docs/trackers/issue-clusters.md` — `IC-3`, `cluster/declared-not-wired`
- `docs/trackers/observer-blindness.md` — `OB-7`, the promoted class
- `src/usage/mod.rs` — `UsageRecorder`, the unattended substrate
