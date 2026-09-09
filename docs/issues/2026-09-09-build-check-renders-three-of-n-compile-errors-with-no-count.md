---
id: '637ad14bbbc663da'
kind: bug
status: open
title: 'BUG: build_check renders three of N compile errors and never says N, so a truncated diagnostic list reads as the whole one'
tags:
- cluster/capped-result-presented-as-complete
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

Not fixed. The change is small and has two halves that must land together, per `IC-13`'s own reading: count the suppressed diagnostics, and emit the count as a field distinct from the rendered text (e.g. `showing 3 of 12`), so a mutation deleting the disclosure has something to delete.

Deliberately **not** fixed in the same pass that found it: this branch's scope is the gate, and a fix whose wording is the load-bearing part deserves its own review rather than riding a rebase.

## Tests added

None yet — the probe row records the gap instead, as `Coverage::Deferred` in `src/tools/core/cap_probe.rs` under id `agent_build_check.rendered_diagnostics`, with the reason stating that no marker exists to assert. When the fix lands, that row becomes `Probed` with a cited test and the mutation can be run.

Tuning the row until it passed was available and refused: it would have converted a finding into coverage.

## Workarounds

Run `cargo check --all-targets` directly — the author's own build reports every diagnostic. This is why severity is **low** rather than medium: the truncated surface is advisory and a full-fidelity source is one command away, unlike the tool-output members of this cluster where the capped response is the only view the caller gets.

## Resume

Add a suppressed-count to `errors_naming` (`src/agent/build_check.rs:281-292`): return the total alongside the rendered lines rather than `Option<String>`, and have `render_notice` (`:302-308`) emit `showing N of M` when `M > N`. Then flip the `agent_build_check.rendered_diagnostics` row in `src/tools/core/cap_probe.rs` to `Coverage::Probed`, cite the new test, and drive the mutation (delete the count emission, observe the cited test red, revert).

## References

- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` — the class, and the `index_state.skipped_sample` precedent that a marker naming the truncated payload is not a marker.
- `src/tools/core/cap_probe.rs` — the probe row and its deferral reason.
- `tests/result_caps.rs` — `every_cap_constant_is_classified`, the gate that surfaced this.
