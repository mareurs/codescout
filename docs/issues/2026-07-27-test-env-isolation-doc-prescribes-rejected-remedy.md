---
kind: bug
status: fixed
tags:
- docs
- testing
- convention-drift
- root-cause
closed: null
opened: 2026-07-27
owner: marius
related:
- docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md
severity: high
---

# BUG: `docs/conventions/test-env-isolation.md` still prescribes the remedy a656f8cec220d347 marked NOT VIABLE, and cites two exemplars that no longer exist

## Summary

`docs/conventions/test-env-isolation.md` (`status: active`) tells readers that the fix for
env-racing tests is **option B — a local `EnvGuard` plus `#[serial_test::serial]`** — and
instructs them to "copy the EnvGuard pattern locally" in new modules. Bug
`a656f8cec220d347` subsequently proved that remedy unsound, recorded that `#[serial]` is
**NOT VIABLE**, and fixed the class project-wide by threading config as explicit
arguments instead (`set_var`/`remove_env` in the default `cargo test` build: 119 → 0). The
convention doc was never updated. It is now an active document that instructs engineers to
reproduce a purged bug — and it has done so at least twice in one session.

## Symptom (Effect)

Two concrete recurrences on 2026-07-27, both traceable to this doc:

1. **`src/retrieval/embedder.rs`** — the Task 4 brief of the
   `2026-07-27-index-lock-and-embedder-batching` plan specified an env-mutating `EnvGuard`
   and cited `src/librarian/indexer.rs:1074` as the pattern to mirror. Result: a
   reproducible test race, 5/5 default-parallel `cargo test` runs failing. Documented in
   `docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`.
2. **`src/librarian/indexer.rs:1070-1183`** — an uncommitted sibling change in the same
   working tree independently reintroduced the same shape (raw `set_var`/`remove_var` on
   `LIBRARIAN_ARTIFACT_VEC_MIGRATE`, plus `#[serial]` on its callers). Still open.

Two independent authors reached for the same anti-pattern in the same session because the
active convention doc told them to.

## Reproduction

Read the doc. Three statements are individually verifiable and currently wrong:

**a) It names the rejected remedy as the fix.** § "Diagnostic shape", final paragraph:

```
If you see this shape, suspect missing isolation in the test's helper
chain. The fix is option B above; document it locally and link back to
this convention from the helper's docstring.
```

Option B is `EnvGuard` + `#[serial]`. `a656f8cec220d347`'s closing note:

> "⚠️ Option 1 ('unify serialization' via `#[serial]`) was attempted and is NOT VIABLE.
> Do not retry it... `serial_test` cannot help with unannotated tests. `#[serial]`
> serializes a test only against other `#[serial]`/`#[parallel]` tests. A plain, untagged
> test runs in parallel with a `#[serial]` one — by design."

**b) It instructs readers to propagate the pattern.** § "Established exemplars":

```
When a future test module hits the same shape, copy the EnvGuard pattern
locally rather than building a shared crate.
```

**c) Both cited exemplars were deleted.** The table points at `src/librarian/mod.rs::tests`
and `src/server.rs::guide_hint_tests`. Verified 2026-07-27 — `struct EnvGuard` now exists in
exactly three places, and neither cited location is among them:

```
src/agent/mod.rs:1797          — deliberate, server-stack gated
src/librarian/indexer.rs:1074  — the uncommitted recurrence (open)
src/retrieval/embedder.rs:902  — the 2026-07-27 recurrence (being replaced)
```

## Environment

- `docs/conventions/test-env-isolation.md`, `status: active`, 110 lines, 5 sections
- Referenced from `CLAUDE.md` § Design Principles ("Test isolation:
  `docs/conventions/test-env-isolation.md`"), so it is on the documented onboarding path

## Root cause

The doc predates `a656f8cec220d347` and was not revisited when that bug landed. Its own
§ "Known gaps (open)" already anticipated the failure and listed the eventual real fix as
deferred option 2:

```
2. Move the librarian DB / workspace resolution off of process-global
   env and onto explicit arguments threaded through `Agent::new`.
   Larger refactor, removes the foot-gun at the source.
```

That deferred option **became** the shipped fix. But the doc still presents option B as
"the fix" and option 2 as a hypothetical, so the hierarchy is now exactly inverted relative
to reality: the thing that was done reads as speculative, and the thing that was rejected
reads as authoritative.

This is not a stale-link problem. A dead link fails loudly; a confidently-worded
prescription that is wrong gets followed.

## Evidence

See Reproduction — all three claims were verified directly against the live doc and a
`grep` for `struct EnvGuard` across `src/**/*.rs`, not inferred from the referenced bugs.

## Hypotheses tried

1. **Hypothesis:** the doc is merely out of date on its exemplar table (a cosmetic
   citation-drift issue).
   **Test:** read § "Diagnostic shape" and § "Established exemplars" in full.
   **Verdict:** rejected. The prescription itself is wrong, and it is imperative ("copy the
   EnvGuard pattern locally"), not descriptive. Fixing only the table would leave the
   active instruction to reproduce the bug in place.

## Fix

Fixed on `experiments`. All five prescribed steps done, each verified against the code
first rather than taken from this file:

1. **Option B demoted to NOT VIABLE**, struck through, with the mechanism spelled out:
   `#[serial]` takes a lock that non-annotated tests never ask for, so any untagged test
   touching the same var still races. The guard restores faithfully and the race happens
   anyway. Explicit instruction not to "copy the EnvGuard pattern locally".
2. **Option A promoted to *the* rule**, with the `from_env()`-at-the-edge shape named.
   § *Diagnostic shape*'s closing line, which said "the fix is option B", now says to
   resolve at the call site and warns that reaching for a guard recreates the bug.
3. **Exemplars table replaced.** Verified present before citing:
   `LibrarianEnv::from_env` (`src/librarian/mod.rs:51`), `ServerEnv::from_env`
   (`src/server.rs:67`), and `EmbedderHttp`'s `api_key` as a same-file worked example.
   Also verified *absent*: `grep` for `EnvGuard` across `src/**/*.rs` returns hits in only
   two files, `src/librarian/indexer.rs` and `src/agent/mod.rs` — neither of the two the
   old table pointed at. The table now says so outright, so a reader arriving from an old
   link learns why the thing they came to copy is gone.
4. **§ Known gaps (open) closed**, with the measured result (`set_var`/`remove_env` in the
   default `cargo test` build: 119 → 0). The gap it described is kept as the *reason the
   rule has its shape* — `#[serial]` could never coordinate across modules, and that is
   not a hole in the discipline but the discipline's ceiling.
5. **Both surviving `EnvGuard` uses named**, so neither reads as a counter-example:
   `src/agent/mod.rs` (server-stack gated, exempt) and `src/librarian/indexer.rs` (known
   debt, tracked in
   `docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md`).

One addition beyond the spec: since Rust 2024 `std::env::set_var` is `unsafe`, for exactly
this reason. The doc now says so — the compiler states out loud what option B tried to work
around, which is a stronger argument than any prose.
## Tests added

None — the change is prose in `docs/conventions/test-env-isolation.md`, with no code
surface. Gate run anyway and green (18 binaries, 3458 passed, 0 failed, clippy clean),
since the same commit carries two code fixes.

Worth noting what *would* have caught this and did not: nothing gates a convention doc
against the code it describes. `audit_doc_refs` checks that referenced paths and symbols
exist — and the old exemplars table named `src/librarian/mod.rs::tests` and
`src/server.rs::guide_hint_tests`, both of which still exist as modules. Only the
`EnvGuard` struct inside them was gone. A reference-level linter cannot see that, which is
why this doc stayed authoritative and wrong for two months.
## Workarounds

Until fixed: treat option A as the only acceptable pattern for new test helpers. When
writing a task brief that touches env-dependent tests, do **not** cite this doc or
`src/librarian/indexer.rs:1074` as precedent — cite `api_key` in
`src/retrieval/embedder.rs` instead.

## Resume

Fixed. Two follow-ons:

1. **The one real remaining instance**, `src/librarian/indexer.rs`, is unchanged and still
   tracked in
   `docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` — open by
   explicit ruling ("commit as-is, bugs track it"), not by oversight. The doc now names it
   as debt so it cannot be mistaken for a sanctioned pattern, which was this entry's actual
   risk.
2. **Master-side SHA** after cherry-pick.

This entry was filed as the *root cause* of two EnvGuard recurrences in one session. That
claim is what the fix targets: the recurrences came from an `status: active` document
telling engineers to do the thing that had been purged, and the document now tells them
not to.
## References

- `docs/conventions/test-env-isolation.md` — the doc itself
- `docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` — the
  `embedder.rs` recurrence and its fix
- `a656f8cec220d347` — the bug whose remedy the doc contradicts
- `src/retrieval/embedder.rs` — `api_key`, the in-repo worked example of option A
- `src/librarian/indexer.rs:1070-1183` — the still-open recurrence
- `src/agent/mod.rs:1797` — the one deliberate, gated `EnvGuard`
