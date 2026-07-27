---
status: open
opened: 2026-07-27
closed:
severity: high
owner: marius
related:
  - docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md
tags: [docs, testing, convention-drift, root-cause]
kind: bug
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

Rewrite the doc so its recommendation matches what the codebase actually does:

1. **Demote option B.** Mark `EnvGuard` + `#[serial]` explicitly NOT VIABLE, with the
   reason (`#[serial]` coordinates only among annotated tests; any untagged test reading
   env still races) and a pointer to `a656f8cec220d347`.
2. **Promote option A** (explicit arguments / injected value) to *the* recommendation, and
   replace § "Diagnostic shape"'s closing line accordingly.
3. **Replace the exemplars table** with live ones. Candidates verified present:
   `LibrarianEnv::from_env` / `ServerEnv::from_env` (the from_env-wrapper shape that
   `a656f8cec220d347` introduced), and — as a same-file worked example —
   `EmbedderHttp`'s `api_key` (`src/retrieval/embedder.rs`), which reads `EMBED_API_KEY` in
   `new()`, stores `Option<String>`, and is injectable. The Task 4 fix round converts
   `batch_override` to that same shape and will be a second example in the same file.
4. **Close out § "Known gaps (open)"** — its deferred option 2 shipped. Say so and link the
   commit, so the section stops reading as an open question.
5. **Note the one legitimate remaining `EnvGuard`** (`src/agent/mod.rs:1797`, server-stack
   gated) and why it is exempt, so the next reader does not treat it as a counter-example
   to the new rule.

## Tests added

None — documentation. A cheap guard worth considering: extend the existing
`audit_doc_refs` lint, or add a test asserting that `docs/conventions/*.md` contains no
`path::to::symbol` reference that no longer resolves. That would have caught claim (c),
though not (a) or (b).

## Workarounds

Until fixed: treat option A as the only acceptable pattern for new test helpers. When
writing a task brief that touches env-dependent tests, do **not** cite this doc or
`src/librarian/indexer.rs:1074` as precedent — cite `api_key` in
`src/retrieval/embedder.rs` instead.

## Resume

Rewrite the five items under "Fix". While in there, verify whether `CLAUDE.md`'s pointer to
this doc needs any accompanying wording change. Then fix the still-open
`src/librarian/indexer.rs:1070-1183` recurrence, which is currently uncommitted work in the
tree and would otherwise ship the anti-pattern the rewritten doc forbids.

## References

- `docs/conventions/test-env-isolation.md` — the doc itself
- `docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md` — the
  `embedder.rs` recurrence and its fix
- `a656f8cec220d347` — the bug whose remedy the doc contradicts
- `src/retrieval/embedder.rs` — `api_key`, the in-repo worked example of option A
- `src/librarian/indexer.rs:1070-1183` — the still-open recurrence
- `src/agent/mod.rs:1797` — the one deliberate, gated `EnvGuard`
