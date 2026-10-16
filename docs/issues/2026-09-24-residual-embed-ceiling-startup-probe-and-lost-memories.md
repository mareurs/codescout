---
id: '9e6043f4db849185'
kind: bug
status: fixed
title: 'RESIDUAL: Implement the startup probe of the effective per-request embed ceiling, and re-write the 8 detected lost memories'
tags:
- cluster/unclassified
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Implement the startup probe of the effective per-request embed ceiling, and re-write the 8 detected lost memories.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` (status `fixed`):

> Two residues, both deliberate: Fix step 3 (a startup probe of the effective per-request embed ceiling) is still ABSENT, so a misconfigured backend is still discovered at first oversized write rather than at connect. And the 8 already-lost memories are now DETECTED but not yet re-written — the repair is named in the verify hint, not performed.

## Fix

Probe shipped in `49c551d1`; the memory half needed no fix. Evidence and identifiers in
the Resolution below.

## Resolution 2026-10-16

Both halves resolved; the second needed no work.

### Half 1 — the startup probe

- **SHA:** `49c551d1` (branch `worktree-agent-aa5d85ef986eb51aa`, on top of `experiments` `3923da8c`)
- **patch-id:** `69106ee4cd2febd1d117bd5fd851de00726d3a27`

**Probe design.** llama.cpp serves the two numbers that bind an embed request, free and
read-only. Measured on the live server (`http://127.0.0.1:48081`, 2026-10-16):

| Endpoint | Answer | Field |
|---|---|---|
| `GET /slots` | 200 | `[0].n_ctx` = 4096 (per slot) |
| `GET /v1/models` | 200 | `data[0].meta.n_ctx_train` = 2048 |
| `GET /props` | **HTTP 500** `vector::_M_range_check` | unusable on this embedding-only server, so not used |

Effective ceiling = `min(per-slot n_ctx, n_ctx_train)`. It is compared against
`chunk_size_for_model(model)` through one shared rule (`budget_chars_for_tokens`, 85 % of
context at 3 chars/token), so the check cannot disagree with the budget it checks.

**Where.** `crates/codescout-embed/src/ceiling.rs` (pure types, parsers, `judge`);
`crates/codescout-embed/src/remote.rs` `probe_embed_ceiling` (two GETs, 2 s each, no API key,
nothing POSTed); `src/retrieval/ceiling.rs` (`assess`, `spawn_startup_report`);
called from `src/main.rs` `Commands::Start` before `server::run`.

**Fail-soft.** The probe's return type has no error arm. Unreachable, non-2xx (OpenAI and
Ollama have neither endpoint), malformed JSON and timeout all yield "unknown", logged at
debug. The report runs in a detached task, so it cannot fail or delay startup. With no
`embedder_url` (`local:`, `ollama:`, `openai:`) nothing is spawned at all, so a paid endpoint
is never contacted.

**Not done, deliberately.** The report is a log line; it is not surfaced in
`index(action=status)`. A binary-search probe (POSTing oversized inputs) was rejected: it is
the one design that would bill a paid endpoint, and the two GETs give the same answer on the
backend that actually exhibits the failure.

**Verified how.** The Rust probe was verified against a loopback server serving the exact
response shapes captured above; the endpoints themselves were verified by `curl` against the
live server. The probe was not run end-to-end against that live server from Rust.

### Review follow-up 2026-10-16: the probe no longer trusts where its URL leads

An Opus review noted that the probe's client followed redirects (reqwest follows up to 10, to
any host) and read a body of any size, bounded only by the 2 s timeout. Both reproduced red
first: a loopback server redirecting to a second loopback port had its target contacted and
the target's `/slots` reported as this server's ceiling, and a valid 257 KB `/slots` body was
parsed whole. Fixed in `probe_embed_ceiling_with_timeout`
(`crates/codescout-embed/src/remote.rs`): redirects are disabled (a 3xx is "unknown"), a
declared `Content-Length` over `CEILING_PROBE_MAX_BODY` is refused before reading, and the
body is streamed and abandoned past the cap. The cap is 256 KiB, chosen against a measured
18 039-byte `/slots` for 16 slots on the live server (about 230 slots of headroom).
Tests: `a_redirect_is_not_followed_to_another_host` (asserts both the unchanged default
ceiling and that the target saw no request), `an_oversized_body_fails_soft_instead_of_being_read_whole`,
and `a_body_just_under_the_cap_is_still_read` (guards the cap against being set too low).

### Half 2 — the 8 lost memories: already repaired, nothing to write

Re-checked against HEAD, as the Fix section instructed. A read-only
`index(action="verify")` on the live project, 2026-10-16:

```json
"memories": { "on_disk": 26, "in_store": 26, "missing_count": 0,
              "orphan_count": 0,
              "hint": "Every memory on disk has a point, and every point has a file." }
```

The 8 were `cargo-test-lib-skips-integration`, `domain-glossary`, `eval-design`,
`language-patterns`, `onboarding`, `project-overview`,
`research/agent-memory-frameworks`, `system-prompt`. A read-only `recall` finds `eval-design`
(the 31 KB one) in the structured store with `created_at` 2026-08-26T13:58, i.e. re-written
the same day it was detected. No memory was written by this resolution.

## Tests added

`crates/codescout-embed/src/ceiling.rs` `tests` (12): both parsers against the captured live
shapes, every rejected shape, the min-of-limits rule, the live host passing its own budget,
the reporter's `--ctx-size 8192 --parallel 8` being caught (1024 tokens, 2611-char max),
the exact-fit boundary, unknown-is-never-fits, and the shared-rule pin against
`chunk_size_for_model`.

`crates/codescout-embed/src/remote.rs` `tests` (5, loopback server, no real or paid service):
reads both limits and asserts exactly two credential-free GETs and no POST; neither endpoint
-> unknown; one endpoint -> partial; unreachable -> unknown; a peer that accepts and never
answers is cut off (timeout injected at 150 ms, with an outer bound so a missing timeout
fails instead of hanging).

`src/retrieval/ceiling.rs` `tests` (5): every report arm, and the probe being called with the
configured url. Mutation: inverting the `slot_binds` comparison turns two of them red
(`a_small_slot_warns_and_names_the_slot_remedy`,
`a_model_limit_below_the_table_warns_without_offering_a_flag_that_cannot_help`).

Targeted runs and clippy (`--workspace --all-targets --features local-embed -D warnings`),
plus `cargo check --no-default-features --all-targets`, were green. The full gate was not run.

## References

- `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md` — parent
