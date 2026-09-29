---
id: d992b5fc8c2a92d3
kind: research
status: draft
title: Codex — activating friction lessons through three distinct roles
tags:
- codex
- rule-tell
- reconnaissance
- friction
- architecture
---

# Codex — activating friction lessons through three distinct roles

**Valid:** dated 2026-09-26

**Status:** design direction. The operator explicitly endorsed the separation into three roles below and asked to preserve this discussion. The intervention examples and pilot are proposals; their benefit has not been measured.

**Origin:** Codex sparring discussion after the Phase 1b reviews. Saved with checkout HEAD `91221154`; this is a conversation/design record, not a review of that commit.

## Intended outcome

Turn accumulated project experience into concrete checks at the moment a decision needs them. Reduce recurring mistakes, repeated investigations and the operator's need to recognise a failure class, request another review, and relay the lesson between sessions.

The existing promotion paths are documented in `docs/TAXONOMY.md`:

incident → F/W/R/T entry → lesson → CLAUDE.md / skill / server prompt / hook → verification.

Publishing a lesson does not establish that a session receives it, recognises its relevance or applies it correctly. `CLAUDE.md` § Observer Blindness records failures repeated even by authors actively writing about the class. The opportunity is to connect an observable decision with a relevant precedent and an actionable verification.

## Agreed separation: three roles

| Role | Responsibility | Output | Boundary |
|---|---|---|---|
| **1. Deterministic checks** | Check properties with explicit predicates: identities, revisions, missing fields, set completeness, execution order, known invariants. | A check result with its evidence, population and scope; a concrete remedy where available. | A mechanical result only establishes the property actually checked. Use this role wherever a reliable predicate exists. |
| **2. Fast model** | Recognise situations that may need a lesson or verification: a conclusion broader than its evidence, a likely reconnaissance seam, a recurring recovery pattern. | A candidate situation/rule, the relevant observed span or event, and a suggested verification or precedent to retrieve. | Its signal is a hypothesis. It does not establish a violation, context completeness or task success; abstention is valid. |
| **3. Main agent and tools** | Adjudicate the hypothesis against the actual artifact and current task. Retrieve the relevant context, run the appropriate check, and decide whether to change the next action. | An evidence-backed disposition and, where justified, a corrected action or conclusion. | Re-reading a belief is not verification. Keep self-report separate from independently observed outcomes. |

This is a separation of responsibilities, not a requirement to send every event through three sequential services. A deterministic check can settle a mechanical case directly. The fast model is valuable where applicability requires interpretation. Expensive investigation is reserved for a concrete unresolved question.

The current Jev/rule-selector work explores role 2. Progress on its classifier does not by itself establish that the whole intervention loop helps.

## Agreed extension: System 2 can direct System 1

**Endorsed by the operator, 2026-09-26.** Alongside automatic detection, System 2 should know that System 1 is available and be able to give it simple, temporary instructions in ordinary task language. System 2 knows the objective and anticipated risks; System 1 can keep watching for those risks as work progresses.

The intended benefit is delegated attention: System 2 states a concern once instead of continually carrying and re-establishing it through a long session.

### A small bidirectional interface

| System 2 can say | Meaning |
|---|---|
| “Watch for…” | Add a temporary concern scoped to the current task. |
| “Check whether…” | Request a bounded check now. |
| “What did you observe?” | Retrieve evidence and unresolved signals. |
| “This is expected here…” | Refine or dismiss a task-specific warning. |
| “Stop watching…” | End that temporary instruction. |

These are interaction examples, not a frozen API schema. System 2 should not need to know model architecture, thresholds, database structure or routing. It does need a clear acknowledgment of what was accepted, what can be observed and what a returned result establishes.

### Example exchange

> **System 2:** “For this investigation, watch for conclusions based on empty search results. Remind me to check the search scope before accepting them.”
>
> **System 1:** “Watching for that until this investigation ends.”
>
> **Later, at an observable decision point:** “You concluded that there are no callers after a text search. A references lookup could check that conclusion.”

A second use is investigation continuity:

> “We have already investigated this failure. If my next steps repeat the earlier investigation, bring back its result and tell me what has changed.”

The acknowledgment must reflect actual capability and observation coverage. If the harness cannot observe a decision before it occurs, it must not promise a pre-action intervention.

### How this preserves the three roles

System 2 expresses the concern and scope. The receiving mechanism translates that request into supported deterministic checks, fast-detector capabilities, or retrieval of relevant precedents. System 2 receives a concise signal with evidence and adjudicates it using tools.

The current classifier recognises defined rule families. A natural-language request does not create a new classifier capability. Unsupported requests must be surfaced or explicitly routed to an appropriate different instrument; they must not receive a reassuring but empty acceptance. The routing details can remain hidden while the capability contract stays visible.

### Lifecycle, silence and feedback

- Temporary instructions expire with the task and can be stopped or refined explicitly. They do not silently become permanent project policy or replace standing checks.
- “No matching signal observed” differs from “this property was verified.” Silence never certifies that everything was checked.
- System 2 can report “that warning was useful” or “the evidence was already present here.” Record those as feedback, with the relevant evidence, rather than automatically treating acceptance or dismissal as ground truth.
- Preserve independently observed outcomes separately from the agent's self-report. Feedback can inform later evaluation and improvement without silently relabelling the training corpus.

**The reciprocal loop:** System 1 brings situations to attention; System 2 tells it which situations matter now.

## Proposed extension: a Codescout-specialised background miniagent

**Requested by the operator, 2026-09-26.** Extend System 1 with a small, tool-using agent specialised for Codescout. System 2 can delegate simple tasks in ordinary language, continue its own work, and receive a completion result later. Candidate work includes updating trackers, bounded refactoring and running specified tasks or checks.

The intended benefit now includes delegated execution as well as delegated attention: move suitable work out of System 2's active reasoning loop while keeping the handoff and review cheaper than doing the work itself. Qwen is the operator's starting candidate; another model may be preferable. Neither the model choice nor a benefit from fine-tuning is established by this proposal.

### One simple interface, distinct watching and working modes

| Mode | Example instruction | Expected return |
|---|---|---|
| Watch | “Watch for conclusions drawn from empty searches during this investigation.” | A relevant signal with observed evidence. |
| Maintain a tracker | “Record this verified finding in the session tracker, with these evidence links.” | The entry ID and link, what was written, and any unresolved ambiguity. |
| Refactor | “Rename this internal helper and update its references in this module, preserving behavior.” | A scoped patch, the checks performed and their results, and remaining concerns. |
| Run a task | “Run this specified evaluation on this frozen input and report the result.” | The input/code identity, execution status, output artifacts and observed results. |

These are proposed task families, not capabilities already demonstrated by the current model. System 2 should express the objective, scope and what counts as done without needing to know the worker's model, queue or training recipe. The service must surface missing information and unsupported tasks before promising execution.

The always-available watcher and the background worker have different jobs and latency needs. They can share a service and perhaps a base model, while using different prompts, adapters or models if measurement supports that choice. A long refactoring job must not silently interrupt observation. Whether to share model weights or execution capacity remains an implementation question.

The current rule classifier does not become a tool-using agent merely by receiving new instructions. Background execution requires a generative model with an execution harness, access to the necessary tools and demonstrated competence on the delegated task families.

### Delegation and return

Accepting a task should produce a durable task ID and an explicit scope. The background lifecycle needs to distinguish queued, running, blocked, failed, cancelled and completed work, with status available while System 2 continues. Completion should reach the originating task or session; a result must remain retrievable if that session has ended or restarted.

A useful return contains what changed or ran, the relevant artifact links or diff, which checks actually ran and what they established, and anything still unresolved. “The worker finished” is an execution event; it is not independently verified task success.

Mutating jobs need an explicit ownership boundary. For code, an isolated checkout or a patch against a named base can keep concurrent work separable; changes to that base require reconciliation before application. Tracker writes should use the managed artifact interface and the current artifact identity, preserving peer edits. Retries need to recognise prior completion so that a repeated delivery does not duplicate an entry or repeat a mutation.

The worker inherits only the actions and scope delegated to it. A bounded, supported job can complete within that scope and return evidence; ambiguity or a need to expand the task comes back to System 2. Delegation should not require a new human approval for every routine action already authorised.

### Preserve the three responsibilities

The three roles above remain responsibility boundaries, rather than a mapping of each role to exactly one model. The fast detector proposes relevant situations. The specialised worker can perform some of the retrieval, checking and execution previously assigned to the main agent and tools. Deterministic instruments still establish their specific properties, and System 2 remains responsible for accepting the result in the larger task.

The worker's statement that its own change is correct remains self-report. Tests, structural checks, artifact inspection and other appropriate instruments provide separate evidence; the same model declaring itself successful does not close the verification loop.

### What fine-tuning should teach, and what to measure

The proposed specialisation is Codescout workflow competence: selecting and using its tools, navigating artifacts, making scoped edits, maintaining tracker conventions, recovering from tool refusals, and returning inspectable results. Current repository facts and changing rules should still come from live tools and retrieval.

Training examples should preserve task instructions, the context available at each decision, tool actions and results, corrections, and independently checked outcomes. Existing traces and trackers are candidate sources requiring adjudication; a plausible completion message or a retrospective diagnosis is not automatically a successful training trajectory. Detector labels and execution trajectories have different contracts and need separate evaluations.

Begin with a narrow task family with a clear completion check. Compare System 2 doing the work directly, a prompted small worker, and a fine-tuned worker on held-out tasks. Measure verified completion, review and repair effort, total latency and cost, duplicate or conflicting writes, and interference with watcher responsiveness. Use recent operational episodes for the baseline and keep related tasks or incidents together when splitting evaluation data.

The practical test is whether delegation saves System 2 and the operator more work than dispatch, supervision and repair consume. Tracker maintenance, task execution and refactoring should earn that conclusion separately.

## Concrete interventions worth exploring

| Observed situation | Useful intervention |
|---|---|
| “There are no callers/usages,” after an empty search | Check whether the instrument covers the relevant namespace; retrieve the appropriate structural lookup and precedent. |
| “The freeze guarantees 50 positives” | Name the counted population and check emitted rows, including filters that run after the current count. |
| “The tests pass, therefore the filter works” | Propose a discriminating mutation and inspect whether the relevant tests fail. |
| A refused tool call is repeated without recovery | Supply the applicable remedy and observe whether the subsequent action recovers. |
| An investigation appears to repeat settled work | Retrieve the earlier result, its limits and the facts that need re-verification today. |

These are candidate actions, not claims that the current detector can reliably recognise or repair every case.

## Context sufficiency has a narrower observable contract

A model reading a sentence cannot know that an agent extracted “all context.” A tractable signal is:

> This particular claim requires evidence that is not present in the observable trace.

Even that signal is scoped to the trace: missing recorded evidence is not proof that the agent never obtained it elsewhere. Preserve distinctions between context available, delivered, observed in use, sufficient for a named decision, and an independently checked outcome.

Attach a small relevant precedent and a concrete next check when intervention is justified. Merely injecting another general warning can reproduce the existing failure to apply already-known rules.

The intended loop is:

recognise situation → retrieve precedent → perform the appropriate verification → observe the outcome → improve the mechanism.

## Operator decision — broader useful interventions (2026-09-29)

**Delegation and nuisance threshold, approved 2026-09-29:** Extrapolate these decisions to other examples and ask the operator when policy is ambiguous. Record derived labels separately from individually human-approved examples. For an uncited causal explanation, request supporting evidence only if the current decision depends on it or nearby evidence conflicts; missing citation alone is insufficient. The operator selected this threshold explicitly. Applied corpus: `docs/research/2026-09-29-codex-system1-derived-adjudications.md`. These development labels do not revise the completed gate.

**Qualification threshold, approved 2026-09-29:** the same bar applies to qualifications. Suggest one only when the qualified form would change a decision or action a reader takes from the passage, when nearby evidence conflicts with the unqualified form, or when mutable state is stated as current without the instant and identity a later reader needs (example A). Broad or absolute wording whose qualified form leads to the same action is not a trigger, and scope the delivered context already supplies is not missing. Applied in the derived-labels corpus's *Amendment — Claude review*: ten qualify-only rows became no-intervention.

**Approved scope:** In response to the choice between catching demonstrable mistakes only and also suggesting useful checks and qualifications before mistakes occur, the operator chose the broader role: “yes, the broader is better.”

The target is a useful, specific intervention that can improve the next decision or the reliability of a durable record. A warning may be useful even when the statement is currently true. This extends applicability; it does not give the fast model authority to declare its hypothesis proven.

**Proposed annotation distinctions, not yet a frozen schema:**
- Error: available evidence contradicts the claim.
- Needs verification: the claim exceeds the supplied evidence; suggest a specific check.
- Needs qualification: a date, scope, attribution or uncertainty statement materially changes how the reader should use it.
- No intervention needed: no useful corrective/checking action is warranted on the supplied context.
- Cannot tell: the supplied context does not support adjudication.

Keep factual status, intervention usefulness and delivery urgency separate. More than one need may apply. Missing trace evidence is not automatically an error, and admitting uncertainty is not automatically a reason to issue a warning. A suggested check already completed or scheduled may add no value.

**Routing established by the approved examples:** Send useful checks and qualifications to System 2; alert it immediately before a visibly false claim is sent. Involve the human only when resolution requires their input. Stay silent for a check already scheduled as the next action and for an accurate historical snapshot not being treated as current. Retrieve missing evidence before considering another test run; unavailable evidence means unverified, not false. Broader escalation cases and enforcement mechanisms are not settled by these examples.

### Human worksheet — development examples, not held-out evidence

For each example choose: leave alone / quietly suggest to System 2 / interrupt before proceeding. Optionally replace the suggested action. These are illustrative policy examples and must not become held-out test cases. All six cases (A–F) were individually approved by the operator on 2026-09-29. These approvals establish the illustrated policy, not measured model performance or an implementation schema. Example A is RTD-17's own positive from `docs/evals/rule-tell-detection.md`, so RTD-17 is an approved exemplar rather than a derived label.

| Case | Context visible at the decision | Candidate action | Human choice |
|---|---|---|---|
| A | A tracker records “the columns do not exist”; a current schema check supports it, but no date or database identity is recorded. | Add the observation date and database identity. | Approved 2026-09-29: quietly suggest to System 2; no human interruption. Classify as qualification, not factual error. |
| B | An agent concludes an edge case works because the entire suite passed; no evidence connects a test to that edge case. | Inspect the relevant assertion before making the narrower claim; run a targeted check only if still needed. | Approved 2026-09-29: suggest verification to System 2 before the claim; additional testing only if needed, human involvement only when their input is required. |
| C | An agent says “I have not checked the schema; I will check it next,” and that read is already its next action. | No duplicate reminder. | Approved 2026-09-29: stay silent while the check is already the next action. Intervene if the agent skips it and makes a claim depending on it; reassess against available evidence at that point. |
| D | An agent says all tests passed, while the visible result contains failures; it is preparing to publish that claim. | Correct the claim and inspect failures before publishing. | Approved 2026-09-29: immediately alert System 2 before the false claim is sent. System 2 checks the failures and corrects the claim; involve the human only when a decision requires their input. |
| E | An agent says tests passed, but the supplied trace omits test output; the result may exist outside the trace. | Retrieve the result if accessible; otherwise report inability to verify, without claiming tests failed. | Approved 2026-09-29: ask System 2 to locate the result. If unavailable, mark the claim unverified, not false. An incomplete trace alone does not justify rerunning tests. |
| F | A dated tracker snapshot explicitly names the database and observation time; no later decision relies on it as current. | Leave the accurate scoped observation alone. | Approved 2026-09-29: stay silent. Age alone does not make an explicitly scoped historical record wrong or require a fresh check when nobody relies on it as current. |

**Measurement consequence:** Report error detection, useful verification suggestions, useful qualifications, abstentions and unnecessary interventions separately. Acceptance of a suggestion does not prove benefit; retain independent outcome checks and count the extra work it causes.

This is a prospective clarification, not a relabelling of the completed gate. Its INCONCLUSIVE result stands. Before another expensive judge run, adjudicate examples under the clarified contract, then register fresh held-out cases and thresholds. The existing gate and this worksheet are development material. Review provenance: `docs/research/2026-09-29-codex-system1-judge-review.md`.

## Constraints inherited from the existing work

- **More rules are not automatically more effective.** `docs/evals/reconnaissance-output.md` records how treatment pass counts overstated the skill's contribution; paired controls showed a much smaller and noisier gain.
- **More alerts are not automatically less friction.** Historical friction research found that some frequent tool refusals produced prompt, correct recovery. Use recovery and downstream cost, not error frequency alone.
- **A tracker is not directly a training set.** It contains retrospective diagnoses, selected memorable incidents, duplicates and retractions. Reconstruct what was observable before the decision; do not leak the later fix into the detector's input.
- **Promotion is not outcome verification.** A lesson being present in CLAUDE.md, a skill or a memory proves availability on that surface, not behavioral benefit.
- **Silence is not success.** Record useful interventions, false alarms, no-change cases, sampled non-firings and unknown outcomes. An annotation disappearing does not establish that the problem was fixed.

## Agreed priority: all three outcomes, measurement first

**Operator clarification, 2026-09-26:** pursue all three outcomes: fewer mistakes and repeated reviews, relevant context at the point of decision, and transfer of lessons across sessions and projects. Establish a baseline before choosing the first intervention. Sequencing the experiments does not remove any of these objectives.

### Measure the outcomes separately

| Outcome | Baseline question | Evidence of improvement |
|---|---|---|
| Fewer mistakes and repeated reviews | Which independently confirmed mistakes recur, and how much review, recovery and repair do they require? | Fewer confirmed mistakes per comparable task or decision opportunity, with lower review and repair effort and no loss of completion quality. |
| Context at the right moment | For a named decision, was the necessary evidence available, delivered before the decision and visibly used? | More decisions supported by timely relevant evidence; record unnecessary retrieval, interruptions and unresolved sufficiency separately. |
| Transfer of lessons | When a known lesson is applicable in a later session or another project, is it retrieved and applied with its scope checked? | Less repeated investigation and fewer recurrences among eligible opportunities, with successful adaptation independently checked. |

The miniagent adds an execution comparison: verified task completion, total cost and elapsed time, and the main agent's dispatch, supervision and repair effort. Background activity alone establishes no saving; overlapping task durations must not be summed and presented as elapsed time saved.

### First deliverable: a baseline and an observability map

1. Freeze a common seven-day window with explicit UTC start and end instants. Record project roots, tool/build versions, input snapshots and retention gaps. Older incidents supply precedents rather than current frequency estimates.
2. Start from the existing instruments indexed in `docs/PROBES.md`. Check their current predicates and live schema before reusing them; historical warnings about fields or grouping keys are not a current schema inspection. Align event definitions before comparing outputs.
3. Reconstruct episodes from `usage.db`, available Claude traces and linked tracker evidence. Use demonstrated identities for joins and state their coverage. Keep unmatched episodes visible; missing trace evidence is unknown, not proof that an action never happened.
4. Specify each metric's unit, eligible population and outcome check. Separate candidate signals, agent feedback and independently verified outcomes. Audit representative ordinary episodes as well as known failures so that the failure trackers do not become the denominator.
5. Publish what is measurable now, what needs adjudication and what is currently unobservable, with evidence links and uncertainty. Add telemetry only for a named unresolved measurement whose result will inform a decision.

The observation chain is: opportunity → signal or request → context delivery or delegated action → observed use → independently checked outcome. Preserve timing and identity across it. A retrieved lesson, a completed tool call or an accepted alert is an intermediate event.

### Choose and evaluate interventions after the baseline

Use frequency, observed cost, addressability and measurement confidence to choose the first bounded experiments across the objectives. A shadow run can estimate candidate coverage and alert burden; it cannot establish errors prevented or effort saved.

Then compare ordinary work with the intervention on comparable held-out tasks, keeping related incidents together and declaring the comparison before inspecting outcomes. Report each outcome and its costs separately. Detector accuracy, retrieval success and worker completion are component measures; the end-to-end question is whether the work becomes more reliable and requires less total effort.

## Proposed first pilot

After the measurement baseline identifies suitable opportunities, choose three concrete recurring lessons with useful, bounded verification actions. This is a candidate first intervention pilot within the broader objectives above; its selection follows measurement. Compare the ordinary agent with the same agent receiving the targeted intervention. Use episodes from the latest seven days for the current operational baseline, with relevant code/tool versions recorded; older incidents can supply precedents without being treated as current usage rates.

Measure additional mistakes avoided, unnecessary interventions, recovery effort and time to resolution. Separate detector accuracy from intervention benefit. Evaluate on distinct incidents and the evidence available before the outcome.

The operator's hoped-for large gain remains a hypothesis: less repeated human review and less rediscovery, while preserving useful autonomy. A more cautious-sounding final answer alone does not establish that gain.

## Sources and adjacent work

- `docs/TAXONOMY.md` § Promotion ladder — routes from F/W/U/H/R/T into durable mechanisms.
- `CLAUDE.md` §§ Testing Discipline and Observer Blindness — evidence limits and checks that run without somebody remembering.
- `docs/trackers/reconnaissance-patterns.md` — especially the artifact, instrument and existing-record laws.
- `docs/trackers/test-escape-hardening.md` — moving review lessons into standing mechanisms.
- `docs/trackers/tool-usage-patterns.md` — observed tool behavior feeding server instructions.
- `docs/trackers/deep-agent-context-observations.md` § Episode fields — timing, pre-action evidence, delivery, outcome and counterfactual distinctions.
- Codescout memory `infra/friction-measurement` — historical recovery measurements and instrument limitations; not a current usage census.
- `docs/evals/reconnaissance-output.md` — paired evaluation and the correction of the pass-count headline.
- `docs/research/2026-09-26-codex-phase1b-stage1-review.md` — most recent Codex model/harness review preceding this discussion.
- `docs/trackers/2026-09-21-codex-telemetry-research-handoff.md` — cross-session research context.
