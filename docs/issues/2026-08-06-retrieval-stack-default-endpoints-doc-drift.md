---
kind: bug
status: fixed
tags:
- docs
- retrieval
- config
closed: 2026-08-14
opened: 2026-08-06
owner: marius
related: []
severity: medium
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

Fixed 2026-08-14 on `experiments`. **The source-of-truth question the Resume left
open answers itself on evidence** — three surfaces already agreed and only the
runtime disagreed:

- `docker-compose.yml:81,144,211` publishes `127.0.0.1:48081`, `:48084`, `:48083`
- `.env.example:14-16` uses those same three
- `docs/manual/src/concepts/retrieval-stack.md` uses them throughout (13 hits)

`8081/8084/8083` are **container-internal** ports (`8080`, `80`, `8080` behind the
published mappings). Nothing listens on them from the host, ever. So the docs were
right and the runtime was wrong — the opposite of the direction the bug title
implies.

**The bug named two drifted surfaces. There were five.**

| # | Surface | Was | Now |
|---|---|---|---|
| 1 | `src/retrieval/config.rs` sparse fallback | `:8084` | `DEFAULT_SPARSE_EMBEDDER_URL` = `:48084` |
| 2 | `src/retrieval/config.rs` reranker fallback | `:8083` | `DEFAULT_RERANKER_URL` = `:48083` |
| 3 | `scripts/sweep-bm25-boost.sh:10-12` | `:8081/:8084/:8083` | `:48081/:48084/:48083` |
| 4 | manual env table | `CODESCOUT_SPARSE_URL` | `CODESCOUT_SPARSE_EMBEDDER_URL` |
| 5 | manual env table | `CODESCOUT_EMBEDDER_URL` default `:48081` | *(unset)* — the default was deliberately removed |

Surfaces 4 and 5 are worse than the port drift and were found only by checking the
doc side rather than assuming it:

- **4** — `CODESCOUT_SPARSE_URL` occurs **exactly once in the whole repository**:
  that table row. Nothing reads it. Code, tests, `.env.example`,
  `contrib/pi/mcp.json.example`, both benchmark trackers and
  `semantic_search.rs`'s own error hint all use `CODESCOUT_SPARSE_EMBEDDER_URL`. A
  reader configuring sparse from the manual set a variable that configures
  nothing, and got **no error at all** — strictly worse than the port bug, which at
  least produced a connect failure.
- **5** — the dense row documented a default that had already been removed on
  purpose (`config.rs:29-30`: *"Previously defaulted to `http://127.0.0.1:8081`,
  which fabricated a server that may never have existed"*). The doc was
  advertising the exact fabrication the code had stopped doing.

**The dense half of this bug was already fixed before this session** — unset now
means "resolve the backend from the model", guarded by
`unset_everything_no_longer_fabricates_anything`. Sparse and reranker were left
behind in the same commit that fixed dense, including in the test (see below).
A half-applied fix, not a stale bug.

The family-split documentation this bug's Resume also asked for landed as
`### Three embedder config families, and they are not the same one` in the same
manual section, covering all three spellings (`CODESCOUT_EMBEDDER_*`,
`CODESCOUT_EMBED_*`, `LIBRARIAN_EMBED_*`), their consumers, their precedence, and
the non-fatal-failure property that let this hide.

Not fixed here, filed separately: the machine-specific dead absolute paths in
`scripts/sweep-bm25-boost.sh` and `scripts/sweep-bm25-cr1200.sh` (both point into
the `code-explorer` root removed in task #45). Different defect class, same files.
## Tests added

**`config::default_port_tests::retrieval_default_ports_match_published_compose_ports`**
(`src/retrieval/config.rs`) — parses the `- "127.0.0.1:<host>:<container>"` mappings
out of the real `docker-compose.yml` and asserts each default's port is one the
compose file actually publishes. Deliberately parsed rather than restated: a
constant duplicated into its own test is a constant the test can no longer
disagree with. It also asserts the parse found *something*, so a change to the
compose port syntax fails loudly instead of degrading the guard to a no-op.

**`config::default_port_tests::sparse_and_reranker_defaults_are_distinct`** — catches
the copy-paste that gives both constants the same port.

**`config_from_env_uses_defaults_when_unset`** (`tests/retrieval_unit.rs:28`) was
*already asserting the bug*: `assert_eq!(cfg.sparse_embedder_url,
"http://127.0.0.1:8084")`. It was the single failure when the fix landed, which is
the correct outcome — but note what it means. This test ran green for the entire
life of the bug **because it was pinning it**. Its dense assertion had been updated
when dense was fixed; the two lines below it were not. A test updated in half
measures the half it was updated for and defends the other half against repair.

Gate at fix: **3707 passed / 0 failed / 44 ignored**, `clippy --all-targets -D
warnings` clean. 3707 = 3705 at the prior session close + the 2 tests above,
reconciling exactly.
## Workarounds
Set explicit env vars for the running MCP process (`CODESCOUT_EMBEDDER_URL`, `CODESCOUT_SPARSE_EMBEDDER_URL`, `CODESCOUT_RERANKER_URL`) rather than relying on defaults.

## Resume

N/A — fixed and verified. The source-of-truth question is settled (compose
publishes the host ports; the runtime was wrong) and the family-split doc is
written. Do not re-open to "align the docs" — the docs were already right.
## References
- `src/retrieval/config.rs`
- `docs/manual/src/concepts/retrieval-stack.md`
