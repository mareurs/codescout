---
id: ec26c313227cbf30
kind: convention
status: active
title: Changing an enforcement gate — the procedure
owners:
- marius
tags:
- gate
- procedure
- testing
- shared-checkout
- mutation
topic: how to change a hook, a gate or a refusal
---

# Changing an enforcement gate — the procedure

**Audience:** anyone about to add, tighten, loosen or reword a hook, a pre-commit check, a CI test
that refuses something, or a refusal message. The principle behind it is
`docs/adrs/2026-10-16-a-rule-lives-where-the-agent-already-acts.md`. This page is the order of work.
Each step carries the failure that earned it, from the 2026-10-16 attempt to tighten `CHECK 3` of
`scripts/pre-commit-ledger-counts.py`.

## 0. Look for the instrument before you measure

Open `docs/PROBES.md` first. It lists every measurement instrument and its blind spot. Three
matter for a gate change:

| Question | Instrument |
|---|---|
| Would a proposed check have refused real past commits? | `python3 scripts/probe-backticked-count.py --replay` is the worked example for the ledger; copy its denominator discipline |
| Does the guard detect the defect it names? | `./scripts/mutation-probe.sh --file <p> --find '<literal>' --replace '<literal>' -- <test cmd>` |
| How often does a gate refuse, and what would unblock it? | `python3 scripts/probe_il3_refusals.py` |

**Why:** the 2026-10-16 attempt wrote about ten one-off scripts for questions these already answer.
A one-off script has no denominator guard and no recorded blind spot.

## 1. Work in an isolated worktree, never in the shared tree

The hook executes the copy of the script in the **working tree**
(`tests/pre-commit-ledger-divergence.sh` explains the divergence). A gate edit that is not yet
committed is therefore **live for every other session in the checkout** from the moment you save it.

**Why:** the tightened `CHECK 3` sat live in the shared tree for part of a session. It would have
refused 6.5% of real past filings that the old gate accepted. Nothing visible broke, but that was luck.
`./scripts/mutation-probe.sh` isolates by default for the same reason. Use the `using-git-worktrees`
skill, and see `docs/conventions/shared-checkout-commit-sequence.md` for the commit.

## 2. Write the cases from real inputs, and watch them fail

- One case per rule. Before you write it, name the production change that would make it fail.
- Take **accept cases from the live corpus**, not from your idea of it. An over-tight rule fails
  these. The 2026-10-16 accept cases came from 428 real entries.
- Take **refuse cases from the bug**, one per reproduced input.
- Run them against the unchanged code. **A case that passes now tests nothing about your change.**
  One case in the 2026-10-16 suite was written as a refusal and passed: the rule it assumed was
  stricter than the shipped one (per class, "one of", not per member), and the refusal text said so.
  That is a finding about the existing gate. Do not hide it: record it and leave the rule alone.

## 3. Replay the rule over history, with the gate's own firing condition

Count the real events the **old** rule accepted and your rule would refuse. That is the false-refusal
population.

- **Derive "the gate would have fired" the way the gate derives it.** The ledger gate fires when a
  bug file gains a cluster tag, not whenever a line changes. The first replay counted every changed
  line and reported 55 refusals. Replayed on firing events (443 of them), it refused 42, and 29 of
  those were events the old gate had accepted.
- **Subtract the baseline.** Events the old rule also refused are commits made without the hook
  enforcing. Of the 42 refusals in that replay, 13 were that.
- **Split by period.** A shape that decays is a different decision from one that recurs. Of the 4
  October events in the false-refusal set, 3 came from one commit on 2026-10-14.
- **Read the refused text.** The aggregate hides the finding: the refused entries were four real,
  repeated shapes, not noise.
- **Report the number and its reasons to the owner before you ship.** A rule that refuses real
  practice at 6% is a decision. It is not an implementation detail.

## 4. Mutate each rule on its own

Arm one mutation per rule with `./scripts/mutation-probe.sh` and run the suite. Read the result
carefully.

- **A mutant killed only by unrelated cases is not killed.** Removing the backtick requirement also
  broke the marker-adjacency check, so it failed the accept cases and left the two cases meant for it
  passing for another reason. Both cases lacked a marker as well as backticks, so the marker rule alone
  refused them and nothing pinned the backticks. Repair the mutant to remove one rule only, then add
  the case that was missing.
- **A case that is refused by two rules pins neither.** Give each case exactly one defect.
- A survivor has three readings (untested, unreachable, inert); see the `mutation-probe.sh` row in
  `docs/PROBES.md`.

## 5. Make the text and the check one thing

If the refusal says what a valid entry looks like, a test must prove that shape is accepted, and a
test must prove each shape the text calls invalid is refused. The 2026-10-16 bug exists because the
text said `+1: <slug>` followed by a derivation, and the code tested a substring.

Every refusal prints the exact remedy and the valid values.

## 6. Ship it whole

- The rule, its cases, its fixture changes and the documentation change go in **one commit**. A gate
  and the change that falsifies it are a pair (see the sequence page, step 3).
- Wire a toolchain-free suite into CI beside `tests/pre-commit-ledger-divergence.sh`, so that
  `--no-verify` is not the end of the check.
- If an existing fixture used an input your rule now refuses, update the fixture in the same commit.
  `tests/commit-mine.sh` used an 8-character derivation as a coupling fixture.

## Anti-patterns, each seen

- Tightening at commit time only: fires late, spans two files, can be skipped, and is not run in CI.
- A rule whose shape lives only in a refusal message.
- Measuring with a scratch script when `docs/PROBES.md` has an instrument.
- A fixture helper copied into a third test file. The shared traps (the hook refuses on
  `len(index_rows) <= 10`, and a slug with a digit is silently not a slug) are then explained again
  in each header. Extract one.
- Leaving a gate edit uncommitted in the shared tree.
