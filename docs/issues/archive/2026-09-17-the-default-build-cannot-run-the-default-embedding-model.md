---
id: f720f7708e32d881
kind: bug
status: fixed
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

Landed. `default = ["remote-embed", "http", "librarian", "local-embed"]`, per the
2026-09-17 operator ruling. The alternative — keep the lean default and point
`default_embed_model()` at a remote model — was rejected in the same ruling because it
leaves codescout with no zero-config path at all.

**Three consequences, none predicted when the fix was chosen, each found by checking and
each a shipped break otherwise.** They are the substance of this fix; the one-word
Cargo.toml edit was the easy part.

1. **`cargo rb` inherits it.** The alias passes no `--no-default-features`, so the live
   MCP binary regains `local-embed`, undoing a documented 2026-09-15 decision to keep it
   lean (~21MB plus the ort/fastembed compile on every live rebuild). Raised with the
   operator before landing rather than absorbed silently; accepted.
   `.cargo/config.toml` now records the supersession **beside** the old reasoning rather
   than replacing it — that reasoning holds a verification (no config file on this
   machine names a `local:` model) worth not repeating.

2. **`scripts/build-windows.sh` broke, and CI runs it three times** — `ci.yml:575`
   (`build`), `:583` (`clippy`), `:778` (`test`). `ort` publishes no prebuilt for
   `x86_64-pc-windows-gnu`, which is precisely why `local-embed-dynamic` exists, so
   taking cargo's default there fails inside the build script:

   ```
   error: ort-sys@2.0.0-rc.11: ort does not provide prebuilt binaries for the
          target `x86_64-pc-windows-gnu` with feature set (no features).
   ```

   Reproduced locally with `scripts/build-windows.sh check` before landing. The script's
   default is now the windows-gnu spelling of cargo's default — the same set with
   `local-embed-dynamic` substituted — and `--edr` became a no-op alias, since it named
   a shape that is now unconditional. Re-verified with the exact CI commands, `check`
   and `clippy --all-targets -- -D warnings`.

3. **The gate went red on any machine without ONNX weights.** The default lane now
   COMPILES `crates/codescout-embed/src/local.rs`'s two weight tests, which panic unless
   `CODESCOUT_TEST_ONNX_DIR` is set. CI was already safe — it exports
   `CODESCOUT_SKIP_ONNX_TESTS=1` on every non-`local-embed` lane — but `gate.sh` was
   not, and would have redded for every session sharing this checkout.
   `gate.sh` now exports the same opt-out.

**Consequence 3 creates a new vacuity, in the dangerous polarity.** Those two tests are
the only ones that catch a correctly-shaped but silently WRONG vector (wrong tokenizer,
wrong pooling), and they now print `... ok` while asserting nothing. The repo's other two
lane-vacuities are *absences* — code a lane never compiled, which a reader can notice is
missing. This is a green line a reader would credit. Recorded in `CLAUDE.md`
§ *Development Commands* beside its siblings, because § *Observer Blindness* says a bound
living only in the enforcement layer — `gate.sh`'s own comment — is published to an
audience that never opens it.

`default_embed_model()` is now `pub`: a guard that cannot see the value cannot guard it.

- **SHA:** `87e434dd488c8b98d600c2c8c4ba457ff259ad68` (on `experiments`)
- **patch-id:** `0d32f175662fee9ba250ae632ba12797ecb939c8`

## Tests added

`the_default_feature_set_can_construct_the_default_embedding_model`
(`tests/feature_lanes.rs`). **Observed RED** before the `Cargo.toml` edit, printing the
closure it actually found:

```
the default embedding model is "local:AllMiniLML6V2Q", which needs one of
["local-embed", "local-embed-dynamic"], but the default feature closure is
{"default", "http", "librarian", "remote-embed"}.
```

**It reads the MANIFEST, and that is the design rather than convenience.** The natural
form — call `create_embedder_with_config(default_embed_model(), …)` and assert it is not
the unsupported-feature bail — must be `#[cfg(feature = "local-embed")]` to survive the
lean lane, and that cfg switches the test OFF in exactly the configuration whose breakage
it exists to catch: deleting `local-embed` from `default` would silently disable the
guard instead of redding it. Monotone under its own subject. Reading the manifest is not
cfg-dependent, so it runs, and means the same thing, in every lane — confirmed by reading
its name out of both the lean and default lanes.

Non-vacuity control is inline: the closure must contain its own seed (`default`), else
`declared_features()` parsed something that is not the `[features]` table and the guard is
standing in front of nothing; and the model string must be non-blank, else the branch
below is trivially satisfiable.

**End-to-end**, the same command before and after:

```
before: Error: … Local embedding requires the 'local-embed' feature   (0 chunks)
after:  retrieval sync finished added=1 … freshproj.db written
```

no server, no env, no `[embeddings]`.

Not regression-tested, and named rather than left implicit: consequence 2 above
(`scripts/build-windows.sh`) is covered by CI's own three windows-gnu steps rather than
by anything added here, and consequence 3 (`gate.sh`'s opt-out) has no test because the
thing it protects is the local developer gate, which CI does not run.

## Workarounds

`cargo build --features local-embed`, or configure a `url` to an
OpenAI-compatible endpoint.

## Resume

N/A — fixed and verified.

One thing deliberately left standing: the archived CyberArk EPM bug
(`docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`) had its
severity lowered `high` → `low` **because** "build with default features" was a valid
escape. That is no longer the default command — the escape still exists, spelled
`--no-default-features --features remote-embed,http,librarian`, and the file now says so.
The severity was NOT re-raised: a workaround still exists and codescout still builds on
such a host; what changed is that a developer there now meets the failure first and the
remedy second. Re-raise it if that ordering actually costs someone — that is a
prediction, and nobody has hit it yet.

## References

- `Cargo.toml` `[features]`
- `src/config/project.rs:389-391`, `:93-95`
- `crates/codescout-embed/src/lib.rs:197-326`
- `docs/manual/src/configuration/embeddings-edr-windows.md`
- `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
