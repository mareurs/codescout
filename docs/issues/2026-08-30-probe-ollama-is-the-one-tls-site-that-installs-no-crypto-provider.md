---
id: ac9aa2f9b38eab9b
kind: bug
status: open
title: probe_ollama is the one TLS construction site that installs no crypto provider, and its failure is reported as "Ollama is not reachable"
tags:
- codescout-embed
- retrieval
- tls
- root-crate-asymmetry
- misleading-error
closed: ''
opened: 2026-08-30
owner: marius
related:
- docs/issues/archive/2026-08-30-remote-embedder-panics-on-a-short-server-response.md
- docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md
severity: low
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

Not yet run — this was found by reading, and the honest status is that the reachability
argument is established from the code path and the `rustls-no-provider` requirement, not
from an observed failure. To run it: a consumer binary depending only on
`codescout-embed`, no `install_default_crypto_provider` call of its own,
`OLLAMA_HOST=https://<any-tls-host>`, `create_embedder("ollama:<model>")`. Expect the
"Ollama is not reachable" bail with a TLS-provider error in the `{e}` slot.

Do this before fixing — per CLAUDE.md, the plan is a hypothesis about the reproduction.
In particular it would settle whether rustls reports a distinguishable provider error or
something more generic, which decides whether option (b) below is worth anything.

## Fix

Three options, not yet chosen:

- **(a) Call the provider install in `probe_ollama`.** Smallest change. Requires making
  the fn reachable from a free function — it is currently an associated fn on
  `RemoteEmbedder`; `RemoteEmbedder::install_default_crypto_provider()` is callable from
  within the crate, so this is a one-line addition with no signature change.
- **(b) Also fix the error text**, so a TLS-setup failure is not reported as an
  unreachable server. Independent of (a) and arguably the more valuable half: (a) fixes
  codescout's consumers, (b) fixes the diagnosis for anyone whose probe fails for any
  other reason too.
- **(c) Export a public installer from the crate** so consumers can do it themselves.
  Broader API question; (a) makes it unnecessary for this path.

(a) and (b) are complementary; doing only (a) leaves the misleading message in place for
every other probe failure.

## Tests added

None yet.

## Resume

Run the reproduction first. Then (a)+(b) together. Worth checking in the same pass whether
any other crate entry point can reach a TLS handshake before `build_client` — this one was
found by enumerating construction sites of `install_default_crypto_provider` and noticing
`probe_ollama` was absent from the list, which is a repeatable check.

## References

- `crates/codescout-embed/src/remote.rs:619-629` — `probe_ollama`
- `crates/codescout-embed/src/remote.rs:164-210` — `install_default_crypto_provider`, `build_client`
- `crates/codescout-embed/src/lib.rs:265-278` — the calling path and the error text
- `src/retrieval/transport.rs:35-38` — the invariant this violates
- `resume-embedding-transport-stages-1-3:ET-4` — the duplication audit, and the direction claim this contradicts

