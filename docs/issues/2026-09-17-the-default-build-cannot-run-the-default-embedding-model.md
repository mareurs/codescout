---
id: c09210c5bcd74208
kind: bug
status: taken
title: 'BUG: the default build cannot run the default embedding model'
tags:
- cluster/repro-env-diverges-from-gate-env
- embeddings
- cargo-features
- onboarding
claimed_at: 2026-09-17
claimed_by: 458a8a26-c380-4f5b-b967-2181f592917e
---

## Summary

`default = ["remote-embed", "http", "librarian"]` does not include
`local-embed`, but the default embedding model is `local:AllMiniLML6V2Q`. A
binary built with `cargo build` therefore cannot run its own default
configuration: the `local:` arm is `#[cfg]`-gated out and the model falls
through to a rebuild-instructions bail. No developer in this repo sees it,
because the gate's clippy lane passes `--features local-embed` and every `.env`
in the tree configures a remote endpoint instead.

## Symptom (Effect)

On a default-feature build, with no `url` configured anywhere:

```
Error: could not build the 'local:AllMiniLML6V2Q' embedder:
Local embedding requires the 'local-embed' feature.
Rebuild with: cargo build --features local-embed

Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)
Offline hosts: local-dir:/path/to/weights (no network at all)
```

The message recommends the model that just failed. It is accurate — the
recommendation is right *once you have rebuilt* — but read in sequence it names
the failing value as the remedy.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`.

```bash
cargo build --release                       # default features
# fresh project, no [embeddings], no CODESCOUT_EMBEDDER_URL, empty CODESCOUT_ENV_FILE
./target/release/codescout index --project /tmp/fresh
```

Not reproduced end-to-end in this session — the local release binary is built by
`cargo rb`, which carries a wider feature set. The mechanism is read from the
cfg gates and the feature list, and is stated here as such.

## Environment

Any default-feature build. `Cargo.toml` `[features]`, tree `35b622f8`.

## Root cause

**This is now measured rather than inferred.** The earlier version of this section said
the mechanism was read from the cfg gates and not observed; it has since been reproduced.
The debug binary built by `cargo build` (default features, no flags) indexing a fresh
project with no `[embeddings]`, no ambient `CODESCOUT_*` and an empty
`CODESCOUT_ENV_FILE`:

```
Error: could not build the 'local:AllMiniLML6V2Q' embedder: Local embedding requires
the 'local-embed' feature.
Rebuild with: cargo build --features local-embed

Recommended: local:AllMiniLML6V2Q (384d, quantized, 22MB)
```

The message recommends, as the remedy, the model that just failed — accurate only if
read as "after you rebuild", which is not how it reads in sequence.

Mechanism as filed: `Cargo.toml`'s `default` omitted `local-embed`;
`default_embed_model()` (`src/config/project.rs`) returns `local:AllMiniLML6V2Q`; the
`local:` arm of `create_embedder_with_config`
(`crates/codescout-embed/src/lib.rs`) is `#[cfg(any(feature = "local-embed", feature =
"local-embed-dynamic"))]`, so resolution falls through to the rebuild-instructions bail.

**Why nobody here observed it** stands as filed, and is `IC-5` exactly: the gate's clippy
lane passes `--features local-embed` while neither test lane does — the arm is compiled
in the lane that does not run tests and absent from the two that do — and every `.env*`
in the tree sets `CODESCOUT_EMBEDDER_URL`, so no local configuration exercised the urlless
path at all.

## Evidence

### The feature list and the default model, side by side

```
Cargo.toml:2                 default = ["remote-embed", "http", "librarian"]
src/config/project.rs:389-391 fn default_embed_model() -> String {
                                 "local:AllMiniLML6V2Q".into() }
crates/codescout-embed/src/lib.rs  #[cfg(any(feature = "local-embed", ...))]
                                   if let Some(m) = model.strip_prefix("local:")
```

### The reachable-alarm question

Per `CLAUDE.md` § Testing Discipline — *"loudness is a property of a PATH"* — the
bail is well-written and reached by a caller who does not exist in this repo:
every in-tree configuration sets a url. The observer who would act on it is a
first-time user on a default build, and no lane in the gate stands in for them.

## Hypotheses tried

1. **Hypothesis:** `local-embed` is omitted from defaults because ONNX Runtime
   is a heavy or platform-fragile dependency.
   **Test:** read the feature comments and the EDR/Windows page.
   **Verdict:** partially confirmed — `docs/manual/src/configuration/embeddings-edr-windows.md`
   and `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
   document real constraints, and `local-embed-dynamic` exists for them. So the
   omission has a reason; what has no reason is the *default model* naming a
   backend the default build cannot construct. Those two decisions were made
   independently and never reconciled.

## Fix

Operator ruling 2026-09-17: **add `local-embed` to `default`**, so the
out-of-the-box configuration works on the machine profile most users have (no
server, no docker-compose, no GPU). Verify against the EDR/Windows constraint
before landing — `local-embed-dynamic` must remain the escape hatch there, and
the Windows CI lane must stay green.

Alternative rejected in the same ruling: keeping the lean default and changing
the default model to a remote one, which would leave codescout with no
zero-config path at all.

Whichever lands, the *reconciliation* is the real fix: the default model and the
default feature set must be checked against each other by a test, not by two
people remembering. A guard asserting `default_embed_model()` is constructible
under `default` features would have caught this on the commit that introduced
the divergence.

SHA / patch-id: pending.

## Tests added

None yet. Owed, and its shape is the point: a test in the **default** lane that
calls `create_embedder_with_config(default_embed_model(), None, None)` and
asserts it does not return the unsupported-feature bail. That test is vacuous in
the clippy lane and meaningful in the default one — the opposite polarity from
the librarian case in `CLAUDE.md`, and worth the annotation on the fixture.

## Workarounds

`cargo build --features local-embed`, or configure a `url` to an
OpenAI-compatible endpoint.

## Resume

Add `local-embed` to `default` in `Cargo.toml`, then run the full four-command
gate plus the Windows CI lane. Check `docs/manual/src/getting-started/installation.md`
and `CONTRIBUTING.md` for build lines that would become redundant or wrong.

## References

- `Cargo.toml` `[features]`
- `src/config/project.rs:389-391`, `:93-95`
- `crates/codescout-embed/src/lib.rs:197-326`
- `docs/manual/src/configuration/embeddings-edr-windows.md`
- `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
