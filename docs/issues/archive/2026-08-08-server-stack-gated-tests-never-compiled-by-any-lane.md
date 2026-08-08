---
kind: bug
status: fixed
title: 'BUG: the `server-stack` feature is in neither `default` nor any CI lane, so every test behind that gate is never compiled and cannot fail'
tags:
- testing
- ci
- feature-flags
- silent-zero-coverage
closed: 2026-08-08
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

**Implemented in `ecf3e461` (`experiments`).** Promotion is by fast-forward, so this SHA
*is* the master SHA — there is no second one to record later.

A dedicated `test-server-stack` job in
`.github/workflows/ci.yml`, running `clippy --features server-stack --all-targets
-- -D warnings` and `cargo test --features server-stack`.

**A job rather than a matrix entry, and Linux-only.** The `test` matrix is 3 OSes × 3
configs; adding a fourth config would have tripled the `qdrant-client` compile across
platforms for little marginal value. `cargo rb` is the Linux dev-box build, and the
matrix already covers default/lean everywhere. The point is that **some** lane compiles
this, not that every platform does. A dedicated job also matches how `windows-gnu`,
`msrv` and `audit-doc-refs` are already structured in this file.

**No Qdrant service container.** The tests are hermetic: `check_has_index` returns false
when the stack is unreachable, and the cache test now drives `resolve_first_probe`
directly instead of performing a live probe. That is written into the job's comment so a
future test needing a live stack adds a service container rather than making the lane
depend on ambient reachability.

The librarian seeding step is copied from the matrix job: `server-stack` sits on top of
the **default** feature set, so `librarian` is on and `build_tool_context` needs a
workspace file.

**Not implemented: the every-feature-has-a-lane guard.** Still the right idea and now
better understood — a naive "does each `[features]` key appear in a workflow" check
false-positives on `librarian`, `remote-embed` and `http`, which are never named in a
lane because they arrive via `default`. Getting it right means resolving `default`'s
transitive members, and a brittle gate that people learn to ignore is worse than no gate.
Tracked separately.
## Tests added

The lane **is** the test — this bug is about tests that never ran, so the fix is running
them.

What it caught on its first execution is the evidence that it was worth adding:
`cargo test --features server-stack` failed immediately on
`index_status_cache_serves_stale_then_refreshes`, which turned out to be a real product
defect (the activation probe enumerating the whole corpus to answer a yes/no), filed and
fixed as
`docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`.

Post-fix the lane is green: 3586 passed / 0 failed, clippy clean on the same feature set.
## Workarounds

Run `cargo test --features server-stack` by hand before touching
`src/retrieval/payload.rs`. Expect compile errors first — the gated code has drifted.

## Resume

N/A — fixed 2026-08-08.

One correction preserved from the investigation, because the file was wrong and the
next reader should trust measurement over prediction: this bug originally said to expect
compile errors from rot, specifically that `payload_roundtrip_preserves_fields` would
not build until its `ast_kind` fixture was updated. It built. `2bc0f9f0` had already
fixed that fixture as a side effect of unrelated work.

The open follow-up is the every-feature-has-a-lane guard — see § Fix for why it is
harder than it looks and was not bundled here.
## References

- `Cargo.toml:175` — `default`, which omits `server-stack`
- `Cargo.toml:198` — `server-stack = ["dep:qdrant-client"]`
- `tests/retrieval_unit.rs` — `payload_roundtrip_preserves_fields`, the gated casualty
- `src/retrieval/payload.rs` — `payload_to_map` / `map_to_payload`, gated and uncovered
