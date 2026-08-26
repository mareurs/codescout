---
id: '59d74b3c091fa369'
kind: bug
status: fixed
title: 'BUG: semantic code index build fails with "embed_batch sparse send" on this machine, leaving the index permanently behind HEAD'
owners:
- marius
tags:
- indexing
- embeddings
- remote-http
- new-machine
closed: 2026-08-24
no_fix_commit: Machine-local config change (an ONNX model symlink), not a repo commit — nothing in this repo's source caused the failure, so no SHA or patch-id exists to cite. See `unverified:` for what that leaves unestablished.
opened: 2026-08-23
owner: marius
related: []
severity: medium
unverified: Fixed by a machine-local config change (symlink), not a repo commit, so there is no SHA/patch-id to cite and no regression test is possible — nothing in this repo's source caused the failure. Reproduction confirmed the fix (sparse error no longer occurs; indexing progressed past the point it previously died), but the full catch-up reindex (943 commits) was still running in the background when this was last checked, not yet confirmed to reach HEAD.
---

# BUG: semantic code index build fails with "embed_batch sparse send" on this machine, leaving the index permanently behind HEAD

## Summary

`index(action="build")` failed every attempt with `error: "embed_batch sparse send"`
because the machine-wide `~/.config/codescout/.env` symlink pointed at a stale, removed
compose profile (`.env.amd`) that re-enabled the sparse embedding leg, while the only
compose profile actually running (`gpu`) keeps the sparse container stopped by design.
Fixed by repointing the symlink to `.env.gpu`, which matches reality; confirmed by
reproducing the build and watching it progress past the previous failure point.
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

**Confirmed by direct reproduction and source tracing — not the network-unreachable-remote-host guess this file originally made.**

`CODESCOUT_SPARSE_EMBEDDER_URL` resolves to `http://127.0.0.1:48084` — loopback, not a
desktop-only remote address. The failure is that nothing is listening on that port on
this machine:

1. `docker ps -a` shows `codescout-sparse-gpu` — `Exited (137) 3 weeks ago`. Stopped by
   project policy (`docker-compose.yml`'s own header): sparse was disabled 2026-07-28 to
   free ~2.4 GiB VRAM, via `CODESCOUT_DISABLE_SPARSE=1` in `.env.gpu`. The `amd`/`cpu`
   compose profiles were removed entirely on 2026-07-27 — `gpu` is the only profile left,
   and it keeps `sparse-gpu` stopped by design.
2. But `CODESCOUT_DISABLE_SPARSE` was **not actually set** in the running MCP process's
   environment. Traced why: `codescout::config::load_startup_env()` (`src/config/global.rs`)
   deliberately never reads the repo's own `.env*` files — only `$CODESCOUT_ENV_FILE`, or
   else `~/.config/codescout/.env` (a `$HOME`-scoped, machine-wide path, by design: "a
   user-scoped server must not absorb an arbitrary repo's `.env`").
3. `~/.config/codescout/.env` was a symlink to `.env.amd` — a profile whose compose
   services (`sparse-cpu`, `dense-amd`, `reranker-amd`) no longer exist in
   `docker-compose.yml` at all. Worse, `.env.amd` was edited 2026-08-07 to **re-enable**
   sparse (`# CODESCOUT_DISABLE_SPARSE=1`, commented out) — the exact opposite of the
   `gpu` profile's policy, and `docker-compose.yml`'s own top-of-file comment (claiming
   `.env.amd` sets `CODESCOUT_DISABLE_SPARSE=1`) is stale relative to that edit.

Net effect: every `codescout` process on this machine — any repo, any session — loaded a
stale, self-contradicting env profile that left sparse enabled while the only running
container stack keeps it off. Not laptop-vs-desktop; a machine-wide symlink pointing at
the wrong (and internally inconsistent) `.env` profile.
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

Repointed the symlink to the profile that actually matches the running stack:

```
ln -sfn /home/marius/work/claude/codescout/.env.gpu ~/.config/codescout/.env
```

`.env.gpu` has `CODESCOUT_DISABLE_SPARSE=1` active (line 120), matching `gpu` being the
only compose profile with services actually running. Since `load_startup_env()` only runs
once at process start, this required an MCP reconnect (`/mcp`, then a full CC restart) to
take effect for already-running sessions on this machine.

Verified after restart: `env | grep CODESCOUT_DISABLE_SPARSE` now shows `1` in the running
process, and `index(action="build")` no longer fails with `embed_batch sparse send` —
indexing proceeded (chunk count climbing across repeated `index(action="status")` polls)
past the point where it previously died on every attempt.
## Tests added

N/A — not yet fixed. No regression test exists for this failure mode.

## Workarounds

No longer needed for this failure mode. (Historical: while broken, `semantic_search`
served the stale pre-fix index; `grep`/`symbols`/`references` were the fallback for
anything landed after `d7988aca`.)
## Resume

1. Find where `sparse_base` (and `dense_base`) are actually resolved for a live `EmbedderHttp` on this project (likely `.codescout/project.toml`, an env var, or a hardcoded default in whatever constructs the embedder for `index(action="build")`) — `grep -rn "EmbedderHttp::new\|EmbedderHttp::with_config" src/`.
2. Once the resolved `sparse_base` value is known, `curl -sf "<sparse_base>/info"` (or equivalent) from this machine to confirm reachability directly — that turns hypothesis 2 above from inferred to measured.
3. If unreachable: either point this machine's config at a reachable sparse-embedding endpoint, or confirm whether `local-embed`/`no-features` build variants (mentioned in CLAUDE.md's CI matrix as existing feature configs) avoid the remote dependency entirely and could serve as a same-machine workaround.
4. Re-run `index(action="build")` after any config change and confirm `git_sync.status` reaches `"up_to_date"`.

## References

- `src/retrieval/embedder.rs:159-350` (`EmbedderHttp` construction), `:490-600` (`embed_one_batch`, the failing call).
- CLAUDE.md § "Umbrella names, member lists..." — documents `local-environment` private memory as the canonical per-host config record; its total absence here is the strongest available signal.
- Companion, unrelated-but-adjacent finding from the same session: `workspace(activate)` flags a legacy `.codescout/embeddings/project.db` needing `codescout migrate-memories` — a second sign this machine's embedding setup was never fully brought up to date after being provisioned.
