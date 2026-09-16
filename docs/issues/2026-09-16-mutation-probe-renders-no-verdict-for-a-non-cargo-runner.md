---
id: '6213a09765698cfa'
kind: bug
status: open
title: 'BUG: mutation-probe renders no verdict for a non-cargo runner, and its INCONCLUSIVE text names three causes that exclude the real one'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- mutation-testing
- probes
- remedy-text
topic: measurement instruments and their blind spots
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

## Classification

`cluster/guard-narrower-than-its-name`. The **interface** accepts any runner; the
**verdict** covers one, and the name reasoned with — "mutation-probe, `-- <command>`" — is
the wide one.

## Fix

Not attempted. Two forms, and only the first is cheap.

1. **A fourth cause in the message**, naming a non-cargo runner and what to do instead:
   read that runner's own count line and the verdict off it. Purely additive, reds on
   deletion under a shape assertion.
2. **A verdict for arbitrary runners** needs either `--count-pattern <regex>` or an
   explicit `--expect-kill` / `--expect-survive` contract the caller supplies. That is a
   design question, not a text fix, and the header's own argument forbids the shortcut of
   inferring a count from an unrecognised format.

## Tests

`tests/mutation-probe.sh` cases 4, 5 and 6 run `-- true`, which the usage block already
names as INCONCLUSIVE-by-design — so the suite has the *shape* of this situation and no
assertion about the message. A guard belongs where `CLAUDE.md` says: assert the refusal text
names a non-cargo runner among its causes. Reds on the deletion, survives rewording, and
cannot tell you the advice is *correct* — only that the case is still addressed.

## References

- `scripts/mutation-probe.sh:320-321` (the parse), `:326-330` (the refusal text), `:62-96`
  (the usage block that advertises `-- <command>` without scoping the verdict).
- `docs/issues/archive/2026-09-16-file-provenance-unknown-branch-omits-the-window-it-names.md`
  — the fix during which this was measured; its mutation table is the six runs above.
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`
