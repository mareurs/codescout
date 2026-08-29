---
status: open
opened: 2026-08-28
closed:
severity: medium
owner: marius
related:
  - docs/trackers/resume-embedding-transport-stages-1-3.md
  - docs/plans/2026-07-25-embedding-transport-consolidation.md
tags:
  - security
  - test-coverage
  - duplication
  - embeddings
kind: bug
---

# BUG: root's `is_https_or_loopback` guard has zero test coverage; only the codescout-embed twin is tested

## Summary

`is_https_or_loopback` exists twice, byte-identical in logic. The
`codescout-embed` copy has 11 assertions pinning its host-spoofing behaviour;
the **root** copy — the one that decides whether `[embeddings].api_key` is sent
over cleartext HTTP — has none. A mutation to the root copy alone passes the
entire test suite.


**This is one of two instances of the same pattern, not an isolated gap.** Found
2026-08-29 in the same file pair: `EmbedderHttp` also lacked the HTTP request
timeout that `RemoteEmbedder::http_client` has carried all along — and the
crate's doc comment for it names the exact trigger that then occurred on this
host (*"a hung embedding server (e.g. Ollama during GPU discovery failure)"*).
See `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`
and `resume-embedding-transport-stages-1-3:ET-4`.

In both cases the drift runs the same direction: the crate original has the
guard, root's duplicate does not. That is the outcome
`src/retrieval/config.rs:292`'s doc comment predicted when it consolidated the
one sibling that *was* de-duplicated — *"so the two conventions cannot drift
apart."*
## Symptom (Effect)

No failure is observed. That is the point: the guard is load-bearing for a
credential-disclosure decision and is unpinned, so a regression in it is silent.

Concretely, mutating root's host-parsing to the unanchored form the comment
warns against —

```rust
host.starts_with("127.") || host.starts_with("localhost")
```

— would make `http://127.evil.com/v1` and `http://localhost.evil.com/v1` read as
loopback and forward `EMBED_API_KEY` / `[embeddings].api_key` in cleartext to an
attacker-controlled host. `cargo test` stays green, because every assertion on
that behaviour lives in the other crate.

## Reproduction

Commit: `dde7491b` (plus the ET-2 working tree).

```
grep -n 'is_https_or_loopback' src/retrieval/embedder.rs
#   135: definition
#   305: sole call site (EmbedderHttp::new)
grep -rn 'is_https_or_loopback' src/ tests/
#   + src/retrieval/client.rs:209 (guarded_api_key)
#   no test anywhere in the root crate
```

Contrast:

```
grep -n 'is_https_or_loopback' crates/codescout-embed/src/remote.rs
#   48: definition
#   190: call site (RemoteEmbedder::from_url)
#   549-562: tests/is_https_or_loopback_matches_host_exactly  ← 11 assertions
```

## Environment

Linux, Rust workspace, branch `experiments`. Independent of feature flags —
though as of ET-2 both root sites are `remote-embed`-gated, so the untested code
is reachable only in an HTTP-capable build.

## Root cause

Two independent definitions of one security predicate, with tests attached to
only one of them:

- `src/retrieval/embedder.rs:135` — `pub(crate) fn is_https_or_loopback`.
  Callers: `EmbedderHttp::new` (`embedder.rs:305`) and
  `RetrievalClient::guarded_api_key` (`client.rs:209`).
- `crates/codescout-embed/src/remote.rs:48` — private `fn is_https_or_loopback`.
  Caller: `RemoteEmbedder::from_url` (`remote.rs:190`). Tested at
  `remote.rs:549-562`.

Verified 2026-08-28 by reading both bodies: identical logic — same
`[userinfo@]host[:port][/path]` parse, same IPv6 `[::1]` branch, same
`eq_ignore_ascii_case("localhost")` + `IpAddr::is_loopback()` fallback. They
differ only in doc comment and visibility.

Root's doc comment is **accurate about what it claims** — "one guard, two
sources" refers to the two *key sources* within root (the `EMBED_API_KEY` env
var and `[embeddings].api_key` from project.toml), not to the cross-crate twin,
which it explicitly acknowledges as a mirror. The defect is not a false comment;
it is that the mirror is unpinned on the root side.

## Evidence

Root's own comment names the exact attack the missing tests would catch:

```rust
// Parse the HOST out of `[userinfo@]host[:port][/path…]` and match it exactly.
// An unanchored prefix check (`starts_with("127.")`/`starts_with("localhost")`)
// would treat http://127.evil.com or http://localhost.evil.com as loopback and
// leak EMBED_API_KEY over cleartext HTTP.
```

The assertions that pin it exist only in `codescout-embed`
(`remote.rs:549-562`), covering `127.evil.com`, `localhost.evil.com`,
`127.0.0.1.evil.com`, `example.com/127.0.0.1`, `user:pass@localhost`,
`[::1]`, and `127.0.0.5` (the `127.0.0.0/8` case).

## Hypotheses tried

1. **Hypothesis:** root's copy is covered indirectly via `guarded_api_key` tests.
   **Test:** read `selection_tests::guarded_api_key_*` (4 tests,
   `client.rs:882-912`).
   **Verdict:** partially rejected. They cover `https`, loopback, plaintext-drop
   and no-key — the four *outcomes* — but none exercises a **spoofed** host, which
   is the case the anchored parse exists for. The unanchored mutation above passes
   all four.
   **Evidence link:** § Evidence.

## Fix

Two options; the second is already planned.

1. **Port the 11 assertions** to a root-side test module. Cheap, immediate, and
   independent of the consolidation work. Keeps the duplication.
2. **Delete root's copy** and route both call sites at the `codescout-embed`
   function, which is the tested one. This is what
   `docs/plans/2026-07-25-embedding-transport-consolidation.md` Stage 3 already
   intends (tracked as `ET-4` in
   `docs/trackers/resume-embedding-transport-stages-1-3.md`). Requires making the
   `codescout-embed` function `pub`, which it currently is not.

Prefer (2) if `ET-4` is executed soon; take (1) if it slips, because the gap is a
credential-disclosure guard and should not wait on a refactor.

**Not fixed in this record.** SHA / patch-id: N/A.

## Tests added

None — this record is the finding, not the fix. See § Fix.

## Workarounds

None needed at runtime; current behaviour is correct. The exposure is to a
*future* regression, not to present code.

## Resume

Decide between Fix (1) and Fix (2) against `ET-4`'s timeline. If (1): copy
`crates/codescout-embed/src/remote.rs:549-562` into a new
`#[cfg(all(test, feature = "remote-embed"))]` module in
`src/retrieval/embedder.rs` and re-point it at the root function. Verify the
tests actually bind by mutating root's `host` parse to the unanchored form and
confirming a **red** run before committing them green.

## References

- `src/retrieval/embedder.rs:135` — untested root definition
- `src/retrieval/client.rs:209` — `guarded_api_key`, the project.toml path
- `crates/codescout-embed/src/remote.rs:48,549-562` — the tested twin
- `docs/trackers/resume-embedding-transport-stages-1-3.md` — `ET-4` (Stage 3),
  `ET-7` (the ET-2 execution record that surfaced this)
