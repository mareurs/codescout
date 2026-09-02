---
id: '713ca66260a29ed7'
kind: adr
status: active
title: ADR-2026-08-27 — A negative result names its scope
owners:
- marius
tags:
- design-principle
- false-negative
- tool-contract
- legibility
topic: tool-contracts
---

# ADR-2026-08-27 — A negative result names its scope

## Status

Accepted — active. Distilled 2026-08-27 from three fixes shipped the same day
(`444d756c`, `fc1bbf21`, `76e287f8`) plus six pre-existing, independently-authored
instantiations of the same rule. Promoted from `bug-fix-session-log:W-72`, whose
`Promote-when` criterion (2+ confirmations across work streams) had fired several times
over before the pattern was named.

This ADR does not introduce a mechanism. It **names one the codebase had already built
six times**, which is the entire point: unnamed, it was re-derived from each new outage.

## Context

A tool that searches, resolves, or filters can return a negative result — `0 matches`,
`0 memories`, `file not found`, an empty list. That number is always true **of what the
tool examined**. It becomes false at the moment the caller reads it as *"the thing does
not exist"*, and nothing in a bare negative distinguishes the two.

Three bugs fixed on 2026-08-27 were this one defect:

| Fix | The negative | The scope it never named |
|---|---|---|
| `444d756c` | `grep` → `0 matches` | the glob admitted **no file at all** — a project-root-relative glob is unsatisfiable once `path` narrows the walk root |
| `fc1bbf21` | `activate` → `0 memories` | **which of two directories** it read; the two coincide only for the root project |
| `76e287f8` | `read_file` → `file not found` | **which tree** it searched, after a subagent reassigned `default_workspace_root` |

Each had been filed as an unrelated bug in an unrelated subsystem. Two had sat open for
weeks; one had been mis-attributed to an LSP staleness window it had nothing to do with.

**The codebase already knew the rule — six times, never once as a shared name:**

- `unsatisfiable_absolute_glob` (`src/tools/grep.rs`) — a `RecoverableError` for an
  absolute glob outside the search root. Its doc comment states the general principle
  outright: *"A false negative that looks like a finding is worse than an error."*
- `WalkAudit.accepted` (`src/tools/symbol/symbols.rs`) — *"Zero here is the strongest
  signal available that `root` is not the tree the caller meant."*
- `WalkAudit::completeness_warning` (`src/tools/grep.rs`) — walk errors and hidden-path
  pruning attached to a zero.
- `semantic_starved` (`e4569fcc`) — `find(semantic=…)` reporting that it widened its KNN
  `k` and still could not fill the page.
- `check_tool_access`'s read-only hint (`00948381`) — a write refusal naming the subagent
  clobber as a cause to check.
- `read_file`'s truncation flag (`21507a26`, patch-id
  `1bc96ae356d6741c08dbcd5a30c462a8e87ab06b`) — and
  note it shipped **inert**, rendered nowhere, which is the same defect one level up.

Six correct instincts with no common vocabulary. The cost is concrete and measurable:
`unsatisfiable_absolute_glob` shipped 2026-08-18 stating the principle in its own doc
comment, and the *relative*-glob case sat unguarded **in the same function** for nine
days. `symbols::WalkAudit` carried the `accepted` counter that `grep::WalkAudit` needed,
one directory away, for months.

## Decision

**A tool that can return a negative result must be able to name the scope it examined —
and must stay quiet when the negative is trustworthy.**

Three clauses, all load-bearing:

1. **Name the scope on a suspicious negative.** Root, filter, directory, index — whatever
   bounds the search. The caller cannot distinguish absence from mis-scoping without it.
2. **Stay silent on a trustworthy negative.** `completeness_warning`'s own doc states why:
   `None` is load-bearing *"or the warning becomes noise attached to every empty result and
   stops being read at all."* A warning on every zero is equivalent to no warning.
3. **Claim only what is proven; offer the rest as a thing to check.** An empty tree and a
   starving filter produce the same count, so *"no file under `<root>` passed the glob
   filter"* is assertable and *"your glob is misanchored"* is not. This is not
   fastidiousness: `unsatisfiable_absolute_glob`'s note records a zero that carried the
   hidden-paths warning whose remedy could not possibly have helped, and *naming an
   unchecked cause ends the search for the real one.*

### How to satisfy it — three established mechanisms

| Shape | When | Example |
|---|---|---|
| **Pre-flight predicate** → `RecoverableError` | the input is unsatisfiable *by construction*, before any work | `unsatisfiable_absolute_glob` |
| **Post-hoc audit counter** → warning field beside the result | the work ran and examined nothing | `WalkAudit.accepted` + `completeness_warning` |
| **Scope carried on an existing error** | something already fails; the error is the carrier | `read_file` `(searched <abs path>)` |

The third is the cheapest and most overlooked. `76e287f8` needed no new field and no new
control flow: `resolved` was already in scope at the failure site and simply absent from
the message.

### The corollary that decides ties

**Agreement is not correctness.** Two surfaces that disagree can be fixed by making them
agree or by making them both right, and on the repo in front of you those are
indistinguishable. `fc1bbf21` chose "agree", generalising from one workspace; on a
second workspace with the opposite layout it would have made both agree at **zero** —
hiding 49 files behind a reader that now looked authoritative. `020ea69a` corrected it.
A disagreement is itself a signal; removing the signal is not the same as removing the
defect.

## Consequences

### Now easier

- A new tool's negative-result behaviour is a checklist item at review time rather than a
  future bug file.
- The three mechanisms above are nameable in review — "use the audit-counter shape here" —
  instead of each author inventing one.

### Now harder / lost

- Slightly more code on every search-shaped tool, and a judgement call per tool about
  which negatives are trustworthy. Clause 2 makes that judgement mandatory rather than
  optional, which is the intended cost.

### Change scenarios absorbed

- A tool gains a new filter dimension → the audit counter already answers "did the filter
  admit anything", whatever the dimension.
- A new false-negative class appears → it lands in one of the three shapes rather than
  needing a bespoke design.

### Deliberately out of scope

`grep`'s and `symbols`' **plain** zeros stay bare. They carry no error to hold the fact,
and clause 2 governs: a root attached to every empty result is exactly the noise that
stops the signal being read. One surface naming the scope is enough to break the illusion,
and the surfaces that already error are the ones that should carry it.

### Revisit-when

- A measured case appears where a bare trustworthy zero **did** mislead a caller. That
  would put clauses 1 and 2 in genuine tension and this ADR would need to say which wins
  and on what evidence — today they have never conflicted in a recorded incident.
- A per-caller identity lands in the MCP `RequestContext`. Some scope-naming here is
  compensating for the workspace clobber
  (`docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md`); a structural fix
  would make that instance redundant, though not the general rule.

**Confidence: high.** Grounded in three same-day fixes across three unrelated subsystems,
six prior independent instantiations, and two peer sessions hitting the same class the
same day. The rule is descriptive of what this codebase already does when it is doing it
right; the ADR's contribution is the name and the third clause.

## Alternatives considered

1. **Leave it as six local idioms.** Rejected — that *is* the status quo, and its cost was
   measured: the relative-glob case unguarded for nine days inside the function whose doc
   comment states the principle.

2. **Attach scope to every negative result, unconditionally.** Rejected by clause 2, on
   evidence rather than taste: `completeness_warning`'s contract already says a warning on
   every empty result stops being read, and the codebase has a recorded incident of a
   correct-but-irrelevant warning ending the search for the real cause.

3. **A lint or test that every tool returning a zero also emits a scope field.** Rejected
   — it would enforce clause 1 while actively violating clause 2, and no mechanical check
   can decide which negatives are trustworthy. That judgement is the work.

4. **Fold it into `docs/PROGRESSIVE_DISCOVERABILITY.md`.** Considered; that document owns
   output *sizing* and overflow hints, and this is about output *truthfulness*. Kept
   separate, cross-referenced.

## Related

- `bug-fix-session-log:W-72` — the promoted win, with the full six-instantiation census.
- `444d756c` (patch-id `71410675cfbaca910d0bc122346b243a66e8d809`) — grep glob starvation.
- `fc1bbf21` (patch-id `f1a072d9ad760b5e57a1c867df1cc89f9bf390e4`) — sub-project memories.
- `76e287f8` (patch-id `f328f65909ef74d80768f7657d3d6e86d1bf4268`) — not-found names its tree.
- `020ea69a` — the memory union, and the "agreement is not correctness" correction.
- Code: `src/tools/grep.rs` (`unsatisfiable_absolute_glob`, `WalkAudit`),
  `src/tools/symbol/symbols.rs` (`WalkAudit.accepted`), `src/tools/read_file.rs`
  (`read_file_text`), `src/tools/markdown/read_markdown.rs` (`resolve_markdown_source`),
  `src/util/path_security.rs` (`check_tool_access`).
- `docs/issues/archive/2026-08-26-workspace-read-only-flips-mid-session.md` — `mitigated` and
  archived 2026-09-02; the structural half was **declined**, not deferred (see that file's
  `unverified:` field for both derivations).


- `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` — **the sibling,
  written later and generalising this one.** This ADR governs a *negative* result; that
  one governs the case this Decision does not reach, a confident non-empty value that is
  plausible and wrong. The distinction is load-bearing in both directions: a zero invites
  a second look, which is why clause 2 here can afford silence on a trustworthy one; a
  plausible value **suppresses** the second look, so it gets no equivalent exemption.
  Clause 3 here — *claim only what is proven* — reappears there as the tie-breaking
  corollary read from the caller's side: prefer the instrument that can return an error
  over the one that returns a value.
