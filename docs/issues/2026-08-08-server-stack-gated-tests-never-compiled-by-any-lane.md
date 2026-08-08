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
severity: medium
---

# BUG: `server-stack`-gated tests are compiled by no lane

## Summary

`Cargo.toml:198` defines `server-stack = ["dep:qdrant-client"]`. It is not in
`default` (`:175` — `remote-embed`, `http`, `librarian`) and appears in no workflow
under `.github/workflows/`. Every `#[cfg(feature = "server-stack")]` test is therefore
never built, never run, and cannot fail — in CI or locally.

The Qdrant payload serialization layer is the main casualty: `payload_to_map` and
`map_to_payload` are both gated, and the only test covering them is gated with them.

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

Not yet implemented. Two candidates:

1. **Add a lane.** `cargo test --features server-stack` in CI. Costs a
   `qdrant-client` compile; buys real coverage of the payload round-trip. Check
   first whether the gated tests still pass — they have not been compiled in a long
   time, and `ast_kind` was removed from `CodePayload` in `cb96aa47`, so
   `payload_roundtrip_preserves_fields` needs its fixture updated before it will
   even build.
2. **Add a compile-only lane.** `cargo check --features server-stack --all-targets`
   is much cheaper and catches bit-rot (the class that has already happened) without
   running anything.

Candidate 2 is the floor; candidate 1 is what actually tests the layer.

Also worth a general guard: a CI step asserting every feature declared in
`[features]` appears in at least one lane. That generalizes past this instance —
the defect is not "someone forgot Qdrant", it is that forgetting is unobservable.

## Tests added

None — this file is about tests that do not run. The fix's own verification is that
`payload_roundtrip_preserves_fields` compiles and passes under the new lane, which it
currently would not (stale `ast_kind` in its fixture).

## Workarounds

Run `cargo test --features server-stack` by hand before touching
`src/retrieval/payload.rs`. Expect compile errors first — the gated code has drifted.

## Resume

Run `cargo check --features server-stack --all-targets` and read the errors. That
output is the real scope of this bug: everything it lists has been un-compiled long
enough to rot. Fix those, then decide between candidate 1 and 2 with the compile cost
in hand rather than estimated.

## References

- `Cargo.toml:175` — `default`, which omits `server-stack`
- `Cargo.toml:198` — `server-stack = ["dep:qdrant-client"]`
- `tests/retrieval_unit.rs` — `payload_roundtrip_preserves_fields`, the gated casualty
- `src/retrieval/payload.rs` — `payload_to_map` / `map_to_payload`, gated and uncovered
