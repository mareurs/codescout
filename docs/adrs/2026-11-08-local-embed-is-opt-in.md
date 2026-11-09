---
id: e43489f53e9a8dc5
kind: adr
status: active
title: ADR-2026-11-08 — local-embed is opt-in
owners:
- marius
tags:
- build-features
- embedding
- default-features
- decision-reversal
topic: build-features
---

# ADR-2026-11-08 — `local-embed` is opt-in

## Status

Accepted — active. Supersedes the 2026-09-17 ruling that put `local-embed` into `default`
(commit `87e434dd488c8b98d600c2c8c4ba457ff259ad68`, patch-id
`0d32f175662fee9ba250ae632ba12797ecb939c8`, on `experiments`; the account is
`docs/issues/archive/2026-09-17-the-default-build-cannot-run-the-default-embedding-model.md`).

The user chose the one open design question on 2026-11-08: keep `default_embed_model()` as
`local:AllMiniLML6V2Q`.

## Context

`default_embed_model()` returns `local:AllMiniLML6V2Q`. Until 2026-09-17 `default` did not
include `local-embed`, so `cargo build` produced a binary that could not construct its own
default configuration. A default build indexing a fresh project failed with "Local embedding
requires the 'local-embed' feature" and named, as its remedy, the model it had just refused.
The 2026-09-17 ruling fixed that by adding `local-embed` to `default`. It rejected the
alternative of a lean default with a remote default model, because that leaves no zero-config
path.

That ruling had a measured price, recorded in the same issue:

- `cargo rb` inherited the feature and lost its lean build.
- `scripts/build-windows.sh` broke, because `ort` publishes no prebuilt for
  `x86_64-pc-windows-gnu`. The script had to hand-list the default set with
  `local-embed-dynamic` swapped in.
- The gate went red on machines without ONNX weights. `scripts/gate.sh` then exported
  `CODESCOUT_SKIP_ONNX_TESTS=1`, and the two weight tests passed while skipped.

On 2026-11-08 the size of the feature was measured directly. A stripped release binary
(`cargo build --release -p codescout --bin codescout`, `strip = true` in `[profile.release]`)
was 42,262,960 bytes (40.3 MiB) without `local-embed` and 64,857,688 bytes (61.9 MiB) with it.
That is 21.6 MiB, or 53%, more. `cargo bloat --release --crates` attributes the largest single
share, 12.3 MiB of `.text`, to `ort_sys`. The figure agrees with the "~21MB" recorded in
`.cargo/config.toml` on 2026-09-15. (The first draft of this decision quoted `cargo bloat`
alone, which measures a different quantity: `local-embed-opt-in-session-log:F-2`.)

The documented default path is the lite stack, which embeds over HTTP
(`docs/manual/src/getting-started/installation.md`). The manual presents `local-embed` as the
option for air-gapped machines. This repository's workflows (`ci.yml`, `manual.yml`) build no
published binaries, so a user who wants the feature builds it.

## Decision

`local-embed` is **not** in `default`. `default` is `["remote-embed", "http", "librarian"]`.
To get in-process ONNX embeddings, build with `--features local-embed`, or with
`--features local-embed-dynamic` where the ONNX build script is blocked. The two are mutually
exclusive.

`default_embed_model()` stays `local:AllMiniLML6V2Q`. A default build in a project with no
`[embeddings]` config and no `CODESCOUT_EMBEDDER_URL` therefore fails at index time. The failure
must be actionable: a `RecoverableError` whose hint names both ways out, setting
`[embeddings].url` or rebuilding with `--features local-embed`.

The decision is pinned, not just stated:

- `the_onnx_backends_are_opt_in_not_default` (tests/feature_lanes.rs) reds if either ONNX
  backend enters the default feature closure. It replaces
  `the_default_feature_set_can_construct_the_default_embedding_model`, which pinned the opposite
  decision.
- `a_default_build_fails_on_the_default_model_with_both_remedies` (src/retrieval/client.rs)
  pins the failure's class and the two remedies. It is compiled only where there is no local
  backend.
- The lane guard in tests/feature_lanes.rs now skips YAML comments and reads CI matrix `flags:`
  entries, so `local-embed`'s test lane is vouched for by the matrix and not by a comment
  (`local-embed-opt-in-session-log:F-1`).

## Consequences

### Now easier

- A plain `cargo build` or `cargo install` is 21.6 MiB smaller and does not compile ONNX Runtime
  or fastembed.
- `cargo rb` is lean again, as decided on 2026-09-15.
- `scripts/build-windows.sh` is `--features local-embed-dynamic` on top of cargo's default. It no
  longer hand-copies the default list and so follows later changes to `default`.
- The default nextest lanes no longer compile the two ONNX weight tests, so `scripts/gate.sh` no
  longer exports `CODESCOUT_SKIP_ONNX_TESTS=1`. The weight tests no longer pass while skipped in
  any lane the gate runs.

### Now harder / lost

- There is no zero-config path in a default build. A project with no `[embeddings]` config and no
  URL cannot index until the operator acts. This is the failure 2026-09-17 removed. It returns on
  purpose, and it was reproduced end to end with a lean binary in a clean environment.
- With `url` set and no `model`, codescout sends the default model's bare name
  (`AllMiniLML6V2Q`) to the server. Ollama needs an exact name, so the documented Ollama route is
  `model = "ollama:nomic-embed-text"`. This was verified by capturing the request
  (`local-embed-opt-in-session-log:F-3`).
- No lane the gate runs executes the ONNX tests. The clippy lane compiles the feature, and CI's
  `local-embed` matrix entry runs the tests. `scripts/gate.sh` prints this limit at the end of
  every run.

### Revisit-when

- A zero-config path becomes a requirement again, for example if codescout ships prebuilt
  binaries. Then a prebuilt that includes `local-embed` removes the cost without changing the
  default of source builds.
- `ort` publishes a prebuilt for `x86_64-pc-windows-gnu` and the EDR blocking of the ONNX build
  script is resolved. Two of the reasons for this decision would then be gone.
- The 21.6 MiB stops mattering to the deployments that build from source.

## Alternatives considered

1. **Keep `local-embed` in `default` (the 2026-09-17 state).** Rejected: +21.6 MiB (+53%) for
   every build, a hand-maintained Windows feature list, and a gate that passed two tests while
   skipping them.
2. **Lean default plus a remote default model.** Rejected by the user on 2026-11-08, who chose to
   keep `local:AllMiniLML6V2Q`. The 2026-09-17 ruling had rejected it too: a remote default still
   needs a URL, so there is still no zero-config path.
3. **Lean default, and make the failure silent** (semantic search reports itself unavailable and
   `index` succeeds without embeddings). Not chosen: it would hide a misconfiguration that is
   cheap to fix, and it is a larger change than this decision.

## Related

- `docs/issues/archive/2026-09-17-the-default-build-cannot-run-the-default-embedding-model.md` —
  the ruling this supersedes, and the measured failure.
- `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md` — why an ONNX build
  script is a problem on EDR-managed machines.
- `local-embed-opt-in-session-log` (`docs/trackers/local-embed-opt-in-session-log.md`): F-1 (the
  lane guard), F-2 (what was measured), F-3 (the HTTP claim), F-4 (the manual), W-1 (reading the
  history first).
- Code: `Cargo.toml` (comment on `default`), `.cargo/config.toml` (`rb` alias),
  `scripts/build-windows.sh`, `scripts/gate.sh`, `tests/feature_lanes.rs`,
  `src/retrieval/client.rs` (`build_embedder`), `crates/codescout-embed/src/lib.rs`
  (`create_embedder_with_config`).
