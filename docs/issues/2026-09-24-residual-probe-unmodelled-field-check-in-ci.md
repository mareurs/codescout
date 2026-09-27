---
id: a11cf095b81c1c5d
kind: bug
status: fixed
title: 'RESIDUAL: Run the probe''s unmodelled-wire-field check in the test lane/CI'
tags:
- cluster/selector-narrower-than-its-population
claimed_at: 2026-09-27
claimed_by: 48d1f0c8-9f60-43bb-a15e-17ec7995813a
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Run the probe's unmodelled-wire-field check in the test lane/CI.

## Verification, and a control that was wrong first

The seed is the parent bug's own original failure: drop `"annotations"` from `MODELLED_WIRE_KEYS` and the probe must report a wire field it cannot size. Run on copies outside the repo so the shared tree was never mutated.

| script | seed | exit | alarm printed | stderr |
|---|---|---:|---|---|
| post-fix | applied | **1** | yes | empty |
| pre-fix | applied | **0** | yes | empty |
| post-fix (HEAD, unseeded) | none | 0 | no | empty |

**The middle row is the defect, stated exactly:** the pre-fix script *printed the alarm* and *exited 0*. Both rows print it; only one can fail a build. That is `CLAUDE.md` § *Testing Discipline*'s *loudness is a property of a PATH* with the path present and the last hop missing.

**The first attempt at that control was WRONG and is recorded rather than quietly redone.** Both seeded copies were first run without `--binary`, and both exited 1 — which read as *"the check fires in both"*, and would have been reported as corroboration. It was a `FileNotFoundError`: `ROOT` derives from the script's own `__file__`, so a copy in a scratch directory looks for `target/debug/codescout` beside itself. **Exit 1 is what an uncaught Python exception returns and also what the working check returns**, so the number agreed with a conclusion for an unrelated reason. The discriminator that settled it was reading **stderr**, which the empty-bytes column above now carries on every row for exactly that purpose — a run that crashes and a run that detects are distinguishable only there.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md` (status `fixed`):

> no CI-level guard: the unmodelled-field alarm fires only on a probe run, never in the test lane

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

**Done 2026-09-27 — and the check was worse than unwired, which is why this took a code change and not a yaml line.**

The parent's caveat (*"no CI-level guard: the unmodelled-field alarm fires only on a probe run, never in the test lane"*) was **still accurate at HEAD** — checked rather than assumed, per this file's own instruction. But it understated the defect. `scripts/probe_tool_surface.py`'s `main()` **returned 0 whatever `unmodelled` contained**: the `--json` branch did a bare `return` and the text path fell off the end. The report was loud to a human reading stdout and mute to anything reading an exit code. **Wiring it into CI as-is would have produced a green lane that could never fail** — an alarm with a path to it and no way to signal.

So the fix is two parts: `main()` now `sys.exit(1)` when `unmodelled` is non-empty, in **both** output modes, and a `tool-surface-probe-check` job runs it. Its own job, not a line in `shell-tests`, per the ruling already in `ci.yml`: that lane is ~25% flaky and a new red there is attributed to whoever pushed last. Appended at file end so no existing `ci.yml:<line>` citation shifts. Job count 20 → 21, confirmed by `yaml.safe_load`.

## Fix provenance

- **SHA:** `a99112f9` (experiments-only) — positional; dies on a rebase of `experiments`.
- **patch-id:** `40d1ed99e0538f117df69ba7fef383b1a1c10111` — content hash of the diff; survives rebase and cherry-pick. Derived through a file, never a pipe from `git show`.

## References

- `docs/issues/archive/2026-09-03-probe-enumerates-wire-fields-so-a-new-one-counts-as-zero.md` — parent
