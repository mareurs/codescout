---
id: a7af9964a16e8056
kind: bug
status: fixed
title: probe_ollama is the one TLS construction site that installs no crypto provider, and its failure is reported as "Ollama is not reachable"
tags:
- codescout-embed
- retrieval
- tls
- root-crate-asymmetry
- misleading-error
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related:
- docs/issues/archive/2026-08-30-remote-embedder-panics-on-a-short-server-response.md
- docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md
severity: high
---

## Summary

`codescout-embed`'s `probe_ollama` (`crates/codescout-embed/src/remote.rs:619-629`) builds
its own `reqwest::Client` directly instead of going through
`RemoteEmbedder::build_client`, and so is **the only TLS construction site in either tree
that does not call `install_default_crypto_provider()` first**. reqwest is configured with
`rustls-no-provider` — the crate's own doc comment states the requirement:

> *"Required because reqwest uses `rustls-no-provider`: callers must install a provider
> before the first TLS handshake."* (`remote.rs:166-168`)

When the host is `https://`, the handshake fails, and `create_embedder_with_config`
reports it as:

> *"Ollama is not reachable at {host}: {e} — Start Ollama or switch to a different
> embedding backend."*

The diagnosis and the offered remedy are both wrong. Ollama may be running perfectly.


> **⚠ Corrected by the reproduction, 2026-08-30. Three premises in this file are wrong,
> and they are left visible below rather than rewritten, because what they got wrong is
> the point.** The reproduction was run before the Fix section was read, per CLAUDE.md.
>
> 1. **It panics; it does not return `Err`.** reqwest's `default_rustls_crypto_provider()`
>    is a literal `panic!("No provider set")` (`reqwest-0.13.2/src/async_impl/client.rs:2461`)
>    that runs inside `ClientBuilder::build()`. So `create_embedder_with_config`'s
>    `if let Err(e) = probe_ollama(&host)` never executes, and the *"Ollama is not
>    reachable at {host}"* message this file's title and Summary are built around is
>    **unreachable code that never prints**. The stated symptom does not happen.
> 2. **The URL scheme is irrelevant.** The client is constructed eagerly, before any
>    request, so plain `http://` panics identically. *"When the host is `https://`, the
>    handshake fails"* is wrong — there is no handshake.
> 3. **Severity was understated** (`low` → `high`). Not "an operator with an https Ollama
>    host": **every** external consumer calling `create_embedder("ollama:…")` aborts, at
>    **zero configuration**, on the default `http://localhost:11434`.
## Symptom (Effect)

An operator with `OLLAMA_HOST=https://ollama.internal:11434` and a `model = "ollama:…"`
config is told their server is down and instructed to start it. Restarting Ollama changes
nothing, because the failure is in the calling process's TLS setup.

## Root cause

`create_embedder_with_config` (`crates/codescout-embed/src/lib.rs:265-278`) reads
`OLLAMA_HOST` — operator-controlled, and legitimately `https://` for a remote Ollama
behind TLS — and calls `probe_ollama(&host)` **before** `RemoteEmbedder::ollama(model_id)`.
That ordering is what makes it reachable: `RemoteEmbedder::ollama` → `build_client` →
`install_default_crypto_provider` is the crate's only install, and the probe runs first.

`probe_ollama` bypasses `build_client` entirely:

```rust
pub async fn probe_ollama(host: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    client.get(host.trim_end_matches('/')).send()  // ← first TLS handshake, no provider
```

**The 2s total timeout here is correct and should stay** — a reachability probe wants a
tight total bound, not `build_client`'s `read_timeout` + 300s pair. The defect is the
missing provider install, not the timeout policy.

## Why it survived, and who is actually exposed

Not reachable in codescout's own binary: `src/main.rs:253` installs the provider at
startup, and `CryptoProvider::install_default` is process-global, so root's call satisfies
the crate's need before any crate code runs. Every other construction site in both trees
installs it — `main.rs:253`, `agent/mod.rs:448`, `reranker.rs:80`, `embedder.rs:355`,
`remote.rs:204`. Root's `transport.rs:35-38` states the invariant as already holding:

> *"The only documented failure is TLS backend initialisation, which
> `crate::install_default_crypto_provider` has already performed at **every construction
> site**."*

That sentence is true of root and false of the crate, and root's startup call is what
hides the difference.

**Exposed: any external consumer of `codescout-embed`.** `create_embedder` is public API,
and the crate's `install_default_crypto_provider` is a **private associated fn** on
`RemoteEmbedder` (`remote.rs:169`, no `pub`), not re-exported by `lib.rs:23-24`. So a
consumer has no supported way to install the provider themselves — they depend on the
crate doing it, and this one path does not.


### The real answer, measured after the fix — the crate's own test for this path could not fail

The account above is true and incomplete. `remote::tests::probe_ollama_errors_when_unreachable`
**already existed** (`remote.rs:832`) and is nearly identical to the regression test added with
the fix: same function, same `http://127.0.0.1:1`, same assertion on `"not reachable"`. So
this path was already covered, and pre-fix that test should have panicked.

It did not, and the reason is **cross-test contamination through process-global state**.
`CryptoProvider::install_default` is process-wide, `remote::tests` is full of siblings that
construct a `RemoteEmbedder` (`from_url_*`, `custom_*`, `openai_*`, the retry tests), and every
one of them routes through `build_client`, whose first line installs the provider. Whichever
sibling ran first installed it for everybody.

Measured 2026-08-30 by re-removing the install and running the **same** pre-fix code two ways:

| run | result |
|---|---|
| `--lib remote::tests::probe_ollama_errors_when_unreachable -- --exact` | **panics**, `No provider set` |
| `--lib` (full suite, 51 tests) | **passes**, suite green 51/0 |

So the guard existed, ran on every invocation, reported green, and was structurally incapable
of failing. That is a third distinct way a check can be worthless, alongside the two this
repo recorded the same day — an assertion monotone under the change it should catch, and a
failure on a path nothing traverses. Here the assertion is fine and the path *is* traversed;
the test's own siblings repair the defect before it can be observed.

It also means no amount of care in the existing suite could have caught this. The only
formulation that can is a **separate test binary**, which is why the added regression test
lives in `tests/` and says, at length, that nothing else may join it.
## Class

Third instance of the asymmetry named in
`docs/issues/archive/2026-08-30-remote-embedder-panics-on-a-short-server-response.md`:
a hazard handled on root's side and not on the crate's, where root's own behaviour is what
masks the gap, so the crate ships it to every external caller while exactly one caller is
shielded.

**One honest difference from the other two:** those are literal duplicated function pairs.
This is not — `probe_ollama` has no root twin. It is the same *mechanism* (root's startup
call masks a crate gap) applied to an invariant rather than to a duplicated body. Recorded
as an instance because the masking structure is what makes the class dangerous, but a
pairwise diff would never have found it.

**It also runs counter to `resume-embedding-transport-stages-1-3:ET-4`'s stated
direction**, which reads *"root's copy is the one missing the guard, both times."* That
held for ET-4's two samples and does not generalise: the three instances found on
2026-08-30 all have the crate as the deficient side.

## Reproduction

**Run 2026-08-30.** Two files under `crates/codescout-embed/tests/` — a `tests/` file
compiles to its own binary linking only `codescout-embed`, which is the consumer scenario
this file specified, and the only isolation strong enough given that
`CryptoProvider::install_default` is **process-global**.

Subject process, nothing installing a provider — three separate `#[test]` fns so one
panic could not hide the others (the first draft put all three in one function and the
very first call aborted it, which hid finding #2 for a run):

```text
[http://127.0.0.1:1]     panicked at reqwest-0.13.2/.../client.rs:2461: No provider set
[http://localhost:11434] panicked at reqwest-0.13.2/.../client.rs:2461: No provider set
[https://example.com]    panicked at reqwest-0.13.2/.../client.rs:2461: No provider set
```

Positive control, a **separate process** differing in exactly one respect — a provider
installed, via `RemoteEmbedder::ollama(...)` → `build_client`, which opens no socket:

```text
[http://127.0.0.1:1]     Err(Ollama not reachable at http://127.0.0.1:1: ...)
[http://localhost:11434] Ok(())
[https://example.com]    Ok(())
```

The control is what makes this a measurement rather than a plausible story: the network
works (the real TLS handshake to `example.com` succeeded), Ollama is in fact running
locally, and the provider is the only difference between the two processes.

**The question this file asked the reproduction to settle — whether rustls reports a
distinguishable provider error — is moot.** There is no error to inspect. That is what
killed fix option (b) as written.
## Fix

**Shipped: (a).** `RemoteEmbedder::install_default_crypto_provider();` as the first line
of `probe_ollama`. It is a private associated fn in the same module, so this compiles with
no signature change and no new public API. `build_client` installs for every other client
in the crate; `probe_ollama` deliberately does not use `build_client` — a reachability
probe wants a tight 2s **total** bound, not that path's `read_timeout` + 300s pair — so it
has to install for itself. The timeout policy was correct and is unchanged.

**(b) is dropped, and the reproduction is why.** This file argued (b) was "arguably the
more valuable half" because a TLS-setup failure is reported as an unreachable server.
There is no such failure: it panics, and the message is never printed. The premise was
false, so the option had no motivation.

What survives is narrower and was **not** in the plan: after (a), a genuine `https://`
Ollama with an expired or untrusted certificate *does* return `Err`, and *is* told
*"Start Ollama or switch to a different embedding backend."* That is a real misdiagnosis,
but it is a message-wording change with its own test and its own reproduction, not part of
this fix. Not filed as a follow-up: it is speculative until someone reproduces a cert
failure, and this file's own history is the argument for not writing plans against
unreproduced symptoms.

**(c) confirmed unnecessary.** The crate has exactly two `reqwest::Client::builder()`
sites (the other is root's `transport.rs`), and (a) closes the only unguarded one, so no
consumer needs a public installer for this path.
## Tests added

`crates/codescout-embed/tests/ollama_probe_installs_its_own_crypto_provider.rs` — one test
in its own binary.

**Asserts a specific `Err` naming the host, not merely "did not panic".** "Did not panic"
is monotone under removal: gut `probe_ollama` to `Ok(())` and it still passes. Verified in
both directions — the unfixed code panics, and a probe mutated to return `Ok(())`
unconditionally kills it **on its own `expect_err`** (`:57`) rather than anywhere earlier,
which is the criterion that separates an informative kill from an incidental one.

**Two coverage facts recorded in the file itself, both measured:**

- **The documented local gate does not run it.** Bare `cargo test` builds only the root
  package's targets (4878 with and without the file); `cargo test --workspace
  --no-default-features` switches `remote-embed` off, so it compiles to nothing. It runs
  under `cargo test --workspace` (4930) — CI's `default` matrix lane. CI-covered,
  locally invisible.
- **No in-tree caller can reach the abort it guards.** Every construction site in this
  repo installs the provider first, and `retrieval/transport.rs` cites that invariant as
  its *reason* for not handling the error. The alarm was in working order and wired to a
  door welded shut — which is why it survived, and why loudness alone is not what makes a
  failure observable.
## Resume

**Fixed on `experiments`.**

- Fix + regression test: `1909e5f0` (`experiments`)
- patch-id: `90c1612bcd948c09e0fd373be2e754134bf9a463`

Gate green at the fix commit: `cargo fmt`; `cargo clippy --workspace --all-targets
--features local-embed -- -D warnings`; `cargo clippy -p codescout-embed --all-targets
--features remote-embed -- -D warnings` (run additionally, because the gate's `local-embed`
form does not compile this test); `cargo test` 4878/0; `cargo test --workspace` 4930/0;
`cargo test --workspace --no-default-features` 3385/0.
## References

- `crates/codescout-embed/src/remote.rs:619-629` — `probe_ollama`
- `crates/codescout-embed/src/remote.rs:164-210` — `install_default_crypto_provider`, `build_client`
- `crates/codescout-embed/src/lib.rs:265-278` — the calling path and the error text
- `src/retrieval/transport.rs:35-38` — the invariant this violates
- `resume-embedding-transport-stages-1-3:ET-4` — the duplication audit, and the direction claim this contradicts
