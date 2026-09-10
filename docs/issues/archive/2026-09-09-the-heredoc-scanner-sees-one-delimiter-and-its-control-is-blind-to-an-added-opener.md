---
id: e08e935cec1ade7e
kind: bug
status: fixed
title: the heredoc backtick scanner selects one delimiter, and its non-vacuity control catches a renamed opener but not an added one
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
topic: test controls and selector scope
closed: 2026-09-09
opened: 2026-09-09
severity: medium
---

# BUG: the heredoc backtick scanner sees one delimiter, and its non-vacuity control is blind in the direction that matters

## Summary

`tests/pre-push-foreign-session-guard.sh:633` scans the guard's heredoc body for an
unescaped backtick — the class-level guard against the defect that once forked 45 processes
and printed no banner (`9dd4792f`). Its selector pins the literal opener `cat >&2 <<EOF` at
column 0. A **second** heredoc opened with any other delimiter is silently not scanned, and
the non-vacuity control beside it cannot see that, because the control asserts a **line
count** and the surviving first body already satisfies it.

So the pair reds on the harmless change (opener renamed) and passes on the dangerous one
(opener **added**).

## Symptom (Effect)

No symptom today — the guard has exactly one such opener, at
`scripts/pre-push-foreign-session-guard.sh:147`. The defect is armed, not firing.

Add a second banner as `cat >&2 <<WARN … WARN` and:

```
== no unescaped backtick survives in the unquoted heredoc body ==
ok  heredoc body has no live backtick
ok  and the scanner reached a real body (114 lines)
```

Both green. The `WARN` body was never read. A live backtick in it command-substitutes at
runtime, which is the `9dd4792f` failure mode: the push re-fires the hook, and the banner
never prints.

## Reproduction

Tree: `58284214` (`experiments`).

```
$ grep -c '^cat >&2 <<EOF$' scripts/pre-push-foreign-session-guard.sh
1
$ grep -nE '<<[A-Z]+$' scripts/pre-push-foreign-session-guard.sh
147:cat >&2 <<EOF
```

The selector, at `tests/pre-push-foreign-session-guard.sh:633`:

```sh
HEREDOC_BODY="$(awk '/^cat >&2 <<EOF$/{inbody=1; next} inbody && /^EOF$/{inbody=0} inbody' "$GUARD")"
```

and its control at `:642-646`:

```sh
BODY_LINES="$(printf '%s\n' "$HEREDOC_BODY" | grep -c . || true)"
[ "${BODY_LINES:-0}" -ge 100 ] \
    && ok "and the scanner reached a real body ($BODY_LINES lines)" \
    || no "and the scanner reached a real body" "found $BODY_LINES lines; the opener selector is stale, so the check above scanned nothing"
```

Current body: **114** non-blank lines. Headroom to the floor: **14**.

## Environment

Linux, `experiments`. Platform-independent.

## Root cause

**The selector enumerates one delimiter; the population is "every unquoted heredoc in the
file".** `awk` concatenates the bodies of *matching* openers, so a second `EOF`-delimited
heredoc would be covered — but any other delimiter is not selected, produces no body, and
therefore contributes no lines to `BODY_LINES`. The excluded member is never examined, so
there is no count to report and nothing to mark: the class's own *"a zero reads as 'not
present' rather than 'not looked at'"*.

**What makes this instance worth recording rather than routine:** the author **anticipated
exactly this risk and wrote a control for it**, and the control covers one direction only.
Its comment (`:638-641`) says:

> An emptiness assertion is monotone under removal: rename the opener, or reflow it onto two
> lines, and the awk matches nothing, `LIVE_TICKS` is empty, and this passes while scanning
> air. So pin that the scanner found a real body.

That reasoning is correct and the remedy is right for the case it names — a **renamed**
opener yields 0 bodies and reds. It does not reach an **added** opener, because the original
body survives and satisfies the floor by itself. Subtraction is caught; addition is not.

Measured 2026-09-09 by reading both sites and counting the openers in the guard — the
scanner was not mutated, because a second peer session was building against this checkout
and an armed mutation is the hazard
`docs/issues/archive/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`
records.

**A second, independent defect in the same four lines: the failure message asserts a cause it
did not measure.** It reads *"the opener selector is stale, so the check above scanned
nothing"*. What was measured is `BODY_LINES < 100`. A banner shortened below 100 lines — a
perfectly ordinary edit — fires this and reports a stale selector that is fine. The check
proportions itself to prose volume and then explains itself with a cause.

## Evidence

### Why it is reachable rather than hypothetical

A design approved on this checkout on 2026-09-09 proposed exactly the triggering edit: a
second, differently-named banner for a warn path alongside the refusal banner, with the
refusal prose cut down. That design has since been abandoned on other grounds, but it was
approved, and the review that killed it found this defect rather than the plan finding it.
The next person to add a second banner gets a green suite.

### The floor is proportional, and the property is not

`BODY_LINES >= 100` against a 114-line body encodes *"the banner is long"*, not *"the scanner
is live"*. Those coincide today by 14 lines.

## Hypotheses tried

1. **Hypothesis** — the awk selector handles multiple heredocs, so only a *rename* is a risk.
   **Test** — read the selector; it matches `/^cat >&2 <<EOF$/` only.
   **Verdict** — half confirmed. Multiple `EOF`-delimited heredocs concatenate correctly; a
   different delimiter is invisible. The risk is delimiter-specific, not count-specific.

2. **Hypothesis** — the floor would catch an added opener because total heredoc content grows.
   **Test** — the floor reads `HEREDOC_BODY`, which contains only *selected* bodies.
   **Verdict** — rejected. An added unselected body contributes zero lines; the floor sees
   the unchanged 114 and passes.

## Fix

Fixed on `experiments` at **`13b721f1`**, patch-id
**`c6fe2f8ea5467957b944bbedf2e9b5087c06e0b9`**.

Fixed as part of the change that would otherwise have **triggered** it: the pre-push banner
shrank from 114 to 42 non-blank lines, which reds the old floor for a reason the floor does
not name.

Three changes:

- **The scanner is delimiter-agnostic.** `/^cat >&2 <<EOF$/` became a match on any unquoted
  heredoc opener, with the delimiter captured and used as the terminator. It is now a named
  function, `scan_heredoc_bodies`, so the positive control below exercises **the code that
  ships** rather than a second copy of it.
- **Coverage floor replaces the size floor.** Scanner openers must equal the count of
  unquoted heredoc openers, and that count uses a **deliberately different** expression —
  it admits the `<<-` indented form and a trailing space, which the scanner's does not — so
  it is not the scanner agreeing with itself. Quoted openers are excluded from both: they
  cannot interpolate, so counting them would red on a construct that is safe by definition.
- **A positive control by mutation.** One live backtick is injected into every body of a
  copy, and the scanner must return one hit per body.

The failure text no longer names a cause it did not measure.

**Scope note:** the scanner now covers *every* unquoted heredoc, not only `cat >&2 <<…`
message banners. The two data heredocs feeding `$commit_rows` are in scope as a result. That
is deliberate — any unquoted heredoc can command-substitute, and drawing the boundary at
"message" heredocs would be the same narrowing this bug is about, one level up.
## Tests added

Two, both in `tests/pre-push-foreign-session-guard.sh`, replacing the single
`BODY_LINES >= 100` floor:

- `the scanner reaches every unquoted heredoc` — the coverage assertion.
- `and it detects an injected backtick in every body` — the positive control.

**Both have an OBSERVED RED**, and the reds were taken against *copies* rather than by
mutating the tree, because five sessions were live on this checkout and an armed mutation is
the hazard
`docs/issues/archive/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`
records:

```
baseline (real guard)                      2 of 2   GREEN   /   2 of 2 ticks   GREEN
append `cat >&2 <<-TAIL` (unseeable form)  2 of 3   RED     <- coverage discriminates
break the scanner's opener regex           0 of 2 ticks     RED  <- control discriminates
```

The pair matters more than either: coverage alone would pass a scanner that matched every
opener and read no bodies; the injection control alone would pass a scanner that read one
body perfectly and never saw the second.

Suite 73 → 75 assertions, 0 failed. Gate green — `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.
## Workarounds

If you add a second heredoc to `scripts/pre-push-foreign-session-guard.sh`, **delimit it
`EOF`** — the existing selector then covers it. That is a convention nobody will remember,
which is why it is a workaround and not the fix.

## Resume

N/A — fixed, with an observed red in the direction that was blind.
## References

- `tests/pre-push-foreign-session-guard.sh:632-646` — the scanner, the emptiness assertion,
  and the control.
- `scripts/pre-push-foreign-session-guard.sh:147` — the sole current opener.
- `docs/issues/2026-09-07-the-pre-push-guards-refusal-text-executes-its-own-example-commands.md`
  — the defect this scanner exists to prevent, and why the delimiter must stay unquoted.
- `.codescout/memories/test-design-discipline.md` — the positive-control prescription.
