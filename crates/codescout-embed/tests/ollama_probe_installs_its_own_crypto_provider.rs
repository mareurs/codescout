//! `probe_ollama` must install the rustls crypto provider itself.
//!
//! # This test MUST be the only thing in this binary, and that is load-bearing
//!
//! `CryptoProvider::install_default` is **process-global**. A file under `tests/`
//! compiles to its own binary, which is the only isolation strong enough here — an
//! in-crate `#[cfg(test)]` module shares a process with every other unit test, and any
//! one of them constructing a `RemoteEmbedder` installs the provider and makes this
//! assertion vacuous forever, silently and with no failure to notice.
//!
//! **Do not add a second test to this file, and do not construct a `RemoteEmbedder`
//! here.** `RemoteEmbedder::ollama` / `from_url` / `openai` all route through
//! `build_client`, whose first line is the install. Any of them, anywhere in this
//! binary, disarms this test. If you need another case, add another *file*.
//!
//! # What it guards
//!
//! reqwest is configured with `rustls-no-provider` (both manifests), so its
//! `default_rustls_crypto_provider()` is compiled as a literal
//! `panic!("No provider set")`. That runs during `ClientBuilder::build()` — **eagerly,
//! before any request, and regardless of URL scheme.** So an unfixed `probe_ollama`
//! aborts the process on plain `http://localhost:11434`, the zero-configuration
//! default, not merely on `https://`.
//!
//! Measured 2026-08-30 against the unfixed code, three cases, all identical:
//!
//! ```text
//! [http://127.0.0.1:1]     panicked at reqwest/src/async_impl/client.rs:2461: No provider set
//! [http://localhost:11434] panicked at reqwest/src/async_impl/client.rs:2461: No provider set
//! [https://example.com]    panicked at reqwest/src/async_impl/client.rs:2461: No provider set
//! ```
//!
//! and in a second process differing only in that a provider was installed:
//!
//! ```text
//! [http://127.0.0.1:1]     Err(Ollama not reachable at http://127.0.0.1:1: ...)
//! [http://localhost:11434] Ok(())
//! [https://example.com]    Ok(())
//! ```
//!
//! # Why it asserts `Err` and not merely "did not panic"
//!
//! "Did not panic" is monotone under removal: gut `probe_ollama` to `Ok(())` and the
//! test still passes. Asserting a specific `Err` whose message names the probe fails in
//! both directions — the unfixed code panics, and a no-op'd probe returns `Ok`.
//!
//! Port 1 on loopback refuses immediately, so this needs no network and no Ollama.
//!
//! # Which commands run this, and whose risk it covers
//!
//! Measured 2026-08-30. **The documented local gate does not run it.** Bare `cargo test`
//! builds only the root package's targets — 4878 passed with and without this file — and
//! `cargo test --workspace --no-default-features` switches `remote-embed` off, so this
//! file compiles to nothing. It runs under `cargo test --workspace` (4930), which is
//! CI's `default` matrix lane (`flags: ""` in `.github/workflows/ci.yml`). So it is
//! CI-covered and locally invisible: use the `--workspace` form when touching this path,
//! or a green local gate will tell you nothing about it.
//!
//! It also guards a path **no in-tree caller can traverse**. Every construction site in
//! this repo installs the provider first — `main.rs`, `agent/mod.rs`, `reranker.rs`,
//! `embedder.rs`, and `build_client` — and `retrieval/transport.rs` cites that invariant
//! as its *reason* for not handling the error. codescout itself therefore cannot reach
//! the abort. The exposure is external consumers, for whom `create_embedder` is public
//! API while the installer is a private associated fn that `lib.rs` does not re-export,
//! so they have no supported way to install it themselves.

#![cfg(feature = "remote-embed")]

use codescout_embed::remote::probe_ollama;

#[tokio::test]
async fn probing_a_dead_port_reports_it_rather_than_aborting_the_process() {
    let result = probe_ollama("http://127.0.0.1:1").await;

    let err = result.expect_err("nothing listens on loopback port 1, so this must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("Ollama not reachable at http://127.0.0.1:1"),
        "the probe must report the unreachable host it was given; got: {msg}"
    );
}
