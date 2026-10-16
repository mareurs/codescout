---
id: '633576a9cd26b4db'
kind: adr
status: proposed
title: ADR-2026-10-16 — A rule lives where the agent already acts
owners:
- marius
tags:
- design-principle
- enforcement
- agent-behavior
- context-cost
topic: where rules and standards live
time_scope: '2026-10-16'
---

# ADR-2026-10-16 — A rule lives where the agent already acts

## Status

**Proposed.** The direction was agreed with the owner on 2026-10-16. No mechanism is built. The
companion decision, which is the first use of this principle, is
`docs/adrs/2026-10-16-the-reasoning-for-a-class-member-lives-in-the-members-own-bug-file.md`. The
procedure for changing a gate is `docs/conventions/changing-an-enforcement-gate.md`.

## Context

A rule changes what an agent does only if the agent meets it at the moment it acts. Five
observations, each dated 2026-10-16, each with its derivation.

1. **Prose with no trigger is not followed.** `docs/adrs/2026-09-14-state-the-property-not-the-snapshot.md`
   measured 0 uptake of a prose mandate across 77 dated claims, at `9045c56a`. The `CLAUDE.md`
   byte budget also only moves down, so prose cannot grow to compensate.
2. **A rule stated only in a commit-time refusal drifts.** The refusal for the ledger's
   `**Members:**` field says the entry has one shape. A replay over the 443 gate-firing events (a class gained a member) in the commits since
   2026-09-03 found at least four shapes in use, and a strict
   version of the stated shape would have refused 29 of those events (6.5%) that the real gate accepted.
   The check tested a substring, not the shape
   (`docs/issues/archive/2026-10-16-the-growth-gate-accepts-a-bare-stem-while-its-refusal-demands-the-plus-one-shape.md`).
3. **A rule inside the file the agent fills is followed.** Counting `## ` headings outside code
   fences in the 1,131 bug files: `## Summary` appears in 1,084 (96%), `## References` in 1,004
   (89%) and `## Resume` in 948 (84%).
4. **A pointer in `CLAUDE.md` did not fire.** The session that wrote this ADR (sessionId
   `0ff90aa2`) had two pointers in context from its first turn. One says to check
   `docs/PROBES.md` before answering with a number. The other names `scripts/mutation-probe.sh`.
   It wrote about ten one-off measurement scripts and its own mutation runner. Both instruments
   already existed: `scripts/probe-backticked-count.py --replay` replays a proposed ledger check over
   history, and `scripts/mutation-probe.sh` arms one mutation in isolation.
5. **The write path has more than one door.** In the 30 days to 2026-10-16, `.codescout/usage.db`
   records 168 `doc(create)` calls for bug files, 137 `create_file` calls under `docs/issues/`, and
   19 copies of the template. The counts are approximate: the first query also matches non-bug
   documents that name `docs/issues/`.

Two precedents show the write path can already host a content rule inside an existing tool, with no
new surface. `BUG_STATUSES` in `src/librarian/tools/create.rs` refuses an out-of-vocabulary bug
status. The `**Valid:**` grammar check in `src/librarian/statements.rs` refuses a malformed line with a
hint. `.codescout/librarian.toml` is an existing per-project policy file; today it only classifies
files, so extending it is a design step, not a feature that exists.

**Cost.** A new MCP tool or parameter adds its description to the context of every session of every
project that connects. A refusal adds text only to the session that makes the mistake.

## Decision

**A rule that agents must follow lives in the channel the agent already reads at the moment it acts.
Choose the channel from the table, and use prose last.**

| Channel | Fires | Context cost | Enforces | Use it for |
|---|---|---|---|---|
| Refusal in the write path, inside an existing tool, with the remedy | at the write | none unless violated | yes | an invariant of one artifact |
| The template the agent fills | at authoring | none | no, but followed 84–96% | the structure |
| Project config read by the write path | at the write | none | — | the policy, so the mechanism stays generic |
| A one-file CI test | at push | none | yes, and `--no-verify` cannot skip it | doors the write path cannot see: a native `Write`, a `cp` |
| A commit hook | at commit | none | bypassable | an invariant across two artifacts, and only until it is restructured into one |
| Prose in `CLAUDE.md` or a guide | never | every session | no | the reason, never the rule |

Six rules follow.

1. An invariant of **one** artifact is checked at the write path and again in a one-file CI test.
2. A rule about **two** artifacts, such as a count and its member, is a design smell. Restructure
   so the fact lives in one artifact. Use a hook only until then.
3. Every refusal prints the remedy and the valid values. The text costs context only when the
   mistake is made.
4. The refusal text and the check are one thing. A test asserts that every shape the text calls
   valid is accepted, and every shape it calls invalid is refused.
5. Prose may explain why. It never carries the rule.
6. The policy is data (`.codescout/librarian.toml`). The server holds only the generic mechanism, so
   another project that uses codescout is not given this repository's conventions.

## Alternatives considered

**A. A new MCP tool for filing a bug.** Rejected. It adds description text to every session of every
project, and the owner ruled out added context.

**B. Hooks that inject a reminder at the moment of filing.** Kept as a supplement only. They need no
tool, but they add context each time they fire, they advise and do not enforce, and they live in
another repository.

**C. More prose in `CLAUDE.md`.** Rejected: observation 1 and the byte budget.

**D. A commit-time gate for everything, the status quo.** Rejected. It fires after the work, across
two files, can be skipped with `--no-verify`, and is not run in CI (`.github/workflows/ci.yml`
exercises the hook script only through fixtures).

**E. Do nothing.** Rejected: observations 2 and 4.

## Consequences

- **Now easier:** feedback arrives at the mistake, per file, with no shared index involved. A new
  artifact kind needs a config entry and a validator hook, not a prompt edit. Session context does
  not grow.
- **Now harder:** the check lives in server code, so a change needs a rebuild and a release. A wrong
  refusal costs a retry, so a proposed rule must be replayed over history first (see the convention
  page). The write path covers only codescout's own tools, so the CI test is not optional.
- **Not changed:** a check enforces presence and shape, not quality. A placeholder still passes.
  Mutation tests must pin each rule individually.

## Change scenarios absorbed

- A new artifact kind needs a required section: add a config entry; no prompt, tool or guide edit.
- A project that uses codescout has different conventions: its own config; this repository's rules do
  not reach it.
- A third way to create a file appears: the CI test already covers it.

## Revisit-when

- A rule's violation cannot be seen at write time, because it spans two artifacts that cannot be
  merged. That rule stays a hook, and this ADR does not apply.
- Refusal text grows long enough that a session pays for it often. Measure the refusal rate in
  `.codescout/usage.db` by error family before deciding.
- Agents begin to route around the write path. The one-file CI test is the backstop.

## Confidence

**Medium.** Observations 1 to 4 are measured. Not established: whether `create_file` can host the
same validator as `doc(create)`, whether `.codescout/librarian.toml` can take new keys without a larger change,
and the refusal retry rate in practice. The first slice of the companion decision is meant to
measure the last of these.

## What this ADR does not decide

The content of the bug-filing rule (`## Class fit` and one cluster tag). That is the companion ADR.
