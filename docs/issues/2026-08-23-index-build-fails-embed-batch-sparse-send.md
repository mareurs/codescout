---
id: '59d74b3c091fa369'
kind: bug
status: open
title: 'BUG: semantic code index build fails with "embed_batch sparse send" on this machine, leaving the index permanently behind HEAD'
owners:
- marius
tags:
- indexing
- embeddings
- remote-http
- new-machine
closed: null
opened: 2026-08-23
owner: marius
related: []
severity: medium
unverified: Root cause not confirmed by network trace — inferred from source + machine context, not measured directly (e.g. no curl/telnet check against the sparse endpoint was run).
---

# BUG: semantic code index build fails with "embed_batch sparse send" on this machine, leaving the index permanently behind HEAD

## Summary

`index(action="build")` fails every attempt on this machine with `error: "embed_batch sparse send"`. The existing index (built on a different machine, last commit `d7988aca`) still serves `semantic_search` queries, but it cannot be refreshed here — it is now 932 commits behind `HEAD` and falling further behind with every session, since every rebuild attempt fails the same way.

## Symptom (Effect)

```
index(action="build") → {"status": "started", ...}
index(action="status") → {
  "indexing": {"status": "failed", "error": "embed_batch sparse send"},
  "git_sync": {"status": "behind", "behind_commits": 932, "last_indexed_commit": "d7988aca", "head_commit": "dffe2546"}
}
```

Reproduced twice in the same session, several minutes apart (once right after activating the project, once after a `git status --short` confirmed no interfering local changes).

## Reproduction

1. On this machine (a laptop this codescout install had not previously run a full reindex on), with the `codescout` project active.
2. `index(action="build")` — returns `{"status": "started"}` immediately (async).
3. Wait, then `index(action="status")` — returns `indexing.status: "failed"`, `indexing.error: "embed_batch sparse send"`.
4. Repeated once more later in the same session with the same result.

Git commit at time of both attempts: `7c3245d7` then `dffe2546` (HEAD advanced between attempts; both failed identically regardless).

## Environment

- `workspace(action="status")` reports `"embedding_backend": "remote-http"`, `"embeddings_model": "CodeRankEmbed"`.
- `memory(action="read", topic="local-environment", private=true)` returns `topic not found` — **no per-machine local-environment memory has ever been written on this host**, consistent with this being a fresh/different machine from whichever one built the existing index (per CLAUDE.md's own documented convention that `local-environment` holds host-specific values like this).
- Legacy on-disk artifact also present: `workspace(activate)` surfaced `.codescout/embeddings/project.db` as a "legacy_semantic_index" needing `codescout migrate-memories` — a second, unrelated signal that this machine's embedding setup has never been fully brought up to date.

## Root cause

*Inferred from source, not measured over the network — see `unverified:`.*

`"embed_batch sparse send"` is the `anyhow::Context` string on the `.send().await` call in `EmbedderHttp::embed_one_batch` (`src/retrieval/embedder.rs:576-582`), for a POST to `format!("{}/embed_sparse", self.sparse_base)`. A `.context(...)?` on a `reqwest::Client::send()` failing is the shape produced by a connection-level failure (refused, timeout, DNS) — `.send()` errors before an HTTP status even exists, so this is not a 4xx/5xx from a reachable server (those are handled separately a few lines down via `status.is_server_error()` retry logic).

`self.sparse_base` is a machine-configured base URL for a remote sparse-embedding (SPLADE/TEI) service (`EmbedderHttp::new`/`with_config`, `src/retrieval/embedder.rs:284-350`). Given the `embedding_backend: "remote-http"` setting and the total absence of any `local-environment` memory on this host, the leading hypothesis is: whatever host/port `sparse_base` resolves to on this machine is not reachable from here — plausibly a value carried over from the original (desktop) machine's config that assumes a host only reachable there (e.g. a LAN-local or localhost-forwarded embedding service).

**Not measured:** which config surface actually supplies `sparse_base` on this host (env var vs `.codescout/project.toml` vs a default), and whether it differs from the value that presumably worked on the original machine. No `curl`/`nc` probe against the resolved sparse endpoint was run.

## Evidence

### First failure
```json
{"indexing": {"status": "failed", "error": "embed_batch sparse send"}, "git_sync": {"status": "behind", "behind_commits": 931, "last_indexed_commit": "d7988aca", "head_commit": "7c3245d7"}}
```

### Second failure (after a retry, HEAD had advanced by one unrelated commit)
```json
{"indexing": {"status": "failed", "error": "embed_batch sparse send"}, "git_sync": {"status": "behind", "behind_commits": 932, "last_indexed_commit": "d7988aca", "head_commit": "dffe2546"}}
```

### Source of the error string
`src/retrieval/embedder.rs:576-582`:
```rust
let resp = self
    .client
    .post(&sparse_url)
    .json(&sparse_body)
    .send()
    .await
    .context("embed_batch sparse send")?;
```

## Hypotheses tried

1. **Hypothesis:** transient overload/backoff exhaustion on a reachable sparse server (the retry-on-424/429/5xx path in the same function). **Test:** retried `index(action="build")` a second time, several minutes later, after HEAD had moved. **Verdict:** rejected as the primary explanation — the retry loop's exhaustion path produces a different, more detailed error (it reads the response body); `.send()` itself failing both times means the request never got a response at all, which points at connectivity, not server load. **Evidence link:** see Evidence above; retry-exhaustion code path is `src/retrieval/embedder.rs:590-598` and was not reached (no response body appears in the error).
2. **Hypothesis:** the sparse embedding endpoint is a machine-specific/local service not configured or reachable on this new host. **Test:** checked for any per-machine config record (`local-environment` memory). **Verdict:** consistent, not confirmed — the memory topic doesn't exist at all on this host, which is exactly what "never configured here" would look like, but no direct network probe was run to confirm the endpoint itself is unreachable (vs. some other cause reachable at the same URL). **Evidence link:** Environment section above.

## Fix

Not yet fixed — this session prioritized the librarian catalog repair and tracker-hygiene sweep the user asked for; this bug was filed on notice per CLAUDE.md's bug-capture discipline rather than chased to resolution.

## Tests added

N/A — not yet fixed. No regression test exists for this failure mode.

## Workarounds

`semantic_search` remains queryable against the stale index (932 commits behind) — results should be treated as increasingly unreliable for anything touched in recent history. Prefer `grep`/`symbols`/`references` for anything landed after `d7988aca` until this is fixed.

## Resume

1. Find where `sparse_base` (and `dense_base`) are actually resolved for a live `EmbedderHttp` on this project (likely `.codescout/project.toml`, an env var, or a hardcoded default in whatever constructs the embedder for `index(action="build")`) — `grep -rn "EmbedderHttp::new\|EmbedderHttp::with_config" src/`.
2. Once the resolved `sparse_base` value is known, `curl -sf "<sparse_base>/info"` (or equivalent) from this machine to confirm reachability directly — that turns hypothesis 2 above from inferred to measured.
3. If unreachable: either point this machine's config at a reachable sparse-embedding endpoint, or confirm whether `local-embed`/`no-features` build variants (mentioned in CLAUDE.md's CI matrix as existing feature configs) avoid the remote dependency entirely and could serve as a same-machine workaround.
4. Re-run `index(action="build")` after any config change and confirm `git_sync.status` reaches `"up_to_date"`.

## References

- `src/retrieval/embedder.rs:159-350` (`EmbedderHttp` construction), `:490-600` (`embed_one_batch`, the failing call).
- CLAUDE.md § "Umbrella names, member lists..." — documents `local-environment` private memory as the canonical per-host config record; its total absence here is the strongest available signal.
- Companion, unrelated-but-adjacent finding from the same session: `workspace(activate)` flags a legacy `.codescout/embeddings/project.db` needing `codescout migrate-memories` — a second sign this machine's embedding setup was never fully brought up to date after being provisioned.

