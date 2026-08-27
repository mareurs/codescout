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
entry_high_water_OP: 4
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

One `always` rule against a 3–5 start band and a 5–10 ceiling, so there is real headroom —
but headroom is not licence: Gate 3(a) binds independently of size, and all four `**Covers:**`
slugs name distinct failure modes. The three `triggered` rules are recorded and validated now;
they do not compile into any profile until Phase 2 builds the routing that reads `**Serves:**`.

## OP-1 — Always verify before asserting

**Imperative:** Do not hypothesise — ALWAYS VERIFY.
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** measured: conclude-last/b2 0% -> 100% (n=35)
**Rests on:** prompt-hamsa-audit-log:A-21
**Status:** active

**Valid:** invariant

The active ingredient is an unconditional imperative that binds at every claim. A-21 measured 11 arms: b2 imperative-only scored 100.0%, beating the full paragraph at 93.3%, against 0% bare. Conditional guards gate on the doubt a planted belief suppresses, which is why the guard-shaped variants lost.

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

## Template for new entries

<!-- Insert new OP-N entries above this line. Use artifact(action="append_entry", id=<this artifact>, id_prefix="OP", anchor_heading="## Template for new entries", title=…, body=…) — never hand-format the heading. -->
