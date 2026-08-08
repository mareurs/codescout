---
kind: bug
status: open
title: 'BUG: the `server-stack` feature is in neither `default` nor any CI lane, so every test behind that gate is never compiled and cannot fail'
tags:
- testing
- ci
- feature-flags
- silent-zero-coverage
closed: null
opened: 2026-08-08
owner: marius
related:
- docs/issues/2026-08-08-metadata-header-computed-but-never-embedded-or-stored.md
severity: high
---

# BUG: `server-stack`-gated tests are compiled by no lane

## Summary

`Cargo.toml:198` defines `server-stack = ["dep:qdrant-client"]`. It is not in `default`
(`:175`) and appears in no workflow under `.github/workflows/`. Every
`#[cfg(feature = "server-stack")]` test is therefore never built, never run, and cannot
fail.

**And it is the configuration that ships.** `.cargo/config.toml` defines
`rb = "build --release --features server-stack"` — `cargo rb` is the command CLAUDE.md
mandates for the live MCP release binary, and it turns the feature **on**. So the build
that actually runs is the one build CI never compiles, and the build CI does compile is
not the one anyone runs.

That is not hypothetical: `cargo test --features server-stack` **fails today** on
`tools::config::tests::index_status_cache_serves_stale_then_refreshes`, and the failure
is a real product defect, filed separately as
`docs/issues/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`.
## Symptom (Effect)

`cargo test --test retrieval_unit` runs 7 tests. `payload_roundtrip_preserves_fields`
is defined in that file and is not among them:

```
running 7 tests
test config_from_env_reads_overrides ... ok
test diff_added_chunk_yields_upsert ... ok
test config_from_env_uses_defaults_when_unset ... ok
test diff_deleted_chunk_yields_delete ... ok
test diff_identical_yields_noop ... ok
test embed_text_prepends_the_ast_header_and_omits_it_when_absent ... ok
test diff_modified_chunk_yields_upsert_for_new_id ... ok
```

## Reproduction

```
1. cargo test --test retrieval_unit          # payload_roundtrip_preserves_fields absent
2. grep -n 'server-stack' Cargo.toml         # :198 defines it; :175 default does not include it
3. grep -rn 'server-stack' .github/workflows # no matches
```

Commit: `cb96aa47` (experiments).

## Environment

codescout `experiments`, all lanes. Not host-specific — the gate is in the manifest.

## Root cause

A feature flag with no lane. Nothing in cargo or CI reports "these tests were never
built"; a filtered-out test is indistinguishable from a test that does not exist.
This is worse than an absent test, because the file *looks* covered: a reader greps
for `payload_to_map`, finds `payload_roundtrip_preserves_fields`, and stops.

*measured 2026-08-08: the three commands in § Reproduction. Not inferred.*

## Evidence

`Cargo.toml`:

```toml
default = ["remote-embed", "http", "librarian"]
...
server-stack = ["dep:qdrant-client"]
```

The project's stated gate (from `45669701`'s commit message) is: `fmt`, `clippy
--all-targets -D warnings`, `cargo test`, `check --no-default-features --all-targets`,
`test --no-default-features`, `test --features local-embed --no-default-features`.
No lane names `server-stack`.

**Corroborating detail.** `payload_roundtrip_preserves_fields` asserted 4 of its
struct's 11 fields while its name claimed all of them, and it constructed
`ast_kind: "fn"` — a field that in production was always `""`. A test that never runs
also never gets corrected: nothing forces its fixture to stay honest about the values
production actually produces.

## Hypotheses tried

1. **Hypothesis:** the tests run under some lane not named in the commit-message gate.
   **Test:** `grep -rn 'server-stack' .github/workflows/`.
   **Verdict:** rejected — no matches.

## Fix

Not yet implemented.

1. **Add `cargo test --features server-stack` to CI.** This is no longer the expensive
   option to weigh against a cheap one — it is the only lane that tests the shipped
   binary. It compiles cleanly today (measured, 40 s including `qdrant-client`), so the
   cost is a build, not a repair.
2. **Reconsider which feature set `default` names.** If `cargo rb` is the real build,
   `server-stack` being absent from `default` means "the tested configuration" and "the
   shipped configuration" are named differently on purpose and drift for free. Either
   lane it, or make the shipped set the default and let the lean build be the opt-out.

Also worth a general guard, unchanged from the original filing: a CI step asserting every
feature declared in `[features]` appears in at least one lane. The defect is not "someone
forgot Qdrant" — it is that forgetting is unobservable.
## Tests added

None — this file is about tests that do not run. The fix's own verification is that
`payload_roundtrip_preserves_fields` compiles and passes under the new lane, which it
currently would not (stale `ast_kind` in its fixture).

## Workarounds

Run `cargo test --features server-stack` by hand before touching
`src/retrieval/payload.rs`. Expect compile errors first — the gated code has drifted.

## Resume

Done: `cargo check --features server-stack --all-targets` (clean, 40 s) and
`cargo test --features server-stack` (3455 passed, **1 failed**, 11 ignored).

**Correction — the original Resume's prediction was wrong.** It said to expect compile
errors because "the gated code has drifted... `payload_roundtrip_preserves_fields` will
not even build until its fixture is updated." It builds. The `ast_kind` removal in
`2bc0f9f0` had already updated that fixture as a side effect of unrelated work, so the
rot the file predicted had been repaired before anyone looked for it. Recorded rather
than quietly deleted: the prediction was reasonable and still false, and the next person
should trust the measurement over the file.

Next action: add the lane (§ Fix candidate 1). The one failing test is a genuine defect
with its own bug file, not a blocker for the lane — land the lane and let it stay red
until that fix goes in, or land them together.
## References

- `Cargo.toml:175` — `default`, which omits `server-stack`
- `Cargo.toml:198` — `server-stack = ["dep:qdrant-client"]`
- `tests/retrieval_unit.rs` — `payload_roundtrip_preserves_fields`, the gated casualty
- `src/retrieval/payload.rs` — `payload_to_map` / `map_to_payload`, gated and uncovered
