---
id: '58355dcbbe21b945'
kind: spec
status: draft
title: Generic evidence-gated repair workflow and outcome-blind comparison
tags:
- effort-steering
- generic-mechanism
- prospective-design
topic: effort-steering
---

# Generic evidence-gated repair workflow

## Status and authority

Design recorded 2026-11-09 under the operator's standing delegation to find a working generic mechanism. The operator explicitly reiterated that the mechanism must be generic. The policy and experimental choices below are the executor's, not individually selected by the operator. This document specifies a candidate; it does not establish efficacy or mark a paid campaign ready. No new model run, source change, merge or push is performed by writing it.

## Goal

Improve independently evaluated end-to-end repair quality of low effort at lower complete-workflow model cost than xhigh. Success means a correct production repair that preserves legitimate behavior. Test counts, protocol compliance, abstention and a passing self-authored suite are secondary observations.

## What generic means

The policy, prompts, phase transitions and evidence schema are identical across projects. They consume the public request, source and public documentation, existing source-only onboarding, available tools and a project-supplied command adapter. Task identifiers are opaque bookkeeping keys, never policy selectors. Language-specific command execution belongs in the adapter, not in semantic policy.

No repository/function names, known missing examples, private expected statuses, reference patches, outcome labels, per-task hints or manually authored task-specific invariants appear in the policy. The actor may derive project-specific hypotheses from its public inputs; the controller cannot supply them from evaluation knowledge. Public command configuration is necessary portability information, not an oracle. Initial Rust adapter support would limit runtime coverage, not establish cross-language efficacy.

## Prior work and novelty boundary

| Earlier mechanism | Established result | Consequence |
|---|---|---|
| Fixed checker and repair feedback | Functional checks can drive a loop; acceptance populations needed correction | Reuse execution/accounting; do not treat public green as semantic correctness |
| Blind challenger and contract-to-assertion | Executed assertions could miss the defect; an inverted assertion control could still miss the relevant behavior | Assertiveness and test sensitivity do not establish coverage |
| Semantic-contract repair pilot | Prompt already requested producer tracing, failing and legitimate cases, exact assertions and pre-edit execution; all three arms had the same task-level acceptance pattern | Another wording-only protocol is a repeat |
| Baseline-first low/xhigh oracle pair | Both retained reference and accepted saved wrong repair | Xhigh is a comparator, never the truth source |
| Low uncertainty follow-up | Real extra probes omitted the declared unresolved dimension; wrong repair still accepted | Require receipts for the actual declared investigation, then measure whether that helps |

Sources: archived notes in this tracker; [semantic diagnosis](../../research/2026-11-07-semantic-contract-diagnosis.md), [baseline pair](../../research/2026-11-07-baseline-first-oracle.md), [uncertainty follow-up](../../research/2026-11-08-oracle-uncertainty-resolution.md).

The candidate intervention is a controller-enforced sequence with executor-generated, source-bound evidence at each transition, including finalization when public checks are green. It is not a new oracle-generation contest. Earlier host qualification gates checked experiment validity; these gates govern the actor's investigation and repair phases. This distinction must be confirmed by reading earlier implementation before code reuse. If the same actor-facing transitions already exist, stop and amend this design before implementing or spending.

## Approaches and chosen implementation

A reusable skill alone is cheap but would again rely on instruction adherence. An xhigh reviewer adds capability and cost without isolating whether low can execute a better workflow. Chosen: a small task-agnostic state machine around the existing qualified native runner, with generic phase prompts and host execution receipts. Deliver a skill wrapper only after measured benefit; do not modify global guidance or Codescout tool behavior for this pilot.

Keep the protocol core independent of Docker, model providers, Rust and benchmark graders. A runtime adapter executes commands and returns evidence. A separate evaluator never sends hidden outcomes back to the actor.

## Actor workflow

Three bounded phases; no unlimited refinement loop:

1. Investigate on the pristine baseline. Record requirement, preservation and uncertainty clauses justified from public inputs. For every recorded material uncertainty, submit a concrete executable observation or explicitly retain it unresolved. At least one bug-reproduction attempt and one adjacent legitimate-behavior investigation are required. Failed reproduction is an observation, not evidence that the bug is absent. Production source must still match baseline at this boundary.
2. Repair. Receive the frozen public investigation and its actual observations, implement a candidate, run public checks and produce focused assertions. The controller freezes the candidate and binds assertions, commands and logs to its source digest.
3. Challenge and finalize. Even after green public checks, inspect the candidate's claimed change boundary, execute a preservation/boundary challenge derived from public interfaces, and revise if justified. Every actual revision invalidates earlier final-verification receipts. Final checks must bind the final source. Unresolved semantics remain explicit; the actor cannot convert missing evidence into complete assurance.

A real observation receipt proves only which command executed against which source, what it emitted and how it exited. The controller checks clause-to-receipt references and phase ordering; it cannot prove that the declared clause inventory is exhaustive, that an input covers an uncertainty, or that an inferred invariant is intended behavior. Those limits are measured by independent outcomes and a frozen transcript audit. Do not label this state machine a semantic coverage guarantee.

## Evidence and state contract

States: investigate -> repair -> challenge -> closed. Terminal alternatives: incomplete, budget_exhausted, measurement_invalid. Hidden grading adds a separate quality result and never changes the recorded procedural result.

Each executor receipt includes run/phase/command identifiers, exact argv and working directory, source and test-content digests, stdout/stderr digests and paths, complete-capture flag, raw exit, resource validity and cleanup evidence. The actor can reference receipt identifiers but cannot mint accepted receipts or overwrite frozen records. Reject duplicate identifiers, unknown clause references, stale source/test digests, production drift during investigate, missing commands, unsupported output, truncated evidence and private input mounts. Use exclusive creation and immutable phase manifests; existing records are never overwritten on restart.

Every intervention phase has a finite budget and continuation count. A structurally incomplete protocol attempt remains in the denominator as an unsuccessful workflow; it is not relabelled infrastructure-invalid to improve quality. Infrastructure invalidity requires independent runner/evaluator evidence. Preserve completed patches for hidden grading even when the public protocol is incomplete, provided capture and grading remain valid; report actual repair acceptance and protocol completion separately.


### Concrete phase execution and limits

Each phase is a separate native invocation with a phase-specific public packet and a model-written submission.json. The controller collects the terminal result, freezes the entire output, captures the patch and validates the submission against its own execution receipts before starting another invocation. Phase0 starts from pristine baseline; Phase1 gets only frozen Phase0 public artifacts and baseline; Phase2 gets only frozen public artifacts and the captured Phase1 candidate. Do not forward hidden evaluation, other arms' transcripts or the controller's historical diagnosis. The runner is the transition authority; no in-session declaration changes state.

Exactly three phase invocations maximum for P and R; at most two ordinary runtime continuations per invocation, only for pending jobs or public-check feedback under the existing lifecycle semantics. At most nine actor turns total (three initial plus six continuations), further limited by phase and workflow budgets. There is no fourth phase, fresh-author replacement or extra correction call. Malformed/missing phase submission or missing evidence ends P as incomplete; it does not buy a model retry. R has the same three invocation/turn/budget opportunities without the semantic artifact/gate requirements. L/X retain the frozen ordinary workflow limits and those limits must be reported.

Phase2 may submit one final candidate patch set incorporating its challenge-driven revision. Multiple editing tool calls can create that set; they are not counted as independent candidates. Freeze that final source before host final-check replay. If final public checks fail functionally, retain the captured candidate and classify the workflow incomplete; no additional model call follows the final replay. Challenge commands recorded before a final source change cannot substitute for verification of that final source.

### Exact source and receipt boundary

The baseline manifest enumerates every tracked regular file, symlink target and executable bit, plus all untracked input files admitted to the prepared actor snapshot. Git metadata, command outputs and build caches are excluded by explicit roles, not by guessing file extensions. No pre-existing file may change in Phase0, including a file mixing tests with production code. New standalone probes/integration-test files are allowed only in adapter-declared public test locations or a separate probe directory; their full bytes enter the execution-input manifest. Changing production merely to expose a probe is unsupported in v1 and yields an incomplete attempt if investigation cannot proceed. This is an explicit portability limit, not infrastructure failure.

The host executes each evidence command against an immutable read-only input snapshot: baseline/candidate source plus the exact admitted probe/test overlay. Build targets, temporary files and command outputs live in separate writable directories. Bind the immutable toolchain/image/dependency manifest and generated executable-input digests to the command receipt. Collect pre/post input-manifest hashes and reject changes; successful post-only hashing of a mutable working tree is insufficient. A command that cannot run under the registered snapshot/adapter rules is unsupported, never silently executed against a different tree. Actor-authored stdout, submission fields and forged receipt files are untrusted content. General hostile-kernel or compiler correctness is outside this protocol.

## Experimental comparison

Four arms per task:

- L: ordinary low complete repair workflow.
- R: low with the same three phase allocations/continuation opportunities as the treatment, but generic continue/inspect/test instructions and no evidence gates.
- P: low with the evidence-gated protocol.
- X: ordinary xhigh complete repair workflow.

All share the exact public issue, baseline, source-only onboarding snapshot, Codescout, project CLAUDE.md and authorized global rules, pinned plugin/model/CLI and common public checks. Use independent writable clones. No repeat onboarding per arm. P and R use equal configured budgets and maximum phase opportunities; actual spend may differ and is reported. R controls scheduled continuation opportunity, not identical realized compute.

Private acceptance tests/reference/control labels are host-only and inaccessible to all four actors. Freeze all outputs, charges and public evidence for the batch before revealing private outcomes. Grade all eligible saved patches using the same frozen independent test population and infrastructure policy. Require the existing negative, reference and repeat evaluator controls before paid admission; those are measurement checks, not actor inputs.

## Outcome-blind population and stages

Stage 0: zero-provider implementation/qualification on small synthetic projects in at least two languages, with the same policy and substituted command adapters. Include correct/wrong/no-op repairs, stale receipts and misleading green test logs. These are instrument fixtures, not evidence that the mechanism repairs real projects.

Stage 1: exactly two new development tasks from two repositories; no replacement task or seed after actor outcomes. Before selection, freeze the candidate metadata universe, exposure exclusion list and deterministic selection rule. Exclude every task previously used/read in this research, including Nushell18863, rust-analyzer22992 and the known Axum/ripgrep controls. Use metadata and build/adapter qualification only, never prior low/xhigh success labels. Select the first two eligible repository-distinct tasks under a recorded deterministic hash order; preserve excluded/ineligible reasons. If two cannot be admitted, stop without selecting from outcomes. Do not read other sessions' model outcomes to find promising contrasts.

Stage 1 is a development diagnostic, not held-out validation or model-training decontamination. If its evidence supports continuation, freeze the unchanged policy before selecting a separate validation population. The validation population size, uncertainty target and budget need a separate registration; two tasks cannot establish general equivalence.

## Endpoints and decisions

Unit of comparison is the task. Publish per-task L/R/P/X hidden acceptance, protocol state, functional versus infrastructure outcome and complete model cost. Keep failed/incomplete workflows and their charges. Report P-L, P-R and P-X paired outcomes, plus pooled descriptive cost and actual counts of task-level wins/losses. Do not treat requests/assertions/repeated attempts as independent tasks. X may fail; count that failure rather than redefining the oracle.

Stage-1 advance screen, fixed before runs: all eight attempts are interpretable and BOTH P workflows complete the registered protocol; P has at least one task-level acceptance gain over both L and R, no acceptance loss against either, at least X's accepted-task count, and lower total model cost than X across the same two tasks. Otherwise close this version as NO-GO. This is a deliberately narrow development screen, not statistical proof or a noninferiority claim. A failed screen does not license another same-task prompt revision.

Stop immediately on unknown charge, source/context drift, hidden-feedback exposure, invalid evaluator control, uncertain cleanup or infrastructure-invalid attempt; preserve every existing attempt and do not resample favorably. No budget increase or replacement run occurs automatically.

## Cost, resources and reuse

Standing series ceiling remains USD200, not a new USD200 allowance. Last reconciled known model subtotal is USD64.401093499999998654 plus the original unresolved USD4 startup reservation; recompute from immutable receipts before admission and count any intervening spend.

Stage-1 proposed admission reservation is USD38: low arms L/R/P each USD3 per task and X USD6 per task (USD30 for two tasks), up to USD4 shared setup/onboarding and USD4 contingency. CLI cap stays below each admission reservation and is not represented as a guaranteed provider hard cap. P/R split their USD3 ceiling across phases as USD0.60/1.80/0.60; all used/unknown charges across phases count. No new onboarding call when an adequate source-only snapshot already exists. Record shared preparation cost once, and report standalone per-workflow cost with an explicit amortization convention.

Reuse the qualified ca35 native adapter source only after current hashes, transitive modules, actual delivery and resource receipts are validated; current main lacks tools/agent_lifecycle. Plan code in a new isolated worktree from the qualified source, leaving original frozen paths untouched. Existing external.launch accepts supervisor and workflow.execute supervises real public-command calls, but neither inspected entry point implements these actor-facing phase transitions. Preserve the already qualified evaluator and policy source bindings; no repeated full expensive qualification without a changed boundary.

Recheck actual disk/tmp execution, host headroom and owned-container limits before native admission. Resource policy is explicit configuration, not a claim that installed RAM is insufficient. Do not delete other sessions' data or silently weaken resource guards.

## Native onboarding and capability admission — added 2026-11-09

A source/knowledge snapshot and a successful index command are insufficient admission evidence. Before any model call, capture readiness in the exact evaluated image/executable, active root and per-arm clone: configured source languages (derived independently from source/runtime adapters), complete actual MCP tools/list including pagination, and successful source-derived symbol/reference/definition/caller canaries. For this Codescout-enabled study also require native index verification and a semantic-search result bound to real public source. Include workspace member scopes actually needed by the task.

Freeze source-only onboarding, common knowledge and global/project rules once per project/version; reuse isolated copies across L/R/P/X. Never share task diagnoses, patches or run feedback as onboarding. Repair language/configuration/runtime drift through a new targeted prepared version and recheck it; do not automatically repeat the LLM authoring process. A genuinely new project still needs its one source-only preparation if no adequate corpus exists.

Hash-bind readiness to the executable/image and source/knowledge version; prove effective global/project/system prompt delivery separately through the actual CLI qualification. A listed LSP tool is not proof that the server or operation works. Record pre-run preparation failure before spending, not as a model-quality failure. Public canaries are host preparation checks, not extra task-specific repair hints.

See the [measured audit](reports/2026-11-09-onboarding-readiness-audit.md). Historical configurations audited there do not show languages=[]; their closed quality outcomes remain unchanged.

## Implementation and review acceptance

Deliver a pure protocol module, append-only receipt validation, thin native adapter, frozen generic prompts, deterministic selection/exposure manifest, scripted no-provider integration and paired analysis. Test the actual transition and command-execution call sites, not just schema helpers. Apply candidate guard mutations before writing missing tests; record observed kills/survivors and invalid mutations separately.

One independent strongest-available whole-change review applies at least eight relevant mutations on isolated copies before paid admission. One bounded correction pass; no repeated same-case review/author loop. The design itself gets a fresh builder check. No paid run is admitted by fake-boundary tests alone: an actual scripted CLI/native check must bind real logs, context, final patch and cleanup without credentials/provider access.

## Open admission items

Task identities, source commit/transitive hashes, prepared snapshots, model/CLI/plugin hashes, actual command adapters, evaluator expected sets/control receipts, current accounting and signed-off review do not yet exist for this new cohort. Paid readiness is false until a new immutable registration binds them. Preserve the old closed studies unchanged.
