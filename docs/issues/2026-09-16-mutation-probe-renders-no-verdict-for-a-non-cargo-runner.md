---
id: '6213a09765698cfa'
kind: bug
status: taken
title: 'BUG: mutation-probe renders no verdict for a non-cargo runner, and its INCONCLUSIVE text names three causes that exclude the real one'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- mutation-testing
- probes
- remedy-text
topic: measurement instruments and their blind spots
claimed_by: a3bf229c-658b-42f9-8f4b-794fcf0d35c7
closed: null
opened: 2026-09-16
severity: medium
---

## Summary

`scripts/mutation-probe.sh` advertises `--file <path>` plus `-- <any command>`, and nothing
in its usage block or its 60-line rationale header says the **verdict** machinery is
cargo-only. It handles a shell suite correctly — mutates, asserts the pattern occurred once,
runs, reverts — and then cannot render a verdict for it, because the count parse at `:320-321`
is `grep -E '^running [0-9]+ tests?$'`, a cargo format. Every run against a `tests/*.sh`
suite is `INCONCLUSIVE` however decisive the result was.

## Symptom (Effect)

Measured 2026-09-16 while fixing `857ccb33f26c7363`. Six mutations of
`scripts/file-provenance.py`, each run as
`mutation-probe.sh --file scripts/file-provenance.py --find … -- bash tests/file-provenance.sh`.

All six returned `INCONCLUSIVE`. Four were decisive **kills** and two decisive **survivals**,
and the discriminator was sitting in the same captured output the probe declined to
interpret — the suite's own `passed=N failed=M` line against a known `passed=153 failed=0`
baseline. The whole mutation table for that fix had to be read off that line by hand.

Nothing failed. No wrong answer, no bad exit code — the tool refused, correctly, and the
refusal is indistinguishable from the three causes it does name.

## Reproduction

```
./scripts/mutation-probe.sh --file scripts/file-provenance.py \
    --find 'if floor and records:' --replace 'if floor:' \
    -- bash tests/file-provenance.sh
  -> passed=152 failed=1            # the suite's own verdict: KILLED
  -> mutation-probe: INCONCLUSIVE — no test-count line in the output, …
```

## Root cause

Read, not inferred. `scripts/mutation-probe.sh:320-321`:

```bash
ran_lines=$(grep -cE '^running [0-9]+ tests?$' "$RUNLOG" || true)
executed=$(grep -oE '^running [0-9]+ tests?$' "$RUNLOG" | awk '{s+=$2} END {print s+0}')
```

A shell suite never emits that line, so `ran_lines` is 0 and the refusal at `:326` fires
before any verdict branch is reached.

### The refusal is CORRECT; the remedy text is what misroutes

The header argues the refusal branch is load-bearing, and it is right — *"if the format ever
changes, this must decline to render a finding rather than fall back to one."* Inferring a
count from an unknown format would reintroduce `absence rendered as a value`, which that
same header is about. So this is **not** a request to guess.

The defect is the next sentence a reader acts on. `:326-330` names three causes:

> the mutation did not COMPILE; the command was not a test runner; or the runner's
> `'^running N tests'` line has changed shape and this parse needs updating.

For a shell suite the first is **inapplicable** (nothing compiles), the second is **false**
(it is a test runner, and it ran 153 tests), and the third implies a regression in a format
that never applied to this run. A reader who trusts the list goes hunting a compile error or
doubts a command that worked. That is `CLAUDE.md` § *Testing Discipline*'s measured law
exactly: a suite tests a guard's **predicate** and never its **remedy text**, so the half
that sends you somewhere useless is untested by construction and no mutation reaches it.

The predicate here has 54-assertion-grade care behind it. The fourth cause is missing.

### The scope was already published — two days earlier, to a surface this reader is not standing on

Found while fixing, not while filing. `docs/PROBES.md` has carried the bound since
`cf3facf6` (2026-09-14 17:05): *"SCOPE: the count keys on cargo's `^running N tests`, so
ANY non-cargo runner — a shell suite, pytest — now renders INCONCLUSIVE always"*, naming
the excluded population **and** the workaround. It was two days old when the six
INCONCLUSIVE runs above were read as the probe failing.

So this is not an unpublished bound, and "document it" is not the remedy. It is `CLAUDE.md`
§ *Observer Blindness* position 3: the scope lives on the surface read when **choosing** an
instrument, and the two surfaces the misrouted reader actually stands on — `usage()` when
typing the command, the refusal text when reading the result — did not carry it.
Publishing it a fourth time would be redundant; **moving** it is the fix, which is why the
change lands in those two places and PROBES.md gains only a pointer.
## Classification

`cluster/guard-narrower-than-its-name`. The **interface** accepts any runner; the
**verdict** covers one, and the name reasoned with — "mutation-probe, `-- <command>`" — is
the wide one.

## Fix

**Shipped 2026-09-16 — form 1, plus the scope moved to both read surfaces.** Form 2 is
untouched and remains a design question.

1. **The `ran_lines == 0` message in `scripts/mutation-probe.sh`** — a fourth cause naming
   the non-cargo runner, and a closing paragraph saying what to do instead: the mutation
   still applied exactly once and reverted, so only the verdict is missing, and it is
   readable off that runner's own summary line against a known-clean baseline — never off
   the exit code alone. Purely additive; the refusal predicate is untouched.
2. **The `usage()` block** — a `SCOPE` paragraph on the `--` line, because the caller
   choosing a runner reads that *before* the run and the refusal only *after* it.
3. **A comment at the parse**, recording why the cause list is deliberately NOT a branch on
   argv. The obvious improvement — print only the non-cargo cause when the command holds no
   `cargo` token — is unsound in the direction that matters: `-- ./scripts/gate.sh` runs
   cargo *inside* a wrapper, so a mutation that fails to COMPILE under one arrives here with
   no `cargo` token in argv and would be told, confidently, that its runner is not cargo.
   That is this same class one level down, and the next reader will have the idea.

**What did NOT change, stated so nobody credits it:** a non-cargo runner still gets no
verdict. Form 2 — `--count-pattern`, or an explicit `--expect-kill` / `--expect-survive`
contract the caller supplies — is unimplemented, and the header's own argument still forbids
the shortcut of inferring a count from an unrecognised format. What changed is that the
reader is now told so at the moment it matters, rather than sent hunting a compile error
that cannot exist.

## Tests

**Case 21 of `tests/mutation-probe.sh`**, reached by CI's own `mutation-probe-tests` job
(`.github/workflows/ci.yml:210-220`) — checked rather than assumed, since an assertion
nothing runs is decoration.

The fixture is the reported shape rather than case 13's compile-error shape: a runner
exiting NON-ZERO with its own decisive summary (`passed=154 failed=1`). Four assertions —
still INCONCLUSIVE, names the non-cargo cause, says where the verdict IS readable, renders
no verdict.

**Observed reds, not written assertions.** Pre-fix, 2 of the 4 were red. Per guarded site,
mutated with the probe itself against a known-clean `50 passed, 0 failed` baseline:

| mutation | result |
|---|---|
| delete the non-cargo clause | `49 passed, 1 failed` — KILLED |
| reword the remedy's `summary line` | `49 passed, 1 failed` — KILLED |

**And one measured SURVIVAL, which changed the test.** The first version asserted the
ENTITY — case-insensitively, *"does this message still say cargo at all"* — which is what
`CLAUDE.md` § *Testing Discipline* prescribes for a remedy text, because it reds on deletion
and survives rewording. It does not hold here: the remedy paragraph two lines below the
clause says *"On the NON-CARGO cause"*, so deleting the clause entirely left `cargo` in the
output and the assertion green — `50 passed, 0 failed`, SURVIVED. That is the scope law
itself: an assertion computed over a POPULATION (the whole message) cannot verify a claim
about a MEMBER (one clause), and re-reading it returns a true sentence either way.
**The pre-fix red did not cover it** — that red was observed for a *different* needle at the
same site, so it was evidence for that needle and not this one.

The needles are therefore phrases (`not CARGO`, `summary line`), which WILL red on a
rewording that keeps the advice. Accepted rather than unnoticed: the message line names both
pinned tokens and points at case 21, so a rewriter meets an instruction rather than a red
they read as a regression.

## References

- `scripts/mutation-probe.sh:320-321` (the parse), `:326-330` (the refusal text), `:62-96`
  (the usage block that advertises `-- <command>` without scoping the verdict).
- `docs/issues/archive/2026-09-16-file-provenance-unknown-branch-omits-the-window-it-names.md`
  — the fix during which this was measured; its mutation table is the six runs above.
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`
