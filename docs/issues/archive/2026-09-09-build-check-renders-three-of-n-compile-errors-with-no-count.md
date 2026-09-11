---
id: 2abd5aae844f4ee2
kind: bug
status: fixed
title: 'BUG: build_check renders three of N compile errors and never says N, so a truncated diagnostic list reads as the whole one'
tags:
- cluster/capped-result-presented-as-complete
claimed_at: 2026-09-11
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
---

## Summary

`build_check`'s notice renders at most three failing compile diagnostics and never says how many there were. An author with twelve errors is shown three, in a message that reads as the complete list. Found by `every_cap_constant_is_classified` when branch `result-cap-marker-gate` was rebased onto `experiments` — the constant arrived unclassified, and reading its use site to classify it truthfully is what surfaced the missing marker.

## Symptom (Effect)

`errors_naming` stops collecting at three and returns the joined lines with no total:

```rust
// src/agent/build_check.rs:287-292
        if hits.len() >= MAX_RENDERED {
            break;
        }
    }

    (!hits.is_empty()).then(|| hits.join("\n"))
```

`render_notice` wraps that string and adds no count either (`src/agent/build_check.rs:302-308`), so the emitted notice is:

```
[codescout] your uncommitted edit does not compile, and this checkout is shared with other live sessions.
  src/mine.rs:1  boom
  src/mine.rs:2  boom
  src/mine.rs:3  boom
This is a statement about your own working tree, not a request — peers' gates see this break too.
```

Nothing in that output distinguishes "there were exactly three errors" from "there were three hundred".

## Reproduction

Not driven at runtime — **inferred from source, not measured**, and the distinction matters because the notice path needs a shared checkout with a live peer to fire. The mechanism is fully visible statically:

1. `git rev-parse HEAD` — branch `result-cap-marker-gate`, rebased onto `experiments` 2026-09-09.
2. Read `src/agent/build_check.rs:81` (`const MAX_RENDERED: usize = 3;`), `:287` (the `break`), `:292` (the `join`), `:302-308` (`render_notice`).
3. The existing unit test already constructs the over-cap case: `at_most_three_errors_are_rendered` (`src/agent/build_check.rs:699-706`) feeds ten errors and asserts `got.lines().count() == MAX_RENDERED`.

That test is the tell. It asserts **the bound holding**, never **a disclosure arriving** — so the suite is green on exactly the behaviour filed here.

## Environment

Linux, Rust 2021, codescout v0.15.0, branch `result-cap-marker-gate` post-rebase onto `experiments` (483 commits), 2026-09-09.

## Root cause

`MAX_RENDERED` bounds the rendered set inside `errors_naming`, and the truncation is discarded at the same statement that applies it: `break` leaves the loop, `hits.join("\n")` serialises only what survived, and the count of what did not is never computed, so there is no value for `render_notice` to carry even if it wanted to. The disclosure would have to be a separate field — which is precisely the generalisation `IC-13` already records from `index_state.skipped_sample`: *a marker that names the truncated result is not a marker at all.*

Inferred from `src/agent/build_check.rs:281-308` — read 2026-09-09, not observed at runtime.

## Evidence

### The cap and its use site

```rust
// src/agent/build_check.rs:79-81
/// How many failing diagnostics to render. A wall of errors is not more actionable than
/// three, and this notice rides on an unrelated tool's response.
const MAX_RENDERED: usize = 3;
```

The doc comment justifies the *bound* and is silent on disclosure. The bound is defensible on its own terms — a wall of errors genuinely is not more actionable. Truncating is not the defect; truncating silently is.

### The test that passes over it

```rust
// src/agent/build_check.rs:699-706
fn at_most_three_errors_are_rendered() {
    let json = (0..10) ... ;
    let got = errors_naming(&edited(&["src/mine.rs"]), &json, &root()).expect("reported");
    assert_eq!(got.lines().count(), MAX_RENDERED, "{got}");
}
```

Ten errors in, three lines out, asserted equal to the cap. A mutation that deleted a disclosure would not red this test, because there is no disclosure in the assertion's reach.

## Hypotheses tried

1. **Hypothesis:** the total is carried somewhere else in the notice envelope.
   **Test:** read `render_notice` (`src/agent/build_check.rs:302-308`) in full; grep `MAX_RENDERED` across `src/**/*.rs` (3 hits: the declaration, the `break`, the test).
   **Verdict:** rejected — the notice is a single `format!` over the joined string, with no count parameter.

2. **Hypothesis:** it is `NOT_A_CAP` because the notice is advisory rather than a tool result.
   **Test:** weighed against `IC-13`'s clause, which is about a caller receiving a partial result with no marker, not about the transport.
   **Verdict:** rejected — the author is the caller here, and the notice is the only surface they read. Classified `RESULT_CAP agent_build_check.rendered_diagnostics`.

## Fix

Fixed. `errors_naming` (`src/agent/build_check.rs`) no longer breaks the outer scan once `hits.len()` reaches `MAX_RENDERED` — it keeps counting every matching diagnostic into a `total` while only rendering the first three, so `total > hits.len()` is now a real, computed condition rather than one that can never be observed (the old code's `hits.len()` was, by construction, never more than the cap it would have been compared to). When capped, the returned string gets a distinct trailing line — `"  … showing 3 of 12"` — appended after the rendered hits, not folded into a fourth hit-shaped line. `render_notice` needed no change: it already wraps the string verbatim.

`cap_probe.rs`'s `agent_build_check.rendered_diagnostics` row is updated from `Coverage::Deferred` to `Coverage::Probed`, citing `at_most_three_errors_are_rendered`.

**Mutation-verified**: `at_most_three_errors_are_rendered` (the exact test this bug named as blind — "asserts the bound holding, never a disclosure arriving") was strengthened first and observed RED against the pre-fix code, then GREEN after. A new control test, `under_the_cap_no_disclosure_is_added`, confirms the marker is conditional (2 errors, under the cap of 3, produces no "showing" line) so it cannot be glued on unconditionally.

**Citation, and why it needs an explanation this time:** this fix landed *inside* `2e2d6971ca8e1260927ca4a01dc8938a4560968b`, a commit authored by a concurrent peer session archiving an unrelated bug (`docs(issues): archive the dashboard-memory bug, file the feature-lane class`). My two files were staged in the shared index (this checkout has one `.git/index` for every session working it) when the peer committed; `git diff HEAD -- src/agent/build_check.rs src/tools/core/cap_probe.rs` reads empty afterward, confirming nothing was lost — the content is safely in history, just not under a commit message that names it. This is a live recurrence of `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (id `e421be689a23ae2a`), recorded there as its own instance rather than re-derived here.

Because the commit is entangled, `git show 2e2d6971 | git patch-id --stable` would hash the peer's docs changes too and is not a citable identifier for *this* fix alone. Scoped instead:

```
git diff 2e2d6971^ 2e2d6971 -- src/agent/build_check.rs src/tools/core/cap_probe.rs | git patch-id --stable
```

**SHA (entangled, contains this fix plus unrelated peer content):** `2e2d6971ca8e1260927ca4a01dc8938a4560968b`
**patch-id (scoped to the two files this fix touched):** `0b0e3a325d30dac0821b3dd6ec924d8f37c900ac`
## Tests added

`src/agent/build_check.rs` test module:

- `at_most_three_errors_are_rendered` (existing test, strengthened) — ten errors in, asserts three hit lines AND a `"showing 3 of 10"` line. This is the exact test the bug's own Evidence section quoted as blind to the defect ("asserts the bound holding... a mutation that deleted a disclosure would not red this test"); it now does.
- `under_the_cap_no_disclosure_is_added` (new) — two errors, under the cap: no `"showing"` line. Over-match guard proving the marker is conditional on something actually being withheld.

Both observed RED against pre-fix `errors_naming`, GREEN after.
## Workarounds

Run `cargo check --all-targets` directly — the author's own build reports every diagnostic. This is why severity is **low** rather than medium: the truncated surface is advisory and a full-fidelity source is one command away, unlike the tool-output members of this cluster where the capped response is the only view the caller gets.

## Resume

Done — see § Fix. Nothing left to resume.
## References

- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` — the class, and the `index_state.skipped_sample` precedent that a marker naming the truncated payload is not a marker.
- `src/tools/core/cap_probe.rs` — the probe row and its deferral reason.
- `tests/result_caps.rs` — `every_cap_constant_is_classified`, the gate that surfaced this.
