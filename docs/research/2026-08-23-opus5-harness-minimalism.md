---
title: "Opus 5 and harness minimalism: what Anthropic deleted from Claude Code"
date: 2026-08-23
topic: prompt-engineering
summary: "Boris Cherny's YC talk on deleting 80% of Claude Code's system prompt for Opus 5 — the ablation method, verification as the real bottleneck, and what it implies for codescout."
status: complete
---

# Opus 5 and Harness Minimalism

Source analysis of a single practitioner talk: Boris Cherny (creator of Claude
Code, Anthropic) interviewed at Y Combinator, published 2026-07-27, one day after
the Opus 5 release. Full transcript archived at
[`transcripts/2026-07-27-boris-cherny-yc-opus5-transcript.md`](./transcripts/2026-07-27-boris-cherny-yc-opus5-transcript.md).

**Evidence class.** This is a conference conversation, not a paper or a benchmark.
Every claim below is an *assertion by a vendor employee about their own product*,
with no published methodology, no N, and no independent replication. It is recorded
here because the speaker has direct knowledge of decisions we can only observe from
outside, and because several claims are directly falsifiable against codescout's own
eval harness. Treat the claims as **hypotheses to test**, not findings to adopt.
Timestamps in brackets index the archived transcript.

**Transcription caveat.** No author-uploaded subtitle track exists for the video;
the transcript is YouTube auto-generated. Proper nouns and identifiers are
unreliable — see § Reliability for the specific items that need verification before
being acted on.

---

## Key Takeaways

1. **Anthropic deleted ~80% of Claude Code's system prompt when Opus 5 shipped**,
   and the stated reason is that most of the deleted text existed to correct
   behaviours the model now performs unprompted [03:21–04:20].

2. **Prompt text is claimed to cost capability, not just context.** The stated
   ablation result is that the model is *"a little bit more intelligent"* without
   the prompts — but that product prompts are still kept, because they shape how the
   product behaves for a human user rather than how capable the model is [05:00].
   This capability-vs-product distinction is the single most useful idea in the talk
   and the one most likely to be mis-cited as "delete everything."

3. **The method is line-by-line ablation, not judgment.** Delete the entire system
   prompt, then reintroduce it one line at a time and measure each line's individual
   impact. Applied to tools as well — tools get unshipped regularly [05:52].

4. **Rebuild empirically, never predictively.** Delete → use the product → observe
   where it *repeatedly* stumbles on the same thing → only then add the instruction
   back. The stated rationale is cost-per-turn: the model reads that instruction on
   every single invocation [07:39–08:20].

5. **Verification is named as the top practitioner failure.** Asked what separates
   strong users, the answer is not prompt craft: give the model a task slightly
   harder than you think it can do, and give it a way to verify its own work.
   Verification is called *"the single most important thing that people do not get
   right"* [19:57].

6. **Long-horizon runs are the headline Opus 5 capability**, and are claimed to need
   *less* scaffolding, not more — explicitly that `/goal` and `/loop`-style scaffolds
   "help" but are not required [00:55, 22:25]. Two supporting anecdotes: a Bun
   Zig→Rust runtime rewrite that ran 11 days and is claimed to be in production
   [17:04], and an Electron→Swift rewrite still running at 14–15 days at talk time
   [22:04].

7. **MCP retains an explicit, narrow endorsement.** In the prescribed fix ladder for
   a struggling agent — better prompting, or a skill, or *"if the model's missing
   context, give it an MCP so it can pull in the context that it needs"* [23:28] —
   MCP is scoped to **context supply**. Nothing in the talk endorses MCP as a
   behaviour-correction surface. This asymmetry is the load-bearing point for
   codescout and is developed in § Implications.

8. **Evals are described as shorter-lived than commonly assumed.** The interviewer
   proposes evals as the stable asset across model generations; the answer partly
   disagrees — evals outlive the harness *"but not by that much"*, roughly one to
   three model generations before saturation forces replacement [09:36].

9. **Stated limits.** "Coding is solved" is explicitly qualified: not for deep
   systems code, not for distributed systems, not for pixel-level UI verification.
   Opus 5 is described as a large vision/computer-use leap that is still imperfect
   [30:38].

---

## What the talk claims, in detail

### The deletion, and what survived

The framing is that Claude Code's harness is continuously rewritten — system prompt,
tool set, and per-tool prompts all change with each model, because an instruction
tuned for one model may not transfer to the next [03:38]. The 80% figure applies to
the system prompt at the Opus 5 boundary.

What is claimed to remain in the harness codebase is notable: *"almost all of it is
about safety and permissions and static analysis and there's a bunch of UI code"*
[06:33]. So the deletion targeted **model-steering text**, while **enforcement,
permissioning, and interface** survived. That is a category distinction, not a
uniform trim, and it is the shape any ablation of our own surfaces should follow.

### Two levers named for experimentation

- `--system-prompt` — a CLI flag to substitute an arbitrary system prompt.
- An undocumented environment variable, rendered by the auto-captions as
  "Claude Code simple equals one" — plausibly `CLAUDE_CODE_SIMPLE=1` — which is
  described as stripping *all* system prompts including the per-tool prompts, and is
  said to be what Anthropic uses internally as the ablation baseline [04:20].

The env var name is a transcription inference and **must be verified before use**
(see § Reliability).

### Orchestration as test-time compute

Dynamic workflows are described as a sandboxed orchestration layer (Bun runtime as
the sandbox) that fans out to many agents in stages — a pass, then a verify or
summarize stage, then another fan-out. The design is characterised as *"an algebra
for agents"*: sequencing and parallel combinators, drawn from a functional
programming background [25:59–26:40]. The conceptual claim is that this constitutes a
new axis of test-time compute beyond token count [27:21].

A separate mechanism, loops (local cron) and routines (cloud), covers repetitive
tasks that share memory but not context. The reported internal application is
self-maintaining codebases: roughly 20–30 daily routines across Anthropic's CLI,
iOS, Android, and desktop apps, including dead-code removal, retiring
fully-rolled-out experiment flags, adding tests to undercovered areas, deleting
low-value tests, and an "abstraction police" routine that unifies near-duplicate
abstractions across a codebase [28:01–29:23].

### Unhobbling and product overhang

Two paired terms. *Product overhang* is capability a current model already has that
no product elicits; *hobbling* is the product actively getting in the way. Claude
Code's own origin is given as the worked example: with Sonnet 3.5, contemporary
coding products offered autocomplete and read-only chat, and the bet was that
stripping scaffolding and granting terminal access would elicit whole-file
authorship [12:10–13:31].

The corresponding user-level advice is to stop over-specifying. Describe the task,
the guardrails, and the exit criteria, then let the model work — over-specified
step-by-step instruction is named as a common failure mode among experienced
engineers [14:47, 24:01].

---

## Reliability

Items to verify before acting on them:

| Item | Status |
|---|---|
| `CLAUDE_CODE_SIMPLE=1` env var name | **Unverified.** Reconstructed from auto-captions. Confirm against the installed CLI before designing an arm around it. |
| `--system-prompt` flag | Plausible but unverified against the installed CLI version. |
| "80%" | A speaker's round number. No baseline character or token count given. |
| "not prompt injectable anymore" [01:35–03:21] | A vendor security claim about their own model, described as three layers (alignment training, an interpretability-derived injection classifier on all traffic, an auto-mode classifier). The stated evidence is *"we cannot demonstrate prompt injection"* — absence of a successful internal demonstration, which is not the same as absence of the vulnerability. **Do not weaken any codescout trust boundary on the strength of this.** |
| 11-day / 14-day run anecdotes | Uncontrolled anecdotes; the 11-day rewrite is explicitly clarified as steered, not one-shot [17:45]. |

---

## Implications for codescout

**Everything in this section is inference by the author of this note, not claims from
the source.** The source discusses Claude Code's harness; the mapping onto codescout
is ours and is untested.

codescout's guidance surfaces are not one thing, and the talk's own delete-vs-keep
split (§ *The deletion, and what survived*) suggests sorting them into three
categories with very different exposure:

**(a) Context supply — not threatened.** `symbols`, `semantic_search`,
`references`, `call_graph`, the librarian catalog, memories. This is precisely the
case the talk endorses for MCP [23:28]: the model cannot infer a codebase's symbol
graph from intelligence alone. A more capable model raises the value of good context
plumbing rather than lowering it.

**(b) Behaviour correction — the actual ablation target.** The Iron Laws
("NEVER full-read source → symbols", "NEVER edit_file structural code → edit_code"),
the anti-pattern lists, and the repeated tool-routing reminders. By the talk's own
account, text that exists to correct *"behaviors the model should have known"*
[04:20] is exactly what became deletable at the Opus 5 boundary. These laws are
load-bearing only if Opus 5 still mis-routes without them — an empirical question we
already have instrumentation to answer.

**(c) Enforcement and safety — survives by the source's own account.**
`approve_write`, the dangerous-command gate, the IL-3 pipe block, the companion
plugin's hard denial of native `Read`/`Bash`. The claim that Claude Code's surviving
harness code is "almost all safety and permissions" [06:33] argues *for* keeping
these.

### The distinction the talk does not make

The talk measures prompts against **capability**. codescout's Iron Laws are also, and
arguably primarily, about **token efficiency**. A model fully capable of reading a
2,000-line file and answering correctly still burns an order of magnitude more
context than one that calls `symbols`. Capability-neutral is not cost-neutral, so an
ablation that measures only task success would mis-score every one of these laws.
**Any codescout ablation must measure tokens-and-tool-calls to a correct answer, not
just correctness.** This is the main reason the talk's conclusion cannot be adopted
wholesale here.

### Enforcement is cheaper than instruction

The stated objection to prompt text is per-turn cost: the model re-reads it on every
invocation [08:20]. That objection does not apply to a hook that denies a call, nor
to a violation error that teaches at the moment of violation — both cost zero tokens
until they fire.

codescout's progressive-disclosure architecture is already this pattern: on-demand
`get_guide` topics, `@ref` buffers, and teaching error messages (the IL-3 block
returns a full explanation of the buffer system only when a pipe is actually
attempted). **The structural answer to the talk's objection is therefore already
built** — the open question is only how much of the always-on `server_instructions`
slice can migrate into it. That reframes the work from "delete guidance" to "move
guidance from the always-on channel to the on-violation channel," which preserves the
behaviour while eliminating the recurring cost.

---

## Proposed experiments

Untested proposals, ordered by expected value per unit of effort. Each targets an
existing codescout surface and uses the existing prompt-tdd harness in the sibling
`prompt-engineering` repo. Per repo convention, pre-register each in this repo's
`docs/trackers/prompt-hamsa-audit-log.md` before believing any count, and first read
the operating guide at `prompt-engineering:docs/trackers/prompt-tdd-operating-guide.md`
— note that path resolves in the **sibling repo**, not here — because a checker missing
its exec bit reports a clean `0/N` that is character-identical to a genuine floor.

1. **Iron Law ablation on `server_instructions`.** Arms: full slice / laws removed,
   routing quickref kept / empty slice. Primary metric: correct tool selection on the
   existing tool-selection scenarios. Secondary and equally weighted: total tokens and
   tool calls to a correct answer. Directly tests category (b) above.

2. **Line-by-line add-back.** Mirrors the source's stated method [05:52]. Start from
   the empty slice and reintroduce one law at a time, measuring each individually. The
   1900-character cap makes the candidate set small enough to enumerate exhaustively,
   which is unusual and worth exploiting.

3. **Instruction vs. enforcement separation.** Disable the companion plugin's
   native-tool denials with the Iron Laws still present, then the reverse. Separates
   "the model needs to be told" from "the model needs to be blocked" — they are
   currently confounded, and only the first is a prompt cost.

4. **Teach-upfront vs. teach-on-violation.** Empty `server_instructions` plus enriched
   violation errors, against the current always-on text. This is the direct test of
   the migration proposed in § *Enforcement is cheaper than instruction*, and the
   highest-value experiment here if the first three show the laws are still doing work.

5. **CLAUDE.md ablation.** The literal form of the talk's advice to delete CLAUDE.md,
   skills, and hooks every six months [06:55]. This repo's CLAUDE.md is large and has
   accumulated across several model generations. The `tool-usage-patterns` tracker
   (T-N verdicts) is the natural measurement surface, since it already scores observed
   tool calls against the ideal.

6. **Verification-affordance audit.** Given that verification is named the top
   practitioner failure [19:57], the question for codescout is whether it *offers* a
   verification surface or merely permits one. An agent can run tests via
   `run_command`, but nothing in the tool set is shaped as "check your own work."
   This is a product-overhang question about codescout itself rather than an ablation.

A caution that applies to all six: takeaway 8 implies any eval built here has a
useful life of roughly one to three model generations. Build them cheap enough to
throw away.

---

## Sources

- Boris Cherny, *"We Cut 80% of Claude Code's Prompt"*, Y Combinator, published
  2026-07-27, 35:51. <https://www.youtube.com/watch?v=qyPCVqFUyDo>
  Full transcript (YouTube auto-generated captions, retrieved 2026-08-23) archived at
  [`transcripts/2026-07-27-boris-cherny-yc-opus5-transcript.md`](./transcripts/2026-07-27-boris-cherny-yc-opus5-transcript.md).
