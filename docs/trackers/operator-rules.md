---
id: fa21bfb35684794d
kind: tracker
status: active
title: Operator Rules (OP-N)
tags:
- operator-rules
- engine-5
- ledger
entry_prefix: OP
entry_high_water_OP: 5
---

# Operator Rules (OP-N)

Rules that hold across every project, tool and model for this operator. Compiled into each Claude Code profile's CLAUDE.md by `codescout operator-rules compile`.

Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

## Index

| ID | Binding | Covers | Evidence | Status |
|---|---|---|---|---|
| OP-1 | always | unverified-assertion | measured: conclude-last/b2 0% -> 100% (n=35) | active |
| OP-2 | triggered | underpowered-subagent-dispatch | unmeasured | active |
| OP-3 | triggered | durable-fact-written-to-per-profile-store | unmeasured | active |
| OP-4 | triggered | partial-profile-config-update | unmeasured | active |
| OP-5 | always | unverified-assertion | measured: a3 13.3% alone; stacked it cuts OP-1 7/10 → 2/10 | **retired** |

One `always` rule against a 3–5 start band and a 5–10 ceiling, so there is real headroom —
but headroom is not licence: Gate 3(a) binds independently of size, and all four `**Covers:**`
slugs name distinct failure modes.

**Routing status, 2026-09-01 — supersedes "they do not compile into any profile until Phase 2
builds the routing that reads `**Serves:**`".** That routing landed. `Tool::selector_key`'s
default was inverted at `30b6fc41` so all 21 registered tools supply a selector, and
`annotate_write_path` at `a6b4fc35` gave write responses a path for `path~` to match. Both
`OP-3` and `OP-4` now route on a real call, each with an end-to-end test through its real tool
(`tools::memory::tests::a_real_memory_write_call_delivers_op_3`,
`tools::core::tests::a_real_edit_file_write_under_dot_claude_delivers_op_4`).

**But routing is not the same as effectiveness, and for two of the three rules it is not even
the binding constraint.** `OP-2` and `OP-4` are the honest cases at either end, and `OP-3` sits
in between in a way worth stating precisely, because it looks solved:

| rule | routes? | does the trigger see the VIOLATION? |
|---|---|---|
| `OP-4` | yes | **yes** — a config write under `~/.claude` is an `edit_file`/`create_file` call, so the rule fires on the act it governs |
| `OP-3` | yes | **no** — it serves `memory.write` (*codescout's* tool), while the violation is a write to Claude Code's **built-in** store under `<config-dir>/projects/…/memory/`. The trigger fires when the agent is already doing the right thing |
| `OP-2` | no | **no** — serves `Agent`/`Task`, harness tools that are not `crate::tools::Tool` implementors in this process at all |

So `OP-3`'s resident copy in `~/.claude*/CLAUDE.md` is **not** made redundant by its routing,
and must not be removed on that basis: it is the only copy reaching an agent *before* it
chooses a store. The routed copy reinforces compliance; the resident copy prevents the failure.
They are different jobs.

**`OP-2` and `OP-3` therefore share one blocker and are one problem, not two:** the event that
should trigger each never enters this process. Neither is reachable by any amount of
`selector_key` work, and both want a companion-plugin hook — which `CAP-10` rejected on
agent-agnosticism grounds, a rejection worth re-opening now that the alternative is measurably
nothing. That is a design decision with an operator call in it, not routing work.

## OP-1 — Always verify before asserting

**Imperative:** Do not hypothesise but ALWAYS VERIFY. Do not go over code or issues found quickly and assume, but ALWAYS VERIFY!
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** measured: conclude-last/b2 0% -> 100% (n=35)
**Rests on:** prompt-hamsa-audit-log:A-21
**Status:** active

**Valid:** invariant

The active ingredient is an unconditional imperative that binds at every claim. A-21 measured 11 arms: b2 imperative-only scored 100.0%, beating the full paragraph at 93.3%, against 0% bare. Conditional guards gate on the doubt a planted belief suppresses, which is why the guard-shaped variants lost.

**The imperative is `b2-imperative-only.md` verbatim, and that is deliberate — corrected 2026-08-28.** It previously read *"Do not hypothesise — ALWAYS VERIFY."*: a condensation of b2 into one clause, carrying b2's `**Evidence:**` line unchanged. That is a measured number attached to a string no arm has ever scored, which is the exact failure `**Evidence:**` exists to prevent — *a law and a hunch must not read alike*. It also confounded the spec's Verification prediction 2: a sub-100% result could not have distinguished "compilation altered the rule's form" from "the condensation did".

So the rule that ships is now the rule that was measured, and the arm varies only the compiler's wrapper.

**One deviation, recorded rather than hidden.** `b2-imperative-only.md` wraps its text across two source lines; `**Imperative:**` is a single-line field, so the rendered block joins them with a space instead of a newline. In markdown both are a soft break inside one paragraph, but the bytes differ and the arm therefore tests wrapper-plus-rewrap, not wrapper alone.

**Related but distinct: `s1-shipped-bootstrap`.** codescout's own `project-activation-bootstrap` guide ships a *third* wording (*"…Do not state what a doc, a memory, or a prior belief says the code does — open the artifact or run the command…"*). It is not this entry's text and its arm is scored separately; do not read `s1`'s number as evidence for `OP-1` or the reverse.

## OP-2 — Sonnet is the subagent-dispatch floor

**Imperative:** Never dispatch an implementer or reviewer subagent on Haiku — Sonnet is the floor.
**Binding:** triggered
**Shape:** imperative
**Covers:** underpowered-subagent-dispatch
**Serves:** Agent, Task
**Evidence:** unmeasured
**Rests on:** `~/.claude/CLAUDE.md` § Iron Rules → *Subagent Dispatch — Model Floor + Review Escalation*
**Status:** active

**Valid:** conditional — the model lineup changes (a new Haiku tier ships, or the named tiers are renamed)

Transcribed, not rewritten. The source rule holds the floor **and** a review-escalation clause; the floor is the part statable as one imperative, so it is the `**Imperative:**`, and the escalation is recorded here rather than split into a second rule or dropped.

The floor binds even when a skill's own model-selection rubric argues down to Haiku — the source names `superpowers:subagent-driven-development`'s "mechanical transcription" case specifically, where the plan text contains the complete code and the rubric therefore says the cheapest tier suffices. It does not.

**Escalation clause (transcribed):** for task reviews, prefer a stronger model than the skill's default mid-tier reviewer, especially for test-rigor and edge-case coverage on load-bearing code. Budget at least one Opus pass per task — or at minimum the whole-branch final review — whenever the code under review is infrastructure later tasks build on.

The source cites one validation, 2026-07-07 (EDU-Planner backend-kotlin, SI-29 SDD execution, teacher-week-headroom plan Task 1): a Sonnet-tier task review approved a new module with zero Important findings, and a blind Opus re-review of the same diff, framed with a mutation-testing lens, found a genuine Important gap — a function's `(owner, date)` key-discrimination logic with zero test coverage, where a mutation dropping either dimension would have passed the suite silently.

`**Evidence:** unmeasured` is correct despite that, and the distinction is the point of the field: one observed case with no base arm is an anecdote, not an arm. Promoting it to `measured:` would require a grid in the shape `A-21` used for OP-1. Per § *Rollout*, commissioning that arm is Phase 3 work still outstanding, and its result is what decides whether this rule keeps its `triggered` binding at all.

## OP-3 — Durable facts go to codescout memory, never Claude Code's built-in memory

**Imperative:** Never write a durable fact to Claude Code's built-in memory — persist it to codescout memory or a tracker.
**Binding:** triggered
**Shape:** imperative
**Covers:** durable-fact-written-to-per-profile-store
**Serves:** memory.write
**Evidence:** unmeasured
**Rests on:** `~/.claude/CLAUDE.md` § Iron Rules → *Memory — Use Codescout, Not Claude Code Memory*
**Status:** active

**Valid:** conditional — Claude Code's built-in memory becomes shared across profiles, or this machine stops running more than one profile

Transcribed, not rewritten. The prohibition is asymmetric and the source says so explicitly: **reading** the built-in store is fine, **writing** durable facts to it is not. The imperative is scoped to the write for that reason, and the selector follows it.

The mechanism is per-profile isolation, which is why this is an operator rule and not a project one. Built-in memory lives under `<config-dir>/projects/.../memory/`, and this machine runs three config dirs — `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`. Anything written to one profile's store is invisible to the other two, so a fact recorded there is lost to two thirds of this operator's sessions. The failure is silent in the worst direction: the write succeeds, so nothing signals that the fact did not travel.

The two durable surfaces the source names: **codescout memory** (`memory()`) for project-exploration notes and system-prompt facts tied to a codebase, and **codescout trackers** (librarian artifacts) for structured ongoing state — sessions, decisions, bugs, friction logs.

`**Covers:** durable-fact-written-to-per-profile-store` names the failure mode rather than the tool, so Gate 3(a) compares it against future rules on the same axis. The failure is not "used the wrong API"; it is "wrote something meant to outlive the session into a store two thirds of the sessions cannot read".

## OP-4 — Config changes apply to all three Claude Code profiles

**Imperative:** Apply every Claude Code config change to all three profiles — `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`.
**Binding:** triggered
**Shape:** imperative
**Covers:** partial-profile-config-update
**Serves:** edit_file(path~/.claude), create_file(path~/.claude)
**Evidence:** unmeasured
**Rests on:** `~/.claude/CLAUDE.md` § *Three Claude Code Instances*
**Status:** active

**Valid:** conditional — this machine stops running three profiles, or the profiles gain a shared config layer

Transcribed, not rewritten. Scope as the source states it: plugins, `settings.json`, `installed_plugins.json` — config that is per-profile by construction. The three profiles share plugin **source** repos but have separate caches, settings, and install records, so a change applied to one is simply absent from the other two.

**The verification half, transcribed:** each profile must use its **own** `cache/` directory. When updating install records, verify that `installPath` starts with the same profile root as the file containing it. The source records the case that motivates this — 2026-05-16, `~/.claude-kat/installed_plugins.json` was found with a cross-profile `installPath` pointing at `~/.claude`'s cache, attributed to past config drift.

That case is why the imperative is "apply to all three" and not "remember there are three": the drift already happened once, and the wrong state was well-formed JSON that no tool objected to.

This rule's own subject matter makes it the most self-demonstrating entry in the ledger, and the compiler is the mechanism that discharges it for one specific file. Measured 2026-08-27 before any compile had run: `~/.claude/CLAUDE.md` is 4639 bytes (md5 `b583ffaa`) while `~/.claude-sdd/CLAUDE.md` and `~/.claude-kat/CLAUDE.md` are both 4640 (`d52fc86c`) — two in step, one drifted, with nothing having reported it. `operator-rules check` is the check that did not exist; this entry is the rule it enforces.

**Note for Phase 2 — this selector is the one most likely to need work.** `path~` reads the **result**, and `names_path_containing` scans `abs_path` / `rel_path` / `items[]` / `violations[]`, shapes that a librarian response carries and an `edit_file` response may not. Recorded here as the spec projects it (§ 4); whether it matches a real `edit_file` call is a Phase 2 question, and a selector that silently never fires is exactly the failure `parse_shape`'s strict `is_ident` check exists to prevent elsewhere.

## OP-5 — Conclude Last — Never Narrate Mid-Evaluation (retired, superseded by OP-1)

**Imperative:** When making any claim that depends on evaluation, complete the full evaluation in thinking before writing the claim.
**Binding:** always
**Shape:** procedure
**Covers:** unverified-assertion
**Evidence:** measured: conclude-last/a3 0% -> 13.3% (n=15)
**Rests on:** prompt-hamsa-audit-log:A-20, prompt-hamsa-audit-log:A-21, prompt-hamsa-audit-log:A-32
**Status:** retired
**Valid:** invariant

**Retired 2026-08-28, and deleted from the three profiles the same day — not discarded.**

> **`**Evidence:**` records its own arm only.** The stacking result that actually retired this rule — it cuts `OP-1` from 7/10 to 2/10 — lives in the prose below, because the field's grammar is strictly `measured: <arm> <base>% -> <shipped>% (n=N)`. Appending prose to it made `operator-rules check` fail with `Error: invalid float literal`, naming neither the rule nor the field. Gate 6 refusing a malformed measurement is correct; the diagnostic is not.

This entry exists so the text survives in git and can be restored by flipping `**Status:**` back to `active`. It was previously resident only in `~/.claude/CLAUDE.md`, `~/.claude-sdd/CLAUDE.md` and `~/.claude-kat/CLAUDE.md`, all **untracked**, so deleting it would otherwise have destroyed the only copy.

`**Binding:** always` with `**Status:** retired` is deliberate and safe: `render_block` and `check_budget` both filter on `binding == Always && status == Active`, so a retired rule is parsed and validated but never compiled into a profile. Flipping the status is the whole restore.

### Why it was retired

It addresses the same failure mode as `OP-1` — `**Covers:** unverified-assertion` on both — and measurement says the pair is worse than `OP-1` alone. `A-32`, 2026-08-28, plausibility-class verified rate:

| configuration | verified (n=15) | excl. broken t2 (n=10) | wrong+unchecked (n=35) |
|---|---|---|---|
| OP-1 block alone | 80.0% | 7/10 | 0/35 |
| this prose + block | 53.3% | 5/10 | 2/35 |
| the real profile (same stack) | 33.3% | **2/10** | 2/35 |

Alone it scored **13.3%** verified (`A-21`, arm `a3-conclude-last`). Stacked, it pulls `OP-1` down toward that figure instead of adding to it — `A-20`'s `a5-both` result on a new pair. Its shape is `procedure`, which `A-21` ranks below `imperative` precisely because *\"procedural detail only applies once checking has begun\"*.

**A repaired variant exists and was never shipped here.** `a3v2-conclude-last-grounded` adds a fifth antidote item and scores **73.3%**. `A-21`'s closing note claims that item was applied to all three profiles; it was not — the profile text is byte-identical to unrepaired `a3`. If this rule is ever revived, revive `a3v2`, not this.

### The text, verbatim as it stood

```markdown
### Conclude Last — Never Narrate Mid-Evaluation

When making any claim that depends on evaluation — tracing logic, comparing values,
following data flows, weighing tradeoffs, determining causation — complete the full
evaluation in thinking before writing the claim.

Once you commit to a direction in your output ("this causes X", "the effect is Y"),
autoregressive momentum pulls subsequent tokens toward coherence with that commitment
rather than toward correctness. The antidote:

1. **Observe first.** State what you see without interpreting it.
2. **Evaluate in thinking.** Work through the logic silently and arrive at a conclusion.
3. **Then narrate.** Write the user-facing claim only after the evaluation is complete.
4. **Self-correct openly.** If you realize mid-stream you committed wrong, say so.
   Contradicting yourself is better than being confidently wrong.

Treat "sounds right" as a red flag. If a claim feels obvious, that is when it most
needs verification — obvious-sounding claims are the ones that slip past unchecked.
```

Also preserved, body-identical, as `prompt-engineering:scenarios/conclude-last/arms/a3-conclude-last.md` (tracked), which is what the `a3` arm measures.

### Restoring it

Flip `**Status:**` to `active` and run `codescout operator-rules compile`. That alone re-arms it — but Gate 3(a) should refuse it while `OP-1` is active, since both carry `**Covers:** unverified-assertion`. Retiring `OP-1` first, or reviving `a3v2` under a distinct `**Covers:**`, are the two coherent paths.

**Do not restore it on the strength of intuition alone.** The measurement that retired it is n=10 on the cleanest cut, with a ±30pt per-cell noise band — a large effect at small n. `A-33` sets up the confirming run.

## Template for new entries

<!-- Insert new OP-N entries above this line. Use artifact(action="append_entry", id=<this artifact>, id_prefix="OP", anchor_heading="## Template for new entries", title=…, body=…) — never hand-format the heading. -->
