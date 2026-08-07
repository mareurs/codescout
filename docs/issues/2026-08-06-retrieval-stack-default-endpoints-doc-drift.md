---
status: open
opened: 2026-08-06
closed:
severity: medium
owner: marius
related: []
tags: [docs, retrieval, config]
kind: bug
---

# BUG: retrieval-stack docs default endpoints drift from runtime defaults

## Summary
The retrieval-stack documentation says default dense/sparse/reranker endpoints are `:48081/:48084/:48083`, but runtime fallback code uses `:8081/:8084/:8083` when env vars are unset. This mismatch leads operators to expect the wrong defaults during setup and troubleshooting.

## Symptom (Effect)
`semantic_search` fails with default runtime fallbacks to `:8081`:

```text
stack search failed: dense embed connect failed: http://127.0.0.1:8081/v1/embeddings
```

## Reproduction
1. Ensure no `CODESCOUT_EMBEDDER_URL` is set in the process environment.
2. Call `semantic_search`.
3. Observe connection attempt to `http://127.0.0.1:8081/v1/embeddings`.
4. Compare with docs/manual/src/concepts/retrieval-stack.md section "How codescout finds the stack", which states default `CODESCOUT_EMBEDDER_URL` is `http://127.0.0.1:48081`.

## Environment
Windows 11, codescout repo, local MCP session.

## Root cause
Doc/runtime drift:
- Runtime defaults in `src/retrieval/config.rs` use `8081/8084/8083`.
- Documentation in `docs/manual/src/concepts/retrieval-stack.md` states `48081/48084/48083`.

## Evidence
1. Runtime fallback source:
   - `src/retrieval/config.rs` (`RetrievalConfig::from_env`) sets:
     - `CODESCOUT_EMBEDDER_URL` default `http://127.0.0.1:8081`
     - `CODESCOUT_SPARSE_EMBEDDER_URL` default `http://127.0.0.1:8084`
     - `CODESCOUT_RERANKER_URL` default `http://127.0.0.1:8083`
2. Docs claim:
   - `docs/manual/src/concepts/retrieval-stack.md` section "How codescout finds the stack" lists `48081/48084/48083`.

## Hypotheses tried
1. **Hypothesis:** Runtime reads repo `.env` defaults implicitly.
   - **Test:** Checked active process env in shell (`set CODESCOUT_`) and confirmed none set.
   - **Verdict:** rejected.
   - **Evidence link:** shell output in session.

## Fix
N/A — not implemented in this session.

## Tests added
N/A — docs/config alignment check not implemented yet.

## Workarounds
Set explicit env vars for the running MCP process (`CODESCOUT_EMBEDDER_URL`, `CODESCOUT_SPARSE_EMBEDDER_URL`, `CODESCOUT_RERANKER_URL`) rather than relying on defaults.

## Resume

Decide source of truth (runtime vs docs), then align one to the other and add a
regression check that docs-stated defaults match `RetrievalConfig::from_env`
fallbacks.

**Still open and confirmed live 2026-08-07.** `memory(action="write")` now
reports it on every call:

```
cross-embed failed: dense embed connect failed: http://127.0.0.1:8081/v1/embeddings
semantic anchor creation failed: ... (same)
```

So the blast radius is wider than `semantic_search`: semantic memory
cross-embedding and anchor creation degrade too. Both fail non-fatally — the
topic write succeeds — which is why this has gone unnoticed.

**Finding that belongs with this bug: there are two independent embedder config
families, and only one is wired up on this machine.**

| Family | Consumer | State on this box |
|---|---|---|
| `CODESCOUT_EMBEDDER_URL` / `_SPARSE_` / `_RERANKER_` | retrieval stack: code `semantic_search`, semantic memory | **unset** → falls back to `127.0.0.1:8081`, nothing listening |
| `LIBRARIAN_EMBED_URL` / `_MODEL` / `_API_KEY` | librarian artifact index | configured → Azure, working (fixed 2026-08-07) |

The names are near-identical and the docs discuss them as if one subsystem, so
"the embedder is configured" can be true and false simultaneously depending on
which one is meant. Fixing the `LIBRARIAN_EMBED_*` 404 earlier today did not
touch this path, and the `CODESCOUT_EMBED_URL` key read by
`ProjectConfig::load_or_default` (`src/config/project.rs:505`) is a *third*
spelling again.

Suggested scope when this is picked up: align the doc/runtime defaults as
originally planned, **and** document the family split explicitly — a reader who
has configured one has no signal that the other exists. Pointing
`CODESCOUT_EMBEDDER_URL` at the same Azure `/openai` base that
`LIBRARIAN_EMBED_URL` now uses would likely resolve the runtime failure, but the
dimension implications (`CODESCOUT_EMBED_DIMENSIONS=768` vs the model's native
3072) need checking first — see the `artifact_vec` / Qdrant rebuild in
`docs/trackers/windows-shell-env-session-log.md` for how that bit last time.
## References
- `src/retrieval/config.rs`
- `docs/manual/src/concepts/retrieval-stack.md`
