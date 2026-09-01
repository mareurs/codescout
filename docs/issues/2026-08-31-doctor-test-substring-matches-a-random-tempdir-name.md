---
id: '08cd85e5953e2807'
kind: bug
status: fixed
title: A doctor test substring-matches a random tempdir name, so it reds CI at roughly 1-in-800
tags:
- cluster/assertion-satisfiable-by-accident
- flaky-test
- test-isolation
- ci
- doctor
- librarian
closed: 2026-08-31
opened: 2026-08-31
owner: marius
severity: medium
unverified: 'Fixed by construction rather than by observation: a backticked `QQ-N` cannot occur in a filesystem path, so the coupling is gone. But the FLAKE itself was never reproduced locally — at ~1-in-800 that would need thousands of runs, and the local suite passed every attempt including a full `cargo test --features server-stack`. Evidence is the CI triple (failed in 33404896131, passed in 33402647396 and 33393947281 either side) plus the detail-construction code. A green CI after this does NOT discriminate the fix from the flake simply not firing.'
---


# A doctor test substring-matches a random tempdir name, so it reds CI at random

## Symptom

`librarian::tools::doctor::tests::cited_prefix_reports_only_the_active_projects_citers`
failed on the `server-stack` CI lane in run `33404896131`, and passed in the runs either
side (`33402647396`, `33393947281`). `cargo test --features server-stack` passes locally.
4790 passed / 1 failed.

## Mechanism

The test asserts that the reported finding is the in-project prefix `ZZ` and not the
sibling-project `QQ`:

```rust
v[0].detail.contains("`ZZ-N`") && !v[0].detail.contains("QQ")
```

The first half is token-shaped; the second is a **bare substring**. And
`scan_cited_prefix_with_no_definer` builds its detail by interpolating the absolute paths
of the citing files:

```
… so this state is reported nowhere else. Citing files: {}. …
```

In this test those paths live under a `tempfile::tempdir()`, whose name carries ~6 random
alphanumerics. So `!detail.contains("QQ")` is not asking about the finding at all — it is
asking whether the fixture's own scratch directory happened to be named with a `Q` next to
a `Q`. Roughly 1-in-800 per run.

The panic output is what makes this diagnosable rather than mysterious: it prints
`v[0].detail`, and that detail **correctly begins** `` `ZZ-N` is cited 3 times across 2
files `` — the finding was right, and the assertion still failed. An assertion whose
message shows the expected value is the tell.

## Why the frequency is the interesting part

At 1-in-800 it is common enough to red a branch every few hundred pushes and rare enough
that the first hypothesis is always something else. This one arrived inside a 5-day CI
red with nine other failing jobs, so it read as part of a systemic break; it is unrelated
to all of them. A test that fails at a rate well below "every time" and well above "never"
costs more attention per occurrence than a deterministic one.

## Fix

Compare the backticked token `` `QQ-N` `` rather than the bare letters. A backticked token
cannot occur in a path, so the assertion depends only on what the check reports. The
comment at the assertion states this and names the CI run, because the next person to
tidy `` `QQ-N` `` back to `QQ` would reintroduce it with no test able to object.

## Fix provenance

- **SHA:** `5816c8eb` (`experiments`)
- **patch-id:** `b1a93d45ecae0c03ed2a068949966f41435d1807`

Carried inside a three-failure CI fix rather than shipped alone, so the commit subject does
not name this bug — the relevant hunk is the backticked `` `QQ-N` `` comparison in
`src/librarian/tools/doctor.rs`. Fixed by construction, never reproduced locally; see
`unverified:` before reading a green CI as discriminating.
## The general form

This is not "a flaky test"; it is an **assertion whose input contains environment-controlled
text**. Any `!haystack.contains(needle)` where the haystack embeds a path, a hostname, a
timestamp or a temp name has the same defect, and it always fails *open* — it passes on
almost every machine and every run, which is what keeps it alive.

Measured 2026-08-31 while filing this: `!\w+(\.\w+\(\))*\.contains\(` matches **466 times
across 99 files** under `src/`. That is not a worklist — the overwhelming majority are
ordinary production logic, and the defect needs the *haystack* to embed environment text,
which the regex cannot see. Recorded so the next person knows the bare grep does not
narrow it and does not repeat the measurement.

## Resume

Sweep for sibling instances: negative `contains` assertions whose receiver is a message
built with `{}` over a `Path`. Do it as its own pass — the 466-hit grep above is the
starting population, not the finding set.
