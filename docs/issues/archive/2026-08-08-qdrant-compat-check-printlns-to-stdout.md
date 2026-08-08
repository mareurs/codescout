---
status: fixed
opened: 2026-08-08
closed: 2026-08-08
severity: high
owner: marius
related: []
tags: [server-stack, qdrant, cli, json, stdout-hygiene]
kind: bug
---

# BUG: qdrant-client's compatibility check `println!`s into the CLI's `--json` stdout

## Summary

In the shipped (`server-stack`) build, every `codescout ... --json` command that
builds the librarian tool context can emit a `qdrant-client` diagnostic on
**stdout**, ahead of the JSON envelope, whenever Qdrant is unreachable. The
envelope is intact but no longer parses, so any machine consumer of the CLI
breaks. `cargo rb` — the build we actually ship — is the affected one; the
default feature set never compiles the code path.

## Symptom (Effect)

CI run `31271255156` on `ffe4a285`, lane **`Test (server-stack — the build cargo
rb ships)`**: 3 of 11 `cli_artifact` tests fail. Verbatim:

```
thread 'artifact_create_then_get_round_trip' (30188) panicked at tests/cli_artifact.rs:126:29:
create stdout is not JSON: Failed to obtain server version. Unable to check client-server compatibility. Set check_compatibility=false to skip version check.
{
  "id": "ce7166a2d1ecae25",
  "abs_path": "/tmp/.tmpECc4mn/project/docs/test-spec.md"
}

error: expected value at line 1 column 1
```

```
failures:
    artifact_create_then_get_round_trip
    artifact_link_then_graph_shows_edge
    artifact_update_status_archived_then_find_excludes

test result: FAILED. 8 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
```

Two properties matter. The **operation succeeded** — the JSON payload is correct
and complete, and the process exit code is 0 (`assert().success()` passes; only
the parse fails). And the contamination is on **real stdout**, not a merged
stream: the test reads `create.get_output().stdout`, and `assert_cmd` keeps
stdout and stderr separate.

## Reproduction

```bash
git rev-parse HEAD          # ffe4a285
# with NO Qdrant listening on 127.0.0.1:6334:
cargo test --features server-stack --test cli_artifact
```

Locally this **passes** — see *Why it hid* below. To reproduce on a machine that
runs Qdrant, point the client at a dead port:

```bash
CODESCOUT_QDRANT_URL=http://127.0.0.1:1 \
  cargo test --features server-stack --test cli_artifact
```

## Environment

- codescout `experiments` @ `ffe4a285`; `qdrant-client` **1.17.0** (`Cargo.lock`).
- Feature `server-stack` — **not** in `default`, so only the `cargo rb` build and
  the CI lane added in `ecf3e461` compile it.
- CI: `ubuntu-latest`, no Qdrant service container.
- Dev machine: Qdrant live on `127.0.0.1:6334`.

## Root cause

`qdrant_client::Qdrant::new` runs a version-compatibility probe whenever
`config.check_compatibility` is set — and it **defaults to `true`**
(`qdrant-client-1.17.0/src/qdrant_client/config.rs:205`). The probe builds a
throwaway channel pool, spawns a thread with a current-thread Tokio runtime, and
blocks on `health_check()`. When that yields no version, it reports the failure
with `println!` — stdout, not stderr:

```rust
// qdrant-client-1.17.0/src/qdrant_client/mod.rs:143-146
println!(
    "Failed to obtain server version. \
    Unable to check client-server compatibility. \
    Set check_compatibility=false to skip version check."
);
```

`Qdrant::new` is what `.build()` calls (`config.rs:177-179`), so both of our
construction sites inherit the probe:

- `src/retrieval/qdrant.rs:48` — `QdrantWrap::connect`
- `src/retrieval/client.rs:84` — `RetrievalClient::from_config_only`

The librarian reaches the first one on the default artifact backend:
`ArtifactBackend::resolve` returns `Qdrant` on a `server-stack` build
(`src/librarian/artifact_store.rs:42-74`), and `build_tool_context_with` then
calls `QdrantWrap::connect` (`src/librarian/mod.rs:169`).

**The degradation path is not at fault.** `build_tool_context_with` handles an
unreachable Qdrant exactly as designed — it catches the error, reports through
`tracing::warn!`, and falls back to `None` rather than crashing
(`src/librarian/mod.rs:154-188`). The `println!` fires *inside the dependency*,
before any of our error handling exists to intercept it, so no amount of correct
handling on our side can suppress it.

*Measured 2026-08-08:* `gh run view 31271255156 --log-failed` for the failure
text; `grep -rn "check_compatibility" <registry>/qdrant-client-1.17.0/src/` and
`sed -n '95,150p' .../qdrant_client/mod.rs` for the `println!` and the default.
Not inferred — the macro was read in the vendored source.

### Why it hid until now

Two independent covers, and the bug needed both to be lifted:

1. **No lane compiled it.** `server-stack` was in no CI job until `ecf3e461`
   (2026-08-08). See `docs/issues/archive/` for that bug and F-31 in
   `docs/trackers/release-promotion-session-log.md`.
2. **The tests are ambient-dependent.** With Qdrant live on `127.0.0.1:6334` the
   probe *succeeds* and prints nothing, so the three tests pass on any developer
   machine running the stack. Their failure is a function of the environment,
   not of the code — the `tests-that-cannot-fail` class, in the variant where a
   test can only fail somewhere the author never runs it.

## Evidence

### CI job list for run 31271255156

```
$ gh run view 31271255156 --json jobs --jq '.jobs[] | select(.conclusion != "success") | "\(.conclusion)\t\(.name)"'
failure	Test (server-stack — the build cargo rb ships)
```

Every other job in the 18-job matrix passed. The lane added this round is the
only one that fails, and it fails on the defect it was added to expose.

### The knob is not named what the error message says

```
$ grep -rn "check_compatibility" .../qdrant-client-1.17.0/src/
config.rs:40:    pub check_compatibility: bool,
config.rs:182:        self.check_compatibility = false;
config.rs:205:            check_compatibility: true,
mod.rs:102:        if config.check_compatibility {
```

`config.rs:181-185` exposes it as **`skip_compatibility_check()`**, a
`QdrantConfig` builder method — the same impl block as `build()` and `timeout()`,
which our call sites already chain. The message's "Set check_compatibility=false"
names the struct field, not the public API; writing it literally does not
compile.

## Hypotheses tried

1. **Hypothesis:** the test harness merges stderr into stdout, so the diagnostic
   is on stderr and only the test is wrong.
   **Test:** read `artifact_create_then_get_round_trip`
   (`tests/cli_artifact.rs:135-166`) — it passes
   `create.get_output().stdout` to the parser; `assert_cmd`'s `Output` keeps the
   streams separate.
   **Verdict:** rejected. Then confirmed positively from the other end — the
   dependency calls `println!`, not `eprintln!` (`mod.rs:143`).

2. **Hypothesis:** the artifact write itself failed against the missing Qdrant,
   and the JSON is an error envelope.
   **Test:** read the captured stdout in the panic message.
   **Verdict:** rejected — the envelope carries a real `id` and `abs_path`, and
   the process exited 0.

3. **Hypothesis:** `RetrievalClient::from_config_only` is safe because its doc
   comment says it "Constructs without connecting to Qdrant".
   **Verdict:** rejected — the comment is false as written. `build()` performs a
   blocking `health_check()` under the default config. The fix restores the
   comment's truth rather than changing it.

## Fix

Chain `.skip_compatibility_check()` at both construction sites, suppressing the
probe entirely:

- `src/retrieval/qdrant.rs` — `QdrantWrap::connect`
- `src/retrieval/client.rs` — `RetrievalClient::from_config_only`

This is right on three independent grounds, not just the stdout one:

- **Stream hygiene.** A library has no business writing to the caller's data
  channel; we cannot fix the dependency, but we can decline to arm it.
- **Latency.** The probe spawns a thread, a runtime, and a channel pool on every
  client construction, and blocks on a network round trip.
- **The hang class.** `QdrantWrap::connect` is already wrapped in
  `spawn_blocking` + `timeout` precisely because `build()` blocks
  uninterruptibly against a reachable-but-unresponsive Qdrant
  (`docs/issues/archive/2026-06-24-qdrant-hang-wedges-mcp-startup.md`). The
  compat probe is more blocking work on that same path.

We give up a version-mismatch warning. It was never actionable — printed to the
wrong stream, unconditional, and not surfaced to any caller.

Landed on `experiments` in **`6bd44274`**. The promotion path is a fast-forward,
so that SHA is already the master-side SHA; there is no second one to record.
Verified by CI run `31272317541` — green across all 18 jobs, including the
`Test (server-stack — the build cargo rb ships)` lane that reported the defect.

## Tests added

`tests/cli_artifact.rs::run_cmd` now pins `CODESCOUT_QDRANT_URL` to an
unreachable port, so the three round-trip tests exercise the Qdrant-down path
**deterministically on every machine** instead of only where nobody runs them.
That converts the existing assertions into a real regression guard: without
`.skip_compatibility_check()` they fail everywhere, not just in CI.

Pinning the env var is the discriminating choice. Setting
`CODESCOUT_ARTIFACT_BACKEND=sqlite-vec` would also make the tests hermetic — and
would make them incapable of ever catching this bug again.

## Workarounds

Run Qdrant, or set `CODESCOUT_ARTIFACT_BACKEND=sqlite-vec` to keep the CLI off
the Qdrant path. Neither is needed once the fix lands.

## Resume

N/A — fixed, mutation-verified, and green on `experiments` in `6bd44274`.
## References

- CI run `31271255156`, lane `Test (server-stack — the build cargo rb ships)`.
- `docs/trackers/release-promotion-session-log.md` — F-31 (the shipped build is
  the one CI never compiled), which predicted this class.
- `.buddy/memory/architecture-snow-lion/tests-that-cannot-fail.md` — the
  ambient-dependency variant.
- `docs/issues/archive/2026-06-24-qdrant-hang-wedges-mcp-startup.md` — the prior
  blocking-`build()` defect on the same call.
