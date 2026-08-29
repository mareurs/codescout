---
status: fixed
opened: 2026-08-28
closed: 2026-08-29
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
unverified: The duplication itself is NOT resolved -- two byte-equivalent copies of this predicate still exist, root's and the crate's. This record covers only the missing coverage. ET-4 removes the duplicate; until it lands, a change must be made in both places or they drift again, which is the failure mode that produced this bug.
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

**Option 1 taken — the 11 assertions are ported to root.**
`is_https_or_loopback_matches_host_exactly` now exists in
`src/retrieval/embedder.rs`'s `remote-embed`-gated test module, ported verbatim
from `crates/codescout-embed/src/remote.rs:549-563` so the two stay comparable
while both copies exist.

**The framing in the original version of this section was wrong and is worth
correcting rather than deleting.** It offered porting the assertions and deleting
root's copy in ET-4 as *alternatives* — "prefer (2) if ET-4 is executed soon".
They are not alternatives. An untested function cannot be shown equivalent to its
replacement, so the tests are the **precondition** for the deletion, not a
substitute for it. Deleting first would have removed the guard and left nothing
able to demonstrate the crate's version behaves identically. This is recorded as
Phase A of `resume-embedding-transport-stages-1-3:ET-8`.

Consequently the test is written to **survive** ET-4: when root's copy goes,
re-point it at the crate's function rather than deleting it.

**Fix commit:** `28bb6e8a` on `experiments`
**patch-id:** `52cb00b5b67d80de322ccc0c9f5a6166d1860fb0`

**Still open, tracked elsewhere:** the duplication. `ET-4` deletes root's copy
once `ET-3` unblocks it, and `ET-8` Phase B4 notes the crate's function must
become `pub` first — it is private today.
## Tests added

`retrieval::embedder::tests::is_https_or_loopback_matches_host_exactly`
— 6 positive cases (https, `localhost`, `127.0.0.1`, `127.0.0.5` for the
`127.0.0.0/8` range, `[::1]`, and `user:pass@localhost` for the userinfo path)
and 4 spoofing negatives (`127.evil.com`, `localhost.evil.com`,
`127.0.0.1.evil.com`, `example.com/127.0.0.1`).

**Verified by mutation twice, because one probe proved less than it appeared
to.** Replacing the host parse wholesale with the unanchored
`starts_with("127.")` form fails at line 1032 on the `[::1]` *positive* — which
establishes that the positives fire, and says nothing whatever about the four
negatives, since the test panics before reaching them. A second mutation leaving
every positive intact and breaking only the prefix check fails at line 1040 on
`!is_https_or_loopback("http://127.evil.com/v1")`.

So both axes are shown live: the false-negative (rejecting real loopback) and
the false-positive (accepting a spoofed host) — and it is the second that this
bug was actually about. A single probe would have left the security-relevant
assertions unproven while looking like a completed verification.

Gate at fix time: fmt, clippy `--workspace --all-targets --features local-embed
-D warnings`, test **4638 passed / 0 failed** (baseline 4637 — one new pass,
this test, no other test changing state).
## Workarounds

None needed at runtime; current behaviour is correct. The exposure is to a
*future* regression, not to present code.

## Resume

N/A for the coverage gap — closed by `28bb6e8a`.

For the duplication, see `resume-embedding-transport-stages-1-3:ET-8`. Phase B4
exports the crate's function; Phase D1 deletes root's copy and re-points this
test at the crate's.
## References

- `src/retrieval/embedder.rs:135` — untested root definition
- `src/retrieval/client.rs:209` — `guarded_api_key`, the project.toml path
- `crates/codescout-embed/src/remote.rs:48,549-562` — the tested twin
- `docs/trackers/resume-embedding-transport-stages-1-3.md` — `ET-4` (Stage 3),
  `ET-7` (the ET-2 execution record that surfaced this)
