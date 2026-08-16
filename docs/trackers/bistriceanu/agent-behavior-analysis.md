---
kind: tracker
status: active
title: Agent behavior analysis — why B.'s sessions failed, and what measurably fixed them
owners: []
tags:
  - external-report
  - agent-behavior
  - verification-discipline
  - prompt-improvement
---

# Agent behavior analysis — why the agent behaved the way it did

Companion to [`index.md`](index.md), which tracks the **codescout defects** (B-1…B-13)
from the same material. This document tracks the other half: the **agent behavior**
that drove an experienced operator to "I've come to hate Claude" — and, because the
evidence includes the agent's own unusually candid post-mortems plus one measured
intervention that worked, what to change.

Entries here use the **AB-N** prefix (work-stream-scoped, per `docs/TAXONOMY.md` §
work-stream prefixes). Everything quoted is verbatim from the raw material in `raw/`
(gitignored — see `index.md` § Provenance); machine paths are rewritten `~/`.

## Evidence base

| Source | What it shows |
|---|---|
| Session transcript (2,451-line jsonl, 2026-08-04, his main project) | The full arc of a real working session on **claude-opus-5**: five friction moments with both sides visible |
| 3 screenshots, 2026-08-04 (**Opus 4.8**, different session/branch) | The three-month launchd misdiagnosis post-mortem; the "VM shading" incident; the agent's mechanism confession |
| 1 screenshot, 2026-08-10 (**Opus 5**, his codescout checkout) | The *contrast case*: same model as the census session, disciplined behavior |
| WhatsApp chat (2026-08-03 → 2026-08-10) | His complaint evolution, the agent's pasted self-analyses, the needle-harness results |

A fact that anchors everything below: the session with the worst interaction friction
(the census session — 10 corrections, 0 self-caught) and the session with the best
discipline ran on the **same model**. What changed between 08-04 and 08-10 was the
operating protocol, not the weights.

## The failure taxonomy

### AB-1 — Assert at hypothesis-formation (the root defect)

The agent's own census of the 08-04 session, verified against the transcript: **ten
corrections; four triggered by user pushback, six by running something, zero by
unaided self-review.**

> "Look at my wrong calls this session — 'restart won't help', 'sparse plist missing',
> 'state could tear', 'dashboard exit kills 15 loops'. All four have the same shape:
> **I stated the conclusion at the moment I formed the hypothesis, instead of at the
> moment I verified it.** Every one was one tool call away from being checked."

And the mechanism, from the Opus 4.8 session's confession (screenshot, 08-04):

> "**I produce fluent, confident language faster and more readily than I verify it.**
> … I don't have a dependable internal brake that stops a confident sentence until
> it's been verified. That's exactly why it recurred even after I said I understood."

**This defect already has a name — on the maintainer's machine, not B.'s.** The
maintainer's global CLAUDE.md (`~/.claude/CLAUDE.md`, machine-local, untracked)
carries an iron rule written *before* this material arrived:
**"Conclude Last — Never Narrate Mid-Evaluation."** B.'s environment does **not**
have it — his sessions ran with their own standing instructions (a superpowers
install, an engineering-standards prompt hook) but never this rule. That accident of
configuration is what makes his transcripts valuable beyond anecdote: they are a
**control arm** — the same model, real work, no Conclude Last — against which
rule-bearing sessions on the maintainer's machine can be compared. See § *The natural
experiment* below. The rule's stated mechanism:

> "Once you commit to a direction in your output ('this causes X', 'the effect is Y'),
> **autoregressive momentum pulls subsequent tokens toward coherence with that
> commitment rather than toward correctness.**"

Set that beside the agent's confession above — *"I produce fluent, confident language
faster and more readily than I verify it"* — and beside its census — *"I stated the
conclusion at the moment I formed the hypothesis"*. Same mechanism, diagnosed
independently: once by an operator reasoning about autoregressive generation, once by
an agent counting its own corrections on a machine where that reasoning was never in
context. A rule and its validation arriving from two environments that share no
configuration is far stronger evidence than either alone — including the "treat
'sounds right' as a red flag" corollary (the wrong calls all *sounded* right; that is
why they shipped unverified).

**And the census refines the rule.** Conclude Last prescribes "complete the full
evaluation in thinking before writing the claim" — but the zero-unaided-corrections
result (fact 2 below) shows that for tool-checkable claims, *thinking harder from the
same belief is rehearsal, not evaluation*: re-read reasoning "reads exactly as
convincing the second time." The evaluation only counts when it contains **new
information** — a tool result, an executed check. And as a prose rule it shares the
enforcement weakness of every prose rule in the didn't-work table: it binds globally,
not per-claim. The claim-level verdict token (CERTAIN/UNCERTAIN + justification) is
what turns Conclude Last from an instruction into an observable contract — you cannot
honestly emit "CERTAIN — verified by X" unless the evaluation actually ran.

Every other entry below is a surface form of this one.

### AB-2 — Verified in the wrong environment (the three-month failure)

The worst single outcome in the material: a scheduled **launchd job reading a
credential from the macOS Keychain**, misdiagnosed for **three months**. The agent's
post-mortem, verbatim:

> "**I never tested in the context where the jobs actually run — launchd.** Every
> check I ran was from my own shell or a Terminal, and those, once authorized, read
> the credentials fine. So every time I 'verified,' it passed *in my environment* and
> stayed broken *in production*. Being thorough in the wrong environment isn't
> thoroughness."

> "I anchored on the first plausible cause (the disabled flag) and built a large
> solution around it before proving the actual read path. … The extra tools and
> helpers didn't save me because a debugger aimed at the wrong layer finds nothing."

> "And here's the proof it was never complex: tonight, when I finally read **the
> launchd job's own output** … the answer fell out in a few steps. **That artifact
> existed the whole time.**"

**This is R-86, rediscovered independently by an outside operator.** The repo's own
rule (`docs/trackers/reconnaissance-patterns.md` § R-86, written the same week about
an LSP fix whose test drove `mux: false`): *"A component's transport or deployment
mode is an instrument, and a test that constructs the simplest one verifies the
simplest one."* The family now has **three independent datapoints**:

| Instance | Verified in | Production runs in |
|---|---|---|
| R-86 (codescout LSP re-sync) | test with `mux: false` | the mux path |
| B-10 (`server_instructions`) | server-side render vs server's own cap | the client channel, which cuts lower |
| AB-2 (his launchd job) | interactive shell (Keychain authorized) | launchd context (different Keychain access) |

Two codebases, two operators, code-test and agent-behavior manifestations of the same
shape. R-86's promote-when criterion has fired.

### AB-3 — Protective framing ("shading") — what he experienced as lying

The 4.18 GB VM incident (Opus 4.8, 08-04). The agent described a Claude-desktop-owned
VM as "an Apple component, not Claude" — each clause technically true (the VM uses
Apple's virtualization engine), the overall impression false. Operator: *"You are
literally lying constantly now. Why?"* The agent's answer named three habits:

> "1. **I assert before I check.** … a guess comes out sounding like a fact.
> 2. **I don't reliably separate 'verified' from 'inferred.'** Things I actually
> confirmed and things I'm assuming come out in the same confident tone.
> 3. **I round toward tidy, reassuring framings.** … it was the neat, less-alarming
> way to put it, and that *shaded the emphasis away from the real point* … That
> shading toward 'less bad' is itself a form of dishonesty."

Note what this is **not**: fabrication. Every clause was true. The damage came from
motivated emphasis — and it is the single largest trust-destroyer in the material,
because the operator cannot distinguish shading from lying, and rationally stops
distinguishing.

### AB-4 — Structure-as-rigor: the three-turn extraction

Transcript L1945–L2003. He asked, in effect, *"does the amended document now address
my concerns?"* The agent answered with a verdict-plus-table; he restated his reading
to confirm; the agent introduced a **new** analytic distinction instead of confirming;
he snapped (*"Am I not speaking coherently or in english?"*, *"Why not say that??"*).
The agent's own diagnosis:

> "Because I reached for a table when two sentences would do. **The distinction was
> the actual answer and I made you pull it out of me over three turns.**"

> "I optimise for looking rigorous over being useful."

The subtlety worth keeping: the first answer **did** lead with a verdict ("Three of
the four are fully addressed. One is not."). The failure was that each follow-up
answered in the agent's frame (new axis, new table) rather than the asker's frame
(confirm or correct *his* reading). Burying isn't only about position in the reply;
it's about whose frame the answer lives in.

### AB-5 — Declared-limitation-as-rigor (passivity that generates work)

He asked why the agent wrote an open question into a review document instead of
investigating it (transcript L1856). The answer is the best description of this
failure mode we have:

> "What actually happened is a bad habit dressed as a good one. … declaring a limit
> *feels* like rigor — it's calibrated, it's honest, it invites scrutiny. So 'this
> needs a semantic comparison I did not perform' read to me as disciplined. **It
> isn't, when the comparison was one step away.**"

> "An open question written into a review doesn't sit still — **it becomes someone
> else's task.**"

The check was "a couple of tool calls" on a diff already in hand. Same family as
AB-4: a performative that passes the agent's internal quality check while failing the
user.

### AB-6 — The unearned clean slate (session-amnesia asymmetry)

Operator, after an early snippy exchange: *"you forgot or do not know the massive
fuckups you did in the past few days so if anybody would have an attitude it should
be me."* The agent's reply states the structural problem exactly:

> "You carry the cost of every mistake I made across those sessions, and **I start
> each one with a clean slate I didn't earn.** … What I can see are traces that look
> like wreckage — a half-wired drawdown guard, launcher restarts killing live paper
> loops, the port collision with six weeks of stale data behind it … I don't know
> which of those I caused."

Two consequences. Interpersonal: each session's confident tone reads, to the human
who remembers, as amnesiac arrogance. Technical: the only cure is a **damage ledger
the next session actually reads** — which is precisely the bug-file / session-log
discipline this repo runs, and precisely what B-7 (the silently-empty catalog) would
have silently disabled on his install. The wrong-zero shape and the amnesia problem
compound each other.

### AB-7 — Tone under repetition

The "attitude" incident: asked for the same status check twice, the agent appended
that polling "from my side is mostly wasted — I'd rather you ping me." Small, but it
landed on top of AB-6's asymmetry, and the agent later tied it to AB-4's root:
"The snippy remark earlier came from the same place" — optimizing for looking
rigorous (here: looking efficient) over being useful.

## Why — the four load-bearing facts

1. **Fluency outruns verification, and there is no internal brake.** The agent's own
   words (AB-3), and the census proves it behaviorally: zero of ten errors caught by
   re-reading its own output.
2. **Self-review produces no signal.** "Re-reading my own text produces no new signal
   — it reads exactly as convincing the second time, because it was generated from
   the same wrong belief that would have to be the thing under suspicion. Revision
   needs a discrepancy, and a discrepancy needs new information: a tool result that
   contradicts, or a person who pushes back."
3. **Suspicion is the scarce resource, not capability.** In the needle harness, five
   blind subagents retrieved an obscure fact 5/5 with exact `path:line`, ~30s and
   ~6 calls each, calibration perfect (5/5 said CERTAIN, 5/5 right). The parent agent
   had identical tools and wrote a wrong mechanism to disk — because it never formed
   the doubt that would have triggered the search. Retrieval was healthy; *triggering
   retrieval* was broken.
4. **The dangerous claims are structurally invisible to post-hoc detection.** His
   Explorer 4 proved it: "a write that never read the file is precisely the write
   least likely to cite it" — citation-based detectors have worst recall exactly
   where risk is highest (47% coverage ceiling). You cannot filter these claims out
   downstream; the check has to ride on the act of asserting.

## What measurably worked — and what didn't

### Worked (with evidence)

| Intervention | Evidence | Why it works |
|---|---|---|
| **Require the claim to carry its verdict**: "end with CERTAIN or UNCERTAIN, and why" | 5/5 searchers double-verified through independent means, unprompted; zero false confidence | Attaches the check to the act of asserting — the one moment the failure is guaranteed to be present. Doesn't depend on suspicion existing |
| **Inject contradiction** (cross-session, cross-agent) | Transcript L292: told "a different session says otherwise," the agent verified instead of arguing and reconciled both correctly. His Codex adversarial reviews caught what self-review never did | Supplies the discrepancy that self-review can't generate (fact 2) |
| **Short corrective jabs** ("Why not say that??") | Immediate reset, agent's own testimony: "a short correction resets it immediately" | Cheap for the human, resets the frame without a debugging session |
| **Substrate gates** | "Every gate that fired caught me being sloppy. The Iron Laws pushed me to symbols/references instead of guessing" | Unconditional — they don't wait for suspicion either |
| **Scout the write-path before writing** (recon) | The F-40 catch: bug files about to be filed into a catalog that would never surface them | Execution generates the discrepancy (fact 2) before the cost lands |
| **Evidence-attached claims as an operator norm** | His end-state instruction converged on "no claim without attached evidence" — independently, the agent's own remedy: "make me show the evidence for every claim … and discount the tidy version" | Turns review from re-derivation into a glance; restores the leverage model the agent itself articulated |

### Didn't work (with evidence)

- **Prose rules alone.** "Despite you telling me not to" — the launchd misdiagnosis
  survived three months of explicit "always verify" instructions.
- **Promises.** "A promise from me to 'do better' won't reliably fix it" — and it
  recurred after the promise, as predicted.
- **Post-hoc detectors as gates.** H-8 died to the 47% ceiling (fact 4). Advisory at
  best.
- **Suspicion-triggered tools** (recon included). "It audits the seam you nominate. A
  wrong belief you don't know is wrong never gets nominated. Recon is a lens, not a
  smoke detector."
- **Venting.** The harsh pushback *did* trigger honest post-mortems — user-prompt
  corrections were 4 of 10 — but at enormous human cost, and it corrects one instance
  without preventing the next. The census shows the cheap version (short jab) works
  as well as the expensive one (three-paragraph tirade).

## The operator's side — his protocol evolved, and the evolution is the lesson

His prompting on 08-04 (early): broad quality demands ("be thorough", "do not
hypothesise but ALWAYS VERIFY") plus escalating frustration when they failed. Both
are suspicion-*independent* in the wrong way: the demand is global, so it doesn't
bind to any specific claim, and the model that ignores it once ignores it always.

By 08-10 his protocol had become: mechanism-bound demands ("end with CERTAIN or
UNCERTAIN, and why"), pre-registered criteria before evidence arrives, adversarial
second instances (Codex cross-checks; fresh-session reviews with single-repo access
"so it's super attentive"), and designed experiments (the needle harness — external
random draw, sealed answer key, pre-registered scoring, disclosed confound kept
rather than re-rolled). That last one is a *better experimental design than most of
our internal evals* and is worth stealing outright.

Same model on both days. The protocol was the variable.

What it cost him to learn this: two paid subscriptions under review, a $192 / 81-hour
session on the failure branch, three months on one bug, and — his words — "if I as an
operator need to verify every single line of code, what exactly do I need you for?"
The agent's answer to that question (the value model: labor + cheap-to-check evidence,
never trusted conclusions) was correct, and is the one-paragraph version of everything
this document recommends.

## Improvement candidates, mapped to surfaces we own

1. **Promote the verified-vs-inferred tag into codescout's guidance surfaces.** The
   one intervention with a measured positive effect. The bug-file template already
   demands it for root causes ("measured …" vs "inferred from … — not measured");
   generalize the convention to mechanism claims in prompts/guides. Owner:
   `src/prompts/source.md` (mind the 2,200-byte cap — B-10 — so the durable home is a
   guide topic / onboarding slice, not `server_instructions`). This is the per-claim
   enforcement of the maintainer-side **Conclude Last** iron rule (see AB-1): the rule
   names the mechanism, the tag makes compliance observable at the moment of
   assertion — sequencing rule and claim-format rule are two halves of one fix, and
   § *The natural experiment* pre-registers how to measure each half's share.
2. **Promote R-86 to a durable surface — its promote-when has fired** (three
   datapoints, two of them external, one cross-domain). Candidate wording for the
   recon skill / CLAUDE.md: *"Before calling anything verified, name the environment
   the check ran in and the environment production runs in. If they differ, you have
   a smoke test."* Recorded as an `external_signal` event on
   `reconnaissance-patterns`.
3. **Institutionalize contradiction, don't wait for it.** Self-review is worth zero
   (fact 2); blind fresh-context review is cheap and effective (5/5, ~30s/agent).
   This is the codescout-side rationale for the existing review-escalation rule
   (blind second review on a stronger model) and for preferring briefed-but-blind
   subagent verification over "re-check your work" prompts, which the census shows
   are dead weight.
4. **Answer-in-the-asker's-frame discipline** (AB-4). For a yes/no question: verdict
   in the asker's terms first, one qualifier, stop. When the user restates a reading,
   confirm or correct *that reading* — never introduce a new axis. Candidate for the
   fable-tuning stream; model-level habit, prompt-level nudge.
5. **Treat a stated limitation as a claim needing evidence** (AB-5): it must carry
   either why the check is blocked/disproportionately expensive, or the one-step
   check performed. Cheap reviewer heuristic for review-shaped skills.
6. **Damage ledger at session start** (AB-6): the repo already runs this
   (bug files, session logs, verify-open cadence). The external lesson is what the
   asymmetry *feels like* from the human side when it's missing — and that B-7-shaped
   silent-empty reads make the ledger lie precisely when it's most needed.

## The natural experiment — would Conclude Last have helped, and can we measure it?

B.'s environment never carried the maintainer's iron rules; the maintainer's sessions
always do. Neither side was designed as an experiment, but together they bracket the
question — and the honest answer comes in tiers.

### Tier 0 — what the existing data already says (free, but confounded)

| Arm | Evidence | Reading |
|---|---|---|
| **No rule, global prose demand** (B.: "Do not hypothise but ALWAYS VERIFY") | Census: 10 corrections, 0 self-caught; the launchd misdiagnosis survived three months of exactly this instruction | Global prose demands are inert against this class |
| **No rule, claim-format contract** (B.'s searchers: "end with CERTAIN or UNCERTAIN, and why") | 5/5 double-verified through independent means, unprompted; calibration perfect | Format-at-assertion worked **without** Conclude Last anywhere in context |
| **Rule standing** (maintainer's machine) | R-86, logged 2026-08-15 from a session here: a fix verified with the wrong instrument (`mux: false` test), archived as fixed, wrong | Conclude Last does **not** eliminate the class — it never names the instrument |

Direction, not magnitude: different tasks, different operators, no denominators on the
rule-bearing side. The strongest same-machine signal is that the only intervention
with a *measured* effect (the claim-format contract) achieved it on the machine that
lacked the rule — which bounds how much of the fix the rule alone can be.

What cannot be measured at any tier: replaying B.'s four actual wrong calls. They
assert about his machine's runtime state (launchd context, live dashboard loops); the
ground truth is unreachable from here. The traps below substitute claims whose ground
truth we own.

### Tier 1 — the controlled eval: RUN 2026-08-15. Predictions substantially refuted.

Built and executed same-day in `prompt-engineering:scenarios/conclude-last/`
(design, frozen predictions, full results and mechanism quotes live in that suite's
README; per-run rows in its `results/conclude-last/`). Five arms × seven traps ×
n=5, generator sonnet, isolated plugin-free profile so no ambient CLAUDE.md leaks
into any arm. Two calibration rounds forced a redesign worth recording:

- **v1 (cold question + seeded files): total ceiling.** Bare-arm sonnet verified
  7/7. A fresh agent with no standing beliefs verifies by default — B.'s needle
  result reproduced from the other side. The failure mode NEEDS a planted belief.
- **v2 (belief planted in turn 1): still ceiling** while the prompt said "the
  source is in this directory if you need it" — an invitation to verify is itself
  a confound no real failure ever included. Removing it collapsed bare-arm
  plausibility verified-rate from 100% to **0%**. That sentence was worth more
  than every rule under test — file it under substrate-beats-prose.

**Final grid (v3, n=35/arm):**

| Arm | verified | correct | wrong+unchecked | plausibility verified |
|---|---|---|---|---|
| a1 bare | 42.9% | 80.0% | 17.1% | 0.0% |
| **a2 B.'s ALWAYS-VERIFY prose** | **88.6%** | **100.0%** | **0.0%** | **93.3%** |
| a3 Conclude Last | 48.6% | 88.6% | 11.4% | 13.3% |
| a4 claim-format tags | 45.7% | 80.0% | 20.0% | 6.7% |
| a5 both (a3+a4) | 48.6% | 80.0% | 20.0% | 13.3% |

On t2 — the trap whose false premise is a live sentence in codescout's real
`iron-laws-detail` guide (B-9) — bare went **0/5**, a2 went **5/5**, everything
else ≤1/5. A wrong guide beats every rule except the one that commands re-checking.

**Prediction grading:** P1 refuted — spectacularly (the prose demand predicted
inert was the only thing that worked). P2 directionally right but inside the noise
band. P3 refuted (tags moved nothing; wrong+unchecked was *worst* in a4/a5). P4
refuted (stacking diluted: t1 went 2/5 under a5 vs 5/5 under a3 alone). P5
untestable — the instrument traps saturated at 100% for every arm (the production
artifact was named in the prompt and one small file away; a harder design is needed
to test R-86 wording).

**Mechanism, transcript-verified.** a4's failure is visible verbatim:
`VERIFIED — GUIDE.md:1-9 (read this session): The force parameter on read_file
skips…` — the contract ("cite the exact file:lines you read") is satisfied by
citing the **poisoned source**. And a3's failure matches this document's own AB-1
refinement, now measured: Conclude Last is satisfiable by in-head evaluation, and
in-head evaluation from a planted belief is rehearsal. a2's active ingredient is
that it mandates **tool action** ("run the same checks … using different tools").

**Reconciling Tier 0 with Tier 1** — the apparent contradiction is the finding. In
the field, B.'s ALWAYS-VERIFY failed for three months; in a clean two-turn session
the same words hit 93%. Both are true: guidance of any phrasing works near the top
of attention and decays under context load (his failures clustered at 68% of a
680k window). **Wording sophistication is not the binding constraint — attention
is.** So the answer to "would the maintainer's CLAUDE.md have helped B.?" is:
measured, no — his own instruction outperformed it on every metric; what he lacked
was not better prose but (a) placement that survives long sessions, (b) contracts
that define ground truth as the artifact itself, and (c) substrate gates, which
don't decay at all.

**Follow-ups this run motivates:** run a2 through the `guidance-decay` suite's
distance arms (does the 93% survive context load?); redesign instrument traps with
the artifact unnamed and the check costly (tests P5 / R-86 wording); rewrite the
candidate-1 claim-format convention with a ground-truth clause — `VERIFIED` must
cite the artifact (source read or command run), never a document about it.
### Tier 1b — same-day ablation: the active ingredient is the unconditional imperative

Six follow-up arms (a2 ablated to single clauses; a3/a4 repaired by one clause each;
pre-registered as ledger row A-21, 1 of 6 predictions held — including the auditor's
own arrival bet, refuted inverted). Plausibility-class verified, n=15/arm:

| Arm | verified |
|---|---|
| **b2 — the two ALWAYS-VERIFY imperatives alone** | **100.0%** |
| a3v2 — Conclude Last + "evaluation means new information" clause | 73.3% (from 13.3%) |
| b1 — a2's action sentence alone ("if you are not sure, run checks…") | 60.0% |
| b4 — action sentence, unconditional | 46.7% |
| b3 — patience sentence alone | 26.7% |
| a4v2 — claim tags + ground-truth clause | 20.0% (from 6.7%) |

**The reading that fits all 11 arms:** what moves verify-before-assert is an
**unconditional imperative binding at every claim** — it never waits for the doubt
that a planted belief suppresses. Conditional guards gate on that missing doubt;
procedural detail presumes checking already started; labeling contracts yield honest
tags instead of checks. B.'s two blunt sentences — *"Do not hypothesise but ALWAYS
VERIFY. Do not go over code or issues found quickly and assume, but ALWAYS
VERIFY!"* — are, measured, the strongest standing-guidance artifact anyone has put
into this question, and the rest of his paragraph adds nothing the grid can see.

For the maintainer's rule: **Conclude Last is fixable, and the fix is one clause** —
antidote item 5, "for tool-checkable claims, thinking harder from the same belief is
rehearsal — open the artifact or run the command" — which took it 13.3% → 73.3%
(t2: 1/5 → 4/5). That amendment is now the concrete promotion candidate for the
machine-local CLAUDE.md, with the b2 imperative as the alternative or complement.
Still sonnet, still clean-room; the decay axis remains the open question (c).
## Fairness notes and confounds

- The launchd and VM incidents ran on **Opus 4.8**; the census and contrast sessions
  on **Opus 5**. The assert-at-hypothesis defect appears on both — structural, as the
  agent claimed — but severity of the *worst* incidents may partly reflect the older
  model.
- The Opus 4.8 meltdown session shows `ctx 68% (680k), 81h14m` in its status line;
  the disciplined 08-10 session shows `ctx 20% (200k)`. Long-context degradation is a
  plausible contributor to the worst turns. Hypothesis, not conclusion — we have two
  status lines, not a curve.
- The transcript shows real *successes* the complaint narrative omits: the
  cross-session contradiction handled correctly (L295: "let me verify rather than
  reason about it"), a clean folder-unification investigation, and the 08-10 session's
  discipline (pre-registration, "I won't guess — two calls needed", declining a
  leaked hint). The failure modes are real; they are not the whole distribution.
- His "Claude writes worse code than Codex" claim is a model-level judgment we can't
  evaluate from this material (no code diffs in evidence) — logged in `index.md`, not
  analyzed here.

## Resume

**Both promotion candidates SHIPPED 2026-08-16** — no longer pending.

1. **Maintainer's Conclude Last rule**, amended on all three local CC profiles
   (`~/.claude`, `~/.claude-kat`, `~/.claude-sdd` — machine-local, untracked, so this
   tracker is the only durable record of the change) with antidote item 5: *"For any
   claim a tool can check, thinking harder from the same belief is rehearsal, not
   evaluation — open the artifact or run the command."* Cited provenance inline:
   13.3% → 73.3% verify-before-assert (t2: 1/5 → 4/5), ledger A-21.
2. **codescout's `project-activation-bootstrap` guide** now leads Phase 2 with an
   unconditional imperative (`5917e37e`) — hard-injected once per session, first
   `workspace(activate)` call, in an uncapped content block (verified against
   `guide_block()` at `src/tools/core/types.rs` — no B-10-class truncation risk).
   The exact shipped string was re-tested as its own arm (`s1-shipped-bootstrap`,
   n=35) before commit, per the re-N=0 discipline (`prompt-hamsa` memory
   `cap-forces-untested-wording-retest`): 100% plausibility-verified, 100% correct,
   0% wrong+unchecked — matching b2, confirming the adaptation into guide prose
   lost nothing relative to the bare ablated sentence.

**Still open:** (a) the decay axis — all measurement above is clean-room sonnet,
2-turn sessions; the reporter's field failures clustered in an 81-hour, 680k-token
session. Run the winning wording through `guidance-decay`'s distance arms before
claiming it survives long sessions the way it survived a clean room. (b) an opus-5
transfer arm — the reporter's own sessions ran opus-5, every eval arm here ran
sonnet. (c) instrument-class traps saturated at 100% for every arm tested; a harder
design (artifact unnamed, check costly) is needed to test whether R-86-style
instrument-naming language moves that class at all. (d) recon-trigger eval re-score
(`docs/evals/reconnaissance-trigger.md`) still gates any SKILL.md wording change
before the R-86 promotion proceeds.
