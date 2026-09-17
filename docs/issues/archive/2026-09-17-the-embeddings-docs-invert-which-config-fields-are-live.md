---
id: 5de82c05bd449fdc
kind: bug
status: fixed
title: 'BUG: the embeddings docs invert which config fields are live'
tags:
- cluster/doc-contradicted-by-code
- embeddings
- docs
- config
claimed_at: 2026-09-17
claimed_by: 458a8a26-c380-4f5b-b967-2181f592917e
---

## Summary

`docs/manual/src/configuration/embeddings.md` tells readers that of the three
`[embeddings]` fields, only `model` is honoured and `url` / `api_key` are dead.
The truth is the exact inverse: `url` and `api_key` both work end-to-end, and
`model` is the one silently discarded on the url path. A reader who follows the
page configures the one field that does nothing and avoids the two that work.
Two further claims on neighbouring pages are also false in the same direction.

## Symptom (Effect)

`docs/manual/src/configuration/embeddings.md`, banner:

```
> **⚠ This page describes the pre-v0.12 single-service embedding model and is
> being phased out.** ... The `[embeddings]` config block still loads but only
> the `model = "local:..."` path is honoured — and only when the binary was
> built with the `local-embed` Cargo feature.
>
> **If you are upgrading from <v0.12:** the `model` / `url` / `api_key`
> fields in `project.toml` no longer drive search.
```

Measured behaviour on that exact block, 2026-09-17, against a logging
`/v1/embeddings` server:

| field | page says | measured |
|---|---|---|
| `[embeddings].url` | does not drive search | **drives search** — selects the endpoint |
| `[embeddings].api_key` | does not drive search | **drives search** — arrives as `Bearer KEY-FROM-PROJECT-TOML` |
| `[embeddings].model` | the only honoured field | **discarded whenever `url` is set** |

The banner is simultaneously the most prominent text on the page and wrong in
both directions.

## Reproduction

Tree `35b622f8f53cb61fa301733403f3a325fcf17199`. Set all three fields in
`.codescout/project.toml`, point `url` at a server that logs request bodies, run
`codescout index --force` with `CODESCOUT_ENV_FILE` neutralised. Observed wire:

```json
{"path": "/v1/embeddings", "model": "M", "auth": "Bearer KEY-FROM-PROJECT-TOML"}
```

— where `M` came from `CODESCOUT_EMBEDDER_MODEL_NAME` and `[embeddings].model`
was `DISTINCTIVE-MODEL-FROM-PROJECT-TOML`.

## Environment

Linux 7.2.4-zen2-1-zen, `experiments`, release binary built by `cargo rb`.

## Root cause

Doc-vs-code drift accumulated across the v0.12 retrieval-stack migration. The
page was updated to warn that `[embeddings]` was superseded, at a moment when
that was true; `url` and `api_key` were subsequently re-wired into the stack
path (`merge_embed_config`, `src/retrieval/config.rs:304-327`;
`RetrievalClient::guarded_api_key`, `src/retrieval/client.rs:209-223`) and the
warning was never revisited. `model` went the other way and was never re-wired.

Two more claims in the same family:

1. `docs/manual/src/configuration/global-config.md:47-52` — *"There is no
   deep-merge within nested tables — if a project config sets `[embeddings]`,
   the entire `[embeddings]` table from the global config is replaced, not
   merged field-by-field."* `merge_toml` (`src/config/project.rs:461-476`)
   recurses, and `merge_toml_base_fills_missing_key` (`:1222-1228`) pins
   field-by-field merging of `[embeddings]` specifically. The doc states the
   opposite of a behaviour a test guards — and it is the behaviour users
   actually want, so the page discourages the working pattern.
2. `src/config/project.rs:82` — the `EmbeddingsSection::model` doc comment opens
   `"ollama:<model>" → Ollama local daemon (default)`. The default is
   `local:AllMiniLML6V2Q` (`default_embed_model`, `:389-391`), which the same
   comment also marks `**default**` eight lines later. Two conflicting
   `(default)` markers in one doc block.

**Measured 2026-09-17** by the wire captures above and by reading
`merge_toml_base_fills_missing_key`; the doc claims were read at the cited lines
in tree `35b622f8`.

## Evidence

### api_key from project.toml reaches the wire

```json
{"path": "/v1/embeddings", "model": "M", "auth": "Bearer KEY-FROM-PROJECT-TOML"}
```

Run with no `EMBED_API_KEY` in the environment and `api_key` set only in
`.codescout/project.toml`.

### The only page documenting the working api_key path disclaims itself

Occurrence counts across the four embedding-related manual pages, 2026-09-17:

```
embedding-backends.md    api_key=0  EMBED_API_KEY=2  OPENAI_API_KEY=3
embeddings.md            api_key=4  EMBED_API_KEY=4  OPENAI_API_KEY=2
global-config.md         api_key=0  EMBED_API_KEY=0  OPENAI_API_KEY=0
retrieval-stack.md       api_key=0  EMBED_API_KEY=1  OPENAI_API_KEY=1
```

`embeddings.md` is the only page that documents `[embeddings].api_key` at all —
and it is the page whose banner says to treat its contents as historical. The
working authentication path is documented exclusively on the surface that
disavows it.

## Hypotheses tried

1. **Hypothesis:** the banner is stale but harmless, since the stack is
   env-configured anyway.
   **Test:** followed the page's own advice on a fresh project.
   **Verdict:** rejected — the advice is actively harmful. It steers a user to
   set `model` (inert on the url path) and to avoid `url`/`api_key` (the two
   that work), which is precisely the configuration that fails with
   *"embedding model name is required and was empty or blank"*.

## Fix

Documentation-only, sequenced **after** Tasks 1–3 so the pages were rewritten once
against settled behaviour rather than twice.

The filed scope was three items. Reading the pages against the code found **nine**, and
the two worst were not in the report — both are surfaces that tell a reader where to put
something, or hand them syntax to paste:

**`docs/manual/src/configuration/embeddings.md`**

1. The banner claimed `[embeddings]` was superseded and that `model` was the only live
   field. Inverted in both directions; replaced with the real two-place ladder, and it
   now states what it used to say, since a reader may remember the old claim.
2. § *Environment Variables* listed three variables. It now lists ten, says every one
   **overrides both config layers**, names `CODESCOUT_EMBEDDER_MODEL_NAME` explicitly
   (the one that decides the wire name when a `url` is set), and warns that
   `~/.config/codescout/.env` is read into the environment at startup — so values there
   behave as an override of every project rather than a default beneath them.

**`docs/manual/src/configuration/global-config.md`** — five errors on one short page:

3. **The "File locations" table named the project file `.codescout/config.toml`.**
   Nothing reads that path; the file is `.codescout/project.toml`. A reader following
   the table got no effect and no warning. Verified by grep: no reader exists.
4. The intro repeated the same wrong filename.
5. § *Merge semantics* said tables do **not** deep-merge — the opposite of `merge_toml`,
   which recurses, and of the test that pins it. It discouraged the exact pattern the
   two layers exist for. Rewritten with a worked example.
6. The `[embeddings]` example used a bare HuggingFace repo id, which resolves to nothing.
7. The same example set `chunk_size`, an inert key (`CODESCOUT_CHUNK_TARGET` is live).
8. § *Load behaviour* claimed a 64 KB file-size guard. The code has always enforced
   1 MiB (`src/config/global.rs`). Also adds the unknown-key-is-dropped-silently note,
   since that is the residual after the shared type landed.

**`docs/manual/src/configuration/embedding-backends.md` and
`docs/manual/src/semantic-search-guide.md`**

9. **Both presented `custom:<model>@<url>` as a live backend** — an entire section with
   four copy-pasteable provider examples in one, a table row in the other. That prefix
   was **removed** and now hard-errors with a migration message. Every example failed.
   Rewritten to the `url` + `model` pair. `embeddings.md` § *Migration* had documented
   the removal correctly all along, which is what makes this a drift between surfaces
   rather than an unknown.

The API-key half of the report is closed by the above rather than by a new page: the
only surface documenting `[embeddings].api_key` was `embeddings.md`, whose banner told
readers to treat it as historical. That banner is gone, and `embedding-backends.md`
gained an § *Authentication* section naming both the config field and `EMBED_API_KEY`,
which layer wins, and the HTTPS-or-loopback drop.

Item 3 of the original report — the duplicated `(default)` marker at
`src/config/project.rs:82` — was already fixed in Task 2, which rewrote that doc block.

- **SHA:** `c76d43de37b828a5b4a946d56a6037a9fce96e63` (on `experiments`)
- **patch-id:** `d32bb8ad57542e3b0577e4ba9ce14c25fb63dd70`

## Tests added

None, and the reason is worth stating rather than leaving as a blank.

Pinning prose reds on every rewording, which `CLAUDE.md` § *Testing Discipline* rightly
warns against. What is cheaply assertable — and what actually failed here — is whether a
doc's claim and the code's behaviour contradict. Two of the nine errors were refuted by
tests that already existed and were simply never read against the prose:
`merge_toml_base_fills_missing_key` refutes the "no deep-merge" claim, and the `custom:`
bail in `create_embedder_with_config` refutes two pages of examples.

So the honest statement is: **this class is caught by `librarian(action="audit_doc_refs")`
only for path-shaped tokens**, and every error above except the wrong filename was prose
that no linter reaches. Run after this change: `exit_code=0`, zero `high` findings across
the four edited pages.

The one mechanical guard that would have caught error 3 — the wrong project filename — is
that `audit_doc_refs` treats `.codescout/config.toml` as a user-created path and
explicitly ignores it (`audit-doc-refs:ignore` comments at the top of that page, with the
reason "a clean checkout has no .codescout/config.toml, and that is the normal state, not
drift"). The suppression is correct for its stated purpose and is exactly what let the
wrong name sit there — a waiver written for absence covering a **misspelling**. Left in
place; noted here because the next person to widen that lint should know it.

## Workarounds

Ignore `embeddings.md`'s banner. Set `url` and `api_key` in
`.codescout/project.toml`; set the model via `CODESCOUT_EMBEDDER_MODEL_NAME`
until the model-discard bug is fixed.

## Resume

N/A — fixed and verified.

One thing knowingly left: the `CODESCOUT_EMBED_*` vs `CODESCOUT_EMBEDDER_*` duplication
is now *documented* as history rather than design, but not yet removed. That is Task 5 of
`docs/plans/2026-09-17-embedding-config-consolidation.md`, which will make these pages
need one more pass — a smaller one, since the ladder itself is now stated correctly.

## References

- `docs/manual/src/configuration/embeddings.md` (banner)
- `docs/manual/src/configuration/global-config.md:31-32`, `:47-52`
- `src/config/project.rs:82`, `:389-391`, `:461-476`, `:1222-1228`
- `src/retrieval/config.rs:304-327`; `src/retrieval/client.rs:209-223`
